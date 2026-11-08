//! 흐름식 → 순수 SQLite 스칼라 식(C# `Storage/FlowCompiler.cs`, docs/column-management.md §11.3).
//!
//! 앱 전용 함수가 없어 외부 읽기 연결에서도 같은 값이 나온다. 바깥 행은 별칭 `현재` 로 읽는다 — 정정
//! (`field_override`)을 마친 면 뷰의 값이다.
//!
//! 출력기가 둘이다. [`Target::Internal`] 은 뷰에 들어가는 식으로, 품목을 **내부 표의 정확한 차수**에서 읽고 연결
//! 문서는 그 개체의 덮개 식을 다시 불러 읽는다 — 공개 뷰를 읽지 않으므로 뷰 사이 순환이 생기지 않는다.
//! [`Target::Public`] 은 「SQL로 꺼내기」로, 같은 단계를 계약면 뷰 위에 옮겨 쓴다. 둘은 최신 차수에서 같다.
//!
//! **글이 C# 과 바이트까지 같아야 한다** — 이 식이 뷰 정의(`sqlite_master.sql`)에 그대로 박힌다. 수 글자는 .NET 의
//! 표기(`Num`·`JsNumber`)를 따른다.

use serde_json::Value;

use crate::error::{Error, Result};
use crate::storage::flow::{Flow, FlowClause, FlowDefinition, FlowFormula, FlowSource, FlowStep};
use crate::storage::flow_check::{self, FlowEnv};
use crate::storage::{computed_column, notice_groups, views};
use crate::text::{dotnet_double_integer, dotnet_double_r, raw};

/// `FlowCompiler.Target`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Internal,
    Public,
}

/// `FlowCompiler.Part` — 앞부분 컴파일의 결과. 목록·행이면 `sql` 이 부질의, 값·수·날짜면 스칼라 식이다.
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    pub shape: &'static str,
    pub sql: String,
    pub fields: Vec<String>,
}

impl Part {
    /// `Part.IsRelation`.
    pub fn is_relation(&self) -> bool {
        matches!(self.shape, "rows" | "values" | "row")
    }
}

/// `FlowCompiler.Self` — 바깥 행의 별칭(§5).
pub const SELF: &str = "현재";

/// `FlowCompiler.Spaces` — JS 의 `trim` 이 걷는 공백(전각 공백 U+3000 이 문서에 흔하다).
const SPACES: &str = "char(9,10,11,12,13,32,160,5760,8192,8193,8194,8195,8196,8197,8198,8199,8200,8201,8202,8232,8233,8239,8287,12288,65279)";

// ── 겉 ─────────────────────────────────────────────

/// `FlowCompiler.Expression` — 뷰의 한 칸이 될 식. 꼴 검사를 지나지 못하면 단계 번호를 담아 거절한다(「3단계: …」).
/// `env` 가 없으면 [`Flow::env_for`] — 맞춤 뷰는 [`Flow::env_for_view`] 를 준다(그때 `entity` 는 쓰지 않는다).
pub fn expression(
    entity: &str,
    formula: &FlowFormula,
    target: Target,
    env: Option<&FlowEnv>,
) -> Result<String> {
    let flow = match (&formula.source, &formula.flow) {
        (Some(source), Some(flow)) if source == "flow" => flow,
        _ => return Err(Error::invalid("SQL 가공열은 아직 받지 않습니다.")),
    };
    let part = prefix_inner(entity, flow, step_count(flow), target, true, env)?;
    Ok(format!("coalesce({}, '')", render(&part)?))
}

/// `FlowCompiler.Eject` — 「SQL로 꺼내기」: 계약면 뷰만 읽는 스칼라 부질의 하나(§5).
pub fn eject(entity: &str, formula: &FlowFormula, env: Option<&FlowEnv>) -> Result<String> {
    let expression = expression(entity, formula, Target::Public, env)?;
    let steps = formula.flow.as_ref().map(steps_of).unwrap_or_default();
    let notes: Vec<String> = steps
        .iter()
        .enumerate()
        .map(|(i, step)| {
            let op = step.op.as_deref().unwrap_or("");
            // C# `Flow.Verbs.First(v => v.Op == step.Op)` — 꼴 검사를 지난 식이라 모르는 동사가 없다.
            let word = flow_check::verb_of(op).map_or("", |v| v.word);
            format!("  -- {}단계: {word}", i + 1)
        })
        .collect();
    let notes = notes.join("\n");
    Ok(format!(
        "SELECT\n{}  {} AS 결과",
        if notes.is_empty() {
            String::new()
        } else {
            notes + "\n"
        },
        indent_lines(&expression)
    ))
}

/// `FlowCompiler.FinalShape` — 끝 꼴(`value`·`number`·`date`). 검사를 지나지 못하면 거절한다. SQL 열은 `value`.
pub fn final_shape(
    entity: &str,
    formula: &FlowFormula,
    env: Option<&FlowEnv>,
) -> Result<&'static str> {
    let flow = match (&formula.source, &formula.flow) {
        (Some(source), Some(flow)) if source == "flow" => flow,
        _ => return Ok("value"),
    };
    let owned;
    let env = match env {
        Some(env) => env,
        None => {
            owned = Flow::env_for(entity)?;
            &owned
        }
    };
    let checked = Flow::check(flow, env);
    if !checked.ok() {
        return Err(Error::InvalidOperation(checked.errors[0].describe()));
    }
    Ok(checked.r#final.expect("검사를 지난 식은 끝 꼴이 있다"))
}

/// `FlowCompiler.Prefix` — 앞부분 컴파일: 단계 `steps` 개까지만 잘라 그 시점의 값을 낸다(§11.3, 보표가 쓴다).
/// 0 이면 원천이다. 그 앞에 꼴 오류가 있으면 단계 번호를 담아 거절한다.
pub fn prefix(
    entity: &str,
    flow: &FlowDefinition,
    steps: usize,
    target: Target,
    env: Option<&FlowEnv>,
) -> Result<Part> {
    prefix_inner(entity, flow, steps, target, false, env)
}

fn steps_of(flow: &FlowDefinition) -> Vec<&FlowStep> {
    flow.steps.iter().flatten().flatten().collect()
}

fn step_count(flow: &FlowDefinition) -> usize {
    flow.steps.as_ref().map_or(0, Vec::len)
}

fn prefix_inner(
    entity: &str,
    flow: &FlowDefinition,
    steps: usize,
    target: Target,
    require_end: bool,
    env: Option<&FlowEnv>,
) -> Result<Part> {
    if steps > step_count(flow) {
        return Err(Error::Argument(
            "Specified argument was out of the range of valid values. (Parameter 'steps')".into(),
        ));
    }
    let owned;
    let env = match env {
        Some(env) => env,
        None => {
            owned = Flow::env_for(entity)?;
            &owned
        }
    };
    let checked = Flow::check(flow, env);
    if let Some(blocking) = checked
        .errors
        .iter()
        .find(|e| e.index <= steps && (require_end || !e.r#final))
    {
        return Err(Error::InvalidOperation(blocking.describe()));
    }

    let sources = Sources {
        entity,
        target,
        env,
    };
    let empty = FlowSource::default();
    let mut part = sources.from(flow.from.as_ref().unwrap_or(&empty), &checked.fields[0])?;
    let all = steps_of(flow);
    for (i, step) in all.iter().take(steps).enumerate() {
        let shape = checked.shapes[i + 1].expect("검사를 지난 단계는 꼴이 있다");
        part = apply(step, part, shape, &checked.fields[i + 1], &|f| {
            sources.row_column(f)
        })?;
    }
    Ok(part)
}

/// `FlowCompiler.Render` — 끝 값을 열에 찍힐 글로(§4.6). 목록·행이면 쓸 수 없다.
pub fn render(part: &Part) -> Result<String> {
    Ok(match part.shape {
        "value" => part.sql.clone(),
        "number" => raw(
            r#"
            (SELECT CASE WHEN 원값 IS NULL THEN '' ELSE (
                SELECT CASE WHEN 정밀값 = CAST(정밀값 AS INTEGER) THEN printf('%,d', CAST(정밀값 AS INTEGER)) ELSE printf('%.15g', 정밀값) END
                FROM (SELECT CAST(printf('%.15g', 원값) AS REAL) AS 정밀값)) END
             FROM (SELECT {part.Sql} AS 원값))
            "#,
            &[("part.Sql", &part.sql)],
        ),
        "date" => format!("replace({}, '-', '/')", part.sql),
        other => {
            return Err(Error::InvalidOperation(format!(
                "마지막 결과가 {}입니다. 값·수·날짜 중 하나로 끝나야 저장할 수 있습니다",
                flow_check::shape_word(other)
            )));
        }
    })
}

// ── 원천 ─────────────────────────────────────────────

/// `FlowCompiler.Sources` — 원천을 읽는 길. 출력기 둘이 갈리는 곳은 여기뿐이다 — 단계는 같은 SQL 을 쌓는다.
///
/// 맞춤 뷰(`env.view`)면 바깥 행이 `현재` 하나가 아니라 같은 조달 건의 세 문서다(docs/user-views.md §5). 뷰가 면 뷰를
/// 별칭 `접수`·`공고`·`계약` 으로 걸어 두므로 연결 문서를 찾아가지 않고 그 별칭을 바로 읽는다.
struct Sources<'a> {
    entity: &'a str,
    target: Target,
    env: &'a FlowEnv,
}

/// `Sources.Faces` — (키, 본번호, 면 뷰, 품목 뷰).
fn faces(kind: &str) -> Result<(&'static str, &'static str, &'static str, &'static str)> {
    Ok(match kind {
        "notice" => ("입찰공고번호", "공고본번호", "v_공고", "v_공고품목"),
        "contract" => ("계약번호", "계약본번호", "v_계약", "v_계약품목"),
        "request" => ("접수번호", "접수본번호", "v_접수", "v_접수품목"),
        // C# 사전의 KeyNotFoundException — 종류는 꼴 검사가 먼저 거른다.
        _ => {
            return Err(Error::invalid(format!(
                "The given key '{kind}' was not present in the dictionary."
            )));
        }
    })
}

/// `Sources.Own` — 바깥 행의 열.
fn own(column: &str) -> String {
    format!("{SELF}.{}", ident(column))
}

/// `Sources.Doc` — 맞춤 뷰의 바깥 행 별칭(그 문서의 면 뷰 한 줄, 없으면 NULL).
fn doc(kind: &str) -> Result<&'static str> {
    flow_check::doc_words(kind).ok_or_else(|| {
        Error::invalid(format!(
            "The given key '{kind}' was not present in the dictionary."
        ))
    })
}

impl Sources<'_> {
    /// `Sources.From`.
    fn from(&self, from: &FlowSource, fields: &[String]) -> Result<Part> {
        let fields = fields.to_vec();
        Ok(match from.kind.as_deref() {
            Some("items") => Part {
                shape: "rows",
                sql: self.items()?,
                fields,
            },
            Some("field") => scalar("value", own(from.field.as_deref().unwrap_or(""))),
            Some("text") => scalar("value", lit(from.text.as_deref().unwrap_or(""))),
            Some("number") => scalar("number", num(from.value.unwrap_or(0.0))),
            Some("linked") => self.linked(from, fields)?,
            other => {
                return Err(Error::InvalidOperation(format!(
                    "알 수 없는 원천: {}",
                    other.unwrap_or("")
                )));
            }
        })
    }

    /// `Sources.RowColumn` — 단계가 읽는 행의 열(`비면`·사칙·`날짜차이`). 가공열은 `현재` 의 열이고, 맞춤 뷰는
    /// `공고.공고명` 꼴을 그 문서의 별칭으로 읽는다 — 문서가 없으면 빈 글이다.
    fn row_column(&self, field: &str) -> Result<String> {
        if !self.env.view {
            return Ok(own(field));
        }
        let (kind, column) = flow_check::qualified(field)
            .ok_or_else(|| Error::InvalidOperation(format!("문서를 붙여 적으세요: {field}")))?;
        Ok(format!("coalesce({}.{}, '')", doc(kind)?, ident(column)))
    }

    fn linked(&self, from: &FlowSource, fields: Vec<String>) -> Result<Part> {
        let kind = from.link.as_deref().unwrap_or("");
        if self.env.view {
            return self.document(kind, from, fields);
        }
        if from.items == Some(true) {
            return Ok(Part {
                shape: "rows",
                sql: self.linked_items(kind)?,
                fields,
            });
        }
        let many = Flow::env_for(self.entity)?
            .link(kind)
            .ok_or_else(|| {
                Error::invalid(format!(
                    "The given key '{kind}' was not present in the dictionary."
                ))
            })?
            .many;
        let rows = self.linked_rows(kind)?;
        let Some(field) = from.field.as_deref() else {
            return Ok(Part {
                shape: if many { "rows" } else { "row" },
                sql: rows,
                fields,
            });
        };
        Ok(scalar(
            "value",
            format!("coalesce((SELECT {} FROM ({rows})), '')", ident(field)),
        ))
    }

    /// `Sources.Document` — 맞춤 뷰의 문서 하나. 별칭을 바로 읽는다.
    fn document(&self, kind: &str, from: &FlowSource, fields: Vec<String>) -> Result<Part> {
        let doc = doc(kind)?;
        let (key, base, _, item_view) = faces(kind)?;
        if from.items == Some(true) {
            let sql = if self.target == Target::Internal {
                computed_column::source_sql(
                    kind,
                    &format!("{doc}.{}", ident(base)),
                    &format!("{doc}.{}", ident("차수")),
                )?
            } else {
                format!(
                    "SELECT 순번 AS __order, {} FROM {item_view} WHERE {key} = {doc}.{}",
                    columns(&computed_column::field_names(kind)?),
                    ident(key)
                )
            };
            return Ok(Part {
                shape: "rows",
                sql,
                fields,
            });
        }
        if let Some(field) = from.field.as_deref() {
            return Ok(scalar(
                "value",
                format!("coalesce({doc}.{}, '')", ident(field)),
            ));
        }
        let columns = fields
            .iter()
            .map(|f| format!("coalesce({doc}.{}, '') AS {}", ident(f), ident(f)))
            .collect::<Vec<_>>()
            .join(", ");
        Ok(Part {
            shape: "row",
            sql: format!(
                "SELECT 0 AS __order, {columns} WHERE {doc}.{} IS NOT NULL",
                ident(base)
            ),
            fields,
        })
    }

    fn items(&self) -> Result<String> {
        let (key, base, _, item_view) = faces(self.entity)?;
        if self.target == Target::Internal {
            return computed_column::source_sql(self.entity, &own(base), &own("차수"));
        }
        Ok(format!(
            "SELECT 순번 AS __order, {} FROM {item_view} WHERE {key} = {}",
            columns(&computed_column::field_names(self.entity)?),
            own(key)
        ))
    }

    // ── 안쪽: 내부 표 + 덮개 식 ──

    /// `Sources.Group` — 이 행이 속한 공고건. 연결의 끝점은 건이다(스키마 V15).
    fn group(&self) -> String {
        match self.entity {
            "contract" => format!(
                "(SELECT pl.notice_group FROM project_link pl WHERE pl.contract_base = {})",
                own("계약본번호")
            ),
            "notice" => format!(
                "(SELECT s.group_base FROM notice_series s WHERE s.notice_base = {})",
                own("공고본번호")
            ),
            _ => format!(
                "(SELECT rl.notice_group FROM request_link rl WHERE rl.request_base = {})",
                own("접수본번호")
            ),
        }
    }

    fn request_base(&self) -> String {
        format!(
            "(SELECT rl.request_base FROM request_link rl WHERE rl.notice_group = {})",
            self.group()
        )
    }

    fn linked_rows(&self, kind: &str) -> Result<String> {
        if self.target == Target::Public {
            return self.public_rows(kind);
        }
        let group = self.group();
        Ok(match kind {
            // 건의 현행 공고 — NoticeGroups.현행공고 하나가 정본이다.
            "notice" => raw(
                r#"
                    SELECT 0 AS __order, {face}
                    FROM notice n
                    JOIN ({current}) 현행 ON 현행.notice_base = n.notice_base AND 현행.seq = n.seq
                    WHERE 현행.group_base = {group}
                    "#,
                &[
                    ("face", &views::face("notice")?),
                    ("current", &notice_groups::current_notice()),
                    ("group", &group),
                ],
            ),
            "request" => raw(
                r#"
                    SELECT 0 AS __order, {face}
                    FROM request r
                    WHERE r.request_base = {base}
                      AND CAST(r.seq AS INTEGER) = (SELECT max(CAST(x.seq AS INTEGER)) FROM request x WHERE x.request_base = r.request_base)
                    "#,
                &[
                    ("face", &views::face("request")?),
                    ("base", &self.request_base()),
                ],
            ),
            _ => raw(
                r#"
                    SELECT c.contract_base AS __order, {face}
                    FROM contract c
                    LEFT JOIN counterparty p ON c.counterparty_number = p.business_number
                    WHERE c.contract_base IN (SELECT pl.contract_base FROM project_link pl WHERE pl.notice_group = {group})
                      AND CAST(c.seq AS INTEGER) = (SELECT max(CAST(x.seq AS INTEGER)) FROM contract x WHERE x.contract_base = c.contract_base)
                    "#,
                &[("face", &views::face("contract")?), ("group", &group)],
            ),
        })
    }

    fn linked_items(&self, kind: &str) -> Result<String> {
        if self.target == Target::Public {
            let (key, _, _, item_view) = faces(kind)?;
            return Ok(format!(
                "SELECT 순번 AS __order, {} FROM {item_view} WHERE {key} = ({})",
                columns(&computed_column::field_names(kind)?),
                self.public_key(kind)?
            ));
        }
        match kind {
            "notice" => {
                let current = notice_groups::current_notice();
                let group = self.group();
                computed_column::source_sql(
                    "notice",
                    &format!(
                        "(SELECT 현행.notice_base FROM ({current}) 현행 WHERE 현행.group_base = {group})"
                    ),
                    &format!(
                        "(SELECT 현행.seq FROM ({current}) 현행 WHERE 현행.group_base = {group})"
                    ),
                )
            }
            "request" => {
                let base = self.request_base();
                computed_column::source_sql(
                    "request",
                    &base,
                    &format!(
                        "(SELECT x.seq FROM request x WHERE x.request_base = {base} ORDER BY CAST(x.seq AS INTEGER) DESC LIMIT 1)"
                    ),
                )
            }
            _ => Err(Error::invalid(
                "연결계약은 여러 건이라 품목을 바로 읽을 수 없습니다. 한 건을 먼저 고르세요.",
            )),
        }
    }

    // ── 바깥: 계약면 뷰 ──

    /// `Sources.PublicKey` — 연결 문서의 키를 계약면 뷰로 찾는다. 계약 줄과 접수 줄은 `v_통합` 이 건의 현행 공고에
    /// 잇고, 공고 줄은 그 공고 문서가 몇 번째 공고든 건에 잇기 위해 `v_통합차수` 를 본다.
    fn public_key(&self, kind: &str) -> Result<String> {
        let own_key = faces(self.entity)?.0;
        let key = faces(kind)?.0;
        let unified = if self.entity == "notice" {
            "v_통합차수"
        } else {
            "v_통합"
        };
        Ok(format!(
            "SELECT {key} FROM {unified} WHERE {own_key} = {} AND {key} <> '' LIMIT 1",
            own(own_key)
        ))
    }

    fn public_rows(&self, kind: &str) -> Result<String> {
        let columns = columns(views::base_columns(kind)?);
        if kind != "contract" {
            let (key, _, view, _) = faces(kind)?;
            return Ok(format!(
                "SELECT 0 AS __order, {columns} FROM {view} WHERE {key} = ({})",
                self.public_key(kind)?
            ));
        }
        let own_key = faces(self.entity)?.0;
        let unified = if self.entity == "notice" {
            "v_통합차수"
        } else {
            "v_통합"
        };
        Ok(format!(
            "SELECT 계약본번호 AS __order, {columns} FROM v_계약 WHERE 계약번호 IN (SELECT 계약번호 FROM {unified} WHERE {own_key} = {})",
            own(own_key)
        ))
    }
}

fn scalar(shape: &'static str, sql: String) -> Part {
    Part {
        shape,
        sql,
        fields: Vec::new(),
    }
}

// ── 단계 ─────────────────────────────────────────────

/// `FlowCompiler.Apply` — 단계 하나를 앞 조각 위에 쌓는다.
fn apply(
    step: &FlowStep,
    part: Part,
    output: &'static str,
    fields: &[String],
    row: &dyn Fn(&str) -> Result<String>,
) -> Result<Part> {
    let rel = part.sql.clone();
    let cols = if part.shape == "values" {
        "__value".to_string()
    } else {
        columns(&part.fields)
    };
    let rel_part = |shape: &'static str, sql: String| Part {
        shape,
        sql,
        fields: if matches!(shape, "rows" | "row") {
            fields.to_vec()
        } else {
            Vec::new()
        },
    };
    // 값 하나를 v 로 한 번만 읽는다 — 식이 v 를 여러 번 불러도 앞 단계 SQL 이 겹쳐 불어나지 않는다.
    let over = |shape: &'static str, body: String| {
        scalar(shape, format!("(SELECT {body} FROM (SELECT {rel} AS v))"))
    };
    let op = step.op.as_deref().unwrap_or("");
    let text = step.text.as_deref().unwrap_or("");
    let field = step.field.as_deref();

    match op {
        "filter" => {
            return Ok(rel_part(
                "rows",
                format!(
                    "SELECT * FROM ({rel}) WHERE {}",
                    condition(&cond(step), &|f| Ok(ident(f)))?
                ),
            ));
        }
        "sort" => {
            let desc = if step.dir.as_deref() == Some("desc") {
                " DESC"
            } else {
                ""
            };
            if part.shape == "rows" && field.is_none() {
                return Ok(if desc.is_empty() {
                    part
                } else {
                    rel_part(
                        "rows",
                        format!(
                            "SELECT row_number() OVER (ORDER BY __order DESC) AS __order, {cols} FROM ({rel})"
                        ),
                    )
                });
            }
            let key = if part.shape == "rows" {
                ident(field.unwrap_or(""))
            } else {
                "__value".to_string()
            };
            let number_key = number(&key);
            // 모두 수로 읽히면 수로, 아니면 글로 견준다. 같으면 원래 차례(안정 정렬).
            return Ok(Part {
                shape: part.shape,
                sql: raw(
                    r#"
                    SELECT row_number() OVER (ORDER BY CASE WHEN __a = 1 THEN __k END{desc}, CASE WHEN __a = 1 THEN NULL ELSE __t END{desc}, __order) AS __order, {cols}
                    FROM (SELECT *, {Number(key)} AS __k, {key} AS __t,
                                 min(CASE WHEN {Number(key)} IS NULL THEN 0 ELSE 1 END) OVER () AS __a
                          FROM ({rel}))
                    "#,
                    &[
                        ("desc", desc),
                        ("cols", &cols),
                        ("Number(key)", &number_key),
                        ("key", &key),
                        ("rel", &rel),
                    ],
                ),
                fields: part.fields,
            });
        }
        "distinct" => {
            let keys = if part.shape == "values" {
                "__value".to_string()
            } else {
                match step.fields.as_ref().filter(|f| !f.is_empty()) {
                    Some(chosen) => columns(
                        &chosen
                            .iter()
                            .map(|f| f.clone().unwrap_or_default())
                            .collect::<Vec<_>>(),
                    ),
                    None => columns(&part.fields),
                }
            };
            return Ok(Part {
                shape: part.shape,
                sql: raw(
                    r#"
                    SELECT __order, {cols}
                    FROM (SELECT *, row_number() OVER (PARTITION BY {keys} ORDER BY __order) AS __rn FROM ({rel}))
                    WHERE __rn = 1
                    "#,
                    &[("cols", &cols), ("keys", &keys), ("rel", &rel)],
                ),
                fields: part.fields,
            });
        }
        "take" => {
            return Ok(Part {
                shape: part.shape,
                sql: format!(
                    "SELECT * FROM ({rel}) ORDER BY __order LIMIT {}",
                    int_or_empty(step.n)
                ),
                fields: part.fields,
            });
        }
        "nth" | "first" | "last" => {
            let pick = if op == "last" {
                "ORDER BY __order DESC LIMIT 1".to_string()
            } else {
                let offset = if op == "nth" {
                    i64::from(step.n.unwrap_or(0)) - 1
                } else {
                    0
                };
                format!("ORDER BY __order LIMIT 1 OFFSET {offset}")
            };
            return Ok(if part.shape == "values" {
                scalar(
                    "value",
                    format!("coalesce((SELECT __value FROM ({rel}) {pick}), '')"),
                )
            } else {
                rel_part("row", format!("SELECT * FROM ({rel}) {pick}"))
            });
        }
        "format" => {
            let text = template(text);
            return Ok(if part.shape == "row" {
                scalar(
                    "value",
                    format!("coalesce((SELECT {text} FROM ({rel})), '')"),
                )
            } else {
                rel_part(
                    "values",
                    format!("SELECT __order, {text} AS __value FROM ({rel})"),
                )
            });
        }
        "get" => {
            let f = ident(field.unwrap_or(""));
            return Ok(if part.shape == "row" {
                scalar("value", format!("coalesce((SELECT {f} FROM ({rel})), '')"))
            } else {
                rel_part(
                    "values",
                    format!("SELECT __order, {f} AS __value FROM ({rel})"),
                )
            });
        }
        "join" => {
            return Ok(scalar(
                "value",
                format!(
                    "coalesce((SELECT group_concat(__value, {} ORDER BY __order) FROM ({rel})), '')",
                    lit(text)
                ),
            ));
        }
        "abbrev" => {
            let n = step.n.unwrap_or(0).to_string();
            let head = lit("{앞}");
            let rest = lit("{나머지}");
            return Ok(scalar(
                "value",
                raw(
                    r#"
                    coalesce((SELECT CASE WHEN count(*) <= {n} THEN group_concat(__value, ', ' ORDER BY __order)
                        ELSE replace(replace({text},
                            {head}, group_concat(CASE WHEN __rn <= {n} THEN __value END, ', ' ORDER BY __order)),
                            {rest}, CAST(count(*) - {n} AS TEXT)) END
                      FROM (SELECT __order, __value, row_number() OVER (ORDER BY __order) AS __rn FROM ({rel}))), '')
                    "#,
                    &[
                        ("n", &n),
                        ("text", &lit(text)),
                        ("head", &head),
                        ("rest", &rest),
                        ("rel", &rel),
                    ],
                ),
            ));
        }
        "count" => {
            return Ok(scalar("number", format!("(SELECT count(*) FROM ({rel}))")));
        }
        "sum" | "min" | "max" | "avg" => {
            let column = if part.shape == "rows" {
                ident(field.unwrap_or(""))
            } else {
                "__value".to_string()
            };
            return Ok(scalar(
                "number",
                format!("(SELECT {op}({}) FROM ({rel}))", number(&column)),
            ));
        }
        "if" if part.shape == "row" => {
            let otherwise = lit(step.r#else.as_deref().unwrap_or(""));
            return Ok(scalar(
                "value",
                format!(
                    "coalesce((SELECT CASE WHEN {} THEN {} ELSE {otherwise} END FROM ({rel})), {otherwise})",
                    condition(&cond(step), &|f| Ok(ident(f)))?,
                    lit(step.then.as_deref().unwrap_or(""))
                ),
            ));
        }
        _ => {}
    }

    // C# 은 여기서 operand 를 먼저 셈한다 — 맞춤 뷰에서 문서를 붙이지 않은 필드는 동사와 상관없이 여기서 거절된다.
    let operand = match field {
        Some(f) => number(&row(f)?),
        None => num(step.value.unwrap_or(0.0)),
    };
    let scale = || -> Result<String> {
        let digits = step.n.unwrap_or(0);
        if !(-15..=15).contains(&digits) {
            return Err(Error::invalid("소수 자리는 -15~15 사이여야 합니다."));
        }
        // Math.Pow(10, digits) — 10 의 거듭제곱은 글로 읽어 가장 가까운 double 로 둔다(정확히 반올림한 값).
        Ok(num(format!("1e{digits}").parse::<f64>().expect("수 글")))
    };
    let value_or_row = |f: Option<&str>, fallback: &str| -> Result<String> {
        match f {
            Some(f) => row(f),
            None => Ok(lit(fallback)),
        }
    };
    Ok(match op {
        "trim" => over("value", format!("trim(v, {SPACES})")),
        "replace" => over(
            "value",
            format!(
                "replace(v, {}, {})",
                lit(text),
                lit(step.to.as_deref().unwrap_or(""))
            ),
        ),
        "before" => {
            let s = lit(text);
            over(
                "value",
                format!(
                    "CASE WHEN instr(v, {s}) > 0 THEN substr(v, 1, instr(v, {s}) - 1) ELSE v END"
                ),
            )
        }
        "after" => {
            let s = lit(text);
            over(
                "value",
                format!(
                    "CASE WHEN instr(v, {s}) > 0 THEN substr(v, instr(v, {s}) + length({s})) ELSE '' END"
                ),
            )
        }
        "substr" => over(
            "value",
            format!(
                "substr(v, {}, {})",
                int_or_empty(step.n),
                int_or_empty(step.length)
            ),
        ),
        "padStart" => {
            let length = step.length.unwrap_or(0);
            over(
                "value",
                format!(
                    "CASE WHEN length(v) >= {length} THEN v ELSE substr(replace(hex(zeroblob({length})), '00', {}), 1, {length} - length(v)) || v END",
                    lit(text)
                ),
            )
        }
        "ifEmpty" => over(
            "value",
            format!(
                "CASE WHEN v = '' THEN {} ELSE v END",
                value_or_row(field, text)?
            ),
        ),
        "map" => {
            let cases: Vec<String> = step
                .pairs
                .iter()
                .flatten()
                .flatten()
                .map(|p| {
                    let a = p.first().cloned().flatten().unwrap_or_default();
                    let b = p.get(1).cloned().flatten().unwrap_or_default();
                    format!("WHEN {} THEN {}", lit(&a), lit(&b))
                })
                .collect();
            let otherwise = step
                .otherwise
                .as_deref()
                .map_or_else(|| "v".to_string(), lit);
            over(
                "value",
                format!("CASE v {} ELSE {otherwise} END", cases.join(" ")),
            )
        }
        "if" => {
            // 빈 필드는 지금 값, 적힌 필드는 바깥의 열이다(비면과 같은 자리).
            let subject = |f: &str| {
                if f.is_empty() {
                    Ok("v".to_string())
                } else {
                    row(f)
                }
            };
            over(
                "value",
                format!(
                    "CASE WHEN {} THEN {} ELSE {} END",
                    condition(&cond(step), &subject)?,
                    lit(step.then.as_deref().unwrap_or("")),
                    lit(step.r#else.as_deref().unwrap_or(""))
                ),
            )
        }
        "cases" => {
            // 차례대로 견주어 처음 맞는 갈래가 이긴다. 맞는 것이 없으면 그외(없으면 빈 글).
            let subject = |f: &str| {
                if f.is_empty() {
                    Ok("v".to_string())
                } else {
                    row(f)
                }
            };
            let mut whens = Vec::new();
            for b in step.branches.iter().flatten().flatten() {
                let clauses: Vec<&FlowClause> = b.cond.iter().flatten().flatten().collect();
                whens.push(format!(
                    "WHEN {} THEN {}",
                    condition(&clauses, &subject)?,
                    lit(b.then.as_deref().unwrap_or(""))
                ));
            }
            over(
                "value",
                format!(
                    "CASE {} ELSE {} END",
                    whens.join(" "),
                    lit(step.otherwise.as_deref().unwrap_or(""))
                ),
            )
        }
        "toNumber" => over("number", number("v")),
        "add" => over("number", format!("v + ({operand})")),
        "sub" => over("number", format!("v - ({operand})")),
        "mul" => over("number", format!("v * ({operand})")),
        "div" => over("number", format!("v * 1.0 / ({operand})")),
        "round" => {
            // Math.round 처럼 절댓값에서 반올림한다(0.5 는 0 에서 멀어지게). 수학 함수를 쓰지 않는다 — 소비자에 없을
            // 수 있다.
            let scale = scale()?;
            over(
                "number",
                format!(
                    "CASE WHEN v IS NULL THEN NULL ELSE (CASE WHEN v < 0 THEN -1 ELSE 1 END) * CAST(abs(v) * {scale} + 0.5 AS INTEGER) * 1.0 / {scale} END"
                ),
            )
        }
        "floor" => {
            let scale = scale()?;
            over(
                "number",
                format!(
                    "(SELECT CASE WHEN x IS NULL THEN NULL WHEN x < CAST(x AS INTEGER) THEN CAST(x AS INTEGER) - 1 ELSE CAST(x AS INTEGER) END * 1.0 / {scale} FROM (SELECT v * {scale} AS x))"
                ),
            )
        }
        "thousands" => over(
            "value",
            "CASE WHEN v IS NULL THEN '' ELSE printf('%,d', v) END".into(),
        ),
        "plain" => over(
            "value",
            "CASE WHEN v IS NULL THEN '' ELSE printf('%.15g', v) END".into(),
        ),
        "toDate" => scalar("date", to_date(&rel)),
        "dateAdd" => {
            let unit = match step.unit.as_deref() {
                Some("month") => "months",
                Some("year") => "years",
                _ => "days",
            };
            let n = step.n.unwrap_or(0);
            let sign = if n < 0 { "-" } else { "+" };
            over(
                "date",
                format!(
                    "date(v, {})",
                    lit(&format!("{sign}{} {unit}", i64::from(n).abs()))
                ),
            )
        }
        "dateDiff" => over(
            "number",
            format!(
                "CAST(round(julianday(v) - julianday({})) AS INTEGER)",
                to_date(&row(field.unwrap_or(""))?)
            ),
        ),
        "year" => over("number", "CAST(substr(v, 1, 4) AS INTEGER)".into()),
        "month" => over("number", "CAST(substr(v, 6, 2) AS INTEGER)".into()),
        "quarter" => over(
            "number",
            "(CAST(substr(v, 6, 2) AS INTEGER) + 2) / 3".into(),
        ),
        "dateFormat" => over(
            "value",
            format!(
                "CASE WHEN v IS NULL THEN '' ELSE {} END",
                date_pattern(text)
            ),
        ),
        _ => {
            let _ = output;
            return Err(Error::InvalidOperation(format!(
                "이 꼴에서 돌 수 없는 연산: {op}"
            )));
        }
    })
}

/// 단계의 조건(C# `step.Cond!`). 읽기([`Flow::parse`])가 빈 마디를 거르므로 모두 차 있다.
fn cond(step: &FlowStep) -> Vec<&FlowClause> {
    step.cond.iter().flatten().flatten().collect()
}

/// C# `{step.N}` — `int?` 를 끼우면 null 은 빈 글이다.
fn int_or_empty(n: Option<i32>) -> String {
    n.map_or_else(String::new, |n| n.to_string())
}

/// 바깥 줄바꿈마다 두 칸을 넣는다 — `SqlLine` 처럼 SQL 글자·식별자 안의 줄바꿈은 결과 값의 일부라 건드리지 않는다.
fn indent_lines(expression: &str) -> String {
    // `'(?:[^']|'')*'|"(?:[^"]|"")*"|\n` 을 왼쪽부터 찾는다. 닫히지 않은 따옴표는 그 자리에서 맞지 않는다.
    let chars: Vec<char> = expression.chars().collect();
    let mut out = String::with_capacity(expression.len() + 16);
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\n' {
            out.push_str("\n  ");
            i += 1;
            continue;
        }
        if (c == '\'' || c == '"')
            && let Some(end) = quoted_end(&chars, i)
        {
            out.extend(&chars[i..=end]);
            i = end + 1;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

/// `q(?:[^q]|qq)*q` 가 `start` 에서 맞으면 닫는 따옴표의 자리. 탐욕 반복이 끝까지 가서 맞지 않으면 짝 따옴표 하나를
/// 닫는 것으로 물러난다(정규식의 되짚기와 같다).
fn quoted_end(chars: &[char], start: usize) -> Option<usize> {
    let q = chars[start];
    let mut j = start + 1;
    let mut last_pair: Option<usize> = None;
    while j < chars.len() {
        if chars[j] != q {
            j += 1;
            continue;
        }
        if chars.get(j + 1) == Some(&q) {
            last_pair = Some(j);
            j += 2;
            continue;
        }
        return Some(j);
    }
    last_pair
}

// ── 조각 ─────────────────────────────────────────────

/// `FlowCompiler.Template` — 서식: `{필드}` 자리에 그 행의 값(`\{([^{}]+)\}`). 필드 값은 NULL 이 아니다.
fn template(text: &str) -> String {
    let mut parts = Vec::new();
    let bytes = text.as_bytes();
    let mut end = 0;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{' {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j] != b'{' && bytes[j] != b'}' {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b'}' && j > i + 1 {
                if i > end {
                    parts.push(lit(&text[end..i]));
                }
                parts.push(ident(&text[i + 1..j]));
                end = j + 1;
                i = j + 1;
                continue;
            }
        }
        i += 1;
    }
    if end < text.len() {
        parts.push(lit(&text[end..]));
    }
    if parts.is_empty() {
        "''".to_string()
    } else {
        parts.join(" || ")
    }
}

/// `FlowCompiler.DatePattern` — `YYYY|MM|M|DD|D` 를 왼쪽부터, 앞의 갈래가 먼저.
fn date_pattern(text: &str) -> String {
    let mut parts = Vec::new();
    let mut end = 0;
    let mut i = 0;
    while i < text.len() {
        let rest = &text[i..];
        let token = ["YYYY", "MM", "M", "DD", "D"]
            .into_iter()
            .find(|t| rest.starts_with(t));
        let Some(token) = token else {
            i += rest.chars().next().map_or(1, char::len_utf8);
            continue;
        };
        if i > end {
            parts.push(lit(&text[end..i]));
        }
        parts.push(
            match token {
                "YYYY" => "substr(v, 1, 4)",
                "MM" => "substr(v, 6, 2)",
                "M" => "CAST(CAST(substr(v, 6, 2) AS INTEGER) AS TEXT)",
                "DD" => "substr(v, 9, 2)",
                _ => "CAST(CAST(substr(v, 9, 2) AS INTEGER) AS TEXT)",
            }
            .to_string(),
        );
        i += token.len();
        end = i;
    }
    if end < text.len() {
        parts.push(lit(&text[end..]));
    }
    if parts.is_empty() {
        "''".to_string()
    } else {
        parts.join(" || ")
    }
}

/// `FlowCompiler.Number` — 글 → 수. 쉼표와 앞뒤 공백을 걷고 `[+-]?(숫자+.?숫자*|.숫자+)` 꼴만 읽는다 — 못 읽으면
/// NULL. `CAST('abc' AS REAL)` 은 0 을 내므로 꼴을 먼저 본다([`Flow::to_number`] 와 같은 규칙).
pub(crate) fn number(text: &str) -> String {
    let t = format!("trim(replace({text}, ',', ''), {SPACES})");
    let u = format!("(CASE WHEN {t} GLOB '[+-]*' THEN substr({t}, 2) ELSE {t} END)");
    format!(
        "(CASE WHEN {u} <> '' AND {u} <> '.' AND {u} NOT GLOB '*[^0-9.]*' AND {u} NOT GLOB '*.*.*' THEN CAST({t} AS REAL) END)"
    )
}

/// `FlowCompiler.ToDate` — 글 → 날짜(`YYYY-MM-DD`). 앞 10자를 보고 `/` 를 `-` 로. 없는 날(2월 30일)은 NULL.
fn to_date(text: &str) -> String {
    format!(
        "(SELECT CASE WHEN t GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]' AND date(t) = t THEN t END FROM (SELECT replace(substr({text}, 1, 10), '/', '-') AS t))"
    )
}

/// `FlowCompiler.Condition` — 그리고가 또는보다 먼저 묶인다(SQL 과 같다). 수와 수면 수로, 아니면 글로 견준다.
fn condition(cond: &[&FlowClause], subject: &dyn Fn(&str) -> Result<String>) -> Result<String> {
    let mut text = String::from("(");
    for (i, c) in cond.iter().enumerate() {
        if i > 0 {
            text.push_str(if c.join.as_deref() == Some("or") {
                " OR "
            } else {
                " AND "
            });
        }
        let a = subject(c.field.as_deref().unwrap_or(""))?;
        let (bs, bn): (String, Option<f64>) = match &c.value {
            Some(Value::Number(n)) => {
                let d: f64 = n.to_string().parse().unwrap_or(f64::NAN);
                (js_number(d), Some(d))
            }
            Some(Value::String(s)) => (s.clone(), Flow::to_number(s)),
            _ => (String::new(), None),
        };
        let op = match c.cmp.as_deref() {
            Some("=") => "=",
            Some("≠") => "<>",
            Some(">") => ">",
            Some("<") => "<",
            Some("≥") => ">=",
            Some("≤") => "<=",
            _ => "",
        };
        match c.cmp.as_deref() {
            Some("비어있음") => text.push_str(&format!("{a} = ''")),
            Some("안비어있음") => text.push_str(&format!("{a} <> ''")),
            Some("포함") => text.push_str(&format!("instr({a}, {}) > 0", lit(&bs))),
            _ => match bn {
                None => text.push_str(&format!("{a} {op} {}", lit(&bs))),
                Some(bn) => {
                    let na = number(&a);
                    text.push_str(&format!(
                        "(CASE WHEN {na} IS NOT NULL THEN {na} {op} {} ELSE {a} {op} {} END)",
                        num(bn),
                        lit(&bs)
                    ));
                }
            },
        }
    }
    text.push(')');
    Ok(text)
}

/// `FlowCompiler.Columns`.
fn columns<S: AsRef<str>>(names: &[S]) -> String {
    names
        .iter()
        .map(|n| ident(n.as_ref()))
        .collect::<Vec<_>>()
        .join(", ")
}

/// `FlowCompiler.Ident` — SQL 식별자.
pub fn ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// `FlowCompiler.Lit` — SQL 글자.
pub fn lit(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

/// `FlowCompiler.Num` — 정수이고 1e15 아래면 정수로, 아니면 .NET 의 왕복 표기(`R`)로.
fn num(value: f64) -> String {
    if value == value.floor() && value.abs() < 1e15 {
        // `(long)value` — -0 도 0 이다.
        (value as i64).to_string()
    } else {
        dotnet_double_r(value)
    }
}

/// `FlowCompiler.JsNumber` — JS 의 `String(수)` 를 흉내 낸 것: 정수이고 1e21 아래면 사용자 서식 `0`, 아니면 `R`.
fn js_number(value: f64) -> String {
    if value == value.floor() && value.abs() < 1e21 {
        dotnet_double_integer(value)
    } else {
        dotnet_double_r(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_and_date_pattern_follow_the_regexes() {
        assert_eq!(template(""), "''");
        assert_eq!(template("{a}-{b}"), "\"a\" || '-' || \"b\"");
        assert_eq!(template("x{a{b}y"), "'x{a' || \"b\" || 'y'");
        assert_eq!(template("{}"), "'{}'");
        assert_eq!(
            date_pattern("YYYY년 M월"),
            "substr(v, 1, 4) || '년 ' || CAST(CAST(substr(v, 6, 2) AS INTEGER) AS TEXT) || '월'"
        );
        assert_eq!(date_pattern("YYY"), "'YYY'");
    }

    #[test]
    fn numbers_follow_dotnet() {
        assert_eq!(num(3.0), "3");
        assert_eq!(num(-0.0), "0");
        assert_eq!(num(0.1), "0.1");
        assert_eq!(num(1e15), "1000000000000000");
        assert_eq!(num(1e17), "1E+17");
        assert_eq!(num(1e-15), "1E-15");
        assert_eq!(js_number(5.0), "5");
        assert_eq!(js_number(2.5), "2.5");
    }

    #[test]
    fn eject_indents_outside_literals() {
        assert_eq!(
            indent_lines("a\n'b\nc'\n\"d\ne\""),
            "a\n  'b\nc'\n  \"d\ne\""
        );
        assert_eq!(indent_lines("'x''y'\nz"), "'x''y'\n  z");
    }
}

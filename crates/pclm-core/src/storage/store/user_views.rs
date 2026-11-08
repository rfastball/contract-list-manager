//! C# `StoreUserViews.cs` — 맞춤 뷰(docs/user-views.md)의 목록·재료·보표·표본·미리보기·저장·지우기.

use std::collections::{BTreeSet, HashSet};

use rusqlite::Connection;
use serde_json::{Map, Value};

use super::columns::{Sample, TRACE_SAMPLES, TraceFrame, trace};
use super::{
    COMPUTED_SAMPLE, Store, begin_deferred, object, query, read_all, readable_schema, scalar_i64,
    single_or_default, strings_json, text_of, view_packages::stored_formula,
};
use crate::dict::Dict;
use crate::error::{Error, Result};
use crate::json::{self, Encoder};
use crate::storage::database::{self, refresh};
use crate::storage::flow::{Flow, FlowDefinition, FlowFormula, FlowSource};
use crate::storage::flow_compiler::{self, Target};
use crate::storage::sql_column::{self, Frame};
use crate::storage::user_view_builder::{self, KEY, NUMBER, TITLE, UserViewFrame};
use crate::storage::views::{self, USER_VIEW_PREFIX};
use crate::text::{eq_ignore_case, ignore_case_key, utf16_len};

/// `Store.UserViewNameLength` — 뷰·필드 이름의 길이 한도.
pub(super) const USER_VIEW_NAME_LENGTH: usize = 40;

/// `Store.UserViewEdges` — 표본 고르개의 가장자리.
pub const USER_VIEW_EDGES: [&str; 2] = ["manyContracts", "unlinked"];

/// C# `Store.UserViewInfo(Id, Name, Definition, Broken)` — 목록의 맞춤 뷰 하나. `broken` 은 필드 식별자 → 까닭.
#[derive(Clone, Debug, PartialEq)]
pub struct UserViewInfo {
    pub id: String,
    pub name: String,
    pub definition: Value,
    pub broken: Dict<String, String>,
}

impl UserViewInfo {
    pub fn to_json(&self) -> Value {
        let mut broken = Map::new();
        for (k, v) in self.broken.iter() {
            broken.insert(k.clone(), Value::String(v.clone()));
        }
        object([
            ("id", Value::String(self.id.clone())),
            ("name", Value::String(self.name.clone())),
            ("definition", self.definition.clone()),
            ("broken", Value::Object(broken)),
        ])
    }
}

/// `Store.ChainOrder`.
const CHAIN_ORDER: &str = "s.공고 IS NULL, s.공고, s.접수 IS NULL, s.접수, s.계약 IS NULL, s.계약";

/// `Store.ChainPrefix`.
const CHAIN_PREFIX: &str = "(coalesce(s.접수, '') || '|' || coalesce(s.공고, '') || '|')";

/// `Store.AllChains`·`Store.AllFolded` — 거르지 않는 줄 꼴.
fn all_frame(fold: bool) -> UserViewFrame {
    UserViewFrame {
        fold,
        kinds: BTreeSet::new(),
    }
}

/// C# `Store.ViewInput(Name, Fold, Columns)` — 화면이 보낸 뷰를 읽은 것. 필드는 (식별자, 이름, 가공식 JSON).
pub(super) struct ViewInput {
    pub(super) name: String,
    pub(super) fold: bool,
    pub(super) columns: Vec<(Option<String>, String, String)>,
}

/// `Store.ParseViewInput` — `{ name, definition: { contracts, columns: [{ id, name, formula }] } }`.
pub(super) fn parse_view_input(json_text: &str) -> Result<ViewInput> {
    let bad = || Error::invalid("맞춤 뷰 형식이 올바르지 않습니다.");
    let root = json::parse(json_text).map_err(|_| bad())?;
    // 같은 키가 두 번 든 뷰(최상위·필드·식 어디든)도 형식 오류다 — 뒤의 값을 골라 받아들이면 C#(형식 오류)과 갈린다.
    if json::has_duplicate_keys(json_text) {
        return Err(bad());
    }
    let Value::Object(view) = &root else {
        return Err(bad());
    };
    parse_view_object(view)
}

/// [`parse_view_input`] 의 몸 — 이미 읽은 객체에서.
pub(super) fn parse_view_object(view: &Map<String, Value>) -> Result<ViewInput> {
    let bad = || Error::invalid("맞춤 뷰 형식이 올바르지 않습니다.");
    fn text(node: Option<&Value>) -> String {
        match node {
            Some(Value::String(s)) => s.clone(),
            _ => String::new(),
        }
    }
    let definition = view.get("definition").and_then(Value::as_object);
    let contracts = text(definition.and_then(|d| d.get("contracts")));
    if !matches!(contracts.as_str(), "" | "split" | "fold") {
        return Err(Error::InvalidOperation(format!(
            "계약이 여럿인 건의 처리가 올바르지 않습니다: {contracts}"
        )));
    }
    let empty = Vec::new();
    let columns = match definition.and_then(|d| d.get("columns")) {
        Some(Value::Array(items)) => items,
        _ => &empty,
    };
    let columns = columns
        .iter()
        .map(|c| match c {
            Value::Object(o) => Ok((
                // C# `o["id"] is null` — 빠졌거나 JSON null 이면 없다.
                o.get("id").filter(|v| !v.is_null()).map(|v| text(Some(v))),
                text(o.get("name")),
                o.get("formula")
                    .filter(|v| !v.is_null())
                    .map(|f| json::to_compact(f, Encoder::Default))
                    .unwrap_or_default(),
            )),
            _ => Err(bad()),
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(ViewInput {
        name: text(view.get("name")),
        fold: contracts == "fold",
        columns,
    })
}

/// `Store.CleanViewName` — 뷰·필드 이름을 다듬는다. 큰따옴표는 뷰 이름(`"u_이름"`)을 깨뜨린다.
pub(super) fn clean_view_name(name: &str, what: &str) -> Result<String> {
    let clean = name.trim();
    if utf16_len(clean) > USER_VIEW_NAME_LENGTH {
        return Err(Error::InvalidOperation(format!(
            "{what}은 {USER_VIEW_NAME_LENGTH}자까지입니다."
        )));
    }
    if clean.chars().any(|ch| ch == '"' || ch.is_control()) {
        return Err(Error::InvalidOperation(format!(
            "{what}에 큰따옴표를 쓸 수 없습니다."
        )));
    }
    Ok(clean.to_string())
}

/// 받은 필드 식별자를 쓸 수 있는가 — 비지 않고 100자 이하, 제어 문자 없음, 아직 안 쓴 것.
pub(super) fn usable_id(id: Option<&str>, taken: &mut HashSet<String>) -> Option<String> {
    let given = id?;
    let len = utf16_len(given);
    (len > 0
        && len <= 100
        && !given.chars().any(char::is_control)
        && taken.insert(given.to_string()))
    .then(|| given.to_string())
}

impl Store<'_> {
    /// `Store.NewColumnId` — `c-` + 새 식별자. 겹치면 다시 뽑는다.
    pub(super) fn new_column_id(&self, taken: &mut HashSet<String>) -> String {
        loop {
            let id = format!("c-{}", self.env.new_id());
            if taken.insert(id.clone()) {
                return id;
            }
        }
    }

    // ── 목록 ─────────────────────────────────────────────

    /// `Store.UserViews` — 저장된 맞춤 뷰와 그 깨진 필드.
    pub fn user_views(&self) -> Result<Vec<UserViewInfo>> {
        let connection = self.database.open_read_only()?;
        let _transaction = begin_deferred(&connection)?;
        list_user_views(&connection)
    }

    // ── 필드 목록 재료 ─────────────────────────────────────

    /// `Store.UserViewContext` — 맞춤 뷰 편집기의 재료와 사슬의 건수.
    pub fn user_view_context(&self) -> Result<Value> {
        let connection = self.database.open_read_only()?;
        let _transaction = begin_deferred(&connection)?;
        let env = user_view_builder::env(&connection)?;
        let chain = views::chain();
        let total = scalar_i64(
            &connection,
            &format!("{chain}\nSELECT count(*) FROM 사슬"),
            [],
        )?;
        let many = scalar_i64(
            &connection,
            &format!(
                "{chain}\nSELECT count(*) FROM (SELECT 1 FROM 사슬 WHERE 공고 IS NOT NULL GROUP BY 접수, 공고 HAVING count(계약) >= 2)"
            ),
            [],
        )?;
        let linked = env
            .linked
            .iter()
            .map(|(kind, l)| {
                object([
                    ("kind", Value::String(kind.clone())),
                    ("fields", strings_json(&l.fields)),
                    ("items", strings_json(&l.items)),
                ])
            })
            .collect();
        Ok(object([
            ("columns", Value::Array(Vec::new())),
            ("rows", Value::Array(Vec::new())),
            ("fields", Value::Array(Vec::new())),
            ("sources", Value::Array(Vec::new())),
            (
                "available",
                object([
                    ("view", Value::String(String::new())),
                    ("base", strings_json(&env.base)),
                    ("items", strings_json(&env.items)),
                    ("linked", Value::Array(linked)),
                    ("schema", readable_schema(&connection)?),
                ]),
            ),
            ("total", total.into()),
            ("manyContracts", many.into()),
        ]))
    }

    // ── 보표 ─────────────────────────────────────────────

    /// `Store.TraceUserViewColumn` — 맞춤 뷰 필드의 값 흐름. 표본 열쇠를 주지 않으면 사슬의 앞 셋이다.
    pub fn trace_user_view_column(
        &self,
        json_text: &str,
        sample_keys: Option<&[String]>,
    ) -> Result<Value> {
        let formula = Flow::parse(json_text)?;
        let connection = sql_column::open_checked(&self.database)?;
        let _transaction = begin_deferred(&connection)?;
        let env = user_view_builder::env(&connection)?;
        let contract = user_view_builder::kinds(json_text).contains("contract");
        let chains = all_frame(false);
        let folded = all_frame(true);
        let keys: Vec<String> = match sample_keys {
            Some(given) if !given.is_empty() => given.iter().take(TRACE_SAMPLES).cloned().collect(),
            _ => query(
                &connection,
                &chains.statement(
                    KEY,
                    None,
                    &format!("ORDER BY {CHAIN_ORDER} LIMIT {TRACE_SAMPLES}"),
                ),
                [],
                |r| Ok(text_of(r.get_ref(0)?).unwrap_or_default()),
            )?,
        };

        // 이 열쇠가 계약마다 한 줄의 것인가 — 아니면 모은 건(계약 자리가 빈 열쇠)이다.
        let split = |c: &Connection, key: &str| -> Result<bool> {
            Ok(scalar_i64(
                c,
                &chains.statement("count(*)", Some(&format!("{KEY} = @key")), ""),
                rusqlite::named_params! {"@key": key},
            )? > 0)
        };
        let row = |select: &str| {
            chains.statement(
                select,
                Some(&format!(
                    "{KEY} = @key OR ({CHAIN_PREFIX} = @key AND s.계약 = (SELECT min(x.계약) FROM 사슬 x WHERE x.공고 = s.공고 AND x.접수 IS s.접수))"
                )),
                "LIMIT 1",
            )
        };
        let r#final = |expression: &str| {
            format!(
                "SELECT coalesce(({}), ({}))",
                chains.statement(expression, Some(&format!("{KEY} = @key")), "LIMIT 1"),
                folded.statement(
                    &folded.column(expression, contract),
                    Some(&format!("{KEY} = @key")),
                    "LIMIT 1"
                )
            )
        };
        let sql = |c: &Connection, sql: &str, keys: &[String]| -> Result<Vec<String>> {
            let mut is_split = HashSet::new();
            for k in keys {
                if split(c, k)? {
                    is_split.insert(k.clone());
                }
            }
            let a_keys: Vec<String> = keys
                .iter()
                .filter(|k| is_split.contains(*k))
                .cloned()
                .collect();
            let b_keys: Vec<String> = keys
                .iter()
                .filter(|k| !is_split.contains(*k))
                .cloned()
                .collect();
            let a = sql_column::validate_on(c, sql, &Frame::UserView(&chains), &a_keys, 20)?;
            let b = sql_column::validate_on(c, sql, &Frame::UserView(&folded), &b_keys, 20)?;
            let (mut ia, mut ib) = (a.into_iter(), b.into_iter());
            Ok(keys
                .iter()
                .map(|k| {
                    if is_split.contains(k) {
                        ia.next()
                    } else {
                        ib.next()
                    }
                    .unwrap_or_default()
                })
                .collect())
        };

        // 표본 머리의 품목 수 — 원천이 문서의 품목이면 그 문서의 것, 아니면 0.
        let items = match (&formula.source, &formula.flow) {
            (
                Some(source),
                Some(FlowDefinition {
                    from: Some(from), ..
                }),
            ) if source == "flow" && from.kind.as_deref() == Some("linked") => match &from.link {
                Some(link) => Some(
                    flow_compiler::prefix(
                        "",
                        &FlowDefinition {
                            from: Some(FlowSource {
                                kind: Some("linked".into()),
                                link: Some(link.clone()),
                                items: Some(true),
                                ..FlowSource::default()
                            }),
                            steps: Some(Vec::new()),
                        },
                        0,
                        Target::Internal,
                        Some(&env),
                    )?
                    .sql,
                ),
                None => None,
            },
            _ => None,
        };
        let mut samples = Vec::new();
        let mut heads = Vec::new();
        for key in keys {
            let rows_expr = match &items {
                None => "0".to_string(),
                Some(items) => format!("(SELECT count(*) FROM ({items}))"),
            };
            let head = single_or_default(
                &connection,
                &row(&format!("{NUMBER}, {TITLE}, {rows_expr}")),
                rusqlite::named_params! {"@key": key},
                |r| {
                    Ok((
                        text_of(r.get_ref(0)?).unwrap_or_default(),
                        text_of(r.get_ref(1)?).unwrap_or_default(),
                        r.get::<_, Option<i64>>(2)?.unwrap_or(0),
                    ))
                },
            )?;
            let Some((number, title, rows)) = head else {
                continue;
            };
            heads.push(object([
                ("base", Value::String(number)),
                ("revision", Value::String(String::new())),
                ("title", Value::String(title)),
                ("rows", rows.into()),
            ]));
            samples.push(Sample { key });
        }
        trace(
            &connection,
            "",
            &formula,
            &env,
            &samples,
            heads,
            &TraceFrame {
                row: Box::new(row),
                r#final: Box::new(r#final),
                sql: Box::new(sql),
            },
        )
    }

    // ── 표본 고르개 ─────────────────────────────────────────

    /// `Store.UserViewSamples` — 미리보기 표본으로 고를 건. `query` 는 번호·건명의 일부, `edge` 는 가장자리.
    pub fn user_view_samples(
        &self,
        view_json: &str,
        query_text: Option<&str>,
        edge: Option<&str>,
    ) -> Result<Vec<Value>> {
        if let Some(edge) = edge
            && !USER_VIEW_EDGES.contains(&edge)
        {
            return Err(Error::InvalidOperation(format!(
                "모르는 표본 가장자리입니다: {edge}"
            )));
        }
        let input = parse_view_input(view_json)?;
        let frame =
            user_view_builder::frame(input.fold, input.columns.iter().map(|c| c.2.as_str()));
        let mut filters = Vec::new();
        let q = query_text.map(str::trim).unwrap_or("");
        if !q.is_empty() {
            filters.push(format!(
                "instr(lower({NUMBER}), lower(@q)) > 0 OR instr(lower({TITLE}), lower(@q)) > 0"
            ));
        }
        if edge == Some("manyContracts") {
            filters.push(frame.many_contracts().to_string());
        }
        if edge == Some("unlinked") {
            filters.push(frame.unlinked());
        }
        let where_ = (!filters.is_empty()).then(|| {
            filters
                .iter()
                .map(|w| format!("({w})"))
                .collect::<Vec<_>>()
                .join(" AND ")
        });
        let connection = self.database.open_read_only()?;
        let sql = frame.statement(
            &format!("{KEY}, {NUMBER}, {TITLE}"),
            where_.as_deref(),
            &format!("ORDER BY {CHAIN_ORDER}"),
        );
        let map = |r: &rusqlite::Row<'_>| {
            Ok(object([
                (
                    "key",
                    Value::String(text_of(r.get_ref(0)?).unwrap_or_default()),
                ),
                (
                    "number",
                    Value::String(text_of(r.get_ref(1)?).unwrap_or_default()),
                ),
                (
                    "title",
                    Value::String(text_of(r.get_ref(2)?).unwrap_or_default()),
                ),
            ]))
        };
        // Dapper 는 글에 없는 매개변수를 넘기지 않는다 — 찾을 말이 있을 때만 `@q` 가 글에 선다.
        if q.is_empty() {
            query(&connection, &sql, [], map)
        } else {
            query(&connection, &sql, rusqlite::named_params! {"@q": q}, map)
        }
    }

    // ── 미리보기 ─────────────────────────────────────────

    /// `Store.PreviewUserView` — 저장하기 전의 뷰를 표본 한 건과 빈 칸 수로 미리 본다. 필드마다 따로 컴파일해 깨진
    /// 필드는 까닭을 싣는다.
    pub fn preview_user_view(&self, view_json: &str, sample_key: Option<&str>) -> Result<Value> {
        let input = parse_view_input(view_json)?;
        let frame =
            user_view_builder::frame(input.fold, input.columns.iter().map(|c| c.2.as_str()));
        let connection = sql_column::open_checked(&self.database)?;
        let _transaction = begin_deferred(&connection)?;
        let env = user_view_builder::env(&connection)?;
        let mut expressions: Vec<Option<String>> = Vec::new();
        let mut errors: Vec<Option<String>> = Vec::new();
        for column in &input.columns {
            let attempt = (|| -> Result<String> {
                let (expression, _) =
                    user_view_builder::compile(&connection, &column.2, &env, &frame, false)?;
                read_all(&connection, &frame.statement(&expression, None, "LIMIT 20"))?;
                Ok(expression)
            })();
            match attempt {
                Ok(expression) => {
                    expressions.push(Some(expression));
                    errors.push(None);
                }
                Err(e @ (Error::InvalidOperation(_) | Error::Sqlite(_))) => {
                    let message = sql_column::error_message(&e);
                    expressions.push(None);
                    errors.push(Some(if message.is_empty() {
                        "식을 계산하지 못했습니다.".into()
                    } else {
                        message
                    }));
                }
                Err(other) => return Err(other),
            }
        }

        let total = scalar_i64(&connection, &frame.statement("count(*)", None, ""), [])?;
        let sample_key = sample_key.filter(|k| !k.is_empty());
        let head = match sample_key {
            None => None,
            Some(key) => single_or_default(
                &connection,
                &frame.statement(
                    &format!("{NUMBER}, {TITLE}"),
                    Some(&format!("{KEY} = @sampleKey")),
                    "LIMIT 1",
                ),
                rusqlite::named_params! {"@sampleKey": key},
                |r| {
                    Ok((
                        text_of(r.get_ref(0)?).unwrap_or_default(),
                        text_of(r.get_ref(1)?).unwrap_or_default(),
                    ))
                },
            )?,
        };

        let live: Vec<(usize, &String)> = expressions
            .iter()
            .enumerate()
            .filter_map(|(i, e)| e.as_ref().map(|e| (i, e)))
            .collect();
        let mut values = vec![String::new(); input.columns.len()];
        let mut blanks = vec![0i64; input.columns.len()];
        if !live.is_empty() {
            let select = live
                .iter()
                .map(|(i, e)| format!("{e} AS c{i}"))
                .collect::<Vec<_>>()
                .join(",\n");
            if head.is_some()
                && let Some(key) = sample_key
            {
                let mut statement = connection.prepare(&frame.statement(
                    &select,
                    Some(&format!("{KEY} = @sampleKey")),
                    "LIMIT 1",
                ))?;
                let mut rows = statement.query(rusqlite::named_params! {"@sampleKey": key})?;
                if let Some(row) = rows.next()? {
                    for (j, (i, _)) in live.iter().enumerate() {
                        values[*i] = text_of(row.get_ref(j)?).unwrap_or_default();
                    }
                }
            }
            let sums = live
                .iter()
                .map(|(i, _)| format!("coalesce(sum(c{i} = ''), 0)"))
                .collect::<Vec<_>>()
                .join(", ");
            let mut statement = connection.prepare(&format!(
                "SELECT {sums} FROM ({})",
                frame.statement(&select, None, "")
            ))?;
            let mut rows = statement.query([])?;
            if let Some(row) = rows.next()? {
                for (j, (i, _)) in live.iter().enumerate() {
                    blanks[*i] = row.get::<_, i64>(j)?;
                }
            }
        }

        let columns = input
            .columns
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let mut row = Map::new();
                row.insert("id".into(), Value::String(c.0.clone().unwrap_or_default()));
                row.insert("value".into(), Value::String(values[i].clone()));
                row.insert("blank".into(), blanks[i].into());
                if let Some(why) = &errors[i] {
                    row.insert("error".into(), Value::String(why.clone()));
                }
                Value::Object(row)
            })
            .collect();
        Ok(object([
            (
                "sample",
                match (head, sample_key) {
                    (Some((number, title)), Some(key)) => object([
                        ("key", Value::String(key.into())),
                        ("number", Value::String(number)),
                        ("title", Value::String(title)),
                    ]),
                    _ => Value::Null,
                },
            ),
            ("total", total.into()),
            ("columns", Value::Array(columns)),
        ]))
    }

    // ── 저장·지우기 ─────────────────────────────────────────

    /// `Store.SaveUserView` — 맞춤 뷰를 저장한다. 필드를 다시 검사하고, 뷰를 다시 지어 깨진 필드가 있으면 되돌린다.
    /// `previous_id` 가 있으면 그 뷰를 고친다.
    pub fn save_user_view(
        &self,
        view_json: &str,
        previous_id: Option<&str>,
    ) -> Result<Vec<UserViewInfo>> {
        let input = parse_view_input(view_json)?;
        let name = clean_view_name(&input.name, "뷰 이름")?;
        if name.is_empty() {
            return Err(Error::invalid("뷰 이름이 비었습니다."));
        }
        if input.columns.is_empty() {
            return Err(Error::invalid("필드를 하나 이상 추가하세요."));
        }

        let mut seen_names = HashSet::new();
        let mut seen_ids = HashSet::new();
        let mut columns: Vec<(String, String, FlowFormula)> = Vec::new();
        for column in &input.columns {
            let column_name = clean_view_name(&column.1, "필드 이름")?;
            if column_name.is_empty() {
                return Err(Error::invalid("이름이 빈 필드가 있습니다."));
            }
            if !seen_names.insert(ignore_case_key(&column_name)) {
                return Err(Error::InvalidOperation(format!(
                    "필드 이름이 겹칩니다: {column_name}"
                )));
            }
            let formula = match Flow::parse(&column.2) {
                Ok(f) => f,
                Err(Error::InvalidOperation(m)) => {
                    return Err(Error::InvalidOperation(format!(
                        "‘{column_name}’ 필드를 세울 수 없습니다: {m}"
                    )));
                }
                Err(other) => return Err(other),
            };
            let id = match usable_id(column.0.as_deref(), &mut seen_ids) {
                Some(id) => id,
                None => self.new_column_id(&mut seen_ids),
            };
            columns.push((id, column_name, formula));
        }
        let frame =
            user_view_builder::frame(input.fold, input.columns.iter().map(|c| c.2.as_str()));

        // SQL 필드의 읽기 권한·모양·표본은 쓰기 전에 읽기 전용 연결에서 본다(가공열과 같다).
        {
            let reader = sql_column::open_checked(&self.database)?;
            let env = user_view_builder::env(&reader)?;
            for (_, column_name, formula) in &columns {
                let checked = if formula.source.as_deref() == Some("sql") {
                    sql_column::validate_on(
                        &reader,
                        formula.sql.as_deref().unwrap_or(""),
                        &Frame::UserView(&frame),
                        &[],
                        20,
                    )
                    .map(|_| ())
                } else {
                    flow_compiler::expression("", formula, Target::Internal, Some(&env)).map(|_| ())
                };
                match checked {
                    Ok(()) => {}
                    Err(e @ (Error::InvalidOperation(_) | Error::Sqlite(_))) => {
                        return Err(Error::InvalidOperation(format!(
                            "‘{column_name}’ 필드를 세울 수 없습니다: {}",
                            sql_column::error_message(&e)
                        )));
                    }
                    Err(other) => return Err(other),
                }
            }
        }

        let definition = stored_view_definition(
            input.fold,
            columns
                .iter()
                .map(|(id, name, formula)| Ok((id.clone(), name.clone(), stored_formula(formula)?)))
                .collect::<Result<Vec<_>>>()?,
        );

        let connection = self.database.open()?;
        let transaction = database::begin(&connection)?;
        let existing = query(&transaction, "SELECT id, name FROM user_view", [], |r| {
            Ok((
                text_of(r.get_ref(0)?).unwrap_or_default(),
                text_of(r.get_ref(1)?).unwrap_or_default(),
            ))
        })?;
        if existing
            .iter()
            .any(|(id, n)| Some(id.as_str()) != previous_id && eq_ignore_case(n, &name))
        {
            return Err(Error::InvalidOperation(format!(
                "‘{name}’ 맞춤 뷰가 이미 있습니다."
            )));
        }
        let view_id = match previous_id {
            Some(previous) => {
                if !existing.iter().any(|(id, _)| id == previous) {
                    return Err(Error::invalid("그런 맞춤 뷰가 없습니다."));
                }
                transaction.execute(
                    "UPDATE user_view SET name = ?1, definition = ?2 WHERE id = ?3",
                    rusqlite::params![name, definition, previous],
                )?;
                previous.to_string()
            }
            None => {
                let view_id = self.env.new_id();
                transaction.execute(
                    "INSERT INTO user_view (id, name, definition, sort_order)\n\
                     VALUES (?1, ?2, ?3, coalesce((SELECT max(sort_order) FROM user_view), 0) + 10)",
                    rusqlite::params![view_id, name, definition],
                )?;
                view_id
            }
        };

        let built = (|| -> Result<()> {
            let broken = refresh(&transaction)?.user_views;
            for (id, column_name, _) in &columns {
                if let Some(why) = broken.get(&(view_id.clone(), id.clone())) {
                    return Err(Error::InvalidOperation(format!(
                        "‘{column_name}’ 필드를 세울 수 없습니다: {why}"
                    )));
                }
            }
            read_all(
                &transaction,
                &format!(
                    "SELECT * FROM {} LIMIT {COMPUTED_SAMPLE}",
                    flow_compiler::ident(&format!("{USER_VIEW_PREFIX}{name}"))
                ),
            )
        })();
        match built {
            Ok(()) => {}
            Err(Error::Sqlite(error)) => {
                return Err(Error::InvalidOperation(format!(
                    "맞춤 뷰를 짓지 못했습니다: {}",
                    sql_column::dotnet_message(&error)
                )));
            }
            Err(other) => return Err(other),
        }
        transaction.commit()?;
        self.user_views()
    }

    /// `Store.DeleteUserView` — 맞춤 뷰를 지운다(정의만 — 자료는 건드리지 않는다).
    pub fn delete_user_view(&self, id: &str) -> Result<Vec<UserViewInfo>> {
        {
            let connection = self.database.open()?;
            let transaction = database::begin(&connection)?;
            if transaction.execute("DELETE FROM user_view WHERE id = ?1", [id])? == 0 {
                return Err(Error::invalid("그런 맞춤 뷰가 없습니다."));
            }
            refresh(&transaction)?;
            transaction.commit()?;
        }
        self.user_views()
    }

    /// `Store.UserViewReaders` — 이 종류의 사람 열마다 그것을 읽는 맞춤 뷰 수.
    pub fn user_view_readers(&self, entity_type: &str) -> Result<Dict<String, i32>> {
        let connection = self.database.open_read_only()?;
        user_view_builder::readers(&connection, entity_type)
    }
}

/// 저장 꼴의 정의 `{version:2, contracts, columns:[{id, name, formula}]}` 를 `Flow.Json` 으로.
pub(super) fn stored_view_definition(fold: bool, columns: Vec<(String, String, Value)>) -> String {
    let columns = columns
        .into_iter()
        .map(|(id, name, formula)| {
            object([
                ("id", Value::String(id)),
                ("name", Value::String(name)),
                ("formula", formula),
            ])
        })
        .collect();
    json::to_compact(
        &object([
            ("version", json::number("2")),
            (
                "contracts",
                Value::String(if fold { "fold" } else { "split" }.into()),
            ),
            ("columns", Value::Array(columns)),
        ]),
        Encoder::Relaxed,
    )
}

/// `Store.ListUserViews`.
fn list_user_views(connection: &Connection) -> Result<Vec<UserViewInfo>> {
    let built: Vec<(String, Option<String>)> = query(
        connection,
        "SELECT name, sql FROM sqlite_master WHERE type = 'view' AND name LIKE 'u\\_%' ESCAPE '\\'",
        [],
        |r| {
            Ok((
                text_of(r.get_ref(0)?).unwrap_or_default(),
                text_of(r.get_ref(1)?),
            ))
        },
    )?;
    let mut out = Vec::new();
    for view in user_view_builder::read_definitions(connection)? {
        let view_name = view.view_name();
        let why = views::broken_in(
            built
                .iter()
                .find(|(n, _)| *n == view_name)
                .and_then(|(_, s)| s.as_deref()),
        );
        let mut broken = Dict::new();
        for column in &view.columns {
            if let Some((_, reason)) = why.iter().find(|(n, _)| *n == column.name) {
                broken.insert(
                    column.id.clone(),
                    if reason.is_empty() {
                        "식을 계산하지 못했습니다.".to_string()
                    } else {
                        reason.clone()
                    },
                );
            }
        }
        // 같은 키가 두 번 든 정의도 읽지 못한 정의다 — 뒤의 값으로 접어 내보내면 C# 과 글이 갈린다.
        let definition = json::parse(&view.definition)
            .ok()
            .filter(|_| !json::has_duplicate_keys(&view.definition))
            .unwrap_or_else(|| {
                json::parse(r#"{"version":2,"contracts":"split","columns":[]}"#)
                    .expect("고정된 JSON")
            });
        out.push(UserViewInfo {
            id: view.id,
            name: view.name,
            definition,
            broken,
        });
    }
    Ok(out)
}

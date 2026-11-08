//! 맞춤 뷰(C# `Storage/UserViewBuilder.cs`) — 조달 건마다 한 줄, 흐름식 필드를 늘어놓은 뷰(docs/user-views.md). 정의를
//! 읽고, 뷰를 짓고([`build`]), 접수·공고·계약의 사람 열 이름이 바뀔 때 정의를 따라 고쳐 쓴다([`rename_field`]).
//! 저장과 미리보기는 저장 단계가 더한다.

use std::collections::BTreeSet;

use rusqlite::{Connection, params};
use serde_json::{Map, Value};

use crate::dict::Dict;
use crate::error::{Error, Result};
use crate::json::{self, Encoder};
use crate::storage::flow::Flow;
use crate::storage::flow_check::{self, FlowEnv};
use crate::storage::flow_compiler::{self, Target};
use crate::storage::sql_column;
use crate::storage::views::{self, USER_VIEW_PREFIX, UserColumnDefinition};
use crate::text::{cmp_ordinal, raw};

/// `UserViewDefinition` 중 보정이 쓰는 자리 — `user_view` 한 줄의 식별자·이름·저장된 정의 원문.
#[derive(Clone, Debug, PartialEq)]
pub struct StoredUserView {
    pub id: String,
    pub name: String,
    pub definition: String,
    pub sort_order: i64,
}

/// `UserViewBuilder.Read` — 저장된 맞춤 뷰 전부. 차례는 `sort_order`, 이름.
pub fn read(connection: &Connection) -> Result<Vec<StoredUserView>> {
    Ok(connection
        .prepare(
            "SELECT id, name, definition, sort_order FROM user_view ORDER BY sort_order, name;",
        )?
        .query_map([], |r| {
            Ok(StoredUserView {
                id: r.get(0)?,
                name: r.get(1)?,
                definition: r.get(2)?,
                sort_order: r.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?)
}

/// `Flow.DocWords` — 맞춤 뷰의 문서 이름(`공고.공고명` 의 앞).
pub(crate) fn doc_word(kind: &str) -> Option<&'static str> {
    match kind {
        "request" => Some("접수"),
        "notice" => Some("공고"),
        "contract" => Some("계약"),
        _ => None,
    }
}

/// 단계가 행의 열을 읽는 자리(`비면`·사칙·`날짜차이`). 맞춤 뷰에서는 `공고.공고명` 꼴이다.
const ROW_FIELD_STEPS: [&str; 6] = ["ifEmpty", "add", "sub", "mul", "div", "dateDiff"];

fn text(node: Option<&Value>) -> &str {
    match node {
        Some(Value::String(s)) => s,
        _ => "",
    }
}

/// 단계의 조건 마디 모두(`경우` 의 갈래 포함)의 자리 — `(마디 경로)` 로 돌려준다.
fn clause_paths(step: &Map<String, Value>) -> Vec<(Option<usize>, usize)> {
    let mut paths = Vec::new();
    if let Some(Value::Array(cond)) = step.get("cond") {
        for (i, c) in cond.iter().enumerate() {
            if c.is_object() {
                paths.push((None, i));
            }
        }
    }
    if let Some(Value::Array(branches)) = step.get("branches") {
        for (b, branch) in branches.iter().enumerate() {
            if let Some(Value::Array(cond)) = branch.get("cond") {
                for (i, c) in cond.iter().enumerate() {
                    if c.is_object() {
                        paths.push((Some(b), i));
                    }
                }
            }
        }
    }
    paths
}

fn clause_mut(
    step: &mut Map<String, Value>,
    path: (Option<usize>, usize),
) -> Option<&mut Map<String, Value>> {
    match path {
        (None, i) => step.get_mut("cond")?.get_mut(i)?.as_object_mut(),
        (Some(b), i) => step
            .get_mut("branches")?
            .get_mut(b)?
            .get_mut("cond")?
            .get_mut(i)?
            .as_object_mut(),
    }
}

/// `UserViewBuilder.Touch` — 흐름식에서 `kind` 문서의 열 `field` 를 읽는 자리를 찾고, `to` 가 있으면 그 이름으로
/// 고친다. 자리는 셋이다 — 원천 `linked(kind) field`, 단계가 읽는 행의 열 `공고.f`, 원천이 그 문서 한 행일 때
/// 첫 단계가 짚는 필드(`꺼내기 f`·`만약 f = …`·`서식 "{f}"`). 찾았는가를 낸다.
pub(crate) fn touch(
    formula: Option<&mut Value>,
    kind: &str,
    field: &str,
    to: Option<&str>,
) -> bool {
    let Some(Value::Object(root)) = formula else {
        return false;
    };
    if text(root.get("source")) != "flow" {
        return false;
    }
    let Some(Value::Object(flow)) = root.get_mut("flow") else {
        return false;
    };
    let Some(word) = doc_word(kind) else {
        // C# 은 Flow.DocWords[kind] 에서 KeyNotFoundException 을 낸다 — 부르는 쪽이 종류를 걸러 부른다.
        return false;
    };
    let mut found = false;
    let (linked, from_field_absent) = match flow.get_mut("from") {
        Some(Value::Object(from)) => {
            let linked = text(from.get("kind")) == "linked" && text(from.get("link")) == kind;
            if linked && text(from.get("field")) == field {
                found = true;
                if let Some(to) = to {
                    from.insert("field".into(), Value::String(to.into()));
                }
            }
            let bare = from.get("field").is_none_or(Value::is_null)
                && from.get("items").is_none_or(Value::is_null);
            (linked, bare)
        }
        _ => (false, false),
    };
    let Some(Value::Array(steps)) = flow.get_mut("steps") else {
        return found;
    };
    let qualified = format!("{word}.{field}");
    for step in steps.iter_mut() {
        let Value::Object(step) = step else { continue };
        if ROW_FIELD_STEPS.contains(&text(step.get("op"))) && text(step.get("field")) == qualified {
            found = true;
            if let Some(to) = to {
                step.insert("field".into(), Value::String(format!("{word}.{to}")));
            }
        }
        // 값 하나를 견주는 조건의 마디도 그 자리를 읽는다(만약 공고.f = …).
        for path in clause_paths(step) {
            if let Some(clause) = clause_mut(step, path)
                && text(clause.get("field")) == qualified
            {
                found = true;
                if let Some(to) = to {
                    clause.insert("field".into(), Value::String(format!("{word}.{to}")));
                }
            }
        }
    }

    // 원천이 그 문서 한 행(필드·품목 없이)이면 첫 단계가 그 행의 필드를 짚는다.
    if !linked || !from_field_absent {
        return found;
    }
    let Some(Value::Object(first)) = steps.first_mut() else {
        return found;
    };
    match text(first.get("op")) {
        "get" if text(first.get("field")) == field => {
            found = true;
            if let Some(to) = to {
                first.insert("field".into(), Value::String(to.into()));
            }
        }
        "if" if matches!(first.get("cond"), Some(Value::Array(_))) => {
            if let Some(Value::Array(cond)) = first.get_mut("cond") {
                for clause in cond.iter_mut().filter_map(Value::as_object_mut) {
                    if text(clause.get("field")) == field {
                        found = true;
                        if let Some(to) = to {
                            clause.insert("field".into(), Value::String(to.into()));
                        }
                    }
                }
            }
        }
        "format" if text(first.get("text")).contains(&format!("{{{field}}}")) => {
            found = true;
            if let Some(to) = to {
                let replaced =
                    text(first.get("text")).replace(&format!("{{{field}}}"), &format!("{{{to}}}"));
                first.insert("text".into(), Value::String(replaced));
            }
        }
        _ => {}
    }
    found
}

/// `(definition as JsonObject)?["columns"]` — 객체가 아닌 정의(배열·글·수, 손으로 고친 파일)는 읽지 못한 정의라 필드가
/// 없다. 예전에는 C# `JsonNode` 가 이름으로 물을 때 던지는 `InvalidOperationException` 을 그대로 옮겨 판올림·뷰 짓기가
/// 멈췄다.
pub(crate) fn columns_of(root: &mut Value) -> Option<&mut Value> {
    match root {
        Value::Object(map) => map.get_mut("columns"),
        _ => None,
    }
}

/// `UserViewBuilder.Readers` — 이 종류의 사람 열마다 그 열을 읽는 맞춤 뷰 수(열 관리가 지우기·이름 바꾸기 전에 보인다).
/// 읽는 뷰가 없는 열은 싣지 않는다.
pub fn readers(connection: &Connection, entity_type: &str) -> Result<Dict<String, i32>> {
    let own: Vec<String> = connection
        .prepare("SELECT field_name FROM user_column WHERE entity_type = ?1")?
        .query_map([entity_type], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<_>>()?;
    let mut counts = Dict::new();
    if doc_word(entity_type).is_none() {
        return Ok(counts);
    }
    for view in read_definitions(connection)? {
        let mut formulas: Vec<Option<Value>> = view
            .columns
            .iter()
            // `Node` — 같은 키가 두 번 든 식은 읽지 못한 것이다.
            .map(|c| {
                json::parse(&c.formula)
                    .ok()
                    .filter(|_| !json::has_duplicate_keys(&c.formula))
            })
            .collect();
        for field in &own {
            if formulas
                .iter_mut()
                .any(|x| touch(x.as_mut(), entity_type, field, None))
            {
                let n = counts.get(field).copied().unwrap_or(0);
                counts.insert(field.clone(), n + 1);
            }
        }
    }
    Ok(counts)
}

/// `UserViewBuilder.RenameField` — 사람 열 이름이 바뀌면 그 열을 읽던 맞춤 뷰 필드를 새 이름으로 고쳐 쓴다.
/// 고친 뷰 수를 낸다. 부르는 쪽의 트랜잭션 안에서 돈다.
pub(crate) fn rename_field(
    connection: &Connection,
    entity_type: &str,
    from: &str,
    to: &str,
) -> Result<usize> {
    if from == to || doc_word(entity_type).is_none() {
        return Ok(0);
    }
    let mut changed = 0;
    for view in read(connection)? {
        let Ok(mut root) = json::parse(&view.definition) else {
            continue;
        };
        // 같은 키가 두 번 든 정의는 읽지 못한 것이라 고쳐 쓰지 않는다 — 뒤의 값을 골라 고쳐 쓰면 겹친 키가 조용히
        // 사라지고, C# 은 그 정의를 건너뛴다.
        if json::has_duplicate_keys(&view.definition) {
            continue;
        }
        let Some(Value::Array(columns)) = columns_of(&mut root) else {
            continue;
        };
        let mut touched = false;
        for column in columns.iter_mut().filter_map(Value::as_object_mut) {
            touched |= touch(column.get_mut("formula"), entity_type, from, Some(to));
        }
        if !touched {
            continue;
        }
        connection.execute(
            "UPDATE user_view SET definition = ?1 WHERE id = ?2",
            params![json::to_compact(&root, Encoder::Relaxed), view.id],
        )?;
        changed += 1;
    }
    Ok(changed)
}

// ── 뷰 짓기 ─────────────────────────────────────────────

/// C# `UserViewColumn` — 맞춤 뷰의 필드 하나(이름 = 흐름식). `formula` 는 가공식 v2 JSON 그대로다.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserViewColumn {
    pub id: String,
    pub name: String,
    pub formula: String,
}

/// C# `UserViewDefinition` — 저장된 맞춤 뷰 하나. `columns`·`fold` 는 `definition` 을 읽은 것이다 — 읽지 못하는 정의는
/// 필드가 없는 것으로 읽는다(앱이 뜨지 못하게 하지 않는다).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserViewDefinition {
    pub id: String,
    pub name: String,
    pub definition: String,
    pub sort_order: i64,
    pub columns: Vec<UserViewColumn>,
    pub fold: bool,
}

impl UserViewDefinition {
    /// `UserViewDefinition.ViewName` — SQLite 에 서는 이름(`u_<name>`).
    pub fn view_name(&self) -> String {
        format!("{USER_VIEW_PREFIX}{}", self.name)
    }
}

/// `UserViewFrame.Key` — 조달 건의 불투명 열쇠(접수·공고건·계약 본번호를 `|` 로 이은 것). 접힌 건은 계약 자리가 빈다.
pub const KEY: &str =
    "(coalesce(s.접수, '') || '|' || coalesce(s.공고, '') || '|' || coalesce(s.계약, ''))";

/// `UserViewFrame.Number` — 보이는 번호(입찰공고번호 → 접수번호 → 계약번호 중 처음 있는 것, §7.4).
pub const NUMBER: &str = "coalesce(nullif(coalesce(공고.\"입찰공고번호\", ''), ''), nullif(coalesce(접수.\"접수번호\", ''), ''), coalesce(계약.\"계약번호\", ''))";

/// `UserViewFrame.Title` — 제목(공고명 → 요청명 → 계약건명 중 처음 있는 것, §7.4).
pub const TITLE: &str = "coalesce(nullif(coalesce(공고.\"공고명\", ''), ''), nullif(coalesce(접수.\"요청명\", ''), ''), coalesce(계약.\"계약건명\", ''))";

/// C# `UserViewFrame` — 맞춤 뷰의 줄이 서는 꼴(docs/user-views.md §2·§5). 줄은 조달 건(`v_통합` 과 같은 사슬)이고, 같은
/// 건의 접수·공고·계약 면 뷰를 별칭 `접수`·`공고`·`계약` 으로 건다. `fold` 면 계약이 여럿인 건을 (접수, 공고건)으로
/// 접는다. `kinds` 는 담긴 흐름식 필드가 읽는 문서 종류 — 그중 하나도 없는 건은 줄이 서지 않는다(비었으면 거르지 않는다).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserViewFrame {
    pub fold: bool,
    /// C# 은 `HashSet` 이고 쓸 때 서수 차례로 늘어놓는다 — 처음부터 차례로 둔다.
    pub kinds: BTreeSet<String>,
}

impl UserViewFrame {
    /// `UserViewFrame.ManyContracts` — 계약이 둘 이상 이어진 건인가(표본 고르개의 「계약 여럿」).
    pub fn many_contracts(&self) -> &'static str {
        if self.fold {
            "s.계약수 >= 2"
        } else {
            "(SELECT count(x.계약) FROM 사슬 x WHERE x.공고 = s.공고 AND x.접수 IS s.접수) >= 2"
        }
    }

    /// `UserViewFrame.Unlinked` — 접수·공고·계약 중 빠진 단계가 있는 건인가(표본 고르개의 「연결 안 된 건」).
    pub fn unlinked(&self) -> String {
        format!(
            "(s.접수 IS NULL OR s.공고 IS NULL OR {})",
            if self.fold {
                "s.계약수 = 0"
            } else {
                "s.계약 IS NULL"
            }
        )
    }

    /// `UserViewFrame.Statement` — 문장 하나. 고른 것을 조달 건마다 낸다 — 거르기(`kinds`)와 `where_` 를 함께 걸고
    /// 끝에 `tail`(`ORDER BY`·`LIMIT`)을 붙인다.
    pub fn statement(&self, select: &str, where_: Option<&str>, tail: &str) -> String {
        let mut filters = Vec::new();
        if !self.kinds.is_empty() {
            let kinds: Vec<&str> = self
                .kinds
                .iter()
                .map(|kind| match kind.as_str() {
                    "request" => "s.접수 IS NOT NULL",
                    "notice" => "s.공고 IS NOT NULL",
                    _ if self.fold => "s.계약수 > 0",
                    _ => "s.계약 IS NOT NULL",
                })
                .collect();
            filters.push(format!("({})", kinds.join(" OR ")));
        }
        if let Some(where_) = where_ {
            filters.push(format!("({where_})"));
        }
        let filters = if filters.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", filters.join(" AND "))
        };
        let folded = if self.fold { folded() } else { String::new() };
        raw(
            r#"
            {사슬}{접힌}
            SELECT
                {select}
            FROM {from} s
            LEFT JOIN v_접수 AS 접수 ON 접수.접수본번호 = s.접수
            LEFT JOIN v_공고 AS 공고 ON 공고.공고본번호 = {현행}
            LEFT JOIN v_계약 AS 계약 ON 계약.계약본번호 = s.계약
            {filters}
            {tail}
            "#,
            &[
                ("사슬", &views::chain()),
                ("접힌", &folded),
                ("select", select),
                ("from", if self.fold { "접힌" } else { "사슬" }),
                ("현행", &views::group_current("s.공고", "현행.notice_base")),
                ("filters", &filters),
                ("tail", tail),
            ],
        )
    }

    /// `UserViewFrame.Column` — 필드 하나의 칸. 모으기에서 계약을 읽는 필드는 그 건의 계약마다 식을 셈해 계약본번호
    /// 차례로 `', '` 로 잇는다. 계약이 하나 이하면 그대로 셈한다.
    pub fn column(&self, expression: &str, reads_contract: bool) -> String {
        if !self.fold || !reads_contract {
            return expression.to_string();
        }
        raw(
            r#"
        (CASE WHEN s.계약수 <= 1 THEN {expression} ELSE coalesce((
            SELECT group_concat({expression}, ', ' ORDER BY x.계약)
            FROM 사슬 x LEFT JOIN v_계약 AS 계약 ON 계약.계약본번호 = x.계약
            WHERE x.공고 = s.공고 AND x.접수 IS s.접수 AND x.계약 IS NOT NULL), '') END)
        "#,
            &[("expression", expression)],
        )
    }
}

/// `UserViewFrame.접힌` — 접은 사슬. 공고건이 있는 것은 (접수, 공고건)마다 한 줄이고, 계약이 하나뿐이면 그 계약을
/// 그대로 들고 간다. 공고건이 없는 사슬은 그대로다.
fn folded() -> String {
    raw(
        r#"
        ,
        접힌(접수, 공고, 계약, 계약수) AS (
            SELECT 접수, 공고, CASE WHEN count(계약) = 1 THEN max(계약) END, count(계약)
            FROM 사슬 WHERE 공고 IS NOT NULL GROUP BY 접수, 공고
            UNION ALL
            SELECT 접수, 공고, 계약, CASE WHEN 계약 IS NULL THEN 0 ELSE 1 END
            FROM 사슬 WHERE 공고 IS NULL
        )
        "#,
        &[],
    )
}

/// `UserViewBuilder.ParseColumns` — 정의 JSON 의 필드. 읽지 못하면(같은 키가 두 번 든 정의·객체가 아닌 정의 포함) 빈
/// 목록이다([`columns_of`]).
pub fn parse_columns(definition: &str) -> Result<Vec<UserViewColumn>> {
    let Ok(mut root) = json::parse(definition) else {
        return Ok(Vec::new());
    };
    // 같은 키가 두 번 든 정의(손으로 고친 파일)도 읽지 못한 것이다 — 뒤의 값을 골라 읽으면 C# 과 갈린다.
    if json::has_duplicate_keys(definition) {
        return Ok(Vec::new());
    }
    let Some(Value::Array(columns)) = columns_of(&mut root) else {
        return Ok(Vec::new());
    };
    Ok(columns
        .iter()
        .filter_map(Value::as_object)
        .map(|c| UserViewColumn {
            id: text(c.get("id")).to_string(),
            name: text(c.get("name")).to_string(),
            // `JsonNode.ToJsonString()` — 옵션이 없어 기본 인코더(비ASCII 를 \u 로)다. JSON null 은 C# 에서 노드가 없어
            // (`?.ToJsonString() ?? ""`) 빈 글이다.
            formula: c
                .get("formula")
                .filter(|f| !f.is_null())
                .map_or_else(String::new, |f| json::to_compact(f, Encoder::Default)),
        })
        .collect())
}

/// `UserViewBuilder.ParseFold` — 계약이 여럿인 건을 한 줄로 모으는가(`contracts` 가 `fold`). 읽지 못하면(같은 키가
/// 두 번 든 정의·객체가 아닌 정의 포함) 계약마다 한 줄이다.
pub fn parse_fold(definition: &str) -> Result<bool> {
    let Ok(root) = json::parse(definition) else {
        return Ok(false);
    };
    if json::has_duplicate_keys(definition) {
        return Ok(false);
    }
    Ok(match &root {
        Value::Object(map) => text(map.get("contracts")) == "fold",
        _ => false,
    })
}

/// `UserViewBuilder.Read` 의 C# 꼴 — 저장된 맞춤 뷰 전부를 읽은 정의로.
pub fn read_definitions(connection: &Connection) -> Result<Vec<UserViewDefinition>> {
    read(connection)?
        .into_iter()
        .map(|v| {
            Ok(UserViewDefinition {
                columns: parse_columns(&v.definition)?,
                fold: parse_fold(&v.definition)?,
                id: v.id,
                name: v.name,
                definition: v.definition,
                sort_order: v.sort_order,
            })
        })
        .collect()
}

/// `UserViewBuilder.Own` — 접수·공고·계약의 사람 열·가공열 이름(종류 → 차례대로, `sort_order` 다음 이름의 서수).
pub fn own(columns: &[UserColumnDefinition]) -> Vec<(String, Vec<String>)> {
    ["request", "notice", "contract"]
        .into_iter()
        .map(|kind| {
            let mut mine: Vec<&UserColumnDefinition> =
                columns.iter().filter(|c| c.entity_type == kind).collect();
            mine.sort_by(|a, b| {
                a.sort_order
                    .cmp(&b.sort_order)
                    .then_with(|| cmp_ordinal(&a.field_name, &b.field_name))
            });
            (
                kind.to_string(),
                mine.into_iter().map(|c| c.field_name.clone()).collect(),
            )
        })
        .collect()
}

/// `UserViewBuilder.Env` — 맞춤 뷰의 재료(세 문서의 면 뷰 열과 품목 필드).
pub fn env(connection: &Connection) -> Result<FlowEnv> {
    let columns: Vec<UserColumnDefinition> = connection
        .prepare("SELECT entity_type, field_name, sort_order FROM user_column")?
        .query_map([], |r| {
            Ok(UserColumnDefinition {
                entity_type: r.get(0)?,
                field_name: r.get(1)?,
                kind: String::new(),
                options: None,
                sort_order: r.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(Flow::env_for_view(&own(&columns)))
}

/// `UserViewBuilder.Kinds` — 흐름식이 읽는 문서 종류(원천 `linked` 와 단계가 읽는 행의 열, 조건 마디의 바깥 열 포함).
/// SQL 필드와 읽지 못하는 식은 없다.
pub fn kinds(formula_json: &str) -> BTreeSet<String> {
    let mut kinds = BTreeSet::new();
    let Ok(formula) = Flow::parse(formula_json) else {
        return kinds;
    };
    let (Some("flow"), Some(flow)) = (formula.source.as_deref(), formula.flow.as_ref()) else {
        return kinds;
    };
    if let Some(from) = &flow.from
        && from.kind.as_deref() == Some("linked")
        && let Some(link) = from.link.as_deref()
        && doc_word(link).is_some()
    {
        kinds.insert(link.to_string());
    }
    for step in flow.steps.iter().flatten().flatten() {
        if let Some(field) = step.field.as_deref()
            && step
                .op
                .as_deref()
                .is_some_and(|op| ROW_FIELD_STEPS.contains(&op))
            && let Some((kind, _)) = flow_check::qualified(field)
        {
            kinds.insert(kind.to_string());
        }
        // 값 하나를 견주는 조건의 마디가 읽는 바깥의 열(만약 공고.f = …).
        for clause in flow_check::clauses(step) {
            if let Some((kind, _)) = clause.field.as_deref().and_then(flow_check::qualified) {
                kinds.insert(kind.to_string());
            }
        }
    }
    kinds
}

/// `UserViewBuilder.Frame` — 이 정의의 줄 꼴(모으기와, 흐름식 필드들이 읽는 문서 종류).
pub fn frame<'a>(fold: bool, formulas: impl IntoIterator<Item = &'a str>) -> UserViewFrame {
    UserViewFrame {
        fold,
        kinds: formulas.into_iter().flat_map(kinds).collect(),
    }
}

/// `UserViewBuilder.Compile` — 필드 하나의 칸과 끝 꼴. 흐름식은 맞춤 뷰 환경으로 꼴 검사·컴파일하고, SQL 필드는
/// `query_only` 를 건 채 authorizer 로 그 자리 위에서 준비해 본다.
pub(crate) fn compile(
    connection: &Connection,
    formula_json: &str,
    env: &FlowEnv,
    frame: &UserViewFrame,
    toggle_query_only: bool,
) -> Result<(String, &'static str)> {
    let formula = Flow::parse(formula_json)?;
    if formula.source.as_deref() == Some("sql") {
        let sql = formula.sql.as_deref().unwrap_or("");
        if toggle_query_only {
            connection.execute_batch("PRAGMA query_only = ON;")?;
        }
        let prepared = sql_column::prepare_on(connection, sql, &sql_column::Frame::UserView(frame));
        if toggle_query_only {
            connection.execute_batch("PRAGMA query_only = OFF;")?;
        }
        prepared?;
        return Ok((sql_column::expression(sql), "text"));
    }
    let expression = flow_compiler::expression("", &formula, Target::Internal, Some(env))?;
    let shape = match flow_compiler::final_shape("", &formula, Some(env))? {
        "number" => "number",
        "date" => "date",
        _ => "text",
    };
    Ok((
        frame.column(&expression, kinds(formula_json).contains("contract")),
        shape,
    ))
}

/// 깨진 맞춤 뷰 필드 — (뷰 식별자, 필드 식별자) → 까닭.
pub type BrokenFields = Dict<(String, String), String>;

/// `UserViewBuilder.Build` — 맞춤 뷰를 모두 짓는다(계약면 뷰를 다 지은 뒤에 부른다). 필드마다 따로 컴파일하고 사슬
/// 위에서 돌려 보아 깨진 필드는 빈 글 + 까닭 주석으로 세운다. 마지막에 `v_맞춤뷰` 를 결과타입과 함께 다시 짓는다.
pub(crate) fn build(
    connection: &Connection,
    user_columns: &[UserColumnDefinition],
) -> Result<BrokenFields> {
    let mut broken = BrokenFields::new();
    let mut types: Dict<(String, String), String> = Dict::new();
    let env = Flow::env_for_view(&own(user_columns));
    for view in read_definitions(connection)? {
        if view.columns.is_empty() {
            continue;
        }
        let frame = frame(view.fold, view.columns.iter().map(|c| c.formula.as_str()));
        let mut columns: Vec<(String, String, Option<String>)> = Vec::new();
        let mut mine: Dict<String, String> = Dict::new();
        for column in &view.columns {
            let tried = compile(connection, &column.formula, &env, &frame, true).and_then(
                |(expression, shape)| {
                    let sql = frame.statement(&expression, None, "LIMIT 20") + ";";
                    let mut statement = connection.prepare(&sql)?;
                    let mut rows = statement.query([])?;
                    while rows.next()?.is_some() {}
                    Ok((expression, shape))
                },
            );
            match tried {
                Ok((expression, shape)) => {
                    columns.push((column.id.clone(), column.name.clone(), Some(expression)));
                    types.insert((view.id.clone(), column.id.clone()), shape.to_string());
                }
                Err(error @ (Error::InvalidOperation(_) | Error::Sqlite(_))) => {
                    let message = sql_column::error_message(&error);
                    mine.insert(
                        column.id.clone(),
                        if message.is_empty() {
                            "식을 계산하지 못했습니다.".to_string()
                        } else {
                            message
                        },
                    );
                    columns.push((column.id.clone(), column.name.clone(), None));
                }
                Err(error) => return Err(error),
            }
        }

        let built = connection
            .execute_batch(&views::user_view(&view.name, &frame, &columns, &mine))
            .and_then(|()| {
                connection.execute_batch(&format!(
                    "SELECT * FROM {} LIMIT 0;",
                    flow_compiler::ident(&view.view_name())
                ))
            });
        if let Err(error) = built {
            // 필드마다 돌려 본 뒤라 여기서 깨질 일은 드물다. 그래도 깨지면 이 뷰의 필드를 모두 빈 글로 세운다.
            let message = sql_column::dotnet_message(&error);
            for (id, _, _) in &columns {
                if !mine.contains_key(id) {
                    mine.insert(id.clone(), message.clone());
                }
            }
            connection.execute_batch(&views::user_view(&view.name, &frame, &columns, &mine))?;
        }
        for (id, why) in mine.iter() {
            broken.insert((view.id.clone(), id.clone()), why.clone());
            types.remove(&(view.id.clone(), id.clone()));
        }
    }
    connection.execute_batch(&views::user_view_meta(&types))?;
    Ok(broken)
}

/// `UserViewBuilder.DropAll` — `sqlite_master` 의 `u_%` 를 모두 치운다(저장된 목록이 아니라 실제로 선 것을 본다).
/// 면 뷰보다 먼저 부른다 — 그 위층이 남아 면 뷰를 다시 짓는 것을 막지 않게.
pub(crate) fn drop_all(connection: &Connection) -> Result<()> {
    let names: Vec<String> = connection
        .prepare(
            "SELECT name FROM sqlite_master WHERE type = 'view' AND name LIKE 'u\\_%' ESCAPE '\\';",
        )?
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    for name in names {
        connection.execute_batch(&format!(
            "DROP VIEW IF EXISTS {};",
            flow_compiler::ident(&name)
        ))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touched(formula: &str, kind: &str, field: &str, to: Option<&str>) -> (bool, String) {
        let mut v = json::parse(formula).unwrap();
        let found = touch(Some(&mut v), kind, field, to);
        (found, json::to_compact(&v, Encoder::Relaxed))
    }

    #[test]
    fn touch_finds_and_renames_the_three_places() {
        let linked = r#"{"version":2,"source":"flow","flow":{"from":{"kind":"linked","link":"notice","field":"메모"},"steps":[]}}"#;
        assert_eq!(
            touched(linked, "notice", "메모", Some("메모_사용자")),
            (
                true,
                r#"{"version":2,"source":"flow","flow":{"from":{"kind":"linked","link":"notice","field":"메모_사용자"},"steps":[]}}"#.into()
            )
        );
        let row = r#"{"source":"flow","flow":{"from":{"kind":"linked","link":"contract","field":"계약명"},"steps":[{"op":"ifEmpty","field":"공고.메모"},{"op":"if","cond":[{"field":"공고.메모","cmp":"안비어있음"}]}]}}"#;
        let (found, out) = touched(row, "notice", "메모", Some("새"));
        assert!(found);
        assert!(out.contains(r#""field":"공고.새"},{"op":"if","cond":[{"field":"공고.새""#));
        let first = r#"{"source":"flow","flow":{"from":{"kind":"linked","link":"notice"},"steps":[{"op":"format","text":"{메모}/{메모}"}]}}"#;
        let (found, out) = touched(first, "notice", "메모", Some("새"));
        assert!(found);
        assert!(out.contains(r#""text":"{새}/{새}""#));
        assert!(!touched(first, "contract", "메모", None).0);
    }
}

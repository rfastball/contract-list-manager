//! 가공식 v2 — 흐름식의 원본(docs/column-management.md §4.5). C# `Storage/Flow.cs` 의 **자료 모형과 읽기·쓰기**만
//! 옮긴다. 꼴 검사(`Flow.Check`)·재료(`EnvFor`)·SQL 로 옮기기(`FlowCompiler`)는 가공식 단계(#9 커밋 5)가 채운다.
//!
//! 저장 꼴은 `Flow.Json` 이다 — `JsonSerializerDefaults.Web`(이름은 camelCase, 읽을 때 대소문자 무시, 수를 글로도
//! 받음) + `WhenWritingNull`(빈 자리는 적지 않음) + `UnsafeRelaxedJsonEscaping`. 속성 차례는 C# 레코드의 매개변수
//! 차례다. 글과 바이트까지 같아야 화면과 소비자가 같은 식을 읽는다.
//!
//! 자리마다 `Option` 인 것은 C# 의 참조형이 JSON 에서 빠지거나 `null` 로 올 수 있어서다 — 그 경우를 가르는 것은
//! 읽기가 아니라 [`Flow::parse`] 의 모양 검사(`Validate`)이고, 오류 글도 거기서 정한다.

use serde_json::{Map, Value};

use crate::error::{Error, Result};
use crate::json::{
    self, Encoder, JsonException, JsonResult, for_each_property, read_array, read_bool, read_f64,
    read_i32, read_object, read_required_i32, read_string,
};
use crate::text::{eq_ignore_case, is_blank, utf16_len};

/// C# `FlowFormula(int Version, string Id, string Source, FlowDefinition? Flow = null, string? Sql = null)`.
#[derive(Clone, Debug, PartialEq)]
pub struct FlowFormula {
    pub version: i32,
    pub id: Option<String>,
    pub source: Option<String>,
    pub flow: Option<FlowDefinition>,
    pub sql: Option<String>,
}

/// C# `FlowDefinition(FlowSource From, FlowStep[] Steps)`.
#[derive(Clone, Debug, PartialEq)]
pub struct FlowDefinition {
    pub from: Option<FlowSource>,
    pub steps: Option<Vec<Option<FlowStep>>>,
}

/// C# `FlowSource(string Kind, string? Field, string? Link, bool? Items, string? Text, double? Value)`.
/// `kind` 는 `items`·`field`·`linked`·`text`·`number`, `link` 는 `notice`·`request`·`contract`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FlowSource {
    pub kind: Option<String>,
    pub field: Option<String>,
    pub link: Option<String>,
    pub items: Option<bool>,
    pub text: Option<String>,
    pub value: Option<f64>,
}

/// C# `FlowClause(string? Join, string Field, string Cmp, JsonElement? Value)`. `value` 는 받은 JSON 값 그대로다
/// (글이나 수 — 모양 검사가 가른다).
#[derive(Clone, Debug, PartialEq)]
pub struct FlowClause {
    pub join: Option<String>,
    pub field: Option<String>,
    pub cmp: Option<String>,
    pub value: Option<Value>,
}

/// C# `FlowBranch(FlowClause[] Cond, string Then)` — `경우` 의 갈래 하나.
#[derive(Clone, Debug, PartialEq)]
pub struct FlowBranch {
    pub cond: Option<Vec<Option<FlowClause>>>,
    pub then: Option<String>,
}

/// C# `FlowStep` — 단계 하나. 자리는 C# 레코드의 차례 그대로다(쓰는 차례가 곧 저장 바이트다).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FlowStep {
    pub op: Option<String>,
    pub field: Option<String>,
    pub fields: Option<Vec<Option<String>>>,
    pub text: Option<String>,
    pub to: Option<String>,
    pub n: Option<i32>,
    pub length: Option<i32>,
    pub dir: Option<String>,
    pub unit: Option<String>,
    pub value: Option<f64>,
    pub cond: Option<Vec<Option<FlowClause>>>,
    pub then: Option<String>,
    pub r#else: Option<String>,
    pub pairs: Option<Vec<Option<Vec<Option<String>>>>>,
    pub otherwise: Option<String>,
    pub branches: Option<Vec<Option<FlowBranch>>>,
}

/// C# `ColumnStep(string Op, string[]? Fields, string? Text, int? Index)` — 가공식 v1 의 단계.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ColumnStep {
    pub op: Option<String>,
    pub fields: Option<Vec<Option<String>>>,
    pub text: Option<String>,
    pub index: Option<i32>,
}

/// C# `ColumnFormula(int Version, string Id, ColumnStep[] Steps)` — 가공식 v1. 읽을 때 v2 로 옮긴다.
#[derive(Clone, Debug, PartialEq)]
pub struct ColumnFormula {
    pub version: i32,
    pub id: Option<String>,
    pub steps: Option<Vec<Option<ColumnStep>>>,
}

/// C# `Flow` 정적 클래스의 읽기·쓰기 부분.
pub struct Flow;

/// `Flow.MaxSteps`.
const MAX_STEPS: usize = 50;
/// `Flow.MaxText`.
const MAX_TEXT: usize = 1000;

/// `Flow.LinkWords` 의 열쇠 — 연결 문서로 쓸 수 있는 종류.
const LINKS: [&str; 3] = ["notice", "request", "contract"];

/// `Flow.Compares`.
const COMPARES: [&str; 9] = [
    "=",
    "≠",
    ">",
    "<",
    "≥",
    "≤",
    "포함",
    "비어있음",
    "안비어있음",
];

impl Flow {
    /// `Flow.Parse` — 저장된·화면이 보낸 가공식을 v2 로 읽는다. v1 은 그 자리에서 옮긴다(§4.5).
    /// 모양이 틀리면 [`Error::InvalidOperation`] 이고 글은 C# 과 같다.
    pub fn parse(json: &str) -> Result<FlowFormula> {
        read(json, false)
    }

    /// `Flow.ReadForIdentity` — 판올림 진단(`migrationError`)이 걸려 있어도 식별자는 읽는다. 모양 검사는 그대로다.
    pub fn read_for_identity(json: &str) -> Result<FlowFormula> {
        read(json, true)
    }

    /// `Flow.Serialize` — `Flow.Json` 꼴의 한 줄. 빈 자리는 적지 않는다.
    ///
    /// 유한하지 않은 수는 .NET 이 쓰기를 거절하므로(`ArgumentException`) 여기서도 [`Error::Argument`] 다 —
    /// [`Flow::parse`] 를 지난 식에는 그런 수가 없다.
    pub fn serialize(formula: &FlowFormula) -> Result<String> {
        Ok(json::to_compact(&formula_value(formula)?, Encoder::Relaxed))
    }

    /// `Flow.MigrateV1` — v1 → v2. `pick` 은 단계로 남기지 않고 뒤 `distinct` 의 기준 필드로 흡수한다.
    pub fn migrate_v1(formula: &ColumnFormula) -> Result<FlowFormula> {
        let mut picked: Vec<Option<String>> = Vec::new();
        let mut steps = Vec::new();
        for step in formula.steps.iter().flatten() {
            let Some(s) = step else {
                return Err(Error::invalid("가공식 형식이 올바르지 않습니다."));
            };
            let op = s.op.as_deref();
            let step = match op {
                Some("pick") => {
                    picked = s.fields.clone().unwrap_or_default();
                    continue;
                }
                Some("distinct") => FlowStep {
                    op: Some("distinct".into()),
                    fields: (!picked.is_empty()).then(|| picked.clone()),
                    ..FlowStep::default()
                },
                Some("at") => FlowStep {
                    op: Some("nth".into()),
                    n: Some(s.index.unwrap_or(1)),
                    ..FlowStep::default()
                },
                Some("get") => FlowStep {
                    op: Some("get".into()),
                    field: Some(s.text.clone().unwrap_or_default()),
                    ..FlowStep::default()
                },
                Some(o @ ("format" | "join")) => FlowStep {
                    op: Some(o.into()),
                    text: Some(s.text.clone().unwrap_or_default()),
                    ..FlowStep::default()
                },
                Some("count") => FlowStep {
                    op: Some("count".into()),
                    ..FlowStep::default()
                },
                other => {
                    return Err(Error::invalid(format!(
                        "가공식 v1 에 모르는 연산이 있습니다: {}",
                        other.unwrap_or("")
                    )));
                }
            };
            steps.push(Some(step));
        }
        Ok(FlowFormula {
            version: 2,
            id: Some(formula.id.clone().unwrap_or_default()),
            source: Some("flow".into()),
            flow: Some(FlowDefinition {
                from: Some(FlowSource {
                    kind: Some("items".into()),
                    ..FlowSource::default()
                }),
                steps: Some(steps),
            }),
            sql: None,
        })
    }
}

/// `Flow.Read`.
fn read(json: &str, identity_only: bool) -> Result<FlowFormula> {
    let bad_shape = || Error::invalid("가공식 형식이 올바르지 않습니다.");
    let bad_version = || Error::invalid("가공식의 판을 알 수 없습니다.");

    let root = json::parse(json).map_err(|_| bad_shape())?;
    // 같은 키가 두 번 든 식은 형식 오류다 — 어느 것을 고를지 읽는 쪽마다 달라(C# 의 JsonNode 는 던지고 본문 읽기는 뒤의
    // 것을 고른다) 저장된 식이 깨진 열로 서게 한다.
    if json::has_duplicate_keys(json) {
        return Err(bad_shape());
    }
    let Value::Object(object) = &root else {
        return Err(bad_version());
    };
    // 본문은 대소문자를 가리지 않고 읽으므로 판도 그렇게 고른다 — 여럿이면 본문 읽기처럼 뒤의 것이다.
    let version = match object
        .iter()
        .rev()
        .find(|(key, _)| eq_ignore_case(key, "version"))
        .map(|(_, value)| value)
    {
        Some(Value::Number(n)) => n.to_string().parse::<i32>().map_err(|_| bad_version())?,
        _ => return Err(bad_version()),
    };
    // 판올림에서 뜻이 갈린 식은 원문을 보존하고 깨진 열 진단으로 알린다. 사람이 고쳐 저장하면 이 진단은 떨어진다.
    if !identity_only && let Some(Value::Object(error)) = object.get("migrationError") {
        let mut current = object.clone();
        current.shift_remove("migrationError");
        let current = Value::Object(current);
        if json::deep_equals(Some(&current), error.get("original")) {
            let message = match error.get("message") {
                Some(Value::String(m)) => m.clone(),
                _ => "판올림 뒤 식을 확인하세요.".to_string(),
            };
            return Err(Error::InvalidOperation(message));
        }
    }

    match version {
        1 => {
            let old = read_column_formula(&root).map_err(|_| bad_shape())?;
            Flow::migrate_v1(&old)
        }
        2 => {
            let mut formula = read_formula(&root).map_err(|_| bad_shape())?;
            validate(&formula)?;
            formula.id.get_or_insert_with(String::new);
            Ok(formula)
        }
        _ => Err(bad_version()),
    }
}

/// `Flow.Validate` — JSON 이 담을 수 있는 모양인가(꼴은 `Check` 가 본다).
fn validate(formula: &FlowFormula) -> Result<()> {
    fn bad(why: &str) -> Error {
        Error::invalid(format!("가공식 형식이 올바르지 않습니다: {why}"))
    }
    fn text(value: Option<&str>) -> Result<()> {
        match value {
            Some(v) if utf16_len(v) > MAX_TEXT || v.contains('\0') => Err(Error::invalid(
                "글자 인자는 1000자 이내여야 하며 NUL 문자를 포함할 수 없습니다.",
            )),
            _ => Ok(()),
        }
    }
    fn check_clauses(cond: Option<&Vec<Option<FlowClause>>>) -> Result<()> {
        for clause in cond.into_iter().flatten() {
            let Some(clause) = clause else {
                return Err(bad("조건이 올바르지 않습니다"));
            };
            match (&clause.field, &clause.cmp) {
                (Some(_), Some(cmp)) if COMPARES.contains(&cmp.as_str()) => {}
                _ => return Err(bad("조건이 올바르지 않습니다")),
            }
            if let Some(join) = &clause.join
                && join != "and"
                && join != "or"
            {
                return Err(bad("조건은 and·or 로 잇습니다"));
            }
            match &clause.value {
                None | Some(Value::Number(_)) => {}
                Some(Value::String(s)) => text(Some(s))?,
                Some(_) => return Err(bad("조건의 값은 글이나 수입니다")),
            }
        }
        Ok(())
    }

    match formula.source.as_deref() {
        Some("sql") => {
            if formula.sql.as_deref().is_none_or(is_blank) {
                return Err(bad("SQL 이 비었습니다"));
            }
            return Ok(());
        }
        Some("flow") => {}
        _ => return Err(bad("source 는 flow 또는 sql 입니다")),
    }

    let flow = formula
        .flow
        .as_ref()
        .ok_or_else(|| bad("flow 가 없습니다"))?;
    let from = flow
        .from
        .as_ref()
        .ok_or_else(|| bad("시작 데이터가 없습니다"))?;
    let steps = flow
        .steps
        .as_ref()
        .ok_or_else(|| bad("steps 가 없습니다"))?;
    if steps.len() > MAX_STEPS {
        return Err(bad(&format!("단계는 {MAX_STEPS}개까지입니다")));
    }
    match from.kind.as_deref() {
        Some("items" | "number") => {}
        Some("text") => {
            if from.text.is_none() {
                return Err(bad("글자 시작 데이터가 비었습니다"));
            }
            text(from.text.as_deref())?;
        }
        Some("field") => {
            if from.field.is_none() {
                return Err(bad("열 시작 데이터가 비었습니다"));
            }
        }
        Some("linked") => {
            if !from.link.as_deref().is_some_and(|l| LINKS.contains(&l)) {
                return Err(bad("연결 문서가 올바르지 않습니다"));
            }
        }
        other => {
            return Err(bad(&format!(
                "알 수 없는 시작 데이터: {}",
                other.unwrap_or("")
            )));
        }
    }
    if from.kind.as_deref() == Some("number") && !from.value.is_some_and(f64::is_finite) {
        return Err(bad("수 시작 데이터가 비었습니다"));
    }

    for step in steps {
        let Some(step) = step.as_ref().filter(|s| s.op.is_some()) else {
            return Err(bad("op 가 없는 단계가 있습니다"));
        };
        for t in [
            &step.text,
            &step.to,
            &step.then,
            &step.r#else,
            &step.otherwise,
        ] {
            text(t.as_deref())?;
        }
        if step.dir.as_deref().is_some_and(|d| d != "desc") {
            return Err(bad("정렬 방향은 desc 만 적습니다"));
        }
        if step
            .unit
            .as_deref()
            .is_some_and(|u| !matches!(u, "day" | "month" | "year"))
        {
            return Err(bad("날짜 단위는 day·month·year 입니다"));
        }
        if step.value.is_some_and(|v| !v.is_finite()) {
            return Err(bad("수 인자가 올바르지 않습니다"));
        }
        if step
            .fields
            .as_ref()
            .is_some_and(|f| f.iter().any(Option::is_none))
        {
            return Err(bad("필드 목록에 빈 자리가 있습니다"));
        }
        for pair in step.pairs.iter().flatten() {
            match pair.as_deref() {
                Some([Some(a), Some(b)]) => {
                    text(Some(a))?;
                    text(Some(b))?;
                }
                _ => return Err(bad("대응은 [찾을 값, 바꿀 값] 짝입니다")),
            }
        }
        check_clauses(step.cond.as_ref())?;
        if let Some(branches) = &step.branches {
            // 갈래 수는 꼴 검사가 본다(화면과 같은 말) — 여기서는 담을 수 있는 크기만 막는다.
            if branches.len() > MAX_STEPS {
                return Err(bad(&format!("경우는 {MAX_STEPS}개까지 담습니다")));
            }
            for branch in branches {
                let Some(FlowBranch {
                    cond: Some(cond),
                    then: Some(then),
                }) = branch
                else {
                    return Err(bad("경우는 조건과 글의 짝입니다"));
                };
                text(Some(then))?;
                check_clauses(Some(cond))?;
            }
        }
    }
    Ok(())
}

// ── 읽기(Flow.Json 으로 역직렬화) ──────────────────────────────────────

fn read_formula(value: &Value) -> JsonResult<FlowFormula> {
    let map = read_object(value)?.ok_or(JsonException)?;
    let mut f = FlowFormula {
        version: 0,
        id: None,
        source: None,
        flow: None,
        sql: None,
    };
    for_each_property(
        map,
        &["version", "id", "source", "flow", "sql"],
        false,
        |i, v| {
            match i {
                0 => f.version = read_required_i32(v)?,
                1 => f.id = read_string(v)?,
                2 => f.source = read_string(v)?,
                3 => f.flow = read_optional(v, read_definition)?,
                _ => f.sql = read_string(v)?,
            }
            Ok(())
        },
    )?;
    Ok(f)
}

/// 객체 자리 하나를 `read` 로 읽는다. `null` 은 `None`.
fn read_optional<T>(
    value: &Value,
    read: fn(&Map<String, Value>) -> JsonResult<T>,
) -> JsonResult<Option<T>> {
    read_object(value)?.map(read).transpose()
}

fn read_definition(map: &Map<String, Value>) -> JsonResult<FlowDefinition> {
    let mut d = FlowDefinition {
        from: None,
        steps: None,
    };
    for_each_property(map, &["from", "steps"], false, |i, v| {
        match i {
            0 => d.from = read_optional(v, read_source)?,
            _ => d.steps = read_array(v, |s| read_optional(s, read_step))?,
        }
        Ok(())
    })?;
    Ok(d)
}

fn read_source(map: &Map<String, Value>) -> JsonResult<FlowSource> {
    let mut s = FlowSource::default();
    for_each_property(
        map,
        &["kind", "field", "link", "items", "text", "value"],
        false,
        |i, v| {
            match i {
                0 => s.kind = read_string(v)?,
                1 => s.field = read_string(v)?,
                2 => s.link = read_string(v)?,
                3 => s.items = read_bool(v)?,
                4 => s.text = read_string(v)?,
                _ => s.value = read_f64(v)?,
            }
            Ok(())
        },
    )?;
    Ok(s)
}

fn read_clause(map: &Map<String, Value>) -> JsonResult<FlowClause> {
    let mut c = FlowClause {
        join: None,
        field: None,
        cmp: None,
        value: None,
    };
    for_each_property(map, &["join", "field", "cmp", "value"], false, |i, v| {
        match i {
            0 => c.join = read_string(v)?,
            1 => c.field = read_string(v)?,
            2 => c.cmp = read_string(v)?,
            // JsonElement? — 어떤 값이든 그대로 담는다. JSON null 은 빈 자리다.
            _ => c.value = (!v.is_null()).then(|| v.clone()),
        }
        Ok(())
    })?;
    Ok(c)
}

fn read_clauses(value: &Value) -> JsonResult<Option<Vec<Option<FlowClause>>>> {
    read_array(value, |c| read_optional(c, read_clause))
}

fn read_strings(value: &Value) -> JsonResult<Option<Vec<Option<String>>>> {
    read_array(value, read_string)
}

fn read_branch(map: &Map<String, Value>) -> JsonResult<FlowBranch> {
    let mut b = FlowBranch {
        cond: None,
        then: None,
    };
    for_each_property(map, &["cond", "then"], false, |i, v| {
        match i {
            0 => b.cond = read_clauses(v)?,
            _ => b.then = read_string(v)?,
        }
        Ok(())
    })?;
    Ok(b)
}

const STEP_NAMES: [&str; 16] = [
    "op",
    "field",
    "fields",
    "text",
    "to",
    "n",
    "length",
    "dir",
    "unit",
    "value",
    "cond",
    "then",
    "else",
    "pairs",
    "otherwise",
    "branches",
];

fn read_step(map: &Map<String, Value>) -> JsonResult<FlowStep> {
    let mut s = FlowStep::default();
    for_each_property(map, &STEP_NAMES, false, |i, v| {
        match i {
            0 => s.op = read_string(v)?,
            1 => s.field = read_string(v)?,
            2 => s.fields = read_strings(v)?,
            3 => s.text = read_string(v)?,
            4 => s.to = read_string(v)?,
            5 => s.n = read_i32(v)?,
            6 => s.length = read_i32(v)?,
            7 => s.dir = read_string(v)?,
            8 => s.unit = read_string(v)?,
            9 => s.value = read_f64(v)?,
            10 => s.cond = read_clauses(v)?,
            11 => s.then = read_string(v)?,
            12 => s.r#else = read_string(v)?,
            13 => s.pairs = read_array(v, read_strings)?,
            14 => s.otherwise = read_string(v)?,
            _ => s.branches = read_array(v, |b| read_optional(b, read_branch))?,
        }
        Ok(())
    })?;
    Ok(s)
}

fn read_column_formula(value: &Value) -> JsonResult<ColumnFormula> {
    let map = read_object(value)?.ok_or(JsonException)?;
    let mut f = ColumnFormula {
        version: 0,
        id: None,
        steps: None,
    };
    for_each_property(map, &["version", "id", "steps"], false, |i, v| {
        match i {
            0 => f.version = read_required_i32(v)?,
            1 => f.id = read_string(v)?,
            _ => f.steps = read_array(v, |s| read_optional(s, read_column_step))?,
        }
        Ok(())
    })?;
    Ok(f)
}

fn read_column_step(map: &Map<String, Value>) -> JsonResult<ColumnStep> {
    let mut s = ColumnStep::default();
    for_each_property(map, &["op", "fields", "text", "index"], false, |i, v| {
        match i {
            0 => s.op = read_string(v)?,
            1 => s.fields = read_strings(v)?,
            2 => s.text = read_string(v)?,
            _ => s.index = read_i32(v)?,
        }
        Ok(())
    })?;
    Ok(s)
}

// ── 쓰기(Flow.Json 으로 직렬화 — 빈 자리는 적지 않는다) ─────────────────

struct Writer(Map<String, Value>);

impl Writer {
    fn new() -> Self {
        Writer(Map::new())
    }
    fn put(&mut self, name: &str, value: Option<Value>) {
        if let Some(v) = value {
            self.0.insert(name.to_string(), v);
        }
    }
    fn done(self) -> Value {
        Value::Object(self.0)
    }
}

fn string(value: &Option<String>) -> Option<Value> {
    value.clone().map(Value::String)
}

fn double(value: Option<f64>) -> Result<Option<Value>> {
    value
        .map(|d| {
            if d.is_finite() {
                Ok(json::number(&json::format_double(d)))
            } else {
                Err(Error::Argument(format!(
                    "{d} 는 JSON 에 쓸 수 없는 수입니다(유한하지 않음)."
                )))
            }
        })
        .transpose()
}

fn strings(value: &Option<Vec<Option<String>>>) -> Option<Value> {
    value.as_ref().map(|items| {
        Value::Array(
            items
                .iter()
                .map(|s| s.clone().map_or(Value::Null, Value::String))
                .collect(),
        )
    })
}

fn list<T>(
    value: &Option<Vec<Option<T>>>,
    write: impl Fn(&T) -> Result<Value>,
) -> Result<Option<Value>> {
    value
        .as_ref()
        .map(|items| {
            items
                .iter()
                .map(|item| item.as_ref().map_or(Ok(Value::Null), &write))
                .collect::<Result<Vec<_>>>()
                .map(Value::Array)
        })
        .transpose()
}

/// `FlowFormula` 를 JSON 값으로 — 저장 꼴의 키 차례와 수 표기 그대로.
pub(crate) fn formula_value(f: &FlowFormula) -> Result<Value> {
    let mut w = Writer::new();
    w.put("version", Some(json::number(&f.version.to_string())));
    w.put("id", string(&f.id));
    w.put("source", string(&f.source));
    w.put("flow", f.flow.as_ref().map(definition_value).transpose()?);
    w.put("sql", string(&f.sql));
    Ok(w.done())
}

fn definition_value(d: &FlowDefinition) -> Result<Value> {
    let mut w = Writer::new();
    w.put("from", d.from.as_ref().map(source_value).transpose()?);
    w.put("steps", list(&d.steps, step_value)?);
    Ok(w.done())
}

fn source_value(s: &FlowSource) -> Result<Value> {
    let mut w = Writer::new();
    w.put("kind", string(&s.kind));
    w.put("field", string(&s.field));
    w.put("link", string(&s.link));
    w.put("items", s.items.map(Value::Bool));
    w.put("text", string(&s.text));
    w.put("value", double(s.value)?);
    Ok(w.done())
}

fn clause_value(c: &FlowClause) -> Result<Value> {
    let mut w = Writer::new();
    w.put("join", string(&c.join));
    w.put("field", string(&c.field));
    w.put("cmp", string(&c.cmp));
    w.put("value", c.value.clone());
    Ok(w.done())
}

fn branch_value(b: &FlowBranch) -> Result<Value> {
    let mut w = Writer::new();
    w.put("cond", list(&b.cond, clause_value)?);
    w.put("then", string(&b.then));
    Ok(w.done())
}

fn step_value(s: &FlowStep) -> Result<Value> {
    let int = |n: Option<i32>| n.map(|n| json::number(&n.to_string()));
    let mut w = Writer::new();
    w.put("op", string(&s.op));
    w.put("field", string(&s.field));
    w.put("fields", strings(&s.fields));
    w.put("text", string(&s.text));
    w.put("to", string(&s.to));
    w.put("n", int(s.n));
    w.put("length", int(s.length));
    w.put("dir", string(&s.dir));
    w.put("unit", string(&s.unit));
    w.put("value", double(s.value)?);
    w.put("cond", list(&s.cond, clause_value)?);
    w.put("then", string(&s.then));
    w.put("else", string(&s.r#else));
    w.put(
        "pairs",
        s.pairs.as_ref().map(|pairs| {
            Value::Array(
                pairs
                    .iter()
                    .map(|p| strings(p).unwrap_or(Value::Null))
                    .collect(),
            )
        }),
    );
    w.put("otherwise", string(&s.otherwise));
    w.put("branches", list(&s.branches, branch_value)?);
    Ok(w.done())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round(json: &str) -> String {
        match Flow::parse(json) {
            Ok(f) => Flow::serialize(&f).unwrap(),
            Err(e) => format!("ERR {e}"),
        }
    }

    /// 입력과 기대값은 C# `Flow.Serialize(Flow.Parse(입력))` 의 실측(scratch 콘솔, 값은 지어낸 것).
    #[test]
    fn parse_then_serialize_matches_csharp() {
        let cases: &[(&str, &str)] = &[
            (
                r#"{"version":2,"source":"flow","flow":{"from":{"kind":"number","value":"1.50"},"steps":[]}}"#,
                r#"{"version":2,"id":"","source":"flow","flow":{"from":{"kind":"number","value":1.5},"steps":[]}}"#,
            ),
            (
                r#"{"version":2,"source":"flow","flow":{"from":{"kind":"items"},"steps":[{"op":"take","n":"3"}]}}"#,
                r#"{"version":2,"id":"","source":"flow","flow":{"from":{"kind":"items"},"steps":[{"op":"take","n":3}]}}"#,
            ),
            (
                r#"{"Version":2,"ID":"x","Source":"flow","Flow":{"From":{"Kind":"items"},"Steps":[]}}"#,
                r#"{"version":2,"id":"x","source":"flow","flow":{"from":{"kind":"items"},"steps":[]}}"#,
            ),
            (
                r#"{"version":2,"id":"x","source":"flow","flow":{"from":{"kind":"items"},"steps":[]},"extra":1}"#,
                r#"{"version":2,"id":"x","source":"flow","flow":{"from":{"kind":"items"},"steps":[]}}"#,
            ),
            (
                r#"{"version":2,"source":"flow","flow":{"from":{"kind":"items"},"steps":[{"op":"filter","cond":[{"field":"a","cmp":"=","value":1.50}]}]}}"#,
                r#"{"version":2,"id":"","source":"flow","flow":{"from":{"kind":"items"},"steps":[{"op":"filter","cond":[{"field":"a","cmp":"=","value":1.50}]}]}}"#,
            ),
            (
                "{\"version\":2,\"source\":\"flow\",\"flow\":{\"from\":{\"kind\":\"items\"},\"steps\":[{\"op\":\"filter\",\"cond\":[{\"field\":\"a<>&'+é😀\\u2028\\u00ad\",\"cmp\":\"=\",\"value\":\"x\\u0041\"}]}]}}",
                "{\"version\":2,\"id\":\"\",\"source\":\"flow\",\"flow\":{\"from\":{\"kind\":\"items\"},\"steps\":[{\"op\":\"filter\",\"cond\":[{\"field\":\"a<>&'+é\\uD83D\\uDE00\\u2028\u{AD}\",\"cmp\":\"=\",\"value\":\"xA\"}]}]}}",
            ),
            (
                r#"{"version":2,"source":"flow","flow":{"from":{"kind":"number","value":1E2},"steps":[{"op":"add","value":1.0}]}}"#,
                r#"{"version":2,"id":"","source":"flow","flow":{"from":{"kind":"number","value":100},"steps":[{"op":"add","value":1}]}}"#,
            ),
            (
                r#"{"version":2,"source":"flow","flow":{"from":{"kind":"items"},"steps":[{"op":"take","n":3.0}]}}"#,
                "ERR 가공식 형식이 올바르지 않습니다.",
            ),
            (
                r#"{"version":"2","source":"flow","flow":{"from":{"kind":"items"},"steps":[]}}"#,
                "ERR 가공식의 판을 알 수 없습니다.",
            ),
            (
                r#"{"version":2.0,"source":"flow","flow":{"from":{"kind":"items"},"steps":[]}}"#,
                "ERR 가공식의 판을 알 수 없습니다.",
            ),
            (
                r#"{"version":1,"id":"k","steps":[{"op":"pick","fields":["a","b"]},{"op":"distinct"},{"op":"at","index":2},{"op":"get","text":"a"}]}"#,
                r#"{"version":2,"id":"k","source":"flow","flow":{"from":{"kind":"items"},"steps":[{"op":"distinct","fields":["a","b"]},{"op":"nth","n":2},{"op":"get","field":"a"}]}}"#,
            ),
            (
                r#"{"version":2,"source":"sql","sql":"SELECT 1","flow":null}"#,
                r#"{"version":2,"id":"","source":"sql","sql":"SELECT 1"}"#,
            ),
            (
                r#"{"version":2,"source":"flow","flow":{"from":{"kind":"items"},"steps":[{"op":"map","pairs":[["a","b"]]},{"op":"cases","branches":[{"cond":[{"field":"","cmp":"=","value":"x"}],"then":"y"}]}]}}"#,
                r#"{"version":2,"id":"","source":"flow","flow":{"from":{"kind":"items"},"steps":[{"op":"map","pairs":[["a","b"]]},{"op":"cases","branches":[{"cond":[{"field":"","cmp":"=","value":"x"}],"then":"y"}]}]}}"#,
            ),
            (
                r#"{"version":2,"source":"flow","flow":{"from":{"kind":"items"},"steps":[{"op":"take","Op":"nth"}]}}"#,
                r#"{"version":2,"id":"","source":"flow","flow":{"from":{"kind":"items"},"steps":[{"op":"nth"}]}}"#,
            ),
            (
                r#"{"version":2,"source":"flow","flow":{"from":{"kind":"number","value":1e400},"steps":[]}}"#,
                "ERR 가공식 형식이 올바르지 않습니다: 수 시작 데이터가 비었습니다",
            ),
            (
                r#"{"version":2,"source":"flow","flow":{"from":{"kind":"items"},"steps":[{"op":"filter","cond":[{"field":"a","cmp":"=","value":true}]}]}}"#,
                "ERR 가공식 형식이 올바르지 않습니다: 조건의 값은 글이나 수입니다",
            ),
            (
                r#"{"version":2,"source":"flow","flow":{"from":{"kind":"items"},"steps":[{"op":"filter","cond":[{"field":"a","cmp":"=","value":null}]}]}}"#,
                r#"{"version":2,"id":"","source":"flow","flow":{"from":{"kind":"items"},"steps":[{"op":"filter","cond":[{"field":"a","cmp":"="}]}]}}"#,
            ),
            (
                r#"{"version":2,"source":"flow","flow":{"from":{"kind":"items"},"steps":[{"op":"filter","cond":[{"field":"a","cmp":"=","value":-0.0}]}]}}"#,
                r#"{"version":2,"id":"","source":"flow","flow":{"from":{"kind":"items"},"steps":[{"op":"filter","cond":[{"field":"a","cmp":"=","value":-0.0}]}]}}"#,
            ),
            (
                r#"{"version":2,"source":"flow","flow":{"from":{"kind":"number","value":-0.0},"steps":[]}}"#,
                r#"{"version":2,"id":"","source":"flow","flow":{"from":{"kind":"number","value":-0},"steps":[]}}"#,
            ),
            (
                r#"{"version":2,"source":"flow","flow":{"from":{"kind":"items","items":"true"},"steps":[]}}"#,
                "ERR 가공식 형식이 올바르지 않습니다.",
            ),
        ];
        for (input, expected) in cases {
            assert_eq!(&round(input), expected, "{input}");
        }
    }

    /// C# `판은_대소문자를_가리지_않고_겹친_키는_형식_오류다` — 판은 본문처럼 대소문자를 가리지 않고 고르고(여럿이면 뒤의 것),
    /// 같은 키가 두 번 든 식은 어디에 들었든 형식 오류다.
    #[test]
    fn version_is_case_insensitive_and_duplicate_keys_are_malformed() {
        let expected =
            r#"{"version":2,"id":"x","source":"flow","flow":{"from":{"kind":"items"},"steps":[]}}"#;
        assert_eq!(round(expected), expected);
        assert_eq!(
            round(
                r#"{"version":7,"VERSION":2,"id":"x","source":"flow","flow":{"from":{"kind":"items"},"steps":[]}}"#
            ),
            expected
        );
        for duplicated in [
            r#"{"version":2,"version":2,"id":"x","source":"flow","flow":{"from":{"kind":"items"},"steps":[]}}"#,
            r#"{"version":2,"id":"x","source":"flow","flow":{"from":{"kind":"items","kind":"items"},"steps":[]}}"#,
            r#"{"version":2,"id":"x","\u0069d":"y","source":"flow","flow":{"from":{"kind":"items"},"steps":[]}}"#,
        ] {
            assert_eq!(
                round(duplicated),
                "ERR 가공식 형식이 올바르지 않습니다.",
                "{duplicated}"
            );
            assert_eq!(
                Flow::read_for_identity(duplicated).unwrap_err().to_string(),
                "가공식 형식이 올바르지 않습니다.",
                "{duplicated}"
            );
        }
    }

    /// C# `정수가_아닌_판은_판을_알_수_없다` — 판이 정수가 아닌 수면 「가공식의 판을 알 수 없습니다.」 다. 저장·미리보기가
    /// 받는 꼴(`parse_for`)도 같다.
    #[test]
    fn non_integer_version_is_unknown() {
        for version in ["1.5", "2147483648", "1e0", "1.0"] {
            let json = format!(
                r#"{{"version":{version},"steps":[{{"op":"pick","fields":["없는필드"]}}]}}"#
            );
            assert_eq!(
                Flow::parse(&json).unwrap_err().to_string(),
                "가공식의 판을 알 수 없습니다.",
                "{version}"
            );
            assert_eq!(
                crate::storage::computed_column::parse_for("contract", &json)
                    .unwrap_err()
                    .to_string(),
                "가공식의 판을 알 수 없습니다.",
                "{version}"
            );
        }
    }

    /// C# `짝_없는_서로게이트와_너무_깊은_글은_형식_오류다` — 짝 없는 서로게이트 이스케이프가 든 글과 64 단계보다 깊은 글은
    /// 읽을 수 없는 글이다. .NET 의 깊이 한도가 64 라 65~128 단계는 serde_json(한도 128)만 읽고 있었다.
    #[test]
    fn lone_surrogates_and_too_deep_text_are_malformed() {
        let body = r#""source":"flow","flow":{"from":{"kind":"items"},"steps":[]}}"#;
        let padded = |arrays: usize| {
            format!(
                r#"{{"version":2,"pad":{}{},{body}"#,
                "[".repeat(arrays),
                "]".repeat(arrays)
            )
        };
        assert!(Flow::parse(&padded(63)).is_ok());
        for json in [
            format!(r#"{{"version":2,"\ud800":1,{body}"#),
            format!(r#"{{"version":2,"id":"\udc00x",{body}"#),
            padded(64),
        ] {
            assert_eq!(
                Flow::parse(&json).unwrap_err().to_string(),
                "가공식 형식이 올바르지 않습니다.",
                "{json}"
            );
            assert_eq!(
                crate::storage::computed_column::parse_for("contract", &json)
                    .unwrap_err()
                    .to_string(),
                "가공식 형식이 올바르지 않습니다.",
                "{json}"
            );
        }
    }

    #[test]
    fn migration_error_blocks_until_edited() {
        let original =
            r#"{"version":2,"source":"flow","flow":{"from":{"kind":"items"},"steps":[]}}"#;
        let marked = r#"{"version":2,"source":"flow","flow":{"from":{"kind":"items"},"steps":[]},"migrationError":{"message":"고치세요","original":{"source":"flow","version":2,"flow":{"steps":[],"from":{"kind":"items"}}}}}"#;
        assert_eq!(Flow::parse(marked).unwrap_err().to_string(), "고치세요");
        assert!(Flow::read_for_identity(marked).is_ok());
        assert!(Flow::parse(original).is_ok());
    }
}

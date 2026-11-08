//! C# `StoreColumns.cs` — 깨진 가공열, 값 흐름 보표, SQL 꺼내기·검증, 값으로 굳히기.

use rusqlite::{Connection, params};
use serde_json::{Map, Value};

use super::{
    Store, begin_deferred, face_view, object, query, single, single_or_default, strings,
    strings_json, table_for, text_of,
};
use crate::dict::Dict;
use crate::error::{Error, Result};
use crate::json::{self, Encoder};
use crate::storage::database::{self, refresh};
use crate::storage::flow::{Flow, FlowFormula, FlowStep};
use crate::storage::flow_check::FlowEnv;
use crate::storage::flow_compiler::{self, Part, Target};
use crate::storage::sql_column::{self, Frame};
use crate::storage::{computed_column, views};

/// `Store.TraceSamples` — 보표가 보이는 표본 수.
pub(super) const TRACE_SAMPLES: usize = 3;
/// `Store.ShowItems`·`Store.ShowRows` — 칸 하나에 보이는 목록 원소·행 수.
const SHOW_ITEMS: usize = 3;
const SHOW_ROWS: usize = 5;

/// `Store.FaceBase` — 면 뷰의 본번호 열.
fn face_base(entity_type: &str) -> &'static str {
    match entity_type {
        "notice" => "공고본번호",
        "request" => "접수본번호",
        _ => "계약본번호",
    }
}

/// C# `Store.Sample(Base, Revision, Title, Key)` — 보표의 표본 하나.
pub(super) struct Sample {
    pub(super) key: String,
}

/// `Store.Samples` — 최신 차수 문서 `count` 개와 그 화면 열쇠.
fn samples(
    connection: &Connection,
    entity_type: &str,
    count: usize,
) -> Result<(Vec<Sample>, Vec<Value>)> {
    let separator = if entity_type == "contract" { "" } else { "-" };
    let rows = query(
        connection,
        &format!(
            "SELECT p.{e}_base, p.seq, coalesce(p.title,'') FROM ({parents}) p\n\
             ORDER BY p.{e}_base LIMIT {count}",
            e = entity_type,
            parents = super::latest_column_parents(entity_type)?,
        ),
        [],
        |r| {
            Ok((
                text_of(r.get_ref(0)?).unwrap_or_default(),
                text_of(r.get_ref(1)?).unwrap_or_default(),
                text_of(r.get_ref(2)?).unwrap_or_default(),
            ))
        },
    )?;
    let heads = rows
        .iter()
        .map(|(b, r, t)| {
            object([
                ("base", Value::String(b.clone())),
                ("revision", Value::String(r.clone())),
                ("title", Value::String(t.clone())),
            ])
        })
        .collect();
    let samples = rows
        .into_iter()
        .map(|(b, r, _)| Sample {
            key: format!("{b}{separator}{r}"),
        })
        .collect();
    Ok((samples, heads))
}

/// C# `Store.FreezePlan(Kind, Cells, Hidden, Series)` — 값으로 굳히기 전에 세는 것. `cells` 는 값이 있는 칸,
/// `hidden` 은 식에 가려 숨어 있던 사람 값, `series` 는 면 뷰의 줄 수다.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FreezePlan {
    pub kind: String,
    pub cells: i64,
    pub hidden: i64,
    pub series: i64,
}

impl FreezePlan {
    pub fn to_json(&self) -> Value {
        object([
            ("kind", Value::String(self.kind.clone())),
            ("cells", self.cells.into()),
            ("hidden", self.hidden.into()),
            ("series", self.series.into()),
        ])
    }
}

/// `Store.TraceFrame(Row, Final, Sql)` — 보표가 서는 자리. 가공열은 면 뷰 한 줄, 맞춤 뷰는 사슬 위의 건.
pub(super) struct TraceFrame<'a> {
    pub(super) row: Box<dyn Fn(&str) -> String + 'a>,
    pub(super) r#final: Box<dyn Fn(&str) -> String + 'a>,
    #[allow(clippy::type_complexity)]
    pub(super) sql: Box<dyn Fn(&Connection, &str, &[String]) -> Result<Vec<String>> + 'a>,
}

/// `Store.FaceTrace`.
fn face_trace(entity_type: &str) -> TraceFrame<'_> {
    let row = move |select: &str| {
        let frame = Frame::Face(entity_type);
        frame.statement(select, Some(&format!("{} = @key", frame.key())), "")
    };
    TraceFrame {
        row: Box::new(row),
        r#final: Box::new(row),
        sql: Box::new(move |connection, sql, keys| {
            sql_column::validate_on(connection, sql, &Frame::Face(entity_type), keys, 20)
        }),
    }
}

impl Store<'_> {
    /// `Store.BrokenColumns` — 깨진 가공열((종류, 이름) → 까닭). 면 뷰의 정의 글에 박힌 표지를 읽는다.
    pub fn broken_columns(&self) -> Result<Dict<(String, String), String>> {
        let connection = self.database.open_read_only()?;
        let mut found = Dict::new();
        for entity in ["notice", "contract", "request"] {
            let sql = single_or_default(
                &connection,
                "SELECT sql FROM sqlite_master WHERE type = 'view' AND name = ?1",
                [face_view(entity)],
                |r| Ok(text_of(r.get_ref(0)?)),
            )?
            .flatten();
            for (name, why) in views::broken_in(sql.as_deref()) {
                found.insert(
                    (entity.to_string(), name),
                    if why.is_empty() {
                        "식을 계산하지 못했습니다.".to_string()
                    } else {
                        why
                    },
                );
            }
        }
        Ok(found)
    }

    // ── 값 흐름 보표 ─────────────────────────────────────────

    /// `Store.TraceComputedColumn` — 표본 셋 위에서 단계마다 값이 어떻게 흐르는지(§11.3).
    pub fn trace_computed_column(&self, entity_type: &str, json: &str) -> Result<Value> {
        let formula = computed_column::parse_for(entity_type, json)?;
        let connection = sql_column::open_checked(&self.database)?;
        let _transaction = begin_deferred(&connection)?;
        let (samples, heads) = samples(&connection, entity_type, TRACE_SAMPLES)?;
        trace(
            &connection,
            entity_type,
            &formula,
            &Flow::env_for(entity_type)?,
            &samples,
            heads,
            &face_trace(entity_type),
        )
    }

    // ── SQL 꺼내기·검증 ─────────────────────────────────────────

    /// `Store.CompileComputedColumn` — 「SQL로 꺼내기」. SQL 열은 그 글 그대로다.
    pub fn compile_computed_column(&self, entity_type: &str, json: &str) -> Result<String> {
        let formula = computed_column::parse_for(entity_type, json)?;
        if formula.source.as_deref() == Some("sql") {
            return Ok(formula.sql.unwrap_or_default());
        }
        flow_compiler::eject(entity_type, &formula, None)
    }

    /// `Store.ValidateComputedSql` — SQL 열을 표본 위에서 검증한다. 거절은 결과에 담아 낸다.
    pub fn validate_computed_sql(&self, entity_type: &str, sql: &str) -> Result<Value> {
        computed_column::fields(entity_type)?;
        let connection = sql_column::open_checked(&self.database)?;
        let _transaction = begin_deferred(&connection)?;
        let (samples, heads) = samples(&connection, entity_type, TRACE_SAMPLES)?;
        let keys: Vec<String> = samples.iter().map(|s| s.key.clone()).collect();
        match sql_column::validate(&connection, entity_type, sql, &keys) {
            Ok(values) => Ok(object([
                ("ok", Value::Bool(true)),
                ("error", Value::Null),
                ("samples", Value::Array(heads)),
                ("final", strings_json(&values)),
            ])),
            Err(e @ (Error::InvalidOperation(_) | Error::Sqlite(_))) => Ok(object([
                ("ok", Value::Bool(false)),
                ("error", Value::String(sql_column::error_message(&e))),
                ("samples", Value::Array(heads)),
                ("final", strings_json(&vec![String::new(); samples.len()])),
            ])),
            Err(other) => Err(other),
        }
    }

    // ── 값으로 굳히기 ─────────────────────────────────────────

    /// `Store.PlanFreezeColumn` — 굳히면 몇 칸이 쓰이는지 센다. 굳히기 전에 반드시 이것을 보인다.
    pub fn plan_freeze_column(&self, entity_type: &str, field_name: &str) -> Result<FreezePlan> {
        let connection = self.database.open_read_only()?;
        let _transaction = begin_deferred(&connection)?;
        Ok(plan_freeze(&connection, entity_type, field_name)?.0)
    }

    /// `Store.FreezeColumn` — 가공열이 지금 내는 값을 사람 값으로 적고 열을 입력열로 바꾼다(식은 남긴다). 센 수
    /// (`expected`)와 지금 센 수가 다르면 아무것도 쓰지 않는다.
    pub fn freeze_column(
        &self,
        entity_type: &str,
        field_name: &str,
        expected: Option<i64>,
    ) -> Result<i64> {
        let connection = self.database.open()?;
        let transaction = database::begin(&connection)?;
        let (plan, values) = plan_freeze(&transaction, entity_type, field_name)?;
        if plan.kind != "computed" {
            return Err(Error::invalid("가공열만 값으로 굳힐 수 있습니다."));
        }
        if let Some(want) = expected
            && want != plan.cells
        {
            return Err(Error::InvalidOperation(format!(
                "칸을 센 뒤에 자료가 바뀌어({want}칸 → {}칸) 아무것도 쓰지 않았습니다. 다시 열어 새로 센 수를 확인하세요.",
                plan.cells
            )));
        }
        let (table, base_column) = table_for(entity_type)?;
        let now = self.env.now_utc();
        let mut written = 0i64;
        for (base, value) in &values {
            if value.is_empty() {
                transaction.execute(
                    &format!("DELETE FROM {table} WHERE {base_column} = ?1 AND field_name = ?2"),
                    params![base, field_name],
                )?;
                continue;
            }
            written += transaction.execute(
                &format!(
                    "INSERT INTO {table} ({base_column}, field_name, value, updated_at) VALUES (?1, ?2, ?3, ?4)\n\
                     ON CONFLICT({base_column}, field_name) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at"
                ),
                params![base, field_name, value, now],
            )? as i64;
        }
        transaction.execute(
            "UPDATE user_column SET kind = 'text' WHERE entity_type = ?1 AND field_name = ?2",
            params![entity_type, field_name],
        )?;
        refresh(&transaction)?;
        if written != plan.cells {
            return Err(Error::InvalidOperation(format!(
                "센 칸({})과 쓸 칸({written})이 달라 멈췄습니다. 아무것도 쓰지 않았습니다.",
                plan.cells
            )));
        }
        transaction.commit()?;
        Ok(written)
    }
}

/// `Store.PlanFreeze` — 센 것과 쓸 값(본번호, 값).
fn plan_freeze(
    connection: &Connection,
    entity_type: &str,
    field_name: &str,
) -> Result<(FreezePlan, Vec<(String, String)>)> {
    let (table, base_column) = table_for(entity_type)?;
    let kind = single_or_default(
        connection,
        "SELECT kind FROM user_column WHERE entity_type = ?1 AND field_name = ?2",
        params![entity_type, field_name],
        |r| Ok(text_of(r.get_ref(0)?)),
    )?
    .flatten()
    .ok_or_else(|| Error::InvalidOperation(format!("‘{field_name}’ 열을 찾을 수 없습니다.")))?;
    let stored: std::collections::HashSet<String> = strings(
        connection,
        &format!("SELECT {base_column} FROM {table} WHERE field_name = ?1"),
        [field_name],
    )?
    .into_iter()
    .collect();
    if kind != "computed" {
        return Ok((
            FreezePlan {
                kind,
                cells: 0,
                hidden: stored.len() as i64,
                series: 0,
            },
            Vec::new(),
        ));
    }
    let values = query(
        connection,
        &format!(
            "SELECT \"{}\", coalesce(CAST({} AS TEXT), '') FROM \"{}\"",
            face_base(entity_type),
            flow_compiler::ident(field_name),
            face_view(entity_type)
        ),
        [],
        |r| {
            Ok((
                text_of(r.get_ref(0)?).unwrap_or_default(),
                text_of(r.get_ref(1)?).unwrap_or_default(),
            ))
        },
    )?;
    let plan = FreezePlan {
        kind,
        cells: values.iter().filter(|v| !v.1.is_empty()).count() as i64,
        hidden: values.iter().filter(|v| stored.contains(&v.0)).count() as i64,
        series: values.len() as i64,
    };
    Ok((plan, values))
}

/// `Store.Trace` — 단계마다 표본 값을 보이는 보표. 앞에서 깨지면 뒤 단계는 막힌 것으로 둔다.
pub(super) fn trace(
    connection: &Connection,
    entity_type: &str,
    formula: &FlowFormula,
    env: &FlowEnv,
    samples: &[Sample],
    heads: Vec<Value>,
    frame: &TraceFrame<'_>,
) -> Result<Value> {
    let blanks = || -> Vec<String> { vec![String::new(); samples.len()] };
    if formula.source.as_deref() == Some("sql") {
        let keys: Vec<String> = samples.iter().map(|s| s.key.clone()).collect();
        let mut error = None;
        let mut values = blanks();
        match (frame.sql)(connection, formula.sql.as_deref().unwrap_or(""), &keys) {
            Ok(v) => values = v,
            Err(e @ (Error::InvalidOperation(_) | Error::Sqlite(_))) => {
                error = Some(sql_column::error_message(&e));
            }
            Err(other) => return Err(other),
        }
        let shown = values.iter().map(|v| value(v)).collect();
        let r#final = if error.is_none() { values } else { blanks() };
        return Ok(object([
            ("samples", Value::Array(heads)),
            (
                "steps",
                Value::Array(vec![step(Some("value"), shown, error, false)]),
            ),
            ("final", strings_json(&r#final)),
            (
                "sql",
                formula.sql.clone().map_or(Value::Null, Value::String),
            ),
        ]));
    }

    let flow = formula
        .flow
        .as_ref()
        .ok_or_else(|| Error::invalid("Object reference not set to an instance of an object."))?;
    let checked = Flow::check(flow, env);
    let mut stop = checked.errors.iter().find(|e| !e.r#final).map(|e| e.index);
    let mut steps = Vec::new();
    let mut counts: Option<Vec<i64>> = None;
    let mut previous: Option<Part> = None;
    let flow_steps: Vec<Option<&FlowStep>> =
        flow.steps.iter().flatten().map(Option::as_ref).collect();
    for k in 0..=flow_steps.len() {
        let shape = checked.shapes[k];
        let error = checked
            .errors
            .iter()
            .find(|e| e.index == k)
            .map(|e| e.message.clone());
        let blank_values = || samples.iter().map(|_| blank()).collect::<Vec<_>>();
        if let Some(s) = stop
            && k > s
        {
            steps.push(step(shape, blank_values(), None, true));
            continue;
        }
        if shape.is_none() {
            steps.push(step(shape, blank_values(), error, false));
            continue;
        }
        let attempt = (|| -> Result<(Part, Vec<(Value, i64)>)> {
            let part = flow_compiler::prefix(entity_type, flow, k, Target::Internal, Some(env))?;
            let current = if k == 0 { None } else { flow_steps[k - 1] };
            let mut shown = Vec::with_capacity(samples.len());
            for (i, sample) in samples.iter().enumerate() {
                shown.push(show(
                    connection,
                    &*frame.row,
                    &sample.key,
                    &part,
                    previous.as_ref(),
                    current,
                    counts.as_ref().map(|c| c[i]),
                )?);
            }
            Ok((part, shown))
        })();
        match attempt {
            Ok((part, shown)) => {
                steps.push(step(
                    shape,
                    shown.iter().map(|x| x.0.clone()).collect(),
                    error,
                    false,
                ));
                counts = Some(shown.iter().map(|x| x.1).collect());
                previous = Some(part);
            }
            Err(e @ (Error::InvalidOperation(_) | Error::Sqlite(_))) => {
                steps.push(step(
                    shape,
                    blank_values(),
                    Some(error.unwrap_or_else(|| {
                        format!("계산하지 못했습니다: {}", sql_column::error_message(&e))
                    })),
                    false,
                ));
                stop = Some(k);
            }
            Err(other) => return Err(other),
        }
    }

    let ok = checked.ok() && stop.is_none();
    let mut r#final = blanks();
    let mut sql = String::new();
    if ok {
        let expression = computed_column::expression(entity_type, formula, Some(env))?;
        let statement = (frame.r#final)(&expression);
        r#final = samples
            .iter()
            .map(|sample| {
                Ok(single_or_default(
                    connection,
                    &statement,
                    rusqlite::named_params! {"@key": sample.key},
                    |r| Ok(text_of(r.get_ref(0)?)),
                )?
                .flatten()
                .unwrap_or_default())
            })
            .collect::<Result<_>>()?;
        sql = flow_compiler::eject(entity_type, formula, Some(env))?;
    }
    Ok(object([
        ("samples", Value::Array(heads)),
        ("steps", Value::Array(steps)),
        ("final", strings_json(&r#final)),
        ("sql", Value::String(sql)),
    ]))
}

/// `Store.Step` — 단계 하나의 칸.
fn step(shape: Option<&str>, values: Vec<Value>, error: Option<String>, blocked: bool) -> Value {
    let mut step = Map::new();
    step.insert(
        "shape".into(),
        shape.map_or(Value::Null, |s| Value::String(s.into())),
    );
    step.insert("values".into(), Value::Array(values));
    if let Some(error) = error {
        step.insert("error".into(), Value::String(error));
    }
    if blocked {
        step.insert("blocked".into(), Value::Bool(true));
    }
    Value::Object(step)
}

/// `Store.Blank`.
fn blank() -> Value {
    object([
        ("kind", Value::String("value".into())),
        ("text", Value::String(String::new())),
        ("blank", Value::Bool(true)),
    ])
}

/// `Store.Value`.
fn value(text: &str) -> Value {
    if text.is_empty() {
        blank()
    } else {
        object([
            ("kind", Value::String("value".into())),
            ("text", Value::String(text.into())),
        ])
    }
}

/// `JsonElement.ToString()` — 글은 그 값, 수는 원문, null 은 빈 글, 참거짓은 `True`·`False`, 나머지는 원문 JSON.
fn element_text(element: &Value) -> String {
    match element {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::Number(n) => n.to_string(),
        other => json::to_compact(other, Encoder::Relaxed),
    }
}

/// `Store.Show` 의 `Row(JsonElement)` — 행 하나를 {필드: 글} 로.
fn row_of(element: &Value) -> Vec<(String, String)> {
    match element {
        Value::Object(o) => o
            .iter()
            .map(|(k, v)| (k.clone(), element_text(v)))
            .collect(),
        _ => Vec::new(),
    }
}

fn row_json(row: &[(String, String)]) -> Value {
    let mut map = Map::new();
    for (k, v) in row {
        map.insert(k.clone(), Value::String(v.clone()));
    }
    Value::Object(map)
}

/// `Store.RowLabel` — 행의 앞 두 빈 아닌 필드를 「 · 」 로 잇는다.
fn row_label(row: &[(String, String)], fields: &[String]) -> String {
    fields
        .iter()
        .map(|f| {
            row.iter()
                .find(|(k, _)| k == f)
                .map_or("", |(_, v)| v.as_str())
        })
        .filter(|v| !v.is_empty())
        .take(2)
        .collect::<Vec<_>>()
        .join(" · ")
}

/// `Store.Narrowing` — 줄 수를 줄이는 단계.
const NARROWING: [&str; 3] = ["filter", "distinct", "take"];

/// `Store.Show` — 표본 하나에서 이 단계의 값(과 목록이면 그 수).
fn show(
    connection: &Connection,
    read: &dyn Fn(&str) -> String,
    key: &str,
    part: &Part,
    previous: Option<&Part>,
    step: Option<&FlowStep>,
    previous_count: Option<i64>,
) -> Result<(Value, i64)> {
    let json_row = |fields: &[String]| {
        format!(
            "json_object({})",
            fields
                .iter()
                .map(|f| format!("{}, {}", flow_compiler::lit(f), flow_compiler::ident(f)))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let narrowing = step.is_some_and(|s| NARROWING.contains(&s.op.as_deref().unwrap_or("")));
    let parse_list = |list: Option<String>| -> Result<Vec<Value>> {
        Ok(match list {
            None => Vec::new(),
            Some(list) => match json::parse(&list).map_err(|_| Error::invalid("JSON"))? {
                Value::Array(items) => items,
                _ => Vec::new(),
            },
        })
    };
    match part.shape {
        "rows" => {
            let (count, list) = single(
                connection,
                &read(&format!(
                    "\n(SELECT count(*) FROM ({sql})),\n(SELECT json_group_array({row} ORDER BY __order) FROM (SELECT * FROM ({sql}) ORDER BY __order LIMIT {SHOW_ROWS}))\n",
                    sql = part.sql,
                    row = json_row(&part.fields)
                )),
                rusqlite::named_params! {"@key": key},
                |r| {
                    Ok((
                        r.get::<_, Option<i64>>(0)?.unwrap_or(0),
                        text_of(r.get_ref(1)?),
                    ))
                },
            )?;
            let rows: Vec<Vec<(String, String)>> = parse_list(list)?.iter().map(row_of).collect();
            let mut value = Map::new();
            value.insert("kind".into(), Value::String("list".into()));
            value.insert("count".into(), count.into());
            value.insert(
                "items".into(),
                Value::Array(
                    rows.iter()
                        .take(SHOW_ITEMS)
                        .map(|r| Value::String(row_label(r, &part.fields)))
                        .collect(),
                ),
            );
            value.insert(
                "rows".into(),
                Value::Array(rows.iter().map(|r| row_json(r)).collect()),
            );
            if narrowing
                && let Some(before) = previous_count
                && before > count
            {
                value.insert("dropped".into(), (before - count).into());
            }
            Ok((Value::Object(value), count))
        }
        "values" => {
            let (count, mut blanks, list) = single(
                connection,
                &read(&format!(
                    "\n(SELECT count(*) FROM ({sql})),\n(SELECT count(*) FROM ({sql}) WHERE __value = ''),\n(SELECT json_group_array(__value ORDER BY __order) FROM (SELECT * FROM ({sql}) ORDER BY __order LIMIT {SHOW_ITEMS}))\n",
                    sql = part.sql
                )),
                rusqlite::named_params! {"@key": key},
                |r| {
                    Ok((
                        r.get::<_, Option<i64>>(0)?.unwrap_or(0),
                        r.get::<_, Option<i64>>(1)?.unwrap_or(0),
                        text_of(r.get_ref(2)?),
                    ))
                },
            )?;
            let items: Vec<Value> = parse_list(list)?
                .iter()
                .map(|e| Value::String(element_text(e)))
                .collect();
            let mut value = Map::new();
            value.insert("kind".into(), Value::String("list".into()));
            value.insert("count".into(), count.into());
            value.insert("items".into(), Value::Array(items));
            if narrowing
                && let Some(before) = previous_count
                && before > count
            {
                value.insert("dropped".into(), (before - count).into());
            }
            // 서식은 비어 찍히는 칸이 아니라 빈 필드를 낀 행을 센다 — 「의자()」 도 빈 값이다(화면과 같다).
            if let Some(step) = step
                && step.op.as_deref() == Some("format")
                && let Some(previous) = previous
                && previous.shape == "rows"
            {
                let holes =
                    Flow::placeholders(step.text.as_deref().unwrap_or("")).unwrap_or_default();
                let mut distinct: Vec<&String> = Vec::new();
                for h in &holes {
                    if !distinct.contains(&h) {
                        distinct.push(h);
                    }
                }
                blanks = if holes.is_empty() {
                    0
                } else {
                    single(
                        connection,
                        &read(&format!(
                            "(SELECT count(*) FROM ({}) WHERE {})",
                            previous.sql,
                            distinct
                                .iter()
                                .map(|h| format!("{} = ''", flow_compiler::ident(h)))
                                .collect::<Vec<_>>()
                                .join(" OR ")
                        )),
                        rusqlite::named_params! {"@key": key},
                        |r| Ok(r.get::<_, Option<i64>>(0)?.unwrap_or(0)),
                    )?
                };
            }
            if blanks > 0 {
                value.insert("blank".into(), blanks.into());
            }
            Ok((Value::Object(value), count))
        }
        "row" => {
            let json = single_or_default(
                connection,
                &read(&format!(
                    "(SELECT {} FROM ({}))",
                    json_row(&part.fields),
                    part.sql
                )),
                rusqlite::named_params! {"@key": key},
                |r| Ok(text_of(r.get_ref(0)?)),
            )?
            .flatten();
            let Some(json) = json else {
                return Ok((blank(), 0));
            };
            let row = row_of(&json::parse(&json).map_err(|_| Error::invalid("JSON"))?);
            Ok((
                object([
                    ("kind", Value::String("value".into())),
                    ("text", Value::String(row_label(&row, &part.fields))),
                    ("row", row_json(&row)),
                ]),
                1,
            ))
        }
        "number" => {
            let text = scalar_text(
                connection,
                &read(&format!(
                    "(SELECT CASE WHEN v IS NULL THEN NULL ELSE printf('%.15g', v) END FROM (SELECT {} AS v))",
                    part.sql
                )),
                key,
            )?;
            Ok((value(&text), 0))
        }
        "date" => {
            let text = scalar_text(
                connection,
                &read(&format!("replace({}, '-', '/')", part.sql)),
                key,
            )?;
            Ok((value(&text), 0))
        }
        _ => Ok((value(&scalar_text(connection, &read(&part.sql), key)?), 0)),
    }
}

/// `QuerySingleOrDefault<string?>(sql, new { key }) ?? ""`.
fn scalar_text(connection: &Connection, sql: &str, key: &str) -> Result<String> {
    Ok(single_or_default(
        connection,
        sql,
        rusqlite::named_params! {"@key": key},
        |r| Ok(text_of(r.get_ref(0)?)),
    )?
    .flatten()
    .unwrap_or_default())
}

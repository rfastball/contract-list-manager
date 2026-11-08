//! 배포된 자료의 입증된 원천값과 열 이름을 같은 판올림 트랜잭션에서 복구한다(C# `Storage/SchemaRepairs.cs`).
//!
//! C# 은 줄마다 `try { … } catch (JsonException or InvalidOperationException or FormatException) { }` 로 그 줄을
//! 건너뛴다 — 저장된 JSON 이 기대와 다른 꼴이면 보정하지 않는다는 뜻이다. 여기서는 그 셋을 [`Skip`] 하나로 모으고,
//! SQL 오류와 겹친 키만 판올림 전체를 멈춘다(C# 에서도 `SqliteException` 과, 같은 키가 두 번 든 객체를 열 때의
//! `ArgumentException` 은 잡히지 않는다 — [`parse_node`]).

use std::collections::HashSet;

use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Map, Value};

use crate::erp::mapping;
use crate::error::{Error, Result};
use crate::json::{self, Encoder, JsonException, NodeError, index, node_string};
use crate::storage::sql_column;
use crate::storage::store::face_view;
use crate::storage::user_view_builder;
use crate::storage::views;
use crate::text::{cmp_ordinal, eq_ignore_case, ignore_case_key, utf16_len, utf16_prefix};

/// 줄 하나를 보정하다 만난 일. `Skip` 은 C# 이 잡아 넘기는 예외, `Fatal` 은 판올림을 멈추는 오류다.
enum Step {
    Skip,
    Fatal(Error),
}

impl From<JsonException> for Step {
    fn from(_: JsonException) -> Self {
        Step::Skip
    }
}
impl From<NodeError> for Step {
    fn from(_: NodeError) -> Self {
        Step::Skip
    }
}
impl From<mapping::AtError> for Step {
    fn from(_: mapping::AtError) -> Self {
        Step::Skip
    }
}
impl From<mapping::ConvertError> for Step {
    fn from(_: mapping::ConvertError) -> Self {
        Step::Skip
    }
}
impl From<rusqlite::Error> for Step {
    fn from(e: rusqlite::Error) -> Self {
        Step::Fatal(Error::Sqlite(e))
    }
}

/// `JsonNode.Parse(text)` 와 그 노드 읽기. 깨진 글은 C# 처럼 그 줄을 건너뛰고([`Step::Skip`]), 같은 키가 두 번 든 글은
/// 판올림을 멈춘다 — C# 의 `JsonNode` 는 그런 객체를 열 때 `ArgumentException` 을 던지고 위의 catch 는 그것을 잡지 않는다.
/// serde_json 은 겹친 키를 뒤 값으로 접어 읽으므로, 두면 Rust 만 뒤 값으로 보수하고 v32 에서는 수집 기록을 접어 다시 쓴다.
fn parse_node(text: &str) -> std::result::Result<Value, Step> {
    let value = json::parse(text)?;
    match json::first_duplicate_key(text) {
        Some(key) => Err(Step::Fatal(Error::Argument(format!(
            "An item with the same key has already been added. Key: {key} (Parameter 'propertyName')"
        )))),
        None => Ok(value),
    }
}

/// 잡힌 것은 넘기고 나머지는 올린다.
fn settle<T>(result: std::result::Result<T, Step>) -> Result<Option<T>> {
    match result {
        Ok(v) => Ok(Some(v)),
        Err(Step::Skip) => Ok(None),
        Err(Step::Fatal(e)) => Err(e),
    }
}

/// `rows?.AsArray().FirstOrDefault(r => (string?)r?["table"] == "notice")` — 수집 기록의 공고 머리 줄.
fn notice_head(document: Option<&Value>) -> std::result::Result<Option<&Value>, Step> {
    let Some(rows) = json::as_array(index(document, "rows")?)? else {
        return Ok(None);
    };
    for row in rows {
        let row = (!row.is_null()).then_some(row);
        if node_string(index(row, "table")?)? == Some("notice") {
            return Ok(row);
        }
    }
    Ok(None)
}

/// `.AsArray().SingleOrDefault(pred)` — 둘 이상이면 C# 은 `InvalidOperationException` 이다.
fn single<'a>(
    items: Option<&'a Vec<Value>>,
    mut pred: impl FnMut(Option<&'a Value>) -> std::result::Result<bool, Step>,
) -> std::result::Result<Option<&'a Value>, Step> {
    let mut found = None;
    for item in items.into_iter().flatten() {
        let item = (!item.is_null()).then_some(item);
        if pred(item)? {
            if found.is_some() {
                return Err(Step::Skip);
            }
            found = Some(item);
        }
    }
    Ok(found.flatten())
}

/// 영수증들 — 그 공고를 그 프로필로 수집한 기록, 최근 것부터.
fn receipts(c: &Connection, profile: &str, base: &str, seq: &str) -> rusqlite::Result<Vec<String>> {
    c.prepare(
        "SELECT snapshot_json FROM erp_capture WHERE profile=?1 AND entity_base=?2 AND entity_seq=?3\n\
         ORDER BY captured_at DESC, capture_id DESC",
    )?
    .query_map(params![profile, base, seq], |r| r.get(0))?
    .collect()
}

fn mapping_json(c: &Connection, revision: Option<&str>) -> rusqlite::Result<Option<String>> {
    c.query_row(
        "SELECT json FROM erp_mapping WHERE revision=?1",
        params![revision],
        |r| r.get(0),
    )
    .optional()
}

/// `SchemaRepairs.Upgrade32` — 출처가 입증된 공고종류의 옛 상태 합성(`실공고(일반공고)` 꼴)을 걷는다.
pub(crate) fn upgrade32(c: &Connection) -> Result<()> {
    let rows: Vec<(String, String, String, String)> = c
        .prepare(
            "SELECT n.notice_base,n.seq,n.notice_kind,s.document_json FROM notice n JOIN erp_source s\n  \
             ON s.entity_type='notice' AND s.entity_base=n.notice_base AND s.entity_seq=n.seq\n\
             WHERE n.notice_kind IS NOT NULL",
        )?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect::<rusqlite::Result<_>>()?;
    if rows.is_empty() {
        return Ok(());
    }
    // C# 은 매번 기본 매핑과 그 판을 짓는다. 줄이 없으면 쓸 데가 없어 여기서는 줄이 있을 때만 짓는다.
    let defaults = mapping::defaults_value();
    let default_revision = mapping::hash(&defaults);
    for (base, seq, value, document) in rows {
        settle(repair_notice_kind(
            c,
            &defaults,
            &default_revision,
            &base,
            &seq,
            &value,
            &document,
        ))?;
    }
    Ok(())
}

fn repair_notice_kind(
    c: &Connection,
    defaults: &Value,
    default_revision: &str,
    base: &str,
    seq: &str,
    value: &str,
    document: &str,
) -> std::result::Result<(), Step> {
    let mut document = parse_node(document)?;
    let head = notice_head(Some(&document))?;
    if node_string(index(index(head, "values")?, "notice_kind")?)? != Some(value) {
        return Ok(());
    }
    let profile = match node_string(index(index(head, "origins")?, "notice_kind")?)? {
        Some(p @ ("g2b-notice-a-v1" | "g2b-notice-a-service-v1")) => p.to_string(),
        _ => return Ok(()),
    };
    for snapshot in receipts(c, &profile, base, seq)? {
        let receipt = parse_node(&snapshot)?;
        let receipt = Some(&receipt).filter(|r| !r.is_null());
        let revision = node_string(index(receipt, "mappingRevision")?)?;
        let historical;
        let mapping_value: &Value = if revision == Some(default_revision) {
            defaults
        } else {
            match mapping_json(c, revision)? {
                Some(text) => {
                    historical = parse_node(&text)?;
                    &historical
                }
                None => break, // 당시 매핑을 모르면 원천 칸을 추정하지 않는다.
            }
        };
        let mapping_value = Some(mapping_value).filter(|v| !v.is_null());
        let profiles = json::as_array(index(mapping_value, "profiles")?)?;
        let found = single(profiles, |p| {
            Ok(node_string(index(p, "id")?)? == Some(profile.as_str()))
        })?;
        let fields = json::as_array(index(found, "fields")?)?;
        let field = single(fields, |f| {
            Ok(node_string(index(f, "target")?)? == Some("notice_kind"))
        })?;
        let field = match field {
            Some(node) => mapping::field_map(node)?,
            None => None,
        };
        let Some(field) = field else { break };
        let source = match field.source.as_deref() {
            Some(s)
                if !s.is_empty()
                    && field.secondary_source.is_none()
                    && !s.starts_with("tables.") =>
            {
                s
            }
            _ => break,
        };
        let Some(point) = index(index(receipt, "data")?, "pointInfo")? else {
            break;
        };
        let Some(observed) = mapping::at(point, source)? else {
            continue;
        };
        let kind = mapping::convert(mapping::text(Some(observed)).as_deref(), &field)?;
        let Some(kind) = kind.filter(|k| !k.is_empty()) else {
            break;
        };
        if !["일반공고", "변경공고", "취소공고", "재공고"]
            .iter()
            .any(|status| value == format!("{kind}({status})"))
        {
            break;
        }
        // 정정 덮개와 영수증은 그대로 두고, 입증된 원천 종류만 정규화한다.
        set_head_value(&mut document, "notice_kind", &kind);
        c.execute(
            "UPDATE notice SET notice_kind=?1 WHERE notice_base=?2 AND seq=?3",
            params![kind, base, seq],
        )?;
        c.execute(
            "UPDATE erp_source SET document_json=?1 WHERE entity_type='notice' AND entity_base=?2 AND entity_seq=?3",
            params![json::to_compact(&document, Encoder::Default), base, seq],
        )?;
        break;
    }
    Ok(())
}

/// `head!["values"]!["notice_kind"] = kind` — 위에서 그 자리가 있음을 이미 확인했다.
fn set_head_value(document: &mut Value, key: &str, value: &str) {
    let Some(Value::Array(rows)) = document.get_mut("rows") else {
        return;
    };
    let head = rows
        .iter_mut()
        .find(|r| r.get("table").and_then(Value::as_str) == Some("notice"));
    if let Some(Value::Object(values)) = head.and_then(|h| h.get_mut("values")) {
        values.insert(key.into(), Value::String(value.into()));
    }
}

/// `SchemaRepairs.Upgrade30` — 재무관 출처 확인과, v28·v29 가 더한 열과 겹친 사용자 열 옮기기.
pub(crate) fn upgrade30(c: &Connection, previous_version: i32) -> Result<()> {
    repair_finance_officer(c)?;
    repair_columns(c, previous_version)
}

fn repair_finance_officer(c: &Connection) -> Result<()> {
    let rows: Vec<(String, String, String, Option<String>)> = c
        .prepare(
            "SELECT n.notice_base,n.seq,n.finance_officer,s.document_json\n\
             FROM notice n LEFT JOIN erp_source s\n  \
             ON s.entity_type='notice' AND s.entity_base=n.notice_base AND s.entity_seq=n.seq\n\
             WHERE n.finance_officer IS NOT NULL",
        )?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect::<rusqlite::Result<_>>()?;
    for (base, seq, value, document) in rows {
        let keep = settle(keeps_finance_officer(
            c,
            &base,
            &seq,
            &value,
            document.as_deref(),
        ))?
        .unwrap_or(false);
        if !keep {
            c.execute(
                "UPDATE notice SET finance_officer=NULL WHERE notice_base=?1 AND seq=?2",
                params![base, seq],
            )?;
        }
    }
    Ok(())
}

fn keeps_finance_officer(
    c: &Connection,
    base: &str,
    seq: &str,
    value: &str,
    document: Option<&str>,
) -> std::result::Result<bool, Step> {
    let document = parse_node(document.unwrap_or("null"))?;
    let head = notice_head(Some(&document))?;
    let values = index(head, "values")?;
    // v29 이후 새 대상으로 실제 수집한 값은 옛 creditor 판올림과 관계없다.
    if node_string_value(index(values, "finance_officer")?) == Some(value) {
        return Ok(true);
    }
    if node_string(index(values, "creditor")?)? == Some(value)
        && let Some(profile @ ("g2b-notice-a-v1" | "g2b-notice-a-service-v1")) =
            node_string(index(index(head, "origins")?, "creditor")?)?
    {
        return proves_legacy_officer(c, base, seq, profile, value);
    }
    Ok(false)
}

/// `node is JsonValue v && v.TryGetValue<string>(out var s)` — 글이 아니면 던지지 않고 `None`.
fn node_string_value(node: Option<&Value>) -> Option<&str> {
    node.and_then(Value::as_str)
}

fn proves_legacy_officer(
    c: &Connection,
    base: &str,
    seq: &str,
    profile: &str,
    value: &str,
) -> std::result::Result<bool, Step> {
    // erp_source.mapping_revision 은 다른 칸을 수집해도 바뀐다. 실제 수집 영수증의 판과 원값을 함께 확인한다.
    for snapshot in receipts(c, profile, base, seq)? {
        let receipt = parse_node(&snapshot)?;
        let receipt = Some(&receipt).filter(|r| !r.is_null());
        let revision = node_string(index(receipt, "mappingRevision")?)?;
        let Some(text) = mapping_json(c, revision)? else {
            return Ok(false); // 당시 기본 매핑 사본이 없으면 출처를 추정하지 않는다.
        };
        let historical = parse_node(&text)?;
        let historical = Some(&historical).filter(|h| !h.is_null());
        let profiles = json::as_array(index(historical, "profiles")?)?;
        let found = single(profiles, |p| {
            Ok(node_string(index(p, "id")?)? == Some(profile))
        })?;
        let fields = json::as_array(index(found, "fields")?)?;
        let field = single(fields, |f| {
            Ok(node_string(index(f, "target")?)? == Some("creditor"))
        })?;
        let Some(field) = field else { continue };
        let source = node_string(index(Some(field), "source")?)?;
        let raw = index(index(receipt, "data")?, "pointInfo")?;
        let (Some(raw), Some(source)) = (raw, source) else {
            return Ok(false);
        };
        let Some(source_value) = mapping::at(raw, source)? else {
            continue; // 이 수집에서는 그 칸을 읽지 않았다.
        };
        // C# 의 && 차례 그대로 — 앞이 거짓이면 뒤의 꺼내기(던질 수 있다)를 하지 않는다.
        return Ok(source == "fncrNm"
            && node_string(index(Some(field), "transform")?)?.unwrap_or("text") == "text"
            && index(Some(field), "secondarySource")?.is_none()
            && mapping::text(Some(source_value)).as_deref() == Some(value));
    }
    Ok(false)
}

fn repair_columns(c: &Connection, previous_version: i32) -> Result<()> {
    for entity in ["notice", "contract", "request"] {
        let table = format!("{entity}_user_field");
        let names: Vec<String> = c
            .prepare(&format!(
                "SELECT field_name FROM user_column WHERE entity_type=?1\nUNION SELECT field_name FROM {table}"
            ))?
            .query_map([entity], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let mut occupied: HashSet<String> = names.iter().map(|n| ignore_case_key(n)).collect();
        // v28·v29 에 더한 머리 열만 충돌할 수 있다. 품목 열에는 사용자 열이 붙지 않는다.
        let builtins = views::hidden_by_default(face_view(entity)).unwrap_or(&[]);
        let base_columns = views::base_columns(entity)?;
        let mut colliding: Vec<&String> = names
            .iter()
            .filter(|n| builtins.iter().any(|b| eq_ignore_case(b, n)))
            .collect();
        colliding.sort_by(|a, b| cmp_ordinal(a, b));
        for from in colliding {
            let candidate = |number: u32| {
                let suffix = if number == 1 {
                    "_사용자".to_string()
                } else {
                    format!("_사용자{number}")
                };
                let keep = 20usize
                    .saturating_sub(utf16_len(&suffix))
                    .min(utf16_len(from));
                format!("{}{suffix}", utf16_prefix(from, keep))
            };
            let mut number = 1;
            let mut to = candidate(number);
            while occupied.contains(&ignore_case_key(&to)) || base_columns.contains(&to.as_str()) {
                number += 1;
                to = candidate(number);
            }
            occupied.insert(ignore_case_key(&to));
            c.execute(
                "UPDATE user_column SET field_name=?1 WHERE entity_type=?2 AND field_name=?3",
                params![to, entity, from],
            )?;
            c.execute(
                &format!("UPDATE {table} SET field_name=?1 WHERE field_name=?2"),
                params![to, from],
            )?;
            let message = format!(
                "표의 새 열과 겹친 사용자 열 「{from}」을 「{to}」로 옮겼습니다. 이 식이 읽을 열을 확인하고 식을 고쳐 저장하세요."
            );
            let introduced = if entity == "notice" && from == "재무관" {
                29
            } else {
                28
            };
            repair_formula_readers(c, entity, from, &message, previous_version >= introduced)?;
            if previous_version < introduced {
                user_view_builder::rename_field(c, entity, from, &to)?;
            }
        }
    }
    Ok(())
}

/// `RepairFormulaReaders` 의 `Mark` — 식이 옮긴 열을 읽을 수 있으면 `migrationError` 를 단다. 바꿨는가.
fn mark(
    formula: Option<&mut Value>,
    entity: &str,
    from: &str,
    message: &str,
    ambiguous_flow: bool,
) -> Result<bool> {
    let Some(Value::Object(root)) = formula else {
        return Ok(false);
    };
    let sql = match (root.get("source"), root.get("sql")) {
        (Some(Value::String(t)), Some(Value::String(sql))) if t == "sql" => Some(sql.clone()),
        _ => None,
    };
    let needs_review = if let Some(text) = sql {
        // shortcut: 다른 뷰의 동명 참조도 수동 확인한다. 이미 깨진 SQL 원문은 그대로 둔다.
        sql_column::check_shape(Some(&text)).is_ok_and(|names| names.contains(from))
    } else if ambiguous_flow {
        let mut node = Value::Object(root.clone());
        user_view_builder::touch(Some(&mut node), entity, from, None)
    } else {
        false
    };
    if !needs_review {
        return Ok(false);
    }
    if let Some(Value::Object(previous)) = root.get_mut("migrationError") {
        // 글이 아닌 진단(손으로 고친 파일)은 없는 것과 같다 — 던지면 판올림 전체가 멈춰 파일을 열 수 없다.
        let existing = match previous.get("message") {
            Some(Value::String(m)) => m.clone(),
            _ => String::new(),
        };
        if existing.contains(message) {
            return Ok(false);
        }
        previous.insert(
            "message".into(),
            Value::String(format!("{existing}\n{message}")),
        );
        return Ok(true);
    }
    let original = Value::Object(root.clone());
    let mut error = Map::new();
    error.insert("message".into(), Value::String(message.into()));
    error.insert("original".into(), original);
    root.insert("migrationError".into(), Value::Object(error));
    Ok(true)
}

fn repair_formula_readers(
    c: &Connection,
    entity: &str,
    from: &str,
    message: &str,
    ambiguous_flow: bool,
) -> Result<()> {
    let columns: Vec<(String, String, String)> = c
        .prepare(
            "SELECT entity_type,field_name,options FROM user_column WHERE options IS NOT NULL AND options LIKE '{%'",
        )?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;
    for (column_entity, name, options) in columns {
        let Ok(mut formula) = json::parse(&options) else {
            continue;
        };
        // 같은 키가 두 번 든 식(손으로 고친 파일)은 읽지 못한 글이라 그대로 둔다 — 뒤의 값을 골라 진단을 달면 겹친 키가
        // 조용히 지워지고, C# 은 그 식을 건너뛴다. 그런 식은 뷰를 지을 때 깨진 열로 선다.
        if json::has_duplicate_keys(&options) {
            continue;
        }
        if mark(Some(&mut formula), entity, from, message, ambiguous_flow)? {
            c.execute(
                "UPDATE user_column SET options=?1 WHERE entity_type=?2 AND field_name=?3",
                params![
                    json::to_compact(&formula, Encoder::Relaxed),
                    column_entity,
                    name
                ],
            )?;
        }
    }
    for view in user_view_builder::read(c)? {
        let Ok(mut definition) = json::parse(&view.definition) else {
            continue;
        };
        // 같은 키가 두 번 든 정의도 읽지 못한 글이다(`user_view_builder::parse_columns` 와 같다).
        if json::has_duplicate_keys(&view.definition) {
            continue;
        }
        // 객체가 아닌 정의(배열·글·수)도 읽지 못한 글이다 — 예전에는 C# 처럼 오류를 내 판올림이 멈췄다.
        let Some(Value::Array(columns)) = user_view_builder::columns_of(&mut definition) else {
            continue;
        };
        let mut changed = false;
        for column in columns.iter_mut().filter_map(Value::as_object_mut) {
            changed |= mark(
                column.get_mut("formula"),
                entity,
                from,
                message,
                ambiguous_flow,
            )?;
        }
        if changed {
            c.execute(
                "UPDATE user_view SET definition=?1 WHERE id=?2",
                params![json::to_compact(&definition, Encoder::Relaxed), view.id],
            )?;
        }
    }
    Ok(())
}

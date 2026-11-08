//! C# `BatchOperations.cs` — 표에서 여러 칸을 한 번에 고치기와 여러 건을 한 번에 지우기.
//!
//! 지우기는 **센 것만 지운다.** 계획(`PlanBatchDeletion`)이 지울 대상과 그것이 닿는 행·연결의 사진을 지문으로 남기고,
//! 지우기(`BatchDelete`)는 같은 지문일 때만 실제로 지운다 — 센 뒤에 자료가 바뀌었으면 아무것도 하지 않는다.

use std::collections::HashSet;

use rusqlite::Connection;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::{
    DeletionPlan, EntityRef, Store, check_correctable_on, clear_override_on, delete_on, face_key,
    face_value_on, object, plan_deletion_on, read_user_columns, refuse_read, resolve_on, rows_json,
    scalar_i64, set_override_on, set_user_field_on, shape, strings, view_columns,
};
use crate::erp::explicit_links;
use crate::error::{Error, Result};
use crate::json::{self, Encoder};
use crate::storage::database;
use crate::storage::views;
use crate::text::{cmp_ordinal, utf16_len};

/// C# `CellChange(RowKey, Field, Value, Original, Operation)` — 칸 하나의 변경. `operation` 은 `set`·`revert`.
/// 자리마다 `Option` 인 것은 화면이 빠뜨리거나 `null` 로 보낼 수 있어서다(검사가 거절한다).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CellChange {
    pub row_key: Option<String>,
    pub field: Option<String>,
    pub value: Option<String>,
    pub original: Option<String>,
    pub operation: Option<String>,
}

/// C# `BatchDeletionPlan` — 여러 건을 지우면 사라지는 것. 수는 실제로 지워 보고 되돌려 센 것이다.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BatchDeletionPlan {
    pub fingerprint: String,
    pub targets: Vec<DeletionPlan>,
    pub document_count: i32,
    pub series_count: i32,
    pub child_rows: i32,
    pub user_fields: i32,
    pub overrides: i32,
    pub links: i32,
}

impl BatchDeletionPlan {
    pub fn to_json(&self) -> Value {
        object([
            ("fingerprint", Value::String(self.fingerprint.clone())),
            (
                "targets",
                Value::Array(self.targets.iter().map(DeletionPlan::to_json).collect()),
            ),
            ("documentCount", self.document_count.into()),
            ("seriesCount", self.series_count.into()),
            ("childRows", self.child_rows.into()),
            ("userFields", self.user_fields.into()),
            ("overrides", self.overrides.into()),
            ("links", self.links.into()),
        ])
    }
}

/// .NET 의 숫자 사이 공백(`IsWhite`) — 0x09..=0x0D 와 0x20.
fn is_white(c: char) -> bool {
    c == ' ' || ('\u{9}'..='\u{d}').contains(&c)
}

/// `decimal.TryParse(text, NumberStyles.Number, CultureInfo.InvariantCulture, out _)` — 앞뒤 공백, 앞 또는 뒤 부호
/// 하나, 첫 숫자 뒤의 자릿점(`,`), 소수점 하나. 지수·통화·괄호는 받지 않는다. `decimal` 을 넘치면 거절한다.
pub(crate) fn dotnet_decimal_number(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() && is_white(chars[i]) {
        i += 1;
    }
    let mut signed = false;
    if i < chars.len() && matches!(chars[i], '+' | '-') {
        signed = true;
        i += 1;
    }
    let mut integer = String::new();
    let mut fraction = String::new();
    let mut digits = false;
    let mut decimal = false;
    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_digit() {
            digits = true;
            if decimal {
                fraction.push(c);
            } else {
                integer.push(c);
            }
        } else if c == '.' && !decimal {
            decimal = true;
        } else if c == ',' && digits && !decimal {
        } else {
            break;
        }
        i += 1;
    }
    if !digits {
        return false;
    }
    while i < chars.len() {
        let c = chars[i];
        if is_white(c) {
        } else if !signed && matches!(c, '+' | '-') {
            signed = true;
        } else {
            return false;
        }
        i += 1;
    }
    // 넘침 — 정수부가 decimal.MaxValue 를 넘으면 거절한다.
    const MAX: &str = "79228162514264337593543950335";
    let integer = integer.trim_start_matches('0');
    integer.len() < MAX.len() || (integer.len() == MAX.len() && integer <= MAX)
}

/// `DateOnly.TryParseExact(text, ["yyyy-MM-dd", "yyyy/MM/dd"], InvariantCulture, None, out _)` — 네 자리·두 자리·두
/// 자리, 있는 날짜만.
pub(crate) fn dotnet_date_exact(text: &str) -> bool {
    let b = text.as_bytes();
    if b.len() != 10 || b[4] != b[7] || !matches!(b[4], b'-' | b'/') {
        return false;
    }
    let digits = |r: std::ops::Range<usize>| -> Option<u32> {
        b[r].iter().try_fold(0u32, |n, d| {
            d.is_ascii_digit().then(|| n * 10 + u32::from(d - b'0'))
        })
    };
    let (Some(year), Some(month), Some(day)) = (digits(0..4), digits(5..7), digits(8..10)) else {
        return false;
    };
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    year >= 1 && (1..=days).contains(&day)
}

impl Store<'_> {
    /// `Store.BatchEdit` — 칸 여러 개를 한 트랜잭션에서 고친다. 하나라도 거절되면 아무것도 쓰지 않는다. 사람 열은
    /// 계열 값, 계약면 열은 그 차수의 덮개다. 고친 칸 수를 낸다.
    pub fn batch_edit(&self, view: &str, changes: &[CellChange]) -> Result<i32> {
        if !matches!(view, "v_통합" | "v_접수" | "v_공고" | "v_계약") {
            return Err(Error::invalid("이 표에서는 일괄 편집할 수 없습니다."));
        }
        if !(1..=10000).contains(&changes.len()) {
            return Err(Error::invalid("한 번에 1~10,000셀을 변경할 수 있습니다."));
        }
        refuse_read(&self.database, "열람 중에는 자료를 고칠 수 없습니다.")?;

        let connection = self.database.open()?;
        let transaction = database::begin(&connection)?;
        let entity_type = views::entity_type_of(view);
        let columns: HashSet<String> = view_columns(&transaction, view)?.into_iter().collect();
        let definitions = read_user_columns(
            &transaction,
            "SELECT entity_type, field_name, kind, options, sort_order FROM user_column WHERE entity_type = ?1;",
            [entity_type],
        )?;
        let mut pending: Vec<(EntityRef, &CellChange, bool)> = Vec::new();
        let mut seen: HashSet<(String, String, String)> = HashSet::new();
        for change in changes {
            let (Some(row_key), Some(field), Some(value), Some(original), Some(operation)) = (
                change.row_key.as_deref(),
                change.field.as_deref(),
                change.value.as_deref(),
                change.original.as_deref(),
                change.operation.as_deref(),
            ) else {
                return Err(Error::invalid("변경할 셀 정보가 올바르지 않습니다."));
            };
            if row_key.is_empty()
                || field.is_empty()
                || utf16_len(value) > 32767
                || !matches!(operation, "set" | "revert")
            {
                return Err(Error::invalid("변경할 셀 정보가 올바르지 않습니다."));
            }
            let entity = resolve_on(&transaction, row_key)?.ok_or_else(|| {
                Error::InvalidOperation(format!("자료가 없어졌습니다: {row_key}"))
            })?;
            if entity.entity_type != entity_type
                || !columns.contains(field)
                || views::KEY_COLUMNS.contains(&field)
                || (view == "v_통합"
                    && (views::UNIFIED_NOTICE_COLUMNS.contains(&field)
                        || views::UNIFIED_REQUEST_COLUMNS.contains(&field)))
            {
                return Err(Error::InvalidOperation(format!(
                    "고칠 수 없는 셀입니다: {row_key} · {field}"
                )));
            }
            // 반드시 현재 표에 있는 정확한 차수여야 한다. Resolve 의 별칭·옛 차수로 우회하지 않는다.
            let display = entity.display();
            if scalar_i64(
                &transaction,
                &format!(
                    "SELECT COUNT(*) FROM \"{view}\" WHERE \"{}\" = ?1;",
                    face_key(entity_type)
                ),
                [&display],
            )? == 0
                || display != row_key
            {
                return Err(Error::invalid(
                    "자료의 차수가 바뀌었습니다. 표를 새로 읽고 다시 해 주세요.",
                ));
            }
            let definition = definitions.iter().find(|d| d.field_name == field);
            let user = definition.is_some();
            if let Some(definition) = definition
                && (definition.kind == "computed" || operation == "revert")
            {
                return Err(Error::InvalidOperation(format!(
                    "원래 값 복원 또는 직접 입력을 할 수 없는 열입니다: {field}"
                )));
            }
            if !user {
                check_correctable_on(&transaction, &entity, field)?;
            }
            let seq = if user {
                String::new()
            } else {
                entity.seq.clone()
            };
            if !seen.insert((entity.base.clone(), seq, field.to_string())) {
                return Err(Error::InvalidOperation(format!(
                    "같은 셀이 두 번 포함되어 있습니다: {field}"
                )));
            }
            if face_value_on(&transaction, &entity, field)? != original {
                return Err(Error::InvalidOperation(format!(
                    "다른 작업에서 값이 바뀌었습니다: {row_key} · {field}. 표를 새로 읽고 다시 해 주세요."
                )));
            }
            if let Some(definition) = definition
                && !value.is_empty()
            {
                let valid = match definition.kind.as_str() {
                    "choice" => definition.choices().iter().any(|c| c == value),
                    "number" => dotnet_decimal_number(value),
                    "date" => dotnet_date_exact(value),
                    _ => true,
                };
                if !valid {
                    return Err(Error::InvalidOperation(format!(
                        "열 형식에 맞지 않는 값입니다: {field} · {value}"
                    )));
                }
            }
            pending.push((entity, change, user));
        }
        for (entity, change, user) in &pending {
            let field = change.field.as_deref().unwrap_or("");
            let value = change.value.as_deref().unwrap_or("");
            if *user {
                set_user_field_on(&transaction, entity, field, Some(value), self.env)?;
            } else if change.operation.as_deref() == Some("revert") {
                clear_override_on(&transaction, entity, field)?;
            } else {
                set_override_on(
                    &transaction,
                    entity,
                    field,
                    value,
                    change.original.as_deref(),
                    self.env,
                )?;
            }
        }
        if pending.iter().any(|(entity, change, user)| {
            !user && entity.entity_type == "notice" && change.field.as_deref() == Some("공고종류")
        }) {
            explicit_links::resolve(&transaction)?;
        }
        transaction.commit()?;
        Ok(pending.len() as i32)
    }

    /// `Store.PlanBatchDeletion` — 여러 건을 지우면 사라지는 것을 실제로 지워 보고 되돌려 센다.
    pub fn plan_batch_deletion(
        &self,
        keys: &[String],
        whole_series: bool,
    ) -> Result<BatchDeletionPlan> {
        refuse_read(&self.database, "열람 중에는 자료를 삭제할 수 없습니다.")?;
        let connection = self.database.open()?;
        let transaction = database::begin(&connection)?;
        let targets = batch_targets(&transaction, keys, whole_series)?;
        plan_batch_deletion_on(&transaction, &targets, whole_series)
    }

    /// `Store.BatchDelete` — 계획의 지문이 지금 센 것과 같을 때만 지운다. 지운 계획을 낸다.
    pub fn batch_delete(
        &self,
        keys: &[String],
        whole_series: bool,
        fingerprint: &str,
    ) -> Result<BatchDeletionPlan> {
        refuse_read(&self.database, "열람 중에는 자료를 삭제할 수 없습니다.")?;
        let connection = self.database.open()?;
        let transaction = database::begin(&connection)?;
        let targets = batch_targets(&transaction, keys, whole_series)?;
        let plan = plan_batch_deletion_on(&transaction, &targets, whole_series)?;
        if fingerprint.is_empty() || plan.fingerprint != fingerprint {
            return Err(Error::invalid(
                "삭제 대상이나 연결이 바뀌었습니다. 삭제 내용을 다시 확인해 주세요.",
            ));
        }
        delete_batch_rows(&transaction, &targets, whole_series)?;
        transaction.commit()?;
        Ok(plan)
    }
}

/// `Store.BatchTargets` — 열쇠를 레코드로 풀고 (종류, 본번호, 차수) 서수 차례로 늘어놓는다. 계열째 지우면 계열마다
/// 하나만 남긴다.
fn batch_targets(
    connection: &Connection,
    keys: &[String],
    whole_series: bool,
) -> Result<Vec<EntityRef>> {
    if !(1..=2000).contains(&keys.len()) || keys.iter().any(|k| k.trim().is_empty()) {
        return Err(Error::invalid("한 번에 1~2,000건을 선택해 주세요."));
    }
    let mut targets = Vec::with_capacity(keys.len());
    for key in keys {
        targets.push(
            resolve_on(connection, key)?
                .ok_or_else(|| Error::InvalidOperation(format!("자료가 없어졌습니다: {key}")))?,
        );
    }
    targets.sort_by(|a, b| {
        cmp_ordinal(&a.entity_type, &b.entity_type)
            .then_with(|| cmp_ordinal(&a.base, &b.base))
            .then_with(|| cmp_ordinal(&a.seq, &b.seq))
    });
    let mut seen = HashSet::new();
    targets.retain(|e| {
        seen.insert((
            e.entity_type.clone(),
            e.base.clone(),
            if whole_series {
                String::new()
            } else {
                e.seq.clone()
            },
        ))
    });
    Ok(targets)
}

/// `Store.PlanBatchDeletion(connection, transaction, entities, wholeSeries)`.
fn plan_batch_deletion_on(
    connection: &Connection,
    entities: &[EntityRef],
    whole_series: bool,
) -> Result<BatchDeletionPlan> {
    let mut plans = entities
        .iter()
        .map(|e| plan_deletion_on(connection, e, whole_series))
        .collect::<Result<Vec<_>>>()?;
    let mut snapshot: Vec<String> = vec![if whole_series { "True" } else { "False" }.into()];
    let mut remember = |sql: &str, rows: Vec<Value>| {
        snapshot.push(sql.to_string());
        let mut rows: Vec<String> = rows
            .iter()
            .map(|r| json::to_compact(r, Encoder::Default))
            .collect();
        rows.sort_by(|a, b| cmp_ordinal(a, b));
        snapshot.extend(rows);
    };
    let sql = "SELECT * FROM pclm_file;";
    remember(sql, rows_json(connection, sql, [])?);
    // 접수·계약뿐 아니라 공고 건을 다시 나누는 관계도 삭제 승인에 포함한다.
    for table in [
        "project_link",
        "request_link",
        "notice_series",
        "notice_relation",
    ] {
        let sql = format!("SELECT * FROM {table};");
        remember(&sql, rows_json(connection, &sql, [])?);
    }
    // 계열마다(처음 나온 차례) 묶는다 — C# `GroupBy`.
    let mut groups: Vec<((String, String), Vec<&EntityRef>)> = Vec::new();
    for e in entities {
        let key = (e.entity_type.clone(), e.base.clone());
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, members)) => members.push(e),
            None => groups.push((key, vec![e])),
        }
    }
    for (_, members) in &groups {
        let entity = members[0];
        let group = Value::Array(members.iter().map(|e| e.to_pascal_json()).collect());
        remember_raw(&mut remember, json::to_compact(&group, Encoder::Default));
        let shape = shape(&entity.entity_type)?;
        let tables = [shape.head, shape.user_fields]
            .into_iter()
            .chain(shape.children.iter().copied());
        for table in tables {
            let sql = format!("SELECT * FROM {table} WHERE {} = @Base;", shape.base_column);
            remember(
                &sql,
                rows_json(
                    connection,
                    &sql,
                    rusqlite::named_params! {"@Base": entity.base},
                )?,
            );
        }
        let sql = "SELECT * FROM field_override WHERE entity_type = @EntityType AND base = @Base;";
        remember(
            sql,
            rows_json(
                connection,
                sql,
                rusqlite::named_params! {"@EntityType": entity.entity_type, "@Base": entity.base},
            )?,
        );
    }
    let snapshot_json = json::to_compact(
        &Value::Array(snapshot.into_iter().map(Value::String).collect()),
        Encoder::Default,
    );
    let fingerprint = Sha256::digest(snapshot_json.as_bytes())
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect();
    let before = deletion_counts(connection)?;
    let links = link_identities(connection)?;
    // 기존 삭제 경로를 그대로 시험하고 되감아, 여러 차수·계열의 겹치는 영향까지 실제로 센다.
    connection.execute_batch("SAVEPOINT deletion_preview;")?;
    let tried = (|| -> Result<([i64; 5], usize)> {
        delete_batch_rows(connection, entities, whole_series)?;
        let after = deletion_counts(connection)?;
        let now = link_identities(connection)?;
        Ok((after, links.difference(&now).count()))
    })();
    connection.execute_batch("ROLLBACK TO deletion_preview; RELEASE deletion_preview;")?;
    let (after, removed_links) = tried?;
    for (i, entity) in entities.iter().enumerate() {
        let selected = entities
            .iter()
            .filter(|e| e.entity_type == entity.entity_type && e.base == entity.base)
            .count();
        if whole_series || selected == plans[i].revisions.len() {
            plans[i].series_goes = true;
        }
    }
    Ok(BatchDeletionPlan {
        fingerprint,
        targets: plans,
        document_count: (before[0] - after[0]) as i32,
        series_count: (before[1] - after[1]) as i32,
        child_rows: (before[2] - after[2]) as i32,
        user_fields: (before[3] - after[3]) as i32,
        overrides: (before[4] - after[4]) as i32,
        links: removed_links as i32,
    })
}

/// `snapshot.Add(text)` — 행 없이 글 하나만 남긴다.
fn remember_raw(remember: &mut impl FnMut(&str, Vec<Value>), text: String) {
    remember(&text, Vec::new());
}

/// `Store.DeletionCounts` — [문서, 계열, 딸린 행, 사람 값, 덮개] 의 지금 수.
fn deletion_counts(connection: &Connection) -> Result<[i64; 5]> {
    let shapes = ["contract", "notice", "request"]
        .iter()
        .map(|e| shape(e))
        .collect::<Result<Vec<_>>>()?;
    let count = |tables: &mut dyn Iterator<Item = &str>| -> Result<i64> {
        let mut sum = 0;
        for t in tables {
            sum += scalar_i64(connection, &format!("SELECT COUNT(*) FROM {t};"), [])?;
        }
        Ok(sum)
    };
    Ok([
        count(&mut shapes.iter().map(|s| s.head))?,
        count(&mut shapes.iter().map(|s| s.series))?,
        count(&mut shapes.iter().flat_map(|s| s.children.iter().copied()))?,
        count(&mut shapes.iter().map(|s| s.user_fields))?,
        count(&mut ["field_override"].into_iter())?,
    ])
}

/// `Store.DeleteBatchRows` — 하나씩 지우고 연결은 끝에 한 번 다시 짓는다.
fn delete_batch_rows(
    connection: &Connection,
    entities: &[EntityRef],
    whole_series: bool,
) -> Result<()> {
    for entity in entities {
        delete_on(connection, entity, whole_series, false)?;
    }
    explicit_links::resolve(connection)?;
    Ok(())
}

/// `Store.LinkIdentities` — 공고 건을 다시 만들며 끝점이 옮겨져도 연결 자체는 살아 있다. 불변 기본키로 소멸만 센다.
fn link_identities(connection: &Connection) -> Result<HashSet<String>> {
    Ok(strings(
        connection,
        "SELECT 'contract:' || contract_base FROM project_link UNION ALL SELECT 'request:' || request_base FROM request_link;",
        [],
    )?
    .into_iter()
    .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimal_number_style() {
        for ok in [
            "0", "12", " 12 ", "-1", "+1", "1-", "1 -", "1,234", "1,,2", "1.5", ".5", "5.",
            "1,234.5", "\t7\r\n", "007",
        ] {
            assert!(dotnet_decimal_number(ok), "{ok:?}");
        }
        for bad in [
            "",
            " ",
            "-",
            ".",
            ",1",
            "1e3",
            "- 1",
            "-1-",
            "1.2.3",
            "1.2,3",
            "a",
            "(1)",
            "$1",
            "79228162514264337593543950336",
            "1 2",
        ] {
            assert!(!dotnet_decimal_number(bad), "{bad:?}");
        }
        assert!(dotnet_decimal_number("79228162514264337593543950335"));
    }

    #[test]
    fn date_exact_formats() {
        for ok in ["2026-09-20", "2026/09/20", "2024-02-29", "0001-01-01"] {
            assert!(dotnet_date_exact(ok), "{ok:?}");
        }
        for bad in [
            "2026-9-20",
            "2026-09/20",
            "2026.09.20",
            "2025-02-29",
            "2026-13-01",
            "0000-01-01",
            " 2026-09-20",
            "2026-09-31",
            "20260920",
        ] {
            assert!(!dotnet_date_exact(bad), "{bad:?}");
        }
    }
}

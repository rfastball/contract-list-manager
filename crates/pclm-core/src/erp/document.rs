//! 문서 만들기 패널의 호스트 메서드 `document`(ADR-045 · 046) — 지금 화면이 저장된 것과 같은가(`check`), 같은 규칙으로
//! 저장(`save`), 「문서 서식」 으로 셈한 그 건 한 줄(`read`).
//!
//! - **저장 판단은 자동 수집과 한 벌이다**([`mirror::plan`]) — 사람이 고친 칸은 지키고, 화면에 비었다고 있던 값을 지우지
//!   않고, 화면에 없는 품목 줄을 지우지 않는다. 담당자 이름 조건은 보지 않는다(사람이 누른 것이다).
//! - **새 쓰기 길을 내지 않는다.** 저장은 [`ErpCapture::save`] 를 탄다 — 쓰기 잠금 · 역할 `work` · 쪽지 · 지문 대조가 거기 있다.
//!   저장하지 못하는 까닭(품목 대응 · 총액 · 수집 규칙)은 자동 수집이 멈추는 것과 같은 거절 글을 `blocked` 로 싣는다.
//! - **읽기는 작업자료의 「문서 서식」 정의를 셈한다**([`read`], 없으면 exe 의 것) — 그 흐름식이 면 뷰를 읽으므로 표기와
//!   덮개는 면 뷰의 것이다.
//!   다른 자리는 저장된 연결(사슬)로만 찾고 짐작하지 않는다 — 한 공고에 계약이 여럿이면 줄마다 다른 칸을 비우고 수만 알린다.
//! - 값은 뷰가 낸 글 그대로다 — 빈 값은 빈 문자열이다.
//! - `check` · `save` 의 답은 무엇이 다른지(`changes`)를 싣는다 — 기본 규칙이 `keep` 으로 두지 않는 바뀜만, 앞에서
//!   [`MAX_CHANGES`] 개. 이름표는 창의 「지금 보는 화면」 이 같은 칸에 다는 면 열 이름(품목 칸은 품목 뷰의 열 이름)이고,
//!   값은 그 면 열이 저장한 뒤에 낼 표기(`12,345,000` · `2026/10/30`)다 — 뷰의 표기식을 그대로 돌려 패널이 흉내 내지 않는다.

use std::path::Path;

use rusqlite::{
    Connection, OpenFlags, OptionalExtension, Transaction, TransactionBehavior, params,
};
use serde_json::{Map, Value};

use super::auto_capture::dashed;
use super::capture::{self, CaptureChange, ErpCapture};
use super::element;
use super::mirror::{self, SavePlan};
use crate::error::{Error, Result};
use crate::storage::database::connect;
use crate::storage::store::{DOCUMENT_VIEW_ID, EntityRef, builtin_view_package, text_of};
use crate::storage::{user_view_builder, views};

/// `changes` 에 싣는 바뀜의 수 — 패널은 무엇이 다른지 알아볼 만큼만 보인다(수는 `changedCount` 가 따로 센다).
pub const MAX_CHANGES: usize = 20;

/// `{op}` 하나에 답한다. 프로토콜 · 요청 번호 · 역할 · 자료 ID 는 부르는 쪽([`ErpCapture::dispatch`])이 이미 봤다.
pub(crate) fn dispatch(
    capture: &ErpCapture<'_>,
    args: &Value,
    files_allowed: bool,
) -> Result<Value> {
    match element::string(element::get(args, "op")?)? {
        Some("check") => check(capture, &capture::live_input(args, files_allowed)?),
        Some("save") => save(capture, &capture::live_input(args, files_allowed)?),
        Some("read") => read(capture, args),
        _ => Err(Error::invalid("지원하지 않는 요청입니다.")),
    }
}

fn check(capture: &ErpCapture<'_>, input: &capture::CaptureInput) -> Result<Value> {
    match mirror::plan(capture, input) {
        Ok(plan) => planned(capture, &plan, None),
        Err(Error::InvalidOperation(reason)) => Ok(unplanned(&reason)),
        Err(e) => Err(e),
    }
}

fn save(capture: &ErpCapture<'_>, input: &capture::CaptureInput) -> Result<Value> {
    let plan = match mirror::plan(capture, input) {
        Ok(plan) => plan,
        Err(Error::InvalidOperation(reason)) => return Ok(saved(unplanned(&reason), false)),
        Err(e) => return Err(e),
    };
    if plan.changed == 0 {
        return Ok(saved(planned(capture, &plan, None)?, false));
    }
    // 자동 수집과 같은 저장 — 짝지은 입력 · 그 지문 · 기본 규칙의 고름 · 새 수집 ID.
    let id = dashed(&capture.env().new_id());
    match capture.save(
        &plan.input,
        Some(&plan.preview.base_token),
        Some(&id),
        &plan.choices,
    ) {
        Ok(_) => {}
        Err(Error::InvalidOperation(reason)) => {
            return Ok(saved(planned(capture, &plan, Some(&reason))?, false));
        }
        Err(e) => return Err(e),
    }
    // 저장한 뒤의 모습을 다시 묻는다 — 「같다」 를 지어 말하지 않는다.
    Ok(saved(check(capture, input)?, true))
}

/// 판단이 선 화면의 답. `blocked` 가 있으면 저장이 그 글로 거절된 것이다.
fn planned(capture: &ErpCapture<'_>, plan: &SavePlan, blocked: Option<&str>) -> Result<Value> {
    let doc = &plan.preview.document;
    let state = match blocked {
        Some(_) => "blocked",
        None if plan.changed == 0 => "same",
        None => "differs",
    };
    let mut out = Map::new();
    out.insert("state".into(), Value::String(state.into()));
    out.insert("entity".into(), Value::String(plan.preview.entity.clone()));
    out.insert(
        "entityType".into(),
        Value::String(plan.preview.entity_type.clone()),
    );
    out.insert(
        "isNew".into(),
        Value::Bool(!stored(capture, &doc.entity_type, &doc.base, &doc.seq)?),
    );
    out.insert("changedCount".into(), plan.changed.into());
    out.insert("changes".into(), Value::Array(changes(capture, plan)?));
    if let Some(reason) = blocked {
        out.insert("reason".into(), Value::String(reason.into()));
    }
    Ok(Value::Object(out))
}

/// 화면을 짝짓지도 못한 답(수집 규칙이 바뀜 · 받지 않는 화면 따위) — 무슨 건인지도 모른다.
fn unplanned(reason: &str) -> Value {
    let mut out = Map::new();
    out.insert("state".into(), Value::String("blocked".into()));
    out.insert("entity".into(), Value::Null);
    out.insert("entityType".into(), Value::Null);
    out.insert("isNew".into(), Value::Bool(false));
    out.insert("changedCount".into(), 0.into());
    out.insert("changes".into(), Value::Array(Vec::new()));
    out.insert("reason".into(), Value::String(reason.into()));
    Value::Object(out)
}

/// 저장하면 바뀔 것 — `changedCount` 가 세는 것(고른 것이 `keep` 이 아닌 바뀜)과 한 벌, 앞에서 [`MAX_CHANGES`] 개.
/// `{label, before, after}` 의 값은 그 칸을 보이는 면 열의 표기 그대로이고([`Shown`]), 없으면 빈 글이다.
fn changes(capture: &ErpCapture<'_>, plan: &SavePlan) -> Result<Vec<Value>> {
    let ty = plan.preview.entity_type.as_str();
    let mut shown = Shown::new(capture.path());
    let mut out = Vec::new();
    for ch in plan
        .preview
        .changes
        .iter()
        .filter(|ch| {
            plan.choices
                .iter()
                .find(|(k, _)| *k == ch.id)
                .map(|(_, v)| v.as_str())
                != Some("keep")
        })
        .take(MAX_CHANGES)
    {
        // 덮개(`__override`)의 값은 이미 면 뷰가 낸 글이고, 줄(`__row`)은 칸이 아니다.
        let cell = (ch.override_column.is_none() && ch.field != "__row")
            .then(|| views::face_cell(ty, &ch.table, &ch.field))
            .flatten();
        let mut value = |v: &Option<String>| -> Result<Value> {
            Ok(Value::String(match (v, &cell) {
                (None, _) => String::new(),
                (Some(v), Some(cell)) => shown.text(cell, &ch.table, &ch.field, v)?,
                (Some(v), None) => v.clone(),
            }))
        };
        let mut row = Map::new();
        row.insert("label".into(), Value::String(label(ty, ch, cell.as_ref())));
        row.insert("before".into(), value(&ch.before)?);
        row.insert("after".into(), value(&ch.after)?);
        out.push(Value::Object(row));
    }
    Ok(out)
}

/// 바뀜의 이름표. 칸은 창의 「지금 보는 화면」 처럼 그 칸을 보이는 면 열 이름([`views::face_cell`] — 머리 칸은 면 뷰의
/// 열, 품목 칸은 품목 뷰의 열), 딸린 표의 칸은 창의 표 이름과 줄 번호를 앞에 단다. 이어지는 면 열이 없으면 칸 이름
/// 그대로다.
fn label(ty: &str, change: &CaptureChange, cell: Option<&views::FaceCell>) -> String {
    if let Some(column) = &change.override_column {
        return column.clone();
    }
    let name = cell.map_or(change.field.as_str(), |c| c.column.as_str());
    if change.table != ty {
        return format!(
            "{} {} · {name}",
            mirror::table_name(&change.table),
            change.line
        );
    }
    name.to_string()
}

/// 업무 표에 들어갈 글을 면 열의 표기로 — 뷰가 그 칸에 씌우는 식([`views::FaceCell::notation`])을, 그 칸과 같은 열
/// 꼴(선언 형식의 친화성)의 칸에 담아 돌린다. 그래서 뷰가 저장한 뒤에 낼 글과 바이트까지 같다. 표기가 서지 않는 열(여러
/// 칸을 엮거나 다른 줄을 모으는 열)은 글 그대로다. 연결은 처음 쓸 때 연다.
struct Shown<'p> {
    path: &'p Path,
    open: Option<(Connection, Connection)>,
}

impl<'p> Shown<'p> {
    fn new(path: &'p Path) -> Self {
        Shown { path, open: None }
    }

    fn text(
        &mut self,
        cell: &views::FaceCell,
        table: &str,
        field: &str,
        value: &str,
    ) -> Result<String> {
        let Some(sql) = cell.notation("value") else {
            return Ok(value.to_string());
        };
        if self.open.is_none() {
            self.open = Some((
                connect(self.path, OpenFlags::SQLITE_OPEN_READ_ONLY)?,
                Connection::open_in_memory()?,
            ));
        }
        let Some((store, memory)) = &self.open else {
            unreachable!("방금 열었다");
        };
        let declared: String = store
            .query_row(
                "SELECT type FROM pragma_table_info(?1) WHERE name=?2",
                params![table, field],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or_default();
        memory.execute_batch(&format!(
            "DROP TABLE IF EXISTS cell; CREATE TABLE cell(value {});",
            affinity(&declared)
        ))?;
        memory.execute("INSERT INTO cell(value) VALUES(?1)", [value])?;
        Ok(
            memory.query_row(&format!("SELECT {sql} FROM cell"), [], |r| {
                Ok(text_of(r.get_ref(0)?).unwrap_or_default())
            })?,
        )
    }
}

/// 선언 형식의 열 친화성(SQLite 「Determination Of Column Affinity」 의 차례) — 담는 칸을 그 이름 하나로 선언한다.
fn affinity(declared: &str) -> &'static str {
    let t = declared.to_ascii_uppercase();
    if t.contains("INT") {
        "INTEGER"
    } else if t.contains("CHAR") || t.contains("CLOB") || t.contains("TEXT") {
        "TEXT"
    } else if t.contains("BLOB") || t.is_empty() {
        "BLOB"
    } else if t.contains("REAL") || t.contains("FLOA") || t.contains("DOUB") {
        "REAL"
    } else {
        "NUMERIC"
    }
}

fn saved(mut shape: Value, saved: bool) -> Value {
    if let Value::Object(out) = &mut shape {
        out.insert("saved".into(), Value::Bool(saved));
    }
    shape
}

/// 그 차수가 이미 쌓였는가. `entity_type` 은 매핑이 낸 종류(`request`·`notice`·`contract`)다.
fn stored(capture: &ErpCapture<'_>, entity_type: &str, base: &str, seq: &str) -> Result<bool> {
    let c = connect(capture.path(), OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    Ok(c.query_row(
        &format!(
            "SELECT EXISTS(SELECT 1 FROM {entity_type} WHERE {entity_type}_base=?1 AND seq=?2)"
        ),
        params![base, seq],
        |r| r.get(0),
    )?)
}

// ── 읽기 ─────────────────────────────────────────────

/// 작업자료에 그 줄이 없을 때 셈하는 exe 의 보기 구성([`builtin_view_package`]) 속 보기 이름.
const VIEW: &str = "문서 서식";

/// 패널이 셈하는 정의 — 작업자료의 「문서 서식」 줄([`DOCUMENT_VIEW_ID`], 사람이 고쳤으면 고친 것, ADR-052). 이름이 아니라
/// 식별자로 찾는다. 그 줄이 없으면 exe 의 정의다. 깔린 `u_*` 는 읽지 않는다 — 탭의 공고로 줄을 집어야 해서 정의를 셈한다.
fn document_definition(tx: &Connection) -> Result<String> {
    let saved: Option<Option<String>> = tx
        .query_row(
            "SELECT definition FROM user_view WHERE id = ?1",
            [DOCUMENT_VIEW_ID],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(Some(definition)) = saved {
        return Ok(definition);
    }
    let package: Value = serde_json::from_str(builtin_view_package())
        .map_err(|e| Error::invalid(format!("통합 뷰를 읽지 못했습니다: {e}")))?;
    package["views"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|v| v["name"] == VIEW)
        .map(|v| v["definition"].to_string())
        .ok_or_else(|| Error::invalid("통합 뷰를 찾지 못했습니다."))
}

/// 지금 탭의 건을 「문서 서식」 의 정의([`document_definition`])로 셈한 한 줄. 정의는 읽을 때마다 컴파일하고, DB 에는 아무것도 쓰지 않는다.
///
/// 답: `{entityType, entity, values: {열 이름: 글}, rows, older, caseKeys}` — 건이 쌓여 있지 않으면 `null`.
/// - 줄은 바탕 사슬(접수 · 공고건 · 계약 본번호)로 집는다 — 뷰의 열 이름이 바뀌어도 집기는 그대로다.
/// - 탭이 공고면 공고 자리는 건의 현행 공고가 아니라 그 탭의 공고다(유찰된 옛 공고 · 취소공고를 보고 있을 수 있다).
/// - 줄이 여럿(한 공고에 계약 여럿)이면 줄마다 다른 칸은 빈 글이고 `rows` 가 그 수다 — 한 줄을 짐작해 고르지 않는다.
/// - `older` 는 탭의 차수가 쌓인 최신 차수보다 앞선 것이다. 값은 최신 차수다(면 뷰가 최신 차수만 낸다).
/// - `caseKeys` 는 저장한 파일을 이 건에 잇는 열쇠(`kind:본번호`, 공고는 공고건) — 차수가 올라도 같다.
fn read(capture: &ErpCapture<'_>, args: &Value) -> Result<Value> {
    let entity_type = match element::string(element::get(args, "entityType")?)? {
        Some(t @ ("request" | "notice" | "contract")) => t,
        _ => {
            return Err(Error::invalid(
                "문서를 만들 건의 종류를 확인하지 못했습니다.",
            ));
        }
    };
    let Some((base, seq)) = element::string(element::get(args, "entity")?)?
        .and_then(|e| e.rsplit_once('-'))
        .filter(|(b, s)| !b.is_empty() && !s.is_empty())
    else {
        return Err(Error::invalid(
            "문서를 만들 건의 번호를 확인하지 못했습니다.",
        ));
    };
    let c = connect(capture.path(), OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let tx = Transaction::new_unchecked(&c, TransactionBehavior::Deferred)?;
    let latest: Option<i64> = tx.query_row(
        &format!("SELECT max(CAST(seq AS INTEGER)) FROM {entity_type} WHERE {entity_type}_base=?1"),
        [base],
        |r| r.get(0),
    )?;
    let Some(latest) = latest else {
        return Ok(Value::Null);
    };

    let definition = document_definition(&tx)?;
    let columns = user_view_builder::parse_columns(&definition)?;
    let frame = user_view_builder::frame(
        user_view_builder::parse_fold(&definition)?,
        columns.iter().map(|c| c.formula.as_str()),
    );
    let env = user_view_builder::env(&tx)?;
    let mut select = Vec::new();
    for column in &columns {
        // 깨진 열은 `u_*` 처럼 빈 글이다 — 사람이 고친 식 하나가 읽기 전체를 막지 않는다.
        select.push(
            user_view_builder::try_column(&tx, &column.formula, &env, &frame, false)?
                .map_or_else(|_| "''".to_string(), |(expression, _)| expression),
        );
    }
    select.push("s.접수, s.공고, s.계약".into());
    let (where_, notice) = match entity_type {
        "request" => ("s.접수 = :base", None),
        "contract" => ("s.계약 = :base", None),
        _ => (
            "s.공고 = (SELECT group_base FROM notice_series WHERE notice_base = :base)",
            Some(":base"),
        ),
    };
    let sql = frame.statement_on(notice, &select.join(",\n"), Some(where_), "");
    let mut statement = tx.prepare(&sql)?;
    let mut found = statement.query(rusqlite::named_params! {":base": base})?;
    let mut merged: Vec<Option<String>> = vec![None; columns.len()];
    let mut keys: Vec<String> = Vec::new();
    let mut rows = 0usize;
    while let Some(row) = found.next()? {
        rows += 1;
        for (i, slot) in merged.iter_mut().enumerate() {
            let v = text_of(row.get_ref(i)?).unwrap_or_default();
            match slot {
                None => *slot = Some(v),
                Some(seen) if *seen != v => seen.clear(),
                Some(_) => {}
            }
        }
        for (i, kind) in ["request", "notice", "contract"].into_iter().enumerate() {
            if let Some(b) = text_of(row.get_ref(columns.len() + i)?) {
                let key = format!("{kind}:{b}");
                if !keys.contains(&key) {
                    keys.push(key);
                }
            }
        }
    }
    let values: Map<String, Value> = columns
        .iter()
        .zip(merged)
        .map(|(c, v)| (c.name.clone(), Value::String(v.unwrap_or_default())))
        .collect();
    let mut out = Map::new();
    out.insert("entityType".into(), Value::String(entity_type.into()));
    out.insert(
        "entity".into(),
        Value::String(EntityRef::new(entity_type, base, seq).display()),
    );
    out.insert("values".into(), Value::Object(values));
    out.insert("rows".into(), rows.into());
    out.insert(
        "older".into(),
        Value::Bool(seq.parse::<i64>().is_ok_and(|s| s < latest)),
    );
    out.insert(
        "caseKeys".into(),
        Value::Array(keys.into_iter().map(Value::String).collect()),
    );
    Ok(Value::Object(out))
}

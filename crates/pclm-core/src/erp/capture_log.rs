//! 수집 기록(C# `Erp/CaptureLog.cs`) — `erp_capture` 를 사람이 읽을 꼴로. **읽기만 한다** — 열람 창의 읽기 전용 사본에서도
//! 같다. 성공한 저장만 기록에 남는다. 검토 중이거나 결과를 확인하지 못한 것은 브라우저에 있다.
//!
//! 날짜 경계는 **로컬 자정**이다 — 사람이 말하는 「오늘」 이 그것이다. 시간대는 부르는 쪽이 [`LocalZone`] 으로 넣는다.

use rusqlite::Connection;
use serde_json::{Map, Value};

use super::mapping::{self, MappingSet};
use crate::error::Result;
use crate::json;
use crate::local_time::{LocalDateTime, LocalZone, parse_to_local, to_utc_text};
use crate::storage::database::Database;
use crate::storage::store::{EntityRef, text_of};

/// C# `CaptureEntry` — 수집 기록 한 줄. `at` 은 **로컬** `yyyy-MM-ddTHH:mm:ss`, `result` 는 `저장`·`변경 없음`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureEntry {
    pub capture_id: String,
    pub at: String,
    pub entity_type: String,
    pub kind: String,
    pub number: String,
    pub title: String,
    pub changed: bool,
    pub result: String,
    pub scope: String,
}

impl CaptureEntry {
    pub fn to_value(&self) -> Value {
        let mut o = Map::new();
        o.insert("captureId".into(), Value::String(self.capture_id.clone()));
        o.insert("at".into(), Value::String(self.at.clone()));
        o.insert("entityType".into(), Value::String(self.entity_type.clone()));
        o.insert("kind".into(), Value::String(self.kind.clone()));
        o.insert("number".into(), Value::String(self.number.clone()));
        o.insert("title".into(), Value::String(self.title.clone()));
        o.insert("changed".into(), Value::Bool(self.changed));
        o.insert("result".into(), Value::String(self.result.clone()));
        o.insert("scope".into(), Value::String(self.scope.clone()));
        Value::Object(o)
    }
}

/// C# `CaptureDay` — 오늘 들어온 수집. `entries` 는 오늘 것 전부(늦은 것부터), `last` 는 가장 늦은 수집 하나(오늘이
/// 아니어도 온다).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureDay {
    pub today: String,
    pub entries: Vec<CaptureEntry>,
    pub requests: i64,
    pub notices: i64,
    pub contracts: i64,
    pub saved: i64,
    pub last: Option<CaptureEntry>,
}

impl CaptureDay {
    pub fn to_value(&self) -> Value {
        let mut o = Map::new();
        o.insert("today".into(), Value::String(self.today.clone()));
        o.insert(
            "entries".into(),
            Value::Array(self.entries.iter().map(CaptureEntry::to_value).collect()),
        );
        o.insert("requests".into(), self.requests.into());
        o.insert("notices".into(), self.notices.into());
        o.insert("contracts".into(), self.contracts.into());
        o.insert("saved".into(), self.saved.into());
        o.insert(
            "last".into(),
            self.last
                .as_ref()
                .map_or(Value::Null, CaptureEntry::to_value),
        );
        Value::Object(o)
    }
}

/// `CaptureLog.KindName` — 모르는 종류는 빈 문자열.
pub fn kind_name(entity_type: &str) -> &'static str {
    match entity_type {
        "request" => "접수",
        "notice" => "공고",
        "contract" => "계약",
        _ => "",
    }
}

struct Raw {
    capture_id: String,
    profile: String,
    entity_base: String,
    entity_seq: String,
    snapshot_json: String,
    result_json: String,
    captured_at: String,
}

const COLUMNS: &str =
    "capture_id, profile, entity_base, entity_seq, snapshot_json, result_json, captured_at";

fn raws(c: &Connection, sql: &str, args: &[&str]) -> Result<Vec<Raw>> {
    let mut statement = c.prepare(sql)?;
    let out = statement
        .query_map(rusqlite::params_from_iter(args.iter()), |r| {
            let t = |i: usize| -> rusqlite::Result<String> {
                Ok(text_of(r.get_ref(i)?).unwrap_or_default())
            };
            Ok(Raw {
                capture_id: t(0)?,
                profile: t(1)?,
                entity_base: t(2)?,
                entity_seq: t(3)?,
                snapshot_json: t(4)?,
                result_json: t(5)?,
                captured_at: t(6)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(out)
}

/// `CaptureLog.Read` — `now` 는 로컬 지금이다(시험이 날짜 경계를 세우려고 넘긴다).
pub fn read(database: &Database, now: LocalDateTime, zone: &dyn LocalZone) -> Result<CaptureDay> {
    let start = now.date();
    let end = start.add_days(1);
    // 저장은 UTC 의 왕복 표기("O")라 글자 순서가 곧 시각 순서다. 하루 앞까지 넉넉히 읽고 아래에서 로컬로 다시 가른다.
    let from = to_utc_text(start.add_days(-1), zone);

    let c = database.open_read_only()?;
    let recent = raws(
        &c,
        &format!(
            "SELECT {COLUMNS} FROM erp_capture WHERE captured_at >= ?1 ORDER BY captured_at DESC"
        ),
        &[&from],
    )?;
    let last = raws(
        &c,
        &format!("SELECT {COLUMNS} FROM erp_capture ORDER BY captured_at DESC LIMIT 1"),
        &[],
    )?
    .into_iter()
    .next();

    // 지금 매핑. 읽지 못하면 기본 매핑으로 물러선다: 기록을 보이는 일이 매핑 탓에 멎을 까닭이 없다.
    let active = mapping::active_in(&c).unwrap_or_else(|_| mapping::defaults());
    let defaults = mapping::defaults();
    let mut profiles: Vec<(String, String)> = Vec::new();
    for p in defaults.profiles_iter().chain(active.profiles_iter()) {
        let id = p.id().to_string();
        let ty = p.entity_type().to_string();
        match profiles.iter_mut().find(|(k, _)| *k == id) {
            Some(slot) => slot.1 = ty,
            None => profiles.push((id, ty)),
        }
    }
    let mut titles: Vec<((String, String, String), String)> = Vec::new();

    let mut entries = Vec::new();
    for raw in &recent {
        let Some(at) = parse_to_local(Some(&raw.captured_at), zone) else {
            continue;
        };
        if at < start || at >= end {
            continue;
        }
        entries.push(entry(&c, raw, at, &active, &profiles, &mut titles)?);
    }
    let last_entry = match &last {
        Some(raw) => match parse_to_local(Some(&raw.captured_at), zone) {
            Some(at) => Some(entry(&c, raw, at, &active, &profiles, &mut titles)?),
            None => None,
        },
        None => None,
    };
    let count = |ty: &str| entries.iter().filter(|e| e.entity_type == ty).count() as i64;
    Ok(CaptureDay {
        today: start.day_text(),
        requests: count("request"),
        notices: count("notice"),
        contracts: count("contract"),
        saved: entries.iter().filter(|e| e.changed).count() as i64,
        entries,
        last: last_entry,
    })
}

fn entry(
    c: &Connection,
    raw: &Raw,
    at: LocalDateTime,
    mapping: &MappingSet,
    profiles: &[(String, String)],
    titles: &mut Vec<((String, String, String), String)>,
) -> Result<CaptureEntry> {
    let mut changed = false;
    let mut result_type = String::new();
    let mut scope = String::new();
    if let Ok(Value::Object(root)) = json::parse(&raw.result_json) {
        if let Some(Value::Bool(b)) = root.get("changed") {
            changed = *b;
        }
        if let Some(Value::String(t)) = root.get("entityType") {
            result_type = t.clone();
        }
        if let Some(Value::String(s)) = root.get("scope") {
            scope = s.clone();
        }
    }
    let entity_type = profiles
        .iter()
        .find(|(k, _)| *k == raw.profile)
        .map_or(result_type, |(_, v)| v.clone());
    let key = (
        entity_type.clone(),
        raw.entity_base.clone(),
        raw.entity_seq.clone(),
    );
    let title = match titles.iter().find(|(k, _)| *k == key) {
        Some((_, t)) => t.clone(),
        None => {
            let t = match current_title(c, &entity_type, &raw.entity_base, &raw.entity_seq)? {
                Some(t) => t,
                None => snapshot_title(raw, mapping).unwrap_or_default(),
            };
            titles.push((key, t.clone()));
            t
        }
    };
    Ok(CaptureEntry {
        capture_id: raw.capture_id.clone(),
        at: at.second_text(),
        kind: kind_name(&entity_type).into(),
        number: EntityRef::new(&entity_type, &raw.entity_base, &raw.entity_seq).display(),
        title,
        result: if changed { "저장" } else { "변경 없음" }.into(),
        changed,
        scope,
        entity_type,
    })
}

/// `CaptureLog.CurrentTitle` — 지금 쌓인 그 차수의 건명. 비었거나 지워졌으면 `None`.
fn current_title(
    c: &Connection,
    entity_type: &str,
    base: &str,
    seq: &str,
) -> Result<Option<String>> {
    // 표 이름은 여기 셋 중 하나뿐이다 — 기록의 값을 SQL 에 그대로 잇지 않는다.
    let table = match entity_type {
        "request" => "request",
        "notice" => "notice",
        "contract" => "contract",
        _ => return Ok(None),
    };
    let mut statement = c.prepare(&format!(
        "SELECT title FROM {table} WHERE {table}_base = ?1 AND seq = ?2"
    ))?;
    let mut rows = statement.query([base, seq])?;
    let Some(first) = rows.next()? else {
        return Ok(None);
    };
    let title = text_of(first.get_ref(0)?);
    if rows.next()?.is_some() {
        return Err(crate::Error::invalid(
            "Sequence contains more than one element",
        ));
    }
    Ok(title.filter(|t| !t.is_empty()))
}

/// `CaptureLog.SnapshotTitle` — 수집 사진에 담긴 건명. 사진을 지금 매핑으로 다시 읽어 본다 — 읽지 못하면 `None`.
fn snapshot_title(raw: &Raw, mapping: &MappingSet) -> Option<String> {
    let Ok(Value::Object(root)) = json::parse(&raw.snapshot_json) else {
        return None;
    };
    let data = root.get("data")?;
    let scope = match root.get("scope") {
        Some(Value::String(s)) => s.as_str(),
        _ => "file",
    };
    let document = mapping::map(mapping, &raw.profile, data, scope).ok()?;
    let head = document
        .rows
        .iter()
        .find(|r| r.table == document.entity_type)?;
    head.value("title")
        .filter(|t| !t.is_empty())
        .map(str::to_string)
}

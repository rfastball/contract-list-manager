//! 수집 저장(C# `Erp/ErpCapture.cs`) — 확장과 파일 입력의 같은 미리보기·저장 경계. 부분 열 갱신만 허용한다.
//!
//! - **미리보기(inspect)** 는 원천을 매핑으로 옮겨 지금 쌓인 값과 견주고(ADR-041), 사람이 고른 것과 지금 상태의 지문
//!   (`baseToken`)을 낸다. 사람이 고친 칸의 덮개는 이번 수집이 그 아래 값을 바꿀 때만 묻는다 — 같은 트랜잭션의
//!   savepoint 에 써 보고 되돌린다.
//! - **저장(capture)** 은 쓰기 잠금 안에서 미리보기를 다시 하고, 지문이 같고 충돌마다 고른 것이 있을 때만 쓴다. 같은 수집
//!   ID 를 다시 보내면 처음의 영수증을 그대로 돌려준다.
//! - **덮개(`field_override`)에는 쓰지 않는다**(ADR-020). 사람이 「수집 원값으로 복원」 을 고를 때 그 차수의 그 칸 덮개를
//!   지우기만 한다.
//! - **값을 검산하지 않는다**(ADR-016). 적힌 그대로 담는다 — 막는 것은 번호를 읽지 못한 자료뿐이다.
//!
//! 홈은 모른다(#10) — 쓰기 잠금 안에서 쪽지가 아직 이 파일을 가리키는지(ADR-032)는 부르는 쪽이 [`Destination`] 으로
//! 넣는다. 확장 폴더도 모른다 — 인사(`hello`)가 디스크의 확장 판을 알리고 연결을 적는 일은 [`ExtensionHello`] 로 넣는다.

use std::path::{Path, PathBuf};

use rusqlite::types::{Value as Sql, ValueRef};
use rusqlite::{
    Connection, OpenFlags, OptionalExtension, Transaction, TransactionBehavior, params,
};
use serde_json::{Map, Value};

use super::element;
use super::explicit_links::{self, ReferenceState};
use super::mapping::{self, MappedDocument, MappedRow, MappingSet, Pairs, set_pair};
use super::screens;
use crate::env::Env;
use crate::error::{Error, Result};
use crate::json::{self, Encoder};
use crate::storage::database::connect;
use crate::storage::pclm_file::PclmRole;
use crate::storage::schema;
use crate::storage::store::{self, EntityRef, face_key, face_view, json_of, text_of};
use crate::text::eq_ignore_case;

/// 확장이 한 번에 보낼 수 있는 요청의 크기(C# `NativeMessaging.MaxRequestBytes`). 인사가 알린다.
pub const MAX_REQUEST_BYTES: i64 = 768 * 1024;

/// 쓰기 잠금 안에서 쪽지가 아직 이 파일을 가리키는가(C# `Home.PointsAt`). 확장 호스트와 창의 저장이 넣는다.
pub trait Destination {
    fn points_at(&self, database_path: &Path, dataset_id: &str) -> bool;
}

/// 업무 호스트의 인사가 하는 일(C# `ExtensionStatus.DiskVersion`·`RecordContact`). 적지 못해도 인사는 성공이다 —
/// 구현이 오류를 삼킨다.
pub trait ExtensionHello {
    /// 디스크에 풀어 둔 확장의 판. 모르면 `None`.
    fn disk_version(&self) -> Option<String>;
    /// 브라우저별 마지막 연결을 적는다.
    fn record_contact(&self, version: &str, browser: &str);
}

// ── 입력과 미리보기 ─────────────────────────────────────────────

/// C# `CaptureInput` — 수집 입력. `screen` 은 그 자료를 읽은 화면의 메뉴 번호다(비었으면 쓰지 않는다 — 같은 요청의
/// 지문이 예전과 같다). `data` 가 `None` 이면 C# 의 기본값 `JsonElement`(Undefined)다.
#[derive(Clone, Debug, PartialEq)]
pub struct CaptureInput {
    pub profile: Option<String>,
    pub mapping_revision: Option<String>,
    pub data: Option<Value>,
    pub scope: Option<String>,
    pub totals: Option<Vec<(String, i32)>>,
    pub row_matches: Option<Vec<(String, i32)>>,
    pub screen: Option<String>,
}

fn int_pairs_value(pairs: &Option<Vec<(String, i32)>>) -> Value {
    match pairs {
        None => Value::Null,
        Some(pairs) => Value::Object(
            pairs
                .iter()
                .map(|(k, v)| (k.clone(), Value::from(*v)))
                .collect(),
        ),
    }
}

fn text_value(text: &Option<String>) -> Value {
    text.clone().map_or(Value::Null, Value::String)
}

impl CaptureInput {
    /// `new CaptureInput(profile, mappingRevision, data)` — 파일 범위.
    pub fn new(profile: &str, mapping_revision: &str, data: Value) -> Self {
        CaptureInput {
            profile: Some(profile.into()),
            mapping_revision: Some(mapping_revision.into()),
            data: Some(data),
            scope: Some("file".into()),
            totals: None,
            row_matches: None,
            screen: None,
        }
    }

    pub fn profile(&self) -> &str {
        self.profile.as_deref().unwrap_or("")
    }

    /// `Mapping.Json` 으로 쓴 꼴. `screen` 은 있을 때만 쓴다.
    pub fn to_value(&self) -> Value {
        let mut out = Map::new();
        out.insert("profile".into(), text_value(&self.profile));
        out.insert("mappingRevision".into(), text_value(&self.mapping_revision));
        out.insert("data".into(), self.data.clone().unwrap_or(Value::Null));
        out.insert("scope".into(), text_value(&self.scope));
        out.insert("totals".into(), int_pairs_value(&self.totals));
        out.insert("rowMatches".into(), int_pairs_value(&self.row_matches));
        if let Some(screen) = &self.screen {
            out.insert("screen".into(), Value::String(screen.clone()));
        }
        Value::Object(out)
    }

    /// `element.Deserialize<CaptureInput>(Mapping.Json)` — 모르는 칸은 거절하고 이름은 대소문자를 가리지 않는다.
    /// JSON `null` 이면 `None`.
    ///
    /// 거절 글은 System.Text.Json 의 글 앞부분만 흉내 낸다 — .NET 은 원문의 줄·바이트 자리를 덧붙인다(같게 내지 않는다).
    pub fn from_json(value: &Value) -> Result<Option<CaptureInput>> {
        const TYPE: &str = "Pclm.Core.Erp.CaptureInput";
        let map = match value {
            Value::Null => return Ok(None),
            Value::Object(map) => map,
            _ => {
                return Err(Error::invalid(format!(
                    "The JSON value could not be converted to {TYPE}. Path: $"
                )));
            }
        };
        let converted = |path: &str, to: &str| {
            Error::invalid(format!(
                "The JSON value could not be converted to {to}. Path: $.{path}"
            ))
        };
        let text = |key: &str, v: &Value| -> Result<Option<String>> {
            json::read_string(v).map_err(|_| converted(key, "System.String"))
        };
        let ints = |key: &str, v: &Value| -> Result<Option<Vec<(String, i32)>>> {
            match v {
                Value::Null => Ok(None),
                Value::Object(m) => m
                    .iter()
                    .map(|(k, item)| {
                        let n = json::read_i32(item)
                            .ok()
                            .flatten()
                            .ok_or_else(|| converted(&format!("{key}.{k}"), "System.Int32"))?;
                        Ok((k.clone(), n))
                    })
                    .collect::<Result<Vec<_>>>()
                    .map(Some),
                _ => Err(converted(
                    key,
                    "System.Collections.Generic.Dictionary`2[System.String,System.Int32]",
                )),
            }
        };
        let mut input = CaptureInput {
            profile: None,
            mapping_revision: None,
            data: None,
            scope: Some("file".into()),
            totals: None,
            row_matches: None,
            screen: None,
        };
        const NAMES: [&str; 7] = [
            "profile",
            "mappingRevision",
            "data",
            "scope",
            "totals",
            "rowMatches",
            "screen",
        ];
        for (key, v) in map {
            match NAMES.iter().position(|n| eq_ignore_case(n, key)) {
                Some(0) => input.profile = text(key, v)?,
                Some(1) => input.mapping_revision = text(key, v)?,
                Some(2) => input.data = Some(v.clone()),
                Some(3) => input.scope = text(key, v)?,
                Some(4) => input.totals = ints(key, v)?,
                Some(5) => input.row_matches = ints(key, v)?,
                Some(6) => input.screen = text(key, v)?,
                _ => {
                    return Err(Error::invalid(format!(
                        "The JSON property '{key}' could not be mapped to any .NET member contained in type '{TYPE}'."
                    )));
                }
            }
        }
        Ok(Some(input))
    }

    fn row_match(&self, id: &str) -> Option<i32> {
        self.row_matches
            .as_ref()?
            .iter()
            .find(|(k, _)| k == id)
            .map(|(_, v)| *v)
    }
}

/// C# `CaptureChange` — 칸 하나의 바뀜. `field` 가 `__row` 면 화면에 없는 행(지우기를 고른다), `__override` 면 사람이
/// 고친 칸의 덮개(걷기를 고른다)다.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureChange {
    pub id: String,
    pub table: String,
    pub line: i64,
    pub field: String,
    pub before: Option<String>,
    pub after: Option<String>,
    pub conflict: bool,
    pub override_column: Option<String>,
}

impl CaptureChange {
    pub fn to_value(&self) -> Value {
        let mut out = Map::new();
        out.insert("id".into(), Value::String(self.id.clone()));
        out.insert("table".into(), Value::String(self.table.clone()));
        out.insert("line".into(), self.line.into());
        out.insert("field".into(), Value::String(self.field.clone()));
        out.insert("before".into(), text_value(&self.before));
        out.insert("after".into(), text_value(&self.after));
        out.insert("conflict".into(), Value::Bool(self.conflict));
        out.insert("override".into(), text_value(&self.override_column));
        Value::Object(out)
    }
}

/// 업무 표의 줄 하나 — 열 이름과 저장된 값, 열 차례 그대로(Dapper 의 동적 행).
pub type StoredRow = Vec<(String, Sql)>;

fn stored_value<'r>(row: &'r StoredRow, name: &str) -> Option<&'r Sql> {
    row.iter().find(|(k, _)| k == name).map(|(_, v)| v)
}

/// 동적 행을 System.Text.Json 으로 쓴 꼴 — 사전처럼 열 이름이 열쇠다.
pub fn stored_row_value(row: &StoredRow) -> Value {
    Value::Object(
        row.iter()
            .map(|(k, v)| (k.clone(), json_of(ValueRef::from(v))))
            .collect(),
    )
}

/// `Convert.ToInt32(row["line_no"])`. 없으면 0.
fn line_of(row: &StoredRow) -> i64 {
    match stored_value(row, "line_no") {
        Some(Sql::Integer(n)) => *n,
        Some(Sql::Real(r)) => r.round() as i64,
        Some(Sql::Text(t)) => t.trim().parse().unwrap_or(0),
        _ => 0,
    }
}

/// C# `RowMatch` — 작업자료에 있는 줄과 짝이 없는 화면의 줄. `existing` 은 그 표의 지금 줄들이다.
#[derive(Clone, Debug, PartialEq)]
pub struct RowMatch {
    pub id: String,
    pub table: String,
    pub source_key: String,
    pub existing: Vec<StoredRow>,
}

impl RowMatch {
    pub fn to_value(&self) -> Value {
        let mut out = Map::new();
        out.insert("id".into(), Value::String(self.id.clone()));
        out.insert("table".into(), Value::String(self.table.clone()));
        out.insert("sourceKey".into(), Value::String(self.source_key.clone()));
        out.insert(
            "existing".into(),
            Value::Array(self.existing.iter().map(stored_row_value).collect()),
        );
        Value::Object(out)
    }
}

/// C# `CapturePreview` — 미리보기. `entity` 는 `본번호-차수`(계약도 줄표를 넣는다 — C# 그대로).
#[derive(Clone, Debug, PartialEq)]
pub struct CapturePreview {
    pub dataset_id: String,
    pub entity: String,
    pub entity_type: String,
    pub base_token: String,
    pub mapping_revision: String,
    pub document: MappedDocument,
    pub changes: Vec<CaptureChange>,
    pub unmatched: Vec<RowMatch>,
    pub item_count: i64,
}

impl CapturePreview {
    /// 다리(camelCase)의 꼴.
    pub fn to_value(&self) -> Value {
        let mut out = Map::new();
        out.insert("datasetId".into(), Value::String(self.dataset_id.clone()));
        out.insert("entity".into(), Value::String(self.entity.clone()));
        out.insert("entityType".into(), Value::String(self.entity_type.clone()));
        out.insert("baseToken".into(), Value::String(self.base_token.clone()));
        out.insert(
            "mappingRevision".into(),
            Value::String(self.mapping_revision.clone()),
        );
        out.insert("document".into(), self.document.to_value());
        out.insert(
            "changes".into(),
            Value::Array(self.changes.iter().map(CaptureChange::to_value).collect()),
        );
        out.insert(
            "unmatched".into(),
            Value::Array(self.unmatched.iter().map(RowMatch::to_value).collect()),
        );
        out.insert("itemCount".into(), self.item_count.into());
        Value::Object(out)
    }
}

/// 저장의 영수증. 처음 저장이면 [`SaveResult::Stored`], 같은 수집 ID 의 재전송이면 그때 적은 영수증 글 그대로
/// [`SaveResult::Replayed`] 다.
#[derive(Clone, Debug, PartialEq)]
pub enum SaveResult {
    Stored(StoredCapture),
    Replayed(Value),
}

impl SaveResult {
    /// 부르는 쪽이 받는 꼴. 처음 저장은 다리·확장(Web) 꼴이라 연결의 칸이 camelCase 이고, 재전송은 적어 둔 영수증
    /// (옵션 없는 직렬화 — 연결의 칸이 PascalCase)이다. C# 이 그렇게 갈린다.
    pub fn to_value(&self) -> Value {
        match self {
            SaveResult::Stored(stored) => stored.to_value(false),
            SaveResult::Replayed(value) => value.clone(),
        }
    }
}

/// 처음 저장한 수집의 결과.
#[derive(Clone, Debug, PartialEq)]
pub struct StoredCapture {
    pub capture_id: String,
    pub dataset_id: String,
    pub entity: String,
    pub entity_type: String,
    pub changed: bool,
    pub item_count: i64,
    pub scope: Option<String>,
    pub links: Vec<ReferenceState>,
}

impl StoredCapture {
    fn to_value(&self, pascal: bool) -> Value {
        let names = if pascal {
            ["EntityType", "Base", "Target", "State"]
        } else {
            ["entityType", "base", "target", "state"]
        };
        let links = self
            .links
            .iter()
            .map(|r| {
                let mut o = Map::new();
                o.insert(names[0].into(), Value::String(r.entity_type.clone()));
                o.insert(names[1].into(), Value::String(r.base.clone()));
                o.insert(names[2].into(), Value::String(r.target.clone()));
                o.insert(names[3].into(), Value::String(r.state.clone()));
                Value::Object(o)
            })
            .collect();
        let mut out = Map::new();
        out.insert("captureId".into(), Value::String(self.capture_id.clone()));
        out.insert("datasetId".into(), Value::String(self.dataset_id.clone()));
        out.insert("status".into(), Value::String("stored".into()));
        out.insert("entity".into(), Value::String(self.entity.clone()));
        out.insert("entityType".into(), Value::String(self.entity_type.clone()));
        out.insert("changed".into(), Value::Bool(self.changed));
        out.insert("itemCount".into(), self.item_count.into());
        out.insert("scope".into(), text_value(&self.scope));
        out.insert("links".into(), Value::Array(links));
        Value::Object(out)
    }
}

/// 고른 것들 — 충돌마다 `keep` 또는 `apply`. 넣은 차례 그대로(지문이 그 차례로 쓴다).
pub type Choices = Vec<(String, String)>;

fn choice_of<'c>(choices: &'c Choices, id: &str) -> Option<&'c str> {
    choices
        .iter()
        .find(|(k, _)| k == id)
        .map(|(_, v)| v.as_str())
}

fn choices_value(choices: &Choices) -> Value {
    Value::Object(
        choices
            .iter()
            .map(|(k, v)| (k.clone(), Value::String(v.clone())))
            .collect(),
    )
}

/// `Deserialize<Dictionary<string, string>>(Mapping.Json)` — 값은 글(또는 null). null 은 빈 글로 든다.
pub fn choices_from_json(value: &Value) -> Result<Choices> {
    let map = value.as_object().ok_or_else(|| {
        Error::invalid("The JSON value could not be converted to System.Collections.Generic.Dictionary`2[System.String,System.String]. Path: $")
    })?;
    map.iter()
        .map(|(k, v)| {
            let text = json::read_string(v).map_err(|_| {
                Error::invalid(format!(
                    "The JSON value could not be converted to System.String. Path: $.{k}"
                ))
            })?;
            Ok((k.clone(), text.unwrap_or_default()))
        })
        .collect()
}

// ── 다리 ─────────────────────────────────────────────

/// `Guid.TryParse` — `N`(32자)·`D`(줄표)·`B`(중괄호)·`P`(괄호) 꼴. 앞뒤 공백은 걷는다.
pub(crate) fn is_guid(text: Option<&str>) -> bool {
    let Some(text) = text else { return false };
    let t = text.trim();
    let hex = |s: &str| s.bytes().all(|b| b.is_ascii_hexdigit());
    let dashed = |s: &str| {
        let parts: Vec<&str> = s.split('-').collect();
        parts.len() == 5
            && parts
                .iter()
                .zip([8, 4, 4, 4, 12])
                .all(|(p, n)| p.len() == n && hex(p))
    };
    (t.len() == 32 && hex(t))
        || dashed(t)
        || t.strip_prefix('{')
            .and_then(|s| s.strip_suffix('}'))
            .is_some_and(dashed)
        || t.strip_prefix('(')
            .and_then(|s| s.strip_suffix(')'))
            .is_some_and(dashed)
}

/// C# `ErpCapture` — 바인딩된 파일 하나 위의 수집 경계. 연산마다 연결을 새로 연다.
pub struct ErpCapture<'a> {
    path: PathBuf,
    env: &'a dyn Env,
    environment: String,
    home: Option<&'a dyn Destination>,
    extension: Option<&'a dyn ExtensionHello>,
}

const PREVIEW: &str = "pclm_capture_preview";
const PEEK: &str = "pclm_capture_peek";

impl<'a> ErpCapture<'a> {
    /// `new ErpCapture(databasePath)` — 업무 호스트(`production`), 쪽지를 보지 않는다.
    pub fn new(path: impl Into<PathBuf>, env: &'a dyn Env) -> Self {
        ErpCapture {
            path: path.into(),
            env,
            environment: "production".into(),
            home: None,
            extension: None,
        }
    }

    /// 이 호스트가 어느 쪽인가 — 개발 호스트면 `development`. 인사가 알린다.
    pub fn with_environment(mut self, environment: &str) -> Self {
        self.environment = environment.into();
        self
    }

    /// 저장할 때 쓰기 잠금 안에서 쪽지가 아직 이 파일을 가리키는지 다시 본다(ADR-032).
    pub fn with_destination(mut self, home: &'a dyn Destination) -> Self {
        self.home = Some(home);
        self
    }

    /// 업무 호스트의 인사가 디스크의 확장 판을 알리고 브라우저별 연결을 적는다.
    pub fn with_extension(mut self, extension: &'a dyn ExtensionHello) -> Self {
        self.extension = Some(extension);
        self
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// `ErpCapture.Open` — 쓰기 연결. 파일을 만들거나 판올림하지 않는다.
    pub(crate) fn open(&self) -> Result<Connection> {
        if !self.path.is_file() {
            return Err(Error::invalid(
                "메인 프로그램을 먼저 실행해 저장 대상을 준비하세요.",
            ));
        }
        let c = connect(&self.path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        c.execute_batch("PRAGMA foreign_keys=ON")?;
        let version: i64 = c.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version == i64::from(schema::VERSION) {
            return Ok(c);
        }
        Err(Error::invalid("메인 프로그램에서 DB 판올림을 완료하세요."))
    }

    /// `ErpCapture.Dispatch` — 확장의 요청 하나에 답한다. 답은 확장이 받는 꼴(Web, camelCase)이다. `files_allowed` 는
    /// 창의 고급 도구처럼 파일 입력·작업자료가 아닌 파일을 받는가.
    ///
    /// 자료 자리를 보거나 옮기는 요청(`dataLocation`·`stageDataMove`)은 받지 않는다(ADR-032).
    pub fn dispatch(&self, request: &Value, files_allowed: bool) -> Value {
        let mut request_id: Option<String> = None;
        match self.dispatch_inner(request, files_allowed, &mut request_id) {
            Ok(result) => reply(&request_id, true, "result", result),
            Err(e) => {
                let mut error = Map::new();
                error.insert("code".into(), Value::String("invalid_request".into()));
                error.insert("message".into(), Value::String(store::error_message(&e)));
                reply(&request_id, false, "error", Value::Object(error))
            }
        }
    }

    fn dispatch_inner(
        &self,
        request: &Value,
        files_allowed: bool,
        request_id: &mut Option<String>,
    ) -> Result<Value> {
        *request_id = element::string(element::get(request, "requestId")?)?.map(str::to_string);
        if !is_guid(request_id.as_deref())
            || get_int32(element::get(request, "protocolVersion")?)? != 2
        {
            return Err(Error::invalid(
                "앱과 확장을 함께 업데이트하세요. 프로토콜 2가 필요합니다.",
            ));
        }
        let c = self.open()?;
        let dataset_id = query_single_text(&c, "SELECT dataset_id FROM pclm_file")?;
        // 확장이 쓰는 것은 작업자료뿐이다. 제출본·취합본·백업은 그때의 기록이다.
        if !files_allowed && query_single_text(&c, "SELECT role FROM pclm_file")? != PclmRole::WORK
        {
            return Err(Error::invalid(
                "메인 프로그램에서 확장 저장 대상을 준비하세요.",
            ));
        }
        let method = element::string(element::get(request, "method")?)?;
        if method == Some("hello") {
            let mapping = mapping::active_in(&c)?;
            let mut extension_version = None;
            if let Some(extension) = self.extension {
                extension_version = extension.disk_version();
                // 옛 확장은 params 없이 인사한다. 적지 못해도 인사는 성공이다.
                if let Some(Value::Object(hello)) = request.get("params")
                    && let (Some(Value::String(version)), Some(Value::String(browser))) =
                        (hello.get("extensionVersion"), hello.get("browser"))
                {
                    extension.record_contact(version, browser);
                }
            }
            let mut out = Map::new();
            out.insert("datasetId".into(), Value::String(dataset_id));
            out.insert(
                "environment".into(),
                Value::String(self.environment.clone()),
            );
            out.insert(
                "destination".into(),
                Value::String(self.path.display().to_string()),
            );
            // 수집하는 화면은 mapping 의 프로필이 단 screen 이다(ADR-037). 확장은 그것으로만 화면을 가린다.
            out.insert("mapping".into(), mapping.to_value());
            out.insert(
                "mappingRevision".into(),
                Value::String(mapping::revision(&mapping)),
            );
            out.insert("maxRequestBytes".into(), MAX_REQUEST_BYTES.into());
            out.insert(
                "extensionVersion".into(),
                extension_version.map_or(Value::Null, Value::String),
            );
            return Ok(Value::Object(out));
        }
        let args = element::get(request, "params")?;
        if element::string(element::get(args, "datasetId")?)? != Some(dataset_id.as_str()) {
            return Err(Error::invalid("저장 대상이 바뀌었습니다. 다시 확인하세요."));
        }
        if method == Some("captureStatus") {
            let id = element::string(element::get(args, "captureId")?)?;
            let stored: Option<String> = c
                .query_row(
                    "SELECT result_json FROM erp_capture WHERE capture_id=?1",
                    [id],
                    |r| r.get(0),
                )
                .optional()?;
            return Ok(match stored {
                None => {
                    let mut out = Map::new();
                    out.insert("status".into(), Value::String("not_found".into()));
                    Value::Object(out)
                }
                Some(text) => parse_stored(&text)?,
            });
        }
        drop(c);
        let input = CaptureInput::from_json(element::get(args, "snapshot")?)?.ok_or_else(|| {
            Error::invalid("Object reference not set to an instance of an object.")
        })?;
        if !files_allowed && input.scope.as_deref() != Some("live") {
            return Err(Error::invalid(
                "확장은 실제 화면 수집만 저장할 수 있습니다.",
            ));
        }
        if method == Some("inspect") {
            let view = self.inspect(&input)?;
            let mut document = Map::new();
            document.insert(
                "rows".into(),
                Value::Array(
                    view.document
                        .rows
                        .iter()
                        .take(1)
                        .map(MappedRow::to_value)
                        .collect(),
                ),
            );
            let mut out = Map::new();
            out.insert("datasetId".into(), Value::String(view.dataset_id));
            out.insert("entity".into(), Value::String(view.entity));
            out.insert("entityType".into(), Value::String(view.entity_type));
            out.insert("baseToken".into(), Value::String(view.base_token));
            out.insert(
                "mappingRevision".into(),
                Value::String(view.mapping_revision),
            );
            out.insert("document".into(), Value::Object(document));
            out.insert("itemCount".into(), view.item_count.into());
            out.insert(
                "changes".into(),
                Value::Array(
                    view.changes
                        .iter()
                        .filter(|ch| ch.conflict)
                        .map(CaptureChange::to_value)
                        .collect(),
                ),
            );
            out.insert(
                "unmatched".into(),
                Value::Array(
                    view.unmatched
                        .iter()
                        .map(|r| {
                            let mut o = Map::new();
                            o.insert("id".into(), Value::String(r.id.clone()));
                            o.insert("table".into(), Value::String(r.table.clone()));
                            o.insert("sourceKey".into(), Value::String(r.source_key.clone()));
                            Value::Object(o)
                        })
                        .collect(),
                ),
            );
            return Ok(Value::Object(out));
        }
        if method == Some("capture") {
            let base_token = element::string(element::get(args, "baseToken")?)?.map(str::to_string);
            let capture_id = element::string(element::get(args, "captureId")?)?.map(str::to_string);
            let choices = match args.get("choices") {
                Some(choices) => choices_from_json(choices)?,
                None => Vec::new(),
            };
            return Ok(self
                .save(
                    &input,
                    base_token.as_deref(),
                    capture_id.as_deref(),
                    &choices,
                )?
                .to_value());
        }
        Err(Error::invalid("지원하지 않는 요청입니다."))
    }

    /// `ErpCapture.Inspect` — 미리보기. 읽기 사진 한 벌 안에서 견주고, 써 본 것은 되돌린다.
    pub fn inspect(&self, input: &CaptureInput) -> Result<CapturePreview> {
        let c = self.open()?;
        let tx = Transaction::new_unchecked(&c, TransactionBehavior::Deferred)?;
        inspect_in(&tx, self.env, input)
    }

    /// `ErpCapture.Save` — 쓰기 잠금 안에서 미리보기를 다시 하고, 지문과 고른 것이 맞을 때만 쓴다.
    pub fn save(
        &self,
        input: &CaptureInput,
        base_token: Option<&str>,
        capture_id: Option<&str>,
        choices: &Choices,
    ) -> Result<SaveResult> {
        if !is_guid(capture_id) {
            return Err(Error::invalid("올바른 수집 ID가 필요합니다."));
        }
        let capture_id = capture_id.unwrap_or_default();
        let c = self.open()?;
        let tx = Transaction::new_unchecked(&c, TransactionBehavior::Immediate)?;
        // 대상 확인을 쓰기 잠금 안에서 다시 한다. 창은 나가는 자료의 쓰기 잠금을 쥔 채 쪽지를 고치므로(ADR-032),
        // 잠금을 얻은 뒤 읽은 쪽지가 지금의 진실이다. 옮긴 뒤의 옛 파일은 역할이 retired 라 여기서 막힌다.
        let (dataset_id, role): (String, String) =
            tx.query_row("SELECT dataset_id, role FROM pclm_file", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?;
        if role != PclmRole::WORK
            || self
                .home
                .is_some_and(|home| !home.points_at(&self.path, &dataset_id))
        {
            return Err(Error::invalid("저장 대상이 바뀌었습니다. 다시 확인하세요."));
        }
        let mut request = Map::new();
        request.insert("input".into(), input.to_value());
        request.insert(
            "baseToken".into(),
            base_token.map_or(Value::Null, |t| Value::String(t.into())),
        );
        request.insert("choices".into(), choices_value(choices));
        let hash = mapping::hash(&Value::Object(request));
        let receipt: Option<(String, String)> = tx
            .query_row(
                "SELECT request_hash,result_json FROM erp_capture WHERE capture_id=?1",
                [capture_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((stored_hash, result)) = receipt {
            if stored_hash != hash {
                return Err(Error::invalid(
                    "같은 수집 ID에 다른 내용을 보낼 수 없습니다.",
                ));
            }
            return Ok(SaveResult::Replayed(parse_stored(&result)?));
        }
        let preview = inspect_in(&tx, self.env, input)?;
        let doc = &preview.document;
        if Some(preview.base_token.as_str()) != base_token {
            return Err(Error::invalid(
                "미리보기 이후 자료가 바뀌었습니다. 다시 확인하세요.",
            ));
        }
        if !preview.unmatched.is_empty() {
            return Err(Error::invalid(
                "기존 품목과 원천 행의 대응을 먼저 선택하세요.",
            ));
        }
        for change in preview.changes.iter().filter(|ch| ch.conflict) {
            if !matches!(choice_of(choices, &change.id), Some("keep" | "apply")) {
                return Err(Error::invalid("변경값과 사용자 정정값을 모두 검토하세요."));
            }
        }
        let now = self.env.now_utc();
        write(
            &tx,
            input,
            doc,
            &preview.changes,
            |ch| choice_of(choices, &ch.id),
            &now,
        )?;
        if !totals_match(&tx, doc)? {
            return Err(Error::invalid(
                "선택한 품목 금액과 계약 총액이 다릅니다. 품목 유지·삭제와 총액 선택을 함께 확인하세요.",
            ));
        }
        for o in preview
            .changes
            .iter()
            .filter(|ch| ch.field == "__override" && choice_of(choices, &ch.id) == Some("apply"))
        {
            tx.execute(
                "DELETE FROM field_override WHERE entity_type=?1 AND base=?2 AND seq=?3 AND column_name=?4",
                params![doc.entity_type, doc.base, doc.seq, o.override_column],
            )?;
        }
        let changed = preview
            .changes
            .iter()
            .any(|ch| choice_of(choices, &ch.id) != Some("keep"));
        let kind = &doc.entity_type;
        if changed {
            tx.execute(
                &format!("UPDATE {kind} SET updated_at=?1 WHERE {kind}_base=?2 AND seq=?3"),
                params![now, doc.base, doc.seq],
            )?;
        }
        let previous: Option<String> = tx
            .query_row(
                "SELECT document_json FROM erp_source WHERE entity_type=?1 AND entity_base=?2 AND entity_seq=?3",
                params![doc.entity_type, doc.base, doc.seq],
                |r| r.get(0),
            )
            .optional()?;
        let mut aggregate = match previous {
            None => Vec::new(),
            Some(text) => mapping::stored_rows(&text)?,
        };
        let retained: Vec<(String, String)> = tx
            .prepare(
                "SELECT table_name,source_key FROM erp_row WHERE entity_type=?1 AND entity_base=?2 AND entity_seq=?3",
            )?
            .query_map(params![doc.entity_type, doc.base, doc.seq], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?
            .collect::<rusqlite::Result<_>>()?;
        aggregate.retain(|r| {
            r.table == *kind
                || retained
                    .iter()
                    .any(|(t, k)| *t == r.table && *k == r.source_key)
        });
        for row in &doc.rows {
            match aggregate
                .iter()
                .position(|r| r.table == row.table && r.source_key == row.source_key)
            {
                None => aggregate.push(row.clone()),
                Some(old) => {
                    let mut values = aggregate[old].values.clone();
                    let mut origins: Pairs = aggregate[old].origins.clone().unwrap_or_default();
                    for (k, v) in &row.values {
                        set_pair(&mut values, k, v.clone());
                        set_pair(&mut origins, k, Some(doc.profile.clone()));
                    }
                    aggregate[old] = MappedRow {
                        values,
                        origins: Some(origins),
                        ..row.clone()
                    };
                }
            }
        }
        let aggregated = MappedDocument {
            rows: aggregate,
            ..doc.clone()
        };
        let document_json = json::to_indented(&aggregated.to_value(), Encoder::Default);
        tx.execute(
            "INSERT INTO erp_source(entity_type,entity_base,entity_seq,document_json,mapping_revision,updated_at) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(entity_type,entity_base,entity_seq) DO UPDATE SET document_json=excluded.document_json,mapping_revision=excluded.mapping_revision,updated_at=excluded.updated_at",
            params![doc.entity_type, doc.base, doc.seq, document_json, preview.mapping_revision, now],
        )?;
        let links: Vec<ReferenceState> = explicit_links::resolve(&tx)?
            .into_iter()
            .filter(|r| r.base == doc.base || r.target == doc.base)
            .collect();
        let active = mapping::active_in(&tx)?;
        let data = input.data.as_ref().ok_or_else(element::undefined)?;
        let sanitized = mapping::sanitize(&active, input.profile(), data)?;
        // screen: 읽은 화면의 번호 — 걸러 담은 data 에는 메뉴 경로가 남지 않아, 이력을 다시 검토할 때 이것을 싣는다.
        let mut snapshot = Map::new();
        snapshot.insert("profile".into(), text_value(&input.profile));
        snapshot.insert(
            "mappingRevision".into(),
            text_value(&input.mapping_revision),
        );
        snapshot.insert("data".into(), sanitized);
        snapshot.insert("scope".into(), text_value(&input.scope));
        snapshot.insert("totals".into(), int_pairs_value(&input.totals));
        snapshot.insert("rowMatches".into(), int_pairs_value(&input.row_matches));
        snapshot.insert("choices".into(), choices_value(choices));
        snapshot.insert(
            "screen".into(),
            screen_of(input).map_or(Value::Null, Value::String),
        );
        let snapshot_json = json::to_indented(&Value::Object(snapshot), Encoder::Default);
        let result = StoredCapture {
            capture_id: capture_id.to_string(),
            dataset_id: preview.dataset_id.clone(),
            entity: preview.entity.clone(),
            entity_type: doc.entity_type.clone(),
            changed,
            item_count: preview.item_count,
            scope: input.scope.clone(),
            links,
        };
        let result_json = json::to_compact(&result.to_value(true), Encoder::Default);
        tx.execute(
            "INSERT INTO erp_capture(capture_id,request_hash,content_hash,profile,entity_base,entity_seq,snapshot_json,result_json,captured_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                capture_id,
                hash,
                mapping::hash(&doc.to_value()),
                doc.profile,
                doc.base,
                doc.seq,
                snapshot_json,
                result_json,
                now
            ],
        )?;
        tx.commit()?;
        Ok(SaveResult::Stored(result))
    }
}

fn reply(request_id: &Option<String>, ok: bool, key: &str, value: Value) -> Value {
    let mut out = Map::new();
    out.insert("protocolVersion".into(), 2.into());
    out.insert("requestId".into(), text_value(request_id));
    out.insert("ok".into(), Value::Bool(ok));
    out.insert(key.into(), value);
    Value::Object(out)
}

/// `JsonElement.GetInt32()` — 수가 아니면 꼴 오류, `int` 로 나타낼 수 없으면 `FormatException`.
fn get_int32(value: &Value) -> Result<i32> {
    match value {
        Value::Number(n) => {
            let text = n.to_string();
            let body = text.strip_prefix('-').unwrap_or(&text);
            if body.bytes().all(|b| b.is_ascii_digit()) {
                text.parse().map_err(|_| {
                    Error::invalid("One of the identified items was in an invalid format.")
                })
            } else {
                Err(Error::invalid(
                    "One of the identified items was in an invalid format.",
                ))
            }
        }
        other => Err(element::wrong_type("Number", other)),
    }
}

/// Dapper `QuerySingle<string>` — 줄이 꼭 하나여야 한다.
fn query_single_text(c: &Connection, sql: &str) -> Result<String> {
    let mut statement = c.prepare(sql)?;
    let mut rows = statement.query([])?;
    let Some(first) = rows.next()? else {
        return Err(Error::invalid("Sequence contains no elements"));
    };
    let value = text_of(first.get_ref(0)?).unwrap_or_default();
    if rows.next()?.is_some() {
        return Err(Error::invalid("Sequence contains more than one element"));
    }
    Ok(value)
}

/// 적어 둔 영수증 글(`JsonSerializer.Deserialize<JsonElement>`).
fn parse_stored(text: &str) -> Result<Value> {
    json::parse(text).map_err(|_| Error::invalid("저장된 수집 영수증을 읽지 못했습니다."))
}

/// `ErpCapture.ScreenOf` — 그 입력을 읽은 화면의 메뉴 번호. 실제 화면은 확장이 실은 번호뿐이다. 파일은 실린 번호(수집
/// 이력의 재검토)가 있으면 그것을, 없으면 원본 JSON 의 메뉴 경로를 본다. 모르면 `None`.
pub fn screen_of(input: &CaptureInput) -> Option<String> {
    if input.scope.as_deref() == Some("live") || input.screen.is_some() {
        return input.screen.clone();
    }
    match &input.data {
        Some(Value::Object(data)) => data.get("pointInfo").and_then(screens::leaf),
        _ => None,
    }
}

/// `ErpCapture.RevisionView` — 덮개 아래 값을 읽는 뷰. 계약면 뷰는 최신 차수만 내므로 차수 뷰에서 읽는다. 접수는 차수
/// 뷰가 없다.
pub fn revision_view(entity_type: &str) -> &'static str {
    match entity_type {
        "notice" => "v_공고차수",
        "contract" => "v_계약차수",
        other => face_view(other),
    }
}

/// `ErpCapture.RowId`.
pub fn row_id(row: &MappedRow) -> String {
    format!("{}/{}", row.table, row.source_key)
}

/// `ErpCapture.ReadRows` — 그 차수의 그 표 줄들, 열 차례 그대로.
pub(crate) fn read_rows(
    c: &Connection,
    doc: &MappedDocument,
    table: &str,
) -> Result<Vec<StoredRow>> {
    let order = if table == doc.entity_type {
        "seq"
    } else {
        "line_no"
    };
    let mut statement = c.prepare(&format!(
        "SELECT * FROM {table} WHERE {}_base=?1 AND seq=?2 ORDER BY {order}",
        doc.entity_type
    ))?;
    let names: Vec<String> = statement.column_names().map(str::to_string).collect();
    let rows = statement
        .query_map(params![doc.base, doc.seq], |r| {
            names
                .iter()
                .enumerate()
                .map(|(i, name)| Ok((name.clone(), r.get::<_, Sql>(i)?)))
                .collect::<rusqlite::Result<StoredRow>>()
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// `ErpCapture.Inspect(connection, transaction, input)` — 열린 트랜잭션 안의 미리보기. 저장과 창의 투영이 같은 것을 쓴다.
pub(crate) fn inspect_in(
    c: &Connection,
    env: &dyn Env,
    input: &CaptureInput,
) -> Result<CapturePreview> {
    let mapping = mapping::active_in(c)?;
    let revision = mapping::revision(&mapping);
    if input.mapping_revision.as_deref() != Some(revision.as_str()) {
        return Err(Error::invalid("매핑이 바뀌었습니다. 다시 확인하세요."));
    }
    if !matches!(input.scope.as_deref(), Some("file" | "live")) {
        return Err(Error::invalid("수집 범위가 잘못되었습니다."));
    }
    // 받는 화면인가(ADR-037). 번호 칸을 읽기 전에 본다. 프로필을 모르면 아래 map 이 거절한다.
    if mapping.profile(input.profile()).is_some() {
        screens::check(
            &mapping,
            screen_of(input).as_deref(),
            input.profile(),
            input.scope.as_deref() == Some("live"),
        )?;
    }
    let data = input.data.as_ref().ok_or_else(element::undefined)?;
    let doc = mapping::map(
        &mapping,
        input.profile(),
        data,
        input.scope.as_deref().unwrap_or(""),
    )?;
    preview_of(c, env, input, &mapping, revision, doc)
}

fn preview_of(
    c: &Connection,
    env: &dyn Env,
    input: &CaptureInput,
    _mapping: &MappingSet,
    revision: String,
    doc: MappedDocument,
) -> Result<CapturePreview> {
    let mut changes: Vec<CaptureChange> = Vec::new();
    let mut unmatched: Vec<RowMatch> = Vec::new();
    let tracked: Vec<(String, String, i64)> = c
        .prepare(
            "SELECT table_name,source_key,line_no FROM erp_row WHERE entity_type=?1 AND entity_base=?2 AND entity_seq=?3",
        )?
        .query_map(params![doc.entity_type, doc.base, doc.seq], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })?
        .collect::<rusqlite::Result<_>>()?;
    let mut used: Vec<String> = Vec::new();
    let mut tables: Vec<&str> = Vec::new();
    for row in &doc.rows {
        if !tables.contains(&row.table.as_str()) {
            tables.push(&row.table);
        }
    }
    for table in &tables {
        let group: Vec<&MappedRow> = doc.rows.iter().filter(|r| r.table == *table).collect();
        let existing = read_rows(c, &doc, table)?;
        let mut next = existing.iter().map(line_of).max().unwrap_or(0);
        for row in &group {
            let id = row_id(row);
            let mut line = 0;
            if row.table != doc.entity_type {
                line = tracked
                    .iter()
                    .find(|(t, k, _)| *t == row.table && *k == row.source_key)
                    .map_or(0, |r| r.2);
                let selected = if line == 0 {
                    input.row_match(&id)
                } else {
                    None
                };
                if let Some(selected) = selected {
                    let selected = i64::from(selected);
                    if selected < 0
                        || (selected > 0 && !existing.iter().any(|r| line_of(r) == selected))
                    {
                        return Err(Error::invalid("대응 품목 행이 존재하지 않습니다."));
                    }
                    line = selected;
                } else if line == 0 && !existing.is_empty() {
                    unmatched.push(RowMatch {
                        id: id.clone(),
                        table: row.table.clone(),
                        source_key: row.source_key.clone(),
                        existing: existing.clone(),
                    });
                }
                if line == 0 {
                    next += 1;
                    line = next;
                }
                let slot = format!("{}:{line}", row.table);
                if used.contains(&slot) {
                    return Err(Error::invalid(
                        "여러 원천 행을 같은 품목 행으로 합칠 수 없습니다.",
                    ));
                }
                used.push(slot);
            }
            let before = existing.iter().find(|r| line == 0 || line_of(r) == line);
            for (field, value) in &row.values {
                let old = before
                    .and_then(|b| stored_value(b, field))
                    .and_then(|v| text_of(ValueRef::from(v)));
                // 덮개는 표시 열 이름을 쓰므로 헤더 전체를 별도로 확인시킨다.
                if old != *value {
                    changes.push(CaptureChange {
                        id: format!("{id}/{field}"),
                        table: row.table.clone(),
                        line,
                        field: field.clone(),
                        conflict: old.as_deref().is_some_and(|o| !o.is_empty()),
                        before: old,
                        after: value.clone(),
                        override_column: None,
                    });
                }
            }
        }
        // pdf: 로 시작하는 행 키는 걷힌 PDF 비교가 남긴 지난 관찰 기록이다(ADR-028). 지우자고 하지 않는다.
        if *table != doc.entity_type {
            for (t, _, line) in tracked.iter().filter(|(t, k, _)| {
                t == table && !k.starts_with("pdf:") && !group.iter().any(|r| r.source_key == *k)
            }) {
                let old = existing.iter().find(|r| line_of(r) == *line);
                if let Some(old) = old
                    && !used.contains(&format!("{t}:{line}"))
                {
                    changes.push(CaptureChange {
                        id: format!("remove/{t}/{line}"),
                        table: t.clone(),
                        line: *line,
                        field: "__row".into(),
                        before: Some(json::to_compact(&stored_row_value(old), Encoder::Default)),
                        after: None,
                        conflict: true,
                        override_column: None,
                    });
                }
            }
        }
    }
    let overrides: Vec<(String, String)> = c
        .prepare(
            "SELECT column_name,value FROM field_override WHERE entity_type=?1 AND base=?2 AND seq=?3",
        )?
        .query_map(params![doc.entity_type, doc.base, doc.seq], |r| {
            Ok((
                text_of(r.get_ref(0)?).unwrap_or_default(),
                text_of(r.get_ref(1)?).unwrap_or_default(),
            ))
        })?
        .collect::<rusqlite::Result<_>>()?;
    if !overrides.is_empty() {
        // 덮개마다 묻던 것을, 이번 수집이 덮개 아래 값을 바꿀 때만 묻는다. 같은 트랜잭션 안에서 평범한 변경을 모두
        // 적용하고 줄은 지우지 않은 모습을 써 보고, 덮개를 하나씩 걷어 표시 값을 읽은 뒤 되돌린다.
        let before = underlying(c, &doc, &overrides)?;
        c.execute_batch(&format!("SAVEPOINT {PREVIEW}"))?;
        let outcome = (|| -> Result<Vec<CaptureChange>> {
            let now = env.now_utc();
            let snapshot = changes.clone();
            write(
                c,
                input,
                &doc,
                &snapshot,
                |ch| Some(if ch.field == "__row" { "keep" } else { "apply" }),
                &now,
            )?;
            let after = underlying(c, &doc, &overrides)?;
            let mut asked = Vec::new();
            for (column, value) in &overrides {
                let a = after
                    .iter()
                    .find(|(k, _)| k == column)
                    .and_then(|(_, v)| v.clone());
                let b = before
                    .iter()
                    .find(|(k, _)| k == column)
                    .and_then(|(_, v)| v.clone());
                // 읽지 못한 열(null)은 판단할 수 없으니 예전처럼 묻는다.
                if a.is_none() || b != a {
                    asked.push(CaptureChange {
                        id: format!("override/{column}"),
                        table: doc.entity_type.clone(),
                        line: 0,
                        field: "__override".into(),
                        before: Some(value.clone()),
                        after: a,
                        conflict: true,
                        override_column: Some(column.clone()),
                    });
                }
            }
            Ok(asked)
        })();
        c.execute_batch(&format!("ROLLBACK TO {PREVIEW}; RELEASE {PREVIEW};"))?;
        changes.extend(outcome?);
    }
    let mut state_rows = Vec::new();
    for table in &tables {
        let mut o = Map::new();
        o.insert("table".into(), Value::String(table.to_string()));
        o.insert(
            "values".into(),
            Value::Array(
                read_rows(c, &doc, table)?
                    .iter()
                    .map(stored_row_value)
                    .collect(),
            ),
        );
        state_rows.push(Value::Object(o));
    }
    let mut state = Map::new();
    state.insert("mapping".into(), Value::String(revision.clone()));
    state.insert(
        "input".into(),
        Value::String(mapping::hash(&input.to_value())),
    );
    state.insert("rows".into(), Value::Array(state_rows));
    state.insert(
        "overrides".into(),
        Value::Array(
            overrides
                .iter()
                .map(|(column, value)| {
                    let mut o = Map::new();
                    o.insert("column".into(), Value::String(column.clone()));
                    o.insert("value".into(), Value::String(value.clone()));
                    Value::Object(o)
                })
                .collect(),
        ),
    );
    state.insert(
        "tracked".into(),
        Value::Array(
            tracked
                .iter()
                .map(|(t, k, l)| {
                    let mut o = Map::new();
                    o.insert("table".into(), Value::String(t.clone()));
                    o.insert("sourceKey".into(), Value::String(k.clone()));
                    o.insert("line".into(), (*l).into());
                    Value::Object(o)
                })
                .collect(),
        ),
    );
    let dataset_id = query_single_text(c, "SELECT dataset_id FROM pclm_file")?;
    let item_table = format!("{}_item", doc.entity_type);
    let item_count = doc.rows.iter().filter(|r| r.table == item_table).count() as i64;
    Ok(CapturePreview {
        dataset_id,
        entity: format!("{}-{}", doc.base, doc.seq),
        entity_type: doc.entity_type.clone(),
        base_token: mapping::hash(&Value::Object(state)),
        mapping_revision: revision,
        document: doc,
        changes,
        unmatched,
        item_count,
    })
}

/// `ErpCapture.Underlying` — 덮개마다 그 덮개 하나만 걷었을 때 계약면에 보이는 값. 읽지 못하면 `None`.
fn underlying(
    c: &Connection,
    doc: &MappedDocument,
    overrides: &[(String, String)],
) -> Result<Vec<(String, Option<String>)>> {
    let key = EntityRef::new(&doc.entity_type, &doc.base, &doc.seq).display();
    let mut values: Vec<(String, Option<String>)> = Vec::new();
    for (column, _) in overrides {
        c.execute_batch(&format!("SAVEPOINT {PEEK}"))?;
        let read = (|| -> Result<Option<String>> {
            c.execute(
                "DELETE FROM field_override WHERE entity_type=?1 AND base=?2 AND seq=?3 AND column_name=?4",
                params![doc.entity_type, doc.base, doc.seq, column],
            )?;
            // 줄이 없으면(최신 차수만 내는 접수 뷰의 옛 차수) 판단할 수 없어 None 으로 둔다.
            let sql = format!(
                "SELECT \"{}\" FROM \"{}\" WHERE \"{}\"=?1",
                column.replace('"', "\"\""),
                revision_view(&doc.entity_type),
                face_key(&doc.entity_type)
            );
            let mut statement = c.prepare(&sql)?;
            let mut rows = statement.query([&key])?;
            Ok(match rows.next()? {
                None => None,
                Some(row) => Some(text_of(row.get_ref(0)?).unwrap_or_default()),
            })
        })();
        let value = match read {
            Ok(v) => v,
            Err(Error::Sqlite(_)) => None,
            Err(e) => {
                c.execute_batch(&format!("ROLLBACK TO {PEEK}; RELEASE {PEEK};"))?;
                return Err(e);
            }
        };
        c.execute_batch(&format!("ROLLBACK TO {PEEK}; RELEASE {PEEK};"))?;
        match values.iter_mut().find(|(k, _)| k == column) {
            Some(slot) => slot.1 = value,
            None => values.push((column.clone(), value)),
        }
    }
    Ok(values)
}

/// `ErpCapture.Write` — 업무 표에 수집값을 쓴다. 저장과, 덮개 아래 값이 바뀌는지 미리 보는 미리보기가 함께 쓴다 — 두
/// 벌이면 미리 본 모습과 실제 저장이 갈린다. `field_override` 에는 쓰지 않는다(ADR-020).
pub(crate) fn write<'c>(
    c: &Connection,
    input: &CaptureInput,
    doc: &MappedDocument,
    changes: &'c [CaptureChange],
    choice: impl Fn(&'c CaptureChange) -> Option<&'c str>,
    now: &str,
) -> Result<()> {
    let kind = &doc.entity_type;
    if kind == "notice" {
        c.execute(
            "INSERT INTO notice_group(group_base) VALUES(?1) ON CONFLICT DO NOTHING",
            [&doc.base],
        )?;
        c.execute(
            "INSERT INTO notice_series(notice_base,group_base) VALUES(?1,?1) ON CONFLICT DO NOTHING",
            [&doc.base],
        )?;
    } else {
        c.execute(
            &format!("INSERT INTO {kind}_series({kind}_base) VALUES(?1) ON CONFLICT DO NOTHING"),
            [&doc.base],
        )?;
    }
    c.execute(
        &format!(
            "INSERT INTO {kind}({kind}_base,seq,updated_at) VALUES(?1,?2,?3) ON CONFLICT DO NOTHING"
        ),
        params![doc.base, doc.seq, now],
    )?;
    for row in &doc.rows {
        let id = row_id(row);
        let prefix = format!("{id}/");
        let mut line = changes
            .iter()
            .find(|ch| ch.id.starts_with(&prefix))
            .map_or(0, |ch| ch.line);
        if row.table != *kind && line == 0 {
            line = c
                .query_row(
                    "SELECT line_no FROM erp_row WHERE entity_type=?1 AND entity_base=?2 AND entity_seq=?3 AND table_name=?4 AND source_key=?5",
                    params![kind, doc.base, doc.seq, row.table, row.source_key],
                    |r| r.get::<_, Option<i64>>(0),
                )
                .optional()?
                .flatten()
                .unwrap_or(0);
        }
        if row.table != *kind {
            if line == 0 {
                line = i64::from(input.row_match(&id).unwrap_or(0));
            }
            if line == 0 {
                line = c.query_row(
                    &format!(
                        "SELECT coalesce(max(line_no),0)+1 FROM {} WHERE {kind}_base=?1 AND seq=?2",
                        row.table
                    ),
                    params![doc.base, doc.seq],
                    |r| r.get(0),
                )?;
            }
            c.execute(
                &format!(
                    "INSERT INTO {}({kind}_base,seq,line_no) VALUES(?1,?2,?3) ON CONFLICT DO NOTHING",
                    row.table
                ),
                params![doc.base, doc.seq, line],
            )?;
            c.execute(
                "INSERT INTO erp_row(entity_type,entity_base,entity_seq,table_name,source_key,line_no) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT DO NOTHING",
                params![kind, doc.base, doc.seq, row.table, row.source_key, line],
            )?;
        }
        for change in changes.iter().filter(|ch| {
            ch.table == row.table
                && ch.line == line
                && ch.field != "__override"
                && ch.field != "__row"
        }) {
            if choice(change) == Some("keep") {
                continue;
            }
            let sql = format!(
                "UPDATE {} SET {}=?1 WHERE {kind}_base=?2 AND seq=?3{}",
                row.table,
                change.field,
                if line == 0 { "" } else { " AND line_no=?4" }
            );
            if line == 0 {
                c.execute(&sql, params![change.after, doc.base, doc.seq])?;
            } else {
                c.execute(&sql, params![change.after, doc.base, doc.seq, line])?;
            }
        }
    }
    for removed in changes
        .iter()
        .filter(|ch| ch.field == "__row" && choice(ch) == Some("apply"))
    {
        remove_row(c, doc, removed)?;
    }
    Ok(())
}

/// `ErpCapture.RemoveRow` — 화면에 없는 행(`__row`)을 지운다 — 업무 표의 줄과 그 추적을 함께.
pub(crate) fn remove_row(
    c: &Connection,
    doc: &MappedDocument,
    removed: &CaptureChange,
) -> Result<()> {
    c.execute(
        &format!(
            "DELETE FROM {} WHERE {}_base=?1 AND seq=?2 AND line_no=?3",
            removed.table, doc.entity_type
        ),
        params![doc.base, doc.seq, removed.line],
    )?;
    c.execute(
        "DELETE FROM erp_row WHERE entity_type=?1 AND entity_base=?2 AND entity_seq=?3 AND table_name=?4 AND line_no=?5",
        params![doc.entity_type, doc.base, doc.seq, removed.table, removed.line],
    )?;
    Ok(())
}

/// `ErpCapture.TotalsMatch` — 계약의 머리 총액이 품목 금액의 합(10원 미만 버림)과 같은가. 계약이 아니면 늘 같다. 저장이
/// 이것으로 막고, 창의 투영이 같은 셈으로 미리 알린다.
pub(crate) fn totals_match(c: &Connection, doc: &MappedDocument) -> Result<bool> {
    use super::decimal::Decimal;
    if doc.entity_type != "contract" {
        return Ok(true);
    }
    let amounts: Vec<Option<String>> = c
        .prepare("SELECT amount FROM contract_item WHERE contract_base=?1 AND seq=?2")?
        .query_map(params![doc.base, doc.seq], |r| Ok(text_of(r.get_ref(0)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let mut total = Decimal::ZERO;
    for amount in &amounts {
        total = total.add(Decimal::parse(amount.as_deref().unwrap_or("0"))?)?;
    }
    let header: Option<String> = c
        .query_row(
            "SELECT amount FROM contract WHERE contract_base=?1 AND seq=?2",
            params![doc.base, doc.seq],
            |r| Ok(text_of(r.get_ref(0)?)),
        )
        .optional()?
        .flatten();
    Ok(Decimal::parse(header.as_deref().unwrap_or("0"))?.equals(total.truncate_tens()))
}

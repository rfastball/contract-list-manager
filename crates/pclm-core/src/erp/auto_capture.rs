//! 자동 수집의 판단(C# `Erp/AutoCapture.cs`) — 현재 화면의 조건을 확인하고 기존 검토·저장 경계를 쓴다. 탭 조작이나 DB
//! 값으로 조건을 보충하지 않는다.
//!
//! 설정 파일(`auto-capture.json`)을 읽고 쓰는 일은 홈의 것이라 여기 없다(#10) — [`AutoCapturePreferences::from_json`]·
//! [`AutoCapturePreferences::to_json`] 이 글만 다루고, 파일은 부르는 쪽이 다룬다.

use serde_json::{Map, Value};

use super::capture::{CaptureInput, Choices, Destination, ErpCapture};
use super::element;
use super::mapping::{self, MappingStore};
use super::mirror;
use super::screens;
use crate::env::Env;
use crate::error::{Error, Result};
use crate::json::{self, Encoder};
use crate::storage::database::Database;
use crate::text::{eq_ignore_case, utf16_len};

/// C# `AutoCapturePreferences` — 켜졌는가, 담당자 이름 조건.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AutoCapturePreferences {
    pub enabled: bool,
    pub name: String,
}

impl AutoCapturePreferences {
    /// `Mapping.Json` 으로 쓴 꼴.
    pub fn to_value(&self) -> Value {
        let mut o = Map::new();
        o.insert("enabled".into(), Value::Bool(self.enabled));
        o.insert("name".into(), Value::String(self.name.clone()));
        Value::Object(o)
    }

    /// `AutoCapturePreferences.Read` 의 글 쪽 — 읽지 못하거나, 비었거나, 이름이 200자를 넘으면 기본값(꺼짐). 모르는 칸은
    /// 건너뛴다.
    pub fn from_json(text: &str) -> AutoCapturePreferences {
        let parsed = (|| -> Option<AutoCapturePreferences> {
            let value = json::parse(text).ok()?;
            let map = json::read_object(&value).ok()??;
            let mut out = AutoCapturePreferences::default();
            for (key, v) in map {
                if eq_ignore_case(key, "enabled") {
                    out.enabled = match v {
                        Value::Bool(b) => *b,
                        _ => return None,
                    };
                } else if eq_ignore_case(key, "name") {
                    out.name = json::read_string(v).ok()??;
                }
            }
            (utf16_len(&out.name) <= 200).then_some(out)
        })();
        parsed.unwrap_or_default()
    }

    /// `AutoCapturePreferences.Write` 의 글 쪽 — 이름이 200자를 넘으면 거절한다.
    pub fn to_json(&self) -> Result<String> {
        if utf16_len(&self.name) > 200 {
            return Err(Error::invalid("자동 수집 설정을 확인하세요."));
        }
        Ok(json::to_indented(&self.to_value(), Encoder::Default))
    }
}

/// C# `AutoCaptureResult` — `state` 는 `hold`·`unchanged`·`stored`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AutoCaptureResult {
    pub state: String,
    pub message: String,
    pub entity: Option<String>,
}

impl AutoCaptureResult {
    fn new(state: &str, message: &str, entity: Option<String>) -> Self {
        AutoCaptureResult {
            state: state.into(),
            message: message.into(),
            entity,
        }
    }

    fn hold(message: &str) -> Self {
        AutoCaptureResult::new("hold", message, None)
    }

    pub fn to_value(&self) -> Value {
        let mut o = Map::new();
        o.insert("state".into(), Value::String(self.state.clone()));
        o.insert("message".into(), Value::String(self.message.clone()));
        o.insert(
            "entity".into(),
            self.entity.clone().map_or(Value::Null, Value::String),
        );
        Value::Object(o)
    }
}

struct Pending {
    input: CaptureInput,
    base_token: String,
    id: String,
    choices: Choices,
}

/// C# `AutoCapture` — 같은 읽기에 한 번만 저장한다. 응답이 불명확해도 처음 요청의 토큰·선택·ID 를 그대로 다시 쓴다.
pub struct AutoCapture<'a> {
    database: &'a Database,
    home: &'a dyn Destination,
    env: &'a dyn Env,
    attempted: Vec<(String, AutoCaptureResult)>,
    pending: Vec<(String, Pending)>,
    current_key: Option<String>,
}

fn text(value: &Value, name: &str) -> String {
    element::text_property(value, name)
        .unwrap_or("")
        .to_string()
}

/// `AutoCapture.At` — 점으로 이은 길의 글이나 수(원문).
fn at(mut value: &Value, path: &str) -> Option<String> {
    for key in path.split('.') {
        value = value.as_object()?.get(key)?;
    }
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// `Guid.NewGuid().ToString()` — 줄표 꼴(`D`). [`Env::new_id`] 는 `N` 꼴이다.
fn dashed(id: &str) -> String {
    if id.len() != 32 || !id.is_ascii() {
        return id.to_string();
    }
    format!(
        "{}-{}-{}-{}-{}",
        &id[..8],
        &id[8..12],
        &id[12..16],
        &id[16..20],
        &id[20..]
    )
}

impl<'a> AutoCapture<'a> {
    pub fn new(database: &'a Database, home: &'a dyn Destination, env: &'a dyn Env) -> Self {
        AutoCapture {
            database,
            home,
            env,
            attempted: Vec::new(),
            pending: Vec::new(),
            current_key: None,
        }
    }

    /// `AutoCapture.TrySave` — `now_ms` 는 지금의 유닉스 밀리초, `may_save` 는 앱이 저장해도 되는가(종료 중이면 거짓).
    /// 판단이 멈춘 까닭은 `hold` 로 돌려준다. C# 이 그 자리에서 잡지 않던 오류(매핑·스냅샷을 읽지 못함 따위)는 오류로 낸다.
    pub fn try_save(
        &mut self,
        tab: &Value,
        preferences: &AutoCapturePreferences,
        now_ms: i64,
        may_save: Option<&dyn Fn() -> bool>,
    ) -> Result<AutoCaptureResult> {
        let stopping = || may_save.is_some_and(|f| !f());
        let hold = |m: &str| Ok(AutoCaptureResult::hold(m));
        if stopping() {
            return hold("앱을 종료하고 있어 저장하지 않습니다.");
        }
        if !preferences.enabled {
            return hold("자동 수집이 꺼져 있습니다.");
        }
        let snapshot = match tab.as_object().and_then(|t| t.get("snapshot")) {
            Some(s @ Value::Object(_)) if text(tab, "state") == "supported" => s,
            _ => return hold("현재 화면을 다시 읽는 중입니다."),
        };
        let readiness = tab.get("readiness");
        let fresh = readiness.and_then(Value::as_object).is_some_and(|r| {
            r.get("stable") == Some(&Value::Bool(true))
                && match r.get("observedAt") {
                    Some(Value::Number(n)) => {
                        let raw = n.to_string();
                        let body = raw.strip_prefix('-').unwrap_or(&raw);
                        !body.is_empty()
                            && body.bytes().all(|b| b.is_ascii_digit())
                            && raw
                                .parse::<i64>()
                                .is_ok_and(|observed| (0..=8000).contains(&(now_ms - observed)))
                    }
                    _ => false,
                }
        });
        let readiness = readiness.unwrap_or(&Value::Null);
        if !fresh || text(readiness, "documentId").is_empty() {
            return hold("현재 화면의 새 읽기를 기다립니다.");
        }
        let mapping = MappingStore::new(self.database, self.env).active()?;
        let input = mirror::input(snapshot)?;
        if input.scope.as_deref() != Some("live")
            || Some(text(tab, "menu").as_str()) != input.screen.as_deref()
            || input.mapping_revision.as_deref() != Some(mapping::revision(&mapping).as_str())
        {
            return hold("화면 또는 수집 규칙이 바뀌어 다시 읽어야 합니다.");
        }
        let profile = screens::profile_for(&mapping, input.screen.as_deref());
        let Some(profile) = profile.filter(|p| p.id.as_deref() == input.profile.as_deref()) else {
            return hold("이 화면의 수집 규칙을 확인하지 못했습니다.");
        };
        let expected = preferences.name.trim();
        if !expected.is_empty() {
            // 수요기관 담당자는 업무 담당자와 다르므로 이름 조건에 쓰지 않는다.
            let staff = profile.fields_iter().find(|f| {
                matches!(f.target(), "staff" | "notice_officer")
                    && f.secondary_source.is_none()
                    && !f.source().starts_with("tables.")
            });
            let Some(staff) = staff else {
                return hold("현재 화면에서 담당자 칸을 확인하지 못했습니다.");
            };
            let data = input.data.as_ref().ok_or_else(element::undefined)?;
            let actual = element::try_get(data, "pointInfo")?.and_then(|p| at(p, staff.source()));
            if !actual.is_some_and(|a| a.contains(expected)) {
                return hold("현재 화면의 담당자에 입력한 이름이 포함되어 있지 않습니다.");
            }
        }
        let dataset: String = self.database.open_read_only()?.query_row(
            "SELECT dataset_id FROM pclm_file",
            [],
            |r| r.get(0),
        )?;
        if text(tab, "datasetId") != dataset || !self.home.points_at(self.database.path(), &dataset)
        {
            return hold("저장 대상이 바뀌어 다시 읽어야 합니다.");
        }
        let mut keyed = Map::new();
        keyed.insert("dataset".into(), Value::String(dataset));
        keyed.insert("input".into(), input.to_value());
        keyed.insert("preferences".into(), preferences.to_value());
        keyed.insert(
            "documentId".into(),
            Value::String(text(readiness, "documentId")),
        );
        let key = mapping::hash(&Value::Object(keyed));
        if self.current_key.as_deref() != Some(key.as_str()) {
            self.attempted.clear();
            self.pending.clear();
            self.current_key = Some(key.clone());
        }
        if let Some((_, prior)) = self.attempted.iter().find(|(k, _)| *k == key) {
            return Ok(prior.clone());
        }
        let result = match self.attempt(&key, &input, &stopping) {
            Ok(Attempt::Done(result)) => result,
            Ok(Attempt::Hold(result)) => return Ok(result),
            Err(Error::InvalidOperation(message)) => {
                self.pending.retain(|(k, _)| *k != key);
                AutoCaptureResult::hold(&message)
            }
            Err(e) => return Err(e),
        };
        self.attempted.push((key, result.clone()));
        Ok(result)
    }

    fn attempt(
        &mut self,
        key: &str,
        input: &CaptureInput,
        stopping: &dyn Fn() -> bool,
    ) -> Result<Attempt> {
        if !self.pending.iter().any(|(k, _)| k == key) {
            let capture = ErpCapture::new(self.database.path(), self.env);
            let (live, preview) = mirror::prepare(&capture, input)?;
            let choices = mirror::default_choices(&preview, &[]);
            let changes = preview.changes.iter().any(|ch| {
                choices
                    .iter()
                    .find(|(k, _)| *k == ch.id)
                    .map(|(_, v)| v.as_str())
                    != Some("keep")
            });
            if !changes {
                return Ok(Attempt::Done(AutoCaptureResult::new(
                    "unchanged",
                    "같은 내용이 이미 저장되어 있습니다.",
                    Some(preview.entity),
                )));
            }
            self.pending.push((
                key.to_string(),
                Pending {
                    input: live,
                    base_token: preview.base_token,
                    id: dashed(&self.env.new_id()),
                    choices,
                },
            ));
        }
        if stopping() {
            return Ok(Attempt::Hold(AutoCaptureResult::hold(
                "앱을 종료하고 있어 저장하지 않습니다.",
            )));
        }
        let (_, request) = self
            .pending
            .iter()
            .find(|(k, _)| k == key)
            .expect("방금 넣었다");
        let saved = ErpCapture::new(self.database.path(), self.env)
            .with_destination(self.home)
            .save(
                &request.input,
                Some(&request.base_token),
                Some(&request.id),
                &request.choices,
            )?
            .to_value();
        let entity = element::text_property(&saved, "entity").map(str::to_string);
        self.pending.retain(|(k, _)| k != key);
        Ok(Attempt::Done(AutoCaptureResult::new(
            "stored",
            "현재 읽은 자료를 자동으로 저장했습니다.",
            Some(entity.unwrap_or_default()),
        )))
    }
}

enum Attempt {
    /// 기억해 두는 결과.
    Done(AutoCaptureResult),
    /// 기억하지 않는 멈춤(종료 중).
    Hold(AutoCaptureResult),
}

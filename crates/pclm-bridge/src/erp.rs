//! 확장과 지금 보는 화면(C# `Bridge` 의 내장 확장 · 수집 기록 · ERP 도구 · 「지금 보는 화면」(ADR-036) · 자동 수집).
//!
//! 탭 보고와 명령은 홈의 `extension-live` 를 거친다(`pclm_host::relay`) — 확장 호스트가 쓰고 이 다리가 읽는다. 투영 ·
//! 가져오기 · 자동 저장의 뜻은 Core(`mirror` · `ErpCapture` · `AutoCapture`)가 쥔다.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use pclm_core::erp::auto_capture::{AutoCapture, AutoCapturePreferences, AutoCaptureResult};
use pclm_core::erp::capture::{CaptureInput, Choices, ErpCapture};
use pclm_core::erp::wire::{try_get_int32, try_get_int64};
use pclm_core::erp::{capture_log, mirror, tools};
use pclm_core::Env;
use pclm_home::{Home, auto_capture};
use pclm_host::{presence, relay, status};
use serde_json::Value;

use crate::{Bridge, core, is_guid, object, parse_int, path_text, text_or_null, wrong_type};

/// 「지금 보는 화면」 의 기억 — 탭마다 마지막 투영과, 결과를 받기 전의 가져오기.
#[derive(Default)]
pub(crate) struct MirrorState {
    /// 같은 스냅샷·같은 dataVersion 이면 다시 계산하지 않는다 — `(열쇠, 투영, 오류)`.
    shots: HashMap<String, (String, Value, Option<String>)>,
    /// 같은 수집 ID 로 다시 부르면 처음 고른 그대로 다시 보낸다(`ErpCapture.Save` 가 같은 ID 의 다른 내용을 거절한다).
    imports: HashMap<String, (CaptureInput, String, Choices)>,
}

/// 자동 수집의 몫 — C# `_autoGate` · `_autoResult` · `_autoStopped`. 시계 실마리와 요청이 함께 본다.
#[derive(Default)]
pub(crate) struct AutoShared {
    /// `_autoGate` 아래의 `_autoResult`.
    gate: Mutex<Option<AutoCaptureResult>>,
    stopped: AtomicBool,
    /// 멈출 때 기다림을 깨운다.
    wake: (Mutex<()>, Condvar),
}

impl AutoShared {
    pub(crate) fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        let _guard = self.wake.0.lock().unwrap_or_else(|e| e.into_inner());
        self.wake.1.notify_all();
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<AutoCaptureResult>> {
        self.gate.lock().unwrap_or_else(|e| e.into_inner())
    }
}

const GONE: &str =
    "이 탭이 닫혔거나 브라우저 연결이 끊겨 가져오지 않았습니다. 나라장터에서 화면을 다시 여세요.";
const STALE: &str = "화면이나 작업자료가 바뀌었습니다. 다시 읽습니다.";
const PRODUCTION_ONLY: &str =
    "내장 확장 연결은 업무 홈에서만 준비합니다. --home 이나 --erp-dev 없이 앱을 다시 실행하세요.";

impl Bridge<'_> {
    /// C# `PrepareExtension` — 업무 홈에서만. 확장을 업무 홈에 꺼내고, 두 브라우저의 등록부에 호스트 쪽지를 적는다.
    pub(crate) fn prepare_extension(&self) -> Result<Value, String> {
        self.require_production_extension()?;
        crate::window::prepare_extension(self.os).map(|p| p.to_json())
    }

    /// C# `ReadExtensionStatus` — 지금 붙은 브라우저와 오늘의 연결 기록을 함께 낸다.
    pub(crate) fn read_extension_status(&self) -> Result<Value, String> {
        self.require_production_extension()?;
        let state = status::read(
            self.home.directory(),
            &crate::bundled::embedded_version()?,
            self.now()?,
        );
        Ok(status_json(&state))
    }

    fn require_production_extension(&self) -> Result<(), String> {
        if self.is_default_home {
            Ok(())
        } else {
            Err(PRODUCTION_ONLY.into())
        }
    }

    pub(crate) fn captures(&self) -> Result<Value, String> {
        Ok(capture_log::read(&self.database, self.now()?, self.zone)
            .map_err(core)?
            .to_value())
    }

    pub(crate) fn erp_tools(&self, operation: &str, json: &str) -> Result<Value, String> {
        tools::invoke(&self.database, self.env, operation, json, Some(&self.home)).map_err(core)
    }

    /// C# `OpenExtensionSetup` — 그 브라우저에 확장이 붙어 있으면 확장에 맡기고(`openExtensions`), 아니면 — 맡겼는데 창이
    /// 서지 않았어도 — 새 창의 주소창에 적어 넣는다.
    pub(crate) fn open_extension_setup(&self, target: &str) -> Result<Value, String> {
        let browser = match target {
            "chrome" => Some("Chrome"),
            "edge" => Some("Edge"),
            _ => None,
        };
        if let Some(browser) = browser {
            for (pid, contact) in presence::live_processes(self.home.directory()) {
                if contact.browser == browser && self.open_through_extension(pid, target)? {
                    return Ok(Value::Null);
                }
            }
        }
        crate::window::open_extension_setup(self.os, target)
    }

    /// 확장에게 새 창을 열게 하고 그 창을 앞으로 가져온다. 새 창이 서지 않으면 거짓 — 보냈다는 것만으로 끝내지 않는다.
    fn open_through_extension(&self, pid: i32, target: &str) -> Result<bool, String> {
        let process = crate::window::process_name(target)?;
        let before = self.os.browser_windows(process);
        let sent = relay::send(self.home.directory(), pid, "openExtensions", None)
            .map_err(|e| core(e.into()))?;
        Ok(sent && self.os.raise_new_window(process, &before, Duration::from_secs(3)))
    }

    /// C# `ReadMirror` — 나라장터 탭들과, 수집할 수 있는 탭마다 작업자료에 비춘 것. 열람 창은 읽지 않는다.
    pub(crate) fn read_mirror(&self) -> Result<Value, String> {
        if self.viewing.is_some() {
            return Ok(object([
                ("readOnly", Value::Bool(true)),
                ("reporting", Value::Bool(false)),
                ("front", Value::Null),
                ("tabs", Value::Array(Vec::new())),
                ("settings", Value::Null),
                ("screens", Value::Array(Vec::new())),
            ]));
        }
        let reports = relay::read_tabs(self.home.directory());
        let version = self.data_version()?;
        let mut front: Option<String> = None;
        let mut front_at = i64::MIN;
        // 확장 설정을 바꿀 브라우저 — 앞 탭의 브라우저, 없으면 처음 붙은 것.
        let mut owner: Option<&relay::TabReport> = None;
        let mut tabs: Vec<(String, i32, i32, String, &Value)> = Vec::new();
        for report in &reports {
            let Value::Object(root) = &report.report else {
                continue;
            };
            // 브라우저가 둘이면 가장 늦게 앞 탭을 바꾼 쪽이 「앞」 이다.
            if let Some(tab) = root.get("front").and_then(try_get_int32)
                && let Some(when) = root.get("frontAt").and_then(try_get_int64)
                && when >= front_at
            {
                front = Some(format!("{}:{tab}", report.pid));
                front_at = when;
                owner = Some(report);
            }
            let Some(Value::Array(list)) = root.get("tabs") else {
                continue;
            };
            for tab in list {
                if let Value::Object(fields) = tab
                    && let Some(tab_id) = fields.get("tabId").and_then(try_get_int32)
                {
                    tabs.push((
                        format!("{}:{tab_id}", report.pid),
                        report.pid,
                        tab_id,
                        report.browser.clone(),
                        tab,
                    ));
                }
            }
        }

        let mut state = self.mirror.lock().unwrap_or_else(|e| e.into_inner());
        let mut next = HashMap::new();
        let mut shown = Vec::new();
        for (id, pid, tab_id, browser, tab) in &tabs {
            let mut shot = Value::Null;
            let mut error: Option<String> = None;
            let tab_state = text(tab, "state");
            // 다시 읽는 동안에도 앞서 읽은 화면을 그대로 비춘다 — 읽는 중이라고 몸이 비지 않게.
            if matches!(tab_state.as_str(), "supported" | "reading")
                && let Some(snapshot @ Value::Object(_)) = tab.get("snapshot")
            {
                let key = format!(
                    "{version}:{}:{}",
                    snapshot,
                    tab.get("screenRows").map(Value::to_string).unwrap_or_default()
                );
                match state.shots.get(id) {
                    Some((cached, s, e)) if *cached == key => {
                        shot = s.clone();
                        error = e.clone();
                    }
                    _ => match self.project(snapshot, tab) {
                        Ok(value) => shot = value,
                        Err(message) => error = Some(message),
                    },
                }
                next.insert(id.clone(), (key, shot.clone(), error.clone()));
            }
            shown.push(object([
                ("id", Value::String(id.clone())),
                ("pid", Value::from(*pid)),
                ("tabId", Value::from(*tab_id)),
                ("browser", Value::String(browser.clone())),
                ("title", Value::String(text(tab, "title"))),
                ("screen", Value::String(text(tab, "screen"))),
                ("menu", Value::String(text(tab, "menu"))),
                ("state", Value::String(tab_state)),
                ("readAt", Value::String(text(tab, "readAt"))),
                ("message", Value::String(text(tab, "message"))),
                ("front", Value::Bool(front.as_deref() == Some(id.as_str()))),
                ("shot", shot),
                ("error", text_or_null(error)),
                (
                    "screenRows",
                    Value::Array(
                        mirror::screen_rows(tab)
                            .iter()
                            .map(mirror::ScreenRow::to_value)
                            .collect(),
                    ),
                ),
                (
                    "export",
                    mirror::export(tab).map_or(Value::Null, |e| e.to_value()),
                ),
            ]));
        }
        state.shots = next;
        drop(state);

        // OrderBy(ConnectedAt, Ordinal).FirstOrDefault() — 같으면 앞의 것.
        let owner = owner.or_else(|| {
            reports.iter().fold(None, |best: Option<&relay::TabReport>, r| match best {
                Some(b) if !r.connected_at.encode_utf16().lt(b.connected_at.encode_utf16()) => {
                    Some(b)
                }
                _ => Some(r),
            })
        });
        let settings = match owner {
            Some(o) => match o.report.get("settings") {
                Some(Value::Object(given)) if o.report.is_object() => object([
                    ("pid", Value::from(o.pid)),
                    ("browser", Value::String(o.browser.clone())),
                    (
                        "siteAccess",
                        Value::Bool(given.get("siteAccess") == Some(&Value::Bool(true))),
                    ),
                ]),
                _ => Value::Null,
            },
            None => Value::Null,
        };
        Ok(object([
            ("readOnly", Value::Bool(false)),
            ("reporting", Value::Bool(!reports.is_empty())),
            ("front", text_or_null(front)),
            ("tabs", Value::Array(shown)),
            ("settings", settings),
            (
                "screens",
                Value::Array(
                    mirror::supported_screens(&self.database, self.env)
                        .iter()
                        .map(mirror::MirrorScreen::to_value)
                        .collect(),
                ),
            ),
        ]))
    }

    /// `Mirror.Project(database, Mirror.Input(snapshot), Mirror.ScreenRows(tab))` — 읽지 못한 것은 그 탭의 오류로 둔다.
    fn project(&self, snapshot: &Value, tab: &Value) -> Result<Value, String> {
        let input = mirror::input(snapshot).map_err(core)?;
        let capture = ErpCapture::new(self.database.path(), self.env);
        mirror::project(
            &capture,
            self.env,
            self.zone,
            &input,
            &mirror::screen_rows(tab),
        )
        .map(|s| s.to_value())
        .map_err(core)
    }

    /// C# `MirrorCommand` — 그 탭을 앞으로(`focusTab`) · 다시 읽기(`read`) · 현재 화면 엑셀로 내보내기(`export`).
    pub(crate) fn mirror_command(
        &self,
        pid: &str,
        tab_id: &str,
        command: &str,
    ) -> Result<Value, String> {
        let pid = parse_int(pid)?;
        let tab_id = parse_int(tab_id)?;
        let sent = relay::send(self.home.directory(), pid, command, Some(tab_id))
            .map_err(|e| core(e.into()))?;
        if !sent {
            return Err("브라우저와의 연결이 끊겨 그 탭에 닿지 못했습니다.".into());
        }
        Ok(object([("sent", Value::Bool(true))]))
    }

    /// C# `ImportShot` — 창에서 가져오기. 수집의 저장과 **같은 절차**를 탄다(ADR-032·036). baseToken 이 낡았으면 쓰지 않고
    /// 그 탭을 다시 읽게 한다.
    pub(crate) fn import_shot(
        &self,
        pid: &str,
        tab: &str,
        base_token: &str,
        picks: &str,
        capture_id: &str,
    ) -> Result<Value, String> {
        if !is_guid(capture_id) {
            return Err("올바른 수집 ID가 필요합니다.".into());
        }
        let pid = parse_int(pid)?;
        let tab_id = parse_int(tab)?;
        let request = {
            let mut state = self.mirror.lock().unwrap_or_else(|e| e.into_inner());
            match state.imports.get(capture_id) {
                Some(request) => request.clone(),
                None => {
                    let reports = relay::read_tabs(self.home.directory());
                    let snapshot = reports
                        .iter()
                        .filter(|r| r.pid == pid)
                        .filter_map(|r| match r.report.get("tabs") {
                            Some(Value::Array(list)) if r.report.is_object() => Some(list),
                            _ => None,
                        })
                        .flatten()
                        .find_map(|t| match t {
                            Value::Object(fields)
                                if fields.get("tabId").and_then(try_get_int32) == Some(tab_id) =>
                            {
                                match fields.get("snapshot") {
                                    Some(s @ Value::Object(_)) => Some(s.clone()),
                                    _ => None,
                                }
                            }
                            _ => None,
                        });
                    // 보고에 그 탭의 화면이 없다 — 닫혔거나 브라우저가 끊겼다. 다시 읽으라고 보내지도 않는다.
                    let Some(snapshot) = snapshot else {
                        return Ok(object([
                            ("status", Value::String("gone".into())),
                            ("message", Value::String(GONE.into())),
                        ]));
                    };
                    let capture = ErpCapture::new(self.database.path(), self.env);
                    let input = mirror::input(&snapshot).map_err(core)?;
                    let (input, preview) = mirror::prepare(&capture, &input).map_err(core)?;
                    if preview.base_token != base_token {
                        return Ok(self.stale(pid, tab_id));
                    }
                    let picked = crate::edit::string_array(picks)
                        .map_err(|()| "고른 자리 형식이 올바르지 않습니다.".to_string())?
                        .unwrap_or_default();
                    let request = (
                        input,
                        base_token.to_string(),
                        mirror::default_choices(&preview, &picked),
                    );
                    state.imports.insert(capture_id.to_string(), request.clone());
                    request
                }
            }
        };
        let (input, token, choices) = &request;
        let saved = ErpCapture::new(self.database.path(), self.env)
            .with_destination(&self.home)
            .save(input, Some(token), Some(capture_id), choices);
        let forget = || {
            self.mirror
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .imports
                .remove(capture_id);
        };
        let stored = match saved {
            Ok(result) => result.to_value(),
            // 거절은 끝난 답이다 — 같은 ID 로 다시 보낼 것이 없다. 그 밖의 실패(디스크·잠금)는 남겨 같은 ID 로 다시 보낸다.
            Err(e @ (pclm_core::Error::InvalidOperation(_) | pclm_core::Error::PclmFile(_))) => {
                forget();
                let message = core(e);
                if message.starts_with("미리보기 이후") {
                    return Ok(self.stale(pid, tab_id));
                }
                return Err(message);
            }
            Err(other) => return Err(core(other)),
        };
        forget();
        // 그 탭을 다시 읽혀 지금 보는 화면이 저장된 뒤의 상태로 비추게 한다. 닿지 못해도 저장은 끝났다.
        let _ = relay::send(self.home.directory(), pid, "read", Some(tab_id));
        let now = self.now()?;
        Ok(object([
            ("status", Value::String("stored".into())),
            (
                "entity",
                text_or_null(stored.get("entity").and_then(Value::as_str).map(String::from)),
            ),
            (
                "changed",
                Value::Bool(stored.get("changed") == Some(&Value::Bool(true))),
            ),
            (
                "at",
                Value::String(format!("{:02}:{:02}", now.hour, now.minute)),
            ),
        ]))
    }

    fn stale(&self, pid: i32, tab_id: i32) -> Value {
        let _ = relay::send(self.home.directory(), pid, "read", Some(tab_id));
        object([
            ("status", Value::String("stale".into())),
            ("message", Value::String(STALE.into())),
        ])
    }

    /// C# `ReadAutoCaptureSettings` — `{enabled, name, lastResult}`. 자동 수집이 한 번 도는 동안은 기다린다.
    pub(crate) fn read_auto_capture_settings(&self) -> Value {
        let result = self.auto.lock();
        auto_settings_json(&auto_capture::read(&self.home), result.as_ref())
    }

    /// C# `SaveAutoCaptureSettings` — `Mapping.Json` 으로 읽는다(이름은 대소문자를 가리지 않고, 모르는 칸은 거절한다).
    pub(crate) fn save_auto_capture_settings(&self, json: &str) -> Result<Value, String> {
        let mut result = self.auto.lock();
        let preferences = read_preferences(json)?
            .ok_or_else(|| "자동 수집 설정이 비었습니다.".to_string())?;
        auto_capture::write(&self.home, &preferences).map_err(core)?;
        *result = None;
        Ok(auto_settings_json(&auto_capture::read(&self.home), None))
    }
}

impl Bridge<'_> {
    /// 자동 수집을 한 번 돌린다 — 타이머 없이(C# 의 private `AutoCaptureTick`). 대조 계기와 시험이 쓴다. `automatic` 은 부르는
    /// 쪽이 쥐고 거듭 넘긴다 — 같은 화면을 두 번 읽어야 저장하는 기다림이 그 안에 있다. 창은 [`Bridge::start_auto_capture`] 를 쓴다.
    pub fn tick_auto_capture(&self, automatic: &mut AutoCapture<'_>) {
        if self.read_only() {
            return;
        }
        tick(&self.auto, automatic, &self.home, self.env);
    }
}

impl Bridge<'static> {
    /// C# `StartAutoCapture` — 창을 세울 때 부른다. 1초마다 앞 탭을 보고, 켜져 있으면 저장한다. 열람 창은 하지 않는다.
    /// 다리를 놓을 때 멈추고 기다린다.
    pub fn start_auto_capture(&self) {
        if self.read_only() {
            return;
        }
        let mut thread = self.auto_thread.lock().unwrap_or_else(|e| e.into_inner());
        if self.auto.stopped.load(Ordering::SeqCst) || thread.is_some() {
            return;
        }
        let shared = Arc::clone(&self.auto);
        let database = self.database.clone();
        let home = self.home.clone();
        let env = self.env;
        *thread = Some(std::thread::spawn(move || {
            let mut automatic = AutoCapture::new(&database, &home, env);
            loop {
                {
                    let guard = shared.wake.0.lock().unwrap_or_else(|e| e.into_inner());
                    let _ = shared
                        .wake
                        .1
                        .wait_timeout_while(guard, Duration::from_secs(1), |()| {
                            !shared.stopped.load(Ordering::SeqCst)
                        });
                }
                if shared.stopped.load(Ordering::SeqCst) {
                    return;
                }
                tick(&shared, &mut automatic, &home, env);
            }
        }));
    }
}

/// C# `AutoCaptureTick` — 앞서 돈 것이 아직 쥐고 있으면 건너뛴다. 실패는 `hold` 로 남긴다.
fn tick(shared: &AutoShared, automatic: &mut AutoCapture<'_>, home: &Home, env: &(dyn Env + Sync)) {
    let Ok(mut result) = shared.gate.try_lock() else {
        return;
    };
    if shared.stopped.load(Ordering::SeqCst) {
        return;
    }
    match tick_once(shared, automatic, home, env) {
        Ok(Tick::Disabled) => *result = None,
        Ok(Tick::Nothing) => {}
        Ok(Tick::Saved(outcome)) => *result = Some(outcome),
        Err(message) => {
            *result = Some(AutoCaptureResult {
                state: "hold".into(),
                message,
                entity: None,
            })
        }
    }
}

enum Tick {
    Disabled,
    Nothing,
    Saved(AutoCaptureResult),
}

fn tick_once(
    shared: &AutoShared,
    automatic: &mut AutoCapture<'_>,
    home: &Home,
    env: &(dyn Env + Sync),
) -> Result<Tick, String> {
    let preferences = auto_capture::read(home);
    if !preferences.enabled {
        return Ok(Tick::Disabled);
    }
    // OrderByDescending(frontAt).FirstOrDefault() — 같으면 앞의 것. 수가 아닌 frontAt 은 .NET 이 던진다.
    let mut best: Option<(i64, relay::TabReport)> = None;
    for report in relay::read_tabs(home.directory()) {
        let at = match &report.report {
            Value::Object(root) => match root.get("frontAt") {
                None => 0,
                Some(v @ Value::Number(_)) => try_get_int64(v).unwrap_or(0),
                Some(other) => return Err(wrong_type("Number", other)),
            },
            other => return Err(wrong_type("Object", other)),
        };
        if best.as_ref().is_none_or(|(b, _)| at > *b) {
            best = Some((at, report));
        }
    }
    let Some((_, report)) = best else {
        return Ok(Tick::Nothing);
    };
    let root = report.report.as_object().cloned().unwrap_or_default();
    let Some(id) = root.get("front").and_then(try_get_int32) else {
        return Ok(Tick::Nothing);
    };
    let Some(tabs) = root.get("tabs") else {
        return Ok(Tick::Nothing);
    };
    let Value::Array(list) = tabs else {
        return Err(wrong_type("Array", tabs));
    };
    let mut found = None;
    for t in list {
        let Value::Object(fields) = t else {
            return Err(wrong_type("Object", t));
        };
        if let Some(v) = fields.get("tabId") {
            if !v.is_number() {
                return Err(wrong_type("Number", v));
            }
            if try_get_int32(v) == Some(id) {
                found = Some(t);
                break;
            }
        }
    }
    let Some(tab) = found else {
        return Ok(Tick::Nothing);
    };
    let now = pclm_core::local_time::parse_to_local(
        Some(&env.now_utc()),
        &pclm_core::local_time::FixedZone(0),
    )
    .map(|t| t.unix_seconds() * 1000 + i64::from(t.ticks / 10_000))
    .unwrap_or(0);
    let may_save = || !shared.stopped.load(Ordering::SeqCst);
    automatic
        .try_save(tab, &preferences, now, Some(&may_save))
        .map(Tick::Saved)
        .map_err(core)
}

fn auto_settings_json(preferences: &AutoCapturePreferences, last: Option<&AutoCaptureResult>) -> Value {
    object([
        ("enabled", Value::Bool(preferences.enabled)),
        ("name", Value::String(preferences.name.clone())),
        ("lastResult", last.map_or(Value::Null, AutoCaptureResult::to_value)),
    ])
}

/// `JsonSerializer.Deserialize<AutoCapturePreferences>(json, Mapping.Json)` — 이름은 대소문자를 가리지 않고 모르는 칸은 건너뛴다
/// (`[JsonUnmappedMemberHandling(Skip)]`). 칸은 원문 차례로 읽어 같은 이름이면 뒤의 것이 이긴다. 꼴이 다른 값은 .NET 의 글로
/// 거절한다 — 생성자로 짓는 기록이라 칸의 꼴이 틀려도 그 기록의 이름이 든다. JSON 의 null 이면 `None`. 이름이 null 이면
/// C# 은 읽기를 지나 `Write` 에서 「자동 수집 설정을 확인하세요.」 로 거절한다.
fn read_preferences(json: &str) -> Result<Option<AutoCapturePreferences>, String> {
    use crate::dotnet_json::{member_not_converted, members, not_converted, value_text};
    const TYPE: &str = "Pclm.Core.Erp.AutoCapturePreferences";
    let value: Value = serde_json::from_str(json).map_err(|e| crate::malformed(e.to_string()))?;
    match value {
        Value::Null => return Ok(None),
        Value::Object(_) => {}
        _ => return Err(not_converted(json, &[], TYPE)),
    }
    let mut out = AutoCapturePreferences::default();
    let mut name_missing = false;
    for (key, start) in members(json).unwrap_or_default() {
        let item: Value = serde_json::from_str(value_text(json, start))
            .map_err(|e| crate::malformed(e.to_string()))?;
        let wrong = || member_not_converted(json, start, &format!("$.{key}"), TYPE);
        if pclm_home::dotnet::eq_ignore_case(&key, "enabled") {
            out.enabled = match item {
                Value::Bool(b) => b,
                _ => return Err(wrong()),
            };
        } else if pclm_home::dotnet::eq_ignore_case(&key, "name") {
            match item {
                Value::String(s) => {
                    out.name = s;
                    name_missing = false;
                }
                Value::Null => name_missing = true,
                _ => return Err(wrong()),
            }
        }
    }
    if name_missing {
        return Err("자동 수집 설정을 확인하세요.".into());
    }
    Ok(Some(out))
}

/// `Text(e, name)` — 글이면 그 글, 아니면 빈 글.
fn text(tab: &Value, name: &str) -> String {
    match tab.get(name) {
        Some(Value::String(s)) => s.clone(),
        _ => String::new(),
    }
}

/// C# `ExtensionStatus.State` 를 다리가 쓴 꼴(camelCase).
fn status_json(state: &status::State) -> Value {
    let problem = |p: &pclm_host::log::Problem| {
        object([
            ("at", Value::String(p.at.clone())),
            ("browser", Value::String(p.browser.clone())),
            ("code", Value::String(p.code.clone())),
            ("recovered", Value::Bool(p.recovered)),
        ])
    };
    object([
        ("prepared", Value::Bool(state.prepared)),
        ("embeddedVersion", Value::String(state.embedded_version.clone())),
        ("diskVersion", text_or_null(state.disk_version.clone())),
        (
            "contacts",
            Value::Array(
                state
                    .contacts
                    .iter()
                    .map(|c| {
                        object([
                            ("browser", Value::String(c.browser.clone())),
                            ("version", Value::String(c.version.clone())),
                            ("at", Value::String(c.at.clone())),
                        ])
                    })
                    .collect(),
            ),
        ),
        ("folder", Value::String(path_text(&state.folder))),
        (
            "live",
            Value::Array(
                state
                    .live
                    .iter()
                    .map(|c| {
                        object([
                            ("browser", Value::String(c.browser.clone())),
                            ("version", Value::String(c.version.clone())),
                            ("connectedAt", Value::String(c.connected_at.clone())),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "log",
            Value::Array(
                state
                    .log
                    .iter()
                    .map(|e| {
                        object([
                            ("at", Value::String(e.at.clone())),
                            ("event", Value::String(e.event.clone())),
                            ("browser", Value::String(e.browser.clone())),
                            ("version", Value::String(e.version.clone())),
                            ("previous", Value::String(e.previous.clone())),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "errors",
            Value::Array(state.errors.iter().map(problem).collect()),
        ),
        (
            "pastError",
            state.past_error.as_ref().map_or(Value::Null, problem),
        ),
    ])
}

/// 대조 계기가 업무 홈의 문지기 없이 상태 읽기를 견주는 자리 — 이 홈(`directory`)을 그대로 읽는다.
pub fn extension_status_at(directory: &Path, now: pclm_core::local_time::LocalDateTime) -> Result<Value, String> {
    Ok(status_json(&status::read(
        directory,
        &crate::bundled::embedded_version()?,
        now,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 자동_수집_설정은_모르는_칸을_건너뛰고_이름은_대소문자를_가리지_않는다() {
        let read = read_preferences(r#"{"Enabled":true,"NAME":"홍길동"}"#).unwrap().unwrap();
        assert!(read.enabled);
        assert_eq!(read.name, "홍길동");
        assert_eq!(read_preferences("null").unwrap(), None);
        let skipped = read_preferences(r#"{"enabled":true,"extra":[1,{"a":2}]}"#).unwrap().unwrap();
        assert!(skipped.enabled);
        // 같은 이름은 원문 차례로 — 뒤의 것이 이긴다.
        let last = read_preferences(r#"{"enabled":true,"ENABLED":false}"#).unwrap().unwrap();
        assert!(!last.enabled);
        // .NET 의 글(jsonprobe 로 본 것) — 생성자로 짓는 기록이라 칸이 틀려도 기록의 이름이 든다.
        assert_eq!(
            read_preferences(r#"{"Enabled":"yes"}"#).unwrap_err(),
            "The JSON value could not be converted to Pclm.Core.Erp.AutoCapturePreferences. Path: $.Enabled | LineNumber: 0 | BytePositionInLine: 16."
        );
        assert_eq!(
            read_preferences(r#""x""#).unwrap_err(),
            "The JSON value could not be converted to Pclm.Core.Erp.AutoCapturePreferences. Path: $ | LineNumber: 0 | BytePositionInLine: 3."
        );
        assert_eq!(read_preferences(r#"{"name":null}"#).unwrap_err(), "자동 수집 설정을 확인하세요.");
    }
}

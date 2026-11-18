//! 상시 연결(ADR-035). 확장이 `connectNative` 포트를 쥐고 있는 동안 브라우저가 띄운
//! 호스트는 살아 있고, 그동안 홈의 `extension-live/<pid>.json` 이 그 호스트를 알린다. 붙고 끊길 때마다 연결 기록에 한 줄씩.
//!
//! 포트를 쥔 호스트는 브라우저가 닫힐 때까지 산다. 그래서 **인사(presence)·탭 보고·박동은 작업자료를 열지 않고** 홈
//! 폴더에 파일만 쓴다(ADR-032) — 작업자료를 여는 것은 그 밖의 요청을 받는 `dispatch` 뿐이다. 그리고 그동안 exe 를
//! 붙든다: 제 실행 파일이 비켜지거나 브라우저 등록이 다른 exe 로 옮겨 가면 포트를 닫고 떠난다.

use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use pclm_core::erp::wire;
use pclm_core::local_time::{FixedZone, parse_to_local};
use pclm_home::{paths, win};
use serde_json::{Map, Value};

use crate::executable::ExecutableIdentity;
use crate::native_messaging::{self, envelope, request_id, status_reply};
use crate::status::{self, LiveContact};
use crate::{log, relay};

pub const METHOD: &str = "presence";
const UPDATE_STATUS_METHOD: &str = "extensionUpdateStatus";
const UPDATE_CHECK_METHOD: &str = "extensionUpdateCheck";

/// 제 실행 파일을 다시 보는 간격.
pub const WATCH_INTERVAL: Duration = Duration::from_secs(3);

pub fn live_directory(directory: &Path) -> PathBuf {
    directory.join("extension-live")
}

pub fn live_path(directory: &Path, pid: i32) -> PathBuf {
    live_directory(directory).join(format!("{pid}.json"))
}

pub fn is_presence(request: &Value) -> bool {
    is_method(request, METHOD)
}

fn is_method(request: &Value, name: &str) -> bool {
    request.get("method").and_then(Value::as_str) == Some(name)
}

/// 이 프로세스의 번호.
pub fn process_id() -> i32 {
    std::process::id() as i32
}

/// [`serve`] 의 고를 것들. 기본은 지켜볼 것 없음, 3초, 떠나는 길은 프로세스 끝내기.
pub struct Options {
    /// 인사를 받은 뒤 지켜볼 실행 파일. 하나라도 비켜지면 끊김을 적고 `leave` 를 부른다. 요청 하나만 받는 수집 호스트는
    /// 인사를 받지 않으므로 지켜보지 않는다 — 저장 도중에 떠나지 않는다.
    pub watch: Vec<ExecutableIdentity>,
    pub interval: Duration,
    /// 떠나는 길. 기본은 프로세스를 0 으로 끝내는 것이다 — stdout 이 닫혀 브라우저가 포트를 끊는다.
    pub leave: Arc<dyn Fn() + Send + Sync>,
    /// 브라우저의 호스트 등록이 아직 이 실행 파일을 가리키는가. 오류면 가리킨다고 친다.
    pub registered: Option<Arc<dyn Fn() -> io::Result<bool> + Send + Sync>>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            watch: Vec::new(),
            interval: WATCH_INTERVAL,
            leave: Arc::new(|| std::process::exit(0)),
            registered: None,
        }
    }
}

/// 호스트의 메시지 고리. 상시 연결 인사·탭 보고는 `dispatch` 에 닿기 전에 여기서
/// 받는다. 입력이 끝나 고리가 끝나면(또는 멈추면) 끊김을 적고 이 프로세스의 파일을 지운다.
pub fn serve<R: Read, W: Write + Send + 'static>(
    input: R,
    output: Arc<Mutex<W>>,
    directory: &Path,
    mut dispatch: impl FnMut(&Value) -> Value,
    options: Options,
) -> Result<(), native_messaging::Error> {
    let push_output = Arc::clone(&output);
    let session = Arc::new(Session {
        directory: directory.to_path_buf(),
        watch: options.watch,
        interval: options.interval,
        leave: options.leave,
        registered: options.registered,
        push: Arc::new(move |command: Value| {
            native_messaging::send(&push_output, &command)
                .map_err(|e| io::Error::other(e.to_string()))
        }),
        state: Mutex::new(State::default()),
        stop: Arc::new((Mutex::new(false), Condvar::new())),
    });
    let result = native_messaging::run(input, &output, |request| {
        // 떠나는 길과 겹치지 않게 — 요청 하나를 다 처리한 뒤에만 떠난다.
        let mut state = session.lock();
        if is_presence(request) {
            session.hello(&mut state, request)
        } else if is_update_status(request) {
            session.update_status(&state, request)
        } else if is_update_check(request) {
            session.update_check(&state, request)
        } else if relay::is_tabs(request) {
            session.tabs(&state, request)
        } else if relay::is_beat(request) {
            session.beat(&state, request)
        } else {
            dispatch(request)
        }
    });
    session.end();
    result
}

type Push = Arc<dyn Fn(Value) -> io::Result<()> + Send + Sync>;

struct Session {
    directory: PathBuf,
    watch: Vec<ExecutableIdentity>,
    interval: Duration,
    leave: Arc<dyn Fn() + Send + Sync>,
    registered: Option<Arc<dyn Fn() -> io::Result<bool> + Send + Sync>>,
    push: Push,
    state: Mutex<State>,
    /// 지켜보는 실마리를 멈추는 신호.
    stop: Arc<(Mutex<bool>, Condvar)>,
}

#[derive(Default)]
struct State {
    hello: Option<(String, String, Option<String>)>,
    commands: Option<relay::Watcher>,
    expected_revision: Option<String>,
    ended: bool,
}

impl Session {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn hello(self: &Arc<Self>, state: &mut State, request: &Value) -> Value {
        let (reply, accepted) = handle(request, &self.directory);
        if let Some(accepted) = accepted
            && state.hello.is_none()
        {
            let _ =
                log::record_connected(&self.directory, &accepted.0, &accepted.1, win::local_now());
            // 응답에 실은 바로 그 판을 기준으로 삼는다. 여기서 디스크를 다시 읽으면, 응답과
            // 기준 사이에 판이 바뀐 경우 지킴이가 그 바뀜을 놓친다.
            state.expected_revision = expected_revision(&reply);
            state.hello = Some(accepted);
            if !self.watch.is_empty()
                || self.registered.is_some()
                || state.hello.as_ref().is_some_and(|h| h.2.is_some())
            {
                let session = Arc::clone(self);
                std::thread::spawn(move || session.watch_loop());
            }
            // 창이 놓는 명령(그 탭을 앞으로·다시 읽기)을 포트로 넘긴다(ADR-036). 인사를 받은 호스트만.
            let push = Arc::clone(&self.push);
            state.commands = relay::Watcher::new(
                &self.directory,
                process_id(),
                Box::new(move |command| push(command)),
                None,
            )
            .ok();
        }
        reply
    }

    /// 나라장터 탭 보고(ADR-036). 인사를 받은 포트에서만 받는다. 받은 것은 홈에만 적고 출력하지 않는다.
    fn tabs(&self, state: &State, request: &Value) -> Value {
        let id = request_id(request);
        if state.hello.is_none() {
            return error(id, "인사 전에는 탭 보고를 받지 않습니다.");
        }
        let Some(report) = request.get("params") else {
            return error(id, "탭 보고가 비었습니다.");
        };
        match relay::write_tabs(&self.directory, process_id(), report, win::local_now()) {
            Ok(None) => status_reply(id, "received"),
            Ok(Some(refused)) => error(id, &refused),
            Err(_) => error(id, "탭 보고를 적지 못했습니다."),
        }
    }

    /// 앞 탭의 박동 — 탭 보고와 같은 결로 인사를 받은 포트에서만 받고 홈에만 적는다.
    fn beat(&self, state: &State, request: &Value) -> Value {
        let id = request_id(request);
        if state.hello.is_none() {
            return error(id, "인사 전에는 박동을 받지 않습니다.");
        }
        let Some(beat) = request.get("params") else {
            return error(id, "박동이 비었습니다.");
        };
        match relay::write_beat(&self.directory, process_id(), beat) {
            Ok(None) => status_reply(id, "received"),
            Ok(Some(refused)) => error(id, &refused),
            Err(_) => error(id, "박동을 적지 못했습니다."),
        }
    }

    fn update_status(&self, state: &State, request: &Value) -> Value {
        let id = request_id(request);
        if state.hello.is_none() {
            return error(id, "인사 전에는 갱신 상태를 받지 않습니다.");
        }
        let value = request
            .get("params")
            .and_then(Value::as_object)
            .and_then(|p| p.get("state"))
            .and_then(Value::as_str);
        if !matches!(
            value,
            Some(
                "preparing" | "waiting-panel" | "waiting-work" | "reloading" | "failed" | "current"
            )
        ) {
            return error(id, "확장이 보낸 갱신 상태를 읽지 못했습니다.");
        }
        if update_live_state(&self.directory, value.unwrap()).is_err() {
            return error(id, "확장 갱신 상태를 적지 못했습니다.");
        }
        status_reply(id, "received")
    }

    fn update_check(&self, state: &State, request: &Value) -> Value {
        if state.hello.is_none() {
            return error(request_id(request), "인사 전에는 갱신 판을 받지 않습니다.");
        }
        revision_reply(request_id(request), status::disk_revision(&self.directory))
    }

    fn still_registered(&self) -> bool {
        self.registered
            .as_ref()
            .is_none_or(|registered| registered().unwrap_or(true))
    }

    /// 인사 뒤에 도는 지킴이. 첫 확인은 한 간격 뒤다.
    fn watch_loop(self: Arc<Self>) {
        let mut expected = self.lock().expected_revision.clone();
        let (stopped, signal) = &*self.stop;
        loop {
            {
                let guard = stopped.lock().unwrap_or_else(PoisonError::into_inner);
                let (guard, _) = signal
                    .wait_timeout_while(guard, self.interval, |stop| !*stop)
                    .unwrap_or_else(PoisonError::into_inner);
                if *guard {
                    return;
                }
            }
            let next = status::disk_revision(&self.directory);
            if next != expected {
                expected = next.clone();
                let mut state = self.lock();
                if state.ended {
                    return;
                }
                state.expected_revision = next.clone();
                let _ = (self.push)(update_notice(next));
            }
            if self.watch.iter().all(ExecutableIdentity::still_here) && self.still_registered() {
                continue;
            }
            {
                let mut state = self.lock();
                if state.ended {
                    return;
                }
                self.finish(&mut state);
            }
            (self.leave)();
            return;
        }
    }

    /// 포트가 닫혔다. 이미 떠났으면 아무것도 하지 않는다.
    fn end(&self) {
        let mut state = self.lock();
        if !state.ended {
            self.finish(&mut state);
        }
    }

    fn finish(&self, state: &mut State) {
        state.ended = true;
        {
            let (stopped, signal) = &*self.stop;
            *stopped.lock().unwrap_or_else(PoisonError::into_inner) = true;
            signal.notify_all();
        }
        state.commands = None;
        relay::forget(&self.directory, process_id());
        if let Some((browser, version, _)) = &state.hello {
            let _ = log::record_disconnected(&self.directory, browser, version, win::local_now());
        }
        forget(&self.directory);
    }
}

fn error(request_id: Option<&str>, message: &str) -> Value {
    envelope(request_id, "invalid_request", message)
}

/// 인사를 받아 이 프로세스의 파일을 쓰고 마지막 연결도 적는다.
fn handle(request: &Value, directory: &Path) -> (Value, Option<(String, String, Option<String>)>) {
    let id = request_id(request);
    let protocol_ok = request.get("protocolVersion").and_then(wire::try_get_int32) == Some(2);
    if !wire::is_guid(id) || !protocol_ok {
        return (error(id, "앱과 확장을 함께 업데이트하세요."), None);
    }
    // 확장이 보낸 값이라 마지막 연결과 같은 모양만 받는다 — 확장이 디스크에 임의 문자열을 쓰지 못하게.
    let args = request.get("params").and_then(Value::as_object);
    let browser = args.and_then(|a| a.get("browser")).and_then(Value::as_str);
    let version = args
        .and_then(|a| a.get("extensionVersion"))
        .and_then(Value::as_str);
    let (Some(browser), Some(version)) = (browser, version) else {
        return (
            error(id, "확장이 보낸 브라우저·판을 읽지 못했습니다."),
            None,
        );
    };
    if !status::accepts(browser, version) {
        return (
            error(id, "확장이 보낸 브라우저·판을 읽지 못했습니다."),
            None,
        );
    }
    let revision = args
        .and_then(|a| a.get("extensionRevision"))
        .and_then(Value::as_str);
    let revision = match revision {
        None => None,
        Some(revision) if status::revision_shaped(revision) => Some(revision.to_string()),
        Some(_) => return (error(id, "확장이 보낸 묶음 판을 읽지 못했습니다."), None),
    };
    if write_live(directory, browser, version, revision.as_deref()).is_err() {
        return (error(id, "확장 연결을 적지 못했습니다."), None);
    }
    let _ = status::record_contact(directory, version, browser, win::local_now());
    (
        revision_reply(id, status::disk_revision(directory)),
        Some((browser.to_string(), version.to_string(), revision)),
    )
}

fn write_live(
    directory: &Path,
    browser: &str,
    version: &str,
    revision: Option<&str>,
) -> io::Result<()> {
    let (seconds, ticks) = win::process_start()?;
    let pid = process_id();
    let mut live = Map::new();
    live.insert("browser".into(), Value::String(browser.into()));
    live.insert("version".into(), Value::String(version.into()));
    if let Some(revision) = revision {
        live.insert("revision".into(), Value::String(revision.into()));
    }
    live.insert("pid".into(), pid.into());
    live.insert(
        "processStartUtc".into(),
        Value::String(win::format_round_trip(seconds, ticks)),
    );
    live.insert(
        "connectedAt".into(),
        Value::String(win::local_now().second_text()),
    );
    std::fs::create_dir_all(live_directory(directory))?;
    paths::replace_file(
        &live_path(directory, pid),
        &wire::to_web_json(&Value::Object(live)),
    )
}

fn update_live_state(directory: &Path, update_state: &str) -> io::Result<()> {
    let path = live_path(directory, process_id());
    let text = paths::read_all_text(&path)?;
    let Some(Value::Object(mut live)) = wire::parse_node(&text) else {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "live"));
    };
    live.insert("updateState".into(), Value::String(update_state.into()));
    paths::replace_file(&path, &wire::to_web_json(&Value::Object(live)))
}

fn revision_reply(id: Option<&str>, revision: Option<String>) -> Value {
    let mut reply = status_reply(id, "present");
    if let Some(result) = reply.get_mut("result").and_then(Value::as_object_mut) {
        result.insert(
            "expectedRevision".into(),
            revision.map_or(Value::Null, Value::String),
        );
    }
    reply
}

fn expected_revision(reply: &Value) -> Option<String> {
    reply
        .get("result")
        .and_then(Value::as_object)
        .and_then(|result| result.get("expectedRevision"))
        .and_then(Value::as_str)
        .filter(|revision| status::revision_shaped(revision))
        .map(str::to_string)
}

fn update_notice(revision: Option<String>) -> Value {
    let mut message = Map::new();
    message.insert("protocolVersion".into(), 2.into());
    message.insert("command".into(), Value::String("extensionUpdate".into()));
    message.insert(
        "revision".into(),
        revision.map_or(Value::Null, Value::String),
    );
    Value::Object(message)
}

fn is_update_status(request: &Value) -> bool {
    is_method(request, UPDATE_STATUS_METHOD)
}
fn is_update_check(request: &Value) -> bool {
    is_method(request, UPDATE_CHECK_METHOD)
}

/// 이 프로세스의 파일을 지운다. 끝나는 길이라 실패를 삼킨다.
pub fn forget(directory: &Path) {
    let _ = std::fs::remove_file(live_path(directory, process_id()));
}

/// 살아 있는 연결 — 연결 시각 차례.
pub fn read_live(directory: &Path) -> Vec<LiveContact> {
    let mut live: Vec<LiveContact> = live_processes(directory)
        .into_iter()
        .map(|(_, c)| c)
        .collect();
    live.sort_by(|a, b| {
        a.connected_at
            .encode_utf16()
            .cmp(b.connected_at.encode_utf16())
    });
    live
}

/// 살아 있는 연결과 그 호스트의 번호. 파일의 프로세스가 아직 돌고 **그 시작 시각까지
/// 같아야**(2초 안) 센다. 끝난 것의 파일은 그 탭 보고·명령 폴더와 함께 거두고, 모양이 깨진 파일은 건너뛴다. 들여다볼 수
/// 없는 프로세스는 세지도 거두지도 않는다.
pub fn live_processes(directory: &Path) -> Vec<(i32, LiveContact)> {
    let mut live = Vec::new();
    let Ok(listing) = std::fs::read_dir(live_directory(directory)) else {
        return live;
    };
    for entry in listing.filter_map(Result::ok) {
        if !entry.file_type().is_ok_and(|t| t.is_file()) {
            continue;
        }
        let file = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let lower = paths::to_upper_invariant(&name);
        if !lower.ends_with(".JSON")
            || lower.ends_with(".TABS.JSON")
            || lower.ends_with(".BEAT.JSON")
        {
            continue;
        }
        let Some((pid, contact, started)) = read_entry(&file, &name) else {
            continue;
        };
        match win::running_process_start(pid) {
            Ok(Some((seconds, ticks))) => {
                let delta = (seconds - started.0) as f64
                    + (f64::from(ticks) - f64::from(started.1)) / 10_000_000.0;
                if delta.abs() <= 2.0 {
                    live.push((pid, contact));
                } else {
                    let _ = std::fs::remove_file(&file);
                    relay::forget(directory, pid);
                }
            }
            Ok(None) => {
                let _ = std::fs::remove_file(&file);
                relay::forget(directory, pid);
            }
            Err(_) => {}
        }
    }
    live
}

/// 연결 파일 하나 — 번호·연결·시작 순간(유닉스 초, 100ns). 모양이 틀리면 `None`.
fn read_entry(file: &Path, name: &str) -> Option<(i32, LiveContact, (i64, u32))> {
    let text = paths::read_all_text(file).ok()?;
    let Value::Object(entry) = wire::parse_node(&text)? else {
        return None;
    };
    let pid_value = entry.get("pid").filter(|v| v.is_number())?;
    let start = entry.get("processStartUtc")?.as_str()?;
    let browser = entry.get("browser")?.as_str()?;
    let version = entry.get("version")?.as_str()?;
    let connected_at = entry.get("connectedAt")?.as_str()?;
    let revision = match entry.get("revision") {
        None => None,
        Some(value) => {
            status::revision_shaped(value.as_str()?).then(|| value.as_str().unwrap().to_string())
        }
    };
    let update_state = match entry.get("updateState") {
        None => None,
        Some(value) => matches!(
            value.as_str()?,
            "preparing" | "waiting-panel" | "waiting-work" | "reloading" | "failed" | "current"
        )
        .then(|| value.as_str().unwrap().to_string()),
    };
    let pid = wire::try_get_int32(pid_value)?;
    if name != format!("{pid}.json") || !status::accepts(browser, version) {
        return None;
    }
    // 앱이 쓰는 꼴(「O」)만 읽는다. 오프셋이 없으면 UTC 로 본다.
    let at = parse_to_local(Some(start), &FixedZone(0))?;
    Some((
        pid,
        LiveContact {
            browser: browser.into(),
            version: version.into(),
            revision,
            update_state,
            connected_at: connected_at.into(),
        },
        (at.unix_seconds(), at.ticks),
    ))
}

//! 확장 ↔ 창의 중계(C# `Erp/ExtensionRelay.cs`, ADR-036). 호스트와 창은 다른 프로세스라 **홈의 파일로 잇는다.**
//!
//! - 확장이 상시 포트로 보낸 나라장터 탭 보고를 호스트가 `extension-live/<pid>.tabs.json` 에 통째로 바꿔 쓴다.
//! - 창은 명령을 `extension-live/<pid>.cmd/<순번>.json` 에 놓고, 호스트가 그 폴더를 지켜 포트로 넘긴 뒤 지운다.
//!
//! 탭 보고에는 **실제 조달 자료**가 들어 있다. 홈 밖으로 내지 않고 로그에 남기지 않는다. 바이트는 C# 과 같다 — 창(C#)이
//! Rust 호스트가 쓴 보고를 그대로 읽는다.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use pclm_core::erp::{screens, wire};
use pclm_core::local_time::LocalDateTime;
use pclm_home::{dotnet, win};
use serde_json::{Map, Value};

use crate::presence;

pub const TABS_METHOD: &str = "tabs";

/// 창이 보낼 수 있는 명령. 확장은 이것 말고는 받지 않는다.
pub const COMMANDS: [&str; 4] = ["focusTab", "read", "export", "openExtensions"];

/// 탭 하나의 화면 줄 수 상한.
pub const MAX_SCREEN_ROWS: usize = 400;
const MAX_SCREEN_TEXT: usize = 1000;

/// 명령 폴더를 다시 보는 간격. 폴더 변화 알림이 놓친 것을 줍는다.
pub const POLL_INTERVAL: Duration = Duration::from_millis(700);

/// 탭 번호가 있어야 하는 명령.
fn for_tab(command: &str) -> bool {
    matches!(command, "focusTab" | "read" | "export")
}

pub fn tabs_path(directory: &Path, pid: i32) -> PathBuf {
    presence::live_directory(directory).join(format!("{pid}.tabs.json"))
}

pub fn command_directory(directory: &Path, pid: i32) -> PathBuf {
    presence::live_directory(directory).join(format!("{pid}.cmd"))
}

pub fn is_tabs(request: &Value) -> bool {
    request.get("method").and_then(Value::as_str) == Some(TABS_METHOD)
}

/// 탭 보고 한 벌을 적는다(C# `ExtensionRelay.WriteTabs`). 모양만 본다 — 값은 확장이 읽은 그대로 둔다(ADR-016).
/// 받지 않았으면 그 까닭.
pub fn write_tabs(
    directory: &Path,
    pid: i32,
    report: &Value,
    now: LocalDateTime,
) -> io::Result<Option<String>> {
    let refuse = || Ok(Some("탭 보고의 모양이 다릅니다.".to_string()));
    let Some(Value::Array(tabs)) = report.as_object().and_then(|r| r.get("tabs")) else {
        return refuse();
    };
    let mut written = Vec::with_capacity(tabs.len());
    for tab in tabs {
        let Value::Object(fields) = tab else {
            return refuse();
        };
        let id_ok = fields.get("tabId").and_then(wire::try_get_int32).is_some();
        let state_ok = matches!(
            fields.get("state").and_then(Value::as_str),
            Some("reading" | "supported" | "unsupported" | "error")
        );
        let rows_ok = fields.get("screenRows").is_none_or(screen_rows_fit);
        // 메뉴 번호(ADR-037)는 빈 글이거나 숫자 다섯이다. 옛 확장은 싣지 않는다.
        let menu_ok = fields.get("menu").is_none_or(|menu| {
            menu.as_str()
                .is_some_and(|m| m.is_empty() || screens::is_code(Some(m)))
        });
        if !(id_ok && state_ok && rows_ok && menu_ok) {
            return refuse();
        }
        written.push(tab.clone());
    }
    let mut settings = Value::Null;
    if let Some(given) = report.get("settings").filter(|v| !v.is_null()) {
        match given.get("siteAccess") {
            Some(Value::Bool(access)) => {
                let mut o = Map::new();
                o.insert("siteAccess".into(), Value::Bool(*access));
                settings = Value::Object(o);
            }
            _ => return refuse(),
        }
    }
    let mut root = Map::new();
    root.insert("pid".into(), pid.into());
    root.insert("receivedAt".into(), Value::String(now.second_text()));
    root.insert(
        "front".into(),
        report
            .get("front")
            .and_then(wire::try_get_int32)
            .map_or(Value::Null, Value::from),
    );
    root.insert(
        "frontAt".into(),
        report
            .get("frontAt")
            .and_then(wire::try_get_int64)
            .unwrap_or(0)
            .into(),
    );
    root.insert("tabs".into(), Value::Array(written));
    root.insert("settings".into(), settings);
    replace(
        &tabs_path(directory, pid),
        &wire::to_web_json(&Value::Object(root)),
    )?;
    Ok(None)
}

fn screen_rows_fit(rows: &Value) -> bool {
    let Value::Array(rows) = rows else {
        return false;
    };
    rows.len() <= MAX_SCREEN_ROWS
        && rows.iter().all(|row| {
            ["group", "label", "text", "source"].iter().all(|name| {
                row.get(name)
                    .and_then(Value::as_str)
                    .is_some_and(|v| wire::utf16_len(v) <= MAX_SCREEN_TEXT)
            })
        })
}

/// 한 호스트가 낸 마지막 탭 보고.
#[derive(Clone, Debug, PartialEq)]
pub struct TabReport {
    pub pid: i32,
    pub browser: String,
    pub report: Value,
    pub connected_at: String,
}

/// 지금 살아 있는 호스트가 낸 탭 보고들(창이 읽는 쪽 — 시험용으로 함께 옮겼다).
pub fn read_tabs(directory: &Path) -> Vec<TabReport> {
    let mut reports = Vec::new();
    for (pid, contact) in presence::live_processes(directory) {
        let Ok(text) = dotnet::read_all_text(&tabs_path(directory, pid)) else {
            continue;
        };
        // JsonDocument.Parse(MaxDepth 64) — 같은 키를 막지 않는다.
        if let Some(report) = pclm_core::erp::parse_json(&text) {
            reports.push(TabReport {
                pid,
                browser: contact.browser,
                report,
                connected_at: contact.connected_at,
            });
        }
    }
    reports
}

/// 창 → 확장 명령 하나를 그 호스트의 명령 폴더에 놓는다(C# `ExtensionRelay.Send`/`SendSetting`). 살아 있지 않은 호스트에는
/// 놓지 않는다(`Ok(false)`). 모르는 명령·모양이 틀린 명령은 `Err`(C# `InvalidOperationException`).
pub fn send(directory: &Path, pid: i32, command: &str, tab_id: Option<i32>) -> io::Result<bool> {
    if !COMMANDS.contains(&command) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("모르는 명령입니다: {command}"),
        ));
    }
    if for_tab(command) != tab_id.is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("명령의 모양이 다릅니다: {command}"),
        ));
    }
    if !presence::live_processes(directory)
        .iter()
        .any(|(p, _)| *p == pid)
    {
        return Ok(false);
    }
    let folder = command_directory(directory, pid);
    std::fs::create_dir_all(&folder)?;
    // 이름이 곧 차례다. 같은 틱에 둘이 와도 뒤의 GUID 가 가른다.
    let since = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let ticks = 621_355_968_000_000_000u64
        + since.as_secs() * 10_000_000
        + u64::from(since.subsec_nanos() / 100);
    let name = format!("{ticks:019}-{}.json", &win::new_guid_n()[..8]);
    let mut body = Map::new();
    body.insert("command".into(), Value::String(command.into()));
    if let Some(tab) = tab_id {
        body.insert("tabId".into(), tab.into());
    }
    replace(&folder.join(name), &wire::to_web_json(&Value::Object(body)))?;
    Ok(true)
}

/// 이 프로세스가 남긴 탭 보고와 명령 폴더를 치운다. 끝나는 길이라 실패를 삼킨다.
pub fn forget(directory: &Path, pid: i32) {
    let _ = std::fs::remove_file(tabs_path(directory, pid));
    let _ = std::fs::remove_dir_all(command_directory(directory, pid));
}

/// 임시 파일은 `.json` 으로 끝나지 않게 한다 — 명령 폴더를 지키는 쪽이 반쯤 쓴 것을 집지 않는다.
fn replace(path: &Path, text: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    dotnet::replace_file(path, text)
}

/// 명령을 넘기는 길. 확장으로 가는 포트에 쓴다.
pub type Deliver = Box<dyn Fn(Value) -> io::Result<()> + Send + Sync>;

/// 호스트 쪽: 명령 폴더를 지켜 들어온 것을 차례대로 넘기고 지운다(C# `ExtensionRelay.Watcher`). 폴더 변화 알림이 놓칠 수
/// 있어 짧게 다시 본다. 모양이 틀린 것은 넘기지 않고 지운다. 놓으면(`Drop`) 지킴을 멈추고 그 실마리를 기다린다.
pub struct Watcher {
    inner: Arc<Inner>,
    thread: Option<JoinHandle<()>>,
}

struct Inner {
    folder: PathBuf,
    deliver: Deliver,
    disposed: Mutex<bool>,
    wait: win::ChangeWait,
}

impl Watcher {
    pub fn new(
        directory: &Path,
        pid: i32,
        deliver: Deliver,
        interval: Option<Duration>,
    ) -> io::Result<Watcher> {
        let folder = command_directory(directory, pid);
        std::fs::create_dir_all(&folder)?;
        let inner = Arc::new(Inner {
            wait: win::ChangeWait::new(&folder)?,
            folder,
            deliver,
            disposed: Mutex::new(false),
        });
        let every = interval.unwrap_or(POLL_INTERVAL);
        inner.drain();
        let worker = Arc::clone(&inner);
        let thread = std::thread::spawn(move || {
            while worker.wait.wait(every) != win::Woke::Stopped {
                worker.drain();
            }
        });
        Ok(Watcher {
            inner,
            thread: Some(thread),
        })
    }

    /// 쌓인 명령을 차례대로 넘긴다. 시험이 직접 부르기도 한다.
    pub fn drain(&self) {
        self.inner.drain();
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        *self
            .inner
            .disposed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = true;
        self.inner.wait.stop();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Inner {
    fn drain(&self) {
        let disposed = self
            .disposed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if *disposed {
            return;
        }
        let Ok(listing) = std::fs::read_dir(&self.folder) else {
            return;
        };
        // Directory.GetFiles("*.json") 뒤 GetExtension(f) == ".json"(대소문자를 가린다), 이름의 UTF-16 차례.
        let mut files: Vec<(Vec<u16>, PathBuf)> = listing
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                name.ends_with(".json")
                    .then(|| (name.encode_utf16().collect(), e.path()))
            })
            .collect();
        files.sort();
        for (_, file) in files {
            let text = match dotnet::read_all_text(&file) {
                Ok(text) => text,
                Err(_) => continue, // 아직 쓰는 중 — 다음에 줍는다.
            };
            let command = command_of(&text);
            if std::fs::remove_file(&file).is_err() {
                continue;
            }
            if let Some(command) = command {
                let _ = (self.deliver)(command);
            }
        }
    }
}

/// 명령 파일의 글 → 포트로 보낼 명령. 모양이 틀리면 `None`(지우기만 한다).
fn command_of(text: &str) -> Option<Value> {
    let Value::Object(parsed) = wire::parse_node(text)? else {
        return None;
    };
    let name = parsed.get("command")?.as_str()?;
    if !COMMANDS.contains(&name) {
        return None;
    }
    let mut command = Map::new();
    command.insert("protocolVersion".into(), 2.into());
    command.insert("command".into(), Value::String(name.into()));
    if for_tab(name) {
        let tab = parsed.get("tabId")?;
        if !tab.is_number() {
            return None;
        }
        // GetValue<int>() — int 로 나타낼 수 없으면 던지고, 모양이 틀린 명령으로 지운다.
        command.insert("tabId".into(), wire::try_get_int32(tab)?.into());
    } else if name != "openExtensions" {
        return None;
    }
    Some(Value::Object(command))
}

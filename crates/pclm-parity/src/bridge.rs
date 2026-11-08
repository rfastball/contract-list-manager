//! 다리 대본(#8 ⑤ · ⑥ · ⑧a) — `bridge <home> <file> <script.json> [--view] [--files <dir>] [--clock now]`. `pclm-bridge` 의 다리를
//! 세우고 대본의 부름을 차례로 `invoke` 에 넘겨, 답 한 통(`{id, ok, result|error}`, `reply_json`)을 한 줄씩
//! `<file>.bridge.txt` 에 적는다. 꼴은 `crates/pclm-parity/README.md` 가 정본이고, C# 쪽은 `tests/Pclm.Parity/BridgeParity.cs` 다.
//!
//! 인자는 C# `HandleAsync` 처럼 편다 — 글은 글, null 은 null, 그 밖의 값은 JSON 글. C# 은 원문(`GetRawText`)을 넘기고
//! 여기서는 다시 지은 짧은 글을 넘기므로, 공백·이스케이프를 원문과 다르게 적은 대본에서만 갈린다 — 대본은 인자를
//! 글이나 null 로 둔다. 글 안의 자리표([`fill`])는 넘기기 전에 바꾼다(README 「다리」).
//!
//! 창([`Host`])은 계기가 단다 — 고르기는 부름의 `pick` 이 답한다(글이면 `--files` 폴더 아래의 그 자리, `/` 는 `\` 로, null 이거나
//! 없으면 그만둔다). 건네받은 것과 창에 넘긴 일(다시 띄우기 · 닫기 · 알림 영역)과 확장에 보낸 명령은 한 줄씩
//! `<file>.prompts.txt` 에 적는다. 지어 둔 이름의 오늘은 `<오늘>` 로 바꾼다.
//!
//! 「지금」 은 C# 계기의 `AsOf` 와 같은 순간(2026-09-10T00:00:00Z)으로 못박는다 — `dashboard` 의 `asOf` 와 단계가 그것으로
//! 갈린다. `--clock now` 면 진짜 시계다 — C# 다리는 수집 기록 · 파일 이름의 날짜를 `DateTime.Now` 로 읽으므로, 그것을 쓰는
//! 대본은 두 쪽을 진짜 시계로 돌리고 대조가 시각을 가린다. `direct` 는 다리의 문을 거치지 않는 부름이다(README 「다리」). 운영체제는 읽기만 진짜고([`Guarded`]) 등록부 쓰기 · 탐색기 ·
//! 새 프로세스는 거절한다 — 대본은 그것을 부르지 않는다.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Duration;

use pclm_bridge::{
    Bridge, FolderPrompt, Host, OpenPrompt, Os, RegistryRoot, RegistryValue, SavePrompt, SpecialFolder,
    WindowsOs, extension_status_at, prepare_bundled_extension, reply_json,
};
use pclm_core::erp::auto_capture::AutoCapture;
use pclm_core::local_time::{LocalZone, parse_to_local};
use pclm_core::storage::pclm_file;
use pclm_core::storage::store::Store;
use pclm_core::{Access, Database, Env};
use serde_json::Value;

use crate::system_env::SystemEnv;

/// 부름마다 대본의 `pick` 을 내주고, 건네받은 것을 적는다.
struct Recorder {
    id: AtomicI32,
    pick: Mutex<Option<String>>,
    today: String,
    prompts: Mutex<Vec<String>>,
}

impl Recorder {
    fn note(&self, fields: &[&str]) {
        let mut line = self.id.load(Ordering::SeqCst).to_string();
        for field in fields {
            line.push('\t');
            line.push_str(field);
        }
        self.prompts.lock().unwrap().push(line);
    }

    fn answer(&self) -> Option<String> {
        self.pick.lock().unwrap().clone()
    }
}

impl Host for Recorder {
    fn pick_save_path(&self, prompt: &SavePrompt) -> Option<String> {
        self.note(&[
            "save",
            &prompt.title,
            &prompt.filter,
            &prompt.default_ext,
            &prompt.file_name.replace(&self.today, "<오늘>"),
            &prompt.allow_overwrite.to_string(),
        ]);
        self.answer()
    }

    fn pick_plan_excel(&self, _: &OpenPrompt) -> Option<String> {
        self.note(&["planExcel"]);
        self.answer()
    }

    fn pick_folder(&self, prompt: &FolderPrompt) -> Option<String> {
        self.note(&[
            "folder",
            &prompt.title,
            prompt.initial.as_deref().unwrap_or(""),
        ]);
        self.answer()
    }

    fn pick_open_file(&self, prompt: &OpenPrompt) -> Option<String> {
        self.note(&["open", &prompt.title]);
        self.answer()
    }

    fn pick_open_json(&self, prompt: &OpenPrompt) -> Option<String> {
        self.note(&["json", &prompt.title]);
        self.answer()
    }

    fn apply_close_to_tray(&self, on: bool) {
        self.note(&["closeToTray", &on.to_string()]);
    }
}

/// 읽기만 진짜인 운영체제 — 계기는 이 기계의 등록부 · 탐색기 · 브라우저를 고치거나 띄우지 않는다.
struct Guarded;

fn refused<T>() -> std::io::Result<T> {
    Err(std::io::Error::other(
        "대조 계기는 등록부를 고치거나 창 · 프로그램을 띄우지 않습니다.",
    ))
}

impl Os for Guarded {
    fn process_path(&self) -> Option<PathBuf> {
        WindowsOs.process_path()
    }
    fn default_home(&self) -> std::io::Result<PathBuf> {
        WindowsOs.default_home()
    }
    fn special_folder(&self, folder: SpecialFolder) -> PathBuf {
        WindowsOs.special_folder(folder)
    }
    fn temp_path(&self) -> PathBuf {
        WindowsOs.temp_path()
    }
    fn registry_get(
        &self,
        root: RegistryRoot,
        key: &str,
        name: &str,
    ) -> std::io::Result<Option<RegistryValue>> {
        WindowsOs.registry_get(root, key, name)
    }
    fn registry_set_string(&self, _: &str, _: &str, _: &str) -> std::io::Result<()> {
        refused()
    }
    fn registry_delete_value(&self, _: &str, _: &str) -> std::io::Result<()> {
        refused()
    }
    fn shell_open(&self, _: &Path) -> std::io::Result<()> {
        refused()
    }
    fn start_process(&self, _: &Path, _: &[String], _: bool) -> std::io::Result<()> {
        refused()
    }
    fn signal_activation(&self, name: &str) -> bool {
        WindowsOs.signal_activation(name)
    }
    fn browser_windows(&self, _: &str) -> Vec<isize> {
        Vec::new()
    }
    fn raise_new_window(&self, _: &str, _: &[isize], _: Duration) -> bool {
        false
    }
    fn navigate_new_window(&self, _: &str, _: &[isize], _: &str) -> bool {
        false
    }
    fn set_clipboard(&self, _: &str) {}
    fn file_version(&self, path: &Path) -> Option<String> {
        WindowsOs.file_version(path)
    }
}

/// 두 계기가 못박는 「지금」.
struct Pinned;

impl Env for Pinned {
    fn now_utc(&self) -> String {
        "2026-09-10T00:00:00.0000000Z".into()
    }

    fn new_id(&self) -> String {
        crate::system_env::SystemEnv.new_id()
    }
}

static PINNED: Pinned = Pinned;
static GUARDED: Guarded = Guarded;
static REAL: SystemEnv = SystemEnv;

pub fn run(args: &[String]) -> Result<(), String> {
    let usage = "bridge <home> <file> <script.json> [--view] [--files <dir>] [--clock now]";
    let [home, file, script, rest @ ..] = args else {
        return Err(usage.into());
    };
    let mut view = false;
    let mut files: Option<&String> = None;
    let mut now = false;
    let mut flags = rest.iter();
    while let Some(flag) = flags.next() {
        match flag.as_str() {
            "--view" => view = true,
            "--files" => files = Some(flags.next().ok_or(usage)?),
            "--clock" if flags.next().map(String::as_str) == Some("now") => now = true,
            _ => return Err(usage.into()),
        }
    }
    let env: &'static (dyn Env + Sync) = if now { &REAL } else { &PINNED };
    let home = pclm_home::Home::new(absolute(home)?).map_err(|e| e.to_string())?;
    let text = std::fs::read_to_string(script).map_err(|e| e.to_string())?;
    let calls: Value =
        serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| e.to_string())?;
    let calls = calls.as_array().ok_or("대본은 JSON 배열이어야 합니다.")?;
    let file = absolute(file)?;
    let pid = std::process::id().to_string();
    let live = home.directory().join("extension-live");

    let viewed = if view {
        let directory = home.view_directory();
        std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        let copy = directory.join(format!("열람-{}.pclm", PINNED.new_id()));
        Some(pclm_file::open_view(&file, &copy, env).map_err(|e| e.to_string())?)
    } else {
        None
    };
    let database = match &viewed {
        Some(v) => v.database().clone(),
        None => Database::new(&file, Access::Write),
    };
    let mapping = pclm_core::erp::mapping::MappingStore::new(&database, env)
        .active()
        .map(|m| pclm_core::erp::mapping::revision(&m))
        .unwrap_or_default();
    // C# 다리의 「오늘」 은 DateTime.Now 다. 여기서는 다리가 읽는 그대로 — 계기의 지금을 이 기계의 시간대로.
    let today = || {
        parse_to_local(Some(&env.now_utc()), &SystemEnv as &dyn LocalZone)
            .map(pclm_excel::exporter::compact_date)
            .unwrap_or_default()
    };
    let recorder = Recorder {
        id: AtomicI32::new(0),
        pick: Mutex::new(None),
        today: today(),
        prompts: Mutex::new(Vec::new()),
    };
    let watched = database.clone();
    let mut automatic = AutoCapture::new(&watched, &home, env);
    let store = Store::new(watched.clone(), env);
    let mut bridge = Bridge::new(database, home.clone(), viewed, env, &SystemEnv)?;
    bridge.set_host(&recorder);
    bridge.set_os(&GUARDED);

    let mut out = String::new();
    let mut replies: Vec<Option<Value>> = Vec::new();
    let mut announced = false;
    for (index, call) in calls.iter().enumerate() {
        let id = index as i32 + 1;
        recorder.id.store(id, Ordering::SeqCst);
        let dataset = pclm_file::inspect(&file).dataset_id.unwrap_or_default();
        let marks = |token: &str| named(token, &pid, &mapping, &dataset, &store);
        let fill = |text: &str| fill(text, &marks, &replies);
        // 탭 보고 — 이 계기가 확장 호스트인 것처럼 홈의 extension-live 에 연결과 보고를 적는다.
        if let Some(report) = call.get("tabs") {
            if !announced {
                announce(&live, &pid)?;
                announced = true;
            }
            // 글이면 그 글이 보고다 — 수(observedAt)에 자리표를 쓰려면 글로 적는다.
            let text = match report {
                Value::String(text) => text.clone(),
                other => other.to_string(),
            };
            std::fs::write(live.join(format!("{pid}.tabs.json")), fill(&text))
                .map_err(|e| e.to_string())?;
        }
        *recorder.pick.lock().unwrap() = match call.get("pick") {
            Some(Value::String(relative)) => Some(format!(
                "{}\\{}",
                files
                    .ok_or("pick 을 쓰려면 --files 가 있어야 합니다.")?
                    .trim_end_matches(['\\', '/']),
                relative.replace('/', "\\")
            )),
            _ => None,
        };
        let arguments: Vec<Option<String>> = match call.get("args") {
            Some(Value::Array(items)) => items
                .iter()
                .map(|x| match x {
                    Value::Null => None,
                    Value::String(s) => Some(fill(s)),
                    other => Some(fill(&other.to_string())),
                })
                .collect(),
            _ => Vec::new(),
        };
        let outcome = match call.get("direct").and_then(Value::as_str) {
            Some("autoCaptureTick") => {
                bridge.tick_auto_capture(&mut automatic);
                Ok(Value::Null)
            }
            Some("pinClock") => watched
                .open()
                .and_then(|c| {
                    c.execute_batch(crate::script::PIN_CLOCK_SQL)
                        .map_err(pclm_core::Error::Sqlite)
                })
                .map(|()| Value::Null)
                .map_err(|e| e.to_string()),
            Some(direct) => self::direct(direct, &arguments, home.directory(), files, env),
            None => {
                let method = call
                    .get("method")
                    .and_then(Value::as_str)
                    .ok_or("method 가 없습니다.")?;
                bridge.invoke(method, &arguments)
            }
        };
        let line = match &outcome {
            Ok(result) => reply_json(id, true, Some(result), None),
            Err(message) => reply_json(id, false, None, Some(message)),
        };
        replies.push(outcome.ok());
        if bridge.take_restart() {
            recorder.note(&["restart"]);
        }
        if bridge.take_close() {
            recorder.note(&["close"]);
        }
        commands(&live, &pid, &recorder)?;
        out.push_str(&hide_pid(&line, &pid));
        out.push('\n');
    }
    // 다리가 쥔 연결과 열람 사본을 걷은 뒤에 쓴다(C# 은 finally 에서 사본을 걷는다).
    drop(bridge);
    drop(automatic);
    drop(store);
    drop(watched);
    settle_wal(&[file.parent().map(Path::to_path_buf), files.map(PathBuf::from)]);
    if announced {
        // 계기가 적은 연결 · 보고 · 명령 폴더를 걷는다 — 대조할 홈에 남기지 않는다.
        let _ = std::fs::remove_dir_all(&live);
    }
    let target = PathBuf::from(format!("{}.bridge.txt", file.display()));
    std::fs::write(target, out).map_err(|e| e.to_string())?;
    let prompts: String = recorder
        .prompts
        .lock()
        .unwrap()
        .iter()
        .map(|l| format!("{}\n", hide_pid(l, &pid)))
        .collect();
    let target = PathBuf::from(format!("{}.prompts.txt", file.display()));
    std::fs::write(target, prompts).map_err(|e| e.to_string())
}

/// 다리를 거치지 않는 부름 — 업무 홈에서만 받는 요청의 몸을 이 홈에서 견준다(README 「다리」).
fn direct(
    name: &str,
    args: &[Option<String>],
    home: &Path,
    files: Option<&String>,
    env: &dyn Env,
) -> Result<Value, String> {
    let under = |i: usize| -> Result<PathBuf, String> {
        let relative = args
            .get(i)
            .and_then(|a| a.as_deref())
            .ok_or("direct 의 값이 없습니다.")?;
        Ok(PathBuf::from(format!(
            "{}\\{}",
            files
                .ok_or("direct 를 쓰려면 --files 가 있어야 합니다.")?
                .trim_end_matches(['\\', '/']),
            relative.replace('/', "\\")
        )))
    };
    match name {
        "extensionStatus" => {
            let now = parse_to_local(Some(&env.now_utc()), &SystemEnv as &dyn LocalZone)
                .ok_or("지금을 읽지 못했습니다")?;
            extension_status_at(home, now)
        }
        "prepareExtension" => prepare_bundled_extension(&under(0)?, &under(1)?).map(|p| p.to_json()),
        other => Err(format!("모르는 direct 입니다: {other}")),
    }
}

/// 이 프로세스가 확장 호스트인 것처럼 살아 있는 연결을 적는다(`ExtensionPresence` 의 파일).
/// 남은 `-wal` 마다 쓰기 연결을 하나 열어 읽고 닫는다 — 읽기 전용 연결이 마지막에 닫히면 `-wal` · `-shm` 이 남는데, C# 은 그
/// 차례가 연결 풀에 달려 정해져 있지 않다. 두 계기가 같은 걷기를 해 목록이 그 차례에 갈리지 않게 한다(C# `BridgeParity` 의
/// finally 와 같은 일). 자료의 뜻은 바뀌지 않는다.
fn settle_wal(near: &[Option<PathBuf>]) {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.to_string_lossy().ends_with("-wal") {
                out.push(path);
            }
        }
    }
    let mut wals = Vec::new();
    for dir in near.iter().flatten() {
        walk(dir, &mut wals);
    }
    wals.sort();
    wals.dedup();
    for wal in wals {
        let text = wal.to_string_lossy();
        let main = PathBuf::from(&text[..text.len() - "-wal".len()]);
        if !main.is_file() {
            continue;
        }
        if let Ok(connection) = rusqlite::Connection::open(&main) {
            let _ = connection.query_row("SELECT count(*) FROM sqlite_master;", [], |r| r.get::<_, i64>(0));
        }
    }
}

fn announce(live: &Path, pid: &str) -> Result<(), String> {
    std::fs::create_dir_all(live).map_err(|e| e.to_string())?;
    let (seconds, ticks) = pclm_home::win::process_start().map_err(|e| e.to_string())?;
    let entry = serde_json::json!({
        "browser": "Chrome",
        "version": "0.9.0",
        "pid": pid.parse::<i64>().unwrap_or(0),
        "processStartUtc": pclm_home::win::format_round_trip(seconds, ticks),
        "connectedAt": "2026-09-10T09:00:00",
    });
    std::fs::write(live.join(format!("{pid}.json")), entry.to_string()).map_err(|e| e.to_string())
}

/// 다리가 확장에 보낸 명령 — 이름(차례)대로 읽어 적고 지운다.
fn commands(live: &Path, pid: &str, recorder: &Recorder) -> Result<(), String> {
    let folder = live.join(format!("{pid}.cmd"));
    let Ok(listing) = std::fs::read_dir(&folder) else {
        return Ok(());
    };
    let mut names: Vec<PathBuf> = listing.filter_map(|e| e.ok().map(|e| e.path())).collect();
    names.sort();
    for path in names {
        let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        recorder.note(&["command", text.trim_start_matches('\u{feff}')]);
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 자리표를 바꾼다 — 이름 붙은 것(`{$pid}` 따위, [`named`])은 `named` 가, `{$n.경로}`(n 번째 답의 result 에서 `.` 로 이은 열쇠 ·
/// 차례를 따라간 값. 글이면 글, 아니면 JSON 글)는 앞의 답이 낸다. 찾지 못한 자리표는 그대로 둔다. 바꾼 값은 JSON 으로
/// 이스케이프하지 않는다 — 대본은 따옴표 · 역빗금이 없는 값에만 쓴다.
fn fill(text: &str, named: &dyn Fn(&str) -> Option<String>, replies: &[Option<Value>]) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("{$") {
        out.push_str(&rest[..start]);
        let Some(end) = rest[start..].find('}') else {
            out.push_str(&rest[start..]);
            return out;
        };
        let token = &rest[start + 2..start + end];
        match named(token).or_else(|| lookup(token, replies)) {
            Some(v) => out.push_str(&v),
            None => out.push_str(&rest[start..start + end + 1]),
        }
        rest = &rest[start + end + 1..];
    }
    out.push_str(rest);
    out
}

/// 이름 붙은 자리표 — `pid`(이 계기의 프로세스 번호), `mapping`(작업자료의 쓰는 매핑 판), `dataset`(작업자료의 신원),
/// `nowMs`(지금의 유닉스 밀리초 — 자동 수집이 8초 안의 읽기만 받는다), `face:<열쇠>:<열>`(그 칸의 지금 계약면 값 —
/// batchEdit 의 original), `view:<이름>`(그 이름의 맞춤 뷰 식별자 — 식별자 차례로 처음 것). 찾지 못하면 바꾸지 않는다.
fn named(token: &str, pid: &str, mapping: &str, dataset: &str, store: &Store<'_>) -> Option<String> {
    match token {
        "pid" => Some(pid.to_string()),
        "mapping" => Some(mapping.to_string()),
        "dataset" => Some(dataset.to_string()),
        "nowMs" => Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis().to_string())
                .unwrap_or_default(),
        ),
        _ => {
            if let Some(name) = token.strip_prefix("view:") {
                let connection = store.database().open().ok()?;
                return connection
                    .query_row(
                        "SELECT id FROM user_view WHERE name = ?1 ORDER BY id LIMIT 1;",
                        [name],
                        |row| row.get::<_, String>(0),
                    )
                    .ok();
            }
            let (key, column) = token.strip_prefix("face:")?.split_once(':')?;
            let entity = store.resolve(key).ok()??;
            store.face_value(&entity, column).ok()
        }
    }
}

fn lookup(token: &str, replies: &[Option<Value>]) -> Option<String> {
    let mut parts = token.split('.');
    let n: usize = parts.next()?.parse().ok()?;
    let mut value = replies.get(n.checked_sub(1)?)?.as_ref()?;
    for part in parts {
        value = match value {
            Value::Array(items) => items.get(part.parse::<usize>().ok()?)?,
            Value::Object(map) => map.get(part)?,
            _ => return None,
        };
    }
    Some(match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    })
}

/// 답 · 물음에 든 이 계기의 프로세스 번호를 가린다 — 두 계기는 다른 프로세스다.
fn hide_pid(line: &str, pid: &str) -> String {
    line.replace(&format!("\"pid\":{pid}"), "\"pid\":\"<pid>\"")
        .replace(&format!("\"{pid}:"), "\"<pid>:")
        .replace(&format!("{pid}.cmd"), "<pid>.cmd")
}

/// `Path.GetFullPath` — 계기는 현재 폴더를 볼 수 있는 층이다.
fn absolute(path: &str) -> Result<PathBuf, String> {
    std::path::absolute(Path::new(path)).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 자리표는_앞의_답에서_찾는다() {
        let replies = vec![Some(serde_json::json!({ "a": [{ "id": "x1" }], "n": 3 })), None];
        let marks = |token: &str| (token == "pid").then(|| "9".to_string());
        assert_eq!(fill("[\"{$1.a.0.id}\"]", &marks, &replies), "[\"x1\"]");
        assert_eq!(fill("{$1.n}:{$pid}:{$mapping}", &marks, &replies), "3:9:{$mapping}");
        assert_eq!(fill("{$2.x} {$2} {$", &marks, &replies), "{$2.x} {$2} {$");
    }
}

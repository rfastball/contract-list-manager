//! C# `ExtensionPresenceTests` 와 `ErpCaptureTests` 의 확장 상태 몫을 옮긴 것, 그리고 중계(탭 보고·명령 폴더)의 시험.
//! 브라우저 없이 길이 머리 stdio 를 직접 흘려 넣는다. 값은 모두 지어낸 것이다.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use pclm_core::local_time::LocalDateTime;
use pclm_home::win;
use serde_json::{Value, json};

use crate::executable::ExecutableIdentity;
use crate::log::{self, Entry};
use crate::presence::{self, Options, live_path, process_id};
use crate::{relay, status};

struct Root(PathBuf);

impl Root {
    fn new() -> Root {
        let path = std::env::temp_dir().join(format!("pclm-presence-{}", win::new_guid_n()));
        std::fs::create_dir_all(&path).unwrap();
        Root(path)
    }

    fn join(&self, part: &str) -> PathBuf {
        self.0.join(part)
    }
}

impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn frame(value: &Value) -> Vec<u8> {
    let payload = value.to_string().into_bytes();
    let mut out = (payload.len() as u32).to_le_bytes().to_vec();
    out.extend_from_slice(&payload);
    out
}

fn guid() -> String {
    let n = win::new_guid_n();
    format!(
        "{}-{}-{}-{}-{}",
        &n[..8],
        &n[8..12],
        &n[12..16],
        &n[16..20],
        &n[20..]
    )
}

fn presence_frame(browser: &str, version: &str) -> Vec<u8> {
    presence_frame_with(browser, version, json!(2), &guid())
}

fn presence_frame_with(browser: &str, version: &str, protocol: Value, request_id: &str) -> Vec<u8> {
    frame(
        &json!({ "protocolVersion": protocol, "requestId": request_id, "method": "presence",
        "params": { "extensionVersion": version, "browser": browser } }),
    )
}

fn event_frame(args: Value) -> Vec<u8> {
    frame(&json!({ "protocolVersion": 2, "requestId": guid(), "method": "event", "params": args }))
}

fn replies(bytes: &[u8]) -> Vec<Value> {
    let mut out = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let n = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
        out.push(serde_json::from_slice(&bytes[at + 4..at + 4 + n]).unwrap());
        at += 4 + n;
    }
    out
}

fn events(directory: &Path) -> Vec<(String, String, String, String)> {
    log::read(directory, at(1, 1, 1, 0, 0))
        .into_iter()
        .map(|e: Entry| (e.event, e.browser, e.version, e.previous))
        .collect()
}

fn row(
    event: &str,
    browser: &str,
    version: &str,
    previous: &str,
) -> (String, String, String, String) {
    (
        event.into(),
        browser.into(),
        version.into(),
        previous.into(),
    )
}

fn at(year: i64, month: u32, day: u32, hour: u32, minute: u32) -> LocalDateTime {
    LocalDateTime {
        year,
        month,
        day,
        hour,
        minute,
        second: 0,
        ticks: 0,
    }
}

fn add_minutes(t: LocalDateTime, minutes: i64) -> LocalDateTime {
    let total = i64::from(t.hour) * 60 + i64::from(t.minute) + minutes;
    let day = t.add_days(total.div_euclid(1440));
    LocalDateTime {
        hour: (total.rem_euclid(1440) / 60) as u32,
        minute: (total.rem_euclid(60)) as u32,
        ..day
    }
}

fn never() -> impl FnMut(&Value) -> Value {
    |_| panic!("호출되면 안 됨")
}

fn serve_bytes(
    input: Vec<u8>,
    directory: &Path,
    dispatch: impl FnMut(&Value) -> Value,
    options: Options,
) -> Vec<Value> {
    let output = Arc::new(Mutex::new(Vec::new()));
    presence::serve(
        &input[..],
        Arc::clone(&output),
        directory,
        dispatch,
        options,
    )
    .unwrap();
    let bytes = output.lock().unwrap().clone();
    replies(&bytes)
}

fn files_under(directory: &Path) -> Vec<String> {
    fn walk(dir: &Path, out: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                walk(&entry.path(), out);
            } else {
                out.push(entry.file_name().to_string_lossy().into_owned());
            }
        }
    }
    let mut out = Vec::new();
    walk(directory, &mut out);
    out.sort();
    out
}

fn wait_until(limit: Duration, mut done: impl FnMut() -> bool) -> bool {
    let until = Instant::now() + limit;
    while Instant::now() < until {
        if done() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    done()
}

#[test]
fn 상시_연결_인사는_작업자료가_없는_홈에도_연결_파일만_쓰고_포트가_닫히면_지우며_붙음과_끊김을_적는다()
 {
    let root = Root::new();
    let home = pclm_home::Home::new(root.join("빈 홈")).unwrap();
    let directory = home.directory().to_path_buf();
    let live = live_path(&directory, process_id());
    let hello = frame(&json!({ "protocolVersion": 2, "requestId": guid(), "method": "hello" }));
    let mut dispatched = Vec::new();
    let mut seen = Vec::new();
    let mut logged = Vec::new();
    let replies = serve_bytes(
        [presence_frame("Chrome", "0.8.0"), hello].concat(),
        &directory,
        |request| {
            dispatched.push(request["method"].as_str().unwrap().to_string());
            assert!(live.exists(), "포트가 열린 동안은 파일이 있다");
            let state = status::read(&directory, "0.8.0", win::local_now());
            seen.extend(state.live);
            logged.extend(state.log);
            json!({ "ok": true })
        },
        Options::default(),
    );

    assert_eq!(dispatched, ["hello"]);
    assert_eq!(replies[0]["ok"], true);
    assert_eq!(replies[0]["result"]["status"], "present");
    assert_eq!(seen.len(), 1);
    assert_eq!(
        (seen[0].browser.as_str(), seen[0].version.as_str()),
        ("Chrome", "0.8.0")
    );
    assert_eq!(seen[0].connected_at.len(), 19);
    assert_eq!(logged.len(), 1);
    assert_eq!(logged[0].event, log::CONNECTED);

    assert!(!live.exists());
    let contacts = status::read(&directory, "0.8.0", win::local_now()).contacts;
    assert_eq!(contacts.len(), 1);
    assert_eq!(
        (contacts[0].browser.as_str(), contacts[0].version.as_str()),
        ("Chrome", "0.8.0")
    );
    assert_eq!(
        events(&directory),
        [
            row(log::CONNECTED, "Chrome", "0.8.0", ""),
            row(log::DISCONNECTED, "Chrome", "0.8.0", "")
        ]
    );
    assert_eq!(home.read_config().state, pclm_home::ConfigState::Missing);
    assert_eq!(
        files_under(&directory),
        ["extension-contact.json", "extension-log.txt"]
    );
}

#[test]
fn 상시_연결은_모양이_틀린_인사를_적지_않는다() {
    let root = Root::new();
    let directory = root.join("상시 연결");
    let frames = [
        presence_frame("Firefox", "0.8.0"),
        presence_frame("Chrome", "../../x"),
        presence_frame("Chrome", "0.8.0\n"),
        presence_frame("Chrome", "1.2.3.4.5"),
        presence_frame("Chrome", "0.8.0\t끊김"),
        presence_frame_with("Chrome", "0.8.0", json!(1), &guid()),
        presence_frame_with("Chrome", "0.8.0", json!(2.0), &guid()),
        presence_frame_with("Chrome", "0.8.0", json!(2), "아님"),
    ];
    let replies = serve_bytes(frames.concat(), &directory, never(), Options::default());
    assert_eq!(replies.len(), frames.len());
    assert!(replies.iter().all(|r| r["ok"] == false));
    assert!(!presence::live_directory(&directory).exists());
    assert!(!directory.join("extension-contact.json").exists());
    assert!(!log::log_path(&directory).exists());
}

#[test]
fn 겹친_키가_든_쪽지는_읽지_못한_쪽지와_같다() {
    let root = Root::new();
    let directory = root.join("겹친 쪽지");
    std::fs::create_dir_all(presence::live_directory(&directory)).unwrap();
    let pid = process_id();
    std::fs::write(
        live_path(&directory, pid),
        format!(r#"{{"browser":"Edge","version":"0.8.0","pid":{pid},"pid":{pid},"processStartUtc":"2026-01-01T00:00:00Z","connectedAt":"2026-01-01T09:00:00"}}"#),
    )
    .unwrap();
    std::fs::write(
        directory.join("extension-contact.json"),
        r#"{"Chrome":{"version":"0.7.0","at":"2026-01-01T09:00:00"},"Chrome":{"version":"0.7.0","at":"2026-01-01T09:00:00"}}"#,
    )
    .unwrap();
    let state = status::read(&directory, "0.8.0", win::local_now());
    assert!(state.live.is_empty());
    assert!(state.contacts.is_empty());

    status::record_contact(&directory, "0.8.0", "Edge", win::local_now()).unwrap();
    let contacts = status::read(&directory, "0.8.0", win::local_now()).contacts;
    assert_eq!(contacts.len(), 1);
    assert_eq!(contacts[0].browser, "Edge");
}

#[test]
fn 겹친_키가_든_확장_메시지는_읽지_못한_메시지다() {
    let root = Root::new();
    let payload =
        br#"{"protocolVersion":2,"requestId":"r-1","method":"hello","method":"hello","params":{}}"#;
    let mut input = (payload.len() as u32).to_le_bytes().to_vec();
    input.extend_from_slice(payload);
    let replies = serve_bytes(
        input,
        &root.join("겹친 메시지"),
        never(),
        Options::default(),
    );
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0]["ok"], false);
    assert_eq!(replies[0]["error"]["code"], "invalid_json");
}

#[test]
fn 연결_기록을_쓰지_못해도_인사는_성공한다() {
    let root = Root::new();
    let directory = root.join("기록 못 씀");
    std::fs::create_dir_all(log::log_path(&directory)).unwrap(); // 기록 자리에 폴더가 섰다.
    let replies = serve_bytes(
        presence_frame("Chrome", "0.8.0"),
        &directory,
        never(),
        Options::default(),
    );
    assert_eq!(replies[0]["ok"], true);
    let state = status::read(&directory, "0.8.0", win::local_now());
    assert_eq!(state.contacts.len(), 1);
    assert!(state.log.is_empty());
}

#[test]
fn 살아_있는_연결은_그_프로세스가_그때_시작한_그대로_돌_때만_센다() {
    let root = Root::new();
    let directory = root.join("살아 있는 연결");
    let folder = presence::live_directory(&directory);
    std::fs::create_dir_all(&folder).unwrap();
    let (seconds, ticks) = win::process_start().unwrap();
    let entry = |pid: i32, seconds: i64| {
        json!({ "browser": "Edge", "version": "0.8.0", "pid": pid,
            "processStartUtc": win::format_round_trip(seconds, ticks), "connectedAt": "2026-01-01T09:00:00" })
        .to_string()
    };
    let pid = process_id();
    let reused = live_path(&directory, pid);
    let dead = live_path(&directory, i32::MAX);
    std::fs::write(&reused, entry(pid, seconds - 3600)).unwrap();
    std::fs::write(&dead, entry(i32::MAX, seconds)).unwrap();
    std::fs::write(folder.join("깨짐.json"), "{ 깨진").unwrap();
    std::fs::write(folder.join("12.json"), entry(pid, seconds)).unwrap(); // 이름과 번호가 다르다.
    assert!(
        status::read(&directory, "0.8.0", win::local_now())
            .live
            .is_empty()
    );
    assert!(!reused.exists());
    assert!(!dead.exists());
    assert!(folder.join("깨짐.json").exists()); // 모양이 깨진 것은 건너뛰기만 한다.

    std::fs::write(&reused, entry(pid, seconds)).unwrap();
    let live = status::read(&directory, "0.8.0", win::local_now()).live;
    assert_eq!(live.len(), 1);
    assert_eq!(
        (
            live[0].browser.as_str(),
            live[0].version.as_str(),
            live[0].connected_at.as_str()
        ),
        ("Edge", "0.8.0", "2026-01-01T09:00:00")
    );
    assert!(reused.exists());
    presence::forget(&directory);
    assert!(!reused.exists());
    presence::forget(&directory); // 없어도 던지지 않는다.
}

#[test]
fn 연결_기록은_브라우저마다_판이_바뀌면_남기고_오늘_것만_읽힌다() {
    let root = Root::new();
    let directory = root.join("기록");
    let yesterday = at(2026, 10, 4, 18, 0);
    let today = LocalDateTime {
        minute: 30,
        ..at(2026, 10, 5, 8, 0)
    };
    log::record_connected(&directory, "Chrome", "0.7.0", yesterday).unwrap();
    log::record_disconnected(&directory, "Chrome", "0.7.0", add_minutes(yesterday, 2)).unwrap();
    log::record_connected(&directory, "Chrome", "0.8.0", today).unwrap();
    log::record_connected(&directory, "Edge", "0.8.0", add_minutes(today, 1)).unwrap();
    log::record_disconnected(&directory, "Chrome", "0.8.0", add_minutes(today, 27)).unwrap();
    log::record_connected(&directory, "Chrome", "0.8.0", add_minutes(today, 42)).unwrap();
    log::record_connected(&directory, "Firefox", "0.8.0", today).unwrap();

    let got: Vec<_> = log::read(&directory, today.date())
        .into_iter()
        .map(|e| (e.event, e.browser, e.version, e.previous))
        .collect();
    assert_eq!(
        got,
        [
            row(log::UPDATED, "Chrome", "0.8.0", "0.7.0"),
            row(log::CONNECTED, "Chrome", "0.8.0", ""),
            row(log::CONNECTED, "Edge", "0.8.0", ""),
            row(log::DISCONNECTED, "Chrome", "0.8.0", ""),
            row(log::CONNECTED, "Chrome", "0.8.0", ""),
        ]
    );
    let stamps: Vec<_> = status::read(&directory, "0.8.0", add_minutes(today, 120))
        .log
        .into_iter()
        .map(|e| e.at)
        .collect();
    assert_eq!(
        stamps,
        [
            "2026-10-05T08:30:00",
            "2026-10-05T08:30:00",
            "2026-10-05T08:31:00",
            "2026-10-05T08:57:00",
            "2026-10-05T09:12:00"
        ]
    );
    assert_eq!(events(&directory).len(), 7);
    assert!(log::read(&root.join("없는 홈"), at(1, 1, 1, 0, 0)).is_empty());
}

#[test]
fn 연결_기록은_이레와_500줄을_넘으면_스스로_줄이고_깨진_줄을_걷는다() {
    let root = Root::new();
    let directory = root.join("줄이기");
    std::fs::create_dir_all(&directory).unwrap();
    let now = at(2026, 10, 5, 12, 0);
    let mut lines = vec![
        "깨진 줄".to_string(),
        "2026-10-05T09:00:00\tconnected\tChrome".to_string(),
    ];
    for i in 0..10 {
        lines.push(format!(
            "{}\tconnected\tEdge\t0.7.0\t",
            add_minutes(now.add_days(-8), i).second_text()
        ));
    }
    for i in 0..600 {
        lines.push(format!(
            "{}\tdisconnected\tChrome\t0.8.0\t",
            add_minutes(now.add_days(-2), i).second_text()
        ));
    }
    // File.WriteAllLines — 줄마다 CR LF.
    let text: String = lines.iter().map(|l| format!("{l}\r\n")).collect();
    std::fs::write(log::log_path(&directory), text).unwrap();

    log::record_connected(&directory, "Edge", "0.8.0", now).unwrap();
    let kept_text = std::fs::read_to_string(log::log_path(&directory)).unwrap();
    let kept: Vec<&str> = kept_text.lines().collect();
    assert_eq!(kept.len(), log::MAX_LINES);
    assert!(kept.iter().all(|l| l.split('\t').count() == 5));
    assert!(kept[0] >= now.add_days(-7).second_text().as_str());
    let all = events(&directory);
    assert_eq!(
        all[all.len() - 2..],
        [
            row(log::UPDATED, "Edge", "0.8.0", "0.7.0"),
            row(log::CONNECTED, "Edge", "0.8.0", "")
        ]
    );

    // 줄일 것이 없으면 덧붙이기만 한다 — 앞의 줄은 글자 하나 바뀌지 않는다.
    let head: String = kept_text
        .split('\n')
        .take(10)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    std::fs::write(log::log_path(&directory), &head).unwrap();
    log::record_disconnected(&directory, "Edge", "0.8.0", add_minutes(now, 1)).unwrap();
    assert!(
        std::fs::read_to_string(log::log_path(&directory))
            .unwrap()
            .starts_with(&head)
    );
}

#[test]
fn 여러_호스트가_함께_써도_줄이_섞이지_않는다() {
    let root = Root::new();
    let directory = root.join("함께 씀");
    let now = at(2026, 10, 5, 9, 0);
    let workers: Vec<_> = (0..8)
        .map(|worker| {
            let directory = directory.clone();
            std::thread::spawn(move || {
                let browser = if worker % 2 == 0 { "Chrome" } else { "Edge" };
                for i in 0..25 {
                    let t = LocalDateTime { second: i, ..now };
                    if i % 2 == 0 {
                        log::record_connected(&directory, browser, "0.8.0", t).unwrap();
                    } else {
                        log::record_disconnected(&directory, browser, "0.8.0", t).unwrap();
                    }
                }
            })
        })
        .collect();
    for w in workers {
        w.join().unwrap();
    }
    let text = std::fs::read_to_string(log::log_path(&directory)).unwrap();
    assert_eq!(text.lines().count(), 200);
    assert_eq!(log::read(&directory, now.date()).len(), 200);
}

/// 지켜보는 호스트를 실마리에 띄운다 — 포트(쓰는 쪽)는 열린 채, 떠나면 `left` 가 선다.
fn spawn_host(
    directory: PathBuf,
    watch: Vec<ExecutableIdentity>,
    registered: Option<Arc<dyn Fn() -> std::io::Result<bool> + Send + Sync>>,
) -> (
    std::io::PipeWriter,
    Arc<AtomicBool>,
    std::thread::JoinHandle<()>,
) {
    let (reader, writer) = std::io::pipe().unwrap();
    let left = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&left);
    let host = std::thread::spawn(move || {
        let options = Options {
            watch,
            interval: Duration::from_millis(20),
            leave: Arc::new(move || flag.store(true, Ordering::SeqCst)),
            registered,
        };
        presence::serve(
            reader,
            Arc::new(Mutex::new(Vec::new())),
            &directory,
            never(),
            options,
        )
        .unwrap();
    });
    (writer, left, host)
}

#[test]
fn 상시_연결_호스트는_제_실행_파일이_비켜지면_끊김을_적고_떠나고_수집_호스트는_떠나지_않는다() {
    let root = Root::new();
    let directory = root.join("비키기");
    let exe = root.join("계약목록.exe");
    std::fs::write(&exe, "옛 판").unwrap();
    let watch = vec![ExecutableIdentity::of(&exe).unwrap()];
    let live = live_path(&directory, process_id());

    // 수집 호스트: 인사 없이 요청 하나. 그사이 exe 가 비켜져도 떠나지 않는다.
    let collector_left = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&collector_left);
    let request = frame(&json!({ "protocolVersion": 2, "requestId": guid(), "method": "hello" }));
    let moved = root.join("계약목록.exe.잠깐");
    serve_bytes(
        request,
        &directory,
        |_| {
            std::fs::rename(&exe, &moved).unwrap();
            std::thread::sleep(Duration::from_millis(200));
            std::fs::rename(&moved, &exe).unwrap();
            json!({ "ok": true })
        },
        Options {
            watch: watch.clone(),
            interval: Duration::from_millis(20),
            leave: Arc::new(move || flag.store(true, Ordering::SeqCst)),
            registered: None,
        },
    );
    assert!(!collector_left.load(Ordering::SeqCst));
    assert!(!log::log_path(&directory).exists());

    // 상시 연결 호스트: 포트는 열린 채. exe 를 비키면 떠난다.
    let (mut browser, left, host) = spawn_host(directory.clone(), watch, None);
    browser.write_all(&presence_frame("Edge", "0.8.0")).unwrap();
    assert!(wait_until(Duration::from_secs(5), || live.exists()));
    std::thread::sleep(Duration::from_millis(100));
    assert!(!left.load(Ordering::SeqCst), "제자리에 있는 동안은 머문다");

    std::fs::rename(&exe, root.join("계약목록.old-20261005091230.exe")).unwrap();
    assert!(wait_until(Duration::from_secs(5), || left.load(Ordering::SeqCst)));
    assert!(!live.exists());
    let got: Vec<_> = events(&directory).into_iter().map(|e| e.0).collect();
    assert_eq!(got, [log::CONNECTED, log::DISCONNECTED]);

    // 그 뒤 포트가 닫혀도 끊김을 두 번 적지 않는다.
    drop(browser);
    host.join().unwrap();
    assert_eq!(events(&directory).len(), 2);
}

#[test]
fn 상시_연결_호스트는_브라우저_등록이_다른_exe_로_옮겨_가면_떠난다() {
    let root = Root::new();
    let directory = root.join("등록 옮김");
    let ours = Arc::new(AtomicBool::new(true));
    let probe = Arc::clone(&ours);
    let (mut browser, left, host) = spawn_host(
        directory.clone(),
        Vec::new(),
        Some(Arc::new(move || Ok(probe.load(Ordering::SeqCst)))),
    );
    browser
        .write_all(&presence_frame("Chrome", "0.8.0"))
        .unwrap();
    assert!(wait_until(Duration::from_secs(5), || live_path(
        &directory,
        process_id()
    )
    .exists()));
    std::thread::sleep(Duration::from_millis(100));
    assert!(!left.load(Ordering::SeqCst));

    ours.store(false, Ordering::SeqCst);
    assert!(wait_until(Duration::from_secs(5), || left.load(Ordering::SeqCst)));
    let got: Vec<_> = events(&directory).into_iter().map(|e| e.0).collect();
    assert_eq!(got, [log::CONNECTED, log::DISCONNECTED]);
    drop(browser);
    host.join().unwrap();
}

#[test]
fn 등록을_읽다_실패하면_떠나지_않는다() {
    let root = Root::new();
    let directory = root.join("등록 못 읽음");
    let (mut browser, left, host) = spawn_host(
        directory.clone(),
        Vec::new(),
        Some(Arc::new(|| {
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
        })),
    );
    browser
        .write_all(&presence_frame("Chrome", "0.8.0"))
        .unwrap();
    std::thread::sleep(Duration::from_millis(200));
    assert!(!left.load(Ordering::SeqCst));
    drop(browser);
    host.join().unwrap();
}

#[test]
fn 확장의_오류와_풀림은_인사_뒤에만_코드만_연결_기록에_남는다() {
    let root = Root::new();
    let directory = root.join("홈");
    let input = [
        event_frame(json!({ "kind": "error", "code": "unconfirmed" })),
        presence_frame("Chrome", "0.8.0"),
        event_frame(json!({ "kind": "error", "code": "unconfirmed", "title": "시험 계약 건명", "number": "R26TA00000001-00" })),
        event_frame(json!({ "kind": "error", "code": "모르는 코드" })),
        event_frame(json!({ "kind": "nonsense" })),
        event_frame(json!({ "kind": "recovered" })),
    ]
    .concat();
    let replies = serve_bytes(input, &directory, never(), Options::default());
    let oks: Vec<_> = replies.iter().map(|r| r["ok"] == true).collect();
    assert_eq!(oks, [false, true, true, false, false, true]);
    assert_eq!(
        events(&directory),
        [
            row(log::CONNECTED, "Chrome", "0.8.0", ""),
            row(log::ERROR, "Chrome", "0.8.0", "unconfirmed"),
            row(log::RECOVERED, "Chrome", "0.8.0", ""),
            row(log::DISCONNECTED, "Chrome", "0.8.0", ""),
        ]
    );
    let text = std::fs::read_to_string(log::log_path(&directory)).unwrap();
    assert!(!text.contains("시험 계약"));
    assert!(!text.contains("R26TA"));
}

#[test]
fn 오늘의_오류와_오늘_전의_마지막_오류를_가르고_연결_기록에는_붙음_끊김_판_바뀜만_낸다() {
    let root = Root::new();
    let directory = root.join("홈");
    let today = LocalDateTime {
        hour: 9,
        ..win::local_now().date()
    };
    let yesterday = LocalDateTime {
        hour: 16,
        ..today.add_days(-1)
    };
    log::record_connected(&directory, "Chrome", "0.8.0", yesterday).unwrap();
    let event = |browser: &str, kind: &str, code: Option<&str>, t: LocalDateTime| {
        log::record_event(&directory, browser, "0.8.0", kind, code, t).unwrap()
    };
    assert!(event(
        "Chrome",
        log::ERROR,
        Some("unconfirmed"),
        add_minutes(yesterday, 1)
    ));
    assert!(event(
        "Edge",
        log::ERROR,
        Some("connection"),
        add_minutes(yesterday, 2)
    ));
    assert!(event(
        "Chrome",
        log::RECOVERED,
        None,
        add_minutes(yesterday, 3)
    ));
    log::record_connected(&directory, "Chrome", "0.8.0", today).unwrap();
    assert!(event(
        "Chrome",
        log::ERROR,
        Some("setup"),
        add_minutes(today, 5)
    ));
    assert!(event(
        "Chrome",
        log::ERROR,
        Some("rejected"),
        add_minutes(today, 9)
    ));
    assert!(!event(
        "Chrome",
        log::ERROR,
        Some("Bad Code!"),
        add_minutes(today, 10)
    ));
    assert!(!event(
        "Chrome",
        log::CONNECTED,
        None,
        add_minutes(today, 10)
    ));
    // 앞으로 더할 사건도 읽는 쪽은 건너뛴다.
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(log::log_path(&directory))
        .unwrap();
    writeln!(
        file,
        "{}\tfuture\tChrome\t0.8.0\t",
        add_minutes(today, 11).second_text()
    )
    .unwrap();
    drop(file);

    let later = add_minutes(today, 60);
    let (errors, earlier) = log::problems(&directory, later);
    let got: Vec<_> = errors
        .iter()
        .map(|e| (e.code.as_str(), e.recovered))
        .collect();
    assert_eq!(got, [("setup", false), ("rejected", false)]);
    let earlier = earlier.unwrap();
    assert_eq!(
        (
            earlier.browser.as_str(),
            earlier.code.as_str(),
            earlier.recovered
        ),
        ("Edge", "connection", false)
    );
    let state = status::read(&directory, "0.8.0", later);
    let got: Vec<_> = state.log.iter().map(|e| e.event.as_str()).collect();
    assert_eq!(got, [log::CONNECTED]);
    assert_eq!(state.errors.len(), 2);
    assert_eq!(state.past_error.unwrap().code, "connection");

    let other = root.join("다른 홈");
    log::record_event(
        &other,
        "Chrome",
        "0.8.0",
        log::ERROR,
        Some("unconfirmed"),
        yesterday,
    )
    .unwrap();
    log::record_event(
        &other,
        "Chrome",
        "0.8.0",
        log::RECOVERED,
        None,
        add_minutes(yesterday, 1),
    )
    .unwrap();
    let (today_errors, earlier) = log::problems(&other, today);
    assert!(earlier.unwrap().recovered);
    assert!(today_errors.is_empty());
}

// ── 확장 상태(C# ErpCaptureTests) ──────────────────────────────

#[test]
fn 확장_상태는_풀어_둔_판과_브라우저별_마지막_연결을_보인다() {
    let root = Root::new();
    let directory = root.join("상태");
    let empty = status::read(&directory, "0.4.1", win::local_now());
    assert!(!empty.prepared);
    assert_eq!(empty.disk_version, None);
    assert!(empty.contacts.is_empty());
    assert_eq!(empty.embedded_version, "0.4.1");

    std::fs::create_dir_all(directory.join("extension")).unwrap();
    std::fs::write(
        directory.join("extension").join("manifest.json"),
        r#"{"version":"0.4.0"}"#,
    )
    .unwrap();
    std::fs::write(directory.join("extension-contact.json"), "깨진 파일").unwrap();
    status::record_contact(&directory, "0.4.0", "Chrome", win::local_now()).unwrap();
    let state = status::read(&directory, "0.4.1", win::local_now());
    assert!(state.prepared);
    assert_eq!(state.disk_version.as_deref(), Some("0.4.0"));
    assert_eq!(state.contacts.len(), 1);
    assert_eq!(
        (
            state.contacts[0].browser.as_str(),
            state.contacts[0].version.as_str()
        ),
        ("Chrome", "0.4.0")
    );
    assert_eq!(state.contacts[0].at.len(), 19);
}

#[test]
fn 마지막_연결은_다른_칸을_그대로_두고_그_브라우저만_바꾼다() {
    let root = Root::new();
    let directory = root.join("연결");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("extension-contact.json"),
        r#"{"Edge":{"version":"0.4.0","at":"2026-01-01T09:00:00"},"x":1.50,"기타":{"version":"1","at":"t"}}"#,
    )
    .unwrap();
    status::record_contact(&directory, "0.4.1", "Edge", at(2026, 2, 3, 4, 5)).unwrap();
    // 자리는 그대로, 수는 원문 그대로, 비 ASCII 열쇠는 \uXXXX(인자 없는 ToJsonString).
    let text = std::fs::read_to_string(directory.join("extension-contact.json")).unwrap();
    let expected = r#"{"Edge":{"version":"0.4.1","at":"2026-02-03T04:05:00"},"x":1.50,"@uAE30@uD0C0":{"version":"1","at":"t"}}"#
        .replace('@', "\\");
    assert_eq!(text, expected);
    // 모양이 틀린 것은 적지 않는다.
    status::record_contact(&directory, "../x", "Edge", at(2026, 2, 3, 4, 5)).unwrap();
    assert_eq!(
        std::fs::read_to_string(directory.join("extension-contact.json")).unwrap(),
        expected
    );
}

// ── 중계(ADR-036) ──────────────────────────────────────────────

#[test]
fn 탭_보고는_인사_뒤에만_받고_모양만_보며_씨샵과_같은_꼴로_적는다() {
    let root = Root::new();
    let directory = root.join("중계");
    let report = json!({
        "tabs": [{ "tabId": 7, "state": "supported", "menu": "01179", "title": "지어낸 화면",
            "screenRows": [{ "group": "기본", "label": "공고명", "text": "시험 공고", "source": "dom" }],
            "snapshot": serde_json::from_str::<Value>(r#"{"n":1.50}"#).unwrap() }], // 수는 원문 그대로.
        "front": 7, "frontAt": 1_700_000_000_000_i64, "settings": { "siteAccess": true, "launcher": "x" },
    });
    let tabs = |params: Value| {
        frame(
            &json!({ "protocolVersion": 2, "requestId": guid(), "method": "tabs", "params": params }),
        )
    };
    let pid = process_id();
    let seen = Arc::new(Mutex::new(None::<String>));
    let probe = Arc::clone(&seen);
    let directory_in = directory.clone();
    let input = [
        tabs(report.clone()),
        presence_frame("Edge", "0.9.0"),
        tabs(report.clone()),
        tabs(json!({ "tabs": [{ "tabId": 1.5, "state": "supported" }] })),
        tabs(json!({ "tabs": [{ "tabId": 1, "state": "모름" }] })),
        tabs(json!({ "tabs": [{ "tabId": 1, "state": "error", "menu": "1179" }] })),
        tabs(json!({ "tabs": [{ "tabId": 1, "state": "error", "screenRows": [{ "group": "", "label": "", "text": "" }] }] })),
        tabs(json!({ "tabs": [], "settings": { "siteAccess": "yes" } })),
        tabs(json!({ "nottabs": [] })),
        frame(&json!({ "protocolVersion": 2, "requestId": guid(), "method": "tabs" })),
        frame(&json!({ "protocolVersion": 2, "requestId": guid(), "method": "check" })),
    ]
    .concat();
    let replies = serve_bytes(
        input,
        &directory,
        move |_| {
            *probe.lock().unwrap() =
                std::fs::read_to_string(relay::tabs_path(&directory_in, pid)).ok();
            json!({ "ok": true })
        },
        Options::default(),
    );
    let oks: Vec<_> = replies.iter().map(|r| r["ok"] == true).collect();
    assert_eq!(
        oks,
        [
            false, true, true, false, false, false, false, false, false, false, true
        ]
    );
    assert_eq!(
        replies[0]["error"]["message"],
        "인사 전에는 탭 보고를 받지 않습니다."
    );
    assert_eq!(replies[2]["result"]["status"], "received");
    assert_eq!(replies[9]["error"]["message"], "탭 보고가 비었습니다.");
    let written = seen
        .lock()
        .unwrap()
        .clone()
        .expect("포트가 열린 동안 보고가 있다");
    let expected = format!(
        r#"{{"pid":{pid},"receivedAt":"{{AT}}","front":7,"frontAt":1700000000000,"tabs":[{{"tabId":7,"state":"supported","menu":"01179","title":"@uC9C0@uC5B4@uB0B8 @uD654@uBA74","screenRows":[{{"group":"@uAE30@uBCF8","label":"@uACF5@uACE0@uBA85","text":"@uC2DC@uD5D8 @uACF5@uACE0","source":"dom"}}],"snapshot":{{"n":1.50}}}}],"settings":{{"siteAccess":true}}}}"#
    )
    .replace('@', "\\");
    let at = &written[written.find("receivedAt\":\"").unwrap() + 13..][..19];
    assert_eq!(written, expected.replace("{AT}", at));
    // 끝나면 보고와 명령 폴더를 치운다.
    assert!(!relay::tabs_path(&directory, pid).exists());
    assert!(!relay::command_directory(&directory, pid).exists());
}

#[test]
fn 명령_폴더의_명령은_차례대로_넘기고_틀린_것은_지우기만_한다() {
    let root = Root::new();
    let directory = root.join("명령");
    let pid = 4242;
    let folder = relay::command_directory(&directory, pid);
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("0002.json"), r#"{"command":"read","tabId":3}"#).unwrap();
    std::fs::write(
        folder.join("0001.json"),
        r#"{"command":"focusTab","tabId":9}"#,
    )
    .unwrap();
    std::fs::write(folder.join("0003.json"), r#"{"command":"openExtensions"}"#).unwrap();
    std::fs::write(folder.join("0004.json"), r#"{"command":"focusTab"}"#).unwrap();
    std::fs::write(
        folder.join("0005.json"),
        r#"{"command":"read","tabId":1.5}"#,
    )
    .unwrap();
    std::fs::write(folder.join("0006.json"), r#"{"command":"close","tabId":1}"#).unwrap();
    std::fs::write(
        folder.join("0007.json"),
        r#"{"command":"read","command":"read","tabId":1}"#,
    )
    .unwrap();
    std::fs::write(folder.join("0008.json"), "{ 깨진").unwrap();
    std::fs::write(folder.join("0009.JSON"), r#"{"command":"read","tabId":1}"#).unwrap();
    std::fs::write(
        folder.join("0010.json.tmp"),
        r#"{"command":"read","tabId":1}"#,
    )
    .unwrap();

    let delivered = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&delivered);
    let watcher = relay::Watcher::new(
        &directory,
        pid,
        Box::new(move |command| {
            sink.lock().unwrap().push(command.to_string());
            Ok(())
        }),
        Some(Duration::from_millis(50)),
    )
    .unwrap();
    assert_eq!(
        *delivered.lock().unwrap(),
        [
            r#"{"protocolVersion":2,"command":"focusTab","tabId":9}"#,
            r#"{"protocolVersion":2,"command":"read","tabId":3}"#,
            r#"{"protocolVersion":2,"command":"openExtensions"}"#,
        ]
    );
    let mut left: Vec<_> = std::fs::read_dir(&folder)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    left.sort();
    assert_eq!(left, ["0009.JSON", "0010.json.tmp"]);

    // 나중에 놓인 명령은 변화 알림이나 다시 보기로 줍는다.
    std::fs::write(
        folder.join("0011.json"),
        r#"{"command":"export","tabId":2}"#,
    )
    .unwrap();
    assert!(wait_until(Duration::from_secs(5), || delivered
        .lock()
        .unwrap()
        .len()
        == 4));
    assert_eq!(
        delivered.lock().unwrap()[3],
        r#"{"protocolVersion":2,"command":"export","tabId":2}"#
    );
    drop(watcher);
    std::fs::write(
        folder.join("0012.json"),
        r#"{"command":"export","tabId":2}"#,
    )
    .unwrap();
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(
        delivered.lock().unwrap().len(),
        4,
        "멈춘 뒤에는 넘기지 않는다"
    );
}

#[test]
fn 인사를_받은_호스트는_창이_놓은_명령을_포트로_밀어_보낸다() {
    let root = Root::new();
    let directory = root.join("밀어 보내기");
    let (reader, mut browser) = std::io::pipe().unwrap();
    let output = Arc::new(Mutex::new(Vec::new()));
    let host = {
        let (directory, output) = (directory.clone(), Arc::clone(&output));
        std::thread::spawn(move || {
            presence::serve(reader, output, &directory, never(), Options::default()).unwrap();
        })
    };
    browser
        .write_all(&presence_frame("Chrome", "0.9.0"))
        .unwrap();
    assert!(wait_until(Duration::from_secs(5), || {
        relay::read_tabs(&directory).is_empty() && !presence::read_live(&directory).is_empty()
    }));
    assert!(relay::send(&directory, process_id(), "focusTab", Some(5)).unwrap());
    assert!(relay::send(&directory, process_id(), "openExtensions", None).unwrap());
    assert!(relay::send(&directory, process_id(), "focusTab", None).is_err());
    assert!(relay::send(&directory, process_id(), "close", Some(1)).is_err());
    assert!(!relay::send(&directory, i32::MAX, "read", Some(1)).unwrap());
    assert!(wait_until(Duration::from_secs(5), || replies(
        &output.lock().unwrap()
    )
    .len()
        == 3));
    let got = replies(&output.lock().unwrap());
    assert_eq!(got[0]["result"]["status"], "present");
    assert_eq!(
        got[1],
        json!({ "protocolVersion": 2, "command": "focusTab", "tabId": 5 })
    );
    assert_eq!(
        got[2],
        json!({ "protocolVersion": 2, "command": "openExtensions" })
    );
    drop(browser);
    host.join().unwrap();
}

/// C# `MirrorTests.Live` — 이 프로세스의 상시 연결 파일. `hours_ago` 만큼 일찍 시작한 것으로 적으면 같은 번호를 다른 프로세스가
/// 물려받은 것(끝난 호스트)이 된다.
fn write_live(directory: &Path, hours_ago: i64) {
    let (seconds, ticks) = win::process_start().unwrap();
    let pid = process_id();
    std::fs::create_dir_all(presence::live_directory(directory)).unwrap();
    std::fs::write(
        live_path(directory, pid),
        json!({ "browser": "Chrome", "version": "0.8.0", "pid": pid,
            "processStartUtc": win::format_round_trip(seconds - hours_ago * 3600, ticks),
            "connectedAt": "2026-10-05T09:00:00" })
        .to_string(),
    )
    .unwrap();
}

fn files_in(directory: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(directory)
        .map(|entries| {
            entries
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

/// C# `MirrorTests.탭_보고는_살아_있는_호스트의_것만_읽히고_끝나면_치워진다` — 상시 연결이 없으면 적은 보고도 산 것으로 세지 않고,
/// 모양이 틀린 보고는 있던 것을 그대로 두며, 같은 번호를 다른 프로세스가 물려받았으면(시작 시각이 다르다) 연결 파일과 함께
/// 보고·명령 폴더를 거둔다.
#[test]
fn 탭_보고는_살아_있는_호스트의_것만_읽히고_끝나면_치워진다() {
    let root = Root::new();
    let home = root.join("홈");
    let pid = process_id();
    let report = json!({ "front": 7, "frontAt": 1, "tabs": [
        { "tabId": 7, "windowId": 1, "title": "나라장터", "state": "supported", "screen": "계약 상세", "readAt": "09:12",
          "snapshot": { "profile": "g2b-contract-v1" } },
        { "tabId": 8, "state": "unsupported", "screen": "개찰 결과" } ] });

    assert_eq!(
        relay::write_tabs(&home, pid, &report, win::local_now()).unwrap(),
        None
    );
    assert!(relay::read_tabs(&home).is_empty());

    write_live(&home, 0);
    let read = relay::read_tabs(&home);
    assert_eq!(read.len(), 1);
    assert_eq!((read[0].pid, read[0].browser.as_str()), (pid, "Chrome"));
    assert_eq!(read[0].report["front"], 7);
    let ids: Vec<i64> = read[0].report["tabs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["tabId"].as_i64().unwrap())
        .collect();
    assert_eq!(ids, [7, 8]);
    assert_eq!(
        read[0].report["tabs"][0]["snapshot"]["profile"],
        "g2b-contract-v1"
    );
    assert!(
        !files_in(&presence::live_directory(&home))
            .iter()
            .any(|n| n.ends_with(".tmp"))
    );

    // 모양이 틀린 보고는 적지 않는다 — 있던 것을 그대로 둔다.
    let bad_id = json!({ "tabs": [{ "tabId": "일곱", "state": "supported" }] });
    let bad_state = json!({ "tabs": [{ "tabId": 1, "state": "아무거나" }] });
    for bad in [bad_id, bad_state] {
        assert!(
            relay::write_tabs(&home, pid, &bad, win::local_now())
                .unwrap()
                .is_some()
        );
    }
    assert_eq!(
        relay::read_tabs(&home)[0].report["tabs"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    // 같은 번호를 다른 프로세스가 물려받았으면 끝난 호스트다 — 연결 파일과 함께 보고·명령 폴더를 거둔다.
    assert!(relay::send(&home, pid, "read", Some(7)).unwrap());
    write_live(&home, 5);
    assert!(relay::read_tabs(&home).is_empty());
    assert!(!relay::tabs_path(&home, pid).exists());
    assert!(!relay::command_directory(&home, pid).exists());
}

/// C# `MirrorTests.명령은_차례대로_넘기고_모양이_틀린_것은_버리며_끝난_호스트에는_놓지_않는다` 와
/// `설정_명령은_탭_없이_가고_모양이_틀리면_놓지_않는다` 의 놓는 쪽 — 아무도 집어 가지 않으면 폴더도 짓지 않고, 모르는 명령·걷은
/// 설정 명령(ADR-042)·탭이 틀린 명령은 놓지 않는다. 넘기는 쪽(차례·버리기·겹친 키)은
/// `명령_폴더의_명령은_차례대로_넘기고_틀린_것은_지우기만_한다` 가 본다.
#[test]
fn 살아_있지_않은_호스트에는_명령을_놓지_않고_모르는_명령은_거절한다() {
    let root = Root::new();
    let home = root.join("홈");
    let pid = process_id();
    assert!(!relay::send(&home, pid, "focusTab", Some(3)).unwrap()); // 아무도 집어 가지 않는다.
    assert!(!relay::command_directory(&home, pid).exists());
    assert!(relay::send(&home, pid, "deleteEverything", Some(3)).is_err());

    write_live(&home, 0);
    for refused in ["setPanelMode", "openShortcuts", "focusTab"] {
        assert!(relay::send(&home, pid, refused, None).is_err(), "{refused}");
    }
    assert!(relay::send(&home, pid, "openExtensions", Some(3)).is_err());
    assert!(relay::send(&home, pid, "focusTab", Some(3)).unwrap());
    assert!(relay::send(&home, pid, "read", Some(4)).unwrap());
    assert!(relay::send(&home, pid, "export", Some(5)).unwrap());
    assert!(relay::send(&home, pid, "openExtensions", None).unwrap());
    assert_eq!(files_in(&relay::command_directory(&home, pid)).len(), 4);
}

/// C# `MirrorTests.탭_보고의_화면_줄과_확장_설정은_모양만_보고_받는다`.
#[test]
fn 탭_보고의_화면_줄과_확장_설정은_모양만_보고_받는다() {
    let root = Root::new();
    let home = root.join("홈");
    let pid = process_id();
    write_live(&home, 0);
    let row = |text: Value| json!({ "group": "기본", "label": "계약명", "text": text, "source": "ctrtNm" });
    let of = |tab: Value, settings: Value| json!({ "front": 1, "frontAt": 1, "tabs": [tab], "settings": settings });
    let write = |report: Value| relay::write_tabs(&home, pid, &report, win::local_now()).unwrap();
    let read = || relay::read_tabs(&home)[0].report.clone();

    assert_eq!(
        write(of(
            json!({ "tabId": 1, "state": "supported", "screenRows": [row(json!("시험"))] }),
            json!({ "siteAccess": true })
        )),
        None
    );
    assert_eq!(read()["settings"]["siteAccess"], true);
    assert_eq!(read()["tabs"][0]["screenRows"][0]["text"], "시험");

    let too_many: Vec<Value> = (0..=relay::MAX_SCREEN_ROWS)
        .map(|_| row(json!("x")))
        .collect();
    for refused in [
        of(
            json!({ "tabId": 1, "state": "supported", "screenRows": [row(json!(7))] }),
            Value::Null,
        ),
        of(
            json!({ "tabId": 1, "state": "supported", "screenRows": "줄" }),
            Value::Null,
        ),
        of(
            json!({ "tabId": 1, "state": "supported", "screenRows": too_many }),
            Value::Null,
        ),
        of(
            json!({ "tabId": 1, "state": "reading" }),
            json!({ "siteAccess": "예" }),
        ),
    ] {
        assert!(write(refused).is_some());
    }
    // 메뉴 번호는 빈 글이거나 숫자 다섯만.
    assert_eq!(
        write(of(
            json!({ "tabId": 1, "state": "unsupported", "menu": "01175" }),
            Value::Null
        )),
        None
    );
    assert_eq!(read()["tabs"][0]["menu"], "01175");
    assert_eq!(
        write(of(
            json!({ "tabId": 1, "state": "unsupported", "menu": "" }),
            Value::Null
        )),
        None
    );
    assert!(
        write(of(
            json!({ "tabId": 1, "state": "unsupported", "menu": "<b>메뉴</b>" }),
            Value::Null
        ))
        .is_some()
    );
    assert!(
        write(of(
            json!({ "tabId": 1, "state": "unsupported", "menu": 1175 }),
            Value::Null
        ))
        .is_some()
    );
    // 옛 확장(0.9 까지)이 함께 싣던 수집기 띄우는 방식·단축키는 받되 적지 않는다(ADR-042) — 홈에 임의 글이 남지 않는다.
    assert_eq!(
        write(of(
            json!({ "tabId": 1, "state": "reading" }),
            json!({ "panelMode": "always", "shortcut": "<script>", "siteAccess": false })
        )),
        None
    );
    let old = read()["settings"].clone();
    assert_eq!(old["siteAccess"], false);
    assert!(old.get("panelMode").is_none() && old.get("shortcut").is_none());
    assert!(
        write(of(
            json!({ "tabId": 1, "state": "reading" }),
            json!({ "shortcut": "" })
        ))
        .is_some()
    );
    // 설정 없는 보고(옛 확장)도 받는다.
    assert_eq!(
        write(json!({ "front": 1, "frontAt": 1, "tabs": [{ "tabId": 1, "state": "reading" }] })),
        None
    );
    assert!(read()["settings"].is_null());
}

//! C# `WorkfileSwitchTests` — 작업자료 바꾸기(ADR-032): 옮기기·다른 작업자료 쓰기·스냅샷에서 새로·새 계약자료·백업.
//!
//! 지키는 것은 둘이다. 바꾸는 사이 확장이 저장한 것이 **사람이 다음에 여는 자료에 있거나, 거절되거나** 둘 중 하나여야
//! 하고, 바꾸다 멈추면 **쪽지도 파일도 그대로**여야 한다. 홈은 모두 임시 폴더다. 값은 전부 지어낸 것이다.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use pclm_core::erp::capture::{CaptureInput, ErpCapture};
use pclm_core::erp::{mapping, tools};
use pclm_core::storage::{pclm_file, schema, views};
use pclm_core::{Access, Database, Error, PclmKind, PclmRole};
use serde_json::{Value, json};

use super::home_tests::{Root, count, hash, info, legacy, work};
use super::*;

struct Fixture {
    root: Root,
    home: Home,
    current: Database,
}

impl Fixture {
    fn new() -> Fixture {
        let root = Root::new("switch");
        let home = Home::new(root.join("홈")).unwrap();
        let HomeState::Ready { database, .. } = home.resolve().unwrap() else {
            panic!("처음 실행은 연다");
        };
        database
            .open()
            .unwrap()
            .execute_batch(
                "INSERT INTO request_series (request_base) VALUES ('R-지어냄');
                 INSERT INTO request (request_base, seq, title, updated_at) VALUES ('R-지어냄', '00', '지어낸 접수', 't');
                 INSERT INTO contract_series (contract_base) VALUES ('C-지어냄');
                 INSERT INTO contract (contract_base, seq, title, updated_at) VALUES ('C-지어냄', '00', '지어낸 계약', 't');
                 INSERT INTO plan (request_number, item_name, source_name, imported_at) VALUES ('P-지어냄', '지어낸 품명', '지어낸.xlsx', 't');",
            )
            .unwrap();
        Fixture {
            root,
            home,
            current: database,
        }
    }

    fn join(&self, part: &str) -> PathBuf {
        self.root.join(part)
    }

    fn config_bytes(&self) -> Vec<u8> {
        std::fs::read(self.home.config_path()).unwrap()
    }

    fn current_id(&self) -> String {
        info(self.current.path()).dataset_id.unwrap()
    }

    fn submission(&self) -> PathBuf {
        pclm_file::snapshot(
            &self.current,
            &self.join("제출.pclm"),
            PclmRole::SUBMISSION,
            false,
            &SystemEnv,
        )
        .unwrap()
    }
}

/// 계약면 뷰 전부를 글로 편다. 옮긴 자리의 뷰가 원본과 한 글자도 다르지 않은지 본다.
fn dump(path: &Path) -> Vec<String> {
    let c = rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .unwrap();
    let mut names = views::NAMES.to_vec();
    names.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    let mut lines = Vec::new();
    for view in names {
        let mut statement = c.prepare(&format!("SELECT * FROM \"{view}\";")).unwrap();
        let width = statement.column_count();
        let mut rows = statement.query([]).unwrap();
        while let Some(row) = rows.next().unwrap() {
            let cells: Vec<String> = (0..width)
                .map(|i| format!("{:?}", row.get::<_, rusqlite::types::Value>(i).unwrap()))
                .collect();
            lines.push(format!("{view}|{}", cells.join("|")));
        }
    }
    lines
}

fn names_in(folder: &Path, recursive: bool) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(folder) else {
        return out;
    };
    for entry in entries.map(|e| e.unwrap()) {
        if entry.file_type().unwrap().is_dir() {
            if recursive {
                out.extend(names_in(&entry.path(), true));
            }
        } else {
            out.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    out
}

fn no_temporaries(folder: &Path, recursive: bool) -> bool {
    !names_in(folder, recursive)
        .iter()
        .any(|n| n.contains(".tmp-"))
}

fn same(a: &Path, b: &str) -> bool {
    dotnet::eq_ignore_case(&a.to_string_lossy(), b)
}

// ── 확장 저장 ────────────────────────────────────────────────────────

fn snapshot(base: &str) -> CaptureInput {
    let mut input = CaptureInput::new(
        "g2b-notice-a-v1",
        &mapping::revision(&mapping::defaults()),
        json!({
            "pointInfo": { "bidPbancNo": base, "bidPbancOrd": "001", "bidPbancNm": "지어낸 공고",
                "pbancInstUntyGrpNm": "시험기관" },
            "tables": {},
        }),
    );
    input.scope = Some("live".into());
    input.screen = Some("01179".into());
    input
}

/// 확장 호스트가 이 파일에 저장할 요청을 미리 짓는다. 공고 하나, 충돌 없음.
fn prepare_capture(path: &Path, home: &Home, dataset_id: &str, base: &str) -> Value {
    let host = ErpCapture::new(path, &SystemEnv)
        .with_environment("development")
        .with_destination(home);
    let input = snapshot(base);
    let preview = host.inspect(&input).unwrap();
    json!({
        "protocolVersion": 2, "requestId": win::new_guid_n(), "method": "capture",
        "params": { "datasetId": dataset_id, "snapshot": input.to_value(),
            "captureId": win::new_guid_n(), "baseToken": preview.base_token, "choices": {} },
    })
}

fn send(path: &Path, home: &Home, request: &Value) -> Value {
    ErpCapture::new(path, &SystemEnv)
        .with_environment("development")
        .with_destination(home)
        .dispatch(request, false)
}

/// 창의 고급 도구 「검토한 자료 저장」 도 쪽지를 본다. 창이 연 파일을 쪽지가 더는 가리키지 않으면 거절한다.
#[test]
fn 고급_도구의_저장은_쪽지가_다른_파일을_가리키면_거절하고_쓰지_않는다() {
    let f = Fixture::new();
    let input = snapshot("R26BK00000002");
    let preview = tools::invoke(
        &f.current,
        &SystemEnv,
        "inspect",
        &input.to_value().to_string(),
        Some(&f.home),
    )
    .unwrap();
    let capture = || {
        json!({ "snapshot": input.to_value(), "baseToken": preview["baseToken"], "captureId": win::new_guid_n(),
            "choices": {} })
        .to_string()
    };

    let other = work(&f.join("다른 작업자료.pclm"));
    f.home
        .write_config(&other, &info(&other).dataset_id.unwrap(), None)
        .unwrap();
    let before = dump(f.current.path());

    let Err(Error::InvalidOperation(message)) =
        tools::invoke(&f.current, &SystemEnv, "capture", &capture(), Some(&f.home))
    else {
        panic!("거절해야 한다");
    };
    assert!(message.contains("저장 대상이 바뀌었습니다"));
    assert_eq!(dump(f.current.path()), before);
    assert_eq!(count(f.current.path(), "erp_capture"), 0);

    // 쪽지가 이 파일로 돌아오면 같은 자료가 들어간다.
    f.home
        .write_config(f.current.path(), &f.current_id(), None)
        .unwrap();
    tools::invoke(&f.current, &SystemEnv, "capture", &capture(), Some(&f.home)).unwrap();
    assert_eq!(count(f.current.path(), "erp_capture"), 1);
}

// ── 옮기기 ───────────────────────────────────────────────────────────

#[test]
fn 옮기기는_같은_자료를_새_자리에_세우고_옛_파일을_retired_로_남긴다() {
    let f = Fixture::new();
    let old = f.current.path().to_path_buf();
    let id = f.current_id();
    let before = dump(&old);
    let target = f.join("새 자리").join("계약자료.pclm");

    let moved_to = f.home.move_to(&f.current, &target).unwrap();
    assert!(same(&moved_to, &target.to_string_lossy()));

    let moved = info(&target);
    assert_eq!(
        (
            moved.kind,
            moved.role.as_deref(),
            moved.dataset_id.as_deref()
        ),
        (PclmKind::Ok, Some(PclmRole::WORK), Some(id.as_str()))
    );
    assert_eq!(dump(&target), before);
    for table in ["request", "contract", "plan"] {
        assert_eq!(count(&target, table), 1);
    }

    let config = f.home.read_config().config.unwrap();
    assert!(same(&target, &config.workfile.unwrap()));
    assert_eq!(config.dataset_id.as_deref(), Some(id.as_str()));
    assert!(same(&old, &config.retired.unwrap()));

    assert_eq!(info(&old).role.as_deref(), Some(PclmRole::RETIRED));
    // 옮기기 전에 떠 있던 다른 손잡이도 옛 파일에 쓰지 못한다(ADR-031).
    let Err(Error::InvalidOperation(message)) = Database::new(&old, Access::Write).open() else {
        panic!("옮긴 옛 파일은 쓰기로 열지 않는다");
    };
    assert!(message.contains("옮겨졌습니다"));
    assert!(no_temporaries(&f.join("새 자리"), false));
}

#[test]
fn 옮긴_뒤_다음_시작은_옛_파일을_지우고_쪽지의_옛_자리를_걷는다() {
    let f = Fixture::new();
    let old = f.current.path().to_path_buf();
    let target = f.join("새 자리.pclm");
    f.home.move_to(&f.current, &target).unwrap();

    let HomeState::Ready { database, notes } = f.home.resolve().unwrap() else {
        panic!("새 자리를 연다");
    };

    assert!(same(database.path(), &target.to_string_lossy()));
    assert!(
        notes
            .iter()
            .any(|n| n.contains("옛 파일을 지웠습니다") && n.contains(&*old.to_string_lossy()))
    );
    assert!(!old.exists());
    assert!(!resolve::suffixed(&old, "-wal").exists());
    assert_eq!(f.home.read_config().config.unwrap().retired, None);
}

/// 쪽지가 가리키는 옛 자리에 retired 가 아닌 것이 놓였으면 남의 것이다 — 지우지 않고 칸만 걷는다.
#[test]
fn 옛_자리의_파일이_retired_가_아니면_지우지_않는다() {
    let f = Fixture::new();
    let other = work(&f.join("다른 작업자료.pclm"));
    f.home
        .write_config(f.current.path(), &f.current_id(), Some(&other))
        .unwrap();

    let HomeState::Ready { notes, .. } = f.home.resolve().unwrap() else {
        panic!("연다");
    };

    assert!(other.exists());
    assert!(notes.is_empty());
    assert_eq!(f.home.read_config().config.unwrap().retired, None);
}

#[test]
fn 옮길_자리에_파일이_있으면_아무것도_바꾸지_않는다() {
    let f = Fixture::new();
    let target = f.join("남의 파일.pclm");
    std::fs::write(&target, "남의 것").unwrap();
    let config = f.config_bytes();

    assert!(matches!(
        f.home.move_to(&f.current, &target),
        Err(Error::Io(_))
    ));

    assert_eq!(std::fs::read_to_string(&target).unwrap(), "남의 것");
    assert_eq!(f.config_bytes(), config);
    assert_eq!(info(f.current.path()).role.as_deref(), Some(PclmRole::WORK));
    assert!(no_temporaries(&f.root.0, true));

    // -wal 만 남은 자리도 비지 않은 것으로 본다.
    let wal_only = f.join("반쯤.pclm");
    std::fs::write(resolve::suffixed(&wal_only, "-wal"), "").unwrap();
    assert!(matches!(
        f.home.move_to(&f.current, &wal_only),
        Err(Error::Io(_))
    ));
    assert!(!wal_only.exists());
    assert_eq!(f.config_bytes(), config);
}

/// 작업자료를 새로 세우는 자리는 덮지 않는다. 이미 있는 자리를 고르면 다른 이름을 고르라고 말해야 한다.
#[test]
fn 새_작업자료_자리에_파일이_있으면_다른_이름을_고르라고_거절한다() {
    let f = Fixture::new();
    let target = f.join("있는 파일.pclm");
    std::fs::write(&target, "남의 것").unwrap();

    let Err(Error::Io(refused)) = check_new_target(&f.current, &target) else {
        panic!("덮지 않는다");
    };

    assert!(refused.to_string().contains("다른 이름을 고르세요"));
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "남의 것");
}

#[test]
fn 옮기기_전에_커밋된_저장은_새_자리에_있다() {
    let f = Fixture::new();
    let request = prepare_capture(f.current.path(), &f.home, &f.current_id(), "R26BK00000001");
    assert_eq!(send(f.current.path(), &f.home, &request)["ok"], true);

    let target = f.join("새 자리.pclm");
    f.home.move_to(&f.current, &target).unwrap();

    assert_eq!(count(&target, "notice"), 1);
    assert_eq!(count(&target, "erp_capture"), 1);
}

/// 잠금 안에 끼워 저장을 줄 세운다. 줄 선 저장의 답을 돌려준다.
fn queue_capture(home: &mut Home, path: &Path, request: Value) -> mpsc::Receiver<Value> {
    let (sent, received) = mpsc::channel();
    let queued = Arc::new(Mutex::new(Some((sent, request))));
    let (path, destination) = (path.to_path_buf(), home.clone());
    home.set_while_locked(move || {
        let Some((sent, request)) = queued.lock().unwrap().take() else {
            return;
        };
        let (path, destination) = (path.clone(), destination.clone());
        let (finished, done) = mpsc::channel();
        std::thread::spawn(move || {
            let reply = send(&path, &destination, &request);
            let _ = finished.send(());
            sent.send(reply).unwrap();
        });
        std::thread::sleep(Duration::from_millis(500));
        assert!(done.try_recv().is_err(), "잠금 앞에 줄 서 있다");
    });
    received
}

/// 전환 경합. 옮기는 창이 잠금을 쥔 사이 저장이 줄을 서면, 잠금이 풀린 뒤 옛 파일이 retired 이고 쪽지가 새 자리를
/// 가리키므로 거절된다 — 옛 파일에도 새 자리에도 쓰이지 않는다.
#[test]
fn 옮기는_사이_줄_선_저장은_거절되고_어느_파일에도_쓰이지_않는다() {
    let mut f = Fixture::new();
    let old = f.current.path().to_path_buf();
    let request = prepare_capture(&old, &f.home, &f.current_id(), "R26BK00000001");
    let reply = queue_capture(&mut f.home, &old, request);

    let target = f.join("새 자리.pclm");
    f.home.move_to(&f.current, &target).unwrap();

    let reply = reply.recv_timeout(Duration::from_secs(60)).unwrap();
    assert_eq!(reply["ok"], false);
    assert!(
        reply["error"]["message"]
            .as_str()
            .unwrap()
            .contains("저장 대상이 바뀌었습니다")
    );
    for path in [&old, &target] {
        assert_eq!(count(path, "notice"), 0);
        assert_eq!(count(path, "erp_capture"), 0);
    }
}

// ── 다른 작업자료 쓰기 ───────────────────────────────────────────────

#[test]
fn 다른_작업자료를_쓰면_쪽지만_바뀌고_지금_자료는_작업자료로_남는다() {
    let f = Fixture::new();
    let other = work(&f.join("다른 작업자료.pclm"));
    let other_id = info(&other).dataset_id;

    let used = f.home.use_in_place(&f.current, &other).unwrap();
    assert!(same(&used, &other.to_string_lossy()));

    let config = f.home.read_config().config.unwrap();
    assert_eq!(
        (config.workfile, config.dataset_id),
        (Some(other.to_string_lossy().into_owned()), other_id)
    );
    assert_eq!(config.retired, None);
    assert_eq!(info(f.current.path()).role.as_deref(), Some(PclmRole::WORK));
    Database::new(f.current.path(), Access::Write)
        .open()
        .unwrap(); // 여전히 쓸 수 있는 작업자료다
}

#[test]
fn 제출본은_그_자리에서_작업자료로_쓰지_않는다() {
    let f = Fixture::new();
    let submission = f.submission();
    let config = f.config_bytes();

    let Err(Error::PclmFile(error)) = f.home.use_in_place(&f.current, &submission) else {
        panic!("제출본은 쓰지 않는다");
    };

    assert!(
        error
            .message
            .contains("그 자리에서 작업자료로 쓸 수 없습니다")
    );
    assert_eq!(f.config_bytes(), config);
}

#[test]
fn 다른_작업자료로_바꾸는_사이_줄_선_저장은_거절된다() {
    let mut f = Fixture::new();
    let other = work(&f.join("다른 작업자료.pclm"));
    let current = f.current.path().to_path_buf();
    let request = prepare_capture(&current, &f.home, &f.current_id(), "R26BK00000001");
    let reply = queue_capture(&mut f.home, &current, request);

    f.home.use_in_place(&f.current, &other).unwrap();

    assert_eq!(
        reply.recv_timeout(Duration::from_secs(60)).unwrap()["ok"],
        false
    );
    assert_eq!(info(&current).role.as_deref(), Some(PclmRole::WORK));
    assert_eq!(count(&current, "notice"), 0);
    assert_eq!(count(&other, "notice"), 0);
}

// ── 스냅샷에서 새로 ──────────────────────────────────────────────────

#[test]
fn 제출본에서_새_계약자료를_만들면_새_신원의_작업자료가_서고_원본은_그대로다() {
    let f = Fixture::new();
    let submission = f.submission();
    let submission_id = info(&submission).dataset_id;
    let before = hash(&submission);
    let target = f.join("이어서.pclm");

    f.home
        .create_from_snapshot(&f.current, &submission, &target)
        .unwrap();

    let made = info(&target);
    assert_eq!(
        (made.kind, made.role.as_deref(), made.version),
        (PclmKind::Ok, Some(PclmRole::WORK), schema::VERSION)
    );
    assert_ne!(made.dataset_id, submission_id);
    assert_ne!(made.dataset_id, Some(f.current_id()));
    assert_eq!(dump(&submission), dump(&target));

    let config = f.home.read_config().config.unwrap();
    assert!(same(&target, &config.workfile.unwrap()));
    assert_eq!(config.dataset_id, made.dataset_id);
    assert_eq!(info(f.current.path()).role.as_deref(), Some(PclmRole::WORK));

    assert_eq!(hash(&submission), before);
    assert!(!resolve::suffixed(&target, "-wal").exists());
}

#[test]
fn 옛_v19_파일에서_새_계약자료를_만들면_사본을_올리고_원본은_그대로다() {
    let f = Fixture::new();
    let old = legacy(
        &f.join("옛 자료.db"),
        "INSERT INTO request_series (request_base) VALUES ('R-옛것');
         INSERT INTO request (request_base, seq, updated_at) VALUES ('R-옛것', '00', 't');",
    );
    let legacy_id = info(&old).dataset_id;
    let before = hash(&old);
    let target = f.join("올린 것.pclm");

    f.home
        .create_from_snapshot(&f.current, &old, &target)
        .unwrap();

    let made = info(&target);
    assert_eq!(
        (made.kind, made.role.as_deref(), made.version),
        (PclmKind::Ok, Some(PclmRole::WORK), schema::VERSION)
    );
    assert_ne!(made.dataset_id, legacy_id);
    assert_eq!(count(&target, "request"), 1);
    assert!(!resolve::suffixed(&target, "-wal").exists());
    assert!(no_temporaries(&f.root.0, false));

    assert_eq!(hash(&old), before);
    assert_eq!(info(&old).version, schema::BASELINE_VERSION);
}

#[test]
fn 새_계약자료를_뜰_자리에_파일이_있으면_덮지_않는다() {
    let f = Fixture::new();
    let submission = f.submission();
    let target = f.join("있는 것.pclm");
    std::fs::write(&target, "남의 것").unwrap();
    let config = f.config_bytes();

    assert!(matches!(
        f.home
            .create_from_snapshot(&f.current, &submission, &target),
        Err(Error::Io(_))
    ));

    assert_eq!(std::fs::read_to_string(&target).unwrap(), "남의 것");
    assert_eq!(f.config_bytes(), config);
    assert!(no_temporaries(&f.root.0, false));
}

// ── 새 계약자료 ──────────────────────────────────────────────────────

#[test]
fn 새_계약자료는_빈_작업자료를_지어_쪽지를_옮긴다() {
    let f = Fixture::new();
    let target = f.join("빈 것.pclm");

    f.home
        .create_new_while_running(&f.current, &target)
        .unwrap();

    let made = info(&target);
    assert_eq!(
        (made.kind, made.role.as_deref()),
        (PclmKind::Ok, Some(PclmRole::WORK))
    );
    assert_eq!(count(&target, "request"), 0);
    let config = f.home.read_config().config.unwrap();
    assert!(same(&target, &config.workfile.unwrap()));
    assert_eq!(config.dataset_id, made.dataset_id);
    assert_eq!(info(f.current.path()).role.as_deref(), Some(PclmRole::WORK));
}

#[test]
fn 새_계약자료를_지을_자리에_파일이_있으면_덮지_않는다() {
    let f = Fixture::new();
    let target = f.join("있는 것.pclm");
    std::fs::write(&target, "남의 것").unwrap();
    let config = f.config_bytes();

    assert!(matches!(
        f.home.create_new_while_running(&f.current, &target),
        Err(Error::Io(_))
    ));

    assert_eq!(std::fs::read_to_string(&target).unwrap(), "남의 것");
    assert_eq!(f.config_bytes(), config);
}

// ── 백업 ─────────────────────────────────────────────────────────────

#[test]
fn 백업은_새_신원의_한_파일이고_쪽지는_그대로다() {
    let f = Fixture::new();
    let config = f.config_bytes();
    let target = f.join("백업.pclm");

    let saved = backup_to(&f.current, &target).unwrap();
    assert!(same(&saved, &target.to_string_lossy()));

    let backup = info(&target);
    assert_eq!(
        (backup.kind, backup.role.as_deref()),
        (PclmKind::Ok, Some(PclmRole::BACKUP))
    );
    assert_ne!(backup.dataset_id, Some(f.current_id()));
    assert!(!resolve::suffixed(&target, "-wal").exists());
    assert!(!resolve::suffixed(&target, "-shm").exists());
    assert_eq!(count(&target, "contract"), 1);
    assert_eq!(f.config_bytes(), config);

    // 사람이 고른 자리의 옛 백업은 덮는다.
    backup_to(&f.current, &target).unwrap();
    assert_eq!(info(&target).role.as_deref(), Some(PclmRole::BACKUP));
}

#[test]
fn 백업은_작업자료를_덮지_않는다() {
    let f = Fixture::new();
    let other = work(&f.join("다른 작업자료.pclm"));
    let before = hash(&other);

    assert!(matches!(
        backup_to(&f.current, &other),
        Err(Error::InvalidOperation(_))
    ));
    assert!(matches!(
        backup_to(&f.current, f.current.path()),
        Err(Error::InvalidOperation(_))
    ));

    assert_eq!(hash(&other), before);
    assert_eq!(info(f.current.path()).role.as_deref(), Some(PclmRole::WORK));
}

// ── 네트워크 자리 ────────────────────────────────────────────────────

#[test]
fn 네트워크_자리는_어느_동작에서도_거절한다() {
    let f = Fixture::new();
    let unc = Path::new(r"\\지어낸서버\공유\계약자료.pclm");
    let submission = f.submission();
    let config = f.config_bytes();

    let results = [
        f.home.move_to(&f.current, unc),
        f.home.use_in_place(&f.current, unc),
        f.home.create_from_snapshot(&f.current, &submission, unc),
        f.home.create_new_while_running(&f.current, unc),
        backup_to(&f.current, unc),
    ];
    for result in results {
        assert!(
            matches!(result, Err(Error::InvalidOperation(_))),
            "{result:?}"
        );
    }

    assert_eq!(f.config_bytes(), config);
    assert_eq!(info(f.current.path()).role.as_deref(), Some(PclmRole::WORK));
}

//! C# `HomeTests` 의 시작 몫 — 처음 실행·옛 자료 옮겨 오기·쪽지가 있을 때·백업·쪽지 쓰기·시작 화면의 두 동작. 쪽지 읽기·
//! 호스트의 묶기·출처·잠금은 `tests.rs` 에 있다.
//!
//! 지키는 것은 "오류 없이 다른 자료를 여는 일" 이다. 홈은 모두 임시 폴더다. 값은 전부 지어낸 것이다.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use pclm_core::storage::{pclm_file, schema};
use pclm_core::{Access, Database, Error, PclmKind, PclmRole};
use sha2::{Digest, Sha256};

use super::*;

pub(crate) struct Root(pub(crate) PathBuf);

impl Root {
    pub(crate) fn new(name: &str) -> Root {
        let path = std::env::temp_dir().join(format!("pclm-{name}-{}", &win::new_guid_n()[..8]));
        std::fs::create_dir_all(&path).unwrap();
        Root(path)
    }

    pub(crate) fn join(&self, part: &str) -> PathBuf {
        self.0.join(part)
    }
}

impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// 파일 바이트의 지문.
pub(crate) fn hash(path: &Path) -> String {
    Sha256::digest(std::fs::read(path).unwrap())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub(crate) fn info(path: &Path) -> pclm_core::PclmInfo {
    pclm_file::inspect(path)
}

pub(crate) fn count(path: &Path, table: &str) -> i64 {
    rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .unwrap()
        .query_row(&format!("SELECT COUNT(*) FROM \"{table}\";"), [], |r| {
            r.get(0)
        })
        .unwrap()
}

/// 표지가 생기기 전의 v19 작업자료(C# `PclmFileTests.LegacyV19`). `sql` 을 더 심는다. 뷰는 짓지 않는다 — 판올림이
/// 먼저 치운다.
pub(crate) fn legacy(path: &Path, sql: &str) -> PathBuf {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let c = rusqlite::Connection::open(path).unwrap();
    c.query_row("PRAGMA journal_mode = WAL;", [], |_| Ok(()))
        .unwrap();
    c.execute_batch(schema::BASELINE).unwrap();
    c.execute_batch(&format!(
        "PRAGMA user_version = {};",
        schema::BASELINE_VERSION
    ))
    .unwrap();
    c.execute_batch(sql).unwrap();
    path.to_path_buf()
}

fn legacy_request(path: &Path) -> PathBuf {
    legacy(
        path,
        "INSERT INTO request_series (request_base) VALUES ('R-지어냄');",
    )
}

pub(crate) fn work(path: &Path) -> PathBuf {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    pclm_file::create(path, PclmRole::WORK, &SystemEnv).unwrap();
    path.to_path_buf()
}

fn submission(root: &Root, name: &str) -> PathBuf {
    let source = work(&root.join(&format!("작업-{name}")));
    pclm_file::snapshot(
        &Database::new(&source, Access::Write),
        &root.join(name),
        PclmRole::SUBMISSION,
        false,
        &SystemEnv,
    )
    .unwrap()
}

fn same(a: &Path, b: &Path) -> bool {
    dotnet::eq_ignore_case(&a.to_string_lossy(), &b.to_string_lossy())
}

fn ready(state: HomeState) -> (Database, Vec<String>) {
    match state {
        HomeState::Ready { database, notes } => (database, notes),
        other => panic!("열려야 한다: {other:?}"),
    }
}

fn problem(state: HomeState) -> (ProblemKind, PathBuf, String) {
    match state {
        HomeState::Problem {
            kind,
            path,
            message,
        } => (kind, path, message),
        other => panic!("열지 않아야 한다: {other:?}"),
    }
}

fn names(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(folder)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn fixture() -> (Root, Home) {
    let root = Root::new("home");
    let home =
        Home::with_legacy_roaming_config(root.join("홈"), root.join("roaming").join("config.json"))
            .unwrap();
    (root, home)
}

// ── 처음 실행 ────────────────────────────────────────────────────────

#[test]
fn 처음_실행은_홈에_작업자료와_쪽지를_짓는다() {
    let (_root, home) = fixture();
    let (database, notes) = ready(home.resolve().unwrap());

    assert!(same(database.path(), &home.default_workfile()));
    assert!(notes.is_empty());

    let made = info(&home.default_workfile());
    assert_eq!(
        (made.kind, made.role.as_deref(), made.version),
        (PclmKind::Ok, Some(PclmRole::WORK), schema::VERSION)
    );

    let config = home.read_config();
    assert_eq!(config.state, ConfigState::Ok);
    let config = config.config.unwrap();
    assert!(same(
        Path::new(&config.workfile.unwrap()),
        &home.default_workfile()
    ));
    assert_eq!(config.dataset_id, made.dataset_id);
    assert!(!home.is_default());
}

/// 지금 작업자료인지는 쪽지가 가리키는 파일로만 가린다 — 열람 창의 「다른 자료 열어 보기」 가 이것으로 내 작업자료를
/// 거절한다.
/// C# 은 연결 풀이 쓰기 연결을 마지막에 닫아 `-wal`·`-shm` 을 걷는다 — 신원을 읽는 읽기 전용 연결이 마지막에 닫히면
/// 빈 곁파일이 남아 시작한 홈이 C# 과 갈린다(동등성 `resolve-*.tree` 가 잡았다).
#[test]
fn 시작은_작업자료_곁에_wal_shm_을_남기지_않는다() {
    let (root, home) = fixture();
    let sidecars = |path: &Path| {
        ["-wal", "-shm"]
            .iter()
            .filter(|s| crate::resolve::suffixed(path, s).exists())
            .count()
    };

    let (first, _) = ready(home.resolve().unwrap());
    assert_eq!(sidecars(first.path()), 0, "처음 실행");
    let (again, _) = ready(home.resolve().unwrap());
    assert_eq!(sidecars(again.path()), 0, "쪽지가 가리키는 작업자료");

    let moved = Home::new(root.join("옛 자료가 있는 홈")).unwrap();
    legacy_request(&moved.directory().join("pclm.db"));
    let (upgraded, notes) = ready(moved.resolve().unwrap());
    assert_eq!(notes.len(), 1);
    assert_eq!(sidecars(upgraded.path()), 0, "옛 자료를 옮겨 와 올리기");
}

#[test]
fn 지금_작업자료인지는_쪽지가_가리키는_파일로_가린다() {
    let (root, home) = fixture();
    let (database, _) = ready(home.resolve().unwrap());

    assert!(home.is_active_workfile(database.path()));
    let upper = dotnet::to_upper_invariant(&database.path().to_string_lossy());
    assert!(home.is_active_workfile(Path::new(&upper)));
    assert!(!home.is_active_workfile(&work(&root.join("다른 작업자료.pclm"))));
    assert!(
        !Home::new(root.join("빈 홈"))
            .unwrap()
            .is_active_workfile(database.path())
    );
}

/// 지금 쓰는 사람의 옛 자료는 **사본을 떠서** 올린다. 원본은 바이트 하나 바뀌지 않고, 신원은 이어진다.
#[test]
fn 홈의_옛_자료는_사본을_떠_올리고_원본은_그대로_둔다() {
    let (_root, home) = fixture();
    let legacy = legacy_request(&home.directory().join("pclm.db"));
    let before = hash(&legacy);
    let legacy_id = info(&legacy).dataset_id;

    let (database, notes) = ready(home.resolve().unwrap());

    assert_eq!(notes.len(), 1);
    assert!(notes[0].contains("옛 자료를 옮겨 왔습니다"));
    assert!(notes[0].contains("원본은 그대로"));
    assert!(same(database.path(), &home.default_workfile()));

    let copied = info(&home.default_workfile());
    assert_eq!(
        (
            copied.kind,
            copied.role.as_deref(),
            copied.version,
            &copied.dataset_id
        ),
        (
            PclmKind::Ok,
            Some(PclmRole::WORK),
            schema::VERSION,
            &legacy_id
        )
    );
    assert_eq!(count(&home.default_workfile(), "request_series"), 1);
    assert_eq!(home.read_config().config.unwrap().dataset_id, legacy_id);

    assert_eq!(hash(&legacy), before);
    assert_eq!(info(&legacy).version, schema::BASELINE_VERSION);

    // 원본이 백업 노릇을 하므로 사본을 다시 백업하지 않는다. 임시 이름도 남지 않는다.
    assert!(!home.backups_directory().exists());
    assert!(!names(home.directory()).iter().any(|n| n.ends_with(".tmp")));
}

#[test]
fn 옛_쪽지의_data_dir_자료가_홈의_옛_자료보다_먼저다() {
    let (root, home) = fixture();
    let roaming = legacy_request(&root.join("옛 자리").join("pclm.db"));
    legacy_request(&home.directory().join("pclm.db"));
    std::fs::create_dir_all(root.join("roaming")).unwrap();
    std::fs::write(
        root.join("roaming").join("config.json"),
        serde_json::json!({ "dataDir": root.join("옛 자리") }).to_string(),
    )
    .unwrap();
    let before = hash(&roaming);

    let (_, notes) = ready(home.resolve().unwrap());

    assert_eq!(notes.len(), 1);
    assert!(notes[0].contains(&*roaming.to_string_lossy()));
    assert_eq!(
        info(&roaming).dataset_id,
        info(&home.default_workfile()).dataset_id
    );
    assert_eq!(hash(&roaming), before);
}

/// 이사가 밀려 있던 옛 쪽지는 dataDir 이 아직 빈 새 자리다. 자료는 옛 자리(pendingMoveFrom)에 있다.
#[test]
fn 옛_쪽지에_밀린_이사가_있으면_옛_자리의_자료를_옮겨_온다() {
    let (root, home) = fixture();
    let real = legacy_request(&root.join("옛 자리").join("pclm.db"));
    std::fs::create_dir_all(root.join("roaming")).unwrap();
    std::fs::write(
        root.join("roaming").join("config.json"),
        serde_json::json!({ "dataDir": root.join("아직 빈 새 자리"), "pendingMoveFrom": root.join("옛 자리") })
            .to_string(),
    )
    .unwrap();

    let (_, notes) = ready(home.resolve().unwrap());

    assert_eq!(notes.len(), 1);
    assert!(notes[0].contains(&*real.to_string_lossy()));
    assert_eq!(
        info(&real).dataset_id,
        info(&home.default_workfile()).dataset_id
    );
    assert!(!root.join("아직 빈 새 자리").exists());
}

#[test]
fn 홈에_이미_작업자료가_있으면_덮지_않고_그것을_쓴다() {
    let (_root, home) = fixture();
    let existing = work(&home.default_workfile());
    let id = info(&existing).dataset_id;
    legacy_request(&home.directory().join("pclm.db"));

    let (_, notes) = ready(home.resolve().unwrap());

    assert!(notes.is_empty());
    assert_eq!(info(&home.default_workfile()).dataset_id, id);
    assert_eq!(home.read_config().config.unwrap().dataset_id, id);
}

/// 받을 수 없는 옛 자료가 있으면 빈 자료를 세우지 않는다 — 사람은 자료가 사라진 줄 안다.
#[test]
fn 받을_수_없는_옛_자료가_있으면_짓지_않고_멈춘다() {
    let (_root, home) = fixture();
    std::fs::create_dir_all(home.directory()).unwrap();
    rusqlite::Connection::open(home.directory().join("pclm.db"))
        .unwrap()
        .execute_batch("CREATE TABLE t (x); PRAGMA user_version = 11;")
        .unwrap();

    let (kind, _, _) = problem(home.resolve().unwrap());

    assert_eq!(kind, ProblemKind::TooOld);
    assert!(!home.default_workfile().exists());
    assert_eq!(home.read_config().state, ConfigState::Missing);
}

// ── 쪽지가 있을 때 ───────────────────────────────────────────────────

#[test]
fn 깨진_쪽지로는_기본_자리로_물러나지_않고_아무것도_짓지_않는다() {
    let (_root, home) = fixture();
    std::fs::create_dir_all(home.directory()).unwrap();
    legacy_request(&home.directory().join("pclm.db"));
    for (text, why) in [
        ("{ 깨진", "쪽지의 JSON 을 읽지 못했습니다."),
        (
            r#"{ "version": 2, "workfile": "C:\\x.pclm", "datasetId": "ab" }"#,
            "모르는 형식(version 2)입니다.",
        ),
        (
            r#"{ "version": 1, "workfile": "상대\\x.pclm", "datasetId": "ab" }"#,
            r"작업자료 경로가 전체 경로가 아닙니다: 상대\x.pclm",
        ),
        (
            r#"{ "version": 1, "datasetId": "ab" }"#,
            "작업자료 경로(workfile)가 없습니다.",
        ),
    ] {
        std::fs::write(home.config_path(), text).unwrap();

        let (kind, path, message) = problem(home.resolve().unwrap());

        assert_eq!(kind, ProblemKind::ConfigCorrupt);
        assert_eq!(path, home.config_path());
        assert_eq!(
            message,
            format!(
                "작업자료를 가리키는 설정 파일을 읽을 수 없습니다({why}). 다른 자료를 대신 열지 않습니다 — \
                 작업자료 파일을 찾아 주거나 새로 만드세요."
            )
        );
        assert!(!home.default_workfile().exists());
        assert_eq!(std::fs::read_to_string(home.config_path()).unwrap(), text);
    }
}

#[test]
fn 쪽지가_가리키는_파일이_없으면_아무것도_짓지_않는다() {
    let (root, home) = fixture();
    let missing = root.join("지워진 자리").join("계약자료.pclm");
    home.write_config(&missing, "ab", None).unwrap();

    let (kind, path, message) = problem(home.resolve().unwrap());

    assert_eq!((kind, &path), (ProblemKind::WorkfileMissing, &missing));
    assert_eq!(
        message,
        format!(
            "작업자료 파일이 없습니다. 지워졌거나 옮겨졌거나 이름이 바뀌었을 수 있습니다: {}",
            missing.display()
        )
    );
    assert!(!root.join("지워진 자리").exists());
    assert!(!home.default_workfile().exists());
}

#[test]
fn 쪽지가_제출본을_가리키면_그_자리에서_열지_않는다() {
    let (root, home) = fixture();
    let submission = submission(&root, "제출.pclm");
    home.write_config(&submission, &info(&submission).dataset_id.unwrap(), None)
        .unwrap();
    let before = hash(&submission);

    let (kind, _, message) = problem(home.resolve().unwrap());

    assert_eq!(kind, ProblemKind::NotWork);
    assert!(message.contains("제출본"));
    assert_eq!(hash(&submission), before);
}

/// 판올림은 되돌릴 수 없다. 올리기 전에 홈의 backups 에 한 벌 뜬다(ADR-033).
#[test]
fn 판이_낮으면_백업을_뜬_뒤_올린다() {
    let (root, home) = fixture();
    let legacy = legacy_request(&root.join("작업").join("옛것.pclm"));
    let id = info(&legacy).dataset_id.unwrap();
    home.write_config(&legacy, &id, None).unwrap();

    let (_, notes) = ready(home.resolve().unwrap());

    let backups = names(&home.backups_directory());
    assert_eq!(backups.len(), 1);
    let name = &backups[0];
    let stamp = name
        .strip_prefix("계약자료_v19_")
        .and_then(|rest| rest.strip_suffix(".pclm"))
        .unwrap();
    assert!(
        stamp.len() == 15
            && stamp.as_bytes()[8] == b'-'
            && stamp
                .bytes()
                .enumerate()
                .all(|(i, b)| i == 8 || b.is_ascii_digit()),
        "{name}"
    );
    let backup = home.backups_directory().join(name);
    let saved = info(&backup);
    assert_eq!(
        (saved.kind, saved.version, saved.dataset_id.as_deref()),
        (PclmKind::Ok, schema::BASELINE_VERSION, Some(id.as_str()))
    );
    assert_eq!(count(&backup, "request_series"), 1);
    assert_eq!(notes.len(), 1);
    assert_eq!(
        notes[0],
        format!(
            "판을 올리기 전에 백업을 떴습니다(v19 → v{}).\n{}",
            schema::VERSION,
            backup.display()
        )
    );

    assert_eq!(info(&legacy).version, schema::VERSION);
    assert_eq!(info(&legacy).dataset_id.as_deref(), Some(id.as_str()));
}

/// v19 가 아닌 옛 판의 백업은 이름표를 단 사본이다 — 역할 backup, 새 신원.
#[test]
fn v19_가_아닌_옛_판의_백업은_역할과_새_신원을_단다() {
    let (root, home) = fixture();
    let path = root.join("작업").join("v31.pclm");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let c = rusqlite::Connection::open(&path).unwrap();
    let mut sql = schema::BASELINE.to_string();
    for step in &schema::STEPS[..(31 - schema::BASELINE_VERSION) as usize] {
        sql.push_str(step);
        sql.push('\n');
    }
    c.execute_batch(&sql).unwrap();
    c.execute_batch("PRAGMA user_version = 31;").unwrap();
    drop(c);
    let id = info(&path).dataset_id.unwrap();
    home.write_config(&path, &id, None).unwrap();

    ready(home.resolve().unwrap());

    let backups = names(&home.backups_directory());
    assert!(backups[0].starts_with("계약자료_v31_"), "{backups:?}");
    let saved = info(&home.backups_directory().join(&backups[0]));
    assert_eq!(saved.role.as_deref(), Some(PclmRole::BACKUP));
    assert_eq!(saved.version, 31);
    assert_ne!(saved.dataset_id.as_deref(), Some(id.as_str()));
}

#[test]
fn 백업은_최근_열_벌만_남기고_남이_둔_파일은_건드리지_않는다() {
    let (root, home) = fixture();
    std::fs::create_dir_all(home.backups_directory()).unwrap();
    let base = SystemTime::UNIX_EPOCH + Duration::from_secs(978_307_200); // 2001-01-01T00:00:00Z
    for n in 0..12u64 {
        let old = home
            .backups_directory()
            .join(format!("계약자료_v19_2001010{}-0000{n:02}.pclm", n % 10));
        std::fs::write(&old, "옛 백업").unwrap();
        std::fs::File::options()
            .write(true)
            .open(&old)
            .unwrap()
            .set_modified(base + Duration::from_secs(60 * n))
            .unwrap();
    }
    let memo = home.backups_directory().join("메모.txt");
    std::fs::write(&memo, "사람이 둔 것").unwrap();

    let legacy = legacy_request(&root.join("작업").join("옛것.pclm"));
    home.write_config(&legacy, &info(&legacy).dataset_id.unwrap(), None)
        .unwrap();
    ready(home.resolve().unwrap());

    let kept: Vec<String> = names(&home.backups_directory())
        .into_iter()
        .filter(|n| n.ends_with(".pclm"))
        .collect();
    assert_eq!(kept.len(), BACKUPS_KEPT);
    // 방금 뜬 것은 남고, 가장 오래된 셋이 지워졌다.
    assert!(
        kept.iter()
            .any(|f| info(&home.backups_directory().join(f)).kind == PclmKind::Ok)
    );
    assert!(!kept.iter().any(|f| f.ends_with("-000000.pclm")
        || f.ends_with("-000001.pclm")
        || f.ends_with("-000002.pclm")));
    assert!(memo.exists());
}

/// 쪽지의 신원이 파일과 다르면 그 자리에 다른 작업자료가 놓인 것이다. 쪽지를 파일의 것으로 고친다.
#[test]
fn 쪽지와_파일의_신원이_다르면_쪽지를_파일의_것으로_고친다() {
    let (root, home) = fixture();
    let work = work(&root.join("작업.pclm"));
    home.write_config(&work, "deadbeef", None).unwrap();

    ready(home.resolve().unwrap());

    assert_eq!(
        home.read_config().config.unwrap().dataset_id,
        info(&work).dataset_id
    );
}

#[test]
fn 쪽지는_임시_파일을_남기지_않고_통째로_바뀐다() {
    let (root, home) = fixture();
    home.write_config(&root.join("가.pclm"), "aa", None)
        .unwrap();
    home.write_config(&root.join("나.pclm"), "bb", None)
        .unwrap();

    let config = home.read_config().config.unwrap();
    assert_eq!(
        (config.workfile.unwrap(), config.dataset_id.unwrap()),
        (
            root.join("나.pclm").to_string_lossy().into_owned(),
            "bb".to_string()
        )
    );
    assert_eq!(names(home.directory()), ["config.json"]);
}

/// 쪽지의 글은 C# `JsonSerializer`(들여쓰기·`UnicodeRanges.All`)와 같은 바이트다 — 한글은 그대로, `retired` 는 있을
/// 때만, 줄바꿈은 CR LF, 끝에 줄바꿈 없음.
#[test]
fn 쪽지의_글은_csharp_과_같은_꼴이다() {
    let (_root, home) = fixture();
    home.write_config(Path::new(r"C:\지어냄\계약자료.pclm"), "ab", None)
        .unwrap();
    assert_eq!(
        std::fs::read(home.config_path()).unwrap(),
        "{\r\n  \"version\": 1,\r\n  \"workfile\": \"C:\\\\지어냄\\\\계약자료.pclm\",\r\n  \"datasetId\": \"ab\"\r\n}"
            .as_bytes()
    );
    home.write_config(
        Path::new(r"C:\a&b\x.pclm"),
        "cd",
        Some(Path::new(r"C:\옛\'+.pclm")),
    )
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(home.config_path()).unwrap(),
        "{\r\n  \"version\": 1,\r\n  \"workfile\": \"C:\\\\a\\u0026b\\\\x.pclm\",\r\n  \"datasetId\": \"cd\",\r\n  \"retired\": \"C:\\\\옛\\\\\\u0027\\u002B.pclm\"\r\n}"
    );
}

// ── 시작 화면의 두 동작 ───────────────────────────────────────────────

#[test]
fn 파일_찾기는_작업자료만_받는다() {
    let (root, home) = fixture();
    let submission = submission(&root, "제출.pclm");
    let Err(Error::PclmFile(error)) = home.adopt(&submission) else {
        panic!("제출본은 받지 않는다");
    };
    assert!(error.message.contains("제출본"));
    assert!(matches!(
        home.adopt(&root.join("없음.pclm")),
        Err(Error::PclmFile(_))
    ));
    assert_eq!(home.read_config().state, ConfigState::Missing);

    let work = work(&root.join("내 자료.pclm"));
    home.adopt(&work).unwrap();
    let config = home.read_config().config.unwrap();
    assert_eq!(
        (config.workfile, config.dataset_id),
        (
            Some(work.to_string_lossy().into_owned()),
            info(&work).dataset_id
        )
    );
    assert_eq!(ready(home.resolve().unwrap()).0.path(), work);
}

#[test]
fn 새로_만들기는_있는_파일을_덮지_않는다() {
    let (root, home) = fixture();
    let existing = work(&root.join("있음.pclm"));
    let before = hash(&existing);

    assert!(matches!(home.create_new(&existing), Err(Error::Io(_))));
    assert_eq!(hash(&existing), before);
    assert_eq!(home.read_config().state, ConfigState::Missing);

    let created = home
        .create_new(&root.join("새 자리").join("새것.pclm"))
        .unwrap();
    let made = info(&created);
    assert_eq!(
        (made.kind, made.role.as_deref()),
        (PclmKind::Ok, Some(PclmRole::WORK))
    );
    assert_eq!(
        home.read_config().config.unwrap().workfile,
        Some(created.to_string_lossy().into_owned())
    );
}

#[test]
fn 네트워크_자리는_작업자료로_받지_않는다() {
    let (_root, home) = fixture();
    for path in [
        r"\\server\share\계약자료.pclm",
        r"\\?\UNC\server\share\계약자료.pclm",
    ] {
        let path = Path::new(path);
        for result in [home.create_new(path).map(|_| ()), home.adopt(path)] {
            let Err(Error::InvalidOperation(message)) = result else {
                panic!("네트워크 자리는 받지 않는다: {}", path.display());
            };
            assert!(message.contains("네트워크"), "{message}");
        }
    }
    assert_eq!(home.read_config().state, ConfigState::Missing);
}

// ── 열람 사본 ────────────────────────────────────────────────────────

/// 하루가 지난 열람 사본만 걷는다 — 지금 떠 있는 다른 열람 창의 사본은 둔다.
#[test]
fn 열람_사본은_하루가_지난_것만_걷는다() {
    let (_root, home) = fixture();
    home.prune_views(); // 폴더가 없어도 넘어지지 않는다
    std::fs::create_dir_all(home.view_directory()).unwrap();
    let old = home.view_directory().join("열람-옛것.pclm");
    let fresh = home.view_directory().join("열람-새것.pclm");
    for path in [&old, &fresh] {
        std::fs::write(path, "사본").unwrap();
    }
    std::fs::File::options()
        .write(true)
        .open(&old)
        .unwrap()
        .set_modified(SystemTime::now() - VIEW_COPIES_KEPT - Duration::from_secs(60))
        .unwrap();

    home.prune_views();

    assert!(!old.exists());
    assert!(fresh.exists());
}

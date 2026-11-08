//! 저장 Core 의 계약면 시험 — C# `SchemaBaselineTests`·`PclmFileTests`(홈 밖)·`UserViewTests` 의 판올림 예시 보기를
//! 옮긴 것. 뷰를 보는 단언은 `views.rs`·`flow_cases.rs` 에 있다.

mod common;

use std::path::Path;

use common::*;
use pclm_core::storage::database::resolve_links;
use pclm_core::storage::pclm_file::{self, PclmFileError};
use pclm_core::storage::store::{
    builtin_view_package, example_view_package_v26, read_view_package, stored_definition,
};
use pclm_core::storage::{schema, user_view_builder};
use pclm_core::{APPLICATION_ID, Access, Database, Error, PclmInfo, PclmKind, PclmRole};
use serde_json::Value;

// ── 스키마 ──────────────────────────────────────────────────────────

/// C# `새로_지은_스키마가_박제와_같다` — 갓 지은 작업자료의 `sqlite_master` 원문과 씨앗이 C# 박제와 한 글자까지 같다.
#[test]
fn fresh_schema_matches_the_csharp_golden() {
    let t = Temp::new("golden");
    let path = t.path("작업.pclm");
    let database = pclm_file::create(&path, PclmRole::WORK, &TestEnv::new()).unwrap();
    let actual = schema_dump(&database.open_read_only().unwrap());
    let golden = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/contract/schema.txt"),
    )
    .unwrap()
    .replace("\r\n", "\n");
    assert!(
        actual == golden,
        "스키마가 박제와 다릅니다:\n{}",
        first_difference(&golden, &actual)
    );
}

fn first_difference(expected: &str, actual: &str) -> String {
    for (i, (e, a)) in expected.lines().zip(actual.lines()).enumerate() {
        if e != a {
            return format!("{}째 줄\n  박제: {e}\n  지음: {a}", i + 1);
        }
    }
    format!(
        "줄 수가 다릅니다: 박제 {} / 지음 {}",
        expected.lines().count(),
        actual.lines().count()
    )
}

/// 같은 `Env` 차례면 같은 파일이 선다 — 이름표·지은 때·예시 보기 식별자가 모두 `Env` 에서 온다.
#[test]
fn identity_and_time_come_from_env() {
    let t = Temp::new("env");
    let path = t.path("작업.pclm");
    pclm_file::create(&path, PclmRole::WORK, &TestEnv::new()).unwrap();
    let c = raw_read(&path);
    let (id, created): (String, String) = c
        .query_row("SELECT dataset_id, created_at FROM pclm_file", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    // 기준선이 1 을 뽑고, v20 은 그것을 이어받아 새로 뽑지 않는다. 예시 보기가 다음 것을 받는다.
    assert_eq!(id, format!("{:032x}", 1));
    assert_eq!(created, "2026-11-07T03:04:05.1230000Z");
    let view: String = c
        .query_row("SELECT id FROM user_view", [], |r| r.get(0))
        .unwrap();
    assert_eq!(view, format!("{:032x}", 2));
    assert_eq!(pragma(&path, "user_version"), schema::VERSION as i64);
}

/// C# `가공열_판올림은_기존_열_정의와_수동값을_보존한다`.
#[test]
fn migration_from_v20_keeps_column_definitions_and_values() {
    let t = Temp::new("v20");
    let path = t.path("옛.db");
    {
        let c = raw(&path);
        c.execute_batch(&format!(
            "{}{}
INSERT INTO contract_series VALUES('TEST');
INSERT INTO contract_user_field VALUES('TEST','메모','보존할 값','2026-01-01');
UPDATE user_column SET kind='choice', options='준비|완료' WHERE field_name='진행상태';
PRAGMA user_version=20;",
            schema::BASELINE,
            schema::STEPS[0]
        ))
        .unwrap();
    }
    Database::new(&path, Access::Write)
        .migrate(&TestEnv::new())
        .unwrap();
    let c = raw_read(&path);
    let (kind, options): (String, String) = c
        .query_row(
            "SELECT kind, options FROM user_column WHERE entity_type='contract' AND field_name='진행상태'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((kind.as_str(), options.as_str()), ("choice", "준비|완료"));
    let value: String = c
        .query_row(
            "SELECT value FROM contract_user_field WHERE field_name='메모'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(value, "보존할 값");
}

/// C# `판올림은_큰따옴표가_든_뷰_이름도_치운다` — 이름에 `"` 가 든 뷰(손으로 만들었거나 남의 도구가 지은 파일)가 있어도
/// 판올림이 뷰를 치우고 끝까지 간다.
#[test]
fn migration_drops_views_whose_names_hold_double_quotes() {
    let t = Temp::new("quoted-view");
    let path = t.path("옛.db");
    {
        let c = raw(&path);
        c.execute_batch(&format!(
            "{}{}
CREATE VIEW \"옛\"\"보기\" AS SELECT 1 AS 값;
PRAGMA user_version=20;",
            schema::BASELINE,
            schema::STEPS[0]
        ))
        .unwrap();
    }
    Database::new(&path, Access::Write)
        .migrate(&TestEnv::new())
        .unwrap();
    let c = raw_read(&path);
    let left: i64 = c
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'view' AND name = '옛\"보기';",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(left, 0);
    assert_eq!(pragma(&path, "user_version"), i64::from(schema::VERSION));
}

/// C# `옛_시험판은_고치지_않고_거절한다` — 한 바이트도 고치지 않고 거절한다.
#[test]
fn pre_baseline_files_are_refused_untouched() {
    for version in 1..schema::BASELINE_VERSION {
        let t = Temp::new("old");
        let path = t.path("옛.db");
        raw(&path)
            .execute_batch(&format!(
                "CREATE TABLE notice (notice_base TEXT, seq TEXT); PRAGMA user_version = {version};"
            ))
            .unwrap();
        let before = std::fs::read(&path).unwrap();
        let error = Database::new(&path, Access::Write)
            .migrate(&TestEnv::new())
            .unwrap_err();
        let Error::InvalidOperation(message) = error else {
            panic!("{error}")
        };
        assert!(
            message.contains(&format!("v{version}")) && message.contains("0.7.0"),
            "{message}"
        );
        assert_eq!(before, std::fs::read(&path).unwrap());
        assert!(!suffixed(&path, "-wal").exists());
    }
}

// ── 알아보기 ────────────────────────────────────────────────────────

fn assert_untouched(path: &Path, before: &[u8]) {
    assert_eq!(before, std::fs::read(path).unwrap().as_slice());
    assert!(!suffixed(path, "-wal").exists());
    assert!(!suffixed(path, "-shm").exists());
}

/// 작업자료에 채움을 넣고 그것을 WAL 이 아닌 한 파일로 뜬다.
fn single_file(t: &Temp, name: &str, role: &str, env: &TestEnv) -> std::path::PathBuf {
    let work = pclm_file::create(&t.path(&format!("작업-{name}")), PclmRole::WORK, env).unwrap();
    work.open().unwrap().execute_batch(FILL).unwrap();
    pclm_file::snapshot(&work, &t.path(name), role, false, env).unwrap()
}

#[test]
fn missing_file_is_not_found_and_nothing_is_made() {
    let t = Temp::new("missing");
    let path = t.path("없는 폴더").join("계약자료.pclm");
    assert_eq!(pclm_file::inspect(&path).kind, PclmKind::NotFound);
    assert!(!path.parent().unwrap().exists());
}

#[test]
fn text_empty_and_plain_sqlite_are_not_pclm() {
    let t = Temp::new("notpclm");
    let text = t.path("글.pclm");
    std::fs::write(&text, "이건 SQLite 가 아니다").unwrap();
    let empty = t.path("빈.pclm");
    std::fs::write(&empty, b"").unwrap();
    let plain = t.path("일반.db");
    raw(&plain)
        .execute_batch("CREATE TABLE notice (notice_base TEXT, seq TEXT);")
        .unwrap();
    // 판 번호만 같은 남의 SQLite — 옛 이름표(erp_dataset)가 없으면 알아보지 않는다.
    let lookalike = t.path("판만같음.db");
    raw(&lookalike)
        .execute_batch(&format!(
            "CREATE TABLE notice (notice_base TEXT); PRAGMA user_version = {};",
            schema::BASELINE_VERSION
        ))
        .unwrap();
    for path in [text, empty, plain, lookalike] {
        let before = std::fs::read(&path).unwrap();
        assert_eq!(
            pclm_file::inspect(&path).kind,
            PclmKind::NotPclm,
            "{}",
            path.display()
        );
        assert_untouched(&path, &before);
    }
}

#[test]
fn old_trial_is_too_old_and_newer_is_too_new() {
    let t = Temp::new("versions");
    let env = TestEnv::new();
    let old = t.path("옛판.pclm");
    raw(&old)
        .execute_batch(
            "CREATE TABLE notice (notice_base TEXT, seq TEXT); PRAGMA user_version = 11;",
        )
        .unwrap();
    let newer = single_file(&t, "새판.pclm", PclmRole::SUBMISSION, &env);
    raw(&newer)
        .execute_batch("PRAGMA user_version = 999;")
        .unwrap();
    let bare = |kind, version| PclmInfo {
        kind,
        role: None,
        dataset_id: None,
        version,
    };
    assert_eq!(pclm_file::inspect(&old), bare(PclmKind::TooOld, 11));
    assert_eq!(pclm_file::inspect(&newer), bare(PclmKind::TooNew, 999));
}

/// 임시 규칙 — 표지가 생기기 전의 v19 작업자료는 PCLM 으로 알아본다(1.0 전에 걷는다).
#[test]
fn legacy_v19_is_recognized_as_work() {
    let t = Temp::new("v19");
    let path = t.path("옛 계약자료.db");
    legacy_v19(&path, None);
    let id: String = raw_read(&path)
        .query_row("SELECT dataset_id FROM erp_dataset", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        pclm_file::inspect(&path),
        PclmInfo {
            kind: PclmKind::Ok,
            role: Some(PclmRole::WORK.into()),
            dataset_id: Some(id),
            version: schema::BASELINE_VERSION,
        }
    );
}

#[test]
fn every_role_is_recognized_as_itself() {
    let t = Temp::new("roles");
    let env = TestEnv::new();
    for role in PclmRole::ALL {
        let path = t.path(&format!("{role}.pclm"));
        pclm_file::create(&path, role, &env).unwrap();
        let info = pclm_file::inspect(&path);
        assert_eq!(
            (info.kind, info.role.as_deref(), info.version),
            (PclmKind::Ok, Some(role), schema::VERSION)
        );
        let id = info.dataset_id.unwrap();
        assert!(
            id.len() == 32
                && id
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        );
        assert_eq!(pragma(&path, "application_id"), APPLICATION_ID);
        // 예시 보기는 작업자료에만 심는다.
        let views = user_view_builder::read(&raw_read(&path)).unwrap();
        assert_eq!(views.len(), usize::from(role == PclmRole::WORK), "{role}");
    }
    assert!(matches!(
        pclm_file::create(&t.path("x.pclm"), "draft", &env),
        Err(Error::Argument(_))
    ));
    assert!(!t.path("x.pclm").exists());
}

#[test]
fn inspecting_does_not_change_the_file() {
    let t = Temp::new("inspect");
    let path = single_file(&t, "제출.pclm", PclmRole::SUBMISSION, &TestEnv::new());
    let before = std::fs::read(&path).unwrap();
    assert_eq!(pclm_file::inspect(&path).kind, PclmKind::Ok);
    assert_untouched(&path, &before);
}

// ── 짓기와 열기 ─────────────────────────────────────────────────────

#[test]
fn create_refuses_an_occupied_place_without_touching_it() {
    let t = Temp::new("occupied");
    let env = TestEnv::new();
    let other = t.path("남의 것.pclm");
    std::fs::write(&other, "남의 것").unwrap();
    let before = std::fs::read(&other).unwrap();
    assert!(matches!(
        pclm_file::create(&other, PclmRole::WORK, &env),
        Err(Error::Io(_))
    ));
    assert_eq!(before, std::fs::read(&other).unwrap());

    // 본체가 없어도 -wal 이 남아 있으면 누가 쓰던 자리다.
    let orphan = t.path("고아.pclm");
    std::fs::write(suffixed(&orphan, "-wal"), "").unwrap();
    assert!(matches!(
        pclm_file::create(&orphan, PclmRole::WORK, &env),
        Err(Error::Io(_))
    ));
    assert!(!orphan.exists());
}

/// C# 은 폴더를 지어 주지만 이 크레이트는 짓지 않는다(#8) — 폴더가 없으면 아무것도 서지 않는다.
#[test]
fn create_does_not_make_folders() {
    let t = Temp::new("nofolder");
    let path = t.path("없는 폴더").join("작업.pclm");
    let error = pclm_file::create(&path, PclmRole::WORK, &TestEnv::new()).unwrap_err();
    assert_eq!(code(&error), Some(14), "{error}");
    assert!(!path.parent().unwrap().exists());
}

#[test]
fn missing_file_is_neither_opened_nor_made() {
    let t = Temp::new("typo");
    let path = t.path("오타").join("계약자료.pclm");
    let Err(Error::PclmFile(PclmFileError { kind, message, .. })) = pclm_file::open_work(&path)
    else {
        panic!("열면 안 됩니다")
    };
    assert_eq!(kind, PclmKind::NotFound);
    assert!(message.contains(&path.display().to_string()));
    assert!(!path.parent().unwrap().exists());

    // Database 손잡이도 짓지 않는다 — 열면 SQLite 가 실패하고 끝이다.
    let error = Database::new(&path, Access::Write).open().unwrap_err();
    assert_eq!(code(&error), Some(14));
    assert!(!path.parent().unwrap().exists());
}

#[test]
fn submission_is_not_opened_as_work() {
    let t = Temp::new("submission");
    let path = single_file(&t, "제출.pclm", PclmRole::SUBMISSION, &TestEnv::new());
    let Err(Error::PclmFile(e)) = pclm_file::open_work(&path) else {
        panic!("열면 안 됩니다")
    };
    assert_eq!(
        (e.kind, e.role.as_deref()),
        (PclmKind::Ok, Some(PclmRole::SUBMISSION))
    );
    assert!(e.message.contains("제출본"), "{}", e.message);
}

/// 옮겨진 옛 자료는 쓰기 손잡이로 열지 않는다(ADR-031). 읽기 손잡이는 연다.
#[test]
fn retired_file_refuses_write_handles() {
    let t = Temp::new("retired");
    let path = t.path("옮긴.pclm");
    pclm_file::create(&path, PclmRole::RETIRED, &TestEnv::new()).unwrap();
    let Err(Error::InvalidOperation(message)) = Database::new(&path, Access::Write).open() else {
        panic!("열면 안 됩니다")
    };
    assert!(message.contains("다른 자리로 옮겨졌습니다"), "{message}");
    Database::new(&path, Access::Read).open().unwrap();
}

// ── v19 → 지금 ──────────────────────────────────────────────────────

/// C# `판올림은_원본과_신원을_보존하고_근거_없는_링크를_버린다` 가운데 뷰 밖의 것 — 신원이 이어지고, 자료는 그대로이고,
/// 명시 참조 없는 옛 연결은 버린다.
#[test]
fn migration_keeps_identity_and_rows_and_drops_unsupported_links() {
    let t = Temp::new("v19up");
    let path = t.path("옛 계약자료.db");
    legacy_v19(&path, Some(FILL));
    let legacy = pclm_file::inspect(&path);
    let tables = [
        "request",
        "request_item",
        "notice",
        "notice_item",
        "contract",
        "contract_item",
        "contract_user_field",
        "field_override",
        "plan",
    ];
    let before: Vec<i64> = {
        let c = raw_read(&path);
        tables.iter().map(|t| count(&c, t)).collect()
    };

    pclm_file::open_work(&path)
        .unwrap()
        .migrate(&TestEnv::new())
        .unwrap();

    let after = pclm_file::inspect(&path);
    assert_eq!(
        after,
        PclmInfo {
            kind: PclmKind::Ok,
            role: Some(PclmRole::WORK.into()),
            dataset_id: legacy.dataset_id,
            version: schema::VERSION,
        }
    );
    assert_eq!(pragma(&path, "application_id"), APPLICATION_ID);
    let c = pclm_file::open_work(&path).unwrap().open().unwrap();
    assert_eq!(count(&c, "project_link"), 0);
    assert_eq!(count(&c, "request_link"), 0);
    assert_eq!(
        before,
        tables.iter().map(|t| count(&c, t)).collect::<Vec<_>>()
    );
    // v19 를 지나는 작업자료는 예시 보기를 받는다(v26 을 심고 v27 이 지금 구성으로 갈아 끼운다).
    let views = user_view_builder::read(&c).unwrap();
    assert_eq!(views.len(), 1);
    assert_eq!(
        column_names(&views[0].definition),
        builtin_field_names(builtin_view_package())
    );
}

/// 명시 참조가 있으면 판올림 끝(과 [`resolve_links`])이 연결을 다시 세운다.
#[test]
fn explicit_references_rebuild_links() {
    let t = Temp::new("links");
    let env = TestEnv::new();
    let work = pclm_file::create(&t.path("작업.pclm"), PclmRole::WORK, &env).unwrap();
    work.open()
        .unwrap()
        .execute_batch(&format!(
            "{FILL}
UPDATE notice_item SET ref_request_base = 'R-1', ref_request_seq = '0', ref_request_item = '1';
UPDATE request_item SET ref_request_base = 'R-1', ref_request_seq = '0', ref_request_item = '1';
UPDATE contract SET ref_notice_base = 'N-1';
INSERT INTO contract_series (contract_base) VALUES ('C-2');
INSERT INTO contract (contract_base, seq, title, updated_at) VALUES ('C-2', '0', '참조 없는 계약', '2026-09-04T09:00:00.0000000Z');"
        ))
        .unwrap();
    // 접수 하나, 계약 하나가 잇는다. 참조 없는 계약은 잇지 않는다.
    assert_eq!(resolve_links(&work).unwrap(), 2);
    let c = work.open_read_only().unwrap();
    let link = |sql: &str| -> Vec<String> {
        c.prepare(sql)
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    };
    assert_eq!(
        link("SELECT request_base || '>' || notice_group FROM request_link"),
        ["R-1>N-1"]
    );
    assert_eq!(
        link("SELECT contract_base || '>' || notice_group FROM project_link"),
        ["C-1>N-1"]
    );
    assert_eq!(
        link(
            "SELECT request_base || '>' || notice_base || '/' || notice_line_no FROM request_item_link"
        ),
        ["R-1>N-1/1"]
    );
}

// ── 읽기 전용 ───────────────────────────────────────────────────────

/// C# `읽기_손잡이는_쓰지_못하고_읽기만_한다` 가운데 저장 Core 몫 — SQLite 가 읽기 전용이라서(코드 8) 거절하고,
/// 지금 판이면 판올림도 아무것도 하지 않는다. 끝난 뒤 파일은 한 바이트도 바뀌지 않는다.
#[test]
fn read_handle_cannot_write() {
    let t = Temp::new("read");
    let env = TestEnv::new();
    let path = single_file(&t, "열람.pclm", PclmRole::SUBMISSION, &env);
    let before = std::fs::read(&path).unwrap();
    let database = Database::new(&path, Access::Read);
    {
        let c = database.open().unwrap();
        for sql in [
            "UPDATE contract_user_field SET value = '납품'",
            "INSERT INTO field_override (entity_type, base, seq, column_name, value, original, updated_at) VALUES ('contract','C-1','0','계약명','x','','t')",
            "DELETE FROM contract",
        ] {
            assert_eq!(raw_code(&c.execute(sql, []).unwrap_err()), Some(8), "{sql}");
        }
        assert_eq!(count(&c, "contract"), 1);
    }
    // 지금 판이면 아무것도 하지 않는다 — 뷰를 다시 짓는 것도 쓰기다. 다시 짓기를 부르면 SQLite 가 거절한다.
    database.migrate(&env).unwrap();
    assert_eq!(
        database.refresh_views().map_err(|e| match e {
            pclm_core::Error::Sqlite(e) => raw_code(&e),
            _ => None,
        }),
        Err(Some(8))
    );
    assert_untouched(&path, &before);
}

#[test]
fn read_handle_does_not_migrate() {
    let t = Temp::new("readold");
    let path = t.path("옛 계약자료.db");
    legacy_v19(&path, Some(FILL));
    raw(&path)
        .query_row("PRAGMA journal_mode = DELETE;", [], |_| Ok(()))
        .unwrap();
    let before = std::fs::read(&path).unwrap();
    let error = Database::new(&path, Access::Read)
        .migrate(&TestEnv::new())
        .unwrap_err();
    assert!(
        matches!(error, Error::InvalidOperation(ref m) if m.contains("읽기 전용")),
        "{error}"
    );
    assert_untouched(&path, &before);
    assert_eq!(
        pragma(&path, "user_version"),
        schema::BASELINE_VERSION as i64
    );
}

// ── 사본 ────────────────────────────────────────────────────────────

/// C# `제출본은_새_신원을_달고_한_파일로_떨어진다`.
#[test]
fn submission_gets_a_new_identity_as_a_single_file() {
    let t = Temp::new("snapshot");
    let env = TestEnv::new();
    let work = pclm_file::create(&t.path("계약자료.pclm"), PclmRole::WORK, &env).unwrap();
    work.open().unwrap().execute_batch(FILL).unwrap();
    let source = pclm_file::inspect(work.path());

    let saved = pclm_file::snapshot(
        &work,
        &t.path("제출_홍길동.pclm"),
        PclmRole::SUBMISSION,
        false,
        &env,
    )
    .unwrap();
    let info = pclm_file::inspect(&saved);
    assert_eq!(
        (info.kind, info.role.as_deref(), info.version),
        (PclmKind::Ok, Some(PclmRole::SUBMISSION), schema::VERSION)
    );
    assert_ne!(source.dataset_id, info.dataset_id);
    assert_eq!(pragma(&saved, "application_id"), APPLICATION_ID);
    assert!(!suffixed(&saved, "-wal").exists() && !suffixed(&saved, "-shm").exists());
    assert_eq!(count(&raw_read(&saved), "contract"), 1);
    assert_eq!(source, pclm_file::inspect(work.path()));
}

/// C# `제출본은_작업자료_자리를_덮지_않는다`.
#[test]
fn snapshot_never_overwrites_work_files() {
    let t = Temp::new("overwrite");
    let env = TestEnv::new();
    let work = pclm_file::create(&t.path("계약자료.pclm"), PclmRole::WORK, &env).unwrap();
    work.open().unwrap().execute_batch(FILL).unwrap();
    let other = pclm_file::create(&t.path("다른_계약자료.pclm"), PclmRole::WORK, &env)
        .unwrap()
        .path()
        .to_path_buf();
    let legacy = t.path("pclm.db");
    legacy_v19(&legacy, None);
    let retired = pclm_file::create(&t.path("옮긴_옛것.pclm"), PclmRole::RETIRED, &env)
        .unwrap()
        .path()
        .to_path_buf();

    for target in [work.path().to_path_buf(), other, legacy, retired] {
        let before = std::fs::read(&target).unwrap();
        let error =
            pclm_file::snapshot(&work, &target, PclmRole::SUBMISSION, true, &env).unwrap_err();
        assert!(matches!(error, Error::InvalidOperation(_)), "{error}");
        assert_eq!(before, std::fs::read(&target).unwrap());
    }

    // 지난 제출본은 결과물이라 덮는다. 덮지 않기로 부르면 VACUUM INTO 가 거절한다.
    let old = pclm_file::snapshot(
        &work,
        &t.path("제출_지난.pclm"),
        PclmRole::SUBMISSION,
        false,
        &env,
    )
    .unwrap();
    assert!(pclm_file::snapshot(&work, &old, PclmRole::SUBMISSION, false, &env).is_err());
    let again = pclm_file::snapshot(&work, &old, PclmRole::BACKUP, true, &env).unwrap();
    assert_eq!(
        pclm_file::inspect(&again).role.as_deref(),
        Some(PclmRole::BACKUP)
    );
}

/// 열어 보기는 원본에 닿지 않는다 — 옛 판이면 사본을 올리고, 사본은 한 파일이며, 놓으면 지워진다.
#[test]
fn open_view_upgrades_a_copy_and_discards_it() {
    let t = Temp::new("view");
    let path = t.path("옛 계약자료.db");
    legacy_v19(&path, Some(FILL));
    raw(&path)
        .query_row("PRAGMA journal_mode = DELETE;", [], |_| Ok(()))
        .unwrap();
    let before = std::fs::read(&path).unwrap();
    let copy = t.path("열람-사본.pclm");
    {
        let viewed = pclm_file::open_view(&path, &copy, &TestEnv::new()).unwrap();
        assert_eq!(
            (viewed.role(), viewed.version()),
            (PclmRole::WORK, schema::BASELINE_VERSION)
        );
        assert_eq!(viewed.database().access(), Access::Read);
        assert_eq!(pragma(&copy, "user_version"), schema::VERSION as i64);
        assert!(!suffixed(&copy, "-wal").exists());
        assert_eq!(count(&viewed.database().open().unwrap(), "contract"), 1);
    }
    assert!(!copy.exists());
    assert_untouched(&path, &before);

    let missing = pclm_file::open_view(&t.path("없음.pclm"), &copy, &TestEnv::new());
    assert!(matches!(
        missing,
        Err(Error::PclmFile(PclmFileError {
            kind: PclmKind::NotFound,
            ..
        }))
    ));
}

fn files_in(directory: &Path) -> Vec<String> {
    match std::fs::read_dir(directory) {
        Ok(entries) => entries
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// C# `ViewTests.한_파일짜리_원본은_열어_보아도_바이트도_옆_파일도_그대로다` — 받은 제출본을 열어 본 것만으로 판이 오르거나 옆에
/// `-wal` 이 서면 보낸 것과 받은 것이 오류 없이 갈린다(ADR-031).
#[test]
fn viewing_a_single_file_leaves_bytes_and_neighbours_alone() {
    let t = Temp::new("view-single");
    let env = TestEnv::new();
    let path = single_file(&t, "제출_홍길동.pclm", PclmRole::SUBMISSION, &env);
    let before = std::fs::read(&path).unwrap();
    let copy = t.path("열람-사본.pclm");
    {
        let viewed = pclm_file::open_view(&path, &copy, &env).unwrap();
        assert_eq!(
            (viewed.source_path(), viewed.role(), viewed.version()),
            (path.as_path(), PclmRole::SUBMISSION, schema::VERSION)
        );
        assert_eq!(viewed.database().access(), Access::Read);
        assert_ne!(viewed.database().path(), path.as_path());
        let summary = pclm_core::storage::reports::Reports::new(Database::new(
            viewed.database().path(),
            Access::Read,
        ))
        .summary()
        .unwrap();
        assert_eq!(summary.contract_bases, 1);
        // 사본도 한 파일이다 — 읽기 손잡이가 옆에 아무것도 세우지 않는다.
        assert!(!suffixed(viewed.database().path(), "-wal").exists());
    }
    assert_untouched(&path, &before);
}

/// C# `ViewTests.쓰는_중인_작업자료도_커밋된_것까지_보인다` — 쥐고 있는 쓰기는 보이지 않는다.
#[test]
fn a_work_file_being_written_shows_what_is_committed() {
    let t = Temp::new("view-busy");
    let env = TestEnv::new();
    let work = pclm_file::create(&t.path("계약자료.pclm"), PclmRole::WORK, &env).unwrap();
    let writer = work.open().unwrap();
    writer.execute_batch(FILL).unwrap();
    // 쓰기 잠금을 쥔 채 아직 커밋하지 않은 계약 하나.
    writer
        .execute_batch(
            "BEGIN IMMEDIATE;
             INSERT INTO contract_series (contract_base) VALUES ('C-2');
             INSERT INTO contract (contract_base, seq, title, updated_at)
             VALUES ('C-2', '0', '아직 커밋 안 한 계약', '2026-09-07T09:00:00.0000000Z');",
        )
        .unwrap();
    {
        let viewed = pclm_file::open_view(work.path(), &t.path("열람.pclm"), &env).unwrap();
        assert_eq!(viewed.role(), PclmRole::WORK);
        let summary = pclm_core::storage::reports::Reports::new(Database::new(
            viewed.database().path(),
            Access::Read,
        ))
        .summary()
        .unwrap();
        assert_eq!(summary.contract_bases, 1);
    }
    writer.execute_batch("ROLLBACK;").unwrap();
}

/// C# `ViewTests.없거나_PCLM_이_아니거나_판이_맞지_않으면_열지_않고_사본도_남기지_않는다`.
#[test]
fn unreadable_files_are_not_viewed_and_leave_no_copy() {
    let t = Temp::new("view-refused");
    let env = TestEnv::new();
    let text = t.path("글.pclm");
    std::fs::write(&text, "이건 SQLite 가 아니다").unwrap();
    let old = t.path("옛판.pclm");
    raw(&old)
        .execute_batch(
            "CREATE TABLE notice (notice_base TEXT, seq TEXT); PRAGMA user_version = 11;",
        )
        .unwrap();
    let newer = single_file(&t, "새판.pclm", PclmRole::SUBMISSION, &env);
    raw(&newer)
        .execute_batch("PRAGMA user_version = 999;")
        .unwrap();
    let views = t.path("홈").join("view");
    std::fs::create_dir_all(&views).unwrap();

    for (path, kind) in [
        (t.path("없는.pclm"), PclmKind::NotFound),
        (text, PclmKind::NotPclm),
        (old, PclmKind::TooOld),
        (newer, PclmKind::TooNew),
    ] {
        let copy = views.join("열람-사본.pclm");
        match pclm_file::open_view(&path, &copy, &env) {
            Err(Error::PclmFile(e)) => {
                assert_eq!(e.kind, kind, "{}", path.display());
                assert!(
                    e.message.contains(&path.display().to_string()),
                    "{}",
                    e.message
                );
            }
            other => panic!("{}: 거절되지 않았다: {other:?}", path.display()),
        }
        assert!(!copy.exists());
    }
    assert!(files_in(&views).is_empty());
    assert!(!t.path("없는.pclm").exists());
}

/// C# `ViewTests.다_보면_사본을_지운다` — 읽은 뒤 놓으면 사본이 사라진다.
#[test]
fn letting_go_of_a_view_removes_the_copy() {
    let t = Temp::new("view-discard");
    let env = TestEnv::new();
    let path = single_file(&t, "백업.pclm", PclmRole::BACKUP, &env);
    let views = t.path("view");
    std::fs::create_dir_all(&views).unwrap();
    let copy = views.join("열람-사본.pclm");
    let viewed = pclm_file::open_view(&path, &copy, &env).unwrap();
    let report = pclm_core::storage::status::Status::new(Database::new(
        viewed.database().path(),
        Access::Read,
    ))
    .report()
    .unwrap();
    assert!(!report.groups.is_empty());
    assert!(copy.exists());
    drop(viewed);
    assert!(!copy.exists());
    assert!(files_in(&views).is_empty());
}

// ── 예시 보기 판올림 ────────────────────────────────────────────────

fn column_names(definition: &str) -> Vec<String> {
    let node: Value = serde_json::from_str(definition).unwrap();
    node["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap().to_string())
        .collect()
}

fn builtin_field_names(package: &str) -> Vec<String> {
    let node: Value = serde_json::from_str(package).unwrap();
    column_names(&node["views"][0]["definition"].to_string())
}

fn views_of(path: &Path) -> Vec<user_view_builder::StoredUserView> {
    user_view_builder::read(&raw_read(path)).unwrap()
}

/// C# `UserViewTests` 의 v25 판올림 — v26 이 예시를 심고 v27 이 지금 구성으로 갈아 끼운다. 지운 것은 되살리지 않고,
/// 같은 이름의 사람 보기는 그대로이며, 작업자료가 아니면 심지 않는다.
#[test]
fn migration_from_v25_seeds_the_current_example() {
    let t = Temp::new("v25");
    let env = TestEnv::new();
    let plain = t.path("작업.pclm");
    legacy_at(&plain, 25, PclmRole::WORK);
    Database::new(&plain, Access::Write).migrate(&env).unwrap();
    let seeded = views_of(&plain);
    assert_eq!(seeded.len(), 1);
    assert_eq!(seeded[0].name, "서식용 예시");
    assert_eq!(
        column_names(&seeded[0].definition),
        builtin_field_names(builtin_view_package())
    );

    raw(&plain).execute("DELETE FROM user_view", []).unwrap();
    Database::new(&plain, Access::Write).migrate(&env).unwrap();
    assert!(views_of(&plain).is_empty());

    let owned = t.path("내것.pclm");
    legacy_at(&owned, 25, PclmRole::WORK);
    let mine = r#"{"version":2,"contracts":"split","columns":[{"id":"c-1","name":"내 필드","formula":null}]}"#;
    raw(&owned)
        .execute(
            "INSERT INTO user_view (id, name, definition, sort_order) VALUES ('v-own', '서식용 예시', ?1, 10)",
            [mine],
        )
        .unwrap();
    Database::new(&owned, Access::Write).migrate(&env).unwrap();
    let kept = views_of(&owned);
    assert_eq!((kept.len(), kept[0].id.as_str()), (1, "v-own"));
    assert_eq!(column_names(&kept[0].definition), ["내 필드"]);

    let submission = t.path("제출.pclm");
    legacy_at(&submission, 25, PclmRole::SUBMISSION);
    Database::new(&submission, Access::Write)
        .migrate(&env)
        .unwrap();
    assert!(views_of(&submission).is_empty());
}

/// C# `판올림은_손대지_않은_예시_보기만_지금_구성으로_갈아_끼운다`.
#[test]
fn migration_from_v26_upgrades_only_untouched_examples() {
    let t = Temp::new("v26");
    let env = TestEnv::new();
    let old = |name: &str, role: &str, edit: Option<&dyn Fn(String) -> String>| {
        let path = t.path(name);
        legacy_at(&path, 26, role);
        if let Some(edit) = edit {
            let view = read_view_package(example_view_package_v26())
                .unwrap()
                .views
                .remove(0);
            raw(&path)
                .execute(
                    "INSERT INTO user_view (id, name, definition, sort_order) VALUES ('v-seed', ?1, ?2, 10)",
                    rusqlite::params![view.name, edit(stored_definition(&view).unwrap())],
                )
                .unwrap();
        }
        path
    };
    let migrated = |path: &Path| {
        Database::new(path, Access::Write).migrate(&env).unwrap();
        let views = views_of(path);
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].name, "서식용 예시");
        column_names(&views[0].definition)
    };
    let v26 = builtin_field_names(example_view_package_v26());
    let now = builtin_field_names(builtin_view_package());
    assert_ne!(v26, now);

    // 손대지 않은 것 — 그대로든, 키 순서만 갈렸든 지금 구성이다.
    assert_eq!(
        migrated(&old("그대로.pclm", PclmRole::WORK, Some(&|d| d))),
        now
    );
    let reversed = |d: String| {
        let Value::Object(map) = serde_json::from_str::<Value>(&d).unwrap() else {
            unreachable!()
        };
        let mut entries: Vec<_> = map.into_iter().collect();
        entries.reverse();
        Value::Object(entries.into_iter().collect()).to_string()
    };
    assert_eq!(
        migrated(&old("순서.pclm", PclmRole::WORK, Some(&reversed))),
        now
    );

    // 고친 것 — 필드 이름 하나만 바꿔도 사람 것이라 그대로 둔다.
    let edited = migrated(&old(
        "고침.pclm",
        PclmRole::WORK,
        Some(&|d: String| d.replace("\"게시일\"", "\"공고게시일\"")),
    ));
    assert_eq!(
        edited,
        v26.iter()
            .map(|n| if n == "게시일" {
                "공고게시일".to_string()
            } else {
                n.clone()
            })
            .collect::<Vec<_>>()
    );

    // 지운 것 — 되살리지 않는다.
    let deleted = old("지움.pclm", PclmRole::WORK, None);
    Database::new(&deleted, Access::Write)
        .migrate(&env)
        .unwrap();
    assert!(views_of(&deleted).is_empty());

    // 작업자료가 아닌 파일은 손대지 않은 예시도 그대로다.
    let submission = old("제출.pclm", PclmRole::SUBMISSION, Some(&|d| d));
    Database::new(&submission, Access::Write)
        .migrate(&env)
        .unwrap();
    assert_eq!(column_names(&views_of(&submission)[0].definition), v26);
}

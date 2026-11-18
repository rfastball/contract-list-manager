//! 저장 Core 의 계약면 시험 — 기준선 스키마, 작업자료 파일(홈 밖). 예시 보기 판올림은 store_user_views.rs 에 있다.
//! 뷰를 보는 단언은 `views.rs`·`flow_cases.rs` 에 있다.

mod common;

use std::path::Path;

use common::*;
use pclm_core::erp::explicit_links;
use pclm_core::storage::pclm_file::{self, PclmFileError};
use pclm_core::storage::store::builtin_view_package;
use pclm_core::storage::{schema, user_view_builder};
use pclm_core::{APPLICATION_ID, Access, Database, Error, PclmInfo, PclmKind, PclmRole};
use serde_json::Value;

// ── 스키마 ──────────────────────────────────────────────────────────

/// 갓 지은 작업자료의 `sqlite_master` 원문과 씨앗이 박제와 한 글자까지 같다.
#[test]
fn fresh_schema_matches_the_golden() {
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

/// 같은 `Env` 차례면 같은 파일이 선다 — 이름표·지은 때가 `Env` 에서 오고, 「문서 서식」 은 고정 식별자다.
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
    // 기준선이 1 을 뽑고, v20 은 그것을 이어받아 새로 뽑지 않는다.
    assert_eq!(id, format!("{:032x}", 1));
    assert_eq!(created, "2026-11-07T03:04:05.1230000Z");
    let view: String = c
        .query_row("SELECT id FROM user_view", [], |r| r.get(0))
        .unwrap();
    assert_eq!(view, pclm_core::storage::store::DOCUMENT_VIEW_ID);
    assert_eq!(pragma(&path, "user_version"), schema::VERSION as i64);
}

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

/// 이름에 `"` 가 든 뷰(손으로 만들었거나 남의 도구가 지은 파일)가 있어도
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

/// 단계가 실패하면 판도 뷰도 그대로다 — 뷰를 치우는 것까지 판올림 트랜잭션 안이다. 이미 올린 파일의 판이 옛
/// 값으로 남은 꼴(v29 의 열 이름 바꾸기를 다시 돌리면 실패한다)로 실패를 낸다.
#[test]
fn failed_migration_step_keeps_the_views() {
    let t = Temp::new("failed-step");
    let path = t.path("작업.pclm");
    pclm_file::create(&path, PclmRole::WORK, &TestEnv::new()).unwrap();
    raw(&path).execute_batch("PRAGMA user_version = 28;").unwrap();
    let views = || -> i64 {
        raw_read(&path)
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'view'",
                [],
                |r| r.get(0),
            )
            .unwrap()
    };
    let before = views();
    assert!(before > 0);

    Database::new(&path, Access::Write)
        .migrate(&TestEnv::new())
        .unwrap_err();

    assert_eq!(views(), before);
    assert_eq!(pragma(&path, "user_version"), 28);
}

/// 한 바이트도 고치지 않고 거절한다.
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
    let newer = single_file(&t, "새판.pclm", PclmRole::BACKUP, &env);
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
    let path = single_file(&t, "백업.pclm", PclmRole::BACKUP, &TestEnv::new());
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

/// 수집 기록을 읽는 두 물음(`capture_log::read`)은 표를 통째로 훑지 않고 시각 색인을 탄다 — 기록은 저장마다 쌓이고
/// 걷지 않으며 줄마다 스냅숏 전체를 든다.
#[test]
fn capture_history_reads_by_the_time_index() {
    let t = Temp::new("capture-index");
    let work = pclm_file::create(&t.path("작업.pclm"), PclmRole::WORK, &TestEnv::new()).unwrap();
    let c = work.open_read_only().unwrap();
    const COLUMNS: &str =
        "capture_id, profile, entity_base, entity_seq, snapshot_json, result_json, captured_at";
    for sql in [
        format!("SELECT {COLUMNS} FROM erp_capture WHERE captured_at >= ?1 ORDER BY captured_at DESC"),
        format!("SELECT {COLUMNS} FROM erp_capture ORDER BY captured_at DESC LIMIT 1"),
    ] {
        let mut statement = c.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap();
        let args = vec![""; statement.parameter_count()];
        let plan: Vec<String> = statement
            .query_map(rusqlite::params_from_iter(args), |r| r.get(3))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert!(
            plan.iter().any(|d| d.contains("ix_erp_capture_captured_at")),
            "{sql}: {plan:?}"
        );
    }
}

/// 빈 파일을 지은 뒤에 실패하면 그 파일을 걷는다 — 남기면 다시 해도 「이미 파일이 있어」 로 막힌다.
#[test]
fn failed_create_leaves_nothing_behind() {
    let t = Temp::new("create-fail");
    let path = t.path("작업.pclm");
    // -shm 자리를 폴더가 차지하면 WAL 로 여는 판올림이 실패한다.
    std::fs::create_dir(suffixed(&path, "-shm")).unwrap();

    assert!(pclm_file::create(&path, PclmRole::WORK, &TestEnv::new()).is_err());

    assert!(!path.exists());
    assert!(!suffixed(&path, "-wal").exists());
}

/// 이 크레이트는 폴더를 짓지 않는다(#8) — 폴더가 없으면 아무것도 서지 않는다.
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
fn backup_is_not_opened_as_work() {
    let t = Temp::new("backup");
    let path = single_file(&t, "받은.pclm", PclmRole::BACKUP, &TestEnv::new());
    let Err(Error::PclmFile(e)) = pclm_file::open_work(&path) else {
        panic!("열면 안 됩니다")
    };
    assert_eq!(
        (e.kind, e.role.as_deref()),
        (PclmKind::Ok, Some(PclmRole::BACKUP))
    );
    assert!(e.message.contains("백업"), "{}", e.message);
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

/// 판올림이 원본과 신원을 보존하고 근거 없는 링크를 버리는가(뷰 밖의 것) — 신원이 이어지고, 자료는 그대로이고,
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
    // v19 를 지나는 작업자료는 손대지 않은 예시를 걷고(v26 이 심고 v27 이 갈아 끼우고 v34 가 걷는다) 「문서 서식」 을 받는다.
    let views = user_view_builder::read(&c).unwrap();
    assert_eq!(views.len(), 1);
    assert_eq!(
        column_names(&views[0].definition),
        builtin_field_names(builtin_view_package())
    );
}

/// 명시 참조가 있으면 [`explicit_links::resolve`](저장 · 판올림 끝이 부르는 것)가 연결을 다시 세운다.
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
    let mut w = work.open().unwrap();
    let tx = w.transaction().unwrap();
    let states = explicit_links::resolve(&tx).unwrap();
    tx.commit().unwrap();
    assert_eq!(states.iter().filter(|r| r.state == "연결됨").count(), 2);
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

/// 읽기 손잡이는 쓰지 못하고 읽기만 한다(저장 Core 몫) — SQLite 가 읽기 전용이라서(코드 8) 거절하고,
/// 지금 판이면 판올림도 아무것도 하지 않는다. 끝난 뒤 파일은 한 바이트도 바뀌지 않는다.
#[test]
fn read_handle_cannot_write() {
    let t = Temp::new("read");
    let env = TestEnv::new();
    let path = single_file(&t, "열람.pclm", PclmRole::BACKUP, &env);
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

#[test]
fn backup_gets_a_new_identity_as_a_single_file() {
    let t = Temp::new("snapshot");
    let env = TestEnv::new();
    let work = pclm_file::create(&t.path("계약자료.pclm"), PclmRole::WORK, &env).unwrap();
    work.open().unwrap().execute_batch(FILL).unwrap();
    let source = pclm_file::inspect(work.path());

    let saved = pclm_file::snapshot(
        &work,
        &t.path("백업.pclm"),
        PclmRole::BACKUP,
        false,
        &env,
    )
    .unwrap();
    let info = pclm_file::inspect(&saved);
    assert_eq!(
        (info.kind, info.role.as_deref(), info.version),
        (PclmKind::Ok, Some(PclmRole::BACKUP), schema::VERSION)
    );
    assert_ne!(source.dataset_id, info.dataset_id);
    assert_eq!(pragma(&saved, "application_id"), APPLICATION_ID);
    assert!(!suffixed(&saved, "-wal").exists() && !suffixed(&saved, "-shm").exists());
    assert_eq!(count(&raw_read(&saved), "contract"), 1);
    assert_eq!(source, pclm_file::inspect(work.path()));
}

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
            pclm_file::snapshot(&work, &target, PclmRole::BACKUP, true, &env).unwrap_err();
        assert!(matches!(error, Error::InvalidOperation(_)), "{error}");
        assert_eq!(before, std::fs::read(&target).unwrap());
    }

    // 지난 백업은 결과물이라 덮는다. 덮지 않기로 부르면 VACUUM INTO 가 거절한다.
    let old = pclm_file::snapshot(
        &work,
        &t.path("백업_지난.pclm"),
        PclmRole::BACKUP,
        false,
        &env,
    )
    .unwrap();
    assert!(pclm_file::snapshot(&work, &old, PclmRole::BACKUP, false, &env).is_err());
    let again = pclm_file::snapshot(&work, &old, PclmRole::BACKUP, true, &env).unwrap();
    assert_eq!(
        pclm_file::inspect(&again).role.as_deref(),
        Some(PclmRole::BACKUP)
    );
}

/// 덮어 뜨다 실패하면 지난 백업이 그대로 남고 임시 파일도 남지 않는다.
#[test]
fn failed_overwrite_keeps_the_old_backup() {
    let t = Temp::new("overwrite-fail");
    let env = TestEnv::new();
    let old = single_file(&t, "백업.pclm", PclmRole::BACKUP, &env);
    let before = std::fs::read(&old).unwrap();
    let broken = t.path("깨진.pclm");
    std::fs::write(&broken, "데이터베이스가 아니다. ".repeat(100)).unwrap();
    let listing = || {
        let mut names: Vec<_> = std::fs::read_dir(old.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        names.sort();
        names
    };
    let files = listing();

    let source = Database::new(&broken, Access::Read);
    assert!(pclm_file::snapshot(&source, &old, PclmRole::BACKUP, true, &env).is_err());

    assert_eq!(before, std::fs::read(&old).unwrap());
    assert_eq!(files, listing());
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

/// 받은 파일을 열어 본 것만으로 판이 오르거나 옆에
/// `-wal` 이 서면 보낸 것과 받은 것이 오류 없이 갈린다(ADR-031).
#[test]
fn viewing_a_single_file_leaves_bytes_and_neighbours_alone() {
    let t = Temp::new("view-single");
    let env = TestEnv::new();
    let path = single_file(&t, "백업.pclm", PclmRole::BACKUP, &env);
    let before = std::fs::read(&path).unwrap();
    let copy = t.path("열람-사본.pclm");
    {
        let viewed = pclm_file::open_view(&path, &copy, &env).unwrap();
        assert_eq!(
            (viewed.source_path(), viewed.role(), viewed.version()),
            (path.as_path(), PclmRole::BACKUP, schema::VERSION)
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

/// 쥐고 있는 쓰기는 보이지 않는다.
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
    let newer = single_file(&t, "새판.pclm", PclmRole::BACKUP, &env);
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

/// 읽은 뒤 놓으면 사본이 사라진다.
#[test]
fn letting_go_of_a_view_removes_the_copy() {
    let t = Temp::new("view-discard");
    let env = TestEnv::new();
    let path = single_file(&t, "백업.pclm", PclmRole::BACKUP, &env);
    let views = t.path("view");
    std::fs::create_dir_all(&views).unwrap();
    let copy = views.join("열람-사본.pclm");
    let viewed = pclm_file::open_view(&path, &copy, &env).unwrap();
    let summary = pclm_core::storage::reports::Reports::new(Database::new(
        viewed.database().path(),
        Access::Read,
    ))
    .summary()
    .unwrap();
    assert_eq!(summary.contract_bases, 1);
    assert!(copy.exists());
    drop(viewed);
    assert!(!copy.exists());
    assert!(files_in(&views).is_empty());
}

// ── 예시 보기 ────────────────────────────────────────────────

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

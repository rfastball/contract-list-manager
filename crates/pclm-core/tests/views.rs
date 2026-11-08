//! 계약면 뷰 시험 — C# `DatasetContractTests` 전부, `FlowColumnTests.기본_열_목록이_면_뷰와_같다`,
//! `WarrantyViewTests`, `NoticeRelationTests` 의 뷰 몫, 그리고 C# 과 같은 SQL 을 받게 맞춘 연결(큰따옴표 글자 끄기,
//! `ceil`·`floor`)과 깨진 가공열 격리.
//!
//! C# `Store` 의 편집 API(`UpsertNotice` 따위)는 다음 조각이라 행을 직접 넣는다. 값은 모두 지어낸 것이다.

mod common;

use std::path::PathBuf;

use common::{Temp, TestEnv};
use pclm_core::storage::database::refresh;
use pclm_core::storage::{pclm_file, store, views};
use pclm_core::{Access, Database};
use rusqlite::{Connection, params};

struct Db {
    _temp: Temp,
    path: PathBuf,
    database: Database,
}

fn fresh() -> Db {
    let temp = Temp::new("views");
    let path = temp.path("views.pclm");
    let database = pclm_file::create(&path, "work", &TestEnv::new()).unwrap();
    Db {
        _temp: temp,
        path,
        database,
    }
}

fn columns_of(c: &Connection, view: &str) -> Vec<String> {
    c.prepare(&format!("PRAGMA table_info(\"{view}\");"))
        .unwrap()
        .query_map([], |r| r.get(1))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

fn view_names(c: &Connection) -> Vec<String> {
    c.prepare("SELECT name FROM sqlite_master WHERE type = 'view';")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

/// `Store.UserColumns(entity)` 의 이름 — 갓 지은 작업자료에도 기본 입력열이 선다.
fn people(c: &Connection, entity: &str) -> Vec<String> {
    c.prepare("SELECT field_name FROM user_column WHERE entity_type = ?1 ORDER BY entity_type, sort_order;")
        .unwrap()
        .query_map([entity], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

fn cell(db: &Db, view: &str, column: &str) -> String {
    db.database
        .open_read_only()
        .unwrap()
        .query_row(&format!("SELECT \"{column}\" FROM \"{view}\";"), [], |r| {
            r.get::<_, Option<String>>(0)
        })
        .unwrap()
        .unwrap_or_default()
}

// ── 연결 ─────────────────────────────────────────────

/// 쓰기·읽기 손잡이 모두 큰따옴표 글자를 받지 않는다(e_sqlite3 의 `SQLITE_DQS=0`). 받으면 없는 열을 큰따옴표로 적은
/// SQL 이 글로 읽혀 C# 에서 깨지는 열이 여기서는 선다.
#[test]
fn every_connection_refuses_double_quoted_strings() {
    let db = fresh();
    for c in [
        db.database.open().unwrap(),
        db.database.open_read_only().unwrap(),
        Database::new(&db.path, Access::Read).open().unwrap(),
    ] {
        let error = c
            .query_row("SELECT \"없는열\";", [], |r| r.get::<_, String>(0))
            .unwrap_err();
        assert!(error.to_string().contains("no such column"), "{error}");
    }
    // DDL 쪽(DQS_DDL) — 뷰에 박힌 큰따옴표도 글이 아니다. SQLite 는 뷰의 이름을 지을 때가 아니라 읽을 때 푼다.
    let c = db.database.open().unwrap();
    c.execute_batch("CREATE TEMP VIEW t AS SELECT \"없는열\";")
        .unwrap();
    let error = c
        .query_row("SELECT * FROM t;", [], |r| r.get::<_, String>(0))
        .unwrap_err();
    assert!(error.to_string().contains("no such column"), "{error}");
}

/// `ceil`·`ceiling`·`floor` 가 SQLite `func.c` 와 같다 — 정수는 그대로, 수로 읽히는 글은 수로, 나머지는 NULL.
#[test]
fn ceil_and_floor_follow_sqlite_math_functions() {
    let db = fresh();
    let c = db.database.open_read_only().unwrap();
    let typed = |sql: &str| -> (String, Option<String>) {
        c.query_row(
            &format!("SELECT typeof({sql}), CAST({sql} AS TEXT);"),
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap()
    };
    let cases: &[(&str, &str, Option<&str>)] = &[
        ("ceil(1.2)", "real", Some("2.0")),
        ("ceiling(-1.2)", "real", Some("-1.0")),
        ("floor(-1.5)", "real", Some("-2.0")),
        ("ceil(5)", "integer", Some("5")),
        ("floor(' 7 ')", "integer", Some("7")),
        ("ceil('1.5')", "real", Some("2.0")),
        ("ceil('1e1')", "real", Some("10.0")),
        ("floor('1,234')", "null", None),
        ("ceil(NULL)", "null", None),
        ("floor(x'31')", "null", None),
    ];
    for (sql, kind, text) in cases {
        assert_eq!(
            (kind.to_string(), text.map(str::to_string)),
            typed(sql),
            "{sql}"
        );
    }
}

// ── 계약면 표면(DatasetContractTests) ─────────────────────────

fn contract_dir() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/contract"))
}

/// `뷰_표면이_박제와_같다` — 열 이름과 순서를 C# 과 같은 박제(`views.txt`)에 견준다. 박제는 C# 시험이 쓴다 — 여기서는
/// 다시 쓰지 않는다(`PCLM_UPDATE_GOLDEN` 을 보지 않는다).
#[test]
fn view_surface_matches_the_golden() {
    let db = fresh();
    let c = db.database.open_read_only().unwrap();
    let mut text = String::new();
    for view in views::NAMES {
        let columns = columns_of(&c, view);
        text.push_str(&format!("{view}  ({}열)\n", columns.len()));
        for column in columns {
            text.push_str(&format!("  {column}\n"));
        }
        text.push('\n');
    }
    let golden = std::fs::read_to_string(contract_dir().join("views.txt")).unwrap();
    assert_eq!(
        golden.replace("\r\n", "\n").trim_end(),
        text.trim_end(),
        "계약면 표면이 박제와 다릅니다"
    );
}

/// `약속한_뷰가_모두_있다`
#[test]
fn every_promised_view_exists() {
    let db = fresh();
    let present = view_names(&db.database.open_read_only().unwrap());
    for name in views::NAMES {
        assert!(present.iter().any(|p| p == name), "{name}");
    }
}

/// `뷰의_열은_모두_문자열이다` — 품목 순번만 정수다.
#[test]
fn every_view_column_is_text() {
    let db = fresh();
    let c = db.database.open_read_only().unwrap();
    for view in views::NAMES {
        let rows: Vec<(String, String)> = c
            .prepare(&format!("PRAGMA table_info(\"{view}\");"))
            .unwrap()
            .query_map([], |r| Ok((r.get(1)?, r.get(2)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        for (name, kind) in rows {
            if name == "순번" {
                continue;
            }
            assert!(
                kind == "TEXT" || kind.is_empty(),
                "{view}.{name} 의 타입이 {kind}"
            );
        }
    }
}

/// `계약_열_목록이_계약_뷰와_같다` — 사람 열까지(지어낸 입력열 하나를 세워 본다).
#[test]
fn contract_columns_match_the_contract_view() {
    let db = fresh();
    {
        let c = db.database.open().unwrap();
        c.execute(
            "INSERT INTO user_column (entity_type, field_name, kind, options, sort_order) VALUES ('contract', '담당메모', 'text', NULL, 10);",
            [],
        )
        .unwrap();
        refresh(&c).unwrap();
    }
    let c = db.database.open_read_only().unwrap();
    let mut expected: Vec<String> = views::CONTRACT_COLUMNS
        .iter()
        .map(|s| s.to_string())
        .collect();
    expected.extend(people(&c, "contract"));
    assert!(expected.iter().any(|c| c == "담당메모"));
    assert_eq!(expected, columns_of(&c, "v_계약"));
}

/// `통합이_계약_뷰의_열을_모두_담는다`
#[test]
fn unified_holds_every_contract_column() {
    let db = fresh();
    let c = db.database.open_read_only().unwrap();
    let unified = columns_of(&c, "v_통합");
    for column in columns_of(&c, "v_계약") {
        assert!(unified.contains(&column), "{column}");
    }
}

/// `차수_뷰의_열이_본_뷰와_같다`
#[test]
fn round_views_have_the_same_columns() {
    let db = fresh();
    let c = db.database.open_read_only().unwrap();
    assert_eq!(columns_of(&c, "v_공고"), columns_of(&c, "v_공고차수"));
    assert_eq!(columns_of(&c, "v_계약"), columns_of(&c, "v_계약차수"));
}

/// `통합차수는_통합의_열에_둘만_더한_것이다`
#[test]
fn unified_rounds_add_two_columns() {
    let db = fresh();
    let c = db.database.open_read_only().unwrap();
    let mut expected = columns_of(&c, "v_통합");
    expected.extend(["공고건".to_string(), "현행공고".to_string()]);
    assert_eq!(expected, columns_of(&c, "v_통합차수"));
}

/// `기본으로_숨기는_열은_그_뷰에_있고_기본_열의_끝에_선다`
#[test]
fn hidden_columns_exist_and_close_the_base_columns() {
    let db = fresh();
    let c = db.database.open_read_only().unwrap();
    for view in views::NAMES {
        let Some(hidden) = views::hidden_by_default(view) else {
            continue;
        };
        let columns = columns_of(&c, view);
        for column in hidden {
            assert!(
                columns.iter().any(|c| c == column),
                "{view} 에 {column} 이 없습니다"
            );
        }
    }
    for (view, entity) in [
        ("v_공고", "notice"),
        ("v_계약", "contract"),
        ("v_접수", "request"),
    ] {
        let hidden = views::hidden_by_default(view).unwrap();
        let base = views::base_columns(entity).unwrap();
        assert_eq!(hidden, &base[base.len() - hidden.len()..], "{view}");
    }
    for view in ["v_공고품목", "v_계약품목", "v_접수품목"] {
        let hidden = views::hidden_by_default(view).unwrap();
        let columns = columns_of(&c, view);
        assert_eq!(hidden, &columns[columns.len() - hidden.len()..], "{view}");
    }
}

/// `옛_이름의_뷰는_열면_모두_치워진다` — 옛 뷰끼리 서로 가리키던 모양까지 세워 두고 판올림(지금 판이면 뷰 다시 짓기)을 부른다.
#[test]
fn legacy_views_are_cleared_on_open() {
    let db = fresh();
    {
        let c = Connection::open(&db.path).unwrap();
        c.execute_batch(
            "CREATE VIEW v_공고_v1 AS SELECT * FROM v_공고;
             CREATE VIEW v_계약_v1 AS SELECT * FROM v_계약;
             CREATE VIEW v_접수_v1 AS SELECT * FROM v_접수;
             CREATE VIEW v_공고품목_v1 AS SELECT * FROM v_공고품목;
             CREATE VIEW v_품목_v1 AS SELECT * FROM v_계약품목;
             CREATE VIEW v_접수품목_v1 AS SELECT * FROM v_접수품목;
             CREATE VIEW v_공고차수_v1 AS SELECT * FROM v_공고차수;
             CREATE VIEW v_계약차수_v1 AS SELECT * FROM v_계약차수;
             CREATE VIEW v_ERP원천_v1 AS SELECT * FROM v_ERP원천;
             CREATE VIEW v_계약업체_v1 AS SELECT * FROM v_계약업체;
             CREATE VIEW v_ERP접수_v1 AS SELECT * FROM v_ERP접수;
             CREATE VIEW v_통합_v1 AS
                 SELECT k.*, g.공고명 FROM v_계약_v1 k LEFT JOIN v_공고_v1 g ON 0;
             CREATE VIEW v_통합_v2 AS
                 SELECT k.계약번호, r.요청명 FROM v_통합_v1 k LEFT JOIN v_접수_v1 r ON 0;
             CREATE VIEW v_통합_v3 AS
                 SELECT k.*, g.차수 AS 공고차수 FROM v_통합_v2 k LEFT JOIN v_공고차수_v1 g ON 0;
             CREATE VIEW v_계획_v1 AS
                 SELECT r.접수번호, g.입찰공고번호, k.계약번호
                 FROM v_접수_v1 r LEFT JOIN v_공고_v1 g ON 0 LEFT JOIN v_계약_v1 k ON 0;
             CREATE VIEW v_사용자뷰 AS SELECT 1 AS 옛;",
        )
        .unwrap();
        let mut legacy: Vec<String> = view_names(&c)
            .into_iter()
            .filter(|n| views::LEGACY_NAMES.contains(&n.as_str()))
            .collect();
        legacy.sort();
        let mut expected: Vec<String> = views::LEGACY_NAMES.iter().map(|s| s.to_string()).collect();
        expected.sort();
        assert_eq!(expected, legacy);
    }

    db.database.migrate(&TestEnv::new()).unwrap();

    let present = view_names(&db.database.open_read_only().unwrap());
    for name in &present {
        assert!(
            !views::LEGACY_NAMES.contains(&name.as_str()),
            "{name} 이 남았습니다"
        );
    }
    for name in views::NAMES {
        assert!(present.iter().any(|p| p == name), "{name}");
    }
}

/// `옛_이름은_지금_이름과_겹치지_않는다`
#[test]
fn legacy_names_do_not_overlap_current_names() {
    for name in views::LEGACY_NAMES {
        assert!(!views::NAMES.contains(name), "{name}");
    }
    // 판을 붙여 부르던 것과, 판 없이 제자리에서 이름을 바꾼 것(2판의 맞춤 뷰 메타) 하나.
    let unversioned: Vec<&str> = views::LEGACY_NAMES
        .iter()
        .copied()
        .filter(|name| {
            let Some((_, tail)) = name.rsplit_once("_v") else {
                return true;
            };
            tail.is_empty() || !tail.bytes().all(|b| b.is_ascii_digit())
        })
        .collect();
    assert_eq!(vec!["v_사용자뷰"], unversioned);
}

/// `엑셀_시트_이름은_머리만_뗀_뷰_이름이다`
#[test]
fn sheet_names_drop_the_prefix() {
    let sheets: Vec<&str> = views::EXPORTED
        .iter()
        .map(|v| views::sheet_name(v))
        .collect();
    assert_eq!(
        vec![
            "통합",
            "접수",
            "접수품목",
            "공고",
            "공고품목",
            "계약",
            "계약품목",
            "ERP원천",
            "계약업체",
            "ERP접수"
        ],
        sheets
    );
    let mut distinct = sheets.clone();
    distinct.sort();
    distinct.dedup();
    assert_eq!(sheets.len(), distinct.len());
    for sheet in sheets {
        assert!((1..=31).contains(&sheet.chars().count()), "{sheet}");
    }
}

/// `FlowColumnTests.기본_열_목록이_면_뷰와_같다` — 가공열이 있으면 이 목록으로 바깥을 다시 적으므로, 어긋나면 감싼 뷰에서
/// 그 열만 사라진다.
#[test]
fn base_columns_match_the_face_views() {
    let db = fresh();
    let c = db.database.open_read_only().unwrap();
    for entity in ["notice", "contract", "request"] {
        let mut expected: Vec<String> = views::base_columns(entity)
            .unwrap()
            .iter()
            .map(|s| s.to_string())
            .collect();
        expected.extend(people(&c, entity));
        assert_eq!(
            expected,
            columns_of(&c, store::face_view(entity)),
            "{entity}"
        );
    }
}

// ── 깨진 가공열 ─────────────────────────────────────────────

/// 깨진 SQL 가공열은 뷰를 막지 않고 빈 글로 선다(`Database.IsolateComputed`). 없는 열을 큰따옴표로 적은 SQL 이 바로 그
/// 경우다 — 큰따옴표 글자를 받는 SQLite 였다면 이 열은 글 `없는열` 로 섰다. 같은 자리의 `ceil` 열은 선다.
#[test]
fn broken_sql_column_stands_empty_and_ceil_column_stands() {
    let db = fresh();
    {
        let c = db.database.open().unwrap();
        c.execute(
            "INSERT INTO contract_series (contract_base) VALUES ('TESTK0009');",
            [],
        )
        .unwrap();
        c.execute(
            "INSERT INTO contract (contract_base, seq, title, updated_at) VALUES ('TESTK0009', '00', '지어낸 계약', '2026-11-07T00:00:00Z');",
            [],
        )
        .unwrap();
        for (i, (name, sql)) in [
            ("깨진열", "SELECT \\\"없는열\\\""),
            ("올림열", "SELECT ceil(1.2)"),
        ]
        .iter()
        .enumerate()
        {
            c.execute(
                "INSERT INTO user_column (entity_type, field_name, kind, options, sort_order) VALUES ('contract', ?1, 'computed', ?2, ?3);",
                params![
                    name,
                    format!("{{\"version\":2,\"id\":\"{i:032x}\",\"source\":\"sql\",\"sql\":\"{sql}\"}}"),
                    (i as i64 + 1) * 10
                ],
            )
            .unwrap();
        }
        let refreshed = refresh(&c).unwrap();
        let broken: Vec<_> = refreshed
            .columns
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        assert_eq!(1, broken.len(), "{broken:?}");
        assert_eq!(("contract".to_string(), "깨진열".to_string()), broken[0].0);
        assert!(broken[0].1.contains("없는열"), "{}", broken[0].1);
    }
    assert_eq!("", cell(&db, "v_계약", "깨진열"));
    assert_eq!("2.0", cell(&db, "v_계약", "올림열"));
    // 깨진 자리는 뷰의 SQL 에 표지로 남아 다시 읽힌다(`Views.BrokenIn`).
    let sql: String = db
        .database
        .open_read_only()
        .unwrap()
        .query_row(
            "SELECT sql FROM sqlite_master WHERE name = 'v_계약';",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let marked = views::broken_in(Some(&sql));
    assert_eq!(1, marked.len(), "{marked:?}");
    assert_eq!("깨진열", marked[0].0);
}

/// C# `이어_붙인_칸은_순번_차례다` — 이어 붙인 세 칸은 넣은 차례가 아니라 순번(line_no) 차례다. 조달요구번호는 같은
/// 번호가 두 줄에 나뉘어도 한 번만, 처음 나온 순번 자리에 선다. 줄은 일부러 거꾸로 넣는다.
#[test]
fn joined_cells_follow_line_order() {
    let db = fresh();
    db.database
        .open()
        .unwrap()
        .execute_batch(
            "INSERT INTO request_series VALUES('TESTR9');
             INSERT INTO request(request_base,seq,updated_at) VALUES('TESTR9','000','2026-10-01');
             INSERT INTO request_item(request_base,seq,line_no,request_number) VALUES
                 ('TESTR9','000',3,'REQB'),('TESTR9','000',2,'REQA'),('TESTR9','000',1,'REQB');
             INSERT INTO notice_group VALUES('TESTN9');
             INSERT INTO notice_series VALUES('TESTN9','TESTN9');
             INSERT INTO notice(notice_base,seq,updated_at) VALUES('TESTN9','000','2026-10-01');
             INSERT INTO notice_relation(notice_base,seq,line_no,related) VALUES
                 ('TESTN9','000',2,'TESTN8-000'),('TESTN9','000',1,'TESTN7-001');
             INSERT INTO notice_restricted_industry(notice_base,seq,line_no,name) VALUES
                 ('TESTN9','000',2,'가업종'),('TESTN9','000',1,'나업종');",
        )
        .unwrap();
    let c = db.database.open_read_only().unwrap();
    let one = |sql: &str| -> String { c.query_row(sql, [], |r| r.get(0)).unwrap() };

    assert_eq!(
        one("SELECT 조달요구번호 FROM v_접수 WHERE 접수번호 = 'TESTR9-000'"),
        "REQB, REQA"
    );
    assert_eq!(
        one(
            "SELECT 관련공고 || ' / ' || 입찰참가제한업종 FROM v_공고 WHERE 입찰공고번호 = 'TESTN9-000'"
        ),
        "TESTN7-001,TESTN8-000 / 나업종, 가업종"
    );
}

// ── 하자담보기간(WarrantyViewTests) ─────────────────────────

fn notice(db: &Db) -> Connection {
    let c = db.database.open().unwrap();
    c.execute_batch(
        "INSERT INTO notice_group (group_base) VALUES ('TESTN0009');
         INSERT INTO notice_series (notice_base, group_base) VALUES ('TESTN0009', 'TESTN0009');
         INSERT INTO notice (notice_base, seq, title, updated_at) VALUES ('TESTN0009', '000', '지어낸 공고', '2026-11-07T00:00:00Z');",
    )
    .unwrap();
    pclm_core::erp::explicit_links::resolve(&c).unwrap();
    c
}

/// `개월만_받은_공고는_해와_달로_편다`
#[test]
fn warranty_months_alone_unfold_into_years_and_months() {
    for (months, expected) in [(36, "3년"), (18, "1년 6개월"), (6, "6개월"), (0, "")] {
        let db = fresh();
        notice(&db)
            .execute("UPDATE notice SET warranty_months = ?1", [months])
            .unwrap();
        assert_eq!(expected, cell(&db, "v_공고", "하자담보기간"), "{months}");
    }
}

/// `해와_달을_받은_공고는_합쳐서_편다`
#[test]
fn warranty_years_and_months_join() {
    for (years, months, expected) in [
        ("2", "0", "2년"),
        ("1", "6", "1년 6개월"),
        ("0", "6", "6개월"),
    ] {
        let db = fresh();
        notice(&db)
            .execute(
                "UPDATE notice SET warranty_years = ?1, warranty_month_part = ?2",
                [years, months],
            )
            .unwrap();
        assert_eq!(
            expected,
            cell(&db, "v_공고", "하자담보기간"),
            "{years}/{months}"
        );
    }
}

/// `문장으로_받은_공고는_글자만_다듬는다`
#[test]
fn warranty_sentences_are_only_tidied() {
    for (text, expected) in [
        ("2 년 0 개월", "2년"),
        ("0 년 6 개월", "6개월"),
        ("0 년 0 개월", ""),
        ("2 년 6 개월", "2년 6개월"),
        ("2년 10개월", "2년 10개월"),
    ] {
        let db = fresh();
        notice(&db)
            .execute("UPDATE notice SET warranty_text = ?1", [text])
            .unwrap();
        assert_eq!(expected, cell(&db, "v_공고", "하자담보기간"), "{text}");
    }
}

/// `계약의_하자담보책임기간도_같은_모양이다`
#[test]
fn contract_warranty_has_the_same_shape() {
    for (years, months, expected) in [("1", "0", "1년"), ("1", "6", "1년 6개월"), ("0", "0", "")]
    {
        let db = fresh();
        let c = db.database.open().unwrap();
        c.execute_batch(
            "INSERT INTO contract_series (contract_base) VALUES ('TESTK0009');
             INSERT INTO contract (contract_base, seq, title, updated_at) VALUES ('TESTK0009', '00', '지어낸 계약', '2026-11-07T00:00:00Z');",
        )
        .unwrap();
        c.execute(
            "UPDATE contract SET warranty_years = ?1, warranty_months = ?2",
            [years, months],
        )
        .unwrap();
        assert_eq!(
            expected,
            cell(&db, "v_계약", "하자담보책임기간"),
            "{years}/{months}"
        );
    }
}

// ── 관련공고(NoticeRelationTests 의 뷰 몫) ─────────────────────────

/// `UpsertNotice` 의 관련공고 몫 — 적힌 그대로와 가른 값을 나란히 담는다.
fn put(c: &Connection, base: &str, seq: &str, related: &[&str]) {
    c.execute(
        "INSERT OR IGNORE INTO notice_group (group_base) VALUES (?1);",
        [base],
    )
    .unwrap();
    c.execute(
        "INSERT OR IGNORE INTO notice_series (notice_base, group_base) VALUES (?1, ?1);",
        [base],
    )
    .unwrap();
    let kind = if seq == "001" {
        "취소공고"
    } else {
        "등록공고"
    };
    c.execute(
        "INSERT INTO notice (notice_base, seq, title, notice_kind, updated_at) VALUES (?1, ?2, '지어낸 공고', ?3, '2026-11-07T00:00:00Z')
         ON CONFLICT(notice_base, seq) DO UPDATE SET notice_kind = excluded.notice_kind;",
        [base, seq, kind],
    )
    .unwrap();
    c.execute(
        "DELETE FROM notice_relation WHERE notice_base = ?1 AND seq = ?2;",
        [base, seq],
    )
    .unwrap();
    for (i, text) in related.iter().enumerate() {
        let split = pclm_core::domain::value_parser::split_notice_ref(Some(text));
        c.execute(
            "INSERT INTO notice_relation (notice_base, seq, line_no, related, related_base, related_seq) VALUES (?1, ?2, ?3, ?4, ?5, ?6);",
            params![
                base,
                seq,
                i as i64 + 1,
                text,
                split.as_ref().map(|s| s.0.clone()),
                split.as_ref().map(|s| s.1.clone())
            ],
        )
        .unwrap();
    }
    pclm_core::erp::explicit_links::resolve(c).unwrap();
}

/// `뷰가_적힌_그대로_낸다` — 쉼표 뒤에 공백이 없다.
#[test]
fn related_notices_print_as_written() {
    let db = fresh();
    put(
        &db.database.open().unwrap(),
        "TESTN0012",
        "000",
        &["TESTN0011-001", "TESTN0011-000"],
    );
    assert_eq!(
        "TESTN0011-001,TESTN0011-000",
        cell(&db, "v_공고", "관련공고")
    );
}

/// `없으면_빈_문자열이다`
#[test]
fn missing_related_notices_are_empty_text() {
    let db = fresh();
    put(&db.database.open().unwrap(), "TESTN0011", "000", &[]);
    assert_eq!("", cell(&db, "v_공고", "관련공고"));
}

/// `취소공고도_여느_공고처럼_선다` — 계열 둘이 각각 최신 차수 하나씩. 취소되었다는 이유로 빠지지 않는다.
#[test]
fn cancelled_notices_stand_like_any_other() {
    let db = fresh();
    {
        let c = db.database.open().unwrap();
        put(&c, "TESTN0011", "000", &[]);
        put(&c, "TESTN0011", "001", &["TESTN0011-000"]);
        put(&c, "TESTN0012", "000", &["TESTN0011-001", "TESTN0011-000"]);
    }
    let rows: Vec<(String, String)> = db
        .database
        .open_read_only()
        .unwrap()
        .prepare("SELECT 입찰공고번호, 공고종류 FROM v_공고 ORDER BY 1;")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(2, rows.len());
    assert_eq!(
        ("TESTN0011-001".to_string(), "취소공고".to_string()),
        rows[0]
    );
    assert_eq!("TESTN0012-000", rows[1].0);
}

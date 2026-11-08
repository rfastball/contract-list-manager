//! 시험이 함께 쓰는 것 — 정해진 시계·식별자, 임시 폴더, 지어낸 자료, 스키마 뜨기.
//!
//! 값은 전부 지어낸 것이다(CLAUDE.md). 파일은 `std::env::temp_dir()` 아래 시험마다 새 폴더에만 짓는다.

#![allow(dead_code)]

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

pub mod store_fixture;

use pclm_core::Env;
use pclm_core::storage::schema;
use rusqlite::Connection;

/// 정해진 시각과 차례대로 늘어나는 식별자. 같은 차례로 부르면 같은 파일이 선다.
pub struct TestEnv {
    next: Cell<u64>,
}

impl TestEnv {
    pub fn new() -> Self {
        TestEnv { next: Cell::new(1) }
    }
}

impl Env for TestEnv {
    fn now_utc(&self) -> String {
        "2026-11-07T03:04:05.1234567Z".into()
    }

    fn new_id(&self) -> String {
        let n = self.next.get();
        self.next.set(n + 1);
        format!("{n:032x}")
    }
}

/// 시험 하나의 폴더. 놓으면 지운다.
pub struct Temp {
    pub root: PathBuf,
}

static COUNTER: AtomicU32 = AtomicU32::new(0);

impl Temp {
    pub fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "pclm-core-{name}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        Temp { root }
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// 경로 뒤에 글을 붙인다(`-wal`·`-shm`).
pub fn suffixed(path: &Path, suffix: &str) -> PathBuf {
    let mut text = path.as_os_str().to_os_string();
    text.push(suffix);
    PathBuf::from(text)
}

/// 접수 → 공고 → 계약 한 줄과 품목·링크·사람 값·덮개·계획(C# `PclmFileTests.채움`, 지어낸 값).
pub const FILL: &str = "
INSERT INTO request_series (request_base) VALUES ('R-1');
INSERT INTO request (request_base, seq, title, received_on, updated_at)
VALUES ('R-1', '0', '지어낸 접수', '2026/09/01', '2026-09-01T09:00:00.0000000Z');
INSERT INTO request_item (request_base, seq, line_no, item_name, quantity, unit_price)
VALUES ('R-1', '0', 1, '지어낸 품목', 10, '1,000');

INSERT INTO notice_group  (group_base) VALUES ('N-1');
INSERT INTO notice_series (notice_base, group_base) VALUES ('N-1', 'N-1');
INSERT INTO notice (notice_base, seq, title, posted_at, updated_at)
VALUES ('N-1', '0', '지어낸 공고', '2026/09/02', '2026-09-02T09:00:00.0000000Z');
INSERT INTO notice_item (notice_base, seq, line_no, item_name, quantity, unit_price)
VALUES ('N-1', '0', 1, '지어낸 품목', 10, '1,000');

INSERT INTO contract_series (contract_base) VALUES ('C-1');
INSERT INTO contract (contract_base, seq, title, updated_at)
VALUES ('C-1', '0', '지어낸 계약', '2026-09-03T09:00:00.0000000Z');
INSERT INTO contract_item (contract_base, seq, line_no, item_name, quantity)
VALUES ('C-1', '0', 1, '지어낸 품목', 10);

INSERT INTO request_link (request_base, notice_group) VALUES ('R-1', 'N-1');
INSERT INTO project_link (contract_base, notice_group) VALUES ('C-1', 'N-1');

INSERT INTO contract_user_field (contract_base, field_name, value, updated_at)
VALUES ('C-1', '메모', '지어낸 메모', '2026-09-05T09:00:00.0000000Z');
INSERT INTO field_override (entity_type, base, seq, column_name, value, original, updated_at)
VALUES ('contract', 'C-1', '0', '계약금액', '1,234', '', '2026-09-05T09:00:00.0000000Z');

INSERT INTO plan (request_number, item_name, quantity, source_name, imported_at)
VALUES ('P-1', '지어낸 계획 품목', 3, '지어낸 계획.xlsx', '2026-09-06T09:00:00.0000000Z');
";

/// 쓰기로 바로 연다(시험이 옛 판 파일을 지을 때). 이 크레이트를 거치지 않는다.
pub fn raw(path: &Path) -> Connection {
    let c = Connection::open(path).unwrap();
    c.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
    c
}

/// 읽기 전용으로 바로 연다.
pub fn raw_read(path: &Path) -> Connection {
    Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap()
}

pub fn pragma(path: &Path, name: &str) -> i64 {
    raw_read(path)
        .query_row(&format!("PRAGMA {name};"), [], |r| r.get(0))
        .unwrap()
}

/// 표지가 생기기 전(v19)의 작업자료. 0.7.0 이 지은 꼴 — 기준선 스키마에 WAL(뷰는 이 단계에 없다).
pub fn legacy_v19(path: &Path, sql: Option<&str>) {
    let c = raw(path);
    c.query_row("PRAGMA journal_mode = WAL;", [], |_| Ok(()))
        .unwrap();
    c.execute_batch(schema::BASELINE).unwrap();
    c.execute_batch(&format!(
        "PRAGMA user_version = {};",
        schema::BASELINE_VERSION
    ))
    .unwrap();
    if let Some(sql) = sql {
        c.execute_batch(sql).unwrap();
    }
}

/// 기준선에 단계를 `version` 까지 밟은 옛 파일(C# 시험이 `Schema.Baseline + Steps.Take(..)` 로 짓는 것). 단계마다
/// 줄을 끊는다 — 주석 한 줄뿐인 단계 뒤에 붙여 쓰면 다음 문장이 주석에 먹힌다.
pub fn legacy_at(path: &Path, version: i32, role: &str) {
    let c = raw(path);
    let mut sql = schema::BASELINE.to_string();
    for step in &schema::STEPS[..(version - schema::BASELINE_VERSION) as usize] {
        sql.push_str(step);
        sql.push('\n');
    }
    c.execute_batch(&sql).unwrap();
    c.execute("UPDATE pclm_file SET role = ?1", [role]).unwrap();
    c.execute_batch(&format!("PRAGMA user_version = {version};"))
        .unwrap();
}

pub fn count(c: &Connection, table: &str) -> i64 {
    c.query_row(&format!("SELECT count(*) FROM \"{table}\""), [], |r| {
        r.get(0)
    })
    .unwrap()
}

/// SQLite 기본 결과 코드.
pub fn code(error: &pclm_core::Error) -> Option<i32> {
    match error {
        pclm_core::Error::Sqlite(rusqlite::Error::SqliteFailure(f, _)) => {
            Some(f.extended_code & 0xff)
        }
        _ => None,
    }
}

pub fn raw_code(error: &rusqlite::Error) -> Option<i32> {
    match error {
        rusqlite::Error::SqliteFailure(f, _) => Some(f.extended_code & 0xff),
        _ => None,
    }
}

/// C# `SchemaDump.Of` — 표·색인·방아쇠의 정의 원문과 씨앗 행. DB 마다 새로 뽑히는 값은 자리표로 바꾼다.
pub fn schema_dump(c: &Connection) -> String {
    const RANDOM: [(&str, &str); 3] = [
        ("pclm_file", "dataset_id"),
        ("pclm_file", "created_at"),
        ("user_view", "id"),
    ];
    let mut text = String::new();
    let definitions: Vec<(String, String, String)> = c
        .prepare(
            "SELECT type, name, sql FROM sqlite_master
WHERE type IN ('table', 'index', 'trigger') AND name NOT LIKE 'sqlite\\_%' ESCAPE '\\'
ORDER BY type, name;",
        )
        .unwrap()
        .query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get::<_, Option<String>>(2)?.unwrap_or_default(),
            ))
        })
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    for (kind, name, sql) in &definitions {
        text.push_str(&format!("{kind} {name}\n{sql}\n\n"));
    }
    text.push_str("== 씨앗 ==\n");
    for (kind, table, _) in &definitions {
        if kind != "table" {
            continue;
        }
        let columns: Vec<String> = c
            .prepare(&format!("PRAGMA table_info(\"{table}\");"))
            .unwrap()
            .query_map([], |r| r.get(1))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        let order = (1..=columns.len())
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let mut statement = c
            .prepare(&format!("SELECT * FROM \"{table}\" ORDER BY {order};"))
            .unwrap();
        let rows: Vec<String> = statement
            .query_map([], |r| {
                Ok(columns
                    .iter()
                    .enumerate()
                    .map(|(i, column)| {
                        let value = if RANDOM.contains(&(table.as_str(), column.as_str())) {
                            "<무작위>".to_string()
                        } else {
                            literal(r.get(i).unwrap())
                        };
                        format!("{column}={value}")
                    })
                    .collect::<Vec<_>>()
                    .join(" | "))
            })
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        if rows.is_empty() {
            continue;
        }
        text.push_str(&format!("\n{table}\n"));
        for row in rows {
            text.push_str(&format!("  {row}\n"));
        }
    }
    text
}

fn literal(value: rusqlite::types::Value) -> String {
    use rusqlite::types::Value;
    match value {
        Value::Null => "NULL".into(),
        Value::Text(s) => format!("'{s}'"),
        Value::Integer(i) => i.to_string(),
        Value::Real(f) => f.to_string(),
        Value::Blob(_) => "System.Byte[]".into(),
    }
}

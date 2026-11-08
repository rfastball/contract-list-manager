//! Store 시험이 자료를 세우는 길 — C# `Store.UpsertNotice`·`UpsertContract`·`UpsertRequest`(시험 픽스처 전용,
//! `internal`)를 SQL 로 옮긴 것. Core 는 이 셋을 옮기지 않았다(프로덕션 호출자가 없다) — 시험만 쓴다.
//!
//! 레코드 대신 **SQL 열 이름과 값의 쌍**을 받는다. C# 레코드의 속성 → 열 이름은 Store.cs 의 각 Upsert 가 정본이다
//! (`Title` → `title`, `PostedAt` → `posted_at` …). 주지 않은 열은 NULL 이다 — C# 레코드의 null 속성과 같다.
//! 넣은 뒤에는 C# 처럼 같은 연결에서 명시 참조로 연결을 다시 짓는다(`ExplicitLinks.Resolve`).
//!
//! 값은 전부 지어낸 것이다(CLAUDE.md).

use pclm_core::domain::value_parser;
use pclm_core::erp::explicit_links;
use pclm_core::storage::store::Store;
use pclm_core::{Access, Database};
use rusqlite::{Connection, params_from_iter};

use super::{Temp, TestEnv};

/// 픽스처가 적는 `updated_at`. C# 은 `DateTime.UtcNow` 다 — 시험은 값을 보지 않는다.
pub const NOW: &str = "2026-01-02T03:04:05.0000000Z";

/// 시험 하나의 작업자료와 그 위의 [`Store`]. `env` 는 시험이 쥔다(시계·식별자를 정해 둔다).
pub struct Fixture {
    pub temp: Temp,
    pub path: std::path::PathBuf,
    pub env: &'static TestEnv,
}

impl Fixture {
    /// `PclmFile.Create(path, Work)` 로 빈 작업자료를 짓는다.
    pub fn new(name: &str) -> Self {
        let temp = Temp::new(name);
        let path = temp.path("store.pclm");
        // 시험 하나가 끝날 때까지만 산다 — 새는 것은 시험 프로세스의 작은 값 하나다.
        let env: &'static TestEnv = Box::leak(Box::new(TestEnv::new()));
        pclm_core::storage::pclm_file::create(&path, "work", env).unwrap();
        Fixture { temp, path, env }
    }

    pub fn database(&self) -> Database {
        Database::new(&self.path, Access::Write)
    }

    /// `new Store(database)`.
    pub fn store(&self) -> Store<'static> {
        Store::new(self.database(), self.env)
    }

    /// 쓰기 연결 하나(Core 를 거치지 않는다 — 시험이 자료를 심거나 읽을 때).
    pub fn conn(&self) -> Connection {
        let c = Connection::open(&self.path).unwrap();
        c.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        c
    }

    /// SQL 묶음을 그대로 돌린다.
    pub fn sql(&self, sql: &str) {
        self.conn().execute_batch(sql).unwrap();
    }

    /// 첫 줄 첫 칸을 글로(NULL 은 `None`, 정수는 십진 글).
    pub fn scalar(&self, sql: &str) -> Option<String> {
        use rusqlite::types::Value;
        let v: Value = self.conn().query_row(sql, [], |r| r.get(0)).unwrap();
        match v {
            Value::Null => None,
            Value::Integer(i) => Some(i.to_string()),
            Value::Real(f) => Some(f.to_string()),
            Value::Text(t) => Some(t),
            Value::Blob(_) => Some("<blob>".into()),
        }
    }

    /// 모든 줄의 첫 칸.
    pub fn column(&self, sql: &str) -> Vec<Option<String>> {
        use rusqlite::types::Value;
        let c = self.conn();
        let mut s = c.prepare(sql).unwrap();
        s.query_map([], |r| r.get::<_, Value>(0))
            .unwrap()
            .map(|v| match v.unwrap() {
                Value::Null => None,
                Value::Integer(i) => Some(i.to_string()),
                Value::Real(f) => Some(f.to_string()),
                Value::Text(t) => Some(t),
                Value::Blob(_) => Some("<blob>".into()),
            })
            .collect()
    }

    /// C# `UpsertNotice` — 건·계열을 세우고 차수 행을 업서트하고 품목을 갈아 끼운다. `related` 는 관련공고(적힌 그대로).
    pub fn notice(
        &self,
        base: &str,
        seq: &str,
        fields: &[(&str, &str)],
        items: &[&[(&str, &str)]],
        related: &[&str],
    ) {
        let c = self.conn();
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
        head(&c, "notice", "notice_base", base, seq, fields);
        children(&c, "notice_item", "notice_base", base, seq, items);
        c.execute(
            "DELETE FROM notice_relation WHERE notice_base = ?1 AND seq = ?2;",
            [base, seq],
        )
        .unwrap();
        for (i, r) in related.iter().enumerate() {
            let split = value_parser::split_notice_ref(Some(r));
            c.execute(
                "INSERT INTO notice_relation (notice_base, seq, line_no, related, related_base, related_seq)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6);",
                rusqlite::params![
                    base,
                    seq,
                    (i + 1) as i64,
                    r,
                    split.as_ref().map(|s| s.0.clone()),
                    split.as_ref().map(|s| s.1.clone())
                ],
            )
            .unwrap();
        }
        explicit_links::resolve(&c).unwrap();
    }

    /// C# `UpsertContract` — 계열을 세우고 차수 행을 업서트하고 품목을 갈아 끼운다(계약상대자 표는 시험이 따로 심는다).
    pub fn contract(
        &self,
        base: &str,
        seq: &str,
        fields: &[(&str, &str)],
        items: &[&[(&str, &str)]],
    ) {
        let c = self.conn();
        c.execute(
            "INSERT OR IGNORE INTO contract_series (contract_base) VALUES (?1);",
            [base],
        )
        .unwrap();
        head(&c, "contract", "contract_base", base, seq, fields);
        children(&c, "contract_item", "contract_base", base, seq, items);
        explicit_links::resolve(&c).unwrap();
    }

    /// C# `UpsertRequest`.
    pub fn request(
        &self,
        base: &str,
        seq: &str,
        fields: &[(&str, &str)],
        items: &[&[(&str, &str)]],
    ) {
        let c = self.conn();
        c.execute(
            "INSERT OR IGNORE INTO request_series (request_base) VALUES (?1);",
            [base],
        )
        .unwrap();
        head(&c, "request", "request_base", base, seq, fields);
        children(&c, "request_item", "request_base", base, seq, items);
        explicit_links::resolve(&c).unwrap();
    }
}

/// 차수 행 업서트 — 준 열만 바꾼다(C# 은 레코드의 모든 열을 바꾼다. 시험은 같은 차수를 다시 넣을 때 바꿀 열을 다 준다).
fn head(
    c: &Connection,
    table: &str,
    base_column: &str,
    base: &str,
    seq: &str,
    fields: &[(&str, &str)],
) {
    let mut columns = vec![base_column.to_string(), "seq".to_string()];
    let mut values = vec![base.to_string(), seq.to_string()];
    for (k, v) in fields {
        columns.push((*k).to_string());
        values.push((*v).to_string());
    }
    columns.push("updated_at".into());
    values.push(NOW.into());
    let marks: Vec<String> = (1..=values.len()).map(|i| format!("?{i}")).collect();
    let updates: Vec<String> = columns[2..]
        .iter()
        .map(|c| format!("{c} = excluded.{c}"))
        .collect();
    let sql = format!(
        "INSERT INTO {table} ({}) VALUES ({}) ON CONFLICT({base_column}, seq) DO UPDATE SET {};",
        columns.join(", "),
        marks.join(", "),
        updates.join(", ")
    );
    c.execute(&sql, params_from_iter(values.iter())).unwrap();
}

/// 딸린 줄을 통째로 갈아 끼운다. `line_no` 를 주지 않으면 1부터 차례로.
fn children(
    c: &Connection,
    table: &str,
    base_column: &str,
    base: &str,
    seq: &str,
    items: &[&[(&str, &str)]],
) {
    c.execute(
        &format!("DELETE FROM {table} WHERE {base_column} = ?1 AND seq = ?2;"),
        [base, seq],
    )
    .unwrap();
    for (i, item) in items.iter().enumerate() {
        let mut columns = vec![base_column.to_string(), "seq".to_string()];
        let mut values = vec![base.to_string(), seq.to_string()];
        if !item.iter().any(|(k, _)| *k == "line_no") {
            columns.push("line_no".into());
            values.push((i + 1).to_string());
        }
        for (k, v) in item.iter() {
            columns.push((*k).to_string());
            values.push((*v).to_string());
        }
        let marks: Vec<String> = (1..=values.len()).map(|i| format!("?{i}")).collect();
        c.execute(
            &format!(
                "INSERT INTO {table} ({}) VALUES ({});",
                columns.join(", "),
                marks.join(", ")
            ),
            params_from_iter(values.iter()),
        )
        .unwrap();
    }
}

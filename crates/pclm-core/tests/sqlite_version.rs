//! 링크된 SQLite 가 가공식이 내는 SQL 을 돌릴 수 있는가(#9 첫 작업).
//!
//! 판 숫자만 보지 않고 쓰는 구문을 실제로 돌린다 — 컴파일 선택으로 기능이 빠질 수도 있어서다.

use rusqlite::Connection;

fn memory() -> Connection {
    let connection = Connection::open_in_memory().unwrap();
    connection
        .execute_batch(
            "CREATE TABLE t (g INTEGER, v TEXT, o INTEGER);
             INSERT INTO t VALUES (1, 'b', 2), (1, 'a', 1), (2, 'c', 1);",
        )
        .unwrap();
    connection
}

#[test]
fn meets_minimum_version() {
    assert!(
        pclm_core::sqlite::meets_minimum(),
        "SQLite {}",
        pclm_core::sqlite::version()
    );
}

#[test]
fn group_concat_honours_order_by() {
    let joined: String = memory()
        .query_row(
            "SELECT group_concat(v, ', ' ORDER BY o) FROM t WHERE g = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(joined, "a, b");
}

#[test]
fn window_functions_run() {
    let ranks: Vec<(String, i64)> = memory()
        .prepare("SELECT v, row_number() OVER (PARTITION BY g ORDER BY o) FROM t ORDER BY g, o")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(ranks, [("a".into(), 1), ("b".into(), 2), ("c".into(), 1)]);
}

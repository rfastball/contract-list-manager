//! 열 관리의 다리 쪽(docs/column-management.md §11.4·§11.5) — C# `ColumnBridgeTests` 전부: SQL 열과 그 검증, 깨진
//! 열의 격리, 값 흐름 보표, 값으로 굳히기. 값은 모두 지어낸 것이다.

mod common;

use common::store_fixture::Fixture;
use pclm_core::Error;
use pclm_core::storage::flow::Flow;
use pclm_core::storage::store::{EntityRef, Store, bridge_json};
use pclm_core::storage::{computed_column, sql_column, views};
use rusqlite::types::Value as Sql;
use serde_json::{Value, json};

/// 생성자 — 공고 둘(하나는 품목 셋)과 계약 하나.
fn setup(name: &str) -> Fixture {
    let f = Fixture::new(name);
    f.notice(
        "TESTN1",
        "000",
        &[("title", "시험 공고 하나")],
        &[
            &[
                ("line_no", "1"),
                ("item_name", "책상"),
                ("detail_item_number", "001"),
            ],
            &[
                ("line_no", "2"),
                ("item_name", "책상"),
                ("detail_item_number", "001"),
            ],
            &[
                ("line_no", "3"),
                ("item_name", "의자"),
                ("detail_item_number", ""),
            ],
        ],
        &[],
    );
    f.notice(
        "TESTN2",
        "000",
        &[("title", "시험 공고 둘")],
        &[&[
            ("line_no", "1"),
            ("item_name", "모니터"),
            ("detail_item_number", "003"),
        ]],
        &[],
    );
    f.contract(
        "TESTK1",
        "00",
        &[("title", "시험 계약")],
        &[&[("line_no", "1"), ("item_name", "책상")]],
    );
    f
}

/// C# `Sql` — 기본 옵션 `JsonSerializer.Serialize` 의 꼴.
fn sql(text: &str) -> String {
    bridge_json(&json!({"version": 2, "id": "", "source": "sql", "sql": text}))
}

/// C# `Flow(from, steps)`.
fn flow(from: Value, steps: &[Value]) -> String {
    bridge_json(
        &json!({"version": 2, "id": "", "source": "flow", "flow": {"from": from, "steps": steps}}),
    )
}

fn items() -> Value {
    json!({"kind": "items"})
}

/// C# `Read` — 앱 함수 없는 독립 읽기 전용 연결에서 `QuerySingle<string>`. NULL 이면 C# 은 null 을 내 비교가 실패한다.
fn read(f: &Fixture, sql: &str) -> String {
    let consumer = common::raw_read(&f.path);
    let mut statement = consumer.prepare(sql).unwrap();
    let mut rows = statement.query([]).unwrap();
    let row = rows
        .next()
        .unwrap()
        .unwrap_or_else(|| panic!("줄이 없습니다: {sql}"));
    let value = match row.get::<_, Sql>(0).unwrap() {
        Sql::Null => panic!("NULL 이 샜습니다: {sql}"),
        Sql::Integer(i) => i.to_string(),
        Sql::Real(d) => d.to_string(),
        Sql::Text(t) => t,
        Sql::Blob(_) => "System.Byte[]".into(),
    };
    assert!(rows.next().unwrap().is_none(), "줄이 둘 이상입니다: {sql}");
    value
}

fn invalid<T: std::fmt::Debug>(result: pclm_core::Result<T>) -> String {
    match result {
        Err(Error::InvalidOperation(message)) => message,
        other => panic!("InvalidOperationException 을 바랐는데: {other:?}"),
    }
}

/// C# `Refused` — 「거절열」 로 저장하려다 받은 말.
fn refused(store: &Store, entity: &str, text: &str) -> String {
    invalid(store.save_computed_column(entity, "거절열", &sql(text), None))
}

fn options(store: &Store, entity: &str, name: &str) -> String {
    let found: Vec<_> = store
        .user_columns(Some(entity))
        .unwrap()
        .into_iter()
        .filter(|c| c.field_name == name)
        .collect();
    assert_eq!(found.len(), 1, "{entity} 의 열 {name}");
    found[0].options.clone().expect("options")
}

fn has_column(store: &Store, name: &str) -> bool {
    store
        .user_columns(None)
        .unwrap()
        .iter()
        .any(|c| c.field_name == name)
}

/// C# `ComputedColumn.ReadForEditing` — 편집 화면은 격리된 식도 원문과 진단을 함께 읽는다. Core 에 옮기지 않은 공개
/// 정적 함수라 같은 몸을 여기 둔다(`Flow.ReadForIdentity` 뒤 `migrationError` 가 있으면 원문, 없으면 `Flow.Json` 꼴).
fn read_for_editing(json: &str) -> pclm_core::Result<Value> {
    let formula = Flow::read_for_identity(json)?;
    let root: Value = serde_json::from_str(json).unwrap();
    if root.get("migrationError").is_some() {
        Ok(root)
    } else {
        Ok(serde_json::from_str(&Flow::serialize(&formula)?).unwrap())
    }
}

fn texts(value: &Value) -> Vec<String> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|x| {
            x.as_str()
                .unwrap_or_else(|| panic!("글이 아닙니다: {x}"))
                .to_string()
        })
        .collect()
}

// ── SQL 열 ─────────────────────────────────────────────

/// `SQL_열은_계약면_뷰를_읽어_뷰에_선다`
#[test]
fn sql_column_reads_face_views_and_stands_in_views() {
    let f = setup("bridge-sql");
    let store = f.store();
    store
        .save_computed_column(
            "notice",
            "품명들",
            &sql("SELECT group_concat(세부품명, ', ' ORDER BY 순번) FROM v_공고품목 WHERE 입찰공고번호 = 현재.입찰공고번호"),
            None,
        )
        .unwrap();
    assert_eq!(
        read(
            &f,
            "SELECT 품명들 FROM v_공고 WHERE 입찰공고번호 = 'TESTN1-000'"
        ),
        "책상, 책상, 의자"
    );
    assert_eq!(
        read(
            &f,
            "SELECT 품명들 FROM v_공고차수 WHERE 입찰공고번호 = 'TESTN2-000'"
        ),
        "모니터"
    );
    assert_eq!(
        read(&f, "SELECT 결과타입 FROM v_가공열 WHERE 열이름 = '품명들'"),
        "text"
    );
    let editable = read_for_editing(&options(&store, "notice", "품명들")).unwrap();
    assert_eq!(editable["source"].as_str(), Some("sql"));
    assert!(editable.get("flow").is_none());
    // 수를 내는 SQL 도 글로 찍히고, 없는 값은 NULL 이 아니라 빈 글이다.
    store
        .save_computed_column(
            "notice",
            "품목수",
            &sql("SELECT count(*) FROM v_공고품목 WHERE 입찰공고번호 = 현재.입찰공고번호"),
            None,
        )
        .unwrap();
    store
        .save_computed_column(
            "notice",
            "없는값",
            &sql("SELECT 세부품명 FROM v_공고품목 WHERE 입찰공고번호 = '없음'"),
            None,
        )
        .unwrap();
    assert_eq!(
        read(
            &f,
            "SELECT 품목수 FROM v_공고 WHERE 입찰공고번호 = 'TESTN1-000'"
        ),
        "3"
    );
    assert_eq!(
        read(
            &f,
            "SELECT 없는값 FROM v_공고 WHERE 입찰공고번호 = 'TESTN1-000'"
        ),
        ""
    );
    // 문서용 조합은 기본 열 대신 사용자가 정의한 SQL 열로 만든다.
    let entity = EntityRef::new("notice", "TESTN1", "000");
    store.set_override(&entity, "계약방법", "일반경쟁").unwrap();
    store.set_override(&entity, "계약구분", "총액계약").unwrap();
    store
        .save_computed_column(
            "notice",
            "입찰방법",
            &sql("SELECT substr(현재.계약방법, 1, 2) || '(' || replace(현재.계약구분, '계약', '') || ')'"),
            None,
        )
        .unwrap();
    assert_eq!(
        read(
            &f,
            "SELECT 입찰방법 FROM v_공고 WHERE 입찰공고번호 = 'TESTN1-000'"
        ),
        "일반(총액)"
    );
    assert_eq!(
        read(
            &f,
            "SELECT 입찰방법 FROM v_공고차수 WHERE 입찰공고번호 = 'TESTN1-000'"
        ),
        "일반(총액)"
    );
    assert!(store.broken_columns().unwrap().is_empty());
}

/// `SQL_열_검증은_읽기_밖의_것을_거절한다`
#[test]
fn sql_column_validation_refuses_outside_reads() {
    let f = setup("bridge-refuse");
    let store = f.store();
    let refused = |text: &str| refused(&store, "notice", text);
    let contains =
        |message: String, part: &str| assert!(message.contains(part), "{part} ∉ {message}");
    contains(refused("SELECT count(*) FROM notice_item"), "공개 뷰");
    contains(refused("SELECT count(*) FROM v_가공열"), "공개 뷰");
    contains(refused("SELECT count(*) FROM v_ERP원천"), "공개 뷰");
    contains(
        refused("SELECT count(*) FROM \"sqlite_master\""),
        "sqlite_master",
    );
    contains(refused("SELECT load_extension('x')"), "함수");
    contains(
        refused("SELECT name FROM pragma_table_info('notice')"),
        "pragma_table_info",
    );
    // 제 CTE 를 세우면 그 안의 읽기가 뷰 안쪽으로 보고되므로 WITH 는 받지 않는다.
    contains(
        refused(
            "SELECT (WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM c LIMIT 5) SELECT count(*) FROM c)",
        ),
        "WITH",
    );
    contains(
        refused("SELECT (with 사슬 AS (SELECT * FROM notice_item) SELECT count(*) FROM 사슬)"),
        "WITH",
    );
    // 괄호 밖으로 새거나 문장을 잇는 글은 준비하기 전에 막는다 — 뷰 정의는 여러 문장을 한 번에 돌린다.
    contains(refused("SELECT 1; DELETE FROM notice"), "하나만");
    contains(
        refused("SELECT 1) AS x FROM v_공고; DROP TABLE notice; SELECT (1"),
        "괄호",
    );
    contains(refused("DELETE FROM notice"), "SELECT");
    contains(refused("SELECT 1 /* 닫지 않음"), "주석");
    // 한 행이 값을 여럿 내면 표본 실행이 거절한다.
    contains(
        refused("SELECT DISTINCT 세부품명 FROM v_공고품목 WHERE 입찰공고번호 = 현재.입찰공고번호"),
        "2개",
    );
    // 여러 열은 준비에서 걸린다.
    invalid(store.save_computed_column("notice", "거절열", &sql("SELECT 1, 2"), None));
    assert!(!has_column(&store, "거절열"));
    assert_eq!(read(&f, "SELECT count(*) FROM notice"), "2");
}

/// `계약면_뷰는_모두_읽을_수_있다` — 뷰 안쪽의 읽기를 authorizer 가 바깥 읽기로 잘못 거절하지 않는다.
#[test]
fn every_face_view_is_readable() {
    let f = setup("bridge-readable");
    let store = f.store();
    for view in sql_column::READABLE {
        let result = store
            .validate_computed_sql("contract", &format!("SELECT count(*) FROM {view}"))
            .unwrap();
        assert_eq!(
            result["ok"].as_bool(),
            Some(true),
            "{view}: {}",
            result["error"]
        );
    }
    let refused = store
        .validate_computed_sql("contract", "SELECT count(*) FROM contract_item")
        .unwrap();
    assert_eq!(refused["ok"].as_bool(), Some(false));
    let error = refused["error"].as_str().unwrap();
    assert!(error.contains("contract_item"), "{error}");
}

/// `뷰를_도는_SQL_열은_저장하지_않는다` — v_통합 은 v_계약 을 받으므로, 계약 열이 v_통합 을 읽으면 뷰가 제 자신을 부른다.
#[test]
fn circular_sql_column_not_saved() {
    let f = setup("bridge-circular");
    let store = f.store();
    let error = refused(
        &store,
        "contract",
        "SELECT count(*) FROM v_통합 WHERE 계약번호 = 현재.계약번호",
    );
    assert!(error.contains("circularly"), "{error}");
    assert!(!has_column(&store, "거절열"));
    assert_eq!(read(&f, "SELECT count(*) FROM v_통합"), "3");
}

// ── 깨진 열 ─────────────────────────────────────────────

/// `겹친_키가_든_가공식은_뷰_짓기를_막지_않고_깨진_열이_된다` — 같은 키가 두 번 든 가공식(손으로 고친 파일)은 깨진 열로 선다.
#[test]
fn duplicate_keys_in_formula_make_a_broken_column() {
    let f = setup("bridge-duplicate-keys");
    let store = f.store();
    store
        .save_computed_column(
            "contract",
            "계약품목수",
            &flow(items(), &[json!({"op": "count"})]),
            None,
        )
        .unwrap();
    f.database()
        .open()
        .unwrap()
        .execute(
            "UPDATE user_column SET options = ?1 WHERE field_name = '계약품목수'",
            [r#"{"version":2,"version":2,"id":"","source":"flow","flow":{"from":{"kind":"items"},"steps":[{"op":"count"}]}}"#],
        )
        .unwrap();

    f.database().refresh_views().unwrap();

    assert_eq!(
        read(
            &f,
            "SELECT 계약품목수 FROM v_통합 WHERE 계약번호 = 'TESTK100'"
        ),
        ""
    );
    let broken = store.broken_columns().unwrap();
    assert_eq!(broken.len(), 1);
    assert_eq!(
        broken
            .get(&("contract".to_string(), "계약품목수".to_string()))
            .map(String::as_str),
        Some("가공식 형식이 올바르지 않습니다.")
    );
}

/// `깨진_글은_메타_뷰를_막지_않고_빈_칸이_된다` — 읽을 수 없는 글(손으로 깨뜨린 가공식·ERP 원문, 객체가 아닌 원소)은 메타
/// 뷰를 막지 않고 그 칸이 빈 글이 된다. 깨진 ERP 원문은 줄을 내지 않는다.
#[test]
fn unreadable_text_does_not_block_meta_views() {
    let f = setup("bridge-meta-broken");
    let store = f.store();
    store
        .save_computed_column(
            "contract",
            "계약품목수",
            &flow(items(), &[json!({"op": "count"})]),
            None,
        )
        .unwrap();
    f.database()
        .open()
        .unwrap()
        .execute_batch(
            r#"UPDATE user_column SET options = 'not json' WHERE field_name = '계약품목수';
INSERT INTO erp_source VALUES('notice','TESTE1','000','not json','m-1','2026-11-01');
INSERT INTO erp_source VALUES('notice','TESTE2','000',
  '{"profile":"p","rows":["x",5,{"table":"t","sourceKey":"k","values":{"공고명":"시험 공고"}}]}','m-1','2026-11-01');
INSERT INTO user_view (id, name, definition, sort_order)
  VALUES ('v-x', '모양', '{"version":2,"contracts":"split","columns":["x",5]}', 10);"#,
        )
        .unwrap();
    f.database().refresh_views().unwrap();

    assert_eq!(
        read(
            &f,
            "SELECT 식별자 FROM v_가공열 WHERE 열이름 = '계약품목수'"
        ),
        ""
    );
    assert_eq!(
        read(&f, "SELECT CAST(count(*) AS TEXT) FROM v_ERP원천"),
        "1"
    );
    assert_eq!(
        read(
            &f,
            "SELECT 내부키 || '|' || 수집프로필 || '|' || 원천표 || '|' || 원천행키 || '|' || 필드 || '|' || 수집값 FROM v_ERP원천"
        ),
        "TESTE2|p|t|k|공고명|시험 공고"
    );
    assert_eq!(
        read(
            &f,
            "SELECT group_concat(열식별자 || '|' || 열이름 || '|' || 차례 || '|' || 결과타입 || '|' || 가공식, ',' ORDER BY 차례) FROM v_맞춤뷰 WHERE 뷰식별자 = 'v-x'"
        ),
        "||1|text|,||2|text|"
    );
}

/// `깨진_정의는_뷰_짓기를_막지_않고_빈_값이_된다`
#[test]
fn broken_definition_does_not_block_views() {
    let f = setup("bridge-broken");
    let store = f.store();
    store
        .save_computed_column(
            "notice",
            "품목수",
            &sql("SELECT count(*) FROM v_공고품목 WHERE 입찰공고번호 = 현재.입찰공고번호"),
            None,
        )
        .unwrap();
    store
        .save_computed_column(
            "notice",
            "흐름",
            &flow(items(), &[json!({"op": "count"})]),
            None,
        )
        .unwrap();
    store
        .save_computed_column(
            "contract",
            "계약품목수",
            &flow(items(), &[json!({"op": "count"})]),
            None,
        )
        .unwrap();
    // 판올림이 뷰 이름을 바꾸었거나 손으로 고친 파일 — 저장된 SQL 이 없는 뷰를 읽는다.
    {
        let c = f.database().open().unwrap();
        c.execute(
            "UPDATE user_column SET options = ?1 WHERE field_name = '품목수'",
            [sql(
                "SELECT count(*) FROM v_없는뷰 WHERE 입찰공고번호 = 현재.입찰공고번호",
            )],
        )
        .unwrap();
        c.execute(
            "UPDATE user_column SET options = '{\"version\":2' WHERE field_name = '계약품목수'",
            [],
        )
        .unwrap();
    }
    f.database().refresh_views().unwrap();

    assert_eq!(
        read(
            &f,
            "SELECT 품목수 FROM v_공고 WHERE 입찰공고번호 = 'TESTN1-000'"
        ),
        ""
    );
    assert_eq!(
        read(
            &f,
            "SELECT 품목수 FROM v_공고차수 WHERE 입찰공고번호 = 'TESTN1-000'"
        ),
        ""
    );
    assert_eq!(
        read(
            &f,
            "SELECT 흐름 FROM v_공고 WHERE 입찰공고번호 = 'TESTN1-000'"
        ),
        "3"
    );
    assert_eq!(
        read(
            &f,
            "SELECT 계약품목수 FROM v_통합 WHERE 계약번호 = 'TESTK100'"
        ),
        ""
    );
    for view in views::NAMES {
        read(&f, &format!("SELECT CAST(count(*) AS TEXT) FROM {view}"));
    }

    let broken = store.broken_columns().unwrap();
    assert_eq!(broken.len(), 2);
    let notice = broken
        .get(&("notice".to_string(), "품목수".to_string()))
        .expect("품목수");
    assert!(notice.contains("v_없는뷰"), "{notice}");
    let contract = broken
        .get(&("contract".to_string(), "계약품목수".to_string()))
        .expect("계약품목수");
    assert!(contract.contains("형식"), "{contract}");
    invalid(read_for_editing(&options(&store, "contract", "계약품목수")));
    assert_eq!(
        read(&f, "SELECT 결과타입 FROM v_가공열 WHERE 열이름 = '품목수'"),
        "text"
    );

    // 다른 열을 저장하는 일도 막히지 않는다. 깨진 열을 고쳐 저장하면 깨짐이 걷힌다.
    store
        .save_computed_column(
            "notice",
            "또하나",
            &flow(items(), &[json!({"op": "count"})]),
            None,
        )
        .unwrap();
    store
        .save_computed_column(
            "notice",
            "품목수",
            &sql("SELECT count(*) FROM v_공고품목 WHERE 입찰공고번호 = 현재.입찰공고번호"),
            Some("품목수"),
        )
        .unwrap();
    assert_eq!(
        read(
            &f,
            "SELECT 품목수 FROM v_공고 WHERE 입찰공고번호 = 'TESTN1-000'"
        ),
        "3"
    );
    let keys: Vec<(String, String)> = store
        .broken_columns()
        .unwrap()
        .iter()
        .map(|(k, _)| k.clone())
        .collect();
    assert_eq!(
        keys,
        vec![("contract".to_string(), "계약품목수".to_string())]
    );
}

/// `판올림_진단이_붙은_식도_편집하고_고친_뒤에만_저장한다`
fn diagnosed_case(source: &str) {
    let f = setup("bridge-diagnosed");
    let store = f.store();
    let json = if source == "sql" {
        sql("SELECT 현재.계약법구분")
    } else {
        flow(json!({"kind": "field", "field": "계약법구분"}), &[])
    };
    store
        .save_computed_column("notice", "진단열", &json, None)
        .unwrap();
    let original: Value = serde_json::from_str(&options(&store, "notice", "진단열")).unwrap();
    let id = original["id"].as_str().map(str::to_string);
    let mut preserved = original.clone();
    const MESSAGE: &str = "판올림 뒤 읽을 열을 확인하세요.";
    preserved["migrationError"] = json!({"message": MESSAGE, "original": original});
    {
        let c = f.database().open().unwrap();
        c.execute(
            "UPDATE user_column SET options=?1 WHERE field_name='진단열'",
            [bridge_json(&preserved)],
        )
        .unwrap();
    }
    f.database().refresh_views().unwrap();

    let stored = options(&store, "notice", "진단열");
    let editable = read_for_editing(&stored).unwrap();
    assert_eq!(preserved, editable);
    let broken = store.broken_columns().unwrap();
    assert_eq!(
        broken
            .get(&("notice".to_string(), "진단열".to_string()))
            .map(String::as_str),
        Some(MESSAGE)
    );
    assert_eq!(
        read(
            &f,
            "SELECT 진단열 FROM v_공고 WHERE 입찰공고번호='TESTN1-000'"
        ),
        ""
    );
    let raw = bridge_json(&editable);
    assert_eq!(invalid(computed_column::parse(&raw)), MESSAGE);
    invalid(computed_column::expression_of("notice", &raw));
    invalid(store.save_computed_column("notice", "이름만수정", &raw, Some("진단열")));

    let mut edited = editable.clone();
    if source == "sql" {
        edited["sql"] = json!("SELECT 현재.공고명");
    } else {
        edited["flow"]["from"]["field"] = json!("공고명");
    }
    store
        .save_computed_column("notice", "진단열", &bridge_json(&edited), Some("진단열"))
        .unwrap();
    let saved = read_for_editing(&options(&store, "notice", "진단열")).unwrap();
    assert_eq!(saved["id"].as_str().map(str::to_string), id);
    assert!(saved.get("migrationError").is_none());
    assert!(store.broken_columns().unwrap().is_empty());
    assert_eq!(
        read(
            &f,
            "SELECT 진단열 FROM v_공고 WHERE 입찰공고번호='TESTN1-000'"
        ),
        "시험 공고 하나"
    );
}

/// `판올림_진단이_붙은_식도_편집하고_고친_뒤에만_저장한다("sql")`
#[test]
fn diagnosed_formula_sql() {
    diagnosed_case("sql");
}

/// `판올림_진단이_붙은_식도_편집하고_고친_뒤에만_저장한다("flow")`
#[test]
fn diagnosed_formula_flow() {
    diagnosed_case("flow");
}

// ── 보표·꺼내기 ─────────────────────────────────────────────

/// `보표는_단계마다_표본_값을_낸다`
#[test]
fn trace_gives_sample_values_per_step() {
    let f = setup("bridge-trace");
    let store = f.store();
    let steps_json = [
        json!({"op": "distinct", "fields": ["세부품명", "세부품명번호"]}),
        json!({"op": "format", "text": "{세부품명}({세부품명번호})"}),
        json!({"op": "join", "text": ", "}),
    ];
    let trace = store
        .trace_computed_column("notice", &flow(items(), &steps_json))
        .unwrap();
    assert_eq!(trace["samples"].as_array().unwrap().len(), 2);
    assert_eq!(trace["samples"][0]["base"].as_str(), Some("TESTN1"));
    let steps = trace["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 4);

    let source = &steps[0]["values"][0];
    assert_eq!(steps[0]["shape"].as_str(), Some("rows"));
    assert_eq!(source["kind"].as_str(), Some("list"));
    assert_eq!(source["count"].as_i64(), Some(3));
    assert_eq!(source["items"][0].as_str(), Some("책상 · 001"));
    assert_eq!(source["rows"][0]["세부품명번호"].as_str(), Some("001"));

    let distinct = &steps[1]["values"][0];
    assert_eq!(distinct["count"].as_i64(), Some(2));
    assert_eq!(distinct["dropped"].as_i64(), Some(1));

    let format = &steps[2]["values"][0];
    assert_eq!(steps[2]["shape"].as_str(), Some("values"));
    assert_eq!(texts(&format["items"]), vec!["책상(001)", "의자()"]);
    assert_eq!(format["blank"].as_i64(), Some(1));

    assert_eq!(
        steps[3]["values"][0]["text"].as_str(),
        Some("책상(001), 의자()")
    );
    assert_eq!(
        texts(&trace["final"]),
        vec!["책상(001), 의자()", "모니터(003)"]
    );
    assert_eq!(
        trace["sql"].as_str().map(str::to_string),
        Some(
            store
                .compile_computed_column("notice", &flow(items(), &steps_json))
                .unwrap()
        )
    );
}

/// `보표의_단계_오류는_그_단계에_붙고_뒤는_막힌다`
#[test]
fn trace_step_error_sticks_to_step_and_blocks_rest() {
    let f = setup("bridge-trace-error");
    let store = f.store();
    let trace = store
        .trace_computed_column(
            "notice",
            &flow(
                items(),
                &[
                    json!({"op": "count"}),
                    json!({"op": "join", "text": ","}),
                    json!({"op": "trim"}),
                ],
            ),
        )
        .unwrap();
    let steps = trace["steps"].as_array().unwrap();
    assert_eq!(steps[1]["shape"].as_str(), Some("number"));
    assert_eq!(steps[1]["values"][0]["text"].as_str(), Some("3"));
    let error = steps[2]["error"].as_str().unwrap();
    assert!(error.contains("잇기"), "{error}");
    assert_eq!(steps[3]["blocked"].as_bool(), Some(true));
    assert!(steps[1].get("blocked").is_none());
    assert_eq!(texts(&trace["final"]), vec!["", ""]);
    assert_eq!(trace["sql"].as_str(), Some(""));

    // SQL 열은 원천 한 줄에 표본 결과를 싣는다.
    let refused = store
        .trace_computed_column("notice", &sql("SELECT count(*) FROM notice_item"))
        .unwrap();
    let error = refused["steps"][0]["error"].as_str().unwrap();
    assert!(error.contains("공개 뷰"), "{error}");
    let ok = store
        .trace_computed_column(
            "notice",
            &sql("SELECT count(*) FROM v_공고품목 WHERE 입찰공고번호 = 현재.입찰공고번호"),
        )
        .unwrap();
    assert_eq!(texts(&ok["final"]), vec!["3", "1"]);
}

/// `열_맥락은_원천으로_쓸_재료를_낸다`
#[test]
fn column_context_gives_source_material() {
    let f = setup("bridge-context");
    let store = f.store();
    let context = store.column_context("notice").unwrap();
    let available = &context["available"];
    assert_eq!(available["view"].as_str(), Some("v_공고"));
    assert_eq!(
        texts(&available["base"]),
        views::base_columns("notice")
            .unwrap()
            .iter()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
    );
    let linked = available["linked"].as_array().unwrap();
    let many = |kind: &str| {
        let found: Vec<&Value> = linked
            .iter()
            .filter(|l| l["kind"].as_str() == Some(kind))
            .collect();
        assert_eq!(found.len(), 1, "linked {kind}");
        found[0]["many"].as_bool().unwrap()
    };
    assert!(many("contract"));
    assert!(!many("request"));
    let mut readable: Vec<String> = sql_column::READABLE.iter().map(|v| v.to_string()).collect();
    readable.sort();
    let mut schema: Vec<String> = available["schema"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    schema.sort();
    assert_eq!(readable, schema);
    assert!(texts(&available["schema"]["v_공고품목"]).contains(&"세부품명".to_string()));
}

// ── 값으로 굳히기 ─────────────────────────────────────────────

fn option_id(store: &Store, name: &str) -> String {
    let options: Value = serde_json::from_str(&options(store, "notice", name)).unwrap();
    options["id"].as_str().unwrap().to_string()
}

/// `굳히기는_센_만큼_계열의_최신_값을_쓰고_식을_남긴다`
#[test]
fn freeze_writes_latest_series_values_and_keeps_formula() {
    let f = setup("bridge-freeze");
    let store = f.store();
    // 옛 차수가 있는 계열 — 굳히는 값은 최신 차수의 것이다.
    f.notice(
        "TESTN1",
        "001",
        &[("title", "시험 공고 하나 정정")],
        &[&[
            ("line_no", "1"),
            ("item_name", "책상"),
            ("detail_item_number", "001"),
        ]],
        &[],
    );
    f.notice("TESTN3", "000", &[("title", "품목 없음")], &[], &[]);
    store
        .add_user_column("notice", "요약", "text", &[])
        .unwrap();
    store
        .set_user_field(
            &EntityRef::new("notice", "TESTN2", "000"),
            "요약",
            Some("옛 손값"),
        )
        .unwrap();
    store
        .set_user_field(
            &EntityRef::new("notice", "TESTN3", "000"),
            "요약",
            Some("숨을 값"),
        )
        .unwrap();
    let hiding = store.plan_freeze_column("notice", "요약").unwrap();
    assert_eq!(hiding.hidden, 2);
    assert_eq!(hiding.cells, 0);

    store
        .save_computed_column(
            "notice",
            "요약",
            &flow(
                items(),
                &[
                    json!({"op": "get", "field": "세부품명"}),
                    json!({"op": "join", "text": "/"}),
                ],
            ),
            Some("요약"),
        )
        .unwrap();
    let id = option_id(&store, "요약");
    assert_eq!(
        read(
            &f,
            "SELECT 요약 FROM v_공고 WHERE 입찰공고번호 = 'TESTN2-000'"
        ),
        "모니터"
    );

    let plan = store.plan_freeze_column("notice", "요약").unwrap();
    assert_eq!(plan.series, 3);
    assert_eq!(plan.cells, 2); // TESTN3 은 품목이 없어 빈 값
    assert_eq!(plan.hidden, 2);
    invalid(store.freeze_column("notice", "요약", Some(plan.cells + 1)));
    assert_eq!(
        store
            .freeze_column("notice", "요약", Some(plan.cells))
            .unwrap(),
        plan.cells
    );

    let column: Vec<_> = store
        .user_columns(Some("notice"))
        .unwrap()
        .into_iter()
        .filter(|c| c.field_name == "요약")
        .collect();
    assert_eq!(column[0].kind, "text");
    assert_eq!(option_id(&store, "요약"), id);
    assert_eq!(
        read(
            &f,
            "SELECT 요약 FROM v_공고 WHERE 입찰공고번호 = 'TESTN1-001'"
        ),
        "책상"
    );
    assert_eq!(
        read(
            &f,
            "SELECT 요약 FROM v_공고 WHERE 입찰공고번호 = 'TESTN2-000'"
        ),
        "모니터"
    );
    assert_eq!(
        read(
            &f,
            "SELECT 요약 FROM v_공고 WHERE 입찰공고번호 = 'TESTN3-000'"
        ),
        ""
    );
    assert_eq!(
        read(
            &f,
            "SELECT count(*) FROM notice_user_field WHERE field_name = '요약'"
        ),
        "2"
    );

    // 굳힌 값은 이제 사람이 고칠 수 있다. 다시 켜면 같은 식별자로 돌아온다.
    store
        .set_user_field(
            &EntityRef::new("notice", "TESTN2", "000"),
            "요약",
            Some("손으로"),
        )
        .unwrap();
    store
        .update_user_column("notice", "요약", "요약", "text", &[])
        .unwrap();
    assert_eq!(option_id(&store, "요약"), id);
    store
        .save_computed_column(
            "notice",
            "요약",
            &flow(items(), &[json!({"op": "count"})]),
            Some("요약"),
        )
        .unwrap();
    assert_eq!(
        read(&f, "SELECT 식별자 FROM v_가공열 WHERE 열이름 = '요약'"),
        id
    );
    assert_eq!(
        read(
            &f,
            "SELECT 요약 FROM v_공고 WHERE 입찰공고번호 = 'TESTN2-000'"
        ),
        "1"
    );
}

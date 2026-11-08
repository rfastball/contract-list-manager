//! 사람이 스스로 만드는 열 — C# `UserColumnTests` 전부.
//!
//! 여기서 가장 조심하는 것은 **열 이름이 뷰 SQL 에 그대로 박힌다**는 점이다. 뷰는 앱이 열릴 때마다 다시 지어지므로,
//! 깨진 이름 하나가 들어가면 그 뒤로 앱이 아예 뜨지 못한다.
//!
//! C# 의 `Store.UpsertNotice`·`UpsertContract` 는 [`Fixture`] 의 SQL 픽스처로 옮겼다. 값은 모두 지어낸 것이다.

mod common;

use common::store_fixture::Fixture;
use pclm_core::storage::store::{EntityRef, Store, bridge_json};
use pclm_core::storage::views::UserColumnDefinition;
use pclm_core::storage::{computed_column, schema};
use pclm_core::{Access, Database, Error};
use rusqlite::Connection;
use rusqlite::types::Value as Sql;
use serde_json::{Value, json};

// ── 준비 ─────────────────────────────────────────────

fn s(items: &[&str]) -> Vec<String> {
    items.iter().map(|x| x.to_string()).collect()
}

/// `Assert.Throws<InvalidOperationException>` — 그 말을 낸다.
fn invalid<T: std::fmt::Debug>(result: pclm_core::Result<T>) -> String {
    match result {
        Err(Error::InvalidOperation(message)) => message,
        other => panic!("InvalidOperationException 을 바랐는데: {other:?}"),
    }
}

fn names(columns: &[UserColumnDefinition]) -> Vec<String> {
    columns.iter().map(|c| c.field_name.clone()).collect()
}

fn column(store: &Store, entity: &str, name: &str) -> UserColumnDefinition {
    let found: Vec<_> = store
        .user_columns(Some(entity))
        .unwrap()
        .into_iter()
        .filter(|c| c.field_name == name)
        .collect();
    assert_eq!(found.len(), 1, "{entity} 의 열 {name}");
    found.into_iter().next().unwrap()
}

/// C# `ViewColumns` — `_database.OpenReadOnly()` 로 `PRAGMA table_info`.
fn view_columns(f: &Fixture, view: &str) -> Vec<String> {
    let c = f.database().open_read_only().unwrap();
    let mut statement = c
        .prepare(&format!("PRAGMA table_info(\"{view}\");"))
        .unwrap();
    statement
        .query_map([], |r| r.get::<_, String>(1))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

/// `ExecuteScalar()?.ToString() ?? ""` — 첫 줄 첫 칸, 없거나 NULL 이면 빈 글.
fn scalar_text(c: &Connection, sql: &str) -> String {
    let mut statement = c.prepare(sql).unwrap();
    let mut rows = statement.query([]).unwrap();
    match rows.next().unwrap() {
        None => String::new(),
        Some(row) => match row.get::<_, Sql>(0).unwrap() {
            Sql::Null => String::new(),
            Sql::Integer(i) => i.to_string(),
            Sql::Real(d) => d.to_string(),
            Sql::Text(t) => t,
            Sql::Blob(_) => "System.Byte[]".into(),
        },
    }
}

/// Dapper `QuerySingle<string>` — 꼭 한 줄이고, NULL 이면 C# 은 null 을 내 `Assert.Equal("", null)` 이 실패한다.
fn single_text(c: &Connection, sql: &str) -> String {
    let mut statement = c.prepare(sql).unwrap();
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

/// C# `Cell` — `v_계약` 의 첫 줄 그 열.
fn cell(f: &Fixture, column: &str) -> String {
    let c = f.database().open_read_only().unwrap();
    scalar_text(&c, &format!("SELECT \"{column}\" FROM v_계약;"))
}

fn contract() -> EntityRef {
    EntityRef::new("contract", "R26TA98000707", "00")
}

/// C# `Put`.
fn put(f: &Fixture) {
    f.contract(
        "R26TA98000707",
        "00",
        &[
            ("title", "2026년 가람 수질측정기 조달"),
            ("amount", "164872340"),
        ],
        &[],
    );
}

/// C# `ColumnStep(Op, Fields, Text, Index)`.
fn step(op: &str, fields: Option<&[&str]>, text: Option<&str>, index: Option<i32>) -> Value {
    json!({
        "op": op,
        "fields": fields.map(|f| f.to_vec()),
        "text": text,
        "index": index,
    })
}

/// C# `Formula` — `ComputedColumn.Json`(웹 기본값: camelCase, null 도 적는다)으로 쓴 v1 가공식.
fn formula(steps: &[Value]) -> String {
    bridge_json(&json!({"version": 1, "id": "", "steps": steps}))
}

/// 품목 줄(소유한 글) → 픽스처가 받는 꼴.
fn with_items<R>(items: &[Vec<(&str, String)>], body: impl FnOnce(&[&[(&str, &str)]]) -> R) -> R {
    let rows: Vec<Vec<(&str, &str)>> = items
        .iter()
        .map(|row| row.iter().map(|(k, v)| (*k, v.as_str())).collect())
        .collect();
    let slices: Vec<&[(&str, &str)]> = rows.iter().map(Vec::as_slice).collect();
    body(&slices)
}

// ── 기본 열 ─────────────────────────────────────────────

/// `기본_열은_자유_입력_둘뿐이다` — 미리 세워 주는 열은 종류마다 둘뿐이고, 모두 자유 입력이다.
fn default_columns_case(entity_type: &str, first: &str) {
    let f = Fixture::new("cols-default");
    let columns = f.store().user_columns(Some(entity_type)).unwrap();

    assert_eq!(names(&columns), s(&[first, "메모"]));
    assert!(columns.iter().all(|c| c.kind == "text"));
    assert!(columns.iter().all(|c| c.choices().is_empty()));
}

/// `기본_열은_자유_입력_둘뿐이다("contract", "진행상태")`
#[test]
fn default_columns_are_two_free_text_contract() {
    default_columns_case("contract", "진행상태");
}

/// `기본_열은_자유_입력_둘뿐이다("notice", "검토여부")`
#[test]
fn default_columns_are_two_free_text_notice() {
    default_columns_case("notice", "검토여부");
}

/// `접수에도_열을_더할_수_있다` — `user_column` 의 `CHECK` 를 넓히지 않았으면 여기서 막힌다(스키마 V11).
#[test]
fn request_can_get_columns() {
    let f = Fixture::new("cols-request");
    let store = f.store();
    store
        .add_user_column("request", "비고", "text", &[])
        .unwrap();

    assert!(view_columns(&f, "v_접수").contains(&"비고".to_string()));
    assert_eq!(
        names(&store.user_columns(Some("request")).unwrap()),
        s(&["비고"])
    );
}

/// `통합_v2_가_쓰는_이름은_계약_열로_쓸_수_없다` — `v_통합` 에만 있는 이름과도 부딪히지 않는다.
fn unified_name_case(name: &str) {
    let f = Fixture::new("cols-unified");
    let message = invalid(f.store().add_user_column("contract", name, "text", &[]));

    assert!(message.contains("표가 이미 쓰는 이름입니다"), "{message}");
}

/// `통합_v2_가_쓰는_이름은_계약_열로_쓸_수_없다("요청명")`
#[test]
fn unified_names_refused_request_title() {
    unified_name_case("요청명");
}

/// `통합_v2_가_쓰는_이름은_계약_열로_쓸_수_없다("접수번호")`
#[test]
fn unified_names_refused_request_number() {
    unified_name_case("접수번호");
}

/// `통합_v2_가_쓰는_이름은_계약_열로_쓸_수_없다("접수수수료")`
#[test]
fn unified_names_refused_request_fee() {
    unified_name_case("접수수수료");
}

/// `열을_더하면_표에_선다`
#[test]
fn added_column_appears_in_views() {
    let f = Fixture::new("cols-add");
    f.store()
        .add_user_column("contract", "담당", "text", &[])
        .unwrap();

    assert!(view_columns(&f, "v_계약").contains(&"담당".to_string()));
    assert!(view_columns(&f, "v_통합").contains(&"담당".to_string()));
}

/// `후보를_정한_열도_만들_수_있다`
#[test]
fn choice_column_can_be_made() {
    let f = Fixture::new("cols-choice");
    let store = f.store();
    store
        .add_user_column(
            "contract",
            "우선순위",
            "choice",
            &s(&["높음", "보통", "낮음"]),
        )
        .unwrap();

    let made = column(&store, "contract", "우선순위");

    assert_eq!(made.kind, "choice");
    assert_eq!(made.choices(), s(&["높음", "보통", "낮음"]));
}

/// `자유_입력_열에는_후보를_담지_않는다` — 후보는 choice 일 때만 뜻이 있다.
#[test]
fn text_column_keeps_no_choices() {
    let f = Fixture::new("cols-text-choices");
    let store = f.store();
    store
        .add_user_column("contract", "담당", "text", &s(&["가", "나"]))
        .unwrap();

    assert!(column(&store, "contract", "담당").choices().is_empty());
}

// ── 판올림 ─────────────────────────────────────────────

/// `판올림은_기본_열과_겹친_사용자_자료와_식의_원문을_보존한다` — 새 기본 열과 겹친 기존 입력·가공열은 이름과 값을
/// 보존하고, 모호한 SQL/이미 배포된 참조는 검토 전 빈값으로 격리한다.
fn migration_case(version: i32, old: &str, introduced: i32) {
    let f = Fixture::new("cols-migrate");
    let store = f.store();
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(common::suffixed(&f.path, suffix));
    }
    let moved = format!("{old}_사용자3");
    let sql_formula =
        |text: &str, id: &str| json!({"version": 2, "id": id, "source": "sql", "sql": text});
    let sql = sql_formula(&format!("SELECT 현재.{old}"), "sql-id");
    let literal = sql_formula("SELECT '계약법구분'", "literal-id");
    let number = json!({"version": 2, "id": "number-id", "source": "flow",
        "flow": {"from": {"kind": "number", "value": 42}, "steps": []}});
    let linked = json!({"version": 2, "source": "flow", "flow": {
        "from": {"kind": "linked", "link": "notice", "field": old}, "steps": []}});
    let definition = json!({"version": 2, "contracts": "split", "columns": [
        {"id": "flow-id", "name": "값", "formula": linked},
        {"id": "view-sql-id", "name": "SQL값", "formula": sql_formula(&format!("SELECT 공고.{old}"), "sql-id")}]});
    {
        // C# 처럼 외래키를 켜지 않은 맨 연결로 옛 판 파일을 짓는다. 단계는 하나씩 돌린다.
        let c = Connection::open(&f.path).unwrap();
        c.execute_batch(schema::BASELINE).unwrap();
        for step in &schema::STEPS[..(version - schema::BASELINE_VERSION) as usize] {
            c.execute_batch(step).unwrap();
        }
        c.execute_batch(&format!("PRAGMA user_version={version}"))
            .unwrap();
        c.execute_batch(&format!(
            "INSERT INTO notice_group VALUES('NOTICECOL'); INSERT INTO notice_series VALUES('NOTICECOL','NOTICECOL');
            INSERT INTO notice(notice_base,seq,updated_at) VALUES('NOTICECOL','000','2026-10-01');
            INSERT INTO contract_series VALUES('CONTRACTCOL');
            INSERT INTO contract(contract_base,seq,updated_at) VALUES('CONTRACTCOL','00','2026-10-01');
            INSERT INTO request_series VALUES('REQUESTCOL');
            INSERT INTO request(request_base,seq,updated_at) VALUES('REQUESTCOL','000','2026-10-01');
            INSERT INTO user_column VALUES('notice','{old}','text',NULL,30),('notice','{old}_사용자','text',NULL,40),
                ('contract','수요기관담당자Id','text',NULL,40),('notice','JSON후보','choice','{{\"source\":1}}',70);
            INSERT INTO notice_user_field VALUES('NOTICECOL','{old}','사용자법','2026-10-01'),
                ('NOTICECOL','{old}_사용자','기존 열','2026-10-01'),('NOTICECOL','{old}_사용자2','지운 열의 값','2026-10-01');
            INSERT INTO request_user_field VALUES('REQUESTCOL','공고종류','숨은 입력','2026-10-01');
            INSERT INTO contract_user_field VALUES('CONTRACTCOL','수요기관담당자ID','지운 ID 값','2026-10-01'),
                ('CONTRACTCOL','수요기관담당자Id','활성 Id 값','2026-10-01');"
        ))
        .unwrap();
        c.execute(
            "INSERT INTO user_column VALUES('notice','법 복사','computed',?1,50),('notice','문자','computed',?2,60),\
             ('contract','물가변동적용기준','computed',?3,30)",
            [bridge_json(&sql), bridge_json(&literal), bridge_json(&number)],
        )
        .unwrap();
        c.execute(
            "INSERT INTO user_view VALUES('view-id','이전보기',?1,10)",
            [bridge_json(&definition)],
        )
        .unwrap();
        if version >= 28 {
            c.execute_batch("UPDATE notice SET contract_law='수집법'")
                .unwrap();
        }
    }
    f.database().migrate(f.env).unwrap();
    {
        let c = f.database().open_read_only().unwrap();
        let expected = if old == "계약법구분" && version >= 28 {
            "수집법"
        } else {
            ""
        };
        assert_eq!(
            single_text(&c, &format!("SELECT {old} FROM v_공고")),
            expected
        );
        assert_eq!(
            single_text(&c, &format!("SELECT {moved} FROM v_공고")),
            "사용자법"
        );
        assert_eq!(
            single_text(&c, &format!("SELECT {old}_사용자 FROM v_공고")),
            "기존 열"
        );
        assert_eq!(
            single_text(
                &c,
                &format!("SELECT value FROM notice_user_field WHERE field_name='{old}_사용자2'")
            ),
            "지운 열의 값"
        );
        assert_eq!(
            single_text(
                &c,
                "SELECT value FROM request_user_field WHERE field_name='공고종류_사용자'"
            ),
            "숨은 입력"
        );
        assert_eq!(
            single_text(&c, "SELECT 물가변동적용기준_사용자 FROM v_계약"),
            "42"
        );
        assert_eq!(
            single_text(
                &c,
                "SELECT 수요기관담당자Id_사용자2 FROM v_통합 WHERE 계약본번호='CONTRACTCOL'"
            ),
            "활성 Id 값"
        );
        assert_eq!(
            single_text(
                &c,
                "SELECT value FROM contract_user_field WHERE field_name='수요기관담당자ID_사용자'"
            ),
            "지운 ID 값"
        );
        for column in store.user_columns(None).unwrap() {
            let length = column.field_name.encode_utf16().count();
            assert!((1..=20).contains(&length), "{}", column.field_name);
        }
        assert_eq!(
            single_text(
                &c,
                "SELECT 식별자 FROM v_가공열 WHERE 열이름='물가변동적용기준_사용자'"
            ),
            "number-id"
        );
        // SQL 문자열 리터럴은 참조가 아니다.
        assert_eq!(single_text(&c, "SELECT 문자 FROM v_공고"), "계약법구분");
        assert_eq!(single_text(&c, "SELECT [법 복사] FROM v_공고"), "");
        assert!(!view_columns(&f, "v_공고").iter().any(|n| n.contains(':')));
    }
    assert_eq!(
        column(&store, "notice", "JSON후보").options.as_deref(),
        Some("{\"source\":1}")
    );
    let broken = store.broken_columns().unwrap();
    let why = broken
        .get(&("notice".to_string(), "법 복사".to_string()))
        .expect("법 복사 가 깨진 열로 잡혀야 한다");
    assert!(why.contains("확인"), "{why}");
    let stored = column(&store, "notice", "법 복사").options.unwrap();
    let mut preserved: Value = serde_json::from_str(&stored).unwrap();
    assert_eq!(
        preserved["sql"].as_str(),
        Some(format!("SELECT 현재.{old}").as_str())
    );
    assert_eq!(preserved["id"].as_str(), Some("sql-id"));
    invalid(store.save_computed_column("notice", "법 복사", &stored, Some("법 복사")));
    preserved["sql"] = Value::String(format!("SELECT 현재.{moved}"));
    store
        .save_computed_column(
            "notice",
            "법 복사",
            &bridge_json(&preserved),
            Some("법 복사"),
        )
        .unwrap();
    let recovered = column(&store, "notice", "법 복사").options.unwrap();
    assert!(!recovered.contains("migrationError"), "{recovered}");
    let recovered: Value = serde_json::from_str(&recovered).unwrap();
    assert_eq!(recovered["id"].as_str(), Some("sql-id"));
    {
        let identity = f.database().open_read_only().unwrap();
        assert_eq!(
            single_text(
                &identity,
                "SELECT 식별자 FROM v_가공열 WHERE 열이름='법 복사'"
            ),
            "sql-id"
        );
    }
    let view = store
        .user_views()
        .unwrap()
        .into_iter()
        .find(|v| v.id == "view-id")
        .expect("view-id");
    assert_eq!(view.broken.len(), if version >= introduced { 2 } else { 1 });
    let mut recovery = view.definition.clone();
    let columns = recovery["columns"].as_array_mut().unwrap();
    assert_eq!(columns[0]["id"].as_str(), Some("flow-id"));
    let expected_field = if version >= introduced {
        old.to_string()
    } else {
        moved.clone()
    };
    assert_eq!(
        columns[0]["formula"]["flow"]["from"]["field"].as_str(),
        Some(expected_field.as_str())
    );
    columns[0]["formula"]["flow"]["from"]["field"] = Value::String(moved.clone());
    columns[1]["formula"]["sql"] = Value::String(format!("SELECT 공고.{moved}"));
    let input = json!({"name": "이전보기", "definition": recovery});
    let saved = store
        .save_user_view(&bridge_json(&input), Some("view-id"))
        .unwrap();
    let saved = saved.into_iter().find(|v| v.id == "view-id").unwrap();
    assert!(saved.broken.is_empty(), "{:?}", saved.broken);
    store
        .set_user_field(
            &EntityRef::new("notice", "NOTICECOL", "000"),
            &moved,
            Some("수정법"),
        )
        .unwrap();
    store
        .add_user_column("request", "공고종류_사용자", "text", &[])
        .unwrap();
    f.database().migrate(f.env).unwrap();
    let last = f.database().open_read_only().unwrap();
    assert_eq!(single_text(&last, "SELECT [법 복사] FROM v_공고"), "수정법");
    assert_eq!(
        single_text(&last, "SELECT 값 FROM u_이전보기 WHERE 값 <> ''"),
        "수정법"
    );
    assert_eq!(
        single_text(&last, "SELECT 공고종류_사용자 FROM v_접수"),
        "숨은 입력"
    );
}

/// `판올림은_글이_아닌_앞선_진단을_빈_것으로_여긴다` — 앞선 `migrationError.message` 가 글이 아니면(손으로 고친 파일)
/// 없는 것처럼 여기고 새 진단만 적는다. 결과 글은 C# 시험과 같은 글자다.
#[test]
fn migration_treats_non_string_previous_message_as_empty() {
    for message in ["5", "{\"a\":1}", "[1]", "true"] {
        let f = Fixture::new("cols-migrate-message");
        let store = f.store();
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(common::suffixed(&f.path, suffix));
        }
        let options = format!(
            "{{\"version\":2,\"id\":\"sql-id\",\"source\":\"sql\",\"sql\":\"SELECT 현재.재무관\",\
             \"migrationError\":{{\"message\":{message},\"original\":{{\"version\":2}}}}}}"
        );
        {
            let c = Connection::open(&f.path).unwrap();
            c.execute_batch(schema::BASELINE).unwrap();
            for step in &schema::STEPS[..(29 - schema::BASELINE_VERSION) as usize] {
                c.execute_batch(step).unwrap();
            }
            c.execute_batch("PRAGMA user_version=29").unwrap();
            c.execute(
                "INSERT INTO user_column VALUES('notice','재무관','text',NULL,30),('notice','법 복사','computed',?1,50)",
                [&options],
            )
            .unwrap();
        }

        f.database().migrate(f.env).unwrap();

        assert_eq!(
            column(&store, "notice", "법 복사").options.as_deref(),
            Some(
                "{\"version\":2,\"id\":\"sql-id\",\"source\":\"sql\",\"sql\":\"SELECT 현재.재무관\",\"migrationError\":\
                 {\"message\":\"\\n표의 새 열과 겹친 사용자 열 「재무관」을 「재무관_사용자」로 옮겼습니다. 이 식이 읽을 열을 확인하고 식을 고쳐 저장하세요.\",\
                 \"original\":{\"version\":2}}}"
            ),
            "{message}"
        );
    }
}

/// `판올림은_겹친_키가_든_식과_보기를_읽지_못한_글로_두고_연다` — 판올림이 겹친 사용자 열을 읽는 식에 진단을 다는 자리는
/// 같은 키가 두 번 든 식·맞춤 뷰 정의를 읽지 못한 글로 보아 그대로 둔다. 뒤의 값을 골라 진단을 달면 겹친 키가 조용히
/// 지워지고 C# 과 갈린다. 겹치지 않은 식에는 진단이 선다.
#[test]
fn migration_leaves_formulas_and_views_with_duplicate_keys_alone() {
    let f = Fixture::new("cols-migrate-dup");
    let store = f.store();
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(common::suffixed(&f.path, suffix));
    }
    let duplicated = "{\"version\":2,\"id\":\"sql-id\",\"source\":\"sql\",\"source\":\"sql\",\"sql\":\"SELECT 현재.재무관\"}";
    let plain =
        "{\"version\":2,\"id\":\"sql-2\",\"source\":\"sql\",\"sql\":\"SELECT 현재.재무관\"}";
    let view = format!(
        "{{\"version\":2,\"contracts\":\"split\",\"columns\":[{{\"id\":\"k1\",\"name\":\"재무\",\"formula\":{duplicated}}}]}}"
    );
    {
        let c = Connection::open(&f.path).unwrap();
        c.execute_batch(schema::BASELINE).unwrap();
        for step in &schema::STEPS[..(29 - schema::BASELINE_VERSION) as usize] {
            c.execute_batch(step).unwrap();
        }
        c.execute_batch("PRAGMA user_version=29").unwrap();
        c.execute(
            "INSERT INTO user_column VALUES('notice','재무관','text',NULL,30),('notice','법 복사','computed',?1,50),             ('notice','법 둘','computed',?2,60)",
            [duplicated, plain],
        )
        .unwrap();
        c.execute(
            "INSERT INTO user_view (id, name, definition, sort_order) VALUES ('v-dup', '겹침', ?1, 10)",
            [&view],
        )
        .unwrap();
    }

    f.database().migrate(f.env).unwrap();

    assert_eq!(
        column(&store, "notice", "법 복사").options.as_deref(),
        Some(duplicated)
    );
    assert!(
        column(&store, "notice", "법 둘")
            .options
            .unwrap()
            .contains("migrationError")
    );
    let read = Connection::open(&f.path).unwrap();
    let stored: String = read
        .query_row(
            "SELECT definition FROM user_view WHERE id = 'v-dup'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(stored, view);
}

/// `판올림은_객체가_아닌_보기_정의를_읽지_못한_글로_두고_연다` — 판올림이 겹친 사용자 열을 읽는 식에 진단을 다는 자리는
/// 객체가 아닌 맞춤 뷰 정의(배열·글·수)를 읽지 못한 글로 보아 그대로 둔다. 예전에는 C# 의 `InvalidOperationException` 을
/// 흉내 내 판올림 전체가 멈췄다.
#[test]
fn migration_leaves_non_object_view_definitions_alone() {
    for view in ["[1]", "5", "\"x\""] {
        let f = Fixture::new("cols-migrate-odd");
        let store = f.store();
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(common::suffixed(&f.path, suffix));
        }
        let plain =
            "{\"version\":2,\"id\":\"sql-2\",\"source\":\"sql\",\"sql\":\"SELECT 현재.재무관\"}";
        {
            let c = Connection::open(&f.path).unwrap();
            c.execute_batch(schema::BASELINE).unwrap();
            for step in &schema::STEPS[..(29 - schema::BASELINE_VERSION) as usize] {
                c.execute_batch(step).unwrap();
            }
            c.execute_batch("PRAGMA user_version=29").unwrap();
            c.execute(
                "INSERT INTO user_column VALUES('notice','재무관','text',NULL,30),('notice','법 둘','computed',?1,60)",
                [plain],
            )
            .unwrap();
            c.execute(
                "INSERT INTO user_view (id, name, definition, sort_order) VALUES ('v-odd', '모양', ?1, 10)",
                [view],
            )
            .unwrap();
        }

        f.database().migrate(f.env).unwrap();

        assert!(
            column(&store, "notice", "법 둘")
                .options
                .unwrap()
                .contains("migrationError"),
            "{view}"
        );
        let read = Connection::open(&f.path).unwrap();
        let stored: String = read
            .query_row(
                "SELECT definition FROM user_view WHERE id = 'v-odd'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(stored, view);
    }
}

/// `판올림은_기본_열과_겹친_사용자_자료와_식의_원문을_보존한다(27, "계약법구분", 28)`
#[test]
fn migration_preserves_user_data_v27_contract_law() {
    migration_case(27, "계약법구분", 28);
}

/// `판올림은_기본_열과_겹친_사용자_자료와_식의_원문을_보존한다(28, "계약법구분", 28)`
#[test]
fn migration_preserves_user_data_v28_contract_law() {
    migration_case(28, "계약법구분", 28);
}

/// `판올림은_기본_열과_겹친_사용자_자료와_식의_원문을_보존한다(29, "계약법구분", 28)`
#[test]
fn migration_preserves_user_data_v29_contract_law() {
    migration_case(29, "계약법구분", 28);
}

/// `판올림은_기본_열과_겹친_사용자_자료와_식의_원문을_보존한다(28, "재무관", 29)`
#[test]
fn migration_preserves_user_data_v28_finance_officer() {
    migration_case(28, "재무관", 29);
}

/// `판올림은_기본_열과_겹친_사용자_자료와_식의_원문을_보존한다(29, "재무관", 29)`
#[test]
fn migration_preserves_user_data_v29_finance_officer() {
    migration_case(29, "재무관", 29);
}

// ── 이름·차례 ─────────────────────────────────────────────

/// `이름을_바꾸면_적어_둔_값이_따라온다` — 값은 열 이름으로 매달려 있다.
#[test]
fn rename_carries_values() {
    let f = Fixture::new("cols-rename");
    let store = f.store();
    put(&f);
    store
        .set_user_field(&contract(), "메모", Some("선금 신청함"))
        .unwrap();

    store
        .update_user_column("contract", "메모", "비고", "text", &[])
        .unwrap();

    assert_eq!(cell(&f, "비고"), "선금 신청함");
}

/// `열을_지워도_값은_남고_다시_만들면_되살아난다`
#[test]
fn removed_column_values_survive_and_return() {
    let f = Fixture::new("cols-remove");
    let store = f.store();
    put(&f);
    store
        .set_user_field(&contract(), "메모", Some("선금 신청함"))
        .unwrap();

    store.remove_user_column("contract", "메모").unwrap();
    assert!(!view_columns(&f, "v_계약").contains(&"메모".to_string()));

    store
        .add_user_column("contract", "메모", "text", &[])
        .unwrap();
    assert_eq!(cell(&f, "메모"), "선금 신청함");
}

/// `차례를_옮기면_표의_열_순서가_바뀐다`
#[test]
fn moving_reorders_view_columns() {
    let f = Fixture::new("cols-move");
    let store = f.store();
    assert_eq!(
        names(&store.user_columns(Some("contract")).unwrap()),
        s(&["진행상태", "메모"])
    );

    store.move_user_column("contract", "메모", -1).unwrap();

    assert_eq!(
        names(&store.user_columns(Some("contract")).unwrap()),
        s(&["메모", "진행상태"])
    );

    let view = view_columns(&f, "v_계약");
    let at = |n: &str| view.iter().position(|c| c == n).map_or(-1, |i| i as i64);
    assert!(at("메모") < at("진행상태"));
}

// ── 막아야 하는 것 ───────────────────────────────────

/// `따옴표가_든_이름은_막는다` — 따옴표가 든 이름은 뷰 정의를 깨뜨리고, 한 번 들어가면 그 뒤로 앱이 뜨지 못한다.
fn quoted_name_case(name: &str) {
    let f = Fixture::new("cols-quote");
    invalid(f.store().add_user_column("contract", name, "text", &[]));

    // 막았으니 뷰는 멀쩡하다 — 다시 지어도 터지지 않아야 한다.
    f.database().refresh_views().unwrap();
    assert!(!view_columns(&f, "v_계약").contains(&name.to_string()));
}

/// `따옴표가_든_이름은_막는다("메모'")`
#[test]
fn quoted_name_refused_single_quote() {
    quoted_name_case("메모'");
}

/// `따옴표가_든_이름은_막는다("메모\"")`
#[test]
fn quoted_name_refused_double_quote() {
    quoted_name_case("메모\"");
}

/// `따옴표가_든_이름은_막는다("메모\\")`
#[test]
fn quoted_name_refused_backslash() {
    quoted_name_case("메모\\");
}

/// `따옴표가_든_이름은_막는다("메모', 1) AS x, (SELECT 1")`
#[test]
fn quoted_name_refused_injection() {
    quoted_name_case("메모', 1) AS x, (SELECT 1");
}

/// `표가_이미_쓰는_이름은_막는다` — 겹치는 이름은 SQLite 가 조용히 `이름:1` 로 바꾼다.
fn taken_name_case(name: &str) {
    let f = Fixture::new("cols-taken");
    let message = invalid(f.store().add_user_column("contract", name, "text", &[]));

    assert!(message.contains(name), "{message}");
}

/// `표가_이미_쓰는_이름은_막는다("계약금액")`
#[test]
fn taken_name_refused_contract_amount() {
    taken_name_case("계약금액");
}

/// `표가_이미_쓰는_이름은_막는다("계약건명")`
#[test]
fn taken_name_refused_contract_title() {
    taken_name_case("계약건명");
}

/// `표가_이미_쓰는_이름은_막는다("공고명")` — 통합 뷰에서 부딪힌다.
#[test]
fn taken_name_refused_notice_title_in_unified() {
    taken_name_case("공고명");
}

/// `이미_있는_열_이름은_막는다`
#[test]
fn existing_column_name_refused() {
    let f = Fixture::new("cols-existing");
    invalid(f.store().add_user_column("contract", "메모", "text", &[]));
}

/// `제_이름_그대로_고치는_것은_막지_않는다` — 이름을 그대로 두고 종류만 고치는 것은 막지 않는다.
#[test]
fn update_with_same_name_allowed() {
    let f = Fixture::new("cols-same-name");
    let store = f.store();
    store
        .update_user_column(
            "contract",
            "진행상태",
            "진행상태",
            "choice",
            &s(&["준비", "종료"]),
        )
        .unwrap();

    assert_eq!(
        column(&store, "contract", "진행상태").choices(),
        s(&["준비", "종료"])
    );
}

/// `빈_이름과_너무_긴_이름은_막는다`
#[test]
fn blank_and_long_names_refused() {
    let f = Fixture::new("cols-blank");
    let store = f.store();
    invalid(store.add_user_column("contract", "   ", "text", &[]));
    invalid(store.add_user_column("contract", &"가".repeat(21), "text", &[]));
}

/// `모르는_종류는_막는다`
#[test]
fn unknown_kind_refused() {
    let f = Fixture::new("cols-kind");
    invalid(f.store().add_user_column("contract", "담당", "색깔", &[]));
}

/// `없는_열은_고치거나_지울_수_없다`
#[test]
fn missing_column_cannot_be_changed_or_removed() {
    let f = Fixture::new("cols-missing");
    let store = f.store();
    invalid(store.remove_user_column("contract", "없는열"));
    invalid(store.update_user_column("contract", "없는열", "담당", "text", &[]));
}

/// `계약과_공고는_따로_센다` — 같은 이름을 양쪽에 하나씩 두는 것은 막을 이유가 없다.
#[test]
fn contract_and_notice_counted_separately() {
    let f = Fixture::new("cols-separate");
    let store = f.store();
    store
        .add_user_column("contract", "검토여부", "text", &[])
        .unwrap();

    assert!(names(&store.user_columns(Some("contract")).unwrap()).contains(&"검토여부".into()));
    assert!(names(&store.user_columns(Some("notice")).unwrap()).contains(&"검토여부".into()));
}

// ── 후보 목록 표기 ───────────────────────────────────
// 세로줄로 나눈다. 개행으로 나누다 거짓 경고가 나던 자리라 시험으로 못 박는다.

/// `후보를_세로줄로_나눈다`
#[test]
fn choices_split_on_bar() {
    let column = UserColumnDefinition {
        entity_type: "contract".into(),
        field_name: "진행상태".into(),
        kind: "choice".into(),
        options: Some("준비|투찰|낙찰|계약|납품".into()),
        sort_order: 10,
    };

    assert_eq!(
        column.choices(),
        s(&["준비", "투찰", "낙찰", "계약", "납품"])
    );
}

/// `후보가_없으면_빈_목록`
#[test]
fn no_options_means_no_choices() {
    let column = UserColumnDefinition {
        entity_type: "contract".into(),
        field_name: "메모".into(),
        kind: "text".into(),
        options: None,
        sort_order: 10,
    };
    assert!(column.choices().is_empty());
}

// ── 가공열 ───────────────────────────────────

/// `가공열은_외부_연결에서_짝과_순서를_보존하고_차수별로_계산된다`
#[test]
fn computed_column_reads_from_outside_per_revision() {
    let f = Fixture::new("cols-computed");
    let store = f.store();
    let old_items: Vec<Vec<(&str, String)>> = [(3, "002"), (1, "001"), (2, "001")]
        .iter()
        .map(|(line, number)| {
            vec![
                ("line_no", line.to_string()),
                ("item_name", "책상".to_string()),
                ("detail_item_number", number.to_string()),
            ]
        })
        .collect();
    with_items(&old_items, |items| {
        f.notice("TESTNOTICE", "000", &[], items, &[])
    });
    let json = formula(&[
        step("pick", Some(&["세부품명", "세부품명번호"]), None, None),
        step("distinct", None, None, None),
        step("format", None, Some("{세부품명}({세부품명번호})"), None),
        step("join", None, Some(", "), None),
    ]);
    store
        .save_computed_column("notice", "물품요약", &json, None)
        .unwrap();
    // 앱 함수를 등록하지 않은 독립 읽기 전용 연결이 소비자 경계다.
    let consumer = common::raw_read(&f.path);
    let read = |sql: &str| scalar_text(&consumer, sql);
    assert_eq!(read("SELECT 물품요약 FROM v_공고"), "책상(001), 책상(002)");
    let id = read("SELECT 식별자 FROM v_가공열 WHERE 열이름='물품요약'");
    assert!(!id.is_empty());
    store
        .save_computed_column("notice", "새요약", &json, Some("물품요약"))
        .unwrap();
    assert_eq!(
        read("SELECT 식별자 FROM v_가공열 WHERE 열이름='새요약'"),
        id
    );
    let entity = EntityRef::new("notice", "TESTNOTICE", "000");
    invalid(store.set_user_field(&entity, "새요약", Some("수동값")));
    invalid(store.set_override(&entity, "새요약", "수동값"));
    f.notice(
        "TESTNOTICE",
        "001",
        &[],
        &[&[
            ("line_no", "1"),
            ("item_name", "의자"),
            ("detail_item_number", "003"),
        ]],
        &[],
    );
    assert_eq!(read("SELECT 새요약 FROM v_공고"), "의자(003)");
    assert_eq!(
        read("SELECT 새요약 FROM v_공고차수 WHERE 차수='000'"),
        "책상(001), 책상(002)"
    );
    f.database().migrate(f.env).unwrap();
    assert_eq!(read("SELECT 새요약 FROM v_공고"), "의자(003)");
    store.remove_user_column("notice", "새요약").unwrap();
    assert_eq!(read("SELECT count(*) FROM v_가공열"), "0");
    assert_eq!(read("SELECT count(*) FROM notice_item"), "4");

    // 열 관리의 원본·부모·계산 결과는 숫자로 고른 같은 최신 차수와 같은 샘플을 가리킨다.
    with_items(&old_items, |items| {
        f.notice("TESTNOTICE", "9", &[("title", "이전9")], items, &[])
    });
    let latest: Vec<Vec<(&str, String)>> = (1..=101)
        .rev()
        .map(|i| {
            vec![
                ("line_no", i.to_string()),
                ("item_name", "의자10".to_string()),
                ("detail_item_number", format!("{i:04}")),
                ("unit_price", "1234.567".to_string()),
            ]
        })
        .collect();
    with_items(&latest, |items| {
        f.notice("TESTNOTICE", "10", &[("title", "최신10")], items, &[])
    });
    for i in 0..5 {
        f.notice(&format!("ZZNOTICE{i}"), "000", &[], &[], &[]);
    }
    let viewer = Store::new(Database::new(&f.path, Access::Read), f.env);
    let context = viewer.column_context("notice").unwrap();
    assert_eq!(context["rows"].as_array().unwrap().len(), 5);
    assert!(
        context["columns"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c.as_str() == Some("입찰공고번호"))
    );
    assert_eq!(
        context["rows"][0]["입찰공고번호"].as_str(),
        Some("TESTNOTICE-10")
    );
    let fields: Vec<String> = context["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c.as_str().unwrap().to_string())
        .collect();
    assert_eq!(fields, computed_column::field_names("notice").unwrap());
    let sources = context["sources"].as_array().unwrap();
    assert_eq!(sources.len(), 5);
    let first = &sources[0];
    assert_eq!(first["base"].as_str(), Some("TESTNOTICE"));
    assert_eq!(first["revision"].as_str(), Some("10"));
    assert_eq!(first["title"].as_str(), Some("최신10"));
    assert_eq!(first["totalRows"].as_i64(), Some(101));
    assert_eq!(first["rows"].as_array().unwrap().len(), 100);
    assert_eq!(first["rows"][0]["세부품명번호"].as_str(), Some("0001"));
    assert_eq!(first["rows"][99]["세부품명번호"].as_str(), Some("0100"));
    assert_eq!(first["rows"][0]["추정단가"].as_str(), Some("1,234.567"));
    assert_eq!(sources[1]["totalRows"].as_i64(), Some(0));
    assert!(sources[1]["rows"].as_array().unwrap().is_empty());
    let previews = viewer
        .preview_computed_column(
            "notice",
            &formula(&[
                step("at", None, None, Some(1)),
                step("get", None, Some("세부품명번호"), None),
            ]),
        )
        .unwrap();
    let previews = previews.as_array().unwrap();
    let bases = |rows: &[Value]| -> Vec<Option<String>> {
        rows.iter()
            .map(|r| r["base"].as_str().map(str::to_string))
            .collect()
    };
    assert_eq!(bases(sources), bases(previews));
    assert_eq!(previews[0]["revision"].as_str(), Some("10"));
    assert_eq!(previews[0]["value"].as_str(), Some("0001"));
    invalid(viewer.column_context("notice; DROP TABLE notice"));
}

/// `가공열은_행선택_개수_빈목록과_문자열_리터럴을_처리한다`
#[test]
fn computed_column_handles_row_pick_count_empty_and_literals() {
    let f = Fixture::new("cols-computed-rows");
    let store = f.store();
    f.contract(
        "TESTCONTRACT",
        "00",
        &[],
        &[
            &[
                ("line_no", "1"),
                ("item_name", "첫째"),
                ("item_id_number", "0001"),
                ("unit_price", "1234.567"),
            ],
            &[
                ("line_no", "2"),
                ("item_name", "둘째"),
                ("item_id_number", "0002"),
            ],
        ],
    );
    store
        .save_computed_column(
            "contract",
            "둘째번호",
            &formula(&[
                step("pick", Some(&["품명", "물품식별번호"]), None, None),
                step("at", None, None, Some(2)),
                step("get", None, Some("물품식별번호"), None),
            ]),
            None,
        )
        .unwrap();
    assert_eq!(cell(&f, "둘째번호"), "0002");
    store
        .save_computed_column(
            "contract",
            "첫단가",
            &formula(&[
                step("at", None, None, Some(1)),
                step("get", None, Some("단가"), None),
            ]),
            None,
        )
        .unwrap();
    assert_eq!(cell(&f, "첫단가"), "1,234.567");
    store
        .save_computed_column(
            "contract",
            "문자열",
            &formula(&[
                step("pick", Some(&["품명"]), None, None),
                step("format", None, Some("'{품명}'"), None),
                step("join", None, Some("'|"), None),
            ]),
            None,
        )
        .unwrap();
    assert_eq!(cell(&f, "문자열"), "'첫째''|'둘째'");
    store
        .save_computed_column(
            "contract",
            "행수",
            &formula(&[step("count", None, None, None)]),
            None,
        )
        .unwrap();
    assert_eq!(cell(&f, "행수"), "2");
    f.contract("TESTCONTRACT", "00", &[], &[]);
    assert_eq!(cell(&f, "행수"), "0");
    assert_eq!(cell(&f, "둘째번호"), "");
    assert_eq!(cell(&f, "문자열"), "");
}

/// `잘못된_가공식은_저장하지_않는다`
fn bad_formula_case(json: &str) {
    let f = Fixture::new("cols-bad-formula");
    let store = f.store();
    invalid(store.save_computed_column("contract", "실패열", json, None));
    assert!(
        !store
            .user_columns(None)
            .unwrap()
            .iter()
            .any(|c| c.field_name == "실패열")
    );
    f.database().refresh_views().unwrap();
}

/// `고르기_검사는_본문_읽기와_같은_키를_본다` — v1 「고르기」 의 사전 검사는 본문 읽기(대소문자를 가리지 않고, 같은
/// 이름이 여럿이면 뒤의 것)와 같은 키를 본다. 같은 키가 두 번 든 식은 맨 앞에서 형식 오류다.
#[test]
fn pick_check_reads_the_same_keys_as_the_body() {
    let cases = [
        (
            r#"{"Version":1,"Steps":[{"Op":"pick","Fields":["없는필드"]},{"op":"count"}]}"#,
            "선택한 필드가 없거나 중복되었습니다.",
        ),
        (
            r#"{"version":2,"VERSION":1,"steps":[{"op":"pick","fields":["없는필드"]},{"op":"count"}]}"#,
            "선택한 필드가 없거나 중복되었습니다.",
        ),
        (
            r#"{"version":1,"steps":[{"op":"pick","fields":["품명"],"FIELDS":["없는필드"]},{"op":"count"}]}"#,
            "선택한 필드가 없거나 중복되었습니다.",
        ),
        (
            r#"{"version":1,"steps":[{"op":"count","OP":"pick","fields":["품명","품명"]},{"op":"count"}]}"#,
            "선택한 필드가 없거나 중복되었습니다.",
        ),
        (
            r#"{"version":1,"steps":[{"op":"pick","fields":["품명"],"fields":["없는필드"]},{"op":"count"}]}"#,
            "가공식 형식이 올바르지 않습니다.",
        ),
    ];
    for (json, message) in cases {
        match computed_column::parse_for("contract", json) {
            Err(Error::InvalidOperation(m)) => assert_eq!(m, message, "{json}"),
            other => panic!("{json}: {other:?}"),
        }
        let f = Fixture::new("cols-pick-keys");
        let store = f.store();
        assert!(
            store
                .save_computed_column("contract", "실패열", json, None)
                .is_err(),
            "{json}"
        );
        assert!(
            !store
                .user_columns(None)
                .unwrap()
                .iter()
                .any(|c| c.field_name == "실패열")
        );
    }
    // 대소문자가 다른 키로 적은 v1 도 고른 필드가 있으면 그대로 읽힌다.
    computed_column::parse_for(
        "contract",
        r#"{"Version":1,"Steps":[{"Op":"pick","Fields":["품명"]},{"Op":"count"}]}"#,
    )
    .unwrap();
}

/// `잘못된_가공식은_저장하지_않는다` — 없는 필드를 고른다.
#[test]
fn bad_formula_unknown_pick_field() {
    bad_formula_case(
        r#"{"version":1,"steps":[{"op":"pick","fields":["없는필드"]},{"op":"count"}]}"#,
    );
}

/// `잘못된_가공식은_저장하지_않는다` — 행목록을 바로 잇는다.
#[test]
fn bad_formula_join_rows() {
    bad_formula_case(r#"{"version":1,"steps":[{"op":"join","text":","}]}"#);
}

/// `잘못된_가공식은_저장하지_않는다` — 0번째 행.
#[test]
fn bad_formula_at_zero() {
    bad_formula_case(r#"{"version":1,"steps":[{"op":"at","index":0},{"op":"get","text":"품명"}]}"#);
}

/// `잘못된_가공식은_저장하지_않는다` — v2 인데 `steps` 꼴.
#[test]
fn bad_formula_v2_with_steps() {
    bad_formula_case(r#"{"version":2,"steps":[{"op":"count"}]}"#);
}

/// `잘못된_가공식은_저장하지_않는다` — 없는 필드를 찍는다.
#[test]
fn bad_formula_unknown_format_field() {
    bad_formula_case(
        r#"{"version":1,"steps":[{"op":"format","text":"{없는필드}"},{"op":"join"}]}"#,
    );
}

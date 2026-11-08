//! 맞춤 뷰(docs/user-views.md) — C# `UserViewTests` 를 옮긴 것. 조달 건마다 한 줄, 접수·공고·계약의 필드를 늘어놓은
//! `u_*` 뷰의 저장 왕복, 줄이 서는 꼴(사슬·거르기·모으기), 깨진 필드, 사람 열과의 맞물림, 미리보기·표본·보표,
//! 판올림, 메타 뷰, 보기 구성(내보내기·미리보기·가져오기)을 본다.
//!
//! 번호·건명은 모두 지어낸 것이다(CLAUDE.md).

mod common;

use common::store_fixture::Fixture;
use common::{Temp, TestEnv, legacy_at, raw, raw_read};
use pclm_core::erp::explicit_links;
use pclm_core::storage::store::{
    EntityRef, Store, UserViewInfo, builtin_view_package, example_view_package_v26,
    read_view_package, stored_definition,
};
use pclm_core::storage::{computed_column, pclm_file, sql_column, user_view_builder, views};
use pclm_core::{Access, Database, Error, Result};
use rusqlite::types::Value as SqlValue;
use serde_json::{Map, Value, json};

const N1: &str = "TESTN1-000";
const N2: &str = "TESTN2-000";

// ── 자리 ─────────────────────────────────────────────────

/// C# 생성자 — 빈 작업자료는 서식용 예시 보기를 들고 선다. 여기 시험들은 빈 보기 목록에서 시작한다.
fn fresh(name: &str) -> Fixture {
    let f = Fixture::new(name);
    let s = f.store();
    for view in s.user_views().unwrap() {
        s.delete_user_view(&view.id).unwrap();
    }
    f
}

/// `ReferenceFixture.Contract` — 계약에 공고 명시 참조를 넣고 연결을 다시 짓는다.
fn ref_contract(f: &Fixture, contract: &str, notice: &str) {
    let c = f.conn();
    c.execute(
        "UPDATE contract SET ref_notice_base=?1 WHERE contract_base=?2",
        [notice, contract],
    )
    .unwrap();
    explicit_links::resolve(&c).unwrap();
}

/// `ReferenceFixture.Request` — 접수 품목과 공고 품목에 접수 명시 참조를 넣는다(공고에 품목이 없으면 하나 세운다).
fn ref_request(f: &Fixture, request: &str, seq: &str, notice: &str) {
    let c = f.conn();
    c.execute(
        "UPDATE request_item SET ref_request_base=request_base, ref_request_seq=seq,
             ref_request_item=CAST(line_no AS TEXT) WHERE request_base=?1",
        [request],
    )
    .unwrap();
    c.execute(
        "UPDATE notice_item SET ref_request_base=?1, ref_request_seq=?2,
             ref_request_item=CAST(line_no AS TEXT) WHERE notice_base=?3",
        [request, seq, notice],
    )
    .unwrap();
    c.execute(
        "INSERT INTO notice_item(notice_base,seq,line_no,ref_request_base)
             SELECT notice_base,seq,1,?1 FROM notice n WHERE notice_base=?2
             AND NOT EXISTS(SELECT 1 FROM notice_item i WHERE i.notice_base=n.notice_base AND i.seq=n.seq)",
        [request, notice],
    )
    .unwrap();
    explicit_links::resolve(&c).unwrap();
}

fn notice_ref(base: &str, seq: &str) -> EntityRef {
    EntityRef::new("notice", base, seq)
}

/// 사슬 여섯 — (접수1, 공고1, 계약1)·(접수1, 공고1, 계약2) 는 계약이 둘인 건이고, 공고2 는 혼자, 접수2 는 공고가 아직
/// 없고, 계약3·계약4 는 어디에도 매달리지 못했다. 계약2 를 먼저 넣는다 — 모으기의 차례가 넣은 차례가 아니라
/// 계약본번호 차례인지 본다.
fn seed(f: &Fixture) {
    f.notice(
        "TESTN1",
        "000",
        &[("title", "시험 공고 하나")],
        &[
            &[("line_no", "1"), ("item_name", "책상")],
            &[("line_no", "2"), ("item_name", "의자")],
        ],
        &[],
    );
    f.notice("TESTN2", "000", &[("title", "시험 공고 둘")], &[], &[]);
    f.contract(
        "TESTK2",
        "00",
        &[("title", "시험 계약 둘")],
        &[&[
            ("line_no", "1"),
            ("item_name", "의자"),
            ("quantity", "3"),
            ("unit_price", "1000"),
            ("amount", "3000"),
        ]],
    );
    f.contract(
        "TESTK1",
        "00",
        &[("title", "시험 계약 하나")],
        &[
            &[
                ("line_no", "1"),
                ("item_name", "책상"),
                ("quantity", "1"),
                ("unit_price", "5000"),
                ("amount", "5000"),
            ],
            &[
                ("line_no", "2"),
                ("item_name", "책상"),
                ("quantity", "1"),
                ("unit_price", "5000"),
                ("amount", "5000"),
            ],
        ],
    );
    f.contract("TESTK3", "00", &[("title", "시험 계약 셋")], &[]);
    f.contract("TESTK4", "00", &[("title", "시험 계약 넷")], &[]);
    f.request("TESTR1", "000", &[("title", "시험 접수 하나")], &[]);
    f.request("TESTR2", "000", &[("title", "시험 접수 둘")], &[]);
    ref_contract(f, "TESTK2", "TESTN1");
    ref_contract(f, "TESTK1", "TESTN1");
    ref_request(f, "TESTR1", "000", "TESTN1");
    f.store()
        .set_user_field(&notice_ref("TESTN1", "000"), "메모", Some("확인함"))
        .unwrap();
}

// ── 지어내는 도구 ─────────────────────────────────────────

fn flow_of(from: Value, steps: Vec<Value>) -> Value {
    json!({ "version": 2, "source": "flow", "flow": { "from": from, "steps": steps } })
}

/// 문서 하나의 열 — `공고.공고명`.
fn doc(link: &str, field: &str) -> Value {
    flow_of(
        json!({ "kind": "linked", "link": link, "field": field }),
        vec![],
    )
}

fn sql(sql: &str) -> Value {
    json!({ "version": 2, "source": "sql", "sql": sql })
}

type Col<'a> = (Option<&'a str>, &'a str, Value);

fn view_with(name: &str, contracts: &str, columns: &[Col<'_>]) -> String {
    let columns: Vec<Value> = columns
        .iter()
        .map(|(id, name, formula)| json!({ "id": id, "name": name, "formula": formula }))
        .collect();
    json!({ "name": name, "definition": { "version": 2, "contracts": contracts, "columns": columns } })
        .to_string()
}

fn view(name: &str, columns: &[Col<'_>]) -> String {
    view_with(name, "split", columns)
}

fn notice_item_count() -> Value {
    flow_of(
        json!({ "kind": "linked", "link": "notice", "items": true }),
        vec![json!({ "op": "count" })],
    )
}

fn text(value: SqlValue) -> String {
    match value {
        SqlValue::Null => panic!("NULL"),
        SqlValue::Integer(i) => i.to_string(),
        SqlValue::Real(r) => r.to_string(),
        SqlValue::Text(t) => t,
        SqlValue::Blob(_) => panic!("blob"),
    }
}

/// 앱 함수가 없는 독립 읽기 연결이 소비자 경계다.
fn read_at(path: &std::path::Path, sql: &str) -> String {
    text(raw_read(path).query_row(sql, [], |r| r.get(0)).unwrap())
}

fn rows_at(path: &std::path::Path, sql: &str) -> Vec<String> {
    let c = raw_read(path);
    let mut s = c.prepare(sql).unwrap();
    s.query_map([], |r| r.get::<_, SqlValue>(0))
        .unwrap()
        .map(|v| text(v.unwrap()))
        .collect()
}

fn read(f: &Fixture, sql: &str) -> String {
    read_at(&f.path, sql)
}

fn rows(f: &Fixture, sql: &str) -> Vec<String> {
    rows_at(&f.path, sql)
}

fn count(f: &Fixture, view: &str) -> i64 {
    read(f, &format!("SELECT CAST(count(*) AS TEXT) FROM \"{view}\""))
        .parse()
        .unwrap()
}

fn columns_of(f: &Fixture, view: &str) -> Vec<String> {
    rows(f, &format!("SELECT name FROM pragma_table_info('{view}')"))
}

fn user_view_names(f: &Fixture) -> Vec<String> {
    rows(
        f,
        "SELECT name FROM sqlite_master WHERE type = 'view' AND name LIKE 'u\\_%' ESCAPE '\\' ORDER BY name",
    )
}

fn master(f: &Fixture) -> String {
    rows(
        f,
        "SELECT type || ' ' || name || ' ' || coalesce(sql, '') FROM sqlite_master ORDER BY type, name",
    )
    .join("\n")
}

fn column_strings(view: &UserViewInfo, key: &str) -> Vec<String> {
    view.definition["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c[key].as_str().unwrap().to_string())
        .collect()
}

fn column_ids(view: &UserViewInfo) -> Vec<String> {
    column_strings(view, "id")
}

fn column_names(view: &UserViewInfo) -> Vec<String> {
    column_strings(view, "name")
}

fn samples(s: &Store<'_>, view: &str, query: Option<&str>, edge: Option<&str>) -> Vec<Value> {
    s.user_view_samples(view, query, edge).unwrap()
}

fn strs(values: &[Value], key: &str) -> Vec<String> {
    values
        .iter()
        .map(|v| v[key].as_str().unwrap().to_string())
        .collect()
}

/// `Assert.Throws<InvalidOperationException>` — 글을 낸다.
#[track_caller]
fn invalid<T: std::fmt::Debug>(result: Result<T>) -> String {
    match result {
        Err(Error::InvalidOperation(message)) => message,
        other => panic!("InvalidOperation 이 아니다: {other:?}"),
    }
}

fn single<T: std::fmt::Debug>(mut items: Vec<T>) -> T {
    assert_eq!(items.len(), 1, "{items:?}");
    items.pop().unwrap()
}

fn builtin_field_names(package: Option<&str>) -> Vec<String> {
    let package: Value = serde_json::from_str(package.unwrap_or(builtin_view_package())).unwrap();
    package["views"][0]["definition"]["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap().to_string())
        .collect()
}

// ── 서식용 예시·보기 구성 ─────────────────────────────────

/// `서식용_예시는_취소와_재공고가_연결되면_현행_공고명을_낸다`
#[test]
fn builtin_example_reads_current_notice_title_across_cancel_and_renotice() {
    let f = fresh("uv-current");
    let s = f.store();
    let package = builtin_view_package();
    s.import_view_package(package, &s.preview_view_package(package).unwrap().token)
        .unwrap();
    f.notice("TESTN1", "000", &[("title", "시험 원공고")], &[], &[]);
    f.notice(
        "TESTN1",
        "001",
        &[("title", "시험 취소공고"), ("notice_kind", "취소공고")],
        &[],
        &["TESTN1-000"],
    );
    let renotice = [("title", "시험 재공고"), ("notice_kind", "재공고")];
    f.notice(
        "TESTN2",
        "000",
        &renotice,
        &[],
        &["TESTN1-001", "TESTN1-000"],
    );
    f.request("TESTR1", "000", &[("title", "시험 접수")], &[]);
    ref_request(&f, "TESTR1", "000", "TESTN1");
    for number in ["TESTK1", "TESTK2"] {
        f.contract(number, "00", &[("title", "시험 계약")], &[]);
        ref_contract(&f, number, "TESTN1");
    }

    let titles = || {
        rows(
            &f,
            "SELECT 건명 || '|' || 입찰공고번호 || '|' || 공고명 || '|' || 계약번호
             FROM \"u_서식용 예시\" ORDER BY 계약번호",
        )
    };
    let current = [
        "시험 재공고|TESTN2-000|시험 재공고|TESTK100",
        "시험 재공고|TESTN2-000|시험 재공고|TESTK200",
    ];
    // 관련공고가 본번호를 잇고, 계약 둘은 원공고를 참조해도 현행 재공고의 이름을 읽는다.
    assert_eq!(titles(), current);

    // ERP처럼 관련공고 없이 같은 접수 품목으로 묶인 경우도 재공고 하나가 현행이다.
    f.notice("TESTN2", "000", &renotice, &[], &[]);
    ref_request(&f, "TESTR1", "000", "TESTN2");
    assert_eq!(titles(), current);

    // 현행 후보가 갈리면 임의로 고르지 않는다. 건명만 접수명으로 물러서고 공고명은 빈다.
    s.set_override(&notice_ref("TESTN2", "000"), "공고종류", "일반공고")
        .unwrap();
    assert_eq!(titles(), ["시험 접수|||TESTK100", "시험 접수|||TESTK200"]);
}

/// `보기_구성은_자료와_입력값_없이_의존열과_차례를_왕복한다`
#[test]
fn view_package_round_trips_columns_and_order_without_data() {
    let f = fresh("uv-package");
    seed(&f);
    let s = f.store();
    s.add_user_column(
        "notice",
        "공유확인",
        "choice",
        &["대기".into(), "완료".into()],
    )
    .unwrap();
    s.set_user_field(
        &notice_ref("TESTN1", "000"),
        "공유확인",
        Some("절대내보내지않는셀값"),
    )
    .unwrap();
    s.save_computed_column(
        "notice",
        "품목총수",
        &flow_of(json!({ "kind": "items" }), vec![json!({ "op": "count" })]).to_string(),
        None,
    )
    .unwrap();
    let saved = single(
        s.save_user_view(
            &view_with(
                "공유문서",
                "fold",
                &[
                    (Some("c-1"), "검토", doc("notice", "공유확인")),
                    (Some("c-2"), "수량", doc("notice", "품목총수")),
                ],
            ),
            None,
        )
        .unwrap(),
    );
    let json = s
        .export_view_package(std::slice::from_ref(&saved.id))
        .unwrap();
    assert!(!json.contains("절대내보내지않는셀값"));
    assert!(!json.contains("시험 공고 하나"));
    assert!(!json.contains("TESTN1"));
    let package: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(package["columns"].as_array().unwrap().len(), 2);
    let preview = s.preview_view_package(&json).unwrap();
    assert!(preview.errors.is_empty(), "{:?}", preview.errors);
    assert_eq!(
        single(preview.views.clone()).import_name,
        "공유문서 (복사본)"
    );
    assert!(preview.columns.iter().all(|c| c.action == "reuse"));
    assert_eq!(s.user_views().unwrap().len(), 1); // preview writes nothing
    let imported = s
        .import_view_package(&json, &preview.token)
        .unwrap()
        .into_iter()
        .find(|v| v.name == "공유문서 (복사본)")
        .unwrap();
    assert_eq!(imported.definition["contracts"], "fold");
    assert_eq!(column_names(&saved), column_names(&imported));
    assert_eq!(column_ids(&saved), column_ids(&imported));
    assert_eq!(saved.definition, imported.definition);
    assert_eq!(
        read(
            &f,
            "SELECT 공유확인 FROM v_공고 WHERE 입찰공고번호='TESTN1-000'"
        ),
        "절대내보내지않는셀값"
    );
}

/// `보기_구성은_SQL_간접의존열을_포함하고_빈_자료에도_가져온다`
#[test]
fn view_package_carries_sql_indirect_columns_into_empty_file() {
    let f = fresh("uv-package-sql");
    let s = f.store();
    s.add_user_column("notice", "원수", "number", &[]).unwrap();
    s.save_computed_column(
        "notice",
        "간접수",
        &sql("SELECT 현재.원수").to_string(),
        None,
    )
    .unwrap();
    s.save_computed_column(
        "notice",
        "다른수",
        &flow_of(json!({ "kind": "number", "value": 2 }), vec![]).to_string(),
        None,
    )
    .unwrap();
    let saved = single(
        s.save_user_view(
            &view("SQL문서", &[(Some("c-1"), "값", doc("notice", "간접수"))]),
            None,
        )
        .unwrap(),
    );
    let json = s
        .export_view_package(std::slice::from_ref(&saved.id))
        .unwrap();
    let package: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(package["includesAllColumnsForSql"], true);
    s.delete_user_view(&saved.id).unwrap();
    s.remove_user_column("notice", "간접수").unwrap();
    s.remove_user_column("notice", "원수").unwrap();
    s.remove_user_column("notice", "다른수").unwrap();
    let preview = s.preview_view_package(&json).unwrap();
    assert!(preview.errors.is_empty(), "{:?}", preview.errors);
    assert!(
        preview
            .columns
            .iter()
            .any(|c| c.field_name == "원수" && c.action == "add")
    );
    assert!(
        !s.user_columns(None)
            .unwrap()
            .iter()
            .any(|c| c.field_name == "원수")
    );
    let imported = single(s.import_view_package(&json, &preview.token).unwrap());
    assert!(imported.broken.is_empty());
    assert!(
        s.user_columns(None)
            .unwrap()
            .iter()
            .any(|c| c.field_name == "원수")
    );
    assert_eq!(count(&f, "u_SQL문서"), 0);
    let formula_ids = || -> Vec<String> {
        s.user_columns(Some("notice"))
            .unwrap()
            .iter()
            .filter(|c| c.kind == "computed")
            .map(|c| {
                computed_column::parse(c.options.as_deref().unwrap())
                    .unwrap()
                    .id
                    .unwrap_or_default()
            })
            .collect()
    };
    let ids = formula_ids();
    assert_eq!(ids.len(), 2);
    assert!(ids.iter().all(|id| !id.trim().is_empty()));
    let distinct: std::collections::HashSet<&String> = ids.iter().collect();
    assert_eq!(distinct.len(), ids.len());
    let reuse = s.preview_view_package(&json).unwrap();
    assert!(reuse.errors.is_empty(), "{:?}", reuse.errors);
    s.import_view_package(&json, &reuse.token).unwrap();
    assert_eq!(formula_ids(), ids);
}

/// `보기_구성의_열정의가_여럿_다르면_받는_사람이_바꿀_내_열을_모두_든다`
#[test]
fn view_package_lists_every_conflicting_column() {
    let f = fresh("uv-package-conflicts");
    let s = f.store();
    s.add_user_column("notice", "검토자", "text", &[]).unwrap();
    s.add_user_column("contract", "확인", "text", &[]).unwrap();
    let saved = single(
        s.save_user_view(
            &view(
                "둘",
                &[
                    (Some("c-1"), "검토", doc("notice", "검토자")),
                    (Some("c-2"), "확인", doc("contract", "확인")),
                ],
            ),
            None,
        )
        .unwrap(),
    );
    let mut modified: Value = serde_json::from_str(
        &s.export_view_package(std::slice::from_ref(&saved.id))
            .unwrap(),
    )
    .unwrap();
    for column in modified["columns"].as_array_mut().unwrap() {
        column["kind"] = json!("number");
    }
    let preview = s.preview_view_package(&modified.to_string()).unwrap();
    assert_eq!(
        preview
            .columns
            .iter()
            .filter(|c| c.action == "conflict")
            .count(),
        2
    );
    assert_eq!(preview.errors.len(), 2);
    assert!(preview.errors.iter().any(|e| {
        e.starts_with("내 공고 열 「검토자」 와 정의가 다릅니다. 열 관리에서 내 열 이름을 바꾸면")
    }));
    assert!(
        preview
            .errors
            .iter()
            .any(|e| e.starts_with("내 계약 열 「확인」 과 정의가 다릅니다."))
    );
    assert!(
        preview
            .errors
            .iter()
            .all(|e| e.ends_with("보낸 사람에게 열 이름을 바꿔 다시 내보내 달라고 해도 됩니다."))
    );
}

/// `보기_구성의_다른_열정의와_오래된_미리보기는_기존것을_덮어쓰지_않는다`
#[test]
fn view_package_conflicts_and_stale_previews_do_not_overwrite() {
    let f = fresh("uv-package-stale");
    let s = f.store();
    s.add_user_column("notice", "공유검토", "text", &[])
        .unwrap();
    let saved = single(
        s.save_user_view(
            &view("공유", &[(Some("c-1"), "검토", doc("notice", "공유검토"))]),
            None,
        )
        .unwrap(),
    );
    let json = s
        .export_view_package(std::slice::from_ref(&saved.id))
        .unwrap();
    let valid_preview = s.preview_view_package(&json).unwrap();
    let mut modified: Value = serde_json::from_str(&json).unwrap();
    modified["columns"][0]["kind"] = json!("number");
    let conflict = s.preview_view_package(&modified.to_string()).unwrap();
    assert!(conflict.columns.iter().any(|c| c.action == "conflict"));
    assert_eq!(
        conflict.errors,
        [
            "내 공고 열 「공유검토」 와 정의가 다릅니다. 열 관리에서 내 열 이름을 바꾸면 이 구성을 가져올 수 있습니다. \
             보낸 사람에게 열 이름을 바꿔 다시 내보내 달라고 해도 됩니다."
        ]
    );
    invalid(s.import_view_package(&modified.to_string(), &conflict.token));
    assert_eq!(
        s.user_columns(Some("notice"))
            .unwrap()
            .into_iter()
            .find(|c| c.field_name == "공유검토")
            .unwrap()
            .kind,
        "text"
    );
    modified["columns"][0]["choices"]
        .as_array_mut()
        .unwrap()
        .push(Value::Null);
    assert!(
        !s.preview_view_package(&modified.to_string())
            .unwrap()
            .errors
            .is_empty()
    );
    s.save_user_view(
        &view(
            "공유 (복사본)",
            &[(Some("c-1"), "번호", doc("notice", "입찰공고번호"))],
        ),
        None,
    )
    .unwrap();
    invalid(s.import_view_package(&json, &valid_preview.token));
    assert_eq!(s.user_views().unwrap().len(), 2);
    let reader = Store::new(Database::new(&f.path, Access::Read), f.env);
    invalid(reader.import_view_package(&json, &valid_preview.token));
}

/// `빈_작업자료는_서식용_예시_보기를_들고_선다` — 서식용 예시 보기(docs/user-views.md §13)는 빈 작업자료가 처음부터
/// 들고 서고, 깨진 필드 없이 서식 자리 이름 그대로의 열을 낸다. 계약마다 한 줄이고, 계약 값이 이기고 비면 공고 값이다.
/// 취합본·제출본으로 짓는 빈 파일에는 들지 않는다.
#[test]
fn fresh_work_file_carries_builtin_example_view() {
    let json = builtin_view_package();
    let package: Value = serde_json::from_str(json).unwrap();
    assert!(package["columns"].as_array().unwrap().is_empty());
    assert!(!json.contains("\"sql\""));
    let fields = builtin_field_names(None);
    // 단계는 「경우」 한 단계다 — 계약번호가 있으면 계약, 없고 입찰공고번호가 있으면 공고, 둘 다 없으면 접수.
    let stage = package["views"][0]["definition"]["columns"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "단계")
        .unwrap();
    assert_eq!(stage["id"], "c32");
    let expected: Value = serde_json::from_str(
        r#"{"from":{"kind":"linked","link":"contract","field":"계약번호"},"steps":[{"op":"cases","branches":[
              {"cond":[{"field":"","cmp":"안비어있음"}],"then":"계약"},
              {"cond":[{"field":"공고.입찰공고번호","cmp":"안비어있음"}],"then":"공고"}],"otherwise":"접수"}]}"#,
    )
    .unwrap();
    assert_eq!(expected, stage["formula"]["flow"]);

    let f = Fixture::new("uv-example");
    let s = f.store();
    let view = single(s.user_views().unwrap());
    assert_eq!(view.name, "서식용 예시");
    assert!(view.broken.is_empty(), "{:?}", view.broken);
    assert_eq!(view.definition["contracts"], "split");

    f.notice(
        "TESTN5",
        "000",
        &[
            ("title", "시험 공고 다섯"),
            ("notice_officer", "홍길동 ( ☎ 000-0000-0000 )"),
            // C# `DateTime` 매개변수가 SQLite 에 서는 꼴.
            ("posted_at", "2000-01-02 10:30:00"),
        ],
        &[&[("line_no", "1"), ("item_name", "책상"), ("unit", "개")]],
        &[],
    );
    f.notice(
        "TESTN6",
        "000",
        &[("title", "시험 공고 여섯")],
        &[&[("line_no", "1"), ("item_name", "의자"), ("unit", "식")]],
        &[],
    );
    f.contract(
        "TESTK5",
        "00",
        &[
            ("title", "시험 계약 다섯"),
            ("item_name", "서랍"),
            ("unit", "대"),
            ("amount", "1000"),
        ],
        &[],
    );
    f.contract(
        "TESTK6",
        "00",
        &[("title", "시험 계약 여섯"), ("amount", "2000")],
        &[],
    );
    ref_contract(&f, "TESTK5", "TESTN5");
    ref_contract(&f, "TESTK6", "TESTN5");
    // 건명의 갈래 — 접수만 있는 건, 공고명이 빈 공고에 이어진 접수, 공고명이 빈 공고에 이어진 계약, 계약만 있는 건.
    f.request("TESTR5", "000", &[("title", "시험 접수 다섯")], &[]);
    f.notice("TESTN7", "000", &[("title", "")], &[], &[]);
    f.request("TESTR7", "000", &[("title", "시험 접수 일곱")], &[]);
    ref_request(&f, "TESTR7", "000", "TESTN7");
    f.notice("TESTN8", "000", &[("title", "")], &[], &[]);
    f.contract("TESTK8", "00", &[("title", "시험 계약 여덟")], &[]);
    ref_contract(&f, "TESTK8", "TESTN8");
    f.contract("TESTK7", "00", &[("title", "시험 계약 일곱")], &[]);

    assert_eq!(
        fields,
        rows(&f, "SELECT name FROM pragma_table_info('u_서식용 예시')")
    );
    let row = |sql: &str| rows(&f, sql).join("\n");
    // 계약이 둘인 건은 계약마다 한 줄 — 계약 값이 이기고, 계약 값이 비면 공고 값이다.
    assert_eq!(
        row(
            "SELECT 계약금액 || '|' || 세부품명 || '|' || 단위 FROM \"u_서식용 예시\" WHERE 입찰공고번호 = 'TESTN5-000' ORDER BY 1"
        ),
        "1,000|서랍|대\n2,000|책상|개"
    );
    // 계약이 없는 공고는 공고 값이다. 빈 칸은 NULL 이 아니라 빈 글이다.
    assert_eq!(
        row(
            "SELECT 세부품명 || '|' || 단위 || '|' || 대표계약업체 || '|' || 계약담당자 FROM \"u_서식용 예시\" WHERE 입찰공고번호 = 'TESTN6-000'"
        ),
        "의자|식||"
    );
    assert_eq!(
        row(
            "SELECT DISTINCT 계약담당자 || '|' || \"계약담당 전화번호\" || '|' || 게시일 FROM \"u_서식용 예시\" WHERE 입찰공고번호 = 'TESTN5-000'"
        ),
        "홍길동|000-0000-0000|2000/01/02"
    );

    // 앞의 일곱 필드 — 사람이 알아보는 건명·단계가 먼저다. 건명은 공고명 → 요청명 → 계약건명으로 물러서 비지 않고,
    // 공고명은 공고가 없거나 비었으면 빈 채로 둔다(받는 쪽의 빈 값 경고가 산다).
    assert_eq!(
        fields[..7],
        [
            "건명",
            "단계",
            "수요기관",
            "대표계약업체",
            "조달요구번호",
            "입찰공고번호",
            "계약번호"
        ]
    );
    assert_eq!(
        row(
            "SELECT 건명 || '|' || 단계 || '|' || 공고명 || '|' || 계약번호 FROM \"u_서식용 예시\" ORDER BY 1"
        ),
        [
            "시험 계약 여덟|계약||TESTK800",
            "시험 계약 일곱|계약||TESTK700",
            "시험 공고 다섯|계약|시험 공고 다섯|TESTK500",
            "시험 공고 다섯|계약|시험 공고 다섯|TESTK600",
            "시험 공고 여섯|공고|시험 공고 여섯|",
            "시험 접수 다섯|접수||",
            "시험 접수 일곱|공고||",
        ]
        .join("\n")
    );

    // 취합본처럼 다른 역할로 짓는 빈 파일에는 들지 않는다.
    let merged = pclm_file::create(&f.temp.path("merged.pclm"), "merged", f.env).unwrap();
    assert!(Store::new(merged, f.env).user_views().unwrap().is_empty());
}

/// `예시_보기는_구성_가져오기로_다시_들고_겹치면_복사본이다` — 지운 예시 보기는 「보기 구성」 에서 다시 들인다.
#[test]
fn builtin_example_reimports_and_copies_on_name_clash() {
    let f = fresh("uv-example-again");
    let s = f.store();
    let json = builtin_view_package();
    let preview = s.preview_view_package(json).unwrap();
    assert!(preview.errors.is_empty(), "{:?}", preview.errors);
    assert!(preview.columns.is_empty());
    assert_eq!(
        preview
            .views
            .iter()
            .map(|v| v.import_name.as_str())
            .collect::<Vec<_>>(),
        ["서식용 예시"]
    );
    assert!(user_view_names(&f).is_empty()); // 미리보기는 되돌린다

    let imported = single(s.import_view_package(json, &preview.token).unwrap());
    assert!(imported.broken.is_empty(), "{:?}", imported.broken);
    assert_eq!(user_view_names(&f), ["u_서식용 예시"]);

    let again = s.preview_view_package(json).unwrap();
    assert!(again.errors.is_empty(), "{:?}", again.errors);
    assert_eq!(
        again
            .views
            .iter()
            .map(|v| v.import_name.as_str())
            .collect::<Vec<_>>(),
        ["서식용 예시 (복사본)"]
    );
}

/// `보기_구성의_잘못된_SQL은_추가열까지_전부_취소한다` (Theory 의 두 경우).
fn bad_sql_cancels_everything(sql_text: &str) {
    let f = fresh("uv-package-bad-sql");
    let s = f.store();
    let saved = single(
        s.save_user_view(
            &view(
                "원본",
                &[(Some("c-1"), "번호", doc("notice", "입찰공고번호"))],
            ),
            None,
        )
        .unwrap(),
    );
    let mut package: Value = serde_json::from_str(
        &s.export_view_package(std::slice::from_ref(&saved.id))
            .unwrap(),
    )
    .unwrap();
    package["columns"].as_array_mut().unwrap().push(
        serde_json::from_str(
            r#"{"entityType":"notice","fieldName":"임시공유열","kind":"text","choices":[]}"#,
        )
        .unwrap(),
    );
    package["views"][0]["definition"]["columns"][0]["formula"] = sql(sql_text);
    let before = master(&f);
    let preview = s.preview_view_package(&package.to_string()).unwrap();
    assert!(!preview.errors.is_empty());
    invalid(s.import_view_package(&package.to_string(), &preview.token));
    assert_eq!(before, master(&f));
    assert_eq!(s.user_views().unwrap().len(), 1);
    assert!(
        !s.user_columns(None)
            .unwrap()
            .iter()
            .any(|c| c.field_name == "임시공유열")
    );
}

/// `보기_구성의_잘못된_SQL은_추가열까지_전부_취소한다("SELECT count(*) FROM notice")`
#[test]
fn view_package_bad_sql_cancels_added_columns_raw_table() {
    bad_sql_cancels_everything("SELECT count(*) FROM notice");
}

/// `보기_구성의_잘못된_SQL은_추가열까지_전부_취소한다("SELECT 공고.없는필드")`
#[test]
fn view_package_bad_sql_cancels_added_columns_missing_field() {
    bad_sql_cancels_everything("SELECT 공고.없는필드");
}

// ── 저장 ─────────────────────────────────────────────

/// `저장_왕복과_식별자는_이름을_바꿔도_남는다` — 이름을 바꿔도 뷰·필드 식별자는 남고 옛 이름의 뷰는 남지 않는다.
/// 필드 식별자가 비어 오면 뽑는다. 정의는 v2 이고 기준이 없다.
#[test]
fn save_round_trip_keeps_ids_across_rename() {
    let f = fresh("uv-save");
    seed(&f);
    let s = f.store();
    let saved = single(
        s.save_user_view(
            &view(
                "공고문서",
                &[
                    (Some("c-1"), "공고번호", doc("notice", "입찰공고번호")),
                    (None, "공고명", doc("notice", "공고명")),
                ],
            ),
            None,
        )
        .unwrap(),
    );
    assert_eq!(saved.name, "공고문서");
    assert!(saved.broken.is_empty());
    assert_eq!(saved.definition["version"].as_i64(), Some(2));
    assert_eq!(saved.definition["contracts"], "split");
    let ids = column_ids(&saved);
    assert_eq!(ids[0], "c-1");
    assert!(!ids[1].is_empty());
    assert_eq!(user_view_names(&f), ["u_공고문서"]);
    assert_eq!(
        read(
            &f,
            &format!("SELECT 공고명 FROM u_공고문서 WHERE 공고번호 = '{N1}' LIMIT 1")
        ),
        "시험 공고 하나"
    );
    assert!(!rows(&f, "SELECT name FROM pragma_table_info('user_view')").contains(&"base".into()));

    // 저장 꼴의 필드 식에는 식별자를 싣지 않는다(§4).
    assert!(
        saved.definition["columns"][0]["formula"]
            .get("id")
            .is_none()
    );

    let renamed = single(
        s.save_user_view(
            &view(
                "공고서류",
                &[
                    (Some(ids[0].as_str()), "번호", doc("notice", "입찰공고번호")),
                    (Some(ids[1].as_str()), "공고명", doc("notice", "공고명")),
                ],
            ),
            Some(&saved.id),
        )
        .unwrap(),
    );
    assert_eq!(renamed.id, saved.id);
    assert_eq!(column_ids(&renamed), ids);
    assert_eq!(user_view_names(&f), ["u_공고서류"]);

    assert!(s.delete_user_view(&saved.id).unwrap().is_empty());
    assert!(user_view_names(&f).is_empty());
    assert!(invalid(s.delete_user_view(&saved.id)).contains("맞춤 뷰가 없습니다"));
}

/// `필드_차례는_정의의_차례다` — 문서의 차례와 상관없다.
#[test]
fn field_order_is_definition_order() {
    let f = fresh("uv-order");
    seed(&f);
    f.store()
        .save_user_view(
            &view(
                "문서",
                &[
                    (None, "공고명", doc("notice", "공고명")),
                    (None, "접수번호", doc("request", "접수번호")),
                    (None, "품목 수", notice_item_count()),
                    (None, "메모", doc("notice", "메모")),
                ],
            ),
            None,
        )
        .unwrap();
    assert_eq!(
        columns_of(&f, "u_문서"),
        ["공고명", "접수번호", "품목 수", "메모"]
    );
}

/// `세_문서의_사람_열과_가공열을_함께_읽는다` — 세 문서의 사람 열·가공열을 함께 읽고, SQL 필드는 바깥 행의 문서를
/// `접수`·`공고`·`계약` 으로 부른다.
#[test]
fn reads_user_and_computed_columns_of_all_three_documents() {
    let f = fresh("uv-three");
    seed(&f);
    let s = f.store();
    s.add_user_column("request", "확인여부", "text", &[])
        .unwrap();
    s.set_user_field(
        &EntityRef::new("request", "TESTR1", "000"),
        "확인여부",
        Some("예"),
    )
    .unwrap();
    s.save_computed_column(
        "contract",
        "품목개수",
        &flow_of(json!({ "kind": "items" }), vec![json!({ "op": "count" })]).to_string(),
        None,
    )
    .unwrap();
    s.save_user_view(
        &view(
            "문서",
            &[
                (None, "계약번호", doc("contract", "계약번호")),
                (None, "확인", doc("request", "확인여부")),
                (None, "메모", doc("notice", "메모")),
                (None, "품목개수", doc("contract", "품목개수")),
                (
                    None,
                    "대표품명",
                    flow_of(
                        json!({ "kind": "linked", "link": "contract", "items": true }),
                        vec![
                            json!({ "op": "get", "field": "품명" }),
                            json!({ "op": "distinct" }),
                            json!({ "op": "abbrev", "n": 1, "text": "{앞} 외 {나머지}종" }),
                        ],
                    ),
                ),
                (None, "공고명SQL", sql("SELECT 공고.공고명")),
                (
                    None,
                    "비면",
                    flow_of(
                        json!({ "kind": "text", "text": "" }),
                        vec![json!({ "op": "ifEmpty", "field": "공고.메모" })],
                    ),
                ),
            ],
        ),
        None,
    )
    .unwrap();

    assert_eq!(
        read(
            &f,
            "SELECT 확인 || '|' || 메모 || '|' || 품목개수 || '|' || 대표품명 || '|' || 공고명SQL || '|' || 비면 FROM u_문서 WHERE 계약번호 = 'TESTK100'"
        ),
        "예|확인함|2|책상|시험 공고 하나|확인함"
    );
    assert_eq!(
        read(
            &f,
            "SELECT 대표품명 FROM u_문서 WHERE 계약번호 = 'TESTK200'"
        ),
        "의자"
    );
}

/// `줄은_v_통합과_같은_사슬이다` — 계약마다 한 줄이면 건 수가 담긴 종류로 거른 `v_통합` 의 줄 수와 같고, 사슬의 글은
/// 두 뷰에 한 벌로 들어 있다.
#[test]
fn rows_follow_the_same_chain_as_unified_view() {
    let f = fresh("uv-chain");
    seed(&f);
    let s = f.store();
    s.save_user_view(
        &view(
            "셋",
            &[
                (None, "접수번호", doc("request", "접수번호")),
                (None, "공고번호", doc("notice", "입찰공고번호")),
                (None, "계약번호", doc("contract", "계약번호")),
            ],
        ),
        None,
    )
    .unwrap();
    s.save_user_view(
        &view("공고만", &[(None, "공고명", doc("notice", "공고명"))]),
        None,
    )
    .unwrap();
    s.save_user_view(
        &view("계약만", &[(None, "계약번호", doc("contract", "계약번호"))]),
        None,
    )
    .unwrap();
    s.save_user_view(
        &view(
            "접수공고",
            &[
                (None, "접수번호", doc("request", "접수번호")),
                (None, "공고명", doc("notice", "공고명")),
            ],
        ),
        None,
    )
    .unwrap();

    assert_eq!(count(&f, "v_통합"), 6);
    assert_eq!(count(&f, "u_셋"), 6);
    assert_eq!(
        rows(
            &f,
            "SELECT 접수번호 || '|' || 입찰공고번호 || '|' || 계약번호 FROM v_통합 ORDER BY 1"
        ),
        rows(
            &f,
            "SELECT 접수번호 || '|' || 공고번호 || '|' || 계약번호 FROM u_셋 ORDER BY 1"
        )
    );
    let n = |sql: &str| -> i64 { read(&f, sql).parse().unwrap() };
    assert_eq!(
        n("SELECT CAST(count(*) AS TEXT) FROM v_통합 WHERE 입찰공고번호 <> ''"),
        count(&f, "u_공고만")
    );
    assert_eq!(
        n("SELECT CAST(count(*) AS TEXT) FROM v_통합 WHERE 계약번호 <> ''"),
        count(&f, "u_계약만")
    );
    assert_eq!(
        n("SELECT CAST(count(*) AS TEXT) FROM v_통합 WHERE 입찰공고번호 <> '' OR 접수번호 <> ''"),
        count(&f, "u_접수공고")
    );

    // 사슬은 한 벌이다 — 두 뷰가 같은 글을 부른다. C# 은 `Views.사슬` 을 견주지만 Rust 의 `views::chain` 은 크레이트
    // 밖에 없어, `v_통합` 의 정의에서 사슬 CTE 를 떠 `u_셋` 의 정의에 그대로 있는지 본다.
    let unified = read(&f, "SELECT sql FROM sqlite_master WHERE name = 'v_통합'");
    let chain = chain_cte(&unified);
    assert!(chain.contains("UNION ALL"), "{chain}");
    assert!(read(&f, "SELECT sql FROM sqlite_master WHERE name = 'u_셋'").contains(chain));
}

/// 뷰 정의에서 `WITH 사슬(…) AS ( … )` 를 괄호 짝으로 떠낸다(`--` 주석과 따옴표 안은 세지 않는다).
fn chain_cte(sql: &str) -> &str {
    let start = sql.find("WITH 사슬(").expect("사슬 CTE");
    let body = sql[start..].find(" AS (").expect("AS (") + start + " AS ".len();
    let bytes = sql.as_bytes();
    let (mut depth, mut i) = (0i32, body);
    while i < bytes.len() {
        match bytes[i] {
            b'-' if bytes.get(i + 1) == Some(&b'-') => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            q @ (b'\'' | b'"') => {
                i += 1;
                while i < bytes.len() && bytes[i] != q {
                    i += 1;
                }
            }
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return &sql[start..=i];
                }
            }
            _ => {}
        }
        i += 1;
    }
    panic!("사슬 CTE 가 닫히지 않는다");
}

/// `모으기는_계약이_여럿인_건을_한_줄로_잇는다` — 계약 필드는 계약마다 셈해 계약본번호 차례로 `", "` 로 잇는다.
#[test]
fn fold_joins_many_contracts_into_one_row() {
    let f = fresh("uv-fold");
    seed(&f);
    let s = f.store();
    let folded = single(
        s.save_user_view(
            &view_with(
                "문서",
                "fold",
                &[
                    (None, "공고번호", doc("notice", "입찰공고번호")),
                    (None, "계약번호", doc("contract", "계약번호")),
                    (
                        None,
                        "품목",
                        flow_of(
                            json!({ "kind": "linked", "link": "contract", "items": true }),
                            vec![json!({ "op": "count" })],
                        ),
                    ),
                    (
                        None,
                        "건명",
                        flow_of(
                            json!({ "kind": "linked", "link": "contract", "field": "계약건명" }),
                            vec![json!({ "op": "after", "text": "시험 " })],
                        ),
                    ),
                ],
            ),
            None,
        )
        .unwrap(),
    );
    assert_eq!(folded.definition["contracts"], "fold");

    assert_eq!(
        read(
            &f,
            &format!(
                "SELECT 계약번호 || '|' || 품목 || '|' || 건명 FROM u_문서 WHERE 공고번호 = '{N1}'"
            )
        ),
        "TESTK100, TESTK200|2, 1|계약 하나, 계약 둘"
    );
    assert_eq!(
        read(
            &f,
            &format!("SELECT CAST(count(*) AS TEXT) FROM u_문서 WHERE 공고번호 = '{N1}'")
        ),
        "1"
    );
    // 공고건 둘 + 매달리지 못한 계약 둘.
    assert_eq!(count(&f, "u_문서"), 4);

    s.save_user_view(
        &view_with(
            "문서",
            "split",
            &[
                (None, "공고번호", doc("notice", "입찰공고번호")),
                (None, "계약번호", doc("contract", "계약번호")),
            ],
        ),
        Some(&folded.id),
    )
    .unwrap();
    assert_eq!(
        rows(
            &f,
            &format!("SELECT 계약번호 FROM u_문서 WHERE 공고번호 = '{N1}' ORDER BY 1")
        ),
        ["TESTK100", "TESTK200"]
    );
    assert!(single(s.user_views().unwrap()).broken.is_empty());
}

/// `공고건이_없는_사슬은_접히지_않는다` — 계약만·접수만인 사슬끼리 한 줄이 되지 않는다.
#[test]
fn chains_without_notice_do_not_fold() {
    let f = fresh("uv-fold-loose");
    seed(&f);
    f.store()
        .save_user_view(
            &view_with(
                "문서",
                "fold",
                &[
                    (None, "접수번호", doc("request", "접수번호")),
                    (None, "계약번호", doc("contract", "계약번호")),
                ],
            ),
            None,
        )
        .unwrap();
    assert_eq!(
        rows(
            &f,
            "SELECT 계약번호 FROM u_문서 WHERE 접수번호 = '' ORDER BY 1"
        ),
        ["TESTK300", "TESTK400"]
    );
    assert_eq!(
        rows(
            &f,
            "SELECT 계약번호 FROM u_문서 WHERE 접수번호 <> '' ORDER BY 1 DESC"
        ),
        ["TESTK100, TESTK200", ""]
    );
    assert_eq!(count(&f, "u_문서"), 4);
}

/// `담긴_종류가_없는_건은_줄이_서지_않는다` — 흐름식이 읽는 문서 종류가 하나도 없는 건은 줄이 서지 않는다. SQL 필드는
/// 거르지 않고, 문서를 읽지 않는 흐름식뿐이면 모든 조달 건이 선다.
#[test]
fn chains_without_read_kinds_produce_no_row() {
    let f = fresh("uv-filter");
    seed(&f);
    let s = f.store();
    s.save_user_view(
        &view("접수", &[(None, "접수번호", doc("request", "접수번호"))]),
        None,
    )
    .unwrap();
    s.save_user_view(
        &view("SQL", &[(None, "n", sql("SELECT count(*) FROM v_공고"))]),
        None,
    )
    .unwrap();
    s.save_user_view(
        &view(
            "글",
            &[(
                None,
                "t",
                flow_of(json!({ "kind": "text", "text": "고정" }), vec![]),
            )],
        ),
        None,
    )
    .unwrap();
    s.save_user_view(
        &view(
            "비면",
            &[(
                None,
                "t",
                flow_of(
                    json!({ "kind": "text", "text": "" }),
                    vec![json!({ "op": "ifEmpty", "field": "계약.계약번호" })],
                ),
            )],
        ),
        None,
    )
    .unwrap();

    // 접수1 은 계약마다 두 줄이고, 접수2 는 한 줄이다.
    assert_eq!(
        rows(&f, "SELECT 접수번호 FROM u_접수 ORDER BY 1"),
        ["TESTR1-000", "TESTR1-000", "TESTR2-000"]
    );
    assert_eq!(count(&f, "u_SQL"), 6);
    assert_eq!(count(&f, "u_글"), 6);
    assert_eq!(count(&f, "u_비면"), 4);
}

/// `이행의_원천과_틀린_이름은_거절한다` — 「이 행」 의 원천(`field`·`items`)은 맞춤 뷰에 없다. 이름이 비거나 겹치거나
/// 큰따옴표가 있어도 거절한다. 뷰끼리는 대소문자도 겹침이다.
#[test]
fn rejects_this_row_sources_and_bad_names() {
    let f = fresh("uv-reject");
    seed(&f);
    let s = f.store();
    let field = invalid(s.save_user_view(
        &view(
            "문서",
            &[(
                None,
                "공고명",
                flow_of(json!({ "kind": "field", "field": "공고명" }), vec![]),
            )],
        ),
        None,
    ));
    assert!(
        field.contains("‘공고명’ 필드를 세울 수 없습니다"),
        "{field}"
    );
    assert!(field.contains("‘공고.공고명’"), "{field}");
    invalid(s.save_user_view(
        &view(
            "문서",
            &[(
                None,
                "n",
                flow_of(json!({ "kind": "items" }), vec![json!({ "op": "count" })]),
            )],
        ),
        None,
    ));
    let missing = invalid(s.save_user_view(
        &view("문서", &[(None, "x", doc("notice", "그런열없음"))]),
        None,
    ));
    assert!(missing.contains("공고에 없는 필드"), "{missing}");
    invalid(s.save_user_view(
        &view_with("문서", "many", &[(None, "a", doc("notice", "공고명"))]),
        None,
    ));

    invalid(s.save_user_view(&view(" ", &[(None, "a", doc("notice", "공고명"))]), None));
    invalid(s.save_user_view(&view("a\"b", &[(None, "a", doc("notice", "공고명"))]), None));
    assert!(invalid(s.save_user_view(&view("문서", &[]), None)).contains("필드를 하나 이상"));
    assert!(
        invalid(s.save_user_view(
            &view(
                "문서",
                &[
                    (None, "a", doc("notice", "공고명")),
                    (None, "A", doc("notice", "입찰공고번호")),
                ],
            ),
            None,
        ))
        .contains("겹칩니다")
    );
    invalid(s.save_user_view(&view("문서", &[(None, " ", doc("notice", "공고명"))]), None));
    s.save_user_view(&view("Doc", &[(None, "a", doc("notice", "공고명"))]), None)
        .unwrap();
    assert!(
        invalid(s.save_user_view(
            &view("doc", &[(None, "a", doc("contract", "계약번호"))]),
            None
        ))
        .contains("‘doc’ 맞춤 뷰가 이미 있습니다")
    );
    assert_eq!(user_view_names(&f), ["u_Doc"]);
}

/// `빈_값은_빈_글이다` — 빈 값은 NULL 이 아니라 빈 글이다. 문서가 없는 건의 칸도 그렇다.
#[test]
fn blank_values_are_empty_text() {
    let f = fresh("uv-blank");
    seed(&f);
    f.store()
        .save_user_view(
            &view(
                "문서",
                &[
                    (None, "메모", doc("notice", "메모")),
                    (None, "접수", doc("request", "요청명")),
                    (None, "개수", notice_item_count()),
                    (
                        None,
                        "SQL",
                        sql("SELECT 계약건명 FROM v_계약 WHERE 계약번호 = 'NONE'"),
                    ),
                    (
                        None,
                        "계약",
                        flow_of(
                            json!({ "kind": "linked", "link": "contract" }),
                            vec![json!({ "op": "get", "field": "계약건명" })],
                        ),
                    ),
                ],
            ),
            None,
        )
        .unwrap();
    for column in ["메모", "접수", "SQL", "계약"] {
        assert_eq!(
            read(
                &f,
                &format!("SELECT typeof(\"{column}\") FROM u_문서 WHERE \"{column}\" = '' LIMIT 1")
            ),
            "text"
        );
    }
    assert_eq!(
        read(
            &f,
            "SELECT CAST(count(*) AS TEXT) FROM u_문서 WHERE 메모 IS NULL OR 접수 IS NULL OR 개수 IS NULL OR SQL IS NULL OR 계약 IS NULL"
        ),
        "0"
    );
    assert_eq!(
        read(
            &f,
            "SELECT 개수 FROM u_문서 WHERE 접수 = 'x' OR 메모 = '' LIMIT 1"
        ),
        "0"
    );
}

/// `깨진_필드는_빈_글로_서고_다른_것은_선다` — 깨진 필드는 빈 글 + 까닭 주석으로 서고, 같은 뷰의 다른 필드와 다른 뷰는
/// 그대로 선다. 까닭은 뷰 정의 글에서 읽혀 목록의 `broken` 에 필드 식별자로 실린다.
#[test]
fn broken_fields_stand_as_empty_text_and_others_stand() {
    let f = fresh("uv-broken");
    seed(&f);
    let s = f.store();
    let good = single(
        s.save_user_view(
            &view("멀쩡", &[(None, "공고명", doc("notice", "공고명"))]),
            None,
        )
        .unwrap(),
    );
    // 손으로 고친 파일처럼 — 없는 열을 읽는 식과 SQL 이 맞춤 뷰를 읽는 식을 넣는다.
    let definition = json!({ "version": 2, "contracts": "split", "columns": [
        { "id": "k1", "name": "공고명", "formula": doc("notice", "공고명") },
        { "id": "k2", "name": "없는열", "formula": doc("notice", "그런열없음") },
        { "id": "k3", "name": "남의뷰", "formula": sql("SELECT count(*) FROM u_멀쩡") },
    ] });
    f.conn()
        .execute(
            "INSERT INTO user_view (id, name, definition, sort_order) VALUES ('v-broken', '깨짐', ?1, 99)",
            [definition.to_string()],
        )
        .unwrap();
    f.database().refresh_views().unwrap();

    assert_eq!(user_view_names(&f), ["u_깨짐", "u_멀쩡"]);
    assert_eq!(columns_of(&f, "u_깨짐"), ["공고명", "없는열", "남의뷰"]);
    assert_eq!(
        read(
            &f,
            "SELECT 공고명 FROM u_깨짐 WHERE 공고명 LIKE '%하나' LIMIT 1"
        ),
        "시험 공고 하나"
    );
    assert_eq!(read(&f, "SELECT 없는열 FROM u_깨짐 LIMIT 1"), "");
    assert_eq!(read(&f, "SELECT 남의뷰 FROM u_깨짐 LIMIT 1"), "");
    assert_eq!(count(&f, "u_멀쩡"), 3);

    let broken = views::broken_in(Some(&read(
        &f,
        "SELECT sql FROM sqlite_master WHERE name = 'u_깨짐'",
    )));
    let mut names: Vec<&str> = broken.iter().map(|(n, _)| n.as_str()).collect();
    names.sort();
    assert_eq!(names, ["남의뷰", "없는열"]);
    assert!(
        broken
            .iter()
            .find(|(n, _)| n == "없는열")
            .unwrap()
            .1
            .contains("그런열없음")
    );

    let listed = s
        .user_views()
        .unwrap()
        .into_iter()
        .find(|v| v.id == "v-broken")
        .unwrap();
    let mut keys: Vec<&str> = listed.broken.iter().map(|(k, _)| k.as_str()).collect();
    keys.sort();
    assert_eq!(keys, ["k2", "k3"]);
    assert!(
        s.user_views()
            .unwrap()
            .into_iter()
            .find(|v| v.id == good.id)
            .unwrap()
            .broken
            .is_empty()
    );

    // 저장은 이 뷰의 필드가 깨진 식으로 서면 거절한다 — 같은 정의를 저장하려 하면 막힌다.
    invalid(s.save_user_view(
        &view(
            "깨짐",
            &[
                (Some("k1"), "공고명", doc("notice", "공고명")),
                (Some("k3"), "남의뷰", sql("SELECT count(*) FROM u_멀쩡")),
            ],
        ),
        Some("v-broken"),
    ));
}

// ── 사람 열과 맞물림 ─────────────────────────────────────

fn reader_count(s: &Store<'_>, entity_type: &str, field: &str) -> Option<i32> {
    s.user_view_readers(entity_type)
        .unwrap()
        .get(&field.to_string())
        .copied()
}

/// `사람_열의_이름을_바꾸면_따라가고_지우면_깨진_식이_된다` — 사람 열 이름을 바꾸면 그 열을 읽던 맞춤 뷰 필드가 새
/// 이름을 따라간다. 다른 종류의 같은 이름은 그대로다. 지우면 그 필드는 빈 글(깨진 식)로 선다.
#[test]
fn renaming_user_column_follows_and_removing_breaks() {
    let f = fresh("uv-rename");
    seed(&f);
    let s = f.store();
    let saved = single(
        s.save_user_view(
            &view(
                "문서",
                &[
                    (None, "메모값", doc("notice", "메모")),
                    (
                        None,
                        "비면",
                        flow_of(
                            json!({ "kind": "text", "text": "" }),
                            vec![json!({ "op": "ifEmpty", "field": "공고.메모" })],
                        ),
                    ),
                    (
                        None,
                        "꺼냄",
                        flow_of(
                            json!({ "kind": "linked", "link": "notice" }),
                            vec![json!({ "op": "get", "field": "메모" })],
                        ),
                    ),
                    (None, "계약메모", doc("contract", "메모")),
                    (None, "공고명", doc("notice", "공고명")),
                ],
            ),
            None,
        )
        .unwrap(),
    );
    s.save_user_view(
        &view("계약문서", &[(None, "메모", doc("contract", "메모"))]),
        None,
    )
    .unwrap();

    assert_eq!(reader_count(&s, "notice", "메모"), Some(1));
    assert_eq!(reader_count(&s, "contract", "메모"), Some(2));
    assert_eq!(reader_count(&s, "notice", "검토여부"), None);

    s.update_user_column("notice", "메모", "비고", "text", &[])
        .unwrap();
    let rewritten = s
        .user_views()
        .unwrap()
        .into_iter()
        .find(|v| v.id == saved.id)
        .unwrap();
    assert!(rewritten.broken.is_empty(), "{:?}", rewritten.broken);
    let flow = |i: usize| rewritten.definition["columns"][i]["formula"]["flow"].clone();
    assert_eq!(flow(0)["from"]["field"], "비고");
    assert_eq!(flow(1)["steps"][0]["field"], "공고.비고");
    assert_eq!(flow(2)["steps"][0]["field"], "비고");
    assert_eq!(flow(3)["from"]["field"], "메모");
    assert_eq!(
        read(
            &f,
            "SELECT 메모값 || '|' || 비면 || '|' || 꺼냄 FROM u_문서 WHERE 메모값 <> '' LIMIT 1"
        ),
        "확인함|확인함|확인함"
    );

    // 가공열 이름 바꾸기(SaveComputedColumn 의 previousName)도 같은 길이다.
    let count_json =
        flow_of(json!({ "kind": "items" }), vec![json!({ "op": "count" })]).to_string();
    s.save_computed_column("notice", "개수", &count_json, None)
        .unwrap();
    s.save_user_view(&view("개수뷰", &[(None, "n", doc("notice", "개수"))]), None)
        .unwrap();
    s.save_computed_column("notice", "품목수", &count_json, Some("개수"))
        .unwrap();
    assert_eq!(
        read(&f, "SELECT n FROM u_개수뷰 WHERE n <> '0' LIMIT 1"),
        "2"
    );

    assert_eq!(reader_count(&s, "notice", "비고"), Some(1));
    s.remove_user_column("notice", "비고").unwrap();
    let after = s
        .user_views()
        .unwrap()
        .into_iter()
        .find(|v| v.id == saved.id)
        .unwrap();
    assert_eq!(after.broken.len(), 3);
    assert_eq!(
        columns_of(&f, "u_문서"),
        ["메모값", "비면", "꺼냄", "계약메모", "공고명"]
    );
    assert_eq!(read(&f, "SELECT 메모값 FROM u_문서 LIMIT 1"), "");
    assert_eq!(
        read(
            &f,
            "SELECT 공고명 FROM u_문서 WHERE 공고명 LIKE '%하나' LIMIT 1"
        ),
        "시험 공고 하나"
    );
}

/// 같은 키가 두 번 든 맞춤 뷰 정의(손으로 고친 파일). 최상위·필드·필드의 식 어디에 들었든 공고의 `메모` 를 읽는
/// 필드 하나다.
fn duplicate_key_definition(place: &str) -> String {
    let mut formula = doc("notice", "메모").to_string();
    if place == "formula" {
        formula = format!("{{\"source\":\"flow\",{}", &formula[1..]);
    }
    let name = if place == "column" {
        "\"name\":\"메모값\",\"name\":\"메모값\""
    } else {
        "\"name\":\"메모값\""
    };
    let contracts = if place == "root" {
        "\"contracts\":\"split\",\"contracts\":\"split\""
    } else {
        "\"contracts\":\"split\""
    };
    format!(
        "{{\"version\":2,{contracts},\"columns\":[{{\"id\":\"k1\",{name},\"formula\":{formula}}}]}}"
    )
}

/// `겹친_키가_든_맞춤_뷰_정의는_읽을_수_없는_정의다` — 같은 키가 두 번 든 맞춤 뷰 정의는 읽을 수 없는 정의와 같다. 필드가
/// 없고, 모으지 않고, 사람 열을 읽는 뷰로 세지 않고, 열 이름이 바뀌어도 그 글을 고쳐 쓰지 않는다. 뒤의 값을 골라 읽으면
/// C#(읽을 수 없는 정의)과 갈린다.
#[test]
fn duplicate_keys_make_a_user_view_definition_unreadable() {
    for place in ["root", "column", "formula"] {
        let f = fresh(&format!("uv-dup-{place}"));
        seed(&f);
        let s = f.store();
        let definition = duplicate_key_definition(place);
        assert!(
            user_view_builder::parse_columns(&definition)
                .unwrap()
                .is_empty(),
            "{place}"
        );
        assert!(
            !user_view_builder::parse_fold(&definition.replace("\"split\"", "\"fold\"")).unwrap(),
            "{place}"
        );

        f.conn()
            .execute(
                "INSERT INTO user_view (id, name, definition, sort_order) VALUES ('v-dup', '겹침', ?1, 10)",
                [&definition],
            )
            .unwrap();
        f.conn()
            .execute(
                "INSERT INTO user_view (id, name, definition, sort_order) VALUES ('v-bad', '못읽음', '{', 20)",
                [],
            )
            .unwrap();
        f.database().refresh_views().unwrap();
        // 필드가 없는 보기는 뷰로 서지 않는다 — 읽을 수 없는 정의와 같다.
        assert!(user_view_names(&f).is_empty(), "{place}");

        assert_eq!(reader_count(&s, "notice", "메모"), None, "{place}");
        s.update_user_column("notice", "메모", "비고", "text", &[])
            .unwrap();
        assert_eq!(
            read(&f, "SELECT definition FROM user_view WHERE id = 'v-dup'"),
            definition,
            "{place}"
        );
    }
}

/// `판올림은_겹친_키가_든_예시_이름의_보기를_그대로_두고_연다` — 예시 보기와 이름이 같고 정의에 같은 키가 두 번 든
/// 보기도 판올림이 멈추지 않는다. 스키마 27 은 손댄 것으로 보아 갈아 끼우지 않고, 뷰를 지을 때는 필드 없는 보기로 선다.
#[test]
fn migration_leaves_example_named_view_with_duplicate_keys_alone() {
    let temp = Temp::new("uv-v25dup");
    let env = TestEnv::new();
    let path = temp.path("작업.pclm");
    legacy_at(&path, 25, "work");
    let definition = duplicate_key_definition("root");
    raw(&path)
        .execute(
            "INSERT INTO user_view (id, name, definition, sort_order) VALUES ('v-own', '서식용 예시', ?1, 10)",
            [&definition],
        )
        .unwrap();
    let database = Database::new(&path, Access::Write);
    database.migrate(&env).unwrap();
    let kept = single(Store::new(database, &env).user_views().unwrap());
    assert_eq!(kept.id, "v-own");
    assert_eq!(
        read_at(&path, "SELECT definition FROM user_view WHERE id = 'v-own'"),
        definition
    );
    // 필드가 없는 보기로 읽어 뷰는 서지 않는다.
    assert!(
        rows_at(
            &path,
            "SELECT name FROM sqlite_master WHERE type = 'view' AND name LIKE 'u\\_%' ESCAPE '\\'"
        )
        .is_empty()
    );
}

/// `객체가_아닌_맞춤_뷰_정의는_읽을_수_없는_정의다` — 객체가 아닌 맞춤 뷰 정의(배열·글·수 — 손으로 고친 파일)는 읽을
/// 수 없는 정의와 같다. 필드가 없고, 모으지 않고, 뷰 짓기·열 이름 바꾸기·지우기 확인 문구가 멈추지 않는다. 예전에는
/// C# 의 `InvalidOperationException` 을 일부러 흉내 내 이 자리들이 멈췄다.
#[test]
fn non_object_user_view_definition_is_unreadable() {
    for definition in ["[1]", "[]", "5", "\"x\""] {
        let f = fresh("uv-odd");
        seed(&f);
        let s = f.store();
        assert!(
            user_view_builder::parse_columns(definition)
                .unwrap()
                .is_empty(),
            "{definition}"
        );
        assert!(
            !user_view_builder::parse_fold(definition).unwrap(),
            "{definition}"
        );

        f.conn()
            .execute(
                "INSERT INTO user_view (id, name, definition, sort_order) VALUES ('v-odd', '모양', ?1, 10)",
                [definition],
            )
            .unwrap();
        f.database().refresh_views().unwrap();
        assert!(user_view_names(&f).is_empty(), "{definition}");
        assert_eq!(single(s.user_views().unwrap()).id, "v-odd");
        assert_eq!(reader_count(&s, "notice", "메모"), None, "{definition}");
        s.update_user_column("notice", "메모", "비고", "text", &[])
            .unwrap();
        assert_eq!(
            read(&f, "SELECT definition FROM user_view WHERE id = 'v-odd'"),
            definition
        );
    }
}

/// `식이_null_인_필드는_빈_식이다` — 필드의 식이 JSON null 이면 식이 없는 필드다(빈 글). `"null"` 이라는 글을 내면 C# 과
/// 거절 문구가 갈릴 수 있었다.
#[test]
fn null_formula_is_an_empty_formula() {
    let column = single(
        user_view_builder::parse_columns(
            r#"{"version":2,"contracts":"split","columns":[{"id":"k1","name":"값","formula":null}]}"#,
        )
        .unwrap(),
    );
    assert_eq!(column.formula, "");
}

/// `겹친_키가_든_맞춤_뷰_입력은_형식_오류다` — 화면이 보낸 뷰에 같은 키가 두 번 들었으면(최상위·정의·필드·식 어디든)
/// 「맞춤 뷰 형식이 올바르지 않습니다.」 다. 뒤의 값을 골라 받아들이면 C# 과 갈린다.
#[test]
fn duplicate_keys_in_user_view_input_are_a_format_error() {
    for place in ["view", "root", "column", "formula", "array"] {
        let f = fresh(&format!("uv-dupin-{place}"));
        seed(&f);
        let s = f.store();
        let json = match place {
            "view" => format!(
                "{{\"name\":\"겹침\",\"name\":\"겹침\",\"definition\":{}}}",
                duplicate_key_definition("none")
            ),
            "array" => format!(
                "[{{\"name\":\"겹침\",\"definition\":{}}}]",
                duplicate_key_definition("none")
            ),
            _ => format!(
                "{{\"name\":\"겹침\",\"definition\":{}}}",
                duplicate_key_definition(place)
            ),
        };
        assert_eq!(
            invalid(s.preview_user_view(&json, None)),
            "맞춤 뷰 형식이 올바르지 않습니다.",
            "{place}"
        );
        assert_eq!(
            invalid(s.user_view_samples(&json, None, None)),
            "맞춤 뷰 형식이 올바르지 않습니다.",
            "{place}"
        );
        assert_eq!(
            invalid(s.save_user_view(&json, None)),
            "맞춤 뷰 형식이 올바르지 않습니다.",
            "{place}"
        );
        assert!(s.user_views().unwrap().is_empty(), "{place}");
    }
}

/// `읽을_수_없는_정의는_빈_정의로_보이고_내보낼_수_없다` — 저장된 정의가 읽을 수 없는 글이면(깨진 글·같은 키가 두 번 든
/// 글) 목록에는 빈 정의로 보이고, 내보내기는 「맞춤 뷰 형식이 올바르지 않습니다.」 다. 겹친 키를 뒤의 값으로 접어 내보내면
/// C#(겹친 채)과 글이 갈렸다.
#[test]
fn unreadable_definition_lists_as_empty_and_does_not_export() {
    let f = fresh("uv-unreadable");
    let s = f.store();
    let definition = duplicate_key_definition("root");
    f.conn()
        .execute(
            "INSERT INTO user_view (id, name, definition, sort_order) VALUES ('v-dup', '겹침', ?1, 10)",
            [&definition],
        )
        .unwrap();
    f.conn()
        .execute(
            "INSERT INTO user_view (id, name, definition, sort_order) VALUES ('v-bad', '못읽음', '{', 20)",
            [],
        )
        .unwrap();
    let views = s.user_views().unwrap();
    assert_eq!(
        views.iter().map(|v| v.id.as_str()).collect::<Vec<_>>(),
        ["v-dup", "v-bad"]
    );
    for view in &views {
        assert_eq!(
            view.definition,
            json!({ "version": 2, "contracts": "split", "columns": [] })
        );
        assert_eq!(
            invalid(s.export_view_package(std::slice::from_ref(&view.id))),
            "맞춤 뷰 형식이 올바르지 않습니다."
        );
    }
}

/// `겹친_키가_든_보기_구성은_읽을_수_없는_글이다` — 같은 키가 두 번 든 보기 구성 파일(손으로 고친 파일)은 읽을 수 없는
/// 글이다. 맨 위·보기 정의·맞춤 열의 식 어디에 들었든 「보기 구성 JSON 형식이 올바르지 않습니다」 다. 뒤의 값으로 접어
/// 읽으면 안쪽의 겹침이 지워져 C# 과 갈렸다.
#[test]
fn duplicate_keys_make_a_view_package_unreadable() {
    for place in ["root", "definition", "formula"] {
        let f = fresh(&format!("uv-pkgdup-{place}"));
        let s = f.store();
        s.add_user_column("notice", "원수", "number", &[]).unwrap();
        s.save_computed_column(
            "notice",
            "간접수",
            &sql("SELECT 현재.원수").to_string(),
            None,
        )
        .unwrap();
        let saved = single(
            s.save_user_view(
                &view("SQL문서", &[(Some("c-1"), "값", doc("notice", "간접수"))]),
                None,
            )
            .unwrap(),
        );
        let exported = s
            .export_view_package(std::slice::from_ref(&saved.id))
            .unwrap();
        assert!(
            s.preview_view_package(&exported).unwrap().errors.is_empty(),
            "{place}"
        );
        let (once, twice) = match place {
            "root" => (
                "\"format\": \"pclm-view-config\"",
                "\"format\": \"pclm-view-config\", \"format\": \"pclm-view-config\"",
            ),
            "definition" => (
                "\"contracts\": \"split\"",
                "\"contracts\": \"split\", \"contracts\": \"split\"",
            ),
            _ => (
                "\"sql\": \"SELECT 현재.원수\"",
                "\"sql\": \"SELECT 현재.원수\", \"sql\": \"SELECT 현재.원수\"",
            ),
        };
        assert_eq!(exported.matches(once).count(), 1, "{place}");
        let json = exported.replace(once, twice);
        assert_eq!(
            invalid(s.preview_view_package(&json)),
            "보기 구성 JSON 형식이 올바르지 않습니다. 자료가 아닌 구성 파일을 골라 주세요.",
            "{place}"
        );
    }
}

fn if_step(cond: Value) -> Value {
    json!({ "op": "if", "cond": cond, "then": "맞음", "else": "" })
}

/// `값_조건의_바깥_열은_비면과_같은_자리를_읽는다` — 값 하나를 견주는 조건의 마디는 바깥의 열을 읽는다. 그 마디가 읽는
/// 문서도 읽는 종류로 세어 줄을 거르고, 사람 열 이름을 바꾸면 따라간다.
#[test]
fn value_condition_reads_outer_column_like_if_empty() {
    let f = fresh("uv-cond");
    seed(&f);
    let s = f.store();
    let saved = single(
        s.save_user_view(
            &view(
                "조건",
                &[
                    (None, "계약번호", doc("contract", "계약번호")),
                    (
                        None,
                        "판정",
                        flow_of(
                            json!({ "kind": "linked", "link": "contract", "field": "계약번호" }),
                            vec![if_step(json!([
                                { "field": "", "cmp": "안비어있음" },
                                { "join": "and", "field": "공고.메모", "cmp": "=", "value": "확인함" }
                            ]))],
                        ),
                    ),
                ],
            ),
            None,
        )
        .unwrap(),
    );
    assert!(saved.broken.is_empty(), "{:?}", saved.broken);
    assert_eq!(
        rows(
            &f,
            "SELECT 계약번호 || '|' || 판정 FROM u_조건 WHERE 계약번호 <> '' ORDER BY 1"
        ),
        ["TESTK100|맞음", "TESTK200|맞음", "TESTK300|", "TESTK400|"]
    );

    // 글자 원천에 공고를 견주는 조건만 있으면 공고가 있는 건만 선다(사슬 여섯 중 공고가 있는 셋).
    s.save_user_view(
        &view(
            "공고조건",
            &[(
                None,
                "t",
                flow_of(
                    json!({ "kind": "text", "text": "" }),
                    vec![if_step(
                        json!([{ "field": "공고.입찰공고번호", "cmp": "안비어있음" }]),
                    )],
                ),
            )],
        ),
        None,
    )
    .unwrap();
    assert_eq!(count(&f, "u_공고조건"), 3);
    assert_eq!(rows(&f, "SELECT DISTINCT t FROM u_공고조건"), ["맞음"]);

    assert_eq!(reader_count(&s, "notice", "메모"), Some(1));
    s.update_user_column("notice", "메모", "비고", "text", &[])
        .unwrap();
    let rewritten = s
        .user_views()
        .unwrap()
        .into_iter()
        .find(|v| v.id == saved.id)
        .unwrap();
    assert!(rewritten.broken.is_empty(), "{:?}", rewritten.broken);
    assert_eq!(
        rewritten.definition["columns"][1]["formula"]["flow"]["steps"][0]["cond"][1]["field"],
        "공고.비고"
    );
    assert_eq!(
        read(&f, "SELECT 판정 FROM u_조건 WHERE 계약번호 = 'TESTK100'"),
        "맞음"
    );

    let bare = invalid(s.save_user_view(
        &view(
            "틀림",
            &[(
                None,
                "x",
                flow_of(
                    json!({ "kind": "text", "text": "" }),
                    vec![if_step(json!([{ "field": "비고", "cmp": "비어있음" }]))],
                ),
            )],
        ),
        None,
    ));
    assert!(
        bare.contains("문서를 붙여 적으세요 — ‘공고.비고’처럼 읽습니다"),
        "{bare}"
    );
    let missing = invalid(s.save_user_view(
        &view(
            "틀림",
            &[(
                None,
                "x",
                flow_of(
                    json!({ "kind": "text", "text": "" }),
                    vec![if_step(
                        json!([{ "field": "공고.없는열", "cmp": "비어있음" }]),
                    )],
                ),
            )],
        ),
        None,
    ));
    assert!(missing.contains("공고에 없는 필드: 없는열"), "{missing}");
}

// ── 미리보기·표본·보표 ─────────────────────────────────────

/// `필드_목록의_재료는_세_문서다` — 접수·공고·계약 셋이 면 뷰의 열(사람 열·가공열 포함)과 품목 필드를 들고, 「이 행」
/// 은 없다. 조달 건 수는 계약마다 한 줄로 세고, 계약이 여럿인 건 수를 함께 낸다.
#[test]
fn field_list_material_is_three_documents() {
    let f = fresh("uv-context");
    seed(&f);
    let s = f.store();
    s.save_computed_column(
        "contract",
        "품목개수",
        &flow_of(json!({ "kind": "items" }), vec![json!({ "op": "count" })]).to_string(),
        None,
    )
    .unwrap();
    let context = s.user_view_context().unwrap();
    let linked = context["available"]["linked"].as_array().unwrap();
    assert_eq!(
        linked
            .iter()
            .map(|l| l["kind"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["request", "notice", "contract"]
    );
    assert!(context["available"]["base"].as_array().unwrap().is_empty());
    let notice: Vec<&str> = linked[1]["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f.as_str().unwrap())
        .collect();
    assert_eq!(notice[notice.len() - 2..], ["검토여부", "메모"]);
    assert_eq!(
        linked[2]["fields"].as_array().unwrap().last().unwrap(),
        "품목개수"
    );
    assert!(
        linked[2]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f == "금액")
    );
    assert!(linked[0].get("many").is_none());
    assert_eq!(context["total"].as_i64(), Some(6));
    assert_eq!(context["manyContracts"].as_i64(), Some(1));
}

/// `표본은_그_뷰의_줄에서_고른다` — 표본 고르개는 그 뷰의 줄(거르기·모으기 뒤)에서 고른다. 보이는 번호는
/// 입찰공고번호 → 접수번호 → 계약번호, 제목은 공고명 → 요청명 → 계약건명.
#[test]
fn samples_come_from_the_view_rows() {
    let f = fresh("uv-samples");
    seed(&f);
    let s = f.store();
    let notices = view("문서", &[(None, "공고명", doc("notice", "공고명"))]);
    let split = samples(&s, &notices, None, None);
    assert_eq!(strs(&split, "number"), [N1, N1, N2]);
    assert_eq!(split[0]["title"], "시험 공고 하나");
    let keys: std::collections::HashSet<String> = strs(&split, "key").into_iter().collect();
    assert_eq!(keys.len(), 3);

    let folded = samples(
        &s,
        &view_with(
            "문서",
            "fold",
            &[
                (None, "공고명", doc("notice", "공고명")),
                (None, "계약번호", doc("contract", "계약번호")),
            ],
        ),
        None,
        None,
    );
    assert_eq!(strs(&folded, "number"), [N1, N2, "TESTK300", "TESTK400"]);
    assert_eq!(folded[2]["title"], "시험 계약 셋");

    let all = samples(&s, &view("빈뷰", &[]), None, None);
    assert_eq!(all.len(), 6);
    assert!(strs(&all, "title").contains(&"시험 접수 둘".to_string()));
    assert_eq!(
        strs(&samples(&s, &notices, Some("공고 둘"), None), "number"),
        ["TESTN2-000"]
    );
    assert_eq!(
        strs(&samples(&s, &notices, Some("testn2"), None), "number"),
        ["TESTN2-000"]
    );
    assert_eq!(
        strs(
            &samples(&s, &notices, None, Some("manyContracts")),
            "number"
        ),
        [N1, N1]
    );
    assert_eq!(
        strs(
            &samples(
                &s,
                &view_with(
                    "문서",
                    "fold",
                    &[(None, "계약번호", doc("contract", "계약번호"))]
                ),
                None,
                Some("manyContracts"),
            ),
            "number"
        ),
        [N1]
    );
    assert_eq!(
        strs(&samples(&s, &notices, None, Some("unlinked")), "number"),
        [N2]
    );
    invalid(s.user_view_samples(&notices, None, Some("weird")));
}

/// `미리보기는_흔적을_남기지_않는다` — 미리보기는 표본 한 건의 값과 필드마다 빈 값 수, 거르기·모으기 뒤의 건 수를
/// 낸다. sqlite_master 가 그대로다. 깨진 필드는 error 를 싣고 다른 필드는 값을 낸다. 이름은 보지 않는다.
#[test]
fn preview_leaves_no_trace() {
    let f = fresh("uv-preview");
    seed(&f);
    let s = f.store();
    let before = master(&f);
    let v = view_with(
        "",
        "fold",
        &[
            (Some("a"), "메모", doc("notice", "메모")),
            (Some("b"), "계약번호", doc("contract", "계약번호")),
            (Some("c"), "깨짐", doc("notice", "그런열없음")),
            (Some("d"), "메모", notice_item_count()),
        ],
    );
    let key = samples(&s, &v, None, None)[0]["key"]
        .as_str()
        .unwrap()
        .to_string();
    let preview = s.preview_user_view(&v, Some(&key)).unwrap();
    assert_eq!(before, master(&f));

    assert_eq!(preview["sample"]["key"], key.as_str());
    assert_eq!(preview["sample"]["number"], N1);
    assert_eq!(preview["sample"]["title"], "시험 공고 하나");
    assert_eq!(preview["total"].as_i64(), Some(4));
    let columns = preview["columns"].as_array().unwrap();
    assert_eq!(strs(columns, "id"), ["a", "b", "c", "d"]);
    assert_eq!(columns[0]["value"], "확인함");
    assert_eq!(columns[0]["blank"].as_i64(), Some(3));
    assert_eq!(columns[1]["value"], "TESTK100, TESTK200");
    assert_eq!(columns[1]["blank"].as_i64(), Some(1));
    assert_eq!(columns[2]["value"], "");
    assert!(columns[2]["error"].as_str().unwrap().contains("그런열없음"));
    assert_eq!(columns[3]["value"], "2");
    assert!(columns[3].get("error").is_none());

    let missing = s
        .preview_user_view(
            &view("", &[(Some("a"), "x", doc("notice", "공고명"))]),
            Some("NONE"),
        )
        .unwrap();
    assert!(missing["sample"].is_null());
    assert_eq!(missing["columns"][0]["value"], "");
}

/// `보표는_세_문서를_읽고_준_표본이_처음이다` — 보표는 맞춤 뷰 환경으로 돈다. 표본은 준 열쇠가 처음이다. 모은 건의
/// 열쇠면 단계는 계약 한 건을 보고 열에 찍힐 값은 뷰가 낼 값(이어 붙인 것)이다. SQL 은 바깥 행을 문서 별칭으로 부른다.
#[test]
fn trace_reads_three_documents_and_given_samples_first() {
    let f = fresh("uv-trace");
    seed(&f);
    let s = f.store();
    let memo = doc("notice", "메모").to_string();
    let keys = strs(
        &samples(
            &s,
            &view("문서", &[(None, "메모", doc("notice", "메모"))]),
            None,
            None,
        ),
        "key",
    );
    let trace = s
        .trace_user_view_column(&memo, Some(&[keys[2].clone(), keys[0].clone()]))
        .unwrap();
    let heads = &trace["samples"];
    assert_eq!(heads[0]["base"], N2);
    assert_eq!(heads[1]["base"], N1);
    assert_eq!(trace["final"], json!(["", "확인함"]));
    assert!(
        trace["sql"].as_str().unwrap().contains("공고.\"메모\""),
        "{}",
        trace["sql"]
    );

    let contract = doc("contract", "계약번호").to_string();
    let fold_key = samples(
        &s,
        &view_with(
            "문서",
            "fold",
            &[(None, "계약번호", doc("contract", "계약번호"))],
        ),
        None,
        Some("manyContracts"),
    )[0]["key"]
        .as_str()
        .unwrap()
        .to_string();
    let folded = s
        .trace_user_view_column(&contract, Some(&[fold_key]))
        .unwrap();
    assert_eq!(folded["steps"][0]["values"][0]["text"], "TESTK100");
    assert_eq!(folded["final"][0], "TESTK100, TESTK200");

    let items = s
        .trace_user_view_column(&notice_item_count().to_string(), Some(&[keys[0].clone()]))
        .unwrap();
    assert_eq!(items["samples"][0]["rows"].as_i64(), Some(2));

    // SQL 필드도 바깥 행의 문서를 별칭으로 읽는다.
    let by_sql = s
        .trace_user_view_column(
            &sql("SELECT 공고.공고명").to_string(),
            Some(&[keys[0].clone()]),
        )
        .unwrap();
    assert_eq!(by_sql["final"][0], "시험 공고 하나");
    assert!(by_sql["steps"][0].get("error").is_none());
}

/// Rust 만의 덫 지킴 — SQLite 의 JSON 부분형은 `json_object`·`json_group_array` 값이 스칼라 부질의를 지나면 사라져,
/// 다른 JSON 함수 안에 들면 글로 한 번 더 따옴표가 쳐진다. 보표의 「rows」 단계는 `json_group_array(json_object(..))`
/// 로 짓는다 — 행이 글로 감싸이면 Rust 쪽에서 객체가 아니라 빈 행이 되고 이름표가 빈다. 행이 객체로 와서 필드 값을
/// 그대로 드는지 본다(맞춤 뷰 보표와 가공열 보표 둘 다).
#[test]
fn json_subtype_survives_scalar_subquery() {
    let f = fresh("uv-json-subtype");
    seed(&f);
    let s = f.store();
    let keys = strs(
        &samples(
            &s,
            &view("문서", &[(None, "공고명", doc("notice", "공고명"))]),
            None,
            None,
        ),
        "key",
    );
    let check = |trace: &Value| {
        let shown = &trace["steps"][0]["values"][0];
        assert_eq!(shown["kind"], "list", "{shown}");
        assert_eq!(shown["count"].as_i64(), Some(2), "{shown}");
        let rows = shown["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 2, "{shown}");
        for row in rows {
            let row = row
                .as_object()
                .unwrap_or_else(|| panic!("행이 객체가 아니다: {row}"));
            assert!(!row.is_empty(), "행이 비었다(글로 감싸였다): {shown}");
            assert!(row.values().all(Value::is_string), "{shown}");
        }
        let cells: Vec<&str> = rows
            .iter()
            .flat_map(|r| r.as_object().unwrap().values())
            .filter_map(Value::as_str)
            .collect();
        assert!(
            cells.contains(&"책상") && cells.contains(&"의자"),
            "{shown}"
        );
        let labels: Vec<&str> = shown["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l.as_str().unwrap())
            .collect();
        assert_eq!(labels.len(), 2, "{shown}");
        assert!(
            labels.iter().all(|l| !l.is_empty() && !l.contains('{')),
            "{shown}"
        );
    };

    let items = flow_of(
        json!({ "kind": "linked", "link": "notice", "items": true }),
        vec![json!({ "op": "count" })],
    );
    check(
        &s.trace_user_view_column(&items.to_string(), Some(&[keys[0].clone()]))
            .unwrap(),
    );
    let own_items = flow_of(json!({ "kind": "items" }), vec![json!({ "op": "count" })]);
    let computed = s
        .trace_computed_column("notice", &own_items.to_string())
        .unwrap();
    let first = computed["samples"]
        .as_array()
        .unwrap()
        .iter()
        .position(|h| h["base"] == "TESTN1")
        .expect("TESTN1 표본");
    let mut one = computed.clone();
    one["steps"][0]["values"] = json!([computed["steps"][0]["values"][first].clone()]);
    check(&one);
}

// ── 계약면과 소비자 ─────────────────────────────────────

/// `메타_뷰는_필드마다_한_줄이고_옛_이름을_치운다` — `v_맞춤뷰` 는 필드 하나당 한 줄, 결과타입은 끝 꼴이다. 2판의
/// `v_사용자뷰` 는 제자리에서 바꾼 것이라 남아 있으면 열 때 치운다(ADR-034).
#[test]
fn meta_view_has_one_row_per_field_and_drops_legacy_name() {
    let f = fresh("uv-meta");
    seed(&f);
    let s = f.store();
    let saved = single(
        s.save_user_view(
            &view(
                "문서",
                &[
                    (Some("c-1"), "공고명", doc("notice", "공고명")),
                    (Some("c-2"), "개수", notice_item_count()),
                ],
            ),
            None,
        )
        .unwrap(),
    );

    assert_eq!(
        rows(
            &f,
            "SELECT 뷰식별자 || '|' || 뷰이름 || '|' || 열식별자 || '|' || 열이름 || '|' || 차례 || '|' || 결과타입 FROM v_맞춤뷰 ORDER BY 차례"
        ),
        [
            format!("{}|u_문서|c-1|공고명|1|text", saved.id),
            format!("{}|u_문서|c-2|개수|2|number", saved.id)
        ]
    );
    assert_eq!(
        columns_of(&f, "v_맞춤뷰"),
        [
            "뷰식별자",
            "뷰이름",
            "열식별자",
            "열이름",
            "차례",
            "결과타입",
            "가공식"
        ]
    );
    let formula: Value = serde_json::from_str(&read(
        &f,
        "SELECT 가공식 FROM v_맞춤뷰 WHERE 열식별자 = 'c-2'",
    ))
    .unwrap();
    assert_eq!(formula["flow"]["steps"][0]["op"], "count");
    assert!(views::NAMES.contains(&"v_맞춤뷰"));
    assert!(views::READ_ONLY.contains(&"v_맞춤뷰"));
    assert!(!views::EXPORTED.contains(&"v_맞춤뷰"));
    assert!(views::LEGACY_NAMES.contains(&"v_사용자뷰"));

    f.sql("CREATE VIEW v_사용자뷰 AS SELECT 1 AS 옛;");
    f.database().refresh_views().unwrap();
    assert!(
        rows(
            &f,
            "SELECT name FROM sqlite_master WHERE name = 'v_사용자뷰'"
        )
        .is_empty()
    );
}

/// `맞춤_뷰는_SQL_필드가_읽지_못한다` — authorizer 와 이름 검사가 막는다.
#[test]
fn sql_fields_cannot_read_user_views() {
    let f = fresh("uv-sql-refuse");
    seed(&f);
    let s = f.store();
    s.save_user_view(
        &view("문서", &[(None, "공고명", doc("notice", "공고명"))]),
        None,
    )
    .unwrap();
    assert!(!sql_column::READABLE.iter().any(|v| v.starts_with("u_")));

    let refused = s
        .validate_computed_sql("notice", "SELECT count(*) FROM u_문서")
        .unwrap();
    assert_eq!(refused["ok"], false);
    assert!(
        refused["error"].as_str().unwrap().contains("u_문서"),
        "{refused}"
    );
    let quoted = s
        .validate_computed_sql("notice", "SELECT count(*) FROM \"u_문서\"")
        .unwrap();
    assert_eq!(quoted["ok"], false);
    invalid(s.save_user_view(
        &view("둘", &[(None, "x", sql("SELECT count(*) FROM u_문서"))]),
        None,
    ));
    invalid(s.save_user_view(
        &view("셋", &[(None, "x", sql("SELECT count(*) FROM notice"))]),
        None,
    ));
}

// ── 판올림 ─────────────────────────────────────────────

/// 옛 파일을 픽스처로 감싼다 — 판올림 뒤 같은 자료 세우기 도구를 쓴다.
fn wrap(temp: Temp, path: std::path::PathBuf) -> Fixture {
    Fixture {
        temp,
        path,
        env: Box::leak(Box::new(TestEnv::new())),
    }
}

/// `판올림은_기준을_걷고_정의를_옮긴다` — 스키마 24 는 기준을 걷고 정의를 v2 로 옮긴다. 공고 기준의
/// `연결계약 |> 꺼내기 f |> 잇기 ", "` 그대로는 `계약.f` 가 되고 그 뷰는 모으기다. 그 밖의 여러 건 식은 옮기지 않아
/// 깨진 식으로 선다.
#[test]
fn migration_drops_base_and_moves_definition_to_v2() {
    let temp = Temp::new("uv-v23");
    let path = temp.path("v23.pclm");
    let column = |id: &str, name: &str, formula: Value| {
        json!({ "id": id, "name": name, "formula": formula }).to_string()
    };
    let notice = format!(
        "{{\"version\":1,\"columns\":[{}]}}",
        [
            column(
                "n1",
                "공고명",
                flow_of(json!({ "kind": "field", "field": "공고명" }), vec![])
            ),
            column(
                "n2",
                "품목 수",
                flow_of(json!({ "kind": "items" }), vec![json!({ "op": "count" })])
            ),
            column(
                "n3",
                "비면",
                flow_of(
                    json!({ "kind": "text", "text": "" }),
                    vec![json!({ "op": "ifEmpty", "field": "메모" })]
                )
            ),
            column(
                "n4",
                "계약번호",
                flow_of(
                    json!({ "kind": "linked", "link": "contract" }),
                    vec![
                        json!({ "op": "get", "field": "계약번호" }),
                        json!({ "op": "join", "text": ", " })
                    ]
                )
            ),
            column(
                "n5",
                "계약 수",
                flow_of(
                    json!({ "kind": "linked", "link": "contract" }),
                    vec![json!({ "op": "count" })]
                )
            ),
            column(
                "n6",
                "접수번호",
                flow_of(
                    json!({ "kind": "linked", "link": "request", "field": "접수번호" }),
                    vec![]
                )
            ),
        ]
        .join(",")
    );
    let contract = format!(
        "{{\"version\":1,\"columns\":[{}]}}",
        column(
            "k1",
            "번호",
            flow_of(json!({ "kind": "field", "field": "계약번호" }), vec![])
        )
    );
    legacy_at(&path, 23, "work");
    {
        let c = raw(&path);
        c.execute(
            "INSERT INTO user_view (id, name, base, definition, sort_order) VALUES ('v-n', '공고문서', 'notice', ?1, 10)",
            [&notice],
        )
        .unwrap();
        c.execute(
            "INSERT INTO user_view (id, name, base, definition, sort_order) VALUES ('v-k', '계약문서', 'contract', ?1, 20)",
            [&contract],
        )
        .unwrap();
    }
    let f = wrap(temp, path);
    f.database().migrate(f.env).unwrap();
    seed(&f);
    let s = f.store();

    let views = s.user_views().unwrap();
    let find = |name: &str| views.iter().find(|v| v.name == name).unwrap();
    let n = &find("공고문서").definition;
    assert_eq!(n["version"].as_i64(), Some(2));
    assert_eq!(n["contracts"], "fold");
    let flow = |i: usize| &n["columns"][i]["formula"]["flow"];
    assert_eq!(
        flow(0)["from"],
        json!({ "kind": "linked", "link": "notice", "field": "공고명" })
    );
    assert_eq!(
        flow(1)["from"],
        json!({ "kind": "linked", "link": "notice", "items": true })
    );
    assert_eq!(flow(2)["steps"][0]["field"], "공고.메모");
    assert_eq!(
        flow(3)["from"],
        json!({ "kind": "linked", "link": "contract", "field": "계약번호" })
    );
    assert_eq!(flow(3)["steps"].as_array().unwrap().len(), 0);
    assert_eq!(
        flow(4)["from"],
        json!({ "kind": "linked", "link": "contract" })
    );
    assert_eq!(
        flow(5)["from"],
        json!({ "kind": "linked", "link": "request", "field": "접수번호" })
    );
    assert_eq!(
        strs(n["columns"].as_array().unwrap(), "id"),
        ["n1", "n2", "n3", "n4", "n5", "n6"]
    );
    assert_eq!(
        find("공고문서")
            .broken
            .iter()
            .map(|(k, _)| k.as_str())
            .collect::<Vec<_>>(),
        ["n5"]
    );

    let k = find("계약문서");
    assert_eq!(k.definition["contracts"], "split");
    assert!(k.broken.is_empty(), "{:?}", k.broken);

    assert_eq!(
        read(
            &f,
            "SELECT 공고명 || '|' || \"품목 수\" || '|' || 비면 || '|' || 계약번호 || '|' || \"계약 수\" || '|' || 접수번호 FROM u_공고문서 WHERE 공고명 LIKE '%하나'"
        ),
        "시험 공고 하나|2|확인함|TESTK100, TESTK200||TESTR1-000"
    );
    assert_eq!(read(&f, "SELECT count(*) FROM u_계약문서"), "4");
    assert!(!rows(&f, "SELECT name FROM pragma_table_info('user_view')").contains(&"base".into()));
}

/// `판올림은_작업자료에만_예시_보기를_한_번_심는다` — 스키마 26 은 이미 있던 작업자료에 서식용 예시 보기를 한 번
/// 심는다. 같은 이름의 보기가 있으면 그것을 두고, 지운 뒤 다시 열어도 되살아나지 않는다. 작업자료가 아닌 파일에는
/// 심지 않는다.
#[test]
fn migration_seeds_example_view_once_and_only_into_work_files() {
    let temp = Temp::new("uv-v25");
    let env = TestEnv::new();
    let old = |name: &str, role: &str, own_view: Option<&str>| {
        let path = temp.path(name);
        legacy_at(&path, 25, role);
        if let Some(definition) = own_view {
            raw(&path)
                .execute(
                    "INSERT INTO user_view (id, name, definition, sort_order) VALUES ('v-own', '서식용 예시', ?1, 10)",
                    [definition],
                )
                .unwrap();
        }
        path
    };
    let migrated = |path: &std::path::Path| {
        let database = Database::new(path, Access::Write);
        database.migrate(&env).unwrap();
        Store::new(database, &env)
    };

    let plain = old("작업.pclm", "work", None);
    let s = migrated(&plain);
    let seeded = single(s.user_views().unwrap());
    assert_eq!(seeded.name, "서식용 예시");
    assert!(seeded.broken.is_empty(), "{:?}", seeded.broken);
    // v26 이 그 판의 예시를 심고 v27 이 손대지 않은 그것을 갈아 끼워, v25 에서 오면 지금 구성이다.
    assert_eq!(column_names(&seeded), builtin_field_names(None));

    s.delete_user_view(&seeded.id).unwrap();
    assert!(migrated(&plain).user_views().unwrap().is_empty());

    let mine = json!({ "version": 2, "contracts": "split", "columns": [
        { "id": "c-1", "name": "내 필드", "formula": doc("notice", "공고명") }
    ] })
    .to_string();
    let owned = old("내것.pclm", "work", Some(&mine));
    let kept = single(migrated(&owned).user_views().unwrap());
    assert_eq!(kept.id, "v-own");
    assert_eq!(column_names(&kept), ["내 필드"]);

    let submission = old("제출.pclm", "submission", None);
    assert!(migrated(&submission).user_views().unwrap().is_empty());
}

/// `판올림은_손대지_않은_예시_보기만_지금_구성으로_갈아_끼운다` — 스키마 27 은 v26 이 심은 예시 보기를 사람이 손대지
/// 않았으면(키 순서는 보지 않는다) 지금 구성으로 갈아 끼운다. 고친 보기와 지운 보기는 그대로이고, 작업자료가 아닌
/// 파일은 건드리지 않는다.
#[test]
fn migration_upgrades_only_untouched_example_view() {
    let temp = Temp::new("uv-v26");
    let env = TestEnv::new();
    // v26 의 파일 — 그 판이 심은 예시를 들었거나(정의를 고칠 수 있게 받는다), 지워서 없다.
    let old = |name: &str, role: &str, edit: Option<&dyn Fn(String) -> String>| {
        let path = temp.path(name);
        legacy_at(&path, 26, role);
        if let Some(edit) = edit {
            let mut c = raw(&path);
            let t = c.transaction().unwrap();
            // C# `Store.SeedExampleViews(old, transaction, ExampleViewPackageV26())` — 크레이트 밖에 내놓지 않은 것이라
            // 같은 일을 공개된 조각(read_view_package·stored_definition)으로 한다.
            for v in read_view_package(example_view_package_v26()).unwrap().views {
                t.execute(
                    "INSERT INTO user_view (id, name, definition, sort_order)
                     VALUES (?1, ?2, ?3, coalesce((SELECT max(sort_order) FROM user_view), 0) + 10)",
                    rusqlite::params![
                        format!("{:032x}", 0xe6u32),
                        v.name.clone().unwrap_or_default(),
                        stored_definition(&v).unwrap()
                    ],
                )
                .unwrap();
            }
            let definition: String = t
                .query_row("SELECT definition FROM user_view", [], |r| r.get(0))
                .unwrap();
            t.execute("UPDATE user_view SET definition = ?1", [edit(definition)])
                .unwrap();
            t.commit().unwrap();
        }
        path
    };

    let v26 = builtin_field_names(Some(example_view_package_v26()));
    assert_ne!(v26, builtin_field_names(None));
    let migrated = |path: &std::path::Path| -> Vec<String> {
        let database = Database::new(path, Access::Write);
        database.migrate(&env).unwrap();
        let view = single(Store::new(database, &env).user_views().unwrap());
        assert_eq!(view.name, "서식용 예시");
        assert!(view.broken.is_empty(), "{:?}", view.broken);
        column_names(&view)
    };

    // 손대지 않은 것 — 그대로든, 키 순서만 갈렸든 지금 구성이다.
    assert_eq!(
        migrated(&old("그대로.pclm", "work", Some(&|d| d))),
        builtin_field_names(None)
    );
    let reverse = |d: String| {
        let node: Map<String, Value> = serde_json::from_str(&d).unwrap();
        let reversed: Map<String, Value> = node.into_iter().rev().collect();
        Value::Object(reversed).to_string()
    };
    assert_eq!(
        migrated(&old("순서.pclm", "work", Some(&reverse))),
        builtin_field_names(None)
    );

    // 고친 것 — 필드 이름 하나만 바꿔도 사람 것이라 그대로 둔다.
    let edited = migrated(&old(
        "고침.pclm",
        "work",
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
    let deleted = old("지움.pclm", "work", None);
    let database = Database::new(&deleted, Access::Write);
    database.migrate(&env).unwrap();
    assert!(Store::new(database, &env).user_views().unwrap().is_empty());

    // 작업자료가 아닌 파일은 손대지 않은 예시도 그대로다.
    let submission = Database::new(old("제출.pclm", "submission", Some(&|d| d)), Access::Write);
    submission.migrate(&env).unwrap();
    assert_eq!(
        column_names(&single(Store::new(submission, &env).user_views().unwrap())),
        v26
    );
}

/// C# `뷰의_group_concat_은_모두_집계_안에서_차례를_정한다` — 뷰 SQL 의 `group_concat` 은 모두 인자 안에 `ORDER BY` 가 있다.
/// 하위 질의의 `ORDER BY` 는 바깥 집계에 차례를 약속하지 않는다. 기본 뷰·가공열·접은 맞춤 뷰가 모두 선 작업자료를 본다.
#[test]
fn every_group_concat_in_views_orders_inside_the_aggregate() {
    let f = fresh("uv-group-concat");
    seed(&f);
    let s = f.store();
    s.save_computed_column(
        "notice",
        "품명모음",
        &flow_of(
            json!({ "kind": "items" }),
            vec![
                json!({ "op": "get", "field": "세부품명" }),
                json!({ "op": "join", "text": ", " }),
            ],
        )
        .to_string(),
        None,
    )
    .unwrap();
    s.save_user_view(
        &view_with(
            "접은보기",
            "fold",
            &[
                (Some("c-1"), "번호", doc("notice", "입찰공고번호")),
                (Some("c-2"), "계약", doc("contract", "계약번호")),
            ],
        ),
        None,
    )
    .unwrap();

    let c = raw_read(&f.path);
    let mut statement = c
        .prepare("SELECT name, sql FROM sqlite_master WHERE type = 'view'")
        .unwrap();
    let views: Vec<(String, String)> = statement
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    let (found, unordered) = group_concats(&views);

    assert!(found >= 5, "group_concat {found}개");
    assert!(unordered.is_empty(), "{unordered:#?}");
}

/// 뷰마다 `group_concat(` 의 짝 괄호까지를 인자로 보고, 그 안에 `ORDER BY` 가 없는 것을 낸다(C# `GroupConcats`).
fn group_concats(views: &[(String, String)]) -> (usize, Vec<String>) {
    const OPEN: &str = "group_concat(";
    let mut found = 0;
    let mut unordered = Vec::new();
    for (name, sql) in views {
        let lower = sql.to_ascii_lowercase();
        let bytes = sql.as_bytes();
        let mut from = 0;
        while let Some(offset) = lower[from..].find(OPEN) {
            found += 1;
            let start = from + offset + OPEN.len();
            let mut depth = 1;
            let mut i = start;
            while i < bytes.len() && depth > 0 {
                match bytes[i] {
                    quote @ (b'\'' | b'"') => {
                        i += 1;
                        while i < bytes.len() && bytes[i] != quote {
                            i += 1;
                        }
                    }
                    b'(' => depth += 1,
                    b')' => depth -= 1,
                    _ => {}
                }
                i += 1;
            }
            let argument = &sql[start..i - 1];
            let words: Vec<String> = argument
                .split_whitespace()
                .map(str::to_ascii_uppercase)
                .collect();
            if !words.windows(2).any(|w| w[0] == "ORDER" && w[1] == "BY") {
                unordered.push(format!("{name}: group_concat({argument})"));
            }
            from = start;
        }
    }
    (found, unordered)
}

// ── 취합 ─────────────────────────────────────────────────

/// `취합은_이름으로_고르고_정의가_다르면_알린다` — 맞춤 뷰도 정의라 열쇠는 이름이다(docs/user-views.md §4). 같은 이름에 정의가
/// 다르면 먼저 온 것을 싣고 밀린 것을 알린다. 다른 이름은 둘 다 실리고, 취합본에도 그 뷰가 선다.
#[test]
fn merge_picks_views_by_name_and_reports_different_definitions() {
    let t = Temp::new("uv-merge");
    let env = TestEnv::new();
    let submission = |who: &str, views: &[String]| {
        let path = t.path(&format!("제출_{who}.pclm"));
        let database = pclm_file::create(&path, pclm_core::PclmRole::SUBMISSION, &env).unwrap();
        database
            .open()
            .unwrap()
            .execute(
                "INSERT INTO app_setting (key, value) VALUES ('submit.name', ?1)",
                [who],
            )
            .unwrap();
        let store = Store::new(database, &env);
        for v in views {
            store.save_user_view(v, None).unwrap();
        }
        path
    };
    let first = submission(
        "홍길동",
        &[
            view("문서", &[(Some("c-1"), "공고명", doc("notice", "공고명"))]),
            view(
                "홍길동뷰",
                &[(Some("c-1"), "번호", doc("contract", "계약번호"))],
            ),
        ],
    );
    let second = submission(
        "김철수",
        &[view_with(
            "문서",
            "fold",
            &[(Some("c-1"), "공고명", doc("notice", "공고명"))],
        )],
    );
    let output = t.path("취합.pclm");
    std::fs::create_dir_all(t.path("임시")).unwrap();

    let report =
        pclm_core::storage::merger::merge(&[&first, &second], &output, &t.path("임시"), &env)
            .unwrap();

    assert_eq!(report.conflicts.len(), 1);
    let conflict = &report.conflicts[0];
    assert_eq!(conflict.kind, "맞춤 뷰");
    assert_eq!(conflict.key, "문서");
    assert_eq!(conflict.kept.submitter, "홍길동");
    assert_eq!(conflict.pushed.len(), 1);
    assert_eq!(conflict.pushed[0].submitter, "김철수");

    let merged = Store::new(Database::new(&output, Access::Read), &env);
    let views = merged.user_views().unwrap();
    let mut names: Vec<&str> = views.iter().map(|v| v.name.as_str()).collect();
    names.sort();
    assert_eq!(names, ["문서", "홍길동뷰"]);
    let document = views.iter().find(|v| v.name == "문서").unwrap();
    assert_eq!(document.definition["contracts"], "split");
    assert_eq!(column_names(document), ["공고명"]);
    assert_eq!(
        read_at(
            &output,
            r"SELECT CAST(count(*) AS TEXT) FROM sqlite_master WHERE name LIKE 'u\_%' ESCAPE '\'"
        ),
        "2"
    );
}

//! 덮개(사람이 고친 값)와 번호 해석 시험 — C# `FieldOverrideTests` 전부와 `RequestResolveTests` 전부.
//!
//! C# 레코드는 `Fixture` 의 SQL 픽스처로 세운다. `decimal` 은 Microsoft.Data.Sqlite 가 묶는 꼴(`0.0###…`, 예:
//! `164872340.0`), `DateTime` 은 `yyyy-MM-dd HH:mm:ss` 다. 값은 모두 지어낸 것이다(CLAUDE.md).

mod common;

use common::store_fixture::Fixture;
use pclm_core::storage::store::{CellChange, EntityRef, Store};
use pclm_core::storage::views;
use pclm_core::{Access, Database, Error, Result};
use rusqlite::types::Value;

const CONTRACT_BASE: &str = "R26TA98000707";
const NOTICE_BASE: &str = "R26BK98000575";
const REQUEST_BASE: &str = "TSTPLA26500286";

fn contract_ref() -> EntityRef {
    EntityRef::new("contract", CONTRACT_BASE, "00")
}

fn notice_ref() -> EntityRef {
    EntityRef::new("notice", NOTICE_BASE, "000")
}

fn request_ref() -> EntityRef {
    EntityRef::new("request", REQUEST_BASE, "000")
}

/// C# `Contract(seq)` — 건명과 금액만 있는 계약.
fn contract(f: &Fixture, seq: &str) {
    f.contract(
        CONTRACT_BASE,
        seq,
        &[("title", "수질측정기 구매"), ("amount", "164872340.0")],
        &[],
    );
}

/// C# `Notice(seq)`.
fn notice(f: &Fixture, seq: &str, items: &[&[(&str, &str)]]) {
    f.notice(
        NOTICE_BASE,
        seq,
        &[("title", "수질측정기 구매")],
        items,
        &[],
    );
}

/// C# `Request(seq)`.
fn request(f: &Fixture, seq: &str) {
    f.request(
        REQUEST_BASE,
        seq,
        &[
            ("title", "윈치 8종 구매"),
            ("received_on", "2026-06-09 00:00:00"),
            ("goods_amount", "181725200.0"),
            ("budget_amount", "183408620.0"),
            ("demand_agency", "한별군수지원단"),
        ],
        &[&[
            ("line_no", "1"),
            ("request_number", REQUEST_BASE),
            ("item_name", "받침목"),
            ("quantity", "70"),
            ("unit_price", "310200.0"),
        ]],
    );
}

/// C# `Cell(view, column)` — 줄이 하나뿐인 시험이라 첫 줄을 본다. NULL 은 빈 글.
fn cell(f: &Fixture, view: &str, column: &str) -> String {
    let c = f.database().open_read_only().unwrap();
    let v: Value = c
        .query_row(&format!("SELECT \"{column}\" FROM \"{view}\";"), [], |r| {
            r.get(0)
        })
        .unwrap_or_else(|e| panic!("{view} 에 줄이 없습니다: {e}"));
    match v {
        Value::Null => String::new(),
        Value::Integer(i) => i.to_string(),
        Value::Real(r) => r.to_string(),
        Value::Text(t) => t,
        Value::Blob(_) => panic!("blob"),
    }
}

/// C# `ViewColumns(view)`.
fn view_columns(f: &Fixture, view: &str) -> Vec<String> {
    let c = f.database().open_read_only().unwrap();
    let mut s = c
        .prepare(&format!("PRAGMA table_info(\"{view}\");"))
        .unwrap();
    s.query_map([], |r| r.get(1))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

/// `Assert.Throws<InvalidOperationException>` — 글을 낸다.
fn invalid<T>(r: Result<T>) -> String {
    match r {
        Err(Error::InvalidOperation(m)) => m,
        Err(e) => panic!("InvalidOperation 이 아니다: {e:?}"),
        Ok(_) => panic!("오류가 나지 않았다"),
    }
}

fn change(row_key: &str, field: &str, value: &str, original: &str, operation: &str) -> CellChange {
    CellChange {
        row_key: Some(row_key.into()),
        field: Some(field.into()),
        value: Some(value.into()),
        original: Some(original.into()),
        operation: Some(operation.into()),
    }
}

// ── 일괄 편집 ────────────────────────────────────────────

/// `일괄편집은_검증과_저장이_한번이며_실패하면_모두_유지한다`
#[test]
fn batch_edit_validates_and_writes_once_and_keeps_all_on_failure() {
    let f = Fixture::new("override-batch");
    let store = f.store();
    contract(&f, "00");
    let key = contract_ref().display();
    let changes = [
        change(&key, "메모", "일괄", "", "set"),
        change(&key, "계약금액", "7", "164,872,340", "set"),
    ];
    let mut stale = changes[1].clone();
    stale.original = Some("낡은 값".into());
    invalid(store.batch_edit("v_계약", &[changes[0].clone(), stale]));
    assert_eq!("", cell(&f, "v_계약", "메모"));

    f.sql("CREATE TRIGGER reject_batch BEFORE INSERT ON field_override BEGIN SELECT RAISE(ABORT, 'test'); END;");
    match store.batch_edit("v_계약", &changes) {
        Err(Error::Sqlite(_)) => {}
        Err(e) => panic!("SqliteException 이 아니다: {e:?}"),
        Ok(n) => panic!("오류가 나지 않았다: {n}"),
    }
    assert_eq!("", cell(&f, "v_계약", "메모"));
    f.sql("DROP TRIGGER reject_batch;");

    assert_eq!(2, store.batch_edit("v_통합", &changes).unwrap());
    assert_eq!("일괄", cell(&f, "v_계약", "메모"));
    assert_eq!("7", cell(&f, "v_계약", "계약금액"));
    let overrides = store.overrides("contract").unwrap();
    assert_eq!(1, overrides.len());
    assert_eq!("164,872,340", overrides[0].original);

    let mut revert = changes[1].clone();
    revert.original = Some("7".into());
    revert.operation = Some("revert".into());
    store.batch_edit("v_계약", &[revert]).unwrap();
    assert_eq!("164,872,340", cell(&f, "v_계약", "계약금액"));
}

/// `일괄편집도_읽기전용과_주소와_열형식을_지킨다`
#[test]
fn batch_edit_guards_read_only_address_and_column_kind() {
    let f = Fixture::new("override-batch-guard");
    let store = f.store();
    contract(&f, "00");
    let c = change(&contract_ref().display(), "메모", "값", "", "set");
    let with_field = |field: &str| CellChange {
        field: Some(field.into()),
        ..c.clone()
    };

    let reader = Store::new(Database::new(&f.path, Access::Read), f.env);
    invalid(reader.batch_edit("v_계약", std::slice::from_ref(&c)));
    invalid(store.batch_edit("v_계약차수", std::slice::from_ref(&c)));
    invalid(store.batch_edit("v_공고", std::slice::from_ref(&c)));
    invalid(store.batch_edit("v_통합", &[with_field("공고명")]));
    invalid(store.batch_edit("v_계약", &[with_field("계약번호")]));
    invalid(store.batch_edit("v_계약", &[c.clone(), c.clone()]));
    store
        .add_user_column("contract", "날짜", "date", &[])
        .unwrap();
    invalid(store.batch_edit(
        "v_계약",
        &[CellChange {
            value: Some("2026-02-31".into()),
            ..with_field("날짜")
        }],
    ));
    contract(&f, "01");
    invalid(store.batch_edit("v_계약", std::slice::from_ref(&c)));
    assert_eq!("", cell(&f, "v_계약", "메모"));
}

// ── 덮개 ─────────────────────────────────────────────────

/// `고친_값이_계약면에_뜬다`
#[test]
fn override_shows_on_contract_face() {
    let f = Fixture::new("override-face");
    let store = f.store();
    contract(&f, "00");
    assert_eq!("164,872,340", cell(&f, "v_계약", "계약금액"));

    store
        .set_override(&contract_ref(), "계약금액", "164,872,341")
        .unwrap();

    assert_eq!("164,872,341", cell(&f, "v_계약", "계약금액"));
}

/// `통합_뷰에도_같은_값이_뜬다` — 통합은 계약 뷰를 통째로 받으므로 저절로 따라와야 한다.
#[test]
fn override_shows_on_unified_view() {
    let f = Fixture::new("override-unified");
    let store = f.store();
    contract(&f, "00");
    store
        .set_override(&contract_ref(), "계약건명", "수질측정기 구매(정정)")
        .unwrap();

    assert_eq!("수질측정기 구매(정정)", cell(&f, "v_통합", "계약건명"));
}

/// `되돌리면_파서가_읽은_값이_돌아온다`
#[test]
fn clearing_restores_parsed_value() {
    let f = Fixture::new("override-clear");
    let store = f.store();
    contract(&f, "00");
    store
        .set_override(&contract_ref(), "계약금액", "0")
        .unwrap();

    store.clear_override(&contract_ref(), "계약금액").unwrap();

    assert_eq!("164,872,340", cell(&f, "v_계약", "계약금액"));
    assert!(store.overrides("contract").unwrap().is_empty());
}

/// `빈_값도_유효한_덮개다` — 칸을 비우는 것과 파서 값으로 돌아가는 것은 다른 일이다.
#[test]
fn empty_value_is_a_valid_override() {
    let f = Fixture::new("override-empty");
    let store = f.store();
    contract(&f, "00");
    store.set_override(&contract_ref(), "계약금액", "").unwrap();

    assert_eq!("", cell(&f, "v_계약", "계약금액"));
    assert_eq!(1, store.overrides("contract").unwrap().len());
}

/// `원래_값은_처음_고칠_때만_담긴다`
#[test]
fn original_is_captured_only_on_first_edit() {
    let f = Fixture::new("override-original");
    let store = f.store();
    contract(&f, "00");

    store
        .set_override(&contract_ref(), "계약금액", "1")
        .unwrap();
    store
        .set_override(&contract_ref(), "계약금액", "2")
        .unwrap();

    let overrides = store.overrides("contract").unwrap();
    assert_eq!(1, overrides.len());
    assert_eq!("164,872,340", overrides[0].original);
}

/// `한_차수의_정정이_다음_차수로_새지_않는다` — 이 표의 존재 이유. 정정은 차수를 따라가지 않는다.
#[test]
fn override_does_not_leak_into_next_revision() {
    let f = Fixture::new("override-leak");
    let store = f.store();
    contract(&f, "00");
    store
        .set_override(&contract_ref(), "계약금액", "999,999,999")
        .unwrap();

    contract(&f, "01");

    assert_eq!("01", cell(&f, "v_계약", "차수"));
    assert_eq!("164,872,340", cell(&f, "v_계약", "계약금액"));
}

/// `새_차수를_지워도_옛_차수의_정정은_살아_있다`
#[test]
fn deleting_new_revision_keeps_old_revision_override() {
    let f = Fixture::new("override-old-rev");
    let store = f.store();
    contract(&f, "00");
    store
        .set_override(&contract_ref(), "계약금액", "999,999,999")
        .unwrap();
    contract(&f, "01");

    store
        .delete(&EntityRef::new("contract", CONTRACT_BASE, "01"), false)
        .unwrap();

    assert_eq!("00", cell(&f, "v_계약", "차수"));
    assert_eq!("999,999,999", cell(&f, "v_계약", "계약금액"));
}

/// `품목_집계로_나오는_열도_고칠_수_있다` — 뷰가 표기를 마친 값 바깥에 씌우므로 집계 열도 열린다.
#[test]
fn item_aggregate_column_is_correctable() {
    let f = Fixture::new("override-aggregate");
    let store = f.store();
    notice(
        &f,
        "000",
        &[
            &[("line_no", "1"), ("item_name", "수질측정기")],
            &[("line_no", "2"), ("item_name", "시약")],
        ],
    );

    assert_eq!("", cell(&f, "v_공고", "세부품명"));

    store
        .set_override(&notice_ref(), "세부품명", "수질측정기 외 1종")
        .unwrap();

    assert_eq!("수질측정기 외 1종", cell(&f, "v_공고", "세부품명"));
}

/// `공고의_모든_열이_감싸여_있다`
#[test]
fn every_notice_column_is_wrapped() {
    let f = Fixture::new("override-notice-all");
    let store = f.store();
    notice(&f, "000", &[]);

    let user: Vec<String> = store
        .user_columns(Some("notice"))
        .unwrap()
        .into_iter()
        .map(|c| c.field_name)
        .collect();

    for column in view_columns(&f, "v_공고") {
        if views::KEY_COLUMNS.contains(&column.as_str()) || user.contains(&column) {
            continue;
        }
        store.set_override(&notice_ref(), &column, "고침").unwrap();
        assert_eq!("고침", cell(&f, "v_공고", &column), "{column}");
    }
}

/// `건과_현행공고는_키라_고칠_수_없다` — 파생값이라 덮개를 씌우면 글자만 바뀌고 묶임은 그대로다.
#[test]
fn group_and_current_notice_are_keys() {
    let f = Fixture::new("override-group-key");
    let store = f.store();
    notice(&f, "000", &[]);

    for column in ["공고건", "현행공고"] {
        assert!(views::KEY_COLUMNS.contains(&column));
        let message = invalid(store.set_override(&notice_ref(), column, "R26BK00000000"));
        assert!(message.contains(column), "{message}");
    }
}

/// `계약방법과_계약구분은_ID를_가공하지_않고_투영한다`
#[test]
fn contract_method_and_kind_project_ids_unchanged() {
    let f = Fixture::new("override-method-kind");
    f.notice(
        NOTICE_BASE,
        "000",
        &[("contract_method", "방법001"), ("contract_kind", "구분002")],
        &[],
        &[],
    );
    for view in ["v_공고", "v_공고차수"] {
        assert_eq!("방법001", cell(&f, view, "계약방법"));
        assert_eq!("구분002", cell(&f, view, "계약구분"));
    }
}

// ── 막는 것들 ────────────────────────────────────────────

/// `레코드를_가리키는_열은_고칠_수_없다`
#[test]
fn key_column_is_not_correctable() {
    let f = Fixture::new("override-key");
    let store = f.store();
    contract(&f, "00");

    let message = invalid(store.set_override(&contract_ref(), "계약번호", "R26TA9800070799"));

    assert!(message.contains("계약번호"), "{message}");
}

/// `손으로_채우는_열은_덮개로_받지_않는다`
#[test]
fn user_column_is_not_an_override() {
    let f = Fixture::new("override-user-column");
    let store = f.store();
    contract(&f, "00");

    let message = invalid(store.set_override(&contract_ref(), "진행상태", "납품"));

    assert!(message.contains("손으로 채우는 열"), "{message}");
}

/// `사람_값을_비우면_줄이_지워지고_칸은_빈_문자열이다` — `None` 도 `""` 도 같다. 지우는 문이 언제나 던지면 한 번 적은
/// 값을 화면에서 지울 길이 없다.
#[test]
fn clearing_user_field_deletes_row_and_shows_empty_string() {
    let f = Fixture::new("override-user-clear");
    let store = f.store();
    contract(&f, "00");
    let rows = "SELECT COUNT(*) FROM contract_user_field;";
    store
        .set_user_field(&contract_ref(), "메모", Some("지울 값"))
        .unwrap();
    assert_eq!(Some("1".into()), f.scalar(rows));

    store.set_user_field(&contract_ref(), "메모", None).unwrap();
    assert_eq!(Some(String::new()), f.scalar("SELECT 메모 FROM v_계약;"));
    assert_eq!(Some("0".into()), f.scalar(rows));

    store
        .set_user_field(&contract_ref(), "메모", Some("다시"))
        .unwrap();
    store
        .set_user_field(&contract_ref(), "메모", Some(""))
        .unwrap();
    assert_eq!(Some(String::new()), f.scalar("SELECT 메모 FROM v_계약;"));
    assert_eq!(Some("0".into()), f.scalar(rows));

    // 없는 값을 비워도 오류가 아니다.
    store.set_user_field(&contract_ref(), "메모", None).unwrap();
    store
        .set_user_field(&contract_ref(), "진행상태", Some(""))
        .unwrap();
    assert_eq!(Some("0".into()), f.scalar(rows));
}

/// `일괄편집은_사람_값을_넣고_비우기를_함께_한다` — 비우기가 던지면 함께 넣은 값까지 되돌아간다.
#[test]
fn batch_edit_sets_and_clears_user_fields_together() {
    let f = Fixture::new("override-batch-user-clear");
    let store = f.store();
    contract(&f, "00");
    store
        .set_user_field(&contract_ref(), "메모", Some("지울 값"))
        .unwrap();
    let key = contract_ref().display();

    let applied = store
        .batch_edit(
            "v_계약",
            &[
                change(&key, "진행상태", "납품", "", "set"),
                change(&key, "메모", "", "지울 값", "set"),
            ],
        )
        .unwrap();

    assert_eq!(2, applied);
    assert_eq!(
        Some("납품".into()),
        f.scalar("SELECT 진행상태 FROM v_계약;")
    );
    assert_eq!(Some(String::new()), f.scalar("SELECT 메모 FROM v_계약;"));
    assert_eq!(
        Some("1".into()),
        f.scalar("SELECT COUNT(*) FROM contract_user_field;")
    );
}

/// `계약면에_없는_열은_거절한다` — 열 이름이 뒤이어 SQL 에 박힌다.
#[test]
fn unknown_column_is_rejected() {
    let f = Fixture::new("override-unknown");
    let store = f.store();
    contract(&f, "00");

    invalid(store.set_override(&contract_ref(), "amount\"; DROP TABLE contract; --", "0"));
}

/// `다른_종류의_열은_거절한다`
#[test]
fn other_entity_column_is_rejected() {
    let f = Fixture::new("override-other-kind");
    let store = f.store();
    contract(&f, "00");

    invalid(store.set_override(&contract_ref(), "공고종류", "일반"));
}

// ── 접수 ─────────────────────────────────────────────────

/// `접수의_고친_값이_계약면에_뜬다`
#[test]
fn request_override_shows_on_face() {
    let f = Fixture::new("override-request");
    let store = f.store();
    request(&f, "000");
    assert_eq!("181,725,200", cell(&f, "v_접수", "품대"));

    store
        .set_override(&request_ref(), "품대", "181,725,201")
        .unwrap();

    assert_eq!("181,725,201", cell(&f, "v_접수", "품대"));
}

/// `접수의_모든_열이_감싸여_있다`
#[test]
fn every_request_column_is_wrapped() {
    let f = Fixture::new("override-request-all");
    let store = f.store();
    request(&f, "000");

    let user: Vec<String> = store
        .user_columns(Some("request"))
        .unwrap()
        .into_iter()
        .map(|c| c.field_name)
        .collect();

    for column in view_columns(&f, "v_접수") {
        if views::KEY_COLUMNS.contains(&column.as_str()) || user.contains(&column) {
            continue;
        }
        store.set_override(&request_ref(), &column, "고침").unwrap();
        assert_eq!("고침", cell(&f, "v_접수", &column), "{column}");
    }
}

/// `접수의_덮개는_차수를_따라가지_않는다`
#[test]
fn request_override_does_not_follow_revision() {
    let f = Fixture::new("override-request-rev");
    let store = f.store();
    request(&f, "000");
    store
        .set_override(&request_ref(), "품대", "181,725,201")
        .unwrap();

    request(&f, "001");

    assert_eq!("181,725,200", cell(&f, "v_접수", "품대"));
}

// ── 번호 해석(C# RequestResolveTests) ────────────────────

const RESOLVE_BASE: &str = "R26DC00000001";

/// C# `RequestResolveTests` 생성자 — 품목마다 다른 조달요구번호를 단 접수 하나.
fn resolve_fixture(name: &str) -> Fixture {
    let f = Fixture::new(name);
    f.request(
        RESOLVE_BASE,
        "000",
        &[("title", "26년 한별 윈치 8종 구매")],
        &[
            &[("line_no", "1"), ("request_number", "TSTPLA26500286")],
            &[("line_no", "2"), ("request_number", "TSTPLA26500290")],
            &[("line_no", "3"), ("request_number", "TSTPLA26500295")],
        ],
    );
    f
}

/// `접수번호로_찾는다`
#[test]
fn resolve_by_request_number() {
    let f = resolve_fixture("resolve-request");
    let found = f.store().resolve("R26DC00000001-000").unwrap().unwrap();

    assert_eq!("request", found.entity_type);
    assert_eq!(RESOLVE_BASE, found.base);
    assert_eq!("000", found.seq);
}

/// `아무_요청번호로도_찾는다` — `[Theory]` 세 줄.
#[test]
fn resolve_by_any_procurement_number() {
    let f = resolve_fixture("resolve-any");
    for key in ["TSTPLA26500286", "TSTPLA26500290", "TSTPLA26500295"] {
        let found = f.store().resolve(key).unwrap().unwrap();

        assert_eq!("request", found.entity_type, "{key}");
        assert_eq!("R26DC00000001-000", found.display(), "{key}");
    }
}

/// `요청번호는_최신_차수를_준다`
#[test]
fn procurement_number_resolves_latest_revision() {
    let f = resolve_fixture("resolve-latest");
    f.request(
        RESOLVE_BASE,
        "001",
        &[],
        &[&[("line_no", "1"), ("request_number", "TSTPLA26500290")]],
    );

    assert_eq!(
        "R26DC00000001-001",
        f.store()
            .resolve("TSTPLA26500290")
            .unwrap()
            .unwrap()
            .display()
    );
}

/// `모르는_번호는_찾지_못한다`
#[test]
fn unknown_number_is_not_found() {
    let f = resolve_fixture("resolve-unknown");
    assert!(f.store().resolve("TSTPLA26500699").unwrap().is_none());
}

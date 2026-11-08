//! 지우기 시험 — C# `DeletionTests` 전부.
//!
//! C# 레코드는 `Fixture` 의 SQL 픽스처로 세우고, 계약상대자·첨부는 C# `UpsertContract` 가 넣는 문 그대로 따로 심는다.
//! 명시 참조는 C# `ReferenceFixture` 를 옮긴 [`reference_contract`]·[`reference_request`] 로 넣는다. 값은 모두
//! 지어낸 것이다(CLAUDE.md).

mod common;

use common::store_fixture::{Fixture, NOW};
use pclm_core::erp::explicit_links;
use pclm_core::storage::store::{EntityRef, Store};
use pclm_core::{Access, Database, Error, Result};
use rusqlite::params;
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

/// C# `Contract(seq)` — 건명과 금액만 있는 계약. `extra` 는 `with { … }` 로 더한 열, `items` 는 품목.
fn contract_with(f: &Fixture, seq: &str, extra: &[(&str, &str)], items: &[&[(&str, &str)]]) {
    let mut fields = vec![("title", "수질측정기 구매"), ("amount", "164872340.0")];
    fields.extend_from_slice(extra);
    f.contract(CONTRACT_BASE, seq, &fields, items);
}

fn contract(f: &Fixture, seq: &str) {
    contract_with(f, seq, &[], &[]);
}

/// C# `Notice("000")`.
fn notice(f: &Fixture) {
    f.notice(
        NOTICE_BASE,
        "000",
        &[("title", "수질측정기 구매")],
        &[],
        &[],
    );
}

/// C# `NoticeWithItem()` — 접수와 (수량, 단가)가 맞는 공고. 그래야 품목 짝이 선다.
fn notice_with_item(f: &Fixture) {
    f.notice(
        NOTICE_BASE,
        "000",
        &[("title", "수질측정기 구매")],
        &[&[
            ("line_no", "1"),
            ("item_name", "받침목"),
            ("quantity", "70"),
            ("unit_price", "310200.0"),
        ]],
        &[],
    );
}

/// C# `Request(seq)`.
fn request(f: &Fixture, seq: &str) {
    f.request(
        REQUEST_BASE,
        seq,
        &[("title", "윈치 8종 구매"), ("goods_amount", "181725200.0")],
        &[&[
            ("line_no", "1"),
            ("request_number", REQUEST_BASE),
            ("item_name", "받침목"),
            ("quantity", "70"),
            ("unit_price", "310200.0"),
        ]],
    );
}

/// C# `ReferenceFixture.Contract` — 계약이 공고를 명시로 가리키게 하고 연결을 다시 짓는다.
fn reference_contract(f: &Fixture, contract: &EntityRef, notice: &EntityRef) {
    let mut c = f.conn();
    let tx = c.transaction().unwrap();
    tx.execute(
        "UPDATE contract SET ref_notice_base=?1 WHERE contract_base=?2",
        params![notice.base, contract.base],
    )
    .unwrap();
    explicit_links::resolve(&tx).unwrap();
    tx.commit().unwrap();
}

/// C# `ReferenceFixture.Request` — 접수 품목과 공고 품목이 서로를 명시로 가리키게 한다.
fn reference_request(f: &Fixture, request: &EntityRef, notice: &EntityRef) {
    let mut c = f.conn();
    let tx = c.transaction().unwrap();
    tx.execute(
        "UPDATE request_item SET ref_request_base=request_base, ref_request_seq=seq,
             ref_request_item=CAST(line_no AS TEXT) WHERE request_base=?1;",
        params![request.base],
    )
    .unwrap();
    tx.execute(
        "UPDATE notice_item SET ref_request_base=?1, ref_request_seq=?2,
             ref_request_item=CAST(line_no AS TEXT) WHERE notice_base=?3;",
        params![request.base, request.seq, notice.base],
    )
    .unwrap();
    tx.execute(
        "INSERT INTO notice_item(notice_base,seq,line_no,ref_request_base)
             SELECT notice_base,seq,1,?1 FROM notice n WHERE notice_base=?2
             AND NOT EXISTS(SELECT 1 FROM notice_item i WHERE i.notice_base=n.notice_base AND i.seq=n.seq);",
        params![request.base, notice.base],
    )
    .unwrap();
    explicit_links::resolve(&tx).unwrap();
    tx.commit().unwrap();
}

/// C# `Count(sql)`.
fn count(f: &Fixture, sql: &str) -> i64 {
    let c = f.database().open_read_only().unwrap();
    c.query_row(sql, [], |r| r.get(0)).unwrap()
}

/// C# `Text(sql)` — NULL 은 빈 글.
fn text(f: &Fixture, sql: &str) -> String {
    let c = f.database().open_read_only().unwrap();
    let v: Value = c
        .query_row(sql, [], |r| r.get(0))
        .unwrap_or_else(|e| panic!("행이 없습니다: {sql} ({e})"));
    match v {
        Value::Null => String::new(),
        Value::Integer(i) => i.to_string(),
        Value::Real(r) => r.to_string(),
        Value::Text(t) => t,
        Value::Blob(_) => panic!("blob"),
    }
}

/// `Assert.Throws<InvalidOperationException>` — 글을 낸다.
fn invalid<T>(r: Result<T>) -> String {
    match r {
        Err(Error::InvalidOperation(m)) => m,
        Err(e) => panic!("InvalidOperation 이 아니다: {e:?}"),
        Ok(_) => panic!("오류가 나지 않았다"),
    }
}

// ── 일괄 삭제 ────────────────────────────────────────────

/// `여러_차수의_삭제계획은_겹침없이_실제_계열과_링크_영향을_센다`
#[test]
fn batch_plan_counts_series_and_links_without_overlap() {
    let f = Fixture::new("delete-batch-plan");
    let store = f.store();
    notice(&f);
    contract_with(&f, "00", &[], &[&[("line_no", "1"), ("item_name", "품목")]]);
    store
        .set_override(&contract_ref(), "계약금액", "1")
        .unwrap();
    contract(&f, "01");
    store
        .set_user_field(&contract_ref(), "메모", Some("보존 확인"))
        .unwrap();
    reference_contract(&f, &contract_ref(), &notice_ref());
    let list = vec![
        contract_ref().display(),
        EntityRef::new("contract", CONTRACT_BASE, "01").display(),
        contract_ref().display(),
    ];
    let plan = store.plan_batch_deletion(&list, false).unwrap();
    assert_eq!(2, plan.document_count);
    assert_eq!(1, plan.series_count);
    assert_eq!(1, plan.child_rows);
    assert_eq!(1, plan.user_fields);
    assert_eq!(1, plan.overrides);
    assert_eq!(1, plan.links);
    assert!(plan.targets.iter().all(|p| p.series_goes));
    assert_eq!(2, count(&f, "SELECT COUNT(*) FROM contract;"));
    assert_eq!(1, count(&f, "SELECT COUNT(*) FROM project_link;"));

    store
        .set_user_field(&contract_ref(), "메모", Some("동시 변경"))
        .unwrap();
    invalid(store.batch_delete(&list, false, &plan.fingerprint));
    assert_eq!(2, count(&f, "SELECT COUNT(*) FROM contract;"));

    let plan = store.plan_batch_deletion(&list, false).unwrap();
    let reversed: Vec<String> = list.iter().rev().cloned().collect();
    store
        .batch_delete(&reversed, false, &plan.fingerprint)
        .unwrap();
    assert_eq!(0, count(&f, "SELECT COUNT(*) FROM contract_series;"));
    assert_eq!(0, count(&f, "SELECT COUNT(*) FROM project_link;"));
    assert_eq!(1, count(&f, "SELECT COUNT(*) FROM notice;"));
}

/// `일괄삭제는_중간_오류도_되감고_전차수_대상을_중복제거한다`
#[test]
fn batch_delete_rolls_back_midway_error_and_dedupes_whole_series() {
    let f = Fixture::new("delete-batch-rollback");
    let store = f.store();
    contract(&f, "00");
    contract(&f, "01");
    notice(&f);
    let list = vec![
        contract_ref().display(),
        EntityRef::new("contract", CONTRACT_BASE, "01").display(),
        notice_ref().display(),
    ];
    let plan = store.plan_batch_deletion(&list, true).unwrap();
    assert_eq!(2, plan.targets.len());
    assert_eq!(3, plan.document_count);

    f.sql("CREATE TRIGGER reject_delete BEFORE DELETE ON notice BEGIN SELECT RAISE(ABORT, 'test'); END;");
    match store.batch_delete(&list, true, &plan.fingerprint) {
        Err(Error::Sqlite(_)) => {}
        Err(e) => panic!("SqliteException 이 아니다: {e:?}"),
        Ok(_) => panic!("오류가 나지 않았다"),
    }
    assert_eq!(2, count(&f, "SELECT COUNT(*) FROM contract;"));
    assert_eq!(1, count(&f, "SELECT COUNT(*) FROM notice;"));

    let reader = Store::new(Database::new(&f.path, Access::Read), f.env);
    invalid(reader.plan_batch_deletion(&list, true));
}

// ── 지우기 ───────────────────────────────────────────────

/// `지우면_계약면에서_사라진다`
#[test]
fn deleted_contract_leaves_face() {
    let f = Fixture::new("delete-face");
    let store = f.store();
    contract(&f, "00");
    assert_eq!(1, count(&f, "SELECT COUNT(*) FROM v_계약;"));

    store.delete(&contract_ref(), true).unwrap();

    assert_eq!(0, count(&f, "SELECT COUNT(*) FROM v_계약;"));
}

/// `딸린_줄도_함께_간다` — 남으면 다음 투입에서 옛 줄이 되살아난다.
#[test]
fn child_rows_go_too() {
    let f = Fixture::new("delete-children");
    let store = f.store();
    contract_with(
        &f,
        "00",
        &[],
        &[&[("line_no", "1"), ("item_name", "수질측정기")]],
    );
    f.conn()
        .execute(
            "INSERT INTO contract_attachment (contract_base, seq, line_no, document_type, file_name)
             VALUES (?1, '00', 1, NULL, '규격서.hwp');",
            [CONTRACT_BASE],
        )
        .unwrap();

    store.delete(&contract_ref(), true).unwrap();

    assert_eq!(0, count(&f, "SELECT COUNT(*) FROM contract_item;"));
    assert_eq!(0, count(&f, "SELECT COUNT(*) FROM contract_attachment;"));
}

/// `계열을_지우면_사람이_적은_값과_링크도_간다` — 남기면 가리킬 것 없이 떠돈다.
#[test]
fn deleting_series_takes_user_fields_and_links() {
    let f = Fixture::new("delete-series");
    let store = f.store();
    notice(&f);
    contract(&f, "00");
    store
        .set_user_field(&contract_ref(), "진행상태", Some("납품"))
        .unwrap();
    reference_contract(&f, &contract_ref(), &notice_ref());

    store.delete(&contract_ref(), true).unwrap();

    assert_eq!(0, count(&f, "SELECT COUNT(*) FROM contract_user_field;"));
    assert_eq!(0, count(&f, "SELECT COUNT(*) FROM project_link;"));
    assert_eq!(0, count(&f, "SELECT COUNT(*) FROM contract_series;"));
    assert_eq!(1, count(&f, "SELECT COUNT(*) FROM v_공고;"));
}

/// `손으로_고친_것도_함께_간다` — 덮개는 외래키가 없어 손으로 지우지 않으면 조용히 남는다.
#[test]
fn overrides_go_too() {
    let f = Fixture::new("delete-overrides");
    let store = f.store();
    contract(&f, "00");
    store
        .set_override(&contract_ref(), "계약금액", "164,872,341")
        .unwrap();

    store.delete(&contract_ref(), true).unwrap();

    assert!(store.overrides("contract").unwrap().is_empty());
}

/// `남은_계약이_없는_상대자는_함께_간다`
#[test]
fn orphaned_counterparty_goes() {
    let f = Fixture::new("delete-counterparty");
    let store = f.store();
    f.conn()
        .execute(
            "INSERT INTO counterparty (business_number, name, representative, address, phone, fax, updated_at)
             VALUES ('123-45-67890', '가나다상사', NULL, NULL, NULL, NULL, ?1);",
            [NOW],
        )
        .unwrap();
    contract_with(&f, "00", &[("counterparty_number", "123-45-67890")], &[]);

    assert_eq!(1, count(&f, "SELECT COUNT(*) FROM counterparty;"));

    store.delete(&contract_ref(), true).unwrap();

    assert_eq!(0, count(&f, "SELECT COUNT(*) FROM counterparty;"));
}

// ── 차수 하나만 지우기 ───────────────────────────────────

/// `차수_하나를_지우면_이전_차수가_다시_최신이_된다`
#[test]
fn deleting_one_revision_restores_previous_as_latest() {
    let f = Fixture::new("delete-one-rev");
    let store = f.store();
    contract(&f, "00");
    contract(&f, "01");

    store
        .delete(&EntityRef::new("contract", CONTRACT_BASE, "01"), false)
        .unwrap();

    assert_eq!(1, count(&f, "SELECT COUNT(*) FROM contract;"));
    assert_eq!("00", text(&f, "SELECT 차수 FROM v_계약;"));
}

/// `마지막_차수를_지우면_계열도_간다` — 계열만 남기면 사람 값이 어느 레코드에도 붙지 않은 채 남는다.
#[test]
fn deleting_last_revision_takes_series() {
    let f = Fixture::new("delete-last-rev");
    let store = f.store();
    contract(&f, "00");
    store
        .set_user_field(&contract_ref(), "진행상태", Some("납품"))
        .unwrap();

    store.delete(&contract_ref(), false).unwrap();

    assert_eq!(0, count(&f, "SELECT COUNT(*) FROM contract_series;"));
    assert_eq!(0, count(&f, "SELECT COUNT(*) FROM contract_user_field;"));
}

/// `계열이_남으면_사람이_적은_값도_남는다`
#[test]
fn surviving_series_keeps_user_fields() {
    let f = Fixture::new("delete-keep-user");
    let store = f.store();
    contract(&f, "00");
    contract(&f, "01");
    store
        .set_user_field(&contract_ref(), "진행상태", Some("납품"))
        .unwrap();

    store
        .delete(&EntityRef::new("contract", CONTRACT_BASE, "01"), false)
        .unwrap();

    assert_eq!("납품", text(&f, "SELECT 진행상태 FROM v_계약;"));
}

// ── 묻기 전에 세어 보이기 ────────────────────────────────

/// `무엇이_사라지는지_먼저_센다`
#[test]
fn plan_counts_what_goes_first() {
    let f = Fixture::new("delete-plan");
    let store = f.store();
    notice(&f);
    contract_with(
        &f,
        "00",
        &[],
        &[&[("line_no", "1"), ("item_name", "수질측정기")]],
    );
    store
        .set_user_field(&contract_ref(), "진행상태", Some("납품"))
        .unwrap();
    store
        .set_override(&contract_ref(), "계약금액", "164,872,341")
        .unwrap();
    reference_contract(&f, &contract_ref(), &notice_ref());

    let plan = store.plan_deletion(&contract_ref(), true).unwrap();

    assert_eq!("R26TA9800070700", plan.display);
    assert_eq!("수질측정기 구매", plan.title);
    assert_eq!(vec!["00"], plan.revisions);
    assert!(plan.series_goes);
    assert_eq!(1, plan.child_rows);
    assert_eq!(1, plan.user_fields);
    assert_eq!(1, plan.overrides);
    assert!(plan.linked);
}

/// `차수_하나만_지울_때는_계열의_것을_세지_않는다`
#[test]
fn single_revision_plan_does_not_count_series_things() {
    let f = Fixture::new("delete-plan-rev");
    let store = f.store();
    contract(&f, "00");
    contract(&f, "01");
    store
        .set_user_field(&contract_ref(), "진행상태", Some("납품"))
        .unwrap();
    store
        .set_override(&contract_ref(), "계약금액", "111")
        .unwrap();

    let plan = store
        .plan_deletion(&EntityRef::new("contract", CONTRACT_BASE, "01"), false)
        .unwrap();

    assert!(!plan.series_goes);
    assert_eq!(vec!["00", "01"], plan.revisions);
    assert_eq!(0, plan.user_fields);
    assert_eq!(0, plan.overrides);
}

/// `없는_레코드는_셀_수_없다`
#[test]
fn missing_record_cannot_be_planned() {
    let f = Fixture::new("delete-plan-missing");
    let store = f.store();

    let message = invalid(store.plan_deletion(&contract_ref(), true));

    assert!(message.contains("R26TA9800070700"), "{message}");
}

/// `공고를_지우면_계약은_남고_링크만_풀린다`
#[test]
fn deleting_notice_keeps_contract_and_unlinks() {
    let f = Fixture::new("delete-notice");
    let store = f.store();
    notice(&f);
    contract(&f, "00");
    reference_contract(&f, &contract_ref(), &notice_ref());

    store.delete(&notice_ref(), true).unwrap();

    assert_eq!(1, count(&f, "SELECT COUNT(*) FROM v_계약;"));
    assert_eq!(0, count(&f, "SELECT COUNT(*) FROM project_link;"));
    assert_eq!("", text(&f, "SELECT 공고명 FROM v_통합;"));
}

// ── 접수 ─────────────────────────────────────────────────

/// `접수를_지우면_계약면에서_사라진다`
#[test]
fn deleted_request_leaves_face() {
    let f = Fixture::new("delete-request");
    let store = f.store();
    request(&f, "000");
    assert_eq!(1, count(&f, "SELECT COUNT(*) FROM v_접수;"));

    store.delete(&request_ref(), true).unwrap();

    assert_eq!(0, count(&f, "SELECT COUNT(*) FROM v_접수;"));
    assert_eq!(0, count(&f, "SELECT COUNT(*) FROM request_item;"));
}

/// `접수의_지울_것을_세어_보인다` — 접수의 `Linked` 는 `request_link` 를 봐야 한다(`project_link` 에는
/// `request_base` 가 없다).
#[test]
fn request_plan_counts_what_goes() {
    let f = Fixture::new("delete-request-plan");
    let store = f.store();
    request(&f, "000");
    notice(&f);

    reference_request(&f, &request_ref(), &notice_ref());
    store
        .add_user_column("request", "비고", "text", &[])
        .unwrap();
    store
        .set_user_field(&request_ref(), "비고", Some("협의중"))
        .unwrap();

    let plan = store.plan_deletion(&request_ref(), true).unwrap();

    assert_eq!("request", plan.entity_type);
    assert_eq!("TSTPLA26500286-000", plan.display);
    assert_eq!(vec!["000"], plan.revisions);
    assert!(plan.series_goes);
    assert_eq!(1, plan.child_rows);
    assert_eq!(1, plan.user_fields);
    assert!(plan.linked);
}

/// `접수를_지우면_링크와_품목_짝도_함께_간다`
#[test]
fn deleting_request_takes_links_and_item_pairs() {
    let f = Fixture::new("delete-request-links");
    let store = f.store();
    request(&f, "000");
    notice_with_item(&f);
    reference_request(&f, &request_ref(), &notice_ref());

    assert_eq!(1, count(&f, "SELECT COUNT(*) FROM request_link;"));
    assert_eq!(1, count(&f, "SELECT COUNT(*) FROM request_item_link;"));

    store.delete(&request_ref(), true).unwrap();

    assert_eq!(0, count(&f, "SELECT COUNT(*) FROM request_link;"));
    assert_eq!(0, count(&f, "SELECT COUNT(*) FROM request_item_link;"));
    assert_eq!(0, count(&f, "SELECT COUNT(*) FROM request_series;"));
}

/// `공고를_지우면_접수_링크도_걷힌다` — 접수 자체는 남는다.
#[test]
fn deleting_notice_unlinks_request() {
    let f = Fixture::new("delete-notice-request");
    let store = f.store();
    request(&f, "000");
    notice_with_item(&f);
    reference_request(&f, &request_ref(), &notice_ref());

    store.delete(&notice_ref(), true).unwrap();

    assert_eq!(0, count(&f, "SELECT COUNT(*) FROM request_link;"));
    assert_eq!(0, count(&f, "SELECT COUNT(*) FROM request_item_link;"));
    assert_eq!(1, count(&f, "SELECT COUNT(*) FROM v_접수;"));
}

/// `접수를_지우면_덮개도_간다` — 세 종류를 한 표에 담아 외래키를 걸지 못했다.
#[test]
fn deleting_request_takes_overrides() {
    let f = Fixture::new("delete-request-override");
    let store = f.store();
    request(&f, "000");
    store
        .set_override(&request_ref(), "품대", "181,725,201")
        .unwrap();

    assert_eq!(1, count(&f, "SELECT COUNT(*) FROM field_override;"));

    store.delete(&request_ref(), true).unwrap();

    assert_eq!(0, count(&f, "SELECT COUNT(*) FROM field_override;"));
}

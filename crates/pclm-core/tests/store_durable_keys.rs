//! C# `DurableKeyTests` — 변경공고·변경계약·변경접수가 들어와도 **사람이 붙인 것이 뷰에서 사라지지 않는지** 본다.
//!
//! 손으로 적은 값과 명시 참조 연결이 `(본번호, 차수)` 에 매달려 있고 뷰는 최신 차수만 내던 시절, 변경계약 한 건이 들어오면
//! 진행상태·담당·메모가 조용히 빈 문자열이 되고 링크가 끊겨 공고 열이 통째로 비었다. 아무 오류도 나지 않아 사람이 알아챌
//! 방법이 없었다(ADR-012). 값은 모두 지어낸 것이다(CLAUDE.md).

mod common;

use std::collections::BTreeMap;

use common::store_fixture::Fixture;
use pclm_core::erp::explicit_links;
use pclm_core::storage::store::EntityRef;
use rusqlite::named_params;
use rusqlite::types::Value;

const CONTRACT_BASE: &str = "R26TA98000707";
const NOTICE_BASE: &str = "R26BK98000575";
const REQUEST_BASE: &str = "TSTPLA26500286";

/// C# `Contract(seq)` — 건명과 금액(`164_872_340m` 을 Microsoft.Data.Sqlite 가 묶는 꼴).
fn contract(f: &Fixture, seq: &str) {
    f.contract(
        CONTRACT_BASE,
        seq,
        &[
            ("title", "2026년 가람 수질측정기 조달"),
            ("amount", "164872340.0"),
        ],
        &[],
    );
}

/// C# `Notice(seq)`. `title` 은 `with { Title = … }` 자리.
fn notice(f: &Fixture, seq: &str, title: &str) {
    f.notice(NOTICE_BASE, seq, &[("title", title)], &[], &[]);
}

/// C# `Request(seq)` — 품목 한 줄.
fn request(f: &Fixture, seq: &str) {
    f.request(
        REQUEST_BASE,
        seq,
        &[("title", "윈치 8종 구매")],
        &[&[
            ("line_no", "1"),
            ("request_number", REQUEST_BASE),
            ("item_name", "받침목"),
            ("quantity", "70"),
            ("unit_price", "310200.0"),
        ]],
    );
}

/// C# `ReferenceFixture.Contract`.
fn reference_contract(f: &Fixture) {
    let mut c = f.conn();
    let tx = c.transaction().unwrap();
    tx.execute(
        "UPDATE contract SET ref_notice_base=:notice WHERE contract_base=:contract",
        named_params! { ":notice": NOTICE_BASE, ":contract": CONTRACT_BASE },
    )
    .unwrap();
    explicit_links::resolve(&tx).unwrap();
    tx.commit().unwrap();
}

/// C# `ReferenceFixture.Request(request(seq), notice)`.
fn reference_request(f: &Fixture, request_seq: &str) {
    let mut c = f.conn();
    let tx = c.transaction().unwrap();
    tx.execute(
        "UPDATE request_item SET ref_request_base=request_base, ref_request_seq=seq,
             ref_request_item=CAST(line_no AS TEXT) WHERE request_base=:request;",
        named_params! { ":request": REQUEST_BASE },
    )
    .unwrap();
    tx.execute(
        "UPDATE notice_item SET ref_request_base=:request, ref_request_seq=:seq,
             ref_request_item=CAST(line_no AS TEXT) WHERE notice_base=:notice;",
        named_params! { ":request": REQUEST_BASE, ":seq": request_seq, ":notice": NOTICE_BASE },
    )
    .unwrap();
    tx.execute(
        "INSERT INTO notice_item(notice_base,seq,line_no,ref_request_base)
             SELECT notice_base,seq,1,:request FROM notice n WHERE notice_base=:notice
             AND NOT EXISTS(SELECT 1 FROM notice_item i WHERE i.notice_base=n.notice_base AND i.seq=n.seq);",
        named_params! { ":request": REQUEST_BASE, ":notice": NOTICE_BASE },
    )
    .unwrap();
    explicit_links::resolve(&tx).unwrap();
    tx.commit().unwrap();
}

/// C# `Single(sql)` — 줄이 꼭 하나다. NULL 은 빈 글.
fn single(f: &Fixture, sql: &str) -> BTreeMap<String, String> {
    let c = f.database().open_read_only().unwrap();
    let mut statement = c.prepare(sql).unwrap();
    let names: Vec<String> = statement.column_names().map(String::from).collect();
    let rows: Vec<BTreeMap<String, String>> = statement
        .query_map([], |r| {
            let mut row = BTreeMap::new();
            for (i, name) in names.iter().enumerate() {
                let text = match r.get::<_, Value>(i)? {
                    Value::Null => String::new(),
                    Value::Integer(n) => n.to_string(),
                    Value::Real(x) => x.to_string(),
                    Value::Text(t) => t,
                    Value::Blob(_) => "<blob>".into(),
                };
                row.insert(name.clone(), text);
            }
            Ok(row)
        })
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(rows.len(), 1, "줄이 하나가 아닙니다: {sql}");
    rows.into_iter().next().unwrap()
}

#[test]
fn 변경계약이_들어와도_손으로_적은_값이_뷰에_남는다() {
    let f = Fixture::new("durable-contract-field");
    let store = f.store();
    contract(&f, "00");
    store
        .set_user_field(
            &EntityRef::new("contract", CONTRACT_BASE, "00"),
            "진행상태",
            Some("납품"),
        )
        .unwrap();

    // 변경계약. 차수가 오른 별개의 행으로 쌓이고 뷰는 이쪽을 낸다.
    contract(&f, "01");

    // 계약의 사용자 열은 계약 뷰에 붙고, 통합은 그 뷰를 통째로 받는다.
    let row = single(&f, "SELECT 차수, 진행상태 FROM v_통합;");
    assert_eq!(row["차수"], "01");
    assert_eq!(row["진행상태"], "납품");
}

#[test]
fn 변경공고가_들어와도_손으로_적은_값이_뷰에_남는다() {
    let f = Fixture::new("durable-notice-field");
    let store = f.store();
    notice(&f, "000", "수질측정기 구매");
    store
        .set_user_field(
            &EntityRef::new("notice", NOTICE_BASE, "000"),
            "검토여부",
            Some("참여"),
        )
        .unwrap();

    notice(&f, "001", "수질측정기 구매");

    let row = single(&f, "SELECT 차수, 검토여부 FROM v_공고;");
    assert_eq!(row["차수"], "001");
    assert_eq!(row["검토여부"], "참여");
}

#[test]
fn 변경계약이_들어와도_명시참조_연결이_끊기지_않는다() {
    let f = Fixture::new("durable-contract-link");
    notice(&f, "000", "수질측정기 구매");
    contract(&f, "00");
    reference_contract(&f);

    contract(&f, "01");
    reference_contract(&f);

    let row = single(&f, "SELECT 계약번호, 공고명 FROM v_통합;");
    assert_eq!(row["계약번호"], "R26TA9800070701");
    assert_eq!(row["공고명"], "수질측정기 구매");
}

#[test]
fn 변경공고가_들어와도_명시참조_연결이_끊기지_않는다() {
    let f = Fixture::new("durable-notice-link");
    notice(&f, "000", "수질측정기 구매");
    contract(&f, "00");
    reference_contract(&f);

    notice(&f, "001", "수질측정기 구매(변경)");

    let row = single(&f, "SELECT 입찰공고번호, 공고명 FROM v_통합;");
    assert_eq!(row["입찰공고번호"], "R26BK98000575-001");
    assert_eq!(row["공고명"], "수질측정기 구매(변경)");
}

#[test]
fn 변경접수가_들어와도_손으로_적은_값이_뷰에_남는다() {
    let f = Fixture::new("durable-request-field");
    let store = f.store();
    request(&f, "000");
    store
        .add_user_column("request", "비고", "text", &[])
        .unwrap();
    store
        .set_user_field(
            &EntityRef::new("request", REQUEST_BASE, "000"),
            "비고",
            Some("협의중"),
        )
        .unwrap();

    request(&f, "001");

    let row = single(&f, "SELECT 차수, 비고 FROM v_접수;");
    assert_eq!(row["차수"], "001");
    assert_eq!(row["비고"], "협의중");
}

#[test]
fn 변경접수가_들어와도_명시참조_연결이_끊기지_않는다() {
    let f = Fixture::new("durable-request-link");
    request(&f, "000");
    notice(&f, "000", "수질측정기 구매");
    reference_request(&f, "000");

    request(&f, "001");
    contract(&f, "00");
    reference_contract(&f);

    // 링크는 계열끼리라 차수가 올라도 통합의 접수 열이 그대로 채워진다.
    let row = single(&f, "SELECT 접수번호, 요청명 FROM v_통합;");
    assert_eq!(row["접수번호"], "TSTPLA26500286-001");
    assert_eq!(row["요청명"], "윈치 8종 구매");
}

/// 차수는 TEXT 라 사전순 비교는 자릿수가 늘어나는 날 뒤집힌다.
#[test]
fn 차수가_두_자리를_넘어도_최신이_최신이다() {
    let f = Fixture::new("durable-seq-width");
    contract(&f, "09");
    contract(&f, "10");

    assert_eq!(single(&f, "SELECT 차수 FROM v_계약;")["차수"], "10");
}

//! C# `OutlineTests` — 쌓인 것을 **생긴 모양대로** 내는지 본다: 접수 → 공고 → 계약 사슬, 계약 아래 명세. 지키는 것은
//! 평평한 표의 **되풀이가 없다**는 사실과, 사슬이 셋 중 어느 것이 없어도 선다는 사실이다. 접수 뷰의 칸 접기(인도조건·
//! 수량·빈 줄)도 C# 이 이 묶음에 두었으므로 함께 옮긴다.
//!
//! 값은 전부 지어낸 것이다(CLAUDE.md).

mod common;

use common::store_fixture::Fixture;
use pclm_core::erp::explicit_links;
use pclm_core::storage::outline::{Outline, OutlineTree};
use rusqlite::named_params;

const REQUEST: &str = "REQTEST26000001";
const REQUEST_SECOND: &str = "REQTEST26000005";
const NOTICE: &str = "T26BK00000001";
const CONTRACT: &str = "T26TA00000001";

fn build(f: &Fixture) -> OutlineTree {
    Outline::new(f.database()).build().unwrap()
}

/// C# `Request(seq)` — 접수일 2026-06-09, 조달요구 두 줄.
fn request(f: &Fixture, seq: &str, items: Option<&[&[(&str, &str)]]>) {
    let default: &[&[(&str, &str)]] = &[
        &[
            ("line_no", "1"),
            ("request_number", REQUEST),
            ("item_name", "시험 측정 장치"),
            ("quantity", "1"),
            ("unit_price", "1000"),
        ],
        &[
            ("line_no", "2"),
            ("request_number", REQUEST_SECOND),
            ("item_name", "시험 시약"),
            ("quantity", "1"),
            ("unit_price", "1000"),
        ],
    ];
    f.request(
        REQUEST,
        seq,
        &[
            ("title", "시험 측정기 구매"),
            ("received_on", "2026-06-09 00:00:00"),
        ],
        items.unwrap_or(default),
    );
}

/// C# `Notice(seq)` — 게시 2026-07-18.
fn notice(f: &Fixture, seq: &str) {
    f.notice(
        NOTICE,
        seq,
        &[
            ("title", "시험 측정기 구매"),
            ("posted_at", "2026-07-18 00:00:00"),
        ],
        &[],
        &[],
    );
}

/// C# `Contract(seq, 두줄)`.
fn contract(f: &Fixture, seq: &str, two_lines: bool) {
    let item = |line: &'static str, name: &'static str| -> [(&'static str, &'static str); 6] {
        [
            ("line_no", line),
            ("item_name", name),
            ("quantity", "1"),
            ("unit", "대"),
            ("unit_price", "1000"),
            ("amount", "1000"),
        ]
    };
    let first = item("1", "시험 측정 장치");
    let second = item("2", "시험 시약");
    let items: Vec<&[(&str, &str)]> = if two_lines {
        vec![&first, &second]
    } else {
        Vec::new()
    };
    f.contract(
        CONTRACT,
        seq,
        &[
            ("title", "2026년 시험 측정기 조달"),
            ("amount", "123456789"),
        ],
        &items,
    );
}

/// C# `Link()` — `ReferenceFixture.Contract`.
fn link(f: &Fixture) {
    let mut c = f.conn();
    let tx = c.transaction().unwrap();
    tx.execute(
        "UPDATE contract SET ref_notice_base=:notice WHERE contract_base=:contract",
        named_params! { ":notice": NOTICE, ":contract": CONTRACT },
    )
    .unwrap();
    explicit_links::resolve(&tx).unwrap();
    tx.commit().unwrap();
}

/// C# `LinkRequest()` — `ReferenceFixture.Request`.
fn link_request(f: &Fixture) {
    let mut c = f.conn();
    let tx = c.transaction().unwrap();
    tx.execute(
        "UPDATE request_item SET ref_request_base=request_base, ref_request_seq=seq,
             ref_request_item=CAST(line_no AS TEXT) WHERE request_base=:request;",
        named_params! { ":request": REQUEST },
    )
    .unwrap();
    tx.execute(
        "UPDATE notice_item SET ref_request_base=:request, ref_request_seq=:seq,
             ref_request_item=CAST(line_no AS TEXT) WHERE notice_base=:notice;",
        named_params! { ":request": REQUEST, ":seq": "000", ":notice": NOTICE },
    )
    .unwrap();
    tx.execute(
        "INSERT INTO notice_item(notice_base,seq,line_no,ref_request_base)
             SELECT notice_base,seq,1,:request FROM notice n WHERE notice_base=:notice
             AND NOT EXISTS(SELECT 1 FROM notice_item i WHERE i.notice_base=n.notice_base AND i.seq=n.seq);",
        named_params! { ":request": REQUEST, ":notice": NOTICE },
    )
    .unwrap();
    explicit_links::resolve(&tx).unwrap();
    tx.commit().unwrap();
}

#[test]
fn 공고_아래_계약_아래_명세로_달린다() {
    let f = Fixture::new("outline-tree");
    notice(&f, "000");
    contract(&f, "00", true);
    link(&f);

    let tree = build(&f);
    assert_eq!(tree.chains.len(), 1);
    let chain = &tree.chains[0];
    assert!(chain.request.is_none());
    assert_eq!(
        chain
            .notice
            .as_ref()
            .unwrap()
            .current
            .as_ref()
            .unwrap()
            .number,
        format!("{NOTICE}-000")
    );
    assert_eq!(chain.contracts.len(), 1);
    assert_eq!(chain.contracts[0].number, format!("{CONTRACT}00"));
    assert_eq!(chain.contracts[0].items.len(), 2);
}

/// 명세가 두 줄이어도 계약은 한 번만 선다.
#[test]
fn 명세가_여러_줄이어도_계약은_한_번만_선다() {
    let f = Fixture::new("outline-items");
    contract(&f, "00", true);

    let tree = build(&f);
    assert_eq!(tree.chains.len(), 1);
    assert_eq!(tree.chains[0].contracts.len(), 1);
    assert_eq!(tree.chains[0].contracts[0].items.len(), 2);
}

/// 공고가 아직 없는 계약은 제 사슬로 선다.
#[test]
fn 이어지지_않은_계약도_제_사슬로_선다() {
    let f = Fixture::new("outline-loose");
    notice(&f, "000");
    contract(&f, "00", false);

    let chains = build(&f).chains;
    assert_eq!(chains.len(), 2);
    let with_notice: Vec<_> = chains.iter().filter(|c| c.notice.is_some()).collect();
    assert_eq!(with_notice.len(), 1);
    assert!(with_notice[0].contracts.is_empty());
    let alone: Vec<_> = chains.iter().filter(|c| c.notice.is_none()).collect();
    assert_eq!(alone.len(), 1);
    assert!(alone[0].request.is_none());
    assert_eq!(alone[0].contracts.len(), 1);
    assert_eq!(alone[0].contracts[0].number, format!("{CONTRACT}00"));
}

/// 접수 → 공고 → 계약이 한 사슬로 선다 — 이 자료의 기본 모양.
#[test]
fn 접수와_공고와_계약이_한_사슬로_선다() {
    let f = Fixture::new("outline-chain");
    request(&f, "000", None);
    notice(&f, "000");
    contract(&f, "00", false);
    link(&f);
    link_request(&f);

    let tree = build(&f);
    assert_eq!(tree.chains.len(), 1);
    let chain = &tree.chains[0];
    assert_eq!(
        chain.request.as_ref().unwrap().number,
        format!("{REQUEST}-000")
    );
    assert_eq!(
        chain
            .notice
            .as_ref()
            .unwrap()
            .current
            .as_ref()
            .unwrap()
            .number,
        format!("{NOTICE}-000")
    );
    assert_eq!(chain.contracts.len(), 1);
    assert_eq!(chain.contracts[0].number, format!("{CONTRACT}00"));
    // 사슬의 이름은 있는 것의 본번호를 이어 만든다.
    assert_eq!(chain.key, format!("{REQUEST}/{NOTICE}"));
}

/// 접수만 들어온 것도 제 사슬로 선다.
#[test]
fn 공고가_없는_접수도_제_사슬로_선다() {
    let f = Fixture::new("outline-request-only");
    request(&f, "000", None);

    let tree = build(&f);
    assert_eq!(tree.chains.len(), 1);
    let chain = &tree.chains[0];
    assert_eq!(
        chain.request.as_ref().unwrap().number,
        format!("{REQUEST}-000")
    );
    assert!(chain.notice.is_none());
    assert!(chain.contracts.is_empty());
}

/// 조달요구번호는 뷰가 이어 붙인 것을 도로 갈라 낸다.
#[test]
fn 조달요구번호가_갈라져_온다() {
    let f = Fixture::new("outline-numbers");
    request(&f, "000", None);

    let tree = build(&f);
    assert_eq!(tree.chains.len(), 1);
    assert_eq!(
        tree.chains[0].request.as_ref().unwrap().request_numbers,
        [REQUEST, REQUEST_SECOND]
    );
}

/// 조달요구 하나가 납품장소마다 줄을 갈라 와도 번호는 한 번만 선다 — 차례는 처음 나온 순번이다.
#[test]
fn 납지로_갈린_조달요구번호는_한_번만_선다() {
    let f = Fixture::new("outline-places");
    request(
        &f,
        "000",
        Some(&[
            &[
                ("line_no", "1"),
                ("request_number", REQUEST_SECOND),
                ("quantity", "2"),
                ("delivery_place", "가부대"),
            ],
            &[
                ("line_no", "2"),
                ("request_number", REQUEST),
                ("quantity", "1"),
                ("delivery_place", "가부대"),
            ],
            &[
                ("line_no", "3"),
                ("request_number", REQUEST_SECOND),
                ("quantity", "3"),
                ("delivery_place", "나부대"),
            ],
        ]),
    );

    let tree = build(&f);
    assert_eq!(tree.chains.len(), 1);
    assert_eq!(
        tree.chains[0].request.as_ref().unwrap().request_numbers,
        [REQUEST_SECOND, REQUEST]
    );
    assert_eq!(
        f.scalar("SELECT COUNT(*) FROM v_접수품목;").as_deref(),
        Some("3")
    );
}

/// 접수도 인도조건은 줄마다 같으면 싣는다.
#[test]
fn 접수_인도조건은_세부품명이_여럿이어도_같으면_싣는다() {
    let f = Fixture::new("outline-terms");
    request(
        &f,
        "000",
        Some(&[
            &[
                ("line_no", "1"),
                ("request_number", REQUEST),
                ("item_name", "시험 기계"),
                ("delivery_terms", "납품장소 입고도"),
            ],
            &[
                ("line_no", "2"),
                ("request_number", REQUEST_SECOND),
                ("item_name", "시험 부품"),
                ("delivery_terms", "납품장소 입고도"),
            ],
        ]),
    );

    assert_eq!(
        f.scalar("SELECT 인도조건 || '|' || 세부품명 FROM v_접수;")
            .as_deref(),
        Some("납품장소 입고도|")
    );
}

/// `[Theory]` — 단위가 모두 같으면 수량을 더하고, 갈리면 수량도 단위도 비운다.
#[test]
fn 접수_수량은_단위가_같을_때만_더한다() {
    for (first, second, expected) in [("대", "대", "3|대"), ("대", "식", "|")] {
        let f = Fixture::new("outline-units");
        request(
            &f,
            "000",
            Some(&[
                &[
                    ("line_no", "1"),
                    ("request_number", "REQ-A"),
                    ("item_name", "지어낸 기계"),
                    ("quantity", "1"),
                    ("unit", first),
                ],
                &[
                    ("line_no", "2"),
                    ("request_number", "REQ-B"),
                    ("item_name", "지어낸 부품"),
                    ("quantity", "2"),
                    ("unit", second),
                ],
            ]),
        );
        assert_eq!(
            f.scalar("SELECT 수량 || '|' || 단위 FROM v_접수;")
                .as_deref(),
            Some(expected),
            "{first} {second}"
        );
    }
}

/// `[Theory]` — 접수도 빈 줄을 한 값으로 센다.
#[test]
fn 접수도_빈_줄이_섞이면_칸을_비운다() {
    /// (첫 줄 재고번호, 둘째 줄 재고번호, 첫 줄 수량, 둘째 줄 수량, 기대).
    type Case = (
        Option<&'static str>,
        Option<&'static str>,
        Option<&'static str>,
        Option<&'static str>,
        &'static str,
    );
    let cases: &[Case] = &[
        (
            Some("STK-1"),
            Some("STK-1"),
            Some("5"),
            Some("3"),
            "STK-1|8",
        ),
        (Some("STK-1"), None, Some("5"), Some("3"), "|8"),
        (Some("STK-1"), Some(""), Some("5"), Some("3"), "|8"),
        (Some("STK-1"), Some("STK-1"), Some("5"), None, "STK-1|"),
    ];
    for &(first_stock, second_stock, first_quantity, second_quantity, expected) in cases {
        let f = Fixture::new("outline-blank-lines");
        let line = |no: &'static str,
                    number: &'static str,
                    name: &'static str,
                    stock: Option<&'static str>,
                    quantity: Option<&'static str>| {
            let mut fields = vec![
                ("line_no", no),
                ("request_number", number),
                ("item_name", name),
                ("unit", "대"),
            ];
            if let Some(q) = quantity {
                fields.push(("quantity", q));
            }
            if let Some(s) = stock {
                fields.push(("stock_number", s));
            }
            fields
        };
        let a = line("1", "REQ-A", "지어낸 기계", first_stock, first_quantity);
        let b = line("2", "REQ-B", "지어낸 부품", second_stock, second_quantity);
        request(&f, "000", Some(&[&a, &b]));
        assert_eq!(
            f.scalar("SELECT 재고번호 || '|' || 수량 FROM v_접수;")
                .as_deref(),
            Some(expected),
            "{expected}"
        );
    }
}

/// 차수는 접는다 — 본문에는 최신 하나가 서고 쌓인 차수는 이름표로 모인다.
#[test]
fn 차수는_접혀서_최신_하나만_선다() {
    let f = Fixture::new("outline-revisions");
    notice(&f, "000");
    notice(&f, "001");
    contract(&f, "00", false);
    contract(&f, "01", false);
    link(&f);

    let tree = build(&f);
    assert_eq!(tree.chains.len(), 1);
    let chain = &tree.chains[0];
    let current = chain.notice.as_ref().unwrap().current.as_ref().unwrap();
    assert_eq!(current.number, format!("{NOTICE}-001"));
    assert_eq!(current.revisions, ["000", "001"]);
    assert_eq!(chain.contracts.len(), 1);
    assert_eq!(chain.contracts[0].number, format!("{CONTRACT}01"));
    assert_eq!(chain.contracts[0].revisions, ["00", "01"]);
}

/// 금액과 날짜는 뷰가 찍은 그대로 온다.
#[test]
fn 금액은_뷰가_찍은_표기로_온다() {
    let f = Fixture::new("outline-amount");
    contract(&f, "00", false);

    let tree = build(&f);
    assert_eq!(tree.chains.len(), 1);
    assert_eq!(tree.chains[0].contracts.len(), 1);
    assert_eq!(tree.chains[0].contracts[0].amount, "123,456,789");
}

/// 차례는 가장 이른 단계를 본다 — 공고가 있으면 게시일, 없으면 접수일.
#[test]
fn 최근_것이_위로_온다() {
    let f = Fixture::new("outline-order");
    request(&f, "000", None); // 2026/06/09
    notice(&f, "000"); // 2026/07/18

    let chains = build(&f).chains;
    assert_eq!(chains.len(), 2);
    assert!(chains[0].notice.is_some()); // 게시일이 늦어 위로
    assert!(chains[1].request.is_some());
}

#[test]
fn 빈_db에서도_돈다() {
    let f = Fixture::new("outline-empty");
    assert!(build(&f).chains.is_empty());
}

/// 다리가 싣는 꼴 — 없는 접수·건은 `null`, 이름은 camelCase, 명세 순번은 수.
#[test]
fn 다리_꼴은_빈_자리를_null로_적는다() {
    let f = Fixture::new("outline-json");
    contract(&f, "00", true);
    let json = build(&f).to_json().to_string();
    assert!(
        json.starts_with(&format!(
            r#"{{"chains":[{{"key":"{CONTRACT}","request":null,"notice":null,"contracts":[{{"key":"{CONTRACT}","number":"{CONTRACT}00","#
        )),
        "{json}"
    );
    assert!(
        json.contains(r#""revisions":["00"],"items":[{"lineNo":1,"name":"시험 측정 장치","#),
        "{json}"
    );
}

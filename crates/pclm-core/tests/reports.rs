//! C# `ReportsTests` — 상태 조회가 자료가 한 건도 없는 DB 에서도 돌고, 브리핑이 KST 경계·재공고·계약 여럿·단독 문서·
//! 정정된 값·납품기한을 C# 과 같이 세는지 본다. 자료는 C# 의 `UpsertNotice`·`UpsertContract`·`UpsertRequest` 가 짓는 줄을
//! SQL 로 세운다(DateTime 은 Microsoft.Data.Sqlite 가 묶는 `yyyy-MM-dd HH:mm:ss`, decimal 은 글, bool 은 0/1).
//!
//! 값은 전부 지어낸 것이다(CLAUDE.md).

mod common;

use common::store_fixture::{Fixture, NOW};
use pclm_core::erp::explicit_links;
use pclm_core::storage::reports::{DashboardCase, DashboardReport, DeliveryDeadline, Reports};
use pclm_core::storage::store::EntityRef;
use pclm_core::{Access, Database};
use rusqlite::named_params;

/// C# `At` — 2026-10-09 09:00 KST.
const AT: &str = "2026-10-09T00:00:00.0000000Z";
const NOTICE: &str = "T26BK00000001";
const CONTRACT: &str = "T26TA00000001";
const NOTICE_TITLE: &str = "시험 측정기 구매";
const CONTRACT_TITLE: &str = "2026년 시험 측정기 조달";

fn dashboard(f: &Fixture) -> DashboardReport {
    Reports::new(f.database()).dashboard(AT).unwrap()
}

fn dashboard_read(f: &Fixture) -> DashboardReport {
    Reports::new(Database::new(&f.path, Access::Read))
        .dashboard(AT)
        .unwrap()
}

fn single(rows: &[DashboardCase], pick: impl Fn(&DashboardCase) -> bool) -> &DashboardCase {
    let found: Vec<&DashboardCase> = rows.iter().filter(|r| pick(r)).collect();
    assert_eq!(found.len(), 1, "꼭 하나여야 한다");
    found[0]
}

fn only(rows: &[DashboardCase]) -> &DashboardCase {
    assert_eq!(rows.len(), 1);
    &rows[0]
}

/// C# `Notice(seq)` 에 필드를 더한 것.
fn notice(f: &Fixture, seq: &str, fields: &[(&str, &str)]) {
    let mut all = vec![("title", NOTICE_TITLE)];
    all.extend_from_slice(fields);
    f.notice(NOTICE, seq, &all, &[], &[]);
}

/// C# `Contract(seq)` 에 필드를 더한 것(같은 열을 주면 뒤의 것이 이긴다).
fn contract(f: &Fixture, seq: &str, fields: &[(&str, &str)], items: &[&[(&str, &str)]]) {
    let mut all: Vec<(&str, &str)> = vec![("title", CONTRACT_TITLE), ("amount", "164872340")];
    for (k, v) in fields {
        all.retain(|(a, _)| a != k);
        all.push((k, v));
    }
    f.contract(CONTRACT, seq, &all, items);
}

/// C# `NoticeRecord.Schedule` — 일정 줄을 갈아 끼운다. 시각은 `None` 이면 NULL.
fn schedule(f: &Fixture, base: &str, seq: &str, steps: &[(i64, &str, Option<&str>, Option<&str>)]) {
    let c = f.conn();
    c.execute(
        "DELETE FROM notice_schedule WHERE notice_base = ?1 AND seq = ?2;",
        [base, seq],
    )
    .unwrap();
    for (line, name, starts, ends) in steps {
        c.execute(
            "INSERT INTO notice_schedule (notice_base, seq, line_no, name, starts_at, ends_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6);",
            rusqlite::params![base, seq, line, name, starts, ends],
        )
        .unwrap();
    }
}

/// C# `ReferenceFixture.Contract` — 계약 문서에 명시 참조를 넣고 연결을 다시 짓는다.
fn reference_contract(f: &Fixture, contract_base: &str, notice_base: &str) {
    let mut c = f.conn();
    let tx = c.transaction().unwrap();
    tx.execute(
        "UPDATE contract SET ref_notice_base=:notice WHERE contract_base=:contract",
        named_params! { ":notice": notice_base, ":contract": contract_base },
    )
    .unwrap();
    explicit_links::resolve(&tx).unwrap();
    tx.commit().unwrap();
}

/// C# `ReferenceFixture.Request` — 공고 품목이 접수를 가리키게 하고 연결을 다시 짓는다.
fn reference_request(f: &Fixture, request_base: &str, request_seq: &str, notice_base: &str) {
    let mut c = f.conn();
    let tx = c.transaction().unwrap();
    tx.execute(
        "UPDATE request_item SET ref_request_base=request_base, ref_request_seq=seq,
             ref_request_item=CAST(line_no AS TEXT) WHERE request_base=:request;",
        named_params! { ":request": request_base },
    )
    .unwrap();
    tx.execute(
        "UPDATE notice_item SET ref_request_base=:request, ref_request_seq=:seq,
             ref_request_item=CAST(line_no AS TEXT) WHERE notice_base=:notice;",
        named_params! { ":request": request_base, ":seq": request_seq, ":notice": notice_base },
    )
    .unwrap();
    tx.execute(
        "INSERT INTO notice_item(notice_base,seq,line_no,ref_request_base)
             SELECT notice_base,seq,1,:request FROM notice n WHERE notice_base=:notice
             AND NOT EXISTS(SELECT 1 FROM notice_item i WHERE i.notice_base=n.notice_base AND i.seq=n.seq);",
        named_params! { ":request": request_base, ":notice": notice_base },
    )
    .unwrap();
    explicit_links::resolve(&tx).unwrap();
    tx.commit().unwrap();
}

fn set_override(f: &Fixture, entity: (&str, &str, &str), column: &str, value: &str) {
    f.store()
        .set_override(&EntityRef::new(entity.0, entity.1, entity.2), column, value)
        .unwrap();
}

fn amount(value: Option<pclm_core::decimal::Decimal>) -> Option<String> {
    value.map(|d| d.to_text())
}

#[test]
fn 빈_db에서도_조회가_돈다() {
    let f = Fixture::new("reports-empty");
    let reports = Reports::new(f.database());

    assert_eq!(reports.summary().unwrap().notice_rows, 0);
    assert!(reports.revisions().unwrap().is_empty());
    let briefing = dashboard_read(&f);
    assert_eq!(briefing.total_cases, 0);
    assert!(briefing.rows.is_empty());
    assert!(briefing.stages.iter().all(|s| s.count == 0));
    assert_eq!(briefing.contract_amount.to_text(), "0");
    let d = &briefing.documents;
    assert_eq!(
        (
            d.requests,
            d.notices,
            d.contracts,
            d.request_revisions,
            d.notice_revisions,
            d.contract_revisions
        ),
        (0, 0, 0, 0, 0, 0)
    );
    assert_eq!(briefing.data_check_count, 0);
    assert!(briefing.deliveries.is_empty());
    let labels: Vec<&str> = briefing.stages.iter().map(|s| s.label.as_str()).collect();
    assert_eq!(labels, ["공고 대기", "공고중", "계약 대기", "계약 완료"]);
    assert_eq!(briefing.as_of, "2026-10-09T09:00:00.0000000+09:00");
}

#[test]
fn 차수는_공고와_계약을_따로_세고_많이_쌓인_것이_앞에_온다() {
    let f = Fixture::new("reports-revisions");
    notice(&f, "000", &[]);
    notice(&f, "001", &[]);
    contract(&f, "00", &[], &[]);

    let groups = Reports::new(f.database()).revisions().unwrap();

    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].kind, "공고");
    assert_eq!(groups[0].base, NOTICE);
    assert_eq!(groups[0].revisions, 2);
    assert_eq!(groups[0].latest, "001"); // 차수 문자열 그대로, 정수로 정렬한 최신
    assert_eq!(groups[0].title.as_deref(), Some(NOTICE_TITLE));
    assert_eq!(groups[1].kind, "계약");
    assert_eq!(groups[1].revisions, 1);
}

/// 제목은 **최신 차수의 것**이다. 사전순 최대를 고르면 「최신 001」 옆에 000 의 제목이 붙는다.
#[test]
fn 제목은_최신_차수의_것이다() {
    let f = Fixture::new("reports-title");
    f.notice(
        NOTICE,
        "000",
        &[("title", "26년 시험 절단기 구매 (최종 공고가 아닙니다.)")],
        &[],
        &[],
    );
    f.notice(
        NOTICE,
        "001",
        &[("title", "26년 시험 절단기 구매")],
        &[],
        &[],
    );

    let groups = Reports::new(f.database()).revisions().unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].latest, "001");
    assert_eq!(groups[0].title.as_deref(), Some("26년 시험 절단기 구매"));
}

/// `[Theory]` — (게시, 개찰, 입찰마감, 단계). 시각은 2026-10-09 의 시(음수면 없음).
#[test]
fn 브리핑은_게시와_개찰_kst_경계만_단계로_쓰고_부족한정보를_따로_센다() {
    let cases: &[(i32, i32, i32, Option<&str>)] = &[
        (8, 10, 8, Some("open")),
        (9, 10, 12, Some("open")),
        (8, 9, 12, Some("awaitingContract")),
        (8, 8, 12, Some("awaitingContract")),
        (10, 11, 12, None),
        (8, -1, 8, None),
        (-1, 10, 11, None),
        (10, 9, 8, None),
    ];
    for &(posted, opening, bid_close, stage) in cases {
        let f = Fixture::new("reports-kst");
        let time = |hour: i32| (hour >= 0).then(|| format!("2026-10-09 {hour:02}:00:00"));
        let posted_at = time(posted);
        let mut fields = vec![("notice_kind", "등록공고")];
        if let Some(p) = &posted_at {
            fields.push(("posted_at", p));
        }
        notice(&f, "000", &fields);
        schedule(
            &f,
            NOTICE,
            "000",
            &[
                (
                    1,
                    "입찰서제출",
                    time(11).as_deref(),
                    time(bid_close).as_deref(),
                ),
                (2, "개찰", time(opening).as_deref(), None),
            ],
        );
        let report = dashboard(&f);
        let row = only(&report.rows);
        assert_eq!(
            row.stage.as_deref(),
            stage,
            "{posted} {opening} {bid_close}"
        );
        assert!(report.as_of.ends_with("+09:00"));
        assert_eq!(
            1,
            report.stages.iter().map(|s| s.count).sum::<i32>() + report.data_check_count
        );
        assert_eq!(i32::from(stage.is_none()), report.data_check_count);
        assert_eq!(row.contract_amount, None);
        assert_eq!(report.contracts_with_amount, 0);
    }
}

#[test]
fn 브리핑은_차수와_재공고를_접고_계약여럿과_단독문서를_빠뜨리지_않는다() {
    let f = Fixture::new("reports-fold");
    f.request("REQ", "000", &[("title", "원제목")], &[]);
    f.request("REQ", "010", &[("title", "최신 접수")], &[]);
    f.request(
        "STANDALONE",
        "000",
        &[("title", "공고 미수집 접수")],
        &[&[("line_no", "1"), ("cancelled", "1")]],
    );
    notice(&f, "000", &[("notice_kind", "등록공고")]);
    notice(&f, "001", &[("notice_kind", "취소공고")]);
    let related = format!("{NOTICE}-001");
    f.notice(
        "RENOTICE",
        "000",
        &[("title", "재공고"), ("notice_kind", "등록공고")],
        &[],
        &[&related],
    );
    contract(&f, "00", &[("amount", "10"), ("terminated", "1")], &[]);
    contract(
        &f,
        "10",
        &[
            ("amount", "25"),
            ("terminated", "0"),
            ("contracted_on", "2026-10-08 00:00:00"),
        ],
        &[],
    );
    f.contract("SECOND", "00", &[("amount", "7"), ("terminated", "1")], &[]);
    f.contract(
        "STANDALONE",
        "00",
        &[
            ("terminated", "1"),
            ("contracted_on", "2026-10-01 00:00:00"),
        ],
        &[],
    );
    reference_request(&f, "REQ", "010", "RENOTICE");
    reference_contract(&f, CONTRACT, "RENOTICE");
    reference_contract(&f, "SECOND", "RENOTICE");
    f.notice("MISSING", "000", &[], &[], &[]);
    f.contract("MISSING", "00", &[("amount", "0")], &[]);
    reference_contract(&f, "MISSING", "MISSING");
    // 문서 없는 계열/링크가 남은 유효한 부분 DB도 계약 문서를 분모에서 잃으면 안 된다.
    f.sql("DELETE FROM notice WHERE notice_base='MISSING'");

    let report = dashboard_read(&f);
    assert_eq!(report.total_cases, 4);
    assert_eq!(report.stages.iter().map(|s| s.count).sum::<i32>(), 4);
    let d = &report.documents;
    assert_eq!(
        (
            d.requests,
            d.notices,
            d.contracts,
            d.request_revisions,
            d.notice_revisions,
            d.contract_revisions
        ),
        (2, 2, 4, 3, 3, 5)
    );
    assert_eq!(report.contract_amount.to_text(), "32"); // 최신 차수 25 + 해지 표시 계약 7, 옛 계약 10은 제외
    assert_eq!(report.contracts_with_amount, 3);
    let group = single(&report.rows, |r| r.notice_number == "RENOTICE-000");
    assert_eq!(group.stage.as_deref(), Some("awaitingContract"));
    assert_eq!(group.draft_contracts, 1);
    let latest = format!("{CONTRACT}10");
    assert!(
        !group
            .contracts
            .iter()
            .find(|c| c.number == latest)
            .unwrap()
            .draft
    );
    assert!(
        group
            .contracts
            .iter()
            .find(|c| c.number == "SECOND00")
            .unwrap()
            .draft
    );
    assert_eq!(group.request_number, "REQ-010");
    assert_eq!(group.title, "재공고");
    assert_eq!(
        group.notice_numbers,
        ["RENOTICE-000".to_string(), related.clone()]
    );
    assert_eq!(group.contracts.len(), 2);
    assert_eq!(amount(group.contract_amount).as_deref(), Some("32"));
    assert!(group.issues.iter().any(|s| s.contains("SECOND00")));
    assert!(group.issues.iter().any(|s| s.contains("관련공고 2개")));
    assert!(
        single(&report.rows, |r| r.stage.as_deref() == Some("requestOnly"))
            .issues
            .iter()
            .any(|i| i.contains("취소 표시 접수 품목 1개"))
    );
    assert_eq!(
        amount(single(&report.rows, |r| r.key == "group:MISSING").contract_amount).as_deref(),
        Some("0")
    );
    let terminated = single(&report.rows, |r| {
        r.terminated_contracts == 1 && r.contracts.len() == 1
    });
    assert_eq!(terminated.stage.as_deref(), Some("contracted"));
    assert_eq!(terminated.contract_amount, None);
    assert!(terminated.issues.iter().any(|s| s.contains("금액")));
}

/// `[Theory]` — 현행이 취소공고·실공고(취소공고)이거나, 서로를 가리켜 현행을 정할 수 없는 건.
#[test]
fn 브리핑은_현행_취소와_모호한_현행을_가공하지_않는다() {
    for kind in ["취소공고", "실공고(취소공고)"] {
        let f = Fixture::new("reports-cancelled");
        notice(&f, "000", &[("notice_kind", "등록공고")]);
        notice(&f, "001", &[("notice_kind", kind)]);
        f.notice("LOOP1", "000", &[], &[], &["LOOP2-000"]);
        f.notice("LOOP2", "000", &[], &[], &["LOOP1-000"]);
        let report = dashboard(&f);
        assert_eq!(report.total_cases, 2);
        assert_eq!(
            single(&report.rows, |r| r.notice_cancelled).notice_number,
            format!("{NOTICE}-001")
        );
        assert!(report.rows.iter().all(|r| r.stage.is_none()));
        assert_eq!(report.data_check_count, 2);
        let unclear = single(&report.rows, |r| !r.notice_cancelled);
        assert_eq!(unclear.notice_number, "");
        assert_eq!(unclear.notice_numbers, ["LOOP1-000", "LOOP2-000"]);
        assert!(unclear.issues.iter().any(|s| s.contains("현행 공고")));
        contract(&f, "00", &[("contracted_on", "2026-10-01 00:00:00")], &[]);
        reference_contract(&f, CONTRACT, NOTICE);
        let after = dashboard(&f);
        let with_contract = single(&after.rows, |r| r.stage.as_deref() == Some("contracted"));
        assert!(
            with_contract
                .issues
                .iter()
                .any(|i| i.contains("취소공고와 계약"))
        );
    }
}

#[test]
fn 브리핑은_정정된_날짜와_금액을_읽되_틀린값을_0으로_바꾸지_않는다() {
    let f = Fixture::new("reports-corrected");
    notice(&f, "000", &[("posted_at", "2026-10-09 08:00:00")]);
    schedule(
        &f,
        NOTICE,
        "000",
        &[(1, "개찰", Some("2026-10-09 11:00:00"), None)],
    );
    set_override(&f, ("notice", NOTICE, "000"), "게시일시", "읽을 수 없는 날");
    contract(
        &f,
        "00",
        &[("amount", "10"), ("contracted_on", "2026-10-01 00:00:00")],
        &[],
    );
    set_override(&f, ("contract", CONTRACT, "00"), "계약금액", "금액 미확인");
    let report = dashboard(&f);
    assert!(
        single(&report.rows, |r| r.stage.is_none())
            .issues
            .iter()
            .any(|i| i.contains("게시일시·개찰일시"))
    );
    assert_eq!(
        single(&report.rows, |r| r.stage.as_deref() == Some("contracted")).contract_amount,
        None
    );
    assert_eq!(report.contract_amount.to_text(), "0");
    assert_eq!(report.contracts_with_amount, 0);
    set_override(&f, ("notice", NOTICE, "000"), "게시일시", "0001/01/01");
    single(&dashboard(&f).rows, |r| r.stage.is_none());
}

/// `[Theory]` — ISO 꼴 정정도 KST 로 읽되, 다른 꼴을 추정하지 않는다.
#[test]
fn iso_날짜_정정도_kst로_읽되_다른_날짜꼴을_추정하지_않는다() {
    let cases: &[(&str, &str, Option<&str>)] = &[
        ("2026-10-09", "2026-10-10", Some("open")),
        (
            "2026-10-09 08:00:00",
            "2026-10-09 09:00:00",
            Some("awaitingContract"),
        ),
        ("10/09/2026", "2026-10-10", None),
        ("2026-02-30", "2026-10-10", None),
    ];
    for &(posted, opening, stage) in cases {
        let f = Fixture::new("reports-iso");
        notice(&f, "000", &[("notice_kind", "등록공고")]);
        set_override(&f, ("notice", NOTICE, "000"), "게시일시", posted);
        set_override(&f, ("notice", NOTICE, "000"), "개찰일시", opening);
        assert_eq!(
            only(&dashboard(&f).rows).stage.as_deref(),
            stage,
            "{posted}"
        );
    }
}

#[test]
fn 공고_대기는_접수를_필요로_하고_날짜없는_계약은_초안이며_틀린날짜는_확인대상이다() {
    let f = Fixture::new("reports-waiting");
    f.request("REQ", "000", &[], &[]);
    notice(&f, "000", &[("posted_at", "2026-10-10 00:00:00")]);
    schedule(
        &f,
        NOTICE,
        "000",
        &[(1, "개찰", Some("2026-10-11 00:00:00"), None)],
    );
    reference_request(&f, "REQ", "000", NOTICE);
    assert_eq!(
        only(&dashboard(&f).rows).stage.as_deref(),
        Some("requestOnly")
    );
    set_override(&f, ("notice", NOTICE, "000"), "개찰일시", "");
    // 게시 전은 개찰 시각 없이도 알 수 있다.
    assert_eq!(
        only(&dashboard(&f).rows).stage.as_deref(),
        Some("requestOnly")
    );
    set_override(&f, ("notice", NOTICE, "000"), "개찰일시", "개찰 미확인");
    let report = dashboard(&f);
    let unread_opening = only(&report.rows);
    // 게시 전이면 공고 대기다. 틀린 개찰 시각은 행의 확인 표시로 남는다.
    assert_eq!(unread_opening.stage.as_deref(), Some("requestOnly"));
    assert!(unread_opening.issues.iter().any(|i| i.contains("개찰일시")));
    contract(&f, "00", &[], &[]);
    reference_contract(&f, CONTRACT, NOTICE);
    let report = dashboard(&f);
    let draft = only(&report.rows);
    assert_eq!(draft.stage.as_deref(), Some("awaitingContract"));
    assert_eq!(draft.draft_contracts, 1);
    set_override(&f, ("contract", CONTRACT, "00"), "계약일자", "날짜 미확인");
    let invalid = dashboard(&f);
    assert_eq!(only(&invalid.rows).stage, None);
    assert_eq!(invalid.data_check_count, 1);
    assert_eq!(only(&invalid.rows).draft_contracts, 0);
}

/// `[Theory]` — 공고 대기는 「접수 후 게시된 현행 공고가 없는 건」이다.
#[test]
fn 공고_대기는_접수_후_게시된_현행_공고가_없는_건이다() {
    let cases: &[(&str, &str, Option<&str>)] = &[
        ("none", "requestOnly", None),
        ("draft", "requestOnly", None),
        ("future", "requestOnly", None),
        ("cancelled", "requestOnly", Some("취소공고")),
        ("cancelledComposite", "requestOnly", Some("취소공고")),
        ("unreadable", "requestOnly", Some("게시일시")),
        ("posted", "open", None),
    ];
    for &(scenario, stage, issue) in cases {
        let f = Fixture::new("reports-request-only");
        f.request("REQTEST01", "000", &[("title", "시험 접수")], &[]);
        let made = |seq: &str, posted: Option<&str>, kind: &str| {
            let mut fields = vec![("title", "시험 공고"), ("notice_kind", kind)];
            if let Some(p) = posted {
                fields.push(("posted_at", p));
            }
            f.notice("NTCTEST01", seq, &fields, &[], &[]);
            schedule(
                &f,
                "NTCTEST01",
                seq,
                &[(1, "개찰", Some("2026-10-20 00:00:00"), None)],
            );
        };
        match scenario {
            "draft" => made("000", None, "등록공고"),
            "future" => made("000", Some("2026-10-10 00:00:00"), "등록공고"),
            "cancelled" | "cancelledComposite" => {
                made("000", Some("2026-10-01 00:00:00"), "등록공고");
                made(
                    "001",
                    Some("2026-10-02 00:00:00"),
                    if scenario == "cancelled" {
                        "취소공고"
                    } else {
                        "실공고(취소공고)"
                    },
                );
            }
            "unreadable" => {
                made("000", Some("2026-10-01 00:00:00"), "등록공고");
                set_override(
                    &f,
                    ("notice", "NTCTEST01", "000"),
                    "게시일시",
                    "게시 미확인",
                );
            }
            "posted" => made("000", Some("2026-10-01 00:00:00"), "등록공고"),
            _ => {}
        }
        if scenario != "none" {
            reference_request(&f, "REQTEST01", "000", "NTCTEST01");
        }
        let report = dashboard(&f);
        let row = only(&report.rows);
        assert_eq!(row.stage.as_deref(), Some(stage), "{scenario}");
        assert_eq!(report.data_check_count, 0);
        match issue {
            None => assert!(row.issues.is_empty(), "{scenario}: {:?}", row.issues),
            Some(issue) => assert!(row.issues.iter().any(|i| i.contains(issue)), "{scenario}"),
        }
    }
}

/// `[Theory]` — 접수가 없는 미게시 공고(게시일시 없음 · 게시 전)는 공고 대기로 세지 않는다.
#[test]
fn 접수가_없는_미게시_공고는_공고_대기로_세지_않는다() {
    for day in [-1, 10] {
        let f = Fixture::new("reports-no-request");
        let posted = format!("2026-10-{day:02} 00:00:00");
        let mut fields = vec![("title", "시험 공고"), ("notice_kind", "등록공고")];
        if day >= 0 {
            fields.push(("posted_at", &posted));
        }
        f.notice("NTCTEST02", "000", &fields, &[], &[]);
        let report = dashboard(&f);
        let row = only(&report.rows);
        assert_eq!(row.stage, None);
        assert!(!row.issues.is_empty());
    }
}

#[test]
fn 납품기한은_최신_계약과_모든_품목을_따로_내고_kst_날짜로_경과일을_센다() {
    let f = Fixture::new("reports-delivery");
    contract(
        &f,
        "00",
        &[("delivery_due", "2020-01-01 00:00:00")],
        &[&[
            ("line_no", "99"),
            ("item_name", "옛 품목"),
            ("delivery_due", "2020-01-01 00:00:00"),
        ]],
    );
    f.conn()
        .execute(
            "INSERT INTO counterparty (business_number, name, updated_at) VALUES ('0000000000', '시험 업체', ?1);",
            [NOW],
        )
        .unwrap();
    contract(
        &f,
        "10",
        &[
            ("contracted_on", "2026-10-01 00:00:00"),
            ("delivery_due", "2026-10-16 23:59:59"),
            ("counterparty_number", "0000000000"),
            ("demand_agency", "기관"),
        ],
        &[
            &[
                ("line_no", "1"),
                ("item_name", "경과"),
                ("delivery_due", "2026-10-08 23:59:59"),
            ],
            &[("line_no", "2"), ("item_name", "계약기한 적용")],
            &[
                ("line_no", "3"),
                ("item_name", "오늘"),
                ("delivery_due", "2026-10-09 00:00:00"),
            ],
            &[
                ("line_no", "4"),
                ("item_name", "30일"),
                ("delivery_due", "2026-11-08 00:00:00"),
            ],
        ],
    );
    set_override(&f, ("contract", CONTRACT, "10"), "납품기한", "2026-10-16");
    f.contract(
        "ENDED",
        "00",
        &[("terminated", "1"), ("delivery_due", "2026-10-09 00:00:00")],
        &[],
    );
    let number = format!("{CONTRACT}10");
    let report = dashboard_read(&f);
    assert_eq!(report.deliveries.len(), 2);
    let delivery = report
        .deliveries
        .iter()
        .find(|d| d.number == number)
        .unwrap();
    assert_eq!(delivery.party, "시험 업체");
    assert_eq!(delivery.agency, "기관");
    assert_eq!(
        delivery.due,
        DeliveryDeadline {
            raw: "2026-10-16".into(),
            date: Some("2026-10-16".into()),
            days_remaining: Some(7)
        }
    );
    let lines: Vec<i32> = delivery.items.iter().map(|i| i.line_no).collect();
    assert_eq!(lines, [1, 2, 3, 4]);
    assert_eq!(delivery.items[0].due.days_remaining, Some(-1));
    assert_eq!(delivery.items[1].due.date, None); // 원래 품목값은 보존; 조회에서 명시 계약기한을 적용한다.
    assert!(!delivery.issues.iter().any(|i| i.starts_with("품목 2:")));
    assert_eq!(delivery.items[2].due.days_remaining, Some(0));
    assert_eq!(delivery.items[3].due.days_remaining, Some(30));
    assert!(
        report
            .deliveries
            .iter()
            .find(|d| d.number == "ENDED00")
            .unwrap()
            .terminated
    );
    f.sql(&format!(
        "UPDATE contract_item SET delivery_due='잘못된 기한' WHERE contract_base='{CONTRACT}' AND seq='10' AND line_no=2"
    ));
    let malformed = dashboard(&f);
    let item = malformed
        .deliveries
        .iter()
        .find(|d| d.number == number)
        .unwrap();
    assert!(!item.items[1].due.raw.is_empty());
    assert_eq!(item.items[1].due.date, None);
    // 잘못 적힌 품목 날짜는 계약기한으로 덮지 않는다.
    assert!(item.issues.iter().any(|i| i.starts_with("품목 2:")));
    set_override(&f, ("contract", CONTRACT, "10"), "납품기한", "기한 미확인");
    let invalid_report = dashboard(&f);
    let invalid = invalid_report
        .deliveries
        .iter()
        .find(|d| d.number == number)
        .unwrap();
    assert_eq!(invalid.due.date, None);
    assert_eq!(invalid.due.days_remaining, None);
    assert!(
        invalid
            .issues
            .iter()
            .any(|i| i.contains("날짜로 읽을 수 없습니다"))
    );
    assert!(invalid.issues.iter().any(|i| i.starts_with("품목 2:")));
    assert_eq!(invalid.items[0].due.days_remaining, Some(-1));
}

/// 다리가 싣는 꼴 — decimal 은 자리를 지킨 수, 빈 기한은 `null`, 이름은 camelCase.
#[test]
fn 다리_꼴은_금액의_자리를_지킨다() {
    let f = Fixture::new("reports-json");
    f.contract("TJSON", "00", &[("amount", "1")], &[]);
    // 열의 친화도가 `1.50` 을 수 1.5 로 바꿔 담으므로, 자리를 지킬 글은 덮개로 준다.
    set_override(&f, ("contract", "TJSON", "00"), "계약금액", "1.50");
    let json = pclm_core::storage::store::bridge_json(&dashboard(&f).to_json());
    assert!(
        json.contains("\"contractAmount\":1.50,\"contractsWithAmount\":1"),
        "{json}"
    );
    assert!(
        json.contains("\"due\":{\"raw\":\"\",\"date\":null,\"daysRemaining\":null}"),
        "{json}"
    );
    assert!(
        json.starts_with("{\"asOf\":\"2026-10-09T09:00:00.0000000\\u002B09:00\",\"totalCases\":1,"),
        "{json}"
    );
}

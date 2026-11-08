//! C# `PlanViewTests`(`v_계획` — 계획을 분모로 세운 뷰)와 `UnifiedLifecycleTests`(`v_통합` 이 생애주기 전부를 내는가).
//!
//! 계획 행은 `PlanImport` 를 거치지 않고 C# 시험처럼 `plan` 표에 바로 담는다. 값은 모두 지어낸 것이다.

mod common;

use std::collections::BTreeMap;

use common::store_fixture::Fixture;
use pclm_core::erp::explicit_links;
use pclm_core::storage::store::EntityRef;
use rusqlite::types::Value;
use rusqlite::{named_params, params};

/// C# 이 `decimal` 을 담는 꼴 — `0.0###…` 이라 뜻 없는 `.0` 이 붙는다.
const AMOUNT: &str = "164872340.0";

// ══ PlanViewTests ═══════════════════════════════════════════════

mod plan {
    use super::*;

    const TITLE: &str = "윈치 8종 구매";

    // ── 단계 네 갈래 ─────────────────────────────────────────

    /// `계획에만_있으면_미착수다`
    #[test]
    fn plan_only_is_not_started() {
        let f = Fixture::new("plan-only");
        plan(&f, "TSTPLA26500286");

        let row = single(rows(&f, "v_계획"));

        assert_eq!(row["단계"], "미착수");
        assert_eq!(row["접수번호"], "");
        assert_eq!(row["입찰공고번호"], "");
        assert_eq!(row["계약건수"], "0");
    }

    /// `접수까지_왔으면_접수다`
    #[test]
    fn reaching_request_is_request_stage() {
        let f = Fixture::new("plan-request");
        plan(&f, "TSTPLA26500286");
        request(&f, TITLE, "TSTPLA26500286", "전동 윈치", "8", "21288647.0");

        let row = single(rows(&f, "v_계획"));

        assert_eq!(row["단계"], "접수");
        assert_eq!(row["접수번호"], "TSTPLA26500286-000");
        assert_eq!(row["접수일자"], "2026/06/09");
        assert_eq!(row["입찰공고번호"], "");
    }

    /// `공고까지_이어졌으면_공고다`
    #[test]
    fn linked_notice_is_notice_stage() {
        let f = Fixture::new("plan-notice");
        plan(&f, "TSTPLA26500286");
        request(&f, TITLE, "TSTPLA26500286", "전동 윈치", "8", "21288647.0");
        notice(&f);
        link_request(&f);

        let row = single(rows(&f, "v_계획"));

        assert_eq!(row["단계"], "공고");
        assert_eq!(row["입찰공고번호"], "R26BK98000575-000");
        assert_eq!(row["게시일시"], "2026/07/18");

        // 개찰은 공고 본문이 아니라 일정 표에서 온다 — v_공고 와 같은 자리를 짚는다.
        assert_eq!(row["개찰일시"], "2026/07/25 11:00:00");

        assert_eq!(row["계약건수"], "0");
        assert_eq!(row["계약번호"], "");
    }

    /// `계약이_달렸으면_계약이다`
    #[test]
    fn attached_contract_is_contract_stage() {
        let f = Fixture::new("plan-contract");
        all_three(&f);

        let row = single(rows(&f, "v_계획"));

        assert_eq!(row["단계"], "계약");
        assert_eq!(row["계약건수"], "1");
        assert_eq!(row["계약번호"], "R26TA9800070700");
        assert_eq!(row["계약일자"], "2026/07/21");
        assert_eq!(row["계약금액"], "164,872,340");
    }

    // ── 줄이 갈라지지 않는다 ─────────────────────────────────

    /// `계약이_여럿이어도_줄이_갈라지지_않는다`
    #[test]
    fn several_contracts_do_not_split_the_row() {
        let f = Fixture::new("plan-split");
        plan(&f, "TSTPLA26500286");
        two_awards(&f);

        let row = single(rows(&f, "v_계획"));

        assert_eq!(row["단계"], "계약");
        assert_eq!(row["계약건수"], "2");
    }

    /// `계약이_둘_이상이면_계약_세_칸을_비운다`
    #[test]
    fn two_or_more_contracts_empty_the_three_contract_cells() {
        let f = Fixture::new("plan-blank");
        plan(&f, "TSTPLA26500286");
        two_awards(&f);

        let row = single(rows(&f, "v_계획"));

        assert_eq!(row["계약건수"], "2");
        assert_eq!(row["계약번호"], "");
        assert_eq!(row["계약일자"], "");
        assert_eq!(row["계약금액"], "");
    }

    // ── 분모의 경계 ──────────────────────────────────────────

    /// `계획에_없는_조달요구번호의_접수는_서지_않는다`
    #[test]
    fn request_outside_the_plan_does_not_stand() {
        let f = Fixture::new("plan-outside");
        plan(&f, "TSTPLA26500286");
        request(&f, TITLE, "TSTPLA26500286", "전동 윈치", "8", "21288647.0");

        // 계획에 없는 번호로 접수 하나를 더 넣는다.
        request(&f, TITLE, "TSTPLA26500599", "전동 윈치", "8", "21288647.0");

        let row = single(rows(&f, "v_계획"));
        assert_eq!(row["조달요구번호"], "TSTPLA26500286");
    }

    /// `계획_행마다_한_줄이다`
    #[test]
    fn one_row_per_plan_row() {
        let f = Fixture::new("plan-rows");
        plan(&f, "TSTPLA26500286");
        plan(&f, "TSTPLA26500287");
        plan(&f, "TSTPLA26500288");

        assert_eq!(rows(&f, "v_계획").len(), 3);
    }

    // ── 값 ───────────────────────────────────────────────────

    /// `담당자가_비면_NULL_이_아니라_빈_문자열이다`
    #[test]
    fn missing_officer_is_empty_text_not_null() {
        let f = Fixture::new("plan-null");
        plan_with(&f, "TSTPLA26500286", None, None);

        let row = single(rows(&f, "v_계획"));
        assert_eq!(row["담당자"], "");
        assert_eq!(row["요청부대부서명"], "");

        assert_no_null(&f, "v_계획");
    }

    /// `계획의_제_칸을_그대로_낸다`
    #[test]
    fn plan_columns_come_through_as_written() {
        let f = Fixture::new("plan-columns");
        plan(&f, "TSTPLA26500286");

        let row = single(rows(&f, "v_계획"));

        assert_eq!(row["재고번호"], "2590-01-234-5678");
        assert_eq!(row["품명"], "전동 윈치");
        assert_eq!(row["화폐구분"], "KRW");
        assert_eq!(row["단위"], "대");
        assert_eq!(row["지시수량"], "8");
        assert_eq!(row["요청부대부서명"], "한별군수지원단 보급창");
        assert_eq!(row["요청부대담당자"], "홍길동");
        assert_eq!(row["요청부대사용자전화번호"], "051-000-0000");
        assert_eq!(row["계약부서"], "계약1과");
        assert_eq!(row["담당자"], "김철수");
        assert_eq!(row["연락처"], "02-000-0000");
    }

    /// `다른_탭에서_고친_값이_계획_탭에도_비친다`
    #[test]
    fn values_corrected_on_other_tabs_show_on_the_plan_tab() {
        let f = Fixture::new("plan-overrides");
        all_three(&f);

        let store = f.store();
        store
            .set_override(
                &EntityRef::new("contract", "R26TA98000707", "00"),
                "계약금액",
                "170,000,000",
            )
            .unwrap();
        store
            .set_override(
                &EntityRef::new("notice", "R26BK98000575", "000"),
                "게시일시",
                "2026/05/28",
            )
            .unwrap();
        store
            .set_override(
                &EntityRef::new("request", "TSTPLA26500286", "000"),
                "접수일자",
                "2026/04/29",
            )
            .unwrap();

        let row = single(rows(&f, "v_계획"));

        assert_eq!(row["계약금액"], "170,000,000");
        assert_eq!(row["게시일시"], "2026/05/28");
        assert_eq!(row["접수일자"], "2026/04/29");
    }

    /// `빈_DB에서도_돈다`
    #[test]
    fn runs_on_an_empty_file() {
        let f = Fixture::new("plan-empty");
        assert!(rows(&f, "v_계획").is_empty());
    }

    // ── 짓기 ─────────────────────────────────────────────────

    fn all_three(f: &Fixture) {
        plan(f, "TSTPLA26500286");
        request(f, TITLE, "TSTPLA26500286", "전동 윈치", "8", "21288647.0");
        notice(f);
        contract(f, TITLE, "R26TA98000707");
        link_request(f);
        link(f, "R26TA98000707");
    }

    /// 한 공고에 계약 둘. 분할 낙찰·수요기관 복수에서 실제로 생기는 모양이다.
    fn two_awards(f: &Fixture) {
        request(f, TITLE, "TSTPLA26500286", "전동 윈치", "8", "21288647.0");
        notice(f);
        contract(f, TITLE, "R26TA98000707");
        contract(f, TITLE, "R26TA98000708");
        link_request(f);
        link(f, "R26TA98000707");
        link(f, "R26TA98000708");
    }

    fn plan(f: &Fixture, number: &str) {
        plan_with(f, number, Some("김철수"), Some("한별군수지원단 보급창"));
    }

    /// 계획 한 줄. 엑셀을 지어 읽히는 것은 `PlanImportTests` 의 몫이라 바로 담는다.
    fn plan_with(f: &Fixture, number: &str, officer: Option<&str>, requesting_unit: Option<&str>) {
        f.conn()
            .execute(
                "INSERT INTO plan (
                    request_number, stock_number, item_name, currency, unit, quantity,
                    requesting_unit, requesting_officer, requesting_phone,
                    contract_department, officer, contact, source_name, imported_at
                ) VALUES (
                    ?1, '2590-01-234-5678', '전동 윈치', 'KRW', '대', 8,
                    ?2, '홍길동', '051-000-0000',
                    '계약1과', ?3, '02-000-0000', '계획.xlsx', '2026-02-14T00:00:00Z'
                );",
                params![number, requesting_unit, officer],
            )
            .unwrap();
    }

    /// 공고 하나와 그 개찰 일정(C# `NoticeRecord.Schedule`).
    fn notice(f: &Fixture) {
        f.notice(
            "R26BK98000575",
            "000",
            &[("title", TITLE), ("posted_at", "2026-07-18 00:00:00")],
            &[],
            &[],
        );
        f.sql(
            "INSERT INTO notice_schedule (notice_base, seq, line_no, name, starts_at)
             VALUES ('R26BK98000575', '000', 1, '개찰', '2026-07-25 11:00:00');",
        );
    }
}

// ══ UnifiedLifecycleTests ═══════════════════════════════════════

mod unified {
    use super::*;

    const TITLE: &str = "수질측정기 구매";

    /// `접수만_들어온_것도_한_줄로_선다`
    #[test]
    fn request_alone_stands_as_one_row() {
        let f = Fixture::new("unified-request");
        req(&f, "TSTPLA26500286");

        let row = single(rows(&f, "v_통합"));

        assert_eq!(row["접수번호"], "TSTPLA26500286-000");
        assert_eq!(row["입찰공고번호"], "");
        assert_eq!(row["계약번호"], "");
    }

    /// `공고까지만_온_것도_한_줄로_선다`
    #[test]
    fn up_to_notice_stands_as_one_row() {
        let f = Fixture::new("unified-notice");
        req(&f, "TSTPLA26500286");
        notice(&f, "000", "R26BK98000575", &[]);
        link_request(&f);

        let row = single(rows(&f, "v_통합"));

        assert_eq!(row["접수번호"], "TSTPLA26500286-000");
        assert_eq!(row["입찰공고번호"], "R26BK98000575-000");
        assert_eq!(row["계약번호"], "");
    }

    /// `셋이_다_온_것은_한_줄이다`
    #[test]
    fn all_three_make_one_row() {
        let f = Fixture::new("unified-three");
        all_three(&f);

        let row = single(rows(&f, "v_통합"));

        assert_eq!(row["접수번호"], "TSTPLA26500286-000");
        assert_eq!(row["입찰공고번호"], "R26BK98000575-000");
        assert_eq!(row["계약번호"], "R26TA9800070700");
        assert_eq!(row["계약금액"], "164,872,340");
    }

    /// `이어지지_않은_계약도_한_줄로_선다`
    #[test]
    fn unlinked_contract_stands_as_its_own_row() {
        let f = Fixture::new("unified-lone");
        notice(&f, "000", "R26BK98000575", &[]);
        contract(&f, TITLE, "R26TA98000707");

        let rows = rows(&f, "v_통합");
        assert_eq!(rows.len(), 2);

        let lone = single(
            rows.iter()
                .filter(|r| !r["계약번호"].is_empty())
                .cloned()
                .collect(),
        );
        assert_eq!(lone["입찰공고번호"], "");

        let notice_only = single(
            rows.iter()
                .filter(|r| r["계약번호"].is_empty())
                .cloned()
                .collect(),
        );
        assert_eq!(notice_only["입찰공고번호"], "R26BK98000575-000");
    }

    /// `계약이_여럿이면_계약마다_한_줄이다`
    #[test]
    fn several_contracts_make_one_row_each() {
        let f = Fixture::new("unified-several");
        notice(&f, "000", "R26BK98000575", &[]);
        contract(&f, TITLE, "R26TA98000707");
        contract(&f, TITLE, "R26TA98000708");
        link(&f, "R26TA98000707");
        link(&f, "R26TA98000708");

        let rows = rows(&f, "v_통합");

        assert_eq!(rows.len(), 2);
        assert!(
            rows.iter()
                .all(|r| r["입찰공고번호"] == "R26BK98000575-000")
        );
        let mut numbers: Vec<&str> = rows.iter().map(|r| r["계약번호"].as_str()).collect();
        numbers.sort_unstable();
        assert_eq!(numbers, ["R26TA9800070700", "R26TA9800070800"]);
    }

    /// `빈_칸은_NULL_이_아니라_빈_문자열이다`
    #[test]
    fn empty_cells_are_empty_text_not_null() {
        let f = Fixture::new("unified-null");
        req(&f, "TSTPLA26500286");

        assert_no_null(&f, "v_통합");
    }

    /// `사람이_세운_계약_열도_함께_온다`
    #[test]
    fn user_contract_columns_come_along() {
        let f = Fixture::new("unified-user");
        all_three(&f);
        f.store()
            .set_user_field(
                &EntityRef::new("contract", "R26TA98000707", "00"),
                "진행상태",
                Some("납품"),
            )
            .unwrap();
        req(&f, "TSTPLA26500300");

        let rows = rows(&f, "v_통합");

        let with = single(
            rows.iter()
                .filter(|r| !r["계약번호"].is_empty())
                .cloned()
                .collect(),
        );
        assert_eq!(with["진행상태"], "납품");
        let without = single(
            rows.iter()
                .filter(|r| r["계약번호"].is_empty())
                .cloned()
                .collect(),
        );
        assert_eq!(without["진행상태"], "");
    }

    /// `빈_DB에서도_돈다`
    #[test]
    fn runs_on_an_empty_file() {
        let f = Fixture::new("unified-empty");
        assert!(rows(&f, "v_통합").is_empty());
    }

    /// `구조와_표가_같은_수를_센다` — 사슬 하나가 계약 여럿을 품으면 표는 계약마다 한 줄이 되므로, 계약을 펴서 센다. 세는
    /// 것부터 견준다 — 붙박이가 늘어 절대값이 어긋난 것과 두 화면이 갈린 것을 실패 한 줄로 구별할 수 있어야 한다.
    #[test]
    fn structure_and_table_count_the_same() {
        let f = Fixture::new("unified-structure");
        all_three(&f);
        request(&f, TITLE, "TSTPLA26500300", "수질검사장치", "1", AMOUNT);
        notice(&f, "000", "R26BK98000576", &[]);
        contract(&f, TITLE, "R26TA98000709");
        cancelled_and_reissued(&f);

        let chains = pclm_core::storage::outline::Outline::new(f.database())
            .build()
            .unwrap()
            .chains;
        let spread: usize = chains.iter().map(|c| c.contracts.len().max(1)).sum();

        assert_eq!(spread, rows(&f, "v_통합").len());
        assert_eq!(chains.len(), 5);
    }

    /// C# `취소되고_재공고된_한_벌` — 본번호는 갈리지만 조달 건은 하나라 접수도 계약도 하나이고 표도 한 줄만 낸다. 사람이
    /// 이어 둔 것은 나중에 취소된 원공고다. 붙박이에 이것이 없으면 이 시험은 합쳐진 건을 한 번도 보지 못한다.
    fn cancelled_and_reissued(f: &Fixture) {
        notice(f, "000", "R26BK98000154", &[]);
        notice(f, "001", "R26BK98000154", &["R26BK98000154-000"]);
        notice(
            f,
            "000",
            "R26BK98000282",
            &["R26BK98000154-001", "R26BK98000154-000"],
        );
        req(f, "MBKLMH26930006");
        contract(f, TITLE, "R26TA98000669");
        reference_contract(f, "R26TA98000669", "R26BK98000154");
        reference_request(f, "MBKLMH26930006", "000", "R26BK98000154");
    }

    fn all_three(f: &Fixture) {
        req(f, "TSTPLA26500286");
        notice(f, "000", "R26BK98000575", &[]);
        contract(f, TITLE, "R26TA98000707");
        link(f, "R26TA98000707");
        link_request(f);
    }

    fn req(f: &Fixture, base: &str) {
        request(f, TITLE, base, "수질검사장치", "1", AMOUNT);
    }

    /// `related` 는 공고서의 「관련공고」 칸 — 취소·재공고로 갈린 본번호를 잇는 단서다(ADR-025).
    fn notice(f: &Fixture, seq: &str, base: &str, related: &[&str]) {
        let kind = if seq == "001" {
            "취소공고"
        } else {
            "등록공고"
        };
        f.notice(
            base,
            seq,
            &[
                ("title", TITLE),
                ("notice_kind", kind),
                ("posted_at", "2026-07-18 00:00:00"),
            ],
            &[],
            related,
        );
    }
}

// ── 함께 쓰는 짓기 ───────────────────────────────────────────────

/// C# `RequestRecord` — 품목 한 줄.
fn request(f: &Fixture, title: &str, base: &str, item: &str, quantity: &str, unit_price: &str) {
    f.request(
        base,
        "000",
        &[("title", title), ("received_on", "2026-06-09 00:00:00")],
        &[&[
            ("line_no", "1"),
            ("request_number", base),
            ("item_name", item),
            ("quantity", quantity),
            ("unit_price", unit_price),
        ]],
    );
}

/// C# `ContractRecord`.
fn contract(f: &Fixture, title: &str, base: &str) {
    f.contract(
        base,
        "00",
        &[
            ("title", title),
            ("contracted_on", "2026-07-21 00:00:00"),
            ("amount", AMOUNT),
        ],
        &[],
    );
}

fn link(f: &Fixture, contract_base: &str) {
    reference_contract(f, contract_base, "R26BK98000575");
}

fn link_request(f: &Fixture) {
    reference_request(f, "TSTPLA26500286", "000", "R26BK98000575");
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

// ── 읽기 ─────────────────────────────────────────────────────────

fn text(v: Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::Integer(i) => i.to_string(),
        Value::Real(r) => r.to_string(),
        Value::Text(t) => t,
        Value::Blob(_) => "System.Byte[]".into(),
    }
}

/// 뷰의 모든 줄을 열 이름 → 글로(NULL 은 빈 글 — C# `c.Value?.ToString() ?? ""`).
fn rows(f: &Fixture, view: &str) -> Vec<BTreeMap<String, String>> {
    let c = f.database().open_read_only().unwrap();
    let mut s = c.prepare(&format!("SELECT * FROM {view};")).unwrap();
    let names: Vec<String> = s.column_names().map(str::to_string).collect();
    s.query_map([], |r| {
        let mut row = BTreeMap::new();
        for (i, name) in names.iter().enumerate() {
            row.insert(name.clone(), text(r.get::<_, Value>(i)?));
        }
        Ok(row)
    })
    .unwrap()
    .map(|r| r.unwrap())
    .collect()
}

/// `Assert.Single`.
fn single(mut rows: Vec<BTreeMap<String, String>>) -> BTreeMap<String, String> {
    assert_eq!(rows.len(), 1, "줄이 하나가 아니다");
    rows.pop().unwrap()
}

/// 첫 줄의 어느 칸도 NULL 이 아니다 — 빈 값이 NULL 로 새면 받는 쪽의 빈 값 경고가 죽는다.
fn assert_no_null(f: &Fixture, view: &str) {
    let c = f.database().open_read_only().unwrap();
    let mut s = c.prepare(&format!("SELECT * FROM {view};")).unwrap();
    let names: Vec<String> = s.column_names().map(str::to_string).collect();
    let mut q = s.query([]).unwrap();
    let row = q.next().unwrap().expect("줄이 하나는 있어야 한다");
    for (i, name) in names.iter().enumerate() {
        let v: Value = row.get(i).unwrap();
        assert!(v != Value::Null, "{name} 가 NULL 입니다.");
    }
}

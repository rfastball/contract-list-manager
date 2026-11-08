//! C# `NoticeGroupTests` — 관련공고를 타고 갈린 본번호를 한 건으로 묶는다(스키마 V15). `NoticeRelationTests` 의 차수 몫도
//! 여기 둔다. 번호 모양은 실측 코퍼스의 것이지만 담긴 값은 모두 지어낸 것이다.

mod common;

use std::collections::BTreeMap;

use common::store_fixture::Fixture;
use pclm_core::erp::explicit_links;
use pclm_core::storage::outline::{Outline, OutlineChain};
use pclm_core::storage::store::EntityRef;
use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, named_params};

/// C# 이 `DateTime` 을 담는 꼴(Microsoft.Data.Sqlite) — `yyyy-MM-dd HH:mm:ss`.
const POSTED: &str = "2026-07-18 00:00:00";

/// C# 이 `decimal` 을 담는 꼴 — `0.0###…` 이라 뜻 없는 `.0` 이 붙는다.
const AMOUNT: &str = "164872340.0";

// ── 묶기 ─────────────────────────────────────────────────────

/// `재공고는_원공고와_한_건이_된다`
#[test]
fn reissued_notice_joins_the_original_case() {
    let f = Fixture::new("ng-reissue");
    put(&f, "R26BK98000154", "000", &[]);
    put(&f, "R26BK98000154", "001", &["R26BK98000154-000"]);
    put(
        &f,
        "R26BK98000282",
        "000",
        &["R26BK98000154-001", "R26BK98000154-000"],
    );

    assert_eq!(groups(&f), ["R26BK98000154"]);
    assert_eq!(group(&f, "R26BK98000154"), "R26BK98000154");
    assert_eq!(group(&f, "R26BK98000282"), "R26BK98000154");

    // 계열 둘이 각각 줄을 갖되(ADR-025), 두 줄 다 같은 건에 서고 현행은 재공고다.
    let rows = rows(&f);
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|r| r["공고건"] == "R26BK98000154"));
    assert!(rows.iter().all(|r| r["현행공고"] == "R26BK98000282-000"));
}

/// `여러_현행_후보중_재공고_하나만_고른다` (Theory) — 뷰와 `Outline` 이 같은 현행을 고른다.
#[test]
fn only_a_single_reissue_among_current_candidates_is_chosen() {
    let cases: &[(Option<&str>, Option<&str>, bool)] = &[
        (Some("재공고"), None, true),
        (Some("실공고(재공고)"), None, true),
        (Some("실공고"), Some("재공고"), true),
        (Some("재공고"), Some("일반공고"), true),
        (Some("취소공고"), Some("재공고"), false),
        (Some(""), Some("재공고"), true),
        (None, Some("재공고"), true),
        (Some("등록공고"), Some("재공고"), false),
        (Some("재공고 아님"), None, false),
        (Some("실공고(재공고예정)"), None, false),
        (Some("등록공고"), None, false),
    ];

    for &(kind, status, selected) in cases {
        let f = Fixture::new("ng-candidates");
        put(&f, "R26BK00000001", "000", &[]);
        let fields: Vec<(&str, &str)> = kind.map(|k| ("notice_kind", k)).into_iter().collect();
        f.notice("R26BK00000002", "000", &fields, &[], &[]);
        {
            let c = f.conn();
            c.execute(
                "UPDATE notice SET source_status=?1 WHERE notice_base='R26BK00000002'",
                [status],
            )
            .unwrap();
            c.execute_batch(
                "INSERT INTO notice_item(notice_base,seq,line_no,ref_request_base)
                 VALUES('R26BK00000001','000',1,'REQ'),('R26BK00000002','000',1,'REQ');",
            )
            .unwrap();
            explicit_links::resolve(&c).unwrap();
        }

        let expected = if selected { "R26BK00000002-000" } else { "" };
        for row in rows(&f) {
            assert_eq!(row["현행공고"], expected, "kind={kind:?} status={status:?}");
        }
        let current = only_chain(&f)
            .notice
            .and_then(|n| n.current)
            .map(|c| c.number);
        assert_eq!(
            current.as_deref(),
            selected.then_some(expected),
            "kind={kind:?} status={status:?}"
        );
    }
}

/// `재공고_우선보다_최신차수와_명시_대체관계가_앞선다`
#[test]
fn latest_round_and_explicit_replacement_come_before_reissue_preference() {
    let f = Fixture::new("ng-precedence");
    f.notice(
        "R26BK00000001",
        "000",
        &[("notice_kind", "재공고")],
        &[],
        &[],
    );
    put(&f, "R26BK00000001", "001", &[]);
    put(&f, "R26BK00000002", "000", &["R26BK00000001-000"]);

    // 재공고인 옛 차수를 후보에 다시 넣지 않는다. 최신 차수 둘은 여전히 모호하다.
    assert!(rows(&f).iter().all(|r| r["현행공고"].is_empty()));

    // C# 은 레코드 전체를 다시 쓴다 — 이 차수의 제목·게시일시는 NULL 로 돌아간다.
    f.sql("UPDATE notice SET title=NULL, posted_at=NULL WHERE notice_base='R26BK00000001' AND seq='001';");
    f.notice(
        "R26BK00000001",
        "001",
        &[("notice_kind", "재공고")],
        &[],
        &[],
    );
    assert!(
        rows(&f)
            .iter()
            .all(|r| r["현행공고"] == "R26BK00000001-001")
    );

    // 명시적으로 대체된 재공고는 빠지고, 남은 일반공고 하나를 그대로 현행으로 삼는다.
    put(&f, "R26BK00000002", "000", &["R26BK00000001-001"]);
    assert!(
        rows(&f)
            .iter()
            .all(|r| r["현행공고"] == "R26BK00000002-000")
    );
}

/// `재출력본의_앞가리킴은_최신_차수를_대체하지_않는다`
#[test]
fn reprint_pointing_forward_does_not_replace_the_latest_round() {
    let f = Fixture::new("ng-reprint");
    put(&f, "R26BK98000530", "000", &["R26BK98000530-001"]);
    put(&f, "R26BK98000530", "001", &["R26BK98000530-000"]);

    assert_eq!(groups(&f), ["R26BK98000530"]);
    let rows = rows(&f);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["현행공고"], "R26BK98000530-001");
}

/// `문서가_없는_조상도_건의_이름이_된다`
#[test]
fn ancestor_without_document_names_the_case() {
    let f = Fixture::new("ng-ancestor");
    put(&f, "R26BK98000491", "000", &["R26BK98000319-000"]);

    assert_eq!(groups(&f), ["R26BK98000319"]);
    assert_eq!(group(&f, "R26BK98000491"), "R26BK98000319");

    // 이름은 없는 문서의 것이라도, 보이는 것은 서 있는 공고다.
    let rows = rows(&f);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["현행공고"], "R26BK98000491-000");
}

/// `합쳐진_건은_사슬_하나로_서고_현행이_본문에_선다` — 건의 이름이 갈리면 접수·공고·계약이 사슬 여럿으로 흩어진다. 자료는
/// 멀쩡하고 갈린 것은 조회하는 키뿐이었다.
#[test]
fn merged_case_stands_as_one_chain_with_the_current_notice_in_front() {
    let f = Fixture::new("ng-outline-merged");
    put(&f, "R26BK98000154", "000", &[]);
    put(&f, "R26BK98000154", "001", &["R26BK98000154-000"]);
    put(
        &f,
        "R26BK98000282",
        "000",
        &["R26BK98000154-001", "R26BK98000154-000"],
    );
    link_request_and_contract(&f, "R26BK98000154");

    let chain = only_chain(&f);
    let notice = chain.notice.as_ref().unwrap();
    // 본문에 서는 것은 살아 있는 재공고다. 취소된 원공고는 대체된 것으로 물러난다.
    assert_eq!(notice.current.as_ref().unwrap().number, "R26BK98000282-000");
    let superseded: Vec<&str> = notice
        .superseded
        .iter()
        .map(|n| n.number.as_str())
        .collect();
    assert_eq!(superseded, ["R26BK98000154-001"]);
    // 접수와 계약은 원공고에 이어 두었는데도 그 사슬에 그대로 붙는다.
    assert_eq!(chain.request.as_ref().unwrap().number, "MBKLMH26930006-000");
    let contracts: Vec<&str> = chain.contracts.iter().map(|c| c.number.as_str()).collect();
    assert_eq!(contracts, ["R26TA9800066900"]);
    // 사슬의 이름은 건 이름이라, 재공고가 한 번 더 나가도 흔들리지 않는다.
    assert_eq!(chain.key, "MBKLMH26930006/R26BK98000154");
}

/// `문서_없는_조상이_건_이름이어도_사슬이_흩어지지_않는다` — 재공고 건에서는 가리키는 조상이 DB 에 없는 것이 보통이다.
#[test]
fn ancestor_without_document_does_not_scatter_the_chain() {
    let f = Fixture::new("ng-outline-ancestor");
    put(&f, "R26BK98000491", "000", &["R26BK98000319-000"]);
    link_request_and_contract(&f, "R26BK98000491");

    let chain = only_chain(&f);
    let notice = chain.notice.as_ref().unwrap();
    assert_eq!(notice.current.as_ref().unwrap().number, "R26BK98000491-000");
    assert_eq!(chain.request.as_ref().unwrap().number, "MBKLMH26930006-000");
    let contracts: Vec<&str> = chain.contracts.iter().map(|c| c.number.as_str()).collect();
    assert_eq!(contracts, ["R26TA9800066900"]);
    // 이름은 들어온 적 없는 번호다. 그래도 사슬은 하나다.
    assert_eq!(chain.key, "MBKLMH26930006/R26BK98000319");
    assert_eq!(notice.group_base, "R26BK98000319");
}

/// `대체된_공고는_늦은_것부터_선다` — 방금 지나간 것이 맨 앞이라야 사람이 찾는 차례와 같다.
#[test]
fn superseded_notices_stand_latest_first() {
    let f = Fixture::new("ng-outline-superseded");
    put_at(&f, "R26BK98000154", "000", "2026-07-07 00:00:00", &[]);
    put_at(
        &f,
        "R26BK98000282",
        "000",
        "2026-07-17 00:00:00",
        &["R26BK98000154-000"],
    );
    put_at(
        &f,
        "R26BK98000491",
        "000",
        "2026-07-25 00:00:00",
        &["R26BK98000282-000"],
    );

    let chain = only_chain(&f);
    let notice = chain.notice.as_ref().unwrap();
    assert_eq!(notice.current.as_ref().unwrap().number, "R26BK98000491-000");
    let superseded: Vec<&str> = notice
        .superseded
        .iter()
        .map(|n| n.number.as_str())
        .collect();
    assert_eq!(superseded, ["R26BK98000282-000", "R26BK98000154-000"]);
}

/// C# `NoticeRelationTests.차수마다_따로_매달린다` — 나중에 넣은 문서가 앞 차수의 관련공고를 조용히 덮었을 자리다.
#[test]
fn related_notices_hang_on_each_round() {
    let f = Fixture::new("ng-relation-rounds");
    put(&f, "R26BK98000530", "000", &["R26BK98000530-001"]);
    put(&f, "R26BK98000530", "001", &["R26BK98000530-000"]);

    assert_eq!(related(&f, "R26BK98000530", "000"), ["R26BK98000530-001"]);
    assert_eq!(related(&f, "R26BK98000530", "001"), ["R26BK98000530-000"]);

    // 변경분을 다시 넣는다 — 이번에는 관련공고가 지워진 판이다.
    put(&f, "R26BK98000530", "001", &[]);

    assert!(related(&f, "R26BK98000530", "001").is_empty());
    assert_eq!(related(&f, "R26BK98000530", "000"), ["R26BK98000530-001"]);
}

/// `재공고를_먼저_넣어도_건_이름이_흔들리지_않는다`
#[test]
fn inserting_the_reissue_first_does_not_move_the_case_name() {
    let f = Fixture::new("ng-order");
    put(
        &f,
        "R26BK98000282",
        "000",
        &["R26BK98000154-001", "R26BK98000154-000"],
    );
    assert_eq!(group(&f, "R26BK98000282"), "R26BK98000154");

    put(&f, "R26BK98000154", "000", &[]);
    put(&f, "R26BK98000154", "001", &["R26BK98000154-000"]);

    assert_eq!(groups(&f), ["R26BK98000154"]);
    assert_eq!(group(&f, "R26BK98000282"), "R26BK98000154");
}

/// `공고를_넣으면_계열과_건이_함께_선다`
#[test]
fn inserting_a_notice_raises_series_and_case() {
    let f = Fixture::new("ng-series");
    put(&f, "R26BK98000154", "000", &[]);

    assert_eq!(count(&f, "SELECT COUNT(*) FROM notice_series;"), 1);
    assert_eq!(group(&f, "R26BK98000154"), "R26BK98000154");
    assert_eq!(groups(&f), ["R26BK98000154"]);
}

// ── 한 규칙, 한 벌 ───────────────────────────────────────────

/// `뷰의_현행공고가_정본과_같다` — 뷰 쪽 단언. 정본(`notice_groups::current_notice`, `pub(crate)`)을 직접 도는 앞 단언은
/// 그 모듈의 단위 시험(`view_current_notice_is_the_canonical_rule`)이 같은 자료·같은 기대값으로 본다.
#[test]
fn view_current_notice_matches_the_canonical_rule() {
    let f = Fixture::new("ng-canonical");
    // 조립대 — 당초 → 취소 → 재채번된 재공고.
    put(&f, "R26BK98000154", "000", &[]);
    put(&f, "R26BK98000154", "001", &["R26BK98000154-000"]);
    put(
        &f,
        "R26BK98000282",
        "000",
        &["R26BK98000154-001", "R26BK98000154-000"],
    );

    // 절단기 — 재출력본이 자기 뒤차수를 가리킨다.
    put(&f, "R26BK98000530", "000", &["R26BK98000530-001"]);
    put(&f, "R26BK98000530", "001", &["R26BK98000530-000"]);

    // 가스성분분석기 — 가리키는 조상이 DB 에 없다.
    put(&f, "R26BK98000491", "000", &["R26BK98000319-000"]);

    let expected = [
        "R26BK98000154|R26BK98000282-000",
        "R26BK98000319|R26BK98000491-000",
        "R26BK98000530|R26BK98000530-001",
    ];

    assert_eq!(
        read(
            &f,
            "SELECT DISTINCT 공고건, 현행공고 FROM v_공고 ORDER BY 공고건;"
        ),
        expected
    );
}

// ── 차수를 편 표 ─────────────────────────────────────────────

/// `통합_v3_은_공고_한_장마다_한_줄이다`
#[test]
fn unified_rounds_have_one_row_per_notice_document() {
    let f = Fixture::new("ng-rounds");
    put(&f, "R26BK98000154", "000", &[]);
    put(&f, "R26BK98000154", "001", &["R26BK98000154-000"]);
    put(
        &f,
        "R26BK98000282",
        "000",
        &["R26BK98000154-001", "R26BK98000154-000"],
    );

    put(&f, "R26BK98000530", "000", &["R26BK98000530-001"]);
    put(&f, "R26BK98000530", "001", &["R26BK98000530-000"]);

    put(&f, "R26BK98000575", "000", &[]);

    assert_eq!(
        read(
            &f,
            "SELECT 공고건, COUNT(*) FROM v_통합차수 GROUP BY 공고건 ORDER BY 공고건;"
        ),
        ["R26BK98000154|3", "R26BK98000530|2", "R26BK98000575|1"]
    );

    assert_eq!(count(&f, "SELECT COUNT(*) FROM v_통합차수;"), 6);
    assert_eq!(count(&f, "SELECT COUNT(*) FROM v_통합;"), 3);
    assert_eq!(count(&f, "SELECT COUNT(*) FROM notice_group;"), 3);
}

/// `통합_v3_에서_접수와_계약이_공고_줄마다_되풀이된다`
#[test]
fn unified_rounds_repeat_request_and_contract_on_every_notice_row() {
    let f = Fixture::new("ng-repeat");
    put(&f, "R26BK98000154", "000", &[]);
    put(&f, "R26BK98000154", "001", &["R26BK98000154-000"]);
    put(
        &f,
        "R26BK98000282",
        "000",
        &["R26BK98000154-001", "R26BK98000154-000"],
    );

    request(&f, "MBKLMH26930006");
    contract(&f, "R26TA98000669");

    reference_request(&f, "MBKLMH26930006", "000", "R26BK98000154");
    reference_contract(&f, "R26TA98000669", "R26BK98000154");

    assert_eq!(
        read(
            &f,
            "SELECT 입찰공고번호, 접수번호, 계약번호, 현행공고
             FROM v_통합차수 ORDER BY 입찰공고번호;"
        ),
        [
            "R26BK98000154-000|MBKLMH26930006-000|R26TA9800066900|R26BK98000282-000",
            "R26BK98000154-001|MBKLMH26930006-000|R26TA9800066900|R26BK98000282-000",
            "R26BK98000282-000|MBKLMH26930006-000|R26TA9800066900|R26BK98000282-000",
        ]
    );

    // 줄 하나가 조달 건인 통합은 그대로 한 줄이다.
    assert_eq!(count(&f, "SELECT COUNT(*) FROM v_통합;"), 1);
}

/// `통합_v3_도_공고_없는_접수와_홀로_선_계약을_낸다`
#[test]
fn unified_rounds_show_requests_without_notice_and_lone_contracts() {
    let f = Fixture::new("ng-lone");
    request(&f, "MBKLMH26930006");
    contract(&f, "R26TA98000669");

    assert_eq!(
        read(
            &f,
            "SELECT 입찰공고번호, 접수번호, 계약번호
             FROM v_통합차수 ORDER BY 접수번호 DESC;"
        ),
        ["|MBKLMH26930006-000|", "||R26TA9800066900"]
    );
}

// ── 다시 지어도 잃지 않는다 ──────────────────────────────────

/// `건이_다시_지어져도_확정한_링크가_살아남는다`
#[test]
fn confirmed_link_survives_case_rebuild() {
    let f = Fixture::new("ng-link-survives");
    put(&f, "R26BK98000154", "000", &[]);
    contract(&f, "R26TA98000669");

    reference_contract(&f, "R26TA98000669", "R26BK98000154");

    // 재공고가 들어오며 건이 합쳐진다.
    put(&f, "R26BK98000282", "000", &["R26BK98000154-000"]);

    assert_eq!(
        read(&f, "SELECT contract_base, notice_group FROM project_link;"),
        ["R26TA98000669|R26BK98000154"]
    );
}

/// `사람이_적은_값이_건_재계산과_삭제에서_살아남는다`
#[test]
fn user_values_survive_case_rebuild_and_deletion() {
    let f = Fixture::new("ng-user-values");
    put(&f, "R26BK98000154", "000", &[]);
    f.store()
        .set_user_field(
            &EntityRef::new("notice", "R26BK98000154", "000"),
            "메모",
            Some("지어낸 메모"),
        )
        .unwrap();

    put(&f, "R26BK98000282", "000", &["R26BK98000154-000"]);
    assert_eq!(
        read(&f, "SELECT value FROM notice_user_field;"),
        ["지어낸 메모"]
    );

    f.store()
        .delete(&EntityRef::new("notice", "R26BK98000282", "000"), true)
        .unwrap();

    assert_eq!(
        read(&f, "SELECT value FROM notice_user_field;"),
        ["지어낸 메모"]
    );
}

// ── 밀어낸 것은 알린다 ───────────────────────────────────────

/// `재공고를_지우면_건이_갈리고_링크는_원공고에_남는다`
#[test]
fn deleting_the_reissue_splits_the_case_and_link_stays_on_the_original() {
    let f = Fixture::new("ng-delete-reissue");
    put(&f, "R26BK98000154", "000", &[]);
    put(&f, "R26BK98000154", "001", &["R26BK98000154-000"]);
    put(
        &f,
        "R26BK98000282",
        "000",
        &["R26BK98000154-001", "R26BK98000154-000"],
    );

    contract(&f, "R26TA98000669");
    reference_contract(&f, "R26TA98000669", "R26BK98000154");

    f.store()
        .delete(&EntityRef::new("notice", "R26BK98000282", "000"), true)
        .unwrap();

    assert_eq!(groups(&f), ["R26BK98000154"]);
    assert_eq!(
        read(&f, "SELECT notice_base FROM notice_series;"),
        ["R26BK98000154"]
    );
    assert_eq!(
        read(&f, "SELECT contract_base, notice_group FROM project_link;"),
        ["R26TA98000669|R26BK98000154"]
    );

    // 남은 계열의 현행은 취소공고다. 취소되었다는 이유로 빼지 않는다(ADR-025).
    let rows = rows(&f);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["현행공고"], "R26BK98000154-001");
}

/// `공고가_다_사라지면_건과_링크가_함께_걷힌다`
#[test]
fn when_every_notice_goes_the_case_and_links_go_too() {
    let f = Fixture::new("ng-delete-all");
    put(&f, "R26BK98000154", "000", &[]);
    contract(&f, "R26TA98000669");
    reference_contract(&f, "R26TA98000669", "R26BK98000154");

    f.store()
        .delete(&EntityRef::new("notice", "R26BK98000154", "000"), true)
        .unwrap();

    assert!(groups(&f).is_empty());
    assert_eq!(count(&f, "SELECT COUNT(*) FROM project_link;"), 0);

    // 계약은 남는다. 공고가 사라졌다고 계약이 없던 일이 되지는 않는다.
    assert_eq!(count(&f, "SELECT COUNT(*) FROM v_계약;"), 1);
}

/// `공고의_지울_것을_셀_때_접수_링크도_함께_센다`
#[test]
fn notice_deletion_plan_counts_request_links_too() {
    let f = Fixture::new("ng-plan-deletion");
    put(&f, "R26BK98000154", "000", &[]);
    request(&f, "MBKLMH26930006");

    let notice = EntityRef::new("notice", "R26BK98000154", "000");
    assert!(!f.store().plan_deletion(&notice, true).unwrap().linked);

    reference_request(&f, "MBKLMH26930006", "000", "R26BK98000154");

    assert!(f.store().plan_deletion(&notice, true).unwrap().linked);
}

// ── 지어낸 자료 ──────────────────────────────────────────────

fn put(f: &Fixture, base: &str, seq: &str, related: &[&str]) {
    put_at(f, base, seq, POSTED, related);
}

/// 게시일시까지 정해 넣는다. 늦고 이른 것이 갈려야 하는 자리에서만 쓴다.
fn put_at(f: &Fixture, base: &str, seq: &str, posted: &str, related: &[&str]) {
    let kind = if seq == "001" {
        "취소공고"
    } else {
        "등록공고"
    };
    f.notice(
        base,
        seq,
        &[
            ("title", "지어낸 공고"),
            ("notice_kind", kind),
            ("posted_at", posted),
        ],
        &[],
        related,
    );
}

/// C# `접수와_계약을_잇는다` — 접수와 계약을 하나씩 넣고 **이 공고에** 잇는다. 사람이 짚는 것은 공고이고, 표에 남는 것은
/// 그 공고가 속한 건이다.
fn link_request_and_contract(f: &Fixture, notice_base: &str) {
    request(f, "MBKLMH26930006");
    contract(f, "R26TA98000669");
    reference_contract(f, "R26TA98000669", notice_base);
    reference_request(f, "MBKLMH26930006", "000", notice_base);
}

/// `Assert.Single(new Outline(_database).Build().Chains)`.
fn only_chain(f: &Fixture) -> OutlineChain {
    let mut chains = Outline::new(f.database()).build().unwrap().chains;
    assert_eq!(chains.len(), 1, "사슬이 하나가 아닙니다");
    chains.remove(0)
}

/// C# `NoticeRelationTests.Related(base, seq)`.
fn related(f: &Fixture, base: &str, seq: &str) -> Vec<String> {
    let c = read_only(f);
    let mut s = c
        .prepare("SELECT related FROM notice_relation WHERE notice_base = ?1 AND seq = ?2 ORDER BY line_no;")
        .unwrap();
    s.query_map([base, seq], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect()
}

fn contract(f: &Fixture, base: &str) {
    f.contract(
        base,
        "00",
        &[
            ("title", "지어낸 계약"),
            ("contracted_on", "2026-07-21 00:00:00"),
            ("amount", AMOUNT),
        ],
        &[],
    );
}

fn request(f: &Fixture, base: &str) {
    f.request(
        base,
        "000",
        &[
            ("title", "지어낸 접수"),
            ("received_on", "2026-06-09 00:00:00"),
        ],
        &[&[
            ("line_no", "1"),
            ("request_number", base),
            ("item_name", "지어낸 품목"),
            ("quantity", "1"),
            ("unit_price", AMOUNT),
        ]],
    );
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

// ── 읽기 ─────────────────────────────────────────────────────

fn read_only(f: &Fixture) -> Connection {
    f.database().open_read_only().unwrap()
}

/// 서 있는 건 이름 전부.
fn groups(f: &Fixture) -> Vec<String> {
    read(
        f,
        "SELECT group_base FROM notice_group ORDER BY group_base;",
    )
}

/// 이 계열이 속한 건.
fn group(f: &Fixture, notice_base: &str) -> String {
    read_only(f)
        .query_row(
            "SELECT group_base FROM notice_series WHERE notice_base = ?1;",
            [notice_base],
            |r| r.get::<_, Option<String>>(0),
        )
        .optional()
        .unwrap()
        .flatten()
        .unwrap_or_default()
}

fn text(v: Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::Integer(i) => i.to_string(),
        Value::Real(r) => r.to_string(),
        Value::Text(t) => t,
        Value::Blob(_) => "System.Byte[]".into(),
    }
}

fn rows(f: &Fixture) -> Vec<BTreeMap<String, String>> {
    let c = read_only(f);
    let mut s = c.prepare("SELECT * FROM v_공고 ORDER BY 1;").unwrap();
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

fn read(f: &Fixture, sql: &str) -> Vec<String> {
    let c = read_only(f);
    let mut s = c.prepare(sql).unwrap();
    let n = s.column_count();
    s.query_map([], |r| {
        (0..n)
            .map(|i| r.get::<_, Value>(i).map(text))
            .collect::<rusqlite::Result<Vec<_>>>()
            .map(|cells| cells.join("|"))
    })
    .unwrap()
    .map(|r| r.unwrap())
    .collect()
}

fn count(f: &Fixture, sql: &str) -> i64 {
    read_only(f).query_row(sql, [], |r| r.get(0)).unwrap()
}

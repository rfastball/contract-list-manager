//! C# `StatusTests` — 현황의 **이음매**가 섰는지 본다. 지킬 것은 "무엇이 몇 건 나오는가" 가 아니라 지표를 더하거나
//! 빼는 일이 `metrics()` 배열만 고치는 일인가이다. 붙박이 지표의 SQL 은 빈 DB 에서도 전부 돌아야 한다.

mod common;

use common::store_fixture::Fixture;
use pclm_core::storage::status::{Metric, Status, metrics};

#[test]
fn 붙박이_지표는_빈_db에서도_전부_돈다() {
    let f = Fixture::new("status-builtin");
    let report = Status::new(f.database()).report().unwrap();

    assert_eq!(
        metrics().len(),
        report.groups.iter().map(|g| g.cells.len()).sum::<usize>()
    );
    assert!(
        report
            .groups
            .iter()
            .all(|g| g.cells.iter().all(|c| c.count == 0))
    );
}

/// 묶음은 배열에 **처음 나온 차례**로 선다. 사전순이 아니다.
#[test]
fn 묶음은_배열에_처음_나온_차례로_선다() {
    let f = Fixture::new("status-order");
    let report = Status::with_metrics(
        f.database(),
        vec![
            Metric::new("하", "가", "SELECT 1;", None),
            Metric::new("가", "나", "SELECT 2;", None),
            Metric::new("하", "다", "SELECT 3;", None),
            Metric::new("나", "라", "SELECT 4;", None),
        ],
    )
    .report()
    .unwrap();

    let names: Vec<&str> = report.groups.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(names, ["하", "가", "나"]);
    // 같은 묶음의 지표는 배열 차례대로 그 묶음 안에 든다.
    let first: Vec<&str> = report.groups[0]
        .cells
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    assert_eq!(first, ["가", "다"]);
}

/// **지표를 하나 더하는 일이 배열을 고치는 일이다.** 더한 칸이 보고에 뜨고 나머지는 그대로다.
#[test]
fn 지표를_하나_더하면_그_칸만_는다() {
    let f = Fixture::new("status-add");
    let before = Status::new(f.database()).report().unwrap();
    let mut more = metrics();
    more.push(Metric::new(
        "쌓인 것",
        "상대자",
        "SELECT COUNT(*) FROM counterparty;",
        None,
    ));
    let after = Status::with_metrics(f.database(), more).report().unwrap();

    assert_eq!(before.groups.len(), after.groups.len());
    let cells = |r: &pclm_core::storage::status::StatusReport| -> Vec<String> {
        r.groups
            .iter()
            .flat_map(|g| g.cells.iter().map(|c| c.name.clone()))
            .collect()
    };
    assert_eq!(cells(&before).len() + 1, cells(&after).len());

    let piled: Vec<_> = after
        .groups
        .iter()
        .filter(|g| g.name == "쌓인 것")
        .collect();
    assert_eq!(piled.len(), 1);
    assert_eq!(piled[0].cells.last().unwrap().name, "상대자");

    // 나머지 칸은 이름도 차례도 그대로다 — 더한 것만 빼면 먼저 것과 같다.
    let rest: Vec<String> = cells(&after)
        .into_iter()
        .filter(|n| n != "상대자")
        .collect();
    assert_eq!(cells(&before), rest);
}

/// 거르개는 손대지 않고 **그대로** 실어 낸다.
#[test]
fn 거르개가_그대로_실려_온다() {
    let f = Fixture::new("status-filter");
    let report = Status::with_metrics(
        f.database(),
        vec![
            Metric::new("단계", "미착수", "SELECT 7;", Some("단계=미착수")),
            Metric::new("단계", "합", "SELECT 9;", None),
        ],
    )
    .report()
    .unwrap();

    assert_eq!(report.groups.len(), 1);
    let group = &report.groups[0];
    assert_eq!(group.cells[0].filter.as_deref(), Some("단계=미착수"));
    assert_eq!(group.cells[0].count, 7);
    assert_eq!(group.cells[1].filter, None);
}

/// 붙박이 지표의 거르개는 `"열이름=값"` 꼴 하나뿐이다.
#[test]
fn 붙박이_거르개는_열이름_등호_값_꼴이다() {
    for filter in metrics().iter().filter_map(|m| m.filter.as_deref()) {
        let parts: Vec<&str> = filter.split('=').collect();
        assert_eq!(parts.len(), 2);
        assert!(!parts[0].is_empty());
        assert!(!parts[1].is_empty());
    }
}

/// 다리가 싣는 꼴 — camelCase 는 한글 이름을 바꾸지 않고, 비어 있는 거르개는 `null` 로 적는다.
#[test]
fn 다리_꼴은_한글_이름을_그대로_둔다() {
    let f = Fixture::new("status-json");
    let report = Status::with_metrics(
        f.database(),
        vec![Metric::new("단계", "합", "SELECT 9;", None)],
    )
    .report()
    .unwrap();
    assert_eq!(
        report.to_json().to_string(),
        r#"{"groups":[{"이름":"단계","cells":[{"이름":"합","수":9,"거르개":null}]}]}"#
    );
}

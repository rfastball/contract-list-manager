//! C# `PlanImportTests` — 연간 조달계획 엑셀을 읽어 넣는다.
//!
//! 붙들어 두는 것은 **표본 서식 그대로 읽히는가**와 **절반만 들어가는 일이 없는가**다. 시험용 엑셀은 그 자리에서
//! 지어 쓴다 — 표본 파일은 실제 조달 자료라 시험에 쓰지 않는다. 값은 모두 지어낸 것이다.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use pclm_core::storage::pclm_file;
use pclm_core::storage::store::text_of;
use pclm_core::{Database, Env, PclmRole};
use pclm_excel::plan_import::{self, PlanImportResult};
use pclm_excel::writer::{self, Cell, Sheet, Style, Value};

struct TestEnv;

impl Env for TestEnv {
    fn now_utc(&self) -> String {
        "2026-11-07T03:04:05.1234567Z".into()
    }

    fn new_id(&self) -> String {
        "0".repeat(32)
    }
}

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// 시험 하나의 폴더와 작업자료. 놓으면 지운다.
struct Fixture {
    root: PathBuf,
    database: Database,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "pclm-excel-plan-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let database = pclm_file::create(&root.join("계획.db"), PclmRole::WORK, &TestEnv).unwrap();
        Fixture { root, database }
    }

    fn import(&self, path: &Path) -> PlanImportResult {
        plan_import::import(&self.database, path, &TestEnv).unwrap()
    }

    /// 지어낸 계획 엑셀 한 권. 값은 모두 글로 적어 엑셀이 재해석하지 않게 한다.
    fn book(&self, headers: &[&str], rows: &[&[&str]]) -> PathBuf {
        let mut all = vec![headers.iter().map(|h| text(h)).collect::<Vec<_>>()];
        for row in rows {
            all.push(row.iter().map(|v| text(v)).collect());
        }
        self.save(all)
    }

    fn save(&self, rows: Vec<Vec<Cell>>) -> PathBuf {
        let path = self.root.join(format!(
            "계획-{}.xlsx",
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        let sheet = Sheet {
            name: "계획".into(),
            rows,
            ..Default::default()
        };
        writer::write(std::fs::File::create(&path).unwrap(), &[sheet]).unwrap();
        path
    }

    fn rows(&self) -> i64 {
        let c = self.database.open_read_only().unwrap();
        c.query_row("SELECT COUNT(*) FROM plan;", [], |r| r.get(0))
            .unwrap()
    }

    /// 수량은 INTEGER 로 담기므로 글로 못박지 않고 받아 적는다.
    fn cell(&self, number: &str, column: &str) -> String {
        let c = self.database.open_read_only().unwrap();
        c.query_row(
            &format!("SELECT \"{column}\" FROM plan WHERE request_number = ?1;"),
            [number],
            |r| Ok(text_of(r.get_ref(0)?).unwrap_or_default()),
        )
        .unwrap_or_default()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn text(v: &str) -> Cell {
    Cell {
        value: Value::Text(v.to_owned()),
        style: Style::DEFAULT,
    }
}

/// 표본의 첫 줄. 열두 개, 이 차례 그대로다.
const 표본머리글: &[&str] = &[
    "조달요구번호",
    "재고번호",
    "품명",
    "화폐구분",
    "단위",
    "지시수량",
    "요청부대부서명",
    "요청부대담당자",
    "요청부대사용자전화번호",
    "계약부서",
    "담당자",
    "연락처",
];

/// 표본 한 줄. 값은 모두 지어낸 것이다.
const 표본줄: &[&str] = &[
    "TSTPLA26500286",
    "2590-01-234-5678",
    "전동 윈치",
    "KRW",
    "대",
    "8",
    "한별군수지원단 보급창",
    "홍길동",
    "051-000-0000",
    "계약1과",
    "김철수",
    "02-000-0000",
];

/// 표본 그대로면 짚어 줄 것 없이 잡힌다 — 갓 깐 사람이 바로 넣을 수 있어야 한다.
#[test]
fn 표본_머리글을_그대로_잡는다() {
    let f = Fixture::new();
    let result = f.import(&f.book(표본머리글, &[표본줄]));

    assert!(result.ok());
    assert_eq!(result.rows, 1);
    assert_eq!(result.created, 1);

    let n = "TSTPLA26500286";
    assert_eq!(f.cell(n, "stock_number"), "2590-01-234-5678");
    assert_eq!(f.cell(n, "item_name"), "전동 윈치");
    assert_eq!(f.cell(n, "currency"), "KRW");
    assert_eq!(f.cell(n, "unit"), "대");
    assert_eq!(f.cell(n, "quantity"), "8");
    assert_eq!(f.cell(n, "requesting_unit"), "한별군수지원단 보급창");
    assert_eq!(f.cell(n, "requesting_officer"), "홍길동");
    assert_eq!(f.cell(n, "requesting_phone"), "051-000-0000");
    assert_eq!(f.cell(n, "contract_department"), "계약1과");
    assert_eq!(f.cell(n, "officer"), "김철수");
    assert_eq!(f.cell(n, "contact"), "02-000-0000");
}

/// 필수 머리글이 없으면 **한 줄도 들어가지 않는다**. 없는 것은 **머리글 이름**으로 적는다 — 사람이 고칠 것은
/// 엑셀의 첫 줄이지 DB 의 열 이름이 아니다.
#[test]
fn 필수_머리글이_없으면_한_줄도_들어가지_않는다() {
    let f = Fixture::new();
    let book = f.book(
        &["조달요구번호", "품명", "책임자"],
        &[
            &["TSTPLA26500286", "전동 윈치", "김철수"],
            &["TSTPLA26500290", "전동 권양기", "김철수"],
        ],
    );

    let result = f.import(&book);

    assert!(!result.ok());
    assert_eq!(result.missing_required, ["담당자"]);
    assert_eq!(result.created, 0);
    assert_eq!(f.rows(), 0);
}

/// 무엇이 들어갔는지는 보여야 한다 — 잡힌 머리글을 표본 차례대로 낸다.
#[test]
fn 실제로_잡힌_머리글을_표본_차례로_낸다() {
    let f = Fixture::new();
    // 표본에 없는 열이 섞여 있어도 그냥 읽는다. 그 열은 잡히지 않을 뿐이다.
    let book = f.book(
        &["담당자", "품명", "비고", "조달요구번호"],
        &[&["김철수", "전동 윈치", "1분기", "TSTPLA26500286"]],
    );

    let result = f.import(&book);

    assert!(result.ok());
    assert_eq!(result.found, ["조달요구번호", "품명", "담당자"]);
}

/// 계획변경은 차수를 올리지 않고 덮어쓴다(스키마 V12). 같은 계획을 다시 넣어도 행이 늘지 않는다.
#[test]
fn 다시_가져오면_조달요구번호_기준으로_갱신된다() {
    let f = Fixture::new();
    let 고친줄: &[&str] = &[
        "TSTPLA26500286",
        "2590-01-234-5678",
        "전동 윈치",
        "KRW",
        "대",
        "6",
        "한별군수지원단 보급창",
        "홍길동",
        "051-000-0000",
        "계약2과",
        "이영희",
        "02-000-0000",
    ];

    f.import(&f.book(표본머리글, &[표본줄]));
    let again = f.import(&f.book(표본머리글, &[고친줄]));

    assert_eq!(again.created, 0);
    assert_eq!(again.updated, 1);
    assert_eq!(f.rows(), 1);
    assert_eq!(f.cell("TSTPLA26500286", "quantity"), "6");
    assert_eq!(f.cell("TSTPLA26500286", "officer"), "이영희");
    assert_eq!(f.cell("TSTPLA26500286", "contract_department"), "계약2과");
}

/// 지시수량은 **INTEGER** 로 담는다 — `request_item.quantity` 와 형을 맞춰야 계획을 분모로 놓고 견줄 수 있다.
/// 자릿점이 찍혀 와도 셈할 수 있는 꼴로 들어간다.
#[test]
fn 지시수량은_정수로_담긴다() {
    for (written, stored) in [
        ("8", Some(8)),
        ("1,200", Some(1200)),
        ("0", Some(0)),
        ("", None),
    ] {
        let f = Fixture::new();
        let book = f.book(
            &["조달요구번호", "담당자", "지시수량"],
            &[&["TSTPLA26500286", "김철수", written]],
        );

        f.import(&book);

        let c = f.database.open_read_only().unwrap();
        let quantity: Option<i64> = c
            .query_row(
                "SELECT quantity FROM plan WHERE request_number = 'TSTPLA26500286';",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(quantity, stored, "{written:?}");
    }
}

#[test]
fn 소수_지시수량이_있으면_행을_알리고_앞선_정상행의_갱신도_되돌린다() {
    let f = Fixture::new();
    let headers = &["조달요구번호", "담당자", "지시수량"];
    f.import(&f.book(headers, &[&["PLAN1", "시험", "8"]]));
    // C# 은 글로 지은 뒤 그 칸에 수 1.5 를 넣고 저장한다 — 여기서는 처음부터 수로 짓는다.
    let path = f.save(vec![
        headers.iter().map(|h| text(h)).collect(),
        vec![text("PLAN1"), text("시험"), text("3")],
        vec![
            text("PLAN2"),
            text("시험"),
            Cell {
                value: Value::Number(1.5),
                style: Style::DEFAULT,
            },
        ],
    ]);

    let result = f.import(&path);

    assert!(!result.ok());
    assert_eq!(result.created, 0);
    assert_eq!(result.updated, 0);
    assert!(result.missing_required.is_empty());
    assert_eq!(result.errors.len(), 1);
    assert!(result.errors[0].contains("3행 지시수량"));
    assert!(result.errors[0].contains("1.5"));
    assert_eq!(f.rows(), 1);
    assert_eq!(f.cell("PLAN1", "quantity"), "8");
}

/// 번호가 빈 줄은 담을 자리가 없다. **조용히 버리지 않고 세어 낸다** — 계획 엑셀에는 소계·머리말 줄이 섞여 있어,
/// 몇 줄이 빠졌는지 사람이 알아야 한다.
#[test]
fn 조달요구번호가_빈_줄은_건너뛰고_세어_낸다() {
    let f = Fixture::new();
    let 소계: &[&str] = &[
        "",
        "",
        "소계",
        "KRW",
        "",
        "11",
        "",
        "",
        "",
        "",
        "김철수",
        "",
    ];
    let 둘째: &[&str] = &[
        "TSTPLA26500290",
        "2590-01-234-9999",
        "전동 권양기",
        "KRW",
        "대",
        "3",
        "한별군수지원단 보급창",
        "홍길동",
        "051-000-0000",
        "계약1과",
        "김철수",
        "02-000-0000",
    ];

    let result = f.import(&f.book(표본머리글, &[표본줄, 소계, 둘째]));

    assert_eq!(result.rows, 3);
    assert_eq!(result.created, 2);
    assert_eq!(result.skipped, 1);
    assert_eq!(f.rows(), 2);
}

/// 명령줄이 무엇을 읽었는지 보여 주려면 머리글이 차례대로 나와야 한다.
#[test]
fn 첫_줄의_머리글을_차례대로_낸다() {
    let f = Fixture::new();
    let book = f.book(&["조달요구번호", "품명", "담당자"], &[]);

    assert_eq!(
        plan_import::headers(&book).unwrap(),
        ["조달요구번호", "품명", "담당자"]
    );
}

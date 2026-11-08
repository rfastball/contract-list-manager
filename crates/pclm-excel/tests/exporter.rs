//! C# `ViewTests.다리의_읽기_길은_열람_손잡이에서_모두_돈다` 의 내보내기 부분 — 전체 내보내기는 열람 손잡이 위에서도
//! 돌고(자료에 쓰지 않는다), 화면 사진은 선택·정렬 차례를 지키며 엑셀 수식·수로 재해석하지 않는다. 값은 지어낸 것이다.

use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use pclm_core::storage::{pclm_file, views};
use pclm_core::{Env, Error, PclmRole};
use pclm_excel::{SheetSnapshot, export, export_sheet};

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

struct Temp(PathBuf);

impl Temp {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "pclm-excel-export-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        Temp(root)
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn part(path: &Path, name: &str) -> String {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    let mut text = String::new();
    zip.by_name(name)
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    text
}

fn snapshot(rows: &[&[&str]]) -> SheetSnapshot {
    let owned = |v: &[&str]| Some(v.iter().map(|s| Some((*s).to_owned())).collect());
    SheetSnapshot {
        columns: owned(&["번호", "메모"]),
        rows: Some(rows.iter().map(|r| owned(r)).collect()),
    }
}

/// export — 파일은 밖에 떨어지고 자료에는 쓰지 않는다. 열람 손잡이(읽기 전용 사본) 위에서 돈다.
#[test]
fn 전체_내보내기는_열람_손잡이에서_돈다() {
    let t = Temp::new();
    let source = t.0.join("제출.pclm");
    drop(pclm_file::create(&source, PclmRole::SUBMISSION, &TestEnv).unwrap());
    let viewed = pclm_file::open_view(&source, &t.0.join("열람.pclm"), &TestEnv).unwrap();

    let saved = export(
        viewed.database(),
        &t.0.join("새 폴더").join("관리대장.xlsx"),
    )
    .unwrap();

    assert!(saved.exists());
    let workbook = part(&saved, "xl/workbook.xml");
    assert_eq!(workbook.matches("<sheet ").count(), views::EXPORTED.len());
    for view in views::EXPORTED {
        let name = views::sheet_name(view);
        assert!(workbook.contains(&format!("name=\"{name}\"")), "{name}");
    }
}

/// 화면 사진은 선택·정렬 순서를 유지하며 엑셀 수식/숫자로 재해석하지 않는다.
#[test]
fn 화면_사진은_차례를_지키고_글로_못박는다() {
    let t = Temp::new();
    let shot = snapshot(&[&["0002", "=1+1"], &["0001", "둘째"]]);

    // 없는 폴더도 짓는다 — ClosedXML 의 SaveAs 가 그렇게 한다.
    let selected =
        export_sheet(&t.0.join("없던 폴더").join("선택.xlsx"), "선택 자료", &shot).unwrap();
    assert!(selected.ends_with("없던 폴더/선택.xlsx"));

    assert_eq!(
        part(&selected, "xl/workbook.xml")
            .matches("<sheet ")
            .count(),
        1
    );
    // 공유 문자열은 처음 나온 차례로 선다 — 번호·메모·0002·=1+1·0001·둘째. 수식(`<f>`)은 없다.
    let body = part(&selected, "xl/worksheets/sheet1.xml");
    let strings = part(&selected, "xl/sharedStrings.xml");
    let order = ["번호", "메모", "0002", "=1+1", "0001", "둘째"]
        .map(|s| strings.find(&format!(">{s}<")).unwrap());
    assert!(order.is_sorted(), "{strings}");
    assert!(
        body.contains(r#"<c r="A2" s="2" t="s"><v>2</v></c>"#),
        "{body}"
    );
    assert!(
        body.contains(r#"<c r="B2" s="2" t="s"><v>3</v></c>"#),
        "{body}"
    );
    assert!(
        body.contains(r#"<c r="A3" s="2" t="s"><v>4</v></c>"#),
        "{body}"
    );
    assert!(!body.contains("<f>"), "{body}");
    // 몸 칸의 꼴 2 는 글(`@`)이다.
    let styles = part(&selected, "xl/styles.xml");
    assert!(
        styles.contains(r#"<numFmt numFmtId="164" formatCode="@" />"#),
        "{styles}"
    );

    let short = snapshot(&[&["부족"]]);
    assert!(matches!(
        export_sheet(&t.0.join("잘못된.xlsx"), "선택 자료", &short),
        Err(Error::InvalidOperation(_))
    ));
}

/// ClosedXML 이 글을 넣을 때 하는 일 — `'` 로 시작하면 따옴표 머리를 켜고 `'` 를 떼며, `\r\n` 이 들면 줄바꿈을 켠다
/// (홀로 선 `\n` 에는 켜지 않는다). 꼴은 칸마다 처음 쓰인 차례로 서고, 이웃한 같은 너비의 열은 `<col>` 하나로 접힌다.
#[test]
fn 따옴표_머리와_줄바꿈은_closedxml_처럼_꼴이_된다() {
    let t = Temp::new();
    let shot = snapshot(&[&["'0012", "가\r\n나"], &["0003", "가\n나"]]);
    let saved = export_sheet(&t.0.join("따옴표.xlsx"), "선택 자료", &shot).unwrap();

    let strings = part(&saved, "xl/sharedStrings.xml");
    assert!(strings.contains("<t>0012</t>"), "{strings}");
    assert!(!strings.contains("'0012"), "{strings}");

    // 기본 0 · 머리글 1 · 글+따옴표 2 · 글+줄바꿈 3 · 글 4.
    let body = part(&saved, "xl/worksheets/sheet1.xml");
    for (cell, style) in [("A1", 1), ("A2", 2), ("B2", 3), ("A3", 4), ("B3", 4)] {
        assert!(
            body.contains(&format!(r#"<c r="{cell}" s="{style}" "#)),
            "{cell} {body}"
        );
    }
    assert!(
        body.contains(r#"<col min="1" max="2" width="40.710625" style="0" customWidth="1" />"#),
        "{body}"
    );
    assert!(body.ends_with(r#"<headerFooter /><tableParts count="0" /></worksheet>"#));

    let styles = part(&saved, "xl/styles.xml");
    assert!(
        styles.contains(r#"<numFmts count="2"><numFmt numFmtId="0" formatCode="" /><numFmt numFmtId="164" formatCode="@" /></numFmts>"#),
        "{styles}"
    );
    // 줄바꿈은 정렬이라 cellStyleXfs 에서는 글과 하나로 접힌다.
    assert!(styles.contains(r#"<cellStyleXfs count="4">"#), "{styles}");
    assert!(styles.contains(r#"<cellXfs count="5">"#), "{styles}");
    assert_eq!(styles.matches(r#"quotePrefix="1""#).count(), 2, "{styles}");
    assert_eq!(styles.matches(r#"wrapText="1""#).count(), 1, "{styles}");
}

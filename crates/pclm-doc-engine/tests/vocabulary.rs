//! 확장에 실려 나가는 서식 셋의 칸이 모두 쓸 수 있는 이름(통합 뷰의 열 · 내 정보)인가 — 통합 뷰의 열 이름을
//! 바꾸면 여기서 붉어진다. 바꾸지 않으면 그 칸은 문서에 【확인 필요】로만 남고 아무것도 실패하지 않는다.
//!
//! 사람이 읽는 이름 표(`docs/document-vocabulary.md`)가 어휘와 같은지도 본다.

use std::path::{Path, PathBuf};

use pclm_doc_engine::vocab::vocabulary;
use pclm_doc_engine::{MISSING, template, text};
use serde_json::{Map, Value, json};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn shipped(name: &str) -> Vec<u8> {
    std::fs::read(root().join("extension/templates").join(name)).expect("실린 서식")
}

const HWPX: &str = "물품 구매입찰 공고.hwpx";
const TEXTS: [&str; 2] = [
    "계약방법 결정 및 구매추진 안내.txt",
    "낙찰자 선정 및 계약체결 안내.txt",
];

/// 통합 뷰의 모든 열에 지어낸 값을 넣은 건.
fn full_case() -> Value {
    let values: Map<String, Value> = vocabulary()
        .columns
        .iter()
        .map(|c| {
            (c.clone(), json!(format!("{c} 값")))
        })
        .collect();
    json!({ "entityType": "notice", "entity": "R26BK00000001-000", "values": values, "rows": 1, "older": false })
}

fn context() -> Value {
    json!({ "me": { "dept": "계약과", "name": "홍길동", "phone": "02-000-0000", "fax": "02-000-0001" } })
}

/// 항목마다 첫 선택을 고른다.
fn first_options(slots: &Value) -> Map<String, Value> {
    slots
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| Some((s["id"].as_str()?.to_owned(), s["options"][0]["id"].clone())))
        .filter(|(_, o)| !o.is_null())
        .collect()
}

fn unknown(fields: &Value) -> Vec<String> {
    fields
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["canonical"].is_null())
        .map(|f| f["name"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn 실린_서식의_칸은_모두_쓸_수_있는_이름이다() {
    let (_, inspected) = template::prepare(&shipped(HWPX)).unwrap();
    assert_eq!(unknown(&inspected["fields"]), Vec::<String>::new(), "{HWPX}");
    for name in TEXTS {
        let text = String::from_utf8(shipped(name)).unwrap();
        assert_eq!(unknown(&text::inspect(&text)["fields"]), Vec::<String>::new(), "{name}");
    }
}

#[test]
fn 통합_뷰의_한_줄로_실린_서식_셋을_빈칸_없이_채운다() {
    let case = full_case();
    let (prepared, inspected) = template::prepare(&shipped(HWPX)).unwrap();
    let (_, report) =
        template::make(&prepared, &case, &context(), &first_options(&inspected["slots"])).unwrap();
    assert_eq!(report["mismatch"], 0);
    let missing: Vec<&Value> = report["fields"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["missing"] == true)
        .collect();
    assert!(missing.is_empty(), "{HWPX}: {missing:?}");
    // 맨 위 날짜는 게시 전에 쓰는 것이라 게시일시가 아니라 쓰는 날의 연 · 월이다.
    let value = |name: &str| {
        report["fields"].as_array().unwrap().iter().find(|f| f["field"] == name).map(|f| f["value"].clone())
    };
    assert_eq!(value("오늘_년월"), Some(json!("오늘_년월 값")), "{HWPX}");
    // 「① 계약담당자」 줄은 내 정보, 「② 수요기관 연락처」 줄은 공고의 수요기관 쪽 담당자(통합 뷰의 열)다.
    assert_eq!(value("계약담당자"), Some(json!("홍길동")));
    assert_eq!(value("계약담당 전화번호"), Some(json!("02-000-0000")));
    assert_eq!(value("계약담당 팩스번호"), Some(json!("02-000-0001")));
    assert_eq!(value("담당자"), Some(json!("담당자 값")));
    assert_eq!(value("담당부서"), Some(json!("담당부서 값")));
    assert_eq!(value("담당자 전화번호"), Some(json!("담당자 전화번호 값")));
    for name in TEXTS {
        let tpl = String::from_utf8(shipped(name)).unwrap();
        let picks = first_options(&text::inspect(&tpl)["slots"]);
        let made = text::make(&tpl, &case, &context(), &picks).unwrap();
        let out = made["text"].as_str().unwrap();
        assert!(!out.contains(MISSING), "{name}:\n{out}");
    }
}

#[test]
fn 어휘_표가_어휘_파일과_같다() {
    let path = root().join("docs/document-vocabulary.md");
    let doc = std::fs::read_to_string(&path)
        .expect("docs/document-vocabulary.md")
        .replace("\r\n", "\n");
    let start = doc.find("<!-- vocab:start").expect("vocab:start 표시");
    let end = doc.find("<!-- vocab:end -->").expect("vocab:end 표시") + "<!-- vocab:end -->".len();
    assert!(
        doc[start..end] == vocabulary().markdown(),
        "docs/document-vocabulary.md 의 표가 어휘와 다릅니다. \
         cargo run -p pclm-doc-engine --example vocabulary_doc 으로 다시 지으세요."
    );
}

/// 실린 한글 서식은 컴파일 전 모습(본문에 `{{#항목}}`·`{{#선택}}` 표시가 글자로 보이는 것)이다 — 컴파일본을 실으면
/// 한글에서 열어 고칠 때 표시가 책갈피 속으로 숨어 서식 작성자가 항목 구조를 볼 수 없고, 아무것도 실패하지 않은 채
/// 다음 판 서식이 컴파일본 위에 덧쌓인다. 불러올 때 `prepare` 가 메모리에서 컴파일한다.
#[test]
fn 실린_한글_서식은_컴파일_전_모습이다() {
    let tpl = shipped(HWPX);
    let inspected = template::inspect(&tpl).unwrap();
    assert!(inspected["markers"].as_u64().unwrap() > 0, "표시가 글자로 남아 있어야 합니다");
    let (_, report) = template::prepare(&tpl).unwrap();
    assert_eq!(report["compiled"], true);
    assert!(!report["slots"].as_array().unwrap().is_empty());
}

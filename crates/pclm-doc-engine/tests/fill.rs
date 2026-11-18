//! 문서나르미 예제 서식(누름틀 수가 다른 3개, `tests/fixtures/templates`)을 네이티브로 채운다. 값은 지어낸 것이다
//! (`tests/fixtures/values.json`).

use std::path::PathBuf;

use serde_json::{Map, Value};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn values() -> Map<String, Value> {
    serde_json::from_slice(&std::fs::read(fixtures().join("values.json")).unwrap()).unwrap()
}

fn templates() -> Vec<(String, Vec<u8>)> {
    let mut found: Vec<_> = std::fs::read_dir(fixtures().join("templates"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "hwpx"))
        .map(|p| {
            (
                p.file_name().unwrap().to_string_lossy().into_owned(),
                std::fs::read(&p).unwrap(),
            )
        })
        .collect();
    found.sort();
    assert_eq!(found.len(), 3, "예제 서식 3개");
    found
}

#[test]
fn 모든_누름틀을_채우고_다시_읽는다() {
    let values = values();
    for (name, template) in templates() {
        let fields = pclm_doc_engine::list_fields(&template).unwrap();
        assert!(!fields.is_empty(), "{name}: 누름틀이 없다");
        let (doc, report) = pclm_doc_engine::fill(&template, &values).unwrap();
        assert_eq!(report["mismatch"], 0, "{name}: 다시 읽은 값이 다르다");
        assert_eq!(report["path"], "batch", "{name}: 빠른 길로 채워야 한다");
        let lines = report["fields"].as_array().unwrap();
        assert_eq!(lines.len(), fields.len(), "{name}");
        for line in lines {
            assert_eq!(line["filled"], true, "{name}: {line}");
            let field = line["field"].as_str().unwrap();
            let expected_missing = values
                .get(field)
                .and_then(Value::as_str)
                .is_none_or(str::is_empty);
            assert_eq!(line["missing"], expected_missing, "{name}: {field}");
            if expected_missing {
                assert_eq!(line["value"], pclm_doc_engine::MISSING);
            }
        }
        // 만든 문서도 서식으로 다시 열린다(누름틀이 그대로 남는다).
        assert_eq!(
            pclm_doc_engine::list_fields(&doc).unwrap(),
            fields,
            "{name}"
        );
    }
}

#[test]
fn 값이_없으면_모든_칸을_확인_필요로_남긴다() {
    for (name, template) in templates() {
        let (_, report) = pclm_doc_engine::fill(&template, &Map::new()).unwrap();
        assert_eq!(report["mismatch"], 0, "{name}");
        assert!(
            report["fields"]
                .as_array()
                .unwrap()
                .iter()
                .all(|f| f["missing"] == true),
            "{name}"
        );
    }
}

#[test]
fn 깨진_서식은_말로_거절한다() {
    let err = pclm_doc_engine::fill(b"not a zip", &Map::new()).unwrap_err();
    assert!(err.starts_with("서식을 읽지 못했습니다"), "{err}");
}

const 특수문자: &str = "R&D 장비 <시험용> \"따옴\" '작은'";
const 토큰_값: &str = "앞 {{건명}} 뒤";

// 한글 값의 줄바꿈은 엔진에 정한 규칙이 없어(ADR-051 은 값을 바꾸지 않는다고만 한다) 여기서 지키지 않는다.
#[test]
fn 엑스엠엘_특수문자와_토큰이_든_값도_그대로_들어가고_다시_읽힌다() {
    for (name, template) in templates() {
        let fields = pclm_doc_engine::list_fields(&template).unwrap();
        assert!(fields.len() >= 2, "{name}");
        let mut given = Map::new();
        given.insert(fields[0].clone(), Value::from(특수문자));
        given.insert(fields[1].clone(), Value::from(토큰_값));
        // 토큰이 두 번 풀리는지 보려고 건명 값도 둔다.
        given.insert("건명".into(), Value::from("지어낸 건명"));
        let (doc, report) = pclm_doc_engine::fill(&template, &given).unwrap();
        assert_eq!(report["mismatch"], 0, "{name}");
        // 만든 문서가 엔진의 다시 읽기 길로 다시 열린다.
        assert_eq!(pclm_doc_engine::list_fields(&doc).unwrap(), fields, "{name}");
        let text_of = |field: &str| {
            report["readback"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["field"] == field)
                .map(|r| r["text"].clone())
        };
        assert_eq!(text_of(&fields[0]), Some(Value::from(특수문자)), "{name}");
        assert_eq!(text_of(&fields[1]), Some(Value::from(토큰_값)), "{name}");
    }
}

#[test]
fn 글_서식의_값은_그대로_들어가고_값_안의_토큰은_다시_풀리지_않는다() {
    let case = serde_json::json!({
        "entityType": "contract", "entity": "R26TA0000000200", "rows": 1,
        "values": { "건명": "지어낸 건명", "계약방법": 특수문자, "대표계약업체": 토큰_값,
                    "수요기관": "첫째 줄\n둘째 줄" }
    });
    let tpl = "A: {{계약방법}}\r\nB: {{대표계약업체}}\r\nC: {{수요기관}}\r\nD: {{건명}}\r\n";
    let got = pclm_doc_engine::text::make(tpl, &case, &serde_json::json!({}), &Map::new()).unwrap();
    assert_eq!(got["mismatch"], 0);
    assert_eq!(
        got["text"].as_str().unwrap(),
        format!("A: {특수문자}\r\nB: {토큰_값}\r\nC: 첫째 줄\n둘째 줄\r\nD: 지어낸 건명\r\n")
    );
}

//! 건(통합 뷰의 한 줄) + 서식 → 문서. 항목이 있는 서식(확장에 실린 「물품 구매입찰 공고」와 같은 바이트)과
//! 본문에 `{{#항목}}` 표시를 적은 서식(시험 안에서 만든다)을 쓴다.

use std::path::PathBuf;

use hwpx_core::package::{HwpxPackage, PackageLimits};
use pclm_doc_engine::{MISSING, template};
use serde_json::{Map, Value, json};

/// 시험용 서식(`tests/fixtures/templates`, 문서나르미 예제).
fn read(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/templates")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("서식이 없습니다 {}: {e}", path.display()))
}

/// 호스트 `document {op:"read"}` 의 답 꼴 — 공고까지 온 건의 통합 뷰 한 줄(값은 지어낸 것).
fn case() -> Value {
    json!({
        "entityType": "notice", "entity": "R26BK00000003-001", "rows": 1, "older": false,
        "caseKeys": ["notice:R26BK00000003"],
        "values": {
            "건명": "청사 복도 LED 조명 교체 물품 구매",
            "입찰공고번호": "R26BK00000003-001", "공고명": "청사 복도 LED 조명 교체 물품 구매", "계약방법": "일반경쟁",
            "사업예산": "67,100,000원", "추정가격": "55,454,545원",
            "담당부서": "청사관리과", "담당자": "김수요", "담당자 전화번호": "000-1111-2222",
            "개찰일시": "2026. 11. 17. 14:00", "게시일시": "2026. 11. 3. 09:30", "수요기관": "○○시 청사관리과",
            "세부품명": "LED 조명", "수량": "120", "단위": "개", "계약금액": ""
        }
    })
}

fn context() -> Value {
    json!({ "me": { "dept": "회계과", "name": "홍길동", "phone": "000-0000-0000" } })
}

fn selections(pairs: &[(&str, &str)]) -> Map<String, Value> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), json!(v)))
        .collect()
}

fn texts(bytes: &[u8]) -> String {
    let package = HwpxPackage::from_bytes(bytes, PackageLimits::default()).unwrap();
    package
        .entries
        .iter()
        .filter(|e| e.name.starts_with("Contents/section"))
        .map(|e| String::from_utf8_lossy(&e.data).into_owned())
        .collect()
}

#[test]
fn 항목_서식은_고른_선택만_남긴다() {
    let tpl = read("물품 구매입찰 공고.hwpx");
    let inspected = template::inspect(&tpl).unwrap();
    assert_eq!(inspected["slots"].as_array().unwrap().len(), 2);
    assert_eq!(inspected["markers"], 0);
    let field = |name: &str| {
        inspected["fields"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["name"] == name)
            .cloned()
            .unwrap()
    };
    // 칸 이름은 통합 뷰의 열 이름이다 — 담당자 · 담당부서는 내 정보가 아니라 공고의 수요기관 쪽 담당자다.
    assert_eq!(field("공고명")["canonical"], "공고명");
    assert_eq!(field("사업예산")["canonical"], "사업예산");
    assert_eq!(field("담당자")["canonical"], "담당자");
    assert_eq!(field("담당부서")["canonical"], "담당부서");
    assert_eq!(field("담당자 전화번호")["group"], "통합 뷰");

    let chosen = selections(&[("slot-i", "option-i2"), ("slot-n", "option-n3")]);
    let (doc, report) = template::make(&tpl, &case(), &context(), &chosen).unwrap();
    assert_eq!(report["mismatch"], 0, "{report}");
    assert_eq!(report["path"], "slots");
    let excluded: Vec<(String, String)> = report["excluded"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            (
                e["slot"].as_str().unwrap().into(),
                e["option"].as_str().unwrap().into(),
            )
        })
        .collect();
    assert_eq!(excluded.len(), 4);
    assert!(!excluded.contains(&("slot-i".into(), "option-i2".into())));
    assert!(!excluded.contains(&("slot-n".into(), "option-n3".into())));

    let line = |name: &str| {
        report["fields"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["field"] == name)
            .cloned()
    };
    assert_eq!(
        line("공고명").unwrap()["value"],
        "청사 복도 LED 조명 교체 물품 구매"
    );
    assert_eq!(line("사업예산").unwrap()["value"], "67,100,000원");
    assert_eq!(line("개찰일시").unwrap()["value"], "2026. 11. 17. 14:00");
    // 「수요기관 연락처」 묶음({{수요기관}} {{담당부서}} {{담당자}} ☎ {{담당자 전화번호}})은 모두 공고의 수요기관 쪽
    // 담당자 값이다 — 내 정보(회계과 · 홍길동 · 000-0000-0000)가 섞이지 않는다.
    for (name, value) in [
        ("수요기관", "○○시 청사관리과"),
        ("담당부서", "청사관리과"),
        ("담당자", "김수요"),
        ("담당자 전화번호", "000-1111-2222"),
    ] {
        let got = line(name).unwrap();
        assert_eq!(got["value"], value, "{name}");
        assert!(
            !got["canonical"]
                .as_str()
                .unwrap_or_default()
                .starts_with("내_"),
            "{name}: {got}"
        );
    }
    assert_eq!(line("인도조건").unwrap()["value"], MISSING);

    // 만든 문서에는 고른 선택의 구역만 남는다.
    let after = template::inspect(&doc).unwrap();
    for slot in after["slots"].as_array().unwrap() {
        assert!(slot["options"].as_array().unwrap().len() <= 1, "{slot}");
    }
    assert!(texts(&doc).contains("청사 복도 LED 조명 교체 물품 구매"));
    // 누름틀이 아닌 필드(하이퍼링크)의 글은 그대로다.
    assert!(texts(&doc).contains("www.law.go.kr"));
    assert!(
        report["fields"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["field"] != "")
    );
}

#[test]
fn 선택이_있는_항목은_모두_골라야_한다() {
    let tpl = read("물품 구매입찰 공고.hwpx");
    let err = template::make(
        &tpl,
        &case(),
        &context(),
        &selections(&[("slot-i", "option-i1")]),
    )
    .unwrap_err();
    assert_eq!(err, "「낙찰자 결정방법」을 골라 주세요.");
    let err = template::make(
        &tpl,
        &case(),
        &context(),
        &selections(&[("slot-i", "x"), ("slot-n", "option-n1")]),
    )
    .unwrap_err();
    assert!(err.contains("없는 선택"), "{err}");
}

#[test]
fn 항목_없는_서식은_통합_뷰의_열로_채운다() {
    let tpl = read("계약체결 안내.hwpx");
    let (_, report) = template::make(&tpl, &case(), &context(), &Map::new()).unwrap();
    assert_eq!(report["mismatch"], 0);
    assert_eq!(report["path"], "batch");
    let line = |name: &str| {
        report["fields"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["field"] == name)
            .cloned()
            .unwrap()
    };
    assert_eq!(line("수요기관")["value"], "○○시 청사관리과");
    assert_eq!(
        line("계약금액")["missing"],
        true,
        "빈 글은 값이 없는 것이다"
    );
    // 통합 뷰의 열이 아닌 이름은 다른 이름으로도 잇지 않는다 — 옛 어휘의 다른 이름(사업명 → 건명)은 걷었다.
    assert_eq!(line("사업명")["canonical"], Value::Null);
    assert_eq!(line("사업명")["value"], MISSING);
}

/// 「계약체결 안내」 끝에 항목 표시 문단을 붙인 서식 — 작성자가 한글 본문에 표시를 적은 것과 같다.
fn with_markers(lines: &[&str]) -> Vec<u8> {
    let mut package =
        HwpxPackage::from_bytes(&read("계약체결 안내.hwpx"), PackageLimits::default()).unwrap();
    let entry = package
        .entries
        .iter_mut()
        .find(|e| e.name == "Contents/section0.xml")
        .unwrap();
    let xml = String::from_utf8(entry.data.clone()).unwrap();
    let paragraphs: String = lines
        .iter()
        .enumerate()
        .map(|(i, text)| {
            format!(r#"<hp:p id="{}" paraPrIDRef="0" styleIDRef="0" pageBreak="0" columnBreak="0" merged="0"><hp:run charPrIDRef="0"><hp:t>{text}</hp:t></hp:run></hp:p>"#, 900_000 + i)
        })
        .collect();
    entry.data = xml
        .replace("</hs:sec>", &format!("{paragraphs}</hs:sec>"))
        .into_bytes();
    package.to_bytes().unwrap()
}

#[test]
fn 본문에_적은_표시는_넣을_때_항목이_된다() {
    let tpl = with_markers(&[
        "{{#항목 특약 특약 사항}}",
        "{{#선택 지체 지체상금 조항}}",
        "지체상금은 계약금액의 일정 비율로 한다.",
        "{{/선택}}",
        "{{#선택 없음 (비움)}}",
        "",
        "{{/선택}}",
        "{{/항목}}",
    ]);
    let before = template::inspect(&tpl).unwrap();
    assert!(before["markers"].as_u64().unwrap() > 0, "{before}");
    let (prepared, report) = template::prepare(&tpl).unwrap();
    assert_eq!(report["compiled"], true, "{report}");
    assert_eq!(report["markers"], 0);
    let slots = report["slots"].as_array().unwrap();
    assert_eq!(slots.len(), 1, "{report}");
    assert_eq!(slots[0]["label"], "특약 사항");
    assert_eq!(slots[0]["options"].as_array().unwrap().len(), 2);

    let keep = selections(&[("특약", "지체")]);
    let (doc, report) = template::make(&prepared, &case(), &context(), &keep).unwrap();
    assert_eq!(report["mismatch"], 0);
    assert!(texts(&doc).contains("지체상금은 계약금액의 일정 비율로 한다."));
    assert!(!texts(&doc).contains("{{#"), "표시는 남지 않는다");

    let empty = selections(&[("특약", "없음")]);
    let (doc, _) = template::make(&prepared, &case(), &context(), &empty).unwrap();
    assert!(!texts(&doc).contains("지체상금은 계약금액의 일정 비율로 한다."));
}

/// 이미 받아들인(컴파일한) 서식을 서식 폴더에 둔 사람도 있다(옛 확장에서 내려받은 것) — 패널은 읽을 때마다 받아들이기를 한 번 더 지난다.
#[test]
fn 받아들인_서식을_다시_받아들여도_그대로다() {
    let tpl = with_markers(&[
        "{{#항목 특약 특약 사항}}",
        "{{#선택 지체 지체상금 조항}}",
        "지체상금은 계약금액의 일정 비율로 한다.",
        "{{/선택}}",
        "{{/항목}}",
    ]);
    let (prepared, first) = template::prepare(&tpl).unwrap();
    let (again, second) = template::prepare(&prepared).unwrap();
    assert_eq!(second["compiled"], false, "{second}");
    assert!(again == prepared, "바이트가 그대로다");
    assert_eq!(second["slots"], first["slots"]);
    assert_eq!(second["fields"], first["fields"]);
}

#[test]
fn 깨진_표시는_말로_거절한다() {
    let tpl = with_markers(&["{{#항목 특약}}", "본문"]);
    let err = template::prepare(&tpl).unwrap_err();
    assert!(err.starts_with("항목 표시를 바꾸지 못했습니다"), "{err}");
}

#[test]
fn 조사_누름틀은_빠른길과_항목길에서_같은_값을_채우고_검증한다() {
    use hwpx_core::native_admission::read_field_value;
    use hwpx_core::structural_boundary::{BoundaryEvent, scan_structural_boundaries};

    for (source, chosen, path) in [
        (read("계약체결 안내.hwpx"), Map::new(), "batch"),
        (
            with_markers(&[
                "{{#항목 끝}}",
                "{{#선택 포함}}",
                "끝.",
                "{{/선택}}",
                "{{/항목}}",
            ]),
            selections(&[("끝", "포함")]),
            "slots",
        ),
    ] {
        let mut package = HwpxPackage::from_bytes(&source, PackageLimits::default()).unwrap();
        for entry in package
            .entries
            .iter_mut()
            .filter(|e| e.name.starts_with("Contents/section"))
        {
            let xml = String::from_utf8(entry.data.clone()).unwrap();
            entry.data = xml
                .replace("name=\"사업명\"", "name=\"@조사:계약담당자:은/는\"")
                .replace("name=\"계약금액\"", "name=\"@조사:계약금액:으로/로\"")
                .replace("name=\"공고번호\"", "name=\"@조사:입찰공고번호:이/가\"")
                .into_bytes();
        }
        let (tpl, inspected) = template::prepare(&package.to_bytes().unwrap()).unwrap();
        assert_eq!(inspected["problems"], json!([]));
        assert_eq!(
            inspected["fields"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|f| f["kind"] == "josa")
                .count(),
            3
        );
        let (doc, report) = template::make(&tpl, &case(), &context(), &chosen).unwrap();
        assert_eq!(report["path"], path);
        assert_eq!(report["mismatch"], 0);
        let actual = HwpxPackage::from_bytes(&doc, PackageLimits::default()).unwrap();
        let scan = scan_structural_boundaries(&actual);
        let mut josa_count = 0;
        for event in scan.entries.iter().flat_map(|e| &e.events) {
            if let BoundaryEvent::FieldBegin {
                raw_name: Some(name),
                pair,
                ..
            } = event
                && name.starts_with("@조사:")
            {
                josa_count += 1;
                let row = report["fields"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|r| r["field"] == *name)
                    .unwrap();
                let text = read_field_value(&actual, &scan, pair).unwrap().text;
                assert_eq!(row["value"], text);
                let (expected, reason) = match name.as_str() {
                    "@조사:계약담당자:은/는" => ("은", Value::Null),
                    "@조사:계약금액:으로/로" => ("(으)로", json!("missing_value")),
                    "@조사:입찰공고번호:이/가" => {
                        ("이(가)", json!("unknown_pronunciation"))
                    }
                    _ => panic!("unexpected token: {name}"),
                };
                assert_eq!(text, expected, "{path}: {name}");
                assert_eq!(row["reason"], reason);
            }
        }
        assert_eq!(josa_count, 3);
        // 잘못된 시스템 이름은 낮은 수준의 채우기에서 일반 누락값으로 넘어가지 않는다.
        let mut broken = HwpxPackage::from_bytes(&tpl, PackageLimits::default()).unwrap();
        for entry in broken
            .entries
            .iter_mut()
            .filter(|e| e.name.starts_with("Contents/section"))
        {
            entry.data = String::from_utf8(entry.data.clone())
                .unwrap()
                .replace("@조사:계약담당자:은/는", "@조사:계약담당자:는/은")
                .into_bytes();
        }
        let broken = broken.to_bytes().unwrap();
        assert!(
            !template::inspect(&broken).unwrap()["problems"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert!(template::make(&broken, &case(), &context(), &chosen).is_err());
    }
}

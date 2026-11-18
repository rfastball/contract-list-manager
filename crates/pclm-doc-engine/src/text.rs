//! 글 서식(.txt) — 기안문처럼 결재 시스템 본문에 붙여 넣을 글. 파일로 저장하지 않고 패널이 복사한다.
//!
//! 문법은 문서나르미 txt 서식과 같다: 칸은 `{{건명}}`, 골라 넣을 문단은 줄을 혼자 차지하는
//! `{{#항목 id 이름}}` · `{{#선택 id 이름}}` … `{{/선택}}` · `{{/항목}}`. 읽기는 문서나르미 `text_structure`·
//! `text_render` 를 그대로 쓴다. 칸 이름을 통합 뷰의 열(과 내 정보)로 잇고 값이 없으면 【확인 필요】로 두는 것은
//! 한글 서식과 같다.

use std::collections::{HashMap, HashSet};

use hwpx_filler::structure_scan::PLACEMENT_OPTION;
use hwpx_filler::text_render::{
    SEG_MISSING, iter_field_token_matches, render_segments, template_fields,
};
use hwpx_filler::text_structure::{TextStructureScan, project_selected_text, scan_text_structure};
use serde_json::{Map, Value, json};

use crate::template::{choices, document_values, inspect_fields, validate_system_fields};
use crate::values::resolve;

/// 일반 토큰 스캐너가 글로 남기는 중첩·미완성 시스템 토큰도 서식 오류다.
fn broken_system_tokens(text: &str) -> Vec<String> {
    let tokens = iter_field_token_matches(text);
    text.match_indices("{{")
        .filter(|(at, _)| {
            text[at + 2..].trim_start().starts_with('@') && !tokens.iter().any(|t| t.start == *at)
        })
        .map(|_| {
            "시스템 토큰을 고쳐 주세요: {{@조사:대상필드:은/는}} (중첩·줄바꿈 없이 닫아 주세요)"
                .to_string()
        })
        .collect()
}

fn slot_json(scan: &TextStructureScan) -> Value {
    Value::Array(
        scan.slots
            .iter()
            .map(|s| {
                json!({
                    "id": s.id,
                    "label": s.label.clone().unwrap_or_else(|| s.id.clone()),
                    "options": s.options.iter().map(|o| json!({ "id": o.id, "label": o.label.clone().unwrap_or_else(|| o.id.clone()) })).collect::<Vec<_>>(),
                })
            })
            .collect(),
    )
}

/// 한글 서식의 [`crate::template::inspect`] 와 같은 꼴: `{fields, slots, markers, problems}`.
/// `fields` 는 항목 안팎의 칸을 모두(고르기에 따라 빠질 칸도) 담는다.
pub fn inspect(text: &str) -> Value {
    let scan = scan_text_structure(text);
    let (fields, mut problems) = inspect_fields(&template_fields(text));
    problems.extend(broken_system_tokens(text));
    problems.extend(scan.diagnostics.iter().map(|d| d.message.clone()));
    json!({
        "fields": fields,
        "slots": slot_json(&scan),
        "markers": scan.placements.len(),
        "problems": problems,
    })
}

/// 만들기. 돌려주는 것: `{text, segments, fields, excluded, mismatch, path}` — 한글 서식의 [`crate::template::make`] 와 같은
/// 보고에 만든 글(`text`)과 그 조각(`segments: [{kind, text}]`, kind 는 문서나르미 `text_render` 의 것)을 더한다. 고른 선택만 남기고, 남은 칸을 값이나 【확인 필요】로 바꾼다.
pub fn make(
    text: &str,
    case: &Value,
    context: &Value,
    selections: &Map<String, Value>,
) -> Result<Value, String> {
    let scan = scan_text_structure(text);
    if !scan.diagnostics.is_empty() {
        return Err("서식의 항목 표시가 깨져 있습니다. 서식을 다시 넣어 주세요.".into());
    }
    if let Some(error) = broken_system_tokens(text).into_iter().next() {
        return Err(error);
    }
    validate_system_fields(&template_fields(text), &case["values"])?;
    let chosen = choices(
        scan.slots.iter().map(|s| {
            (
                s.id.as_str(),
                s.label.as_deref().unwrap_or(&s.id),
                s.options.iter().map(|o| o.id.as_str()).collect(),
            )
        }),
        selections,
    )?;
    let picked: HashMap<String, HashSet<String>> = chosen
        .iter()
        .map(|(slot, option)| (slot.clone(), HashSet::from([option.clone()])))
        .collect();
    let kept = project_selected_text(text, &scan, &picked).map_err(|e| e.0)?;
    let resolved = resolve(case, context);
    let names = template_fields(&kept);
    let (values, report) = document_values(&names, &resolved, &case["values"])?;
    let record: HashMap<String, Value> = values.into_iter().collect();
    let (segments, _) = render_segments(&kept, &record);
    // 모든 칸에 값(또는 【확인 필요】)을 주었으니 바뀌지 않은 칸은 없어야 한다 — 있으면 저장(복사)하지 않는다.
    let mismatch = segments.iter().filter(|s| s.kind == SEG_MISSING).count();
    let out: String = segments.iter().map(|s| s.text.as_str()).collect();
    // 미리보기가 서식의 글과 건에서 온 값을 가려 그린다 — 이으면 `text` 와 같다.
    let pieces: Vec<Value> = segments
        .into_iter()
        .map(|s| {
            let mut piece = json!({ "kind": s.kind, "text": s.text });
            if report
                .iter()
                .any(|r| r["field"] == s.name && r["status"] == "needs_review")
            {
                piece["needs_review"] = json!(true);
            }
            piece
        })
        .collect();
    let excluded: Vec<Value> = scan
        .placements
        .iter()
        .filter(|p| {
            p.kind == PLACEMENT_OPTION && p.option_id.as_ref() != chosen.get(&p.slot_id)
        })
        .filter_map(|p| {
            let slot = scan.slots.iter().find(|s| s.id == p.slot_id)?;
            let option = slot
                .options
                .iter()
                .find(|o| Some(&o.id) == p.option_id.as_ref())?;
            Some(json!({ "slot": slot.id, "option": option.id, "label": option.label.clone().unwrap_or_else(|| option.id.clone()) }))
        })
        .collect();
    Ok(json!({
        "text": out,
        "segments": pieces,
        "fields": report,
        "excluded": excluded,
        "mismatch": mismatch,
        "path": "txt",
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn case() -> Value {
        json!({ "entityType": "contract", "entity": "R26TA0000000200", "rows": 1,
                "values": { "건명": "청사 방역 용역", "계약방법": "수의계약", "계약금액": "", "대표계약업체": "(주)깨끗방역" } })
    }

    #[test]
    fn 통합_뷰의_칸은_채우고_나머지는_확인_필요로_둔다() {
        let tpl = "제목: {{건명}} 계약\r\n상대: {{대표계약업체}}\r\n금액: {{계약금액}}원\r\n기타: {{없는 칸}}\r\n";
        let got = make(tpl, &case(), &json!({}), &Map::new()).unwrap();
        assert_eq!(
            got["text"],
            "제목: 청사 방역 용역 계약\r\n상대: (주)깨끗방역\r\n금액: 【확인 필요】원\r\n기타: 【확인 필요】\r\n"
        );
        assert_eq!(got["mismatch"], 0);
        // 조각을 이으면 만든 글과 같다 — 미리보기의 표시가 복사되는 글에 섞이지 않는다.
        let segments = got["segments"].as_array().unwrap();
        let joined: String = segments
            .iter()
            .map(|s| s["text"].as_str().unwrap())
            .collect();
        assert_eq!(joined, got["text"].as_str().unwrap());
        let kinds: Vec<(&str, &str)> = segments
            .iter()
            .map(|s| (s["kind"].as_str().unwrap(), s["text"].as_str().unwrap()))
            .filter(|(k, _)| *k != "literal")
            .collect();
        assert_eq!(
            kinds,
            [
                ("fill", "청사 방역 용역"),
                ("fill", "(주)깨끗방역"),
                ("fill", crate::MISSING),
                ("fill", crate::MISSING)
            ]
        );
        let missing: Vec<&str> = got["fields"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|f| f["missing"] == true)
            .map(|f| f["field"].as_str().unwrap())
            .collect();
        assert_eq!(missing, ["계약금액", "없는 칸"]);
        let fields = inspect(tpl)["fields"].clone();
        assert_eq!(fields[0]["canonical"], "건명");
        assert_eq!(fields[3]["canonical"], Value::Null);
    }

    #[test]
    fn 고른_선택만_남긴다() {
        let tpl = "본문\n{{#항목 붙임 붙임}}\n{{#선택 a 계약서}}\n붙임 계약서 1부. 끝.\n{{/선택}}\n{{#선택 b 없음}}\n끝.\n{{/선택}}\n{{/항목}}\n";
        let inspected = inspect(tpl);
        assert_eq!(inspected["problems"], json!([]));
        assert_eq!(inspected["slots"][0]["label"], "붙임");
        assert_eq!(inspected["slots"][0]["options"][1]["label"], "없음");
        let pick = |o: &str| Map::from_iter([("붙임".to_string(), json!(o))]);
        let got = make(tpl, &case(), &json!({}), &pick("b")).unwrap();
        assert_eq!(got["text"], "본문\n끝.\n");
        assert_eq!(got["excluded"][0]["label"], "계약서");
        assert!(
            make(tpl, &case(), &json!({}), &Map::new())
                .unwrap_err()
                .contains("「붙임」을 골라 주세요")
        );
        assert!(
            make(tpl, &case(), &json!({}), &pick("z"))
                .unwrap_err()
                .contains("없는 선택")
        );
    }

    #[test]
    fn 깨진_표시는_고칠_것으로_보인다() {
        let tpl = "{{#항목 x}}\n내용\n";
        assert!(!inspect(tpl)["problems"].as_array().unwrap().is_empty());
        assert!(make(tpl, &case(), &json!({}), &Map::new()).is_err());
    }

    #[test]
    fn 조사_토큰은_위치와_무관하게_대상을_참조하고_미확정을_보고한다() {
        let tpl = "‘{{수요기관}}’{{@조사:수요기관:은/는}} / {{@조사:계약담당자:이/가}} / {{@조사:추가열:을/를}} / {{@조사:빈사용자열:과/와}} / {{@조사:계약금액:으로/로}} / {{@조사:기호:열:은/는}} / 은(는)";
        let case = json!({"values": {"수요기관": "국방부", "추가 열": "ABC", "빈 사용자 열": " ", "기호:열": "서울"}});
        let result = make(tpl, &case, &json!({"me": {"name": "홍길동"}}), &Map::new()).unwrap();
        assert_eq!(
            result["text"],
            "‘국방부’는 / 이 / 을(를) / 과(와) / (으)로 / 은 / 은(는)"
        );
        assert_eq!(result["mismatch"], 0);
        let fields = result["fields"].as_array().unwrap();
        assert_eq!(fields[2]["canonical"], "내_이름");
        assert_eq!(fields[3]["reason"], "unknown_pronunciation");
        assert_eq!(fields[4]["reason"], "missing_value");
        assert_eq!(fields[5]["reason"], "missing_value");
        let segments = result["segments"].as_array().unwrap();
        assert_eq!(
            segments
                .iter()
                .filter(|s| s["needs_review"] == true)
                .count(),
            3
        );
        assert_eq!(
            segments
                .iter()
                .map(|s| s["text"].as_str().unwrap())
                .collect::<String>(),
            result["text"]
        );
        let inspected = inspect(tpl);
        assert_eq!(inspected["problems"], json!([]));
        assert_eq!(inspected["fields"][1]["kind"], "josa");
        assert_eq!(inspected["fields"][1]["target"], "수요기관");
        // 같은 필드를 반복해도 원값은 변하지 않고, 조사별로 독립해 선택한다.
        let result = make(
            "{{수요기관}}{{@조사:수요기관:은/는}} {{수요기관}}{{@조사:수요기관:을/를}}",
            &case,
            &json!({}),
            &Map::new(),
        )
        .unwrap();
        assert_eq!(result["text"], "국방부는 국방부를");
    }

    #[test]
    fn 시스템_토큰의_오타는_누락값으로_숨기지_않는다() {
        for tpl in [
            "{{@조사:수요기관:은/는",
            "{{@조사:{{수요기관}}:은/는}}",
            "{{@조사:수요기관\n:은/는}}",
        ] {
            assert!(
                !inspect(tpl)["problems"].as_array().unwrap().is_empty(),
                "{tpl}"
            );
            assert!(
                make(tpl, &case(), &json!({}), &Map::new()).is_err(),
                "{tpl}"
            );
        }
        for name in [
            "@오늘",
            "@조사:수요기관",
            "@조사::은/는",
            "@조사:수요기관:는/은",
            "@조사:@조사:수요기관:은/는",
        ] {
            let tpl = format!("{{{{{name}}}}}");
            assert!(
                !inspect(&tpl)["problems"].as_array().unwrap().is_empty(),
                "{name}"
            );
            assert!(
                make(&tpl, &case(), &json!({}), &Map::new()).is_err(),
                "{name}"
            );
        }
        let tpl = "{{@조사:없는열:은/는}}";
        assert!(
            inspect(tpl)["problems"].as_array().unwrap().is_empty(),
            "사용자 열은 건을 받아야 알 수 있다"
        );
        assert!(
            make(tpl, &case(), &json!({}), &Map::new())
                .unwrap_err()
                .contains("조사 대상 필드가 없습니다")
        );
        let tpl = "{{#항목 a}}\n{{#선택 yes}}\n{{@조사:수요기관:은/는}}\n{{/선택}}\n{{#선택 no}}\n끝\n{{/선택}}\n{{/항목}}";
        let picks = Map::from_iter([("a".into(), json!("no"))]);
        let result = make(tpl, &case(), &json!({}), &picks).unwrap();
        assert_eq!(
            result["fields"],
            json!([]),
            "제외한 조사에는 값 확인을 요구하지 않는다"
        );
        let invalid = tpl.replace("수요기관:은/는", "없는열:은/는");
        assert!(
            make(&invalid, &case(), &json!({}), &picks).is_err(),
            "제외해도 서식 오류는 숨기지 않는다"
        );
    }
}

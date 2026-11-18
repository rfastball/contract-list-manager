//! 서식: 살피기(누름틀·항목), 받아들이기(본문의 `{{#항목}}` 표시를 구조로), 만들기(값 + 고른 항목 → 문서).
//!
//! 항목이 있는 서식은 문서나르미 `hwpx_authoring_trial::trial_hwpx` 로 만든다 — 문서나르미가 미리보기에 쓰는
//! 실행기·검증기와 같은 길이다(고르지 않은 선택 지우기 → 누름틀 채우기 → 다시 열어 검증). 항목이 없으면
//! [`crate::fill`] 의 빠른 길로 채운다.

use std::collections::HashMap;

use hwpx_core::package::HwpxPackage;
use hwpx_filler::hwpx_authoring_trial::trial_hwpx;
use hwpx_filler::hwpx_product_inspection::{SlotSnapshot, inspect_slot_regions};
use hwpx_filler::hwpx_structure::scan_structure;
use hwpx_filler::hwpx_structure_ops::compile_structure;
use serde_json::{Map, Value, json};

use crate::values::resolve;
use crate::vocab::{key, vocabulary};
use crate::{MISSING, field_names, open, read_back};

/// 검사에는 건이 없으므로 사용자 열의 존재는 생성할 때 확인한다.
pub(crate) fn inspect_fields(names: &[String]) -> (Vec<Value>, Vec<String>) {
    let v = vocabulary();
    let mut fields: Vec<Value> = Vec::new();
    let mut problems = Vec::new();
    for name in names {
        if fields.iter().any(|f| f["name"] == *name) {
            continue;
        }
        let token = match crate::josa::parse(name) {
            Ok(token) => token,
            Err(error) => {
                problems.push(error);
                None
            }
        };
        let target = token.as_ref().map_or(name.as_str(), |t| t.target);
        let canonical = v.canonical(target);
        let mut field = json!({ "name": name, "canonical": canonical, "group": canonical.map(|c| v.group_of(c)) });
        if let Some(token) = token {
            field["kind"] = json!("josa");
            field["target"] = json!(token.target);
            field["pair"] = json!(token.pair);
        }
        fields.push(field);
    }
    (fields, problems)
}

fn canonical<'a>(name: &str, available: &'a Value) -> Option<&'a str> {
    let wanted = key(name);
    vocabulary().canonical(name).or_else(|| {
        available
            .as_object()?
            .keys()
            .map(String::as_str)
            .find(|k| !wanted.is_empty() && key(k) == wanted)
    })
}

/// 선택에서 제외되는 토큰도 문법·대상 이름 오류를 숨기지 않는다.
pub(crate) fn validate_system_fields(names: &[String], available: &Value) -> Result<(), String> {
    for name in names {
        if let Some(token) = crate::josa::parse(name)?
            && canonical(token.target, available).is_none()
        {
            return Err(format!(
                "조사 대상 필드가 없습니다: {} ({name})",
                token.target
            ));
        }
    }
    Ok(())
}

fn slots(package: &HwpxPackage) -> Result<SlotSnapshot, String> {
    inspect_slot_regions(package).map_err(|e| format!("항목을 읽지 못했습니다: {e}"))
}

fn slot_json(snapshot: &SlotSnapshot) -> Value {
    Value::Array(
        snapshot
            .slots
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

/// 누름틀(쓸 수 있는 이름과 맞는지), 항목·선택, 아직 구조로 바꾸지 않은 본문 표시 수, 고쳐야 할 것.
pub fn inspect(template: &[u8]) -> Result<Value, String> {
    let package = open(template)?;
    let (fields, mut problems) = inspect_fields(&field_names(&package));
    let snapshot = slots(&package)?;
    let markers =
        scan_structure(&package).map_err(|e| format!("항목 표시를 읽지 못했습니다: {e}"))?;
    problems.extend(
        snapshot
            .diagnostics
            .iter()
            .map(|d| d.message.clone())
            .collect::<Vec<_>>(),
    );
    problems.extend(markers.diagnostics.iter().map(|d| d.message.clone()));
    Ok(json!({
        "fields": fields,
        "slots": slot_json(&snapshot),
        "markers": markers.placements.len(),
        "problems": problems,
    }))
}

/// 받아들이기: 본문에 적은 `{{#항목}}`·`{{#선택}}` 표시를 문서나르미와 같은 구조(책갈피 구역)로 바꾼다. 패널은 서식 폴더의
/// 한글 서식을 읽을 때마다 이것으로 메모리에서 컴파일한다(ADR-049). 표시가 없으면 바이트를 그대로 돌려준다. 돌려주는 것: (서식 바이트, 살핀 결과).
pub fn prepare(template: &[u8]) -> Result<(Vec<u8>, Value), String> {
    let mut package = open(template)?;
    let report = compile_structure(&mut package)
        .map_err(|e| format!("항목 표시를 바꾸지 못했습니다: {e}"))?;
    if let Some(refusal) = report.refusal {
        let reasons: Vec<String> = refusal.into_iter().map(|r| r.message).collect();
        return Err(format!(
            "항목 표시를 바꾸지 못했습니다: {}",
            reasons.join(" / ")
        ));
    }
    let bytes = if report.modified {
        package
            .to_bytes()
            .map_err(|e| format!("서식을 쓰지 못했습니다: {e:?}"))?
    } else {
        template.to_vec()
    };
    let mut inspected = inspect(&bytes)?;
    inspected["compiled"] = json!(report.modified);
    Ok((bytes, inspected))
}

/// 서식의 누름틀마다 넣을 글: 쓸 수 있는 이름(통합 뷰의 열 · 내 정보)에 맞고 값이 있으면 그 값, 아니면 【확인 필요】.
/// 어휘(exe 의 정의)에 없는 이름은 건의 값에서 같은 이름을 찾는다 — 사람이 「문서 서식」 에 더한 열이다(ADR-052).
pub(crate) fn document_values(
    names: &[String],
    resolved: &Map<String, Value>,
    available: &Value,
) -> Result<(Map<String, Value>, Vec<Value>), String> {
    validate_system_fields(names, available)?;
    let mut values = Map::new();
    let mut report = Vec::new();
    for name in names {
        if values.contains_key(name) {
            continue;
        }
        let token = crate::josa::parse(name)?;
        let target = token.as_ref().map_or(name.as_str(), |t| t.target);
        let canonical = canonical(target, available);
        let found = canonical
            .and_then(|c| resolved.get(c))
            .and_then(Value::as_str);
        let line = if let Some(token) = token {
            let (value, reason) = token.select(found);
            json!({ "field": name, "canonical": canonical, "value": value, "missing": reason.is_some(),
                "kind": "josa", "target": token.target, "pair": token.pair,
                "status": if reason.is_some() { "needs_review" } else { "selected" }, "reason": reason })
        } else {
            json!({ "field": name, "canonical": canonical, "value": found.unwrap_or(MISSING), "missing": found.is_none() })
        };
        values.insert(name.clone(), line["value"].clone());
        report.push(line);
    }
    Ok((values, report))
}

/// 항목 고르기를 검사한다. 선택이 있는 항목은 하나를 꼭 골라야 한다(문서나르미와 같다 — 비울 수 있는 항목은
/// 서식 작성자가 빈 선택을 넣는다). `slots` 는 (항목 id, 이름, 선택 id들). 한글·글 서식이 같이 쓴다.
pub(crate) fn choices<'a>(
    slots: impl IntoIterator<Item = (&'a str, &'a str, Vec<&'a str>)>,
    selections: &Map<String, Value>,
) -> Result<HashMap<String, String>, String> {
    let mut chosen = HashMap::new();
    let mut ids = Vec::new();
    for (id, label, options) in slots {
        ids.push(id);
        if options.is_empty() {
            continue;
        }
        match selections.get(id).and_then(Value::as_str) {
            Some(option) if options.contains(&option) => {
                chosen.insert(id.to_string(), option.to_string());
            }
            Some(_) => return Err(format!("「{label}」에 없는 선택입니다. 다시 골라 주세요.")),
            None => return Err(format!("「{label}」을 골라 주세요.")),
        }
    }
    if let Some(unknown) = selections.keys().find(|k| !ids.contains(&k.as_str())) {
        return Err(format!("서식에 없는 항목을 골랐습니다: {unknown}"));
    }
    Ok(chosen)
}

fn check_selections(
    snapshot: &SlotSnapshot,
    selections: &Map<String, Value>,
) -> Result<HashMap<String, String>, String> {
    choices(
        snapshot.slots.iter().map(|s| {
            (
                s.id.as_str(),
                s.label.as_deref().unwrap_or(&s.id),
                s.options.iter().map(|o| o.id.as_str()).collect(),
            )
        }),
        selections,
    )
}

/// 만들기. 돌려주는 것: (문서 바이트, 보고 `{fields, excluded, mismatch, path}`).
/// `mismatch` 가 0 이 아니면 패널은 저장하지 않는다.
pub fn make(
    template: &[u8],
    case: &Value,
    context: &Value,
    selections: &Map<String, Value>,
) -> Result<(Vec<u8>, Value), String> {
    let package = open(template)?;
    let resolved = resolve(case, context);
    let names = field_names(&package);
    validate_system_fields(&names, &case["values"])?;
    let snapshot = slots(&package)?;
    if !snapshot.diagnostics.is_empty() {
        return Err("서식의 항목 구조가 깨져 있습니다. 서식을 다시 넣어 주세요.".into());
    }
    if snapshot.slots.is_empty() {
        let (values, report) = document_values(&names, &resolved, &case["values"])?;
        let (doc, filled) = crate::fill(template, &values)?;
        return Ok((
            doc,
            json!({ "fields": report, "excluded": [], "mismatch": filled["mismatch"], "path": filled["path"] }),
        ));
    }
    let chosen = check_selections(&snapshot, selections)?;
    // 고른 뒤에 남는 누름틀만 값이 있어야 하지만, 모든 이름에 값을 주어도 실행기는 남은 것만 쓴다.
    let (values, report) = document_values(&names, &resolved, &case["values"])?;
    let trial = trial_hwpx(&package, &values, &chosen)
        .map_err(|e| format!("문서를 만들지 못했습니다: {e}"))?;
    let readback = read_back(&trial.bytes)?;
    let mismatch = readback
        .iter()
        .filter(|r| {
            let field = r["field"].as_str().unwrap_or_default();
            values.get(field) != Some(&r["text"])
        })
        .count();
    let kept: Vec<String> = readback
        .iter()
        .filter_map(|r| r["field"].as_str().map(str::to_owned))
        .collect();
    let report: Vec<Value> = report
        .into_iter()
        .filter(|line| kept.iter().any(|k| line["field"] == k.as_str()))
        .collect();
    Ok((
        trial.bytes,
        json!({
            "fields": report,
            "excluded": trial.excluded.iter().map(|e| json!({ "slot": e["slot_id"], "option": e["option_id"], "label": e["label"] })).collect::<Vec<_>>(),
            "mismatch": mismatch,
            "path": "slots",
        }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 사람이 「문서 서식」 에 더한 열은 어휘에 없어도 건의 값에서 찾는다. 어휘의 열이 먼저다.
    #[test]
    fn names_outside_the_vocabulary_come_from_the_case() {
        let resolved: Map<String, Value> =
            serde_json::from_value(json!({ "납품 장소": "지어낸 창고", "계약금액": "1,000원" }))
                .unwrap();
        let names = ["납품장소", "계약 금액", "없는 칸"].map(String::from);
        let (values, report) = document_values(&names, &resolved, &json!(resolved)).unwrap();
        assert_eq!(values["납품장소"], "지어낸 창고");
        assert_eq!(report[0]["canonical"], "납품 장소");
        assert_eq!(values["계약 금액"], "1,000원");
        assert_eq!(values["없는 칸"], MISSING);
    }

    #[test]
    fn 조사쌍은_대상의_끝음절만_판정하고_불확실하면_병기를_남긴다() {
        let pairs = ["은/는", "이/가", "을/를", "과/와", "으로/로"];
        let names = pairs.map(|p| format!("@조사:수요기관:{p}"));
        for (word, expected, reason) in [
            ("국방부", ["는", "가", "를", "와", "로"], None),
            ("조달청", ["은", "이", "을", "과", "으로"], None),
            ("서울", ["은", "이", "을", "과", "로"], None),
            ("30,000원", ["은", "이", "을", "과", "으로"], None),
            ("ABC코리아 \t", ["는", "가", "를", "와", "로"], None),
            ("서울", ["은", "이", "을", "과", "로"], None),
            ("서울", ["은", "이", "을", "과", "로"], None),
            ("부", ["는", "가", "를", "와", "로"], None),
            (
                "",
                ["은(는)", "이(가)", "을(를)", "과(와)", "(으)로"],
                Some("missing_value"),
            ),
            (
                MISSING,
                ["은(는)", "이(가)", "을(를)", "과(와)", "(으)로"],
                Some("missing_value"),
            ),
        ] {
            let case = json!({"values": {"수요기관": word}});
            let (values, report) =
                document_values(&names, &resolve(&case, &json!({})), &case["values"]).unwrap();
            for (i, name) in names.iter().enumerate() {
                assert_eq!(values[name], expected[i], "{word}: {name}");
                assert_eq!(report[i]["reason"], json!(reason));
                assert_eq!(report[i]["missing"], reason.is_some());
            }
        }
        for word in [
            "10",
            "ABC",
            "Company",
            "국방부(본부)",
            "‘국방부’",
            "부.",
            "ᆯ",
            "ᅮᆯ",
        ] {
            let case = json!({"values": {"수요기관": word}});
            let (_, report) =
                document_values(&names, &resolve(&case, &json!({})), &case["values"]).unwrap();
            assert!(
                report.iter().all(
                    |r| r["status"] == "needs_review" && r["reason"] == "unknown_pronunciation"
                ),
                "{word}"
            );
        }
    }
}

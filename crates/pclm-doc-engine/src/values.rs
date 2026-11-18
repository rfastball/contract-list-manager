//! 건(호스트 `document {op:"read"}` 의 답) + 내 정보 → 이름마다 문서에 넣을 글.
//!
//! 건의 꼴: `{entityType, entity, values, rows, older, caseKeys}`. `values` 는 통합 뷰의 한 줄(열 이름 → 글)이고 문서에는
//! 그 글을 그대로 넣는다 — 금액의 「원」 · 공문서 날짜 · 한글 금액 · 오늘도 뷰가 짓는다. 엔진은 값을 바꾸거나 더하지
//! 않는다(ADR-051): 뷰의 글과 문서의 글이 같은 이름으로 다르면, 창에서 본 것이 문서에 찍힌다고 믿을 수 없다.
//! 빈 글은 값이 없는 것이다 — 그 칸은 【확인 필요】로 남는다.

use serde_json::{Map, Value};

use crate::vocab::vocabulary;

/// `case` 는 호스트 `document {op:"read"}` 의 답, `context` = `{me: {dept, name, phone, fax}}`.
/// 돌려주는 것: 이름 → 문서에 넣을 글(값이 있는 것만).
pub fn resolve(case: &Value, context: &Value) -> Map<String, Value> {
    let mut values: Map<String, Value> = case["values"]
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(_, value)| value.as_str().is_some_and(|t| !t.trim().is_empty()))
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect();
    for m in &vocabulary().me {
        if let Some(text) = context["me"][&m.key]
            .as_str()
            .filter(|t| !t.trim().is_empty())
        {
            values.insert(m.name.clone(), Value::String(text.trim().to_string()));
        }
    }
    values
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn 통합_뷰의_값에_내_정보만_더한다() {
        let case = json!({
            "entityType": "notice", "entity": "R26BK00000001-000",
            "values": { "건명": "청사 방역 용역", "추정가격": "24,545,454원", "게시일시": "2026. 10. 20. 09:30",
                        "추정가격_한글": "이천사백오십사만오천사백오십사", "계약금액": "", "세부품명": "방역 외 1건" },
            "rows": 1, "older": false, "caseKeys": ["notice:R26BK00000001"]
        });
        let r = resolve(&case, &json!({ "me": { "name": " 김계약 " } }));
        let get = |k: &str| r.get(k).and_then(Value::as_str).map(str::to_owned);
        assert_eq!(get("내_이름").as_deref(), Some("김계약"));
        assert_eq!(get("내_부서"), None);
        assert_eq!(get("계약금액"), None, "빈 글은 값이 없는 것이다");
        assert_eq!(r.len(), 5 + 1);
    }

    #[test]
    fn 뷰의_값은_하나도_바꾸지_않는다() {
        // 엔진이 꾸미던 꼴(금액 · 날짜 · 일시 · 오늘)의 글도 뷰가 낸 그대로다.
        let values = json!({
            "추정가격": "24,545,454", "계약금액": "114,500", "사업예산": "별도 공지", "게시일시": "2026/10/20 09:30:00",
            "게시일": "2026/10/20", "계약보증금납부기한": "2026-01-05", "납품기한": "계약 후 90일 이내",
            "개찰일시": "2026. 10. 27. 10:00:15", "오늘_년월": "2026. 11.", "수량": " 120 "
        });
        let r = resolve(&json!({ "values": values }), &json!({}));
        assert_eq!(Value::Object(r), values);
    }
}

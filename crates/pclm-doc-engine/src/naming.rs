//! 저장할 파일 이름. 서식마다의 패턴(패널 아랫단의 파일 이름 줄에서 사람이 정한 것, 없으면 동봉 서식의 기본)을 따르고, 빈 패턴이면 `{{건명}}_{{서식이름}}` 이다.
//! 같은 건·같은 서식·같은 패턴이면 늘 같은 이름이라, 정정공고·변경계약 뒤에 다시 만들면 같은 파일을 덮어쓴다(덮기 전에 묻는다).
//!
//! 패턴의 해석과 그리기는 hwpx-filler(filename-pattern/v1)의 것을 쓴다. 쓸 수 있는 것은 `{{칸 이름}}`(통합 뷰의 열),
//! `{{서식이름}}`, `{{date:…}}`(패널이 넘기는 오늘) 이다. 파일 하나만 만드니 `{{seq}}` 는 없는 칸으로 본다.

use hwpx_filler::copy_name::device_safe_file_name;
use hwpx_filler::output_name::{
    OUTPUT_EXTENSION, OUTPUT_NAME_PATTERN_INVALID, PatternToken, clean_filename,
    parse_filename_pattern, render_output_name,
};
use serde_json::{Map, Value};

/// 이름 부분(확장자 앞)의 글자 수 상한. Windows 경로 260자 안에 폴더 이름이 들어갈 자리를 남긴다.
const STEM_LIMIT: usize = 100;
pub const DEFAULT_PATTERN: &str = "{{건명}}_{{서식이름}}";
/// 빈 칸의 자리 표시. 값에서 제어 글자를 걷으므로(tidy) 값과 겹치지 않는다.
const GAP: char = '\u{1}';
const SEP: [char; 2] = ['_', ' '];

/// 이름을 짓지 못한 까닭. 패널이 한 줄로 말한다.
#[derive(Debug, PartialEq, Eq)]
pub enum NameError {
    /// 그 건에 없는 칸(들) — 패턴에 적힌 그대로, 적힌 차례로.
    Missing(Vec<String>),
    /// 패턴의 문법 · 날짜 꼴이 틀렸다.
    Pattern(String),
}

fn tidy(text: &str) -> String {
    let cleaned: String = clean_filename(text)
        .chars()
        .filter(|c| !c.is_control())
        .collect();
    let collapsed: Vec<&str> = cleaned.split_whitespace().collect();
    collapsed.join(" ").trim_matches(['.', ' ']).to_string()
}

/// 빈 칸이 남긴 자리마다, 그 때문에 겹친 구분(`_` · 공백)과 맨 앞뒤에 남은 구분을 걷는다.
fn close_gaps(text: &str) -> String {
    let mut s = text.to_string();
    while let Some(at) = s.rfind(GAP) {
        s.remove(at);
        let left = s[..at].trim_end_matches(SEP).len();
        let right = at + (s[at..].len() - s[at..].trim_start_matches(SEP).len());
        if left == 0 || right == s.len() {
            s.replace_range(left..right, "");
        } else if left < at && right > at {
            s.replace_range(at..right, "");
        }
    }
    s
}

/// `case` 는 [`crate::values::resolve`] 와 같은 건. `pattern` 이 비면 기본 패턴, `today` 는 `2026-11-09` 꼴.
pub fn file_name(
    case: &Value,
    template_name: &str,
    pattern: Option<&str>,
    today: Option<&str>,
) -> Result<String, NameError> {
    // 건명이 비면 탭의 번호를 쓴다.
    let title = [case["values"]["건명"].as_str(), case["entity"].as_str()]
        .into_iter()
        .flatten()
        .map(tidy)
        .find(|t| !t.is_empty())
        .unwrap_or_else(|| "이름 없는 건".into());
    let template = tidy(
        template_name
            .strip_suffix(OUTPUT_EXTENSION)
            .unwrap_or(template_name),
    );

    let mut fields = Map::new();
    if let Some(values) = case["values"].as_object() {
        for (name, value) in values {
            fields.insert(
                name.clone(),
                Value::String(tidy(value.as_str().unwrap_or_default())),
            );
        }
    }
    fields.insert("건명".into(), Value::String(title.clone()));
    fields.insert("서식이름".into(), Value::String(template));

    let pattern = pattern
        .filter(|p| !p.trim().is_empty())
        .unwrap_or(DEFAULT_PATTERN);
    let tokens = parse_filename_pattern(&Value::String(pattern.into())).map_err(|e| {
        NameError::Pattern(if e.code == Some(OUTPUT_NAME_PATTERN_INVALID) {
            "파일 이름의 {{ }} 가 닫히지 않았거나 비어 있습니다".into()
        } else {
            e.detail
        })
    })?;
    let mut missing: Vec<String> = Vec::new();
    for token in &tokens {
        let name = match token {
            PatternToken::Field(id) if !fields.contains_key(id) => id.clone(),
            PatternToken::Sequence(pad) => {
                pad.as_ref().map_or("seq".into(), |p| format!("seq:{p}"))
            }
            _ => continue,
        };
        if !missing.contains(&name) {
            missing.push(name);
        }
    }
    if !missing.is_empty() {
        return Err(NameError::Missing(missing));
    }
    for value in fields.values_mut() {
        if value.as_str() == Some("") {
            *value = Value::String(GAP.into());
        }
    }
    let now = today.and_then(|t| format!("{t}T00:00:00").parse().ok());
    let (name, _) = render_output_name(&tokens, &fields, 0, now, false)
        .map_err(|_| NameError::Pattern("파일 이름의 날짜 꼴을 읽지 못했습니다".into()))?;
    // 패턴에 적은 글자에도 금지 글자가 있을 수 있다. 확장자는 늘 소문자 .hwpx 로 붙인다.
    let stem = clean_filename(&name[..name.len() - OUTPUT_EXTENSION.len()]);
    let stem = close_gaps(&stem);
    let stem = if stem.trim_matches(['.', ' ']).is_empty() {
        title
    } else {
        stem
    };
    let stem: String = stem.chars().take(STEM_LIMIT).collect();
    Ok(device_safe_file_name(&format!(
        "{}{OUTPUT_EXTENSION}",
        stem.trim_end_matches(['.', ' '])
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn name(case: &Value, template: &str) -> String {
        file_name(case, template, None, None).unwrap()
    }

    #[test]
    fn 이름은_건과_서식을_따른다() {
        let case =
            json!({ "entity": "R26BK00000001-000", "values": { "건명": "청사 방역: 용역/2차" } });
        assert_eq!(
            name(&case, "검수 조서.hwpx"),
            "청사 방역_ 용역_2차_검수 조서.hwpx"
        );
        let unnamed = json!({ "entity": "R26TA0000000200", "values": { "건명": "" } });
        assert_eq!(
            name(&unnamed, "계약 의뢰서"),
            "R26TA0000000200_계약 의뢰서.hwpx"
        );
        assert_eq!(name(&json!({}), "CON"), "이름 없는 건_CON.hwpx");
        let long = json!({ "values": { "건명": "가".repeat(300) } });
        assert_eq!(name(&long, "서식").chars().count(), STEM_LIMIT + 5);
    }

    /// 패턴 전의 이름 짓기 그대로(95a6788) — 기본 패턴이 이것과 글자 하나까지 같아야 한다.
    fn old(case: &Value, template_name: &str) -> String {
        let old_tidy = |text: &str| {
            let cleaned = clean_filename(text);
            let collapsed: Vec<&str> = cleaned.split_whitespace().collect();
            collapsed.join(" ").trim_matches(['.', ' ']).to_string()
        };
        let title = [case["values"]["건명"].as_str(), case["entity"].as_str()]
            .into_iter()
            .flatten()
            .map(old_tidy)
            .find(|t| !t.is_empty())
            .unwrap_or_else(|| "이름 없는 건".into());
        let template = old_tidy(template_name.strip_suffix(".hwpx").unwrap_or(template_name));
        let stem = if template.is_empty() {
            title
        } else {
            format!("{title}_{template}")
        };
        let stem: String = stem.chars().take(STEM_LIMIT).collect();
        device_safe_file_name(&format!("{}.hwpx", stem.trim_end_matches(['.', ' '])))
    }

    #[test]
    fn 기본_패턴은_옛_이름과_같다() {
        let cases = [
            json!({ "entity": "R26BK00000001-000", "values": { "건명": " 합성. 건명 ", "업체명": "" } }),
            json!({ "entity": "R26BK00000001-000", "values": { "건명": "a/b: c\t d__ _e." } }),
            json!({ "entity": "R26TA0000000200", "values": { "건명": " . " } }),
            json!({ "values": { "건명": "con" } }),
            json!({ "values": { "건명": "가".repeat(98) } }),
            json!({ "values": { "건명": format!("{}. 나", "가".repeat(98)) } }),
            json!({}),
        ];
        for case in &cases {
            for template in [
                "검수 조서.hwpx",
                "",
                ".hwpx",
                "a:b",
                "  ",
                "_x_",
                "가 . hwpx",
                "{{건명}}",
            ] {
                let expected = old(case, template);
                assert_eq!(name(case, template), expected, "{case} {template:?}");
                let explicit = file_name(case, template, Some(DEFAULT_PATTERN), Some("2026-11-09"));
                assert_eq!(explicit.unwrap(), expected);
                assert_eq!(
                    file_name(case, template, Some("  "), None).unwrap(),
                    expected
                );
            }
        }
        assert_eq!(name(&cases[0], ""), "합성. 건명.hwpx");
        assert_eq!(
            name(&json!({ "values": { "건명": "con" } }), ""),
            "con_.hwpx"
        );
    }

    #[test]
    fn 없는_칸과_seq_는_이름을_짓지_않고_칸을_돌려준다() {
        let case = json!({ "values": { "건명": "합성 건", "업체명": "" } });
        assert_eq!(
            file_name(
                &case,
                "서식",
                Some("{{업체}}_{{건명}}_{{담당}}_{{업체}}"),
                None
            ),
            Err(NameError::Missing(vec!["업체".into(), "담당".into()]))
        );
        assert_eq!(
            file_name(&case, "서식", Some("{{seq}}-{{seq:001}}"), None),
            Err(NameError::Missing(vec!["seq".into(), "seq:001".into()]))
        );
        for bad in ["{{건명", "a{{}}b", "{{a{b}}"] {
            assert!(
                matches!(
                    file_name(&case, "서식", Some(bad), None),
                    Err(NameError::Pattern(_))
                ),
                "{bad}"
            );
        }
        assert!(matches!(
            file_name(&case, "서식", Some("{{date:%q}}"), Some("2026-11-09")),
            Err(NameError::Pattern(_))
        ));
    }

    #[test]
    fn 빈_칸은_자리를_비우고_구분을_정리한다() {
        let case = json!({ "values": { "건명": "합성 건", "업체명": "", "계약번호": "C-1", "담당자": " " } });
        let n = |p: &str| file_name(&case, "검수 조서", Some(p), None).unwrap();
        assert_eq!(n("{{업체명}}_{{건명}}"), "합성 건.hwpx");
        assert_eq!(n("{{건명}}_{{업체명}}"), "합성 건.hwpx");
        assert_eq!(n("{{건명}}_{{업체명}}_{{계약번호}}"), "합성 건_C-1.hwpx");
        assert_eq!(
            n("{{건명}} {{업체명}} {{담당자}} {{서식이름}}"),
            "합성 건 검수 조서.hwpx"
        );
        assert_eq!(n("{{건명}}_{{업체명}}_{{담당자}}"), "합성 건.hwpx");
        assert_eq!(n("{{업체명}}_{{담당자}}_{{계약번호}}"), "C-1.hwpx");
        // 적은 글자의 구분은 빈 칸 때문이 아니면 그대로 둔다.
        assert_eq!(n("__{{건명}}"), "__합성 건.hwpx");
        // 모두 비면 건 이름으로 물러선다.
        assert_eq!(n("{{업체명}}"), "합성 건.hwpx");
    }

    #[test]
    fn 날짜는_패널이_넘긴_오늘로_적는다() {
        let case = json!({ "values": { "건명": "합성 건" } });
        let n = |p: &str| file_name(&case, "서식", Some(p), Some("2026-11-09")).unwrap();
        assert_eq!(n("{{date}}_{{건명}}"), "20261109_합성 건.hwpx");
        assert_eq!(n("{{건명}}_{{date:YYYY-MM-DD}}"), "합성 건_2026-11-09.hwpx");
        assert!(file_name(&case, "서식", Some("{{date}}"), None).is_err());
    }

    #[test]
    fn 금지_글자와_길이_한도() {
        let case = json!({ "values": { "건명": "합성 건", "업체명": "가/나:다", "긴칸": "가".repeat(300) } });
        let n = |p: &str| file_name(&case, "서식", Some(p), None).unwrap();
        assert_eq!(n("{{업체명}}<{{건명}}>"), "가_나_다_합성 건_.hwpx");
        assert_eq!(n("{{긴칸}}").chars().count(), STEM_LIMIT + 5);
        assert_eq!(n("{{건명}}.HWPX"), "합성 건.hwpx");
        assert_eq!(n("CON"), "CON_.hwpx");
    }
}

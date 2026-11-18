//! ADR-054: 명시한 필드의 최종 값으로 조사만 선택한다. 주변 본문은 읽지 않는다.

pub(crate) struct Josa<'a> {
    pub target: &'a str,
    pub pair: &'a str,
    forms: (&'static str, &'static str, &'static str),
}

pub(crate) fn parse(name: &str) -> Result<Option<Josa<'_>>, String> {
    let name = name.trim();
    if !name.starts_with('@') {
        return Ok(None);
    }
    let invalid = || format!("시스템 토큰을 고쳐 주세요: {name} (@조사:대상필드:은/는)");
    let body = name.strip_prefix("@조사:").ok_or_else(invalid)?;
    // 마지막 ':'만 구분한다 — 대상 열 이름에 ':'가 있어도 일반 필드와 같은 이름이다.
    let (target, pair) = body.rsplit_once(':').ok_or_else(invalid)?;
    if crate::vocab::key(target).is_empty()
        || target.trim_start().starts_with('@')
        || target.contains(['{', '}', '\r', '\n'])
    {
        return Err(invalid());
    }
    let forms = match pair {
        "은/는" => ("은", "는", "은(는)"),
        "이/가" => ("이", "가", "이(가)"),
        "을/를" => ("을", "를", "을(를)"),
        "과/와" => ("과", "와", "과(와)"),
        "으로/로" => ("으로", "로", "(으)로"),
        _ => return Err(invalid()),
    };
    Ok(Some(Josa {
        target,
        pair,
        forms,
    }))
}

impl Josa<'_> {
    pub fn select(&self, value: Option<&str>) -> (&'static str, Option<&'static str>) {
        let Some(value) = value.filter(|v| !v.trim().is_empty() && *v != crate::MISSING) else {
            return (self.forms.2, Some("missing_value"));
        };
        match batchim(value.trim_end()) {
            Some(0) => (self.forms.1, None),
            Some(8) if self.pair == "으로/로" => (self.forms.1, None),
            Some(_) => (self.forms.0, None),
            None => (self.forms.2, Some("unknown_pronunciation")),
        }
    }
}

/// 끝 음절의 NFC/NFD 한글만 판정한다. 원문은 바꾸지 않고, 기호나 괄호는 벗기지 않는다.
fn batchim(value: &str) -> Option<u32> {
    let mut chars = value.chars().rev();
    let last = chars.next()? as u32;
    if (0xac00..=0xd7a3).contains(&last) {
        return Some((last - 0xac00) % 28);
    }
    let trailing = (0x11a8..=0x11c2).contains(&last);
    let vowel = if trailing { chars.next()? as u32 } else { last };
    if trailing && (0xac00..=0xd7a3).contains(&vowel) && (vowel - 0xac00) % 28 == 0 {
        return Some(last - 0x11a7);
    }
    if (0x1161..=0x1175).contains(&vowel) && (0x1100..=0x1112).contains(&(chars.next()? as u32)) {
        return Some(if trailing { last - 0x11a7 } else { 0 });
    }
    None
}

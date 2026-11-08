//! `JsonSerializer.Deserialize<T>` 가 꼴이 다른 값에 던지는 글 — `The JSON value could not be converted to {T}. Path: {길} |
//! LineNumber: {줄} | BytePositionInLine: {바이트}.` 줄과 바이트는 원문에서 그 값을 연 토큰이 끝난 자리다(`{`·`[` 면 그 한 바이트
//! 뒤, 글 · 수 · 참거짓 · null 이면 그 토큰 뒤). 줄은 `\n` 을 센다(0부터), 바이트는 그 줄의 처음부터 센 UTF-8 바이트다.
//!
//! 깨진 JSON(문법 오류)의 글은 흉내 내지 않는다 — 다리가 받는 값은 화면이 `JSON.stringify` 로 지은 것이다.

/// 값에 이르는 길의 한 마디.
#[derive(Clone, Debug)]
pub(crate) enum Step {
    Key(String),
    Index(usize),
}

/// 그 길의 값이 `target` 으로 바뀌지 않는다는 .NET 의 글.
pub(crate) fn not_converted(text: &str, path: &[Step], target: &str) -> String {
    let mut shown = String::from("$");
    for step in path {
        match step {
            Step::Key(key) => {
                shown.push('.');
                shown.push_str(key);
            }
            Step::Index(i) => shown.push_str(&format!("[{i}]")),
        }
    }
    let end = locate(text.as_bytes(), 0, path).unwrap_or(0);
    message(text, end, &shown, target)
}

/// 뿌리 객체의 칸 — 원문 차례 그대로 `(열쇠, 값이 시작하는 자리)`. 같은 열쇠가 둘이어도 둘 다 낸다(.NET 은 차례로 읽어 뒤의
/// 것이 이긴다). 객체가 아니면 `None`.
pub(crate) fn members(text: &str) -> Option<Vec<(String, usize)>> {
    let bytes = text.as_bytes();
    let at = skip_space(bytes, 0);
    if bytes.get(at) != Some(&b'{') {
        return None;
    }
    let mut out = Vec::new();
    let mut i = at + 1;
    loop {
        i = skip_space(bytes, i);
        if bytes.get(i) != Some(&b'"') {
            return Some(out);
        }
        let key = decode(bytes, i)?;
        i = skip_space(bytes, string_end(bytes, i));
        if bytes.get(i) != Some(&b':') {
            return Some(out);
        }
        i = skip_space(bytes, i + 1);
        out.push((key, i));
        i = skip_space(bytes, skip_value(bytes, i));
        if bytes.get(i) != Some(&b',') {
            return Some(out);
        }
        i += 1;
    }
}

/// `start` 에서 시작하는 값의 원문.
pub(crate) fn value_text(text: &str, start: usize) -> &str {
    &text[start..skip_value(text.as_bytes(), start)]
}

/// [`members`] 의 칸 하나가 바뀌지 않는다는 글 — `path` 는 이미 지은 길(`$.Enabled`).
pub(crate) fn member_not_converted(text: &str, start: usize, path: &str, target: &str) -> String {
    message(text, locate(text.as_bytes(), start, &[]).unwrap_or(start), path, target)
}

fn message(text: &str, end: usize, path: &str, target: &str) -> String {
    let before = &text.as_bytes()[..end.min(text.len())];
    let line = before.iter().filter(|&&b| b == b'\n').count();
    let column = before.iter().rposition(|&b| b == b'\n').map_or(end, |n| end - n - 1);
    format!(
        "The JSON value could not be converted to {target}. Path: {path} | LineNumber: {line} | BytePositionInLine: {column}."
    )
}

fn skip_space(text: &[u8], mut at: usize) -> usize {
    while at < text.len() && matches!(text[at], b' ' | b'\t' | b'\n' | b'\r') {
        at += 1;
    }
    at
}

/// `at` 에서 시작하는 글 토큰의 끝(닫는 따옴표 뒤).
fn string_end(text: &[u8], at: usize) -> usize {
    let mut i = at + 1;
    while i < text.len() {
        match text[i] {
            b'\\' => i += 2,
            b'"' => return i + 1,
            _ => i += 1,
        }
    }
    text.len()
}

/// 글 · 수 · 참거짓 · null 토큰의 끝.
fn literal_end(text: &[u8], mut at: usize) -> usize {
    while at < text.len() && !matches!(text[at], b',' | b']' | b'}' | b' ' | b'\t' | b'\n' | b'\r') {
        at += 1;
    }
    at
}

/// `at` 의 값을 통째로 건넌 뒤의 자리.
fn skip_value(text: &[u8], at: usize) -> usize {
    let at = skip_space(text, at);
    match text.get(at) {
        Some(b'"') => string_end(text, at),
        Some(b'{' | b'[') => {
            let mut depth = 0usize;
            let mut i = at;
            while i < text.len() {
                match text[i] {
                    b'"' => {
                        i = string_end(text, i);
                        continue;
                    }
                    b'{' | b'[' => depth += 1,
                    b'}' | b']' => {
                        depth -= 1;
                        if depth == 0 {
                            return i + 1;
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
            text.len()
        }
        Some(_) => literal_end(text, at),
        None => at,
    }
}

/// 글 토큰의 뜻(열쇠를 견주려고). 이스케이프는 `serde_json` 에 맡긴다.
fn decode(text: &[u8], at: usize) -> Option<String> {
    let end = string_end(text, at);
    serde_json::from_slice(&text[at..end]).ok()
}

/// 길을 따라가 그 값을 연 토큰이 끝난 자리. 같은 열쇠가 둘이면 뒤의 것이다(`serde_json::Value` 가 그렇게 읽는다).
fn locate(text: &[u8], at: usize, path: &[Step]) -> Option<usize> {
    let at = skip_space(text, at);
    let Some((step, rest)) = path.split_first() else {
        return Some(match text.get(at)? {
            b'{' | b'[' => at + 1,
            b'"' => string_end(text, at),
            _ => literal_end(text, at),
        });
    };
    match step {
        Step::Index(wanted) => {
            if text.get(at) != Some(&b'[') {
                return None;
            }
            let mut i = at + 1;
            let mut index = 0;
            loop {
                i = skip_space(text, i);
                if text.get(i) == Some(&b']') {
                    return None;
                }
                if index == *wanted {
                    return locate(text, i, rest);
                }
                i = skip_space(text, skip_value(text, i));
                if text.get(i) != Some(&b',') {
                    return None;
                }
                i += 1;
                index += 1;
            }
        }
        Step::Key(wanted) => {
            if text.get(at) != Some(&b'{') {
                return None;
            }
            let mut found = None;
            let mut i = at + 1;
            loop {
                i = skip_space(text, i);
                if text.get(i) != Some(&b'"') {
                    return found;
                }
                let key = decode(text, i);
                i = skip_space(text, string_end(text, i));
                if text.get(i) != Some(&b':') {
                    return found;
                }
                i = skip_space(text, i + 1);
                if key.as_deref() == Some(wanted.as_str()) {
                    found = locate(text, i, rest).or(found);
                }
                i = skip_space(text, skip_value(text, i));
                if text.get(i) != Some(&b',') {
                    return found;
                }
                i += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 값을_연_토큰의_끝을_센다() {
        assert_eq!(
            not_converted(r#"{"rowKey":"TESTK000100"}"#, &[], "Pclm.Core.Storage.CellChange[]"),
            "The JSON value could not be converted to Pclm.Core.Storage.CellChange[]. Path: $ | LineNumber: 0 | BytePositionInLine: 1."
        );
        assert_eq!(
            not_converted(r#"["TESTK000100"]"#, &[Step::Index(0)], "Pclm.Core.Storage.CellChange"),
            "The JSON value could not be converted to Pclm.Core.Storage.CellChange. Path: $[0] | LineNumber: 0 | BytePositionInLine: 14."
        );
        assert_eq!(
            not_converted(
                "[{\"a\": [1, {\"x\": \"}\"}],\n \"rowKey\" : 12}]",
                &[Step::Index(0), Step::Key("rowKey".into())],
                "System.String"
            ),
            "The JSON value could not be converted to System.String. Path: $[0].rowKey | LineNumber: 1 | BytePositionInLine: 14."
        );
        assert_eq!(
            not_converted("\"글\"", &[], "System.String[]"),
            "The JSON value could not be converted to System.String[]. Path: $ | LineNumber: 0 | BytePositionInLine: 5."
        );
    }
}

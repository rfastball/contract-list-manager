//! 명령줄이 글을 짓는 꼴 — 칸 맞춤은 UTF-16 단위로 세고, 소문자는 글자마다의 단순 대응이다.

/// 글의 길이 — UTF-16 단위의 수.
pub fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// `{s,-width}` — 오른쪽을 공백으로 채운다. 길면 그대로.
pub fn pad_right(s: &str, width: usize) -> String {
    let len = utf16_len(s);
    let mut out = s.to_string();
    out.extend(std::iter::repeat_n(' ', width.saturating_sub(len)));
    out
}

/// `{s,width}` — 왼쪽을 공백으로 채운다. 길면 그대로.
pub fn pad_left(s: &str, width: usize) -> String {
    let len = utf16_len(s);
    let mut out: String = std::iter::repeat_n(' ', width.saturating_sub(len)).collect();
    out.push_str(s);
    out
}

/// 언어와 무관한 소문자 — 글자마다 단순 대응이라 길이가 바뀌지 않는다. `İ`(U+0130)는 그대로 둔다
/// (터키어 i 를 빼고 짓는다 — 대문자 쪽이 `ı` 를 그대로 두는 것과 짝이다).
pub fn to_lower_invariant(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c == '\u{130}' {
                return c;
            }
            let mut lower = c.to_lowercase();
            match (lower.next(), lower.next()) {
                (Some(one), None) => one,
                _ => c,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 칸_맞춤은_utf16_단위로_센다() {
        assert_eq!(pad_right("진행상태", 12), "진행상태        ");
        assert_eq!(pad_right("😀", 3), "😀 ");
        assert_eq!(pad_left("7", 4), "   7");
        assert_eq!(pad_left("12345", 4), "12345");
        assert_eq!(
            pad_right("넘치는긴이름입니다넘치는", 10),
            "넘치는긴이름입니다넘치는"
        );
    }

    #[test]
    fn 소문자는_글자마다_바꾸고_터키어_i_는_둔다() {
        assert_eq!(to_lower_invariant("STATUS"), "status");
        assert_eq!(to_lower_invariant("현황"), "현황");
        assert_eq!(to_lower_invariant("PLAN\u{130}"), "plan\u{130}");
        assert_eq!(to_lower_invariant("ÄÖ"), "äö");
    }
}

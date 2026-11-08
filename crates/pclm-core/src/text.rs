//! .NET 문자열의 뜻을 옮긴 것. C# 의 `string` 은 UTF-16 이라 길이·차례·대소문자 무시가 Rust 의 `str` 과
//! 갈리는 자리가 있다 — 그 자리에서만 이것을 쓴다.

use std::cmp::Ordering;

/// `string.Length` — UTF-16 코드 단위 수.
pub(crate) fn utf16_len(s: &str) -> usize {
    s.chars().map(char::len_utf16).sum()
}

/// `s[..n]`(UTF-16 단위). 서로게이트 짝 가운데에서 끊기면 그 글자는 뺀다 — C# 은 짝 잃은 서로게이트를 남기지만
/// Rust 문자열에는 담을 수 없다(한글·영숫자 이름에서는 갈리지 않는다).
pub(crate) fn utf16_prefix(s: &str, n: usize) -> &str {
    let mut units = 0;
    for (i, c) in s.char_indices() {
        if units + c.len_utf16() > n {
            return &s[..i];
        }
        units += c.len_utf16();
    }
    s
}

/// `string.CompareOrdinal` / `StringComparer.Ordinal` — UTF-16 코드 단위 차례.
pub(crate) fn cmp_ordinal(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// `char.ToUpperInvariant` 의 단순 대응 — 한 글자가 여러 글자로 바뀌는 경우(ß)는 그대로 둔다.
fn upper(c: char) -> char {
    let mut u = c.to_uppercase();
    match (u.next(), u.next()) {
        (Some(one), None) => one,
        _ => c,
    }
}

/// `StringComparison.OrdinalIgnoreCase` 의 같음.
pub(crate) fn eq_ignore_case(a: &str, b: &str) -> bool {
    a.chars().map(upper).eq(b.chars().map(upper))
}

/// `StringComparer.OrdinalIgnoreCase` 로 모은 집합의 열쇠.
pub(crate) fn ignore_case_key(s: &str) -> String {
    s.chars().map(upper).collect()
}

/// `string.IsNullOrWhiteSpace` 의 공백 쪽.
pub(crate) fn is_blank(s: &str) -> bool {
    s.chars().all(char::is_whitespace)
}

/// C# 원시 문자열(`$"""` … `"""`)을 그대로 옮긴 글. 뷰 SQL 이 C# 과 **바이트까지** 같아야 해서(계약면 박제·동등성
/// 계기) 들여쓰기를 손으로 걷지 않고 C# 이 하는 대로 걷는다.
///
/// `template` 은 C# 원본의 여는 `"""` 다음 줄부터 닫는 `"""` 앞의 공백까지를 그대로 옮긴 것이다 — 첫 글자는 줄바꿈,
/// 마지막 줄은 닫는 자리의 들여쓰기(공백뿐)다. 줄마다 그 들여쓰기를 걷고(빈 줄은 빈 줄), 첫 줄바꿈과 마지막 줄을
/// 뗀다. 그다음 `{자리}` 를 `args` 의 값으로 채운다 — C# 처럼 **걷은 뒤에** 넣으므로 여러 줄 값은 제 줄바꿈 그대로
/// 들어간다. 모르는 자리나 남는 인자는 짓는 쪽의 잘못이라 멈춘다(시험이 잡는다).
pub(crate) fn raw(template: &str, args: &[(&str, &str)]) -> String {
    raw_with(template, args, &|_| None)
}

/// [`raw`] 에 더해, `args` 에 없는 자리를 `resolve` 에 묻는다 — 뷰는 C# 의 자리 글(`{Text("n.title")}`)을 그대로 두고
/// 그 꼴의 부름을 여기서 푼다.
pub(crate) fn raw_with(
    template: &str,
    args: &[(&str, &str)],
    resolve: &dyn Fn(&str) -> Option<String>,
) -> String {
    let body = template
        .strip_prefix('\n')
        .expect("원시 문자열은 줄바꿈으로 시작한다");
    let (content, indent) = body
        .rsplit_once('\n')
        .expect("원시 문자열은 닫는 줄로 끝난다");
    assert!(
        indent.chars().all(|c| c == ' '),
        "닫는 줄은 공백뿐이다: {indent:?}"
    );
    let mut text = String::with_capacity(content.len());
    for (i, line) in content.split('\n').enumerate() {
        if i > 0 {
            text.push('\n');
        }
        if !line.is_empty() {
            text.push_str(
                line.strip_prefix(indent)
                    .unwrap_or_else(|| panic!("들여쓰기가 모자란 줄: {line:?}")),
            );
        }
    }
    fill(&text, args, resolve)
}

/// `{자리}` 를 채운다. 자리는 `{` 에서 짝이 맞는 `}` 까지다(C# 의 자리 글이 `$"…{…}…"` 를 품을 수 있다). 채운 값은
/// 다시 훑지 않는다.
fn fill(text: &str, args: &[(&str, &str)], resolve: &dyn Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    let mut used = vec![false; args.len()];
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let mut depth = 1;
        let close = after
            .char_indices()
            .find(|&(_, c)| {
                match c {
                    '{' => depth += 1,
                    '}' => depth -= 1,
                    _ => {}
                }
                depth == 0
            })
            .map(|(i, _)| i)
            .filter(|&i| i > 0)
            .unwrap_or_else(|| panic!("닫히지 않은 자리: {:?}", &rest[open..]));
        let name = &after[..close];
        match args.iter().position(|(n, _)| *n == name) {
            Some(index) => {
                used[index] = true;
                out.push_str(args[index].1);
            }
            None => out.push_str(&resolve(name).unwrap_or_else(|| panic!("모르는 자리: {name}"))),
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    assert!(used.iter().all(|u| *u), "쓰지 않은 인자가 있다");
    out
}

/// `string.ReplaceLineEndings(replacement)` — CR·LF·CRLF·NEL·LS·PS·FF 를 하나씩 바꾼다(CRLF 는 한 번).
pub(crate) fn replace_line_endings(s: &str, replacement: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                out.push_str(replacement);
            }
            '\n' | '\u{0C}' | '\u{85}' | '\u{2028}' | '\u{2029}' => out.push_str(replacement),
            _ => out.push(c),
        }
    }
    out
}

/// `double.ToString("R", CultureInfo.InvariantCulture)`(.NET Core 3.0+) — 왕복 최단 자릿수. 소수점 자리(scale)가
/// `max(자릿수, 17)` 을 넘거나 -3 보다 작으면 지수 꼴(`1E+17`·`1E-05`·`1.2345678901234568E+17`)이다. 그 사이는 그냥
/// 적는다(`1000000000000000`·`10000000000000000` — .NET 8 실측). 인자 없는 `ToString()`·`Convert.ToString(d, Invariant)` 도
/// 같은 글이다.
/// `-0` 은 `-0`, 유한하지 않은 값은 `NaN`·`Infinity`·`-Infinity`.
pub(crate) fn dotnet_double_r(d: f64) -> String {
    if d.is_nan() {
        return "NaN".into();
    }
    if d.is_infinite() {
        return if d > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    let s = format!("{d:e}");
    let (mantissa, exponent) = s.split_once('e').unwrap_or((&s, "0"));
    let negative = mantissa.starts_with('-');
    let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    let exponent: i32 = exponent.parse().unwrap_or(0);
    let mut out = String::new();
    if negative {
        out.push('-');
    }
    if digits == "0" {
        out.push('0');
        return out;
    }
    let scale = exponent + 1;
    let max_digits = (digits.len() as i32).max(17);
    if scale > max_digits || scale < -3 {
        out.push_str(&digits[..1]);
        if digits.len() > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        out.push('E');
        out.push(if exponent < 0 { '-' } else { '+' });
        out.push_str(&format!("{:02}", exponent.unsigned_abs()));
    } else if scale <= 0 {
        out.push_str("0.");
        for _ in 0..-scale {
            out.push('0');
        }
        out.push_str(&digits);
    } else {
        let scale = scale as usize;
        if digits.len() <= scale {
            out.push_str(&digits);
            for _ in digits.len()..scale {
                out.push('0');
            }
        } else {
            out.push_str(&digits[..scale]);
            out.push('.');
            out.push_str(&digits[scale..]);
        }
    }
    out
}

/// `double.ToString("0", CultureInfo.InvariantCulture)` — 사용자 서식 `0` 은 유효 숫자 15자리로 반올림한 뒤 정수로
/// 적는다(`123456789012345678` → `123456789012346000`). 정수인 값에만 부른다. `-0` 은 `-0` 이다(.NET Core 3.0+).
pub(crate) fn dotnet_double_integer(d: f64) -> String {
    // 유효 숫자 15자리 — `{:.14e}` 는 가수 15자리로 반올림한다(짝수 반올림이 아니라 .NET 과 같은 0.5 올림은
    // 15자리를 넘는 수에서만 갈린다 — 정수 값의 16번째 자리부터라 1e15 이상에서만 생긴다).
    let s = format!("{d:.14e}");
    let (mantissa, exponent) = s.split_once('e').unwrap_or((&s, "0"));
    let negative = mantissa.starts_with('-');
    let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    let exponent: i32 = exponent.parse().unwrap_or(0);
    let mut out = String::new();
    if negative {
        out.push('-');
    }
    if exponent < 0 {
        out.push('0');
        return out;
    }
    let width = exponent as usize + 1;
    let digits = digits.trim_end_matches('0');
    if digits.is_empty() {
        out.push('0');
        return out;
    }
    for i in 0..width {
        out.push(digits.as_bytes().get(i).map_or('0', |b| *b as char));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinal_is_utf16_order() {
        // U+FF5E 는 UTF-16 에서 서로게이트(U+D83D…)보다 뒤다 — 코드 포인트 차례와 갈린다.
        assert_eq!(cmp_ordinal("\u{FF5E}", "\u{1F600}"), Ordering::Greater);
        assert_eq!(cmp_ordinal("a", "b"), Ordering::Less);
    }

    #[test]
    fn ignore_case_matches_dotnet_simple_mapping() {
        assert!(eq_ignore_case("Version", "version"));
        assert!(eq_ignore_case("\u{0131}d", "ID")); // ı → I
        assert!(!eq_ignore_case("straße", "STRASSE"));
    }

    #[test]
    fn prefix_counts_utf16_units() {
        assert_eq!(utf16_prefix("가나다", 2), "가나");
        assert_eq!(utf16_prefix("a\u{1F600}b", 2), "a");
    }

    #[test]
    fn raw_strips_like_csharp() {
        let t = raw(
            "
        CASE
            WHEN {c} = '' THEN ''

        END
        ",
            &[("c", "a\n  b")],
        );
        assert_eq!(t, "CASE\n    WHEN a\n  b = '' THEN ''\n\nEND");
    }

    #[test]
    fn line_endings_and_doubles_match_dotnet() {
        assert_eq!(
            replace_line_endings("a\r\nb\rc\nd\u{2028}e", " "),
            "a b c d e"
        );
        assert_eq!(dotnet_double_r(0.1), "0.1");
        assert_eq!(dotnet_double_r(1e15), "1000000000000000");
        assert_eq!(dotnet_double_r(1e16), "10000000000000000");
        assert_eq!(dotnet_double_r(12345678901234568.0), "12345678901234568");
        assert_eq!(dotnet_double_r(1e17), "1E+17");
        assert_eq!(
            dotnet_double_r(1.2345678901234568E+17),
            "1.2345678901234568E+17"
        );
        assert_eq!(dotnet_double_r(1234567890123456.0), "1234567890123456");
        assert_eq!(dotnet_double_r(0.0001), "0.0001");
        assert_eq!(dotnet_double_r(0.00001), "1E-05");
        assert_eq!(dotnet_double_r(1e-15), "1E-15");
        assert_eq!(dotnet_double_r(-2.5), "-2.5");
        assert_eq!(dotnet_double_integer(12.0), "12");
        assert_eq!(dotnet_double_integer(1e20), "100000000000000000000");
        assert_eq!(dotnet_double_integer(-0.0), "-0");
    }
}

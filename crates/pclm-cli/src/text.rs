//! C# 명령줄이 글을 짓는 꼴 — 칸 맞춤(`{x,-12}`·`{n,4}`)은 UTF-16 단위로 세고, `ToLowerInvariant` 는 글자마다의 단순
//! 대응이며, `0.#` 은 15자리로 줄인 뒤 반올림한다.

use std::cmp::Ordering;

use pclm_home::dotnet::to_upper_invariant;

/// .NET `string.Length` — UTF-16 단위의 수.
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

/// .NET `string.ToLowerInvariant()` — 글자마다 단순 대응이라 길이가 바뀌지 않는다. `İ`(U+0130)는 그대로 둔다
/// (.NET 의 ICU 불변 소문자가 터키어 i 를 빼고 짓는다 — `ToUpperInvariant` 가 `ı` 를 그대로 두는 것과 짝이다).
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

/// `StringComparer.OrdinalIgnoreCase` 의 차례 — 불변 대문자로 바꾼 UTF-16 단위의 차례.
pub fn cmp_ignore_case(a: &str, b: &str) -> Ordering {
    to_upper_invariant(a)
        .encode_utf16()
        .cmp(to_upper_invariant(b).encode_utf16())
}

/// `double.ToString("0.#")` — .NET Core 3.0 부터 사용자 꼴은 값을 유효 숫자 15자리로 먼저 줄이고, 그 숫자 글을 소수 한
/// 자리에서 반올림(5 이상 올림)한다. 소수 자리가 0 이면 소수점을 쓰지 않는다. 음수는 쓰는 자리가 없어 다루지 않는다.
pub fn format_0_hash(value: f64) -> String {
    if value == 0.0 || !value.is_finite() || value < 0.0 {
        return "0".into();
    }
    // 유효 숫자 15자리: d.dddddddddddddde±x
    let scientific = format!("{value:.14e}");
    let (mantissa, exponent) = scientific.split_once('e').unwrap_or((&scientific, "0"));
    let exponent: i32 = exponent.parse().unwrap_or(0);
    let mut digits: Vec<u8> = mantissa
        .bytes()
        .filter(u8::is_ascii_digit)
        .map(|b| b - b'0')
        .collect();
    // 소수점 위의 자리 수. digits[0] 이 10^exponent 의 자리다.
    let mut integer_digits = exponent + 1;
    // 소수 첫째 자리까지 남긴다.
    let keep = integer_digits + 1;
    if keep < 0 {
        return "0".into();
    }
    let keep = keep as usize;
    if keep < digits.len() {
        let round_up = digits[keep] >= 5;
        digits.truncate(keep);
        if round_up {
            let mut i = digits.len();
            loop {
                if i == 0 {
                    digits.insert(0, 1);
                    integer_digits += 1;
                    break;
                }
                i -= 1;
                if digits[i] == 9 {
                    digits[i] = 0;
                } else {
                    digits[i] += 1;
                    break;
                }
            }
        }
    }
    let digit_at = |position: i32| -> u8 {
        // position 0 이 가장 높은 자리.
        if position < 0 {
            0
        } else {
            digits.get(position as usize).copied().unwrap_or(0)
        }
    };
    let mut out = String::new();
    if integer_digits <= 0 {
        out.push('0');
    } else {
        for p in 0..integer_digits {
            out.push((b'0' + digit_at(p)) as char);
        }
    }
    let tenth = digit_at(integer_digits);
    if tenth != 0 {
        out.push('.');
        out.push((b'0' + tenth) as char);
    }
    out
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

    #[test]
    fn 영점샵_꼴은_dotnet_과_같다() {
        assert_eq!(format_0_hash(0.0), "0");
        assert_eq!(format_0_hash(0.04), "0");
        assert_eq!(format_0_hash(0.05), "0.1");
        assert_eq!(format_0_hash(0.25), "0.3");
        assert_eq!(format_0_hash(0.2734375), "0.3");
        assert_eq!(format_0_hash(0.4765625), "0.5");
        assert_eq!(format_0_hash(1.0), "1");
        assert_eq!(format_0_hash(9.96), "10");
        assert_eq!(format_0_hash(12.34), "12.3");
        assert_eq!(format_0_hash(99.95), "100");
        assert_eq!(format_0_hash(1234567.89), "1234567.9");
        // 15자리로 먼저 줄인다 — 0.15 는 이진으로 0.1499999… 이지만 15자리 글은 0.150000000000000 이라 올린다.
        assert_eq!(format_0_hash(0.15), "0.2");
        assert_eq!(format_0_hash(0.35), "0.4");
    }

    #[test]
    fn 대소문자_없는_차례() {
        assert_eq!(cmp_ignore_case("a.pclm", "B.pclm"), Ordering::Less);
        assert_eq!(cmp_ignore_case("A", "a"), Ordering::Equal);
    }
}

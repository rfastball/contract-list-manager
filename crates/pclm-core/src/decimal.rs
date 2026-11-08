//! .NET `decimal` 의 좁은 옮김 — 브리핑(`Reports.Dashboard`)이 금액 글을 읽고 더하고 다리에 싣는 만큼만.
//!
//! - **읽기**는 `decimal.TryParse(text, NumberStyles.Number, CultureInfo.InvariantCulture)` 다(.NET 8 실측): 앞뒤 공백
//!   (U+0009..U+000D·U+0020), 앞 **또는** 뒤의 부호 하나(`+`·`-`, 앞 부호 뒤에는 공백 없이), 숫자가 하나라도 나온 뒤의
//!   천 단위 쉼표(소수점 앞에서만, 몇 개든), 소수점 하나, 끝의 NUL. 지수·괄호·통화 기호·NBSP 는 받지 않는다.
//!   소수 자리는 적힌 그대로 지킨다(`1.10` 은 자리 둘). 96비트·자리 28 을 넘으면 짝수 쪽으로 반올림해 줄이고, 정수 부분이
//!   96비트를 넘으면 읽지 못한다. .NET 은 유효 숫자 29자리 뒤를 버린 채 반올림하므로 **30자리 넘는 글**에서만 갈릴 수 있다.
//! - **더하기**는 자리를 큰 쪽에 맞춘다(`1.50 + 2` = `3.50`).
//! - **쓰기**는 `Utf8JsonWriter.WriteNumberValue(decimal)`(`G`) — 자리를 지키고, 0 에는 부호를 붙이지 않는다(`-0.00` → `0.00`).

use std::cmp::Ordering;

/// 2^96 - 1.
const MAX: u128 = (1u128 << 96) - 1;
const MAX_SCALE: u32 = 28;

/// `decimal` 값 — 부호, 96비트 가수, 소수 자리.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decimal {
    negative: bool,
    mantissa: u128,
    scale: u32,
}

impl Decimal {
    pub const ZERO: Decimal = Decimal {
        negative: false,
        mantissa: 0,
        scale: 0,
    };

    /// `decimal.TryParse(text, NumberStyles.Number, CultureInfo.InvariantCulture)`. 읽지 못하면 `None`.
    pub fn parse_number(text: &str) -> Option<Decimal> {
        let chars: Vec<char> = text.chars().collect();
        let white = |c: char| matches!(c, '\u{9}'..='\u{d}' | ' ');
        let mut i = 0;
        let mut negative = false;
        let mut signed = false;
        // 앞: 공백과 부호 하나. 부호 뒤에는 공백을 받지 않는다(Invariant 의 NumberNegativePattern 은 1).
        while i < chars.len() {
            let c = chars[i];
            if white(c) && !signed {
                i += 1;
            } else if (c == '+' || c == '-') && !signed {
                negative = c == '-';
                signed = true;
                i += 1;
            } else {
                break;
            }
        }
        let mut integral = String::new();
        let mut fraction = String::new();
        let mut digits = false;
        let mut point = false;
        while i < chars.len() {
            let c = chars[i];
            if c.is_ascii_digit() {
                digits = true;
                if point {
                    fraction.push(c);
                } else {
                    integral.push(c);
                }
            } else if c == '.' && !point {
                point = true;
            } else if c == ',' && digits && !point {
            } else {
                break;
            }
            i += 1;
        }
        if !digits {
            return None;
        }
        // 뒤: 공백과(앞에 부호가 없었으면) 부호 하나.
        while i < chars.len() {
            let c = chars[i];
            if white(c) {
                i += 1;
            } else if (c == '+' || c == '-') && !signed {
                negative = c == '-';
                signed = true;
                i += 1;
            } else {
                break;
            }
        }
        if chars[i..].iter().any(|&c| c != '\0') {
            return None;
        }

        let integral = integral.trim_start_matches('0');
        if integral.len() > 29 {
            return None;
        }
        let mut mantissa: u128 = integral
            .bytes()
            .fold(0, |m, b| m * 10 + u128::from(b - b'0'));
        if mantissa > MAX {
            return None;
        }
        let fraction = fraction.as_bytes();
        let mut scale = 0usize;
        while scale < fraction.len() && scale < MAX_SCALE as usize {
            let next = mantissa * 10 + u128::from(fraction[scale] - b'0');
            if next > MAX {
                break;
            }
            mantissa = next;
            scale += 1;
        }
        // 남은 자리로 짝수 쪽 반올림.
        if let Some(&first) = fraction.get(scale) {
            let rest_nonzero = fraction[scale + 1..].iter().any(|&b| b != b'0');
            let up = match first.cmp(&b'5') {
                Ordering::Greater => true,
                Ordering::Equal => rest_nonzero || mantissa % 2 == 1,
                Ordering::Less => false,
            };
            if up {
                mantissa += 1;
                if mantissa > MAX {
                    if scale == 0 {
                        return None;
                    }
                    mantissa = div_round(mantissa, 10);
                    scale -= 1;
                }
            }
        }
        Some(Decimal {
            negative,
            mantissa,
            scale: scale as u32,
        })
    }

    /// `a + b` — 자리를 큰 쪽에 맞춘다. 96비트를 넘으면 자리를 줄이고, 정수로도 넘으면 `None`(.NET 은 `OverflowException`).
    pub fn checked_add(self, other: Decimal) -> Option<Decimal> {
        let (mut a, mut b) = (self, other);
        // 맞출 자리에서 넘치면 자리가 큰 쪽을 깎아 내려온다.
        while a.scale != b.scale {
            let (low, high) = if a.scale < b.scale {
                (&mut a, &mut b)
            } else {
                (&mut b, &mut a)
            };
            match low.mantissa.checked_mul(10).filter(|m| *m <= MAX) {
                Some(m) => {
                    low.mantissa = m;
                    low.scale += 1;
                }
                None => {
                    high.mantissa = div_round(high.mantissa, 10);
                    high.scale -= 1;
                }
            }
        }
        let scale = a.scale;
        let (negative, mut mantissa) = if a.negative == b.negative {
            (a.negative, a.mantissa + b.mantissa)
        } else if a.mantissa >= b.mantissa {
            (a.negative, a.mantissa - b.mantissa)
        } else {
            (b.negative, b.mantissa - a.mantissa)
        };
        let mut scale = scale;
        while mantissa > MAX {
            if scale == 0 {
                return None;
            }
            mantissa = div_round(mantissa, 10);
            scale -= 1;
        }
        Some(Decimal {
            negative,
            mantissa,
            scale,
        })
    }

    /// `decimal.ToString("G", Invariant)` — 자리를 지킨다. 0 에는 부호가 없다.
    pub fn to_text(self) -> String {
        let digits = self.mantissa.to_string();
        let scale = self.scale as usize;
        let mut out = String::new();
        if self.negative && self.mantissa != 0 {
            out.push('-');
        }
        if scale == 0 {
            out.push_str(&digits);
        } else if digits.len() > scale {
            out.push_str(&digits[..digits.len() - scale]);
            out.push('.');
            out.push_str(&digits[digits.len() - scale..]);
        } else {
            out.push_str("0.");
            for _ in digits.len()..scale {
                out.push('0');
            }
            out.push_str(&digits);
        }
        out
    }
}

/// `m / d` 의 짝수 쪽 반올림.
fn div_round(m: u128, d: u128) -> u128 {
    let (q, r) = (m / d, m % d);
    match (r * 2).cmp(&d) {
        Ordering::Greater => q + 1,
        Ordering::Equal if q % 2 == 1 => q + 1,
        _ => q,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(text: &str) -> Option<String> {
        Decimal::parse_number(text).map(Decimal::to_text)
    }

    /// .NET 8 실측(scratch 콘솔, `decimal.TryParse(…, NumberStyles.Number, InvariantCulture)` → `JsonSerializer`).
    #[test]
    fn reads_like_dotnet() {
        let cases: &[(&str, Option<&str>)] = &[
            ("1,000", Some("1000")),
            (",5", None),
            ("1,", Some("1")),
            (" 12 ", Some("12")),
            ("+5", Some("5")),
            ("5-", Some("-5")),
            ("-0", Some("0")),
            ("0.00", Some("0.00")),
            ("1.5e3", None),
            ("(5)", None),
            ("1 000", None),
            ("\u{a0}5", None),
            ("\t5\n", Some("5")),
            ("1,000.00.0", None),
            (".", None),
            ("", None),
            ("-", None),
            ("1.", Some("1")),
            (".5", Some("0.5")),
            ("0001.10", Some("1.10")),
            ("-0.00", Some("0.00")),
            ("1,2,3", Some("123")),
            ("1.2,3", None),
            ("+-5", None),
            ("5+", Some("5")),
            (" - 5", None),
            ("-5 ", Some("-5")),
            ("5 -", Some("-5")),
            ("- 5", None),
            ("\0", None),
            ("5\0", Some("5")),
            ("5 \0", Some("5")),
            (
                "79228162514264337593543950335",
                Some("79228162514264337593543950335"),
            ),
            ("79228162514264337593543950336", None),
            (
                "0.0000000000000000000000000001",
                Some("0.0000000000000000000000000001"),
            ),
            (
                "0.00000000000000000000000000001",
                Some("0.0000000000000000000000000000"),
            ),
            (
                "0.00000000000000000000000000005",
                Some("0.0000000000000000000000000000"),
            ),
            (
                "0.000000000000000000000000000051",
                Some("0.0000000000000000000000000001"),
            ),
            (
                "1234567890123456789012345678.95",
                Some("1234567890123456789012345679.0"),
            ),
            (
                "12345678901234567890123456789.5",
                Some("12345678901234567890123456790"),
            ),
            ("164,872,340", Some("164872340")),
            ("00", Some("0")),
            (
                "0.10000000000000000000000000000000000001",
                Some("0.1000000000000000000000000000"),
            ),
        ];
        for (text, expected) in cases {
            assert_eq!(read(text).as_deref(), *expected, "{text:?}");
        }
    }

    #[test]
    fn adds_at_the_larger_scale() {
        let a = Decimal::parse_number("1.50").unwrap();
        let b = Decimal::parse_number("2").unwrap();
        assert_eq!(a.checked_add(b).unwrap().to_text(), "3.50");
        let c = Decimal::parse_number("-5").unwrap();
        assert_eq!(b.checked_add(c).unwrap().to_text(), "-3");
        assert_eq!(Decimal::ZERO.to_text(), "0");
    }
}

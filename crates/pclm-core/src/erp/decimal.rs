//! 계약 총액의 셈(C# `decimal`) — 품목 금액의 합에서 10원 미만을 버린 것과 머리 총액을 견준다.
//!
//! 값은 (가수, 소수 자리) 한 쌍이다. .NET `decimal` 은 96비트 가수에 소수 자리 0~28 이고, 여기서 쓰는 것은 더하기·10 으로
//! 나눠 버리기·같은가 뿐이라 정확한 정수 셈으로 같은 답이 나온다.

use crate::error::Error;

/// .NET `decimal` 의 가수 한도(2^96 - 1).
const MAX: i128 = 79_228_162_514_264_337_593_543_950_335;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Decimal {
    mantissa: i128,
    scale: u32,
}

fn overflow() -> Error {
    Error::InvalidOperation("Value was either too large or too small for a Decimal.".into())
}

fn format_error(s: &str) -> Error {
    Error::InvalidOperation(format!(
        "The input string '{s}' was not in a correct format."
    ))
}

impl Decimal {
    pub(crate) const ZERO: Decimal = Decimal {
        mantissa: 0,
        scale: 0,
    };

    /// `decimal.Parse(s, CultureInfo.InvariantCulture)` — `NumberStyles.Number`: 앞뒤 공백, 앞 또는 뒤의 부호, 천 단위
    /// 쉼표, 소수점. 지수·괄호·통화 기호는 받지 않는다(`FormatException`).
    pub(crate) fn parse(s: &str) -> Result<Decimal, Error> {
        let ws = |c: char| matches!(c, '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ');
        let t = s.trim_matches(ws);
        let (mut negative, mut body) = match t.as_bytes().first() {
            Some(b'-') => (true, &t[1..]),
            Some(b'+') => (false, &t[1..]),
            _ => (false, t),
        };
        if body.len() == t.len() {
            if let Some(rest) = body.strip_suffix('-') {
                negative = true;
                body = rest;
            } else if let Some(rest) = body.strip_suffix('+') {
                body = rest;
            }
        }
        let (int_part, frac_part) = body.split_once('.').unwrap_or((body, ""));
        let int_ok = int_part.is_empty()
            || (int_part.as_bytes()[0].is_ascii_digit()
                && int_part.bytes().all(|b| b.is_ascii_digit() || b == b','));
        let digits: String = int_part.chars().filter(char::is_ascii_digit).collect();
        if !int_ok
            || !frac_part.bytes().all(|b| b.is_ascii_digit())
            || digits.len() + frac_part.len() == 0
        {
            return Err(format_error(s));
        }
        let mut scale = frac_part.len().min(28);
        loop {
            let all = format!("{digits}{}", &frac_part[..scale]);
            let all = all.trim_start_matches('0');
            if all.len() <= 29 {
                let kept: i128 = if all.is_empty() {
                    0
                } else {
                    all.parse().map_err(|_| overflow())?
                };
                let rest = &frac_part[scale..];
                let round_up = match rest.as_bytes().first() {
                    Some(&b) if b > b'5' => true,
                    Some(b'5') => rest[1..].bytes().any(|b| b != b'0') || kept % 2 == 1,
                    _ => false,
                };
                let mantissa = kept + i128::from(round_up);
                if mantissa <= MAX {
                    return Ok(Decimal {
                        mantissa: if negative { -mantissa } else { mantissa },
                        scale: scale as u32,
                    });
                }
            }
            if scale == 0 {
                return Err(overflow());
            }
            scale -= 1;
        }
    }

    fn rescale(self, scale: u32) -> Result<i128, Error> {
        let factor = 10i128
            .checked_pow(scale - self.scale)
            .ok_or_else(overflow)?;
        self.mantissa.checked_mul(factor).ok_or_else(overflow)
    }

    /// `a + b`.
    pub(crate) fn add(self, other: Decimal) -> Result<Decimal, Error> {
        let scale = self.scale.max(other.scale);
        let mantissa = self
            .rescale(scale)?
            .checked_add(other.rescale(scale)?)
            .ok_or_else(overflow)?;
        if mantissa.abs() > MAX {
            return Err(overflow());
        }
        Ok(Decimal { mantissa, scale })
    }

    /// `decimal.Truncate(x / 10) * 10` — 10원 미만을 버린 정수.
    pub(crate) fn truncate_tens(self) -> Decimal {
        let divisor = 10i128.pow(self.scale + 1);
        Decimal {
            mantissa: self.mantissa / divisor * 10,
            scale: 0,
        }
    }

    /// 수로 같은가(`1000.0 == 1000`).
    pub(crate) fn equals(self, other: Decimal) -> bool {
        let scale = self.scale.max(other.scale);
        match (self.rescale(scale), other.rescale(scale)) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        }
    }

    /// `ToString(CultureInfo.InvariantCulture)` — 소수 자리를 그대로 적는다. 음의 0 은 부호가 없다.
    pub(crate) fn to_text(self) -> String {
        let negative = self.mantissa < 0;
        let digits = format!(
            "{:0>width$}",
            self.mantissa.unsigned_abs(),
            width = self.scale as usize + 1
        );
        let (i, f) = digits.split_at(digits.len() - self.scale as usize);
        let mut out = String::new();
        if negative {
            out.push('-');
        }
        out.push_str(i);
        if self.scale > 0 {
            out.push('.');
            out.push_str(f);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_style_like_dotnet() {
        let ok = |s: &str| Decimal::parse(s).unwrap().to_text();
        assert_eq!(ok(" 1,000.50 "), "1000.50");
        assert_eq!(ok("5-"), "-5");
        assert_eq!(ok("-0"), "0");
        assert_eq!(ok(".5"), "0.5");
        assert!(Decimal::parse("1e3").is_err());
        assert!(Decimal::parse(",1").is_err());
        assert!(Decimal::parse("").is_err());
    }

    #[test]
    fn truncates_tens_toward_zero() {
        let sum = Decimal::parse("1234.5")
            .unwrap()
            .add(Decimal::parse("10").unwrap())
            .unwrap();
        assert_eq!(sum.truncate_tens().to_text(), "1240");
        assert_eq!(
            Decimal::parse("-15").unwrap().truncate_tens().to_text(),
            "-10"
        );
        assert!(
            Decimal::parse("1000.0")
                .unwrap()
                .equals(Decimal::parse("1000").unwrap())
        );
    }
}

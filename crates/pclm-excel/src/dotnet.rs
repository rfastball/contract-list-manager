//! 계획 엑셀의 칸을 글로 바꿀 때 C# 이 부르는 .NET 의 꼴 — 수의 `ToString("0.###…")`, `DateTime.FromOADate`,
//! ClosedXML 의 `TimeSpan.ToExcelString`.

/// `double.ToString("0.###########################", InvariantCulture)` — 사용자 지정 꼴은 유효 숫자 15자리로 반올림한
/// 뒤(.NET Core 3.0+), 소수 27자리까지 펴고 끝의 0 을 지운다. 음수가 0 으로 반올림되면 `-0` 이다.
pub fn number_text(value: f64) -> String {
    if value.is_nan() {
        return "NaN".into();
    }
    if value.is_infinite() {
        return if value > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    let negative = value.is_sign_negative();
    if value == 0.0 {
        return if negative { "-0" } else { "0" }.into();
    }
    // 유효 숫자 15자리(정확한 십진 전개를 짝수 쪽으로 반올림 — .NET 의 Dragon4 와 같다).
    let scientific = format!("{:.14e}", value.abs());
    let (mantissa, exponent) = scientific.split_once('e').unwrap_or((&scientific, "0"));
    let exponent: i32 = exponent.parse().unwrap_or(0);
    let mut digits: Vec<u8> = mantissa.bytes().filter(u8::is_ascii_digit).collect();
    while digits.len() > 1 && digits.last() == Some(&b'0') {
        digits.pop();
    }
    // 소수점은 첫 숫자 뒤 exponent 자리.
    let point = exponent + 1;
    let mut integer = String::new();
    let mut fraction: Vec<u8> = Vec::new();
    if point <= 0 {
        integer.push('0');
        fraction.extend(std::iter::repeat_n(b'0', (-point) as usize));
        fraction.extend(&digits);
    } else {
        let point = point as usize;
        for i in 0..point {
            integer.push(*digits.get(i).unwrap_or(&b'0') as char);
        }
        if digits.len() > point {
            fraction.extend(&digits[point..]);
        }
    }
    // 소수 27자리 — 넘으면 그 자리에서 반올림(.NET RoundNumber: 다음 숫자가 5 이상이면 올림).
    if fraction.len() > 27 {
        let up = fraction[27] >= b'5';
        fraction.truncate(27);
        if up {
            let mut carry = true;
            for d in fraction.iter_mut().rev() {
                if *d == b'9' {
                    *d = b'0';
                } else {
                    *d += 1;
                    carry = false;
                    break;
                }
            }
            if carry {
                // 정수부로 넘긴다 — 정수부는 이 자리에서 늘 "0" 이다(값이 1e-27 쪽이므로).
                integer = increment(&integer);
            }
        }
    }
    while fraction.last() == Some(&b'0') {
        fraction.pop();
    }
    let mut out = String::new();
    if negative {
        out.push('-');
    }
    out.push_str(&integer);
    if !fraction.is_empty() {
        out.push('.');
        out.push_str(std::str::from_utf8(&fraction).unwrap_or(""));
    }
    out
}

fn increment(integer: &str) -> String {
    let mut digits: Vec<u8> = integer.bytes().collect();
    for d in digits.iter_mut().rev() {
        if *d == b'9' {
            *d = b'0';
        } else {
            *d += 1;
            return String::from_utf8(digits).unwrap_or_default();
        }
    }
    let mut out = String::from("1");
    out.push_str(std::str::from_utf8(&digits).unwrap_or(""));
    out
}

/// 날짜와 시각(밀리초까지).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DateTime {
    pub year: i64,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    pub millisecond: u32,
}

impl DateTime {
    /// C# `PlanImport.Raw` 의 날짜 — 시각이 0 이면 `yyyy-MM-dd`, 아니면 `yyyy-MM-dd HH:mm:ss`(밀리초는 버린다).
    pub fn plan_text(self) -> String {
        let day = format!("{:04}-{:02}-{:02}", self.year, self.month, self.day);
        if self.hour == 0 && self.minute == 0 && self.second == 0 && self.millisecond == 0 {
            day
        } else {
            format!(
                "{day} {:02}:{:02}:{:02}",
                self.hour, self.minute, self.second
            )
        }
    }

    /// 0001-01-01 부터의 밀리초.
    pub fn millis(self) -> i64 {
        let days = days_from_civil(self.year, self.month, self.day) - days_from_civil(1, 1, 1);
        days * MILLIS_PER_DAY
            + i64::from(self.hour) * 3_600_000
            + i64::from(self.minute) * 60_000
            + i64::from(self.second) * 1000
            + i64::from(self.millisecond)
    }

    pub fn from_millis(millis: i64) -> DateTime {
        let days = millis.div_euclid(MILLIS_PER_DAY) + days_from_civil(1, 1, 1);
        let rest = millis.rem_euclid(MILLIS_PER_DAY);
        let (year, month, day) = civil_from_days(days);
        DateTime {
            year,
            month,
            day,
            hour: (rest / 3_600_000) as u32,
            minute: (rest % 3_600_000 / 60_000) as u32,
            second: (rest % 60_000 / 1000) as u32,
            millisecond: (rest % 1000) as u32,
        }
    }
}

const MILLIS_PER_DAY: i64 = 86_400_000;
/// 0001-01-01 에서 1899-12-30 까지의 밀리초(`DateTime.DoubleDateOffset / TicksPerMillisecond`).
const DOUBLE_DATE_OFFSET_MILLIS: i64 = 59_926_435_200_000;
/// 9999-12-31 끝(`DateTime.MaxMillis`).
const MAX_MILLIS: i64 = 315_537_897_600_000;

/// ClosedXML `ToSerialDateTime` — 1900 윤년 버그(60 = 1900-02-29)를 건너 `DateTime.FromOADate` 를 부른다. 글은 .NET 의 예외 글.
pub fn serial_date(value: f64) -> Result<DateTime, String> {
    if value >= 61.0 {
        from_oa_date(value)
    } else if value <= 60.0 {
        from_oa_date(value + 1.0)
    } else {
        Err("Serial date 60 is on a leap year of 1900 - date that doesn't exist and isn't representable in DateTime.".into())
    }
}

/// `DateTime.FromOADate`.
pub fn from_oa_date(value: f64) -> Result<DateTime, String> {
    if !(value < 2_958_466.0 && value > -657_435.0) {
        return Err("Not a legal OleAut date.".into());
    }
    let mut millis = (value * MILLIS_PER_DAY as f64 + if value >= 0.0 { 0.5 } else { -0.5 }) as i64;
    if millis < 0 {
        millis -= (millis % MILLIS_PER_DAY) * 2;
    }
    millis += DOUBLE_DATE_OFFSET_MILLIS;
    if !(0..MAX_MILLIS).contains(&millis) {
        return Err("OLE Automation Date is out of range. Valid values are between -657435.0 and 2958465.99999999.".into());
    }
    Ok(DateTime::from_millis(millis))
}

/// ClosedXML `XLHelper.GetTimeSpan` 뒤 `TimeSpan.ToExcelString(ko-KR)` — `시:분:초[.밀리초]`(시는 날을 더한 것).
pub fn time_span_text(total_days: f64) -> Result<String, String> {
    const TICKS_PER_DAY: f64 = 864_000_000_000.0;
    let ticks = (total_days * TICKS_PER_DAY).round();
    if !(ticks <= i64::MAX as f64 && ticks >= i64::MIN as f64) {
        return Err(
            "The serial date time value is too large to be represented in a TimeSpan.".into(),
        );
    }
    let ticks = ticks as i64;
    let days = ticks / 864_000_000_000;
    let hours = (ticks / 36_000_000_000) % 24;
    let minutes = (ticks / 600_000_000) % 60;
    let seconds = (ticks / 10_000_000) % 60;
    let two = |n: i64| {
        if n < 0 {
            format!("-{:02}", -n)
        } else {
            format!("{n:02}")
        }
    };
    let mut out = format!("{}:{}:{}", hours + 24 * days, two(minutes), two(seconds));
    let ms = ((ticks % 10_000_000) as f64 * 1000.0 / 10_000_000.0).round_ties_even() as i64;
    if ms != 0 {
        out.push('.');
        if ms % 100 == 0 {
            out.push_str(&(ms / 100).to_string());
        } else if ms % 10 == 0 {
            out.push_str(&format!("{:02}", ms / 10));
        } else {
            out.push_str(&format!("{ms:03}"));
        }
    }
    Ok(out)
}

/// 그레고리력 날짜 → 1970-01-01 부터의 날 수.
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(m);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_like_dotnet_custom_format() {
        assert_eq!(number_text(8.0), "8");
        assert_eq!(number_text(1.5), "1.5");
        assert_eq!(number_text(0.1 + 0.2), "0.3");
        assert_eq!(number_text(1e20), "100000000000000000000");
        assert_eq!(number_text(123_456_789_012_345_680.0), "123456789012346000");
        assert_eq!(number_text(-2.25), "-2.25");
        assert_eq!(number_text(1e-7), "0.0000001");
        assert_eq!(number_text(-1e-30), "-0");
        assert_eq!(number_text(2_590_012_345_678.0), "2590012345678");
    }

    #[test]
    fn oa_dates() {
        let d = serial_date(46_000.0).unwrap();
        assert_eq!(d.plan_text(), "2025-12-09");
        assert_eq!(
            serial_date(46_000.5).unwrap().plan_text(),
            "2025-12-09 12:00:00"
        );
        assert_eq!(serial_date(1.0).unwrap().plan_text(), "1900-01-01");
        assert!(serial_date(60.5).is_err());
    }

    #[test]
    fn time_spans() {
        assert_eq!(time_span_text(0.5).unwrap(), "12:00:00");
        assert_eq!(time_span_text(1.25).unwrap(), "30:00:00");
    }
}

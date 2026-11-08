//! 로컬 시각 — 저장은 UTC(`"O"`)인데 사람이 읽는 날짜(수집 기록의 「오늘」, 투영의 「고친 날」)는 로컬이다.
//!
//! C# 은 `DateTime.ToLocalTime()` 으로 이 기계의 시간대를 그 자리에서 읽었다. 이 크레이트는 시간대를 스스로 읽지 않는다 —
//! 부르는 쪽이 [`LocalZone`] 으로 넣는다(시험은 고정 오프셋을 넣는다).

/// UTC 순간 하나에서 그 순간의 로컬 오프셋(초). 일광 절약 시간이 있는 시간대도 순간마다 답한다.
pub trait LocalZone {
    fn offset_seconds(&self, utc_unix_seconds: i64) -> i64;
}

/// 고정 오프셋 시간대(초).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FixedZone(pub i64);

impl LocalZone for FixedZone {
    fn offset_seconds(&self, _: i64) -> i64 {
        self.0
    }
}

/// 날짜와 시각(초 아래는 100나노초 단위 `ticks`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct LocalDateTime {
    pub year: i64,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    pub ticks: u32,
}

impl LocalDateTime {
    /// 그날 자정.
    pub fn date(self) -> LocalDateTime {
        LocalDateTime {
            hour: 0,
            minute: 0,
            second: 0,
            ticks: 0,
            ..self
        }
    }

    /// 1970-01-01 부터의 초(이 날짜·시각을 UTC 로 읽었을 때).
    pub fn unix_seconds(self) -> i64 {
        days_from_civil(self.year, self.month, self.day) * 86_400
            + i64::from(self.hour) * 3600
            + i64::from(self.minute) * 60
            + i64::from(self.second)
    }

    fn from_unix(seconds: i64, ticks: u32) -> LocalDateTime {
        let days = seconds.div_euclid(86_400);
        let rest = seconds.rem_euclid(86_400);
        let (year, month, day) = civil_from_days(days);
        LocalDateTime {
            year,
            month,
            day,
            hour: (rest / 3600) as u32,
            minute: (rest % 3600 / 60) as u32,
            second: (rest % 60) as u32,
            ticks,
        }
    }

    /// 다음 날 같은 시각.
    pub fn add_days(self, days: i64) -> LocalDateTime {
        LocalDateTime::from_unix(self.unix_seconds() + days * 86_400, self.ticks)
    }

    /// `yyyy-MM-dd`.
    pub fn day_text(self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    /// `MM/dd`.
    pub fn month_day(self) -> String {
        format!("{:02}/{:02}", self.month, self.day)
    }

    /// `yyyy-MM-ddTHH:mm:ss`.
    pub fn second_text(self) -> String {
        format!(
            "{}T{:02}:{:02}:{:02}",
            self.day_text(),
            self.hour,
            self.minute,
            self.second
        )
    }
}

/// 로컬 날짜·시각을 UTC 의 `"O"` 표기로(`DateTime.SpecifyKind(local, Local).ToUniversalTime().ToString("O")`). 오프셋은
/// 그 로컬 시각을 UTC 로 읽은 순간의 것으로 근사한다 — 일광 절약 전환의 한 시간 안에서만 C# 과 갈릴 수 있다.
pub fn to_utc_text(local: LocalDateTime, zone: &dyn LocalZone) -> String {
    let naive = local.unix_seconds();
    let utc = LocalDateTime::from_unix(naive - zone.offset_seconds(naive), local.ticks);
    format!("{}.{:07}Z", utc.second_text(), utc.ticks)
}

/// 저장된 시각을 로컬로(`DateTime.TryParse(…, RoundtripKind)` 뒤 `ToLocalTime`). 오프셋이 없는 시각은 UTC 로 읽는다 — C# 의
/// 두 자리(`Mirror.LocalDay`·`CaptureLog.Local`)가 모두 그렇게 한다. 읽지 못하면 `None`.
///
/// `DateTime.TryParse` 는 훨씬 너그럽다. 여기서는 앱이 쓰는 꼴만 읽는다 — `yyyy-MM-dd`, 뒤에 `T` 또는 공백과 `HH:mm[:ss[.f…]]`,
/// 그 뒤에 `Z` 나 `±HH:mm`.
pub fn parse_to_local(stored: Option<&str>, zone: &dyn LocalZone) -> Option<LocalDateTime> {
    let (seconds, ticks) = parse_utc(stored?.trim())?;
    Some(LocalDateTime::from_unix(
        seconds + zone.offset_seconds(seconds),
        ticks,
    ))
}

fn parse_utc(s: &str) -> Option<(i64, u32)> {
    let bytes = s.as_bytes();
    let number = |from: usize, len: usize| -> Option<u32> {
        let part = s.get(from..from + len)?;
        part.bytes()
            .all(|b| b.is_ascii_digit())
            .then(|| part.parse().ok())?
    };
    if bytes.len() < 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    let (year, month, day) = (number(0, 4)?, number(5, 2)?, number(8, 2)?);
    if !(1..=12).contains(&month) || day == 0 || day > days_in_month(i64::from(year), month) {
        return None;
    }
    let (mut hour, mut minute, mut second, mut ticks) = (0, 0, 0, 0u32);
    let mut i = 10;
    if i < bytes.len() && (bytes[i] == b'T' || bytes[i] == b' ') {
        hour = number(i + 1, 2)?;
        if bytes.get(i + 3) != Some(&b':') {
            return None;
        }
        minute = number(i + 4, 2)?;
        i += 6;
        if bytes.get(i) == Some(&b':') {
            second = number(i + 1, 2)?;
            i += 3;
            if bytes.get(i) == Some(&b'.') {
                let start = i + 1;
                let mut end = start;
                while end < bytes.len() && bytes[end].is_ascii_digit() {
                    end += 1;
                }
                if end == start {
                    return None;
                }
                let digits = &s[start..end.min(start + 7)];
                ticks = format!("{digits:0<7}").parse().ok()?;
                i = end;
            }
        }
        if hour > 23 || minute > 59 || second > 59 {
            return None;
        }
    }
    let mut offset = 0i64;
    match bytes.get(i) {
        None => {}
        Some(b'Z') if i + 1 == bytes.len() => {}
        Some(sign @ (b'+' | b'-')) if bytes.len() == i + 6 && bytes[i + 3] == b':' => {
            let minutes = i64::from(number(i + 1, 2)?) * 60 + i64::from(number(i + 4, 2)?);
            offset = if *sign == b'+' { minutes } else { -minutes } * 60;
        }
        _ => return None,
    }
    let local = LocalDateTime {
        year: i64::from(year),
        month,
        day,
        hour,
        minute,
        second,
        ticks,
    };
    Some((local.unix_seconds() - offset, ticks))
}

pub(crate) fn days_in_month(year: i64, month: u32) -> u32 {
    match month {
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

// Howard Hinnant 의 날짜 셈.
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(month);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_round_trip_text_into_local() {
        let seoul = FixedZone(9 * 3600);
        let at = parse_to_local(Some("2026-12-31T20:30:00.1234567Z"), &seoul).unwrap();
        assert_eq!(at.second_text(), "2027-01-01T05:30:00");
        assert_eq!(at.month_day(), "01/01");
        let unspecified = parse_to_local(Some("2026-03-01 00:00:00"), &seoul).unwrap();
        assert_eq!(unspecified.second_text(), "2026-03-01T09:00:00");
        let offset = parse_to_local(Some("2026-03-01T09:00:00+09:00"), &FixedZone(0)).unwrap();
        assert_eq!(offset.second_text(), "2026-03-01T00:00:00");
        assert!(parse_to_local(Some("어제"), &seoul).is_none());
        assert!(parse_to_local(None, &seoul).is_none());
    }

    #[test]
    fn local_midnight_back_to_utc() {
        let seoul = FixedZone(9 * 3600);
        let start = parse_to_local(Some("2026-07-02T03:00:00Z"), &seoul)
            .unwrap()
            .date();
        assert_eq!(to_utc_text(start, &seoul), "2026-07-01T15:00:00.0000000Z");
        assert_eq!(
            to_utc_text(start.add_days(-1), &seoul),
            "2026-06-30T15:00:00.0000000Z"
        );
    }
}

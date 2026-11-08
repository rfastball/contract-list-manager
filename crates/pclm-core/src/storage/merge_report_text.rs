//! 취합 한 번의 보고를 글로 짓는다(C# `Storage/MergeReportText.cs`).
//!
//! **글이 한 벌인 까닭.** 명령줄은 화면에 찍고 창은 취합본 옆에 파일로 남기는데, 같은 취합을 두 길로 한 사람이 서로
//! 다른 글을 읽으면 어느 것이 진짜인지 가릴 수 없다. 두 벌로 두면 한쪽이 조용히 늙는다 — 그래서 짓는 자리를 하나로
//! 둔다.

use std::path::Path;

use crate::local_time::LocalZone;
use crate::storage::merger::{MergeReport, MergeSide, TOO_NEW, TOO_OLD};
use crate::text::utf16_len;

/// C# `AppendLine` 의 줄바꿈(`Environment.NewLine`) — Windows 에서는 CR LF, 그 밖에서는 LF.
pub const NEWLINE: &str = if cfg!(windows) { "\r\n" } else { "\n" };

/// `MergeReportText.Render` — 취합 한 번의 보고. **알림이 본체다** — 무엇이 누구와 누구에게서 왔고 어느 것을
/// 실었는지 한 줄씩 적는다. 마지막은 줄바꿈 하나로 닫는다.
///
/// 겹친 자리의 시각은 월/일까지만 적는데, C# 은 `DateTime.TryParse` 로 읽어 `Z` 붙은 시각을 **그 기계의 시각**으로
/// 바꾼다. 이 크레이트는 시간대를 보지 않으므로 부르는 쪽이 넣은 [`LocalZone`] 으로 같은 일을 한다.
pub fn render(report: &MergeReport, merged_path: &Path, zone: &dyn LocalZone) -> String {
    let mut text = String::new();
    let mut line = |s: &str| {
        text.push_str(s);
        text.push_str(NEWLINE);
    };

    line("취합 결과");
    line(&format!(
        "  제출본 {}개 · 조달요구 {} · 접수 {} · 공고 {} · 계약 {}",
        report.submissions, report.plans, report.requests, report.notices, report.contracts
    ));

    // 빈 구역은 통째로 뺀다. "거절 없음" 은 적을 말이 아니라 없는 일이라, 적어 두면 실제로 알릴 것이 있는 줄이 늘
    // 있는 줄에 묻힌다.
    if !report.rejected.is_empty() {
        line("");
        line(&format!("  받지 않은 파일 {}개", report.rejected.len()));
        for rejected in &report.rejected {
            line(&format!("    {} — {}", rejected.file, rejected.reason));
        }

        // 할 일이 있는 까닭만 풀어 적는다. 취합본·백업이 섞인 것은 고를 때 빼면 되는 일이라 말을 보태지 않는다.
        let has = |reason: &str| report.rejected.iter().any(|r| r.reason == reason);
        if has(TOO_NEW) {
            line(
                "    새 판으로 지은 자료는 내릴 길이 없습니다. 이 프로그램을 새로 받아 다시 합치세요.",
            );
        }
        if has(TOO_OLD) {
            line(
                "    정식판 이전의 옛 시험판 자료는 0.7.0 으로 한 번 열어 올린 뒤 다시 제출받으세요.",
            );
        }
    }

    if !report.conflicts.is_empty() {
        line("");
        line(&format!(
            "  겹친 것 {}건 — 늦은 것을 실었습니다",
            report.conflicts.len()
        ));
        for c in &report.conflicts {
            // `{키,-16}` — UTF-16 단위로 열여섯 자리까지 오른쪽을 채운다.
            let pad = 16usize.saturating_sub(utf16_len(&c.key));
            line(&format!(
                "    {}{} {} ◀ 실림   {}",
                c.key,
                " ".repeat(pad),
                side(&c.kept, zone),
                c.pushed
                    .iter()
                    .map(|s| side(s, zone))
                    .collect::<Vec<_>>()
                    .join("  ")
            ));
        }
    }

    line("");
    line(&format!("  파일  {}", merged_path.display()));

    text
}

/// 겹친 자리의 한쪽. 시각은 사람이 훑을 것이라 월/일까지만 적는다. 읽지 못한 시각은 그대로 적는다.
fn side(side: &MergeSide, zone: &dyn LocalZone) -> String {
    if side.time.is_empty() {
        return side.submitter.clone();
    }
    match month_day(&side.time, zone) {
        Some((month, day)) => format!("{}({month:02}/{day:02})", side.submitter),
        None => format!("{}({})", side.submitter, side.time),
    }
}

/// C# `DateTime.TryParse(시각)` 의 월/일. 이 저장소가 시각 열에 적는 꼴만 읽는다 — `yyyy-M-d` 또는 `yyyy/M/d`, 뒤에
/// `T`·빈칸과 `H:mm[:ss[.f…]]`, 그 뒤에 `Z`·`±HH:mm`(·`±HHmm`·`±HH`). 시간대가 붙은 것은 C# 처럼 그 기계의 시각으로
/// 바꾼다(`DateTimeKind.Local`). 앞뒤 빈칸은 걷는다. 그 밖의 꼴(달 이름, 문화권 차례)은 읽지 않은 것으로 본다 — C# 은
/// 더 너그럽게 읽지만, 이 저장소가 적는 시각은 모두 `DateTime.UtcNow.ToString("O")` 꼴이다.
fn month_day(text: &str, machine: &dyn LocalZone) -> Option<(u32, u32)> {
    let s = text.trim();
    let mut p = Cursor {
        s: s.as_bytes(),
        at: 0,
    };

    let year = p.digits(4, 4)? as i64;
    let sep = p.one_of(b"-/")?;
    let month = p.digits(1, 2)?;
    p.expect(sep)?;
    let day = p.digits(1, 2)?;
    if !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
        return None;
    }

    let (mut hour, mut minute, mut second) = (0, 0, 0);
    let mut zone: Option<i64> = None;
    if !p.done() {
        p.one_of(b"T ")?;
        hour = p.digits(1, 2)?;
        p.expect(b':')?;
        minute = p.digits(2, 2)?;
        if p.peek() == Some(b':') {
            p.at += 1;
            second = p.digits(2, 2)?;
            if p.peek() == Some(b'.') {
                p.at += 1;
                p.digits(1, 7)?;
            }
        }
        if hour > 23 || minute > 59 || second > 59 {
            return None;
        }
        match p.peek() {
            None => {}
            Some(b'Z') => {
                p.at += 1;
                zone = Some(0);
            }
            Some(sign @ (b'+' | b'-')) => {
                p.at += 1;
                let hours = p.digits(2, 2)? as i64;
                let minutes = if p.peek() == Some(b':') {
                    p.at += 1;
                    p.digits(2, 2)? as i64
                } else if p.done() {
                    0
                } else {
                    p.digits(2, 2)? as i64
                };
                if hours > 14 || minutes > 59 {
                    return None;
                }
                let offset = (hours * 60 + minutes) * 60;
                zone = Some(if sign == b'-' { -offset } else { offset });
            }
            Some(_) => return None,
        }
    }
    if !p.done() {
        return None;
    }

    let Some(offset) = zone else {
        return Some((month, day));
    };
    let wall = days_from_civil(year, month, day) * 86_400
        + i64::from(hour) * 3_600
        + i64::from(minute) * 60
        + i64::from(second);
    let utc = wall - offset;
    let local = utc + machine.offset_seconds(utc);
    let (_, m, d) = civil_from_days(local.div_euclid(86_400));
    Some((m, d))
}

struct Cursor<'a> {
    s: &'a [u8],
    at: usize,
}

impl Cursor<'_> {
    fn done(&self) -> bool {
        self.at >= self.s.len()
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.at).copied()
    }

    fn expect(&mut self, b: u8) -> Option<()> {
        (self.peek()? == b).then(|| self.at += 1)
    }

    fn one_of(&mut self, set: &[u8]) -> Option<u8> {
        let b = self.peek()?;
        set.contains(&b).then(|| {
            self.at += 1;
            b
        })
    }

    /// 십진 숫자 `min`..=`max` 자.
    fn digits(&mut self, min: usize, max: usize) -> Option<u32> {
        let start = self.at;
        while self.at < self.s.len() && self.at - start < max && self.s[self.at].is_ascii_digit() {
            self.at += 1;
        }
        let run = &self.s[start..self.at];
        if run.len() < min {
            return None;
        }
        Some(run.iter().fold(0u32, |n, d| n * 10 + u32::from(d - b'0')))
    }
}

fn days_in_month(year: i64, month: u32) -> u32 {
    match month {
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// 그레고리력 날짜 → 1970-01-01 로부터의 날 수(H. Hinnant 의 days_from_civil).
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let m = i64::from(month);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// 1970-01-01 로부터의 날 수 → 그레고리력 날짜(H. Hinnant 의 civil_from_days).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
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

    use crate::local_time::FixedZone;

    /// 서울(UTC+9).
    const SEOUL: FixedZone = FixedZone(9 * 3_600);

    #[test]
    fn zoned_times_turn_local_like_try_parse() {
        // 15:30Z 는 서울의 다음 날 00:30 이다.
        assert_eq!(
            month_day("2026-08-22T15:30:00.0000000Z", &SEOUL),
            Some((8, 23))
        );
        assert_eq!(
            month_day("2026-08-22T09:00:00.0000000Z", &SEOUL),
            Some((8, 22))
        );
        assert_eq!(
            month_day("2026-12-31T23:00:00+09:00", &SEOUL),
            Some((12, 31))
        );
        assert_eq!(month_day("2026-12-31T23:00:00-01:00", &SEOUL), Some((1, 1)));
    }

    #[test]
    fn unzoned_times_stay_as_written() {
        assert_eq!(month_day("2026-09-27", &SEOUL), Some((9, 27)));
        assert_eq!(month_day("2026/9/1 23:59", &SEOUL), Some((9, 1)));
        assert_eq!(month_day(" 2026-08-20 ", &SEOUL), Some((8, 20)));
    }

    #[test]
    fn unreadable_times_are_none() {
        for s in [
            "",
            "어제",
            "2026-02-30",
            "2026-13-01",
            "2026-08-22T25:00",
            "20260822",
        ] {
            assert_eq!(month_day(s, &SEOUL), None, "{s}");
        }
    }

    #[test]
    fn civil_round_trip() {
        for days in [-719_468, -1, 0, 11_016, 20_000, 2_932_896] {
            let (y, m, d) = civil_from_days(days);
            assert_eq!(days_from_civil(y, m, d), days);
        }
    }
}

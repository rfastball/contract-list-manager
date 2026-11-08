//! 계기가 Core 에 넣어 주는 시계·식별자. Core 는 이것을 스스로 부르지 않는다(#8) — 계기는 크레이트 밖의
//! 층이라 여기서 진짜 시계를 읽는다. 값은 판마다 갈리므로 대조는 그 칸을 `--mask` 로 가린다.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct SystemEnv;

static COUNTER: AtomicU64 = AtomicU64::new(0);

impl pclm_core::Env for SystemEnv {
    fn now_utc(&self) -> String {
        let since = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        format_o(since.as_secs() as i64, since.subsec_nanos() / 100)
    }

    fn new_id(&self) -> String {
        // 계기 한 번 돌리는 동안만 겹치지 않으면 된다 — 의존을 더하지 않고 표준 라이브러리의 무작위 씨앗을 쓴다.
        let mut id = String::with_capacity(32);
        for _ in 0..2 {
            let mut hasher = RandomState::new().build_hasher();
            hasher.write_u64(COUNTER.fetch_add(1, Ordering::Relaxed));
            hasher.write_u128(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos(),
            );
            id.push_str(&format!("{:016x}", hasher.finish()));
        }
        id
    }
}

impl pclm_core::local_time::LocalZone for SystemEnv {
    fn offset_seconds(&self, utc_unix_seconds: i64) -> i64 {
        local_offset(utc_unix_seconds)
    }
}

/// 그 순간의 이 기계 시간대 차이. C# 의 `DateTime.TryParse`(→ `TimeZoneInfo.Local`)와 같은 자리를 Windows 의
/// `SystemTimeToTzSpecificLocalTime`(지금 시간대, 그날의 일광 절약 규칙)으로 잰다. 의존을 더하지 않으려고 직접 부른다.
#[cfg(windows)]
fn local_offset(unix_seconds: i64) -> i64 {
    #[repr(C)]
    #[derive(Default)]
    struct SystemTime {
        year: u16,
        month: u16,
        day_of_week: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
        milliseconds: u16,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn SystemTimeToTzSpecificLocalTime(
            time_zone: *const std::ffi::c_void,
            universal: *const SystemTime,
            local: *mut SystemTime,
        ) -> i32;
    }

    let days = unix_seconds.div_euclid(86_400);
    let rest = unix_seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    if !(1601..=30827).contains(&year) {
        return 0;
    }
    let universal = SystemTime {
        year: year as u16,
        month: month as u16,
        day: day as u16,
        hour: (rest / 3600) as u16,
        minute: (rest % 3600 / 60) as u16,
        second: (rest % 60) as u16,
        ..SystemTime::default()
    };
    let mut local = SystemTime::default();
    // SAFETY: 두 구조체는 SYSTEMTIME 과 같은 꼴이고 호출 동안 살아 있다. 시간대 NULL 은 지금 시간대다.
    let ok = unsafe { SystemTimeToTzSpecificLocalTime(std::ptr::null(), &universal, &mut local) };
    if ok == 0 {
        return 0;
    }
    let local_seconds = days_from_civil(
        i64::from(local.year),
        u32::from(local.month),
        u32::from(local.day),
    ) * 86_400
        + i64::from(local.hour) * 3600
        + i64::from(local.minute) * 60
        + i64::from(local.second);
    local_seconds - unix_seconds
}

/// Windows 가 아니면 UTC 로 본다 — 대조는 Windows 에서만 돈다.
#[cfg(not(windows))]
fn local_offset(_unix_seconds: i64) -> i64 {
    0
}

/// 그레고리력 날짜 → 1970-01-01 로부터의 날 수(H. Hinnant 의 days_from_civil).
#[cfg(windows)]
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let m = i64::from(month);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// .NET `DateTime.ToString("O")` 꼴(UTC) — `yyyy-MM-ddTHH:mm:ss.fffffffZ`. `ticks` 는 100ns 단위.
fn format_o(seconds: i64, ticks: u32) -> String {
    let days = seconds.div_euclid(86_400);
    let rest = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{ticks:07}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
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

    #[test]
    fn formats_like_dotnet_round_trip() {
        // 2026-11-07T03:04:05Z = 1794020645
        assert_eq!(
            format_o(1_794_020_645, 1_234_567),
            "2026-11-07T03:04:05.1234567Z"
        );
        assert_eq!(format_o(951_782_400, 0), "2000-02-29T00:00:00.0000000Z");
    }
}

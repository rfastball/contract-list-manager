//! 이 층이 부르는 Windows API — 이름 붙은 뮤텍스, 로컬 시각, 프로세스 시작 시각, 폴더 변화 알림, 등록부, 난수.
//!
//! 의존을 더하지 않으려고(`windows-sys` 등) 쓰는 몇 개만 직접 선언한다. C# 이 `Mutex`·`DateTime.Now`·
//! `Process.StartTime`·`FileSystemWatcher`·`Registry`·`Guid.NewGuid` 로 하던 일이다. 모두 Windows 전용이다 —
//! 다른 체계에서는 이 크레이트가 지어지지 않는다(대조와 배포가 Windows 에서만 돈다).

#![allow(unsafe_code)]

use std::ffi::c_void;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::time::Duration;

use pclm_core::local_time::LocalDateTime;

type Handle = *mut c_void;

#[repr(C)]
#[derive(Default, Clone, Copy)]
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

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct FileTime {
    low: u32,
    high: u32,
}

impl FileTime {
    fn ticks(self) -> u64 {
        (u64::from(self.high) << 32) | u64::from(self.low)
    }
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateMutexW(attributes: *const c_void, initial_owner: i32, name: *const u16) -> Handle;
    fn CloseHandle(handle: Handle) -> i32;
    fn GetLastError() -> u32;
    fn GetLocalTime(time: *mut SystemTime);
    fn SystemTimeToTzSpecificLocalTime(
        time_zone: *const c_void,
        universal: *const SystemTime,
        local: *mut SystemTime,
    ) -> i32;
    fn GetCurrentProcess() -> Handle;
    fn GetProcessTimes(
        process: Handle,
        creation: *mut FileTime,
        exit: *mut FileTime,
        kernel: *mut FileTime,
        user: *mut FileTime,
    ) -> i32;
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
    fn GetExitCodeProcess(process: Handle, code: *mut u32) -> i32;
    fn FindFirstChangeNotificationW(path: *const u16, subtree: i32, filter: u32) -> Handle;
    fn FindNextChangeNotification(handle: Handle) -> i32;
    fn FindCloseChangeNotification(handle: Handle) -> i32;
    fn CreateEventW(
        attributes: *const c_void,
        manual_reset: i32,
        initial: i32,
        name: *const u16,
    ) -> Handle;
    fn SetEvent(handle: Handle) -> i32;
    fn WaitForMultipleObjects(count: u32, handles: *const Handle, all: i32, ms: u32) -> u32;
    fn ExpandEnvironmentStringsW(source: *const u16, target: *mut u16, size: u32) -> u32;
    fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
    fn GetDriveTypeW(root: *const u16) -> u32;
}

#[link(name = "advapi32")]
unsafe extern "system" {
    fn RegOpenKeyExW(
        key: Handle,
        sub_key: *const u16,
        options: u32,
        desired: u32,
        result: *mut Handle,
    ) -> i32;
    fn RegQueryValueExW(
        key: Handle,
        name: *const u16,
        reserved: *mut u32,
        kind: *mut u32,
        data: *mut u8,
        size: *mut u32,
    ) -> i32;
    fn RegCloseKey(key: Handle) -> i32;
}

#[link(name = "bcrypt")]
unsafe extern "system" {
    fn BCryptGenRandom(algorithm: Handle, buffer: *mut u8, size: u32, flags: u32) -> i32;
}

const ERROR_ALREADY_EXISTS: u32 = 183;
const ERROR_ACCESS_DENIED: u32 = 5;
const STILL_ACTIVE: u32 = 259;
const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
const FILE_NOTIFY_CHANGE_FILE_NAME: u32 = 0x1;
const WAIT_OBJECT_0: u32 = 0;
const KEY_READ: u32 = 0x20019;
const REG_SZ: u32 = 1;
const REG_EXPAND_SZ: u32 = 2;
const BCRYPT_USE_SYSTEM_PREFERRED_RNG: u32 = 0x2;
const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
const MOVEFILE_COPY_ALLOWED: u32 = 0x2;
const DRIVE_REMOTE: u32 = 4;
/// FILETIME(1601-01-01 부터 100ns)과 유닉스 시각(1970-01-01)의 차.
const FILETIME_UNIX_EPOCH: u64 = 116_444_736_000_000_000;

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn wide_path(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

// ── 이름 붙은 뮤텍스 ─────────────────────────────────────────────

/// 이름 붙은 뮤텍스의 손잡이. 놓으면(`Drop`) 손잡이를 닫는다 — 마지막 손잡이가 닫히면 이름이 사라진다.
#[derive(Debug)]
pub struct NamedMutex(Handle);

// SAFETY: 커널 손잡이는 어느 실마리에서 닫아도 된다. 소유를 따지지 않고 있음만 본다(C# `Home.TryLock`).
unsafe impl Send for NamedMutex {}
// SAFETY: 손잡이로 하는 일은 닫기뿐이다.
unsafe impl Sync for NamedMutex {}

impl Drop for NamedMutex {
    fn drop(&mut self) {
        // SAFETY: 이 구조체만 쥔 손잡이를 한 번 닫는다.
        unsafe {
            CloseHandle(self.0);
        }
    }
}

/// `new Mutex(initiallyOwned: false, name, out createdNew)` — 새로 지었으면 그 손잡이, 이미 있었으면 `None`(손잡이는
/// 곧바로 닫는다). 지을 수 없으면(이름이 다른 종류의 객체·권한) 오류.
pub fn create_mutex(name: &str) -> io::Result<Option<NamedMutex>> {
    let name = wide(name);
    // SAFETY: 이름은 NUL 로 끝나는 UTF-16 이고 호출 동안 산다. 보안 속성 NULL 은 기본값이다.
    let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
    // SAFETY: 바로 앞 호출의 마지막 오류를 읽는다.
    let last = unsafe { GetLastError() };
    if handle.is_null() {
        return Err(io::Error::from_raw_os_error(last as i32));
    }
    let mutex = NamedMutex(handle);
    Ok((last != ERROR_ALREADY_EXISTS).then_some(mutex))
}

// ── 시각 ─────────────────────────────────────────────────────

/// `DateTime.Now` — 이 기계의 로컬 시각(밀리초까지).
pub fn local_now() -> LocalDateTime {
    let mut t = SystemTime::default();
    // SAFETY: 구조체는 SYSTEMTIME 과 같은 꼴이다.
    unsafe { GetLocalTime(&mut t) };
    LocalDateTime {
        year: i64::from(t.year),
        month: u32::from(t.month),
        day: u32::from(t.day),
        hour: u32::from(t.hour),
        minute: u32::from(t.minute),
        second: u32::from(t.second),
        ticks: u32::from(t.milliseconds) * 10_000,
    }
}

/// 그 UTC 순간의 이 기계 시간대 차이(초) — `DateTime.ToLocalTime()`·`DateTime.Now` 가 읽는 자리를 Windows 의
/// `SystemTimeToTzSpecificLocalTime`(지금 시간대, 그날의 일광 절약 규칙)으로 잰다. SYSTEMTIME 이 담지 못하는 해(1601 앞)거나
/// 실패하면 0 이다.
pub fn local_offset_seconds(unix_seconds: i64) -> i64 {
    let at = LocalDateTime {
        year: 1970,
        month: 1,
        day: 1,
        hour: 0,
        minute: 0,
        second: 0,
        ticks: 0,
    }
    .add_days(unix_seconds.div_euclid(86_400));
    if !(1601..=30827).contains(&at.year) {
        return 0;
    }
    let rest = unix_seconds.rem_euclid(86_400);
    let universal = SystemTime {
        year: at.year as u16,
        month: at.month as u16,
        day: at.day as u16,
        hour: (rest / 3600) as u16,
        minute: (rest % 3600 / 60) as u16,
        second: (rest % 60) as u16,
        ..SystemTime::default()
    };
    let mut local = SystemTime::default();
    // SAFETY: 두 구조체는 SYSTEMTIME 과 같은 꼴이고 호출 동안 살아 있다. 시간대 NULL 은 지금 시간대다.
    if unsafe { SystemTimeToTzSpecificLocalTime(std::ptr::null(), &universal, &mut local) } == 0 {
        return 0;
    }
    let local = LocalDateTime {
        year: i64::from(local.year),
        month: u32::from(local.month),
        day: u32::from(local.day),
        hour: u32::from(local.hour),
        minute: u32::from(local.minute),
        second: u32::from(local.second),
        ticks: 0,
    };
    local.unix_seconds() - unix_seconds
}

/// UTC 순간(유닉스 초와 그 아래 100ns 단위) → .NET `ToString("O")` 꼴 `yyyy-MM-ddTHH:mm:ss.fffffffZ`.
pub fn format_round_trip(unix_seconds: i64, ticks: u32) -> String {
    let at = LocalDateTime {
        year: 1970,
        month: 1,
        day: 1,
        hour: 0,
        minute: 0,
        second: 0,
        ticks,
    }
    .add_days(unix_seconds.div_euclid(86_400));
    let rest = unix_seconds.rem_euclid(86_400);
    format!(
        "{}T{:02}:{:02}:{:02}.{ticks:07}Z",
        at.day_text(),
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

fn filetime_to_unix(ticks: u64) -> (i64, u32) {
    let since = ticks.saturating_sub(FILETIME_UNIX_EPOCH);
    ((since / 10_000_000) as i64, (since % 10_000_000) as u32)
}

/// 이 프로세스가 시작한 UTC 순간(유닉스 초, 100ns) — `Process.GetCurrentProcess().StartTime.ToUniversalTime()`.
pub fn process_start() -> io::Result<(i64, u32)> {
    // SAFETY: 의사 손잡이는 닫지 않아도 된다.
    let process = unsafe { GetCurrentProcess() };
    start_of(process)
}

fn start_of(process: Handle) -> io::Result<(i64, u32)> {
    let (mut creation, mut exit, mut kernel, mut user) = Default::default();
    // SAFETY: 손잡이는 살아 있고 네 칸은 FILETIME 꼴이다.
    let ok = unsafe { GetProcessTimes(process, &mut creation, &mut exit, &mut kernel, &mut user) };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(filetime_to_unix(FileTime::ticks(creation)))
}

/// 그 번호의 프로세스가 아직 도는가, 돈다면 그 시작 순간. 없거나 끝났으면 `Ok(None)`, 들여다볼 수 없으면(권한) 오류 —
/// C# `ExtensionPresence.Running` 이 `Process.GetProcessById` 로 가리던 세 갈래다.
pub fn running_process_start(pid: i32) -> io::Result<Option<(i64, u32)>> {
    let Ok(pid) = u32::try_from(pid) else {
        return Ok(None);
    };
    // SAFETY: 실패하면 NULL 이고 그때만 마지막 오류를 읽는다.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        // SAFETY: 바로 앞 호출의 마지막 오류.
        let last = unsafe { GetLastError() };
        return if last == ERROR_ACCESS_DENIED {
            Err(io::Error::from_raw_os_error(last as i32))
        } else {
            Ok(None)
        };
    }
    let result = (|| {
        let mut code = 0u32;
        // SAFETY: 손잡이는 살아 있다.
        if unsafe { GetExitCodeProcess(process, &mut code) } == 0 {
            return Err(io::Error::last_os_error());
        }
        // .NET 의 GetProcessById 는 끝난 프로세스(손잡이만 남은 것)를 목록에서 찾지 못한다.
        if code != STILL_ACTIVE {
            return Ok(None);
        }
        start_of(process).map(Some)
    })();
    // SAFETY: 위에서 연 손잡이를 한 번 닫는다.
    unsafe { CloseHandle(process) };
    result
}

// ── 파일 ─────────────────────────────────────────────────────

/// `File.Move(source, target, overwrite)` — .NET 처럼 `MoveFileExW(COPY_ALLOWED [| REPLACE_EXISTING])`. 덮지 않기로 했는데
/// 그 자리에 파일이 있으면 실패한다(`std::fs::rename` 은 언제나 덮는다).
pub fn move_file(source: &Path, target: &Path, overwrite: bool) -> io::Result<()> {
    let (from, to) = (wide_path(source), wide_path(target));
    let flags = MOVEFILE_COPY_ALLOWED
        | if overwrite {
            MOVEFILE_REPLACE_EXISTING
        } else {
            0
        };
    // SAFETY: 두 경로는 NUL 로 끝나는 UTF-16 이고 호출 동안 산다.
    if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), flags) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// `new DriveInfo(root).DriveType == DriveType.Network` — `GetDriveTypeW` 가 `DRIVE_REMOTE` 인가. `root` 는 `C:\` 꼴.
pub fn is_remote_drive(root: &str) -> bool {
    let root = wide(root);
    // SAFETY: 뿌리는 NUL 로 끝나는 UTF-16 이고 호출 동안 산다.
    unsafe { GetDriveTypeW(root.as_ptr()) == DRIVE_REMOTE }
}

// ── 난수 ─────────────────────────────────────────────────────

/// 운영체제의 난수 `N` 바이트.
pub fn random_bytes<const N: usize>() -> [u8; N] {
    let mut buffer = [0u8; N];
    // SAFETY: 버퍼는 N 바이트이고 알고리즘 NULL 은 시스템 기본 RNG 를 뜻한다(플래그).
    let status = unsafe {
        BCryptGenRandom(
            std::ptr::null_mut(),
            buffer.as_mut_ptr(),
            N as u32,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    assert!(status >= 0, "BCryptGenRandom 실패: {status:#x}");
    buffer
}

/// `Guid.NewGuid().ToString("N")` — 판 4·RFC 4122 변형 비트를 단 무작위 식별자, 소문자 16진 32자.
pub fn new_guid_n() -> String {
    let mut bytes = random_bytes::<16>();
    bytes[6] = (bytes[6] & 0x0F) | 0x40;
    bytes[8] = (bytes[8] & 0x3F) | 0x80;
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ── 폴더 변화 알림 ─────────────────────────────────────────────

/// 폴더의 파일 이름 변화(생김·지움·이름 바꿈)를 기다리는 것 — `FileSystemWatcher(NotifyFilters.FileName)` 의 자리.
/// [`ChangeWait::stop`] 으로 기다림을 깨운다.
pub struct ChangeWait {
    change: Option<Handle>,
    stop: Handle,
}

// SAFETY: 두 손잡이는 커널 객체이고 어느 실마리에서 기다리고 깨워도 된다.
unsafe impl Send for ChangeWait {}
// SAFETY: 같다. `stop` 은 SetEvent 만 한다.
unsafe impl Sync for ChangeWait {}

/// 기다림의 끝.
#[derive(Debug, PartialEq, Eq)]
pub enum Woke {
    Changed,
    Timeout,
    Stopped,
}

impl ChangeWait {
    /// 폴더를 지켜보기 시작한다. 알림을 걸 수 없으면(폴더가 없음 등) 깨우기만 되고 시간이 다 될 때까지 잔다 — C# 도
    /// 감시가 실패하면 짧은 간격의 다시 보기만 남는다.
    pub fn new(folder: &Path) -> io::Result<ChangeWait> {
        // SAFETY: 이름 없는 수동 재설정 사건.
        let stop = unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) };
        if stop.is_null() {
            return Err(io::Error::last_os_error());
        }
        let path = wide_path(folder);
        // SAFETY: 경로는 NUL 로 끝난다.
        let change =
            unsafe { FindFirstChangeNotificationW(path.as_ptr(), 0, FILE_NOTIFY_CHANGE_FILE_NAME) };
        let change = (!change.is_null() && change as isize != -1).then_some(change);
        Ok(ChangeWait { change, stop })
    }

    /// 변화·깨움·시간 끝 중 먼저 오는 것까지 잔다.
    pub fn wait(&self, timeout: Duration) -> Woke {
        let ms = u32::try_from(timeout.as_millis()).unwrap_or(u32::MAX - 1);
        let mut handles = vec![self.stop];
        if let Some(change) = self.change {
            handles.push(change);
        }
        // SAFETY: 손잡이들은 이 구조체가 살아 있는 동안 열려 있다.
        let woke = unsafe { WaitForMultipleObjects(handles.len() as u32, handles.as_ptr(), 0, ms) };
        match woke {
            WAIT_OBJECT_0 => Woke::Stopped,
            w if w == WAIT_OBJECT_0 + 1 => {
                if let Some(change) = self.change {
                    // SAFETY: 다음 변화를 다시 건다.
                    unsafe { FindNextChangeNotification(change) };
                }
                Woke::Changed
            }
            _ => Woke::Timeout,
        }
    }

    /// 기다림을 깨운다. 다음 기다림도 곧바로 깨어난다.
    pub fn stop(&self) {
        // SAFETY: 사건 손잡이는 열려 있다.
        unsafe { SetEvent(self.stop) };
    }
}

impl Drop for ChangeWait {
    fn drop(&mut self) {
        // SAFETY: 이 구조체만 쥔 손잡이들을 한 번 닫는다.
        unsafe {
            if let Some(change) = self.change {
                FindCloseChangeNotification(change);
            }
            CloseHandle(self.stop);
        }
    }
}

// ── 등록부 ───────────────────────────────────────────────────

/// `Registry.CurrentUser.OpenSubKey(subKey)?.GetValue("")` 가 글이면 그 글. 키가 없거나 글이 아니면 `None`.
/// `REG_EXPAND_SZ` 는 .NET 처럼 환경 변수를 펴서 낸다.
pub fn current_user_default_value(sub_key: &str) -> io::Result<Option<String>> {
    const HKEY_CURRENT_USER: isize = 0x8000_0001_u32 as i32 as isize;
    let sub = wide(sub_key);
    let mut key: Handle = std::ptr::null_mut();
    // SAFETY: 미리 정의된 뿌리 키와 NUL 로 끝나는 하위 키 이름.
    let status = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER as Handle,
            sub.as_ptr(),
            0,
            KEY_READ,
            &mut key,
        )
    };
    if status == 2 {
        return Ok(None); // ERROR_FILE_NOT_FOUND — 키가 없다.
    }
    if status != 0 {
        return Err(io::Error::from_raw_os_error(status));
    }
    let result = (|| {
        let empty = [0u16];
        let (mut kind, mut size) = (0u32, 0u32);
        // SAFETY: 크기만 묻는다.
        let status = unsafe {
            RegQueryValueExW(
                key,
                empty.as_ptr(),
                std::ptr::null_mut(),
                &mut kind,
                std::ptr::null_mut(),
                &mut size,
            )
        };
        if status == 2 {
            return Ok(None);
        }
        if status != 0 {
            return Err(io::Error::from_raw_os_error(status));
        }
        if kind != REG_SZ && kind != REG_EXPAND_SZ {
            return Ok(None);
        }
        let mut buffer = vec![0u16; (size as usize).div_ceil(2) + 1];
        let mut bytes = (buffer.len() * 2) as u32;
        // SAFETY: 버퍼는 `bytes` 바이트다.
        let status = unsafe {
            RegQueryValueExW(
                key,
                empty.as_ptr(),
                std::ptr::null_mut(),
                &mut kind,
                buffer.as_mut_ptr().cast(),
                &mut bytes,
            )
        };
        if status != 0 {
            return Err(io::Error::from_raw_os_error(status));
        }
        buffer.truncate(bytes as usize / 2);
        while buffer.last() == Some(&0) {
            buffer.pop();
        }
        let text = String::from_utf16_lossy(&buffer);
        Ok(Some(if kind == REG_EXPAND_SZ {
            expand(&text)
        } else {
            text
        }))
    })();
    // SAFETY: 위에서 연 키를 한 번 닫는다.
    unsafe { RegCloseKey(key) };
    result
}

fn expand(text: &str) -> String {
    let source = wide(text);
    // SAFETY: 크기를 묻는다(대상 NULL, 크기 0).
    let needed = unsafe { ExpandEnvironmentStringsW(source.as_ptr(), std::ptr::null_mut(), 0) };
    if needed == 0 {
        return text.to_string();
    }
    let mut target = vec![0u16; needed as usize];
    // SAFETY: 대상은 `needed` 글자다.
    let written =
        unsafe { ExpandEnvironmentStringsW(source.as_ptr(), target.as_mut_ptr(), needed) };
    if written == 0 || written > needed {
        return text.to_string();
    }
    target.truncate(written as usize - 1);
    String::from_utf16_lossy(&target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_format_matches_dotnet() {
        assert_eq!(
            format_round_trip(1_794_020_645, 1_234_567),
            "2026-11-07T03:04:05.1234567Z"
        );
        assert_eq!(
            format_round_trip(951_782_400, 0),
            "2000-02-29T00:00:00.0000000Z"
        );
    }

    #[test]
    fn local_offset_agrees_with_local_now() {
        let since = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let local = local_now().unix_seconds();
        let offset = local_offset_seconds(since);
        // 두 번 읽는 사이에 몇 초가 흐를 수 있다.
        assert!(
            (local - since - offset).abs() <= 5,
            "{local} {since} {offset}"
        );
        assert_eq!(offset % 60, 0);
        assert_eq!(local_offset_seconds(-20_000_000_000), 0);
    }

    #[test]
    fn this_process_is_running_since_its_start() {
        let started = process_start().unwrap();
        let pid = std::process::id() as i32;
        assert_eq!(running_process_start(pid).unwrap(), Some(started));
        assert_eq!(running_process_start(i32::MAX).unwrap(), None);
        assert_eq!(running_process_start(-1).unwrap(), None);
    }

    #[test]
    fn guid_n_has_version_and_variant() {
        let id = new_guid_n();
        assert_eq!(id.len(), 32);
        assert_eq!(&id[12..13], "4");
        assert!(matches!(&id[16..17], "8" | "9" | "a" | "b"));
    }

    #[test]
    fn mutex_is_held_until_dropped() {
        let name = format!("Local\\pclm-home-test-{}", new_guid_n());
        let first = create_mutex(&name).unwrap().expect("새로 지었다");
        assert!(create_mutex(&name).unwrap().is_none());
        drop(first);
        assert!(create_mutex(&name).unwrap().is_some());
    }
}

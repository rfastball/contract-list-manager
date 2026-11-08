//! [`Os`] 의 진짜 — Windows API 를 직접 부른다. 의존을 더하지 않으려고(`windows-sys` 등) 쓰는 몇 개만 선언한다
//! (`pclm-home` 의 `win` 과 같은 자세).
//!
//! C# 과 갈리는 손이 하나 있다. 새로 뜬 브라우저 창의 주소창에 주소를 넣는 일(`ExtensionSetup.Navigate`)을 C# 은 UI
//! Automation 으로 주소창(Edit)을 찾아 값을 넣었다. 여기서는 COM 을 들이지 않고, 그 창이 정말 앞에 온 것을 본 뒤에만
//! Ctrl+L(Chrome · Edge 의 주소창 가기) · 글자 · Enter 를 보낸다. 다른 창에 Enter 가 가지 않게 하는 자리는 같다.

#![allow(unsafe_code)]

use std::ffi::c_void;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::os::{Os, RegistryRoot, RegistryValue, SpecialFolder};

type Handle = *mut c_void;

const HKEY_CURRENT_USER: isize = 0x8000_0001_u32 as i32 as isize;
const HKEY_LOCAL_MACHINE: isize = 0x8000_0002_u32 as i32 as isize;
const KEY_READ: u32 = 0x20019;
const KEY_WRITE: u32 = 0x20006;
const KEY_SET_VALUE: u32 = 0x0002;
const REG_SZ: u32 = 1;
const REG_EXPAND_SZ: u32 = 2;
const REG_BINARY: u32 = 3;
const ERROR_FILE_NOT_FOUND: i32 = 2;
const EVENT_MODIFY_STATE: u32 = 0x0002;
const SYNCHRONIZE: u32 = 0x0010_0000;
const ASFW_ANY: u32 = u32::MAX;
const TH32CS_SNAPPROCESS: u32 = 0x2;
const GMEM_MOVEABLE: u32 = 0x2;
const CF_UNICODETEXT: u32 = 13;
const SW_SHOWNORMAL: i32 = 1;
const INPUT_KEYBOARD: u32 = 1;
const KEYEVENTF_KEYUP: u32 = 0x2;
const KEYEVENTF_UNICODE: u32 = 0x4;
const VK_CONTROL: u16 = 0x11;
const VK_RETURN: u16 = 0x0D;
const VK_L: u16 = 0x4C;

#[repr(C)]
struct ProcessEntry {
    size: u32,
    usage: u32,
    pid: u32,
    heap: usize,
    module: u32,
    threads: u32,
    parent: u32,
    priority: i32,
    flags: u32,
    exe: [u16; 260],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct KeyboardInput {
    vk: u16,
    scan: u16,
    flags: u32,
    time: u32,
    extra: usize,
}

/// `INPUT` — 공용체의 가장 큰 칸(MOUSEINPUT, 64비트 32바이트)에 맞춰 덧댄다.
#[repr(C)]
#[derive(Clone, Copy)]
struct Input {
    kind: u32,
    keyboard: KeyboardInput,
    pad: [u8; 8],
}

#[link(name = "advapi32")]
unsafe extern "system" {
    fn RegOpenKeyExW(key: isize, sub: *const u16, options: u32, desired: u32, out: *mut isize)
    -> i32;
    fn RegCreateKeyExW(
        key: isize,
        sub: *const u16,
        reserved: u32,
        class: *const u16,
        options: u32,
        desired: u32,
        security: *const c_void,
        out: *mut isize,
        disposition: *mut u32,
    ) -> i32;
    fn RegQueryValueExW(
        key: isize,
        name: *const u16,
        reserved: *mut u32,
        kind: *mut u32,
        data: *mut u8,
        size: *mut u32,
    ) -> i32;
    fn RegSetValueExW(
        key: isize,
        name: *const u16,
        reserved: u32,
        kind: u32,
        data: *const u8,
        size: u32,
    ) -> i32;
    fn RegDeleteValueW(key: isize, name: *const u16) -> i32;
    fn RegCloseKey(key: isize) -> i32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn OpenEventW(access: u32, inherit: i32, name: *const u16) -> Handle;
    fn SetEvent(handle: Handle) -> i32;
    fn CloseHandle(handle: Handle) -> i32;
    fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> Handle;
    fn Process32FirstW(snapshot: Handle, entry: *mut ProcessEntry) -> i32;
    fn Process32NextW(snapshot: Handle, entry: *mut ProcessEntry) -> i32;
    fn GlobalAlloc(flags: u32, bytes: usize) -> Handle;
    fn GlobalLock(memory: Handle) -> *mut c_void;
    fn GlobalUnlock(memory: Handle) -> i32;
    fn GlobalFree(memory: Handle) -> Handle;
    fn ExpandEnvironmentStringsW(source: *const u16, target: *mut u16, size: u32) -> u32;
}

#[link(name = "user32")]
unsafe extern "system" {
    fn EnumWindows(callback: extern "system" fn(isize, isize) -> i32, param: isize) -> i32;
    fn IsWindowVisible(window: isize) -> i32;
    fn GetWindowThreadProcessId(window: isize, pid: *mut u32) -> u32;
    fn SetForegroundWindow(window: isize) -> i32;
    fn GetForegroundWindow() -> isize;
    fn AllowSetForegroundWindow(pid: u32) -> i32;
    fn SendInput(count: u32, inputs: *const Input, size: i32) -> u32;
    fn OpenClipboard(owner: isize) -> i32;
    fn EmptyClipboard() -> i32;
    fn SetClipboardData(format: u32, memory: Handle) -> Handle;
    fn CloseClipboard() -> i32;
}

#[link(name = "shell32")]
unsafe extern "system" {
    fn ShellExecuteW(
        window: isize,
        verb: *const u16,
        file: *const u16,
        parameters: *const u16,
        directory: *const u16,
        show: i32,
    ) -> isize;
}

#[link(name = "version")]
unsafe extern "system" {
    fn GetFileVersionInfoSizeW(file: *const u16, handle: *mut u32) -> u32;
    fn GetFileVersionInfoW(file: *const u16, handle: u32, size: u32, data: *mut c_void) -> i32;
    fn VerQueryValueW(
        block: *const c_void,
        sub: *const u16,
        buffer: *mut *mut c_void,
        size: *mut u32,
    ) -> i32;
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn wide_path(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn root_key(root: RegistryRoot) -> isize {
    match root {
        RegistryRoot::CurrentUser => HKEY_CURRENT_USER,
        RegistryRoot::LocalMachine => HKEY_LOCAL_MACHINE,
    }
}

fn status(code: i32) -> io::Result<()> {
    if code == 0 {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(code))
    }
}

fn expand(text: &str) -> String {
    let source = wide(text);
    // SAFETY: 크기만 묻는다.
    let needed = unsafe { ExpandEnvironmentStringsW(source.as_ptr(), std::ptr::null_mut(), 0) };
    if needed == 0 {
        return text.to_string();
    }
    let mut buffer = vec![0u16; needed as usize];
    // SAFETY: 버퍼는 `needed` 글자다.
    let written = unsafe { ExpandEnvironmentStringsW(source.as_ptr(), buffer.as_mut_ptr(), needed) };
    if written == 0 || written > needed {
        return text.to_string();
    }
    String::from_utf16_lossy(&buffer[..written as usize - 1])
}

/// 그 이름(확장자 없이, 대소문자 무시 — `Process.GetProcessesByName`)의 프로세스 번호들.
fn processes_named(name: &str) -> Vec<u32> {
    let wanted = format!("{}.exe", name).to_lowercase();
    let mut pids = Vec::new();
    // SAFETY: 스냅숏 손잡이를 이 함수 안에서 닫는다. 구조체는 크기를 적어 넘긴다.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot.is_null() || snapshot as isize == -1 {
            return pids;
        }
        let mut entry: ProcessEntry = std::mem::zeroed();
        entry.size = std::mem::size_of::<ProcessEntry>() as u32;
        let mut more = Process32FirstW(snapshot, &mut entry) != 0;
        while more {
            let end = entry.exe.iter().position(|&c| c == 0).unwrap_or(260);
            if String::from_utf16_lossy(&entry.exe[..end]).to_lowercase() == wanted {
                pids.push(entry.pid);
            }
            more = Process32NextW(snapshot, &mut entry) != 0;
        }
        CloseHandle(snapshot);
    }
    pids
}

struct Collect {
    pids: Vec<u32>,
    windows: Vec<isize>,
}

extern "system" fn collect(window: isize, param: isize) -> i32 {
    // SAFETY: `param` 은 아래 `top_windows` 가 넘긴 `Collect` 다 — 열거가 끝날 때까지 산다.
    let found = unsafe { &mut *(param as *mut Collect) };
    // SAFETY: 열거가 건넨 창 손잡이.
    unsafe {
        if IsWindowVisible(window) != 0 {
            let mut pid = 0u32;
            GetWindowThreadProcessId(window, &mut pid);
            if found.pids.contains(&pid) {
                found.windows.push(window);
            }
        }
    }
    1
}

/// 그 프로세스들의 보이는 최상위 창(UI Automation 의 바탕 화면 아래 자식과 같은 것).
fn top_windows(process: &str) -> Vec<isize> {
    let mut found = Collect {
        pids: processes_named(process),
        windows: Vec::new(),
    };
    if found.pids.is_empty() {
        return Vec::new();
    }
    // SAFETY: 되부름은 열거 동안만 `found` 를 쓴다.
    unsafe {
        EnumWindows(collect, &mut found as *mut Collect as isize);
    }
    found.windows
}

fn new_window(process: &str, before: &[isize]) -> Option<isize> {
    top_windows(process)
        .into_iter()
        .find(|w| !before.contains(w))
}

fn key(vk: u16, scan: u16, flags: u32) -> Input {
    Input {
        kind: INPUT_KEYBOARD,
        keyboard: KeyboardInput {
            vk,
            scan,
            flags,
            time: 0,
            extra: 0,
        },
        pad: [0; 8],
    }
}

fn send(inputs: &[Input]) -> bool {
    // SAFETY: 배열과 그 원소 크기를 그대로 넘긴다.
    let sent = unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<Input>() as i32,
        )
    };
    sent as usize == inputs.len()
}

/// Windows 명령줄 규칙으로 인자 하나를 감싼다(.NET `PasteArguments`).
fn quote(value: &str) -> String {
    if !value.is_empty() && !value.contains([' ', '\t', '"']) {
        return value.to_string();
    }
    let mut out = String::from('"');
    let mut slashes = 0;
    for c in value.chars() {
        match c {
            '\\' => slashes += 1,
            '"' => {
                out.push_str(&"\\".repeat(slashes * 2 + 1));
                out.push('"');
                slashes = 0;
            }
            _ => {
                out.push_str(&"\\".repeat(slashes));
                out.push(c);
                slashes = 0;
            }
        }
    }
    out.push_str(&"\\".repeat(slashes * 2));
    out.push('"');
    out
}

fn shell_execute(file: &Path, parameters: Option<&str>) -> io::Result<()> {
    let file = wide_path(file);
    let parameters = parameters.map(wide);
    // SAFETY: NUL 로 끝나는 글들. 동사는 기본(null)이다 — .NET `UseShellExecute` 와 같다.
    let result = unsafe {
        ShellExecuteW(
            0,
            std::ptr::null(),
            file.as_ptr(),
            parameters.as_ref().map_or(std::ptr::null(), |p| p.as_ptr()),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if result > 32 {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(result as i32))
    }
}

/// 진짜 운영체제.
#[derive(Clone, Copy, Debug, Default)]
pub struct WindowsOs;

impl Os for WindowsOs {
    fn process_path(&self) -> Option<PathBuf> {
        std::env::current_exe().ok()
    }

    fn default_home(&self) -> io::Result<PathBuf> {
        pclm_home::Home::default_home().map(|h| h.directory().to_path_buf())
    }

    fn special_folder(&self, folder: SpecialFolder) -> PathBuf {
        let name = match folder {
            SpecialFolder::LocalApplicationData => "LOCALAPPDATA",
            SpecialFolder::ProgramFiles => "ProgramFiles",
            SpecialFolder::ProgramFilesX86 => "ProgramFiles(x86)",
        };
        std::env::var_os(name).map(PathBuf::from).unwrap_or_default()
    }

    fn temp_path(&self) -> PathBuf {
        std::env::temp_dir()
    }

    fn registry_get(
        &self,
        root: RegistryRoot,
        key: &str,
        name: &str,
    ) -> io::Result<Option<RegistryValue>> {
        let sub = wide(key);
        let mut handle = 0isize;
        // SAFETY: 미리 정의된 뿌리 키와 NUL 로 끝나는 하위 키 이름.
        let code = unsafe { RegOpenKeyExW(root_key(root), sub.as_ptr(), 0, KEY_READ, &mut handle) };
        if code == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        status(code)?;
        let value = wide(name);
        let result = (|| {
            let (mut kind, mut size) = (0u32, 0u32);
            // SAFETY: 크기만 묻는다.
            let code = unsafe {
                RegQueryValueExW(
                    handle,
                    value.as_ptr(),
                    std::ptr::null_mut(),
                    &mut kind,
                    std::ptr::null_mut(),
                    &mut size,
                )
            };
            if code == ERROR_FILE_NOT_FOUND {
                return Ok(None);
            }
            status(code)?;
            let mut buffer = vec![0u8; size as usize + 2];
            let mut bytes = buffer.len() as u32;
            // SAFETY: 버퍼는 `bytes` 바이트다.
            let code = unsafe {
                RegQueryValueExW(
                    handle,
                    value.as_ptr(),
                    std::ptr::null_mut(),
                    &mut kind,
                    buffer.as_mut_ptr(),
                    &mut bytes,
                )
            };
            status(code)?;
            buffer.truncate(bytes as usize);
            Ok(Some(match kind {
                REG_SZ | REG_EXPAND_SZ => {
                    let units: Vec<u16> = buffer
                        .chunks_exact(2)
                        .map(|c| u16::from_le_bytes([c[0], c[1]]))
                        .collect();
                    let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
                    let text = String::from_utf16_lossy(&units[..end]);
                    RegistryValue::String(if kind == REG_EXPAND_SZ {
                        expand(&text)
                    } else {
                        text
                    })
                }
                REG_BINARY => RegistryValue::Binary(buffer),
                _ => RegistryValue::Other,
            }))
        })();
        // SAFETY: 위에서 연 키.
        unsafe { RegCloseKey(handle) };
        result
    }

    fn registry_set_string(&self, key: &str, name: &str, value: &str) -> io::Result<()> {
        let sub = wide(key);
        let mut handle = 0isize;
        let mut disposition = 0u32;
        // SAFETY: NUL 로 끝나는 하위 키 이름, 나머지는 기본.
        let code = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                sub.as_ptr(),
                0,
                std::ptr::null(),
                0,
                KEY_WRITE,
                std::ptr::null(),
                &mut handle,
                &mut disposition,
            )
        };
        status(code)?;
        let name = wide(name);
        let data = wide(value);
        // SAFETY: 글은 NUL 까지 바이트 수로 넘긴다.
        let code = unsafe {
            RegSetValueExW(
                handle,
                name.as_ptr(),
                0,
                REG_SZ,
                data.as_ptr().cast(),
                (data.len() * 2) as u32,
            )
        };
        // SAFETY: 위에서 연 키.
        unsafe { RegCloseKey(handle) };
        status(code)
    }

    fn registry_delete_value(&self, key: &str, name: &str) -> io::Result<()> {
        let sub = wide(key);
        let mut handle = 0isize;
        // SAFETY: 미리 정의된 뿌리 키와 NUL 로 끝나는 하위 키 이름.
        let code = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                sub.as_ptr(),
                0,
                KEY_SET_VALUE,
                &mut handle,
            )
        };
        if code == ERROR_FILE_NOT_FOUND {
            return Ok(());
        }
        status(code)?;
        let name = wide(name);
        // SAFETY: 위에서 연 키와 NUL 로 끝나는 값 이름.
        let code = unsafe { RegDeleteValueW(handle, name.as_ptr()) };
        // SAFETY: 위에서 연 키.
        unsafe { RegCloseKey(handle) };
        if code == ERROR_FILE_NOT_FOUND {
            Ok(())
        } else {
            status(code)
        }
    }

    fn shell_open(&self, path: &Path) -> io::Result<()> {
        shell_execute(path, None)
    }

    fn start_process(&self, program: &Path, args: &[String], shell: bool) -> io::Result<()> {
        if shell {
            let parameters: Vec<String> = args.iter().map(|a| quote(a)).collect();
            let joined = parameters.join(" ");
            shell_execute(program, (!joined.is_empty()).then_some(joined.as_str()))
        } else {
            std::process::Command::new(program).args(args).spawn().map(drop)
        }
    }

    fn signal_activation(&self, name: &str) -> bool {
        let name = wide(name);
        // SAFETY: NUL 로 끝나는 이름. 연 손잡이는 여기서 닫는다.
        unsafe {
            let handle = OpenEventW(EVENT_MODIFY_STATE | SYNCHRONIZE, 0, name.as_ptr());
            if handle.is_null() {
                return false;
            }
            AllowSetForegroundWindow(ASFW_ANY);
            let set = SetEvent(handle) != 0;
            CloseHandle(handle);
            set
        }
    }

    fn browser_windows(&self, process: &str) -> Vec<isize> {
        top_windows(process)
    }

    fn raise_new_window(&self, process: &str, before: &[isize], wait: Duration) -> bool {
        let step = Duration::from_millis(100);
        let mut waited = Duration::ZERO;
        while waited < wait {
            std::thread::sleep(step);
            waited += step;
            let Some(window) = new_window(process, before) else {
                continue;
            };
            // SAFETY: 열거로 얻은 창 손잡이 — 사라졌으면 그저 실패한다.
            unsafe {
                SetForegroundWindow(window);
                return GetForegroundWindow() == window;
            }
        }
        false
    }

    fn navigate_new_window(&self, process: &str, before: &[isize], url: &str) -> bool {
        for _ in 0..50 {
            std::thread::sleep(Duration::from_millis(100));
            let Some(window) = new_window(process, before) else {
                continue;
            };
            // 창이 선 직후에는 주소창이 아직 글을 받지 못한다 — C# 은 주소창이 나타날 때까지 기다렸다.
            std::thread::sleep(Duration::from_millis(400));
            // SAFETY: 열거로 얻은 창 손잡이.
            unsafe {
                SetForegroundWindow(window);
                if GetForegroundWindow() != window {
                    return false;
                }
            }
            let mut inputs = vec![
                key(VK_CONTROL, 0, 0),
                key(VK_L, 0, 0),
                key(VK_L, 0, KEYEVENTF_KEYUP),
                key(VK_CONTROL, 0, KEYEVENTF_KEYUP),
            ];
            for unit in url.encode_utf16() {
                inputs.push(key(0, unit, KEYEVENTF_UNICODE));
                inputs.push(key(0, unit, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP));
            }
            if !send(&inputs) {
                return false;
            }
            // SAFETY: 위와 같다. Enter 앞에서 한 번 더 본다.
            if unsafe { GetForegroundWindow() } != window {
                return false;
            }
            return send(&[key(VK_RETURN, 0, 0), key(VK_RETURN, 0, KEYEVENTF_KEYUP)]);
        }
        false
    }

    fn set_clipboard(&self, text: &str) {
        let units: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: 잡은 기억은 클립보드에 넘기거나 놓는다. 클립보드는 연 실마리에서 닫는다.
        unsafe {
            if OpenClipboard(0) == 0 {
                return;
            }
            EmptyClipboard();
            let memory = GlobalAlloc(GMEM_MOVEABLE, units.len() * 2);
            if !memory.is_null() {
                let target = GlobalLock(memory);
                if target.is_null() {
                    GlobalFree(memory);
                } else {
                    std::ptr::copy_nonoverlapping(units.as_ptr(), target.cast(), units.len());
                    GlobalUnlock(memory);
                    if SetClipboardData(CF_UNICODETEXT, memory).is_null() {
                        GlobalFree(memory);
                    }
                }
            }
            CloseClipboard();
        }
    }

    fn file_version(&self, path: &Path) -> Option<String> {
        let file = wide_path(path);
        let mut ignored = 0u32;
        // SAFETY: 크기를 묻고, 그만큼의 버퍼에 받고, 그 버퍼 안의 자리만 읽는다.
        unsafe {
            let size = GetFileVersionInfoSizeW(file.as_ptr(), &mut ignored);
            if size == 0 {
                return None;
            }
            let mut block = vec![0u8; size as usize];
            if GetFileVersionInfoW(file.as_ptr(), 0, size, block.as_mut_ptr().cast()) == 0 {
                return None;
            }
            let query = |sub: &str| -> Option<(*mut c_void, u32)> {
                let sub = wide(sub);
                let mut buffer: *mut c_void = std::ptr::null_mut();
                let mut length = 0u32;
                (VerQueryValueW(block.as_ptr().cast(), sub.as_ptr(), &mut buffer, &mut length)
                    != 0
                    && !buffer.is_null())
                .then_some((buffer, length))
            };
            let mut codes = Vec::new();
            if let Some((buffer, length)) = query("\\VarFileInfo\\Translation")
                && length >= 4
            {
                let pair = buffer as *const u16;
                codes.push(format!("{:04x}{:04x}", *pair, *pair.add(1)));
            }
            codes.extend(["040904b0", "040904e4", "04090000"].map(String::from));
            for code in codes {
                if let Some((buffer, length)) = query(&format!("\\StringFileInfo\\{code}\\FileVersion"))
                {
                    let units = std::slice::from_raw_parts(buffer as *const u16, length as usize);
                    let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
                    return Some(String::from_utf16_lossy(&units[..end]));
                }
            }
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 인자는_윈도_명령줄_규칙으로_싼다() {
        assert_eq!(quote("--new-window"), "--new-window");
        assert_eq!(quote(""), "\"\"");
        assert_eq!(quote(r"C:\a b\"), r#""C:\a b\\""#);
        assert_eq!(quote(r#"a"b"#), r#""a\"b""#);
    }

    #[test]
    fn input_은_윈도의_크기다() {
        assert_eq!(std::mem::size_of::<Input>(), 40);
    }
}

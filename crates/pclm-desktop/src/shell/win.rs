//! 창이 Windows 에 곧바로 하는 작은 일들 — 알림 상자(C# `MessageBox.Show`), 등록부(파일 연결), 창을 내려 둔 채 띄우기.

#![allow(unsafe_code)]

use std::io;
use std::path::Path;

use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, HWND};
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_NONE, REG_OPTION_NON_VOLATILE, REG_SZ,
    RegCloseKey, RegCreateKeyExW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
};
use windows_sys::Win32::UI::Shell::{SHCNE_ASSOCCHANGED, SHCNF_IDLIST, SHChangeNotify};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    MB_ICONERROR, MB_ICONINFORMATION, MB_ICONWARNING, MB_OK, MESSAGEBOX_STYLE, MessageBoxW,
    SW_SHOWMINNOACTIVE, ShowWindow,
};

use crate::association::Registry;

/// 제목(C# 의 알림 상자는 모두 「계약 목록」).
pub const CAPTION: &str = "계약 목록";

/// NUL 로 끝나는 UTF-16. 글 안의 NUL 은 그 앞에서 끊긴다.
pub fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16()
        .take_while(|&u| u != 0)
        .chain([0])
        .collect()
}

/// 알림 상자의 그림.
#[derive(Clone, Copy)]
pub enum Icon {
    Information,
    Warning,
    Error,
}

/// `MessageBox.Show(text, "계약 목록", OK, icon)`. `owner` 가 있으면 그 창의 모달이다.
pub fn message_box(owner: Option<HWND>, text: &str, icon: Icon) {
    let style: MESSAGEBOX_STYLE = MB_OK
        | match icon {
            Icon::Information => MB_ICONINFORMATION,
            Icon::Warning => MB_ICONWARNING,
            Icon::Error => MB_ICONERROR,
        };
    let (text, caption) = (wide(text), wide(CAPTION));
    // SAFETY: NUL 로 끝나는 글. 손잡이는 없거나 살아 있는 창이다.
    unsafe {
        MessageBoxW(
            owner.unwrap_or(std::ptr::null_mut()),
            text.as_ptr(),
            caption.as_ptr(),
            style,
        );
    }
}

/// WPF 의 `ShowActivated = false` + `WindowState = Minimized` — 작업 표시줄에 내려 둔 채 띄우고 앞에 내지 않는다.
pub fn show_minimized_without_activating(window: HWND) {
    // SAFETY: 살아 있는 창 손잡이.
    unsafe {
        ShowWindow(window, SW_SHOWMINNOACTIVE);
    }
}

/// 진짜 HKCU — 파일 연결([`crate::association`])이 쓴다. 업무 홈일 때만 불린다.
pub struct CurrentUser;

struct Key(HKEY);

impl Drop for Key {
    fn drop(&mut self) {
        // SAFETY: 연 손잡이를 한 번 닫는다.
        unsafe {
            RegCloseKey(self.0);
        }
    }
}

fn status(code: u32) -> io::Result<()> {
    if code == 0 {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(code as i32))
    }
}

fn create(key: &str) -> io::Result<Key> {
    let name = wide(key);
    let mut handle: HKEY = std::ptr::null_mut();
    // SAFETY: NUL 로 끝나는 이름, 받는 자리는 지역 변수.
    status(unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            name.as_ptr(),
            0,
            std::ptr::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE | KEY_READ,
            std::ptr::null(),
            &mut handle,
            std::ptr::null_mut(),
        )
    })?;
    Ok(Key(handle))
}

fn set(key: &str, name: &str, kind: u32, data: &[u8]) -> io::Result<()> {
    let key = create(key)?;
    let name = wide(name);
    // SAFETY: 연 키, NUL 로 끝나는 이름, 길이가 맞는 바이트.
    status(unsafe {
        RegSetValueExW(
            key.0,
            name.as_ptr(),
            0,
            kind,
            data.as_ptr(),
            data.len() as u32,
        )
    })
}

impl Registry for CurrentUser {
    fn default_value(&self, key: &str) -> io::Result<Option<String>> {
        let name = wide(key);
        let mut handle: HKEY = std::ptr::null_mut();
        // SAFETY: NUL 로 끝나는 이름.
        let opened =
            unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, name.as_ptr(), 0, KEY_READ, &mut handle) };
        if opened == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        status(opened)?;
        let key = Key(handle);
        let mut kind = 0u32;
        let mut size = 0u32;
        let empty = [0u16];
        // SAFETY: 크기만 묻는다.
        let asked = unsafe {
            RegQueryValueExW(
                key.0,
                empty.as_ptr(),
                std::ptr::null(),
                &mut kind,
                std::ptr::null_mut(),
                &mut size,
            )
        };
        if asked == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        status(asked)?;
        if kind != REG_SZ {
            return Ok(None);
        }
        let mut buffer = vec![0u16; (size as usize).div_ceil(2) + 1];
        let mut bytes = (buffer.len() * 2) as u32;
        // SAFETY: 버퍼의 바이트 길이를 넘긴다.
        status(unsafe {
            RegQueryValueExW(
                key.0,
                empty.as_ptr(),
                std::ptr::null(),
                &mut kind,
                buffer.as_mut_ptr().cast(),
                &mut bytes,
            )
        })?;
        let units = (bytes as usize / 2).min(buffer.len());
        let text = &buffer[..units];
        let end = text.iter().position(|&u| u == 0).unwrap_or(text.len());
        Ok(Some(String::from_utf16_lossy(&text[..end])))
    }

    fn set_string(&self, key: &str, name: &str, value: &str) -> io::Result<()> {
        let data: Vec<u8> = wide(value).iter().flat_map(|u| u.to_le_bytes()).collect();
        set(key, name, REG_SZ, &data)
    }

    fn set_empty(&self, key: &str, name: &str) -> io::Result<()> {
        set(key, name, REG_NONE, &[])
    }

    fn notify_changed(&self) {
        // SAFETY: 항목 없이 알린다.
        unsafe {
            SHChangeNotify(
                SHCNE_ASSOCCHANGED as i32,
                SHCNF_IDLIST,
                std::ptr::null(),
                std::ptr::null(),
            )
        }
    }
}

/// 오류 기록 한 줄의 때 — C# `DateTime.Now.ToString("O")`(지역 시각과 그 차이).
pub fn now_round_trip() -> String {
    let now = pclm_home::win::local_now();
    let utc = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default();
    let offset = pclm_home::win::local_offset_seconds(utc);
    let sign = if offset < 0 { '-' } else { '+' };
    let offset = offset.abs();
    format!(
        "{}T{:02}:{:02}:{:02}.{:07}{sign}{:02}:{:02}",
        now.day_text(),
        now.hour,
        now.minute,
        now.second,
        now.ticks,
        offset / 3600,
        offset % 3600 / 60
    )
}

/// C# `App.Report` — 오류를 기록 파일에 덧붙이고 알림 상자로 보인다. 기록조차 못 남기면 알림 상자로 족하다.
pub fn report(log: &Path, message: &str, detail: &str) {
    let entry = format!("{}\r\n{detail}\r\n\r\n", now_round_trip());
    if let Some(parent) = log.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)
        .and_then(|mut file| io::Write::write_all(&mut file, entry.as_bytes()));
    message_box(
        None,
        &format!("{message}\r\n\r\n자세한 내용: {}", log.display()),
        Icon::Error,
    );
}

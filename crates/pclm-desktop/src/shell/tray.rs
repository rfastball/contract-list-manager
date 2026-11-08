//! 알림 영역의 아이콘(C# `Pclm.App/TrayIcon.cs`). 「알림 영역에 두기」 가 켜져 있는 동안만 선다 — 닫기(X)를 누른 창이 여기로
//! 숨는다.
//!
//! 두 번 누르면 창을 다시 띄우고, 오른쪽 단추 메뉴로 「열기」(굵게 — 두 번 누르기와 같은 일) · 「끝내기」 를 고른다. 창을 닫아도
//! 끝나지 않으니 **정말 끝내는 길은 여기 「끝내기」** 다. 아이콘은 제 실마리의 숨은 창이 쥔다 — 창(Tauri)의 차례와 따로 돈다.
//!
//! **내릴 때 반드시 치운다**(`Drop`). 손잡이를 놓지 않고 프로세스가 끝나면 마우스를 갖다 댈 때까지 죽은 아이콘이 남는다.

#![allow(unsafe_code)]

use std::sync::mpsc;
use std::thread::JoinHandle;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Shell::{
    ExtractIconExW, NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_INFO, NIM_ADD, NIM_DELETE,
    NIM_MODIFY, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon, DestroyMenu,
    DestroyWindow, DispatchMessageW, GWLP_USERDATA, GetCursorPos, GetMessageW, GetWindowLongPtrW,
    HICON, IDI_APPLICATION, LoadIconW, MF_SEPARATOR, MF_STRING, MSG, PostMessageW, PostQuitMessage,
    RegisterClassW, RegisterWindowMessageW, SetForegroundWindow, SetMenuDefaultItem,
    SetWindowLongPtrW, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu, TranslateMessage, WM_APP,
    WM_DESTROY, WM_LBUTTONDBLCLK, WM_NULL, WM_QUERYENDSESSION, WM_RBUTTONUP, WNDCLASSW,
    WS_OVERLAPPED,
};

use super::win::wide;

const CALLBACK: u32 = WM_APP + 1;
const BALLOON: u32 = WM_APP + 2;
const REMOVE: u32 = WM_APP + 3;
const OPEN: usize = 1;
const EXIT: usize = 2;

/// 알림 영역에서 고른 일 — 창의 차례로 넘기는 것은 부르는 쪽이다(이 실마리는 창의 것이 아니다).
pub struct Actions {
    pub open: Box<dyn Fn() + Send>,
    pub exit: Box<dyn Fn() + Send>,
    /// 로그오프 · 종료(C# `SessionEnding`) — 창이 알림 영역으로 숨으며 Windows 를 붙들지 않게 한다.
    pub session_ending: Box<dyn Fn() + Send>,
}

struct State {
    actions: Actions,
    tip: Vec<u16>,
    icon: HICON,
    owned: bool,
    taskbar_created: u32,
}

pub struct Tray {
    window: usize,
    thread: Option<JoinHandle<()>>,
}

impl Tray {
    /// `text` 는 마우스를 갖다 대면 보일 글. 알림 영역은 63자에서 자른다(C# 처럼 62자 뒤에 「…」).
    pub fn new(text: &str, actions: Actions) -> Option<Tray> {
        let units: Vec<u16> = text.encode_utf16().collect();
        let tip = if units.len() > 63 {
            let mut cut = units[..62].to_vec();
            cut.extend("…".encode_utf16());
            cut
        } else {
            units
        };
        let (sender, receiver) = mpsc::channel::<usize>();
        let thread = std::thread::Builder::new()
            .name("pclm-tray".into())
            .spawn(move || run(tip, actions, sender))
            .ok()?;
        let window = receiver.recv().ok().filter(|&w| w != 0)?;
        Some(Tray {
            window,
            thread: Some(thread),
        })
    }

    /// 풍선 알림(C# `ShowBalloonTip(10_000, …, Info)`). 몇 초 동안만 보인다 — 놓쳐도 아이콘은 남는다.
    pub fn notify(&self, title: &str, text: &str) {
        let message = Box::new((wide(title), wide(text)));
        let pointer = Box::into_raw(message);
        // SAFETY: 받는 쪽(BALLOON)이 상자를 되찾아 놓는다. 보내지 못했으면 여기서 놓는다.
        unsafe {
            if PostMessageW(self.window as HWND, BALLOON, 0, pointer as LPARAM) == 0 {
                drop(Box::from_raw(pointer));
            }
        }
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        // SAFETY: 이 실마리가 지은 창. 받은 쪽이 아이콘을 지우고 창을 부순다.
        unsafe {
            PostMessageW(self.window as HWND, REMOVE, 0, 0);
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn data(window: HWND, state: &State) -> NOTIFYICONDATAW {
    let mut data = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: window,
        uID: 1,
        uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
        uCallbackMessage: CALLBACK,
        hIcon: state.icon,
        ..Default::default()
    };
    for (slot, unit) in data.szTip.iter_mut().zip(state.tip.iter().take(127)) {
        *slot = *unit;
    }
    data
}

fn run(tip: Vec<u16>, actions: Actions, sender: mpsc::Sender<usize>) {
    // SAFETY: 이 실마리의 창 하나 — 지은 실마리에서만 쓰고 부순다.
    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let class = wide("PclmTrayWindow");
        let register = WNDCLASSW {
            lpfnWndProc: Some(procedure),
            hInstance: instance,
            lpszClassName: class.as_ptr(),
            ..Default::default()
        };
        RegisterClassW(&register); // 이미 있으면 그대로 쓴다.
        let title = wide("계약 목록");
        let window = CreateWindowExW(
            0,
            class.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        );
        if window.is_null() {
            let _ = sender.send(0);
            return;
        }
        let (icon, owned) = icon();
        let state = Box::new(State {
            actions,
            tip,
            icon,
            owned,
            taskbar_created: RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()),
        });
        let notify = data(window, &state);
        SetWindowLongPtrW(window, GWLP_USERDATA, Box::into_raw(state) as isize);
        Shell_NotifyIconW(NIM_ADD, &notify);
        let _ = sender.send(window as usize);

        let mut message = MSG::default();
        while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

/// exe 에 박힌 앱 아이콘(작은 것). 없으면 빌려 온 시스템 그림 — 그것은 놓지 않는다.
unsafe fn icon() -> (HICON, bool) {
    let exe = std::env::current_exe()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    let exe = wide(&exe);
    let mut small: HICON = std::ptr::null_mut();
    // SAFETY: NUL 로 끝나는 경로, 받는 자리 하나.
    unsafe {
        if ExtractIconExW(exe.as_ptr(), 0, std::ptr::null_mut(), &mut small, 1) > 0
            && !small.is_null()
        {
            return (small, true);
        }
        (LoadIconW(std::ptr::null_mut(), IDI_APPLICATION), false)
    }
}

unsafe extern "system" fn procedure(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: GWLP_USERDATA 는 run 이 맡긴 상자 — WM_DESTROY 에서 되찾기 전까지 산다.
    unsafe {
        let state = GetWindowLongPtrW(window, GWLP_USERDATA) as *mut State;
        if state.is_null() {
            return DefWindowProcW(window, message, wparam, lparam);
        }
        let state = &mut *state;
        match message {
            CALLBACK => {
                match (lparam as u32) & 0xFFFF {
                    WM_LBUTTONDBLCLK => (state.actions.open)(),
                    WM_RBUTTONUP => match menu(window) {
                        OPEN => (state.actions.open)(),
                        EXIT => (state.actions.exit)(),
                        _ => {}
                    },
                    _ => {}
                }
                0
            }
            BALLOON => {
                let (title, text) = *Box::from_raw(lparam as *mut (Vec<u16>, Vec<u16>));
                let mut notify = data(window, state);
                notify.uFlags = NIF_INFO;
                notify.dwInfoFlags = NIIF_INFO;
                notify.Anonymous.uTimeout = 10_000;
                for (slot, unit) in notify.szInfoTitle.iter_mut().zip(title.iter().take(63)) {
                    *slot = *unit;
                }
                for (slot, unit) in notify.szInfo.iter_mut().zip(text.iter().take(255)) {
                    *slot = *unit;
                }
                Shell_NotifyIconW(NIM_MODIFY, &notify);
                0
            }
            REMOVE => {
                Shell_NotifyIconW(NIM_DELETE, &data(window, state));
                DestroyWindow(window);
                0
            }
            WM_QUERYENDSESSION => {
                (state.actions.session_ending)();
                1
            }
            WM_DESTROY => {
                let boxed = Box::from_raw(state as *mut State);
                SetWindowLongPtrW(window, GWLP_USERDATA, 0);
                if boxed.owned {
                    DestroyIcon(boxed.icon);
                }
                PostQuitMessage(0);
                0
            }
            // 탐색기가 다시 켜지면 알림 영역이 비어 있다 — 다시 세운다(WinForms NotifyIcon 과 같다).
            m if m == state.taskbar_created && m != 0 => {
                Shell_NotifyIconW(NIM_ADD, &data(window, state));
                0
            }
            _ => DefWindowProcW(window, message, wparam, lparam),
        }
    }
}

/// 오른쪽 단추 메뉴 — 고른 항목의 번호(그만두면 0).
unsafe fn menu(window: HWND) -> usize {
    // SAFETY: 이 실마리의 창 위에 띄우고 곧바로 부순다.
    unsafe {
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return 0;
        }
        AppendMenuW(menu, MF_STRING, OPEN, wide("열기").as_ptr());
        AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
        AppendMenuW(menu, MF_STRING, EXIT, wide("끝내기").as_ptr());
        SetMenuDefaultItem(menu, OPEN as u32, 0); // 굵게 — 두 번 누르기와 같은 일
        let mut at = POINT::default();
        GetCursorPos(&mut at);
        // 앞에 내지 않으면 메뉴 밖을 눌러도 메뉴가 닫히지 않는다.
        SetForegroundWindow(window);
        let chosen = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON,
            at.x,
            at.y,
            0,
            window,
            std::ptr::null(),
        );
        PostMessageW(window, WM_NULL, 0, 0);
        DestroyMenu(menu);
        chosen as usize
    }
}

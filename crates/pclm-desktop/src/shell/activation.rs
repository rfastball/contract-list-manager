//! 떠 있는 작업자료 창을 앞으로 불러낸다(C# `Pclm.App/Activation.cs`, `Home.ActivationName`).
//!
//! 창은 홈마다 하나라(ADR-030) 두 번째로 띄우면 잠금을 얻지 못한다. 그쪽이 이름 붙은 신호를 울리고 조용히 끝나며
//! (울리기는 다리의 [`pclm_bridge::Os::signal_activation`] — `AllowSetForegroundWindow` 뒤에 울린다), 떠 있는 쪽이 이것으로
//! 그 신호를 듣고 창을 띄워 앞으로 낸다.

#![allow(unsafe_code)]

use std::thread::JoinHandle;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows_sys::Win32::System::Threading::{
    CreateEventW, INFINITE, SetEvent, WaitForMultipleObjects,
};

use super::win::wide;

/// 듣는 중. 놓으면(`Drop`) 듣기를 멈추고 손잡이를 닫는다.
pub struct Activation {
    signal: usize,
    stop: usize,
    thread: Option<JoinHandle<()>>,
}

impl Activation {
    /// 신호를 듣기 시작한다(자동으로 되돌아가는 이벤트, C# `EventResetMode.AutoReset`). `show` 는 듣는 실마리에서 불리므로
    /// 부르는 쪽이 창의 차례로 넘긴다. 듣지 못하게 되었으면 `None` — 그러면 두 번째로 띄운 쪽이 「이미 열려 있습니다」 를 알린다.
    pub fn listen(name: &str, show: impl Fn() + Send + 'static) -> Option<Activation> {
        let name = wide(name);
        // SAFETY: NUL 로 끝나는 이름. 실패하면 null.
        let signal = unsafe { CreateEventW(std::ptr::null(), 0, 0, name.as_ptr()) };
        if signal.is_null() {
            return None;
        }
        // SAFETY: 이름 없는 수동 이벤트 — 멈춤 신호.
        let stop = unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) };
        if stop.is_null() {
            // SAFETY: 방금 연 손잡이.
            unsafe { CloseHandle(signal) };
            return None;
        }
        let (signal, stop) = (signal as usize, stop as usize);
        let thread = std::thread::Builder::new()
            .name("pclm-activation".into())
            .spawn(move || {
                let handles = [signal as HANDLE, stop as HANDLE];
                loop {
                    // SAFETY: 둘 다 이 값이 사는 동안 열려 있다(Drop 이 멈춘 뒤 닫는다).
                    let woke = unsafe { WaitForMultipleObjects(2, handles.as_ptr(), 0, INFINITE) };
                    if woke != WAIT_OBJECT_0 {
                        return;
                    }
                    show();
                }
            })
            .ok()?;
        Some(Activation {
            signal,
            stop,
            thread: Some(thread),
        })
    }
}

impl Drop for Activation {
    fn drop(&mut self) {
        // SAFETY: 이 값이 쥔 손잡이 — 멈춤을 울리고, 실마리가 끝난 뒤 닫는다.
        unsafe { SetEvent(self.stop as HANDLE) };
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        unsafe {
            CloseHandle(self.signal as HANDLE);
            CloseHandle(self.stop as HANDLE);
        }
    }
}

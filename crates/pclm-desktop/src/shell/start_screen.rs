//! 작업자료를 열 수 없을 때의 시작 화면(C# `Pclm.App/StartWindow.cs`, ADR-030) — WPF 창 대신 작업 대화상자(TaskDialog).
//!
//! **여기서는 사람이 누르기 전에 아무것도 만들지 않는다.** 쪽지가 깨졌거나 가리키는 파일이 없을 때 빈 자료를 세우고 넘어가면,
//! 사람은 자료가 사라진 줄 알고 확장은 그 빈 자료에 쌓는다. 무엇이 왜 안 되는지 보이고, 있는 파일을 찾을지 새로 만들지를
//! 사람이 고른다. 빈 자료가 서는 길은 「새로 만들기」(`Home::create_new` → `PclmFile.Create`) 하나다.
//!
//! 글 · 단추 · 그 뒤 동작은 C# 과 같다: 「파일 찾기…」 · 「새로 만들기…」 · 「닫기」(Esc 도). 고른 파일을 받지 못하면 까닭을
//! 알림 상자로 보이고 이 화면에 머문다. 받았으면 닫고 부른 쪽이 처음부터 다시 본다(쪽지가 바뀌었다) — 여기서 곧장 여는 길을
//! 따로 두면 시작 흐름이 두 벌이 된다.
//!
//! C# 과 다른 것: 경로가 글 상자가 아니라 본문의 한 줄이다(골라 복사하는 대신 Ctrl+C 가 대화상자의 글 전부를 복사한다).

#![allow(unsafe_code)]

use std::path::Path;

use pclm_core::Error;
use pclm_core::storage::store::error_message;
use pclm_home::Home;
use windows_sys::Win32::Foundation::{HWND, LPARAM, S_FALSE, S_OK, WPARAM};
use windows_sys::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows_sys::Win32::UI::Controls::{
    TASKDIALOG_BUTTON, TASKDIALOGCONFIG, TDF_ALLOW_DIALOG_CANCELLATION, TDN_BUTTON_CLICKED,
};
use windows_sys::core::HRESULT;

use super::dialogs::{self, Pick};
use super::win::{CAPTION, Icon, message_box, wide};

const FILTER: &str = "계약 목록 자료 (*.pclm)|*.pclm";
const FIND: i32 = 101;
const CREATE: i32 = 102;
const CLOSE: i32 = 103;

/// 시작 화면이 어떻게 끝났나.
pub enum Outcome {
    /// 파일을 찾았거나 새로 만들었다(C# `DialogResult = true`) — 부른 쪽이 처음부터 다시 본다.
    Chosen,
    /// 닫았다 — 앱을 내린다.
    Closed,
    /// 뜻밖의 오류(C# 에서 잡지 않던 것). 부른 쪽이 오류 기록에 남긴다.
    Failed(String),
}

struct Context<'a> {
    home: &'a Home,
    failed: Option<String>,
}

type TaskDialogIndirect = unsafe extern "system" fn(
    *const TASKDIALOGCONFIG,
    *mut i32,
    *mut i32,
    *mut windows_sys::core::BOOL,
) -> HRESULT;

/// 시작 화면을 띄운다. `message` · `path` 는 `HomeState::Problem` 의 것.
pub fn show(home: &Home, message: &str, path: Option<&Path>) -> Outcome {
    // SAFETY: 창 실마리의 COM — 고르기 창이 이 실마리에서 뜬다. 끝에 닫는다(창(Tauri)은 아직 서지 않았다).
    let initialized =
        unsafe { CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32) } >= 0;
    let outcome = run(home, message, path);
    if initialized {
        // SAFETY: 위에서 연 것.
        unsafe { CoUninitialize() };
    }
    outcome
}

fn run(home: &Home, message: &str, path: Option<&Path>) -> Outcome {
    // 공용 컨트롤 6(exe 의 매니페스트)에만 있다 — 이름으로 찾는다. 없으면(매니페스트 없는 시험 빌드) 띄울 수 없다.
    let library = wide("comctl32.dll");
    // SAFETY: NUL 로 끝나는 이름. 찾은 함수의 꼴은 SDK 의 TaskDialogIndirect 다.
    let indirect: Option<TaskDialogIndirect> = unsafe {
        let module = LoadLibraryW(library.as_ptr());
        if module.is_null() {
            None
        } else {
            GetProcAddress(module, c"TaskDialogIndirect".as_ptr().cast())
                .map(|f| std::mem::transmute(f))
        }
    };
    let Some(indirect) = indirect else {
        return Outcome::Failed("시작 화면(작업 대화상자)을 띄우지 못했습니다.".into());
    };

    let mut content = message.to_string();
    if let Some(path) = path {
        content.push_str("\n\n");
        content.push_str(&path.display().to_string());
    }
    let title = wide(CAPTION);
    let instruction = wide("작업자료를 열지 못했습니다");
    let content = wide(&content);
    let footer = wide(
        "다른 자료를 대신 열거나 빈 자료를 만들지 않았습니다. 있는 작업자료를 찾거나 새로 만드세요.",
    );
    let labels = [wide("파일 찾기…"), wide("새로 만들기…"), wide("닫기")];
    let buttons = [
        TASKDIALOG_BUTTON {
            nButtonID: FIND,
            pszButtonText: labels[0].as_ptr(),
        },
        TASKDIALOG_BUTTON {
            nButtonID: CREATE,
            pszButtonText: labels[1].as_ptr(),
        },
        TASKDIALOG_BUTTON {
            nButtonID: CLOSE,
            pszButtonText: labels[2].as_ptr(),
        },
    ];
    let mut context = Context { home, failed: None };
    let config = TASKDIALOGCONFIG {
        cbSize: std::mem::size_of::<TASKDIALOGCONFIG>() as u32,
        dwFlags: TDF_ALLOW_DIALOG_CANCELLATION,
        pszWindowTitle: title.as_ptr(),
        pszMainInstruction: instruction.as_ptr(),
        pszContent: content.as_ptr(),
        cButtons: buttons.len() as u32,
        pButtons: buttons.as_ptr(),
        nDefaultButton: FIND,
        pszFooter: footer.as_ptr(),
        pfCallback: Some(callback),
        lpCallbackData: &mut context as *mut Context as isize,
        ..Default::default()
    };
    let mut pressed = 0i32;
    // SAFETY: 모든 글 · 단추 · 문맥은 이 함수가 끝날 때까지 산다.
    let shown = unsafe {
        indirect(
            &config,
            &mut pressed,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if let Some(failed) = context.failed {
        return Outcome::Failed(failed);
    }
    if shown < 0 {
        return Outcome::Failed(format!(
            "시작 화면(작업 대화상자)을 띄우지 못했습니다(0x{:08X}).",
            shown as u32
        ));
    }
    match pressed {
        FIND | CREATE => Outcome::Chosen,
        _ => Outcome::Closed,
    }
}

unsafe extern "system" fn callback(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    _: LPARAM,
    data: isize,
) -> HRESULT {
    if message != TDN_BUTTON_CLICKED as u32 {
        return S_OK;
    }
    // SAFETY: run 이 넘긴 문맥 — 대화상자가 떠 있는 동안 산다.
    let context = unsafe { &mut *(data as *mut Context) };
    let owner = Some(window as isize);
    let tried = match wparam as i32 {
        FIND => {
            let pick = Pick::Open {
                title: "작업자료 찾기".into(),
                filter: FILTER.into(),
                initial: None,
            };
            let Some(file) = dialogs::show(owner, &pick) else {
                return S_FALSE;
            };
            context.home.adopt(Path::new(&file))
        }
        CREATE => {
            // 덮는 것은 우리가 거절한다. 대화상자의 「바꾸시겠습니까?」 에 예를 눌러도 덮이지 않으니 묻지 않는다.
            let pick = Pick::Save {
                title: "새 작업자료 만들기".into(),
                filter: FILTER.into(),
                default_ext: ".pclm".into(),
                file_name: pclm_home::DEFAULT_WORKFILE_NAME.into(),
                overwrite_prompt: false,
                initial: documents(),
            };
            let Some(file) = dialogs::show(owner, &pick) else {
                return S_FALSE;
            };
            context.home.create_new(Path::new(&file)).map(|_| ())
        }
        _ => return S_OK, // 닫기 · Esc
    };
    match tried {
        Ok(()) => S_OK,
        Err(error @ (Error::InvalidOperation(_) | Error::Io(_) | Error::PclmFile(_))) => {
            message_box(Some(window), &error_message(&error), Icon::Warning);
            S_FALSE
        }
        Err(error) => {
            context.failed = Some(error_message(&error));
            S_OK
        }
    }
}

/// `Environment.GetFolderPath(SpecialFolder.MyDocuments)` — 열기 · 저장 고르기 창이 처음 여는 자리.
pub fn documents() -> Option<String> {
    use windows_sys::Win32::System::Com::CoTaskMemFree;
    use windows_sys::Win32::UI::Shell::{FOLDERID_Documents, SHGetKnownFolderPath};
    let mut path: windows_sys::core::PWSTR = std::ptr::null_mut();
    // SAFETY: 받은 글은 CoTaskMemFree 로 놓는다.
    unsafe {
        if SHGetKnownFolderPath(&FOLDERID_Documents, 0, std::ptr::null_mut(), &mut path) < 0
            || path.is_null()
        {
            return None;
        }
        let length = (0..).take_while(|&i| *path.add(i) != 0).count();
        let text = String::from_utf16_lossy(std::slice::from_raw_parts(path, length));
        CoTaskMemFree(path as *const _);
        Some(text)
    }
}

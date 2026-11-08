//! 고르기 창 — WPF 의 `OpenFileDialog` · `SaveFileDialog` · `OpenFolderDialog` 와 같은 공용 항목 대화상자(IFileDialog).
//!
//! 무엇을 고르라는지(제목 · 거르개 · 지어 둔 이름 · 덮어쓰기 묻기)는 다리가 정해 건넨다(`pclm_bridge::host`). 처음 자리는 C# 창의
//! 규칙 그대로 — 열기 · 저장은 문서 폴더(계획 엑셀은 정하지 않는다), 폴더는 지금 가리키는 폴더(없으면 정하지 않는다).
//!
//! 다리는 화면 실마리를 막지 않으려고 요청을 따로 실마리에서 푼다. 고르기 창은 그 실마리를 막고 **제 STA 실마리**에서 뜨며,
//! 주인은 본 창이다 — 주인 없는 대화창은 본 창에 모달이 아니라 뒤로 숨을 수 있다(C# `OnUiThread` 가 막던 것). 본 창의
//! 실마리는 그동안 메시지를 돌리고 있어 모달이 걸린다.

#![allow(unsafe_code)]

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoCreateInstance,
    CoInitializeEx, CoTaskMemFree, CoUninitialize,
};
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{
    FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM, FOS_NOCHANGEDIR, FOS_OVERWRITEPROMPT,
    FOS_PATHMUSTEXIST, FOS_PICKFOLDERS, FileOpenDialog, FileSaveDialog, IFileDialog,
    IFileOpenDialog, IFileSaveDialog, IShellItem, SHCreateItemFromParsingName, SIGDN_FILESYSPATH,
};
use windows::core::{HSTRING, Interface, PCWSTR};

/// 무엇을 고르나.
#[derive(Clone, Debug)]
pub enum Pick {
    /// `OpenFolderDialog { Title, InitialDirectory }`.
    Folder {
        title: String,
        initial: Option<String>,
    },
    /// `OpenFileDialog { Title, Filter, Multiselect = false, InitialDirectory }`.
    Open {
        title: String,
        filter: String,
        initial: Option<String>,
    },
    /// `SaveFileDialog { Title, Filter, FileName, DefaultExt, AddExtension = true, OverwritePrompt, InitialDirectory }`.
    Save {
        title: String,
        filter: String,
        default_ext: String,
        file_name: String,
        overwrite_prompt: bool,
        initial: Option<String>,
    },
}

/// Win32 꼴 거르개(`이름|*.확장자|…`)를 짝으로.
pub fn filter_pairs(filter: &str) -> Vec<(String, String)> {
    let parts: Vec<&str> = filter.split('|').collect();
    parts
        .chunks(2)
        .filter(|c| c.len() == 2)
        .map(|c| (c[0].to_string(), c[1].to_string()))
        .collect()
}

/// 제 STA 실마리에서 띄우고 고른 자리를 기다린다. 그만두었거나 띄우지 못했으면 `None`.
pub fn show_on_own_thread(owner: Option<isize>, pick: Pick) -> Option<String> {
    std::thread::Builder::new()
        .name("pclm-file-dialog".into())
        .spawn(move || {
            // SAFETY: 이 실마리의 COM 을 열고 끝에 닫는다.
            let initialized =
                unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) }
                    .is_ok();
            let picked = show(owner, &pick);
            if initialized {
                // SAFETY: 위에서 연 것을 닫는다.
                unsafe { CoUninitialize() };
            }
            picked
        })
        .ok()?
        .join()
        .ok()
        .flatten()
}

/// 이 실마리에서 띄운다 — COM(STA)이 이미 열린 실마리여야 한다(시작 화면은 창의 실마리에서 부른다).
pub fn show(owner: Option<isize>, pick: &Pick) -> Option<String> {
    // SAFETY: COM 호출. 실패는 「고르지 않음」 으로 접는다.
    unsafe { show_inner(owner, pick).ok().flatten() }
}

unsafe fn show_inner(owner: Option<isize>, pick: &Pick) -> windows::core::Result<Option<String>> {
    unsafe {
        let dialog: IFileDialog = match pick {
            Pick::Save { .. } => {
                CoCreateInstance::<_, IFileSaveDialog>(&FileSaveDialog, None, CLSCTX_INPROC_SERVER)?
                    .cast()?
            }
            _ => {
                CoCreateInstance::<_, IFileOpenDialog>(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?
                    .cast()?
            }
        };
        let mut options =
            dialog.GetOptions()? | FOS_FORCEFILESYSTEM | FOS_NOCHANGEDIR | FOS_PATHMUSTEXIST;
        let (title, filter, initial) = match pick {
            Pick::Folder { title, initial } => {
                options |= FOS_PICKFOLDERS;
                (
                    title,
                    None,
                    initial
                        .as_deref()
                        .filter(|p| std::path::Path::new(p).is_dir()),
                )
            }
            Pick::Open {
                title,
                filter,
                initial,
            } => {
                options |= FOS_FILEMUSTEXIST;
                (title, Some(filter), initial.as_deref())
            }
            Pick::Save {
                title,
                filter,
                default_ext,
                file_name,
                overwrite_prompt,
                initial,
            } => {
                options = if *overwrite_prompt {
                    options | FOS_OVERWRITEPROMPT
                } else {
                    options & !FOS_OVERWRITEPROMPT
                };
                if !file_name.is_empty() {
                    dialog.SetFileName(&HSTRING::from(file_name.as_str()))?;
                }
                let extension = default_ext.trim_start_matches('.');
                if !extension.is_empty() {
                    dialog.SetDefaultExtension(&HSTRING::from(extension))?;
                }
                (title, Some(filter), initial.as_deref())
            }
        };
        dialog.SetOptions(options)?;
        dialog.SetTitle(&HSTRING::from(title.as_str()))?;
        // 거르개 글은 Show 가 끝날 때까지 살아 있어야 한다.
        let pairs: Vec<(HSTRING, HSTRING)> = filter
            .map(|f| {
                filter_pairs(f)
                    .into_iter()
                    .map(|(n, s)| (HSTRING::from(n), HSTRING::from(s)))
                    .collect()
            })
            .unwrap_or_default();
        if !pairs.is_empty() {
            let specs: Vec<COMDLG_FILTERSPEC> = pairs
                .iter()
                .map(|(n, s)| COMDLG_FILTERSPEC {
                    pszName: PCWSTR(n.as_ptr()),
                    pszSpec: PCWSTR(s.as_ptr()),
                })
                .collect();
            dialog.SetFileTypes(&specs)?;
        }
        if let Some(folder) = initial.filter(|p| !p.is_empty())
            && let Ok(item) =
                SHCreateItemFromParsingName::<_, _, IShellItem>(&HSTRING::from(folder), None)
        {
            dialog.SetFolder(&item)?;
        }
        if dialog.Show(owner.map(|h| HWND(h as *mut _))).is_err() {
            return Ok(None); // 그만두었다(ERROR_CANCELLED) — 띄우지 못한 것도 고르지 않은 것으로 접는다.
        }
        let item = dialog.GetResult()?;
        let name = item.GetDisplayName(SIGDN_FILESYSPATH)?;
        let text = name.to_string().ok();
        CoTaskMemFree(Some(name.0 as *const _));
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::filter_pairs;

    #[test]
    fn 거르개는_이름과_무늬의_짝이다() {
        assert_eq!(
            filter_pairs("계약 목록 자료 (*.pclm)|*.pclm"),
            [("계약 목록 자료 (*.pclm)".to_string(), "*.pclm".to_string())]
        );
        assert_eq!(filter_pairs("A|*.a|B|*.b").len(), 2);
        assert!(filter_pairs("").is_empty());
    }
}

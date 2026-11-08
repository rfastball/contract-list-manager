//! 창의 일 — 이 창이 무엇을 여는가 · 다른 자료 열어 보기 · 판과 고지 · 창의 몸가짐(알림 영역 · 자동 실행) · 확장 설정 화면
//! (C# `Bridge` 의 `Session` · `OpenOther` · `About` · `ReadWindowPrefs` · `SaveWindowPrefs` 와 `ExtensionSetup.cs`).
//!
//! 운영체제에 하는 일은 모두 [`Os`] 를 거친다. 무엇을 언제 하는지(업무 홈이 아니면 등록부를 고치지 않는다)는 여기서 정한다.

use std::path::{Path, PathBuf};

use pclm_core::storage::pclm_file::PclmRole;
use pclm_home::dotnet;
use pclm_home::window_prefs::WindowPrefs;
use pclm_host::HOST_NAME;
use serde_json::Value;

use crate::bundled::{self, Prepared};
use crate::os::{Os, RegistryRoot, RegistryValue, SpecialFolder};
use crate::{Bridge, autostart, core, object, path_text, text_or_null};

impl Bridge<'_> {
    /// C# `Session` — 이 창이 무엇을 열고 있는가. 화면은 이것으로 띠를 띄우고 편집 칸을 잠근다(ADR-031 넷째 겹).
    pub(crate) fn session(&self) -> Value {
        let role = self
            .viewing
            .as_ref()
            .map_or(PclmRole::WORK, |v| v.role())
            .to_string();
        object([
            ("readOnly", Value::Bool(self.viewing.is_some())),
            ("role", Value::String(role.clone())),
            ("roleName", Value::String(PclmRole::name(Some(&role)))),
            ("path", Value::String(path_text(&self.shown_path()))),
        ])
    }

    /// C# `OpenOther` — 다른 자료를 **새 창(새 프로세스)**으로 열어 본다(ADR-031). 홈과 화면의 자리를 함께 넘긴다.
    pub(crate) fn open_other(&self) -> Result<Value, String> {
        let prompt = crate::OpenPrompt {
            title: "열어 볼 자료 고르기".into(),
            filter: "계약 목록 자료 (*.pclm)|*.pclm".into(),
        };
        let Some(picked) = self.host.pick_open_file(&prompt).filter(|p| !p.is_empty()) else {
            return Ok(Value::Null);
        };
        let full = dotnet::full_path(&picked).map_err(|e| core(e.into()))?;
        let full_text = path_text(&full);

        // 내 작업자료는 이미 이 창이 쓰고 있다. 열람 창을 띄우면 같은 파일을 두 창이 보며 한쪽은 낡는다.
        if self.viewing.is_none()
            && dotnet::eq_ignore_case(&full_text, &path_text(self.database.path()))
        {
            return Err(format!(
                "지금 이 창에서 쓰고 있는 작업자료입니다: {full_text}"
            ));
        }
        // 열람 창에서 내 작업자료를 골랐으면 새 창이 서지 않는다 — 본 창을 부른다.
        if self.viewing.is_some() && self.home.is_active_workfile(&full) {
            return Err(if self.os.signal_activation(&self.home.activation_name()) {
                format!("지금 작업자료입니다 — 본 창을 앞으로 불렀습니다. 본 창에서 보세요: {full_text}")
            } else {
                format!("지금 작업자료입니다 — 본 창에서 보세요: {full_text}")
            });
        }

        let exe = self
            .os
            .process_path()
            .ok_or("이 프로그램의 자리를 알 수 없습니다.")?;
        let mut args = vec!["--view".to_string(), full_text.clone()];
        if !self.home.is_default() {
            args.push("--home".into());
            args.push(path_text(self.home.directory()));
        }
        if let Some(origin) = &self.ui_origin {
            args.push("--ui".into());
            args.push(origin.clone());
        }
        self.os
            .start_process(&exe, &args, false)
            .map_err(|e| core(e.into()))?;
        Ok(object([("path", Value::String(full_text))]))
    }

    /// C# `ReadWindowPrefs` — 알림 영역 두기는 홈의 `window.json` 에, 자동 실행은 등록부에 있다.
    pub(crate) fn read_window_prefs(&self) -> Value {
        let unavailable = autostart::unavailable(self.os, self.is_default_home);
        let copy_target = if unavailable.is_none() {
            autostart::copy_target(self.os).map(|p| path_text(&p))
        } else {
            None
        };
        object([
            (
                "closeToTray",
                Value::Bool(WindowPrefs::read(&self.home.window_prefs_path()).close_to_tray),
            ),
            ("autostart", Value::Bool(autostart::is_enabled(self.os))),
            ("autostartAvailable", Value::Bool(unavailable.is_none())),
            ("autostartReason", text_or_null(unavailable)),
            ("autostartCopyTarget", text_or_null(copy_target)),
        ])
    }

    /// C# `SaveWindowPrefs` — 적고 곧바로 세운다. 자동 실행을 걸 수 없는 자리에서 바꾸려 하면 **아무것도 적기 전에** 거절한다.
    pub(crate) fn save_window_prefs(
        &self,
        close_to_tray: bool,
        autostart: bool,
    ) -> Result<Value, String> {
        let change = autostart != autostart::is_enabled(self.os);
        if change && let Some(reason) = autostart::unavailable(self.os, self.is_default_home) {
            return Err(reason);
        }
        let path = self.home.window_prefs_path();
        WindowPrefs {
            close_to_tray,
            ..WindowPrefs::read(&path)
        }
        .write(&path)
        .map_err(|e| core(e.into()))?;
        self.host.apply_close_to_tray(close_to_tray);
        if change {
            autostart::set(self.os, autostart)?;
        }
        Ok(self.read_window_prefs())
    }
}

/// C# `About` — 판과 라이선스. 배포물은 exe 하나라 고지를 안에 박아 둔다.
pub(crate) fn about() -> Value {
    object([
        ("version", Value::String(env!("CARGO_PKG_VERSION").into())),
        ("license", Value::String(strip_bom(LICENSE).into())),
        ("notices", Value::String(strip_bom(NOTICES).into())),
    ])
}

const LICENSE: &str = include_str!("../../../LICENSE");
const NOTICES: &str = include_str!("../../../THIRD-PARTY-NOTICES.txt");

/// `StreamReader(stream, Encoding.UTF8)` 는 머리표를 먹는다.
fn strip_bom(text: &str) -> &str {
    text.strip_prefix('\u{feff}').unwrap_or(text)
}

/// C# `ExtensionSetup.Prepare` — 확장을 업무 홈에 꺼내고, 두 브라우저의 HKCU 등록부에 호스트 쪽지의 자리를 적는다.
pub(crate) fn prepare_extension(os: &dyn Os) -> Result<Prepared, String> {
    let home = os.default_home().map_err(|e| core(e.into()))?;
    let exe = os
        .process_path()
        .ok_or("Value cannot be null. (Parameter 'path')")?;
    let prepared = bundled::prepare(&home, &exe)?;
    for browser in [r"Google\Chrome", r"Microsoft\Edge"] {
        os.registry_set_string(
            &format!(r"Software\{browser}\NativeMessagingHosts\{HOST_NAME}"),
            "",
            &path_text(&prepared.manifest_path),
        )
        .map_err(|e| core(e.into()))?;
    }
    Ok(prepared)
}

/// 창이 시작할 때 부른다(C# `App` 의 `ExtensionSetup.Refresh` · `Autostart.Refresh`) — **업무 홈일 때만.** 이미 꺼내 둔 내장
/// 확장을 이 판으로 고치고, 걸려 있는 자동 실행이 이 exe 를 가리키게 한다. 창을 막을 일이 아니라 실패는 삼킨다 — 고치지
/// 못했으면 설정 패널의 확장 상태가 옛 판을 드러낸다.
pub fn refresh_installation(os: &dyn Os, is_default_home: bool) {
    if !is_default_home {
        return;
    }
    if os
        .default_home()
        .is_ok_and(|home| home.join("extension").is_dir())
    {
        let _ = prepare_extension(os);
    }
    autostart::refresh(os);
}

/// 브라우저의 프로세스 이름.
pub(crate) fn process_name(target: &str) -> Result<&'static str, String> {
    match target {
        "edge" => Ok("msedge"),
        "chrome" => Ok("chrome"),
        _ => Err("Edge 또는 Chrome을 선택하세요.".into()),
    }
}

/// C# `ExtensionSetup.Open` — 확장 폴더를 탐색기로, 또는 브라우저의 확장 관리 화면을 새 창으로. 브라우저는 명령줄로 받은
/// `chrome://`·`edge://` 주소를 막고 새 탭만 열므로, 새 창을 띄우고 그 창의 주소창에 적어 넣는다.
pub(crate) fn open_extension_setup(os: &dyn Os, target: &str) -> Result<Value, String> {
    let io = |e: std::io::Error| core(e.into());
    if target == "folder" {
        let folder = os.default_home().map_err(io)?.join("extension");
        if !folder.is_dir() {
            return Err("먼저 내장 확장 준비를 누르세요.".into());
        }
        os.shell_open(&folder).map_err(io)?;
        return Ok(Value::Null);
    }
    let (exe, scheme, relative) = match target {
        "edge" => ("msedge.exe", "edge", r"Microsoft\Edge\Application\msedge.exe"),
        "chrome" => ("chrome.exe", "chrome", r"Google\Chrome\Application\chrome.exe"),
        _ => return Err("Edge 또는 Chrome을 선택하세요.".into()),
    };
    let url = format!("{scheme}://extensions/");
    let mut candidates: Vec<PathBuf> = Vec::new();
    for root in [RegistryRoot::CurrentUser, RegistryRoot::LocalMachine] {
        if let Ok(Some(RegistryValue::String(path))) = os.registry_get(
            root,
            &format!(r"Software\Microsoft\Windows\CurrentVersion\App Paths\{exe}"),
            "",
        ) {
            candidates.push(PathBuf::from(path.trim_matches('"')));
        }
    }
    for folder in [
        SpecialFolder::LocalApplicationData,
        SpecialFolder::ProgramFiles,
        SpecialFolder::ProgramFilesX86,
    ] {
        let base = os.special_folder(folder);
        candidates.push(if base.as_os_str().is_empty() {
            PathBuf::from(relative)
        } else {
            base.join(relative)
        });
    }
    let Some(path) = candidates.into_iter().find(|p| Path::new(p).is_file()) else {
        return Err(format!(
            "{}를 찾지 못했습니다. 브라우저를 직접 열고 주소창에 {url}를 입력하세요.",
            if target == "edge" { "Edge" } else { "Chrome" }
        ));
    };
    let process = process_name(target)?;
    let before = os.browser_windows(process);
    os.start_process(&path, &["--new-window".to_string()], true)
        .map_err(io)?;
    if !os.navigate_new_window(process, &before, &url) {
        os.set_clipboard(&url);
        return Err(format!(
            "브라우저 창에 주소를 넣지 못했습니다. {url} 을 복사해 두었으니 주소창에 붙여넣고 Enter 를 누르세요."
        ));
    }
    Ok(Value::Null)
}

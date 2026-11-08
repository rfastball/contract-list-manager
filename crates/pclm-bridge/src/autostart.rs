//! 자동 실행(C# `Pclm.App/Autostart.cs`) — 로그인할 때 알림 영역으로 켠다. 걸려 있는지는 등록부의 `Run` 값이 정본이다:
//! 사람이 작업 관리자에서 끈 것(`StartupApproved\Run` 의 첫 바이트가 홀수)도 꺼짐으로 읽는다. 따로 적어 두지 않는다 —
//! 두 벌이면 한쪽이 조용히 낡는다.
//!
//! 압축 안(임시 폴더)에서 켠 창이면 그 exe 는 곧 사라진다 — 켤 때 [`stable_path`] 에 복사해 두고 그것을 적는다.

use std::path::{Path, PathBuf};

use crate::os::{Os, RegistryRoot, RegistryValue, SpecialFolder};

/// 자동 실행으로 켤 때 붙이는 깃발 — 창을 띄우지 않고 알림 영역에만 둔다.
pub const BACKGROUND_FLAG: &str = "--background";

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const APPROVED_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
const VALUE_NAME: &str = "계약목록";

/// `%LOCALAPPDATA%\Programs\계약목록\계약목록.exe`.
pub fn stable_path(os: &dyn Os) -> PathBuf {
    os.special_folder(SpecialFolder::LocalApplicationData)
        .join("Programs")
        .join("계약목록")
        .join("계약목록.exe")
}

/// 자동 실행을 바꿀 수 없는 까닭. 바꿀 수 있으면 `None`.
pub fn unavailable(os: &dyn Os, is_default_home: bool) -> Option<String> {
    if !is_default_home {
        return Some(
            "시험 홈(--home)으로 띄운 창이라 자동 실행을 바꾸지 않습니다. 업무 홈의 창에서 바꾸세요.".into(),
        );
    }
    if os.process_path().is_none() {
        return Some("이 프로그램의 자리를 알 수 없어 자동 실행을 걸 수 없습니다.".into());
    }
    None
}

/// 켤 때 exe 를 복사해 둘 자리 — 압축 안에서 켠 창일 때만.
pub fn copy_target(os: &dyn Os) -> Option<PathBuf> {
    os.process_path()
        .filter(|exe| in_temp(os, exe))
        .map(|_| stable_path(os))
}

/// 걸려 있는가. 읽지 못하면 꺼짐이다.
pub fn is_enabled(os: &dyn Os) -> bool {
    matches!(
        os.registry_get(RegistryRoot::CurrentUser, RUN_KEY, VALUE_NAME),
        Ok(Some(RegistryValue::String(_)))
    ) && !turned_off_by_user(os)
}

/// 켜거나 끈다. 켤 때 압축 안이면 먼저 복사본을 둔다. 사람이 작업 관리자에서 끈 표시도 걷는다.
pub fn set(os: &dyn Os, on: bool) -> Result<(), String> {
    let io = |e: std::io::Error| crate::core(e.into());
    let mut exe = os
        .process_path()
        .ok_or("이 프로그램의 자리를 알 수 없어 자동 실행을 걸 수 없습니다.")?;
    if on && in_temp(os, &exe) {
        let stable = stable_path(os);
        install(&exe, &stable).map_err(|e| {
            format!(
                "자동 실행용 복사본을 {} 에 두지 못했습니다: {}",
                stable.display(),
                crate::core(e.into())
            )
        })?;
        exe = stable;
    }
    if on {
        os.registry_set_string(RUN_KEY, VALUE_NAME, &command(&exe))
            .map_err(io)?;
    } else {
        os.registry_delete_value(RUN_KEY, VALUE_NAME).map_err(io)?;
    }
    os.registry_delete_value(APPROVED_KEY, VALUE_NAME).map_err(io)
}

/// C# `Autostart.Refresh` — 업무 홈의 창을 켤 때. 걸려 있는데 적힌 자리가 이 exe 가 아니면 고쳐 적는다(판을 올리며
/// 자리를 옮겼다). 압축 안에서 켰으면 복사본을 새 판으로 바꾼다. 실패는 삼킨다.
pub fn refresh(os: &dyn Os) {
    let Some(exe) = os.process_path() else {
        return;
    };
    let Ok(Some(RegistryValue::String(current))) =
        os.registry_get(RegistryRoot::CurrentUser, RUN_KEY, VALUE_NAME)
    else {
        return;
    };
    if in_temp(os, &exe) {
        let stable = stable_path(os);
        if current == command(&stable) && differs(os, &exe, &stable) {
            let _ = install(&exe, &stable);
        }
        return;
    }
    let wanted = command(&exe);
    if current != wanted {
        let _ = os.registry_set_string(RUN_KEY, VALUE_NAME, &wanted);
    }
}

fn command(exe: &Path) -> String {
    format!("\"{}\" {BACKGROUND_FLAG}", exe.display())
}

fn differs(os: &dyn Os, source: &Path, target: &Path) -> bool {
    let length = |p: &Path| std::fs::metadata(p).map(|m| m.len()).ok();
    !target.is_file()
        || length(source) != length(target)
        || os.file_version(source) != os.file_version(target)
}

/// 옛 복사본을 걷고, 곁에 `.new` 로 복사한 뒤 제자리로. 켜져 있는 exe 라 바꾸지 못하면 비켜 두고 바꾼다.
fn install(source: &Path, target: &Path) -> std::io::Result<()> {
    let folder = target.parent().map(Path::to_path_buf).unwrap_or_default();
    std::fs::create_dir_all(&folder)?;
    if let Ok(entries) = std::fs::read_dir(&folder) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with("계약목록.old") && name.to_ascii_lowercase().ends_with(".exe") {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    let staged = PathBuf::from(format!("{}.new", target.display()));
    std::fs::copy(source, &staged)?;
    if let Err(e) = std::fs::rename(&staged, target) {
        if !target.is_file() {
            let _ = std::fs::remove_file(&staged);
            return Err(e);
        }
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let aside = folder.join(format!("계약목록.old-{stamp}.exe"));
        std::fs::rename(target, &aside)?;
        std::fs::rename(&staged, target)?;
        let _ = std::fs::remove_file(&aside);
    }
    Ok(())
}

fn turned_off_by_user(os: &dyn Os) -> bool {
    matches!(
        os.registry_get(RegistryRoot::CurrentUser, APPROVED_KEY, VALUE_NAME),
        Ok(Some(RegistryValue::Binary(mark))) if mark.first().is_some_and(|b| b & 1 == 1)
    )
}

fn in_temp(os: &dyn Os, exe: &Path) -> bool {
    let full = |p: &Path| {
        pclm_home::dotnet::full_path(p)
            .map(|f| crate::path_text(&f))
            .unwrap_or_default()
    };
    let exe = pclm_home::dotnet::to_upper_invariant(&full(exe));
    let temp = pclm_home::dotnet::to_upper_invariant(&full(&os.temp_path()));
    !temp.is_empty() && exe.starts_with(&temp)
}

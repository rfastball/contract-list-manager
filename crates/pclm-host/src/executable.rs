//! 실행 파일의 정체(C# `Erp/Executable.cs` 의 `ExecutableIdentity`, ADR-035) — 자리·크기·마지막 쓴 때. 상시 연결 호스트는
//! 브라우저가 켜져 있는 동안 exe 를 붙든다. 사람이 옛 exe 를 비키고 새 것을 놓으면 호스트는 제 자리가 더는 자기가 아님을
//! 보고 떠난다. 옛 exe 치우기(`OldExecutables`)는 창의 일이라 옮기지 않았다.

use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};

use pclm_home::dotnet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutableIdentity {
    /// `FileInfo.FullName`.
    pub path: String,
    pub length: u64,
    /// 마지막 쓴 때(FILETIME, UTC 100ns).
    pub last_write: u64,
}

impl ExecutableIdentity {
    /// 그 자리의 파일. 없으면(또는 자리가 비었으면) `None`.
    pub fn of(path: &Path) -> Option<ExecutableIdentity> {
        if path.as_os_str().is_empty() {
            return None;
        }
        let full = dotnet::full_path(path).ok()?;
        let meta = std::fs::metadata(&full).ok().filter(|m| m.is_file())?;
        Some(ExecutableIdentity {
            path: full.to_string_lossy().into_owned(),
            length: meta.len(),
            last_write: meta.last_write_time(),
        })
    }

    /// 이 프로세스를 띄운 파일(`Environment.ProcessPath`). Rust 호스트는 exe 하나다.
    pub fn of_this_process() -> Vec<ExecutableIdentity> {
        std::env::current_exe()
            .ok()
            .and_then(|p| ExecutableIdentity::of(&p))
            .into_iter()
            .collect()
    }

    /// 그 자리에 아직 같은 파일이 있는가. 비켜졌거나 다른 파일이 섰으면(크기·쓴 때가 다르다) 거짓.
    pub fn still_here(&self) -> bool {
        ExecutableIdentity::of(&PathBuf::from(&self.path)).as_ref() == Some(self)
    }
}

/// 브라우저의 호스트 등록 문서(`{"path": …}`)가 이 실행 파일을 가리키는가(C# `ExecutableIdentity.ManifestPointsAt`).
/// 읽지 못하는 문서는 가리킨다고 친다 — 확실하지 않을 때 연결을 끊지 않는다.
pub fn manifest_points_at(manifest_json: &str, process_path: &Path) -> bool {
    let Some(document) = pclm_core::erp::parse_json(manifest_json) else {
        return true;
    };
    // 뿌리가 객체가 아니면 C# 은 TryGetProperty 에서 던지고, 부르는 쪽(등록 확인)이 가리킨다고 친다.
    let Some(serde_json::Value::String(path)) = document.as_object().and_then(|o| o.get("path"))
    else {
        return true;
    };
    match (dotnet::full_path(path), dotnet::full_path(process_path)) {
        (Ok(a), Ok(b)) => dotnet::eq_ignore_case(&a.to_string_lossy(), &b.to_string_lossy()),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 실행_파일의_정체는_비켜지거나_다른_파일이_서면_달라진다() {
        let root = std::env::temp_dir().join(format!("pclm-exe-{}", pclm_home::win::new_guid_n()));
        std::fs::create_dir_all(&root).unwrap();
        let exe = root.join("계약목록.exe");
        std::fs::write(&exe, "옛 판").unwrap();
        let identity = ExecutableIdentity::of(&exe).unwrap();
        assert!(identity.still_here());
        assert!(ExecutableIdentity::of(&root.join("없음.exe")).is_none());
        assert!(ExecutableIdentity::of(Path::new("")).is_none());
        assert!(
            ExecutableIdentity::of(&root).is_none(),
            "폴더는 파일이 아니다"
        );

        let aside = root.join("계약목록.old-20261005091230.exe");
        std::fs::rename(&exe, &aside).unwrap();
        assert!(!identity.still_here());
        // 그 자리에 새 판이 섰다 — 크기가 같아도 쓴 때가 다르다.
        std::fs::write(&exe, "새 판").unwrap();
        let later = std::fs::metadata(&aside).unwrap().modified().unwrap()
            + std::time::Duration::from_secs(5);
        std::fs::File::options()
            .write(true)
            .open(&exe)
            .unwrap()
            .set_modified(later)
            .unwrap();
        assert!(!identity.still_here());
        // 같은 파일이 같은 때로 돌아오면 같다.
        std::fs::remove_file(&exe).unwrap();
        std::fs::rename(&aside, &exe).unwrap();
        assert!(identity.still_here());

        assert!(!ExecutableIdentity::of_this_process().is_empty());
        assert!(
            ExecutableIdentity::of_this_process()
                .iter()
                .all(ExecutableIdentity::still_here)
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn 등록_문서가_가리키는_exe_를_대소문자_없이_견준다() {
        let exe = std::env::temp_dir().join("옛 자리").join("계약목록.exe");
        let manifest = |path: &Path| {
            serde_json::json!({ "name": "kr.rfastball.pclm.erp", "path": path.to_string_lossy() })
                .to_string()
        };
        assert!(manifest_points_at(&manifest(&exe), &exe));
        let upper = PathBuf::from(dotnet::to_upper_invariant(&exe.to_string_lossy()));
        assert!(manifest_points_at(&manifest(&upper), &exe));
        assert!(!manifest_points_at(
            &manifest(&std::env::temp_dir().join("새 자리").join("계약목록.exe")),
            &exe
        ));
        // 읽지 못하는 문서는 가리킨다고 친다.
        assert!(manifest_points_at("{ 깨진", &exe));
        assert!(manifest_points_at("{}", &exe));
        assert!(manifest_points_at("[]", &exe));
        assert!(manifest_points_at(r#"{"path":""}"#, &exe));
    }
}

//! 비켜 둔 옛 실행 파일 치우기(C# `Erp/Executable.cs` 의 `OldExecutables`, ADR-035). 창이 켜질 때 부른다.
//!
//! 이름은 `<이름>.old.exe` 또는 `<이름>.old-<숫자>.exe` 하나로 정한다 — 스크립트는 `계약목록.old-yyyyMMddHHmmss.exe` 로 비키고,
//! 사람은 `계약목록.old.exe` 처럼 바꾸면 된다. 아직 쓰는 중이면(브라우저의 호스트가 쥐고 있다) 다음 번으로 미룬다.

use std::path::Path;

use pclm_home::dotnet;

/// 배포물의 이름. 사람이 exe 이름을 바꿔 써도 이 이름의 옛 것은 치운다.
pub const SHIPPED_STEM: &str = "계약목록";

/// 그 이름(`stem`)의 옛 exe 인가 — `\A{stem}\.old(-\d+)?\.exe\z`, 대소문자를 가리지 않는다. 다른 프로그램의 `*.old.exe` 는
/// 건드리지 않는다.
pub fn is_old(file_name: &str, stem: &str) -> bool {
    let name = dotnet::to_upper_invariant(file_name);
    let Some(rest) = name.strip_prefix(&dotnet::to_upper_invariant(stem)) else {
        return false;
    };
    let Some(rest) = rest.strip_prefix(".OLD") else {
        return false;
    };
    let Some(rest) = rest.strip_suffix(".EXE") else {
        return false;
    };
    rest.is_empty()
        || rest
            .strip_prefix('-')
            .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
}

/// 폴더에서 옛 exe 를 지운다. 지운 수. 지우지 못한 것(쓰는 중 · 권한)은 조용히 남긴다.
pub fn sweep(directory: &Path, stems: &[&str]) -> usize {
    let mut names: Vec<&str> = Vec::new();
    for stem in stems.iter().copied().filter(|s| !s.is_empty()) {
        if !names.iter().any(|n| dotnet::eq_ignore_case(n, stem)) {
            names.push(stem);
        }
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return 0;
    };
    let mut removed = 0;
    for entry in entries.filter_map(Result::ok) {
        if !entry.file_type().is_ok_and(|t| t.is_file()) {
            continue;
        }
        let file = entry.file_name().to_string_lossy().into_owned();
        if !names.iter().any(|stem| is_old(&file, stem)) {
            continue;
        }
        if std::fs::remove_file(entry.path()).is_ok() {
            removed += 1;
        }
    }
    removed
}

/// 창이 켜질 때(C# `SweepAround`): 이 exe 옆과 홈에서 이 exe 이름 · 배포물 이름의 옛 것을 치운다.
pub fn sweep_around(process_path: Option<&Path>, home_directory: &Path) {
    let stem = process_path
        .and_then(Path::file_stem)
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let stems = [SHIPPED_STEM, stem.as_str()];
    if let Some(beside) = process_path
        .and_then(Path::parent)
        .filter(|p| !p.as_os_str().is_empty())
    {
        sweep(beside, &stems);
    }
    sweep(home_directory, &stems);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 옛_이름만_옛것이다() {
        for name in [
            "계약목록.old.exe",
            "계약목록.OLD-20261108120000.EXE",
            "계약목록.old-1.exe",
        ] {
            assert!(is_old(name, SHIPPED_STEM), "{name}");
        }
        for name in [
            "계약목록.exe",
            "계약목록.old-.exe",
            "계약목록.old-1a.exe",
            "다른것.old.exe",
            "계약목록.old.exe.bak",
            "x계약목록.old.exe",
        ] {
            assert!(!is_old(name, SHIPPED_STEM), "{name}");
        }
    }

    #[test]
    fn 옆과_홈에서_그_이름의_옛것만_지운다() {
        let root = std::env::temp_dir().join(format!("pclm-desktop-old-{}", std::process::id()));
        let beside = root.join("배포");
        let home = root.join("홈");
        std::fs::create_dir_all(&beside).unwrap();
        std::fs::create_dir_all(&home).unwrap();
        for (folder, name) in [
            (&beside, "계약목록.exe"),
            (&beside, "계약목록.old-20261108.exe"),
            (&beside, "내이름.old.exe"),
            (&beside, "남의것.old.exe"),
            (&home, "계약목록.old.exe"),
        ] {
            std::fs::write(folder.join(name), b"x").unwrap();
        }
        sweep_around(Some(&beside.join("내이름.exe")), &home);
        let left = |folder: &Path| {
            let mut names: Vec<String> = std::fs::read_dir(folder)
                .unwrap()
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect();
            names.sort();
            names
        };
        assert_eq!(left(&beside), ["계약목록.exe", "남의것.old.exe"]);
        assert!(left(&home).is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }

    /// C# `ExtensionPresenceTests.비켜_둔_옛_exe_는_이름이_맞는_것만_치우고_쓰는_중인_것은_미룬다` — 브라우저의 호스트가 쥐고 있는
    /// 것은 다음 번으로 미루고, 다른 프로그램의 `*.old.exe` · 숫자가 아닌 꼬리 · 다른 확장자는 건드리지 않는다.
    #[cfg(windows)]
    #[test]
    fn 쓰는_중인_옛것은_미루고_세어_낸다() {
        use std::os::windows::fs::OpenOptionsExt;

        let root =
            std::env::temp_dir().join(format!("pclm-desktop-old-busy-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let beside = root.join("exe 옆");
        let home = root.join("홈");
        std::fs::create_dir_all(&beside).unwrap();
        std::fs::create_dir_all(&home).unwrap();
        let make = |folder: &Path, name: &str| {
            let path = folder.join(name);
            std::fs::write(&path, name).unwrap();
            path
        };
        let gone = [
            make(&beside, "계약목록.old-20261005091230.exe"),
            make(&beside, "계약목록.OLD.exe"),
            make(&beside, "내 계약목록.old-1.exe"), // 사람이 exe 이름을 바꿔 쓰고 있다
            make(&home, "계약목록.old.exe"),
        ];
        let busy = make(&home, "계약목록.old-2.exe");
        let current = make(&beside, "내 계약목록.exe");
        let others = [
            make(&beside, "설치.old.exe"),
            make(&beside, "계약목록.old-x.exe"),
            make(&beside, "계약목록.old.dll"),
            make(&beside, "계약목록.exe"),
        ];

        {
            let _held = std::fs::OpenOptions::new()
                .read(true)
                .share_mode(0)
                .open(&busy)
                .unwrap();
            sweep_around(Some(&current), &home);
        }
        for path in &gone {
            assert!(!path.exists(), "{}", path.display());
        }
        assert!(busy.exists()); // 쓰는 중 — 다음 번에.
        assert!(current.exists());
        for path in &others {
            assert!(path.exists(), "{}", path.display());
        }

        assert_eq!(sweep(&home, &[SHIPPED_STEM]), 1);
        assert_eq!(sweep(&root.join("없는 폴더"), &[SHIPPED_STEM]), 0);
        assert!(is_old("계약목록.old-20261005091230.exe", "계약목록"));
        assert!(!is_old("계약목록.exe", "계약목록"));
        std::fs::remove_dir_all(root).unwrap();
    }
}

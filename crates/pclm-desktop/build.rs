//! 창(`shell`)을 지을 때만 두 가지를 한다.
//!
//! 1. 화면(vite 산출물, `artifacts/web`)을 실행 파일에 박는다 — 폴더를 훑어 `include_bytes!` 목록을 짓는다. 배포물은
//!    exe 하나라 곁에 화면 파일을 남기지 않는다(C# 은 `EmbeddedResource` 로 같은 일을 했다). 폴더가 없으면 빈 목록이다 —
//!    창이 「화면이 exe 안에 들어 있지 않습니다」 를 띄운다(run.ps1 · publish.ps1 이 vite build 를 먼저 부른다).
//! 2. `tauri_build::build()` — 창 아이콘 · 매니페스트(공용 컨트롤 6) · 권한 묶음.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=tauri.conf.json");
    println!("cargo:rerun-if-changed=capabilities");
    if std::env::var_os("CARGO_FEATURE_SHELL").is_some() {
        embed_screen();
    }
    #[cfg(feature = "shell")]
    tauri_build::build();
}

fn embed_screen() {
    let manifest =
        PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let root = manifest
        .join("..")
        .join("..")
        .join("artifacts")
        .join("web");
    println!("cargo:rerun-if-changed={}", root.display());
    let mut files = Vec::new();
    if root.is_dir() {
        collect(&root, &root, &mut files);
    }
    files.sort();
    let mut out = String::from("&[\n");
    for (name, path) in &files {
        writeln!(
            out,
            "    ({name:?}, include_bytes!({:?}) as &[u8]),",
            path.to_string_lossy()
        )
        .unwrap();
    }
    out.push(']');
    let target = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR")).join("web_assets.rs");
    std::fs::write(target, out).unwrap();
}

/// 폴더 아래 파일 전부 — 이름은 `/` 로 이은 상대 경로(요청 경로와 같은 꼴).
fn collect(root: &Path, folder: &Path, files: &mut Vec<(String, PathBuf)>) {
    let entries = std::fs::read_dir(folder)
        .unwrap_or_else(|e| panic!("화면 폴더를 읽지 못했습니다 {}: {e}", folder.display()));
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            collect(root, &path, files);
        } else if path.is_file() {
            let relative = path.strip_prefix(root).unwrap();
            let name = relative
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            println!("cargo:rerun-if-changed={}", path.display());
            files.push((name, path));
        }
    }
}

//! 내장 확장을 실행 파일에 박는다. 저장소의 `extension/` 아래 파일을 하위 폴더까지 이름 차례로 골라 `include_bytes!` 목록을
//! 짓는다 — 이름은 `extension/` 에서 본 상대 경로이고 나눔표는 `/` 다(`panel/panel.js`).
//!
//! 싣는 확장자는 [`KINDS`] 뿐이다. 그 밖의 파일이 있으면 **빌드를 세운다** — 예전에는 네 확장자만 골라 나머지(엔진 `.wasm`,
//! 기본 서식)를 조용히 버렸고, 빌드는 초록인 채 사용자 PC 에 알맹이 빠진 확장이 풀렸다(ADR-045).

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// 확장에 싣는 파일의 확장자.
const KINDS: [&str; 7] = [".json", ".js", ".html", ".css", ".wasm", ".hwpx", ".txt"];

fn collect(folder: &Path, prefix: &str, out: &mut Vec<(String, PathBuf)>) {
    println!("cargo:rerun-if-changed={}", folder.display());
    let entries = std::fs::read_dir(folder)
        .unwrap_or_else(|e| panic!("내장 확장 폴더를 읽지 못했습니다 {}: {e}", folder.display()));
    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        let relative = format!("{prefix}{name}");
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() {
            collect(&path, &format!("{relative}/"), out);
        } else if kind.is_file() {
            let lower = name.to_ascii_lowercase();
            if !KINDS.iter().any(|x| lower.ends_with(x)) {
                panic!(
                    "내장 확장에 싣지 않는 파일이 있습니다: extension/{relative} — 싣는다면 build.rs 의 KINDS 에 더하고, 아니면 치우세요."
                );
            }
            out.push((relative, path));
        }
    }
}

fn main() {
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let folder = manifest.join("..").join("..").join("extension");
    let mut files = Vec::new();
    collect(&folder, "", &mut files);
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let mut out = String::from("&[\n");
    for (name, path) in &files {
        println!("cargo:rerun-if-changed={}", path.display());
        writeln!(
            out,
            "    ({name:?}, include_bytes!({:?}) as &[u8]),",
            path.to_string_lossy()
        )
        .unwrap();
    }
    out.push(']');
    let target = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR")).join("extension_files.rs");
    std::fs::write(target, out).unwrap();
}

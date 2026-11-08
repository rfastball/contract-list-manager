//! 내장 확장(C# `Pclm.Core.csproj` 의 `EmbeddedResource ../../extension/*.json;*.js;*.html;*.css`)을 실행 파일에 박는다.
//! 저장소의 `extension/` 에서 그 넷의 확장자를 가진 파일을 이름 차례로 골라 `include_bytes!` 목록을 짓는다.

use std::fmt::Write as _;
use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let folder = manifest.join("..").join("..").join("extension");
    println!("cargo:rerun-if-changed={}", folder.display());
    let mut names: Vec<String> = std::fs::read_dir(&folder)
        .unwrap_or_else(|e| panic!("내장 확장 폴더를 읽지 못했습니다 {}: {e}", folder.display()))
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| {
            let lower = n.to_ascii_lowercase();
            [".json", ".js", ".html", ".css"].iter().any(|x| lower.ends_with(x))
        })
        .collect();
    names.sort();
    let mut out = String::from("&[\n");
    for name in &names {
        let path = folder.join(name);
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

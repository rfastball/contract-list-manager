//! 판은 한 자리에서 정한다 — 작업공간 Cargo.toml 의 `[workspace.package] version` 이 정본이고(publish.ps1 이 묶음 이름에
//! 쓰고, 창 exe 의 파일 판이 된다), 크레이트는 모두 그것을 물려받는다.
//! 크레이트 하나가 제 판을 따로 적으면 배포 exe 와 묶음 이름이 조용히 갈라진다 — 여기서 붉어진다.

use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

#[test]
fn 크레이트는_모두_작업공간의_판을_물려받는다() {
    let mut own = Vec::new();
    for entry in std::fs::read_dir(root().join("crates"))
        .expect("crates")
        .flatten()
    {
        let manifest = entry.path().join("Cargo.toml");
        let Ok(text) = std::fs::read_to_string(&manifest) else {
            continue;
        };
        if !text.lines().any(|l| l.trim() == "version.workspace = true") {
            own.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    assert!(
        own.is_empty(),
        "제 판을 따로 적은 크레이트: {own:?} — version.workspace = true 로 물려받으세요."
    );
}

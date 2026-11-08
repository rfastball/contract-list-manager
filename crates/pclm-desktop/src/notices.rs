//! 배포 exe 에 실려 나가는 Rust 크레이트의 고지(THIRD-PARTY-NOTICES.txt 의 `== crate` 갈래).
//!
//! **무엇이 실리는가**는 손으로 적지 않고 `cargo tree` 에 묻는다 — `pclm-desktop` 을 `shell` 기능으로, windows 대상
//! (`x86_64-pc-windows-msvc`)에서, 운영 의존(`-e normal`)만, 짓는 동안에만 도는 proc-macro 와 그 의존은 빼고
//! (`no-proc-macro`). `cargo metadata` 는 작업공간 전체를 한 번에 풀어 다른 크레이트가 켠 기능까지 섞이지만, `cargo tree -p` 는
//! `cargo build -p` 와 같은 기능 해석이다. 저장소 안의 크레이트(경로 의존)는 고지 대상이 아니다.
//!
//! 고지를 짓는 것은 `examples/crate_notices.rs`(scripts/generate-crate-notices.ps1)이고, 대조하는 것은
//! `tests/notices.rs` 다 — 둘이 같은 목록을 이 모듈에서 얻는다.

use std::collections::BTreeSet;
use std::io;
use std::path::Path;
use std::process::Command;

/// 실리는 크레이트 하나 — 고지의 머리줄 `== crate <name> <version> — <license>` 와 같은 셋.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Crate {
    pub name: String,
    pub version: String,
    pub license: String,
}

/// 배포물을 짓는 대상(publish.ps1 이 이 기계의 기본으로 짓는다).
pub const TARGET: &str = "x86_64-pc-windows-msvc";

/// `cargo tree` 의 인자 — 짓기(`cargo build --release -p pclm-desktop --features shell`)와 같은 해석.
pub fn tree_args() -> Vec<&'static str> {
    vec![
        "tree",
        "--locked",
        "-p",
        "pclm-desktop",
        "--features",
        "shell",
        "-e",
        "normal,no-proc-macro",
        "--target",
        TARGET,
        "--prefix",
        "none",
        "-f",
        "{p}|{l}",
    ]
}

/// `cargo` 로 실리는 크레이트를 묻는다. `workspace` 는 작업공간 뿌리(또는 그 안 어디든).
pub fn shipped(cargo: &Path, workspace: &Path) -> io::Result<BTreeSet<Crate>> {
    let output = Command::new(cargo)
        .args(tree_args())
        .current_dir(workspace)
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "cargo tree 가 실패했습니다: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(parse_tree(&String::from_utf8_lossy(&output.stdout)))
}

/// `cargo tree --prefix none -f "{p}|{l}"` 의 줄들 → 레지스트리 크레이트. `{p}` 는 `이름 v판`, 경로 · git 의존이면 뒤에
/// ` (출처)` 가 붙고, 이미 보인 것은 줄 끝(라이선스 뒤)에 ` (*)` 가 붙는다.
pub fn parse_tree(text: &str) -> BTreeSet<Crate> {
    let mut crates = BTreeSet::new();
    for line in text.lines() {
        let line = line.trim().trim_end_matches(" (*)");
        let Some((package, license)) = line.split_once('|') else {
            continue;
        };
        let package = package.trim();
        let mut words = package.split(' ');
        let (Some(name), Some(version)) = (words.next(), words.next()) else {
            continue;
        };
        if words.next().is_some() {
            continue; // (경로) · (git) — 이 저장소의 것이거나 레지스트리 밖
        }
        let Some(version) = version.strip_prefix('v') else {
            continue;
        };
        crates.insert(Crate {
            name: name.to_owned(),
            version: version.to_owned(),
            license: license.trim().to_owned(),
        });
    }
    crates
}

/// 고지에 적힌 크레이트 — `== crate <name> <version> — <license>` 줄들.
pub fn written(notices: &str) -> BTreeSet<Crate> {
    notices
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("== crate ")?;
            let (head, license) = rest.split_once(" — ")?;
            let (name, version) = head.split_once(' ')?;
            Some(Crate {
                name: name.to_owned(),
                version: version.to_owned(),
                license: license.trim().to_owned(),
            })
        })
        .collect()
}

/// 라이선스 식의 한 갈래를 어떻게 채우는가.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Text {
    /// 문서 끝 `[MIT]` 전문
    Mit,
    /// 문서 끝 `[Apache-2.0]` 전문
    Apache,
    /// 그 밖의 라이선스 — 크레이트의 라이선스 파일을 그 항목에 싣는다
    Inline,
}

/// 라이선스 식(`MIT OR Apache-2.0`, `MIT/Apache-2.0`, `BSD-3-Clause AND MIT` …) → 함께 지켜야 하는 갈래마다 고를 전문.
/// 갈래 안에서 고를 수 있으면 MIT, 다음 Apache-2.0 을 고른다.
pub fn texts(license: &str) -> Vec<Text> {
    let expression = license.replace(['(', ')'], " ").replace('/', " OR ");
    expression
        .split(" AND ")
        .map(|conjunct| {
            let options: Vec<&str> = conjunct.split(" OR ").map(str::trim).collect();
            if options.contains(&"MIT") {
                Text::Mit
            } else if options.contains(&"Apache-2.0") {
                Text::Apache
            } else {
                Text::Inline
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 나무의_줄에서_레지스트리_크레이트만_가린다() {
        let text = "pclm-desktop v0.17.1 (C:\\x\\crates\\pclm-desktop)|MIT\n\
                    serde v1.0.228|MIT OR Apache-2.0\n\
                    serde v1.0.228|MIT OR Apache-2.0 (*)\n\
                    some-git v0.1.0 (https://example.invalid/x#abc)|MIT\n\
                    icu_provider v2.4.0|Unicode-3.0\n";
        let crates: Vec<_> = parse_tree(text)
            .into_iter()
            .map(|c| format!("{} {} {}", c.name, c.version, c.license))
            .collect();
        assert_eq!(
            crates,
            [
                "icu_provider 2.4.0 Unicode-3.0",
                "serde 1.0.228 MIT OR Apache-2.0"
            ]
        );
    }

    #[test]
    fn 고지의_머리줄을_읽는다() {
        let text = "== npm react 19.2.8 — MIT\n== crate brotli 9.1.1 — BSD-3-Clause AND MIT\n본문\n";
        let crates: Vec<_> = written(text).into_iter().collect();
        assert_eq!(
            crates,
            [Crate {
                name: "brotli".into(),
                version: "9.1.1".into(),
                license: "BSD-3-Clause AND MIT".into()
            }]
        );
    }

    #[test]
    fn 고를_수_있으면_끝의_전문을_고르고_아니면_싣는다() {
        assert_eq!(texts("MIT OR Apache-2.0"), [Text::Mit]);
        assert_eq!(texts("Apache-2.0 / MIT"), [Text::Mit]);
        assert_eq!(texts("Apache-2.0"), [Text::Apache]);
        assert_eq!(texts("CC0-1.0 OR MIT-0 OR Apache-2.0"), [Text::Apache]);
        assert_eq!(texts("BSD-3-Clause AND MIT"), [Text::Inline, Text::Mit]);
        assert_eq!(texts("Unicode-3.0"), [Text::Inline]);
        assert_eq!(texts("Unlicense/MIT"), [Text::Mit]);
    }
}

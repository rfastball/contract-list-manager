//! THIRD-PARTY-NOTICES.txt 의 「Rust 크레이트」 절을 다시 짓는다 — `scripts/generate-crate-notices.ps1` 이 부른다.
//!
//! ```text
//! cargo run -q -p pclm-desktop --example crate_notices -- <THIRD-PARTY-NOTICES.txt>
//! ```
//!
//! 실리는 목록은 [`pclm_desktop::notices::shipped`](`cargo tree`)에서, 크레이트의 자리 · 저자 · 저장소는 `cargo metadata` 에서
//! 얻는다. 저작권 줄은 크레이트에 든 라이선스 파일에서 옮기고, MIT · Apache-2.0 은 문서 끝의 전문을 가리키며, 그 밖의
//! 라이선스(BSD · Zlib · Unicode · MPL …)는 그 파일을 항목 안에 싣는다(같은 글이 앞에 실렸으면 그 항목을 가리킨다).
//!
//! 사람이 살펴야 할 자리에서는 지어내지 않고 멈춘다 — 라이선스 파일이 없는 크레이트, 아래 표가 아는 판에서 달라진
//! WebView2 · SQLite 바인딩.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use pclm_desktop::notices::{self, Crate, Text};
use serde_json::Value;

const RULE: &str = "------------------------------------------------------------------------------";
const BAR: &str = "==============================================================================";
const TITLE: &str = "Rust 크레이트";
/// 이 절 바로 뒤에 오는 절(없으면 그 앞에 새로 넣는다).
const NEXT: &str = "글꼴";

/// 패키지에 라이선스 파일이 없어 같은 상류 저장소의 다른 크레이트 파일을 따르는 것.
const BORROWED: &[(&str, &str, &str)] = &[(
    "alloc-stdlib",
    "alloc-no-stdlib",
    "https://github.com/dropbox/rust-alloc-no-stdlib",
)];

/// 패키지에 라이선스 파일이 없어 상류 저장소의 LICENSE 에서 저작권 줄을 옮긴 것(MIT 일 때만).
const UPSTREAM: &[(&str, &str, &str)] = &[
    (
        "webview2-com",
        "Copyright (c) 2021 Bill Avery",
        "https://github.com/wravery/webview2-rs",
    ),
    (
        "webview2-com-sys",
        "Copyright (c) 2021 Bill Avery",
        "https://github.com/wravery/webview2-rs",
    ),
];

/// webview2-com-sys 가 정적으로 링크하는 WebView2LoaderStatic.lib 의 출처 — 판이 바뀌면 SDK 판도 다시 살핀다.
const WEBVIEW2_SYS: (&str, &str) = ("0.39.1", "1.0.3800.47");

fn main() -> ExitCode {
    let Some(path) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("쓰는 법: crate_notices <THIRD-PARTY-NOTICES.txt>");
        return ExitCode::from(2);
    };
    match run(&path) {
        Ok(count) => {
            println!("크레이트 {count} 개 — {}", path.display());
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

struct Package {
    directory: PathBuf,
    authors: Vec<String>,
    repository: Option<String>,
}

fn run(path: &Path) -> Result<usize, String> {
    let cargo = PathBuf::from(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"));
    let crates = notices::shipped(&cargo, workspace).map_err(|e| e.to_string())?;
    let packages = metadata(&cargo, workspace)?;
    let section = section(&crates, &packages)?;

    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let text = text.replace("\r\n", "\n");
    let header = format!("{BAR}\n{TITLE}\n{BAR}\n");
    let next = format!("{BAR}\n{NEXT}\n{BAR}\n");
    let replaced = match text.find(&header) {
        Some(start) => {
            let after = start + header.len();
            let end = text[after..]
                .find(&format!("{BAR}\n"))
                .map(|at| after + at)
                .ok_or("「Rust 크레이트」 절 뒤에 다음 절이 없습니다.")?;
            format!("{}{section}{}", &text[..start], &text[end..])
        }
        None => {
            let at = text
                .find(&next)
                .ok_or_else(|| format!("「{NEXT}」 절을 찾지 못했습니다."))?;
            format!("{}{section}{}", &text[..at], &text[at..])
        }
    };
    std::fs::write(path, replaced).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(crates.len())
}

fn metadata(cargo: &Path, workspace: &Path) -> Result<BTreeMap<(String, String), Package>, String> {
    let output = Command::new(cargo)
        .args([
            "metadata",
            "--format-version",
            "1",
            "--locked",
            "--filter-platform",
            notices::TARGET,
            "--features",
            "pclm-desktop/shell",
        ])
        .current_dir(workspace)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata 가 실패했습니다: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let value: Value = serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
    let mut packages = BTreeMap::new();
    for package in value["packages"].as_array().into_iter().flatten() {
        let text = |key: &str| package[key].as_str().unwrap_or_default().to_owned();
        let manifest = PathBuf::from(text("manifest_path"));
        packages.insert(
            (text("name"), text("version")),
            Package {
                directory: manifest.parent().unwrap_or(Path::new(".")).to_owned(),
                authors: package["authors"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|a| a.as_str().map(str::to_owned))
                    .collect(),
                repository: package["repository"].as_str().map(str::to_owned),
            },
        );
    }
    Ok(packages)
}

/// 크레이트 폴더의 라이선스 파일(이름순).
fn license_files(directory: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(directory)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_ascii_uppercase())
                    .is_some_and(|n| {
                        [
                            "LICENSE",
                            "LICENCE",
                            "COPYING",
                            "NOTICE",
                            "COPYRIGHT",
                            "UNLICENSE",
                        ]
                        .iter()
                        .any(|p| n.starts_with(p))
                    })
        })
        .collect();
    files.sort();
    files
}

fn read(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let text = String::from_utf8_lossy(&bytes).replace("\r\n", "\n");
    Ok(text
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim_matches('\n')
        .to_owned())
}

/// 라이선스 파일의 저작권 줄 — 틀(`[yyyy] [name of copyright owner]`)과 조항 안의 낱말은 뺀다.
fn copyrights(texts: &[String]) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for text in texts {
        for line in text.lines() {
            let line = line.trim();
            let lower = line.to_lowercase();
            let starts = lower.starts_with("copyright") || line.starts_with('©');
            let template = [
                "[yyyy]",
                "[name of",
                "<year>",
                "{yyyy}",
                "notice and this permission",
                "holders and contributors",
                "copyright notice",
                "copyright owner",
                "copyright license",
                "copyright statement",
                "permission notice",
            ]
            .iter()
            .any(|t| lower.contains(t));
            if starts && !template && !lines.iter().any(|l| l == line) {
                lines.push(line.to_owned());
            }
        }
    }
    lines
}

fn is_standard_file(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_uppercase())
        .unwrap_or_default();
    name.contains("MIT") || name.contains("APACHE")
}

fn section(
    crates: &std::collections::BTreeSet<Crate>,
    packages: &BTreeMap<(String, String), Package>,
) -> Result<String, String> {
    let mut out = format!(
        "{BAR}\n{TITLE}\n{BAR}\n\n\
         배포 exe 에 실려 나가는 Rust 크레이트(운영 의존 · windows 대상 · 창 기능). scripts/generate-crate-notices.ps1 이\n\
         cargo tree 에서 짓는다 — 손으로 고치지 않는다(시험이 실제 해석과 대조한다).\n\n"
    );
    // 이미 실은 전문 → 그것을 실은 항목
    let mut printed: BTreeMap<String, String> = BTreeMap::new();
    for item in crates {
        let key = (item.name.clone(), item.version.clone());
        let package = packages.get(&key).ok_or_else(|| {
            format!(
                "cargo metadata 에 없는 크레이트: {} {}",
                item.name, item.version
            )
        })?;
        let identity = format!("{} {}", item.name, item.version);
        let mut notes: Vec<String> = Vec::new();

        let mut files = license_files(&package.directory);
        if files.is_empty()
            && let Some((_, from, source)) = BORROWED.iter().find(|(n, _, _)| *n == item.name)
        {
            let donor = packages
                .iter()
                .filter(|((n, _), _)| n == from)
                .find(|((n, v), _)| crates.iter().any(|c| &c.name == n && &c.version == v))
                .ok_or_else(|| {
                    format!("{identity}: 라이선스를 빌려 올 {from} 이 실리지 않습니다.")
                })?;
            files = license_files(&donor.1.directory);
            notes.push(format!(
                "패키지에 라이선스 파일이 없어 같은 상류 저장소({source})의 {} {} 의 파일을 따릅니다.",
                donor.0.0, donor.0.1
            ));
        }
        let texts: Vec<String> = files.iter().map(|f| read(f)).collect::<Result<_, _>>()?;
        let mut owners = copyrights(&texts);
        if files.is_empty() {
            let (_, line, source) = UPSTREAM
                .iter()
                .find(|(n, _, _)| *n == item.name)
                .ok_or_else(|| {
                    format!(
                        "{identity}: 패키지에 라이선스 파일이 없습니다 — 상류를 살펴 표에 더하세요."
                    )
                })?;
            if item.license != "MIT" {
                return Err(format!(
                    "{identity}: 라이선스가 {} 로 바뀌었습니다 — 상류를 다시 살피세요.",
                    item.license
                ));
            }
            owners.push((*line).to_owned());
            notes.push(format!(
                "패키지에 라이선스 파일이 없어 상류 저장소({source})의 LICENSE 에서 옮겼습니다."
            ));
        }
        if owners.is_empty() {
            owners.push(if package.authors.is_empty() {
                format!(
                    "저작권: {} 의 기여자",
                    package
                        .repository
                        .clone()
                        .unwrap_or_else(|| format!("https://crates.io/crates/{}", item.name))
                )
            } else {
                format!("저작자: {}", package.authors.join(", "))
            });
        }

        let _ = writeln!(
            out,
            "== crate {} {} — {}",
            item.name, item.version, item.license
        );
        for line in &owners {
            let _ = writeln!(out, "{line}");
        }
        for line in &notes {
            let _ = writeln!(out, "{line}");
        }
        extra(&mut out, item, package)?;

        let mut inline = false;
        for text in notices::texts(&item.license) {
            match text {
                Text::Mit => {
                    let _ = writeln!(out, "라이선스 전문: 문서 끝 [MIT] 참조");
                }
                Text::Apache => {
                    let _ = writeln!(out, "라이선스 전문: 문서 끝 [Apache-2.0] 참조");
                }
                Text::Inline => inline = true,
            }
        }
        if inline {
            if item.license.contains("MPL") {
                let source = package.repository.clone().unwrap_or_else(|| {
                    format!("https://crates.io/crates/{}/{}", item.name, item.version)
                });
                let _ = writeln!(
                    out,
                    "MPL-2.0 대상 소스는 고치지 않고 쓰며 {source} 에서 얻을 수 있습니다."
                );
            }
            let own: Vec<(&PathBuf, &String)> = files
                .iter()
                .zip(&texts)
                .filter(|(f, _)| files.len() == 1 || !is_standard_file(f))
                .collect();
            if own.is_empty() {
                return Err(format!(
                    "{identity}: {} 의 라이선스 파일을 찾지 못했습니다.",
                    item.license
                ));
            }
            for (_, text) in own {
                match printed.get(text) {
                    Some(first) => {
                        let _ = writeln!(out, "라이선스 전문: 위 {first} 항목의 전문과 같음");
                    }
                    None => {
                        let _ = writeln!(out, "라이선스 전문:\n{RULE}\n{text}\n{RULE}");
                        printed.insert(text.clone(), identity.clone());
                    }
                }
            }
        }
        out.push('\n');
    }
    Ok(out)
}

/// 크레이트가 안에 싣는 다른 것 — 정적으로 링크하는 WebView2 로더, 소스째 넣은 SQLite.
fn extra(out: &mut String, item: &Crate, package: &Package) -> Result<(), String> {
    match item.name.as_str() {
        "webview2-com-sys" => {
            let (version, sdk) = WEBVIEW2_SYS;
            if item.version != version {
                return Err(format!(
                    "webview2-com-sys 가 {} 입니다 — 정적으로 링크하는 WebView2LoaderStatic.lib 의 SDK 판(지금 {sdk})을 다시 살피세요.",
                    item.version
                ));
            }
            let license = read(
                &Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("notices/Microsoft-WebView2-SDK-LICENSE.txt"),
            )?;
            let _ = writeln!(
                out,
                "WebView2LoaderStatic.lib(Microsoft WebView2 SDK {sdk}, BSD-3-Clause 형식)를 정적으로 링크합니다. 그 전문:\n{RULE}\n{license}\n{RULE}"
            );
        }
        "libsqlite3-sys" => {
            let source = package.directory.join("sqlite3").join("sqlite3.c");
            let text = std::fs::read_to_string(&source)
                .map_err(|e| format!("{}: {e}", source.display()))?;
            let version = text
                .lines()
                .find_map(|l| l.strip_prefix("#define SQLITE_VERSION "))
                .map(|v| v.trim().trim_matches('"').to_owned())
                .ok_or("sqlite3.c 에서 SQLITE_VERSION 을 찾지 못했습니다.")?;
            let _ = writeln!(
                out,
                "SQLite {version} 소스(sqlite3/sqlite3.c)를 정적으로 포함합니다. SQLite 는 퍼블릭 도메인으로 공개되어 있습니다(https://sqlite.org/copyright.html)."
            );
        }
        _ => {}
    }
    Ok(())
}

//! 내장 확장 — 실행 파일에 박아 둔 `extension/` 을 홈에 꺼내고, 브라우저가 이
//! 실행 파일을 native messaging 호스트로 부를 수 있게 호스트 쪽지(`kr.rfastball.pclm.erp.json`)와 출처를 적는다.
//!
//! 앱을 켤 때마다 불린다 — 같은 바이트면 다시 쓰지 않는다(브라우저가 읽던 파일이 까닭 없이 바뀌지 않게). 쓸 때는 곁의 임시
//! 파일에 쓴 뒤 이름을 바꾸고, `manifest.json` 을 마지막에 바꾼다. 등록부(어느 브라우저가 이 쪽지를 보는가)는 여기가 아니라
//! 다리의 `prepareExtension` 이 적는다.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use pclm_core::storage::store::bridge_json;
use pclm_host::HOST_NAME;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::object;

/// 박아 둔 확장 파일 — `(이름, 바이트)`, 이름 차례.
pub const EMBEDDED_FILES: &[(&str, &[u8])] =
    include!(concat!(env!("OUT_DIR"), "/extension_files.rs"));

const MANIFEST: &str = "manifest.json";
const REVISION: &str = "extension-revision.json";
const REVISION_TOKEN: &str = "__PCLM_BUNDLE_REVISION__";
static EMBEDDED_REVISION: OnceLock<String> = OnceLock::new();

/// 옛 판이 꺼내 두었고 이 판에는 없는 파일 — 꺼낼 때 지운다. 쪽지가 가리키지 않아 남아도 돌지는 않지만, 확장 폴더가 이 판의
/// 것과 다르면 무엇이 실려 있는지 폴더만 보고는 알 수 없다. 툴바 팝업은 ADR-046, 설정 창(옵션 페이지)은 ADR-048 에서 걷었다.
const RETIRED: &[&str] = &[
    "popup.html",
    "popup.js",
    "popup.css",
    "options.html",
    "options.js",
    "options.css",
];

/// 꺼낸 결과 — 폴더 · 판 · 확장 ID · `manifest.json` 자리 · 이번에 바뀌었는가.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prepared {
    pub folder: PathBuf,
    pub version: String,
    pub extension_id: String,
    pub manifest_path: PathBuf,
    pub changed: bool,
}

impl Prepared {
    /// 다리가 내는 꼴(camelCase).
    pub fn to_json(&self) -> Value {
        object([
            ("folder", Value::String(crate::path_text(&self.folder))),
            ("version", Value::String(self.version.clone())),
            ("extensionId", Value::String(self.extension_id.clone())),
            (
                "manifestPath",
                Value::String(crate::path_text(&self.manifest_path)),
            ),
            ("changed", Value::Bool(self.changed)),
        ])
    }
}

fn embedded_manifest() -> Result<Value, String> {
    let missing = || "내장 확장이 없습니다. 실행 파일을 다시 빌드하세요.".to_string();
    let (_, bytes) = EMBEDDED_FILES
        .iter()
        .find(|(name, _)| *name == MANIFEST)
        .ok_or_else(missing)?;
    let text = std::str::from_utf8(bytes)
        .map_err(|e| e.to_string())?
        .trim_start_matches('\u{feff}');
    serde_json::from_str(text).map_err(|e| e.to_string())
}

fn text_property(manifest: &Value, name: &str) -> Result<String, String> {
    match manifest.get(name) {
        Some(Value::String(s)) => Ok(s.clone()),
        Some(other) => Err(crate::wrong_type("글", other)),
        None => Err(format!("확장 manifest.json 에 {name} 칸이 없습니다.")),
    }
}

/// 박아 둔 `manifest.json` 의 판.
pub fn embedded_version() -> Result<String, String> {
    text_property(&embedded_manifest()?, "version")
}

/// 내장 확장 전체의 바이트를 나타내는 SHA-256. 이름과 길이도 넣어 파일을 바꾸거나
/// 늘리고 빼는 경우를 모두 가른다. background.js 안의 자리표는 이 값으로 바꾼 뒤 쓴다.
pub fn embedded_revision() -> String {
    EMBEDDED_REVISION
        .get_or_init(|| files_revision(EMBEDDED_FILES))
        .clone()
}

fn files_revision(files: &[(&str, &[u8])]) -> String {
    let mut files: Vec<&(&str, &[u8])> = files.iter().collect();
    files.sort_by_key(|(name, _)| *name);
    let mut hasher = Sha256::new();
    for (name, bytes) in files {
        hasher.update((name.len() as u64).to_be_bytes());
        hasher.update(name.as_bytes());
        hasher.update((bytes.len() as u64).to_be_bytes());
        hasher.update(bytes);
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn prepared_files(revision: &str) -> Result<Vec<(&'static str, Vec<u8>)>, String> {
    EMBEDDED_FILES
        .iter()
        .map(|&(name, bytes)| {
            if name != "background.js" {
                return Ok((name, bytes.to_vec()));
            }
            let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
            if text.matches(REVISION_TOKEN).count() != 1 {
                return Err("확장 background.js의 판 자리표를 읽지 못했습니다.".into());
            }
            Ok((name, text.replace(REVISION_TOKEN, revision).into_bytes()))
        })
        .collect()
}

/// `directory` 는 홈(언제나 업무 홈), `executable` 은 브라우저가 부를
/// 실행 파일. 배포된 `.exe` 가 아니면 거절한다.
pub fn prepare(directory: &Path, executable: &Path) -> Result<Prepared, String> {
    let io = |e: std::io::Error| crate::core(e.into());
    let executable = std::path::absolute(executable).map_err(io)?;
    let is_exe = executable
        .extension()
        .is_some_and(|x| x.to_string_lossy().eq_ignore_ascii_case("exe"));
    if !executable.is_file() || !is_exe {
        return Err("배포된 계약목록.exe에서 확장을 준비하세요.".into());
    }
    let manifest = embedded_manifest()?;
    let revision = embedded_revision();
    let files = prepared_files(&revision)?;
    let key = decode_base64(&text_property(&manifest, "key")?)?;
    let id: String = Sha256::digest(&key)[..16]
        .iter()
        .flat_map(|b| [char::from(b'a' + (b >> 4)), char::from(b'a' + (b & 15))])
        .collect();
    let origin = format!("chrome-extension://{id}/");
    let folder = std::path::absolute(directory)
        .map_err(io)?
        .join("extension");
    std::fs::create_dir_all(&folder).map_err(io)?;
    let mut changed = false;
    // 새 파일을 한 장이라도 쓸 때는 완료 표식을 먼저 걷는다. 중간에 실패하면 호스트가
    // 반쯤 풀린 확장을 새 판으로 알리지 않는다.
    let sync = files.iter().any(|(name, bytes)| {
        !folder.join(name).is_file()
            || std::fs::read(folder.join(name)).ok().as_deref() != Some(bytes.as_slice())
    }) || RETIRED.iter().any(|name| folder.join(name).exists());
    let revision_path = directory.join(REVISION);
    if sync && revision_path.exists() {
        std::fs::remove_file(&revision_path).map_err(io)?;
        changed = true;
    }
    // manifest 를 마지막에 바꾼다. 경로와 공개 키는 업데이트 뒤에도 같아야 한다.
    let mut files: Vec<&(&str, Vec<u8>)> = files.iter().collect();
    files.sort_by_key(|(name, _)| *name == MANIFEST);
    for (name, bytes) in files {
        changed |= write(&folder.join(name), bytes).map_err(io)?;
    }
    for name in RETIRED {
        match std::fs::remove_file(folder.join(name)) {
            Ok(()) => changed = true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(io(e)),
        }
    }
    let host_manifest = directory.join(format!("{HOST_NAME}.json"));
    let body = object([
        ("name", Value::String(HOST_NAME.into())),
        ("description", Value::String("PCLM ERP".into())),
        ("path", Value::String(crate::path_text(&executable))),
        ("type", Value::String("stdio".into())),
        (
            "allowed_origins",
            Value::Array(vec![Value::String(origin.clone())]),
        ),
    ]);
    changed |= write(&host_manifest, bridge_json(&body).as_bytes()).map_err(io)?;
    changed |= write(&directory.join("extension-origin.txt"), origin.as_bytes()).map_err(io)?;
    // 모든 확장 파일과 연결 쪽지를 성공적으로 쓴 뒤에만 새 판을 보인다.
    let revision_json = format!(r#"{{"revision":"{revision}"}}"#);
    changed |= write(&revision_path, revision_json.as_bytes()).map_err(io)?;
    Ok(Prepared {
        folder,
        version: text_property(&manifest, "version")?,
        extension_id: id,
        manifest_path: std::path::absolute(&host_manifest).map_err(io)?,
        changed,
    })
}

/// 같은 바이트면 쓰지 않는다(거짓). 곁의 `<path>.<guid>.tmp` 에 쓴 뒤 이름을 바꾼다.
fn write(path: &Path, bytes: &[u8]) -> std::io::Result<bool> {
    if path.is_file() && std::fs::read(path)? == bytes {
        return Ok(false);
    }
    // 내장 이름에는 하위 폴더가 있다(`panel/panel.js`).
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = PathBuf::from(format!(
        "{}.{}.tmp",
        path.display(),
        pclm_home::win::new_guid_n()
    ));
    let result = std::fs::write(&temporary, bytes).and_then(|()| std::fs::rename(&temporary, path));
    if temporary.is_file() {
        let _ = std::fs::remove_file(&temporary);
    }
    result.map(|()| true)
}

/// Base64 풀기 — 표준 알파벳, 공백은 건너뛰고 `=` 로 채운다.
pub(crate) fn decode_base64(text: &str) -> Result<Vec<u8>, String> {
    let invalid = || "Base64 글이 아닙니다.".to_string();
    let symbols: Vec<u8> = text
        .bytes()
        .filter(|b| !matches!(b, b' ' | b'\t' | b'\r' | b'\n'))
        .collect();
    if !symbols.len().is_multiple_of(4) {
        return Err(invalid());
    }
    let padding = symbols.iter().rev().take_while(|&&b| b == b'=').count();
    if padding > 2 {
        return Err(invalid());
    }
    let mut out = Vec::with_capacity(symbols.len() / 4 * 3);
    let mut buffer = 0u32;
    let mut bits = 0;
    for &b in &symbols[..symbols.len() - padding] {
        let value = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return Err(invalid()),
        };
        buffer = (buffer << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    struct Temp(PathBuf);

    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn files_under(folder: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(folder).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                files_under(&path, out);
            } else {
                out.push(path);
            }
        }
    }

    /// 확장의 manifest · HTML · 모듈이 가리키는 파일이 모두 실렸는가. 빠지면 빌드는 초록인데 사용자 PC 의 확장이 빈 껍데기다.
    #[test]
    fn 확장이_가리키는_파일은_모두_실렸다() {
        let names: Vec<&str> = EMBEDDED_FILES.iter().map(|(n, _)| *n).collect();
        let resolve = |from: &str, target: &str| -> Option<String> {
            if target.contains("://") || target.starts_with("data:") || target.starts_with('#') {
                return None;
            }
            let mut parts: Vec<&str> = if target.starts_with('/') {
                Vec::new()
            } else {
                from.split('/').collect::<Vec<_>>()[..from.matches('/').count()].to_vec()
            };
            for piece in target.trim_start_matches('/').split('/') {
                match piece {
                    "." | "" => {}
                    ".." => {
                        parts.pop();
                    }
                    p => parts.push(p),
                }
            }
            Some(parts.join("/"))
        };
        let mut wanted: Vec<(String, String)> = Vec::new();
        let manifest = embedded_manifest().unwrap();
        for pointer in ["/background/service_worker", "/side_panel/default_path"] {
            if let Some(Value::String(path)) = manifest.pointer(pointer) {
                wanted.push(("manifest.json".into(), path.clone()));
            }
        }
        // 따옴표 안의 글 중 HTML 속성(src · href)과 모듈 지정자(import · from · new URL)만 본다.
        let quoted = |text: &str, lead: &str, out: &mut Vec<String>| {
            for (at, _) in text.match_indices(lead) {
                let rest = &text[at + lead.len()..];
                let rest = rest.trim_start_matches([' ', '(', '=']);
                let Some(quote) = rest.chars().next().filter(|c| matches!(c, '"' | '\'')) else {
                    continue;
                };
                if let Some(end) = rest[1..].find(quote) {
                    out.push(rest[1..1 + end].to_string());
                }
            }
        };
        for (name, bytes) in EMBEDDED_FILES {
            let lower = name.to_ascii_lowercase();
            let Ok(text) = std::str::from_utf8(bytes) else {
                continue;
            };
            let mut targets = Vec::new();
            if lower.ends_with(".html") {
                quoted(text, "src=", &mut targets);
                quoted(text, "href=", &mut targets);
            } else if lower.ends_with(".js") {
                quoted(text, "import", &mut targets);
                quoted(text, "from", &mut targets);
                quoted(text, "new URL", &mut targets);
            }
            for target in targets {
                // 상대 경로(./ ../ 또는 확장자를 가진 이름)만 — 맨 이름의 글자는 모듈 지정자가 아니다.
                let relative = target.starts_with("./")
                    || target.starts_with("../")
                    || target.starts_with('/');
                let file_like = !target.contains(' ')
                    && target.rsplit('/').next().is_some_and(|l| l.contains('.'));
                if relative || (lower.ends_with(".html") && file_like) {
                    wanted.push((name.to_string(), target));
                }
            }
        }
        let missing: Vec<String> = wanted
            .iter()
            .filter_map(|(from, target)| {
                let path = resolve(from, target)?;
                (!names.contains(&path.as_str())).then(|| format!("{from} → {target}"))
            })
            .collect();
        assert!(missing.is_empty(), "실리지 않은 파일: {missing:?}");
        assert!(
            wanted.len() >= 7,
            "가리키는 것을 거의 찾지 못했습니다: {wanted:?}"
        );
    }

    #[test]
    fn 하위_폴더_이름도_꺼낸다() {
        let root = Temp(
            std::env::temp_dir().join(format!("pclm-bundled-sub-{}", pclm_home::win::new_guid_n())),
        );
        let file = root.0.join("extension").join("panel/panel.js");
        assert!(write(&file, b"x").unwrap());
        assert_eq!(std::fs::read(&file).unwrap(), b"x");
        assert!(!write(&file, b"x").unwrap());
    }

    #[test]
    fn base64_는_닷넷처럼_푼다() {
        assert_eq!(decode_base64("TWFu").unwrap(), b"Man");
        assert_eq!(decode_base64("TWE=").unwrap(), b"Ma");
        assert_eq!(decode_base64("TQ==").unwrap(), b"M");
        assert!(decode_base64("TQ=").is_err());
        assert!(decode_base64("T!==").is_err());
    }

    #[test]
    fn 확장_묶음_판은_파일_이름과_바이트_전부를_가른다() {
        let base = [
            ("a.js", b"one".as_slice()),
            ("panel/b.js", b"two".as_slice()),
        ];
        let revision = files_revision(&base);
        assert_eq!(revision, files_revision(&[base[1], base[0]]));
        assert_ne!(revision, files_revision(&[("a.js", b"ONE"), base[1]]));
        assert_ne!(revision, files_revision(&[("a.js", b"one")]));
        assert_ne!(
            revision,
            files_revision(&[("a.js", b"one"), ("panel/c.js", b"two")])
        );
    }

    #[test]
    fn 내장_확장을_꺼내고_같은_id_로_파일과_exe_연결을_갱신한다() {
        let root = Temp(
            std::env::temp_dir().join(format!("pclm-bundled-{}", pclm_home::win::new_guid_n())),
        );
        std::fs::create_dir_all(&root.0).unwrap();
        let directory = root.0.join("사용자 폴더");
        let exe = root.0.join("계약목록.exe");
        assert_eq!(
            prepare(&directory, &exe).unwrap_err(),
            "배포된 계약목록.exe에서 확장을 준비하세요."
        );
        assert!(!directory.exists());
        std::fs::write(&exe, "test executable").unwrap();
        let prepared = prepare(&directory, &exe).unwrap();
        for file in [
            "manifest.json",
            "background.js",
            "capture.js",
            "export.js",
            "chip.js",
        ] {
            assert!(
                !std::fs::read(prepared.folder.join(file))
                    .unwrap()
                    .is_empty()
            );
        }
        for file in RETIRED {
            assert!(
                !EMBEDDED_FILES.iter().any(|(name, _)| name == file),
                "{file}"
            );
        }
        let manifest: Value = serde_json::from_str(
            &std::fs::read_to_string(prepared.folder.join("manifest.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(manifest["version"].as_str().unwrap(), prepared.version);
        assert_eq!(prepared.extension_id, "hnfnknkojoobmpcgnimdjkpnmannhigc");
        let script = std::fs::read_to_string(prepared.folder.join("background.js")).unwrap();
        let revision = embedded_revision();
        assert_eq!(revision.len(), 64);
        assert!(
            revision
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        );
        assert!(script.contains(&revision));
        assert!(!script.contains(REVISION_TOKEN));
        assert_eq!(
            std::fs::read_to_string(directory.join(REVISION)).unwrap(),
            format!(r#"{{"revision":"{revision}"}}"#)
        );
        std::fs::write(prepared.folder.join("background.js"), "old").unwrap();
        std::fs::write(directory.join("config.json"), "existing home config").unwrap();
        let moved = root.0.join("옮긴 앱.exe");
        std::fs::rename(&exe, &moved).unwrap();
        let updated = prepare(&directory, &moved).unwrap();
        assert_eq!(prepared.folder, updated.folder);
        assert_eq!(prepared.extension_id, updated.extension_id);
        assert_eq!(
            script,
            std::fs::read_to_string(updated.folder.join("background.js")).unwrap()
        );
        assert_eq!(
            std::fs::read_to_string(directory.join("config.json")).unwrap(),
            "existing home config"
        );
        let host: Value =
            serde_json::from_str(&std::fs::read_to_string(&updated.manifest_path).unwrap())
                .unwrap();
        assert_eq!(host["path"].as_str().unwrap(), moved.to_string_lossy());
        assert_eq!(host["name"].as_str().unwrap(), HOST_NAME);
        assert_eq!(host["type"].as_str().unwrap(), "stdio");
        let origins = host["allowed_origins"].as_array().unwrap();
        assert_eq!(origins.len(), 1);
        let origin = format!("chrome-extension://{}/", updated.extension_id);
        assert_eq!(origins[0].as_str().unwrap(), origin);
        assert_eq!(
            std::fs::read_to_string(directory.join("extension-origin.txt")).unwrap(),
            origin
        );
        let mut all = Vec::new();
        files_under(&directory, &mut all);
        assert!(all.iter().all(|f| f.extension().is_none_or(|x| x != "tmp")));
        assert!(updated.changed);

        // 앱을 켤 때마다 불린다. 같은 것이면 아무것도 쓰지 않는다.
        let files: Vec<PathBuf> = all
            .into_iter()
            .filter(|f| f.file_name().is_some_and(|n| n != "config.json"))
            .collect();
        let old = SystemTime::UNIX_EPOCH + Duration::from_secs(1_577_836_800);
        let touch = |f: &Path| {
            std::fs::OpenOptions::new()
                .write(true)
                .open(f)
                .unwrap()
                .set_modified(old)
                .unwrap();
        };
        for file in &files {
            touch(file);
        }
        assert!(!prepare(&directory, &moved).unwrap().changed);
        for file in &files {
            assert_eq!(std::fs::metadata(file).unwrap().modified().unwrap(), old);
        }
        std::fs::write(updated.folder.join("chip.js"), "old").unwrap();
        assert!(prepare(&directory, &moved).unwrap().changed);
        assert_ne!(
            std::fs::read_to_string(updated.folder.join("chip.js")).unwrap(),
            "old"
        );
        // 파일이 바뀌면 완료 표식도 마지막에 새 판으로 돌아온다.
        assert_eq!(
            std::fs::read_to_string(directory.join(REVISION)).unwrap(),
            format!(r#"{{"revision":"{revision}"}}"#)
        );
        // 옛 판이 남긴 팝업은 새 판을 꺼낼 때 치운다.
        std::fs::write(updated.folder.join("popup.html"), "old").unwrap();
        assert!(prepare(&directory, &moved).unwrap().changed);
        assert!(!updated.folder.join("popup.html").exists());
        assert_eq!(
            std::fs::metadata(updated.folder.join("background.js"))
                .unwrap()
                .modified()
                .unwrap(),
            old
        );
        assert_eq!(prepared.version, embedded_version().unwrap());
        // 갱신 중 뒤 파일을 쓰지 못하면 먼저 걷은 완료 표식이 돌아오지 않는다. 다음 앱
        // 시작이 전체 확장을 다시 맞출 때까지 호스트는 새 판을 알리지 못한다.
        std::fs::write(updated.folder.join("capture.js"), "old").unwrap();
        let panel = updated.folder.join("panel");
        std::fs::remove_dir_all(&panel).unwrap();
        std::fs::write(&panel, "not a directory").unwrap();
        assert!(prepare(&directory, &moved).is_err());
        assert!(!directory.join(REVISION).exists());
        assert_ne!(
            std::fs::read_to_string(updated.folder.join("capture.js")).unwrap(),
            "old"
        );
    }
}

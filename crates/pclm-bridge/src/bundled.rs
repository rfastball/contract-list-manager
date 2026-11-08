//! 내장 확장(C# `Pclm.Core/Erp/BundledExtension.cs`) — 실행 파일에 박아 둔 `extension/` 을 홈에 꺼내고, 브라우저가 이
//! 실행 파일을 native messaging 호스트로 부를 수 있게 호스트 쪽지(`kr.rfastball.pclm.erp.json`)와 출처를 적는다.
//!
//! 앱을 켤 때마다 불린다 — 같은 바이트면 다시 쓰지 않는다(브라우저가 읽던 파일이 까닭 없이 바뀌지 않게). 쓸 때는 곁의 임시
//! 파일에 쓴 뒤 이름을 바꾸고, `manifest.json` 을 마지막에 바꾼다. 등록부(어느 브라우저가 이 쪽지를 보는가)는 여기가 아니라
//! 다리의 `prepareExtension` 이 적는다.

use std::path::{Path, PathBuf};

use pclm_core::storage::store::bridge_json;
use pclm_host::HOST_NAME;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::object;

/// 박아 둔 확장 파일 — `(이름, 바이트)`, 이름 차례.
pub const EMBEDDED_FILES: &[(&str, &[u8])] = include!(concat!(env!("OUT_DIR"), "/extension_files.rs"));

const MANIFEST: &str = "manifest.json";

/// C# `BundledExtension.Prepared(Folder, Version, ExtensionId, ManifestPath, Changed)`.
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
        Some(other) => Err(crate::wrong_type("String", other)),
        None => Err("The given key was not present in the dictionary.".into()),
    }
}

/// C# `BundledExtension.EmbeddedVersion` — 박아 둔 `manifest.json` 의 판.
pub fn embedded_version() -> Result<String, String> {
    text_property(&embedded_manifest()?, "version")
}

/// C# `BundledExtension.Prepare(directory, executable)` — `directory` 는 홈(언제나 업무 홈), `executable` 은 브라우저가 부를
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
    let key = decode_base64(&text_property(&manifest, "key")?)?;
    let id: String = Sha256::digest(&key)[..16]
        .iter()
        .flat_map(|b| [char::from(b'a' + (b >> 4)), char::from(b'a' + (b & 15))])
        .collect();
    let origin = format!("chrome-extension://{id}/");
    let folder = std::path::absolute(directory).map_err(io)?.join("extension");
    std::fs::create_dir_all(&folder).map_err(io)?;
    let mut changed = false;
    // manifest 를 마지막에 바꾼다. 경로와 공개 키는 업데이트 뒤에도 같아야 한다.
    let mut files: Vec<&(&str, &[u8])> = EMBEDDED_FILES.iter().collect();
    files.sort_by_key(|(name, _)| *name == MANIFEST);
    for (name, bytes) in files {
        changed |= write(&folder.join(name), bytes).map_err(io)?;
    }
    let host_manifest = directory.join(format!("{HOST_NAME}.json"));
    let body = object([
        ("name", Value::String(HOST_NAME.into())),
        ("description", Value::String("PCLM ERP".into())),
        ("path", Value::String(crate::path_text(&executable))),
        ("type", Value::String("stdio".into())),
        ("allowed_origins", Value::Array(vec![Value::String(origin.clone())])),
    ]);
    changed |= write(&host_manifest, bridge_json(&body).as_bytes()).map_err(io)?;
    changed |= write(&directory.join("extension-origin.txt"), origin.as_bytes()).map_err(io)?;
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

/// `Convert.FromBase64String` — 표준 알파벳, 공백은 건너뛰고 `=` 로 채운다.
fn decode_base64(text: &str) -> Result<Vec<u8>, String> {
    let invalid = || {
        "The input is not a valid Base-64 string as it contains a non-base 64 character, more than two padding characters, or an illegal character among the padding characters.".to_string()
    };
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

    #[test]
    fn base64_는_닷넷처럼_푼다() {
        assert_eq!(decode_base64("TWFu").unwrap(), b"Man");
        assert_eq!(decode_base64("TWE=").unwrap(), b"Ma");
        assert_eq!(decode_base64("TQ==").unwrap(), b"M");
        assert!(decode_base64("TQ=").is_err());
        assert!(decode_base64("T!==").is_err());
    }

    /// C# `ErpCaptureTests.내장_확장을_꺼내고_같은_ID로_파일과_exe_연결을_갱신한다`.
    #[test]
    fn 내장_확장을_꺼내고_같은_id_로_파일과_exe_연결을_갱신한다() {
        let root = Temp(std::env::temp_dir().join(format!(
            "pclm-bundled-{}",
            pclm_home::win::new_guid_n()
        )));
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
            "popup.js",
            "popup.html",
            "popup.css",
            "options.html",
            "options.js",
        ] {
            assert!(!std::fs::read(prepared.folder.join(file)).unwrap().is_empty());
        }
        let manifest: Value = serde_json::from_str(
            &std::fs::read_to_string(prepared.folder.join("manifest.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(manifest["version"].as_str().unwrap(), prepared.version);
        assert_eq!(prepared.extension_id, "hnfnknkojoobmpcgnimdjkpnmannhigc");
        let script = std::fs::read_to_string(prepared.folder.join("background.js")).unwrap();
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
            serde_json::from_str(&std::fs::read_to_string(&updated.manifest_path).unwrap()).unwrap();
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
        std::fs::write(updated.folder.join("popup.js"), "old").unwrap();
        assert!(prepare(&directory, &moved).unwrap().changed);
        assert_ne!(
            std::fs::read_to_string(updated.folder.join("popup.js")).unwrap(),
            "old"
        );
        assert_eq!(
            std::fs::metadata(updated.folder.join("background.js"))
                .unwrap()
                .modified()
                .unwrap(),
            old
        );
        assert_eq!(prepared.version, embedded_version().unwrap());
    }
}

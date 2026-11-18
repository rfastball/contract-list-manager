//! 서식 폴더(ADR-049) — 문서 만들기 패널이 쓰는 서식은 이 폴더의 파일이다. 한글 · 메모장에서 고쳐 저장하면 그것이 곧
//! 반영이다. 확장은 폴더에 닿지 못하므로 호스트 메서드 `templates` 가 대신 읽는다:
//!
//! ```text
//! {op:"list"}              → {folder, files:[{name, kind, size}]}   폴더가 없으면 짓고, 없는 동봉 서식만 채운다
//! {op:"read", name}        → {name, kind, data(Base64)}             폴더 바로 아래의 서식 하나
//! {op:"open"}              → {folder}                               탐색기로 연다
//! ```
//!
//! - 자리: 업무 홈이면 문서 폴더의 「계약문서 서식」, 다른 홈(개발 · `--home`)이면 그 홈 아래 「서식」 — 가짜 나라장터 시험이
//!   사람의 진짜 서식 폴더를 건드리지 않게. 자리를 바꾸는 설정은 없다.
//! - **있는 파일은 덮어쓰지 않는다.** 채우는 것은 exe 에 실린 동봉 서식(`extension/templates/`) 가운데 폴더에 없는 이름뿐이다.
//! - 다루는 것은 폴더 바로 아래의 `.hwpx` · `.txt` 뿐이다(하위 폴더 · 숨김 · 이음 · `~` · `.` 로 시작하는 한글의 임시 파일은
//!   건너뛴다). 쓰기 · 지우기 명령은 없다.
//! - `read` 의 이름은 확장이 보낸 것이라 믿지 않는다 — 경로 구분자 · `..` · 절대 경로를 거절하고, 목록에 든 이름만 연다.
//! - 답 한도([`MAX_RESPONSE_BYTES`])를 넘는 서식은 나눠 보내지 않고 거절한다.

use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use pclm_core::erp::wire::{self, MAX_RESPONSE_BYTES};
use pclm_home::Home;
use pclm_host::native_messaging::{envelope, request_id};
use serde_json::{Map, Value};

use crate::bundled::EMBEDDED_FILES;
use crate::os::Os;

/// 호스트 메서드 이름.
pub const METHOD: &str = "templates";
const DOCUMENTS_FOLDER: &str = "계약문서 서식";
const HOME_FOLDER: &str = "서식";
const BUNDLED_PREFIX: &str = "templates/";
/// 답의 봉투(protocolVersion · requestId · ok)와 이름 · 종류 칸이 차지하는 몫.
const ENVELOPE_ROOM: usize = 1024;

pub fn is_request(request: &Value) -> bool {
    request.get("method").and_then(Value::as_str) == Some(METHOD)
}

/// 서식 폴더 — 업무 홈이면 `documents()` 의 「계약문서 서식」, 아니면 홈 아래 「서식」. 문서 폴더를 구하지 못하면 거절한다.
pub fn folder(home: &Home, documents: impl FnOnce() -> PathBuf) -> Result<PathBuf, String> {
    if !home.is_default() {
        return Ok(home.directory().join(HOME_FOLDER));
    }
    let documents = documents();
    if documents.as_os_str().is_empty() {
        return Err("문서 폴더를 찾지 못했습니다.".into());
    }
    Ok(documents.join(DOCUMENTS_FOLDER))
}

/// 요청 하나에 답한다 — 확장이 받는 봉투 꼴(`{protocolVersion: 2, requestId, ok, result|error}`).
pub fn dispatch(folder: Result<PathBuf, String>, os: &dyn Os, request: &Value) -> Value {
    let id = request_id(request);
    if request.get("protocolVersion").and_then(Value::as_i64) != Some(2) {
        return envelope(id, "invalid_request", "앱과 확장을 함께 업데이트하세요.");
    }
    let params = request.get("params").cloned().unwrap_or(Value::Null);
    let answer = folder.and_then(|folder| match params.get("op").and_then(Value::as_str) {
        Some("list") => list(&folder),
        Some("read") => read(&folder, params.get("name").and_then(Value::as_str).unwrap_or("")),
        Some("open") => open(&folder, os),
        _ => Err("지원하지 않는 요청입니다.".into()),
    });
    match answer {
        Ok(result) => {
            let mut out = Map::new();
            out.insert("protocolVersion".into(), 2.into());
            out.insert("requestId".into(), id.map_or(Value::Null, |i| Value::String(i.into())));
            out.insert("ok".into(), Value::Bool(true));
            out.insert("result".into(), result);
            Value::Object(out)
        }
        Err(message) => envelope(id, "invalid_request", &message),
    }
}

fn io_message(e: &io::Error) -> String {
    format!("서식 폴더를 읽지 못했습니다: {e}")
}

/// 폴더를 짓고 없는 동봉 서식만 채운 뒤 서식 목록.
pub(crate) fn list(folder: &Path) -> Result<Value, String> {
    ensure(folder).map_err(|e| io_message(&e))?;
    let files = entries(folder)
        .map_err(|e| io_message(&e))?
        .into_iter()
        .map(|(name, kind, size)| {
            let mut file = Map::new();
            file.insert("name".into(), Value::String(name));
            file.insert("kind".into(), Value::String(kind.into()));
            file.insert("size".into(), size.into());
            Value::Object(file)
        })
        .collect();
    let mut out = Map::new();
    out.insert("folder".into(), Value::String(crate::path_text(folder)));
    out.insert("files".into(), Value::Array(files));
    Ok(Value::Object(out))
}

/// 서식 하나의 바이트(Base64). 이름은 폴더 바로 아래 목록에 든 것만 받는다.
pub(crate) fn read(folder: &Path, name: &str) -> Result<Value, String> {
    let path = Path::new(name);
    if name.is_empty()
        || name.contains(['/', '\\', ':', '\0'])
        || name == "."
        || name == ".."
        || path.is_absolute()
        || path.components().count() != 1
    {
        return Err("서식 이름이 올바르지 않습니다.".into());
    }
    let listed = entries(folder).map_err(|e| io_message(&e))?;
    let Some((_, kind, size)) = listed.into_iter().find(|(n, _, _)| n == name) else {
        return Err(format!("서식을 찾지 못했습니다: {name}"));
    };
    // Base64 는 4/3 배 — 읽기 전에 크기로 먼저 거른다.
    if size.saturating_mul(4) / 3 > MAX_RESPONSE_BYTES as u64 {
        return Err(format!("서식이 너무 큽니다: {name}"));
    }
    let bytes = std::fs::read(folder.join(name)).map_err(|e| io_message(&e))?;
    let mut out = Map::new();
    out.insert("name".into(), Value::String(name.into()));
    out.insert("kind".into(), Value::String(kind.into()));
    out.insert("data".into(), Value::String(encode_base64(&bytes)));
    let out = Value::Object(out);
    // 실제로 나갈 글로 잰다 — 기본 인코더가 `+` 를 늘린다. 넘으면 호스트의 send 가 멈춘다.
    if wire::to_web_json(&out).len() + ENVELOPE_ROOM > MAX_RESPONSE_BYTES {
        return Err(format!("서식이 너무 큽니다: {name}"));
    }
    Ok(out)
}

fn open(folder: &Path, os: &dyn Os) -> Result<Value, String> {
    ensure(folder).map_err(|e| io_message(&e))?;
    os.shell_open(folder)
        .map_err(|e| format!("서식 폴더를 열지 못했습니다: {e}"))?;
    let mut out = Map::new();
    out.insert("folder".into(), Value::String(crate::path_text(folder)));
    Ok(Value::Object(out))
}

/// 폴더를 짓고, exe 에 실린 동봉 서식 가운데 폴더에 없는 이름만 쓴다. 곁의 임시 파일에 쓴 뒤 **덮어쓰지 않는** 이름 바꾸기로
/// 놓는다 — 그사이 사람이 같은 이름을 만들었으면 그것이 이긴다.
fn ensure(folder: &Path) -> io::Result<()> {
    std::fs::create_dir_all(folder)?;
    for (name, bytes) in EMBEDDED_FILES {
        let Some(file) = name.strip_prefix(BUNDLED_PREFIX).filter(|f| !f.contains('/')) else {
            continue;
        };
        let target = folder.join(file);
        if std::fs::symlink_metadata(&target).is_ok() {
            continue;
        }
        let temporary = folder.join(format!(".{file}.{}.tmp", pclm_home::win::new_guid_n()));
        let placed = std::fs::File::create_new(&temporary)
            .and_then(|mut f| f.write_all(bytes).and_then(|()| f.sync_all()))
            .and_then(|()| pclm_home::win::move_file(&temporary, &target, false));
        let _ = std::fs::remove_file(&temporary);
        match placed {
            Ok(()) => {}
            Err(e) if target.exists() => drop(e), // 그사이 생겼다 — 있는 것을 둔다.
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

/// 폴더 바로 아래의 서식 — `(이름, 종류, 크기)`, 이름 차례.
fn entries(folder: &Path) -> io::Result<Vec<(String, &'static str, u64)>> {
    use std::os::windows::fs::MetadataExt as _;
    const HIDDEN_OR_SYSTEM: u32 = 0x2 | 0x4;
    let mut out = Vec::new();
    for entry in std::fs::read_dir(folder)? {
        let entry = entry?;
        // DirEntry::metadata 는 이음을 따라가지 않는다 — 이음은 파일이 아니라 건너뛴다.
        let Ok(meta) = entry.metadata() else { continue };
        let Ok(name) = entry.file_name().into_string() else { continue };
        if !meta.is_file()
            || meta.file_attributes() & HIDDEN_OR_SYSTEM != 0
            || name.starts_with(['~', '.'])
        {
            continue;
        }
        let Some(kind) = kind_of(&name) else { continue };
        out.push((name, kind, meta.len()));
    }
    out.sort();
    Ok(out)
}

fn kind_of(name: &str) -> Option<&'static str> {
    let lower = name.to_lowercase();
    if lower.ends_with(".hwpx") {
        Some("hwpx")
    } else if lower.ends_with(".txt") {
        Some("txt")
    } else {
        None
    }
}

/// Base64 — 표준 알파벳, `=` 로 채운다.
fn encode_base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, &b)| n | u32::from(b) << (16 - 8 * i));
        for i in 0..4 {
            out.push(if i <= chunk.len() {
                char::from(ALPHABET[(n >> (18 - 6 * i) & 63) as usize])
            } else {
                '='
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Temp(PathBuf);

    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn temp() -> Temp {
        Temp(std::env::temp_dir().join(format!("pclm-templates-{}", pclm_home::win::new_guid_n())))
    }

    fn bundled() -> Vec<(&'static str, &'static [u8])> {
        EMBEDDED_FILES
            .iter()
            .filter_map(|(n, b)| Some((n.strip_prefix(BUNDLED_PREFIX)?, *b)))
            .collect()
    }

    fn names(listed: &Value) -> Vec<String> {
        listed["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["name"].as_str().unwrap().to_owned())
            .collect()
    }

    #[test]
    fn base64_는_풀면_그대로다() {
        for bytes in [&b""[..], b"M", b"Ma", b"Man", "한글 서식".as_bytes(), &[0xff, 0xfe, 0x00, 0x3e]] {
            assert_eq!(crate::bundled::decode_base64(&encode_base64(bytes)).unwrap(), bytes);
        }
        assert_eq!(encode_base64(b"Ma"), "TWE=");
    }

    /// 다른 홈(개발 · `--home`)은 사람의 문서 폴더를 건드리지 않는다 — 문서 폴더를 묻지도 않는다.
    #[test]
    fn 다른_홈의_서식_폴더는_홈_아래다() {
        let root = temp();
        let home = Home::new(&root.0).unwrap();
        let folder = folder(&home, || panic!("문서 폴더를 물으면 안 됨")).unwrap();
        assert_eq!(folder, home.directory().join("서식"));
    }

    #[test]
    fn 업무_홈의_서식_폴더는_문서_폴더_아래다() {
        let home = Home::default_home().unwrap();
        assert_eq!(
            folder(&home, || PathBuf::from(r"D:\문서")).unwrap(),
            PathBuf::from(r"D:\문서\계약문서 서식")
        );
        assert!(folder(&home, PathBuf::new).is_err());
    }

    /// 목록은 폴더를 짓고 동봉 서식을 채운다. 사람이 고친 동봉 서식 · 넣은 서식은 그대로 두고, 지운 동봉 서식만 다시 채운다.
    #[test]
    fn 목록은_없는_동봉_서식만_채우고_있는_파일은_그대로_둔다() {
        let root = temp();
        let folder = root.0.join("계약문서 서식");
        let shipped = bundled();
        assert!(shipped.len() >= 3, "{shipped:?}");
        let listed = list(&folder).unwrap();
        let mut expected: Vec<String> = shipped.iter().map(|(n, _)| n.to_string()).collect();
        expected.sort();
        assert_eq!(names(&listed), expected);
        assert_eq!(listed["folder"], crate::path_text(&folder));
        for (name, bytes) in &shipped {
            assert_eq!(std::fs::read(folder.join(name)).unwrap(), *bytes);
        }

        let (edited, _) = shipped[0];
        std::fs::write(folder.join(edited), "사람이 고쳤다").unwrap();
        let (removed, removed_bytes) = shipped[1];
        std::fs::remove_file(folder.join(removed)).unwrap();
        std::fs::write(folder.join("내 기안.txt"), "{{건명}}").unwrap();
        // 다루지 않는 것들.
        std::fs::write(folder.join("메모.docx"), "x").unwrap();
        std::fs::write(folder.join("~$임시.hwpx"), "x").unwrap();
        std::fs::write(folder.join(".숨은.txt"), "x").unwrap();
        std::fs::create_dir(folder.join("묶음.hwpx")).unwrap();
        std::fs::write(folder.join("묶음.hwpx").join("안.txt"), "x").unwrap();
        let hidden = folder.join("숨김 속성.txt");
        std::fs::write(&hidden, "x").unwrap();
        std::process::Command::new("attrib").arg("+h").arg(&hidden).status().unwrap();

        let listed = list(&folder).unwrap();
        assert_eq!(std::fs::read_to_string(folder.join(edited)).unwrap(), "사람이 고쳤다");
        assert_eq!(std::fs::read(folder.join(removed)).unwrap(), removed_bytes);
        let mut expected = expected.clone();
        expected.push("내 기안.txt".into());
        expected.sort();
        assert_eq!(names(&listed), expected);
        let mine = listed["files"].as_array().unwrap().iter().find(|f| f["name"] == "내 기안.txt").unwrap();
        assert_eq!(mine["kind"], "txt");
        assert_eq!(mine["size"], "{{건명}}".len());
        // 임시 파일을 남기지 않는다.
        assert!(std::fs::read_dir(&folder).unwrap().flatten().all(|e| !e.file_name().to_string_lossy().ends_with(".tmp")));
    }

    #[test]
    fn 읽기는_폴더_바로_아래의_목록에_든_서식만_연다() {
        let root = temp();
        let folder = root.0.join("서식");
        list(&folder).unwrap();
        std::fs::write(folder.join("내 기안.txt"), "{{건명}}").unwrap();
        let read_back = read(&folder, "내 기안.txt").unwrap();
        assert_eq!(read_back["kind"], "txt");
        assert_eq!(crate::bundled::decode_base64(read_back["data"].as_str().unwrap()).unwrap(), b"{{\xea\xb1\xb4\xeb\xaa\x85}}");
        std::fs::write(root.0.join("바깥.txt"), "x").unwrap();
        std::fs::create_dir(folder.join("속")).unwrap();
        std::fs::write(folder.join("속").join("안.txt"), "x").unwrap();
        std::fs::write(folder.join("메모.docx"), "x").unwrap();
        let outside = crate::path_text(&root.0.join("바깥.txt"));
        for name in ["", ".", "..", "../바깥.txt", "..\\바깥.txt", "속/안.txt", "속\\안.txt", outside.as_str(), "C:바깥.txt", "내 기안.txt:흐름", "메모.docx", "없는.txt"] {
            assert!(read(&folder, name).is_err(), "{name}");
        }
    }

    #[test]
    fn 답_한도를_넘는_서식은_거절한다() {
        let root = temp();
        let folder = root.0.join("서식");
        list(&folder).unwrap();
        std::fs::write(folder.join("큰 서식.hwpx"), vec![0xfb; MAX_RESPONSE_BYTES * 3 / 4]).unwrap();
        assert_eq!(read(&folder, "큰 서식.hwpx").unwrap_err(), "서식이 너무 큽니다: 큰 서식.hwpx");
        // 크기는 한도 안이어도 나갈 글이 넘으면 거절한다 — 0xfb 의 Base64(`+/v7`)는 `+` 가 + 로 늘어난다.
        std::fs::write(folder.join("큰 서식.hwpx"), vec![0xfb; MAX_RESPONSE_BYTES * 3 / 5]).unwrap();
        assert!(read(&folder, "큰 서식.hwpx").is_err());
        std::fs::write(folder.join("큰 서식.hwpx"), vec![0; MAX_RESPONSE_BYTES * 3 / 5]).unwrap();
        assert!(read(&folder, "큰 서식.hwpx").is_ok());
    }

    #[test]
    fn 봉투는_요청_번호를_싣고_모르는_것은_거절한다() {
        let root = temp();
        let folder = root.0.join("서식");
        let ok = dispatch(Ok(folder.clone()), &crate::WindowsOs, &serde_json::json!({ "protocolVersion": 2, "requestId": "r-1", "method": METHOD, "params": { "op": "list" } }));
        assert_eq!(ok["ok"], true, "{ok}");
        assert_eq!(ok["requestId"], "r-1");
        assert!(!ok["result"]["files"].as_array().unwrap().is_empty());
        let unknown = dispatch(Ok(folder.clone()), &crate::WindowsOs, &serde_json::json!({ "protocolVersion": 2, "requestId": "r-2", "method": METHOD, "params": { "op": "write" } }));
        assert_eq!(unknown["ok"], false);
        assert_eq!(unknown["error"]["code"], "invalid_request");
        let old = dispatch(Ok(folder), &crate::WindowsOs, &serde_json::json!({ "protocolVersion": 1, "requestId": "r-3", "method": METHOD, "params": { "op": "list" } }));
        assert_eq!(old["ok"], false);
        let nowhere = dispatch(Err("문서 폴더를 찾지 못했습니다.".into()), &crate::WindowsOs, &serde_json::json!({ "protocolVersion": 2, "requestId": "r-4", "method": METHOD, "params": { "op": "list" } }));
        assert_eq!(nowhere["error"]["message"], "문서 폴더를 찾지 못했습니다.");
    }
}

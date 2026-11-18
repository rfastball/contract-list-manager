//! 경로와 글 파일 — 홈 · 호스트 · 창이 경로를 펴고 견주고, 쪽지 같은 글 파일을 읽고 통째로 바꿔 쓰는 규칙.
//!
//! - 두 경로는 온전히 편 뒤 끝 구분자 하나를 떼고 대소문자 없이 견준다. 대소문자는 글자마다 대문자로 접는다
//!   ([`pclm_core::text`]) — 한 글자를 한 글자로만 바꾸고, 여러 글자가 되는 것(ß)은 그대로 둔다.
//! - 글 파일은 머리표(UTF-8 · UTF-16 · UTF-32)를 보고 푼다. 쓸 때는 같은 폴더의 임시 파일에 쓴 뒤 이름을 바꿔, 반쯤 쓴
//!   파일이 남지 않게 한다.

use std::io;
use std::path::{Path, PathBuf};

pub use pclm_core::text::eq_ignore_case;

/// 지금 폴더에 대어 온전한 경로로(Windows 의 `GetFullPathNameW`). 빈 경로는 오류다.
pub fn full_path(path: impl AsRef<Path>) -> io::Result<PathBuf> {
    std::path::absolute(path.as_ref())
}

/// 끝 구분자 떼기 — 뿌리가 아니면 끝의 구분자 하나를 뗀다.
pub fn trim_ending_separator(path: &str) -> &str {
    let root = root_length(path);
    match path.chars().last() {
        Some('\\' | '/') if path.len() > root => &path[..path.len() - 1],
        _ => path,
    }
}

/// 뿌리의 길이(바이트) — `C:\`·`\\server\share\`·`\\?\C:\` 의 끝. 뗄 수 없는 끝 구분자를 가리는 데만 쓴다.
fn root_length(path: &str) -> usize {
    let b = path.as_bytes();
    let sep = |c: u8| c == b'\\' || c == b'/';
    if b.len() >= 3 && b[1] == b':' && sep(b[2]) {
        return 3;
    }
    if b.len() >= 2 && b[1] == b':' {
        return 2;
    }
    if b.len() >= 2 && sep(b[0]) && sep(b[1]) {
        // \\server\share\ — 둘째 구분자 뒤까지.
        let rest = &path[2..];
        let mut seen = 0;
        for (i, c) in rest.char_indices() {
            if c == '\\' || c == '/' {
                seen += 1;
                if seen == 2 {
                    return 2 + i + 1;
                }
            }
        }
        return path.len();
    }
    usize::from(b.first().is_some_and(|&c| sep(c)))
}

/// 온전한 경로인가 — 드라이브와 구분자(`C:\`)로 시작하거나 UNC·장치 경로(`\\`·`\?`).
pub fn is_path_fully_qualified(path: &str) -> bool {
    let units: Vec<u16> = path.encode_utf16().take(3).collect();
    let sep = |c: u16| c == u16::from(b'\\') || c == u16::from(b'/');
    if units.len() < 2 {
        return false;
    }
    if sep(units[0]) {
        return units[1] == u16::from(b'?') || sep(units[1]);
    }
    units.len() >= 3
        && units[1] == u16::from(b':')
        && sep(units[2])
        && u8::try_from(units[0]).is_ok_and(|c| c.is_ascii_alphabetic())
}

/// 대문자로 — 글자마다 한 글자로만 바꾼다.
pub fn to_upper_invariant(s: &str) -> String {
    pclm_core::text::ignore_case_key(s)
}

/// 같은 경로인가 — 두 경로를 온전히 펴고 끝 구분자를 뗀 뒤 대소문자 없이 견준다. 펴지 못하는 쪽이 있으면 다르다.
pub fn same_path(a: &str, b: &str) -> bool {
    let (Ok(a), Ok(b)) = (full_path(a), full_path(b)) else {
        return false;
    };
    let (a, b) = (a.to_string_lossy(), b.to_string_lossy());
    eq_ignore_case(trim_ending_separator(&a), trim_ending_separator(&b))
}

/// `path` 가 `folder` 아래에 있는가 — 경로 조각 단위로(`Temp2` 는 `Temp` 아래가 아니다), 대소문자 없이 견준다. 있는 경로는
/// 디스크의 이름으로 풀어(8.3 짧은 이름 · 이음) 견주고, 없는 경로는 편 글 그대로 견준다.
pub fn is_under(path: &Path, folder: &Path) -> bool {
    let normal = |p: &Path| {
        let text = match std::fs::canonicalize(p) {
            Ok(real) => {
                let real = real.to_string_lossy().into_owned();
                match real.strip_prefix(r"\\?\UNC\") {
                    Some(unc) => format!(r"\\{unc}"),
                    None => real.strip_prefix(r"\\?\").map(str::to_owned).unwrap_or(real),
                }
            }
            Err(_) => full_path(p).map(|f| f.to_string_lossy().into_owned()).unwrap_or_default(),
        };
        PathBuf::from(to_upper_invariant(&text))
    };
    let folder = normal(folder);
    !folder.as_os_str().is_empty() && normal(path).starts_with(&folder)
}

/// 글 파일을 통째로 읽는다 — 머리표(UTF-8·UTF-16·UTF-32)를 보고 풀고, 없으면 UTF-8 로 읽는다. 잘못된 바이트는 U+FFFD.
pub fn read_all_text(path: &Path) -> io::Result<String> {
    Ok(decode_text(&std::fs::read(path)?))
}

/// [`read_all_text`] 의 바이트 쪽.
pub fn decode_text(bytes: &[u8]) -> String {
    let utf16 = |body: &[u8], le: bool| {
        let units: Vec<u16> = body
            .chunks(2)
            .map(|p| match (p, le) {
                ([a, b], true) => u16::from_le_bytes([*a, *b]),
                ([a, b], false) => u16::from_be_bytes([*a, *b]),
                _ => 0xFFFD,
            })
            .collect();
        String::from_utf16_lossy(&units)
    };
    let utf32 = |body: &[u8], le: bool| {
        body.chunks(4)
            .map(|p| {
                let v = match (p, le) {
                    ([a, b, c, d], true) => u32::from_le_bytes([*a, *b, *c, *d]),
                    ([a, b, c, d], false) => u32::from_be_bytes([*a, *b, *c, *d]),
                    _ => 0xFFFD,
                };
                char::from_u32(v).unwrap_or('\u{FFFD}')
            })
            .collect::<String>()
    };
    match bytes {
        [0xEF, 0xBB, 0xBF, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        [0xFF, 0xFE, 0, 0, rest @ ..] => utf32(rest, true),
        [0, 0, 0xFE, 0xFF, rest @ ..] => utf32(rest, false),
        [0xFF, 0xFE, rest @ ..] => utf16(rest, true),
        [0xFE, 0xFF, rest @ ..] => utf16(rest, false),
        _ => String::from_utf8_lossy(bytes).into_owned(),
    }
}

/// 그 자리를 통째로 바꿔 쓴다 — `path + "." + 무작위 32자 + ".tmp"` 에 UTF-8(머리표 없음)로 쓴 뒤 이름을 바꾼다
/// (덮어쓰기로 옮긴다). 남은 임시 파일은 지운다. 폴더는 짓지 않는다 — 부르는 쪽이 짓는다.
/// 이름을 바꾸기 전에 내용을 디스크까지 내린다 — 전원이 나가면 이름 바꾸기만 남고 빈 파일이 될 수 있다.
pub fn replace_file(path: &Path, text: &str) -> io::Result<()> {
    use std::io::Write as _;
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(format!(".{}.tmp", crate::win::new_guid_n()));
    let temporary = PathBuf::from(temporary);
    let result = std::fs::File::create(&temporary)
        .and_then(|mut f| f.write_all(text.as_bytes()).and_then(|()| f.sync_all()))
        .and_then(|()| std::fs::rename(&temporary, path));
    if temporary.exists() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fully_qualified_needs_drive_root_or_unc() {
        for full in [
            r"C:\x",
            "C:/x",
            r"\\server\share",
            r"\\?\C:\x",
            "//s/s",
            r"\?",
        ] {
            assert!(is_path_fully_qualified(full), "{full}");
        }
        for partial in [r"상대\x.pclm", r"C:x", r"\x", "x", "", "C:", "1:\\x"] {
            assert!(!is_path_fully_qualified(partial), "{partial}");
        }
    }

    #[test]
    fn under_folder_by_component_ignoring_case() {
        let fake = Path::new(r"C:\없는-임시\Temp\");
        assert!(is_under(Path::new(r"C:\없는-임시\Temp\풀린\계약목록.exe"), fake));
        assert!(is_under(Path::new(r"c:\없는-임시\TEMP\x.exe"), fake));
        assert!(!is_under(Path::new(r"C:\없는-임시\Temp2\x.exe"), fake));
        assert!(!is_under(Path::new(r"C:\앱\x.exe"), Path::new("")));

        // 있는 경로는 디스크의 이름으로 견준다 — 하나는 있고 하나는 없어도 같은 꼴로 편다.
        let root = std::env::temp_dir().join(format!("pclm-under-{}", crate::win::new_guid_n()));
        let temp = root.join("Temp");
        std::fs::create_dir_all(&temp).unwrap();
        std::fs::create_dir_all(root.join("Temp2")).unwrap();
        std::fs::write(temp.join("a.exe"), "").unwrap();
        assert!(is_under(&temp.join("a.exe"), &temp));
        assert!(is_under(&temp.join("없는.exe"), &temp));
        assert!(!is_under(&root.join("Temp2"), &temp));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn trims_one_separator_but_not_root() {
        assert_eq!(trim_ending_separator(r"C:\a\"), r"C:\a");
        assert_eq!(trim_ending_separator(r"C:\"), r"C:\");
        assert_eq!(trim_ending_separator(r"\\s\share\"), r"\\s\share\");
        assert_eq!(trim_ending_separator(r"\\s\share\x\"), r"\\s\share\x");
    }

    #[test]
    fn upper_invariant_maps_one_char_to_one_char() {
        // 여러 글자가 되는 ß 는 그대로다. U+0131(점 없는 i)은 I, ǅ 는 Ǆ.
        assert_eq!(to_upper_invariant("aßıǅ한é"), "AßIǄ한É");
        assert!(same_path(r"C:\Temp\계약\", r"c:\temp\계약"));
        assert!(!same_path(r"C:\Temp\a", r"C:\Temp\b"));
    }

    #[test]
    fn decodes_byte_order_marks() {
        assert_eq!(decode_text(b"\xEF\xBB\xBFab"), "ab");
        assert_eq!(decode_text(b"\xFF\xFEa\0b\0"), "ab");
        assert_eq!(decode_text(b"\xFE\xFF\0a\0b"), "ab");
        assert_eq!(decode_text(b"a\xFFb"), "a\u{FFFD}b");
    }
}

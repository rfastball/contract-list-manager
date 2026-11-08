//! .NET 의 경로·글·파일 뜻 중 홈과 호스트가 기대는 것 — `Path.GetFullPath`·`Path.IsPathFullyQualified`·
//! `Path.TrimEndingDirectorySeparator`·`StringComparison.OrdinalIgnoreCase`·`ToUpperInvariant`·`File.ReadAllText`, 그리고
//! 「같은 폴더의 임시 파일에 쓴 뒤 이름을 바꾼다」 는 C# 의 쓰기 꼴.

use std::io;
use std::path::{Path, PathBuf};

use crate::upper_table::UPPER;

/// `Path.GetFullPath` — 지금 폴더에 대어 온전한 경로로(Windows 의 `GetFullPathNameW`). 빈 경로는 오류다.
pub fn full_path(path: impl AsRef<Path>) -> io::Result<PathBuf> {
    std::path::absolute(path.as_ref())
}

/// `Path.TrimEndingDirectorySeparator` — 뿌리가 아니면 끝의 구분자 하나를 뗀다.
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

/// `Path.IsPathFullyQualified` — 드라이브와 구분자(`C:\`)로 시작하거나 UNC·장치 경로(`\\`·`\?`).
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

/// `string.ToUpperInvariant` — .NET 8 의 표대로 글자마다 바꾼다.
pub fn to_upper_invariant(s: &str) -> String {
    s.chars().map(upper).collect()
}

fn upper(c: char) -> char {
    let cp = c as u32;
    if cp < 0x80 {
        return c.to_ascii_uppercase();
    }
    match UPPER.binary_search_by_key(&cp, |&(from, _)| from) {
        Ok(i) => char::from_u32(UPPER[i].1).unwrap_or(c),
        Err(_) => c,
    }
}

/// `string.Equals(a, b, StringComparison.OrdinalIgnoreCase)` — 대문자로 접어 견준다.
pub fn eq_ignore_case(a: &str, b: &str) -> bool {
    a.chars().map(upper).eq(b.chars().map(upper))
}

/// C# 의 `SamePath` — 두 경로를 온전히 펴고 끝 구분자를 뗀 뒤 대소문자 없이 견준다. 펴지 못하는 쪽이 있으면 다르다.
pub fn same_path(a: &str, b: &str) -> bool {
    let (Ok(a), Ok(b)) = (full_path(a), full_path(b)) else {
        return false;
    };
    let (a, b) = (a.to_string_lossy(), b.to_string_lossy());
    eq_ignore_case(trim_ending_separator(&a), trim_ending_separator(&b))
}

/// `File.ReadAllText` — 머리표(UTF-8·UTF-16·UTF-32)를 보고 풀고, 없으면 UTF-8 로 읽는다. 잘못된 바이트는 U+FFFD.
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

/// 그 자리를 통째로 바꿔 쓴다 — `path + "." + Guid("N") + ".tmp"` 에 UTF-8(머리표 없음)로 쓴 뒤 이름을 바꾼다
/// (`File.Move(…, overwrite: true)`). 남은 임시 파일은 지운다. 폴더는 짓지 않는다 — 부르는 쪽이 짓는다.
pub fn replace_file(path: &Path, text: &str) -> io::Result<()> {
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(format!(".{}.tmp", crate::win::new_guid_n()));
    let temporary = PathBuf::from(temporary);
    let result = std::fs::write(&temporary, text).and_then(|()| std::fs::rename(&temporary, path));
    if temporary.exists() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fully_qualified_follows_dotnet() {
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
    fn trims_one_separator_but_not_root() {
        assert_eq!(trim_ending_separator(r"C:\a\"), r"C:\a");
        assert_eq!(trim_ending_separator(r"C:\"), r"C:\");
        assert_eq!(trim_ending_separator(r"\\s\share\"), r"\\s\share\");
        assert_eq!(trim_ending_separator(r"\\s\share\x\"), r"\\s\share\x");
    }

    #[test]
    fn upper_invariant_uses_the_dotnet_table() {
        // U+0131(점 없는 i)·ß 는 .NET 불변 대문자에서 그대로다. ǅ 는 Ǆ.
        assert_eq!(to_upper_invariant("aßıǅ한é"), "AßıǄ한É");
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

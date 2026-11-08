//! 확장이 어디까지 깔렸는지(C# `Erp/ExtensionStatus.cs`). 앱은 브라우저를 들여다볼 수 없으므로 확장이 호스트에 인사할 때
//! 남긴 흔적(`extension-contact.json`)과 풀어 둔 확장의 판(`extension/manifest.json`)으로 안다.
//!
//! [`read`] 는 창이 읽는 쪽이라 호스트는 부르지 않는다 — 같은 파일을 두 구현이 같게 읽는지 시험하려고 함께 옮겼다.

use std::io;
use std::path::{Path, PathBuf};

use pclm_core::erp::capture::ExtensionHello;
use pclm_core::erp::wire;
use pclm_core::local_time::LocalDateTime;
use pclm_home::{dotnet, win};
use serde_json::{Map, Value};

use crate::log::{self, Entry, Problem};
use crate::presence;

/// 브라우저별 마지막 연결.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Contact {
    pub browser: String,
    pub version: String,
    pub at: String,
}

/// 지금 포트를 쥐고 있는 확장. 시각은 포트가 열린 때다.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveContact {
    pub browser: String,
    pub version: String,
    pub connected_at: String,
}

/// C# `ExtensionStatus.State`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State {
    pub prepared: bool,
    pub embedded_version: String,
    pub disk_version: Option<String>,
    pub contacts: Vec<Contact>,
    pub folder: PathBuf,
    pub live: Vec<LiveContact>,
    pub log: Vec<Entry>,
    pub errors: Vec<Problem>,
    pub past_error: Option<Problem>,
}

const BROWSERS: [&str; 3] = ["Edge", "Chrome", "기타"];

/// .NET 정규식 `\d`(BMP 의 유니코드 Nd) — 실측표. 서로게이트 짝의 숫자는 `\d` 가 아니다.
const DIGITS: &[(u32, u32)] = &[
    (0x0030, 0x0039),
    (0x0660, 0x0669),
    (0x06F0, 0x06F9),
    (0x07C0, 0x07C9),
    (0x0966, 0x096F),
    (0x09E6, 0x09EF),
    (0x0A66, 0x0A6F),
    (0x0AE6, 0x0AEF),
    (0x0B66, 0x0B6F),
    (0x0BE6, 0x0BEF),
    (0x0C66, 0x0C6F),
    (0x0CE6, 0x0CEF),
    (0x0D66, 0x0D6F),
    (0x0DE6, 0x0DEF),
    (0x0E50, 0x0E59),
    (0x0ED0, 0x0ED9),
    (0x0F20, 0x0F29),
    (0x1040, 0x1049),
    (0x1090, 0x1099),
    (0x17E0, 0x17E9),
    (0x1810, 0x1819),
    (0x1946, 0x194F),
    (0x19D0, 0x19D9),
    (0x1A80, 0x1A89),
    (0x1A90, 0x1A99),
    (0x1B50, 0x1B59),
    (0x1BB0, 0x1BB9),
    (0x1C40, 0x1C49),
    (0x1C50, 0x1C59),
    (0xA620, 0xA629),
    (0xA8D0, 0xA8D9),
    (0xA900, 0xA909),
    (0xA9D0, 0xA9D9),
    (0xA9F0, 0xA9F9),
    (0xAA50, 0xAA59),
    (0xABF0, 0xABF9),
    (0xFF10, 0xFF19),
];

fn is_digit(c: char) -> bool {
    let cp = c as u32;
    DIGITS.iter().any(|&(from, to)| (from..=to).contains(&cp))
}

/// 판의 모양 `\A\d+(\.\d+){0,3}\z`.
fn version_shaped(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    parts.len() <= 4
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(is_digit))
}

/// 확장이 보낸 브라우저·판이 디스크에 적어도 되는 모양인가(C# `ExtensionStatus.Accepts`). 상시 연결·연결 기록도 같은
/// 판정을 쓴다 — 확장이 디스크에 임의 글을 쓰지 못하게.
pub fn accepts(browser: &str, version: &str) -> bool {
    BROWSERS.contains(&browser) && version_shaped(version)
}

fn contact_path(directory: &Path) -> PathBuf {
    directory.join("extension-contact.json")
}

/// 사용자 폴더에 풀어 둔 확장의 판(C# `ExtensionStatus.DiskVersion`). 없거나 못 읽으면 `None`.
pub fn disk_version(directory: &Path) -> Option<String> {
    let bytes = std::fs::read(directory.join("extension").join("manifest.json")).ok()?;
    // JsonDocument.Parse(byte[]) — 머리표가 있으면 읽지 못한다. 같은 이름이 둘이면 TryGetProperty 는 뒤의 것이다.
    let manifest = pclm_core::erp::parse_json(std::str::from_utf8(&bytes).ok()?)?;
    manifest.get("version")?.as_str().map(str::to_string)
}

/// 브라우저별 마지막 연결을 적는다(C# `ExtensionStatus.RecordContact`). 모양이 맞는 것만 받는다. 기록이 깨졌거나 같은
/// 키가 두 번 들었으면 읽지 못한 기록으로 보고 새로 쓴다.
pub fn record_contact(
    directory: &Path,
    version: &str,
    browser: &str,
    now: LocalDateTime,
) -> io::Result<()> {
    if !accepts(browser, version) {
        return Ok(());
    }
    let path = contact_path(directory);
    let mut root = match dotnet::read_all_text(&path) {
        Ok(text) => match wire::parse_node(&text) {
            Some(Value::Object(map)) => map,
            _ => Map::new(),
        },
        Err(_) => Map::new(),
    };
    let mut entry = Map::new();
    entry.insert("version".into(), Value::String(version.into()));
    entry.insert("at".into(), Value::String(now.second_text()));
    root.insert(browser.into(), Value::Object(entry));
    std::fs::create_dir_all(directory)?;
    dotnet::replace_file(&path, &wire::to_web_json(&Value::Object(root)))
}

/// `now` 를 「오늘」 의 기준으로 읽는다(C# `ExtensionStatus.Read`). 연결 기록은 그날 로컬 자정부터 낸다.
pub fn read(directory: &Path, embedded_version: &str, now: LocalDateTime) -> State {
    let mut contacts = Vec::new();
    if let Ok(text) = dotnet::read_all_text(&contact_path(directory))
        && let Some(Value::Object(root)) = wire::parse_node(&text)
    {
        for browser in BROWSERS {
            if let Some(Value::Object(entry)) = root.get(browser)
                && let (Some(Value::String(version)), Some(Value::String(at))) =
                    (entry.get("version"), entry.get("at"))
            {
                contacts.push(Contact {
                    browser: browser.into(),
                    version: version.clone(),
                    at: at.clone(),
                });
            }
        }
    }
    let folder = dotnet::full_path(directory)
        .unwrap_or_else(|_| directory.to_path_buf())
        .join("extension");
    let log = log::read(directory, now.date())
        .into_iter()
        .filter(|e| {
            matches!(
                e.event.as_str(),
                log::CONNECTED | log::DISCONNECTED | log::UPDATED
            )
        })
        .collect();
    let (errors, past_error) = log::problems(directory, now);
    State {
        prepared: folder.is_dir(),
        embedded_version: embedded_version.into(),
        disk_version: disk_version(directory),
        contacts,
        folder,
        live: presence::read_live(directory),
        log,
        errors,
        past_error,
    }
}

/// 업무 호스트의 인사가 하는 일 — Core 의 [`ExtensionHello`] 로 넣는다. 적지 못해도 인사는 성공이다.
#[derive(Clone, Debug)]
pub struct HelloFiles {
    pub directory: PathBuf,
}

impl ExtensionHello for HelloFiles {
    fn disk_version(&self) -> Option<String> {
        disk_version(&self.directory)
    }

    fn record_contact(&self, version: &str, browser: &str) {
        let _ = record_contact(&self.directory, version, browser, win::local_now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 판의_모양은_dotnet_의_숫자를_따른다() {
        for ok in ["0.8.0", "1", "1.2.3.4", "١.٢", "０.９"] {
            assert!(accepts("Chrome", ok), "{ok}");
        }
        for bad in [
            "../../x",
            "0.8.0\n",
            "1.2.3.4.5",
            "0.8.0\t끊김",
            "",
            "1..2",
            "v1",
        ] {
            assert!(!accepts("Chrome", bad), "{bad}");
        }
        assert!(accepts("기타", "1"));
        assert!(!accepts("Firefox", "1"));
    }
}

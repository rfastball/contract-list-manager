//! 창의 몸가짐(C# `Storage/WindowPrefs.cs`) — 이 컴퓨터에서 창을 어떻게 다룰지. 홈의 `window.json`
//! ([`crate::Home::window_prefs_path`])에 적는다.
//!
//! **자료가 아니라 자리의 것이다.** 작업자료의 설정에 두면 제출본·작업자료 바꾸기를 따라 남의 컴퓨터로 건너가고,
//! 쪽지에 두면 밖에 내놓은 약속(dataset-contract §1)이 커진다.
//!
//! 읽기는 **너그럽다** — 없거나 깨졌으면 기본값(모두 꺼짐)으로 선다. 쓰기는 쪽지처럼 임시 파일에 쓴 뒤 바꿔치기한다.

use std::io;
use std::path::Path;

use pclm_core::erp::{parse_json, wire};
use serde_json::{Map, Value};

use crate::dotnet;

/// C# `WindowPrefs`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WindowPrefs {
    /// 닫기(X)를 눌러도 끝내지 않고 알림 영역에 둔다.
    pub close_to_tray: bool,
    /// 알림 영역에 처음 둘 때의 풍선을 이미 보였다. 한 번만 보인다.
    pub tray_notice_shown: bool,
}

impl WindowPrefs {
    /// 읽는다(C# `WindowPrefs.Read`). 실패하지 않는다 — 없거나 깨졌거나 못 읽으면 기본값이다.
    ///
    /// `JsonSerializer.Deserialize<WindowPrefs>` 의 뜻: 칸 이름은 대소문자를 가리고, 모르는 칸은 넘기고, 빠진 칸은 꺼짐,
    /// 같은 칸이 둘이면 뒤의 것. 값은 `true`·`false` 만 받는다(글·수·`null` 은 깨진 것). 뿌리가 `null` 이면 기본값.
    pub fn read(path: &Path) -> WindowPrefs {
        if !path.is_file() {
            return WindowPrefs::default();
        }
        dotnet::read_all_text(path)
            .ok()
            .and_then(|text| deserialize(&text))
            .unwrap_or_default()
    }

    /// 쓴다(C# `WindowPrefs.Write`). 같은 폴더의 임시 파일에 쓴 뒤 이름을 바꾼다. 글은 C# 의 `JsonSerializer`
    /// (`WriteIndented = true`)와 바이트까지 같다.
    pub fn write(&self, path: &Path) -> io::Result<()> {
        let full = dotnet::full_path(path)?;
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent)?;
        }
        dotnet::replace_file(path, &self.to_json())
    }

    /// `JsonSerializer.Serialize(this, { WriteIndented = true })`.
    pub fn to_json(&self) -> String {
        let mut map = Map::new();
        map.insert("closeToTray".into(), Value::Bool(self.close_to_tray));
        map.insert(
            "trayNoticeShown".into(),
            Value::Bool(self.tray_notice_shown),
        );
        wire::to_indented_json(&Value::Object(map))
    }
}

/// `None` 은 깨진 글(C# 의 `JsonException`). 뿌리 `null` 은 기본값.
fn deserialize(text: &str) -> Option<WindowPrefs> {
    let map = match parse_json(text)? {
        Value::Null => return Some(WindowPrefs::default()),
        Value::Object(map) => map,
        _ => return None,
    };
    let mut prefs = WindowPrefs::default();
    for (key, value) in &map {
        let slot = match key.as_str() {
            "closeToTray" => &mut prefs.close_to_tray,
            "trayNoticeShown" => &mut prefs.tray_notice_shown,
            _ => continue,
        };
        *slot = value.as_bool()?;
    }
    Some(prefs)
}

/// C# `WindowPrefsTests`. 읽기가 **어떤 꼴에도 넘어지지 않고** 기본값(모두 꺼짐)으로 서는지, 쓴 것이 그대로 돌아오는지.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::Home;
    use crate::home_tests::Root;

    fn fixture() -> (Root, Home) {
        let root = Root::new("window");
        let home = Home::new(root.join("홈")).unwrap();
        (root, home)
    }

    #[test]
    fn 파일이_없으면_모두_꺼짐이다() {
        let (_root, home) = fixture();
        assert_eq!(
            WindowPrefs::read(&home.window_prefs_path()),
            WindowPrefs::default()
        );
        assert!(!home.window_prefs_path().exists()); // 읽기만으로 짓지 않는다
    }

    #[test]
    fn 깨졌으면_넘어지지_않고_모두_꺼짐이다() {
        let (_root, home) = fixture();
        std::fs::create_dir_all(home.directory()).unwrap();
        for text in [
            "",
            "{ 깨진",
            "null",
            "[]",
            "{\"closeToTray\": \"yes\"}",
            "{\"closeToTray\": true, \"trayNoticeShown\": null}",
            "{\"closeToTray\": 1}",
            "{\"closeToTray\": true,}",
        ] {
            std::fs::write(home.window_prefs_path(), text).unwrap();
            assert_eq!(
                WindowPrefs::read(&home.window_prefs_path()),
                WindowPrefs::default(),
                "{text}"
            );
        }
    }

    #[test]
    fn 쓴_것이_그대로_돌아오고_임시_파일을_남기지_않는다() {
        let (_root, home) = fixture();
        let path = home.window_prefs_path();
        let both = WindowPrefs {
            close_to_tray: true,
            tray_notice_shown: true,
        };
        both.write(&path).unwrap();
        assert_eq!(WindowPrefs::read(&path), both);

        let shown = WindowPrefs {
            close_to_tray: false,
            tray_notice_shown: true,
        };
        shown.write(&path).unwrap();
        assert_eq!(WindowPrefs::read(&path), shown);

        let names: Vec<String> = std::fs::read_dir(home.directory())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["window.json"]);
        // C# JsonSerializer(WriteIndented) 와 같은 바이트.
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "{\r\n  \"closeToTray\": false,\r\n  \"trayNoticeShown\": true\r\n}"
        );
    }

    #[test]
    fn 모르는_칸은_넘기고_빠진_칸은_꺼짐이다() {
        let (_root, home) = fixture();
        std::fs::create_dir_all(home.directory()).unwrap();
        std::fs::write(
            home.window_prefs_path(),
            "{\"closeToTray\": true, \"뒷날\": 1, \"TrayNoticeShown\": true}",
        )
        .unwrap();
        assert_eq!(
            WindowPrefs::read(&home.window_prefs_path()),
            WindowPrefs {
                close_to_tray: true,
                tray_notice_shown: false
            }
        );
    }

    /// 창을 불러내는 신호는 잠금처럼 홈마다 하나다 — 표기가 달라도 같은 홈이면 같은 이름이다.
    #[test]
    fn 불러내기_신호는_홈마다_하나다() {
        let (root, home) = fixture();
        let same = Home::new(format!(
            "{}\\",
            crate::dotnet::to_upper_invariant(&home.directory().to_string_lossy())
        ))
        .unwrap();
        let other = Home::new(root.join("다른 홈")).unwrap();

        assert_eq!(home.activation_name(), same.activation_name());
        assert_ne!(home.activation_name(), other.activation_name());
        assert!(home.activation_name().starts_with("Local\\"));
    }
}

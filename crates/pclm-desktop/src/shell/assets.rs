//! exe 에 박은 화면(C# `MainWindow` 의 `EmbeddedResource` + `WebResourceRequested`) — Tauri 의 앱 프로토콜이 여기서 받는다.
//!
//! 폴더를 쓰지 않는 까닭은 C# 과 같다 — 디스크의 폴더를 요구하면 배포 단위가 폴더가 되고, 화면을 빠뜨린 채 나가는 사고가 열린다.
//! 여기서는 화면이 exe 와 한 몸이다. 박을 화면이 없으면(vite build 를 건너뛰었다) 안내 한 장만 낸다.

use std::borrow::Cow;

use tauri::utils::assets::{AssetKey, AssetsIter, CspHash};
use tauri::{Assets, Runtime};

/// build.rs 가 지은 목록 — `(요청 경로, 바이트)`.
static FILES: &[(&str, &[u8])] = include!(concat!(env!("OUT_DIR"), "/web_assets.rs"));

/// C# 이 창에 띄우던 글(`ShowFallback`).
pub const MISSING: &str = "화면이 exe 안에 들어 있지 않습니다.\n\n\
                           npm install 뒤 npx vite build 를 돌리고 다시 빌드해 주세요 \
                           (run.ps1 · publish.ps1 이 그 순서를 쥡니다).";

/// 화면이 박혀 있는가(C# `Read("wwwroot/index.html") is not null`).
pub fn has_screen() -> bool {
    FILES.iter().any(|(name, _)| *name == "index.html")
}

pub struct Screen {
    fallback: Vec<u8>,
}

impl Screen {
    pub fn new() -> Screen {
        let text = MISSING
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('\n', "<br>");
        let fallback = format!(
            "<!doctype html><html lang=\"ko\"><meta charset=\"utf-8\"><title>계약 목록</title>\
             <body style=\"margin:32px;font:14px sans-serif;display:flex;align-items:center;justify-content:center;\
             min-height:80vh\"><p>{text}</p></body></html>"
        );
        Screen {
            fallback: fallback.into_bytes(),
        }
    }

    fn get(&self, key: &str) -> Option<&[u8]> {
        let path = key.trim_start_matches('/');
        let path = if path.is_empty() { "index.html" } else { path };
        if !has_screen() {
            return (path == "index.html").then_some(self.fallback.as_slice());
        }
        // 대소문자 · 구분자에 기대지 않는다(C# 이 박은 이름을 접어 찾던 것과 같은 까닭 — 표기 하나가 어긋나 하얀 화면이 되지 않게).
        FILES
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(&path.replace('\\', "/")))
            .map(|(_, bytes)| *bytes)
    }
}

impl<R: Runtime> Assets<R> for Screen {
    fn get(&self, key: &AssetKey) -> Option<Cow<'_, [u8]>> {
        Screen::get(self, key.as_ref()).map(Cow::Borrowed)
    }

    fn iter(&self) -> Box<AssetsIter<'_>> {
        Box::new(
            FILES
                .iter()
                .map(|(name, bytes)| (Cow::Owned(format!("/{name}")), Cow::Borrowed(*bytes))),
        )
    }

    fn csp_hashes(&self, _html_path: &AssetKey) -> Box<dyn Iterator<Item = CspHash<'_>> + '_> {
        Box::new(std::iter::empty())
    }
}

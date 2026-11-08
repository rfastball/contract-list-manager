//! ERP 자료(C# `Pclm.Core.Erp`) — 수집 매핑, 수집 저장과 차이 검토, 지금 보는 화면의 투영, 수집 기록, 명시 참조 연결.
//!
//! 환경 효과가 없다(#8, #10) — 홈·확장 폴더·시계를 모른다. 홈의 쪽지가 아직 이 파일을 가리키는지(ADR-032)는 부르는
//! 쪽이 [`capture::Destination`] 으로 넣어 주고, 시각은 [`crate::Env`] 가 낸다.

pub mod auto_capture;
pub mod capture;
pub mod capture_log;
pub(crate) mod decimal;
pub(crate) mod element;
pub mod explicit_links;
pub mod mapping;
pub mod mirror;
pub mod screens;
pub mod tools;
pub mod wire;

/// `JsonDocument.Parse` — ERP JSON 한 벌을 읽는다. 수는 원문 그대로, 깊이 64 를 넘으면 거절한다. 읽지 못하면 `None`.
pub fn parse_json(text: &str) -> Option<serde_json::Value> {
    crate::json::parse(text).ok()
}

//! 브라우저 확장의 native messaging 호스트가 홈에 남기는 파일과 메시지 고리(C# `Erp/ExtensionPresence`·`ExtensionRelay`·
//! `ExtensionStatus`·`ExtensionLog`·`Executable`·`NativeMessaging`, #10).
//!
//! **C# 창이 같은 파일을 읽고 쓴다.** 상시 연결 파일·탭 보고·명령 폴더·연결 기록·마지막 연결의 바이트가 C# 호스트와 같아서,
//! 지금 창은 Rust 호스트를 C# 호스트와 가리지 않는다. 글의 뜻은 Core([`pclm_core::erp::wire`]), 홈은 [`pclm_home`] 가 쥐고
//! 여기는 파일과 실마리만 다룬다.

pub mod executable;
pub mod log;
pub mod native_messaging;
pub mod presence;
pub mod relay;
pub mod status;

/// 업무 호스트의 이름(C# `ErpConnection.HostName`) — 브라우저 등록부의 키.
pub const HOST_NAME: &str = "kr.rfastball.pclm.erp";

/// 개발 호스트의 이름(C# `ErpDevelopment.HostName`). 업무 호스트와 다른 것은 이름과 홈뿐이다.
pub const DEVELOPMENT_HOST_NAME: &str = "kr.rfastball.pclm.erp.dev";

#[cfg(test)]
mod tests;

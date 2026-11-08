//! 창 · 확장 호스트 한 몸(#8 ⑧b) — 배포되는 `계약목록.exe`.
//!
//! 창은 [`shell`](`shell` 기능 뒤). 창 없이 따질 수 있는 규칙(인자 · 옛 exe 치우기 · 파일 연결 · 크레이트 고지)은 여기 있다.

pub mod association;
pub mod launch;
pub mod notices;
pub mod old_executables;

#[cfg(all(windows, feature = "shell"))]
pub mod shell;

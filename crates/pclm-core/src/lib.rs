//! 계약 목록 작업자료(`.pclm`)의 저장 Core.
//!
//! **환경 효과가 없다**(#8). 창도, 홈(`%LOCALAPPDATA%\Pclm`) 탐색도, 시계도 없다 — 파일은 호출자가 준
//! 경로에서만 열고, 시각과 식별자는 호출자가 넣어 준다([`Env`]). 그래야 단독 창과 문서나르미가 같은 크레이트를
//! 같은 프로세스에서 링크할 수 있다. 창·홈은 이 크레이트 밖의 얇은 층이다.

pub mod decimal;
pub mod dict;
pub mod domain;
pub mod env;
pub mod erp;
pub mod error;
pub(crate) mod json;
pub mod local_time;
pub mod sqlite;
pub mod storage;
pub(crate) mod text;

pub use env::Env;
pub use error::{Error, Result};
pub use storage::database::{Access, Database};
pub use storage::pclm_file::{
    APPLICATION_ID, PclmFileError, PclmInfo, PclmKind, PclmRole, ViewedFile,
};

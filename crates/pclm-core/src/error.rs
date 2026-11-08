//! 오류. C# 의 예외 갈래를 하나의 열거로 옮긴다 — 부르는 쪽(창·명령줄·문서나르미)이 갈래로 가른다.

use std::fmt;

use crate::storage::pclm_file::PclmFileError;

/// 이 크레이트의 오류.
#[derive(Debug)]
pub enum Error {
    /// SQLite 가 낸 오류(C# `SqliteException`).
    Sqlite(rusqlite::Error),
    /// 파일 오류(C# `IOException`·`FileNotFoundException`). 이미 있는 자리에 짓지 않을 때도 이것이다.
    Io(std::io::Error),
    /// 할 수 없는 일(C# `InvalidOperationException`). 글은 C# 과 같은 사람 말이다.
    InvalidOperation(String),
    /// 잘못된 인자(C# `ArgumentException`).
    Argument(String),
    /// 열지 않기로 한 파일(C# `PclmFileException`).
    PclmFile(PclmFileError),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    pub(crate) fn invalid(message: impl Into<String>) -> Self {
        Error::InvalidOperation(message.into())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Sqlite(e) => write!(f, "{e}"),
            Error::Io(e) => write!(f, "{e}"),
            Error::InvalidOperation(m) | Error::Argument(m) => f.write_str(m),
            Error::PclmFile(e) => f.write_str(&e.message),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Sqlite(e) => Some(e),
            Error::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Sqlite(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<PclmFileError> for Error {
    fn from(e: PclmFileError) -> Self {
        Error::PclmFile(e)
    }
}

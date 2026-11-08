//! `.pclm` 파일을 알아보고, 짓고, 연다(C# `Storage/PclmFile.cs`).
//!
//! **생성과 열기를 가른다**(ADR-031). [`create`] 는 있는 파일을 덮지 않고, [`open_work`] 는 없는 파일을 짓지
//! 않는다. **판별은 한 함수가 낸다** — [`inspect`]. 창·명령줄·호스트·취합이 저마다 따로 보면 어느 하나가 조용히
//! 다른 것을 받아들인다.

use std::fmt;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags, OptionalExtension};

use crate::env::{Env, sqlite_now};
use crate::error::{Error, Result};
use crate::storage::database::{
    Access, Database, begin, connect, refresh, user_version, with_suffix,
};
use crate::storage::{schema, store};

/// `PclmFile.ApplicationId` — SQLite 머리의 `application_id`. `'PCLM'` 네 글자다.
pub const APPLICATION_ID: i64 = 0x50434C4D;

/// C# `PclmKind` — 파일을 들여다본 결과의 갈래.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PclmKind {
    /// 그 자리에 파일이 없다.
    NotFound,
    /// SQLite 가 아니거나, SQLite 라도 PCLM 표지가 없다. 열지 않는다.
    NotPclm,
    /// 기준선보다 옛 시험판(v1~v18). 이 프로그램이 올리지 못한다.
    TooOld,
    /// 이 프로그램보다 새 판. 내릴 길이 없다.
    TooNew,
    /// 이 프로그램이 받을 수 있는 PCLM 파일. 역할은 [`PclmInfo::role`] 에 있다.
    Ok,
}

/// C# `PclmInfo` — [`inspect`] 의 결과. `role`·`dataset_id` 는 [`PclmKind::Ok`] 일 때만 찬다. `version` 은 읽었으면
/// 그 판, 못 읽었으면 0.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PclmInfo {
    pub kind: PclmKind,
    pub role: Option<String>,
    pub dataset_id: Option<String>,
    pub version: i32,
}

impl PclmInfo {
    fn bare(kind: PclmKind, version: i32) -> Self {
        PclmInfo {
            kind,
            role: None,
            dataset_id: None,
            version,
        }
    }
}

/// C# `PclmRole` — 파일의 역할. `pclm_file.role` 에 적히는 글자 그대로다(스키마의 CHECK 와 같은 다섯).
///
/// 쓰기를 허락하는 것은 [`PclmRole::WORK`] 하나다. 제출본·취합본·백업은 그때의 기록이라 그 자리에서 고치는 길이
/// 없다(ADR-032). C# 처럼 글자 상수로 둔다 — 파일에서 읽은 역할은 모르는 글자일 수도 있다.
pub struct PclmRole;

impl PclmRole {
    pub const WORK: &'static str = "work";
    pub const SUBMISSION: &'static str = "submission";
    pub const MERGED: &'static str = "merged";
    pub const BACKUP: &'static str = "backup";
    pub const RETIRED: &'static str = "retired";

    /// `PclmRole.All`.
    pub const ALL: [&'static str; 5] = [
        Self::WORK,
        Self::SUBMISSION,
        Self::MERGED,
        Self::BACKUP,
        Self::RETIRED,
    ];

    /// `PclmRole.Name` — 사람에게 보일 이름.
    pub fn name(role: Option<&str>) -> String {
        match role {
            Some(Self::WORK) => "작업자료".into(),
            Some(Self::SUBMISSION) => "제출본".into(),
            Some(Self::MERGED) => "취합본".into(),
            Some(Self::BACKUP) => "백업".into(),
            Some(Self::RETIRED) => "옮겨진 옛 자료".into(),
            _ => format!("알 수 없는 역할({})", role.unwrap_or("")),
        }
    }

    /// `PclmRole.Require` — 다섯 가운데 하나가 아니면 거절한다.
    pub fn require(role: &str) -> Result<()> {
        if Self::ALL.contains(&role) {
            Ok(())
        } else {
            Err(Error::Argument(format!("모르는 역할입니다: {role}")))
        }
    }
}

/// C# `PclmFileException` — 열지 않기로 한 파일. 무엇이 왜 안 되는지를 함께 든다.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PclmFileError {
    pub path: PathBuf,
    pub kind: PclmKind,
    pub role: Option<String>,
    pub message: String,
}

impl fmt::Display for PclmFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for PclmFileError {}

/// `PclmFile.Inspect` — 파일을 들여다본다. **아무것도 쓰지 않고 아무 파일도 만들지 않는다** — 읽기 전용
/// 연결이고, WAL 이 아닌 파일이면 `-wal`·`-shm` 도 생기지 않는다.
pub fn inspect(path: &Path) -> PclmInfo {
    if !path.is_file() {
        return PclmInfo::bare(PclmKind::NotFound, 0);
    }
    // 빈 파일도 SQLite 는 "빈 DB" 로 열어 준다. 표지가 없으니 아래에서 NotPclm 으로 떨어지지만, 열어 볼 것도
    // 없어 먼저 거른다.
    if std::fs::metadata(path).map(|m| m.len()).unwrap_or(0) == 0 {
        return PclmInfo::bare(PclmKind::NotPclm, 0);
    }
    // 26(not a database)이 대부분이다. 무엇으로 실패했든 열지 않을 파일이다.
    look(path).unwrap_or_else(|_| PclmInfo::bare(PclmKind::NotPclm, 0))
}

fn look(path: &Path) -> Result<PclmInfo> {
    let connection = connect(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let application_id: i64 = connection.query_row("PRAGMA application_id;", [], |r| r.get(0))?;
    let version = user_version(&connection)?;

    if application_id == APPLICATION_ID {
        if version > schema::VERSION {
            return Ok(PclmInfo::bare(PclmKind::TooNew, version));
        }
        if schema::is_pre_baseline(version) {
            return Ok(PclmInfo::bare(PclmKind::TooOld, version));
        }
        if !has_table(&connection, "pclm_file")? {
            return Ok(PclmInfo::bare(PclmKind::NotPclm, version));
        }
        let row: Option<(String, String)> = connection
            .query_row(
                "SELECT role, dataset_id FROM pclm_file WHERE singleton = 1;",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        return Ok(match row {
            Some((role, dataset_id)) => PclmInfo {
                kind: PclmKind::Ok,
                role: Some(role),
                dataset_id: Some(dataset_id),
                version,
            },
            None => PclmInfo::bare(PclmKind::NotPclm, version),
        });
    }

    if application_id != 0 {
        return Ok(PclmInfo::bare(PclmKind::NotPclm, version));
    }

    // 표지가 생기기 전의 파일. 옛 시험판은 표지 없이 판만 달고 있다.
    if schema::is_pre_baseline(version) {
        return Ok(PclmInfo::bare(PclmKind::TooOld, version));
    }

    // 임시 규칙: 표지가 생기기 전의 v19 작업자료(erp_dataset 이 있다)를 PCLM 으로 알아본다. v20 판올림이 표지를
    // 박으므로 지금 쓰는 사람의 자료를 한 번 건네주는 다리일 뿐이다. 1.0 전에 걷는다(ADR-031).
    if version == schema::BASELINE_VERSION && has_table(&connection, "erp_dataset")? {
        let dataset_id: Option<String> = connection
            .query_row(
                "SELECT dataset_id FROM erp_dataset WHERE singleton = 1;",
                [],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        return Ok(PclmInfo {
            kind: PclmKind::Ok,
            role: Some(PclmRole::WORK.into()),
            dataset_id,
            version,
        });
    }

    Ok(PclmInfo::bare(PclmKind::NotPclm, version))
}

/// `PclmFile.Create` — 새 PCLM 파일을 짓는다. **빈 자료가 서는 길은 이것 하나다**(ADR-031). 판올림까지 마친 쓰기
/// 손잡이를 낸다.
///
/// 그 자리에 파일(이나 `-wal`)이 이미 있으면 **아무것도 하지 않고** 실패한다 — 덮는 순간 남의 자료가 빈 자료로
/// 바뀐다. **C# 과 달리 폴더를 짓지 않는다** — 폴더는 부르는 쪽(셸)이 짓고, 없으면 SQLite 가 열지 못해(코드 14)
/// 실패한다(#8: 이 크레이트는 준 경로의 파일 말고는 만들지 않는다).
pub fn create(path: &Path, role: &str, env: &dyn Env) -> Result<Database> {
    PclmRole::require(role)?;

    if path.exists() || with_suffix(path, "-wal").exists() {
        return Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!(
                "그 자리에 이미 파일이 있어 새로 만들지 않습니다: {}",
                path.display()
            ),
        )));
    }

    Database::create_empty_file(path)?;

    let database = Database::new(path, Access::Write);
    database.migrate(env)?;

    let connection = database.open()?;
    // 판올림은 언제나 work 로 심는다. 다른 역할로 지을 때만 고쳐 적는다.
    if role != PclmRole::WORK {
        connection.execute(
            "UPDATE pclm_file SET role = ?1 WHERE singleton = 1;",
            [role],
        )?;
    } else {
        // 빈 작업자료는 서식용 예시 보기를 들고 선다(docs/user-views.md §13). 취합본·제출본으로 짓는 빈 파일에는
        // 들지 않는다 — 취합본의 맞춤 뷰는 제출본에서만 온다.
        let transaction = begin(&connection)?;
        store::seed_example_views(&transaction, None, env)?;
        refresh(&transaction)?;
        transaction.commit()?;
    }
    Ok(database)
}

/// `PclmFile.IsWork` — 열린 파일의 역할이 작업자료인가. 판올림이 예시 보기를 심을지 정한다.
pub(crate) fn is_work(connection: &Connection) -> Result<bool> {
    let role: Option<rusqlite::types::Value> = connection
        .query_row("SELECT role FROM pclm_file WHERE singleton = 1;", [], |r| {
            r.get(0)
        })
        .optional()?;
    // C# `ExecuteScalar() as string == Work` — 글이 아니면 아니다.
    Ok(matches!(role, Some(rusqlite::types::Value::Text(t)) if t == PclmRole::WORK))
}

/// `PclmFile.OpenWork` — 작업자료로 연다. **PCLM 이고 역할이 `work` 인 파일만** 쓰기 손잡이를 받는다. 없는 파일을
/// 짓지 않는다. 판올림은 부르는 쪽이 [`Database::migrate`] 로 한다.
pub fn open_work(path: &Path) -> Result<Database> {
    let info = inspect(path);
    if info.kind == PclmKind::Ok && info.role.as_deref() == Some(PclmRole::WORK) {
        return Ok(Database::new(path, Access::Write));
    }
    Err(refusal(path, info))
}

fn refusal(path: &Path, info: PclmInfo) -> Error {
    Error::PclmFile(PclmFileError {
        path: path.to_path_buf(),
        kind: info.kind,
        message: why(&info, path),
        role: info.role,
    })
}

/// C# `ViewedFile` — 열어 본 파일([`open_view`]). 손잡이는 **원본이 아니라 사본**을 가리키는 읽기 손잡이다.
/// 놓으면(`Drop`) 사본을 지운다 — 지우지 못한 것은 둔다(C# 은 다음 시작의 `Home.PruneViews` 가 걷는다).
#[derive(Debug)]
pub struct ViewedFile {
    database: Database,
    source_path: PathBuf,
    role: String,
    version: i32,
}

impl ViewedFile {
    /// 사본 위의 읽기 손잡이([`Access::Read`]).
    pub fn database(&self) -> &Database {
        &self.database
    }

    /// 사람이 고른 원본의 경로. 화면에 보이는 것은 이것이다.
    pub fn source_path(&self) -> &Path {
        &self.source_path
    }

    /// 원본의 역할([`PclmRole`]).
    pub fn role(&self) -> &str {
        &self.role
    }

    /// 원본의 판. 사본은 이 프로그램의 판까지 올라가 있을 수 있다.
    pub fn version(&self) -> i32 {
        self.version
    }
}

impl Drop for ViewedFile {
    fn drop(&mut self) {
        discard(self.database.path());
    }
}

/// `ViewedFile.Discard` — 사본을 지운다. 못 지우면 둔다. rusqlite 에는 연결 풀이 없어 `ReleasePool` 이 할 일이 없다.
fn discard(copy: &Path) {
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let _ = std::fs::remove_file(with_suffix(copy, suffix));
    }
}

/// `PclmFile.OpenView` — 남의 것·제출본·취합본·백업을 **열어 본다**(ADR-031). 원본을 읽기 전용 연결로 `copy` 에
/// 한 벌 뜨고, 그 사본을 읽기 손잡이로 건넨다. 역할은 가리지 않는다.
///
/// **원본에는 닿지 않는다.** 판이 낮으면 **사본을** 올린다. 사본은 WAL 이 아닌 한 파일로 둔다.
///
/// C# 은 사본 자리를 `workDirectory` 안에 `열람-{Guid}.pclm` 로 스스로 짓는다. 이 크레이트는 이름을 짓지 않는다
/// — 사본의 온전한 경로 `copy` 를 부르는 쪽(셸, 홈의 열람 폴더)이 준다. 폴더도 부르는 쪽이 짓는다.
pub fn open_view(path: &Path, copy: &Path, env: &dyn Env) -> Result<ViewedFile> {
    let info = inspect(path);
    if info.kind != PclmKind::Ok {
        return Err(refusal(path, info));
    }
    let made = (|| -> Result<()> {
        Database::new(path, Access::Read).snapshot(copy, false)?;
        if info.version < schema::VERSION {
            // 쓰기 손잡이로 여는 것은 사본이다. 올리는 김에 뷰도 새로 선다.
            Database::new(copy, Access::Write).migrate(env)?;
            // 쓰기 손잡이가 WAL 로 바꿔 두었다. 한 파일로 되돌린다.
            let connection = connect(copy, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
            connection.execute_batch("PRAGMA journal_mode = DELETE;")?;
        }
        Ok(())
    })();
    if let Err(e) = made {
        discard(copy);
        return Err(e);
    }
    Ok(ViewedFile {
        database: Database::new(copy, Access::Read),
        source_path: path.to_path_buf(),
        role: info.role.unwrap_or_default(),
        version: info.version,
    })
}

/// `PclmFile.Snapshot` — `source` 를 `target` 에 한 벌 뜨고 그 사본에 역할과 **새 신원**을 적는다. 뜬 자리를 낸다.
///
/// **사본은 언제나 새 `dataset_id` 를 받는다**(ADR-031). `created_at` 도 지금으로 고친다. C# 이 SQL 의
/// `randomblob`·`strftime('now')` 로 짓던 둘을 `env` 에서 받는다 — 시각은 C# 과 같은 꼴(밀리초 + `0000`).
///
/// 고쳐 적는 연결은 **WAL 로 바꾸지 않는다** — `VACUUM INTO` 가 떨군 한 파일을 그대로 두어야 받는 쪽이 파일
/// 하나만 가져가도 알맹이가 빠지지 않는다.
pub fn snapshot(
    source: &Database,
    target: &Path,
    role: &str,
    overwrite: bool,
    env: &dyn Env,
) -> Result<PathBuf> {
    PclmRole::require(role)?;
    if overwrite {
        require_overwritable(target)?;
    }
    let saved = source.snapshot(target, overwrite)?;
    let connection = connect(&saved, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    // SQL 이 dataset_id 를 created_at 보다 먼저 지으므로 같은 차례로 받는다.
    let dataset_id = env.new_id();
    let created_at = sqlite_now(env);
    let changed = connection.execute(
        "UPDATE pclm_file\n   SET role = ?1,\n       dataset_id = ?2,\n       created_at = ?3\n WHERE singleton = 1;",
        rusqlite::params![role, dataset_id, created_at],
    )?;
    if changed != 1 {
        return Err(Error::invalid(format!(
            "뜬 사본에 파일 이름표(pclm_file)가 없습니다: {}",
            saved.display()
        )));
    }
    Ok(saved)
}

/// `PclmFile.RequireOverwritable` — 결과물(제출본·취합본·백업)로 덮어도 되는 자리인가. 아니면 그 까닭으로 거절한다.
///
/// **작업자료와 옮겨진 옛 자료는 덮지 않는다** — 표지 전의 v19 작업자료도 작업자료다. PCLM 이 아닌 파일은 사람이
/// 덮기로 고른 것이라 막지 않는다.
pub fn require_overwritable(target: &Path) -> Result<()> {
    let existing = inspect(target);
    let role = existing.role.as_deref();
    if existing.kind == PclmKind::Ok && matches!(role, Some(PclmRole::WORK | PclmRole::RETIRED)) {
        return Err(Error::invalid(format!(
            "그 자리에는 {}이(가) 있어 덮지 않습니다 — 다른 이름을 고르세요: {}",
            PclmRole::name(role),
            target.display()
        )));
    }
    Ok(())
}

/// `PclmFile.Why` — 열지 않는 까닭. 경로를 함께 적는다 — 어느 파일 이야기인지 모르면 고칠 데를 찾지 못한다.
pub fn why(info: &PclmInfo, path: &Path) -> String {
    let path = path.display();
    match info.kind {
        PclmKind::NotFound => format!("파일이 없습니다: {path}"),
        PclmKind::NotPclm => format!("계약 목록 자료(PCLM) 파일이 아닙니다: {path}"),
        PclmKind::TooOld => format!(
            "정식판 이전의 옛 시험판(v{})으로 지은 자료라 이 프로그램이 올리지 못합니다. \
             0.7.0 으로 한 번 열어 v{} 로 올린 뒤 다시 여세요: {path}",
            info.version,
            schema::BASELINE_VERSION
        ),
        PclmKind::TooNew => format!(
            "이 프로그램보다 새 판(v{})으로 지은 자료입니다. 프로그램을 새로 받아 여세요: {path}",
            info.version
        ),
        PclmKind::Ok => format!(
            "작업자료가 아니라 {}입니다 — 그 자리에서 고치지 않습니다: {path}",
            PclmRole::name(info.role.as_deref())
        ),
    }
}

fn has_table(connection: &Connection, name: &str) -> Result<bool> {
    let count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1;",
        [name],
        |r| r.get(0),
    )?;
    Ok(count > 0)
}

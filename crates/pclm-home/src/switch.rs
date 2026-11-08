//! 작업자료 바꾸기(C# `Storage/HomeSwitch.cs`, ADR-032) — 옮기기·다른 작업자료 쓰기·스냅샷에서 새로·새 계약자료,
//! 그리고 백업.
//!
//! **바꾸는 일은 모두 한 절차([`Home::switch`])를 탄다.** 나가는 작업자료의 쓰기 잠금(`BEGIN IMMEDIATE`)을 쥔 채 쪽지를
//! 고치고, 잠금을 놓는다. 확장 호스트의 저장(`ErpCapture::save`)은 같은 잠금을 얻은 뒤 쪽지를 다시 보므로, 저장은
//! "바꾸기 전 옛 자료에" 아니면 "거절" 둘 중 하나다.
//!
//! 바꾼 뒤 창은 스스로를 다시 띄운다. 손잡이를 갈아 끼우지 않는다.

use std::path::{Path, PathBuf};

use pclm_core::storage::{pclm_file, schema};
use pclm_core::{Access, Database, Error, PclmFileError, PclmKind, PclmRole, Result};
use rusqlite::{Connection, OpenFlags, OptionalExtension, Transaction, TransactionBehavior};

use crate::resolve::{
    create_work, dataset_id_of, refusal_message, require_local, require_work, suffixed,
};
use crate::{Home, SystemEnv, dotnet, win};

/// 옮기기의 사본 검사가 세는 표(C# `CountedTables`). 개체 셋과 계획 — 사람이 쌓은 것의 몸통이다.
const COUNTED_TABLES: [&str; 4] = ["request", "notice", "contract", "plan"];

/// 표마다 줄 수. 차례는 [`COUNTED_TABLES`] 다.
type Counts = Vec<(&'static str, i64)>;

/// 전환이 쪽지에 적을 것 — 새 작업자료, 그 신원, 옛 자리(옮기기만).
struct Next {
    workfile: PathBuf,
    dataset_id: String,
    retired: Option<PathBuf>,
}

/// 시험의 끼움 자리(C# `Home.WhileLocked`). 같음·복제는 뜻이 없다 — 늘 같다고 친다.
#[cfg(test)]
#[derive(Clone, Default)]
pub(crate) struct Hook(pub(crate) Option<std::sync::Arc<dyn Fn() + Send + Sync>>);

#[cfg(test)]
impl std::fmt::Debug for Hook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.0.is_some() {
            "Hook(Some)"
        } else {
            "Hook(None)"
        })
    }
}

#[cfg(test)]
impl PartialEq for Hook {
    fn eq(&self, _: &Hook) -> bool {
        true
    }
}

#[cfg(test)]
impl Eq for Hook {}

impl Home {
    /// 작업자료를 다른 자리로 옮긴다(C# `Home.MoveTo`). **같은 자료다** — 신원을 그대로 가져가므로 확장의 열린 검토
    /// 화면이 새 자리로 이어진다. 옛 파일은 `retired` 로 표시되고 다음 시작에 지워진다. 사본은 쓰기 잠금을 쥔 채 뜬다.
    ///
    /// 네트워크 자리·지금 자리와 같음·검사 실패는 [`Error::InvalidOperation`], 그 자리에 이미 파일이 있으면 [`Error::Io`].
    /// 새 자리의 온전한 경로를 낸다.
    pub fn move_to(&self, current: &Database, target: &Path) -> Result<PathBuf> {
        let full = check_new_target(current, target)?;
        let temporary = suffixed(&full, &format!(".tmp-{}", win::new_guid_n()));

        self.switch(
            current,
            |transaction, dataset_id, created| {
                // 읽기 전용 연결의 VACUUM INTO 다. 쥔 것은 쓰기 잠금이라 읽기는 막히지 않고, 커밋된 것까지만 뜬다.
                created.push(temporary.clone());
                create_parent(&temporary)?;
                current.snapshot(&temporary, false)?;

                // 이름을 바꾸기 전에 본다(ADR-031). 셈은 잠금 안에서 본 것과 견준다.
                verify(
                    &temporary,
                    PclmRole::WORK,
                    |id| id == Some(dataset_id),
                    &counts(transaction)?,
                )?;

                win::move_file(&temporary, &full, false)?;
                created.push(full.clone());
                Ok(Next {
                    workfile: full.clone(),
                    dataset_id: dataset_id.to_string(),
                    retired: Some(current.path().to_path_buf()),
                })
            },
            true,
            Vec::new(),
        )?;
        Ok(full)
    }

    /// 있는 다른 작업자료를 그 자리에서 쓴다(C# `Home.UseInPlace`, 「내 계약자료로 사용」). 역할이 `work` 인 것만
    /// 받는다([`Error::PclmFile`]). 지금 작업자료는 그대로 작업자료로 남는다.
    pub fn use_in_place(&self, current: &Database, path: &Path) -> Result<PathBuf> {
        let full = dotnet::full_path(path)?;
        require_local(&full)?;
        if dotnet::same_path(&full.to_string_lossy(), &current.path().to_string_lossy()) {
            return Err(Error::InvalidOperation(format!(
                "지금 이 창에서 쓰고 있는 작업자료입니다: {}",
                full.display()
            )));
        }
        let info = require_work(&full)?;
        let dataset_id = info.dataset_id.unwrap_or_default();
        self.switch(
            current,
            |_, _, _| {
                Ok(Next {
                    workfile: full.clone(),
                    dataset_id,
                    retired: None,
                })
            },
            false,
            Vec::new(),
        )?;
        Ok(full)
    }

    /// 제출본·취합본·백업(무엇이든)의 사본을 떠서 새 작업자료로 삼는다(C# `Home.CreateFromSnapshot`, 「이 파일로 새
    /// 계약자료 만들기」). **원본은 그대로다.** 사본은 **새 신원**을 받는다. 판이 낮으면 사본을 올린다.
    pub fn create_from_snapshot(
        &self,
        current: &Database,
        source: &Path,
        target: &Path,
    ) -> Result<PathBuf> {
        let from = dotnet::full_path(source)?;
        let full = check_new_target(current, target)?;

        let info = pclm_file::inspect(&from);
        if info.kind != PclmKind::Ok {
            let message = refusal_message(&info, &from);
            return Err(Error::PclmFile(PclmFileError {
                path: from,
                kind: info.kind,
                role: info.role,
                message,
            }));
        }

        let temporary = suffixed(&full, &format!(".tmp-{}", win::new_guid_n()));
        let mut created = Vec::new();
        let made = (|| -> Result<String> {
            created.push(temporary.clone());
            let counts = counts_of(&from)?;
            create_parent(&temporary)?;
            Database::new(&from, Access::Read).snapshot(&temporary, false)?;

            // 판이 낮으면 사본을 올린다. 이름표가 있으면 먼저 work 로 고쳐 적는다 — 쓰기 손잡이는 retired 를 열지
            // 않는다. 표지가 생기기 전의 v19 는 이름표가 없어, 판올림이 심은 뒤 아래에서 새 신원을 받는다.
            if info.version < schema::VERSION {
                stamp(&temporary, false)?;
                Database::new(&temporary, Access::Write).migrate(&SystemEnv)?;
            }
            stamp(&temporary, true)?;

            verify(
                &temporary,
                PclmRole::WORK,
                |id| id != info.dataset_id.as_deref(),
                &counts,
            )?;
            let dataset_id = pclm_file::inspect(&temporary)
                .dataset_id
                .unwrap_or_default();

            win::move_file(&temporary, &full, false)?;
            created.push(full.clone());
            Ok(dataset_id)
        })();
        let dataset_id = match made {
            Ok(id) => id,
            Err(e) => {
                discard(&created);
                return Err(e);
            }
        };

        self.switch(
            current,
            |_, _, _| {
                Ok(Next {
                    workfile: full.clone(),
                    dataset_id,
                    retired: None,
                })
            },
            false,
            created,
        )?;
        Ok(full)
    }

    /// 빈 작업자료를 지어 그것으로 바꾼다(C# `Home.CreateNewWhileRunning`, 「새 계약자료」). 지금 작업자료는 그대로 남는다.
    pub fn create_new_while_running(&self, current: &Database, target: &Path) -> Result<PathBuf> {
        let full = check_new_target(current, target)?;
        let mut created = Vec::new();
        let made = (|| -> Result<String> {
            create_work(&full)?;
            created.push(full.clone());
            dataset_id_of(&full)
        })();
        let dataset_id = match made {
            Ok(id) => id,
            Err(e) => {
                discard(&created);
                return Err(e);
            }
        };

        self.switch(
            current,
            |_, _, _| {
                Ok(Next {
                    workfile: full.clone(),
                    dataset_id,
                    retired: None,
                })
            },
            false,
            created,
        )?;
        Ok(full)
    }

    // ── 전환 절차 ────────────────────────────────────────────────

    /// 전환 절차(C# `Home.Switch`, ADR-032). 바꾸는 일은 모두 이것 하나를 탄다.
    ///
    /// ```text
    /// 1. 나가는 작업자료에 연결 A, BEGIN IMMEDIATE      ← 확장 호스트·명령줄의 쓰기가 여기서 줄 선다
    /// 2. (옮기기) A 를 쥔 채 사본을 뜨고 검사한 뒤 제자리 이름으로
    /// 3. 쪽지를 새 작업자료로 쓴다(원자적)             ← 여기를 지나면 되돌리지 않는다
    /// 4. (옮기기) A 안에서 옛 파일을 retired 로
    /// 5. A 커밋 → 줄 서 있던 저장은 잠금을 얻은 뒤 새 쪽지를 보고 거절된다
    /// ```
    ///
    /// 3 앞에서 무엇이 실패하든 이 절차가 지은 파일(`created`)을 지우고 쪽지는 그대로 둔다. 3 을 지난 뒤의 실패는 그대로
    /// 올라가되 지은 파일을 지우지 않는다. 연결 A 를 열거나 잠금을 얻다 실패하면 C# 처럼 지은 파일을 지우지 않는다.
    fn switch(
        &self,
        current: &Database,
        prepare: impl FnOnce(&Transaction<'_>, &str, &mut Vec<PathBuf>) -> Result<Next>,
        retire_current: bool,
        mut created: Vec<PathBuf>,
    ) -> Result<()> {
        let connection = current.open()?;
        let transaction = Transaction::new_unchecked(&connection, TransactionBehavior::Immediate)?;

        let prepared = (|| -> Result<()> {
            // 잠금을 얻은 뒤에 다시 본다. 이 창이 연 뒤로 다른 손이 쪽지를 바꿨으면 이 창의 자료는 이미 작업자료가
            // 아니다 — 거기서 또 바꾸면 쪽지가 두 번 갈려 어느 것이 진짜인지 모른다.
            let (dataset_id, role) = identity(&transaction)?;
            if role != PclmRole::WORK || !self.points_at(current.path(), &dataset_id) {
                return Err(Error::InvalidOperation(format!(
                    "이 창이 연 자료가 더 이상 작업자료가 아닙니다 — 다른 곳에서 바꿨습니다. 프로그램을 다시 실행하세요: {}",
                    current.path().display()
                )));
            }
            let next = prepare(&transaction, &dataset_id, &mut created)?;
            self.write_config(&next.workfile, &next.dataset_id, next.retired.as_deref())?;
            Ok(())
        })();
        if let Err(e) = prepared {
            drop(transaction.rollback());
            discard(&created);
            return Err(e);
        }

        if retire_current {
            transaction
                .execute_batch("UPDATE pclm_file SET role = 'retired' WHERE singleton = 1;")?;
        }

        #[cfg(test)]
        if let Some(hook) = &self.while_locked.0 {
            hook();
        }
        transaction.commit()?;
        // 연결은 여기서 닫힌다 — 남아 있으면 다음 시작이 옛 파일을 지우지 못한다(C# 의 ReleasePool).
        drop(connection);
        Ok(())
    }

    /// 시험이 잠금 안에 끼울 일(C# `Home.WhileLocked = …`).
    #[cfg(test)]
    pub(crate) fn set_while_locked(&mut self, hook: impl Fn() + Send + Sync + 'static) {
        self.while_locked = Hook(Some(std::sync::Arc::new(hook)));
    }
}

/// 지금 시점을 백업 파일로 뜬다(C# `Home.BackupTo`, 「백업 만들기」). 역할은 `backup`, 신원은 새로 — 쪽지는 그대로다.
///
/// 자리는 사람이 저장 대화상자에서 고른 것이라 그 자리의 옛 백업은 덮는다(ADR-031). 다만 **작업자료는 덮지 않는다** —
/// 그 검사는 `pclm_file::snapshot` 이 쥔다. 뜬 자리의 온전한 경로를 낸다.
pub fn backup_to(current: &Database, target: &Path) -> Result<PathBuf> {
    let full = dotnet::full_path(target)?;
    require_local(&full)?;
    create_parent(&full)?;
    pclm_file::snapshot(current, &full, PclmRole::BACKUP, true, &SystemEnv)
}

/// 새 작업자료가 설 자리(C# `Home.CheckNewTarget`). 로컬 디스크이고, 지금 자리가 아니고, **비어 있어야 한다** —
/// `-wal` 만 남은 자리도 비지 않은 것으로 본다. 창은 자리를 고른 직후에도 이것을 부른다. 온전한 경로를 낸다.
pub fn check_new_target(current: &Database, target: &Path) -> Result<PathBuf> {
    let full = dotnet::full_path(target)?;
    require_local(&full)?;
    if dotnet::same_path(&full.to_string_lossy(), &current.path().to_string_lossy()) {
        return Err(Error::InvalidOperation(format!(
            "지금 쓰고 있는 작업자료 자리입니다: {}",
            full.display()
        )));
    }
    if full.is_file() || suffixed(&full, "-wal").is_file() {
        return Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!(
                "그 자리에 이미 파일이 있어 덮지 않습니다 — 다른 이름을 고르세요: {}",
                full.display()
            ),
        )));
    }
    Ok(full)
}

/// C# 의 `Database.Snapshot` 이 짓던 폴더.
fn create_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

/// 사본을 제자리 이름으로 바꾸기 전에 본다(C# `Verify`, ADR-031): PCLM·역할·신원, `quick_check`, 표마다 줄 수.
fn verify(
    path: &Path,
    role: &str,
    identity: impl Fn(Option<&str>) -> bool,
    counts: &Counts,
) -> Result<()> {
    let info = pclm_file::inspect(path);
    if info.kind != PclmKind::Ok
        || info.role.as_deref() != Some(role)
        || !identity(info.dataset_id.as_deref())
    {
        return Err(Error::InvalidOperation(format!(
            "뜬 사본의 이름표가 맞지 않아 쓰지 않습니다: {}",
            path.display()
        )));
    }

    let connection = read_only(path)?;
    let check: Option<rusqlite::types::Value> = connection
        .query_row("PRAGMA quick_check;", [], |r| r.get(0))
        .optional()?;
    if !matches!(check, Some(rusqlite::types::Value::Text(ref t)) if t == "ok") {
        return Err(Error::InvalidOperation(format!(
            "뜬 사본이 온전하지 않아 쓰지 않습니다: {}",
            path.display()
        )));
    }

    let copied = count_tables(&connection)?;
    for ((table, count), (_, now)) in counts.iter().zip(&copied) {
        if count != now {
            return Err(Error::InvalidOperation(format!(
                "뜬 사본의 {table} 줄 수가 원본과 다릅니다({count} → {now}). 쓰지 않습니다: {}",
                path.display()
            )));
        }
    }
    Ok(())
}

fn counts_of(path: &Path) -> Result<Counts> {
    count_tables(&read_only(path)?)
}

fn counts(transaction: &Transaction<'_>) -> Result<Counts> {
    count_tables(transaction)
}

fn count_tables(connection: &Connection) -> Result<Counts> {
    COUNTED_TABLES
        .iter()
        .map(|&table| {
            let n: i64 =
                connection.query_row(&format!("SELECT COUNT(*) FROM \"{table}\";"), [], |r| {
                    r.get(0)
                })?;
            Ok((table, n))
        })
        .collect()
}

fn identity(transaction: &Transaction<'_>) -> Result<(String, String)> {
    transaction
        .query_row(
            "SELECT dataset_id, role FROM pclm_file WHERE singleton = 1;",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .ok_or_else(|| {
            Error::InvalidOperation("작업자료의 이름표(pclm_file)가 비어 있습니다.".into())
        })
}

/// 사본에 작업자료의 이름표를 적는다(C# `Stamp`) — 역할 `work`, **새 신원**, 지은 때는 지금. 판올림이 WAL 로 바꿔
/// 두었으면 한 파일로 되돌린다. `required` 면 이름표가 없을 때 거절한다(판올림 전의 옛 v19 사본은 아직 없다).
///
/// 신원과 시각은 C# 과 같은 SQL(`randomblob`·`strftime('now')`)이 짓는다 — 이 층은 환경 효과를 가진다.
fn stamp(path: &Path, required: bool) -> Result<()> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    connection.busy_timeout(std::time::Duration::from_secs(30))?;
    connection.execute_batch("PRAGMA foreign_keys = ON;")?;
    connection.execute_batch("PRAGMA journal_mode = DELETE;")?;

    let exists: i64 = connection.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'pclm_file';",
        [],
        |r| r.get(0),
    )?;
    if exists == 0 {
        if required {
            return Err(Error::InvalidOperation(format!(
                "뜬 사본에 파일 이름표(pclm_file)가 없습니다: {}",
                path.display()
            )));
        }
        return Ok(());
    }

    connection.execute_batch(
        "UPDATE pclm_file\n   SET role = 'work',\n       dataset_id = lower(hex(randomblob(16))),\n       created_at = strftime('%Y-%m-%dT%H:%M:%f0000Z', 'now')\n WHERE singleton = 1;",
    )?;
    Ok(())
}

/// 읽기 전용 연결. 다 보고 나면 이 파일의 이름을 바꾸므로 곧 닫는다.
fn read_only(path: &Path) -> Result<Connection> {
    Ok(Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?)
}

/// 이 전환이 지은 파일을 지운다(C# `ViewedFile.Discard`). 지은 것만 담겨 오므로 남의 파일을 지울 일이 없다.
fn discard(created: &[PathBuf]) {
    for path in created {
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let _ = std::fs::remove_file(suffixed(path, suffix));
        }
    }
}

//! 시작(C# `Home.Resolve` 무리) — 지금 쓸 작업자료를 정해 연다(ADR-030). 처음 실행·옛 자료 옮겨 오기·판올림 전
//! 백업·옮기기가 남긴 옛 파일 지우기·열람 사본 걷기·시작 화면의 두 동작(파일 찾기·새로 만들기)·로컬 디스크 검사.
//!
//! 글은 C# 과 **바이트까지 같다** — 시작 화면과 명령줄이 그대로 보인다.

use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use pclm_core::erp::parse_json;
use pclm_core::storage::{pclm_file, schema};
use pclm_core::{
    Access, Database, Env, Error, PclmFileError, PclmInfo, PclmKind, PclmRole, Result,
};
use serde_json::Value;

use crate::{ConfigState, Home, SystemEnv, dotnet, win};

/// 시작할 수 없는 까닭의 갈래(C# `ProblemKind`). 시작 화면이 무엇을 권할지 고르는 데 쓴다.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProblemKind {
    /// 쪽지가 깨졌다.
    ConfigCorrupt,
    /// 쪽지가 가리키는 파일이 없다.
    WorkfileMissing,
    /// PCLM 파일이 아니다.
    NotPclm,
    /// 기준선보다 옛 시험판이다.
    TooOld,
    /// 이 프로그램보다 새 판이다.
    TooNew,
    /// 작업자료가 아니다(제출본·취합본·백업·옮겨진 옛 자료).
    NotWork,
}

/// [`Home::resolve`] 의 결과(C# `HomeState`) — 열었거나, 열지 않은 까닭이거나.
#[derive(Debug)]
pub enum HomeState {
    /// 작업자료를 열었다. 판올림까지 마친 쓰기 손잡이다. `notes` 는 오는 길에 있었던 일(옛 자료 옮겨 오기·백업)이다.
    Ready {
        database: Database,
        notes: Vec<String>,
    },
    /// 열지 않았다. **아무 파일도 만들지 않았다.** 사람이 시작 화면에서 고른다.
    Problem {
        kind: ProblemKind,
        path: PathBuf,
        message: String,
    },
}

/// 처음 실행에 작업자료를 짓거나 옛 자료를 옮겨 올 이름(C# `Home.DefaultWorkfileName`).
pub const DEFAULT_WORKFILE_NAME: &str = "계약자료.pclm";

/// 판올림 전 백업을 몇 벌 남기나(C# `Home.BackupsKept`).
pub const BACKUPS_KEPT: usize = 10;

/// 열람 사본을 얼마나 오래 두나(C# `Home.ViewCopiesKept`).
pub const VIEW_COPIES_KEPT: Duration = Duration::from_secs(24 * 60 * 60);

/// 옛 자료의 파일 이름. 처음 실행에 한 번 옮겨 올 때만 본다.
const LEGACY_FILE_NAME: &str = "pclm.db";

/// 작업자료는 로컬 디스크에만 둔다는 까닭(C# `Home.RequireLocal` 의 `why`).
const NOT_LOCAL: &str = "작업자료는 이 컴퓨터의 디스크에 두어야 합니다 — 네트워크 자리에서는 SQLite 의 WAL 이 동작하지 않습니다";

impl Home {
    pub fn backups_directory(&self) -> PathBuf {
        self.directory.join("backups")
    }

    pub fn webview2_directory(&self) -> PathBuf {
        self.directory.join("WebView2")
    }

    pub fn error_log_path(&self) -> PathBuf {
        self.directory.join("error.log")
    }

    /// 창의 몸가짐([`crate::window_prefs`]). 자료가 아니라 이 컴퓨터의 것이라 작업자료 밖, 홈에 둔다.
    pub fn window_prefs_path(&self) -> PathBuf {
        self.directory.join("window.json")
    }

    pub fn default_workfile(&self) -> PathBuf {
        self.directory.join(DEFAULT_WORKFILE_NAME)
    }

    /// 열어 본 파일의 임시 사본이 놓이는 자리(`PclmFile.OpenView`). 이 폴더는 앱의 것이다.
    pub fn view_directory(&self) -> PathBuf {
        self.directory.join("view")
    }

    // ── 시작 ─────────────────────────────────────────────────────

    /// 지금 쓸 작업자료를 정해 연다(C# `Home.Resolve`, ADR-030). **어떤 연결보다 먼저 부른다.**
    ///
    /// 빈 자료가 서는 것은 **쪽지가 없는 처음 실행**뿐이다. 쪽지가 깨졌거나 가리키는 파일이 없거나 작업자료가 아니면
    /// 아무것도 만들지 않고 [`HomeState::Problem`] 을 낸다. 예상한 상태에서는 오류를 내지 않는다 — 판올림 실패 같은
    /// 뜻밖의 일만 그대로 올라간다.
    pub fn resolve(&self) -> Result<HomeState> {
        let read = self.read_config();
        let config = match read.state {
            ConfigState::Missing => return self.first_run(),
            ConfigState::Corrupt => {
                return Ok(HomeState::Problem {
                    kind: ProblemKind::ConfigCorrupt,
                    path: self.config_path(),
                    message: format!(
                        "작업자료를 가리키는 설정 파일을 읽을 수 없습니다({}). \
                         다른 자료를 대신 열지 않습니다 — 작업자료 파일을 찾아 주거나 새로 만드세요.",
                        read.error.unwrap_or_default()
                    ),
                });
            }
            ConfigState::Ok => read.config.expect("Ok 이면 쪽지가 있다"),
        };

        let mut notes = Vec::new();
        let workfile = config.workfile.unwrap_or_default();
        let state = self.open(
            Path::new(&workfile),
            config.dataset_id.as_deref(),
            &mut notes,
            true,
        )?;

        // 옮기기가 남긴 옛 파일은 새 자리가 열린 뒤에만 지운다. 새 자리가 열리지 않으면 옛 파일이 남은 한 벌일 수
        // 있다 — 그때는 쪽지에 그대로 두어 다음 시작이 다시 본다.
        Ok(match (state, config.retired) {
            (
                HomeState::Ready {
                    database,
                    mut notes,
                },
                Some(retired),
            ) => {
                discard_retired(Path::new(&retired), &mut notes);
                self.write_config(database.path(), &dataset_id_held(&database)?, None)?;
                HomeState::Ready { database, notes }
            }
            (state, _) => state,
        })
    }

    /// 처음 실행. 옛 자료가 있으면 사본을 떠 오고, 홈에 이미 작업자료가 있으면 그것을 쓰고, 둘 다 없으면 짓는다.
    fn first_run(&self) -> Result<HomeState> {
        let mut notes = Vec::new();
        let default = self.default_workfile();

        // 임시 규칙: 표지가 생기기 전의 자료(pclm.db)를 한 번 건네받는다. 1.0 전에 걷는다(ADR-031). 원본은 그대로 둔다.
        if !default.is_file() {
            for candidate in self.legacy_candidates() {
                let info = pclm_file::inspect(&candidate);
                if info.kind == PclmKind::NotFound {
                    continue;
                }
                // 옛 자료가 있는데 받을 수 없다. 빈 자료를 새로 세우면 사람은 자료가 사라진 줄 안다.
                if !is_work(&info) {
                    return Ok(refuse(&info, &candidate));
                }
                self.copy_legacy(&candidate)?;
                notes.push(format!(
                    "옛 자료를 옮겨 왔습니다.\n{}\n→ {}\n(원본은 그대로 남아 있습니다)",
                    candidate.display(),
                    default.display()
                ));
                // 원본이 백업 노릇을 하므로 사본을 다시 백업하지 않는다.
                return self.open(&default, None, &mut notes, false);
            }
        }

        if default.is_file() {
            return self.open(&default, None, &mut notes, true);
        }

        // 빈 자료가 서는 두 길 중 하나다(다른 하나는 시작 화면의 「새로 만들기」). ADR-031.
        std::fs::create_dir_all(&self.directory)?;
        let created = pclm_file::create(&default, PclmRole::WORK, &SystemEnv)?;
        self.write_config(created.path(), &dataset_id_held(&created)?, None)?;
        Ok(HomeState::Ready {
            database: created,
            notes,
        })
    }

    /// 옛 자료가 있을 수 있는 자리. 먼저 오는 것이 이긴다.
    fn legacy_candidates(&self) -> Vec<PathBuf> {
        let mut candidates: Vec<PathBuf> = self
            .legacy_roaming_config
            .as_deref()
            .map(read_legacy_folders)
            .unwrap_or_default()
            .into_iter()
            .map(|folder| PathBuf::from(folder).join(LEGACY_FILE_NAME))
            .collect();
        // 업무 홈의 기본 자리이자 옛 개발 DB(Pclm.Erp.Dev\pclm.db)의 자리다.
        candidates.push(self.directory.join(LEGACY_FILE_NAME));
        candidates
    }

    /// 옛 자료를 홈의 작업자료 자리에 한 벌 뜬다. **임시 이름으로 뜬 뒤 제자리 이름으로 바꾼다**(ADR-031).
    /// `PclmFile.Snapshot` 이 아니라 `Database.Snapshot` 이다 — 옮겨 오기라 신원(`dataset_id`)을 이어 간다.
    fn copy_legacy(&self, source: &Path) -> Result<()> {
        let default = self.default_workfile();
        let temporary = suffixed(&default, &format!(".{}.tmp", win::new_guid_n()));
        let result = (|| -> Result<()> {
            // C# 의 Database.Snapshot 은 뜰 자리의 폴더를 짓는다.
            std::fs::create_dir_all(&self.directory)?;
            Database::new(source, Access::Read).snapshot(&temporary, false)?;
            win::move_file(&temporary, &default, false)?;
            Ok(())
        })();
        if temporary.is_file() {
            let _ = std::fs::remove_file(&temporary);
        }
        result
    }

    /// 작업자료로 연다. 판이 낮으면 백업을 뜨고 올린다. 쪽지의 신원이 파일과 다르면 쪽지를 고친다.
    fn open(
        &self,
        path: &Path,
        config_dataset_id: Option<&str>,
        notes: &mut Vec<String>,
        backup: bool,
    ) -> Result<HomeState> {
        let full = dotnet::full_path(path)?;
        let info = pclm_file::inspect(&full);
        if !is_work(&info) {
            return Ok(refuse(&info, &full));
        }

        if backup && info.version < schema::VERSION {
            let saved = self.backup(&full, info.version)?;
            notes.push(format!(
                "판을 올리기 전에 백업을 떴습니다(v{} → v{}).\n{}",
                info.version,
                schema::VERSION,
                saved.display()
            ));
        }

        let database = pclm_file::open_work(&full)?;
        database.migrate(&SystemEnv)?;
        let dataset_id = dataset_id_held(&database)?;

        // 쪽지의 신원과 다르면 그 자리에 다른 작업자료가 놓인 것이다. 쪽지를 파일의 것으로 고친다 — 확장 호스트는
        // 쪽지의 신원으로 대상을 확인하므로, 이로써 옛 자료를 보고 연 검토 화면의 저장이 거절된다.
        let pointed = self
            .read_config()
            .config
            .and_then(|c| c.workfile)
            .is_some_and(|w| dotnet::same_path(&w, &full.to_string_lossy()));
        if config_dataset_id != Some(dataset_id.as_str()) || !pointed {
            self.write_config(&full, &dataset_id, None)?;
        }
        Ok(HomeState::Ready {
            database,
            notes: std::mem::take(notes),
        })
    }

    /// 판올림 전 백업(ADR-033). 최근 [`BACKUPS_KEPT`] 벌만 남긴다. 뜬 자리를 낸다.
    fn backup(&self, path: &Path, version: i32) -> Result<PathBuf> {
        let folder = self.backups_directory();
        std::fs::create_dir_all(&folder)?;

        let now = win::local_now();
        let stamp = format!(
            "{:04}{:02}{:02}-{:02}{:02}{:02}",
            now.year, now.month, now.day, now.hour, now.minute, now.second
        );
        let mut target = folder.join(format!("계약자료_v{version}_{stamp}.pclm"));
        let mut n = 2;
        while target.is_file() {
            target = folder.join(format!("계약자료_v{version}_{stamp}-{n}.pclm"));
            n += 1;
        }

        // 표지가 생기기 전의 v19 에는 pclm_file 이 없어 역할을 적을 자리가 없다 — 날것 그대로 뜬다.
        let source = Database::new(path, Access::Read);
        let saved = if version == schema::BASELINE_VERSION {
            source.snapshot(&target, false)?
        } else {
            pclm_file::snapshot(&source, &target, PclmRole::BACKUP, false, &SystemEnv)?
        };

        self.prune();
        Ok(saved)
    }

    /// 오래된 백업을 지운다. 우리가 지은 이름(`계약자료_v*_*.pclm`)의 것만 — 사람이 거기 둔 다른 파일은 건드리지 않는다.
    /// 차례는 C# 과 같다 — 고친 시각(UTC)이 늦은 것부터, 같으면 이름의 서수 역순.
    fn prune(&self) {
        let Ok(entries) = std::fs::read_dir(self.backups_directory()) else {
            return;
        };
        let mut files: Vec<(SystemTime, String, PathBuf)> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                let modified = e.metadata().ok()?.modified().ok()?;
                is_backup_name(&name).then(|| (modified, name, e.path()))
            })
            .collect();
        files.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then_with(|| b.1.encode_utf16().cmp(a.1.encode_utf16()))
        });
        for (_, _, path) in files.into_iter().skip(BACKUPS_KEPT) {
            // 못 지우면 다음 판올림에서 다시 지운다.
            let _ = std::fs::remove_file(path);
        }
    }

    // ── 열람 ─────────────────────────────────────────────────────

    /// 쪽지가 지금 가리키는 작업자료가 이 파일인가(C# `Home.IsActiveWorkfile`). 쪽지가 없거나 깨졌으면 아니다.
    pub fn is_active_workfile(&self, path: &Path) -> bool {
        let read = self.read_config();
        read.state == ConfigState::Ok
            && read
                .config
                .and_then(|c| c.workfile)
                .is_some_and(|w| dotnet::same_path(&w, &path.to_string_lossy()))
    }

    /// 남은 열람 사본을 걷는다(C# `Home.PruneViews`). [`VIEW_COPIES_KEPT`] 보다 오래된 것만 지운다 — 지금 떠 있는 다른
    /// 열람 창의 사본을 걷으면 그 창이 읽던 자리가 사라진다. 열린 채인 파일은 지워지지 않아 그대로 넘어간다.
    pub fn prune_views(&self) {
        let Ok(entries) = std::fs::read_dir(self.view_directory()) else {
            return;
        };
        let cutoff = SystemTime::now() - VIEW_COPIES_KEPT;
        for entry in entries.filter_map(|e| e.ok()) {
            if !entry.file_type().is_ok_and(|t| t.is_file()) {
                continue;
            }
            let Some(modified) = entry.metadata().ok().and_then(|m| m.modified().ok()) else {
                continue;
            };
            if modified >= cutoff {
                continue;
            }
            let _ = std::fs::remove_file(entry.path());
        }
    }

    /// 남의 파일을 열어 본다(C# `PclmFile.OpenView(file, home.ViewDirectory)`, ADR-031) — 원본을 온전한 경로로 고치고, 받을
    /// 파일이면 열람 폴더를 지어 그 안의 `열람-<id>.pclm` 에 뜬다. 받지 않는 파일이면 폴더를 짓지 않고 거절한다(C# 은 검사 뒤에
    /// 폴더를 짓는다). 창의 열람 창과 명령줄의 `--file` 이 함께 쓴다.
    pub fn open_view(&self, file: &Path, env: &dyn Env) -> Result<pclm_file::ViewedFile> {
        let full = dotnet::full_path(file)?;
        let directory = self.view_directory();
        if pclm_file::inspect(&full).kind == PclmKind::Ok {
            std::fs::create_dir_all(&directory)?;
        }
        let copy = directory.join(format!("열람-{}.pclm", env.new_id()));
        pclm_file::open_view(&full, &copy, env)
    }

    // ── 시작 화면의 두 동작 ───────────────────────────────────────

    /// 있는 작업자료를 고른다(C# `Home.Adopt`, 시작 화면 「파일 찾기」). PCLM 이고 역할이 `work` 인 것만 받는다.
    /// 네트워크 자리는 [`Error::InvalidOperation`], 받을 수 없는 파일은 [`Error::PclmFile`].
    pub fn adopt(&self, path: &Path) -> Result<()> {
        let full = dotnet::full_path(path)?;
        require_local(&full)?;
        let info = require_work(&full)?;
        self.write_config(&full, info.dataset_id.as_deref().unwrap_or_default(), None)?;
        Ok(())
    }

    /// 새 작업자료를 짓는다(C# `Home.CreateNew`, 시작 화면 「새로 만들기」). 그 자리에 파일이 있으면 덮지 않고
    /// 실패한다([`Error::Io`]). 지은 파일의 온전한 경로를 낸다.
    pub fn create_new(&self, path: &Path) -> Result<PathBuf> {
        let full = dotnet::full_path(path)?;
        require_local(&full)?;
        let database = create_work(&full)?;
        self.write_config(database.path(), &dataset_id_held(&database)?, None)?;
        Ok(database.path().to_path_buf())
    }
}

/// 작업자료는 로컬 디스크에만 둔다(C# `Home.RequireLocal`, ADR-006). UNC(`\\server`·`\\?\UNC\`)와 네트워크 드라이브는
/// [`Error::InvalidOperation`]. `\\?\C:\` 처럼 드라이브 문자로 볼 수 없는 뿌리는 C# 의 `DriveInfo` 처럼 보지 않는다.
pub fn require_local(path: &Path) -> Result<()> {
    let full = dotnet::full_path(path)?;
    let text = full.to_string_lossy();
    let refused = || Error::InvalidOperation(format!("{NOT_LOCAL}: {text}"));

    let unc = text.starts_with(r"\\") && !text.starts_with(r"\\?\");
    let device_unc = text
        .get(..8)
        .is_some_and(|head| dotnet::eq_ignore_case(head, r"\\?\UNC\"));
    if unc || device_unc {
        return Err(refused());
    }

    // Path.GetPathRoot 이 `X:\` 인 것만 DriveInfo 가 받는다. `X:` 뒤에 구분자가 없는 꼴은 온전한 경로에서 나오지 않는다.
    let bytes = text.as_bytes();
    if bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'\\' | b'/')
        && win::is_remote_drive(&format!("{}:\\", bytes[0] as char))
    {
        return Err(refused());
    }
    Ok(())
}

/// 작업자료로 쓸 수 있는 파일인가(C# `Home.RequireWork`). 아니면 시작 화면 「파일 찾기」 와 같은 말로 거절한다.
pub(crate) fn require_work(full: &Path) -> Result<PclmInfo> {
    let info = pclm_file::inspect(full);
    if is_work(&info) {
        return Ok(info);
    }
    let message = if info.kind == PclmKind::Ok {
        format!(
            "{} 파일은 그 자리에서 작업자료로 쓸 수 없습니다 — 그때의 기록이라 고치지 않습니다. \
             일하려면 사본을 떠서 새 작업자료로 삼으세요: {}",
            PclmRole::name(info.role.as_deref()),
            full.display()
        )
    } else {
        refusal_message(&info, full)
    };
    Err(Error::PclmFile(PclmFileError {
        path: full.to_path_buf(),
        kind: info.kind,
        role: info.role,
        message,
    }))
}

/// 시작하지 않는 까닭(C# `Home.Refuse`). 없는 파일의 글은 `PclmFile.Why` 와 다르다 — 쪽지가 가리키던 파일이다.
pub(crate) fn refuse(info: &PclmInfo, path: &Path) -> HomeState {
    let kind = match info.kind {
        PclmKind::NotFound => ProblemKind::WorkfileMissing,
        PclmKind::NotPclm => ProblemKind::NotPclm,
        PclmKind::TooOld => ProblemKind::TooOld,
        PclmKind::TooNew => ProblemKind::TooNew,
        PclmKind::Ok => ProblemKind::NotWork,
    };
    HomeState::Problem {
        kind,
        path: path.to_path_buf(),
        message: refusal_message(info, path),
    }
}

pub(crate) fn refusal_message(info: &PclmInfo, path: &Path) -> String {
    match info.kind {
        PclmKind::NotFound => format!(
            "작업자료 파일이 없습니다. 지워졌거나 옮겨졌거나 이름이 바뀌었을 수 있습니다: {}",
            path.display()
        ),
        _ => pclm_file::why(info, path),
    }
}

pub(crate) fn is_work(info: &PclmInfo) -> bool {
    info.kind == PclmKind::Ok && info.role.as_deref() == Some(PclmRole::WORK)
}

/// C# `Home.DatasetIdOf` — 파일의 신원. 못 읽으면 [`Error::InvalidOperation`].
pub(crate) fn dataset_id_of(path: &Path) -> Result<String> {
    pclm_file::inspect(path).dataset_id.ok_or_else(|| {
        Error::InvalidOperation(format!(
            "작업자료의 신원(dataset_id)을 읽지 못했습니다: {}",
            path.display()
        ))
    })
}

/// 작업자료의 신원을 쓰기 연결을 쥔 채 읽는다(C# `DatasetIdOf` 를 부르는 자리). C# 은 연결 풀이 앞선 쓰기 연결(판올림·
/// 짓기)을 쥐고 있어 읽기 전용 `Inspect` 가 먼저 닫히고, 쓰기 연결이 마지막에 닫히며 `-wal`·`-shm` 을 걷는다. 풀이 없는
/// Rust 에서 읽기 전용 연결이 마지막이면 빈 `-wal`·`-shm` 이 남아, 시작한 홈의 파일이 C# 과 갈린다.
pub(crate) fn dataset_id_held(database: &Database) -> Result<String> {
    let _writer = database.open()?;
    dataset_id_of(database.path())
}

/// C# `PclmFile.Create(full, Work)` — 폴더를 지은 뒤 짓는다(Core 는 폴더를 짓지 않는다).
pub(crate) fn create_work(full: &Path) -> Result<Database> {
    if let Some(parent) = full.parent().filter(|p| !p.as_os_str().is_empty()) {
        // C# 은 그 자리에 파일이 있는지 먼저 본다. 있으면 폴더도 이미 있으니 차례가 갈려도 남는 것은 같다.
        if !(full.exists() || suffixed(full, "-wal").exists()) {
            std::fs::create_dir_all(parent)?;
        }
    }
    pclm_file::create(full, PclmRole::WORK, &SystemEnv)
}

/// 경로 뒤에 글을 붙인다(`-wal`·`.tmp`).
pub(crate) fn suffixed(path: &Path, suffix: &str) -> PathBuf {
    let mut text = path.as_os_str().to_os_string();
    text.push(suffix);
    PathBuf::from(text)
}

/// 옮기기 전 자리의 옛 파일을 지운다(C# `Home.DiscardRetired`). **역할이 `retired` 인 PCLM 일 때만** — 그 사이 사람이
/// 그 자리에 다른 자료를 놓았으면 그것은 남의 것이다. 못 지우면 둔다.
fn discard_retired(path: &Path, notes: &mut Vec<String>) {
    let info = pclm_file::inspect(path);
    if info.kind != PclmKind::Ok || info.role.as_deref() != Some(PclmRole::RETIRED) {
        return;
    }
    // 본체를 먼저 지운다. 본체가 남았는데 -wal 만 사라지면 커밋된 retired 표시를 잃을 수 있다. File.Delete 는 없는
    // 파일에 실패하지 않는다.
    let delete = |p: &Path| match std::fs::remove_file(p) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    };
    if delete(path).is_err() {
        return;
    }
    for suffix in ["-wal", "-shm"] {
        if delete(&suffixed(path, suffix)).is_err() {
            return;
        }
    }
    notes.push(format!(
        "옮기기 전 자리의 옛 파일을 지웠습니다: {}",
        path.display()
    ));
}

/// 옛 쪽지가 가리키는 폴더(C# `Home.ReadLegacyFolders`). 이사가 밀려 있었으면(`pendingMoveFrom`) 자료는 아직 옛 자리에
/// 있으므로 그것이 먼저다. 없거나 깨졌으면 아무것도 내지 않는다.
fn read_legacy_folders(path: &Path) -> Vec<String> {
    if !path.is_file() {
        return Vec::new();
    }
    // JsonDocument.Parse — 같은 이름이 둘이면 TryGetProperty 는 뒤의 것을 본다(serde_json 도 뒤의 값을 남긴다).
    let Some(Value::Object(root)) = dotnet::read_all_text(path)
        .ok()
        .and_then(|t| parse_json(&t))
    else {
        return Vec::new();
    };
    ["pendingMoveFrom", "dataDir"]
        .into_iter()
        .filter_map(|key| match root.get(key) {
            Some(Value::String(folder)) if dotnet::is_path_fully_qualified(folder) => {
                Some(folder.clone())
            }
            _ => None,
        })
        .collect()
}

/// `DirectoryInfo.GetFiles("계약자료_v*_*.pclm")` 의 이름 거르기 — Windows 의 이름 견주기라 대소문자를 가리지 않는다.
fn is_backup_name(name: &str) -> bool {
    let upper = dotnet::to_upper_invariant(name);
    let Some(rest) = upper.strip_prefix(&dotnet::to_upper_invariant("계약자료_v")) else {
        return false;
    };
    let Some(rest) = rest.strip_suffix(".PCLM") else {
        return false;
    };
    rest.contains('_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_names_follow_the_wildcard() {
        for name in [
            "계약자료_v19_20260101-000000.pclm",
            "계약자료_V31_x-2.PCLM",
            "계약자료_v_.pclm",
        ] {
            assert!(is_backup_name(name), "{name}");
        }
        for name in [
            "계약자료_v19.pclm",
            "메모.txt",
            "계약자료_v19_1.pclm.bak",
            "x계약자료_v19_1.pclm",
        ] {
            assert!(!is_backup_name(name), "{name}");
        }
    }
}

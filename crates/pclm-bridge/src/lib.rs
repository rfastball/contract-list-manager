//! 화면과 앱 사이의 통로 — 창(Tauri·WebView) 없는 라이브러리(#8 ⑤ · ⑥ · ⑧a).
//!
//! 화면은 `{id, method, args}` 를 보내고 여기서 `{id, ok, result|error}` 로 답한다. [`Bridge::invoke`] 가 요청 하나를 풀고,
//! [`reply_json`] 이 답 글을 짓는다(camelCase · null 도 적는다 ·
//! `bridge_json` 의 이스케이프). [`Bridge::handle`] 은 글 한 통을 풀어 답을 보내고,
//! **보낸 뒤에** 다시 띄우기·닫기를 창에 넘긴다.
//!
//! 받는 요청은 [`METHODS`] 가 전부다. 열람 문지기([`WRITES`] · [`VIEWER_ALLOWED`])가 열람 중의 쓰기를 막고, 모르는 요청은
//! 거절한다.
//!
//! **창이 다는 것은 [`Host`] 하나다**(고르기 창 · 다시 띄우기 · 닫기 · 알림 영역). 운영체제에 곧바로 하는 일(등록부 ·
//! 탐색기 · 새 프로세스 · 브라우저 창)은 창에 맡기지 않고 여기서 한다 — 손만 [`Os`] 로 갈아 끼울 수 있게 했다
//! (시험·대조는 진짜 HKCU 에 닿지 않는다). 창(⑧b)은 [`Host`] 를 구현하고, 시작할 때 [`refresh_installation`] 을 부르고,
//! 화면이 보낸 글을 [`Bridge::handle`] 에 넘기기만 한다.

mod autostart;
mod bundled;
mod edit;
mod erp;
mod files;
mod host;
mod os;
mod os_windows;
pub mod templates;
mod window;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use pclm_core::local_time::{LocalDateTime, LocalZone, parse_to_local};
use pclm_core::storage::outline::Outline;
use pclm_core::storage::pclm_file::{PclmRole, ViewedFile};
use pclm_core::storage::reports::Reports;
use pclm_core::storage::store::{EntityRef, Store, bridge_json, error_message};
use pclm_core::storage::views;
use pclm_core::{Access, Database, Env};
use pclm_excel::SheetSnapshot;
use pclm_home::Home;
use rusqlite::Connection;
use serde_json::{Map, Value};

pub use autostart::BACKGROUND_FLAG;
pub use bundled::{EMBEDDED_FILES, Prepared, embedded_version};
pub use host::{Host, NoHost, OpenPrompt, SavePrompt};
pub use os::{Os, RegistryRoot, RegistryValue, SpecialFolder};
pub use os_windows::WindowsOs;
pub use window::refresh_installation;

/// 자료를 고치는 요청. 열람 중이면 SQLite 에 닿기 전에 사람 말로 거절한다(ADR-031 셋째 겹).
pub const WRITES: &[&str] = &[
    "addColumn",
    "updateColumn",
    "removeColumn",
    "moveColumn",
    "saveComputedColumn",
    "freezeColumn",
    "saveUserView",
    "deleteUserView",
    "resetDocumentView",
    "pickViewPackage",
    "importViewPackage",
    "setField",
    "setOverride",
    "batchEdit",
    "batchDeletionPlan",
    "batchDelete",
    "prepareExtension",
    "openExtensionSetup",
    "erpTools",
    "importShot",
    "mirrorCommand",
    "pickWorkfile",
    "moveWorkfile",
    "switchWorkfile",
    "newWorkfile",
    "backupWorkfile",
    "windowPrefs",
    "saveWindowPrefs",
    "autoCaptureSettings",
    "saveAutoCaptureSettings",
];

/// 열람 중에도 받는 요청. 둘 중 어디에도 없는 요청은 열람 중에 거절된다(막히는 쪽으로 틀린다).
pub const VIEWER_ALLOWED: &[&str] = &[
    "session",
    "status",
    "dashboard",
    "dataVersion",
    "sheet",
    "outline",
    "userColumns",
    "columnContext",
    "traceComputedColumn",
    "compileComputedColumn",
    "validateComputedSql",
    "planFreezeColumn",
    "userViews",
    "userViewContext",
    "traceUserViewColumn",
    "userViewSamples",
    "previewUserView",
    "exportViewPackage",
    "extensionStatus",
    "captures",
    "mirror",
    "export",
    "exportSheet",
    "dataLocation",
    "revealDataFolder",
    "about",
    "openOther",
    "setUnsaved",
    "closeWindow",
];

/// 다리가 아는 요청 전부. 여기 없는 요청은 「모르는 요청」 이다.
pub const METHODS: &[&str] = &[
    "session",
    "openOther",
    "prepareExtension",
    "extensionStatus",
    "captures",
    "openExtensionSetup",
    "erpTools",
    "mirror",
    "autoCaptureSettings",
    "saveAutoCaptureSettings",
    "mirrorCommand",
    "importShot",
    "status",
    "dashboard",
    "dataVersion",
    "sheet",
    "outline",
    "userColumns",
    "columnContext",
    "traceComputedColumn",
    "compileComputedColumn",
    "validateComputedSql",
    "planFreezeColumn",
    "freezeColumn",
    "userViews",
    "userViewContext",
    "traceUserViewColumn",
    "userViewSamples",
    "previewUserView",
    "saveUserView",
    "exportViewPackage",
    "pickViewPackage",
    "importViewPackage",
    "deleteUserView",
    "resetDocumentView",
    "saveComputedColumn",
    "addColumn",
    "updateColumn",
    "removeColumn",
    "moveColumn",
    "setField",
    "setOverride",
    "batchEdit",
    "batchDeletionPlan",
    "batchDelete",
    "export",
    "exportSheet",
    "dataLocation",
    "pickWorkfile",
    "moveWorkfile",
    "switchWorkfile",
    "newWorkfile",
    "backupWorkfile",
    "revealDataFolder",
    "about",
    "windowPrefs",
    "saveWindowPrefs",
    "setUnsaved",
    "closeWindow",
];

/// 창이 화면에 먼저 보내는 알림 — 저장하지 않은 편집이 있는데 닫기·끝내기를 눌렀다([`Bridge::ask_before_closing`]).
pub const CLOSE_REQUESTED: &str = "closeRequested";

static NO_HOST: NoHost = NoHost;
static WINDOWS_OS: WindowsOs = WindowsOs;

/// 파일 하나(작업자료 또는 열람 사본) 위의 요청 풀이.
///
/// 여러 실마리에서 불려도 된다(요청마다 따로 돈다). 그래서 시계·시간대·창·운영체제는 `Sync` 다.
pub struct Bridge<'e> {
    database: Database,
    home: Home,
    viewing: Option<ViewedFile>,
    env: &'e (dyn Env + Sync),
    /// 「지금」을 읽는 시간대 — 파일 이름에 박는 날짜, 수집 기록의 「오늘」.
    zone: &'e (dyn LocalZone + Sync),
    host: &'e dyn Host,
    os: &'e dyn Os,
    /// 업무 홈의 창인가. 창이 `home.is_default()` 로 단다. 아니면 등록부를 고치지 않는다(내장 확장 ·
    /// 자동 실행).
    is_default_home: bool,
    /// 화면을 딴 데서 열고 있으면 그 자리(`--ui`). 열람 창에도 넘긴다.
    ui_origin: Option<String>,
    /// `data_version` 은 같은 연결에서 읽은 값끼리만 견줄 수 있다 — 다리 하나에 읽기 전용 연결 하나를 쥔다.
    /// 놓을 때 차례를 정하려고 `Option` 에 둔다([`Drop`]).
    changes: Mutex<Option<Connection>>,
    /// 답을 보낸 뒤 다시 띄울 것인가 · 닫을 것인가. 한 번만 넘기게 바꿔 쥔다.
    restart: AtomicBool,
    close: AtomicBool,
    /// 화면에 저장하지 않은 편집이 있는가.
    unsaved: AtomicBool,
    mirror: Mutex<erp::MirrorState>,
    auto: Arc<erp::AutoShared>,
    auto_thread: Mutex<Option<JoinHandle<()>>>,
}

/// 놓을 때 자동 수집을 멈추고 `changes` 를 닫는다. 연결 풀이 없어 읽기 전용 연결이 마지막에 닫히면 빈 곁파일(`-wal`·`-shm`)이
/// 남는다 — 쓰기 손잡이면 쓰기 연결을 쥔 채 닫아 곁파일을 걷는다. 그리고 열람 사본([`ViewedFile`])은 이 연결이 닫힌 **뒤에** 걷힌다(필드는 이 함수 다음에 놓인다).
/// 열린 연결이 있으면 Windows 는 사본을 지우지 못해 홈의 `view` 에 남는다.
impl Drop for Bridge<'_> {
    fn drop(&mut self) {
        self.auto.stop();
        if let Some(thread) = self
            .auto_thread
            .get_mut()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            let _ = thread.join();
        }
        let changes = self
            .changes
            .get_mut()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        let writer = if self.database.access() == Access::Write {
            self.database.open().ok()
        } else {
            None
        };
        drop(changes);
        drop(writer);
    }
}

impl<'e> Bridge<'e> {
    /// 세울 때 `data_version` 의 읽기 전용 연결을 연다 — 못 열면 그 글로 거절.
    ///
    /// 「지금」 은 `env` 의 지금을 `zone` 으로 읽는다. 창은 달지 않은 것([`NoHost`]), 운영체제는 진짜다.
    pub fn new(
        database: Database,
        home: Home,
        viewing: Option<ViewedFile>,
        env: &'e (dyn Env + Sync),
        zone: &'e (dyn LocalZone + Sync),
    ) -> Result<Bridge<'e>, String> {
        let changes = database.open_read_only().map_err(|e| error_message(&e))?;
        Ok(Bridge {
            database,
            home,
            viewing,
            env,
            zone,
            host: &NO_HOST,
            os: &WINDOWS_OS,
            is_default_home: false,
            ui_origin: None,
            changes: Mutex::new(Some(changes)),
            restart: AtomicBool::new(false),
            close: AtomicBool::new(false),
            unsaved: AtomicBool::new(false),
            mirror: Mutex::new(erp::MirrorState::default()),
            auto: Arc::new(erp::AutoShared::default()),
            auto_thread: Mutex::new(None),
        })
    }

    /// 창을 단다 — 고르기 창 · 다시 띄우기 · 닫기 · 알림 영역 두기.
    pub fn set_host(&mut self, host: &'e dyn Host) {
        self.host = host;
    }

    /// 운영체제의 손을 갈아 끼운다 — 시험·대조만 쓴다.
    pub fn set_os(&mut self, os: &'e dyn Os) {
        self.os = os;
    }

    /// 창은 `home.is_default()` 를 넘긴다.
    pub fn set_default_home(&mut self, is_default_home: bool) {
        self.is_default_home = is_default_home;
    }

    /// 화면을 딴 데서 열고 있으면 그 자리를 건넨다.
    pub fn set_ui_origin(&mut self, origin: Option<String>) {
        self.ui_origin = origin;
    }

    /// 열람 창인가.
    pub fn read_only(&self) -> bool {
        self.viewing.is_some()
    }

    /// 이 다리가 선 홈.
    pub fn home(&self) -> &Home {
        &self.home
    }

    /// 화면에 저장하지 않은 편집이 있는가.
    pub fn unsaved(&self) -> bool {
        self.unsaved.load(Ordering::SeqCst)
    }

    /// 닫기 전에 묻는 일 중 다리의 몫 — 저장하지 않은 편집이 있으면 화면에 보낼 「닫기 요청」 알림 글을 낸다. 창은 그것을
    /// 보내고(창을 앞으로 내어) 닫기·끝내기를 멈춘다. 없으면 `None` — 그대로 닫는다.
    pub fn ask_before_closing(&self) -> Option<String> {
        self.unsaved().then(|| notice_json(CLOSE_REQUESTED))
    }

    /// 화면이 죽었을 때 — 화면이 죽으면 「닫기 요청」 에 답할 쪽이 없다. 깃발이 선 채 남으면 닫기를 눌러도 창이
    /// 영영 닫히지 않으므로 내린다.
    pub fn page_failed(&self) {
        self.unsaved.store(false, Ordering::SeqCst);
    }

    /// 화면이 보낸 글 한 통을 풀어 답하고(`post`), **답을 보낸 뒤에** 다시 띄우기·닫기를 창에 넘긴다.
    ///
    /// 인자는 이렇게 편다 — 글은 글, null 은 null, 그 밖의 값은 JSON 글(다시 지은 짧은 글).
    pub fn handle(&self, message: &str, post: &mut dyn FnMut(String)) {
        let (id, outcome) = match parse_message(message) {
            Ok((id, method, args)) => (id, self.invoke(&method, &args)),
            Err((id, error)) => (id, Err(error)),
        };
        post(match outcome {
            Ok(result) => reply_json(id, true, Some(&result), None),
            Err(error) => reply_json(id, false, None, Some(&error)),
        });
        if self.take_restart() {
            self.host.restart();
        }
        if self.take_close() {
            self.host.close_window();
        }
    }

    /// 다시 띄울 깃발을 거두어 온다 — 거둔 쪽에서만 참이다.
    pub fn take_restart(&self) -> bool {
        self.restart.swap(false, Ordering::SeqCst)
    }

    /// 닫을 깃발을 거두어 온다.
    pub fn take_close(&self) -> bool {
        self.close.swap(false, Ordering::SeqCst)
    }

    /// 열람 문지기를 지난 뒤 요청을 푼다. 실패는 오류의 글 그대로다.
    pub fn invoke(&self, method: &str, args: &[Option<String>]) -> Result<Value, String> {
        // 고치는 요청은 물론이고, 어느 쪽으로도 갈라 두지 않은 요청도 열람 중에는 받지 않는다.
        if let Some(viewing) = &self.viewing
            && (WRITES.contains(&method) || !VIEWER_ALLOWED.contains(&method))
        {
            return Err(format!(
                "열람 중인 자료라 고칠 수 없습니다: {} 「{}」",
                PclmRole::name(Some(viewing.role())),
                file_name(viewing.source_path())
            ));
        }
        self.dispatch(method, args)
    }

    fn dispatch(&self, method: &str, args: &[Option<String>]) -> Result<Value, String> {
        let a = |i: usize| require(args, i);
        let o = |i: usize| optional(args, i);
        match method {
            "session" => Ok(self.session()),
            "openOther" => self.open_other(),
            "prepareExtension" => self.prepare_extension(),
            "extensionStatus" => self.read_extension_status(),
            "captures" => self.captures(),
            "openExtensionSetup" => self.open_extension_setup(a(0)?),
            "erpTools" => self.erp_tools(a(0)?, a(1)?),
            "mirror" => self.read_mirror(),
            "autoCaptureSettings" => Ok(self.read_auto_capture_settings()),
            "saveAutoCaptureSettings" => self.save_auto_capture_settings(a(0)?),
            "mirrorCommand" => self.mirror_command(a(0)?, a(1)?, a(2)?),
            "importShot" => self.import_shot(a(0)?, a(1)?, a(2)?, a(3)?, a(4)?),
            "status" => Ok(Reports::new(self.database.clone())
                .summary()
                .map_err(core)?
                .to_json()),
            // 시계는 부르는 쪽(Env)에서 받는다.
            "dashboard" => Ok(Reports::new(self.database.clone())
                .dashboard(&self.env.now_utc())
                .map_err(core)?
                .to_json()),
            "dataVersion" => self.data_version().map(Value::from),
            "sheet" => self.read_sheet(a(0)?),
            "outline" => Ok(Outline::new(self.database.clone())
                .build()
                .map_err(core)?
                .to_json()),
            "userColumns" => self.user_columns(o(0)),
            "columnContext" => self.store().column_context(a(0)?).map_err(core),
            "traceComputedColumn" => self
                .store()
                .trace_computed_column(a(0)?, a(1)?)
                .map_err(core),
            "compileComputedColumn" => self
                .store()
                .compile_computed_column(a(0)?, a(1)?)
                .map(Value::String)
                .map_err(core),
            "validateComputedSql" => self
                .store()
                .validate_computed_sql(a(0)?, a(1)?)
                .map_err(core),
            "planFreezeColumn" => self
                .store()
                .plan_freeze_column(a(0)?, a(1)?)
                .map(|p| p.to_json())
                .map_err(core),
            "freezeColumn" => self.freeze_column(a(0)?, a(1)?, o(2)),
            "userViews" => self.user_views(),
            "userViewContext" => self.store().user_view_context().map_err(core),
            "traceUserViewColumn" => {
                let json = a(0)?;
                let keys = edit::sample_keys(o(1))?;
                self.store()
                    .trace_user_view_column(json, keys.as_deref())
                    .map_err(core)
            }
            "userViewSamples" => self
                .store()
                .user_view_samples(a(0)?, o(1), o(2))
                .map(Value::Array)
                .map_err(core),
            "previewUserView" => self.store().preview_user_view(a(0)?, o(1)).map_err(core),
            "saveUserView" => self.save_user_view(a(0)?, o(1)),
            "exportViewPackage" => self.export_view_package(a(0)?),
            "pickViewPackage" => self.pick_view_package(),
            "importViewPackage" => self.import_view_package(a(0)?, a(1)?),
            "deleteUserView" => self.delete_user_view(a(0)?),
            "resetDocumentView" => self.reset_document_view(),
            "saveComputedColumn" => self.save_computed_column(a(0)?, a(1)?, a(2)?, o(3)),
            "addColumn" => self.add_column(a(0)?, a(1)?, a(2)?, o(3)),
            "updateColumn" => self.update_column(a(0)?, a(1)?, a(2)?, a(3)?, o(4)),
            "removeColumn" => self.remove_column(a(0)?, a(1)?),
            "moveColumn" => self.move_column(a(0)?, a(1)?, a(2)?),
            "setField" => self.set_field(a(0)?, a(1)?, o(2)),
            "setOverride" => self.set_override(a(0)?, a(1)?, o(2)),
            "batchEdit" => {
                let view = a(0)?;
                let changes = edit::read_changes(a(1)?)?;
                let changed = self.store().batch_edit(view, &changes).map_err(core)?;
                Ok(object([("changed", Value::from(changed))]))
            }
            "batchDeletionPlan" => {
                let keys = edit::read_strings(a(0)?)?;
                let whole = edit::whole(a(1)?)?;
                self.store()
                    .plan_batch_deletion(&keys, whole)
                    .map(|p| p.to_json())
                    .map_err(core)
            }
            "batchDelete" => {
                let keys = edit::read_strings(a(0)?)?;
                let whole = edit::whole(a(1)?)?;
                self.store()
                    .batch_delete(&keys, whole, a(2)?)
                    .map(|p| p.to_json())
                    .map_err(core)
            }
            "export" => self.export(),
            "exportSheet" => {
                let title = a(0)?;
                let snapshot = read_snapshot(a(1)?)?;
                self.export_sheet(title, &snapshot)
            }
            "dataLocation" => Ok(self.data_location()),
            "pickWorkfile" => self.pick_workfile(a(0)?),
            "moveWorkfile" => self.switching(|| {
                self.home
                    .move_to(&self.database, Path::new(a(0)?))
                    .map_err(core)
            }),
            "switchWorkfile" => self.switching(|| {
                match o(1) {
                    Some(target) => self.home.create_from_snapshot(
                        &self.database,
                        Path::new(a(0)?),
                        Path::new(target),
                    ),
                    None => self.home.use_in_place(&self.database, Path::new(a(0)?)),
                }
                .map_err(core)
            }),
            "newWorkfile" => self.switching(|| {
                self.home
                    .create_new_while_running(&self.database, Path::new(a(0)?))
                    .map_err(core)
            }),
            "backupWorkfile" => self.backup_workfile(),
            "revealDataFolder" => self.reveal_data_folder(),
            "about" => Ok(window::about()),
            "windowPrefs" => Ok(self.read_window_prefs()),
            "saveWindowPrefs" => {
                let close_to_tray = flag(a(0)?)?;
                let autostart = flag(a(1)?)?;
                self.save_window_prefs(close_to_tray, autostart)
            }
            "setUnsaved" => {
                let on = flag(a(0)?)?;
                self.unsaved.store(on, Ordering::SeqCst);
                Ok(Value::Bool(on))
            }
            "closeWindow" => {
                // 확인을 받았으니 깃발을 내리고, 답을 보낸 뒤 창을 닫는다.
                self.unsaved.store(false, Ordering::SeqCst);
                self.close.store(true, Ordering::SeqCst);
                Ok(Value::Null)
            }
            _ => Err(format!("모르는 요청입니다: {method}")),
        }
    }

    /// `PRAGMA data_version` — 다리를 세울 때 연 연결에서.
    fn data_version(&self) -> Result<i64, String> {
        let connection = self.changes.lock().unwrap_or_else(|e| e.into_inner());
        let connection = connection.as_ref().ok_or("창이 이미 닫혔습니다.")?;
        connection
            .query_row("PRAGMA data_version;", [], |r| r.get::<_, i64>(0))
            .map_err(|e| error_message(&e.into()))
    }

    /// `env` 의 지금을 이 다리의 시간대로.
    fn now(&self) -> Result<LocalDateTime, String> {
        let now = self.env.now_utc();
        parse_to_local(Some(&now), self.zone)
            .ok_or_else(|| format!("지금을 읽지 못했습니다: {now}"))
    }

    /// 파일 이름에 박는 오늘(`yyyyMMdd`).
    fn stamp(&self) -> Result<String, String> {
        self.now().map(pclm_excel::exporter::compact_date)
    }

    /// 지금 보고 있는 파일. 열람 중이면 원본이다.
    fn shown_path(&self) -> PathBuf {
        match &self.viewing {
            Some(v) => v.source_path().to_path_buf(),
            None => self.database.path().to_path_buf(),
        }
    }

    fn pick_save_path(&self, prompt: SavePrompt) -> Option<String> {
        self.host
            .pick_save_path(&prompt)
            .filter(|t| !t.is_empty())
    }

    /// 쌓인 것을 엑셀 한 권으로. 자리를 사람이 고르게 하고, 실제로 쓴 전체 경로를 낸다. 그만두면 null.
    fn export(&self) -> Result<Value, String> {
        let prompt = SavePrompt {
            title: "엑셀로 내보내기".into(),
            filter: XLSX_FILTER.into(),
            default_ext: ".xlsx".into(),
            file_name: pclm_excel::default_file_name(self.now()?),
            allow_overwrite: true,
        };
        let Some(target) = self.pick_save_path(prompt) else {
            return Ok(Value::Null);
        };
        pclm_excel::export(&self.database, Path::new(&target))
            .map(|full| Value::String(path_text(&full)))
            .map_err(|e| excel_writing_error(e, &target))
    }

    /// 화면에서 확정한 한 장. 검사가 고르기 창보다 먼저다.
    fn export_sheet(&self, title: &str, snapshot: &SheetSnapshot) -> Result<Value, String> {
        pclm_excel::validate_snapshot(title, snapshot).map_err(|e| error_message(&e))?;
        let prompt = SavePrompt {
            title: "현재 표를 엑셀로 내보내기".into(),
            filter: XLSX_FILTER.into(),
            default_ext: ".xlsx".into(),
            file_name: format!("{title}_{}.xlsx", self.stamp()?),
            allow_overwrite: true,
        };
        let Some(target) = self.pick_save_path(prompt) else {
            return Ok(Value::Null);
        };
        pclm_excel::export_sheet(Path::new(&target), title, snapshot)
            .map(|full| Value::String(path_text(&full)))
            .map_err(|e| excel_writing_error(e, &target))
    }

    fn store(&self) -> Store<'e> {
        Store::new(self.database.clone(), self.env)
    }

    /// 계약면 또는 저장한 맞춤 뷰 하나를 통째로. 등록된 이름만 읽는다.
    fn read_sheet(&self, view: &str) -> Result<Value, String> {
        let store = self.store();
        let user_view = view.starts_with(views::USER_VIEW_PREFIX)
            && store
                .user_views()
                .map_err(core)?
                .iter()
                .any(|v| format!("{}{}", views::USER_VIEW_PREFIX, v.name) == view);
        if !views::NAMES.contains(&view) && !user_view {
            return Err(format!("내보이지 않는 표입니다: {view}"));
        }

        let entity_type = if user_view {
            "contract"
        } else {
            views::entity_type_of(view)
        };
        let read_only = user_view || views::READ_ONLY.contains(&view) || self.viewing.is_some();

        let (editable, computed): (Vec<String>, Vec<String>) = if read_only {
            (Vec::new(), Vec::new())
        } else {
            let editable = store
                .user_columns(Some(entity_type))
                .map_err(core)?
                .into_iter()
                .filter(|c| c.kind != "computed")
                .map(|c| c.field_name)
                .collect();
            let computed = store
                .user_columns(Some(entity_type))
                .map_err(core)?
                .into_iter()
                .filter(|c| c.kind == "computed")
                .map(|c| c.field_name)
                .collect();
            (editable, computed)
        };

        let elsewhere: Vec<&str> = match view {
            "v_통합" => views::UNIFIED_NOTICE_COLUMNS
                .iter()
                .chain(views::UNIFIED_REQUEST_COLUMNS)
                .copied()
                .collect(),
            "v_통합차수" => views::CONTRACT_COLUMNS
                .iter()
                .chain(views::UNIFIED_REQUEST_COLUMNS)
                .copied()
                .collect(),
            _ => Vec::new(),
        };

        let connection = self.database.open_read_only().map_err(core)?;
        let sql = format!("SELECT * FROM \"{}\";", view.replace('"', "\"\""));
        let sqlite = |e: rusqlite::Error| error_message(&e.into());
        let mut statement = connection.prepare(&sql).map_err(sqlite)?;
        let columns: Vec<String> = statement.column_names().map(String::from).collect();
        let mut rows = Vec::new();
        let mut cursor = statement.query([]).map_err(sqlite)?;
        while let Some(row) = cursor.next().map_err(sqlite)? {
            // Dictionary 의 `row[열] = 값` — 같은 이름의 열이 또 오면 처음 자리에서 값만 바뀐다.
            let mut cells = Map::new();
            for (i, column) in columns.iter().enumerate() {
                let value = pclm_excel::exporter::display(row.get_ref(i).map_err(sqlite)?);
                cells.insert(column.clone(), Value::String(value));
            }
            rows.push(Value::Object(cells));
        }

        let correctable: Vec<Value> = if read_only {
            Vec::new()
        } else {
            columns
                .iter()
                .filter(|c| {
                    !views::KEY_COLUMNS.contains(&c.as_str())
                        && !editable.contains(c)
                        && !computed.contains(c)
                        && !elsewhere.contains(&c.as_str())
                })
                .map(|c| Value::String(c.clone()))
                .collect()
        };
        let overrides = if read_only {
            Map::new()
        } else {
            self.overrides(&store, entity_type)?
        };

        let mut out = Map::new();
        out.insert("name".into(), Value::String(view.to_string()));
        out.insert("columns".into(), strings(columns.iter()));
        out.insert(
            "editable".into(),
            strings(columns.iter().filter(|c| editable.contains(c))),
        );
        out.insert("correctable".into(), Value::Array(correctable));
        out.insert("overrides".into(), Value::Object(overrides));
        out.insert("rows".into(), Value::Array(rows));
        Ok(Value::Object(out))
    }

    /// 걸려 있는 덮개를 줄 키(`EntityRef` 의 표시 글)로 묶는다. 값은 고치기 전의 것.
    fn overrides(
        &self,
        store: &Store<'_>,
        entity_type: &str,
    ) -> Result<Map<String, Value>, String> {
        let mut by_row = Map::new();
        for o in store.overrides(entity_type).map_err(core)? {
            let display = EntityRef::new(entity_type, &o.base, &o.seq).display();
            let cells = by_row
                .entry(display)
                .or_insert_with(|| Value::Object(Map::new()));
            if let Value::Object(cells) = cells {
                cells.insert(o.column_name, Value::String(o.original));
            }
        }
        Ok(by_row)
    }
}

/// 답 한 통의 글: `{"id":…,"ok":…,"result":…,"error":…}`(null 도 적는다).
pub fn reply_json(id: i32, ok: bool, result: Option<&Value>, error: Option<&str>) -> String {
    let mut map = Map::new();
    map.insert("id".into(), Value::from(id));
    map.insert("ok".into(), Value::Bool(ok));
    map.insert("result".into(), result.cloned().unwrap_or(Value::Null));
    map.insert(
        "error".into(),
        error.map_or(Value::Null, |e| Value::String(e.to_string())),
    );
    bridge_json(&Value::Object(map))
}

/// 알림으로 보내는 글 — `{"notice":…}`. 답과 같은 통로로 보낸다.
pub fn notice_json(notice: &str) -> String {
    bridge_json(&object([("notice", Value::String(notice.into()))]))
}

/// `HandleAsync` 의 풀이 — `(id, method, args)`. 실패하면 그때까지 읽은 `id`(못 읽었으면 0)와 거절 글.
#[allow(clippy::type_complexity)]
fn parse_message(message: &str) -> Result<(i32, String, Vec<Option<String>>), (i32, String)> {
    let root: Value = serde_json::from_str(message)
        .map_err(|e| (0, format!("JSON 을 읽지 못했습니다: {e}")))?;
    let property = |name: &str| -> Result<&Value, String> {
        match &root {
            Value::Object(map) => map
                .get(name)
                .ok_or_else(|| format!("요청에 {name} 칸이 없습니다.")),
            other => Err(wrong_type("객체", other)),
        }
    };
    let id = match property("id").map_err(|e| (0, e))? {
        Value::Number(n) => n
            .as_i64()
            .and_then(|n| i32::try_from(n).ok())
            .ok_or_else(|| (0, "요청의 id 가 정수가 아닙니다.".to_string()))?,
        other => return Err((0, wrong_type("수", other))),
    };
    let method = match property("method").map_err(|e| (id, e))? {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => return Err((id, wrong_type("글", other))),
    };
    let args = match root.get("args") {
        None => Vec::new(),
        Some(Value::Array(items)) => items
            .iter()
            .map(|x| match x {
                Value::Null => None,
                Value::String(s) => Some(s.clone()),
                other => Some(other.to_string()),
            })
            .collect(),
        Some(other) => return Err((id, wrong_type("배열", other))),
    };
    Ok((id, method, args))
}

const XLSX_FILTER: &str = "엑셀 통합 문서 (*.xlsx)|*.xlsx";

/// 엑셀이 그 파일을 붙들고 있으면 덮어쓰지 못한다 — 그때는 닫으라고 말한다. 권한이 없는 것은 [`files::writing_error`] 의 글이다.
fn excel_writing_error(error: pclm_core::Error, target: &str) -> String {
    match &error {
        pclm_core::Error::Io(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            files::writing_error(error, target)
        }
        pclm_core::Error::Io(_) => {
            format!("파일을 쓰지 못했습니다 — 엑셀에서 열려 있다면 닫고 다시 해 주세요.  {target}")
        }
        _ => error_message(&error),
    }
}

/// `{columns, rows}` 를 읽는다(camelCase). JSON 의 null 이면 「작업할 자료가 없습니다.」. 빠진 속성 · null 원소는 `None` 으로
/// 두어 검사가 거절하게 한다. 모르는 속성은 버린다. 꼴이 다른 값은 그 칸의 길을 대어 거절한다([`wrong_shape`]).
fn read_snapshot(json: &str) -> Result<SheetSnapshot, String> {
    let value: Value = serde_json::from_str(json).map_err(|e| malformed(e.to_string()))?;
    let strings = |v: &Value, at: &str| -> Result<Option<Vec<Option<String>>>, String> {
        match v {
            Value::Null => Ok(None),
            Value::Array(items) => items
                .iter()
                .enumerate()
                .map(|(i, x)| match x {
                    Value::Null => Ok(None),
                    Value::String(s) => Ok(Some(s.clone())),
                    _ => Err(wrong_shape(&format!("{at}[{i}]"))),
                })
                .collect::<Result<Vec<_>, _>>()
                .map(Some),
            _ => Err(wrong_shape(at)),
        }
    };
    match value {
        Value::Null => Err(NOTHING_TO_DO.into()),
        Value::Object(map) => {
            let columns = match map.get("columns") {
                Some(v) => strings(v, "$.columns")?,
                None => None,
            };
            let rows = match map.get("rows") {
                None | Some(Value::Null) => None,
                Some(Value::Array(items)) => Some(
                    items
                        .iter()
                        .enumerate()
                        .map(|(i, row)| strings(row, &format!("$.rows[{i}]")))
                        .collect::<Result<Vec<_>, _>>()?,
                ),
                Some(_) => return Err(wrong_shape("$.rows")),
            };
            Ok(SheetSnapshot { columns, rows })
        }
        _ => Err(wrong_shape("$")),
    }
}

/// 요청 JSON 의 꼴이 다르다 — `path` 는 틀린 칸의 길(`$[0].rowKey`).
pub(crate) fn wrong_shape(path: &str) -> String {
    format!("요청의 모양이 다릅니다: {path}")
}

/// `ReadBatch` 가 JSON 의 null 을 받았을 때.
const NOTHING_TO_DO: &str = "작업할 자료가 없습니다.";

/// 깨진 JSON 의 거절 글.
fn malformed(detail: String) -> String {
    format!("JSON 을 읽지 못했습니다: {detail}")
}

/// 없거나 null 이면 그 말을 거절 글로 낸다.
fn require(args: &[Option<String>], index: usize) -> Result<&str, String> {
    args.get(index)
        .and_then(|a| a.as_deref())
        .ok_or_else(|| format!("{}번째 값이 없습니다.", index + 1))
}

/// 인자 하나 — 없으면 `None`.
fn optional(args: &[Option<String>], index: usize) -> Option<&str> {
    args.get(index).and_then(|a| a.as_deref())
}

/// 켜고 끄는 값은 `true`·`false` 둘만 받는다.
fn flag(value: &str) -> Result<bool, String> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(format!("켜짐·꺼짐이 아닙니다: {value}")),
    }
}

/// 32비트 정수로 읽는다 — 앞뒤 공백, 앞의 부호 하나, 십진 숫자.
fn parse_int(text: &str) -> Result<i32, String> {
    let trimmed = text.trim_matches(|c: char| matches!(c, ' ' | '\t' | '\n' | '\u{b}' | '\u{c}' | '\r'));
    let (negative, digits) = match trimmed.as_bytes().first() {
        Some(b'-') => (true, &trimmed[1..]),
        Some(b'+') => (false, &trimmed[1..]),
        _ => (false, trimmed),
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("정수가 아닙니다: {text}"));
    }
    let significant = digits.trim_start_matches('0');
    if significant.len() > 10 {
        return Err(OVERFLOW.into());
    }
    let magnitude: i64 = significant.parse().unwrap_or(0);
    let value = if negative { -magnitude } else { magnitude };
    i32::try_from(value).map_err(|_| OVERFLOW.to_string())
}

const OVERFLOW: &str = "정수 범위를 벗어났습니다.";

/// 값이 바란 꼴(객체 · 배열 · 글 · 수)이 아니다.
fn wrong_type(expected: &str, actual: &Value) -> String {
    format!("값의 꼴이 다릅니다(바란 것: {expected}, 받은 것: {}).", kind_name(actual))
}

/// 값의 꼴 이름.
fn kind_name(value: &Value) -> &'static str {
    match value {
        Value::Object(_) => "객체",
        Value::Array(_) => "배열",
        Value::String(_) => "글",
        Value::Number(_) => "수",
        Value::Bool(_) => "참거짓",
        Value::Null => "null",
    }
}

fn core(error: pclm_core::Error) -> String {
    error_message(&error)
}

/// 이름이 정해진 칸들의 객체 — 적은 차례 그대로.
fn object<const N: usize>(pairs: [(&str, Value); N]) -> Value {
    Value::Object(
        pairs
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
    )
}

fn text_or_null(text: Option<String>) -> Value {
    text.map_or(Value::Null, Value::String)
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn strings<'a>(values: impl Iterator<Item = &'a String>) -> Value {
    Value::Array(values.map(|v| Value::String(v.clone())).collect())
}

/// 파일 이름 — 마지막 `\`·`/` 뒤.
fn file_name(path: &Path) -> String {
    let text = path.to_string_lossy();
    text.rsplit(['\\', '/']).next().unwrap_or("").to_string()
}

/// 폴더 경로 — 뿌리면 `None`.
fn directory_name(path: &Path) -> Option<String> {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(path_text)
}

#[cfg(test)]
mod tests;

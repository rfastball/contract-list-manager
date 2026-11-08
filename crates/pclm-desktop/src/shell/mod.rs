//! 창(C# `Pclm.App` 의 `App.xaml.cs` · `MainWindow.xaml.cs`) — Tauri 2 위의 WebView2 하나와 다리 하나.
//!
//! 그림은 전부 화면(React)이 그리고 여기서는 **창을 띄우고 다리를 놓는 일**만 한다. 화면과의 통로는 앱 명령 `bridge` 하나
//! (`{id, method, args}` 글 → 다리의 답 글, C# `ReplyJson` 과 같은 바이트)와 알림 이벤트 `pclm:notice`(`{notice}` 글)다.
//!
//! 시작의 차례는 C# `OnStartup` 그대로다: 인자 → 홈 → 열람 사본 · 옛 exe 치우기 · 파일 연결(업무 홈만) → 열람 창이면 그리로 →
//! 홈 잠금(못 얻으면 떠 있는 창을 불러내고 끝) → 쪽지 → 작업자료(못 열면 시작 화면) → 알릴 것 → 확장 · 자동 실행 고침(업무 홈만)
//! → 다리 → 창.

mod activation;
mod assets;
mod dialogs;
mod start_screen;
mod tray;
mod win;

use std::path::Path;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use pclm_bridge::{Bridge, FolderPrompt, Host, OpenPrompt, Os, SavePrompt, WindowsOs};
use pclm_core::Error;
use pclm_core::storage::store::error_message;
use pclm_home::window_prefs::WindowPrefs;
use pclm_home::{Home, HomeState, SystemEnv};
use tauri::ipc::CapabilityBuilder;
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalSize, RunEvent, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, Wry,
};

use self::dialogs::Pick;
use self::start_screen::Outcome;
use self::win::{Icon, message_box};
use crate::launch::{self, HomeChoice};
use crate::{association, old_executables};

/// 시계 · 식별자 · 시간대 — 진짜(C# `DateTime.Now`).
static SYSTEM: SystemEnv = SystemEnv;

/// 창은 하나다. 화면이 받는 권한(capabilities/main.json)이 이 이름에 걸려 있다.
const LABEL: &str = "main";

/// 알림 이벤트의 이름. 실리는 것은 다리의 `notice_json` 글(`{"notice":…}`) 그대로다.
const NOTICE: &str = "pclm:notice";

/// C# `MainWindow.xaml` 의 `Width="1560" Height="820"` — 창 테두리까지 친 크기(DIP).
const WIDTH: f64 = 1560.0;
const HEIGHT: f64 = 820.0;

/// 이 창의 모든 것 — 프로세스가 끝날 때까지 산다(다리가 `&'static dyn Host` 로 쥔다).
struct Shell {
    /// 작업자료 창인가. 열람 창에는 알림 영역도 불러내기도 닫기 물음도 없다 — 닫으면 그냥 끝난다.
    main: bool,
    title: String,
    home: Home,
    ui: Option<String>,
    app: OnceLock<AppHandle<Wry>>,
    bridge: Mutex<Option<Arc<Bridge<'static>>>>,
    tray: Mutex<Option<tray::Tray>>,
    /// 정말로 내리는 중인가(C# `_exiting`). 닫기를 숨기기로 바꾸는 길이 진짜 끝내기까지 삼키면 안 된다.
    exiting: AtomicBool,
    /// 작업자료를 바꿔 다시 띄우기로 했는가(C# `_restartAsked`).
    restart_asked: AtomicBool,
    /// 창(WebView2)을 지었는가. 로그인할 때 알림 영역으로만 켜졌으면 처음 보일 때 짓는다.
    built: AtomicBool,
    /// 작업 표시줄에 내려 둔 채 띄우는가(로그인할 때 켜졌고 알림 영역 두기가 꺼져 있다).
    minimized: bool,
}

pub fn run(args: Vec<String>) -> ExitCode {
    // 콘솔이 없는 창 프로그램이라, 터지면 까닭을 볼 데가 없다. 화면에 띄우고 파일로도 남긴다. 홈이 정해지기 전에 터지면 업무
    // 홈에 남긴다(C# `_errorLog` 의 처음 값).
    let default_log = Home::default_home().map(|h| h.error_log_path()).ok();
    let launch = match launch::parse(&args) {
        Ok(launch) => launch,
        Err(message) => return fail(default_log.as_deref(), &message),
    };
    let home = match &launch.home {
        HomeChoice::Default => Home::default_home(),
        HomeChoice::Development => Home::development(),
        HomeChoice::Directory(directory) => Home::new(directory),
    };
    let home = match home {
        Ok(home) => home,
        Err(error) => return fail(default_log.as_deref(), &error.to_string()),
    };
    let log = home.error_log_path();
    let exe = std::env::current_exe().ok();

    // 지난번에 죽은 열람 창이 남긴 사본을 걷는다. 오래된 것만.
    home.prune_views();
    // 확장 호스트가 붙든 exe 는 이름을 바꿔 비킨다(ADR-035). 그 옛 exe 를 이 exe 옆과 홈에서 치운다.
    old_executables::sweep_around(exe.as_deref(), home.directory());
    // .pclm 을 두 번 누르면 이 exe 가 뜨게 적는다. 열람 창으로 떠도 적는다. 업무 홈일 때만.
    association::refresh_for(
        &win::CurrentUser,
        &home,
        exe.as_deref(),
        &std::env::temp_dir(),
    );

    let view = match launch::view(&args, launch::file_exists) {
        Ok(view) => view,
        Err(message) => return fail(Some(&log), &message),
    };
    // 내 작업자료를 짚었으면 열람이 아니라 보통으로 연다.
    if let Some(view) = view.filter(|v| !home.is_active_workfile(Path::new(v))) {
        return viewer(home, &view, launch.ui, &log);
    }

    // 작업자료는 한 창만 연다(ADR-030). 작업자료를 바꾼 창이 띄운 새 창만 잠깐 기다린다.
    let wait = if launch.restarted {
        Duration::from_secs(5)
    } else {
        Duration::ZERO
    };
    let lock = match home.try_lock(wait) {
        Ok(Some(lock)) => lock,
        Ok(None) => {
            // 로그인할 때 켜진 것이 이미 떠 있는 창을 불러내면 사람은 띄운 적 없는 창을 본다. 말도 없이 끝난다.
            if launch.background {
                return ExitCode::SUCCESS;
            }
            // 떠 있는 창을 앞으로 불러내고 조용히 끝난다. 다시 띄우던 길은 부르지 않는다: 그 신호는 내려가는 옛 창이 받을 수 있다.
            if !launch.restarted && WindowsOs.signal_activation(&home.activation_name()) {
                return ExitCode::SUCCESS;
            }
            message_box(None, "계약 목록이 이미 열려 있습니다.", Icon::Information);
            return ExitCode::SUCCESS;
        }
        Err(error) => return fail(Some(&log), &error.to_string()),
    };

    // 쪽지 → 작업자료. 열 수 없으면 시작 화면이 사람에게 고르게 한다 — 여기서 빈 자료를 세우지 않는다.
    let (database, notes) = loop {
        match home.resolve() {
            Ok(HomeState::Ready { database, notes }) => break (database, notes),
            Ok(HomeState::Problem { message, path, .. }) => {
                match start_screen::show(&home, &message, Some(&path)) {
                    Outcome::Chosen => continue,
                    Outcome::Closed => return ExitCode::SUCCESS,
                    Outcome::Failed(message) => return fail(Some(&log), &message),
                }
            }
            Err(error) => return fail(Some(&log), &error_message(&error)),
        }
    };
    // 옛 자료를 옮겨 왔거나 판올림 전에 백업을 떴으면 조용히 넘어가지 않는다.
    for note in &notes {
        message_box(None, note, Icon::Information);
    }
    // 확장 연결과 자동 실행은 업무 홈에서만 고친다(등록부의 호스트 연결은 컴퓨터에 하나뿐이다).
    pclm_bridge::refresh_installation(&WindowsOs, home.is_default());

    let prefs = WindowPrefs::read(&home.window_prefs_path());
    let shell = leak(Shell {
        main: true,
        title: launch::main_title(launch.erp_dev(), &home),
        home: home.clone(),
        ui: launch.ui.clone(),
        app: OnceLock::new(),
        bridge: Mutex::new(None),
        tray: Mutex::new(None),
        exiting: AtomicBool::new(false),
        restart_asked: AtomicBool::new(false),
        built: AtomicBool::new(false),
        minimized: launch.background,
    });
    let bridge = match Bridge::new(database, home.clone(), None, &SYSTEM, &SYSTEM) {
        Ok(bridge) => bridge,
        Err(message) => return fail(Some(&log), &message),
    };
    shell.attach(bridge);
    shell.set_tray(prefs.close_to_tray);
    let activation = activation::Activation::listen(&home.activation_name(), move || {
        shell.on_main(Shell::show_main)
    });

    // 로그인할 때 켜졌으면 앞에 나서지 않는다. 알림 영역에 두기가 켜져 있으면 창 없이 아이콘만 선다 — 화면(WebView2)은 처음
    // 보일 때 선다. 꺼져 있으면 숨을 곳이 없으니 작업 표시줄에 내려 둔다.
    let code = shell.run_app(!(launch.background && prefs.close_to_tray), &log);

    // C# 의 차례: 창이 닫히며 다리가 놓이고(자동 수집 멈춤 · 연결 닫기), 앱이 내려가며 불러내기 · 알림 영역 · 잠금을 놓는다.
    shell.detach();
    drop(activation);
    shell.set_tray(false);
    drop(lock);
    if shell.restart_asked.load(Ordering::SeqCst) {
        start_again(&launch.restart_args, &log);
    }
    exit_code(code)
}

/// 열람 창(ADR-031). **홈 잠금을 잡지 않고, 쪽지를 읽어 작업자료를 열지 않고, 확장 연결을 고치지 않는다** — 그래야 작업자료
/// 창이 떠 있는 동안에도 몇 개든 띄울 수 있다. 원본을 열지 않고 홈의 임시 사본을 연다. 창을 닫으면 사본을 지운다.
fn viewer(home: Home, view: &str, ui: Option<String>, log: &Path) -> ExitCode {
    let viewed = match home.open_view(Path::new(view), &SYSTEM) {
        Ok(viewed) => viewed,
        Err(Error::PclmFile(error)) => {
            message_box(None, &error.message, Icon::Warning);
            return ExitCode::from(1);
        }
        Err(error) => return fail(Some(log), &error_message(&error)),
    };
    let shell = leak(Shell {
        main: false,
        title: launch::viewer_title(viewed.source_path(), viewed.role()),
        home: home.clone(),
        ui,
        app: OnceLock::new(),
        bridge: Mutex::new(None),
        tray: Mutex::new(None),
        exiting: AtomicBool::new(false),
        restart_asked: AtomicBool::new(false),
        built: AtomicBool::new(false),
        minimized: false,
    });
    let database = viewed.database().clone();
    let bridge = match Bridge::new(database, home, Some(viewed), &SYSTEM, &SYSTEM) {
        Ok(bridge) => bridge,
        Err(message) => return fail(Some(log), &message),
    };
    shell.attach(bridge);
    let code = shell.run_app(true, log);
    // 다리가 먼저 놓인다 — 그것이 쥔 연결이 닫혀야 사본이 지워진다(다리가 사본을 쥐고 함께 놓는다).
    shell.detach();
    exit_code(code)
}

/// 창은 프로세스와 함께 산다 — 다리가 `&'static dyn Host` 로, 명령이 [`SHELL`] 로 찾는다.
fn leak(shell: Shell) -> &'static Shell {
    let shell: &'static Shell = Box::leak(Box::new(shell));
    let _ = SHELL.set(shell);
    shell
}

fn exit_code(code: i32) -> ExitCode {
    ExitCode::from(u8::try_from(code).unwrap_or(1))
}

/// C# `App.Report` 뒤의 `Quit(1)`.
fn fail(log: Option<&Path>, message: &str) -> ExitCode {
    match log {
        Some(log) => win::report(log, message, message),
        None => message_box(None, message, Icon::Error),
    }
    ExitCode::from(1)
}

/// 작업자료를 바꿨으면 같은 인자로 다시 띄운다(ADR-032 의 5). 창 · 다리 · 연결 · 잠금을 모두 놓은 다음이다.
fn start_again(restart_args: &[String], log: &Path) {
    let Ok(exe) = std::env::current_exe() else {
        win::report(
            log,
            "이 프로그램의 자리를 알 수 없습니다.",
            "이 프로그램의 자리를 알 수 없습니다.",
        );
        return;
    };
    let mut command = std::process::Command::new(exe);
    command.args(restart_args).arg(launch::RESTARTED);
    if let Err(error) = command.spawn() {
        win::report(log, &error.to_string(), &error.to_string());
    }
}

impl Shell {
    fn attach(&'static self, mut bridge: Bridge<'static>) {
        bridge.set_default_home(self.home.is_default());
        bridge.set_ui_origin(self.ui.clone());
        bridge.set_host(self);
        let bridge = Arc::new(bridge);
        // C# `MainWindow` 가 지을 때 부른다. 열람 창은 하지 않는다(다리가 가린다).
        bridge.start_auto_capture();
        *self.bridge.lock().unwrap_or_else(|e| e.into_inner()) = Some(bridge);
    }

    /// 다리를 놓는다(C# `Closed += _bridge.Dispose()`). 아직 풀고 있는 요청이 쥔 것은 그 요청이 끝날 때 놓인다.
    fn detach(&self) {
        let bridge = self.bridge.lock().unwrap_or_else(|e| e.into_inner()).take();
        drop(bridge);
    }

    fn bridge(&self) -> Option<Arc<Bridge<'static>>> {
        self.bridge
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    fn window(&self) -> Option<WebviewWindow<Wry>> {
        self.app.get()?.get_webview_window(LABEL)
    }

    /// 창의 차례(Tauri 의 주 실마리)에서 한다 — C# `Dispatcher.BeginInvoke`.
    fn on_main(&'static self, work: fn(&'static Shell)) {
        if let Some(app) = self.app.get() {
            let _ = app.run_on_main_thread(move || work(self));
        }
    }

    fn run_app(&'static self, show: bool, log: &Path) -> i32 {
        let mut context = tauri::generate_context!();
        context.set_assets(Box::new(assets::Screen::new()));
        let built = tauri::Builder::default()
            .invoke_handler(tauri::generate_handler![bridge])
            .setup(move |app| {
                let _ = self.app.set(app.handle().clone());
                // 개발 서버(--ui)에서 연 화면은 앱의 출처가 아니다 — 그때만 그 출처에 알림 듣기를 연다.
                if let Some(origin) = &self.ui {
                    app.add_capability(
                        CapabilityBuilder::new("dev-ui")
                            .remote(format!("{}/*", origin.trim_end_matches('/')))
                            .window(LABEL)
                            .permission("core:event:allow-listen")
                            .permission("core:event:allow-unlisten"),
                    )?;
                }
                if show {
                    self.build_window(self.minimized);
                }
                Ok(())
            })
            .on_window_event(move |window, event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event
                    && self.close_requested(window)
                {
                    api.prevent_close();
                }
            })
            .build(context);
        let app = match built {
            Ok(app) => app,
            Err(error) => {
                win::report(log, &error.to_string(), &format!("{error:?}"));
                return 1;
            }
        };
        app.run_return(|_, event| {
            if let RunEvent::ExitRequested {
                code: None, api, ..
            } = event
                && self.main
                && !self.exiting.load(Ordering::SeqCst)
                && !self.built.load(Ordering::SeqCst)
            {
                // 알림 영역으로만 켜져 창이 아직 없다 — 창이 없다고 내리지 않는다.
                api.prevent_exit();
            }
        })
    }

    /// 창을 짓는다(C# `MainWindow` + `StartAsync`). 주 실마리에서 지으면 WebView2 가 막힐 수 있어(Tauri 의 경고) 따로 실마리에서.
    fn build_window(&'static self, minimized: bool) {
        if self.built.swap(true, Ordering::SeqCst) {
            return;
        }
        let Some(app) = self.app.get().cloned() else {
            return;
        };
        std::thread::spawn(move || {
            if let Err(message) = self.try_build_window(&app, minimized) {
                message_box(None, &message, Icon::Error);
                self.quit(1);
            }
        });
    }

    fn try_build_window(
        &'static self,
        app: &AppHandle<Wry>,
        minimized: bool,
    ) -> Result<(), String> {
        // --ui 는 지어 둔 화면 대신 딴 곳에서 연다. 개발 중 Vite 서버를 짚으면 화면만 갈아 끼우면서 다리와 DB 는 진짜를 쓴다.
        let url = match &self.ui {
            Some(origin) => {
                WebviewUrl::External(origin.parse().map_err(|e| format!("{e}: {origin}"))?)
            }
            None => WebviewUrl::App("index.html".into()),
        };
        let window = WebviewWindowBuilder::new(app, LABEL, url)
            .title(&self.title)
            .inner_size(WIDTH, HEIGHT)
            .visible(false)
            // 사용자 자료가 아니라 홈에 WebView2 프로필을 둔다(C# 과 같은 자리).
            .data_directory(self.home.webview2_directory())
            // 앱 화면이지 브라우저가 아니다 — 개발자 도구는 디버그 빌드에서만 연다. 깃발이 아니라 빌드 설정에 매단다.
            .devtools(cfg!(debug_assertions))
            .build()
            .map_err(|e| {
                format!(
                    "WebView2 런타임을 열지 못했습니다.\n\n{e}\n\nMicrosoft Edge WebView2 런타임을 설치한 뒤 다시 실행해 주세요."
                )
            })?;
        fit_outer_size(&window);
        let _ = window.center();
        self.configure_webview(&window);
        if minimized {
            if let Ok(hwnd) = window.hwnd() {
                win::show_minimized_without_activating(hwnd.0 as _);
            }
        } else {
            let _ = window.show();
            let _ = window.set_focus();
        }
        Ok(())
    }

    /// C# `StartAsync` 의 WebView2 설정 — 나가는 물건에서는 오른쪽 단추 메뉴와 개발자 도구를 닫고, 상태 표시줄을 끄고, 화면
    /// 프로세스가 죽으면 다리에 알린다(`page_failed` — 「닫기 요청」 에 답할 쪽이 없으니 저장하지 않음 깃발을 내린다).
    fn configure_webview(&'static self, window: &WebviewWindow<Wry>) {
        let _ = window.with_webview(move |webview| {
            #[allow(unsafe_code)]
            // SAFETY: Tauri 가 넘긴 살아 있는 WebView2 의 COM 손잡이.
            unsafe {
                let Ok(core) = webview.controller().CoreWebView2() else {
                    return;
                };
                if let Ok(settings) = core.Settings() {
                    let debug = cfg!(debug_assertions);
                    let _ = settings.SetAreDefaultContextMenusEnabled(debug);
                    let _ = settings.SetAreDevToolsEnabled(debug);
                    let _ = settings.SetIsStatusBarEnabled(false);
                }
                let handler =
                    webview2_com::ProcessFailedEventHandler::create(Box::new(move |_, _| {
                        if let Some(bridge) = self.bridge() {
                            bridge.page_failed();
                        }
                        Ok(())
                    }));
                let mut token = 0i64;
                let _ = core.add_ProcessFailed(&handler, &mut token);
            }
        });
    }

    /// 닫기(X)를 눌렀다. 막았으면 참.
    fn close_requested(&'static self, window: &tauri::Window<Wry>) -> bool {
        if self.exiting.load(Ordering::SeqCst) || !self.main {
            return false;
        }
        // 알림 영역에 두기가 켜져 있으면 숨긴다. 처음 숨을 때 한 번만 풍선으로 어디로 갔는지 알린다.
        let hidden = {
            let tray = self.tray.lock().unwrap_or_else(|e| e.into_inner());
            tray.as_ref().map(|tray| {
                let _ = window.hide();
                let path = self.home.window_prefs_path();
                let prefs = WindowPrefs::read(&path);
                if !prefs.tray_notice_shown {
                    tray.notify(
                        "계약 목록",
                        "계약 목록은 알림 영역에서 계속 돕니다. 아이콘을 두 번 누르면 다시 열리고, 끝내려면 오른쪽 단추로 「끝내기」 를 고르세요.",
                    );
                    // 적지 못하면 다음에 한 번 더 알릴 뿐이다.
                    let _ = WindowPrefs { tray_notice_shown: true, ..prefs }.write(&path);
                }
            })
        };
        if hidden.is_some() {
            return true;
        }
        // 정말 내리는 길이면 저장하지 않은 편집이 있는지 먼저 묻는다. 숨기는 것은 화면을 그대로 두므로 묻지 않는다.
        self.ask_first()
    }

    /// 화면에 저장하지 않은 편집이 있으면 창을 앞으로 내고 「닫기 요청」 을 보낸다 — 화면이 확인을 띄우고, 버려도 되면
    /// `closeWindow` 로 정말 닫는다. 물었으면 참.
    fn ask_first(&'static self) -> bool {
        if self.exiting.load(Ordering::SeqCst) {
            return false;
        }
        let Some(notice) = self.bridge().and_then(|b| b.ask_before_closing()) else {
            return false;
        };
        self.show_main();
        if let Some(app) = self.app.get() {
            let _ = app.emit_to(LABEL, NOTICE, notice);
        }
        true
    }

    /// 작업자료 창을 띄워 앞으로 낸다 — 아직 없으면 짓고, 숨었으면 보이고, 최소화했으면 풀고.
    fn show_main(&'static self) {
        if !self.main || self.exiting.load(Ordering::SeqCst) {
            return;
        }
        let Some(window) = self.window() else {
            self.build_window(false);
            return;
        };
        if !window.is_visible().unwrap_or(true) {
            let _ = window.show();
        }
        if window.is_minimized().unwrap_or(false) {
            let _ = window.unminimize();
        }
        let _ = window.set_focus();
    }

    /// 정말로 내린다(C# `Quit`). 닫기를 숨기기로 바꾸는 길이 이것만은 삼키지 않는다.
    fn quit(&self, code: i32) {
        self.exiting.store(true, Ordering::SeqCst);
        if let Some(app) = self.app.get() {
            app.exit(code);
        }
    }

    /// 알림 영역 아이콘을 세우거나 거둔다. 켜져 있는 동안은 창이 보여도 아이콘이 서 있다 — 닫으면 어디로 가는지 미리 보이게.
    fn set_tray(&'static self, on: bool) {
        let mut tray = self.tray.lock().unwrap_or_else(|e| e.into_inner());
        if on == tray.is_some() || !self.main {
            return;
        }
        if !on {
            *tray = None;
            return;
        }
        *tray = tray::Tray::new(
            &self.title,
            tray::Actions {
                open: Box::new(move || self.on_main(Shell::show_main)),
                exit: Box::new(move || {
                    self.on_main(|shell| {
                        if !shell.ask_first() {
                            shell.quit(0);
                        }
                    })
                }),
                session_ending: Box::new(move || self.exiting.store(true, Ordering::SeqCst)),
            },
        );
    }

    fn owner(&self) -> Option<isize> {
        self.window()
            .and_then(|w| w.hwnd().ok())
            .map(|h| h.0 as isize)
    }
}

/// C# `Height="820" Width="1560"` 은 테두리까지 친 크기다. Tauri 는 안쪽 크기를 받으므로 지은 뒤 테두리만큼 뺀다.
fn fit_outer_size(window: &WebviewWindow<Wry>) {
    let (Ok(scale), Ok(outer), Ok(inner)) = (
        window.scale_factor(),
        window.outer_size(),
        window.inner_size(),
    ) else {
        return;
    };
    let frame_width = outer.width.saturating_sub(inner.width);
    let frame_height = outer.height.saturating_sub(inner.height);
    let wanted = LogicalSize::new(WIDTH, HEIGHT).to_physical::<u32>(scale);
    let _ = window.set_size(PhysicalSize::new(
        wanted.width.saturating_sub(frame_width).max(1),
        wanted.height.saturating_sub(frame_height).max(1),
    ));
}

/// 화면이 보낸 글 한 통 → 다리의 답 글. 다리는 화면 실마리를 막지 않게 따로 실마리에서 푼다(C# 은 요청마다 `Task.Run`).
/// 답을 낸 뒤 다시 띄우기 · 닫기는 다리 `handle` 이 창에 넘긴다 — 창은 그것을 제 차례로 미룬다.
#[tauri::command]
async fn bridge(request: String) -> Result<String, String> {
    let shell = SHELL.get().copied().ok_or("창이 아직 서지 않았습니다.")?;
    let bridge = shell.bridge().ok_or("다리가 놓였습니다.")?;
    tauri::async_runtime::spawn_blocking(move || {
        let mut reply = String::new();
        bridge.handle(&request, &mut |answer| reply = answer);
        reply
    })
    .await
    .map_err(|e| e.to_string())
}

/// 명령이 창을 찾는 자리. 프로세스에 창은 하나다.
static SHELL: OnceLock<&'static Shell> = OnceLock::new();

/// 고르기 창과 창의 일 — C# `App.Wire` 와 `Restart` · `CloseWindow` · `ApplyCloseToTray`. 열람 창은 고르기 창만 단다.
impl Host for Shell {
    fn pick_folder(&self, prompt: &FolderPrompt) -> Option<String> {
        dialogs::show_on_own_thread(
            self.owner(),
            Pick::Folder {
                title: prompt.title.clone(),
                initial: prompt.initial.clone(),
            },
        )
    }

    fn pick_save_path(&self, prompt: &SavePrompt) -> Option<String> {
        dialogs::show_on_own_thread(
            self.owner(),
            Pick::Save {
                title: prompt.title.clone(),
                filter: prompt.filter.clone(),
                default_ext: prompt.default_ext.clone(),
                file_name: prompt.file_name.clone(),
                overwrite_prompt: prompt.allow_overwrite,
                initial: start_screen::documents(),
            },
        )
    }

    fn pick_plan_excel(&self, prompt: &OpenPrompt) -> Option<String> {
        dialogs::show_on_own_thread(
            self.owner(),
            Pick::Open {
                title: prompt.title.clone(),
                filter: prompt.filter.clone(),
                initial: None,
            },
        )
    }

    fn pick_open_file(&self, prompt: &OpenPrompt) -> Option<String> {
        dialogs::show_on_own_thread(
            self.owner(),
            Pick::Open {
                title: prompt.title.clone(),
                filter: prompt.filter.clone(),
                initial: start_screen::documents(),
            },
        )
    }

    fn pick_open_json(&self, prompt: &OpenPrompt) -> Option<String> {
        self.pick_open_file(prompt)
    }

    fn restart(&self) {
        if !self.main {
            return;
        }
        let shell = registered();
        shell.on_main(|shell| {
            shell.restart_asked.store(true, Ordering::SeqCst);
            shell.quit(0);
        });
    }

    fn close_window(&self) {
        if !self.main {
            return;
        }
        registered().on_main(|shell| shell.quit(0));
    }

    fn apply_close_to_tray(&self, on: bool) {
        if !self.main {
            return;
        }
        let shell = registered();
        if let Some(app) = shell.app.get() {
            let _ = app.run_on_main_thread(move || shell.set_tray(on));
        }
    }
}

/// 다리가 `&self` 로 부르는 자리에서 `'static` 창을 찾는다.
fn registered() -> &'static Shell {
    SHELL.get().copied().expect("창이 등록되어 있다")
}

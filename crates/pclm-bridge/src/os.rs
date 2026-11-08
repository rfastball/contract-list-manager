//! 다리가 운영체제에 곧바로 하는 일 — C# 다리가 창에 맡기지 않고 스스로 하던 것(`Registry` · `Process.Start` ·
//! `Environment.ProcessPath` · `EventWaitHandle` · UI Automation · 클립보드 · `FileVersionInfo`).
//!
//! **뜻은 다리가 쥐고, 여기에는 손만 있다.** 어느 키에 무엇을 적을지, 언제 막을지(업무 홈이 아니면 등록부를 고치지 않는다 —
//! `IsDefaultHome`), 무엇을 띄울지는 다리가 정하고, 이 트레이트는 그 한 걸음만 한다. 그래서 시험·대조는 이것을 갈아 끼워
//! **무엇을 시켰는지**만 보고 진짜 등록부·탐색기·브라우저에는 닿지 않는다. 진짜는 [`WindowsOs`](crate::WindowsOs) 다.

use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// 등록부의 뿌리 둘(C# `Registry.CurrentUser` · `Registry.LocalMachine`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegistryRoot {
    CurrentUser,
    LocalMachine,
}

/// 등록부 값 하나. 다리는 글과 바이트만 읽는다 — 그 밖의 꼴은 [`RegistryValue::Other`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegistryValue {
    /// `REG_SZ` · `REG_EXPAND_SZ`(환경 변수를 편 것 — .NET `GetValue` 처럼).
    String(String),
    /// `REG_BINARY`.
    Binary(Vec<u8>),
    Other,
}

/// .NET `Environment.SpecialFolder` 가운데 다리가 쓰는 셋.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpecialFolder {
    LocalApplicationData,
    ProgramFiles,
    ProgramFilesX86,
}

/// 운영체제의 손. 다리는 여러 실마리에서 불리므로 `Sync` 다.
pub trait Os: Sync {
    /// `Environment.ProcessPath` — 이 프로그램의 실행 파일. 모르면 `None`.
    fn process_path(&self) -> Option<PathBuf>;

    /// `Home.Default.Directory` — 업무 홈(`%LOCALAPPDATA%\Pclm`). 내장 확장은 언제나 여기 둔다.
    fn default_home(&self) -> io::Result<PathBuf>;

    /// `Environment.GetFolderPath(folder)`. 없으면 빈 경로.
    fn special_folder(&self, folder: SpecialFolder) -> PathBuf;

    /// `Path.GetTempPath()` — 압축 안에서 켠 창인지 가린다(자동 실행).
    fn temp_path(&self) -> PathBuf;

    /// `root.OpenSubKey(key)?.GetValue(name)` — 키나 값이 없으면 `None`. `name` 이 빈 글이면 기본값이다.
    fn registry_get(
        &self,
        root: RegistryRoot,
        key: &str,
        name: &str,
    ) -> io::Result<Option<RegistryValue>>;

    /// `Registry.CurrentUser.CreateSubKey(key).SetValue(name, value)` — 글 값(`REG_SZ`)을 적는다. 키가 없으면 짓는다.
    fn registry_set_string(&self, key: &str, name: &str, value: &str) -> io::Result<()>;

    /// `Registry.CurrentUser.OpenSubKey(key, writable: true)?.DeleteValue(name, throwOnMissingValue: false)` — 키나 값이
    /// 없으면 아무것도 하지 않는다.
    fn registry_delete_value(&self, key: &str, name: &str) -> io::Result<()>;

    /// `Process.Start(new ProcessStartInfo(path) { UseShellExecute = true })` — 폴더면 탐색기로 연다.
    fn shell_open(&self, path: &Path) -> io::Result<()>;

    /// `Process.Start` — `shell` 이면 `UseShellExecute = true`. 기다리지 않는다.
    fn start_process(&self, program: &Path, args: &[String], shell: bool) -> io::Result<()>;

    /// `Activation.Signal` — 그 이름의 신호를 듣는 창이 있으면 앞으로 내는 권리를 넘기고(`AllowSetForegroundWindow`) 울린다.
    /// 듣는 창이 없으면 거짓.
    fn signal_activation(&self, name: &str) -> bool;

    /// `ExtensionSetup.Windows` 의 손 — 그 이름(`chrome` · `msedge`)의 프로세스들이 가진 지금 최상위 창들.
    fn browser_windows(&self, process: &str) -> Vec<isize>;

    /// `ExtensionSetup.Raise` 의 손 — `before` 뒤에 새로 선 그 브라우저의 창을 `wait` 동안 기다려 앞으로 가져온다. 서지
    /// 않았거나 앞으로 오지 않았으면 거짓.
    fn raise_new_window(&self, process: &str, before: &[isize], wait: Duration) -> bool;

    /// `ExtensionSetup.Navigate` 의 손 — 새로 뜬 창의 주소창에 `url` 을 적고 Enter. 그 창이 정말 앞에 왔을 때만 누른다.
    fn navigate_new_window(&self, process: &str, before: &[isize], url: &str) -> bool;

    /// `Clipboard.SetText` — 실패는 삼킨다.
    fn set_clipboard(&self, text: &str);

    /// `FileVersionInfo.GetVersionInfo(path).FileVersion` — 없으면 `None`.
    fn file_version(&self, path: &Path) -> Option<String>;
}

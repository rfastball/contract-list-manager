//! 창이 다리에 다는 것(C# `Bridge` 의 `Func`·`Action` 속성) — 고르기 창, 다시 띄우기·닫기, 알림 영역 두기.
//!
//! **창(⑧b 의 껍데기)이 이것 하나만 구현한다.** 무엇을 고르라는지(제목·거르개·지어 둔 이름), 언제 다시 띄우고 닫는지는 다리가
//! 정해 건넨다 — 창은 대화상자를 띄우고 고른 자리를 돌려주거나, 받은 일을 그대로 할 뿐이다. 달지 않은 자리는 C# 에서 그
//! 속성이 비었을 때와 같다: 고르기는 그만둔 것(`None`), 다시 띄우기·닫기·알림 영역은 아무것도 하지 않는다.
//!
//! 운영체제에 곧바로 하는 일(등록부·탐색기·새 프로세스·브라우저 창)은 여기가 아니라 [`crate::Os`] 다 — C# 다리도 그것은
//! 창에 맡기지 않고 스스로 했다.

/// C# `FolderPrompt(Title, Initial)` — 폴더 고르기. 처음 열 자리가 없거나 없는 폴더면 창이 정한 자리에서 연다.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FolderPrompt {
    pub title: String,
    pub initial: Option<String>,
}

/// C# `SavePrompt(Title, Filter, DefaultExt, FileName, AllowOverwrite = true)` — 저장할 자리를 고르게 할 때 건네는 것.
///
/// 거르개는 Win32 꼴(`이름|*.확장자`)이다. `allow_overwrite` 가 거짓이면 대화상자가 덮어쓰기를 묻지 않는다 — 작업자료를
/// 새로 세우는 자리는 무엇이 있든 덮지 않으므로 「바꾸시겠습니까?」 에 「예」 를 받은 뒤 거절하게 두지 않는다. C# 창은
/// 문서 폴더에서 열고(`InitialDirectory = MyDocuments`) 확장자를 붙인다(`AddExtension`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavePrompt {
    pub title: String,
    pub filter: String,
    pub default_ext: String,
    /// 지어 둔 이름.
    pub file_name: String,
    pub allow_overwrite: bool,
}

/// 파일 하나 고르기(C# `PickPlanExcel` · `PickOpenFile` · `PickOpenJson`). 하나만 고른다(`Multiselect = false`).
///
/// C# 창은 계획 엑셀 말고는 문서 폴더에서 연다. 제목·거르개는 다리가 정한다 — C# 에서는 계획 엑셀의 제목과 세 거르개가
/// 창(App.xaml.cs) 쪽에 적혀 있었는데, 창이 정할 것이 남지 않게 여기로 옮겼다(글은 같다).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenPrompt {
    pub title: String,
    pub filter: String,
}

/// 창이 다는 것. 모든 메서드에 「달지 않음」 의 기본이 있다.
///
/// 다리는 여러 실마리에서 불릴 수 있다(C# 은 요청마다 `Task.Run`) — 그래서 `Sync` 다. 고르기 창은 창의 UI 실마리에서 띄워
/// 답을 기다려 돌려준다(C# `OnUiThread`).
pub trait Host: Sync {
    /// C# `PickFolder` — 폴더 고르기 창. 그만두면 `None`.
    fn pick_folder(&self, prompt: &FolderPrompt) -> Option<String> {
        let _ = prompt;
        None
    }

    /// C# `PickSavePath` — 저장할 자리. 그만두면 `None`.
    fn pick_save_path(&self, prompt: &SavePrompt) -> Option<String> {
        let _ = prompt;
        None
    }

    /// C# `PickPlanExcel` — 계획 엑셀(`*.xlsx`) 고르기. 그만두면 `None`.
    fn pick_plan_excel(&self, prompt: &OpenPrompt) -> Option<String> {
        let _ = prompt;
        None
    }

    /// C# `PickOpenFile` — `.pclm` 고르기(열어 볼 자료 · 작업자료로 쓸 파일). 그만두면 `None`.
    fn pick_open_file(&self, prompt: &OpenPrompt) -> Option<String> {
        let _ = prompt;
        None
    }

    /// C# `PickOpenJson` — 보기 구성(`*.json`) 고르기. 그만두면 `None`.
    fn pick_open_json(&self, prompt: &OpenPrompt) -> Option<String> {
        let _ = prompt;
        None
    }

    /// C# `Restart` — 이 창을 다시 띄운다(ADR-032 의 5). 작업자료를 바꾼 요청의 **답을 보낸 뒤에** 불린다
    /// ([`crate::Bridge::handle`]). 곧바로 내리지 말고 창의 차례로 미룬다 — 보낸 답이 화면에 닿을 틈을 둔다.
    fn restart(&self) {}

    /// C# `CloseWindow` — 화면이 확인을 받은 뒤의 「정말 닫기」. 답을 보낸 뒤에 불린다. 다시 띄우기와 같은 까닭으로 미룬다.
    fn close_window(&self) {}

    /// C# `ApplyCloseToTray` — 닫기를 알림 영역으로 돌릴지 **지금** 바꾼다(아이콘을 곧바로 세우거나 거둔다). 달지 않으면
    /// 적어 두기만 하고 다음에 띄울 때 선다.
    fn apply_close_to_tray(&self, on: bool) {
        let _ = on;
    }
}

/// 아무것도 달지 않은 창 — C# 에서 속성을 하나도 달지 않은 다리.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoHost;

impl Host for NoHost {}

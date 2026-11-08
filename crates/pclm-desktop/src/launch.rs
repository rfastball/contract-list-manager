//! 창을 어떻게 띄우는가 — C# `App.OnStartup` 의 인자 규칙(어느 홈 · 무엇을 열람 · 다시 띄울 때 넘길 것 · 창 제목).
//!
//! 글은 C# 의 `InvalidOperationException` 글과 같다. 거절은 [`crate`] 를 부르는 창이 오류 기록과 알림 상자로 보인다.

use std::path::Path;

use pclm_core::PclmRole;
use pclm_home::Home;

/// 작업자료를 바꾼 창이 띄운 새 창(C# `--restarted`). 옛 창이 내려가며 잠금을 놓는 것과 겹칠 수 있어 잠깐 기다린다.
pub const RESTARTED: &str = "--restarted";

/// 로그인할 때 Windows 가 켠 것(C# `Autostart.BackgroundFlag`). 사람이 띄운 것이 아니라 창을 앞에 내지 않는다.
pub use pclm_bridge::BACKGROUND_FLAG;

/// 어느 홈을 보는가.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HomeChoice {
    /// 업무 홈(`%LOCALAPPDATA%\Pclm`).
    Default,
    /// `--erp-dev` — 개발 호스트가 보는 홈.
    Development,
    /// `--home <폴더>` — 개발 · 시험 격리.
    Directory(String),
}

/// 인자에서 읽은 창의 모양.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Launch {
    pub home: HomeChoice,
    /// `--ui <origin>` — 지어 둔 화면 대신 이 자리(개발 서버)를 연다.
    pub ui: Option<String>,
    /// 다시 띄울 때 넘길 인자(C# `_restartArgs`). 같은 홈 · 같은 화면 자리로 다시 서야 한다 — `--view` 와 `--background` 는
    /// 넘기지 않는다(바꾸는 것은 본 창뿐이고, 사람이 보던 창에서 바꿨으니 다시 선 창도 보여야 한다).
    pub restart_args: Vec<String>,
    pub restarted: bool,
    pub background: bool,
}

impl Launch {
    pub fn erp_dev(&self) -> bool {
        self.home == HomeChoice::Development
    }
}

/// C# `App.Option` — 그 이름의 첫 자리 바로 뒤의 인자. 없으면 `None`(뒤가 다른 깃발이어도 그것을 값으로 읽는다).
pub fn option<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    let at = args.iter().position(|a| a == name)?;
    args.get(at + 1).map(String::as_str)
}

fn has(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

/// 어느 홈을 볼지 정한다(C# 차례 그대로). `--db` 는 걷었다 — 모르는 깃발로 흘려보내면 그 사람은 시험 자료를 연 줄 알고
/// 실제 자료를 고친다.
pub fn parse(args: &[String]) -> Result<Launch, String> {
    if has(args, "--db") {
        return Err("--db 는 없어졌습니다. 시험 자료는 --home <폴더> 로 따로 띄우세요.".into());
    }
    let home_arg = option(args, "--home");
    let erp_dev = has(args, "--erp-dev");
    if has(args, "--home") && home_arg.is_none() {
        return Err("--home 뒤에 폴더를 적으세요.".into());
    }
    if erp_dev && home_arg.is_some() {
        return Err("--erp-dev 는 개발 홈을 씁니다. --home 과 함께 쓸 수 없습니다.".into());
    }
    let home = match (erp_dev, home_arg) {
        (true, _) => HomeChoice::Development,
        (false, Some(directory)) => HomeChoice::Directory(directory.to_string()),
        (false, None) => HomeChoice::Default,
    };
    let ui = option(args, "--ui").map(str::to_string);
    let mut restart_args = Vec::new();
    if erp_dev {
        restart_args.push("--erp-dev".to_string());
    }
    if let Some(directory) = home_arg {
        restart_args.extend(["--home".to_string(), directory.to_string()]);
    }
    if let Some(ui) = &ui {
        restart_args.extend(["--ui".to_string(), ui.clone()]);
    }
    Ok(Launch {
        home,
        ui,
        restart_args,
        restarted: has(args, RESTARTED),
        background: has(args, BACKGROUND_FLAG),
    })
}

/// 열어 볼 파일(C#). `--view <파일>` 이거나, 첫 인자로 놓인 파일 하나다(파일 연결이 그렇게 부른다). 확장자는 보지 않는다 —
/// 메일을 지나며 이름이 바뀐 것을 「연결 프로그램」 으로 고르면 그대로 와야 하고, PCLM 파일인지는 여는 쪽이 표지로 가린다.
pub fn view(args: &[String], exists: impl Fn(&str) -> bool) -> Result<Option<String>, String> {
    let view = option(args, "--view");
    if has(args, "--view") && view.is_none() {
        return Err("--view 뒤에 열어 볼 파일을 적으세요.".into());
    }
    if let Some(view) = view {
        return Ok(Some(view.to_string()));
    }
    Ok(args
        .first()
        .filter(|first| !first.starts_with("--") && exists(first))
        .cloned())
}

/// `File.Exists` — 파일이 있는가(폴더는 아니다).
pub fn file_exists(path: &str) -> bool {
    !path.is_empty() && Path::new(path).is_file()
}

/// 작업자료 창의 제목(C#). 개발 홈 · 시험 홈이면 어느 홈인지 제목에 보인다.
pub fn main_title(erp_dev: bool, home: &Home) -> String {
    if erp_dev {
        "계약 목록 — ERP 개발 DB".into()
    } else if !home.is_default() {
        format!("계약 목록 — {}", home.directory().display())
    } else {
        "계약 목록".into()
    }
}

/// 열람 창의 제목(C# `$"계약 목록 — 열람: {Path.GetFileName(SourcePath)} ({PclmRole.Name(Role)})"`).
pub fn viewer_title(source: &Path, role: &str) -> String {
    let name = source
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    format!("계약 목록 — 열람: {name} ({})", PclmRole::name(Some(role)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn 홈은_차례대로_가리고_거절_글도_같다() {
        assert_eq!(
            parse(&args(&["--home", "D:/x", "--db", "a.db"])).unwrap_err(),
            "--db 는 없어졌습니다. 시험 자료는 --home <폴더> 로 따로 띄우세요."
        );
        assert_eq!(
            parse(&args(&["--home"])).unwrap_err(),
            "--home 뒤에 폴더를 적으세요."
        );
        assert_eq!(
            parse(&args(&["--erp-dev", "--home", "D:/x"])).unwrap_err(),
            "--erp-dev 는 개발 홈을 씁니다. --home 과 함께 쓸 수 없습니다."
        );
        assert_eq!(parse(&args(&[])).unwrap().home, HomeChoice::Default);
        assert_eq!(
            parse(&args(&["--erp-dev"])).unwrap().home,
            HomeChoice::Development
        );
        // 뒤가 깃발이어도 값으로 읽는다(C# Option).
        assert_eq!(
            parse(&args(&["--home", "--ui", "http://localhost:5173/"]))
                .unwrap()
                .home,
            HomeChoice::Directory("--ui".into())
        );
    }

    #[test]
    fn 다시_띄울_인자는_홈과_화면_자리만_넘긴다() {
        let launch = parse(&args(&[
            "--background",
            "--home",
            "D:/시험 홈",
            "--view",
            "D:/받은.pclm",
            "--ui",
            "http://localhost:5173/",
            "--restarted",
        ]))
        .unwrap();
        assert_eq!(
            launch.restart_args,
            args(&["--home", "D:/시험 홈", "--ui", "http://localhost:5173/"])
        );
        assert!(launch.background && launch.restarted);
        assert_eq!(
            parse(&args(&["--erp-dev"])).unwrap().restart_args,
            args(&["--erp-dev"])
        );
    }

    #[test]
    fn 열람은_view_이거나_첫_인자의_파일이다() {
        let any = |_: &str| true;
        let none = |_: &str| false;
        assert_eq!(
            view(&args(&["--view"]), any).unwrap_err(),
            "--view 뒤에 열어 볼 파일을 적으세요."
        );
        assert_eq!(
            view(&args(&["--view", "a.pclm"]), none).unwrap(),
            Some("a.pclm".into())
        );
        assert_eq!(
            view(&args(&["받은.bin", "--home", "D:/x"]), any).unwrap(),
            Some("받은.bin".into())
        );
        assert_eq!(view(&args(&["받은.bin"]), none).unwrap(), None);
        assert_eq!(view(&args(&["--home", "D:/x"]), any).unwrap(), None);
    }

    #[test]
    fn 열람_제목은_원본_이름과_역할이다() {
        assert_eq!(
            viewer_title(
                Path::new(r"D:\받은\홍길동_제출본.pclm"),
                PclmRole::SUBMISSION
            ),
            "계약 목록 — 열람: 홍길동_제출본.pclm (제출본)"
        );
    }
}

//! 브라우저가 띄우는 native messaging 호스트 — C# `src/Pclm.App/Program.cs` 의 호스트 갈래와 같은 일을 한다.
//!
//! 몸은 이 라이브러리의 [`run`] 이다. 따로 지은 `pclm-erp-host.exe`(동등성 계기가 쓴다)와 창 exe(`pclm-desktop`, 배포되는
//! `계약목록.exe`)가 같은 것을 부른다 — 창 exe 는 UI 를 세우기 전에 [`is_host_launch`] 로 갈라 여기로 온다.
//!
//! ```text
//! pclm-erp-host chrome-extension://<id>/ [--parent-window=…]   브라우저가 등록으로 띄운다
//! pclm-erp-host --native-dev [--home <폴더>]                    손으로 띄운 개발 호스트(등록을 보지 않는다)
//! ```
//!
//! 실행 파일 이름이 `-erp-dev` 로 끝나면 개발 홈(`Pclm.Erp.Dev`)을 본다. `--home` 은 격리용이다 — C# 에는 없고 동등성 계기가
//! 같은 홈을 두 구현에 보이려고 쓴다. 종료 코드: 0 정상, 1 틀·입출력 오류, 2 허락되지 않은 확장, 64 쓰는 법이 틀림.
//! stdout 에는 틀 지은 메시지 말고는 아무것도 쓰지 않는다.

use std::path::Path;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};

use pclm_core::erp::capture::ErpCapture;
use pclm_home::{Home, SystemEnv, dotnet, win};
use pclm_host::executable::{ExecutableIdentity, manifest_points_at};
use pclm_host::presence::{self, Options};
use pclm_host::status::HelloFiles;
use pclm_host::{DEVELOPMENT_HOST_NAME, HOST_NAME, native_messaging};
use serde_json::Value;

const ORIGIN_PREFIX: &str = "chrome-extension://";
const NATIVE_DEV: &str = "--native-dev";

/// C# `Program.Main` 의 첫 갈래 — 브라우저가 띄웠거나(`chrome-extension://…` 이 첫 인자) 손으로 띄운 개발 호스트(`--native-dev`)
/// 인가. 그러면 창·대화상자를 세우기 **전에** [`run`] 으로 간다.
pub fn is_host_launch(args: &[String]) -> bool {
    args.first().is_some_and(|a| a.starts_with(ORIGIN_PREFIX))
        || args.iter().any(|a| a == NATIVE_DEV)
}

/// 호스트 한 번 — 인자는 프로그램 이름을 뺀 것. 종료 코드: 0 정상, 1 틀·입출력 오류, 2 허락되지 않은 확장, 64 쓰는 법이 틀림.
pub fn run(mut args: Vec<String>) -> ExitCode {
    let home_override = match take_home(&mut args) {
        Ok(home) => home,
        Err(()) => return usage(),
    };
    let native_dev = args.iter().any(|a| a == NATIVE_DEV);
    let origin = args.first().filter(|a| a.starts_with(ORIGIN_PREFIX));
    // C# 은 둘 다 없으면 창을 띄운다. 이 몸에는 창이 없다 — 창 exe 는 [`is_host_launch`] 로 미리 가른다.
    if origin.is_none() && !native_dev {
        return usage();
    }
    let development = native_dev || process_stem().ends_with("-erp-dev");
    // 업무·개발 호스트는 홈만 다르다. 쪽지를 읽기만 하고 쓰지 않는다.
    let home = match home_override {
        Some(directory) => Home::new(directory),
        None if development => Home::development(),
        None => Home::default_home(),
    };
    let Ok(home) = home else {
        return ExitCode::from(1);
    };
    if !native_dev && !home.is_allowed_origin(&args[0]) {
        return ExitCode::from(2);
    }
    let environment = if development {
        "development"
    } else {
        "production"
    };
    let registered: Option<Arc<dyn Fn() -> std::io::Result<bool> + Send + Sync>> = if native_dev {
        None // 손으로 띄운 개발 호스트는 등록으로 뜬 것이 아니라 등록을 보지 않는다.
    } else {
        let name = if development {
            DEVELOPMENT_HOST_NAME
        } else {
            HOST_NAME
        };
        Some(Arc::new(move || still_registered(name)))
    };
    let options = Options {
        watch: ExecutableIdentity::of_this_process(),
        registered,
        ..Options::default()
    };
    let directory = home.directory().to_path_buf();
    let hello = HelloFiles {
        directory: directory.clone(),
    };
    // 상시 연결 인사는 작업자료를 열기 전에 serve 가 받는다 — 포트를 쥔 채 브라우저만큼 사는 호스트가 DB 를 쥐면 안 된다.
    let dispatch = |request: &Value| -> Value {
        match home.open_bound() {
            Ok(database) => ErpCapture::new(database.path(), &SystemEnv)
                .with_environment(environment)
                .with_destination(&home)
                .with_extension(&hello)
                .dispatch(request, false),
            // 요청은 확장이 보낸 것이라 모양을 믿지 않는다 — 여기서 멈추면 메시지 고리 밖으로 새어 호스트가 죽는다.
            Err(_) => setup_required(request),
        }
    };
    let output = Arc::new(Mutex::new(std::io::stdout()));
    match presence::serve(std::io::stdin(), output, &directory, dispatch, options) {
        Ok(()) => ExitCode::SUCCESS,
        // 길이 오류·끊긴 파이프: stdout 에 진단문이나 원문을 섞지 않는다.
        Err(_) => ExitCode::from(1),
    }
}

/// `--home <폴더>` 를 빼낸다. 값이 없으면 쓰는 법이 틀렸다.
fn take_home(args: &mut Vec<String>) -> Result<Option<String>, ()> {
    let Some(at) = args.iter().position(|a| a == "--home") else {
        return Ok(None);
    };
    if at + 1 >= args.len() {
        return Err(());
    }
    let directory = args.remove(at + 1);
    args.remove(at);
    Ok(Some(directory))
}

fn usage() -> ExitCode {
    eprintln!("쓰는 법: pclm-erp-host chrome-extension://<id>/ | --native-dev [--home <폴더>]");
    ExitCode::from(64)
}

/// `Path.GetFileNameWithoutExtension(Environment.ProcessPath)`.
fn process_stem() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_default()
}

/// C# 의 catch 가 내는 답과 같은 꼴 — `requestId` 는 요청이 객체이고 그 값이 글일 때만 싣는다.
fn setup_required(request: &Value) -> Value {
    let request_id = request
        .as_object()
        .and_then(|o| o.get("requestId"))
        .and_then(Value::as_str);
    native_messaging::envelope(
        request_id,
        "setup_required",
        "메인 프로그램에서 저장 대상과 확장 연결을 준비하세요.",
    )
}

/// Chrome·Edge 의 등록 중 하나라도 이 exe 를 가리키는가(C# `Program.StillRegistered`). 하나도 읽히지 않으면 가리킨다고 친다.
fn still_registered(host_name: &str) -> std::io::Result<bool> {
    let Ok(this) = std::env::current_exe() else {
        return Ok(true);
    };
    let mut seen = false;
    for browser in ["Google\\Chrome", "Microsoft\\Edge"] {
        let key = format!("Software\\{browser}\\NativeMessagingHosts\\{host_name}");
        let Some(manifest) = win::current_user_default_value(&key)? else {
            continue;
        };
        // File.Exists — 빈 글·폴더·잘못된 경로는 없는 것으로 본다.
        if manifest.is_empty() || !Path::new(&manifest).is_file() {
            continue;
        }
        seen = true;
        if manifest_points_at(&dotnet::read_all_text(Path::new(&manifest))?, &this) {
            return Ok(true);
        }
    }
    Ok(!seen)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn 홈_인자는_빼내고_나머지_차례를_지킨다() {
        let mut list = args(&[
            "chrome-extension://abc/",
            "--home",
            "D:/격리",
            "--parent-window=0",
        ]);
        assert_eq!(take_home(&mut list), Ok(Some("D:/격리".into())));
        assert_eq!(
            list,
            args(&["chrome-extension://abc/", "--parent-window=0"])
        );
        let mut none = args(&["--native-dev"]);
        assert_eq!(take_home(&mut none), Ok(None));
        assert_eq!(take_home(&mut args(&["--native-dev", "--home"])), Err(()));
    }

    /// C# `Program.Main` 의 갈래 — 첫 인자가 확장 출처이거나 어디든 `--native-dev` 가 있으면 호스트다. 그 밖(파일 · 깃발)은 창.
    #[test]
    fn 호스트로_뜬_것만_가른다() {
        assert!(is_host_launch(&args(&[
            "chrome-extension://abc/",
            "--parent-window=0"
        ])));
        assert!(is_host_launch(&args(&[
            "--home",
            "D:/격리",
            "--native-dev"
        ])));
        assert!(!is_host_launch(&args(&[
            "--home",
            "chrome-extension://abc/"
        ])));
        assert!(!is_host_launch(&args(&["D:/받은 자료.pclm"])));
        assert!(!is_host_launch(&args(&[])));
    }

    #[test]
    fn 준비가_안_된_답은_글인_요청_번호만_싣는다() {
        let id = "00000000-0000-4000-8000-000000000001";
        assert_eq!(setup_required(&json!({ "requestId": id }))["requestId"], id);
        assert_eq!(
            setup_required(&json!({ "requestId": 3 }))["requestId"],
            Value::Null
        );
        assert_eq!(setup_required(&json!([1, 2]))["requestId"], Value::Null);
        let reply = setup_required(&json!("글"));
        assert_eq!(reply["protocolVersion"], 2);
        assert_eq!(reply["ok"], false);
        assert_eq!(reply["error"]["code"], "setup_required");
    }
}

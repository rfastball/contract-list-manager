//! `pclm` — C# `src/Pclm.Cli` 와 같은 명령줄(#8 ⑦). 일은 [`pclm_cli::run`] 이 하고, 여기서는 진짜 시계·시간대·업무 홈을
//! 넣고 표준 출력·오류를 잇는다.

use std::io::Write as _;

use pclm_cli::{Context, run};
use pclm_home::{Home, SystemEnv};

fn main() {
    // C# 은 UTF-16 인자를 받는다. 짝이 맞지 않는 대리 글자는 .NET 처럼 U+FFFD 로 바꾼다.
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let env = SystemEnv;
    let default_home = Home::default_home;
    let context = Context {
        env: &env,
        zone: &env,
        default_home: &default_home,
    };
    let stdout = std::io::stdout();
    let stderr = std::io::stderr();
    let mut out = stdout.lock();
    let mut err = stderr.lock();
    let code = run(&args, &context, &mut out, &mut err);
    let _ = out.flush();
    let _ = err.flush();
    std::process::exit(code);
}

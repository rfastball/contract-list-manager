//! 배포되는 `계약목록.exe`. 브라우저가 띄운 호스트면 창을 세우기 **전에** 갈라 확장 호스트로 돈다(C# `Program.Main`).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if pclm_erp_host::is_host_launch(&args) {
        return pclm_erp_host::run(args);
    }
    pclm_desktop::shell::run(args)
}

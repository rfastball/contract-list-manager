//! 따로 지은 호스트 실행 파일 — 몸은 [`pclm_erp_host::run`] 이다(창 exe 도 같은 것을 부른다).

use std::process::ExitCode;

fn main() -> ExitCode {
    pclm_erp_host::run(std::env::args().skip(1).collect())
}

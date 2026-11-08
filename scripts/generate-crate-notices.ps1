<#
.SYNOPSIS
    THIRD-PARTY-NOTICES.txt 의 「Rust 크레이트」 절을 다시 짓는다 — 배포 exe(계약목록.exe)에 실려 나가는 크레이트의 고지.

.DESCRIPTION
    실리는 목록은 cargo tree 에 묻는다: pclm-desktop 을 shell 기능으로, windows 대상(x86_64-pc-windows-msvc)에서,
    운영 의존만(build · dev 의존과 proc-macro 는 짓는 동안에만 돈다). 저작권 줄은 크레이트의 라이선스 파일에서 옮기고,
    MIT · Apache-2.0 은 문서 끝의 전문을 가리키며, 그 밖의 라이선스는 그 파일을 항목 안에 싣는다.
    짓는 것은 crates/pclm-desktop/examples/crate_notices.rs 이고, 대조는 crates/pclm-desktop/tests/notices.rs 가 한다 —
    의존을 더하거나 판을 올리고 이것을 돌리지 않으면 cargo test 가 붉어진다.

    사람이 살펴야 할 자리(라이선스 파일이 없는 새 크레이트, WebView2 · SQLite 바인딩의 판이 바뀜)에서는 멈춘다.
    지은 뒤 git diff 로 눈으로 보고 커밋한다.

.EXAMPLE
    ./scripts/generate-crate-notices.ps1
#>
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path -Parent $PSScriptRoot)   # 저장소 뿌리
$notices = Join-Path (Get-Location).Path 'THIRD-PARTY-NOTICES.txt'

# cargo 는 진행을 stderr 로 쓴다 — 출력을 받아 내도 'Stop' 이 그 줄에서 멈추지 않게 종료 코드로만 가린다.
& { $ErrorActionPreference = 'Continue'; cargo run -q -p pclm-desktop --example crate_notices -- $notices }
if ($LASTEXITCODE -ne 0) { throw '크레이트 고지를 짓지 못했습니다.' }

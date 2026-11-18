<#
.SYNOPSIS
    문서 만들기 엔진(crates/pclm-doc-engine)을 wasm 으로 지어 extension/engine.wasm 에 놓는다.

.DESCRIPTION
    확장의 「문서 만들기」 사이드 패널이 이 wasm 을 부른다(ADR-045). 파일은 빌드 산출물이라 저장소에 없고(.gitignore),
    pclm-bridge 의 build.rs 가 extension/ 아래를 통째로 exe 에 박으므로 창을 짓기 **전에** 이것이 먼저 돌아야 한다 —
    run.ps1 · publish.ps1 이 그 차례를 쥔다. 건너뛰면 엔진이 빠졌거나 옛 엔진이 든 확장이 나간다.

    짓는 대상(wasm32-unknown-unknown)은 rust-toolchain.toml 의 targets 가 받아 둔다. --locked 로 짓는다 — Cargo.lock 과
    어긋나면 조용히 다른 판을 받지 않고 멈춘다.

.EXAMPLE
    ./scripts/build-engine.ps1
#>
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path -Parent $PSScriptRoot)   # 저장소 뿌리

$built = 'target/wasm32-unknown-unknown/release/pclm_doc_engine.wasm'
$shipped = 'extension/engine.wasm'

# cargo 는 진행을 stderr 로 쓴다 — 'Stop' 이 그 줄에서 멈추지 않게 종료 코드로만 가린다.
& { $ErrorActionPreference = 'Continue'; cargo build --release --locked -p pclm-doc-engine --target wasm32-unknown-unknown }
if ($LASTEXITCODE -ne 0) { throw '문서 만들기 엔진 빌드(cargo build --target wasm32-unknown-unknown)가 실패했습니다.' }
if (-not (Test-Path -LiteralPath $built)) { throw "엔진이 나오지 않았습니다: $built" }

# 같은 바이트면 다시 쓰지 않는다 — build.rs 가 바뀐 것으로 보고 exe 를 다시 짓지 않게.
$same = (Test-Path -LiteralPath $shipped) -and
        ((Get-FileHash -LiteralPath $built).Hash -eq (Get-FileHash -LiteralPath $shipped).Hash)
if (-not $same) { Copy-Item -LiteralPath $built -Destination $shipped -Force }

$size = (Get-Item -LiteralPath $shipped).Length
Write-Host ("엔진  {0}  ({1:N1} MB)" -f $shipped, ($size / 1MB)) -ForegroundColor DarkGray

<#
.SYNOPSIS
    배포물을 짓는다 — 화면부터 짓고, 그다음 실행 파일.

.DESCRIPTION
    나오는 것은 파일 하나다 — dist/계약목록/계약목록.exe.

    창은 crates/pclm-desktop(Rust + Tauri 2, ADR-044)이고, 같은 exe 가 브라우저 확장의 native
    messaging 호스트도 겸한다. SQLite 는 정적으로 들어가고(rusqlite bundled), 화면(vite 산출물 artifacts/web)은
    빌드 때 exe 안에 박힌다(build.rs 의 include_bytes!) — 창이 메모리에서 내준다(shell/assets.rs).
    창을 띄우려면 Microsoft Edge WebView2 런타임이 필요하다.

    cargo 의 실행 파일 이름은 ASCII(pclm-desktop.exe)라, 여기서 계약목록.exe 로 놓는다.

    명령줄(pclm.exe)은 배포물에 넣지 않는다. 받는 쪽은 창만 쓴다. 개발할 때는 ./run.ps1 -Cli 로 돌린다.

    화면은 빌드 산출물이라 저장소에 없다. 이 스크립트가 cargo build 보다 먼저 vite build 를
    부르는 까닭이 그것이다 — 순서를 건너뛰면 화면 없는 앱이 나간다.

.EXAMPLE
    ./publish.ps1
    ./publish.ps1 -NoZip
#>
[CmdletBinding()]
param(
    [string]$Runtime = 'win-x64',   # 묶음 이름에만 쓴다 — 짓는 대상은 이 기계의 기본(x86_64-pc-windows-msvc)
    [string]$Output  = 'dist/계약목록',
    [switch]$NoZip
)

$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot   # 어디서 부르든 저장소 뿌리에서 돈다

function Need($command, $hint) {
    if (-not (Get-Command $command -ErrorAction SilentlyContinue)) {
        throw "$command 를 찾지 못했습니다. $hint"
    }
}

function Step($message) { Write-Host "`n== $message" -ForegroundColor Cyan }

Need cargo  'Rust(rustup)를 설치해 주세요.'
Need npm    'Node 를 설치해 주세요.'

# 판의 정본은 Cargo.toml 의 [workspace.package] version 이다. 크레이트는 모두 그것을 물려받고, 따로 적으면
# pclm-desktop 의 시험이 붉어진다.
$version = (Select-String -Path 'Cargo.toml' -Pattern '^version = "([^"]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
if (-not $version) { throw 'Cargo.toml 에서 판을 읽지 못했습니다.' }

# ── 화면 ──────────────────────────────────────────────────────────────────────
if (-not (Test-Path 'node_modules')) {
    Step '의존성 받기 (npm ci)'
    npm ci
    if ($LASTEXITCODE -ne 0) { throw 'npm ci 가 실패했습니다.' }
}

Step '화면 짓기 (vite build)'
npx vite build
if ($LASTEXITCODE -ne 0) { throw 'vite build 가 실패했습니다.' }

$screen = 'artifacts/web/index.html'
if (-not (Test-Path $screen)) { throw "화면이 나오지 않았습니다: $screen" }

# ── 실행 파일 ─────────────────────────────────────────────────────────────────
# 브라우저가 켜져 있으면 확장의 상시 연결 호스트가 지난 배포물의 exe 를 붙들고 있다(ADR-035) — 지우기·덮어쓰기는
# 막히고 이름 바꾸기는 된다. 그 exe 를 출력 폴더 **밖**(옆 폴더)으로 비켜 두고 진행한다. 안에 남기면 배포물이
# 파일 하나가 아니게 된다. 호스트는 제 exe 가 비켜진 것을 보고 떠나고, 확장이 그 자리의 새 exe 로 다시 붙는다.
# 지난번에 비켜 둔 것은 이제 놓였을 테니 먼저 치운다(아직 쥐고 있으면 다음 번으로).
$beside = Split-Path -Parent $Output
if (-not $beside) { $beside = '.' }
if (Test-Path -LiteralPath $beside) {
    Get-ChildItem -LiteralPath $beside -File | Where-Object { $_.Name -match '^계약목록\.old(-\d+)?\.exe$' } |
        ForEach-Object { try { Remove-Item -LiteralPath $_.FullName -Force -ErrorAction Stop } catch { } }
}
if (Test-Path $Output) {
    try { Remove-Item $Output -Recurse -Force -ErrorAction Stop }
    catch {
        $held = Join-Path $Output '계약목록.exe'
        if (-not (Test-Path -LiteralPath $held)) { throw }
        $aside = Join-Path $beside ('계약목록.old-{0}.exe' -f (Get-Date -Format 'yyyyMMddHHmmss'))
        Move-Item -LiteralPath $held -Destination $aside
        Write-Host "붙들린 exe 를 비켰습니다: $aside — 브라우저의 확장이 쥐고 있었습니다. 다음 배포 때 치웁니다." -ForegroundColor Yellow
        Remove-Item $Output -Recurse -Force
    }
}

Step '창 짓기 (cargo build --release -p pclm-desktop --features shell)'
# cargo 는 진행을 stderr 로 쓴다 — 출력을 받아 내도(*> 기록) 'Stop' 이 그 줄에서 멈추지 않게 종료 코드로만 가린다.
& { $ErrorActionPreference = 'Continue'; cargo build --release -p pclm-desktop --features shell }
if ($LASTEXITCODE -ne 0) { throw 'pclm-desktop 빌드가 실패했습니다.' }

New-Item -ItemType Directory -Path $Output -Force | Out-Null
Copy-Item -LiteralPath 'target/release/pclm-desktop.exe' -Destination (Join-Path $Output '계약목록.exe')

# ── 확인 ──────────────────────────────────────────────────────────────────────
# 배포물은 눈으로 못 보고 나가는 물건이라, 있어야 할 것이 있는지 여기서 따진다.
if (-not (Test-Path (Join-Path $Output '계약목록.exe'))) {
    throw "배포물에 계약목록.exe 가 없습니다."
}

# 나가는 것은 파일 하나여야 한다. 딸린 것이 하나라도 남으면 받는 쪽이 exe 만 복사해 가고,
# 그러면 화면이나 네이티브가 빠진 채로 돈다 — 여기서 크게 실패하는 편이 낫다.
$strays = Get-ChildItem $Output -Recurse | Where-Object { $_.Name -ne '계약목록.exe' }
if ($strays) {
    throw ("배포물에 계약목록.exe 말고 다른 것이 남았습니다: " +
           (($strays | ForEach-Object { $_.Name }) -join ', '))
}

# ── 묶기 ──────────────────────────────────────────────────────────────────────
if (-not $NoZip) {
    Step '묶기'
    $zip = "dist/계약목록_$version`_$Runtime.zip"
    if (Test-Path $zip) { Remove-Item $zip -Force }
    Compress-Archive -Path $Output -DestinationPath $zip
}

Write-Host ''
Get-ChildItem $Output -Recurse -File |
    Sort-Object Length -Descending |
    Select-Object -First 5 @{n='파일';e={Resolve-Path $_.FullName -Relative}},
                           @{n='MB';e={[math]::Round($_.Length / 1MB, 1)}} |
    Format-Table -AutoSize

Write-Host "계약 목록 $version — $Output" -ForegroundColor Green
if (-not $NoZip) { Write-Host $zip -ForegroundColor Green }

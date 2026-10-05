<#
.SYNOPSIS
    개발 중에 이 저장소를 돌린다 — 디버그 빌드로, 어느 자료를 여는지 적으면서.

.DESCRIPTION
    세 가지 모양이 있다.

        ./run.ps1              지어 둔 화면으로 창을 띄운다 (배포본과 같은 모양)
        ./run.ps1 -Dev         Vite 개발 서버로 창을 띄운다 (화면만 갈아 끼운다)
        ./run.ps1 -Cli status  명령줄을 돌린다

    -Dev 가 이 스크립트가 있는 까닭이다. `npm run dev` 는 브라우저에 가짜 다리(mock.ts)를
    물려 화면만 손보게 해 주는데, 그러면 **진짜 다리와 어긋난 것은 드러나지 않는다**.
    -Dev 는 창(WebView2)이 개발 서버를 열게 해서 화면은 고칠 때마다 바뀌고 다리와 DB 는
    진짜를 쓴다. 어긋남이 여기서 잡힌다. 다만 홈은 artifacts/dev-home 이다 — 진짜 DB 이되
    실제 자료는 아니다. 실제 홈에서 띄우려면 -HomeDir 로 짚는다.

    디버그 빌드에서는 개발자 도구가 열린다(F12). 배포본에는 그 문이 아예 없다 —
    깃발이 아니라 빌드 설정에 매달아 두었다(MainWindow.xaml.cs).

.PARAMETER HomeDir
    다른 홈을 연다(--home). 홈은 작업자료를 가리키는 쪽지·백업·WebView2 프로필이 사는 폴더다.
    -Dev 는 짚지 않아도 artifacts/dev-home 을 쓴다 — 화면을 고치며 넣고 지우는 것이 실제 자료에
    닿지 않게. 그 밖에는 짚지 않으면 창을 그냥 열었을 때와 같은 홈이다 — **실제 자료다.**
    처음 여는 홈이면 그 안에 빈 작업자료(계약자료.pclm)가 선다.

    (-Home 이 아닌 까닭: $Home 은 PowerShell 이 이미 쥔 자동 변수다.)

.EXAMPLE
    ./run.ps1 -Dev
    ./run.ps1 -HomeDir artifacts/dev-home
    ./run.ps1 -Cli status
#>
# PositionalBinding 을 끈다 — 켜 두면 "-Cli status" 의 status 가 자리 인자로 읽혀
# $Port 에 가서 붙는다. 이름 없는 인자는 모두 $Rest 로 흘러야 한다.
[CmdletBinding(PositionalBinding = $false)]
param(
    [switch]$Dev,
    [switch]$Cli,
    [string]$HomeDir,
    [int]$Port = 5173,
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$Rest
)

$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot   # 어디서 부르든 저장소 뿌리에서 돈다

# 한글이 깨지지 않게. 콘솔 코드페이지가 949 면 로그로 받아 낼 때 글자가 뭉개진다.
try { [Console]::OutputEncoding = [System.Text.Encoding]::UTF8 } catch { }

function Need($command, $hint) {
    if (-not (Get-Command $command -ErrorAction SilentlyContinue)) {
        throw "$command 를 찾지 못했습니다. $hint"
    }
}

function Step($message) { Write-Host "== $message" -ForegroundColor Cyan }

# 확장 호스트(업무·개발)가 이 저장소의 창 빌드 출력을 가리키면, 브라우저가 켜진 동안 상시 연결 호스트가 그 dll 을
# 붙들어 빌드가 덮어쓰지 못한다(ADR-035). 등록된 경로를 보고 그 폴더에서 **붙들린 것만** 같은 자리에서
# <이름>.old-<시각><확장자> 로 비킨다 — 이름 바꾸기는 붙들린 채로도 된다. 호스트는 제 어셈블리가 비켜진 것을 보고
# 떠나고, 확장이 새 빌드로 다시 붙는다. 빌드가 다시 만드는 것(dll 과 계약목록.exe)만 비킨다: 등록 스크립트가 만든
# 개발 호스트 사본(pclm-erp-dev.exe)은 빌드가 되살리지 않으므로 건드리지 않는다.
function Move-HeldOutputs {
    $bin = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot 'src/Pclm.App/bin'))
    $folders = @()
    foreach ($name in 'kr.rfastball.pclm.erp', 'kr.rfastball.pclm.erp.dev') {
        foreach ($browser in 'Google\Chrome', 'Microsoft\Edge') {
            $key = Get-Item -LiteralPath "HKCU:\Software\$browser\NativeMessagingHosts\$name" -ErrorAction SilentlyContinue
            if (-not $key) { continue }
            $manifest = $key.GetValue('')
            if (-not $manifest -or -not (Test-Path -LiteralPath $manifest)) { continue }
            try { $exe = (Get-Content -Raw -Encoding UTF8 -LiteralPath $manifest | ConvertFrom-Json).path } catch { continue }
            if (-not $exe) { continue }
            $folder = [System.IO.Path]::GetDirectoryName([System.IO.Path]::GetFullPath($exe))
            if ($folder.StartsWith($bin + [System.IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
                $folders += $folder
            }
        }
    }
    foreach ($folder in ($folders | Sort-Object -Unique)) { Move-HeldFiles $folder }
}

# 한 폴더에서: 지난번에 비켜 둔 것을 치우고(아직 쥐고 있으면 남긴다), 지금 붙들린 빌드 출력을 비킨다.
function Move-HeldFiles($folder) {
    if (-not (Test-Path -LiteralPath $folder)) { return }
    $stamp = Get-Date -Format 'yyyyMMddHHmmss'
    Get-ChildItem -LiteralPath $folder -File | Where-Object { $_.Name -match '\.old-\d+\.(dll|exe)$' } |
        ForEach-Object { try { Remove-Item -LiteralPath $_.FullName -Force -ErrorAction Stop } catch { } }
    Get-ChildItem -LiteralPath $folder -File |
        Where-Object { $_.Extension -eq '.dll' -or $_.Name -eq '계약목록.exe' } |
        ForEach-Object {
            try { [System.IO.File]::Open($_.FullName, 'Open', 'ReadWrite', 'None').Dispose(); return } catch { }
            $aside = Join-Path $folder ('{0}.old-{1}{2}' -f $_.BaseName, $stamp, $_.Extension)
            $file = $_.Name
            try {
                Move-Item -LiteralPath $_.FullName -Destination $aside -ErrorAction Stop
                Write-Host "붙들린 출력 비킴  $file → $(Split-Path -Leaf $aside)" -ForegroundColor DarkGray
            } catch {
                Write-Warning "$file 을 비키지 못했습니다: $($_.Exception.Message) — 브라우저를 닫고 다시 하세요."
            }
        }
}

Need dotnet '.NET 8 SDK 를 설치해 주세요.'

# 어느 홈을 여는지 늘 적는다. 디버그로 띄운 창이 조용히 실제 자료를 건드리는 일이
# 없게 — 정본은 Pclm.Core 의 Home.Default 다. 홈의 config.json 이 작업자료를 가리킨다.
if ($Dev -and -not $HomeDir) { $HomeDir = 'artifacts/dev-home' }
$homeArgs = @()
if ($HomeDir) {
    # Path.GetFullPath(경로, 기준) 은 .NET Framework 에 없다 — 5.1 에서는 여기서 터진다.
    # 창과 명령줄의 작업 폴더가 달라도 같은 홈을 보게 온전한 경로로 넘긴다.
    $rooted = if ([System.IO.Path]::IsPathRooted($HomeDir)) { $HomeDir }
              else { Join-Path (Get-Location).Path $HomeDir }
    $rooted = [System.IO.Path]::GetFullPath($rooted)

    $homeArgs = @('--home', $rooted)
    Write-Host "홈    $rooted" -ForegroundColor Yellow
} else {
    Write-Host "홈    $(Join-Path $env:LOCALAPPDATA 'Pclm')  (실제 자료)" -ForegroundColor Yellow
}

# ── 명령줄 ────────────────────────────────────────────────────────────────────
if ($Cli) {
    if (-not $Rest) { $Rest = @('help') }
    Step "명령줄 (pclm $($Rest -join ' '))"
    dotnet run --project src/Pclm.Cli -- @Rest @homeArgs
    exit $LASTEXITCODE
}

Need npm 'Node 를 설치해 주세요.'

if (-not (Test-Path 'node_modules')) {
    Step '의존성 받기 (npm install)'
    npm install
    if ($LASTEXITCODE -ne 0) { throw 'npm install 이 실패했습니다.' }
}

# ── 창 ────────────────────────────────────────────────────────────────────────
if (-not $Dev) {
    # 지어 둔 화면이 소스보다 오래되었으면 다시 짓는다. 이것을 사람에게 맡기면
    # 고친 화면이 아닌 옛 화면을 보면서 왜 안 바뀌는지 찾게 된다.
    $built = 'src/Pclm.App/wwwroot/index.html'
    $stale = $true

    if (Test-Path $built) {
        # node_modules 를 뺀다. vite 의 root 가 frontend/ 라 vitest 가 그 아래
        # node_modules/.vite 에 캐시를 쓰는데, 그것까지 세면 npm test 를 한 번 돌린 뒤로
        # 늘 "낡음"이 되어 이 검사가 아무 일도 하지 않게 된다.
        $builtAt = (Get-Item $built).LastWriteTimeUtc
        $newest = Get-ChildItem 'frontend', 'vite.config.mts', 'package.json' -Recurse -File |
            Where-Object { $_.FullName -notlike '*node_modules*' } |
            Sort-Object LastWriteTimeUtc -Descending | Select-Object -First 1
        $stale = $newest.LastWriteTimeUtc -gt $builtAt
    }

    if ($stale) {
        Step '화면 짓기 (vite build)'
        npx vite build
        if ($LASTEXITCODE -ne 0) { throw 'vite build 가 실패했습니다.' }
    } else {
        Write-Host '화면  지어 둔 것이 최신입니다' -ForegroundColor DarkGray
    }

    Step '창 띄우기 (Debug · 개발자 도구 F12)'
    Move-HeldOutputs
    dotnet run --project src/Pclm.App -- @homeArgs
    exit $LASTEXITCODE
}

# ── 창 + 개발 서버 ────────────────────────────────────────────────────────────
# vite 를 node 로 바로 띄운다. npx 를 거치면 중간 프로세스가 하나 더 생겨서
# 창이 닫힐 때 서버만 살아남는다.
$origin = "http://localhost:$Port/"
$vite = $null

try {
    Step "개발 서버 띄우기 ($origin)"
    $vite = Start-Process node `
        -ArgumentList 'node_modules/vite/bin/vite.js', '--port', $Port, '--strictPort' `
        -PassThru -NoNewWindow

    # 서버가 답하기 전에 창을 열면 빈 화면이 뜬다. 답할 때까지 기다린다.
    $ready = $false
    foreach ($attempt in 1..60) {
        if ($vite.HasExited) {
            throw "개발 서버가 곧바로 멈췄습니다. $Port 번을 이미 누가 쓰고 있는지 보세요."
        }

        try {
            Invoke-WebRequest -Uri $origin -UseBasicParsing -TimeoutSec 2 | Out-Null
            $ready = $true
            break
        } catch {
            Start-Sleep -Milliseconds 300
        }
    }

    if (-not $ready) { throw "개발 서버가 열리지 않았습니다: $origin" }

    Step '창 띄우기 (Debug · 화면은 개발 서버에서 · 다리와 DB 는 진짜)'
    Move-HeldOutputs
    dotnet run --project src/Pclm.App -- --ui $origin @homeArgs
    $code = $LASTEXITCODE
} finally {
    if ($vite -and -not $vite.HasExited) {
        Write-Host '개발 서버 내림' -ForegroundColor DarkGray
        Stop-Process -Id $vite.Id -Force -ErrorAction SilentlyContinue
    }
}

exit $code

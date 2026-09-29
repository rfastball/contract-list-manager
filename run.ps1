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
    진짜를 쓴다. 어긋남이 여기서 잡힌다.

    디버그 빌드에서는 개발자 도구가 열린다(F12). 배포본에는 그 문이 아예 없다 —
    깃발이 아니라 빌드 설정에 매달아 두었다(MainWindow.xaml.cs).

.PARAMETER DbPath
    다른 자료를 연다. 짚지 않으면 창을 그냥 열었을 때와 같은 자리다 —
    **실제 자료다.** 시험 삼아 넣고 지울 것이라면 ./run.ps1 -DbPath artifacts/debug.db 처럼 짚는다.

    (-Db 가 아닌 까닭: CmdletBinding 이 붙여 주는 -Debug 의 별칭이 이미 Db 다.)

.EXAMPLE
    ./run.ps1 -Dev
    ./run.ps1 -DbPath artifacts/debug.db
    ./run.ps1 -Cli status
#>
# PositionalBinding 을 끈다 — 켜 두면 "-Cli status" 의 status 가 자리 인자로 읽혀
# $Port 에 가서 붙는다. 이름 없는 인자는 모두 $Rest 로 흘러야 한다.
[CmdletBinding(PositionalBinding = $false)]
param(
    [switch]$Dev,
    [switch]$Cli,
    [string]$DbPath,
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

Need dotnet '.NET 8 SDK 를 설치해 주세요.'

# 어느 자료를 여는지 늘 적는다. 디버그로 띄운 창이 조용히 실제 자료를 건드리는 일이
# 없게 — 정본은 Pclm.Core 의 Database.DefaultPath 다.
$dbArgs = @()
if ($DbPath) {
    # Path.GetFullPath(경로, 기준) 은 .NET Framework 에 없다 — 5.1 에서는 여기서 터진다.
    $rooted = if ([System.IO.Path]::IsPathRooted($DbPath)) { $DbPath }
              else { Join-Path (Get-Location).Path $DbPath }

    $dbArgs = @('--db', $DbPath)
    Write-Host "자료  $([System.IO.Path]::GetFullPath($rooted))" -ForegroundColor Yellow
} else {
    Write-Host "자료  $(Join-Path $env:LOCALAPPDATA 'Pclm/pclm.db')  (실제 자료)" -ForegroundColor Yellow
}

# ── 명령줄 ────────────────────────────────────────────────────────────────────
if ($Cli) {
    if (-not $Rest) { $Rest = @('help') }
    Step "명령줄 (pclm $($Rest -join ' '))"
    dotnet run --project src/Pclm.Cli -- @Rest @dbArgs
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
    dotnet run --project src/Pclm.App -- @dbArgs
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
    dotnet run --project src/Pclm.App -- --ui $origin @dbArgs
    $code = $LASTEXITCODE
} finally {
    if ($vite -and -not $vite.HasExited) {
        Write-Host '개발 서버 내림' -ForegroundColor DarkGray
        Stop-Process -Id $vite.Id -Force -ErrorAction SilentlyContinue
    }
}

exit $code

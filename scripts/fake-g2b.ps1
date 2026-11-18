<#
.SYNOPSIS
    가짜 나라장터를 세우고 확장을 올린 브라우저를 띄운다 — 문서 만들기 패널을 실제로 써 보며 다듬으려고.

.DESCRIPTION
    실제 나라장터는 로그인이 있어 개발에 쓸 수 없다. 그래서 사이트만 가짜로 세우고 확장 · 네이티브 호스트 ·
    앱 · DB 는 진짜를 쓴다.

        ./scripts/fake-g2b.ps1             창 + 서버 + 브라우저 (브라우저를 닫으면 서버를 내리고 창을 닫는다)
        ./scripts/fake-g2b.ps1 -Port 9443  다른 포트

    하는 일:
      1. artifacts/fake-g2b/ 에 www.g2b.go.kr 자체 서명 인증서(pfx)를 짓는다 — 없을 때만. 인증서 저장소에는 남기지 않는다.
      2. ./run.ps1 -BuildOnly 로 화면 · 엔진(extension/engine.wasm) · 창을 짓는다 — 엔진을 고친 뒤에도 새것을 쓴다.
      3. 개발 호스트(kr.rfastball.pclm.erp.dev)를 target/debug/pclm-desktop.exe 와 **개발용 확장 ID** 로 등록한다.
      4. 개발 홈(%LOCALAPPDATA%\Pclm.Erp.Dev)의 창을 띄운다 — 처음이면 창이 작업자료를 세운다.
      5. scripts/fake-g2b/server.mjs 를 띄운다 — 127.0.0.1 의 HTTPS 정적 서버. extension/ 을 개발 홈의 extension/
         (업무 홈의 「확장 준비」 와 같은 자리 · 같은 꼴이라 창이 확장 상태를 읽는다)으로 뜨되(manifest 의 key 만 개발용)
         고칠 때마다 다시 뜬다.
         확장은 자동으로 다시 불러온다. 문서 패널이 열려 있으면 닫을 때까지 기다린다. 옛 확장은 최초 한 번 ↻ 가 필요할 수 있다.
      6. 브라우저마다 따로 쓰는 프로필(artifacts/fake-g2b/profile-msedge · profile-chrome)로 Edge(없으면 Chrome)를
         띄운다. www.g2b.go.kr 은 이 서버로 가고, 이 서버의 인증서만 받는다. Chrome 은 137 판쯤부터
         --load-extension 을 막아 처음 한 번 손으로 올려야 한다.

    가짜 페이지 위의 띠로 화면(접수 01112 · 공고 01179 · 계약 01579 · 수집하지 않는 화면)을 갈고, 차수를 올리고,
    로딩 표시를 켜고, 품목 줄 수를 바꾼다. 세 화면은 한 건이라 차례로 저장하면 접수 → 공고 → 계약이 이어진다.
    입력칸을 고치면 확장은 고친 값을 읽는다 — 앱의 「다릅니다」 단계를 밟아 볼 수 있다.

    복사본은 진짜 확장과 ID 가 달라, 제 ID 를 보고 개발 호스트에만 잇는다(extension/background.js 의
    DEVELOPMENT_ID). 업무용 호스트는 이 ID 를 받지 않는다 — 복사본이 진짜 확장의 ID 를 가지면 업무용 호스트에
    붙어 지어낸 건이 실제 작업자료에 저장된다.

    개발 호스트는 확장 ID 하나만 받는다(Pclm.Erp.Dev\extension-origin.txt) — 이 스크립트가 개발용 ID 로 등록한다.

.PARAMETER Port
    가짜 서버의 포트. 브라우저는 www.g2b.go.kr 의 어느 포트든 여기로 보낸다.

.EXAMPLE
    ./scripts/fake-g2b.ps1
#>
[CmdletBinding(PositionalBinding = $false)]
param(
    [int]$Port = 8443
)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
Set-Location $repo   # 어디서 부르든 저장소 뿌리에서 돈다

try { [Console]::OutputEncoding = [System.Text.Encoding]::UTF8 } catch { }

function Step($message) { Write-Host "== $message" -ForegroundColor Cyan }

if (-not (Get-Command node -ErrorAction SilentlyContinue)) { throw 'node 를 찾지 못했습니다. Node 를 설치해 주세요.' }

$work = Join-Path $repo 'artifacts/fake-g2b'
$pfx = Join-Path $work 'www.g2b.go.kr.pfx'
$cer = Join-Path $work 'www.g2b.go.kr.cer'
$secret = 'fake-g2b'   # 이 기계에만 있는 자체 서명 인증서 파일의 암호 — 지키는 것이 없다
$server = Join-Path $repo 'scripts/fake-g2b/server.mjs'
$devHome = Join-Path $env:LOCALAPPDATA 'Pclm.Erp.Dev'
New-Item -ItemType Directory -Force $work | Out-Null

# ── 인증서 ────────────────────────────────────────────────────────────────────
if (-not (Test-Path $pfx) -or -not (Test-Path $cer)) {
    Step '인증서 짓기 (www.g2b.go.kr · 자체 서명)'
    $cert = New-SelfSignedCertificate -DnsName 'www.g2b.go.kr' -CertStoreLocation 'Cert:\CurrentUser\My' `
        -KeyExportPolicy Exportable -NotAfter (Get-Date).AddYears(5) -FriendlyName 'pclm 가짜 나라장터(개발용)'
    try {
        Export-PfxCertificate -Cert $cert -FilePath $pfx -Password (ConvertTo-SecureString $secret -AsPlainText -Force) | Out-Null
        Export-Certificate -Cert $cert -FilePath $cer -Type CERT | Out-Null
    } finally {
        # 저장소에 남기면 이 기계의 다른 일이 이 인증서를 고를 수 있다. 파일로만 둔다.
        Remove-Item -LiteralPath "Cert:\CurrentUser\My\$($cert.Thumbprint)" -DeleteKey
    }
}
$spki = "$(node $server --spki $cer)".Trim()
$devId = "$(node $server --id)".Trim()
if ($spki -notmatch '^[A-Za-z0-9+/]{43}=$' -or $devId -notmatch '^[a-p]{32}$') { throw '인증서 공개키나 개발용 확장 ID 를 읽지 못했습니다.' }

# ── 짓기 ──────────────────────────────────────────────────────────────────────
# 띄워 둔 창이 exe 를 쥐고 있어도 run.ps1 이 그것을 비키고 짓는다(Move-HeldOutputs).
& (Join-Path $repo 'run.ps1') -BuildOnly -HomeDir $devHome | Out-Host
if ($LASTEXITCODE -ne 0) { throw "./run.ps1 -BuildOnly 가 실패했습니다(종료 코드 $LASTEXITCODE)." }

# ── 개발 호스트 ───────────────────────────────────────────────────────────────
$exe = Join-Path $repo 'target/debug/pclm-desktop.exe'
Step "개발 호스트 등록 (kr.rfastball.pclm.erp.dev → 확장 $devId)"
try {
    & (Join-Path $repo 'scripts/register-erp-host.ps1') -Development -ExtensionId $devId -ExePath $exe | Out-Host
} catch {
    # 브라우저가 상시 연결로 pclm-erp-dev.exe 를 쥐고 있으면 덮어쓰지 못한다. 지난 등록 그대로 간다.
    Write-Warning "개발 호스트를 다시 등록하지 못했습니다: $($_.Exception.Message) — 지난 등록으로 갑니다. 새 빌드를 쓰려면 브라우저를 닫고 다시 하세요."
}

# ── 브라우저 ──────────────────────────────────────────────────────────────────
# Edge 를 먼저 — Chrome 은 --load-extension 을 막아 손으로 올려야 한다(Edge 157 은 받았다).
$browser = @(
    "${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe",
    "$env:ProgramFiles\Microsoft\Edge\Application\msedge.exe",
    "$env:ProgramFiles\Google\Chrome\Application\chrome.exe",
    "${env:ProgramFiles(x86)}\Google\Chrome\Application\chrome.exe",
    "$env:LOCALAPPDATA\Google\Chrome\Application\chrome.exe"
) | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $browser) { throw 'Edge 나 Chrome 을 찾지 못했습니다.' }
$browserName = [System.IO.Path]::GetFileNameWithoutExtension($browser)   # msedge · chrome

$extension = Join-Path $devHome 'extension'   # server.mjs 가 개발 홈을 받아 같은 자리에 뜬다
$node = $null
$window = $null
try {
    # 빈 작업자료를 여기서 짓지 않는다 — 처음 실행에서 창이 세운다.
    Step "개발 홈의 창 띄우기 ($devHome)"
    $window = Start-Process $exe -ArgumentList '--home', "`"$devHome`"" -PassThru -NoNewWindow

    Step "가짜 서버 띄우기 (https://www.g2b.go.kr/ → 127.0.0.1:$Port)"
    $node = Start-Process node -ArgumentList "`"$server`"", "`"$pfx`"", $Port, $secret, "`"$devHome`"" -PassThru -NoNewWindow
    $ready = $false
    foreach ($attempt in 1..50) {
        if ($node.HasExited) { throw "가짜 서버가 곧바로 멈췄습니다. $Port 번을 이미 누가 쓰고 있는지 보세요." }
        if (Test-Path (Join-Path $extension 'manifest.json')) {
            $client = New-Object System.Net.Sockets.TcpClient
            try { $client.Connect('127.0.0.1', $Port); $ready = $true } catch { } finally { $client.Dispose() }
            if ($ready) { break }
        }
        Start-Sleep -Milliseconds 200
    }
    if (-not $ready) { throw "가짜 서버가 열리지 않았습니다: 127.0.0.1:$Port" }

    # Start-Process 는 인자를 빈칸으로만 잇는다 — 빈칸이 든 값은 직접 따옴표로 싼다.
    $arguments = @(
        "--user-data-dir=`"$(Join-Path $work "profile-$browserName")`"",
        "--host-resolver-rules=`"MAP www.g2b.go.kr 127.0.0.1:$Port`"",
        "--ignore-certificate-errors-spki-list=$spki",
        "--load-extension=`"$extension`"",
        '--disable-features=DisableLoadExtensionCommandLineSwitch',
        '--no-first-run', '--no-default-browser-check',
        'https://www.g2b.go.kr/'
    )
    Step "브라우저 띄우기 ($(Split-Path -Leaf $browser))"
    if ($browserName -eq 'chrome') {
        Write-Host @"

처음 한 번 할 일
  - 확장이 안 보이면 이 브라우저판이 --load-extension 을 막은 것이다(Chrome 은 137 판쯤부터 막는다 — 156 실측).
    손으로 올린다 — 한 번이면 이 프로필에 남는다:
      chrome://extensions → 개발자 모드 켜기 → 「압축해제된 확장 프로그램을 로드합니다」
      → $extension
"@
    }
    Write-Host @"

확장 소스를 고치면 자동 적용합니다. 문서 패널이 열려 있으면 닫아 주세요.
옛 확장은 최초 한 번 $(if ($browserName -eq 'chrome') { 'chrome' } else { 'edge' })://extensions 에서 확장의 ↻ 를 눌러야 할 수 있습니다.

"@
    $process = Start-Process $browser -ArgumentList $arguments -PassThru
    $process.WaitForExit()
} finally {
    if ($node -and -not $node.HasExited) {
        Write-Host '가짜 서버 내림' -ForegroundColor DarkGray
        Stop-Process -Id $node.Id -Force -ErrorAction SilentlyContinue
    }
    # 창은 SQLite 를 쥐고 있어 죽이지 않고 닫기를 청한다. 트레이로 숨으면 남는다 — 다음 실행의 빌드가 exe 를 비키고 짓는다.
    if ($window -and -not $window.HasExited) {
        Write-Host '창 닫기' -ForegroundColor DarkGray
        $window.Refresh()
        if (-not $window.CloseMainWindow() -or -not $window.WaitForExit(5000)) {
            Write-Warning "창(pid $($window.Id))이 닫히지 않았습니다 — 트레이에 있으면 거기서 끝내세요."
        }
    }
}

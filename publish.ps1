<#
.SYNOPSIS
    배포물을 짓는다 — 화면부터 짓고, 그다음 실행 파일.

.DESCRIPTION
    나오는 것은 파일 하나다 — dist/계약목록/계약목록.exe.

    self-contained 라 .NET 을 설치하지 않은 컴퓨터에서도 돈다. SQLite 네이티브까지 실행 파일
    안에 넣고(IncludeNativeLibrariesForSelfExtract), 화면(wwwroot)도 EmbeddedResource 로
    안에 들어간다 — WebView2 가 폴더 매핑 대신 WebResourceRequested 로 메모리에서 받는다
    (MainWindow.xaml.cs). 창을 띄우려면 Microsoft Edge WebView2 런타임이 필요하다.

    명령줄(pclm.exe)은 배포물에 넣지 않는다. 자기 런타임을 통째로 안고 있어 41MB 를 더하는데,
    받는 쪽은 창만 쓴다. 개발할 때는 ./run.ps1 -Cli 로 돌린다.

    화면은 빌드 산출물이라 저장소에 없다. 이 스크립트가 dotnet publish 보다 먼저 vite build 를
    부르는 까닭이 그것이다 — 순서를 건너뛰면 화면 없는 앱이 나간다.

.EXAMPLE
    ./publish.ps1
    ./publish.ps1 -NoZip
#>
[CmdletBinding()]
param(
    [string]$Runtime = 'win-x64',
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

Need dotnet '.NET 8 SDK 를 설치해 주세요.'
Need npm    'Node 를 설치해 주세요.'

$version = ([xml](Get-Content -Raw 'Directory.Build.props')).Project.PropertyGroup.Version

# ── 화면 ──────────────────────────────────────────────────────────────────────
if (-not (Test-Path 'node_modules')) {
    Step '의존성 받기 (npm ci)'
    npm ci
    if ($LASTEXITCODE -ne 0) { throw 'npm ci 가 실패했습니다.' }
}

Step '화면 짓기 (vite build)'
npx vite build
if ($LASTEXITCODE -ne 0) { throw 'vite build 가 실패했습니다.' }

$wwwroot = 'src/Pclm.App/wwwroot/index.html'
if (-not (Test-Path $wwwroot)) { throw "화면이 나오지 않았습니다: $wwwroot" }

# ── 실행 파일 ─────────────────────────────────────────────────────────────────
if (Test-Path $Output) { Remove-Item $Output -Recurse -Force }

$common = @(
    '-c', 'Release',
    '-r', $Runtime,
    '--self-contained', 'true',
    '-p:PublishSingleFile=true',
    '-p:IncludeNativeLibrariesForSelfExtract=true',   # e_sqlite3.dll 을 exe 안으로
    '-p:EnableCompressionInSingleFile=true',
    '-p:DebugType=none',                              # pdb 를 배포물에 남기지 않는다
    '-p:SatelliteResourceLanguages=en',               # 쓰지 않는 번역 어셈블리 폴더 제거
    '-p:PublishReferencesDocumentationFiles=false',   # 참조 패키지의 xml 문서 제외
    '-o', $Output
)

Step '창 짓기 (Pclm.App)'
dotnet publish src/Pclm.App @common
if ($LASTEXITCODE -ne 0) { throw 'Pclm.App publish 가 실패했습니다.' }

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

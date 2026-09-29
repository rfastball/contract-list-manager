<#
.SYNOPSIS
    공개 저장소에 이 판의 스냅샷을 올린다 — 릴리스 절차의 마지막 단계.

.DESCRIPTION
    이 저장소는 비공개 작업장이다. 이력에 실제 조달 문서에서 온 값이 남아 있어 그대로 열 수 없다.
    그래서 공개 저장소(rfastball/contract-list-manager)는 이력을 잇지 않고, 판마다 트리 한 벌을
    커밋 하나로 받는다. 무엇을 빼는지는 .gitattributes 의 export-ignore 가 쥔다 — git archive 가
    그것을 따르므로 목록이 두 벌이 되지 않는다.

    순서: vX.Y.Z 태그 확인 → 공개 저장소를 받아 그 태그의 트리로 통째로 갈아 끼움 →
    막을 것 검사 → 커밋 · 태그 · push → GitHub 릴리스(비공개 쪽 노트와 zip 을 그대로).

    막을 것 검사는 마지막 그물이다. 실제 값을 걸러내는 일은 커밋할 때 이미 끝나 있어야 한다 —
    여기서는 이 기계의 절대경로와 자료 파일만 본다(실제 값 목록을 저장소에 둘 수는 없다).

.EXAMPLE
    ./scripts/publish-public.ps1
    ./scripts/publish-public.ps1 -NoRelease
#>
[CmdletBinding()]
param(
    [string]$Repo = 'rfastball/contract-list-manager',
    [string]$Work = (Join-Path $env:LOCALAPPDATA 'Pclm-public\contract-list-manager'),
    [switch]$NoRelease
)

$ErrorActionPreference = 'Stop'
# gh·git 이 내는 한글을 콘솔 코드페이지(cp949)로 읽으면 릴리스 노트가 깨진다.
[Console]::OutputEncoding = [Text.Encoding]::UTF8
Set-Location (Split-Path $PSScriptRoot)   # 저장소 뿌리

function Step($message) { Write-Host "`n== $message" -ForegroundColor Cyan }
# 이름을 Git 으로 두면 PowerShell 은 대소문자를 가리지 않아 git 호출이 모두 이 함수로 와서 끝없이 돈다.
# 5.1 은 stderr 가 리다이렉트되면(! 로 부를 때) git 의 진행 메시지까지 오류로 감싸 Stop 에 걸린다 —
# 판정은 종료 코드로만 하고, 출력은 문자열로 돌려준다.
function Invoke-Native([string]$exe) {
    # 찾지 못한 실행 파일은 Continue 아래에서 오류가 되지 않고 지난 종료 코드가 남는다 — 먼저 찾는다.
    $exe = (Get-Command $exe -CommandType Application -ErrorAction Stop | Select-Object -First 1).Source
    $ErrorActionPreference = 'Continue'
    & $exe @args 2>&1 | ForEach-Object { "$_" }
    if ($LASTEXITCODE -ne 0) { throw "$exe $args 가 실패했습니다." }
}
function G { Invoke-Native git.exe @args }

$version = ([xml](Get-Content -Raw 'Directory.Build.props')).Project.PropertyGroup.Version
$tag = "v$version"

Step "확인 ($tag)"
# 작업 트리가 아니라 태그의 트리를 낸다 — 릴리스한 판만 나가고, 태그 뒤에 이 스크립트를 고쳐도 태그를 다시 세울 일이 없다.
if (-not (G tag --list $tag)) { throw "$tag 태그가 없습니다. 릴리스한 판만 공개합니다." }
$commit = G rev-parse --short "$tag^{commit}"
$private = (G remote get-url origin) -replace '.*github\.com[:/]', '' -replace '\.git$', ''

Step '공개 저장소 받기'
if (-not (Test-Path (Join-Path $Work '.git'))) {
    New-Item -ItemType Directory -Force (Split-Path $Work) | Out-Null
    G clone -q "https://github.com/$Repo.git" $Work
}
G -C $Work fetch -q origin
if (G -C $Work ls-remote --tags origin $tag) { throw "공개 저장소에 이미 $tag 가 있습니다." }
if (G -C $Work ls-remote --heads origin main) { G -C $Work checkout -q -B main origin/main }
else { G -C $Work symbolic-ref HEAD refs/heads/main }   # 빈 저장소 — 첫 커밋이 main 에 선다

Step '트리 갈아 끼우기'
# 지운 파일도 공개 쪽에서 사라져야 하므로 통째로 비우고 다시 푼다.
G -C $Work rm -rq --ignore-unmatch .
$archive = Join-Path $env:TEMP "pclm-public-$version.tar"
G archive --format=tar -o $archive $tag
# Git Bash 에서 부르면 PATH 앞의 GNU tar 가 잡히는데, 그것은 C: 를 원격 호스트로 읽는다. Windows 것을 짚는다.
Invoke-Native (Join-Path $env:SystemRoot 'System32\tar.exe') -xf $archive -C $Work
Remove-Item $archive

Step '막을 것 검사'
# 풀기가 조용히 실패하면 빈 트리가 모든 파일을 지우는 커밋이 된다.
foreach ($must in 'README.md', 'LICENSE', 'THIRD-PARTY-NOTICES.txt', 'Pclm.sln') {
    if (-not (Test-Path (Join-Path $Work $must))) { throw "풀어 놓은 트리에 $must 가 없습니다." }
}
$blocked = Get-ChildItem $Work -Recurse -File -Force |
    Where-Object { $_.FullName -notmatch '\\\.git\\' } |
    Where-Object { $_.Extension -in '.db', '.pclm', '.pdf', '.xlsx' -or (Select-String -Path $_.FullName -Pattern 'C:[\\/]Users[\\/]' -SimpleMatch:$false -Quiet) }
if ($blocked) { throw "공개하면 안 되는 파일이 있습니다:`n$($blocked.FullName -join "`n")" }

Step '커밋 · push'
G -C $Work add -A
$message = Join-Path $env:TEMP "pclm-public-$version.txt"
# PowerShell 5.1 의 utf8 은 BOM 을 붙인다. 메시지·노트 첫 글자에 BOM 이 박히지 않게 직접 쓴다.
$utf8 = New-Object System.Text.UTF8Encoding $false
[IO.File]::WriteAllText($message, "계약 목록 $version`n`n비공개 저장소 $tag ($commit) 의 트리.`n", $utf8)
G -C $Work commit -q -F $message
Remove-Item $message
G -C $Work tag $tag
G -C $Work push -q origin main $tag

if ($NoRelease) { return }

Step 'GitHub 릴리스'
$zip = "dist/계약목록_$version`_win-x64.zip"
if (-not (Test-Path $zip)) { throw "$zip 이 없습니다. ./publish.ps1 을 먼저 돌리세요." }
$asset = Join-Path $env:TEMP "pclm_$version`_win-x64.zip"   # 자산 이름은 ASCII 로
Copy-Item $zip $asset -Force
$notes = Join-Path $env:TEMP "pclm-notes-$version.md"
$body = (Invoke-Native gh.exe release view $tag -R $private --json body --jq .body) -join "`n"
if (-not $body.Trim()) { throw "비공개 쪽 $tag 릴리스 노트를 읽지 못했습니다." }
[IO.File]::WriteAllText($notes, $body, $utf8)
Invoke-Native gh.exe release create $tag $asset -R $Repo --title "계약 목록 $version" --notes-file $notes
Remove-Item $asset, $notes
Write-Host "https://github.com/$Repo/releases/tag/$tag" -ForegroundColor Green

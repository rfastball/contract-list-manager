param(
    [Parameter(Mandatory)][ValidatePattern('^[a-p]{32}$')][string]$ExtensionId,
    [Parameter(Mandatory)][string]$ExePath,
    [switch]$Development,
    [switch]$Unregister
)
$ErrorActionPreference = 'Stop'
$hostName = if ($Development) { 'kr.rfastball.pclm.erp.dev' } else { 'kr.rfastball.pclm.erp' }
$registryPaths = @(
    "HKCU:\Software\Google\Chrome\NativeMessagingHosts\$hostName",
    "HKCU:\Software\Microsoft\Edge\NativeMessagingHosts\$hostName"
)
if ($Unregister) {
    foreach ($registryPath in $registryPaths) {
        if (Test-Path -LiteralPath $registryPath) { Remove-Item -LiteralPath $registryPath }
    }
    return
}
$resolvedExe = (Resolve-Path -LiteralPath $ExePath).Path
if ([IO.Path]::GetExtension($resolvedExe) -ne '.exe') { throw '빌드한 계약목록.exe 경로를 지정하세요.' }
if ($Development) {
    $devExe = Join-Path (Split-Path -Parent $resolvedExe) 'pclm-erp-dev.exe'
    # 상시 연결 호스트가 사본을 쥐고 있으면 덮어쓰지 못한다 — 이름을 바꿔 비킨다(run.ps1 의 Move-HeldFiles 와 같은 꼴,
    # 다음 빌드가 .old- 를 치운다). 호스트는 제 exe 가 비켜진 것을 보고 떠나고 확장이 새 사본으로 다시 붙는다.
    if (Test-Path -LiteralPath $devExe) {
        try { [IO.File]::Open($devExe, 'Open', 'ReadWrite', 'None').Dispose() }
        catch { Move-Item -LiteralPath $devExe -Destination ($devExe -replace '\.exe$', ".old-$(Get-Date -Format yyyyMMddHHmmss).exe") }
    }
    Copy-Item -LiteralPath $resolvedExe -Destination $devExe -Force
    $resolvedExe = $devExe
}
$hostDirectory = Join-Path $env:LOCALAPPDATA $(if ($Development) { 'Pclm.Erp.Dev' } else { 'Pclm' })
New-Item -ItemType Directory -Path $hostDirectory -Force | Out-Null
$origin = "chrome-extension://$ExtensionId/"
$manifestPath = Join-Path $hostDirectory "$hostName.json"
$manifest = @{ name = $hostName; description = 'PCLM ERP'; path = $resolvedExe;
    type = 'stdio'; allowed_origins = @($origin) } | ConvertTo-Json
[IO.File]::WriteAllText($manifestPath, $manifest, [Text.UTF8Encoding]::new($false))
[IO.File]::WriteAllText((Join-Path $hostDirectory 'extension-origin.txt'), $origin, [Text.UTF8Encoding]::new($false))
foreach ($registryPath in $registryPaths) {
    New-Item -Path $registryPath -Force | Out-Null
    Set-Item -LiteralPath $registryPath -Value $manifestPath
}
Write-Output "Native Host 등록: $manifestPath"

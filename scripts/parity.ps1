<#
.SYNOPSIS
    C# 와 Rust 가 같은 .pclm 을 같게 보는지 잰다(#9). 두 계기(tests/Pclm.Parity, crates/pclm-parity)를 짓고 돌린다.

.DESCRIPTION
    사례마다 한 줄을 낸다 — 이름, 표·뷰 수와 행 수, same/DIFF(DIFF 면 처음 갈린 줄 번호).
    산출물은 모두 artifacts/parity/<시각>/ 에 남는다(gitignore). 덤프 꼴은 crates/pclm-parity/README.md 가 정본이다.

        ./scripts/parity.ps1                 Rust 를 박제(tests/parity-golden — C# 계기가 낸 것)와 견준다
        $env:PCLM_PARITY_RECORD = '1'; ./scripts/parity.ps1
                                             C# 와 Rust 를 나란히 돌려 견주고, 모두 같으면 박제를 새로 뜬다(C# 계기가 있어야 한다)
        ./scripts/parity.ps1 -Erp <폴더>     (C# 와 나란히) seed 에 그 폴더의 ERP JSON 을 「JSON 파일로 가져오기」 길로 들인다
        ./scripts/parity.ps1 -Real           (C# 와 나란히) 실제 홈의 백업·작업자료도 — 원본은 읽기 전용 사본으로만 뜬다

    실제 자료(-Real, -Erp)의 덤프는 artifacts 에만 남는다. 콘솔에는 칸 값을 한 글자도 내지 않는다 — 수와 줄 번호뿐이다.
    -Real · -Erp 는 박제를 뜨지 않는다.

    박제(#8 ⑨). 사례마다의 견주기는 그대로 두고 C# 계기의 부름만 테이프로 바꾼다 — 뜰 때 C# 부름마다 인자 · 표준 출력 ·
    종료 코드와 그 부름이 작업 폴더에 남긴 변화(지은 · 고친 · 지운 파일과 폴더)를 적고, 견줄 때는 그 부름을 돌리는 대신 적힌
    변화를 차례대로 되살린다. 다만 C# 덤프(dump · xlsx-dump)가 C# 가 짓지 않은 파일(Rust 의 산출물)을 읽는 부름은 박제가
    아니라 Rust 계기의 같은 명령으로 갈아 끼운다 — 뜰 때 그 둘이 바이트로 같은지 본다. 파일은 내용마다 gzip 한 벌
    (tests/parity-golden/blobs)이고 작업 폴더 · 저장소 경로는 <<work>> · <<root>> 로 적는다. 견줄 때는 뜬 날과 오늘의 날짜를
    두 쪽에서 함께 <날짜> 로 바꾼 뒤 바이트로 견주고, 어긋나면 처음 갈린 줄과 양쪽 줄 몇 개를 보인다(지어낸 자료만 다룬다).
    박제를 새로 쓰는 길은 PCLM_PARITY_RECORD=1 하나뿐이다 — 계약면 박제처럼 눈으로 본 뒤에만.

    뒤 작업이 「C# 가 사본 1 에 X 를 하고 Rust 가 사본 2 에 X 를 한 결과」 를 견줄 때는 사례 목록에 Compare-Op 한 줄을
    더한다. 인자의 {file} 이 그 사본의 경로로 바뀐다:

        Compare-Op -Name 'migrate-v25' -Source $synth25 -CsArgs @('migrate', '{file}') -RsArgs @('migrate', '{file}')

    편집(Store)의 동등성은 Compare-Run 이 잰다 — 두 사본에 같은 편집 대본(crates/pclm-parity/scripts/*.json)을 시키고,
    연산마다의 결과 글(<사본>.out.json)과 끝난 파일의 덤프를 함께 견준다.

    다리(화면의 요청)의 동등성은 Compare-Bridge 가 잰다 — 두 사본에 같은 다리 대본(scripts/bridge-*.json)을 시키고 답 글
    (<사본>.bridge.txt, 한 줄에 답 한 통)을 바이트로 견준다. 엑셀 부름(내보내기 · 계획 가져오기)은 Compare-BridgeExcel 이
    더 잰다 — 고르기 창의 물음, 끝난 자료, 내보낸 통합 문서마다의 덤프(xlsx-dump).

    명령줄(pclm)의 동등성은 Compare-Cli 가 잰다 — 같은 자리에 같은 홈을 두 번 두고 C# pclm 과 Rust pclm 에 같은 명령들을
    시킨 뒤, 명령마다의 종료 코드 · 표준 출력 · 표준 오류(바이트), 끝난 자리의 목록과 SQLite 파일의 덤프를 견준다.

.PARAMETER Erp
    seed 가 들일 ERP JSON 폴더. 저장소 밖의 실제 자료다 — 안으로 복사하지 않는다.

.PARAMETER Real
    %LOCALAPPDATA%\Pclm 의 backups\*.pclm 과 config.json 이 가리키는 작업자료를 잰다. 원본의 SHA256 을 뜨기 전후로 견주어
    한 바이트라도 바뀌었으면 실패한다.

.PARAMETER NoBuild
    짓지 않고 이미 지어 둔 계기를 쓴다.
#>
[CmdletBinding()]
param(
    [string]$Erp,
    [switch]$Real,
    [switch]$NoBuild
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 3
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)

$root = Split-Path -Parent $PSScriptRoot
$targetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $root 'target' }
# 다리(#8 ⑤)를 견주려고 창(Pclm.App)을 참조해 net8.0-windows 로 짓는다.
$cs = Join-Path $root 'tests\Pclm.Parity\bin\Release\net8.0-windows\Pclm.Parity.exe'
$rs = Join-Path $targetDir 'release\pclm-parity.exe'
$rsHost = Join-Path $targetDir 'release\pclm-erp-host.exe'
# 명령줄(#8 ⑦) — 개발 명령줄은 아직 C# 쪽이다(run.ps1 -Cli). 둘 다 이름이 pclm 이다.
$csCli = Join-Path $root 'src\Pclm.Cli\bin\Release\net8.0\pclm.exe'
$rsCli = Join-Path $targetDir 'release\pclm.exe'

# 갈래 — record(C# 와 나란히 돌리며 박제를 뜬다) · replay(C# 부름을 박제로 되살린다) · live(실제 자료 — 박제에 닿지 않는다).
$golden = Join-Path $root 'tests\parity-golden'
$recording = $env:PCLM_PARITY_RECORD -eq '1'
if ($Real -or $Erp) {
    if ($recording) { throw '박제는 지어낸 자료로만 뜬다 — -Real · -Erp 와 함께 뜨지 않는다.' }
    $mode = 'live'
}
elseif ($recording) { $mode = 'record' }
else { $mode = 'replay' }
if ($mode -ne 'replay' -and -not (Test-Path (Join-Path $root 'tests\Pclm.Parity\Pclm.Parity.csproj'))) {
    throw 'C# 계기(tests/Pclm.Parity)가 없어 C# 와 나란히 견줄 수 없습니다 — 박제와 견주려면 PCLM_PARITY_RECORD · -Real · -Erp 없이 돌린다.'
}
if ($mode -eq 'replay' -and -not (Test-Path (Join-Path $golden 'tape.tsv.gz'))) { throw "박제가 없습니다: $golden" }
$script:tape = $mode -ne 'live'
# 테이프에 담지 않는 대목(창 잠금 · 교차 읽기)이 잠시 켠다.
$script:tapeOff = $false

# ── 박제 ─────────────────────────────────────────────

$latin1 = [System.Text.Encoding]::GetEncoding(28591)
$tapeUtf8 = New-Object System.Text.UTF8Encoding($false)
$script:sha256 = [System.Security.Cryptography.SHA256]::Create()
$script:calls = New-Object System.Collections.Generic.List[object]
$script:cursor = 0
$script:named = @{}
$script:csMade = New-Object 'System.Collections.Generic.HashSet[string]'
$script:rsMade = New-Object 'System.Collections.Generic.HashSet[string]'
$script:needed = New-Object 'System.Collections.Generic.HashSet[string]'
$script:tapeProblems = New-Object System.Collections.Generic.List[string]
$script:dateForms = @()
$script:dayShift = 0

function Get-Hash([byte[]]$bytes) {
    return [System.BitConverter]::ToString($script:sha256.ComputeHash($bytes)).Replace('-', '').ToLowerInvariant()
}

# 작업 폴더 · 저장소 경로가 글에 드는 꼴 — 그대로, JSON 의 \\, 슬래시. 박제에는 낱말로 적는다(이 기계의 경로를 싣지 않는다).
function Get-PathTokens {
    $pairs = New-Object System.Collections.Generic.List[object]
    foreach ($place in @(@($work, 'work'), @($root, 'root'))) {
        $pairs.Add(@($place[0].Replace('\', '\\'), "<<$($place[1])-json>>"))
        $pairs.Add(@($place[0].Replace('\', '/'), "<<$($place[1])-slash>>"))
        $pairs.Add(@($place[0], "<<$($place[1])>>"))
    }
    return , $pairs
}

function Convert-Paths([string]$text, [bool]$ToTokens) {
    foreach ($pair in $script:pathTokens) {
        if ($ToTokens) { $text = $text.Replace($pair[0], $pair[1]) } else { $text = $text.Replace($pair[1], $pair[0]) }
    }
    return $text
}

# 길이 머리(4바이트 LE) + 글 틀의 이음(확장 호스트의 대본 · 답). 꼴이 맞지 않으면 $null.
function Split-Frames([byte[]]$bytes) {
    $frames = New-Object System.Collections.Generic.List[byte[]]
    $at = 0
    while ($at + 4 -le $bytes.Length) {
        $length = [System.BitConverter]::ToInt32($bytes, $at)
        if ($length -lt 0 -or $at + 4 + $length -gt $bytes.Length) { return $null }
        $frame = New-Object byte[] $length
        [System.Array]::Copy($bytes, $at + 4, $frame, 0, $length)
        $frames.Add($frame)
        $at += 4 + $length
    }
    if ($at -ne $bytes.Length) { return $null }
    return , $frames
}

# 파일 바이트의 경로를 낱말로(또는 되돌려). 바이트는 Latin-1 로 한 글자씩 옮겨 바꾼다 — 경로는 ASCII 라 UTF-8 글 안에서도
# 같은 바이트다. 틀로 이은 파일(.bin)은 틀마다 바꾸고 길이 머리를 다시 적는다.
function Convert-PathBytes([byte[]]$bytes, [string]$name, [bool]$ToTokens) {
    if ($name.EndsWith('.bin')) {
        $frames = Split-Frames $bytes
        if ($null -ne $frames) {
            $out = New-Object System.IO.MemoryStream
            foreach ($frame in $frames) {
                $body = $latin1.GetBytes((Convert-Paths $latin1.GetString($frame) $ToTokens))
                $out.Write([System.BitConverter]::GetBytes([int]$body.Length), 0, 4)
                $out.Write($body, 0, $body.Length)
            }
            return , $out.ToArray()
        }
    }
    return , $latin1.GetBytes((Convert-Paths $latin1.GetString($bytes) $ToTokens))
}

function Save-Blob([byte[]]$bytes) {
    $sha = Get-Hash $bytes
    $path = Join-Path $script:blobDir "$sha.gz"
    if (-not (Test-Path -LiteralPath $path)) {
        $file = [System.IO.File]::Create($path)
        try {
            $gzip = New-Object System.IO.Compression.GZipStream($file, [System.IO.Compression.CompressionLevel]::Optimal)
            $gzip.Write($bytes, 0, $bytes.Length)
            $gzip.Dispose()
        }
        finally { $file.Dispose() }
    }
    return $sha
}

function Read-Gzip([string]$path) {
    $file = [System.IO.File]::OpenRead($path)
    try {
        $gzip = New-Object System.IO.Compression.GZipStream($file, [System.IO.Compression.CompressionMode]::Decompress)
        $memory = New-Object System.IO.MemoryStream
        $gzip.CopyTo($memory)
        return , $memory.ToArray()
    }
    finally { $file.Dispose() }
}

function Read-Blob([string]$sha) { return , (Read-Gzip (Join-Path $script:blobDir "$sha.gz")) }

# 작업 폴더의 모든 파일 · 폴더 — 상대 경로 → 크기와 고친 시각(폴더는 <dir>).
function Get-Tree {
    $map = New-Object 'System.Collections.Generic.Dictionary[string,string]' ([System.StringComparer]::Ordinal)
    foreach ($entry in (New-Object System.IO.DirectoryInfo($work)).EnumerateFileSystemInfos('*', [System.IO.SearchOption]::AllDirectories)) {
        $relative = $entry.FullName.Substring($work.Length + 1)
        if ($entry -is [System.IO.DirectoryInfo]) { $map[$relative] = '<dir>' }
        else { $map[$relative] = "$($entry.Length) $($entry.LastWriteTimeUtc.Ticks)" }
    }
    return , $map
}

# 두 목록 사이의 변화 — 지운 것(깊은 것부터), 지은 폴더(얕은 것부터)와 지었거나 고친 파일.
function Get-Changes($before, $after) {
    $ops = New-Object System.Collections.Generic.List[object]
    $gone = New-Object System.Collections.Generic.List[string]
    foreach ($key in $before.Keys) { if (-not $after.ContainsKey($key)) { $gone.Add($key) } }
    foreach ($key in ($gone | Sort-Object -Property Length -Descending)) {
        $op = if ($before[$key] -eq '<dir>') { 'deldir' } else { 'del' }
        $ops.Add([pscustomobject]@{ Op = $op; Path = $key; Blob = '-' })
    }
    $made = New-Object System.Collections.Generic.List[string]
    foreach ($key in $after.Keys) {
        if (-not $before.ContainsKey($key) -or $before[$key] -ne $after[$key]) { $made.Add($key) }
    }
    foreach ($key in ($made | Sort-Object -Property Length)) {
        $op = if ($after[$key] -eq '<dir>') { 'dir' } else { 'file' }
        $ops.Add([pscustomobject]@{ Op = $op; Path = $key; Blob = '-' })
    }
    return , $ops
}

# Rust 쪽이 읽을 수 있는 C# 산출물의 지문 — 인자(와 작업 폴더)가 가리키는 파일 · 폴더와 그 위 폴더(사례 자리) 아래의 것.
# 여기 들지 않은 SQLite 파일은 C# 부름(박제로 되살린다)과 목록(머리와 크기만 본다)만 읽으므로, 박제에는 머리 100바이트만
# 싣는다(Compress-Tape). 잘못 덜어 냈으면 견주기에서 Rust 가 그 파일을 읽지 못해 갈린다 — 박제를 뜬 뒤 견주기로 확인한다.
function Add-RustReads([string[]]$paths) {
    $roots = New-Object System.Collections.Generic.List[string]
    foreach ($path in $paths) {
        if (-not $path -or -not $path.StartsWith("$work\") -or $path.IndexOfAny([System.IO.Path]::GetInvalidPathChars()) -ge 0) { continue }
        $roots.Add($path)
        $parent = [System.IO.Path]::GetDirectoryName($path)
        if ($parent -and $parent.StartsWith("$work\")) { $roots.Add($parent) }
    }
    foreach ($place in $roots) {
        $files = @()
        if ([System.IO.File]::Exists($place)) { $files = @($place) }
        elseif ([System.IO.Directory]::Exists($place)) { $files = [System.IO.Directory]::GetFiles($place, '*', [System.IO.SearchOption]::AllDirectories) }
        foreach ($file in $files) {
            $hash = Get-Hash ([System.IO.File]::ReadAllBytes($file))
            if ($script:csMade.Contains($hash)) { $null = $script:needed.Add($hash) }
        }
    }
}

# 뜬 뒤 — Rust 가 읽지 않는 C# 의 SQLite 파일(과 -wal · -shm)은 머리(또는 빈 것)만 남기고, 쓰이지 않는 내용을 지운다.
function Compress-Tape {
    foreach ($call in $script:calls) {
        foreach ($op in $call.Ops) {
            if ($op.Op -ne 'file' -or $script:needed.Contains($op.RawHash)) { continue }
            $bytes = Read-Blob $op.Blob
            $isDatabase = $bytes.Length -ge 100 -and $latin1.GetString($bytes, 0, 16) -ceq "SQLite format 3$([char]0)"
            if ($isDatabase) { $op.Blob = Save-Blob ([byte[]]$bytes[0..99]) }
            elseif ($op.Path -match '-(wal|shm|journal)$') { $op.Blob = Save-Blob (New-Object byte[] 0) }
        }
    }
    $kept = New-Object 'System.Collections.Generic.HashSet[string]'
    foreach ($call in $script:calls) { foreach ($op in $call.Ops) { if ($op.Op -eq 'file') { $null = $kept.Add($op.Blob) } } }
    foreach ($sha in $script:named.Values) { $null = $kept.Add($sha) }
    foreach ($blob in (Get-ChildItem -LiteralPath $script:blobDir -File)) {
        if (-not $kept.Contains($blob.BaseName)) { Remove-Item -LiteralPath $blob.FullName }
    }
}

# Rust 쪽이 지었거나 고친 파일의 지문 — C# 부름이 그것을 입력으로 받으면 박제로 되살릴 수 없다(뜰 때 막는다).
function Add-RustMade($before) {
    foreach ($op in (Get-Changes $before (Get-Tree))) {
        if ($op.Op -ne 'file') { continue }
        $null = $script:rsMade.Add((Get-Hash ([System.IO.File]::ReadAllBytes((Join-Path $work $op.Path)))))
    }
}

function Get-CallKey([string]$Kind, [string[]]$Arguments, [string]$Cwd, [string]$Stdin) {
    $text = $Kind + [char]31 + ($Arguments -join [char]31) + [char]30 + $Cwd + [char]30 + $Stdin
    return Convert-Paths $text $true
}

# 부름을 맞춰 볼 때 가리는 것 — Rust 쪽이 지금 시계로 짓는 이름(백업의 시각, 기본 이름의 「오늘」). 사례의 글에서 가리는 것과 같다.
function Hide-KeyClock([string]$key) {
    $key = [regex]::Replace($key, '(계약자료_v\d+_)\d{8}-\d{6}', '$1<t>')
    return [regex]::Replace($key, '(?<=_)\d{8}(?=\.(xlsx|pclm|txt|json))', '<오늘>')
}

function Show-CallKey([string]$key) { return $key.Replace([string][char]31, ' ').Replace([string][char]30, ' | ') }

# C# 부름이 읽을 파일이 Rust 의 산출물이면 박제가 그때의 바이트를 되살려도 Rust 를 재지 못한다 — 뜰 때 막는다.
function Test-CsInputs([int]$index, [string[]]$Arguments) {
    foreach ($argument in $Arguments) {
        if (-not $argument.StartsWith($work)) { continue }
        # 파일 이름에 쓸 수 없는 글자가 든 인자(쪽지 쓰기의 이스케이프 사례)는 파일이 아니다.
        if ($argument.IndexOfAny([System.IO.Path]::GetInvalidPathChars()) -ge 0) { continue }
        $files = @()
        if ([System.IO.File]::Exists($argument)) { $files = @($argument) }
        elseif ([System.IO.Directory]::Exists($argument)) {
            $files = @(Get-ChildItem -LiteralPath $argument -Recurse -File -Force | ForEach-Object { $_.FullName })
        }
        foreach ($file in $files) {
            $hash = Get-Hash ([System.IO.File]::ReadAllBytes($file))
            if ($script:rsMade.Contains($hash) -and -not $script:csMade.Contains($hash)) {
                $script:tapeProblems.Add("C# 부름 $index ($($Arguments[0])) 가 Rust 가 쓴 파일을 읽습니다: $($file.Substring($work.Length + 1))")
            }
        }
    }
}

function Get-OutputIndex([string[]]$Arguments) {
    if ($Arguments[0] -eq 'xlsx-dump') { return 2 }
    return [Array]::IndexOf($Arguments, '--out') + 1
}

function Invoke-Taped([string]$Kind, [string]$Exe, [string[]]$Arguments, [string]$Cwd = '', [string]$Stdin = '') {
    if ($mode -eq 'record') { return Save-Call $Kind $Exe $Arguments $Cwd $Stdin }
    return Restore-Call $Kind $Arguments $Cwd $Stdin
}

function Get-OutputBytes($result, [string]$Kind, [string]$name) {
    if ($Kind -eq 'cli') { return , [byte[]]$result.$name }
    return , $tapeUtf8.GetBytes([string]$result.$name)
}

function Save-Call([string]$Kind, [string]$Exe, [string[]]$Arguments, [string]$Cwd, [string]$Stdin) {
    $index = $script:calls.Count
    $key = Get-CallKey $Kind $Arguments $Cwd $Stdin
    $substitute = $false
    if ($Kind -eq 'tool' -and $Arguments[0] -in 'dump', 'xlsx-dump') {
        $source = $Arguments[1]
        if (Test-Path -LiteralPath $source -PathType Leaf) {
            $substitute = -not $script:csMade.Contains((Get-Hash ([System.IO.File]::ReadAllBytes($source))))
        }
    }
    else { Test-CsInputs $index $Arguments }
    if ($substitute) {
        # 견줄 때 이 부름은 Rust 계기가 맡는다 — 지금 같은 바이트를 내는지 본다.
        $at = Get-OutputIndex $Arguments
        $check = [string[]]$Arguments.Clone()
        $check[$at] = Join-Path $script:checkDir "$index.txt"
        $rsResult = Start-Tool $rs $check
        $result = Start-Tool $Exe $Arguments
        $same = $rsResult.Exit -eq $result.Exit
        if ($same -and $result.Exit -eq 0) {
            $same = (Get-Hash ([System.IO.File]::ReadAllBytes($check[$at]))) -eq (Get-Hash ([System.IO.File]::ReadAllBytes($Arguments[$at])))
        }
        if (-not $same) { $script:tapeProblems.Add("C# 부름 $index ($($Arguments[0])): Rust 계기가 같은 파일을 다르게 덤프합니다 — $($Arguments[1].Replace($work, '<work>'))") }
        $script:calls.Add([pscustomobject]@{ Kind = $Kind; Mode = 'rust'; Exit = $result.Exit; Key = $key; Out = ''; Err = ''; Ops = @() })
        return $result
    }
    $before = Get-Tree
    if ($Kind -eq 'cli') { $result = Start-Cli $Exe $Arguments $Cwd $Stdin } else { $result = Start-Tool $Exe $Arguments }
    $ops = Get-Changes $before (Get-Tree)
    foreach ($op in $ops) {
        if ($op.Op -ne 'file') { continue }
        $bytes = [System.IO.File]::ReadAllBytes((Join-Path $work $op.Path))
        $raw = Get-Hash $bytes
        $null = $script:csMade.Add($raw)
        $op.Blob = Save-Blob (Convert-PathBytes $bytes $op.Path $true)
        $op | Add-Member -NotePropertyName RawHash -NotePropertyValue $raw
    }
    $out = [Convert]::ToBase64String((Convert-PathBytes (Get-OutputBytes $result $Kind 'Out') '' $true))
    $err = [Convert]::ToBase64String((Convert-PathBytes (Get-OutputBytes $result $Kind 'Err') '' $true))
    $script:calls.Add([pscustomobject]@{ Kind = $Kind; Mode = 'replay'; Exit = $result.Exit; Key = $key; Out = $out; Err = $err; Ops = $ops })
    return $result
}

function Restore-Call([string]$Kind, [string[]]$Arguments, [string]$Cwd, [string]$Stdin) {
    $index = $script:cursor
    $key = Get-CallKey $Kind $Arguments $Cwd $Stdin
    if ($index -ge $script:calls.Count) { throw "박제에 없는 C# 부름입니다($index): $(Show-CallKey $key)" }
    $call = $script:calls[$index]
    $script:cursor++
    if ((Hide-KeyClock $call.Key) -cne (Hide-KeyClock $key)) {
        throw "박제의 $index 번째 C# 부름과 다릅니다 — 박제 「$(Show-CallKey $call.Key)」 · 지금 「$(Show-CallKey $key)」"
    }
    if ($call.Mode -eq 'rust') { return Start-Tool $rs $Arguments }
    foreach ($op in $call.Ops) {
        $full = Join-Path $work $op.Path
        switch ($op.Op) {
            'del' { if ([System.IO.File]::Exists($full)) { [System.IO.File]::Delete($full) } }
            'deldir' { if ([System.IO.Directory]::Exists($full)) { [System.IO.Directory]::Delete($full, $true) } }
            'dir' { $null = [System.IO.Directory]::CreateDirectory($full) }
            'file' {
                $null = [System.IO.Directory]::CreateDirectory([System.IO.Path]::GetDirectoryName($full))
                [System.IO.File]::WriteAllBytes($full, (Convert-PathBytes (Read-Blob $op.Blob) $op.Path $false))
            }
        }
    }
    $out = Convert-PathBytes ([Convert]::FromBase64String($call.Out)) '' $false
    $err = Convert-PathBytes ([Convert]::FromBase64String($call.Err)) '' $false
    if ($Kind -eq 'cli') { return [pscustomobject]@{ Out = $out; Err = $err; Exit = $call.Exit } }
    return [pscustomobject]@{ Out = $tapeUtf8.GetString($out); Err = $tapeUtf8.GetString($err); Exit = $call.Exit }
}

function Initialize-Tape {
    $script:pathTokens = Get-PathTokens
    if ($mode -eq 'record') {
        $script:staging = "$work.golden"
        $script:blobDir = Join-Path $script:staging 'blobs'
        $script:checkDir = "$work.check"
        New-Item -ItemType Directory -Force -Path $script:blobDir, $script:checkDir | Out-Null
        return
    }
    $script:blobDir = Join-Path $golden 'blobs'
    $lines = $tapeUtf8.GetString((Read-Gzip (Join-Path $golden 'tape.tsv.gz'))).Split("`n")
    if ($lines[0] -ne 'pclm-parity-golden 1') { throw "박제의 꼴을 모릅니다: $($lines[0])" }
    foreach ($line in $lines) {
        $f = $line.Split("`t")
        switch ($f[0]) {
            'meta' { if ($f[1] -eq 'date') { $script:recordedDate = $f[2] } elseif ($f[1] -eq 'cases') { $script:recordedCases = [int]$f[2] } }
            'call' {
                $script:calls.Add([pscustomobject]@{ Kind = $f[2]; Mode = $f[3]; Exit = [int]$f[4]
                        Key = $tapeUtf8.GetString([Convert]::FromBase64String($f[5])); Out = $f[6]; Err = $f[7]
                        Ops = (New-Object System.Collections.Generic.List[object]) })
            }
            'op' {
                $script:calls[[int]$f[1]].Ops.Add([pscustomobject]@{ Op = $f[2]; Path = $tapeUtf8.GetString([Convert]::FromBase64String($f[3])); Blob = $f[4] })
            }
            'named' { $script:named[$f[1]] = $f[2] }
        }
    }
    # 뜬 날과 다른 날 견주면 「오늘」 이 두 쪽에서 다르다 — 두 날짜를 두 쪽에서 함께 가린다(Write-Result).
    $today = Get-Date
    $then = [DateTime]::ParseExact($script:recordedDate, 'yyyyMMdd', [Globalization.CultureInfo]::InvariantCulture)
    if ($then.Date -ne $today.Date) {
        # 「오늘」 의 꼴은 셋이다 — 덤프 · 이름의 yyyy-MM-dd · yyyyMMdd, 수집 기록의 저장한 날 MM/dd.
        $script:dateForms = foreach ($day in $then, $today) {
            $day.ToString('yyyy-MM-dd'); $day.ToString('yyyyMMdd'); $day.ToString('MM/dd', [Globalization.CultureInfo]::InvariantCulture)
        }
        # 「오늘」 에서 센 남은 날 수는 가리지 않고 박제 쪽을 그만큼 옮겨 견준다 — 셈이 맞는지는 그대로 잰다.
        $script:dayShift = [int]($today.Date - $then.Date).TotalDays
    }
}

function Save-Tape([int]$cases) {
    $text = New-Object System.Text.StringBuilder
    $null = $text.Append("pclm-parity-golden 1`n")
    $null = $text.Append("meta`tdate`t$(Get-Date -Format 'yyyyMMdd')`n")
    $null = $text.Append("meta`tcases`t$cases`n")
    for ($i = 0; $i -lt $script:calls.Count; $i++) {
        $call = $script:calls[$i]
        $key = [Convert]::ToBase64String($tapeUtf8.GetBytes($call.Key))
        $null = $text.Append("call`t$i`t$($call.Kind)`t$($call.Mode)`t$($call.Exit)`t$key`t$($call.Out)`t$($call.Err)`n")
        foreach ($op in $call.Ops) {
            $null = $text.Append("op`t$i`t$($op.Op)`t$([Convert]::ToBase64String($tapeUtf8.GetBytes($op.Path)))`t$($op.Blob)`n")
        }
    }
    foreach ($name in ($script:named.Keys | Sort-Object)) { $null = $text.Append("named`t$name`t$($script:named[$name])`n") }
    $bytes = $tapeUtf8.GetBytes($text.ToString())
    $file = [System.IO.File]::Create((Join-Path $script:staging 'tape.tsv.gz'))
    try {
        $gzip = New-Object System.IO.Compression.GZipStream($file, [System.IO.Compression.CompressionLevel]::Optimal)
        $gzip.Write($bytes, 0, $bytes.Length)
        $gzip.Dispose()
    }
    finally { $file.Dispose() }
}

# ── 실행 ─────────────────────────────────────────────

# Windows 명령줄 규칙으로 인자 하나를 감싼다. PowerShell 5.1 의 & 는 빈 인자·따옴표를 고르게 넘기지 않아 직접 짓는다.
function Format-Argument([string]$value) {
    if ($value -ne '' -and $value -notmatch '[\s"]') { return $value }
    $quoted = '"'
    $slashes = 0
    foreach ($ch in $value.ToCharArray()) {
        if ($ch -eq '\') { $slashes++; continue }
        if ($ch -eq '"') { $quoted += ('\' * ($slashes * 2 + 1)) + '"' }
        else { $quoted += ('\' * $slashes) + $ch }
        $slashes = 0
    }
    return $quoted + ('\' * ($slashes * 2)) + '"'
}

# 실행하고 표준 출력을 돌려준다. 실패하면 표준 오류의 끝을 들고 던진다. -Log 가 있으면 두 출력을 그 파일에 남긴다 —
# 실제 자료를 다루는 명령의 출력은 콘솔이 아니라 거기로 간다. -AllowExit 의 종료 코드는 실패로 보지 않는다.
# C# 계기의 부름은 박제를 거친다(record 면 적고, replay 면 되살린다).
function Invoke-Tool {
    param([string]$Exe, [string[]]$Arguments, [string]$Log, [int[]]$AllowExit = @())
    if ($Exe -eq $cs -and $script:tape -and -not $script:tapeOff) { $result = Invoke-Taped 'tool' $Exe $Arguments }
    elseif ($mode -eq 'record' -and $Exe -ne $cs -and $Exe -notin 'dotnet', 'cargo') {
        Add-RustReads $Arguments
        $before = Get-Tree
        $result = Start-Tool $Exe $Arguments
        Add-RustMade $before
    }
    else { $result = Start-Tool $Exe $Arguments }
    if ($Log) {
        $line = ($Arguments | ForEach-Object { Format-Argument $_ }) -join ' '
        [System.IO.File]::WriteAllText($Log, "> $(Split-Path -Leaf $Exe) $line`n$($result.Out)`n--- stderr ---`n$($result.Err)",
            (New-Object System.Text.UTF8Encoding($false)))
    }
    if ($result.Exit -ne 0 -and $AllowExit -notcontains $result.Exit) {
        $tail = ($result.Err -split "`n" | Where-Object { $_.Trim() } | Select-Object -Last 3) -join ' / '
        throw "$(Split-Path -Leaf $Exe) $($Arguments[0]) 실패(종료 $($result.Exit)): $tail"
    }
    return [pscustomobject]@{ Out = $result.Out; Err = $result.Err; Exit = $result.Exit }
}

function Start-Tool([string]$Exe, [string[]]$Arguments) {
    $info = New-Object System.Diagnostics.ProcessStartInfo
    $info.FileName = $Exe
    $info.Arguments = ($Arguments | ForEach-Object { Format-Argument $_ }) -join ' '
    $info.UseShellExecute = $false
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $info.StandardOutputEncoding = [System.Text.Encoding]::UTF8
    $info.StandardErrorEncoding = [System.Text.Encoding]::UTF8
    $info.WorkingDirectory = $root
    $process = [System.Diagnostics.Process]::Start($info)
    $stdout = $process.StandardOutput.ReadToEndAsync()
    $stderr = $process.StandardError.ReadToEndAsync()
    $process.WaitForExit()
    return [pscustomobject]@{ Out = $stdout.Result; Err = $stderr.Result; Exit = $process.ExitCode }
}

function Invoke-Build {
    if ($mode -ne 'replay') {
        Write-Host '짓는 중: tests/Pclm.Parity (Release)'
        $null = Invoke-Tool -Exe 'dotnet' -Arguments @('build', (Join-Path $root 'tests\Pclm.Parity'), '-c', 'Release', '--nologo', '-v', 'q') `
            -Log (Join-Path $work 'build-cs.log')
        Write-Host '짓는 중: src/Pclm.Cli (Release)'
        $null = Invoke-Tool -Exe 'dotnet' -Arguments @('build', (Join-Path $root 'src\Pclm.Cli'), '-c', 'Release', '--nologo', '-v', 'q') `
            -Log (Join-Path $work 'build-cs-cli.log')
    }
    Write-Host '짓는 중: crates/pclm-parity · pclm-erp-host · pclm-cli (release)'
    $null = Invoke-Tool -Exe 'cargo' -Arguments @('build', '-p', 'pclm-parity', '-p', 'pclm-erp-host', '-p', 'pclm-cli', '--release') -Log (Join-Path $work 'build-rs.log')
}

# ── 견주기 ─────────────────────────────────────────────

$script:results = New-Object System.Collections.Generic.List[object]

# 덤프에서 수만 센다 — 값은 읽지 않는다.
function Measure-Dump([string]$path) {
    $tables = 0; $tableRows = 0; $views = 0; $viewRows = 0; $viewErrors = 0
    foreach ($line in [System.IO.File]::ReadLines($path)) {
        if ($line.StartsWith("table`t")) {
            $tables++; $tableRows += [long]($line.Split("`t")[2])
        }
        elseif ($line.StartsWith("view`t")) {
            $views++
            $count = $line.Split("`t")[2]
            if ($count -eq 'error') { $viewErrors++ } else { $viewRows += [long]$count }
        }
    }
    $text = "표 $tables 행 $tableRows · 뷰 $views 행 $viewRows"
    if ($viewErrors) { $text += " (깨진 뷰 $viewErrors)" }
    return $text
}

# 처음 갈린 줄 번호(1부터) 몇 개. 줄의 내용은 내지 않는다.
function Find-FirstDifferences([string]$a, [string]$b, [int]$limit = 5) {
    $left = [System.IO.File]::ReadAllLines($a, [System.Text.Encoding]::UTF8)
    $right = [System.IO.File]::ReadAllLines($b, [System.Text.Encoding]::UTF8)
    $found = New-Object System.Collections.Generic.List[string]
    $max = [Math]::Max($left.Length, $right.Length)
    for ($i = 0; $i -lt $max -and $found.Count -lt $limit; $i++) {
        $l = if ($i -lt $left.Length) { $left[$i] } else { $null }
        $r = if ($i -lt $right.Length) { $right[$i] } else { $null }
        if ($l -cne $r) { $found.Add([string]($i + 1)) }
    }
    if ($left.Length -ne $right.Length) { $found.Add("줄 수 $($left.Length)/$($right.Length)") }
    return ($found -join ', ')
}

# 박제와 견줄 때 읽는 꼴 — 뜬 날과 오늘이 다르면 두 날짜를 두 쪽에서 함께 가린다.
function Read-Compared([string]$path, [bool]$golden = $false) {
    $text = $latin1.GetString([System.IO.File]::ReadAllBytes($path))
    foreach ($form in $script:dateForms) { $text = $text.Replace($form, '<<date>>') }
    $text = Hide-AppVersion $text
    if ($golden) { $text = Move-DaysRemaining $text }
    return $text
}

# 「정보」 의 앱 판은 판을 올릴 때마다 박제와 갈린다 — 두 쪽에서 함께 가린다(뜬 판은 0.17.1).
function Hide-AppVersion([string]$text) {
    return [regex]::Replace($text, '"version":"\d+\.\d+\.\d+"', '"version":"<<version>>"')
}

# 박제 쪽의 "daysRemaining":N 을 뜬 날에서 오늘까지 지난 날만큼 줄인다(같은 날이면 그대로).
function Move-DaysRemaining([string]$text) {
    if ($script:dayShift -eq 0) { return $text }
    return [regex]::Replace($text, '"daysRemaining":(-?\d+)', { param($m) '"daysRemaining":' + ([int]$m.Groups[1].Value - $script:dayShift) })
}

# 박제와 견줄 때만 — 처음 갈린 줄과 양쪽의 줄 몇 개(지어낸 자료뿐이라 값을 보인다).
function Show-Difference([string]$a, [string]$b) {
    $read = {
        param($path, $golden)
        , @([System.IO.File]::ReadAllLines($path, [System.Text.Encoding]::UTF8) | ForEach-Object {
                $line = $_
                foreach ($form in $script:dateForms) { $line = $line.Replace($form, '<<date>>') }
                $line = Hide-AppVersion $line
                if ($golden) { $line = Move-DaysRemaining $line }
                $line
            })
    }
    $left = & $read $a $true
    $right = & $read $b $false
    $at = 0
    while ($at -lt $left.Count -and $at -lt $right.Count -and $left[$at] -ceq $right[$at]) { $at++ }
    Write-Host "    줄 $($at + 1) (줄 수 박제 $($left.Count) · Rust $($right.Count))"
    foreach ($side in @(@('박제', $left), @('Rust', $right))) {
        Write-Host "    $($side[0]):"
        $lines = $side[1]
        for ($i = $at; $i -lt [Math]::Min($at + 3, $lines.Count); $i++) {
            $line = [string]$lines[$i]
            if ($line.Length -gt 160) { $line = $line.Substring(0, 160) + ' …' }
            Write-Host "      | $line"
        }
    }
}

function Write-Result([string]$name, [string]$counts, [string]$a, [string]$b) {
    # 시계를 박아 둔 사례(대시보드의 2026-09-10 등)는 남은 날 수가 날마다 갈리지 않는다 — 옮기지 않은 박제와 같아도 같다.
    if ($mode -eq 'replay') {
        $rust = Read-Compared $b
        $same = ((Read-Compared $a) -ceq $rust) -or ($script:dayShift -ne 0 -and (Read-Compared $a $true) -ceq $rust)
    }
    else { $same = (Get-FileHash -Algorithm SHA256 $a).Hash -eq (Get-FileHash -Algorithm SHA256 $b).Hash }
    $verdict = if ($same) { 'same' } elseif ($mode -eq 'replay') { 'DIFF' } else { "DIFF (줄 $(Find-FirstDifferences $a $b))" }
    Write-Host ('{0,-24} {1}  {2}' -f $name, $counts, $verdict)
    if (-not $same -and $mode -eq 'replay') { Show-Difference $a $b }
    $script:results.Add([pscustomobject]@{ Name = $name; Same = $same })
}

function Write-Failure([string]$name, [string]$message) {
    Write-Host ('{0,-24} 실패: {1}' -f $name, $message)
    $script:results.Add([pscustomobject]@{ Name = $name; Same = $false })
}

<#
.SYNOPSIS
    한 파일을 두 계기로 덤프해 견준다 — 자(덤프)끼리의 동등성.
#>
function Compare-Dump {
    param([string]$Name, [string]$File, [string[]]$Mask = @(), [switch]$SkipViews)
    $extra = @()
    foreach ($m in $Mask) { $extra += @('--mask', $m) }
    if ($SkipViews) { $extra += '--skip-views' }
    $a = Join-Path $work "$Name.cs.dump"
    $b = Join-Path $work "$Name.rs.dump"
    try {
        $null = Invoke-Tool -Exe $cs -Arguments (@('dump', $File, '--out', $a) + $extra)
        $null = Invoke-Tool -Exe $rs -Arguments (@('dump', $File, '--out', $b) + $extra)
        Write-Result $Name (Measure-Dump $a) $a $b
    }
    catch { Write-Failure $Name $_.Exception.Message }
}

<#
.SYNOPSIS
    같은 입력의 사본 둘에 C# 와 Rust 가 같은 일을 하고, 두 결과를 같은 자(C# 덤프)로 견준다 — 동작의 동등성.
    인자의 {file} 이 각 사본의 경로로 바뀐다. 시각·무작위 식별자처럼 일부러 갈리는 칸은 -Mask 로 가린다.
#>
function Compare-Op {
    param([string]$Name, [string]$Source, [string[]]$CsArgs, [string[]]$RsArgs, [string[]]$Mask = @())
    $one = Join-Path $work "$Name.cs.pclm"
    $two = Join-Path $work "$Name.rs.pclm"
    $extra = @()
    foreach ($m in $Mask) { $extra += @('--mask', $m) }
    $a = Join-Path $work "$Name.cs.dump"
    $b = Join-Path $work "$Name.rs.dump"
    try {
        # 사본도 읽기 전용 VACUUM INTO 로 뜬다 — 입력의 -wal 을 빠뜨리는 복사가 없다.
        $null = Invoke-Tool -Exe $cs -Arguments @('snapshot', $Source, $one)
        $null = Invoke-Tool -Exe $cs -Arguments @('snapshot', $Source, $two)
        $null = Invoke-Tool -Exe $cs -Arguments ($CsArgs | ForEach-Object { $_.Replace('{file}', $one) }) -Log (Join-Path $work "$Name.cs.log")
        $null = Invoke-Tool -Exe $rs -Arguments ($RsArgs | ForEach-Object { $_.Replace('{file}', $two) }) -Log (Join-Path $work "$Name.rs.log")
        $null = Invoke-Tool -Exe $cs -Arguments (@('dump', $one, '--out', $a) + $extra)
        $null = Invoke-Tool -Exe $cs -Arguments (@('dump', $two, '--out', $b) + $extra)
        Write-Result $Name (Measure-Dump $a) $a $b
    }
    catch { Write-Failure $Name $_.Exception.Message }
}

<#
.SYNOPSIS
    빈 자리에 C# 와 Rust 가 각자 새 파일을 짓고 같은 자(C# 덤프)로 견준다.
#>
function Compare-Create {
    param([string]$Name, [string]$Role, [string[]]$Mask = @())
    $one = Join-Path $work "$Name.cs.pclm"
    $two = Join-Path $work "$Name.rs.pclm"
    $extra = @()
    foreach ($m in $Mask) { $extra += @('--mask', $m) }
    $a = Join-Path $work "$Name.cs.dump"
    $b = Join-Path $work "$Name.rs.dump"
    try {
        $null = Invoke-Tool -Exe $cs -Arguments @('create', $one, $Role)
        $null = Invoke-Tool -Exe $rs -Arguments @('create', $two, $Role)
        $null = Invoke-Tool -Exe $cs -Arguments (@('dump', $one, '--out', $a) + $extra)
        $null = Invoke-Tool -Exe $cs -Arguments (@('dump', $two, '--out', $b) + $extra)
        Write-Result $Name (Measure-Dump $a) $a $b
    }
    catch { Write-Failure $Name $_.Exception.Message }
}

<#
.SYNOPSIS
    같은 입력의 사본 둘을 C# 로 판올림한 뒤 C# 와 Rust 가 같은 편집 대본을 돌리고, 연산마다의 결과 글과 끝난 파일을
    견준다(#9 E4). 결과는 사례 둘 — 이름(파일 덤프)과 이름.out(결과 글).

    결과 글에서는 소문자 16진 32자리 덩이(Guid "N" — 맞춤 뷰·가공식의 식별자)만 m 으로 바꾼 뒤 견준다. 그 밖의 칸은
    가리지 않는다. 판올림을 C# 하나로 하는 것은 대본 앞의 차이를 지우려는 것이다 — 판올림 자체는 migrate-* 가 잰다.
#>
function Compare-Run {
    param([string]$Name, [string]$Source, [string]$Script, [string[]]$Mask = @(), [string[]]$MaskHex = @())
    $one = Join-Path $work "$Name.cs.pclm"
    $two = Join-Path $work "$Name.rs.pclm"
    $extra = @()
    foreach ($m in $Mask) { $extra += @('--mask', $m) }
    foreach ($m in $MaskHex) { $extra += @('--mask-hex', $m) }
    $a = Join-Path $work "$Name.cs.dump"
    $b = Join-Path $work "$Name.rs.dump"
    try {
        $null = Invoke-Tool -Exe $cs -Arguments @('snapshot', $Source, $one)
        $null = Invoke-Tool -Exe $cs -Arguments @('snapshot', $Source, $two)
        $null = Invoke-Tool -Exe $cs -Arguments @('migrate', $one)
        $null = Invoke-Tool -Exe $cs -Arguments @('migrate', $two)
        $null = Invoke-Tool -Exe $cs -Arguments @('run', $one, $Script) -Log (Join-Path $work "$Name.cs.log")
        $null = Invoke-Tool -Exe $rs -Arguments @('run', $two, $Script) -Log (Join-Path $work "$Name.rs.log")
        $null = Invoke-Tool -Exe $cs -Arguments (@('dump', $one, '--out', $a) + $extra)
        $null = Invoke-Tool -Exe $cs -Arguments (@('dump', $two, '--out', $b) + $extra)
        Write-Result $Name (Measure-Dump $a) $a $b
    }
    catch { Write-Failure $Name $_.Exception.Message; return }
    try {
        $outA = Join-Path $work "$Name.cs.out"
        $outB = Join-Path $work "$Name.rs.out"
        $utf8 = New-Object System.Text.UTF8Encoding($false)
        $id = New-Object System.Text.RegularExpressions.Regex('(?<=^|[^0-9a-f]|\\u[0-9A-F]{4})[0-9a-f]{32}(?![0-9a-f])',
            [System.Text.RegularExpressions.RegexOptions]::Multiline)
        foreach ($pair in @(@("$one.out.json", $outA), @("$two.out.json", $outB))) {
            $text = [System.IO.File]::ReadAllText($pair[0], $utf8)
            [System.IO.File]::WriteAllText($pair[1], $id.Replace($text, 'm'), $utf8)
        }
        $lines = [System.IO.File]::ReadAllLines($outA, $utf8)
        $ops = @($lines | Where-Object { $_.StartsWith('{"op":') }).Count
        $errors = @($lines | Where-Object { $_.StartsWith('{"op":') -and $_.Contains('","error":') }).Count
        Write-Result "$Name.out" "연산 $ops (오류 $errors)" $outA $outB
    }
    catch { Write-Failure "$Name.out" $_.Exception.Message }
}

<#
.SYNOPSIS
    다리(#8 ⑤) — 같은 입력의 사본 둘을 C# 로 판올림한 뒤, C# 다리(Pclm.App Bridge.Invoke)와 Rust 다리(pclm-bridge)가 같은
    다리 대본의 부름을 차례로 풀고, 답 글(<사본>.bridge.txt)을 바이트로 견준다. -View 면 열람 창처럼 사본을 다시 떠 읽기 전용
    으로 연다(PclmFile.OpenView — 열람 사본은 쪽마다의 홈 view 에 선다).

    두 사본은 쪽마다 다른 폴더에 같은 이름(계약자료.pclm)으로 둔다 — 열람 문지기의 거절 글에 파일 이름이 든다. 가리는
    것은 없다: 「지금」 은 두 계기가 같은 순간(2026-09-10T00:00:00Z)으로 못박는다(README 「다리」). 끝나면 열람 사본이 남지
    않았는지 본다(다리를 놓으면 걷혀야 한다).
#>
function Compare-Bridge {
    param([string]$Name, [string]$Source, [string]$Script, [switch]$View)
    $flag = if ($View) { @('--view') } else { @() }
    try {
        $sides = @(@('cs', $cs), @('rs', $rs))
        foreach ($side in $sides) {
            $dir = Join-Path $work "$Name\$($side[0])"
            New-Item -ItemType Directory -Force -Path (Join-Path $dir 'home') | Out-Null
            $copy = Join-Path $dir '계약자료.pclm'
            $null = Invoke-Tool -Exe $cs -Arguments @('snapshot', $Source, $copy)
            $null = Invoke-Tool -Exe $cs -Arguments @('migrate', $copy)
        }
        foreach ($side in $sides) {
            $dir = Join-Path $work "$Name\$($side[0])"
            $null = Invoke-Tool -Exe $side[1] -Arguments (@('bridge', (Join-Path $dir 'home'), (Join-Path $dir '계약자료.pclm'), $Script) + $flag) `
                -Log (Join-Path $work "$Name.$($side[0]).log")
            $left = @(Get-ChildItem -Path (Join-Path $dir 'home\view') -File -ErrorAction SilentlyContinue)
            if ($left.Count -gt 0) { throw "$($side[0]) 의 열람 사본이 남았습니다($($left.Count))" }
        }
        $a = Join-Path $work "$Name\cs\계약자료.pclm.bridge.txt"
        $b = Join-Path $work "$Name\rs\계약자료.pclm.bridge.txt"
        $lines = [System.IO.File]::ReadAllLines($a, [System.Text.Encoding]::UTF8)
        $refused = @($lines | Where-Object { $_.Contains('"ok":false') }).Count
        Write-Result $Name "부름 $($lines.Length) (거절 $refused)" $a $b
    }
    catch { Write-Failure $Name $_.Exception.Message }
}

<#
.SYNOPSIS
    엑셀 다리(#8 ⑥) — Compare-Bridge 에 고르기 창을 더한다. 두 계기가 같은 대본을 풀되 부름의 "pick" 이 고르기 창의 답이다
    (--files 아래의 상대 경로, null 이면 취소). C# 계기가 지은 계획 엑셀(plan-fixtures, 지어낸 값)을 두 쪽이 같은 자리에서
    읽고, 내보낸 파일(files\out)은 쪽마다 옮겨 둔다.

    사례는 넷 이상이다 — 이름(답 글), 이름.prompts(고르기 창에 건넨 물음), 이름.db(끝난 사본의 C# 덤프, 계획을 들인 시각
    plan.imported_at 과 $randomColumns 를 가린다), 이름/<파일>(내보낸 통합 문서마다 덤프). 통합 문서는 두 계기의 덤프가
    먼저 서로 같아야 한다 — 자끼리 갈리면 그 파일은 실패다.
#>
function Compare-BridgeExcel {
    param([string]$Name, [string]$Source, [string]$Script, [switch]$View)
    $flag = if ($View) { @('--view') } else { @() }
    $utf8 = New-Object System.Text.UTF8Encoding($false)
    $files = Join-Path $work "$Name\files"
    $sides = @(@('cs', $cs), @('rs', $rs))
    try {
        $null = Invoke-Tool -Exe $cs -Arguments @('plan-fixtures', (Join-Path $files 'plan'))
        foreach ($side in $sides) {
            $dir = Join-Path $work "$Name\$($side[0])"
            New-Item -ItemType Directory -Force -Path (Join-Path $dir 'home') | Out-Null
            $copy = Join-Path $dir '계약자료.pclm'
            $null = Invoke-Tool -Exe $cs -Arguments @('snapshot', $Source, $copy)
            $null = Invoke-Tool -Exe $cs -Arguments @('migrate', $copy)
        }
        foreach ($side in $sides) {
            $dir = Join-Path $work "$Name\$($side[0])"
            $null = Invoke-Tool -Exe $side[1] -Arguments (@('bridge', (Join-Path $dir 'home'), (Join-Path $dir '계약자료.pclm'), $Script,
                    '--files', $files) + $flag) -Log (Join-Path $work "$Name.$($side[0]).log")
            $left = @(Get-ChildItem -Path (Join-Path $dir 'home\view') -File -ErrorAction SilentlyContinue)
            if ($left.Count -gt 0) { throw "$($side[0]) 의 열람 사본이 남았습니다($($left.Count))" }
            $out = Join-Path $files 'out'
            if (Test-Path $out) { Move-Item -Path $out -Destination (Join-Path $dir 'out') }
            else { New-Item -ItemType Directory -Force -Path (Join-Path $dir 'out') | Out-Null }
        }
        $a = Join-Path $work "$Name\cs\계약자료.pclm.bridge.txt"
        $b = Join-Path $work "$Name\rs\계약자료.pclm.bridge.txt"
        $lines = [System.IO.File]::ReadAllLines($a, [System.Text.Encoding]::UTF8)
        $refused = @($lines | Where-Object { $_.Contains('"ok":false') }).Count
        Write-Result $Name "부름 $($lines.Length) (거절 $refused)" $a $b
    }
    catch { Write-Failure $Name $_.Exception.Message; return }
    try {
        $a = Join-Path $work "$Name\cs\계약자료.pclm.prompts.txt"
        $b = Join-Path $work "$Name\rs\계약자료.pclm.prompts.txt"
        Write-Result "$Name.prompts" "물음 $([System.IO.File]::ReadAllLines($a, $utf8).Length)" $a $b
    }
    catch { Write-Failure "$Name.prompts" $_.Exception.Message }
    try {
        $extra = @()
        foreach ($m in $randomColumns + @('plan.imported_at')) { $extra += @('--mask', $m) }
        $a = Join-Path $work "$Name.cs.dump"
        $b = Join-Path $work "$Name.rs.dump"
        $null = Invoke-Tool -Exe $cs -Arguments (@('dump', (Join-Path $work "$Name\cs\계약자료.pclm"), '--out', $a) + $extra)
        $null = Invoke-Tool -Exe $cs -Arguments (@('dump', (Join-Path $work "$Name\rs\계약자료.pclm"), '--out', $b) + $extra)
        Write-Result "$Name.db" (Measure-Dump $a) $a $b
    }
    catch { Write-Failure "$Name.db" $_.Exception.Message }
    $books = @(Get-ChildItem -Path (Join-Path $work "$Name\cs\out"), (Join-Path $work "$Name\rs\out") -Filter '*.xlsx' -File |
        ForEach-Object { $_.Name } | Sort-Object -Unique)
    foreach ($book in $books) {
        $case = "$Name/$([System.IO.Path]::GetFileNameWithoutExtension($book))"
        try {
            $dumps = @{}
            foreach ($made in 'cs', 'rs') {
                $file = Join-Path $work "$Name\$made\out\$book"
                if (-not (Test-Path $file)) { throw "$made 쪽에 파일이 없습니다" }
                foreach ($side in $sides) {
                    $d = Join-Path $work "$Name\$made\out\$book.$($side[0]).xlsx.txt"
                    $null = Invoke-Tool -Exe $side[1] -Arguments @('xlsx-dump', $file, $d)
                    $dumps["$made.$($side[0])"] = $d
                }
                $one = (Get-FileHash -Algorithm SHA256 $dumps["$made.cs"]).Hash
                $two = (Get-FileHash -Algorithm SHA256 $dumps["$made.rs"]).Hash
                if ($one -ne $two) { throw "$made 쪽 파일을 두 계기가 다르게 덤프했습니다(줄 $(Find-FirstDifferences $dumps["$made.cs"] $dumps["$made.rs"]))" }
            }
            $dumpLines = [System.IO.File]::ReadAllLines($dumps['cs.cs'], $utf8)
            $sheets = @($dumpLines | Where-Object { $_.StartsWith("sheet`t") }).Count
            $cells = @($dumpLines | Where-Object { $_.StartsWith("cell`t") }).Count
            Write-Result $case "시트 $sheets 칸 $cells" $dumps['cs.cs'] $dumps['rs.cs']
        }
        catch { Write-Failure $case $_.Exception.Message }
    }
}

<#
.SYNOPSIS
    제출본 묶음 하나를 C# 와 Rust 가 각자 취합하고(#11), 두 취합본을 같은 자(C# 덤프)로, 두 보고 글(<out>.txt)을 바이트로
    견준다. 결과는 사례 둘 — 이름(취합본 덤프)과 이름.txt(보고 글). -Expect 2 면 받아들인 것이 없어 멈추는 사례다 — 두 쪽
    모두 2 로 끝나고 취합본을 짓지 않아야 하며, 글(멈춘 까닭)만 견준다.

    보고 글에서는 취합본 경로(마지막 줄 「파일」)만 <out> 으로 바꾼다 — 두 쪽이 다른 자리에 짓기 때문이다. 입력은 취합
    전후의 SHA256 이 같아야 한다(남의 파일은 읽기만 한다, ADR-031).
#>
function Compare-Merge {
    param([string]$Name, [string[]]$Sources, [int]$Expect = 0)
    $one = Join-Path $work "$Name.cs.pclm"
    $two = Join-Path $work "$Name.rs.pclm"
    $extra = @()
    foreach ($m in $randomColumns) { $extra += @('--mask', $m) }
    try {
        $inputs = @(foreach ($s in $Sources) {
                if (Test-Path $s -PathType Container) { Get-ChildItem -Path $s -File | ForEach-Object { $_.FullName } }
                elseif (Test-Path $s) { $s }
            })
        $before = @($inputs | ForEach-Object { (Get-FileHash -Algorithm SHA256 $_).Hash })
        $a = Invoke-Tool -Exe $cs -Arguments (@('merge', $one) + $Sources) -Log (Join-Path $work "$Name.cs.log") -AllowExit @(2)
        $b = Invoke-Tool -Exe $rs -Arguments (@('merge', $two) + $Sources) -Log (Join-Path $work "$Name.rs.log") -AllowExit @(2)
        if ($a.Exit -ne $Expect -or $b.Exit -ne $Expect) { throw "종료 코드 C# $($a.Exit) · Rust $($b.Exit) (기대 $Expect)" }
        $after = @($inputs | ForEach-Object { (Get-FileHash -Algorithm SHA256 $_).Hash })
        if (($before -join ',') -ne ($after -join ',')) { throw '취합이 입력 파일을 바꿨습니다' }
        if ($Expect -eq 0) {
            $da = Join-Path $work "$Name.cs.dump"
            $db = Join-Path $work "$Name.rs.dump"
            $null = Invoke-Tool -Exe $cs -Arguments (@('dump', $one, '--out', $da) + $extra)
            $null = Invoke-Tool -Exe $cs -Arguments (@('dump', $two, '--out', $db) + $extra)
            Write-Result $Name (Measure-Dump $da) $da $db
        }
        elseif ((Test-Path $one) -or (Test-Path $two)) { throw '멈춰야 할 취합이 취합본을 지었습니다' }
    }
    catch { Write-Failure $Name $_.Exception.Message; return }
    try {
        $utf8 = New-Object System.Text.UTF8Encoding($false)
        $ta = Join-Path $work "$Name.cs.txt"
        $tb = Join-Path $work "$Name.rs.txt"
        foreach ($pair in @(@("$one.txt", $one, $ta), @("$two.txt", $two, $tb))) {
            $text = [System.IO.File]::ReadAllText($pair[0], $utf8)
            [System.IO.File]::WriteAllText($pair[2], $text.Replace($pair[1], '<out>'), $utf8)
        }
        $lines = [System.IO.File]::ReadAllLines($ta, $utf8).Length
        Write-Result "$Name.txt" "글 $($lines)줄" $ta $tb
    }
    catch { Write-Failure "$Name.txt" $_.Exception.Message }
}

<#
.SYNOPSIS
    E5 — Rust 가 쓴 파일을 남이 읽는가. C# 의 읽기 손잡이(Inspect → Access.Read 의 Migrate → 뷰마다 count)가 지나고,
    두 계기의 덤프가 같으며 — Rust 계기는 문서나르미(data_pclm.rs)처럼 SQLITE_OPEN_READ_ONLY 만으로 연다, Core 의 연결
    설정(DQS·수학 함수) 없이 — 어느 뷰도 읽기에 실패하지 않고, 읽은 뒤 파일 해시가 그대로인가.
#>
#   박제(replay)에서는 C# 의 읽기 손잡이를 되살릴 수 없다 — 그때 C# 이 읽은 것은 그때 Rust 가 지은 파일이다. 박제는 그때
#   C# 이 그 파일을 판마다 뽑는 칸($editColumns)을 가려 덤프한 것이고, 지금 Rust 가 지은 같은 파일의 같은 덤프가 그와 같은지,
#   어느 뷰도 읽기에 실패하지 않는지, 읽은 뒤 해시가 그대로인지 본다.
function Get-ReadMask {
    $extra = @()
    foreach ($m in $editColumns) { $extra += @('--mask', $m) }
    foreach ($m in $editHexColumns) { $extra += @('--mask-hex', $m) }
    return , $extra
}

function Compare-Read {
    param([string]$Name, [string]$File)
    $script:tapeOff = $true
    try {
        if (-not (Test-Path $File)) { throw "견줄 파일이 없습니다(앞 사례가 실패했나): $File" }
        $before = (Get-FileHash -Algorithm SHA256 $File).Hash
        if ($mode -eq 'replay') {
            if (-not $script:named.ContainsKey($Name)) { throw "박제에 이 사례가 없습니다: $Name" }
            $a = Join-Path $work "$Name.cs.dump"
            $b = Join-Path $work "$Name.rs.dump"
            [System.IO.File]::WriteAllBytes($a, (Convert-PathBytes (Read-Blob $script:named[$Name]) $a $false))
            $null = Invoke-Tool -Exe $rs -Arguments (@('dump', $File, '--out', $b) + (Get-ReadMask))
            if (Select-String -Path $b -Pattern "^view`t[^`t]*`terror`t" -Quiet) { throw "읽히지 않는 뷰가 있습니다: $Name.rs.dump" }
            $after = (Get-FileHash -Algorithm SHA256 $File).Hash
            if ($before -ne $after) { throw "읽기만 했는데 파일이 바뀌었습니다: $File" }
            Write-Result $Name (Measure-Dump $a) $a $b
            return
        }
        $check = Invoke-Tool -Exe $cs -Arguments @('readcheck', $File) -Log (Join-Path $work "$Name.readcheck.log")
        if ($check.Out -notmatch "inspect Ok \w+ v$schemaVersion") { throw "C# 가 지금 판의 PCLM 으로 읽지 못했습니다: $(($check.Out -split "`n")[0])" }
        Compare-Dump -Name $Name -File $File
        foreach ($dump in "$Name.cs.dump", "$Name.rs.dump") {
            if (Select-String -Path (Join-Path $work $dump) -Pattern "^view`t[^`t]*`terror`t" -Quiet) { throw "읽히지 않는 뷰가 있습니다: $dump" }
        }
        if ($mode -eq 'record') {
            $expected = Join-Path $work "$Name.golden.cs.dump"
            $twin = Join-Path $work "$Name.golden.rs.dump"
            $null = Invoke-Tool -Exe $cs -Arguments (@('dump', $File, '--out', $expected) + (Get-ReadMask))
            $null = Invoke-Tool -Exe $rs -Arguments (@('dump', $File, '--out', $twin) + (Get-ReadMask))
            if ((Get-FileHash -Algorithm SHA256 $expected).Hash -ne (Get-FileHash -Algorithm SHA256 $twin).Hash) {
                $script:tapeProblems.Add("$Name : 가린 덤프가 C# 와 Rust 에서 다릅니다")
            }
            $script:named[$Name] = Save-Blob (Convert-PathBytes ([System.IO.File]::ReadAllBytes($expected)) $expected $true)
        }
        $after = (Get-FileHash -Algorithm SHA256 $File).Hash
        if ($before -ne $after) { throw "읽기만 했는데 파일이 바뀌었습니다: $File" }
    }
    catch { Write-Failure $Name $_.Exception.Message }
    finally { $script:tapeOff = $false }
}

# ── 사례 ─────────────────────────────────────────────

<#
.SYNOPSIS
    확장 호스트(#10) — 같은 홈(같은 경로)을 두 번 짓고 C# 호스트(계기의 host, Program.cs 의 갈래)와 Rust 호스트
    (pclm-erp-host --native-dev --home)에 차례로 같은 틀 지은 대본을 흘린다. 결과는 사례 셋 — 이름(답 글), 이름.home(끝난
    홈의 파일 목록과 글 파일), -Db 면 이름.db(작업자료 덤프). 두 쪽 종료 코드가 -Expect 와 같아야 한다.

    가리는 것(README 「호스트 견주기」): 답에서는 baseToken(지금 상태의 지문 — 저장 뒤에는 고친 시각을 담는다)만, 홈의 글
    파일에서는 yyyy-MM-ddTHH:mm:ss 시각과 홈 경로만, 덤프에서는 $hostColumns 만. pid 가 든 연결·탭 파일은 끝나면 지워지므로
    목록에 남으면 그대로 갈린다.
#>
function Compare-Host {
    param([string]$Name, [string]$Seed, [string]$Variant, [string]$Transcript, [string]$Origin, [int]$Expect = 0, [switch]$Db)
    $homeDir = Join-Path $work "$Name.home"
    $csHome = "$homeDir.cs"
    $rsHome = "$homeDir.rs"
    $utf8 = New-Object System.Text.UTF8Encoding($false)
    try {
        $null = Invoke-Tool -Exe $cs -Arguments @('host-home', $Seed, $homeDir, $Variant)
        $csArgs = @('host', $homeDir, $Transcript, (Join-Path $work "$Name.cs.bin"))
        if ($Origin) { $csArgs += $Origin }
        $a = Invoke-Tool -Exe $cs -Arguments $csArgs -Log (Join-Path $work "$Name.cs.log") -AllowExit @(1, 2)
        Rename-Item -LiteralPath $homeDir -NewName (Split-Path -Leaf $csHome)
        $null = Invoke-Tool -Exe $cs -Arguments @('host-home', $Seed, $homeDir, $Variant)
        $rsArgs = @('--native-dev', '--home', $homeDir)
        if ($Origin) { $rsArgs = @('--home', $homeDir, $Origin) }
        # 틀은 바이너리다 — 표준 입출력을 파일에 바로 잇는다(글로 읽으면 길이 머리가 깨진다).
        if ($mode -eq 'record') { Add-RustReads ($rsArgs + @($Transcript)) }
        $treeBefore = if ($mode -eq 'record') { Get-Tree } else { $null }
        $process = Start-Process -FilePath $rsHost -ArgumentList (($rsArgs | ForEach-Object { Format-Argument $_ }) -join ' ') `
            -RedirectStandardInput $Transcript -RedirectStandardOutput (Join-Path $work "$Name.rs.bin") `
            -RedirectStandardError (Join-Path $work "$Name.rs.log") -NoNewWindow -Wait -PassThru
        if ($null -ne $treeBefore) { Add-RustMade $treeBefore }
        Rename-Item -LiteralPath $homeDir -NewName (Split-Path -Leaf $rsHome)
        if ($a.Exit -ne $Expect -or $process.ExitCode -ne $Expect) { throw "종료 코드 C# $($a.Exit) · Rust $($process.ExitCode) (기대 $Expect)" }
        $count = 0
        foreach ($side in 'cs', 'rs') {
            $bin = Join-Path $work "$Name.$side.bin"
            # 끝 바이트까지 — if 의 출력으로 받으면 빈 배열이 $null 로 풀린다. C# 은 2 로 멈출 때 파일을 짓지 않는다(빈 것과 같다).
            [byte[]]$bytes = @()
            if (Test-Path $bin) { $bytes = [System.IO.File]::ReadAllBytes($bin) }
            $lines = New-Object System.Collections.Generic.List[string]
            $at = 0
            while ($at + 4 -le $bytes.Length) {
                $length = [System.BitConverter]::ToInt32($bytes, $at)
                if ($at + 4 + $length -gt $bytes.Length) { break }
                $text = $utf8.GetString($bytes, $at + 4, $length)
                $lines.Add(($text -creplace '"baseToken":"[0-9A-F]{64}"', '"baseToken":"<m>"'))
                $at += 4 + $length
            }
            if ($at -ne $bytes.Length) { $lines.Add("잘린 꼬리 $($bytes.Length - $at)바이트") }
            $count = $lines.Count
            [System.IO.File]::WriteAllText((Join-Path $work "$Name.$side.out"), (($lines -join "`n") + "`n"), $utf8)
        }
        Write-Result $Name "답 $count · 종료 $Expect" (Join-Path $work "$Name.cs.out") (Join-Path $work "$Name.rs.out")
    }
    catch { Write-Failure $Name $_.Exception.Message; return }
    try {
        $stamp = New-Object System.Text.RegularExpressions.Regex('\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d')
        foreach ($pair in @(@($csHome, 'cs'), @($rsHome, 'rs'))) {
            $base = (Resolve-Path -LiteralPath $pair[0]).Path
            $entries = [string[]]@(Get-ChildItem -LiteralPath $base -Recurse -Force | ForEach-Object {
                    $relative = $_.FullName.Substring($base.Length + 1)
                    if ($_.PSIsContainer) { "$relative\" } else { $relative }
                })
            [System.Array]::Sort($entries, [System.StringComparer]::Ordinal)
            $text = New-Object System.Text.StringBuilder
            foreach ($entry in $entries) {
                $null = $text.Append("== $entry`n")
                if ($entry.EndsWith('\') -or $entry -match '\.pclm(-wal|-shm|-journal)?$') { continue }
                $content = [System.IO.File]::ReadAllText((Join-Path $base $entry), $utf8)
                # 쪽지는 홈 경로를 담는다 — 두 쪽이 같은 자리에서 돌았으니 이름만 바뀐 지금 경로를 <home> 으로.
                $content = $content.Replace($homeDir, '<home>').Replace($homeDir.Replace('\', '\\'), '<home>')
                $null = $text.Append($stamp.Replace($content, '<t>')).Append("`n")
            }
            [System.IO.File]::WriteAllText((Join-Path $work "$Name.files.$($pair[1])"), $text.ToString(), $utf8)
        }
        Write-Result "$Name.home" "파일 $(@(Get-ChildItem -LiteralPath $csHome -Recurse -Force).Count)" (Join-Path $work "$Name.files.cs") (Join-Path $work "$Name.files.rs")
    }
    catch { Write-Failure "$Name.home" $_.Exception.Message }
    if (-not $Db) { return }
    try {
        $extra = @()
        foreach ($m in $hostColumns) { $extra += @('--mask', $m) }
        $dumpA = Join-Path $work "$Name.db.cs.dump"
        $dumpB = Join-Path $work "$Name.db.rs.dump"
        $null = Invoke-Tool -Exe $cs -Arguments (@('dump', (Join-Path $csHome 'work.pclm'), '--out', $dumpA) + $extra)
        $null = Invoke-Tool -Exe $cs -Arguments (@('dump', (Join-Path $rsHome 'work.pclm'), '--out', $dumpB) + $extra)
        Write-Result "$Name.db" (Measure-Dump $dumpA) $dumpA $dumpB
    }
    catch { Write-Failure "$Name.db" $_.Exception.Message }
}

<#
.SYNOPSIS
    창 잠금(Home.TryLock)이 두 구현에서 같은 커널 객체인가. 한쪽이 잡은 채 머무는 동안 다른 쪽이 같은 홈(대문자로 쓴
    경로·끝 구분자를 붙인 경로 포함)은 busy, 다른 홈은 free 로 봐야 한다. 홈 이름에 ı·ä 를 넣는다 — .NET
    ToUpperInvariant 는 ı 를 그대로 두고 Rust to_uppercase 는 I 로 바꿔, 표를 쓰지 않으면 다른 이름이 된다.
#>
function Compare-Lock {
    param([string]$Name)
    $lockHome = Join-Path $work 'lock-ı-ä-home'
    $other = Join-Path $work 'lock-other'
    New-Item -ItemType Directory -Force -Path $lockHome, $other | Out-Null
    $expected = Join-Path $work "$Name.expected"
    $observed = Join-Path $work "$Name.observed"
    $utf8 = New-Object System.Text.UTF8Encoding($false)
    # 커널 객체를 두 구현이 함께 쓰는지는 C# 이 있을 때만 잴 수 있다 — 박제(replay)에서는 두 자리 모두 Rust 다(같은 기대 글).
    $turns = if ($mode -eq 'replay') { @(@($rs, $rs, 'cs-hold'), @($rs, $rs, 'rs-hold')) } else { @(@($cs, $rs, 'cs-hold'), @($rs, $cs, 'rs-hold')) }
    $script:tapeOff = $true
    try {
        $lines = New-Object System.Collections.Generic.List[string]
        foreach ($turn in $turns) {
            $info = New-Object System.Diagnostics.ProcessStartInfo
            $info.FileName = $turn[0]
            $info.Arguments = (@('lock', $lockHome, '--hold') | ForEach-Object { Format-Argument $_ }) -join ' '
            $info.UseShellExecute = $false
            $info.RedirectStandardInput = $true
            $info.RedirectStandardOutput = $true
            $info.StandardOutputEncoding = $utf8
            $holder = [System.Diagnostics.Process]::Start($info)
            try {
                $lines.Add("$($turn[2]) $($holder.StandardOutput.ReadLine())")
                foreach ($probe in @(@('same', $lockHome), @('upper', $lockHome.ToUpperInvariant()), @('slash', "$lockHome\"), @('other', $other))) {
                    $lines.Add("$($turn[2]) $($probe[0]) $((Invoke-Tool -Exe $turn[1] -Arguments @('lock', $probe[1])).Out.Trim())")
                }
            }
            finally {
                $holder.StandardInput.Close()
                $holder.WaitForExit()
            }
        }
        [System.IO.File]::WriteAllText($observed, (($lines -join "`n") + "`n"), $utf8)
        $want = foreach ($side in 'cs-hold', 'rs-hold') { "$side held"; "$side same busy"; "$side upper busy"; "$side slash busy"; "$side other free" }
        [System.IO.File]::WriteAllText($expected, (($want -join "`n") + "`n"), $utf8)
        Write-Result $Name "잡기 2 · 보기 8" $expected $observed
    }
    catch { Write-Failure $Name $_.Exception.Message }
    finally { $script:tapeOff = $false }
}

<#
.SYNOPSIS
    홈(#8 ④) — 쪽지 쓰기. 두 계기가 각자의 홈에 같은 인자로 쪽지를 쓰고, 두 config.json 을 바이트로 견준다.
#>
function Compare-ConfigWrite {
    param([string]$Name, [string[]]$Arguments)
    try {
        $files = foreach ($side in @(@($cs, 'cs'), @($rs, 'rs'))) {
            $homeDir = Join-Path $work "$Name.$($side[1])"
            $null = Invoke-Tool -Exe $side[0] -Arguments (@('config-write', $homeDir) + $Arguments) -Log (Join-Path $work "$Name.$($side[1]).log")
            Join-Path $homeDir 'config.json'
        }
        $length = (Get-Item -LiteralPath $files[0]).Length
        Write-Result $Name "쪽지 $($length)바이트" $files[0] $files[1]
    }
    catch { Write-Failure $Name $_.Exception.Message }
}

# 홈 사례의 글에서 가리는 것 — 사례 자리(두 쪽이 같은 자리에서 돌고 이름만 바꿨다), 백업 이름의 시각, -RandomId 면 신원.
function Hide-HomeText([string]$text, [string]$caseDir, [switch]$RandomId) {
    $text = $text.Replace($caseDir, '<root>').Replace($caseDir.Replace('\', '\\'), '<root>')
    $text = [regex]::Replace($text, '(계약자료_v\d+_)\d{8}-\d{6}', '$1<t>')
    if ($RandomId) { $text = [regex]::Replace($text, '(?<![0-9a-f])[0-9a-f]{32}(?![0-9a-f])', '<id>') }
    return $text
}

function Test-Sqlite([string]$path) {
    $stream = [System.IO.File]::OpenRead($path)
    try {
        $head = New-Object byte[] 16
        $read = $stream.Read($head, 0, 16)
        return $read -eq 16 -and [System.Text.Encoding]::ASCII.GetString($head, 0, 15) -eq 'SQLite format 3'
    }
    finally { $stream.Dispose() }
}

<#
.SYNOPSIS
    홈(#8 ④) — 같은 자리에 같은 입력 홈을 두 번 두고(한 번 지어 복사한다 — 입력의 무작위 신원이 두 쪽에서 같다) C# 와 Rust
    계기에 같은 명령(resolve · switch)을 시킨 뒤 .cs/.rs 로 이름을 바꾼다. 사례 셋 — 이름(낸 글), 이름.tree(끝난 자리의
    파일·폴더 목록과 SQLite 가 아닌 파일의 글), 이름.db(SQLite 파일마다 C# 덤프, $randomColumns 만 가림).

    -Build 는 사례 자리(빈 폴더)를 받아 입력을 짓는다. 인자의 {case} 는 사례 자리로 바뀐다.
#>
function Compare-HomeCase {
    param([string]$Name, [scriptblock]$Build, [string[]]$Arguments, [switch]$RandomId)
    $caseDir = Join-Path $work $Name
    $pristine = "$caseDir.input"
    $utf8 = New-Object System.Text.UTF8Encoding($false)
    try {
        New-Item -ItemType Directory -Force -Path $caseDir | Out-Null
        & $Build $caseDir
        Copy-Item -LiteralPath $caseDir -Destination $pristine -Recurse
        $outputs = @{}
        foreach ($side in @(@($cs, 'cs'), @($rs, 'rs'))) {
            if (-not (Test-Path -LiteralPath $caseDir)) { Copy-Item -LiteralPath $pristine -Destination $caseDir -Recurse }
            $run = Invoke-Tool -Exe $side[0] -Arguments ($Arguments | ForEach-Object { $_.Replace('{case}', $caseDir) }) `
                -Log (Join-Path $work "$Name.$($side[1]).log")
            $outputs[$side[1]] = $run.Out
            Rename-Item -LiteralPath $caseDir -NewName "$(Split-Path -Leaf $caseDir).$($side[1])"
        }
        foreach ($side in 'cs', 'rs') {
            [System.IO.File]::WriteAllText((Join-Path $work "$Name.$side.out"), (Hide-HomeText $outputs[$side] $caseDir -RandomId:$RandomId), $utf8)
        }
        # 사례 줄에는 갈래만 — ready · ok 는 낱말 하나, problem · error 는 그 갈래까지(경로·글은 내지 않는다).
        $head = @(($outputs['cs'] -split "`n")[0] -split "`t")
        $first = if ($head[0] -in 'ready', 'ok' -or $head.Count -lt 2) { $head[0] } else { "$($head[0]) $($head[1])" }
        Write-Result $Name $first (Join-Path $work "$Name.cs.out") (Join-Path $work "$Name.rs.out")
    }
    catch { Write-Failure $Name $_.Exception.Message; return }

    # SQLite 파일은 가린 이름으로 짝짓는다 — 백업 이름의 시각이 두 쪽에서 다르다.
    $databases = New-Object System.Collections.Generic.List[string]
    $actual = @{ cs = @{}; rs = @{} }
    try {
        foreach ($side in 'cs', 'rs') {
            $base = (Resolve-Path -LiteralPath "$caseDir.$side").Path
            $entries = [string[]]@(Get-ChildItem -LiteralPath $base -Recurse -Force | ForEach-Object {
                    $relative = $_.FullName.Substring($base.Length + 1)
                    if ($_.PSIsContainer) { "$relative\" } else { $relative }
                })
            [System.Array]::Sort($entries, [System.StringComparer]::Ordinal)
            $text = New-Object System.Text.StringBuilder
            foreach ($entry in $entries) {
                $null = $text.Append("== $entry`n")
                if ($entry.EndsWith('\')) { continue }
                $full = Join-Path $base $entry
                if ((Get-Item -LiteralPath $full).Length -eq 0) { continue }
                if (Test-Sqlite $full) {
                    $masked = Hide-HomeText $entry $caseDir
                    $actual[$side][$masked] = $entry
                    if ($side -eq 'cs') { $databases.Add($masked) }
                    continue
                }
                if ($entry -match '-(wal|shm|journal)$') { continue }
                $null = $text.Append([System.IO.File]::ReadAllText($full, $utf8)).Append("`n")
            }
            [System.IO.File]::WriteAllText((Join-Path $work "$Name.tree.$side"), (Hide-HomeText $text.ToString() $caseDir -RandomId:$RandomId), $utf8)
        }
        Write-Result "$Name.tree" "파일 $(@(Get-ChildItem -LiteralPath "$caseDir.cs" -Recurse -Force -File).Count)" `
            (Join-Path $work "$Name.tree.cs") (Join-Path $work "$Name.tree.rs")
    }
    catch { Write-Failure "$Name.tree" $_.Exception.Message; return }

    if ($databases.Count -eq 0) { return }
    try {
        $extra = @()
        foreach ($m in $randomColumns) { $extra += @('--mask', $m) }
        foreach ($side in 'cs', 'rs') {
            $text = New-Object System.Text.StringBuilder
            foreach ($entry in $databases) {
                $null = $text.Append("== $entry`n")
                if (-not $actual[$side].ContainsKey($entry)) { $null = $text.Append("없음`n"); continue }
                $file = Join-Path "$caseDir.$side" $actual[$side][$entry]
                $dump = Join-Path $work "$Name.db.$side.part"
                $null = Invoke-Tool -Exe $cs -Arguments (@('dump', $file, '--out', $dump) + $extra)
                $null = $text.Append([System.IO.File]::ReadAllText($dump, $utf8))
            }
            [System.IO.File]::WriteAllText((Join-Path $work "$Name.db.$side"), (Hide-HomeText $text.ToString() $caseDir), $utf8)
        }
        Write-Result "$Name.db" "파일 $($databases.Count)" (Join-Path $work "$Name.db.cs") (Join-Path $work "$Name.db.rs")
    }
    catch { Write-Failure "$Name.db" $_.Exception.Message }
}

<#
.SYNOPSIS
    명령줄 하나를 표준 입력·작업 폴더를 정해 돌리고 표준 출력·오류를 바이트 그대로 받는다. 글로 읽으면 BOM·줄 끝이 지워진다.
#>
function Invoke-Cli {
    param([string]$Exe, [string[]]$Arguments, [string]$Cwd, [string]$Stdin = '')
    if ($Exe -eq $csCli -and $script:tape -and -not $script:tapeOff) { return Invoke-Taped 'cli' $Exe $Arguments $Cwd $Stdin }
    if ($mode -eq 'record') {
        Add-RustReads ($Arguments + @($Cwd))
        $before = Get-Tree
        $result = Start-Cli $Exe $Arguments $Cwd $Stdin
        Add-RustMade $before
        return $result
    }
    return Start-Cli $Exe $Arguments $Cwd $Stdin
}

function Start-Cli([string]$Exe, [string[]]$Arguments, [string]$Cwd, [string]$Stdin) {
    $info = New-Object System.Diagnostics.ProcessStartInfo
    $info.FileName = $Exe
    $info.Arguments = ($Arguments | ForEach-Object { Format-Argument $_ }) -join ' '
    $info.UseShellExecute = $false
    $info.RedirectStandardInput = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $info.WorkingDirectory = $Cwd
    $process = [System.Diagnostics.Process]::Start($info)
    $out = New-Object System.IO.MemoryStream
    $err = New-Object System.IO.MemoryStream
    $copyOut = $process.StandardOutput.BaseStream.CopyToAsync($out)
    $copyErr = $process.StandardError.BaseStream.CopyToAsync($err)
    $bytes = (New-Object System.Text.UTF8Encoding($false)).GetBytes($Stdin)
    $process.StandardInput.BaseStream.Write($bytes, 0, $bytes.Length)
    $process.StandardInput.Close()
    $process.WaitForExit()
    $copyOut.Wait(); $copyErr.Wait()
    return [pscustomobject]@{ Out = $out.ToArray(); Err = $err.ToArray(); Exit = $process.ExitCode }
}

# 명령줄 사례의 글에서 가리는 것 — 홈 사례의 것(사례 자리 · 백업 이름의 시각 · -RandomId 면 신원)에 더해, 「오늘」 을 박은
# 기본 파일 이름(관리대장_ · 제출_ · 취합_ 의 yyyyMMdd). 두 쪽은 같은 날 다른 순간에 돈다.
function Hide-CliText([string]$text, [string]$caseDir, [switch]$RandomId) {
    $text = Hide-HomeText $text $caseDir -RandomId:$RandomId
    return [regex]::Replace($text, '(?<=_)\d{8}(?=\.(xlsx|pclm))', '<오늘>')
}

<#
.SYNOPSIS
    명령줄(#8 ⑦) — 같은 자리에 같은 입력(홈 · 작업자료 · 넣을 파일)을 두 번 두고 C# pclm(src/Pclm.Cli)과 Rust pclm(crates/
    pclm-cli)에 같은 명령들을 차례로 시킨 뒤 .cs/.rs 로 이름을 바꾼다. 작업 폴더는 사례 자리의 cwd 다 — 기본 이름으로
    떨어지는 파일(관리대장_ · 제출_ · 취합_)이 거기 선다. 표준 입력은 비워 넘긴다(읽는 명령이 없다).

    사례 셋 — 이름(명령마다 종료 코드 · 표준 출력 · 표준 오류, 바이트 그대로이되 CR 은 <CR> 로 드러낸다), 이름.tree(끝난 자리의
    파일 · 폴더 목록과 글 파일의 글, 통합 문서는 C# xlsx-dump), 이름.db(SQLite 파일마다 C# 덤프 — 가리는 것은 $cliColumns).

    -Build 는 사례 자리(빈 폴더)를 받아 입력을 짓는다. 명령의 {case} 는 사례 자리로 바뀐다. -Steps 는 명령의 배열이다(명령
    하나는 글 배열, 하나뿐이면 글 배열 그대로).
#>
function Compare-Cli {
    param([string]$Name, [scriptblock]$Build, [object[]]$Steps, [switch]$RandomId)
    if ($Steps.Count -gt 0 -and $Steps[0] -is [string]) { $Steps = @(, $Steps) }
    $caseDir = Join-Path $work $Name
    $pristine = "$caseDir.input"
    $utf8 = New-Object System.Text.UTF8Encoding($false)
    try {
        New-Item -ItemType Directory -Force -Path (Join-Path $caseDir 'cwd') | Out-Null
        & $Build $caseDir
        Copy-Item -LiteralPath $caseDir -Destination $pristine -Recurse
        $codes = @{}
        foreach ($side in @(@($csCli, 'cs'), @($rsCli, 'rs'))) {
            if (-not (Test-Path -LiteralPath $caseDir)) { Copy-Item -LiteralPath $pristine -Destination $caseDir -Recurse }
            $text = New-Object System.Text.StringBuilder
            $n = 0
            $exits = @()
            foreach ($step in $Steps) {
                $n++
                $arguments = @($step | ForEach-Object { $_.Replace('{case}', $caseDir) })
                $run = Invoke-Cli -Exe $side[0] -Arguments $arguments -Cwd (Join-Path $caseDir 'cwd')
                $exits += $run.Exit
                $null = $text.Append("== $n exit $($run.Exit)`n--- stdout ---`n").Append($utf8.GetString($run.Out))
                $null = $text.Append("`n--- stderr ---`n").Append($utf8.GetString($run.Err)).Append("`n")
            }
            $codes[$side[1]] = $exits -join ' '
            $shown = (Hide-CliText $text.ToString() $caseDir -RandomId:$RandomId).Replace("`r", '<CR>')
            [System.IO.File]::WriteAllText((Join-Path $work "$Name.$($side[1]).out"), $shown, $utf8)
            Rename-Item -LiteralPath $caseDir -NewName "$(Split-Path -Leaf $caseDir).$($side[1])"
        }
        Write-Result $Name "명령 $($Steps.Count) · 종료 $($codes['cs'])" (Join-Path $work "$Name.cs.out") (Join-Path $work "$Name.rs.out")
    }
    catch { Write-Failure $Name $_.Exception.Message; return }

    # SQLite 파일은 가린 이름으로 짝짓는다 — 기본 이름의 날짜 · 백업 이름의 시각이 두 쪽에서 다를 수 있다.
    $databases = New-Object System.Collections.Generic.List[string]
    $actual = @{ cs = @{}; rs = @{} }
    try {
        foreach ($side in 'cs', 'rs') {
            $base = (Resolve-Path -LiteralPath "$caseDir.$side").Path
            $entries = [string[]]@(Get-ChildItem -LiteralPath $base -Recurse -Force | ForEach-Object {
                    $relative = $_.FullName.Substring($base.Length + 1)
                    if ($_.PSIsContainer) { "$relative\" } else { $relative }
                })
            [System.Array]::Sort($entries, [System.StringComparer]::Ordinal)
            $text = New-Object System.Text.StringBuilder
            foreach ($entry in $entries) {
                $null = $text.Append("== $entry`n")
                if ($entry.EndsWith('\')) { continue }
                $full = Join-Path $base $entry
                if ((Get-Item -LiteralPath $full).Length -eq 0) { continue }
                if (Test-Sqlite $full) {
                    $masked = Hide-CliText $entry $caseDir
                    $actual[$side][$masked] = $entry
                    if ($side -eq 'cs') { $databases.Add($masked) }
                    continue
                }
                if ($entry -match '-(wal|shm|journal)$') { continue }
                if ($entry -match '\.xlsx$') {
                    # 통합 문서는 zip 바이트가 아니라 칸을 견준다 — 같은 자(C# xlsx-dump).
                    $dump = Join-Path $work "$Name.xlsx.$side.part"
                    $null = Invoke-Tool -Exe $cs -Arguments @('xlsx-dump', $full, $dump)
                    $null = $text.Append([System.IO.File]::ReadAllText($dump, $utf8))
                    continue
                }
                $null = $text.Append([System.IO.File]::ReadAllText($full, $utf8)).Append("`n")
            }
            [System.IO.File]::WriteAllText((Join-Path $work "$Name.tree.$side"), (Hide-CliText $text.ToString() $caseDir -RandomId:$RandomId), $utf8)
        }
        Write-Result "$Name.tree" "파일 $(@(Get-ChildItem -LiteralPath "$caseDir.cs" -Recurse -Force -File).Count)" `
            (Join-Path $work "$Name.tree.cs") (Join-Path $work "$Name.tree.rs")
    }
    catch { Write-Failure "$Name.tree" $_.Exception.Message; return }

    if ($databases.Count -eq 0) { return }
    try {
        $extra = @()
        foreach ($m in $cliColumns) { $extra += @('--mask', $m) }
        foreach ($m in $editHexColumns) { $extra += @('--mask-hex', $m) }
        foreach ($side in 'cs', 'rs') {
            $text = New-Object System.Text.StringBuilder
            foreach ($entry in $databases) {
                $null = $text.Append("== $entry`n")
                if (-not $actual[$side].ContainsKey($entry)) { $null = $text.Append("없음`n"); continue }
                $file = Join-Path "$caseDir.$side" $actual[$side][$entry]
                $dump = Join-Path $work "$Name.db.$side.part"
                $null = Invoke-Tool -Exe $cs -Arguments (@('dump', $file, '--out', $dump) + $extra)
                $null = $text.Append([System.IO.File]::ReadAllText($dump, $utf8))
            }
            [System.IO.File]::WriteAllText((Join-Path $work "$Name.db.$side"), (Hide-CliText $text.ToString() $caseDir), $utf8)
        }
        Write-Result "$Name.db" "파일 $($databases.Count)" (Join-Path $work "$Name.db.cs") (Join-Path $work "$Name.db.rs")
    }
    catch { Write-Failure "$Name.db" $_.Exception.Message }
}

# 다리 홈 사례의 글에서 가리는 것 — 명령줄 사례의 것(사례 자리 · 백업 이름의 시각 · 신원 · 「오늘」 을 박은 파일 이름)에 더해,
# 두 쪽이 다른 순간에 적는 시각(ISO 꼴 · 가져오기 답의 HH:mm)과 새로 뽑은 GUID(자동 수집의 수집 ID), 「오늘」 을 박은 글 · JSON
# 파일 이름(취합_ 보고 · 보기구성_).
function Hide-BridgeText([string]$text, [string]$caseDir) {
    $text = Hide-CliText $text $caseDir -RandomId
    $text = [regex]::Replace($text, '\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(\.\d{1,7})?(Z|[+-]\d\d:\d\d)?', '<시각>')
    $text = [regex]::Replace($text, '"at":"\d\d:\d\d"', '"at":"<시분>"')
    # 기준 지문은 갱신 시각까지 담은 저장 상태의 해시라 진짜 시계로 돈 두 계기에서 다르다.
    $text = [regex]::Replace($text, '"baseToken":"[0-9A-F]{64}"', '"baseToken":"<지문>"')
    $text = [regex]::Replace($text, '(?<![0-9a-f])[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}(?![0-9a-f])', '<guid>')
    return [regex]::Replace($text, '(?<=_)\d{8}(?=\.(txt|json))', '<오늘>')
}

<#
.SYNOPSIS
    다리의 홈 부름(#8 ⑧a) — 같은 자리에 같은 홈(seed 의 사본 data\계약자료.pclm 을 쪽지로 묶은 home, 고르기 창이 답할 files)을
    두 번 두고 C# 다리와 Rust 다리에 같은 다리 대본을 시킨 뒤 .cs/.rs 로 이름을 바꾼다. 두 계기는 진짜 시계로 돈다(--clock now —
    C# 다리가 수집 기록 · 파일 이름의 날짜를 DateTime.Now 로 읽는다). -View 면 그 작업자료를 열람 창으로 연다.

    사례 넷 — 이름(답 글), 이름.prompts(창에 건넨 물음 · 다시 띄우기 · 닫기 · 알림 영역 · 확장에 보낸 명령), 이름.tree(끝난 자리의
    파일 · 폴더 목록과 글 파일의 글), 이름.db(SQLite 파일마다 C# 덤프). 가리는 것은 Hide-BridgeText 와, 덤프의 $cliColumns ·
    $hostColumns · 수집 기록의 지문(erp_capture.request_hash · result_json 의 식별자 덩이)이다.

    -Build 는 홈을 묶은 사례 자리를 받아 입력을 더 짓는다(files 아래).
#>
function Compare-BridgeHome {
    param([string]$Name, [string]$Script, [scriptblock]$Build, [switch]$View)
    $caseDir = Join-Path $work $Name
    $pristine = "$caseDir.input"
    $utf8 = New-Object System.Text.UTF8Encoding($false)
    $workfile = Join-Path $caseDir 'data\계약자료.pclm'
    $flag = if ($View) { @('--view') } else { @() }
    try {
        New-Item -ItemType Directory -Force -Path (Join-Path $caseDir 'files') | Out-Null
        New-Item -ItemType Directory -Force -Path (Join-Path $caseDir 'home') | Out-Null
        Copy-Seed $workfile
        $null = Invoke-Tool -Exe $cs -Arguments @('migrate', $workfile)
        Bind $caseDir $workfile
        if ($Build) { & $Build $caseDir }
        Copy-Item -LiteralPath $caseDir -Destination $pristine -Recurse
        foreach ($side in @(@($cs, 'cs'), @($rs, 'rs'))) {
            if (-not (Test-Path -LiteralPath $caseDir)) { Copy-Item -LiteralPath $pristine -Destination $caseDir -Recurse }
            $null = Invoke-Tool -Exe $side[0] -Arguments (@('bridge', (Join-Path $caseDir 'home'), $workfile, $Script,
                    '--files', (Join-Path $caseDir 'files'), '--clock', 'now') + $flag) -Log (Join-Path $work "$Name.$($side[1]).log")
            foreach ($kind in 'bridge', 'prompts') {
                $made = "$workfile.$kind.txt"
                $text = [System.IO.File]::ReadAllText($made, $utf8)
                [System.IO.File]::WriteAllText((Join-Path $work "$Name.$kind.$($side[1])"), (Hide-BridgeText $text $caseDir), $utf8)
                Remove-Item -LiteralPath $made
            }
            $left = @(Get-ChildItem -Path (Join-Path $caseDir 'home\view') -File -ErrorAction SilentlyContinue)
            if ($left.Count -gt 0) { throw "$($side[1]) 의 열람 사본이 남았습니다($($left.Count))" }
            if (Test-Path -LiteralPath (Join-Path $caseDir 'home\extension-live')) { throw "$($side[1]) 의 확장 연결 기록이 남았습니다" }
            Rename-Item -LiteralPath $caseDir -NewName "$(Split-Path -Leaf $caseDir).$($side[1])"
        }
        $lines = [System.IO.File]::ReadAllLines((Join-Path $work "$Name.bridge.cs"), $utf8)
        $refused = @($lines | Where-Object { $_.Contains('"ok":false') }).Count
        Write-Result $Name "부름 $($lines.Length) (거절 $refused)" (Join-Path $work "$Name.bridge.cs") (Join-Path $work "$Name.bridge.rs")
        $asked = [System.IO.File]::ReadAllLines((Join-Path $work "$Name.prompts.cs"), $utf8).Length
        Write-Result "$Name.prompts" "물음 $asked" (Join-Path $work "$Name.prompts.cs") (Join-Path $work "$Name.prompts.rs")
    }
    catch { Write-Failure $Name $_.Exception.Message; return }

    # SQLite 파일은 가린 이름으로 짝짓는다 — 「오늘」 을 박은 이름 · 백업 이름의 시각이 두 쪽에서 다를 수 있다.
    $databases = New-Object System.Collections.Generic.List[string]
    $actual = @{ cs = @{}; rs = @{} }
    try {
        foreach ($side in 'cs', 'rs') {
            $base = (Resolve-Path -LiteralPath "$caseDir.$side").Path
            $entries = [string[]]@(Get-ChildItem -LiteralPath $base -Recurse -Force | ForEach-Object {
                    $relative = $_.FullName.Substring($base.Length + 1)
                    if ($_.PSIsContainer) { "$relative\" } else { $relative }
                })
            [System.Array]::Sort($entries, [System.StringComparer]::Ordinal)
            $text = New-Object System.Text.StringBuilder
            foreach ($entry in $entries) {
                $null = $text.Append("== $entry`n")
                if ($entry.EndsWith('\')) { continue }
                $full = Join-Path $base $entry
                if ((Get-Item -LiteralPath $full).Length -eq 0) { continue }
                if (Test-Sqlite $full) {
                    $masked = Hide-BridgeText $entry $caseDir
                    $actual[$side][$masked] = $entry
                    if ($side -eq 'cs') { $databases.Add($masked) }
                    continue
                }
                if ($entry -match '-(wal|shm|journal)$') { continue }
                # 지어 둔 실행 파일(내장 확장 준비의 입력)은 글이 아니다 — 크기만.
                if ($entry -match '\.exe$') { $null = $text.Append("$((Get-Item -LiteralPath $full).Length) 바이트`n"); continue }
                $null = $text.Append([System.IO.File]::ReadAllText($full, $utf8)).Append("`n")
            }
            [System.IO.File]::WriteAllText((Join-Path $work "$Name.tree.$side"), (Hide-BridgeText $text.ToString() $caseDir), $utf8)
        }
        Write-Result "$Name.tree" "파일 $(@(Get-ChildItem -LiteralPath "$caseDir.cs" -Recurse -Force -File).Count)" `
            (Join-Path $work "$Name.tree.cs") (Join-Path $work "$Name.tree.rs")
    }
    catch { Write-Failure "$Name.tree" $_.Exception.Message; return }

    if ($databases.Count -eq 0) { return }
    try {
        $extra = @()
        foreach ($m in $cliColumns + $hostColumns + @('erp_capture.request_hash')) { $extra += @('--mask', $m) }
        foreach ($m in $editHexColumns + @('erp_capture.result_json')) { $extra += @('--mask-hex', $m) }
        foreach ($side in 'cs', 'rs') {
            $text = New-Object System.Text.StringBuilder
            foreach ($entry in $databases) {
                $null = $text.Append("== $entry`n")
                if (-not $actual[$side].ContainsKey($entry)) { $null = $text.Append("없음`n"); continue }
                $file = Join-Path "$caseDir.$side" $actual[$side][$entry]
                $dump = Join-Path $work "$Name.db.$side.part"
                $null = Invoke-Tool -Exe $cs -Arguments (@('dump', $file, '--out', $dump) + $extra)
                $null = $text.Append([System.IO.File]::ReadAllText($dump, $utf8))
            }
            [System.IO.File]::WriteAllText((Join-Path $work "$Name.db.$side"), (Hide-BridgeText $text.ToString() $caseDir), $utf8)
        }
        Write-Result "$Name.db" "파일 $($databases.Count)" (Join-Path $work "$Name.db.cs") (Join-Path $work "$Name.db.rs")
    }
    catch { Write-Failure "$Name.db" $_.Exception.Message }
}

$schemaVersion = 32

# 판마다 새로 뽑는 칸 — 파일 이름표의 정체와 지은 시각, 예시 보기의 식별자(표와, 그것을 그대로 내놓는 v_맞춤뷰).
$randomColumns = @('pclm_file.dataset_id', 'pclm_file.created_at', 'user_view.id', 'v_맞춤뷰.뷰식별자')
# 편집이 더 뽑는 칸 — 사람 값·덮개를 적은 시각과 가공열의 식별자(표의 options 안 JSON 과 v_가공열). options·가공식은
# 식별자 덩이만 가리고(--mask-hex) 나머지 글자는 견준다.
$editColumns = $randomColumns + @('contract_user_field.updated_at', 'notice_user_field.updated_at', 'request_user_field.updated_at',
    'field_override.updated_at', 'v_가공열.식별자')
$editHexColumns = @('user_column.options', 'v_가공열.가공식')
# 명령줄이 더 적는 시각 — 계획을 들인 때(plan import). 사람 값 · 덮개의 시각과 제출본 · 취합본 · 첫 실행의 신원은 위에 있다.
$cliColumns = $editColumns + @('plan.imported_at')
$scripts = Join-Path $root 'crates\pclm-parity\scripts'
# 확장 호스트의 저장이 새로 적는 시각 — 수집 기록·원천·공고를 고친 때와 그것을 그대로 내놓는 뷰.
$hostColumns = @('erp_capture.captured_at', 'erp_source.updated_at', 'notice.updated_at', 'v_ERP원천.수집시각')

$work = Join-Path $root ("artifacts\parity\" + (Get-Date -Format 'yyyyMMdd-HHmmss'))
New-Item -ItemType Directory -Force -Path $work | Out-Null
Write-Host "작업 폴더: $work"
$gloss = @{ record = '박제를 뜬다 (C# 와 나란히)'; replay = '박제와 견준다'; live = 'C# 와 나란히' }
Write-Host "갈래: $($gloss[$mode])"

if (-not $NoBuild) { Invoke-Build }
if ($script:tape) { Initialize-Tape }

# 자 자체 — 저장 꼴·이스케이프·정렬·깨진 뷰.
$fixture = Join-Path $work 'fixture.db'
$null = Invoke-Tool -Exe $cs -Arguments @('fixture', $fixture)
Compare-Dump -Name 'fixture' -File $fixture

# (a) 갓 지은 작업자료.
$created = Join-Path $work 'create-work.pclm'
$null = Invoke-Tool -Exe $cs -Arguments @('create', $created, 'work')
Compare-Dump -Name 'create-work' -File $created

foreach ($role in 'work', 'submission', 'merged') {
    Compare-Create -Name "make-$role" -Role $role -Mask $randomColumns
}

# (b) 비교용 자료 — 시험 문서 + (있으면) ERP JSON.
$seeded = Join-Path $work 'seed.pclm'
$seedArgs = @('seed', $seeded)
if ($Erp) { $seedArgs += @('--erp', (Resolve-Path $Erp).Path) }
try {
    $seed = Invoke-Tool -Exe $cs -Arguments $seedArgs -Log (Join-Path $work 'seed.log')
    $name = if ($Erp) { 'seed+erp' } else { 'seed' }
    Compare-Dump -Name $name -File $seeded
    $erpLine = ($seed.Out -split "`n" | Where-Object { $_.StartsWith('erp_files') }) -join ''
    if ($erpLine) { Write-Host "  ERP: $($erpLine.Trim())  (건너뛴 까닭은 seed.log)" }
}
catch { Write-Failure 'seed' $_.Exception.Message }
# (b2) 같은 seed 를 Rust 계기가 따로 짓는다(#10) — 시험 문서의 넣기와, ERP JSON 의 「JSON 파일로 가져오기」 길(Prepare →
# DefaultChoices → Save). 두 파일을 같은 자(C# 덤프)로 견주고, 표준 출력(센 수)과 건너뛴 까닭도 견준다(콘솔에는 내지
# 않는다 — 사례 줄의 same/DIFF 뿐). 수집 ID 는 두 계기가 파일 차례로 같게 짓는다. 그 밖에 판마다 갈리는 칸 — 고친 시각과,
# 그 시각을 담은 지금 상태의 지문(baseToken)에서 오는 요청 지문, 영수증의 datasetId — 을 가린다.
$seedColumns = $randomColumns + @('request.updated_at', 'notice.updated_at', 'contract.updated_at',
    'erp_source.updated_at', 'erp_capture.captured_at', 'erp_capture.request_hash', 'v_ERP원천.수집시각')
$seedHexColumns = @('erp_capture.result_json')
if (Test-Path $seeded) {
    $twin = if ($Erp) { 'seed+erp-rs' } else { 'seed-rs' }
    $seededRs = Join-Path $work 'seed.rs.pclm'
    $rsSeedArgs = @('seed', $seededRs)
    if ($Erp) { $rsSeedArgs += @('--erp', (Resolve-Path $Erp).Path) }
    try {
        $rsSeed = Invoke-Tool -Exe $rs -Arguments $rsSeedArgs -Log (Join-Path $work 'seed.rs.log')
        $a = Join-Path $work "$twin.cs.dump"
        $b = Join-Path $work "$twin.rs.dump"
        $extra = @()
        foreach ($m in $seedColumns) { $extra += @('--mask', $m) }
        foreach ($m in $seedHexColumns) { $extra += @('--mask-hex', $m) }
        $null = Invoke-Tool -Exe $cs -Arguments (@('dump', $seeded, '--out', $a) + $extra)
        $null = Invoke-Tool -Exe $cs -Arguments (@('dump', $seededRs, '--out', $b) + $extra)
        Write-Result $twin (Measure-Dump $a) $a $b
        $utf8 = New-Object System.Text.UTF8Encoding($false)
        $outA = Join-Path $work "$twin.cs.out"
        $outB = Join-Path $work "$twin.rs.out"
        foreach ($pair in @(@($seed, $outA), @($rsSeed, $outB))) {
            $text = ($pair[0].Out -replace "`r", '') + "--- stderr ---`n" + ($pair[0].Err -replace "`r", '')
            [System.IO.File]::WriteAllText($pair[1], $text, $utf8)
        }
        $skips = @(($seed.Err -split "`n") | Where-Object { $_.StartsWith('건너뜀 ') }).Count
        Write-Result "$twin.out" "건너뜀 $skips" $outA $outB
    }
    catch { Write-Failure $twin $_.Exception.Message }
}
# 뷰만 다시 짓기 — 지금 판 자료(예시 보기 + 시험 문서 + ERP)에서 두 Core 가 같은 뷰를 짓는가. 사람이 채우는 열이
# 든 자료는 (d) 의 판올림 견주기가 본다 — 판올림도 끝에 뷰를 다시 짓는다.
if (Test-Path $seeded) {
    Compare-Op -Name 'refresh-seed' -Source $seeded -CsArgs @('refresh', '{file}') -RsArgs @('refresh', '{file}')
    # 편집 — 지어낸 값만 적는 대본(seed.json)과, 실제 자료에도 돌리는 작은 대본(real.json).
    Compare-Run -Name 'run-seed' -Source $seeded -Script (Join-Path $scripts 'seed.json') -Mask $editColumns -MaskHex $editHexColumns
    Compare-Run -Name 'run-real-on-seed' -Source $seeded -Script (Join-Path $scripts 'real.json') -Mask $editColumns -MaskHex $editHexColumns
    # 다리 — 읽기 요청 전부(작업자료 창과 열람 창)와, 열람 창의 문지기.
    Compare-Bridge -Name 'bridge-seed' -Source $seeded -Script (Join-Path $scripts 'bridge-read.json')
    Compare-Bridge -Name 'bridge-seed-view' -Source $seeded -Script (Join-Path $scripts 'bridge-read.json') -View
    Compare-Bridge -Name 'bridge-seed-gate' -Source $seeded -Script (Join-Path $scripts 'bridge-view.json') -View
    # 엑셀 — 설정 · 계획 가져오기(지어낸 계획 엑셀) · 내보내기 · 화면 사진(작업자료 창)과, 열람 창의 같은 부름.
    Compare-BridgeExcel -Name 'bridge-excel' -Source $seeded -Script (Join-Path $scripts 'bridge-excel.json')
    Compare-BridgeExcel -Name 'bridge-excel-view' -Source $seeded -Script (Join-Path $scripts 'bridge-excel-view.json') -View
}

# (c) 옛 판 — 판올림 견주기의 입력.
foreach ($version in 19, 21, 25, 31) {
    $synth = Join-Path $work "synth-v$version.pclm"
    try {
        $null = Invoke-Tool -Exe $cs -Arguments @('synth', $synth, [string]$version)
        Compare-Dump -Name "synth-v$version" -File $synth
        Compare-Op -Name "migrate-v$version" -Source $synth -CsArgs @('migrate', '{file}') -RsArgs @('migrate', '{file}') -Mask $randomColumns
    }
    catch { Write-Failure "synth-v$version" $_.Exception.Message }
}
# 손으로 고친 글(깨진 가공식·겹친 키 가공식·객체가 아닌 맞춤 뷰 정의·겹친 키 정의)을 더 심은 옛 판. v21 은 v22 의 옮기기를,
# 둘 다 v30 의 진단 달기를, v25 는 예시 보기 갈아 끼우기와 열 이름 옮기기를 지난다. ERP 원문의 겹친 키는 넣지 않는다 —
# C# 판올림이 멈추는 것이 지금의 동작이라 견줄 것이 없다.
foreach ($version in 21, 25) {
    $synth = Join-Path $work "synth-v$version-odd.pclm"
    try {
        $null = Invoke-Tool -Exe $cs -Arguments @('synth', $synth, [string]$version, '--odd')
        Compare-Dump -Name "synth-v$version-odd" -File $synth
        Compare-Op -Name "migrate-v$version-odd" -Source $synth -CsArgs @('migrate', '{file}') -RsArgs @('migrate', '{file}') -Mask $randomColumns
    }
    catch { Write-Failure "synth-v$version-odd" $_.Exception.Message }
}

# (c2) 제출본·취합본(#11). 입력은 C# 계기가 짓는다 — seed 의 작업자료 사본 셋에 편집 대본(merge-*.json, 사람 값·덮개·
# 사용자 열)과 날 SQL(merge-*.sql, 접수·공고·계약 줄과 상대자·계획·맞춤 뷰)을 시켜 표마다 겹침을 심고, 둘은 제출본으로
# 뜬다. 셋째는 작업자료 그대로 넣는다(이름이 없어 파일 이름으로 불린다). 거절할 것 — 없는 파일, PCLM 이 아닌 파일, 옛
# 시험판, 새 판, 취합본, 백업 — 과 옛 판(v31) 작업자료를 함께 넣는다.
if (Test-Path $seeded) {
    $mergeIn = Join-Path $work 'merge-in'
    try {
        New-Item -ItemType Directory -Force -Path $mergeIn | Out-Null
        foreach ($who in 'a', 'b', 'c') {
            $copy = Join-Path $mergeIn "work-$who.pclm"
            $null = Invoke-Tool -Exe $cs -Arguments @('snapshot', $seeded, $copy)
            $null = Invoke-Tool -Exe $cs -Arguments @('run', $copy, (Join-Path $scripts "merge-$who.json")) -Log (Join-Path $work "merge-in-$who.log")
            $null = Invoke-Tool -Exe $cs -Arguments @('sql', $copy, (Join-Path $scripts "merge-$who.sql"))
        }
        $subA = Join-Path $mergeIn '제출_홍길동.pclm'
        $subB = Join-Path $mergeIn '제출_김철수.pclm'
        $null = Invoke-Tool -Exe $cs -Arguments @('submit', (Join-Path $mergeIn 'work-a.pclm'), $subA)
        $null = Invoke-Tool -Exe $cs -Arguments @('submit', (Join-Path $mergeIn 'work-b.pclm'), $subB)
        $workC = Join-Path $mergeIn 'work-c.pclm'
        $missing = Join-Path $mergeIn '없는파일.pclm'
        $notPclm = Join-Path $mergeIn '엉뚱.pclm'
        [System.IO.File]::WriteAllText($notPclm, '남의 것', (New-Object System.Text.UTF8Encoding($false)))
        $old = Join-Path $mergeIn '옛판.pclm'
        $null = Invoke-Tool -Exe $cs -Arguments @('sql', $old, (Join-Path $scripts 'merge-old.sql'))
        $newer = Join-Path $mergeIn '새판.pclm'
        $null = Invoke-Tool -Exe $cs -Arguments @('submit', $seeded, $newer)
        $null = Invoke-Tool -Exe $cs -Arguments @('sql', $newer, (Join-Path $scripts 'merge-newer.sql'))
        $mergedRole = Join-Path $mergeIn '지난취합.pclm'
        $null = Invoke-Tool -Exe $cs -Arguments @('create', $mergedRole, 'merged')
        $backupRole = Join-Path $mergeIn '백업.pclm'
        $null = Invoke-Tool -Exe $cs -Arguments @('create', $backupRole, 'backup')
        # 폴더로 넘기는 묶음 — find_submissions 가 걷는다. .pclm 이 아닌 파일은 걸리지 않는다.
        $mergeDir = Join-Path $work 'merge-dir'
        New-Item -ItemType Directory -Force -Path $mergeDir | Out-Null
        Copy-Item $subA, $subB, $mergedRole -Destination $mergeDir
        [System.IO.File]::WriteAllText((Join-Path $mergeDir '메모.txt'), '취합 대상이 아님', (New-Object System.Text.UTF8Encoding($false)))

        $synth31 = Join-Path $work 'synth-v31.pclm'
        $mixed = @($subA, $subB, $workC, $missing, $notPclm, $old, $newer, $mergedRole, $backupRole)
        if (Test-Path $synth31) { $mixed += $synth31 }
        Compare-Merge -Name 'merge-conflicts' -Sources $mixed
        Compare-Merge -Name 'merge-folder' -Sources @($mergeDir)
        Compare-Merge -Name 'merge-single' -Sources @($seeded)
        Compare-Merge -Name 'merge-none' -Sources @($missing, $notPclm, $mergedRole) -Expect 2
    }
    catch { Write-Failure 'merge-in' $_.Exception.Message }
}

# (d) 실제 자료. 원본은 읽기 전용 사본으로만 뜨고, 뜨기 전후의 해시가 같아야 한다.
if ($Real) {
    $homeDir = Join-Path $env:LOCALAPPDATA 'Pclm'
    $originals = New-Object System.Collections.Generic.List[string]
    $backups = Join-Path $homeDir 'backups'
    if (Test-Path $backups) {
        Get-ChildItem -Path $backups -Filter '*.pclm' -File | Sort-Object Name | ForEach-Object { $originals.Add($_.FullName) }
    }
    $config = Join-Path $homeDir 'config.json'
    if (Test-Path $config) {
        $workfile = (Get-Content -Raw -Encoding UTF8 $config | ConvertFrom-Json).workfile
        if ($workfile -and (Test-Path $workfile)) { $originals.Add($workfile) }
    }
    if ($originals.Count -eq 0) { Write-Host '실제 자료: 잴 파일이 없습니다.' }
    $index = 0
    foreach ($original in $originals) {
        $index++
        $name = "real-$index"
        $copy = Join-Path $work "$name.pclm"
        try {
            $before = (Get-FileHash -Algorithm SHA256 $original).Hash
            $null = Invoke-Tool -Exe $cs -Arguments @('snapshot', $original, $copy)
            $after = (Get-FileHash -Algorithm SHA256 $original).Hash
            if ($before -ne $after) { throw "원본이 바뀌었습니다: $original" }
            Write-Host "  $name = $(Split-Path -Leaf $original) (원본 해시 그대로)"
            Compare-Dump -Name $name -File $copy
            Compare-Op -Name "migrate-$name" -Source $copy -CsArgs @('migrate', '{file}') -RsArgs @('migrate', '{file}') -Mask $randomColumns
            Compare-Run -Name "run-$name" -Source $copy -Script (Join-Path $scripts 'real.json') -Mask $editColumns -MaskHex $editHexColumns
            # 다리의 읽기 요청 — 답 글은 artifacts 에만 남고 콘솔에는 부름 수뿐이다.
            Compare-Bridge -Name "bridge-$name" -Source $copy -Script (Join-Path $scripts 'bridge-read.json')
            Compare-Bridge -Name "bridge-$name-view" -Source $copy -Script (Join-Path $scripts 'bridge-read.json') -View
        }
        catch { Write-Failure $name $_.Exception.Message }
    }
}

# (d2) 확장 호스트(#10). seed 의 사본을 작업자료로 묶은 홈과, 묶이지 않은 홈 갈래(쪽지 없음·깨짐·겹친 열쇠·상대 경로·
# 다른 datasetId·판 2·null·배열·제출본)에서 같은 대본에 같은 답·같은 홈 파일을 내는가. 종료 코드 — 틀이 잘리거나 길이가
# 0 이거나 초과면 1, 허락되지 않은 확장이면 2. 그리고 창 잠금의 커널 객체 이름.
if (Test-Path $seeded) {
    $transcripts = Join-Path $work 'host-transcripts'
    try {
        $null = Invoke-Tool -Exe $cs -Arguments @('host-transcripts', $seeded, $transcripts)
        Compare-Host -Name 'host-full' -Seed $seeded -Variant 'ok' -Transcript (Join-Path $transcripts 'full.bin') -Expect 1 -Db
        foreach ($variant in 'ok', 'missing', 'corrupt', 'duplicate', 'relative', 'foreign', 'version', 'nulls', 'array', 'submission') {
            Compare-Host -Name "host-setup-$variant" -Seed $seeded -Variant $variant -Transcript (Join-Path $transcripts 'setup.bin')
        }
        Compare-Host -Name 'host-truncated' -Seed $seeded -Variant 'ok' -Transcript (Join-Path $transcripts 'truncated.bin') -Expect 1
        Compare-Host -Name 'host-zero' -Seed $seeded -Variant 'ok' -Transcript (Join-Path $transcripts 'zero.bin') -Expect 1
        Compare-Host -Name 'host-origin' -Seed $seeded -Variant 'ok' -Transcript (Join-Path $transcripts 'setup.bin') `
            -Origin 'chrome-extension://bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb/' -Expect 2
        Compare-Host -Name 'host-origin-shape' -Seed $seeded -Variant 'ok' -Transcript (Join-Path $transcripts 'setup.bin') `
            -Origin 'chrome-extension://nope/' -Expect 2
    }
    catch { Write-Failure 'host-transcripts' $_.Exception.Message }
}
Compare-Lock -Name 'host-lock'

# (d3) 홈(#8 ④) — 쪽지 쓰기, 시작(resolve), 작업자료 바꾸기(switch). 입력은 C# 계기가 짓는다(seed 의 사본·synth·create·
# submit·sql). 값은 모두 지어낸 것이다. 가리는 것은 사례 자리·백업 이름의 시각·(-RandomId) 새로 뽑은 신원과, 덤프의
# $randomColumns 뿐이다(README 「홈」).
if (Test-Path $seeded) {
    $utf8 = New-Object System.Text.UTF8Encoding($false)
    function Write-Text([string]$path, [string]$text) {
        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $path) | Out-Null
        [System.IO.File]::WriteAllText($path, $text, $utf8)
    }
    function Copy-Seed([string]$path) {
        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $path) | Out-Null
        $null = Invoke-Tool -Exe $cs -Arguments @('snapshot', $seeded, $path)
    }
    function Get-DatasetId([string]$path) { ((Invoke-Tool -Exe $cs -Arguments @('inspect', $path)).Out.Trim() -split "`t")[3] }
    function Write-Config([string]$homeDir, [string]$workfile, [string]$id, [string]$retired) {
        $arguments = @('config-write', $homeDir, $workfile, $id)
        if ($retired) { $arguments += $retired }
        $null = Invoke-Tool -Exe $cs -Arguments $arguments
    }
    function Bind([string]$case, [string]$workfile) { Write-Config (Join-Path $case 'home') $workfile (Get-DatasetId $workfile) }
    function New-Synth([string]$path, [int]$version) {
        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $path) | Out-Null
        $null = Invoke-Tool -Exe $cs -Arguments @('synth', $path, [string]$version)
    }
    function New-Sql([string]$path, [string]$script) {
        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $path) | Out-Null
        $null = Invoke-Tool -Exe $cs -Arguments @('sql', $path, (Join-Path $scripts $script))
    }
    function Json([string]$text) { ConvertTo-Json -InputObject $text -Compress }

    # 쪽지 쓰기 — 한글·HTML 에 민감한 글자·NBSP·BMP 밖 글자·`/`·`..`·끝의 점·상대 경로(작업 폴더는 저장소 뿌리)·retired.
    $configCases = [ordered]@{
        'config-plain'    = @("$work\설정\계약자료.pclm", '0123456789abcdef0123456789abcdef')
        'config-escapes'  = @("$work\a&b'+<>``c\x.pclm", 'ab')
        'config-unicode'  = @("$work\공백 $([char]0x00A0)$([char]0x00AD)$([char]::ConvertFromUtf32(0x1F600))é\x.pclm", 'cd')
        'config-shape'    = @("$($work.Replace('\', '/'))/가/../나/x.pclm.", 'ef')
        'config-relative' = @('상대\x.pclm', '')
        'config-retired'  = @("$work\새 자리.pclm", 'ab', "$work\옛 자리.pclm")
    }
    foreach ($case in $configCases.GetEnumerator()) { Compare-ConfigWrite -Name $case.Key -Arguments $case.Value }

    # 시작 — 처음 실행(옛 자료 옮겨 오기 포함)과 쪽지가 있을 때의 갈래.
    $resolve = @('resolve', '{case}\home')
    Compare-HomeCase -Name 'resolve-first-run' -Arguments $resolve -RandomId -Build { param($c) }
    Compare-HomeCase -Name 'resolve-legacy-home' -Arguments $resolve -Build { param($c) New-Synth "$c\home\pclm.db" 19 }
    Compare-HomeCase -Name 'resolve-legacy-roaming' -Arguments ($resolve + @('--legacy', '{case}\roaming\config.json')) -Build {
        param($c)
        New-Synth "$c\old\pclm.db" 19
        New-Synth "$c\home\pclm.db" 19
        Write-Text "$c\roaming\config.json" "{`"dataDir`":$(Json "$c\old")}"
    }
    Compare-HomeCase -Name 'resolve-legacy-pending' -Arguments ($resolve + @('--legacy', '{case}\roaming\config.json')) -Build {
        param($c)
        New-Synth "$c\old\pclm.db" 19
        Write-Text "$c\roaming\config.json" "{`"dataDir`":$(Json "$c\new"),`"pendingMoveFrom`":$(Json "$c\old")}"
    }
    Compare-HomeCase -Name 'resolve-legacy-refused' -Arguments $resolve -Build { param($c) New-Sql "$c\home\pclm.db" 'merge-old.sql' }
    Compare-HomeCase -Name 'resolve-default-exists' -Arguments $resolve -Build {
        param($c)
        Copy-Seed "$c\home\계약자료.pclm"
        New-Synth "$c\home\pclm.db" 19
    }
    Compare-HomeCase -Name 'resolve-ok' -Arguments $resolve -Build { param($c) Copy-Seed "$c\data\work.pclm"; Bind $c "$c\data\work.pclm" }
    Compare-HomeCase -Name 'resolve-case-path' -Arguments $resolve -Build {
        param($c)
        Copy-Seed "$c\data\work.pclm"
        Write-Config "$c\home" "$c\DATA\WORK.PCLM" (Get-DatasetId "$c\data\work.pclm")
    }
    Compare-HomeCase -Name 'resolve-missing' -Arguments $resolve -Build { param($c) Write-Config "$c\home" "$c\지워진 자리\계약자료.pclm" 'ab' }
    Compare-HomeCase -Name 'resolve-not-pclm' -Arguments $resolve -Build {
        param($c)
        Write-Text "$c\data\엉뚱.pclm" '남의 것'
        Write-Config "$c\home" "$c\data\엉뚱.pclm" 'ab'
    }
    Compare-HomeCase -Name 'resolve-submission' -Arguments $resolve -Build {
        param($c)
        Copy-Seed "$c\data\work.pclm"
        $null = Invoke-Tool -Exe $cs -Arguments @('submit', "$c\data\work.pclm", "$c\data\제출.pclm")
        Bind $c "$c\data\제출.pclm"
    }
    Compare-HomeCase -Name 'resolve-too-old' -Arguments $resolve -Build {
        param($c)
        New-Sql "$c\data\옛판.pclm" 'merge-old.sql'
        Write-Config "$c\home" "$c\data\옛판.pclm" 'ab'
    }
    Compare-HomeCase -Name 'resolve-too-new' -Arguments $resolve -Build {
        param($c)
        Copy-Seed "$c\data\새판.pclm"
        New-Sql "$c\data\새판.pclm" 'merge-newer.sql'
        Write-Config "$c\home" "$c\data\새판.pclm" 'ab'
    }
    Compare-HomeCase -Name 'resolve-old-v19' -Arguments $resolve -Build { param($c) New-Synth "$c\data\옛것.pclm" 19; Bind $c "$c\data\옛것.pclm" }
    Compare-HomeCase -Name 'resolve-old-v31' -Arguments $resolve -Build { param($c) New-Synth "$c\data\v31.pclm" 31; Bind $c "$c\data\v31.pclm" }
    Compare-HomeCase -Name 'resolve-prune' -Arguments $resolve -Build {
        param($c)
        New-Item -ItemType Directory -Force -Path "$c\home\backups" | Out-Null
        for ($n = 0; $n -lt 12; $n++) {
            $old = "$c\home\backups\계약자료_v19_2001010$($n % 10)-0000$('{0:00}' -f $n).pclm"
            Write-Text $old '옛 백업'
            (Get-Item -LiteralPath $old).LastWriteTimeUtc = (New-Object DateTime 2001, 1, 1, 0, 0, 0, ([DateTimeKind]::Utc)).AddMinutes($n)
        }
        # 같은 시각이면 이름의 서수 역순 — 둘을 같은 시각에 둔다.
        (Get-Item -LiteralPath "$c\home\backups\계약자료_v19_20010101-000001.pclm").LastWriteTimeUtc = (Get-Item -LiteralPath "$c\home\backups\계약자료_v19_20010103-000003.pclm").LastWriteTimeUtc
        Write-Text "$c\home\backups\메모.txt" '사람이 둔 것'
        New-Synth "$c\data\옛것.pclm" 19
        Bind $c "$c\data\옛것.pclm"
    }
    Compare-HomeCase -Name 'resolve-retired' -Arguments $resolve -Build {
        param($c)
        Copy-Seed "$c\data\새 자리.pclm"
        $null = Invoke-Tool -Exe $cs -Arguments @('create', "$c\data\옛 자리.pclm", 'retired')
        Write-Config "$c\home" "$c\data\새 자리.pclm" (Get-DatasetId "$c\data\새 자리.pclm") "$c\data\옛 자리.pclm"
    }
    Compare-HomeCase -Name 'resolve-retired-not-ours' -Arguments $resolve -Build {
        param($c)
        Copy-Seed "$c\data\새 자리.pclm"
        $null = Invoke-Tool -Exe $cs -Arguments @('create', "$c\data\남의 것.pclm", 'work')
        Write-Config "$c\home" "$c\data\새 자리.pclm" (Get-DatasetId "$c\data\새 자리.pclm") "$c\data\남의 것.pclm"
    }
    Compare-HomeCase -Name 'resolve-mismatch' -Arguments $resolve -Build { param($c) Copy-Seed "$c\data\work.pclm"; Write-Config "$c\home" "$c\data\work.pclm" 'deadbeef' }
    # 깨진 쪽지 — .NET 의 JSON 예외 글이 드는 갈래(깨진 JSON·틀린 형)는 견주지 않는다(README). {case} 는 사례 자리(JSON 글).
    $corrupt = [ordered]@{
        'version'     = '{"version":2,"workfile":"{case}\\data\\work.pclm","datasetId":"ab"}'
        'relative'    = '{"version":1,"workfile":"상대\\x.pclm","datasetId":"ab"}'
        'no-workfile' = '{"version":1,"datasetId":"ab"}'
        'blank'       = '{"version":1,"workfile":"  ","datasetId":"ab"}'
        'nulls'       = '{"version":1,"workfile":null,"datasetId":null}'
        'empty-root'  = 'null'
    }
    foreach ($variant in $corrupt.GetEnumerator()) {
        $script:corruptText = $variant.Value
        Compare-HomeCase -Name "resolve-corrupt-$($variant.Key)" -Arguments $resolve -Build {
            param($c)
            New-Synth "$c\home\pclm.db" 19
            Write-Text "$c\home\config.json" $script:corruptText.Replace('{case}', $c.Replace('\', '\\'))
        }
    }

    # 작업자료 바꾸기 — 사례 자리의 data\work.pclm 이 창이 연 작업자료다(쪽지가 가리킨다).
    function New-SwitchCase([string]$c) { Copy-Seed "$c\data\work.pclm"; Bind $c "$c\data\work.pclm" }
    function Switch-Args([string]$op, [string[]]$rest) { @('switch', $op, '{case}\home', '{case}\data\work.pclm') + $rest }
    $unc = '\\지어낸서버\공유\계약자료.pclm'
    Compare-HomeCase -Name 'switch-move' -Arguments (Switch-Args 'move' @('{case}\data\새 자리\계약자료.pclm')) -Build { param($c) New-SwitchCase $c }
    Compare-HomeCase -Name 'switch-move-exists' -Arguments (Switch-Args 'move' @('{case}\data\남의 파일.pclm')) -Build {
        param($c) New-SwitchCase $c; Write-Text "$c\data\남의 파일.pclm" '남의 것'
    }
    Compare-HomeCase -Name 'switch-move-wal' -Arguments (Switch-Args 'move' @('{case}\data\반쯤.pclm')) -Build {
        param($c) New-SwitchCase $c; Write-Text "$c\data\반쯤.pclm-wal" ''
    }
    Compare-HomeCase -Name 'switch-move-same' -Arguments (Switch-Args 'move' @('{case}\DATA\work.pclm')) -Build { param($c) New-SwitchCase $c }
    Compare-HomeCase -Name 'switch-move-unc' -Arguments (Switch-Args 'move' @($unc)) -Build { param($c) New-SwitchCase $c }
    Compare-HomeCase -Name 'switch-use' -Arguments (Switch-Args 'use' @('{case}\data\다른 작업자료.pclm')) -Build {
        param($c) New-SwitchCase $c; $null = Invoke-Tool -Exe $cs -Arguments @('create', "$c\data\다른 작업자료.pclm", 'work')
    }
    Compare-HomeCase -Name 'switch-use-submission' -Arguments (Switch-Args 'use' @('{case}\data\제출.pclm')) -Build {
        param($c) New-SwitchCase $c; $null = Invoke-Tool -Exe $cs -Arguments @('submit', "$c\data\work.pclm", "$c\data\제출.pclm")
    }
    Compare-HomeCase -Name 'switch-use-current' -Arguments (Switch-Args 'use' @('{case}\data\WORK.pclm')) -Build { param($c) New-SwitchCase $c }
    Compare-HomeCase -Name 'switch-use-missing' -Arguments (Switch-Args 'use' @('{case}\data\없음.pclm')) -Build { param($c) New-SwitchCase $c }
    Compare-HomeCase -Name 'switch-stale' -Arguments (Switch-Args 'use' @('{case}\data\셋째.pclm')) -Build {
        param($c)
        New-SwitchCase $c
        $null = Invoke-Tool -Exe $cs -Arguments @('create', "$c\data\다른 작업자료.pclm", 'work')
        $null = Invoke-Tool -Exe $cs -Arguments @('create', "$c\data\셋째.pclm", 'work')
        Bind $c "$c\data\다른 작업자료.pclm"
    }
    Compare-HomeCase -Name 'switch-from-submission' -RandomId -Arguments (Switch-Args 'from-snapshot' @('{case}\data\제출.pclm', '{case}\data\이어서.pclm')) -Build {
        param($c) New-SwitchCase $c; $null = Invoke-Tool -Exe $cs -Arguments @('submit', "$c\data\work.pclm", "$c\data\제출.pclm")
    }
    Compare-HomeCase -Name 'switch-from-v19' -RandomId -Arguments (Switch-Args 'from-snapshot' @('{case}\data\옛 자료.db', '{case}\data\올린 것.pclm')) -Build {
        param($c) New-SwitchCase $c; New-Synth "$c\data\옛 자료.db" 19
    }
    Compare-HomeCase -Name 'switch-from-v31' -RandomId -Arguments (Switch-Args 'from-snapshot' @('{case}\data\v31.pclm', '{case}\data\올린 것.pclm')) -Build {
        param($c) New-SwitchCase $c; New-Synth "$c\data\v31.pclm" 31
    }
    Compare-HomeCase -Name 'switch-from-not-pclm' -Arguments (Switch-Args 'from-snapshot' @('{case}\data\엉뚱.pclm', '{case}\data\이어서.pclm')) -Build {
        param($c) New-SwitchCase $c; Write-Text "$c\data\엉뚱.pclm" '남의 것'
    }
    Compare-HomeCase -Name 'switch-from-exists' -Arguments (Switch-Args 'from-snapshot' @('{case}\data\제출.pclm', '{case}\data\있는 것.pclm')) -Build {
        param($c)
        New-SwitchCase $c
        $null = Invoke-Tool -Exe $cs -Arguments @('submit', "$c\data\work.pclm", "$c\data\제출.pclm")
        Write-Text "$c\data\있는 것.pclm" '남의 것'
    }
    Compare-HomeCase -Name 'switch-new' -RandomId -Arguments (Switch-Args 'new' @('{case}\data\새 폴더\빈 것.pclm')) -Build { param($c) New-SwitchCase $c }
    Compare-HomeCase -Name 'switch-new-exists' -Arguments (Switch-Args 'new' @('{case}\data\있는 것.pclm')) -Build {
        param($c) New-SwitchCase $c; Write-Text "$c\data\있는 것.pclm" '남의 것'
    }
    Compare-HomeCase -Name 'switch-backup' -Arguments (Switch-Args 'backup' @('{case}\data\백업\백업.pclm')) -Build { param($c) New-SwitchCase $c }
    Compare-HomeCase -Name 'switch-backup-over' -Arguments (Switch-Args 'backup' @('{case}\data\백업.pclm')) -Build {
        param($c) New-SwitchCase $c; $null = Invoke-Tool -Exe $cs -Arguments @('create', "$c\data\백업.pclm", 'backup')
    }
    Compare-HomeCase -Name 'switch-backup-work' -Arguments (Switch-Args 'backup' @('{case}\data\다른 작업자료.pclm')) -Build {
        param($c) New-SwitchCase $c; $null = Invoke-Tool -Exe $cs -Arguments @('create', "$c\data\다른 작업자료.pclm", 'work')
    }
    Compare-HomeCase -Name 'switch-backup-unc' -Arguments (Switch-Args 'backup' @($unc)) -Build { param($c) New-SwitchCase $c }
    Compare-HomeCase -Name 'switch-check' -Arguments (Switch-Args 'check' @('{case}\data\.\새것.pclm')) -Build { param($c) New-SwitchCase $c }
    Compare-HomeCase -Name 'switch-check-exists' -Arguments (Switch-Args 'check' @('{case}\data\있는 것.pclm')) -Build {
        param($c) New-SwitchCase $c; Write-Text "$c\data\있는 것.pclm" '남의 것'
    }
}

# (d4) 명령줄(#8 ⑦) — C# pclm 과 Rust pclm 이 같은 인자에 같은 바이트를 내고 같은 종료 코드로 끝나며 같은 자리를 남기는가.
# 입력은 C# 계기가 짓는다 — seed 의 사본을 쪽지로 묶은 홈, 거기에 cli.sql(둘째 차수)과 cli.json(고르는 열 · 사람 값 · 덮개)을
# 더한 홈, 깨진 홈들, 열람할 남의 파일, 계획 엑셀(plan-fixtures), 제출본. 값은 모두 지어낸 것이다. --home 없이는 돌리지
# 않는다 — 업무 홈(%LOCALAPPDATA%\Pclm)을 연다.
if (Test-Path $seeded) {
    $cliPlans = Join-Path $work 'cli-plans'
    $null = Invoke-Tool -Exe $cs -Arguments @('plan-fixtures', $cliPlans)
    function New-CliHome([string]$c, [switch]$Rich) {
        Copy-Seed "$c\data\work.pclm"
        if ($Rich) {
            $null = Invoke-Tool -Exe $cs -Arguments @('sql', "$c\data\work.pclm", (Join-Path $scripts 'cli.sql'))
            $null = Invoke-Tool -Exe $cs -Arguments @('run', "$c\data\work.pclm", (Join-Path $scripts 'cli.json'))
            Remove-Item -LiteralPath "$c\data\work.pclm.out.json"
        }
        Bind $c "$c\data\work.pclm"
    }
    $h = @('--home', '{case}\home')
    function CliStep { , [string[]]$args }

    # 인자 — 거절(없어진 --db · 모르는 옵션 · 값 누락)과 `--` 뒤, 도움말(모르는 명령 · help · 대문자 명령).
    Compare-Cli -Name 'cli-args' -Build { param($c) New-CliHome $c } -Steps @(
        (CliStep status --db 시험자료 @h),
        (CliStep fix TESTK000100 계약건명 --긴급 @h),
        (CliStep merge --out),
        (CliStep merge --out --예 @h),
        (CliStep status @h --file),
        (CliStep fix TESTK000100 계약건명 @h '--' --긴급 --예),
        (CliStep help @h),
        (CliStep 없는명령 @h),
        (CliStep STATUS @h))
    # --file 은 status · 현황 · export 에만 — 거절 글에는 소문자로 바꾼 명령이 든다.
    Compare-Cli -Name 'cli-file-refused' -Build { param($c) New-CliHome $c } -Steps @(
        (CliStep PLAN --file '{case}\data\work.pclm' @h),
        (CliStep set --file '{case}\data\work.pclm' @h),
        (CliStep merge --file x @h),
        (CliStep Help --file x @h))
    # 열람 — 제출본 · 옛 판(사본을 올린다) · 상대 경로 · 받지 않는 파일. 사본은 홈의 view 에 섰다가 걷힌다.
    Compare-Cli -Name 'cli-view' -Build {
        param($c)
        New-CliHome $c
        $null = Invoke-Tool -Exe $cs -Arguments @('submit', "$c\data\work.pclm", "$c\data\제출.pclm")
        New-Synth "$c\data\옛것.pclm" 19
        Write-Text "$c\data\엉뚱.pclm" '남의 것'
        $null = Invoke-Tool -Exe $cs -Arguments @('create', "$c\data\취합.pclm", 'merged')
    } -Steps @(
        (CliStep status --file '{case}\data\제출.pclm' @h),
        (CliStep 현황 --file '..\data\제출.pclm' @h),
        (CliStep export '{case}\out\열람.xlsx' --file '{case}\data\제출.pclm' @h),
        (CliStep export --file '{case}\data\취합.pclm' @h),
        (CliStep status --file '{case}\data\옛것.pclm' @h),
        (CliStep status --file '{case}\data\엉뚱.pclm' @h),
        (CliStep status --file '{case}\data\없음.pclm' @h))
    # 시작하지 않는 홈 — 글에 경로가 들면 경로 줄을 더 쓰지 않는다(없는 파일), 들지 않으면 쓴다(깨진 쪽지).
    Compare-Cli -Name 'cli-home-missing' -Build { param($c) Write-Config "$c\home" "$c\지워진 자리\계약자료.pclm" 'ab' } -Steps (CliStep status @h)
    Compare-Cli -Name 'cli-home-corrupt' -Build {
        param($c) Write-Text "$c\home\config.json" "{`"version`":2,`"workfile`":$(Json "$c\data\work.pclm"),`"datasetId`":`"ab`"}"
    } -Steps (CliStep 현황 @h)
    Compare-Cli -Name 'cli-home-submission' -Build {
        param($c)
        Copy-Seed "$c\data\work.pclm"
        $null = Invoke-Tool -Exe $cs -Arguments @('submit', "$c\data\work.pclm", "$c\data\제출.pclm")
        Bind $c "$c\data\제출.pclm"
    } -Steps (CliStep set @h)
    # 시작의 알림 — 처음 실행, 옛 자료 옮겨 오기, 판올림 전 백업. 알림마다 빈 줄이 따른다.
    Compare-Cli -Name 'cli-first-run' -RandomId -Build { param($c) } -Steps @((CliStep status @h), (CliStep status @h))
    Compare-Cli -Name 'cli-legacy' -RandomId -Build { param($c) New-Synth "$c\home\pclm.db" 19 } -Steps (CliStep 현황 @h)
    Compare-Cli -Name 'cli-old-v31' -Build { param($c) New-Synth "$c\data\v31.pclm" 31; Bind $c "$c\data\v31.pclm" } -Steps (CliStep status @h)
    # 읽기와 내보내기 — 차수가 쌓인 건, 현황, 기본 이름(작업 폴더) · 없는 폴더 · 상대 경로.
    Compare-Cli -Name 'cli-read' -Build { param($c) New-CliHome $c -Rich } -Steps @(
        (CliStep status @h),
        (CliStep 현황 @h),
        (CliStep export @h),
        (CliStep export '{case}\out\새 폴더\관리대장.xlsx' @h),
        (CliStep export 상대.xlsx @h))
    # 계획 — 고른 것이 없음 · 없는 파일 · 필수 머리글 없음 · 소수 지시수량 · 정상(건너뛴 줄) · 다시 읽기 · 이상한 칸.
    $plans = $cliPlans
    Compare-Cli -Name 'cli-plan' -Build { param($c) New-CliHome $c } -Steps @(
        (CliStep plan @h),
        (CliStep plan '{case}\없는.xlsx' @h),
        (CliStep plan "$plans\머리글틀림.xlsx" @h),
        (CliStep plan "$plans\소수.xlsx" @h),
        (CliStep plan "$plans\표본.xlsx" @h),
        (CliStep plan @h),
        (CliStep plan "$plans\이상한칸.xlsx" @h),
        (CliStep 현황 @h))
    # 사람 값 — 목록 · 쓰는 법 · 없는 키 · 모르는 열 · 여러 낱말 · `--` 뒤 · 후보에 없는 값 · 후보 · 지우기, 공고와 접수.
    Compare-Cli -Name 'cli-set' -Build { param($c) New-CliHome $c -Rich } -Steps @(
        (CliStep set @h),
        (CliStep set TESTK000100 @h),
        (CliStep set 없는키 메모 @h),
        (CliStep set TESTK000100 없는열 값 @h),
        (CliStep set TESTK000100 메모 지어낸 긴 메모 @h),
        (CliStep set TESTK000100 진행상태 @h '--' --긴급),
        (CliStep set TESTK000100 단계표시 없는후보 @h),
        (CliStep set TESTK000200 단계표시 준비 @h),
        (CliStep set TESTK000100 메모 @h),
        (CliStep set TESTN0001-000 검토여부 확인 @h),
        (CliStep set TESTR0001-00 메모 @h),
        (CliStep set @h))
    # 고치기 — 쓰는 법 · 없는 키 · 고칠 수 없는 열(키 열 · 사람 열 · 없는 열) · 고치기 · 빈 값 · 되돌리기 · 공고종류(연결 다시).
    Compare-Cli -Name 'cli-fix' -Build { param($c) New-CliHome $c -Rich } -Steps @(
        (CliStep fix TESTK000100 @h),
        (CliStep fix 없는키 계약건명 @h),
        (CliStep fix TESTK000100 계약번호 x @h),
        (CliStep fix TESTK000100 메모 x @h),
        (CliStep fix TESTK000100 없는열 x @h),
        (CliStep fix TESTK000200 계약건명 고친 의자 계약 @h),
        (CliStep fix TESTK000200 납품장소 '' @h),
        (CliStep fix TESTK000100 계약건명 @h),
        (CliStep fix TESTK000200 계약건명 @h),
        (CliStep fix TESTN0001-000 공고종류 재공고 @h),
        (CliStep fix TESTN0001-000 공고종류 @h),
        (CliStep status @h))
    # 지우기 — 쓰는 법 · 없는 키 · 세어 보이기(차수 하나 · 전 차수) · 지우기(차수 하나 · 사람 값과 덮개가 든 계열) · 연결된 공고.
    Compare-Cli -Name 'cli-delete' -Build { param($c) New-CliHome $c -Rich } -Steps @(
        (CliStep delete @h),
        (CliStep delete 없는키 @h),
        (CliStep delete TESTK000201 @h),
        (CliStep delete TESTK000200 --계열 @h),
        (CliStep delete TESTK000201 --예 @h),
        (CliStep delete TESTK000100 --계열 --예 @h),
        (CliStep delete TESTN0001-000 --계열 @h),
        (CliStep delete TESTR0001-00 --예 --계열 @h),
        (CliStep status @h))
    # 제출본 — 이름 없음(작업 폴더) · 이름을 정하며 폴더로 · 정해 둔 이름으로 파일 자리 · 공백 이름(지운다) · 없는 폴더 안.
    Compare-Cli -Name 'cli-submit' -Build { param($c) New-CliHome $c -Rich; New-Item -ItemType Directory -Force -Path "$c\out" | Out-Null } -Steps @(
        (CliStep submit @h),
        (CliStep submit --이름 홍길동 '{case}\out' @h),
        (CliStep submit '{case}\out\지정.pclm' @h),
        (CliStep submit --이름 '  ' '{case}\out\새 폴더\빈이름.pclm' @h),
        (CliStep submit '{case}\out\' @h))
    # 취합 — 찾은 것 없음(인자 없음 · 없는 자리 · .pclm 없는 폴더) · 폴더 · 겹친 파일과 없는 파일 · 받을 것 없음 · 작업자료 덮기 ·
    # 기본 이름(작업 폴더) · --out 폴더.
    $mergeSources = Join-Path $work 'merge-in'
    Compare-Cli -Name 'cli-merge' -Build {
        param($c)
        New-CliHome $c
        New-Item -ItemType Directory -Force -Path "$c\in", "$c\빈 폴더", "$c\out" | Out-Null
        Copy-Item (Join-Path $mergeSources '제출_홍길동.pclm'), (Join-Path $mergeSources '제출_김철수.pclm') -Destination "$c\in"
        Write-Text "$c\빈 폴더\메모.txt" '제출본이 아님'
        Write-Text "$c\엉뚱.pclm" '남의 것'
    } -Steps @(
        (CliStep merge @h),
        (CliStep merge '{case}\없음' @h),
        (CliStep merge '{case}\빈 폴더' '{case}\없음.pclm' @h),
        (CliStep merge '{case}\in' --out '{case}\out' @h),
        (CliStep merge '{case}\in\제출_홍길동.pclm' '{case}\없음.pclm' '{case}\IN\제출_홍길동.pclm' --out '{case}\out\합.pclm' @h),
        (CliStep merge '{case}\엉뚱.pclm' --out '{case}\out\못함.pclm' @h),
        (CliStep merge '{case}\in' --out '{case}\data\work.pclm' @h),
        (CliStep merge '{case}\in' '{case}\빈 폴더' @h))
}

# (d5) 다리의 홈 부름(#8 ⑧a) — 편집 · 맞춤 뷰 · 보기 구성(seed.json 을 다리 부름으로 옮긴 bridge-edit), 확장 · 지금 보는 화면 ·
# 가져오기 · 자동 수집(bridge-erp — 계기가 확장 호스트인 것처럼 탭 보고를 적는다), 제출 · 취합 · 백업, 작업자료 바꾸기(다시
# 띄우기의 깃발), 창(알림 영역 · 닫기 · 열어 보기의 거절)과 열람 창의 문(모든 요청을 값 없이). 탐색기 · 새 창 · 등록부에 닿는
# 부름은 대조하지 않는다(README 「다리」). 입력은 C# 계기가 짓고, 값은 모두 지어낸 것이다.
if (Test-Path $seeded) {
    Compare-BridgeHome -Name 'bridge-edit' -Script (Join-Path $scripts 'bridge-edit.json')
    Compare-BridgeHome -Name 'bridge-erp' -Script (Join-Path $scripts 'bridge-erp.json') -Build {
        param($c)
        # 내장 확장 준비의 입력 — 이름만 exe 인 지어낸 파일.
        Write-Text "$c\files\app\계약목록.exe" 'MZ 지어낸 실행 파일'
        Write-Text "$c\files\app\글.txt" '실행 파일이 아니다'
    }
    Compare-BridgeHome -Name 'bridge-files' -Script (Join-Path $scripts 'bridge-files.json') -Build {
        param($c) New-Item -ItemType Directory -Force -Path "$c\files\빈 폴더" | Out-Null
    }
    Compare-BridgeHome -Name 'bridge-workfile' -Script (Join-Path $scripts 'bridge-workfile.json') -Build {
        param($c)
        Write-Text "$c\files\남\글.pclm" '작업자료가 아니다'
        $null = Invoke-Tool -Exe $cs -Arguments @('create', "$c\files\남\작업.pclm", 'work')
        $null = Invoke-Tool -Exe $cs -Arguments @('submit', "$c\data\계약자료.pclm", "$c\files\남\제출본.pclm")
    }
    Compare-BridgeHome -Name 'bridge-window' -Script (Join-Path $scripts 'bridge-window.json')
    Compare-BridgeHome -Name 'bridge-gate-all' -Script (Join-Path $scripts 'bridge-gate-all.json') -View
}

# (e) 교차 읽기 — Rust 가 짓고·올리고·고친 파일. 사람이 SQL 열에 ceil·floor 를 써 저장하면 그 뷰는 수학 함수 없는
# 읽기(문서나르미의 rusqlite bundled)에서 「no such function」 으로 서지 않는다 — C# 이 쓴 파일도 같은 기존 위험이라
# (#9 보고) 대본은 그런 열을 검사만 하고 저장하지 않는다.
Compare-Read -Name 'read-make-work' -File (Join-Path $work 'make-work.rs.pclm')
Compare-Read -Name 'read-run-seed' -File (Join-Path $work 'run-seed.rs.pclm')
Compare-Read -Name 'read-migrate-v21' -File (Join-Path $work 'migrate-v21.rs.pclm')
Compare-Read -Name 'read-run-real-on-seed' -File (Join-Path $work 'run-real-on-seed.rs.pclm')
if ($Real) {
    for ($i = 1; $i -le $index; $i++) {
        Compare-Read -Name "read-migrate-real-$i" -File (Join-Path $work "migrate-real-$i.rs.pclm")
    }
}

# ── 끝 ─────────────────────────────────────────────

$failed = @($script:results | Where-Object { -not $_.Same })
Write-Host ''
Write-Host "사례 $($script:results.Count) · same $($script:results.Count - $failed.Count) · DIFF/실패 $($failed.Count)"
if ($mode -eq 'replay') {
    # 사례 목록이 박제와 어긋나면(사례를 더하거나 뺐다) 박제를 다 되살리지 못한다 — 그것도 실패다.
    if ($script:cursor -ne $script:calls.Count -or $script:results.Count -ne $script:recordedCases) {
        Write-Host "박제와 사례 목록이 어긋납니다 — C# 부름 $($script:cursor)/$($script:calls.Count), 사례 $($script:results.Count)/$($script:recordedCases)"
        exit 1
    }
}
if ($mode -eq 'record') {
    if ($failed.Count -gt 0 -or $script:tapeProblems.Count -gt 0) {
        foreach ($problem in $script:tapeProblems) { Write-Host "  박제: $problem" }
        Write-Host '모두 같지 않아 박제를 뜨지 않았습니다.'
        exit 1
    }
    Compress-Tape
    Save-Tape $script:results.Count
    if (Test-Path $golden) { Remove-Item -LiteralPath $golden -Recurse -Force }
    Move-Item -LiteralPath $script:staging -Destination $golden
    $blobs = @(Get-ChildItem -LiteralPath (Join-Path $golden 'blobs') -File)
    $size = ($blobs | Measure-Object -Property Length -Sum).Sum + (Get-Item -LiteralPath (Join-Path $golden 'tape.tsv.gz')).Length
    Write-Host ("박제: C# 부름 {0} · 파일 {1} · {2:N1} MB → tests/parity-golden" -f $script:calls.Count, $blobs.Count, ($size / 1MB))
}
if ($failed.Count -gt 0) { exit 1 }
exit 0

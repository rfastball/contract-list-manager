using System.Globalization;
using System.Reflection;
using System.Text.RegularExpressions;

namespace Pclm.Core.Erp;

/// <summary>
/// 실행 파일의 정체 — 자리·크기·마지막 쓴 때(ADR-035). 상시 연결 호스트는 브라우저가 켜져 있는 동안 exe 를 붙들어,
/// 그 자리에 새 판을 덮어쓰지 못한다. 이름 바꾸기는 된다. 그래서 사람(또는 <c>publish.ps1</c>·<c>run.ps1</c>)이 옛 exe 를
/// 비키고 새 것을 놓으면, 호스트는 제 자리가 더는 자기가 아님을 보고 떠난다 — 확장이 다시 붙으며 그 자리의 새 exe 가 뜬다.
/// </summary>
public sealed record ExecutableIdentity(string Path, long Length, DateTime LastWriteUtc)
{
    /// <summary>그 자리의 파일. 없으면(또는 자리가 비었으면) null.</summary>
    public static ExecutableIdentity? Of(string? path)
    {
        if (string.IsNullOrEmpty(path)) return null;
        var info = new FileInfo(path);
        return info.Exists ? new(info.FullName, info.Length, info.LastWriteTimeUtc) : null;
    }

    /// <summary>
    /// 이 프로세스를 띄운 파일들 — exe, 그리고 디스크에 따로 있으면 앱 어셈블리(개발 빌드의 <c>계약목록.dll</c>).
    /// 한 파일 배포물에서는 어셈블리가 exe 안에 있어 exe 하나다.
    /// </summary>
    public static List<ExecutableIdentity> OfThisProcess()
    {
        var entry = Assembly.GetEntryAssembly()?.GetName().Name;
        var paths = new[] { Environment.ProcessPath, entry is null ? null : System.IO.Path.Combine(AppContext.BaseDirectory, entry + ".dll") };
        return paths.Select(Of).OfType<ExecutableIdentity>()
            .DistinctBy(i => i.Path, StringComparer.OrdinalIgnoreCase).ToList();
    }

    /// <summary>그 자리에 아직 같은 파일이 있는가. 비켜졌거나(없다) 다른 파일이 섰으면(크기·쓴 때가 다르다) 거짓.</summary>
    public bool StillHere() => Of(Path) == this;

    /// <summary>
    /// 브라우저의 호스트 등록 문서(<c>{"path": …}</c>)가 이 실행 파일을 가리키는가. 새 판을 <b>다른 자리</b>에 놓고 켜면 창이
    /// 등록을 그리로 고쳐 적는다 — 제 자리는 그대로라 <see cref="StillHere"/> 로는 모르고, 옛 호스트가 브라우저를 닫을 때까지
    /// 옛 판으로 남는다. 읽지 못하는 문서는 가리킨다고 친다: 확실하지 않을 때 연결을 끊지 않는다.
    /// </summary>
    public static bool ManifestPointsAt(string manifestJson, string processPath)
    {
        try
        {
            using var document = System.Text.Json.JsonDocument.Parse(manifestJson);
            if (!document.RootElement.TryGetProperty("path", out var path) || path.ValueKind != System.Text.Json.JsonValueKind.String) return true;
            return string.Equals(System.IO.Path.GetFullPath(path.GetString()!), System.IO.Path.GetFullPath(processPath), StringComparison.OrdinalIgnoreCase);
        }
        catch (Exception e) when (e is System.Text.Json.JsonException or ArgumentException or NotSupportedException or PathTooLongException) { return true; }
    }
}

/// <summary>
/// 비켜 둔 옛 실행 파일. 이름은 <c>&lt;이름&gt;.old.exe</c> 또는 <c>&lt;이름&gt;.old-&lt;숫자&gt;.exe</c> 하나로 정한다 —
/// 스크립트는 <c>계약목록.old-yyyyMMddHHmmss.exe</c> 로 비키고, 사람은 <c>계약목록.old.exe</c> 처럼 바꾸면 된다.
/// 창이 켜질 때 exe 옆과 홈에서 이 이름인 것을 지운다. 아직 쓰는 중이면(브라우저의 호스트가 쥐고 있다) 다음 번으로 미룬다.
/// </summary>
public static class OldExecutables
{
    /// <summary>배포물의 이름. 사람이 exe 이름을 바꿔 써도 이 이름의 옛 것은 치운다.</summary>
    public const string ShippedStem = "계약목록";

    /// <summary>스크립트가 비킬 때 붙이는 이름.</summary>
    public static string NameFor(string path, DateTime now) => System.IO.Path.Combine(
        System.IO.Path.GetDirectoryName(System.IO.Path.GetFullPath(path))!,
        System.IO.Path.GetFileNameWithoutExtension(path) + ".old-" + now.ToString("yyyyMMddHHmmss", CultureInfo.InvariantCulture) +
        System.IO.Path.GetExtension(path));

    /// <summary>그 이름(<paramref name="stem"/>)의 옛 exe 인가. 다른 프로그램의 <c>*.old.exe</c> 는 건드리지 않는다.</summary>
    public static bool IsOld(string fileName, string stem) =>
        Regex.IsMatch(fileName, @"\A" + Regex.Escape(stem) + @"\.old(-\d+)?\.exe\z", RegexOptions.IgnoreCase);

    /// <summary>폴더에서 옛 exe 를 지운다. 지운 수. 지우지 못한 것(쓰는 중·권한)은 조용히 남긴다.</summary>
    public static int Sweep(string directory, IEnumerable<string> stems)
    {
        var names = stems.Where(s => !string.IsNullOrEmpty(s)).Distinct(StringComparer.OrdinalIgnoreCase).ToList();
        string[] files;
        try { files = Directory.GetFiles(directory, "*.exe"); }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException or ArgumentException) { return 0; }
        var removed = 0;
        foreach (var file in files)
        {
            if (!names.Any(stem => IsOld(System.IO.Path.GetFileName(file), stem))) continue;
            try { File.Delete(file); removed++; }
            catch (Exception e) when (e is IOException or UnauthorizedAccessException) { }
        }
        return removed;
    }

    /// <summary>창이 켜질 때: 이 exe 옆과 홈에서 이 exe 이름·배포물 이름의 옛 것을 치운다.</summary>
    public static void SweepAround(string? processPath, string homeDirectory)
    {
        var stems = new[] { ShippedStem, System.IO.Path.GetFileNameWithoutExtension(processPath ?? "") };
        if (System.IO.Path.GetDirectoryName(processPath ?? "") is { Length: > 0 } beside) Sweep(beside, stems);
        Sweep(homeDirectory, stems);
    }
}

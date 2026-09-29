using System.Text.Json;
using System.Text.Json.Serialization;

namespace Pclm.Core.Storage;

/// <summary>
/// 자료가 쌓이는 자리를 적어 두는 쪽지. <b>DB 안이 아니라 밖에 산다.</b>
///
/// <para>나머지 설정(제출자 이름·계획 엑셀 자리)은 DB 안에 있다(<see cref="Settings"/>) — 자료와 함께
/// 옮겨 다니는 것이 맞기 때문이다. 그런데 <b>어느 DB를 열지</b>만은 거기 둘 수 없다:
/// 그 값을 읽으려면 DB를 먼저 열어야 하는데, 어느 것을 열지가 바로 그 값이다.
/// 그래서 이 하나만 잘 알려진 고정 자리(<c>%APPDATA%\Pclm\config.json</c>)에 둔다.</para>
///
/// <para><b>자리는 여전히 하나다</b>(ADR-017). 여러 DB 를 오가는 것이 아니라, 하나뿐인 그 자리가
/// 어디인지를 가리킬 뿐이다. 밖에서 읽는 쪽은 이 쪽지를 보고 찾아온다
/// (<c>docs/dataset-contract.md</c> §1).</para>
/// </summary>
public sealed class DataLocationConfig
{
    /// <summary><c>pclm.db</c> 를 담은 폴더. 비면 기본 자리를 쓴다.</summary>
    [JsonPropertyName("dataDir")]
    public string? DataDir { get; set; }

    /// <summary>
    /// 옮기기로 정했지만 <b>아직 자료가 실제로 있는</b> 자리.
    ///
    /// <para>고르는 순간에 복사하지 않는 까닭이다. 그렇게 하면 고른 뒤 재시작 전까지의 편집이
    /// 옛 자리에만 쌓이고, 다음 실행은 고른 시점의 낡은 사진을 열어 그 사이의 편집이
    /// <b>오류 없이</b> 사라진다. 복사는 다음 실행 맨 앞, 아무 연결도 열기 전에 한다.</para>
    /// </summary>
    [JsonPropertyName("pendingMoveFrom")]
    public string? PendingMoveFrom { get; set; }
}

/// <summary>어디를 열었고, 오는 길에 무슨 일이 있었는지.</summary>
/// <param name="DbPath">실제로 열 <c>pclm.db</c> 경로.</param>
/// <param name="IsDefault">기본 자리인가.</param>
/// <param name="Note">사람에게 알릴 말. 없으면 <c>null</c>.</param>
public sealed record ResolvedLocation(string DbPath, bool IsDefault, string? Note);

/// <summary>쪽지를 읽고 쓰고, 밀린 이사를 치른다.</summary>
public static class DataLocation
{
    /// <summary>사용자가 고른 자리 아래에 우리가 만드는 폴더 이름.</summary>
    /// <remarks>
    /// 사람은 <b>위치만 가리키고</b> 빈 폴더를 손수 만들지 않는다. <c>D:\</c> 를 고르면
    /// <c>D:\계약목록_자료\</c> 가 새로 선다 — 이미 있던 파일과 섞일 수 없다.
    /// </remarks>
    public const string FolderName = "계약목록_자료";

    private const string FileName = "pclm.db";

    private static readonly JsonSerializerOptions Json = new() { WriteIndented = true };

    /// <summary>쪽지가 있는 자리. <c>%APPDATA%\Pclm\config.json</c>.</summary>
    public static string ConfigPath { get; } = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
        "Pclm", "config.json");

    /// <summary>쪽지가 없거나 비었을 때 쓰는 자리를 담은 폴더.</summary>
    public static string DefaultDir { get; } = Path.GetDirectoryName(Database.DefaultPath)!;

    public static DataLocationConfig Load(string? configPath = null)
    {
        var path = configPath ?? ConfigPath;
        try
        {
            return File.Exists(path)
                ? JsonSerializer.Deserialize<DataLocationConfig>(File.ReadAllText(path)) ?? new()
                : new();
        }
        catch
        {
            // 쪽지가 깨졌다고 앱이 못 뜨면 안 된다. 기본 자리로 뜨는 편이 낫다.
            return new();
        }
    }

    public static void Save(DataLocationConfig config, string? configPath = null)
    {
        var path = configPath ?? ConfigPath;
        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        File.WriteAllText(path, JsonSerializer.Serialize(config, Json));
    }

    /// <summary>그 폴더 안에서 DB 가 놓일 자리.</summary>
    public static string DbIn(string directory) => Path.Combine(directory, FileName);

    /// <summary>
    /// 지금 열 자리를 정한다. <b>어떤 연결보다도 먼저 부른다</b> — 밀린 이사를 여기서 치르는데,
    /// 그 전에 누가 파일을 열어 두면 옮기는 중에 갈라진다.
    ///
    /// <para>이사에 실패하면 <b>옛 자리로 되돌린다.</b> 초기화되지 않은 새 자리에서 빈 채로
    /// 뜨느니, 진짜 자료 위에서 계속 도는 쪽이 언제나 낫다.</para>
    /// </summary>
    public static ResolvedLocation Resolve(string? configPath = null)
    {
        var config = Load(configPath);
        var target = string.IsNullOrWhiteSpace(config.DataDir) ? DefaultDir : config.DataDir!;
        string? note = null;

        if (!string.IsNullOrWhiteSpace(config.PendingMoveFrom))
        {
            var from = config.PendingMoveFrom!;
            if (!PathsEqual(from, target))
            {
                try
                {
                    // 한 벌 뜨는 일은 Database 가 쥔다(VACUUM INTO). 여기서는 부르기만 한다 —
                    // 같은 일이 두 벌이 되면 한쪽이 조용히 늙는다.
                    // 원본은 지우지 않는다. 옮긴 뒤에도 옛 자리에 그대로 남아, 이사가 잘못됐을 때
                    // 돌아갈 자리가 된다.
                    new Database(DbIn(from)).Snapshot(DbIn(target));
                    note = $"자료를 옮겼습니다.\n{from} → {target}";
                }
                catch (Exception e)
                {
                    target = from;
                    config.DataDir = from;
                    note = $"새 자리로 옮기지 못해({e.Message}) 기존 자리를 계속 씁니다.\n{from}";
                }
            }

            config.PendingMoveFrom = null;
            try { Save(config, configPath); }
            catch { /* 쪽지를 못 고쳐도 이번 실행은 제대로 돈다. 다음 실행에서 다시 시도한다. */ }
        }

        return new ResolvedLocation(DbIn(target), PathsEqual(target, DefaultDir), note);
    }

    /// <summary>
    /// 옮길 자리로 삼아도 되는지 본다. 되면 <c>null</c>, 안 되면 까닭.
    /// <b>아무것도 적기 전에</b> 부른다 — 여기서 막으면 되돌릴 것이 없다.
    /// </summary>
    public static string? WhyNotMoveTo(string directory)
    {
        if (File.Exists(DbIn(directory)))
            return "고른 자리에 이미 계약 목록 자료가 있습니다.\n" +
                   "그것을 덮어쓰지 않으려고 이동을 취소했습니다 — 비어 있는 다른 자리를 고르세요.";

        try
        {
            Directory.CreateDirectory(directory);
            var probe = Path.Combine(directory, ".pclm-write-test");
            File.WriteAllText(probe, "");
            File.Delete(probe);
        }
        catch (Exception e)
        {
            return $"고른 자리에 쓸 수 없습니다: {e.Message}";
        }

        return null;
    }

    /// <summary>
    /// 다음 실행에 옮기도록 적어 둔다. 지금 옮기지 않는 까닭은
    /// <see cref="DataLocationConfig.PendingMoveFrom"/> 에 있다.
    /// </summary>
    public static void StageMove(string currentDbPath, string newDirectory, string? configPath = null)
    {
        Save(new DataLocationConfig
        {
            DataDir = newDirectory,
            PendingMoveFrom = Path.GetDirectoryName(Path.GetFullPath(currentDbPath))!,
        }, configPath);
    }

    /// <summary>
    /// 사람이 가리킨 <b>부모 폴더</b> 아래 <see cref="FolderName"/> 으로 옮기도록 예약한다.
    /// 앱(폴더 선택)과 확장 호스트(입력한 경로)가 함께 부르는 한 길이다 — 검증이 두 벌이 되면
    /// 한쪽 입구로만 남의 자료를 덮는 길이 열린다. 옮긴 뒤의 자리를 돌려준다.
    ///
    /// <para><paramref name="currentDbPath"/> 는 <b>지금 실제로 자료가 있는</b> DB 여야 한다.
    /// 쪽지가 다른 자리를 가리키면(<c>--db</c> 로 연 자료, 어긋난 연결) 막는다 — 그 자료를 옮기라고
    /// 적으면 다음 실행이 엉뚱한 자료를 새 자리의 진짜로 세운다.</para>
    /// </summary>
    /// <exception cref="InvalidOperationException">받아들일 수 없는 자리. 메시지가 그 까닭이다.</exception>
    public static string StageMoveUnder(string currentDbPath, string parentFolder, string? configPath = null)
    {
        var parent = parentFolder?.Trim() ?? "";
        string target;
        try
        {
            if (parent.Length == 0 || parent.IndexOfAny(Path.GetInvalidPathChars()) >= 0 ||
                !Path.IsPathFullyQualified(parent))
                throw new ArgumentException();
            target = Path.GetFullPath(Path.Combine(parent, FolderName));
        }
        catch (Exception e) when (e is ArgumentException or NotSupportedException or PathTooLongException)
        {
            throw new InvalidOperationException(
                "옮길 폴더는 드라이브부터 적은 전체 경로여야 합니다. 예: D:\\업무");
        }

        // 지금 자료가 있는 자리 — 이사가 밀려 있으면 아직 옛 자리다.
        var config = Load(configPath);
        var current = !string.IsNullOrWhiteSpace(config.PendingMoveFrom) ? config.PendingMoveFrom!
            : !string.IsNullOrWhiteSpace(config.DataDir) ? config.DataDir! : DefaultDir;
        if (!PathsEqual(Path.GetDirectoryName(Path.GetFullPath(currentDbPath))!, current))
            throw new InvalidOperationException(
                "지금 연결된 자료가 설정된 저장 자리와 다릅니다. 메인 프로그램을 다시 실행해 연결을 갱신하세요.");

        // 아무것도 적기 전에 막는다. 적어 둔 뒤에 알면 다음 실행이 남의 자료를 덮는다.
        if (WhyNotMoveTo(target) is { } why)
            throw new InvalidOperationException(why);

        try { StageMove(currentDbPath, target, configPath); }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException)
        {
            throw new InvalidOperationException($"자료 위치 설정을 적지 못했습니다: {e.Message}");
        }
        return target;
    }

    private static bool PathsEqual(string a, string b) =>
        string.Equals(
            Path.TrimEndingDirectorySeparator(Path.GetFullPath(a)),
            Path.TrimEndingDirectorySeparator(Path.GetFullPath(b)),
            StringComparison.OrdinalIgnoreCase);
}

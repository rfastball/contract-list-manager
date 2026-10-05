using System.Globalization;
using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.RegularExpressions;

namespace Pclm.Core.Erp;

/// <summary>
/// 확장이 어디까지 깔렸는지. 앱은 브라우저를 들여다볼 수 없으므로 확장이 호스트에 인사할 때 남긴 흔적으로 안다.
/// </summary>
public static class ExtensionStatus
{
    public sealed record Contact(string Browser, string Version, string At);
    /// <summary>지금 포트를 쥐고 있는 확장(<see cref="ExtensionPresence"/>). 시각은 포트가 열린 때다.</summary>
    public sealed record LiveContact(string Browser, string Version, string ConnectedAt);
    /// <param name="Folder">확장을 풀어 둘 자리. 아직 준비하지 않았어도 그 자리를 낸다 — 브라우저의 「압축해제된 확장
    /// 프로그램을 로드합니다」 에서 사람이 고르는 폴더다.</param>
    /// <param name="Live">지금 붙어 있는 브라우저. 호스트 프로세스가 그때 시작한 그대로 살아 있는 것만 온다.</param>
    /// <param name="Log">오늘의 연결 기록(<see cref="ExtensionLog"/>) — 붙음·끊김·판 바뀜만, 일어난 차례대로. 오류는 따로 낸다.</param>
    /// <param name="Errors">오늘 확장의 수집 흐름이 알린 오류, 일어난 차례대로.</param>
    /// <param name="PastError">오늘 전의 마지막 오류. 기록이 남은 이레 안에 없으면 null.</param>
    public sealed record State(bool Prepared, string EmbeddedVersion, string? DiskVersion, List<Contact> Contacts, string Folder,
        List<LiveContact> Live, List<ExtensionLog.Entry> Log, List<ExtensionLog.Problem> Errors, ExtensionLog.Problem? PastError);

    private static readonly string[] Browsers = ["Edge", "Chrome", "기타"];
    private static readonly Regex VersionPattern = new(@"\A\d+(\.\d+){0,3}\z");

    /// <summary>확장이 보낸 브라우저·판이 디스크에 적어도 되는 모양인가. 상시 연결·연결 기록도 같은 판정을 쓴다.</summary>
    internal static bool Accepts(string browser, string version) => Browsers.Contains(browser) && VersionPattern.IsMatch(version);

    private static string ContactPath(string directory) => Path.Combine(directory, "extension-contact.json");

    /// <summary>사용자 폴더에 풀어 둔 확장의 판. 없거나 못 읽으면 null.</summary>
    public static string? DiskVersion(string directory)
    {
        try
        {
            using var manifest = JsonDocument.Parse(File.ReadAllBytes(Path.Combine(directory, "extension", "manifest.json")));
            return manifest.RootElement.TryGetProperty("version", out var version) && version.ValueKind == JsonValueKind.String
                ? version.GetString() : null;
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException or JsonException) { return null; }
    }

    /// <summary>
    /// 브라우저별 마지막 연결을 적는다. 값은 확장이 보낸 것이라 모양이 맞는 것만 받는다 —
    /// 확장이 디스크에 임의 문자열을 쓰지 못하게.
    /// </summary>
    public static void RecordContact(string directory, string version, string browser)
    {
        if (!Accepts(browser, version)) return;
        var path = ContactPath(directory);
        JsonObject root;
        try { root = JsonNode.Parse(File.ReadAllText(path)) as JsonObject ?? new(); }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException or JsonException) { root = new(); }
        root[browser] = new JsonObject { ["version"] = version, ["at"] = DateTime.Now.ToString("yyyy-MM-ddTHH:mm:ss", CultureInfo.InvariantCulture) };
        Directory.CreateDirectory(directory);
        var temporary = path + "." + Guid.NewGuid().ToString("N") + ".tmp";
        try
        {
            File.WriteAllText(temporary, root.ToJsonString());
            File.Move(temporary, path, overwrite: true);
        }
        finally { if (File.Exists(temporary)) File.Delete(temporary); }
    }

    /// <param name="now">「오늘」 의 기준. 연결 기록은 그날 로컬 자정부터 낸다.</param>
    public static State Read(string directory, string embeddedVersion, DateTime? now = null)
    {
        var contacts = new List<Contact>();
        try
        {
            if (JsonNode.Parse(File.ReadAllText(ContactPath(directory))) is JsonObject root)
                foreach (var browser in Browsers)
                    if (root[browser] is JsonObject entry && entry["version"]?.GetValueKind() == JsonValueKind.String &&
                        entry["at"]?.GetValueKind() == JsonValueKind.String)
                        contacts.Add(new(browser, entry["version"]!.GetValue<string>(), entry["at"]!.GetValue<string>()));
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException or JsonException) { }
        var folder = Path.Combine(Path.GetFullPath(directory), "extension");
        var at = now ?? DateTime.Now;
        var log = ExtensionLog.Read(directory, at.Date)
            .Where(e => e.Event is ExtensionLog.Connected or ExtensionLog.Disconnected or ExtensionLog.Updated).ToList();
        var (errors, past) = ExtensionLog.Problems(directory, at);
        return new(Directory.Exists(folder), embeddedVersion, DiskVersion(directory), contacts, folder,
            ExtensionPresence.ReadLive(directory), log, errors, past);
    }
}

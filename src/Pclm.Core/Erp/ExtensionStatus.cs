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
    public sealed record State(bool Prepared, string EmbeddedVersion, string? DiskVersion, List<Contact> Contacts);

    private static readonly string[] Browsers = ["Edge", "Chrome", "기타"];
    private static readonly Regex VersionPattern = new(@"\A\d+(\.\d+){0,3}\z");

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
        if (!Browsers.Contains(browser) || !VersionPattern.IsMatch(version)) return;
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

    public static State Read(string directory, string embeddedVersion)
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
        return new(Directory.Exists(Path.Combine(directory, "extension")), embeddedVersion, DiskVersion(directory), contacts);
    }
}

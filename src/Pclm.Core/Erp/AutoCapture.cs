using System.Text.Json;
using System.Text.Json.Serialization;
using Dapper;
using Pclm.Core.Storage;

namespace Pclm.Core.Erp;

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Skip)]
public sealed record AutoCapturePreferences(bool Enabled = false, string Name = "")
{
    public static string PathFor(Home home) => Path.Combine(home.Directory, "auto-capture.json");

    public static AutoCapturePreferences Read(Home home)
    {
        try
        {
            var value = JsonSerializer.Deserialize<AutoCapturePreferences>(File.ReadAllText(PathFor(home)), Mapping.Json);
            return value is null || value.Name is null || value.Name.Length > 200 ? new() : value;
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException or JsonException) { return new(); }
    }

    public void Write(Home home)
    {
        if (Name is null || Name.Length > 200) throw new InvalidOperationException("자동 수집 설정을 확인하세요.");
        Directory.CreateDirectory(home.Directory);
        var path = PathFor(home);
        var temporary = path + "." + Guid.NewGuid().ToString("N") + ".tmp";
        try
        {
            File.WriteAllText(temporary, JsonSerializer.Serialize(this, Mapping.Json));
            File.Move(temporary, path, overwrite: true);
        }
        finally { if (File.Exists(temporary)) File.Delete(temporary); }
    }
}

public sealed record AutoCaptureResult(string State, string Message, string? Entity = null);

/// <summary>현재 화면의 조건을 확인하고 기존 검토·저장 경계를 쓴다. 탭 조작이나 DB 값으로 조건을 보충하지 않는다.</summary>
public sealed class AutoCapture(Database database, Home home)
{
    private readonly Dictionary<string, AutoCaptureResult> _attempted = new(StringComparer.Ordinal);
    private readonly Dictionary<string, (CaptureInput Input, string BaseToken, string Id, Dictionary<string, string> Choices)> _pending = new(StringComparer.Ordinal);
    private string? _currentKey;

    public AutoCaptureResult TrySave(JsonElement tab, AutoCapturePreferences preferences, DateTimeOffset now, Func<bool>? maySave = null)
    {
        AutoCaptureResult Hold(string message) => new("hold", message);
        if (maySave?.Invoke() == false) return Hold("앱을 종료하고 있어 저장하지 않습니다.");
        if (!preferences.Enabled) return Hold("자동 수집이 꺼져 있습니다.");
        if (tab.ValueKind != JsonValueKind.Object || Text(tab, "state") != "supported" || !tab.TryGetProperty("snapshot", out var snapshot) || snapshot.ValueKind != JsonValueKind.Object)
            return Hold("현재 화면을 다시 읽는 중입니다.");
        if (!tab.TryGetProperty("readiness", out var readiness) || readiness.ValueKind != JsonValueKind.Object ||
            !readiness.TryGetProperty("stable", out var stable) || stable.ValueKind != JsonValueKind.True ||
            !readiness.TryGetProperty("observedAt", out var observed) || observed.ValueKind != JsonValueKind.Number || !observed.TryGetInt64(out var observedAt) ||
            now.ToUnixTimeMilliseconds() - observedAt is < 0 or > 8000 || Text(readiness, "documentId") == "")
            return Hold("현재 화면의 새 읽기를 기다립니다.");
        var mapping = new MappingStore(database).Active();
        var input = Mirror.Input(snapshot);
        if (input.Scope != "live" || Text(tab, "menu") != input.Screen || input.MappingRevision != Mapping.Hash(mapping))
            return Hold("화면 또는 수집 규칙이 바뀌어 다시 읽어야 합니다.");
        var profile = Erp.Screens.ProfileFor(mapping, input.Screen);
        if (profile?.Id != input.Profile) return Hold("이 화면의 수집 규칙을 확인하지 못했습니다.");
        var expected = preferences.Name?.Trim();
        if (!string.IsNullOrEmpty(expected))
        {
            // 수요기관 담당자는 업무 담당자와 다르므로 이름 조건에 쓰지 않는다.
            var staff = profile.Fields.FirstOrDefault(f => f.Target is "staff" or "notice_officer" &&
                f.SecondarySource is null && !f.Source.StartsWith("tables.", StringComparison.Ordinal));
            if (staff is null) return Hold("현재 화면에서 담당자 칸을 확인하지 못했습니다.");
            var actual = input.Data.TryGetProperty("pointInfo", out var point) ? At(point, staff.Source) : null;
            if (actual is null || !string.Equals(actual.Trim(), expected, StringComparison.Ordinal))
                return Hold("현재 화면의 담당자가 내 이름과 일치하지 않습니다.");
        }
        using var c = database.OpenReadOnly();
        var dataset = c.QuerySingle<string>("SELECT dataset_id FROM pclm_file");
        if (Text(tab, "datasetId") != dataset || !home.PointsAt(database.Path, dataset)) return Hold("저장 대상이 바뀌어 다시 읽어야 합니다.");
        var key = Mapping.Hash(new { dataset, input, preferences, documentId = Text(readiness, "documentId") });
        if (_currentKey != key) { _attempted.Clear(); _pending.Clear(); _currentKey = key; }
        if (_attempted.TryGetValue(key, out var prior)) return prior;
        AutoCaptureResult result;
        try
        {
            if (!_pending.TryGetValue(key, out var request))
            {
                var (live, preview) = Mirror.Prepare(database, input);
                var choices = Mirror.DefaultChoices(preview, new HashSet<string>());
                if (!preview.Changes.Any(ch => choices.GetValueOrDefault(ch.Id) != "keep"))
                    return Remember(new("unchanged", "같은 내용이 이미 저장되어 있습니다.", preview.Entity));
                request = (live, preview.BaseToken, Guid.NewGuid().ToString(), choices);
                _pending[key] = request;
            }
            if (maySave?.Invoke() == false) return Hold("앱을 종료하고 있어 저장하지 않습니다.");
            // 응답이 불명확해도 처음 요청의 토큰·선택·ID를 그대로 재사용한다.
            var saved = JsonSerializer.SerializeToElement(new ErpCapture(database, home: home).Save(request.Input, request.BaseToken, request.Id, request.Choices));
            result = new("stored", "현재 읽은 자료를 자동으로 저장했습니다.", Text(saved, "entity"));
            _pending.Remove(key);
        }
        catch (InvalidOperationException e) { _pending.Remove(key); result = Hold(e.Message); }
        return Remember(result);

        AutoCaptureResult Remember(AutoCaptureResult value)
        {
            _attempted[key] = value;
            return value;
        }
    }

    private static string? At(JsonElement value, string path)
    {
        foreach (var key in path.Split('.'))
            if (value.ValueKind != JsonValueKind.Object || !value.TryGetProperty(key, out value)) return null;
        return value.ValueKind is JsonValueKind.String ? value.GetString() : value.ValueKind is JsonValueKind.Number ? value.GetRawText() : null;
    }
    private static string Text(JsonElement value, string name) => value.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.String ? v.GetString() ?? "" : "";
}

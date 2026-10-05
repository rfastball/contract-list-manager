using System.Globalization;
using System.Text.Json;
using Dapper;
using Pclm.Core.Storage;

namespace Pclm.Core.Erp;

/// <summary>
/// 수집 기록 한 줄. 값은 화면에 그대로 찍을 문자열이다.
/// </summary>
/// <param name="At">수집한 때, <b>로컬</b> <c>yyyy-MM-ddTHH:mm:ss</c>. 저장은 UTC 라 여기서 옮긴다.</param>
/// <param name="Kind">접수·공고·계약.</param>
/// <param name="Number">번호-차수. 다른 화면과 같은 표기(<see cref="EntityRef.Display"/>)라 표 검색에 그대로 건다.</param>
/// <param name="Title">지금 그 차수의 건명. 없으면 수집 사진에서, 그래도 없으면 빈 문자열.</param>
/// <param name="Changed">저장이 무엇을 바꿨는가. 같은 화면을 다시 저장하면 거짓이다.</param>
/// <param name="Result"><c>저장</c> · <c>변경 없음</c>.</param>
/// <param name="Scope">확장의 실제 화면(<c>live</c>)인가 JSON 파일(<c>file</c>)인가.</param>
public sealed record CaptureEntry(
    string CaptureId, string At, string EntityType, string Kind, string Number, string Title,
    bool Changed, string Result, string Scope);

/// <summary>
/// 오늘 들어온 수집. 날짜 경계는 <b>로컬 자정</b>이다 — 사람이 말하는 「오늘」 이 그것이다.
/// </summary>
/// <param name="Today">로컬 날짜 <c>yyyy-MM-dd</c>.</param>
/// <param name="Entries">오늘 것 전부, 늦은 것부터.</param>
/// <param name="Saved">오늘 것 중 무엇을 바꾼(<c>저장</c>) 수.</param>
/// <param name="Last">가장 늦은 수집 하나. 오늘이 아니어도 온다 — 없으면 null.</param>
public sealed record CaptureDay(
    string Today, IReadOnlyList<CaptureEntry> Entries,
    int Requests, int Notices, int Contracts, int Saved, CaptureEntry? Last);

/// <summary>
/// 수집 기록(<c>erp_capture</c>)을 사람이 읽을 꼴로. <b>읽기만 한다</b> — 열람 창의 읽기 전용 사본에서도 같다.
///
/// <para>성공한 저장만 기록에 남는다. 검토 중이거나 결과를 확인하지 못한 것은 브라우저에 있다.</para>
/// </summary>
public static class CaptureLog
{
    public static string KindName(string entityType) => entityType switch
    {
        "request" => "접수",
        "notice" => "공고",
        "contract" => "계약",
        _ => "",
    };

    private sealed record Raw(string capture_id, string profile, string entity_base, string entity_seq,
        string snapshot_json, string result_json, string captured_at);

    /// <param name="now">로컬 지금. 시험이 날짜 경계를 세우려고 넘긴다.</param>
    public static CaptureDay Read(Database database, DateTime now)
    {
        var start = now.Date;
        var end = start.AddDays(1);
        // 저장은 UTC 의 왕복 표기("O")라 글자 순서가 곧 시각 순서다. 하루 앞까지 넉넉히 읽고 아래에서 로컬로 다시 가른다.
        var from = DateTime.SpecifyKind(start, DateTimeKind.Local).ToUniversalTime().AddDays(-1).ToString("O", CultureInfo.InvariantCulture);

        using var c = database.OpenReadOnly();
        const string columns = "capture_id, profile, entity_base, entity_seq, snapshot_json, result_json, captured_at";
        var recent = c.Query<Raw>($"SELECT {columns} FROM erp_capture WHERE captured_at >= @from ORDER BY captured_at DESC", new { from }).ToList();
        var last = c.QuerySingleOrDefault<Raw>($"SELECT {columns} FROM erp_capture ORDER BY captured_at DESC LIMIT 1");

        var active = ActiveMapping(database);
        var profiles = new Dictionary<string, string>(StringComparer.Ordinal);
        foreach (var p in Mapping.Defaults().Profiles.Concat(active.Profiles)) profiles[p.Id] = p.EntityType;
        var titles = new Dictionary<(string, string, string), string>();

        var entries = new List<CaptureEntry>();
        foreach (var raw in recent)
        {
            var at = Local(raw.captured_at);
            if (at is null || at < start || at >= end) continue;
            entries.Add(Entry(c, raw, at.Value, active, profiles, titles));
        }

        CaptureEntry? lastEntry = null;
        if (last is not null && Local(last.captured_at) is { } lastAt)
            lastEntry = Entry(c, last, lastAt, active, profiles, titles);

        return new(
            start.ToString("yyyy-MM-dd", CultureInfo.InvariantCulture), entries,
            entries.Count(e => e.EntityType == "request"),
            entries.Count(e => e.EntityType == "notice"),
            entries.Count(e => e.EntityType == "contract"),
            entries.Count(e => e.Changed),
            lastEntry);
    }

    private static DateTime? Local(string stored) =>
        DateTime.TryParse(stored, CultureInfo.InvariantCulture, DateTimeStyles.RoundtripKind, out var at)
            ? (at.Kind == DateTimeKind.Unspecified ? DateTime.SpecifyKind(at, DateTimeKind.Utc) : at).ToLocalTime()
            : null;

    /// <summary>
    /// 지금 매핑. 프로필이 어느 종류를 담는지와 사진 속 건명을 여기서 읽는다 — 프로필 이름은 기본 매핑의 것도
    /// 함께 본다(위). 읽지 못하면 기본 매핑으로 물러선다: 기록을 보이는 일이 매핑 탓에 멎을 까닭이 없다.
    /// </summary>
    private static MappingSet ActiveMapping(Database database)
    {
        try { return new MappingStore(database).Active(); }
        catch (Exception e) when (e is InvalidOperationException or JsonException) { return Mapping.Defaults(); }
    }

    private static CaptureEntry Entry(Microsoft.Data.Sqlite.SqliteConnection c, Raw raw, DateTime at, MappingSet mapping,
        Dictionary<string, string> profiles, Dictionary<(string, string, string), string> titles)
    {
        var changed = false;
        var resultType = "";
        var scope = "";
        try
        {
            using var result = JsonDocument.Parse(raw.result_json);
            var root = result.RootElement;
            if (root.TryGetProperty("changed", out var ch) && ch.ValueKind is JsonValueKind.True or JsonValueKind.False)
                changed = ch.GetBoolean();
            if (root.TryGetProperty("entityType", out var et) && et.ValueKind == JsonValueKind.String)
                resultType = et.GetString() ?? "";
            if (root.TryGetProperty("scope", out var sc) && sc.ValueKind == JsonValueKind.String)
                scope = sc.GetString() ?? "";
        }
        catch (JsonException) { }

        var entityType = profiles.GetValueOrDefault(raw.profile) ?? resultType;
        var key = (entityType, raw.entity_base, raw.entity_seq);
        if (!titles.TryGetValue(key, out var title))
            titles[key] = title = CurrentTitle(c, entityType, raw.entity_base, raw.entity_seq) ?? SnapshotTitle(raw, mapping) ?? "";

        return new(
            raw.capture_id,
            at.ToString("yyyy-MM-ddTHH:mm:ss", CultureInfo.InvariantCulture),
            entityType,
            KindName(entityType),
            new EntityRef(entityType, raw.entity_base, raw.entity_seq).Display,
            title,
            changed,
            changed ? "저장" : "변경 없음",
            scope);
    }

    /// <summary>지금 쌓인 그 차수의 건명. 비었거나 지워졌으면 null.</summary>
    private static string? CurrentTitle(Microsoft.Data.Sqlite.SqliteConnection c, string entityType, string @base, string seq)
    {
        // 표 이름은 여기 셋 중 하나뿐이다 — 기록의 값을 SQL 에 그대로 잇지 않는다.
        var table = entityType switch { "request" => "request", "notice" => "notice", "contract" => "contract", _ => null };
        if (table is null) return null;
        var title = c.QuerySingleOrDefault<string?>(
            $"SELECT title FROM {table} WHERE {table}_base = @base AND seq = @seq", new { @base, seq });
        return string.IsNullOrEmpty(title) ? null : title;
    }

    /// <summary>수집 사진에 담긴 건명. 사진을 지금 매핑으로 다시 읽어 본다 — 읽지 못하면 null.</summary>
    private static string? SnapshotTitle(Raw raw, MappingSet mapping)
    {
        try
        {
            using var snapshot = JsonDocument.Parse(raw.snapshot_json);
            var root = snapshot.RootElement;
            if (!root.TryGetProperty("data", out var data)) return null;
            var scope = root.TryGetProperty("scope", out var s) && s.ValueKind == JsonValueKind.String ? s.GetString()! : "file";
            var document = Mapping.Map(mapping, raw.profile, data, scope);
            var head = document.Rows.FirstOrDefault(r => r.Table == document.EntityType);
            return head is not null && head.Values.TryGetValue("title", out var title) && !string.IsNullOrEmpty(title) ? title : null;
        }
        catch (Exception e) when (e is InvalidOperationException or JsonException or KeyNotFoundException or FormatException)
        {
            return null;
        }
    }
}

using System.Globalization;
using System.Text.Json;
using System.Text.RegularExpressions;
using Dapper;
using Pclm.Core.Storage;

namespace Pclm.Core.Erp;

public sealed record FieldMap(string Source, string Target, string Transform = "text",
    Dictionary<string, string>? Codes = null, string? SecondarySource = null);
public sealed record TableMap(string Source, string Target, string[] Keys, FieldMap[] Fields,
    bool Required = false, string? TotalSelector = null);
public sealed record MappingProfile(string Id, string EntityType, string IdentitySource,
    string BaseField, string SeqField, FieldMap[] Fields, TableMap[] Tables,
    string? Heading = null, Dictionary<string, string>? Selectors = null);
public sealed record MappingSet(int Version, MappingProfile[] Profiles);
public sealed record MappedRow(string Table, string SourceKey, Dictionary<string, string?> Values,
    Dictionary<string, string>? Origins = null);
public sealed record MappedDocument(string Profile, string EntityType, string Base, string Seq,
    List<MappedRow> Rows, string Scope);

/// <summary>SQL 식별자와 변환 종류는 프로그램이 허용한다. 매핑 파일은 실행 코드가 아니다.</summary>
public static class Mapping
{
    public static readonly JsonSerializerOptions Json = new(JsonSerializerDefaults.Web)
    {
        WriteIndented = true,
        UnmappedMemberHandling = System.Text.Json.Serialization.JsonUnmappedMemberHandling.Disallow,
        MaxDepth = 32,
    };
    public static MappingSet Defaults()
    {
        using var stream = typeof(Mapping).Assembly.GetManifestResourceStream("Pclm.Core.Erp.mapping.json")!;
        return JsonSerializer.Deserialize<MappingSet>(stream, Json)!;
    }
    public static MappingSet Parse(string json)
    {
        var value = JsonSerializer.Deserialize<MappingSet>(json, Json)
            ?? throw new InvalidOperationException("매핑이 비어 있습니다.");
        Validate(value);
        return value;
    }
    public static string Hash(object value) => Convert.ToHexString(
        System.Security.Cryptography.SHA256.HashData(System.Text.Encoding.UTF8.GetBytes(JsonSerializer.Serialize(value, Json))));

    // 기본 매핑의 목적지 목록이 수집 허용 목록이다. 임의 DB 열/민감 필드를 추가할 수 없다.
    public static void Validate(MappingSet set)
    {
        var defaults = Defaults();
        if (set.Version != 1 || set.Profiles is null || set.Profiles.Any(p => p is null || p.Fields is null || p.Tables is null || p.BaseField is null || p.SeqField is null || p.IdentitySource is null) || set.Profiles.Length != defaults.Profiles.Length ||
            set.Profiles.Select(p => p.Id).Distinct().Count() != set.Profiles.Length)
            throw new InvalidOperationException("지원하는 매핑 판·프로필 구성을 확인하세요.");
        foreach (var p in set.Profiles)
        {
            var original = defaults.Profiles.SingleOrDefault(x => x.Id == p.Id);
            if (original is null || original.EntityType != p.EntityType || p.Tables.Length > 12)
                throw new InvalidOperationException("자료 종류나 프로필 ID는 바꿀 수 없습니다.");
            CheckPath(p.IdentitySource); CheckPath(p.BaseField); CheckPath(p.SeqField);
            CheckFields(p.Fields, original.Fields);
            if (!p.Fields.Any(f => f.Target == "title")) throw new InvalidOperationException("문서 제목 매핑이 필요합니다.");
            if (p.Tables.Select(t => t?.Target).Distinct().Count() != p.Tables.Length)
                throw new InvalidOperationException("동일 목적지 표를 중복 수집할 수 없습니다.");
            foreach (var table in p.Tables)
            {
                if (table is null || table.Keys is null || table.Fields is null || table.Source is null) throw new InvalidOperationException("표 매핑을 확인하세요.");
                var allowed = original.Tables.SingleOrDefault(t => t.Target == table.Target)
                    ?? throw new InvalidOperationException("지원하지 않는 대상 표입니다.");
                CheckPath(table.Source);
                if (table.Keys.Length is < 1 or > 5) throw new InvalidOperationException("원천 행 복합키가 필요합니다.");
                foreach (var key in table.Keys) CheckPath(key);
                CheckFields(table.Fields, allowed.Fields);
                if (table.TotalSelector?.Length > 500) throw new InvalidOperationException("전체 건수 선택자가 너무 깁니다.");
            }
            if (p.Id != "g2b-public-notice-header-v1" && !p.Tables.Any(t => t.Target == p.EntityType + "_item" && t.Required))
                throw new InvalidOperationException("품목 전체 범위 검증은 필수입니다.");
            if (p.Selectors?.Any(x => !p.Fields.Any(f => f.Source == x.Key || f.SecondarySource == x.Key) && x.Key != p.BaseField && x.Key != p.SeqField || x.Value is null || x.Value.Length > 500) == true)
                throw new InvalidOperationException("허용 필드의 선택자만 지정할 수 있습니다.");
        }
    }
    private static void CheckFields(FieldMap[] fields, FieldMap[] allowed)
    {
        if (fields.Any(f => f is null || f.Source is null || f.Target is null)) throw new InvalidOperationException("필드 매핑을 확인하세요.");
        if (fields.Select(f => f.Target).Distinct().Count() != fields.Length)
            throw new InvalidOperationException("한 대상 필드에 두 출처를 지정할 수 없습니다.");
        foreach (var f in fields)
        {
            CheckPath(f.Source);
            if (f.SecondarySource is not null) CheckPath(f.SecondarySource);
            if (!allowed.Any(a => a.Target == f.Target) ||
                !new[] { "text", "decimal", "integer", "date", "datetime", "boolean", "code" }.Contains(f.Transform))
                throw new InvalidOperationException($"허용되지 않은 대상·변환입니다: {f.Target}");
            if (Regex.IsMatch(f.Source, "account(?!ing)|actno|prsnno|password|atflpath|sess|ipar", RegexOptions.IgnoreCase))
                throw new InvalidOperationException("민감·내부 필드는 매핑할 수 없습니다.");
        }
    }
    private static void CheckPath(string? path)
    {
        if (string.IsNullOrWhiteSpace(path) || Regex.IsMatch(path, "account(?!ing)|actno|prsnno|password|atflpath|sess|ipar", RegexOptions.IgnoreCase) || path.Length > 250 || !Regex.IsMatch(path, @"\A[a-zA-Z0-9_*]+(?:\.[a-zA-Z0-9_]+)*\z"))
            throw new InvalidOperationException("경로는 필드 ID와 점, 표 접미사 와일드카드만 지원합니다.");
    }
    public static JsonElement? At(JsonElement value, string path)
    {
        foreach (var part in path.Split('.'))
        {
            if (value.ValueKind == JsonValueKind.Array && part == "0")
            {
                if (value.GetArrayLength() != 1) throw new InvalidOperationException("문서 전체 금액은 단일 분류 표에서만 가져올 수 있습니다.");
                value = value[0];
            }
            else if (value.ValueKind != JsonValueKind.Object || !value.TryGetProperty(part, out value)) return null;
        }
        return value;
    }
    public static string? Text(JsonElement? value) => value is null || value.Value.ValueKind == JsonValueKind.Null
        ? null : value.Value.ValueKind == JsonValueKind.String ? value.Value.GetString()?.Trim() : value.Value.ToString();
    private static bool HasTable(JsonElement input, string source) => input.TryGetProperty("tables", out var tables) &&
        tables.EnumerateObject().Any(p => source.StartsWith('*') ? p.Name.EndsWith(source[1..], StringComparison.Ordinal) : p.Name == source);
    public static JsonElement[] Table(JsonElement input, string source)
    {
        if (!input.TryGetProperty("tables", out var tables)) throw new InvalidOperationException("tables가 없습니다.");
        var matches = tables.EnumerateObject().Where(p => source.StartsWith('*')
            ? p.Name.EndsWith(source[1..], StringComparison.Ordinal) : p.Name == source).ToArray();
        if (matches.Length != 1 || matches[0].Value.ValueKind != JsonValueKind.Array)
            throw new InvalidOperationException($"대표 표를 하나로 확인하지 못했습니다: {source}");
        return matches[0].Value.EnumerateArray().ToArray();
    }
    public static JsonElement Sanitize(MappingSet set, string profile, JsonElement input)
    {
        var p = set.Profiles.Single(x => x.Id == profile);
        var pointKeys = p.Fields.SelectMany(f => f.SecondarySource is null ? new[] { f.Source } : new[] { f.Source, f.SecondarySource }).Where(k => !k.StartsWith("tables.")).Concat(p.IdentitySource == "pointInfo" ? new[] { p.BaseField, p.SeqField } : []);
        var tables = new Dictionary<string, object>();
        foreach (var t in p.Tables)
        {
            JsonElement[] rows;
            try { rows = Table(input, t.Source); }
            catch (InvalidOperationException) when (!HasTable(input, t.Source)) { continue; }
            var key = input.GetProperty("tables").EnumerateObject().Single(x => t.Source.StartsWith('*') ? x.Name.EndsWith(t.Source[1..], StringComparison.Ordinal) : x.Name == t.Source).Name;
            tables[key] = rows.Select(row => Pick(row, t.Fields.SelectMany(f => f.SecondarySource is null ? new[] { f.Source } : new[] { f.Source, f.SecondarySource }).Concat(t.Keys))).ToArray();
        }
        foreach (var group in p.Fields.Where(f => f.Source.StartsWith("tables.")).GroupBy(f => f.Source.Split('.')[1]))
            if (input.GetProperty("tables").TryGetProperty(group.Key, out var values))
                tables[group.Key] = values.EnumerateArray().Select(r => Pick(r, group.Select(f => string.Join('.', f.Source.Split('.')[3..])))).ToArray();
        return JsonSerializer.SerializeToElement(new { pointInfo = Pick(input.GetProperty("pointInfo"), pointKeys), tables });
        static System.Text.Json.Nodes.JsonObject Pick(JsonElement row, IEnumerable<string> paths)
        {
            var result = new System.Text.Json.Nodes.JsonObject();
            foreach (var path in paths.Distinct())
            {
                var value = At(row, path); if (value is null) continue;
                var parts = path.Split('.'); var target = result;
                foreach (var part in parts[..^1])
                {
                    target[part] ??= new System.Text.Json.Nodes.JsonObject();
                    target = target[part]!.AsObject();
                }
                target[parts[^1]] = System.Text.Json.Nodes.JsonNode.Parse(value.Value.GetRawText());
            }
            return result;
        }
    }
    public static MappedDocument Map(MappingSet set, string profileId, JsonElement input, string scope = "file")
    {
        Validate(set);
        var p = set.Profiles.SingleOrDefault(x => x.Id == profileId)
            ?? throw new InvalidOperationException("매핑 프로필을 선택하세요.");
        var point = input.GetProperty("pointInfo");
        var identity = p.IdentitySource == "pointInfo" ? point : Table(input, p.IdentitySource).FirstOrDefault();
        var b = Text(At(identity, p.BaseField)); var s = Text(At(identity, p.SeqField));
        if (p.BaseField == p.SeqField)
        {
            var match = Regex.Match(b ?? "", @"\A(R\d{2}[A-Z]{2}\d{8})\s*-\s*(\d{2,3})\z");
            if (!match.Success) throw new InvalidOperationException("본번호·차수 형식을 확인하세요.");
            b = match.Groups[1].Value; s = match.Groups[2].Value;
        }
        if (string.IsNullOrEmpty(b) || !Regex.IsMatch(b, @"\A[A-Za-z0-9]+\z") ||
            string.IsNullOrEmpty(s) || !Regex.IsMatch(s, @"\A\d{1,3}\z"))
            throw new InvalidOperationException("문서 식별자와 차수가 필요합니다. 선행 0을 유지하세요.");
        if (p.EntityType == "notice" && (!Regex.IsMatch(b, @"\AR\d{2}BK\d{8}\z") || s.Length != 3) ||
            p.EntityType == "contract" && (!Regex.IsMatch(b, @"\AR\d{2}TA\d{8}\z") || s.Length != 2))
            throw new InvalidOperationException("공고·계약 식별자 형식이 다릅니다.");
        // 문서를 가르는 것은 번호·차수뿐이다. 제목·표가 없으면 그 값을 수집하지 않은 것으로 두고 기존 값을 건드리지 않는다.
        var rows = new List<MappedRow> { new(p.EntityType, "", Fields(point, p.Fields, input)) };
        foreach (var t in p.Tables)
        {
            JsonElement[] source;
            try { source = Table(input, t.Source); }
            catch (InvalidOperationException) when (!HasTable(input, t.Source)) { continue; }
            if (source.Length == 0) continue; // 빈 표는 덜 불러온 탭일 수 있다 — 있던 행을 지우지 않는다.
            var keys = new HashSet<string>();
            foreach (var row in source)
            {
                var parts = t.Keys.Select(k => Text(At(row, k))).ToArray();
                if (parts.Any(string.IsNullOrEmpty)) throw new InvalidOperationException($"원천 행 키가 없습니다: {t.Source}");
                var key = JsonSerializer.Serialize(parts);
                if (!keys.Add(key)) throw new InvalidOperationException($"원천 행 키가 중복됩니다: {t.Source}");
                if (p.EntityType == "request" && t.Target == "request_item" &&
                    (Text(At(row, p.BaseField)) != b || Text(At(row, p.SeqField)) != s))
                    throw new InvalidOperationException("서로 다른 접수를 한 문서로 저장할 수 없습니다.");
                rows.Add(new(t.Target, key, Fields(row, t.Fields)));
            }
        }
        // 품목을 걷지 않았으면 총액을 다시 셈하지 않는다 — 0 으로 덮게 된다.
        if (p.EntityType == "contract" && rows.Any(r => r.Table == "contract_item"))
        {
            var amounts = rows.Where(r => r.Table == "contract_item").Select(r => r.Values.GetValueOrDefault("amount")).ToArray();
            if (amounts.Any(string.IsNullOrEmpty)) throw new InvalidOperationException("전체 품목 금액이 있어야 계약 총액을 계산할 수 있습니다.");
            rows[0].Values["amount"] = (decimal.Truncate(amounts.Sum(a => decimal.Parse(a!, CultureInfo.InvariantCulture)) / 10) * 10).ToString(CultureInfo.InvariantCulture);
        }
        if (p.Id == "g2b-notice-a-v1")
        {
            var values = rows[0].Values;
            if (values.GetValueOrDefault("notice_kind") is { } kind && values.GetValueOrDefault("source_status") is { } status && !status.Contains("미확정"))
                values["notice_kind"] = kind + "(" + status + ")";
            foreach (var (name, start, end) in new[] { ("입찰서제출", "bid_opens_at", "bid_closes_at"), ("개찰", "opening_at", ""), ("입찰참가자격등록", "", "registration_closes_at") })
            {
                var schedule = new Dictionary<string, string?> { ["name"] = name };
                if (values.ContainsKey(start)) schedule["starts_at"] = values[start];
                if (values.ContainsKey(end)) schedule["ends_at"] = values[end];
                if (schedule.Count > 1) rows.Add(new("notice_schedule", "schedule:" + name, schedule));
            }
        }
        rows = rows.Select(r => r with { Origins = r.Values.Keys.ToDictionary(k => k, _ => p.Id) }).ToList();
        // 접수의 키는 접수번호·접수차수 그대로다(ADR-027). 접두어를 달지 않는다.
        return new(p.Id, p.EntityType, b, s, rows, scope);
    }
    private static Dictionary<string, string?> Fields(JsonElement source, FieldMap[] fields, JsonElement? root = null)
    {
        var result = new Dictionary<string, string?>();
        foreach (var f in fields)
        {
            var value = At(f.Source.StartsWith("tables.") && root is not null ? root.Value : source, f.Source);
            if (value?.ValueKind is JsonValueKind.Object or JsonValueKind.Array) throw new InvalidOperationException("원천 필드는 단일 값이어야 합니다.");
            if (value is not null)
            {
                var text = Text(value);
                if (f.SecondarySource is not null)
                {
                    var suffix = Text(At(source, f.SecondarySource));
                    if (!string.IsNullOrEmpty(text) && string.IsNullOrEmpty(suffix)) continue;
                    text += suffix;
                }
                result[f.Target] = ConvertValue(text, f);
            }
        }
        return result;
    }
    public static string? ConvertValue(string? value, FieldMap f)
    {
        if (string.IsNullOrWhiteSpace(value) || value == "-" && f.Transform is "date" or "datetime") return null;
        try
        {
            return f.Transform switch
            {
                "text" => value,
                "code" => f.Codes?.GetValueOrDefault(value) ?? $"{value} (명칭 미확정)",
                "decimal" => decimal.Parse(value.Replace(",", ""), NumberStyles.AllowLeadingSign | NumberStyles.AllowDecimalPoint, CultureInfo.InvariantCulture).ToString(CultureInfo.InvariantCulture),
                "integer" => int.Parse(value.Replace(",", ""), CultureInfo.InvariantCulture).ToString(CultureInfo.InvariantCulture),
                "boolean" => value switch { "Y" or "True" or "true" or "1" or "허용" or "가능" or "예" => "1", "N" or "False" or "false" or "0" or "불허" or "불가" or "아니오" => "0", _ => throw new FormatException() },
                "date" or "datetime" => DateTime.ParseExact(value,
                    ["yyyy-MM-ddTHH:mm:ss", "yyyy-MM-ddTHH:mm:ss.FFFFFFF", "yyyyMMdd", "yyyyMMddHHmmss", "yyyyMMddHHmm", "yyyy-MM-dd", "yyyy/MM/dd", "yyyy-MM-dd HH:mm:ss", "yyyy/MM/dd HH:mm:ss", "yyyy-MM-dd HH:mm", "yyyy/MM/dd HH:mm"],
                    CultureInfo.InvariantCulture, DateTimeStyles.None).ToString(f.Transform == "date" ? "yyyy-MM-dd" : "yyyy-MM-dd HH:mm:ss", CultureInfo.InvariantCulture),
                _ => throw new FormatException(),
            };
        }
        catch (Exception e) when (e is FormatException or OverflowException)
        { throw new InvalidOperationException($"필드 형식 오류: {f.Source} → {f.Target} ({f.Transform})"); }
    }
}

public sealed class MappingStore(Database database)
{
    public MappingSet Active()
    {
        using var c = database.OpenReadOnly();
        var json = c.QuerySingleOrDefault<string>("SELECT json FROM erp_mapping WHERE active = 1");
        return json is null ? Mapping.Defaults() : Mapping.Parse(json);
    }
    public object List()
    {
        using var c = database.OpenReadOnly();
        return new { active = Active(), defaults = Mapping.Defaults(), versions = c.Query("SELECT revision, json, active, validated, created_at FROM erp_mapping ORDER BY created_at DESC") };
    }
    public string Save(string json)
    {
        var set = Mapping.Parse(json); var revision = Mapping.Hash(set);
        using var c = database.Open();
        c.Execute("INSERT INTO erp_mapping(revision,json,created_at) VALUES(@revision,@json,@now) ON CONFLICT DO NOTHING",
            new { revision, json = JsonSerializer.Serialize(set, Mapping.Json), now = DateTime.UtcNow.ToString("O") });
        return revision;
    }
    public void Activate(string revision)
    {
        using var c = database.Open(); using var tx = c.BeginTransaction();
        if (c.ExecuteScalar<int>("SELECT count(*) FROM erp_mapping WHERE revision=@revision AND validated=1", new { revision }, tx) != 1)
            throw new InvalidOperationException("샘플 입력으로 매핑을 검증한 뒤 활성화하세요.");
        c.Execute("UPDATE erp_mapping SET active=0; UPDATE erp_mapping SET active=1 WHERE revision=@revision", new { revision }, tx);
        tx.Commit();
    }
    public MappedDocument ValidateSample(string json, string profile, JsonElement input)
    {
        var set = Mapping.Parse(json); var document = Mapping.Map(set, profile, input);
        var revision = Save(json);
        using var c = database.Open();
        using var tx = c.BeginTransaction();
        var samples = JsonSerializer.Deserialize<HashSet<string>>(c.QuerySingle<string>("SELECT samples FROM erp_mapping WHERE revision=@revision", new { revision }, tx))!;
        samples.Add(profile);
        var required = set.Profiles.Where(p => HashProfile(p) != HashProfile(DefaultsProfile(p.Id))).Select(p => p.Id).ToArray();
        var validated = required.All(samples.Contains) ? 1 : 0;
        c.Execute("UPDATE erp_mapping SET validated=@validated,samples=@samples WHERE revision=@revision", new { revision, validated, samples = JsonSerializer.Serialize(samples) }, tx);
        tx.Commit();
        static string HashProfile(MappingProfile p) => Mapping.Hash(p);
        static MappingProfile DefaultsProfile(string id) => Mapping.Defaults().Profiles.Single(p => p.Id == id);
        return document;
    }
}

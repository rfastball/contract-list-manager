using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.Json.Serialization;
using Dapper;
using Microsoft.Data.Sqlite;

namespace Pclm.Core.Storage;

public sealed partial class Store
{
    public sealed record ViewPackageColumn(string EntityType, string FieldName, string Kind, string[] Choices, JsonElement? Formula);
    public sealed record ViewPackageView(string Name, JsonElement Definition);
    public sealed record ViewPackage(string Format, int Version, bool IncludesAllColumnsForSql,
        IReadOnlyList<ViewPackageView> Views, IReadOnlyList<ViewPackageColumn> Columns);
    public sealed record ViewPackagePlannedView(string Name, string ImportName);
    public sealed record ViewPackagePlannedColumn(string EntityType, string FieldName, string Action);
    public sealed record ViewPackagePreview(IReadOnlyList<ViewPackagePlannedView> Views,
        IReadOnlyList<ViewPackagePlannedColumn> Columns, IReadOnlyList<string> Errors,
        bool IncludesAllColumnsForSql, string Token);

    private static readonly JsonSerializerOptions PackageJson = new(Flow.Json)
    {
        UnmappedMemberHandling = JsonUnmappedMemberHandling.Disallow,
        WriteIndented = true,
    };

    /// <summary>Configuration only: no rows, input cells, overrides or source payloads.</summary>
    public string ExportViewPackage(IReadOnlyList<string> ids)
    {
        using var connection = _database.OpenReadOnly();
        using var transaction = connection.BeginTransaction(deferred: true);
        var saved = UserViewBuilder.Read(connection, transaction);
        var chosen = saved.Where(v => ids.Contains(v.Id)).ToList();
        if (chosen.Count == 0 || ids.Distinct().Count() != chosen.Count)
            throw new InvalidOperationException("내보낼 보기를 하나 이상 고르세요. 삭제된 보기는 목록을 새로 읽어 주세요.");
        var all = connection.Query<UserColumnDefinition>("""
            SELECT entity_type AS EntityType, field_name AS FieldName, kind AS Kind,
                   options AS Options, sort_order AS SortOrder FROM user_column ORDER BY entity_type, sort_order
            """, transaction: transaction).ToList();
        var needed = new HashSet<(string, string)>();
        var formulas = new Queue<(string? Entity, string Json)>(chosen.SelectMany(v => v.Columns.Select(c => ((string?)null, c.Formula))));
        var sql = false;
        while (formulas.TryDequeue(out var item))
        {
            var formula = Flow.Parse(item.Json);
            if (formula.Source == "sql")
            {
                sql = true;
                // SQL can hide indirect references; carry all definitions rather than guess from text.
                foreach (var c in all)
                    if (needed.Add((c.EntityType, c.FieldName)) && c.Kind == "computed")
                        formulas.Enqueue((c.EntityType, c.Options ?? ""));
            }
            else if (item.Entity is null)
            {
                var node = JsonNode.Parse(item.Json);
                foreach (var c in all)
                    if (UserViewBuilder.Touch(node, c.EntityType, c.FieldName, null) && needed.Add((c.EntityType, c.FieldName)) && c.Kind == "computed")
                        formulas.Enqueue((c.EntityType, c.Options ?? ""));
            }
        }
        var package = new ViewPackage("pclm-view-config", 1, sql,
            chosen.Select(v => new ViewPackageView(v.Name, JsonDocument.Parse(v.Definition).RootElement.Clone())).ToList(),
            all.Where(c => needed.Contains((c.EntityType, c.FieldName))).Select(c => new ViewPackageColumn(
                c.EntityType, c.FieldName, c.Kind, [.. c.Choices], c.Kind == "computed"
                    ? JsonSerializer.SerializeToElement(ComputedColumn.Parse(c.Options ?? ""), Flow.Json) : null)).ToList());
        return JsonSerializer.Serialize(package, PackageJson);
    }

    private static ViewPackage ReadViewPackage(string json)
    {
        try
        {
            if (json.Length > 2_000_000) throw new InvalidOperationException("보기 구성 파일은 2MB까지 가져올 수 있습니다.");
            var p = JsonSerializer.Deserialize<ViewPackage>(json, PackageJson);
            if (p is null || p.Format != "pclm-view-config" || p.Version != 1 || p.Views is null || p.Columns is null
                || p.Views.Count is < 1 or > 100 || p.Columns.Count > 500)
                throw new InvalidOperationException("지원하는 보기 구성 파일이 아닙니다(구성 판 1).");
            return p;
        }
        catch (JsonException) { throw new InvalidOperationException("보기 구성 JSON 형식이 올바르지 않습니다. 자료가 아닌 구성 파일을 골라 주세요."); }
    }

    /// <summary>
    /// 서식용 예시 보기 — exe 에 실린 보기 구성(docs/user-views.md §13). 맞춤 열에 기대지 않고 계약면의 기본 열만 읽으며 SQL 필드가
    /// 없다. 작업자료에는 처음부터 들어 있고(<see cref="SeedExampleViews"/>), 지웠으면 「보기 구성」 에서 골라 같은 미리보기·가져오기로
    /// 다시 들인다.
    /// </summary>
    public static string BuiltinViewPackage() => Preset("presets/builtin-views.json");

    /// <summary>
    /// 스키마 26 이 심었던 예시 보기 그대로 — 얼려 둔 사본이다(<c>presets/example-view-v26.json</c>). v26 단계는 여전히 이것을 심고,
    /// v27 단계는 저장된 정의가 이것과 같을 때(사람이 손대지 않았을 때)만 지금 구성으로 갈아 끼운다. 고치지 않는다.
    /// </summary>
    internal static string ExampleViewPackageV26() => Preset("presets/example-view-v26.json");

    private static string Preset(string name)
    {
        using var stream = typeof(Store).Assembly.GetManifestResourceStream(name)
            ?? throw new InvalidOperationException("서식용 예시 보기가 이 판에 실려 있지 않습니다.");
        using var reader = new StreamReader(stream, Encoding.UTF8);
        return reader.ReadToEnd();
    }

    /// <summary>
    /// 예시 보기를 작업자료에 심는다 — 같은 이름(대소문자 무시)의 보기가 이미 있으면 그것을 두고 넘어간다. 부르는 곳은 둘뿐이다:
    /// 빈 작업자료를 짓는 <see cref="PclmFile.Create"/>(지금 구성)와, <see cref="Schema.ExampleViewVersion"/> 을 지나는 작업자료의
    /// <see cref="Database.Migrate"/>(그 판의 구성 <paramref name="package"/>). 뷰를 짓는 것은 부르는 쪽이다.
    /// </summary>
    internal static void SeedExampleViews(SqliteConnection connection, SqliteTransaction transaction, string? package = null)
    {
        var taken = connection.Query<string>("SELECT name FROM user_view", transaction: transaction).ToHashSet(StringComparer.OrdinalIgnoreCase);
        foreach (var view in ReadViewPackage(package ?? BuiltinViewPackage()).Views)
        {
            if (!taken.Add(view.Name)) continue;
            connection.Execute("""
                INSERT INTO user_view (id, name, definition, sort_order)
                VALUES (@viewId, @name, @definition, coalesce((SELECT max(sort_order) FROM user_view), 0) + 10)
                """, new { viewId = Guid.NewGuid().ToString("N"), name = view.Name, definition = StoredDefinition(view) }, transaction);
        }
    }

    /// <summary>
    /// 스키마 27 — v26 이 심은 예시 보기를 사람이 손대지 않았으면 지금 구성으로 갈아 끼운다(docs/user-views.md §13). 이름이 같고
    /// 저장된 정의가 v26 의 것과 <b>같은 JSON</b>(키 순서 무시)일 때만이다. 고쳤거나 지웠거나 이름을 바꾼 보기는 그대로 둔다.
    /// 뷰를 짓는 것은 부르는 쪽이다.
    /// </summary>
    internal static void UpgradeExampleViews(SqliteConnection connection, SqliteTransaction transaction)
    {
        var current = ReadViewPackage(BuiltinViewPackage()).Views.ToDictionary(v => v.Name, StringComparer.Ordinal);
        foreach (var old in ReadViewPackage(ExampleViewPackageV26()).Views)
        {
            if (!current.TryGetValue(old.Name, out var next)) continue;
            var stored = connection.Query<(string Id, string Definition)>(
                "SELECT id, definition FROM user_view WHERE name = @name", new { name = old.Name }, transaction).ToList();
            if (stored is not [var view] || !SameJson(view.Definition, StoredDefinition(old))) continue;
            connection.Execute("UPDATE user_view SET definition = @definition WHERE id = @id",
                new { id = view.Id, definition = StoredDefinition(next) }, transaction);
        }
    }

    /// <summary>구성의 보기 하나를 저장 꼴 정의로 — 화면이 저장하는 것과 같은 꼴(필드 식에 식 식별자 없음).</summary>
    private static string StoredDefinition(ViewPackageView view)
    {
        var input = ParseViewInput(JsonSerializer.Serialize(view, Flow.Json));
        return new JsonObject
        {
            ["version"] = 2,
            ["contracts"] = input.Fold ? "fold" : "split",
            ["columns"] = new JsonArray([.. input.Columns.Select(c => (JsonNode)new JsonObject
            {
                ["id"] = c.Id,
                ["name"] = c.Name,
                ["formula"] = StoredFormula(Flow.Parse(c.Formula)),
            })]),
        }.ToJsonString(Flow.Json);
    }

    /// <summary>두 JSON 이 같은가 — 객체 키 순서는 보지 않고, 배열 순서와 값은 본다. 읽을 수 없는 글은 다르다.</summary>
    private static bool SameJson(string a, string b)
    {
        static string Canon(JsonNode? node) => node switch
        {
            JsonObject o => "{" + string.Join(",", o.OrderBy(p => p.Key, StringComparer.Ordinal)
                .Select(p => JsonSerializer.Serialize(p.Key) + ":" + Canon(p.Value))) + "}",
            JsonArray xs => "[" + string.Join(",", xs.Select(Canon)) + "]",
            null => "null",
            _ => node.ToJsonString(),
        };
        try { return Canon(JsonNode.Parse(a)) == Canon(JsonNode.Parse(b)); }
        catch (JsonException) { return false; }
    }

    public ViewPackagePreview PreviewViewPackage(string json)
    {
        if (_database.Access == Access.Read) throw new InvalidOperationException("열람 중에는 보기 구성을 가져올 수 없습니다.");
        using var connection = _database.Open();
        using var transaction = connection.BeginTransaction();
        return StageViewPackage(connection, transaction, ReadViewPackage(json)); // disposal rolls back the preview
    }

    public IReadOnlyList<UserViewInfo> ImportViewPackage(string json, string token)
    {
        if (_database.Access == Access.Read) throw new InvalidOperationException("열람 중에는 보기 구성을 가져올 수 없습니다.");
        using (var connection = _database.Open())
        using (var transaction = connection.BeginTransaction())
        {
            var preview = StageViewPackage(connection, transaction, ReadViewPackage(json));
            if (preview.Errors.Count > 0) throw new InvalidOperationException(string.Join("\n", preview.Errors));
            if (preview.Token != token) throw new InvalidOperationException("미리보기 뒤 구성이 바뀌었습니다. 파일을 다시 골라 확인하세요. 가져온 것은 없습니다.");
            transaction.Commit();
        }
        return UserViews();
    }

    private static string CopyViewName(string name, HashSet<string> taken)
    {
        if (taken.Add(name)) return name;
        for (var i = 1; ; i++)
        {
            var suffix = i == 1 ? " (복사본)" : $" (복사본 {i})";
            var candidate = name[..Math.Min(name.Length, UserViewNameLength - suffix.Length)] + suffix;
            if (taken.Add(candidate)) return candidate;
        }
    }

    private static ViewPackagePreview StageViewPackage(SqliteConnection connection, SqliteTransaction transaction, ViewPackage package)
    {
        var errors = new List<string>();
        var plannedColumns = new List<ViewPackagePlannedColumn>();
        var plannedViews = new List<ViewPackagePlannedView>();
        var viewIds = new List<string>();
        var seenColumns = new HashSet<(string, string)>();
        foreach (var c in package.Columns)
        {
            try
            {
                if (c is null || c.Choices is null || c.Choices.Any(choice => choice is null) || c.EntityType is not ("notice" or "contract" or "request"))
                    throw new InvalidOperationException("맞춤 열의 자료 종류 또는 후보 형식이 올바르지 않습니다.");
                var name = CleanName(c.FieldName ?? "");
                if (!seenColumns.Add((c.EntityType, name))) throw new InvalidOperationException($"구성 안에서 열이 겹칩니다: {name}");
                string? options;
                if (c.Kind == "computed")
                {
                    if (c.Formula is not { } f) throw new InvalidOperationException("가공식이 없습니다.");
                    var formula = ComputedColumn.ParseFor(c.EntityType, f.GetRawText());
                    if (formula.Source == "sql") SqlColumn.CheckShape(formula.Sql);
                    else _ = FlowCompiler.Expression(c.EntityType, formula);
                    options = Flow.Serialize(formula with { Id = "" });
                }
                else
                {
                    CheckKind(c.Kind);
                    if (c.Formula is not null) throw new InvalidOperationException("입력열에는 활성 가공식을 넣을 수 없습니다.");
                    options = Pack(c.Kind, c.Choices);
                }
                var existing = connection.QuerySingleOrDefault<(string Name, string Kind, string? Options)?>(
                    "SELECT field_name, kind, options FROM user_column WHERE entity_type=@EntityType AND field_name=@name COLLATE NOCASE",
                    new { c.EntityType, name }, transaction);
                if (existing is { } old)
                {
                    var oldOptions = old.Kind == "computed" ? Flow.Serialize(ComputedColumn.Parse(old.Options ?? "") with { Id = "" })
                        : old.Kind == "choice" ? old.Options : null;
                    if (old.Name != name || old.Kind != c.Kind || oldOptions != options)
                    {
                        // 받는 사람이 할 수 있는 길을 먼저 말한다 — 원본을 고치는 것은 보낸 사람의 일이다. 겹친 열마다 한 줄씩 선다.
                        plannedColumns.Add(new(c.EntityType, name, "conflict"));
                        errors.Add(ConflictMessage(c.EntityType, old.Name));
                        continue;
                    }
                    plannedColumns.Add(new(c.EntityType, name, "reuse"));
                    continue;
                }
                CheckFree(connection, c.EntityType, name);
                if (c.Kind == "computed")
                    options = Flow.Serialize(Flow.Parse(options!) with { Id = Guid.NewGuid().ToString("N") });
                connection.Execute("""
                    INSERT INTO user_column(entity_type,field_name,kind,options,sort_order)
                    VALUES(@EntityType,@name,@Kind,@options,coalesce((SELECT max(sort_order) FROM user_column WHERE entity_type=@EntityType),0)+10)
                    """, new { c.EntityType, name, c.Kind, options }, transaction);
                plannedColumns.Add(new(c.EntityType, name, "add"));
            }
            catch (Exception e) when (e is InvalidOperationException or SqliteException or ArgumentException)
            { errors.Add($"{c?.EntityType}.{c?.FieldName}: {e.Message}"); }
        }
        var taken = connection.Query<string>("SELECT name FROM user_view", transaction: transaction).ToHashSet(StringComparer.OrdinalIgnoreCase);
        foreach (var v in package.Views)
        {
            try
            {
                if (v is null || v.Definition.ValueKind != JsonValueKind.Object || !v.Definition.TryGetProperty("version", out var version)
                    || !version.TryGetInt32(out var number) || number != 2) throw new InvalidOperationException("보기 정의는 판 2여야 합니다.");
                var input = ParseViewInput(JsonSerializer.Serialize(v, Flow.Json));
                var name = CleanViewName(input.Name, "보기 이름");
                if (name.Length == 0 || input.Columns.Count == 0) throw new InvalidOperationException("보기 이름과 필드가 필요합니다.");
                var seenNames = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
                var ids = new HashSet<string>();
                var columns = input.Columns.Select(c =>
                {
                    var field = CleanViewName(c.Name, "필드 이름");
                    if (field.Length == 0 || !seenNames.Add(field)) throw new InvalidOperationException("필드 이름이 비었거나 겹칩니다.");
                    return (JsonNode)new JsonObject { ["id"] = c.Id is { Length: > 0 and <= 100 } id && !id.Any(char.IsControl) && ids.Add(id) ? id : NewColumnId(ids),
                        ["name"] = field, ["formula"] = StoredFormula(Flow.Parse(c.Formula)) };
                }).ToArray();
                var importName = CopyViewName(name, taken);
                var definition = new JsonObject { ["version"] = 2, ["contracts"] = input.Fold ? "fold" : "split", ["columns"] = new JsonArray(columns) }.ToJsonString(Flow.Json);
                var viewId = Guid.NewGuid().ToString("N");
                connection.Execute("""
                    INSERT INTO user_view(id,name,definition,sort_order)
                    VALUES(@viewId,@importName,@definition,coalesce((SELECT max(sort_order) FROM user_view),0)+10)
                    """, new { viewId, importName, definition }, transaction);
                viewIds.Add(viewId);
                plannedViews.Add(new(name, importName));
            }
            catch (Exception e) when (e is InvalidOperationException or SqliteException or ArgumentException)
            { errors.Add($"{v?.Name}: {e.Message}"); }
        }
        if (errors.Count == 0)
        {
            try
            {
                var broken = Database.Refresh(connection, transaction);
                foreach (var c in package.Columns)
                    if (broken.Columns.TryGetValue((c.EntityType, c.FieldName), out var why)) errors.Add($"{c.EntityType}.{c.FieldName}: {why}");
                foreach (var b in broken.UserViews.Where(b => viewIds.Contains(b.Key.View))) errors.Add($"보기 필드: {b.Value}");
                // Same scalar row-count check as normal saves, against the staged definitions.
                connection.Execute("PRAGMA query_only=ON", transaction: transaction);
                try
                {
                    foreach (var c in package.Columns.Where(c => c.Kind == "computed"))
                    {
                        var f = Flow.Parse(c.Formula!.Value.GetRawText());
                        if (f.Source == "sql") SqlColumn.Validate(connection, c.EntityType, f.Sql!, []);
                    }
                    foreach (var v in UserViewBuilder.Read(connection, transaction).Where(v => viewIds.Contains(v.Id)))
                    {
                        var frame = UserViewBuilder.Frame(v.Fold, v.Columns.Select(c => c.Formula));
                        foreach (var c in v.Columns)
                        {
                            var f = Flow.Parse(c.Formula);
                            if (f.Source == "sql") SqlColumn.Validate(connection, f.Sql!, frame.Sql, []);
                        }
                    }
                }
                finally { connection.Execute("PRAGMA query_only=OFF", transaction: transaction); }
            }
            catch (Exception e) when (e is InvalidOperationException or SqliteException) { errors.Add(e.Message); }
        }
        var stamp = JsonSerializer.Serialize(new { package, plannedViews, plannedColumns, errors }, Flow.Json);
        return new(plannedViews, plannedColumns, errors, package.IncludesAllColumnsForSql,
            Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(stamp))));
    }

    /// <summary>
    /// 받는 쪽 열과 정의가 다른 맞춤 열. 받는 사람이 할 수 있는 일(내 열 이름 바꾸기)을 먼저, 보낸 사람에게 부탁하는 길을 다음에 둔다.
    /// </summary>
    private static string ConflictMessage(string entityType, string mine)
    {
        var kind = entityType switch { "request" => "접수", "notice" => "공고", _ => "계약" };
        var c = mine.Length == 0 ? ' ' : mine[^1];
        var batchim = c is >= '가' and <= '힣' ? (c - '가') % 28 != 0 : c is '0' or '1' or '3' or '6' or '7' or '8';
        return $"내 {kind} 열 「{mine}」 {(batchim ? "과" : "와")} 정의가 다릅니다. 열 관리에서 내 열 이름을 바꾸면 이 구성을 가져올 수 있습니다. " +
            "보낸 사람에게 열 이름을 바꿔 다시 내보내 달라고 해도 됩니다.";
    }
}

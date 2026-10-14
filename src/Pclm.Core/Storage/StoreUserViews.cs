using System.Text.Json;
using System.Text.Json.Nodes;
using Dapper;
using Microsoft.Data.Sqlite;

namespace Pclm.Core.Storage;

/// <summary>
/// 맞춤 뷰의 다리 쪽(docs/user-views.md §8) — 목록·필드 목록 재료·보표·표본·미리보기·저장·지우기. 읽는 것은 모두 읽기 전용
/// 연결에서 한다 — 열람 창도 같은 길이다. 줄은 조달 건이고(<see cref="UserViewFrame"/>), 표본 하나도 조달 건 하나다.
/// </summary>
public sealed partial class Store
{
    /// <summary>뷰 이름·필드 이름의 길이. 화면의 입력 칸과 같다.</summary>
    private const int UserViewNameLength = 40;

    /// <summary>표본 고르개의 가장자리 — 계약 여럿·연결 안 된 건.</summary>
    public static readonly IReadOnlySet<string> UserViewEdges = new HashSet<string>(StringComparer.Ordinal) { "manyContracts", "unlinked" };

    /// <summary>목록의 한 줄 — 화면의 <c>UserView</c>. <see cref="Broken"/> 은 필드 식별자 → 까닭.</summary>
    public sealed record UserViewInfo(string Id, string Name, JsonElement Definition, IReadOnlyDictionary<string, string> Broken);

    // ── 목록 ─────────────────────────────────────────────

    /// <summary>
    /// 저장된 맞춤 뷰와 그 깨진 필드. 깨진 까닭은 지어 둔 뷰의 정의 글에 남아 있어(<see cref="Views.BrokenIn"/>) 필드 이름으로
    /// 찾고 필드 식별자로 돌려 준다 — 남의 파일을 읽기로 열어도 같은 것이 보인다.
    /// </summary>
    public IReadOnlyList<UserViewInfo> UserViews()
    {
        using var connection = _database.OpenReadOnly();
        using var transaction = connection.BeginTransaction(deferred: true);
        return ListUserViews(connection);
    }

    private static List<UserViewInfo> ListUserViews(SqliteConnection connection)
    {
        var built = connection.Query<(string Name, string Sql)>(
                "SELECT name, sql FROM sqlite_master WHERE type = 'view' AND name LIKE 'u\\_%' ESCAPE '\\'")
            .ToDictionary(v => v.Name, v => v.Sql, StringComparer.Ordinal);
        return [.. UserViewBuilder.Read(connection).Select(view =>
        {
            var broken = new Dictionary<string, string>(StringComparer.Ordinal);
            var why = Views.BrokenIn(built.GetValueOrDefault(view.ViewName));
            foreach (var column in view.Columns)
                if (why.TryGetValue(column.Name, out var reason))
                    broken[column.Id] = reason.Length == 0 ? "식을 계산하지 못했습니다." : reason;
            JsonElement definition;
            try { definition = JsonDocument.Parse(view.Definition).RootElement.Clone(); }
            catch (JsonException) { definition = JsonDocument.Parse("""{"version":2,"contracts":"split","columns":[]}""").RootElement.Clone(); }
            return new UserViewInfo(view.Id, view.Name, definition, broken);
        })];
    }

    // ── 필드 목록 재료 ─────────────────────────────────────

    /// <summary>계약마다 한 줄 — 사슬 그대로, 거르지 않는다.</summary>
    private static readonly UserViewFrame AllChains = new(false, new HashSet<string>());

    /// <summary>계약이 여럿인 건을 한 줄로 모은 사슬, 거르지 않는다.</summary>
    private static readonly UserViewFrame AllFolded = new(true, new HashSet<string>());

    /// <summary>
    /// <c>userViewContext</c> — <see cref="ColumnContext"/> 와 같은 꼴이되 「이 행」 이 없다: <c>available.linked</c> 가 접수·공고·계약
    /// 셋(차례대로)이고 저마다 면 뷰의 열(사람 열·가공열 포함)과 품목 필드를 든다. <c>total</c> 은 조달 건 수(계약마다 한 줄),
    /// <c>manyContracts</c> 는 계약이 여럿인 건 수다.
    /// </summary>
    public object UserViewContext()
    {
        using var connection = _database.OpenReadOnly();
        using var transaction = connection.BeginTransaction(deferred: true);
        var env = UserViewBuilder.Env(connection);
        var total = connection.ExecuteScalar<long>($"{Views.사슬}\nSELECT count(*) FROM 사슬");
        var many = connection.ExecuteScalar<long>(
            $"{Views.사슬}\nSELECT count(*) FROM (SELECT 1 FROM 사슬 WHERE 공고 IS NOT NULL GROUP BY 접수, 공고 HAVING count(계약) >= 2)");
        return new
        {
            columns = Array.Empty<string>(), rows = Array.Empty<object>(), fields = Array.Empty<string>(), sources = Array.Empty<object>(),
            available = new
            {
                view = "",
                @base = env.Base,
                items = env.Items,
                linked = env.Linked.Select(l => new { kind = l.Key, fields = l.Value.Fields, items = l.Value.Items }).ToList(),
                schema = SqlColumn.Readable.ToDictionary(v => v, v => ViewColumns(connection, v)),
            },
            total,
            manyContracts = many,
        };
    }

    // ── 보표 ─────────────────────────────────────────────

    /// <summary>
    /// 필드 하나의 값 흐름 보표 — <see cref="TraceComputedColumn"/> 을 맞춤 뷰 환경으로. 표본은 <paramref name="sampleKeys"/>
    /// (미리보기가 보는 표본이 처음)이고, 없으면 처음 조달 건 셋이다. 단계마다의 값은 계약 한 건을 본다 — 모은 건이면 계약본번호가
    /// 가장 앞선 계약이다(꼴 검사가 언제나 계약 한 건을 보는 것과 같다). 열에 찍힐 값은 뷰가 낼 값 그대로다(모았으면 이어 붙인 것).
    /// </summary>
    public object TraceUserViewColumn(string json, IReadOnlyList<string>? sampleKeys)
    {
        var formula = Flow.Parse(json);
        using var connection = SqlColumn.OpenChecked(_database);
        using var transaction = connection.BeginTransaction(deferred: true);
        var env = UserViewBuilder.Env(connection);
        var contract = UserViewBuilder.Kinds(json).Contains("contract");
        var keys = sampleKeys is { Count: > 0 } given ? [.. given.Take(TraceSamples)]
            : connection.Query<string>(AllChains.Statement(UserViewFrame.Key, null, $"ORDER BY {ChainOrder} LIMIT {TraceSamples}")).ToList();

        // 이 열쇠가 계약마다 한 줄의 것인가 — 아니면 모은 건(계약 자리가 빈 열쇠)이다.
        bool Split(string key) => connection.ExecuteScalar<long>(
            AllChains.Statement("count(*)", $"{UserViewFrame.Key} = @key"), new { key }) > 0;
        string Row(string select) => AllChains.Statement(select,
            $"{UserViewFrame.Key} = @key OR ({ChainPrefix} = @key AND s.계약 = (SELECT min(x.계약) FROM 사슬 x WHERE x.공고 = s.공고 AND x.접수 IS s.접수))",
            "LIMIT 1");
        string Final(string expression) =>
            $"SELECT coalesce(({AllChains.Statement(expression, $"{UserViewFrame.Key} = @key", "LIMIT 1")}), " +
            $"({AllFolded.Statement(AllFolded.Column(expression, contract), $"{UserViewFrame.Key} = @key", "LIMIT 1")}))";
        IReadOnlyList<string> Sql(SqliteConnection c, string sql, IReadOnlyList<string> keys)
        {
            var split = keys.Where(Split).ToHashSet(StringComparer.Ordinal);
            var a = SqlColumn.Validate(c, sql, AllChains.Sql, [.. keys.Where(split.Contains)]);
            var b = SqlColumn.Validate(c, sql, AllFolded.Sql, [.. keys.Where(k => !split.Contains(k))]);
            int ia = 0, ib = 0;
            return [.. keys.Select(k => split.Contains(k) ? a[ia++] : b[ib++])];
        }

        // 표본 머리의 품목 수 — 원천이 문서의 품목이면 그 문서의 것, 아니면 0.
        var items = formula is { Source: "flow", Flow.From: { Kind: "linked", Link: { } link } }
            ? FlowCompiler.Prefix("", new FlowDefinition(new FlowSource("linked", Link: link, Items: true), []), 0, FlowCompiler.Target.Internal, env).Sql
            : null;
        var samples = new List<Sample>();
        var heads = new List<object>();
        foreach (var key in keys)
        {
            var head = connection.QuerySingleOrDefault<(string Number, string Title, long Rows)?>(
                Row($"{UserViewFrame.Number}, {UserViewFrame.Title}, {(items is null ? "0" : $"(SELECT count(*) FROM ({items}))")}"), new { key });
            if (head is not { } h) continue;
            samples.Add(new Sample(h.Number, "", h.Title, key));
            heads.Add(new { @base = h.Number, revision = "", title = h.Title, rows = h.Rows });
        }
        return Trace(connection, "", formula, env, samples, heads, new TraceFrame(Row, Final, Sql));
    }

    /// <summary>조달 건의 차례 — 공고건, 접수, 계약 본번호(없는 것은 뒤로).</summary>
    private const string ChainOrder = "s.공고 IS NULL, s.공고, s.접수 IS NULL, s.접수, s.계약 IS NULL, s.계약";

    /// <summary>열쇠에서 계약 자리를 뺀 것 — 모은 건의 열쇠와 같다.</summary>
    private const string ChainPrefix = "(coalesce(s.접수, '') || '|' || coalesce(s.공고, '') || '|')";

    // ── 표본 고르개 ─────────────────────────────────────────

    /// <summary>
    /// 표본 고르개(§7.4) — <b>그 뷰의 줄</b>(거르기·모으기 뒤)에서 조달 건의 열쇠·보이는 번호·제목. <paramref name="query"/> 는
    /// 번호와 제목에서 찾고, <paramref name="edge"/> 는 <c>manyContracts</c>(계약이 둘 이상 이어진 건)·<c>unlinked</c>(접수·공고·계약
    /// 중 빠진 단계가 있는 건)다. 필드가 없으면 모든 조달 건이다.
    /// </summary>
    public IReadOnlyList<object> UserViewSamples(string viewJson, string? query, string? edge)
    {
        if (edge is not null && !UserViewEdges.Contains(edge)) throw new InvalidOperationException($"모르는 표본 가장자리입니다: {edge}");
        var input = ParseViewInput(viewJson);
        var frame = UserViewBuilder.Frame(input.Fold, input.Columns.Select(c => c.Formula));
        var where = new List<string>();
        var q = query?.Trim() ?? "";
        if (q.Length > 0)
            where.Add($"instr(lower({UserViewFrame.Number}), lower(@q)) > 0 OR instr(lower({UserViewFrame.Title}), lower(@q)) > 0");
        if (edge == "manyContracts") where.Add(frame.ManyContracts);
        if (edge == "unlinked") where.Add(frame.Unlinked);

        using var connection = _database.OpenReadOnly();
        return [.. connection.Query<(string Key, string Number, string Title)>(frame.Statement(
                $"{UserViewFrame.Key}, {UserViewFrame.Number}, {UserViewFrame.Title}",
                where.Count == 0 ? null : string.Join(" AND ", where.Select(w => $"({w})")),
                $"ORDER BY {ChainOrder}"), new { q })
            .Select(r => (object)new { key = r.Key, number = r.Number, title = r.Title })];
    }

    // ── 미리보기 ─────────────────────────────────────────

    /// <summary>화면이 보낸 뷰 — <c>{"name","definition":{"version":2,"contracts","columns":[{"id","name","formula"}]}}</c>.</summary>
    private sealed record ViewInput(string Name, bool Fold, IReadOnlyList<(string? Id, string Name, string Formula)> Columns);

    private static ViewInput ParseViewInput(string json)
    {
        JsonNode? root;
        try { root = JsonNode.Parse(json); }
        catch (JsonException) { throw new InvalidOperationException("맞춤 뷰 형식이 올바르지 않습니다."); }
        if (root is not JsonObject view) throw new InvalidOperationException("맞춤 뷰 형식이 올바르지 않습니다.");
        string Text(JsonNode? node) => node is JsonValue v && v.TryGetValue<string>(out var t) ? t : "";
        var definition = view["definition"] as JsonObject;
        var contracts = Text(definition?["contracts"]);
        if (contracts is not ("" or "split" or "fold")) throw new InvalidOperationException($"계약이 여럿인 건의 처리가 올바르지 않습니다: {contracts}");
        var columns = definition?["columns"] as JsonArray ?? [];
        return new ViewInput(Text(view["name"]), contracts == "fold", [.. columns.Select(c => c is JsonObject o
            ? (o["id"] is null ? null : Text(o["id"]), Text(o["name"]), o["formula"]?.ToJsonString() ?? "")
            : throw new InvalidOperationException("맞춤 뷰 형식이 올바르지 않습니다."))]);
    }

    /// <summary>
    /// 저장하지 않은 정의로 그 표본 한 건의 값과 필드마다 그 뷰의 전체 건 중 빈 값 수, 그리고 뷰의 건 수(거르기·모으기 뒤)(§8).
    /// 뷰를 짓지 않는다 — 맞춤 뷰와 <b>같은 SELECT</b>(<see cref="UserViewFrame.Statement"/>)를 읽기 전용 연결에서 돌린다. 필드마다
    /// 따로 컴파일하고 돌려 보아 깨진 필드는 그 까닭을 <c>error</c> 에 싣고 값은 빈 글로 둔다. 이름은 보지 않는다 — 필드 목록의 후보도
    /// 이 길로 값을 센다.
    /// </summary>
    public object PreviewUserView(string viewJson, string? sampleKey)
    {
        var input = ParseViewInput(viewJson);
        var frame = UserViewBuilder.Frame(input.Fold, input.Columns.Select(c => c.Formula));

        using var connection = SqlColumn.OpenChecked(_database);
        using var transaction = connection.BeginTransaction(deferred: true);
        var env = UserViewBuilder.Env(connection);
        var expressions = new List<string?>();
        var errors = new List<string?>();
        foreach (var column in input.Columns)
        {
            try
            {
                var (expression, _) = UserViewBuilder.Compile(connection, transaction, column.Formula, env, frame, toggleQueryOnly: false);
                connection.Query(frame.Statement(expression, null, "LIMIT 20"), transaction: transaction).ToList();
                expressions.Add(expression);
                errors.Add(null);
            }
            catch (Exception e) when (e is InvalidOperationException or SqliteException)
            {
                expressions.Add(null);
                errors.Add(e.Message.Length == 0 ? "식을 계산하지 못했습니다." : e.Message);
            }
        }

        var total = connection.ExecuteScalar<long>(frame.Statement("count(*)"), transaction: transaction);
        var head = string.IsNullOrEmpty(sampleKey) ? null : connection.QuerySingleOrDefault<(string Number, string Title)?>(
            frame.Statement($"{UserViewFrame.Number}, {UserViewFrame.Title}", $"{UserViewFrame.Key} = @sampleKey", "LIMIT 1"),
            new { sampleKey }, transaction);

        var live = expressions.Select((e, i) => (e, i)).Where(x => x.e is not null).ToList();
        var values = new string[input.Columns.Count];
        var blanks = new long[input.Columns.Count];
        Array.Fill(values, "");
        if (live.Count > 0)
        {
            var select = string.Join(",\n", live.Select(x => $"{x.e} AS c{x.i}"));
            if (head is not null)
            {
                using var reader = connection.ExecuteReader(
                    frame.Statement(select, $"{UserViewFrame.Key} = @sampleKey", "LIMIT 1"), new { sampleKey }, transaction);
                if (reader.Read())
                    for (var j = 0; j < live.Count; j++) values[live[j].i] = reader.IsDBNull(j) ? "" : Convert.ToString(reader.GetValue(j)) ?? "";
            }
            using var counted = connection.ExecuteReader(
                $"SELECT {string.Join(", ", live.Select(x => $"coalesce(sum(c{x.i} = ''), 0)"))} FROM ({frame.Statement(select)})",
                transaction: transaction);
            if (counted.Read())
                for (var j = 0; j < live.Count; j++) blanks[live[j].i] = counted.GetInt64(j);
        }

        return new
        {
            sample = head is { } h ? new { key = sampleKey, number = h.Number, title = h.Title } : null,
            total,
            columns = input.Columns.Select((c, i) =>
            {
                var row = new Dictionary<string, object?> { ["id"] = c.Id ?? "", ["value"] = values[i], ["blank"] = blanks[i] };
                if (errors[i] is { } why) row["error"] = why;
                return row;
            }).ToList(),
        };
    }

    // ── 저장·지우기 ─────────────────────────────────────────

    private static string CleanViewName(string name, string what)
    {
        var clean = name.Trim();
        if (clean.Length > UserViewNameLength) throw new InvalidOperationException($"{what}은 {UserViewNameLength}자까지입니다.");
        if (clean.Any(ch => ch == '"' || char.IsControl(ch))) throw new InvalidOperationException($"{what}에 큰따옴표를 쓸 수 없습니다.");
        return clean;
    }

    /// <summary>
    /// 맞춤 뷰를 저장한다(§5). <b>화면을 믿지 않고 다시 검사한다</b>: 이름(비지 않음·겹치지 않음·큰따옴표 없음) → 필드 이름(비지 않음·
    /// 뷰 안에서 겹치지 않음) → 필드마다 꼴 검사·컴파일(SQL 필드는 읽기 전용 연결에서 검증) → 같은 트랜잭션에서 뷰를 다시 지어
    /// 이 뷰의 필드가 깨진 식으로 서면 거절 → 표본 실행. 하나라도 실패하면 되돌린다. 뷰 식별자는 처음 저장할 때 뽑고 이름을 바꿔도
    /// 남는다. 필드 식별자는 화면이 보낸 것을 두되, 비었거나 겹치면 새로 뽑는다.
    /// </summary>
    public IReadOnlyList<UserViewInfo> SaveUserView(string viewJson, string? previousId)
    {
        var input = ParseViewInput(viewJson);
        var name = CleanViewName(input.Name, "뷰 이름");
        if (name.Length == 0) throw new InvalidOperationException("뷰 이름이 비었습니다.");
        if (input.Columns.Count == 0) throw new InvalidOperationException("필드를 하나 이상 추가하세요.");

        var seenNames = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        var seenIds = new HashSet<string>(StringComparer.Ordinal);
        var columns = new List<(string Id, string Name, FlowFormula Formula)>();
        foreach (var column in input.Columns)
        {
            var columnName = CleanViewName(column.Name, "필드 이름");
            if (columnName.Length == 0) throw new InvalidOperationException("이름이 빈 필드가 있습니다.");
            if (!seenNames.Add(columnName)) throw new InvalidOperationException($"필드 이름이 겹칩니다: {columnName}");
            FlowFormula formula;
            try { formula = Flow.Parse(column.Formula); }
            catch (InvalidOperationException e) { throw new InvalidOperationException($"‘{columnName}’ 필드를 세울 수 없습니다: {e.Message}"); }
            var id = column.Id is { Length: > 0 and <= 100 } given && !given.Any(char.IsControl) && seenIds.Add(given)
                ? given : NewColumnId(seenIds);
            columns.Add((id, columnName, formula));
        }
        var frame = UserViewBuilder.Frame(input.Fold, input.Columns.Select(c => c.Formula));

        // SQL 필드의 읽기 권한·모양·표본은 쓰기 전에 읽기 전용 연결에서 본다(가공열과 같다).
        using (var reader = SqlColumn.OpenChecked(_database))
        {
            var env = UserViewBuilder.Env(reader);
            foreach (var (_, columnName, formula) in columns)
            {
                try
                {
                    if (formula.Source == "sql") SqlColumn.Validate(reader, formula.Sql!, frame.Sql, []);
                    else _ = FlowCompiler.Expression("", formula, FlowCompiler.Target.Internal, env);
                }
                catch (Exception e) when (e is InvalidOperationException or SqliteException)
                {
                    throw new InvalidOperationException($"‘{columnName}’ 필드를 세울 수 없습니다: {e.Message}");
                }
            }
        }

        var definition = new JsonObject
        {
            ["version"] = 2,
            ["contracts"] = input.Fold ? "fold" : "split",
            ["columns"] = new JsonArray([.. columns.Select(c => (JsonNode)new JsonObject
            {
                ["id"] = c.Id,
                ["name"] = c.Name,
                ["formula"] = StoredFormula(c.Formula),
            })]),
        }.ToJsonString(Flow.Json);

        using var connection = _database.Open();
        using var transaction = connection.BeginTransaction();
        var existing = connection.Query<(string Id, string Name)>("SELECT id, name FROM user_view", transaction: transaction).ToList();
        if (existing.Any(v => v.Id != previousId && string.Equals(v.Name, name, StringComparison.OrdinalIgnoreCase)))
            throw new InvalidOperationException($"‘{name}’ 맞춤 뷰가 이미 있습니다.");
        string viewId;
        if (previousId is not null)
        {
            if (!existing.Any(v => v.Id == previousId)) throw new InvalidOperationException("그런 맞춤 뷰가 없습니다.");
            viewId = previousId;
            connection.Execute("UPDATE user_view SET name = @name, definition = @definition WHERE id = @viewId",
                new { name, definition, viewId }, transaction);
        }
        else
        {
            viewId = Guid.NewGuid().ToString("N");
            connection.Execute("""
                INSERT INTO user_view (id, name, definition, sort_order)
                VALUES (@viewId, @name, @definition, coalesce((SELECT max(sort_order) FROM user_view), 0) + 10)
                """, new { viewId, name, definition }, transaction);
        }

        try
        {
            var (_, broken) = Database.Refresh(connection, transaction);
            foreach (var (id, columnName, _) in columns)
                if (broken.TryGetValue((viewId, id), out var why))
                    throw new InvalidOperationException($"‘{columnName}’ 필드를 세울 수 없습니다: {why}");
            connection.Query($"SELECT * FROM {FlowCompiler.Ident(Views.UserViewPrefix + name)} LIMIT {ComputedSample}", transaction: transaction).ToList();
        }
        catch (SqliteException error)
        {
            throw new InvalidOperationException($"맞춤 뷰를 짓지 못했습니다: {error.Message}");
        }
        transaction.Commit();
        return UserViews();
    }

    private static string NewColumnId(HashSet<string> taken)
    {
        string id;
        do id = "c-" + Guid.NewGuid().ToString("N");
        while (!taken.Add(id));
        return id;
    }

    /// <summary>저장 꼴의 필드 식(§4) — 가공식 v2 그대로이되 식 식별자는 싣지 않는다(필드 식별자가 그 자리다).</summary>
    private static JsonNode StoredFormula(FlowFormula formula)
    {
        var node = JsonNode.Parse(Flow.Serialize(formula))!.AsObject();
        node.Remove("id");
        return node;
    }

    /// <summary>맞춤 뷰를 지운다. 자료가 아니라 정의라 <see cref="PlanDeletion"/> 대상이 아니다 — 화면이 이름을 보이고 묻는다.</summary>
    public IReadOnlyList<UserViewInfo> DeleteUserView(string id)
    {
        using (var connection = _database.Open())
        using (var transaction = connection.BeginTransaction())
        {
            if (connection.Execute("DELETE FROM user_view WHERE id = @id", new { id }, transaction) == 0)
                throw new InvalidOperationException("그런 맞춤 뷰가 없습니다.");
            Database.RefreshViews(connection, transaction);
            transaction.Commit();
        }
        return UserViews();
    }

    /// <summary>
    /// <paramref name="entityType"/> 의 사람 열을 쓰는 맞춤 뷰 수 — 열 이름 → 뷰 수. 열을 지우는 확인 문구가
    /// 「맞춤 뷰 n곳이 이 열을 씁니다」 로 센다. 지우면 그 필드들은 깨진 식(빈 글)으로 선다.
    /// </summary>
    public IReadOnlyDictionary<string, int> UserViewReaders(string entityType)
    {
        using var connection = _database.OpenReadOnly();
        return UserViewBuilder.Readers(connection, entityType);
    }
}

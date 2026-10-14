using System.Text.Json;
using System.Text.Json.Nodes;
using Dapper;
using Microsoft.Data.Sqlite;

namespace Pclm.Core.Storage;

/// <summary>맞춤 뷰의 필드 하나 — 이름 = 흐름식(docs/user-views.md §2). <see cref="Formula"/> 는 가공식 v2 JSON 그대로다.</summary>
public sealed record UserViewColumn(string Id, string Name, string Formula);

/// <summary>
/// 저장된 맞춤 뷰 하나(<c>user_view</c> 한 줄). <see cref="Definition"/> 은 저장된 JSON 원문이고 <see cref="Columns"/>·<see cref="Fold"/> 는
/// 그것을 읽은 것이다 — 손으로 고쳐 읽지 못하는 정의는 필드가 없는 것으로 읽는다(앱이 뜨지 못하게 하지 않는다).
/// </summary>
public sealed record UserViewDefinition(string Id, string Name, string Definition, long SortOrder,
    IReadOnlyList<UserViewColumn> Columns, bool Fold)
{
    /// <summary>SQLite 에 서는 이름 — <c>u_&lt;name&gt;</c>.</summary>
    public string ViewName => Views.UserViewPrefix + Name;
}

/// <summary>
/// 맞춤 뷰의 줄이 서는 꼴(docs/user-views.md §2·§5). 줄은 <b>조달 건</b> — <c>v_통합</c> 과 같은 사슬(<see cref="Views.사슬"/>)이고,
/// 같은 건의 접수·공고·계약 면 뷰를 별칭 <c>접수</c>·<c>공고</c>·<c>계약</c> 으로 건다. 공고는 그 건의 현행 공고다.
///
/// <list type="bullet">
/// <item><see cref="Fold"/> — 계약이 여럿인 건을 (접수, 공고건)으로 접는다. 계약 필드는 계약마다 셈해 계약본번호 차례로
///   <c>", "</c> 로 잇는다(<see cref="Column"/>). 공고건이 없는 사슬(접수만·계약만)은 접지 않는다.</item>
/// <item><see cref="Kinds"/> — 담긴 흐름식 필드가 읽는 문서 종류. 그중 하나도 없는 건은 줄이 서지 않는다. 비었으면 거르지 않는다.</item>
/// </list>
/// </summary>
public sealed record UserViewFrame(bool Fold, IReadOnlySet<string> Kinds)
{
    /// <summary>조달 건의 불투명 열쇠 — 접수·공고건·계약 본번호를 <c>|</c> 로 이은 것. 접힌 건은 계약 자리가 빈다.</summary>
    public const string Key = "(coalesce(s.접수, '') || '|' || coalesce(s.공고, '') || '|' || coalesce(s.계약, ''))";

    /// <summary>보이는 번호 — 입찰공고번호 → 접수번호 → 계약번호 중 처음 있는 것(§7.4).</summary>
    public const string Number =
        "coalesce(nullif(coalesce(공고.\"입찰공고번호\", ''), ''), nullif(coalesce(접수.\"접수번호\", ''), ''), coalesce(계약.\"계약번호\", ''))";

    /// <summary>제목 — 공고명 → 요청명 → 계약건명 중 처음 있는 것(§7.4).</summary>
    public const string Title =
        "coalesce(nullif(coalesce(공고.\"공고명\", ''), ''), nullif(coalesce(접수.\"요청명\", ''), ''), coalesce(계약.\"계약건명\", ''))";

    /// <summary>계약이 둘 이상 이어진 건인가 — 표본 고르개의 「계약 여럿」.</summary>
    public string ManyContracts => Fold ? "s.계약수 >= 2"
        : "(SELECT count(x.계약) FROM 사슬 x WHERE x.공고 = s.공고 AND x.접수 IS s.접수) >= 2";

    /// <summary>접수·공고·계약 중 빠진 단계가 있는 건인가 — 표본 고르개의 「연결 안 된 건」.</summary>
    public string Unlinked => $"(s.접수 IS NULL OR s.공고 IS NULL OR {(Fold ? "s.계약수 = 0" : "s.계약 IS NULL")})";

    /// <summary>
    /// 문장 하나. 고른 것(<paramref name="select"/>)을 조달 건마다 낸다 — 거르기(<see cref="Kinds"/>)와 <paramref name="where"/> 를
    /// 함께 걸고 끝에 <paramref name="tail"/>(<c>ORDER BY</c>·<c>LIMIT</c>)을 붙인다.
    /// </summary>
    public string Statement(string select, string? where = null, string tail = "")
    {
        var filters = new List<string>();
        if (Kinds.Count > 0)
            filters.Add("(" + string.Join(" OR ", Kinds.Order(StringComparer.Ordinal).Select(kind => kind switch
            {
                "request" => "s.접수 IS NOT NULL",
                "notice" => "s.공고 IS NOT NULL",
                _ => Fold ? "s.계약수 > 0" : "s.계약 IS NOT NULL",
            })) + ")");
        if (where is not null) filters.Add($"({where})");
        return $"""
            {Views.사슬}{(Fold ? 접힌 : "")}
            SELECT
                {select}
            FROM {(Fold ? "접힌" : "사슬")} s
            LEFT JOIN v_접수 AS 접수 ON 접수.접수본번호 = s.접수
            LEFT JOIN v_공고 AS 공고 ON 공고.공고본번호 = {Views.건현행("s.공고", "현행.notice_base")}
            LEFT JOIN v_계약 AS 계약 ON 계약.계약본번호 = s.계약
            {(filters.Count == 0 ? "" : "WHERE " + string.Join(" AND ", filters))}
            {tail}
            """;
    }

    /// <summary>
    /// 접은 사슬. 공고건이 있는 것은 (접수, 공고건)마다 한 줄이고, 계약이 하나뿐이면 그 계약을 그대로 들고 간다 — 둘 이상일 때만
    /// 계약 자리가 비고 계약 필드가 <see cref="Column"/> 의 부분질의로 잇는다. 공고건이 없는 사슬은 그대로다.
    /// </summary>
    private const string 접힌 = """
        ,
        접힌(접수, 공고, 계약, 계약수) AS (
            SELECT 접수, 공고, CASE WHEN count(계약) = 1 THEN max(계약) END, count(계약)
            FROM 사슬 WHERE 공고 IS NOT NULL GROUP BY 접수, 공고
            UNION ALL
            SELECT 접수, 공고, 계약, CASE WHEN 계약 IS NULL THEN 0 ELSE 1 END
            FROM 사슬 WHERE 공고 IS NULL
        )
        """;

    /// <summary>
    /// 필드 하나의 칸. 모으기에서 계약을 읽는 필드는 그 건의 계약마다 식을 셈해 계약본번호 차례로 <c>', '</c> 로 잇는다 — 꼴 검사는
    /// 언제나 계약 한 건을 보므로 계약마다 한 줄과 모으기 사이를 오가도 깨지는 필드가 없다. 계약이 하나 이하면 그대로 셈한다.
    /// </summary>
    public string Column(string expression, bool readsContract) => !Fold || !readsContract ? expression : $"""
        (CASE WHEN s.계약수 <= 1 THEN {expression} ELSE coalesce((
            SELECT group_concat({expression}, ', ' ORDER BY x.계약)
            FROM 사슬 x LEFT JOIN v_계약 AS 계약 ON 계약.계약본번호 = x.계약
            WHERE x.공고 = s.공고 AND x.접수 IS s.접수 AND x.계약 IS NOT NULL), '') END)
        """;

    /// <summary>SQL 필드가 서는 자리(<see cref="SqlColumn.Frame"/>).</summary>
    public SqlColumn.Frame Sql => new(Key, (select, where, tail) => Statement(select, where, tail));
}

/// <summary>
/// 맞춤 뷰(docs/user-views.md) — 조달 건마다 한 줄, 흐름식 필드를 늘어놓은 뷰. 정의를 읽고, 뷰를 짓고, 접수·공고·계약의 사람 열
/// 이름이 바뀔 때 정의를 따라 고쳐 쓴다. 저장과 미리보기는 <see cref="Store"/> 가 한다.
/// </summary>
public static class UserViewBuilder
{
    /// <summary>정의 JSON 의 필드. 읽지 못하면 빈 목록이다.</summary>
    public static IReadOnlyList<UserViewColumn> ParseColumns(string definition)
    {
        try
        {
            if (JsonNode.Parse(definition)?["columns"] is not JsonArray columns) return [];
            return [.. columns.OfType<JsonObject>().Select(c => new UserViewColumn(
                Text(c["id"]), Text(c["name"]), c["formula"]?.ToJsonString() ?? ""))];
        }
        catch (JsonException) { return []; }
    }

    /// <summary>계약이 여럿인 건을 한 줄로 모으는가(<c>contracts</c> 가 <c>fold</c>). 읽지 못하면 계약마다 한 줄이다.</summary>
    public static bool ParseFold(string definition)
    {
        try { return Text(JsonNode.Parse(definition)?["contracts"]) == "fold"; }
        catch (JsonException) { return false; }
    }

    private static string Text(JsonNode? node) =>
        node is JsonValue value && value.TryGetValue<string>(out var text) ? text : "";

    /// <summary>저장된 맞춤 뷰 전부. 차례는 <c>sort_order</c>, 이름.</summary>
    public static IReadOnlyList<UserViewDefinition> Read(SqliteConnection connection, SqliteTransaction? transaction = null) =>
        [.. connection.Query<(string Id, string Name, string Definition, long SortOrder)>(
                "SELECT id, name, definition, sort_order FROM user_view ORDER BY sort_order, name;",
                transaction: transaction)
            .Select(v => new UserViewDefinition(v.Id, v.Name, v.Definition, v.SortOrder, ParseColumns(v.Definition), ParseFold(v.Definition)))];

    /// <summary>접수·공고·계약의 사람 열·가공열 이름(종류 → 차례대로). 맞춤 뷰의 필드는 이것까지 읽는다(§3).</summary>
    public static IReadOnlyDictionary<string, IReadOnlyList<string>> Own(IEnumerable<UserColumnDefinition> columns)
    {
        var list = columns.ToList();
        return new[] { "request", "notice", "contract" }.ToDictionary(kind => kind, kind => (IReadOnlyList<string>)
            [.. list.Where(c => c.EntityType == kind).OrderBy(c => c.SortOrder).ThenBy(c => c.FieldName, StringComparer.Ordinal).Select(c => c.FieldName)]);
    }

    /// <summary>맞춤 뷰의 재료 — 세 문서의 면 뷰 열(사람 열·가공열 포함)과 품목 필드(<see cref="Flow.EnvForView"/>).</summary>
    public static FlowEnv Env(SqliteConnection connection, SqliteTransaction? transaction = null) =>
        Flow.EnvForView(Own(connection.Query<(string EntityType, string FieldName, long SortOrder)>(
                "SELECT entity_type, field_name, sort_order FROM user_column", transaction: transaction)
            .Select(c => new UserColumnDefinition(c.EntityType, c.FieldName, "", null, c.SortOrder))));

    /// <summary>
    /// 흐름식이 읽는 문서 종류 — 원천 <c>linked</c> 와 단계가 읽는 행의 열(<c>공고.공고명</c>, 조건 마디의 바깥 열 포함). SQL 필드와 읽지 못하는 식은 없다.
    /// 줄 거르기(§2)와 모으기의 계약 필드(<see cref="UserViewFrame.Column"/>)가 이것을 본다.
    /// </summary>
    public static IReadOnlySet<string> Kinds(string formulaJson)
    {
        var kinds = new HashSet<string>(StringComparer.Ordinal);
        FlowFormula formula;
        try { formula = Flow.Parse(formulaJson); }
        catch (InvalidOperationException) { return kinds; }
        if (formula.Source != "flow" || formula.Flow is null) return kinds;
        if (formula.Flow.From is { Kind: "linked", Link: { } link } && Flow.DocWords.ContainsKey(link)) kinds.Add(link);
        foreach (var step in formula.Flow.Steps)
        {
            if (step?.Field is { } field && RowFieldSteps.Contains(step.Op) && Flow.Qualified(field) is { } q) kinds.Add(q.Kind);
            // 값 하나를 견주는 조건의 마디가 읽는 바깥의 열(만약 공고.f = …).
            foreach (var clause in Flow.Clauses(step!))
                if (Flow.Qualified(clause.Field) is { } c) kinds.Add(c.Kind);
        }
        return kinds;
    }

    /// <summary>이 정의의 줄 꼴 — 모으기와, 흐름식 필드들이 읽는 문서 종류.</summary>
    public static UserViewFrame Frame(bool fold, IEnumerable<string> formulas) =>
        new(fold, formulas.SelectMany(Kinds).ToHashSet(StringComparer.Ordinal));

    /// <summary>
    /// 필드 하나의 칸과 끝 꼴. 흐름식은 맞춤 뷰 환경으로 꼴 검사·컴파일하고(모으기면 계약 필드를 잇는 칸으로 감싼다), SQL 필드는
    /// <c>query_only</c> 를 건 채 authorizer 로 그 자리(<paramref name="frame"/>) 위에서 준비해 본다. 실패하면 까닭을 담아 던진다.
    /// </summary>
    internal static (string Expression, string Shape) Compile(SqliteConnection connection, SqliteTransaction? transaction,
        string formulaJson, FlowEnv env, UserViewFrame frame, bool toggleQueryOnly)
    {
        var formula = Flow.Parse(formulaJson);
        if (formula.Source == "sql")
        {
            if (toggleQueryOnly) Execute(connection, transaction, "PRAGMA query_only = ON;");
            try { SqlColumn.Prepare(connection, transaction, formula.Sql!, frame.Sql); }
            finally { if (toggleQueryOnly) Execute(connection, transaction, "PRAGMA query_only = OFF;"); }
            return (SqlColumn.Expression(formula.Sql!), "text");
        }
        var expression = FlowCompiler.Expression("", formula, FlowCompiler.Target.Internal, env);
        var shape = FlowCompiler.FinalShape("", formula, env) switch { "number" => "number", "date" => "date", _ => "text" };
        return (frame.Column(expression, Kinds(formulaJson).Contains("contract")), shape);
    }

    /// <summary>
    /// 맞춤 뷰를 모두 짓는다 — 계약면 뷰를 다 지은 뒤에 부른다(그 위층이다). 치우는 것은 <see cref="DropAll"/> 이 먼저 했다.
    /// 필드마다 따로 컴파일하고 사슬 위에서 돌려 보아(<see cref="Database"/> 의 가공열 격리와 같은 길) 깨진 필드는 빈 글 +
    /// 까닭 주석으로 세운다. 다른 필드와 다른 뷰는 그대로 선다. 마지막에 <c>v_맞춤뷰</c> 를 결과타입과 함께 다시 짓는다.
    /// (뷰 식별자, 필드 식별자) → 까닭을 낸다.
    /// </summary>
    internal static Dictionary<(string View, string Column), string> Build(
        SqliteConnection connection, SqliteTransaction transaction, IReadOnlyList<UserColumnDefinition> userColumns)
    {
        var broken = new Dictionary<(string View, string Column), string>();
        var types = new Dictionary<(string View, string Column), string>();
        var env = Flow.EnvForView(Own(userColumns));
        foreach (var view in Read(connection, transaction))
        {
            if (view.Columns.Count == 0) continue;
            var frame = Frame(view.Fold, view.Columns.Select(c => c.Formula));
            var columns = new List<(string Id, string Name, string? Expression)>();
            var mine = new Dictionary<string, string>();
            foreach (var column in view.Columns)
            {
                try
                {
                    var (expression, shape) = Compile(connection, transaction, column.Formula, env, frame, toggleQueryOnly: true);
                    using (var command = connection.CreateCommand())
                    {
                        command.Transaction = transaction;
                        command.CommandText = frame.Statement(expression, null, "LIMIT 20") + ";";
                        using var reader = command.ExecuteReader();
                        while (reader.Read()) { }
                    }
                    columns.Add((column.Id, column.Name, expression));
                    types[(view.Id, column.Id)] = shape;
                }
                catch (Exception error) when (error is InvalidOperationException or SqliteException)
                {
                    mine[column.Id] = error.Message.Length == 0 ? "식을 계산하지 못했습니다." : error.Message;
                    columns.Add((column.Id, column.Name, null));
                }
            }

            try
            {
                Execute(connection, transaction, Views.UserView(view.Name, frame, columns, mine));
                Execute(connection, transaction, $"SELECT * FROM {FlowCompiler.Ident(view.ViewName)} LIMIT 0;");
            }
            catch (SqliteException error)
            {
                // 필드마다 돌려 본 뒤라 여기서 깨질 일은 드물다. 그래도 깨지면 이 뷰의 필드를 모두 빈 글로 세운다 — 앱이 뜨지 못하는 것보다 낫다.
                foreach (var column in columns.Where(c => !mine.ContainsKey(c.Id))) mine[column.Id] = error.Message;
                Execute(connection, transaction, Views.UserView(view.Name, frame, columns, mine));
            }
            foreach (var (id, why) in mine)
            {
                broken[(view.Id, id)] = why;
                types.Remove((view.Id, id));
            }
        }
        Execute(connection, transaction, Views.UserViewMeta(types));
        return broken;
    }

    /// <summary>
    /// <c>sqlite_master</c> 의 <c>u_%</c> 를 모두 치운다 — 저장된 목록이 아니라 실제로 선 것을 본다. 지웠거나 이름을 바꾼 뷰가
    /// 남지 않게, 그리고 그 위층이 남아 면 뷰를 다시 짓는 것을 막지 않게 면 뷰보다 먼저 부른다.
    /// </summary>
    internal static void DropAll(SqliteConnection connection, SqliteTransaction transaction)
    {
        var names = connection.Query<string>(
            "SELECT name FROM sqlite_master WHERE type = 'view' AND name LIKE 'u\\_%' ESCAPE '\\';", transaction: transaction).ToList();
        foreach (var name in names) Execute(connection, transaction, $"DROP VIEW IF EXISTS {FlowCompiler.Ident(name)};");
    }

    // ── 사람 열과 맞물림(§5) ─────────────────────────

    /// <summary>
    /// 단계가 행의 열을 읽는 자리(<c>비면</c>·사칙·<c>날짜차이</c>). 맞춤 뷰에서는 <c>공고.공고명</c> 꼴이다. 그 밖의 단계 필드는
    /// 목록의 행(품목·문서 한 행)을 짚는 것이다.
    /// </summary>
    private static readonly HashSet<string> RowFieldSteps = ["ifEmpty", "add", "sub", "mul", "div", "dateDiff"];

    /// <summary>
    /// 흐름식에서 <paramref name="kind"/> 문서의 열 <paramref name="field"/> 를 읽는 자리를 찾고, <paramref name="to"/> 가 있으면 그
    /// 이름으로 고친다. 자리는 셋이다 — 원천 <c>linked(kind) field</c>, 단계가 읽는 행의 열 <c>공고.f</c>, 원천이 그 문서 한 행일 때
    /// 첫 단계가 짚는 필드(<c>꺼내기 f</c>·<c>만약 f = …</c>·<c>서식 "{f}"</c>). 찾았는가를 낸다.
    /// </summary>
    internal static bool Touch(JsonNode? formula, string kind, string field, string? to)
    {
        if (formula is not JsonObject root || Text(root["source"]) != "flow" || root["flow"] is not JsonObject flow) return false;
        var found = false;
        var from = flow["from"] as JsonObject;
        var linked = from is not null && Text(from["kind"]) == "linked" && Text(from["link"]) == kind;
        if (linked && Text(from!["field"]) == field)
        {
            found = true;
            if (to is not null) from["field"] = to;
        }
        if (flow["steps"] is not JsonArray steps) return found;
        var qualified = Flow.DocWords[kind] + "." + field;
        foreach (var step in steps.OfType<JsonObject>())
        {
            if (RowFieldSteps.Contains(Text(step["op"])) && Text(step["field"]) == qualified)
            {
                found = true;
                if (to is not null) step["field"] = Flow.DocWords[kind] + "." + to;
            }
            // 값 하나를 견주는 조건의 마디도 그 자리를 읽는다(만약 공고.f = …). 행을 견주는 마디는 문서 이름이 붙지 않아 걸리지 않는다.
            foreach (var clause in Clauses(step).Where(c => Text(c["field"]) == qualified))
            {
                found = true;
                if (to is not null) clause["field"] = Flow.DocWords[kind] + "." + to;
            }
        }

        // 원천이 그 문서 한 행(필드·품목 없이)이면 첫 단계가 그 행의 필드를 짚는다 — 행에서 나가는 동사는 값을 내므로 첫 단계뿐이다.
        if (!linked || from!["field"] is not null || from["items"] is not null || steps.FirstOrDefault() is not JsonObject first) return found;
        switch (Text(first["op"]))
        {
            case "get" when Text(first["field"]) == field:
                found = true;
                if (to is not null) first["field"] = to;
                break;
            case "if" when first["cond"] is JsonArray cond:
                foreach (var clause in cond.OfType<JsonObject>().Where(c => Text(c["field"]) == field))
                {
                    found = true;
                    if (to is not null) clause["field"] = to;
                }
                break;
            case "format" when Text(first["text"]).Contains("{" + field + "}", StringComparison.Ordinal):
                found = true;
                if (to is not null) first["text"] = Text(first["text"]).Replace("{" + field + "}", "{" + to + "}", StringComparison.Ordinal);
                break;
        }
        return found;
    }

    /// <summary>단계의 조건 마디 모두(<c>경우</c> 의 갈래 포함).</summary>
    private static IEnumerable<JsonObject> Clauses(JsonObject step) =>
        (step["cond"] is JsonArray cond ? cond.OfType<JsonObject>() : [])
        .Concat(step["branches"] is JsonArray branches
            ? branches.OfType<JsonObject>().SelectMany(b => b["cond"] is JsonArray c ? c.OfType<JsonObject>() : [])
            : []);

    /// <summary>
    /// <paramref name="entityType"/> 의 사람 열을 읽는 맞춤 뷰 수 — 열 이름 → 뷰 수. 열을 지우는 쪽 확인 문구가 센다
    /// (「맞춤 뷰 n곳이 이 열을 씁니다」). SQL 필드의 글은 세지 않는다 — 식이 아니라 글이라 읽는지 알 수 없다.
    /// </summary>
    public static IReadOnlyDictionary<string, int> Readers(SqliteConnection connection, string entityType)
    {
        var own = connection.Query<string>("SELECT field_name FROM user_column WHERE entity_type = @entityType", new { entityType }).ToList();
        var counts = new Dictionary<string, int>(StringComparer.Ordinal);
        if (!Flow.DocWords.ContainsKey(entityType)) return counts;
        foreach (var view in Read(connection))
        {
            var formulas = view.Columns.Select(c => Node(c.Formula)).ToList();
            foreach (var field in own.Where(f => formulas.Any(x => Touch(x, entityType, f, null))))
                counts[field] = counts.GetValueOrDefault(field) + 1;
        }
        return counts;
    }

    private static JsonNode? Node(string json)
    {
        try { return JsonNode.Parse(json); }
        catch (JsonException) { return null; }
    }

    /// <summary>
    /// 사람 열 이름이 바뀌면 그 열을 읽던 맞춤 뷰 필드를 새 이름으로 고쳐 쓴다(§5) — 이름을 바꾼 트랜잭션 안에서. 고친 뷰 수를 낸다.
    /// </summary>
    internal static int RenameField(SqliteConnection connection, SqliteTransaction transaction, string entityType, string from, string to)
    {
        if (from == to || !Flow.DocWords.ContainsKey(entityType)) return 0;
        var changed = 0;
        foreach (var view in Read(connection, transaction))
        {
            JsonNode? root;
            try { root = JsonNode.Parse(view.Definition); }
            catch (JsonException) { continue; }
            if (root?["columns"] is not JsonArray columns) continue;
            var touched = false;
            foreach (var column in columns.OfType<JsonObject>())
                touched |= Touch(column["formula"], entityType, from, to);
            if (!touched) continue;
            connection.Execute("UPDATE user_view SET definition = @definition WHERE id = @id",
                new { definition = root.ToJsonString(Flow.Json), id = view.Id }, transaction);
            changed++;
        }
        return changed;
    }

    private static void Execute(SqliteConnection connection, SqliteTransaction? transaction, string sql)
    {
        using var command = connection.CreateCommand();
        if (transaction is not null) command.Transaction = transaction;
        command.CommandText = sql;
        command.ExecuteNonQuery();
    }
}

using System.Text.Json;
using Dapper;
using Microsoft.Data.Sqlite;

namespace Pclm.Core.Storage;

/// <summary>
/// 열 관리의 다리 쪽(docs/column-management.md §11.4) — 값 흐름 보표, SQL 꺼내기·검증, 깨진 열, 값으로 굳히기.
/// 읽는 것은 모두 읽기 전용 연결 한 벌에서 한다.
/// </summary>
public sealed partial class Store
{
    /// <summary>보표가 보이는 표본 문서 수.</summary>
    private const int TraceSamples = 3;

    /// <summary>목록 칸에 보이는 앞 값 수(화면의 <c>SHOW</c>)와 서식 단계에서 누를 표본 행 수.</summary>
    private const int ShowItems = 3, ShowRows = 5;

    private static string FaceBase(string entityType) => entityType switch
    {
        "notice" => "공고본번호",
        "request" => "접수본번호",
        _ => "계약본번호",
    };

    private sealed record Sample(string Base, string Revision, string Title, string Key);

    private static List<Sample> Samples(SqliteConnection connection, string entityType, int count)
    {
        var separator = entityType == "contract" ? "" : "-";
        return [.. connection.Query<(string Base, string Revision, string Title)>($"""
                SELECT p.{entityType}_base, p.seq, coalesce(p.title,'') FROM ({LatestColumnParents(entityType)}) p
                ORDER BY p.{entityType}_base LIMIT {count}
                """)
            .Select(p => new Sample(p.Base, p.Revision, p.Title, p.Base + separator + p.Revision))];
    }

    /// <summary>
    /// 원천으로 쓸 수 있는 것(§11.4 <c>columnContext.available</c>) — <see cref="Flow.EnvFor"/> 와 같은 재료에,
    /// SQL 자동완성에 쓰는 계약면 뷰와 그 열(사람 열 포함, 실제 뷰에서 읽는다).
    /// </summary>
    private static object Available(SqliteConnection connection, string entityType)
    {
        var env = Flow.EnvFor(entityType);
        return new
        {
            view = FaceView(entityType),
            @base = env.Base,
            items = env.Items,
            linked = env.Linked.Select(l => new { kind = l.Key, fields = l.Value.Fields, items = l.Value.Items, many = l.Value.Many }).ToList(),
            schema = SqlColumn.Readable.ToDictionary(v => v, v => ViewColumns(connection, v)),
        };
    }

    /// <summary>
    /// 깨진 가공열 — (종류, 이름) → 까닭. 뷰를 다시 지을 때 열마다 따로 준비해 보고 깨진 것은 빈 글로 세웠다
    /// (<see cref="Database"/>). 그 까닭은 면 뷰 정의 안에 남아 있어 여기서 읽는다.
    /// </summary>
    public IReadOnlyDictionary<(string Entity, string Name), string> BrokenColumns()
    {
        using var connection = _database.OpenReadOnly();
        var found = new Dictionary<(string, string), string>();
        foreach (var entity in new[] { "notice", "contract", "request" })
        {
            var sql = connection.QuerySingleOrDefault<string>(
                "SELECT sql FROM sqlite_master WHERE type = 'view' AND name = @view", new { view = FaceView(entity) });
            foreach (var (name, why) in Views.BrokenIn(sql))
                found[(entity, name)] = why.Length == 0 ? "식을 계산하지 못했습니다." : why;
        }
        return found;
    }

    // ── 값 흐름 보표 ─────────────────────────────────────────

    /// <summary>
    /// 표본 셋(최신 문서)에 대해 원천과 단계마다 그 시점의 값(§11.4). 목록은
    /// <c>{kind:"list",count,items(앞 3),dropped,blank,rows(앞 5)}</c>, 값은 <c>{kind:"value",text,blank,row}</c>.
    /// 단계 오류는 그 단계에 붙고 뒤 단계는 <c>blocked</c> 다. SQL 열이면 원천 한 줄에 표본 결과를 싣는다.
    /// </summary>
    public object TraceComputedColumn(string entityType, string json)
    {
        var formula = ComputedColumn.ParseFor(entityType, json);
        using var connection = SqlColumn.OpenChecked(_database);
        using var transaction = connection.BeginTransaction(deferred: true);
        var samples = Samples(connection, entityType, TraceSamples);
        var heads = samples.Select(s => new { @base = s.Base, revision = s.Revision, title = s.Title }).ToList();
        return Trace(connection, entityType, formula, Flow.EnvFor(entityType), samples, heads, FaceTrace(entityType));
    }

    /// <summary>
    /// 보표가 표본 한 건을 읽는 자리. <see cref="Row"/> 는 고른 것을 받아 표본 열쇠 <c>@key</c> 의 한 줄을 읽는 문장(단계마다의 값),
    /// <see cref="Final"/> 은 끝 식을 받아 그 열에 찍힐 값을 읽는 문장, <see cref="Sql"/> 은 SQL 열을 표본마다 검증해 찍힐 값을 낸다.
    /// 가공열은 면 뷰 한 줄, 맞춤 뷰는 사슬 위의 세 문서다.
    /// </summary>
    private sealed record TraceFrame(Func<string, string> Row, Func<string, string> Final,
        Func<SqliteConnection, string, IReadOnlyList<string>, IReadOnlyList<string>> Sql);

    private static TraceFrame FaceTrace(string entityType)
    {
        var frame = SqlColumn.Frame.Face(entityType);
        string Row(string select) => frame.Statement(select, $"{frame.Key} = @key", "");
        return new TraceFrame(Row, Row, (connection, sql, keys) => SqlColumn.Validate(connection, sql, frame, keys));
    }

    /// <summary>
    /// 보표의 몸 — 가공열과 맞춤 뷰가 함께 쓴다. <paramref name="env"/> 가 꼴 검사·컴파일의 재료이고(맞춤 뷰는 세 문서의 모든 열),
    /// <paramref name="heads"/> 는 화면에 보일 표본 머리, <paramref name="frame"/> 은 표본 한 건을 읽는 자리다.
    /// </summary>
    private static object Trace<THead>(SqliteConnection connection, string entityType, FlowFormula formula, FlowEnv env,
        List<Sample> samples, List<THead> heads, TraceFrame frame)
    {

        if (formula.Source == "sql")
        {
            string? error = null;
            IReadOnlyList<string> values = [.. samples.Select(_ => "")];
            try { values = frame.Sql(connection, formula.Sql!, [.. samples.Select(s => s.Key)]); }
            catch (Exception e) when (e is InvalidOperationException or SqliteException) { error = e.Message; }
            return new
            {
                samples = heads,
                steps = new[] { Step("value", [.. values.Select(Value)], error, false) },
                final = error is null ? values : [.. samples.Select(_ => "")],
                sql = formula.Sql,
            };
        }

        var flow = formula.Flow!;
        var checkedFlow = Flow.Check(flow, env);
        var stop = checkedFlow.Errors.FirstOrDefault(e => !e.Final)?.Index;
        var steps = new List<object>();
        long[]? counts = null;
        FlowCompiler.Part? previous = null;
        for (var k = 0; k <= flow.Steps.Length; k++)
        {
            var shape = checkedFlow.Shapes[k];
            var error = checkedFlow.Errors.FirstOrDefault(e => e.Index == k)?.Message;
            var blank = samples.Select(_ => Blank()).ToList();
            if (stop is { } s && k > s)
            {
                steps.Add(Step(shape, blank, null, true));
                continue;
            }
            if (shape is null)
            {
                steps.Add(Step(shape, blank, error, false));
                continue;
            }
            try
            {
                var part = FlowCompiler.Prefix(entityType, flow, k, FlowCompiler.Target.Internal, env);
                var step = k == 0 ? null : flow.Steps[k - 1];
                var shown = samples.Select((sample, i) => Show(connection, frame.Row, sample.Key, part, previous, step, counts?[i])).ToList();
                steps.Add(Step(shape, [.. shown.Select(x => x.Value)], error, false));
                counts = [.. shown.Select(x => x.Count)];
                previous = part;
            }
            catch (Exception e) when (e is InvalidOperationException or SqliteException)
            {
                steps.Add(Step(shape, blank, error ?? $"계산하지 못했습니다: {e.Message}", false));
                stop = k;
            }
        }

        var ok = checkedFlow.Ok && stop is null;
        var final = samples.Select(_ => "").ToList();
        var sql = "";
        if (ok)
        {
            var expression = ComputedColumn.Expression(entityType, formula, env);
            final = [.. samples.Select(sample => connection.QuerySingleOrDefault<string>(frame.Final(expression), new { key = sample.Key }) ?? "")];
            sql = FlowCompiler.Eject(entityType, formula, env);
        }
        return new { samples = heads, steps, final, sql };
    }

    private static Dictionary<string, object?> Step(string? shape, List<Dictionary<string, object?>> values, string? error, bool blocked)
    {
        var step = new Dictionary<string, object?> { ["shape"] = shape, ["values"] = values };
        if (error is not null) step["error"] = error;
        if (blocked) step["blocked"] = true;
        return step;
    }

    private static Dictionary<string, object?> Blank() => new() { ["kind"] = "value", ["text"] = "", ["blank"] = true };

    private static Dictionary<string, object?> Value(string text) =>
        text.Length == 0 ? Blank() : new() { ["kind"] = "value", ["text"] = text };

    /// <summary>행의 꼬리표 — 빈 값을 건너 앞 두 값(화면의 <c>rowLabel</c>).</summary>
    private static string RowLabel(Dictionary<string, string> row, IReadOnlyList<string> fields) =>
        string.Join(" · ", fields.Select(f => row.GetValueOrDefault(f, "")).Where(v => v.Length > 0).Take(2));

    private static readonly HashSet<string> Narrowing = ["filter", "distinct", "take"];

    /// <summary>표본 하나에서 앞부분 <paramref name="part"/> 의 값. 목록이면 수도 함께(다음 단계의 「빠짐」).</summary>
    private static (Dictionary<string, object?> Value, long Count) Show(SqliteConnection connection, Func<string, string> read, string key,
        FlowCompiler.Part part, FlowCompiler.Part? previous, FlowStep? step, long? previousCount)
    {
        string JsonRow(IReadOnlyList<string> fields) =>
            "json_object(" + string.Join(", ", fields.Select(f => $"{FlowCompiler.Lit(f)}, {FlowCompiler.Ident(f)}")) + ")";
        Dictionary<string, string> Row(JsonElement e) =>
            e.EnumerateObject().ToDictionary(p => p.Name, p => p.Value.ValueKind == JsonValueKind.String ? p.Value.GetString()! : p.Value.ToString());

        switch (part.Shape)
        {
            case "rows":
            {
                var (count, list) = connection.QuerySingle<(long, string?)>(read($"""
                    (SELECT count(*) FROM ({part.Sql})),
                    (SELECT json_group_array({JsonRow(part.Fields)} ORDER BY __order) FROM (SELECT * FROM ({part.Sql}) ORDER BY __order LIMIT {ShowRows}))
                    """), new { key });
                var rows = list is null ? [] : JsonDocument.Parse(list).RootElement.EnumerateArray().Select(Row).ToList();
                var value = new Dictionary<string, object?>
                {
                    ["kind"] = "list", ["count"] = count,
                    ["items"] = rows.Take(ShowItems).Select(r => RowLabel(r, part.Fields)).ToList(),
                    ["rows"] = rows,
                };
                if (step is not null && Narrowing.Contains(step.Op) && previousCount is { } before && before > count) value["dropped"] = before - count;
                return (value, count);
            }
            case "values":
            {
                var (count, blanks, list) = connection.QuerySingle<(long, long, string?)>(read($"""
                    (SELECT count(*) FROM ({part.Sql})),
                    (SELECT count(*) FROM ({part.Sql}) WHERE __value = ''),
                    (SELECT json_group_array(__value ORDER BY __order) FROM (SELECT * FROM ({part.Sql}) ORDER BY __order LIMIT {ShowItems}))
                    """), new { key });
                var value = new Dictionary<string, object?>
                {
                    ["kind"] = "list", ["count"] = count,
                    ["items"] = list is null ? new List<string>() : JsonDocument.Parse(list).RootElement.EnumerateArray().Select(e => e.ToString()).ToList(),
                };
                if (step is not null && Narrowing.Contains(step.Op) && previousCount is { } before && before > count) value["dropped"] = before - count;
                // 서식은 비어 찍히는 칸이 아니라 빈 필드를 낀 행을 센다 — 「의자()」 도 빈 값이다(화면과 같다).
                if (step is { Op: "format" } && previous is { Shape: "rows" })
                {
                    var holes = Flow.Placeholders(step.Text ?? "", out _) ?? [];
                    blanks = holes.Count == 0 ? 0 : connection.QuerySingle<long>(
                        read($"(SELECT count(*) FROM ({previous.Sql}) WHERE {string.Join(" OR ", holes.Distinct().Select(h => $"{FlowCompiler.Ident(h)} = ''"))})"),
                        new { key });
                }
                if (blanks > 0) value["blank"] = blanks;
                return (value, count);
            }
            case "row":
            {
                var json = connection.QuerySingleOrDefault<string?>(read($"(SELECT {JsonRow(part.Fields)} FROM ({part.Sql}))"), new { key });
                if (json is null) return (Blank(), 0);
                var row = Row(JsonDocument.Parse(json).RootElement);
                return (new Dictionary<string, object?> { ["kind"] = "value", ["text"] = RowLabel(row, part.Fields), ["row"] = row }, 1);
            }
            case "number":
            {
                var text = connection.QuerySingleOrDefault<string?>(
                    read($"(SELECT CASE WHEN v IS NULL THEN NULL ELSE printf('%.15g', v) END FROM (SELECT {part.Sql} AS v))"), new { key });
                return (Value(text ?? ""), 0);
            }
            case "date":
            {
                var text = connection.QuerySingleOrDefault<string?>(read($"replace({part.Sql}, '-', '/')"), new { key });
                return (Value(text ?? ""), 0);
            }
            default:
                return (Value(connection.QuerySingleOrDefault<string?>(read(part.Sql), new { key }) ?? ""), 0);
        }
    }

    // ── SQL 꺼내기·검증 ─────────────────────────────────────────

    /// <summary>「SQL로 꺼내기」 — 계약면 뷰만 읽는 스칼라 부질의(§5). 꼴 검사를 지나지 못하면 단계 번호를 담아 던진다.</summary>
    public string CompileComputedColumn(string entityType, string json)
    {
        var formula = ComputedColumn.ParseFor(entityType, json);
        if (formula.Source == "sql") return formula.Sql!;
        return FlowCompiler.Eject(entityType, formula);
    }

    /// <summary>
    /// SQL 열 검증(§5) — 읽기 전용 연결에 <c>query_only</c>, authorizer, 준비, 표본 실행. 거절하면
    /// <c>{ok:false,error}</c>, 통과하면 표본마다 찍힐 값. 순환은 저장할 때(뷰에 넣어 본 뒤) 잡힌다.
    /// </summary>
    public object ValidateComputedSql(string entityType, string sql)
    {
        _ = ComputedColumn.Fields(entityType);
        using var connection = SqlColumn.OpenChecked(_database);
        using var transaction = connection.BeginTransaction(deferred: true);
        var samples = Samples(connection, entityType, TraceSamples);
        var heads = samples.Select(s => new { @base = s.Base, revision = s.Revision, title = s.Title }).ToList();
        try
        {
            var values = SqlColumn.Validate(connection, entityType, sql, [.. samples.Select(s => s.Key)]);
            return new { ok = true, error = (string?)null, samples = heads, final = values };
        }
        catch (Exception e) when (e is InvalidOperationException or SqliteException)
        {
            return new { ok = false, error = (string?)e.Message, samples = heads, final = (IReadOnlyList<string>)[.. samples.Select(_ => "")] };
        }
    }

    // ── 값으로 굳히기 ─────────────────────────────────────────

    /// <summary>「값으로 굳히기」·「가공열로 바꾸기」가 건드릴 칸 수.</summary>
    /// <param name="Cells">굳히면 입력값으로 쓸 칸 — 계열마다 최신 차수의 값 중 빈 글이 아닌 것.</param>
    /// <param name="Hidden">지금 그 이름으로 적혀 있는 입력값. 가공열이면 식에 가려 숨은 옛 값(굳히면 덮이거나 지워진다),
    /// 입력열이면 가공열로 바꿀 때 숨겨질 값.</param>
    public sealed record FreezePlan(string Kind, long Cells, long Hidden, long Series);

    public FreezePlan PlanFreezeColumn(string entityType, string fieldName)
    {
        using var connection = _database.OpenReadOnly();
        using var transaction = connection.BeginTransaction(deferred: true);
        return PlanFreeze(connection, entityType, fieldName).Plan;
    }

    private static (FreezePlan Plan, List<(string Base, string Value)> Values) PlanFreeze(SqliteConnection connection, string entityType, string fieldName)
    {
        var (table, _) = TableFor(entityType);
        var kind = connection.QuerySingleOrDefault<string>(
            "SELECT kind FROM user_column WHERE entity_type = @entityType AND field_name = @fieldName",
            new { entityType, fieldName }) ?? throw new InvalidOperationException($"‘{fieldName}’ 열을 찾을 수 없습니다.");
        var (_, baseColumn) = TableFor(entityType);
        var stored = connection.Query<string>($"SELECT {baseColumn} FROM {table} WHERE field_name = @fieldName", new { fieldName }).ToHashSet();
        if (kind != "computed") return (new FreezePlan(kind, 0, stored.Count, 0), []);
        var values = connection.Query<(string Base, string Value)>(
            $"SELECT \"{FaceBase(entityType)}\", coalesce(CAST({FlowCompiler.Ident(fieldName)} AS TEXT), '') FROM \"{FaceView(entityType)}\"").ToList();
        return (new FreezePlan(kind, values.Count(v => v.Value.Length > 0), values.Count(v => stored.Contains(v.Base)), values.Count), values);
    }

    /// <summary>
    /// 「값으로 굳히기」(§3.1·§7) — 각 <b>계열의 최신 차수</b> 값을 그 계열의 입력값으로 쓴다(ADR-012: 사람 값은
    /// 계열에 매단다). 빈 값인 계열은 숨어 있던 옛 입력값을 지운다 — 굳힌 뒤의 열이 굳히기 전에 보이던 것과 같게.
    /// 지금 계열이 없는 옛 입력값(ADR-007 로 남은 것)은 건드리지 않는다.
    /// 식은 <c>options</c> 에 비활성으로 남겨 다시 켤 수 있다. <paramref name="expected"/> 가 오면 센 수와 견줘
    /// 그 사이 자료가 바뀌었으면 쓰지 않는다. 쓴 칸 수를 낸다.
    /// </summary>
    public long FreezeColumn(string entityType, string fieldName, long? expected = null)
    {
        using var connection = _database.Open();
        using var transaction = connection.BeginTransaction();
        var (plan, values) = PlanFreeze(connection, entityType, fieldName);
        if (plan.Kind != "computed") throw new InvalidOperationException("가공열만 값으로 굳힐 수 있습니다.");
        if (expected is { } want && want != plan.Cells)
            throw new InvalidOperationException($"칸을 센 뒤에 자료가 바뀌어({want}칸 → {plan.Cells}칸) 아무것도 쓰지 않았습니다. 다시 열어 새로 센 수를 확인하세요.");
        var (table, baseColumn) = TableFor(entityType);
        var now = DateTime.UtcNow.ToString("O");
        long written = 0;
        foreach (var (@base, value) in values)
        {
            if (value.Length == 0)
            {
                connection.Execute($"DELETE FROM {table} WHERE {baseColumn} = @base AND field_name = @fieldName", new { @base, fieldName }, transaction);
                continue;
            }
            written += connection.Execute($"""
                INSERT INTO {table} ({baseColumn}, field_name, value, updated_at) VALUES (@base, @fieldName, @value, @now)
                ON CONFLICT({baseColumn}, field_name) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at
                """, new { @base, fieldName, value, now }, transaction);
        }
        connection.Execute(
            "UPDATE user_column SET kind = 'text' WHERE entity_type = @entityType AND field_name = @fieldName",
            new { entityType, fieldName }, transaction);
        Database.RefreshViews(connection, transaction);
        if (written != plan.Cells) throw new InvalidOperationException($"센 칸({plan.Cells})과 쓸 칸({written})이 달라 멈췄습니다. 아무것도 쓰지 않았습니다.");
        transaction.Commit();
        return written;
    }
}

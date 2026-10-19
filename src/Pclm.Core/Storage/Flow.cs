using System.Globalization;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Text.Json.Nodes;
using System.Text.RegularExpressions;

namespace Pclm.Core.Storage;

/// <summary>
/// 가공식 v2 — 흐름식의 원본(docs/column-management.md §4.5). 원천에서 시작해 단계를 잇는다.
///
/// <para><b>화면의 <c>frontend/src/flow/</c> 와 한 벌이다.</b> 동사 이름·인자 자리·꼴 표가 갈리면 화면에서
/// 되던 식이 저장에서 막히거나, 막혀야 할 식이 저장된다. 두 쪽이 <c>tests/flow-cases.json</c> 하나를 함께
/// 돌려 갈림을 잡는다.</para>
/// </summary>
public sealed record FlowFormula(
    int Version,
    string Id,
    string Source,
    FlowDefinition? Flow = null,
    string? Sql = null);

public sealed record FlowDefinition(FlowSource From, FlowStep[] Steps);

/// <param name="Kind"><c>items</c>·<c>field</c>·<c>linked</c>·<c>text</c>·<c>number</c>.</param>
/// <param name="Link"><c>notice</c>·<c>request</c>·<c>contract</c>.</param>
public sealed record FlowSource(
    string Kind,
    string? Field = null,
    string? Link = null,
    bool? Items = null,
    string? Text = null,
    double? Value = null);

/// <summary>
/// 조건의 한 마디. 행을 견주면 <see cref="Field"/> 는 그 행의 필드다. 값 하나를 견주면 빈 글이 지금 값 자체이고,
/// 적힌 글은 바깥의 열이다(<c>비면</c> 이 읽는 자리 — 가공열은 이 행의 열, 맞춤 뷰는 <c>공고.공고명</c>).
/// </summary>
public sealed record FlowClause(string? Join, string Field, string Cmp, JsonElement? Value = null);

/// <summary><c>경우</c> 의 갈래 하나 — 조건(값 조건과 같은 규칙)이 맞으면 그 글. 차례대로 견주어 처음 맞는 것이 이긴다.</summary>
public sealed record FlowBranch(FlowClause[] Cond, string Then);

public sealed record FlowStep(
    string Op,
    string? Field = null,
    string[]? Fields = null,
    string? Text = null,
    string? To = null,
    int? N = null,
    int? Length = null,
    string? Dir = null,
    string? Unit = null,
    double? Value = null,
    FlowClause[]? Cond = null,
    string? Then = null,
    string? Else = null,
    string[][]? Pairs = null,
    string? Otherwise = null,
    FlowBranch[]? Branches = null);

/// <summary>가공식 v1 — 물품 행 위의 pick·distinct·format·join·at·get·count. 읽을 때 v2 로 옮긴다.</summary>
public sealed record ColumnStep(string Op, string[]? Fields = null, string? Text = null, int? Index = null);
public sealed record ColumnFormula(int Version, string Id, ColumnStep[] Steps);

/// <summary>이을 수 있는 연결 문서 하나. <see cref="Many"/> 면 원천이 행목록이다(§11.1-2).</summary>
public sealed record FlowLinked(IReadOnlyList<string> Fields, IReadOnlyList<string> Items, bool Many);

/// <summary>
/// 꼴 검사가 보는 재료. 가공열에서는 연결 문서의 사용자 열이 들지 않는다(§4.1).
/// <see cref="View"/> 면 맞춤 뷰의 재료다(docs/user-views.md §3) — 「이 행」 이 없고(<see cref="Base"/>·<see cref="Items"/> 가 빈다)
/// 접수·공고·계약이 모두 한 건이며, 글 꼴은 <c>연결공고.</c> 가 아니라 <c>공고.</c> 이고 단계가 읽는 행의 열도 <c>공고.공고명</c> 꼴이다.
/// </summary>
public sealed record FlowEnv(
    IReadOnlyList<string> Base,
    IReadOnlyList<string> Items,
    IReadOnlyDictionary<string, FlowLinked> Linked,
    bool View = false);

/// <summary>꼴 검사의 오류. <see cref="Index"/> 0 이 원천, i+1 이 <c>steps[i]</c> 다(보표와 같다).</summary>
public sealed record FlowIssue(int Index, string Message, bool Final = false)
{
    /// <summary>저장 거절에 싣는 말 — 「3단계: …」.</summary>
    public string Describe() => (Index == 0 ? "시작 데이터" : $"{Index}단계") + ": " + Message;
}

public sealed record FlowChecked(
    IReadOnlyList<string?> Shapes,
    IReadOnlyList<IReadOnlyList<string>> Fields,
    IReadOnlyList<FlowIssue> Errors,
    string? Final)
{
    public bool Ok => Errors.Count == 0;
}

public static class Flow
{
    /// <summary>저장 꼴. 빈 자리는 적지 않는다 — 화면이 <c>field: null</c> 을 「적힌 필드」로 읽지 않게.</summary>
    public static readonly JsonSerializerOptions Json = new(JsonSerializerDefaults.Web)
    {
        DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull,
        Encoder = System.Text.Encodings.Web.JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
    };

    public static readonly IReadOnlyDictionary<string, string> Shapes = new Dictionary<string, string>
    {
        ["rows"] = "행목록", ["values"] = "값목록", ["row"] = "행", ["value"] = "값", ["number"] = "수", ["date"] = "날짜",
    };

    public static readonly IReadOnlyDictionary<string, string> LinkWords = new Dictionary<string, string>
    {
        ["notice"] = "연결공고", ["request"] = "연결접수", ["contract"] = "연결계약",
    };

    /// <summary>맞춤 뷰의 문서 이름(docs/user-views.md §3) — 글 꼴의 <c>공고.공고명</c> 이자 뷰 SQL 의 바깥 행 별칭이다.</summary>
    public static readonly IReadOnlyDictionary<string, string> DocWords = new Dictionary<string, string>
    {
        ["request"] = "접수", ["notice"] = "공고", ["contract"] = "계약",
    };

    /// <summary>이 재료에서 연결 문서를 부르는 말 — 가공열은 <c>연결공고</c>, 맞춤 뷰는 <c>공고</c>.</summary>
    public static string LinkWord(FlowEnv env, string? link) =>
        (env.View ? DocWords : LinkWords).GetValueOrDefault(link ?? "", link ?? "");

    /// <summary>
    /// 맞춤 뷰에서 단계가 읽는 행의 열 — <c>공고.공고명</c> 을 (종류, 열)로 가른다. 문서 이름이 앞에 없으면 null.
    /// 열 이름에 점이 있어도 첫 점에서 가른다(문서 이름에는 점이 없다).
    /// </summary>
    public static (string Kind, string Column)? Qualified(string field)
    {
        var dot = field.IndexOf('.');
        if (dot <= 0) return null;
        var word = field[..dot];
        var kind = DocWords.FirstOrDefault(w => w.Value == word).Key;
        return kind is null ? null : (kind, field[(dot + 1)..]);
    }

    /// <summary>동사 하나. <see cref="Io"/> 는 들어감 → 나옴. 화면의 <c>verbs.ts</c> 와 같은 표다.</summary>
    public sealed record Verb(string Op, string Word, (string In, string Out)[] Io);

    private static (string, string) L(string a, string b) => (a, b);

    public static readonly IReadOnlyList<Verb> Verbs =
    [
        new("filter", "거르기", [L("rows", "rows")]),
        new("sort", "정렬", [L("rows", "rows"), L("values", "values")]),
        new("distinct", "고유", [L("rows", "rows"), L("values", "values")]),
        new("take", "앞에서", [L("rows", "rows"), L("values", "values")]),
        new("nth", "번째", [L("rows", "row"), L("values", "value")]),
        new("first", "처음", [L("rows", "row"), L("values", "value")]),
        new("last", "마지막", [L("rows", "row"), L("values", "value")]),
        new("format", "서식", [L("rows", "values"), L("row", "value")]),
        new("get", "꺼내기", [L("rows", "values"), L("row", "value")]),
        new("join", "잇기", [L("values", "value")]),
        new("abbrev", "줄여쓰기", [L("values", "value")]),
        new("count", "개수", [L("rows", "number"), L("values", "number")]),
        new("sum", "합계", [L("rows", "number"), L("values", "number")]),
        new("min", "최소", [L("rows", "number"), L("values", "number")]),
        new("max", "최대", [L("rows", "number"), L("values", "number")]),
        new("avg", "평균", [L("rows", "number"), L("values", "number")]),
        new("trim", "다듬기", [L("value", "value")]),
        new("replace", "바꾸기", [L("value", "value")]),
        new("before", "앞부분", [L("value", "value")]),
        new("after", "뒷부분", [L("value", "value")]),
        new("substr", "자르기", [L("value", "value")]),
        new("padStart", "앞채우기", [L("value", "value")]),
        new("ifEmpty", "비면", [L("value", "value")]),
        new("map", "대응", [L("value", "value")]),
        new("if", "만약", [L("row", "value"), L("value", "value")]),
        new("cases", "경우", [L("value", "value")]),
        new("toNumber", "숫자로", [L("value", "number")]),
        new("add", "더하기", [L("number", "number")]),
        new("sub", "빼기", [L("number", "number")]),
        new("mul", "곱하기", [L("number", "number")]),
        new("div", "나누기", [L("number", "number")]),
        new("round", "반올림", [L("number", "number")]),
        new("floor", "버림", [L("number", "number")]),
        new("thousands", "천단위", [L("number", "value")]),
        new("plain", "그대로", [L("number", "value")]),
        new("toDate", "날짜로", [L("value", "date")]),
        new("dateAdd", "날짜더하기", [L("date", "date")]),
        new("dateDiff", "날짜차이", [L("date", "number")]),
        new("year", "연도", [L("date", "number")]),
        new("month", "월", [L("date", "number")]),
        new("quarter", "분기", [L("date", "number")]),
        new("dateFormat", "날짜서식", [L("date", "value")]),
    ];

    private static readonly Dictionary<string, Verb> ByOp = Verbs.ToDictionary(v => v.Op);

    /// <summary>단계의 조건 마디 모두(<c>경우</c> 의 갈래 포함). 값 하나를 견주는 마디의 필드는 바깥의 열이라 맞춤 뷰가 읽는 문서를 셀 때 이것을 본다.</summary>
    public static IEnumerable<FlowClause> Clauses(FlowStep step) =>
        (step.Cond ?? []).Concat((step.Branches ?? []).SelectMany(b => b.Cond ?? []));

    public static readonly IReadOnlySet<string> Compares =
        new HashSet<string> { "=", "≠", ">", "<", "≥", "≤", "포함", "비어있음", "안비어있음" };

    private static readonly string[] Ends = ["value", "number", "date"];

    private const int MaxSteps = 50;
    private const int MaxText = 1000;

    /// <summary><c>경우</c> 의 갈래 수 상한 — 화면 <c>check.ts</c> 의 <c>maxBranches</c> 와 같다.</summary>
    public const int MaxBranches = 20;

    // ── 읽기 ─────────────────────────────────────────────

    /// <summary>
    /// 저장된·화면이 보낸 가공식을 v2 로 읽는다. v1 은 그 자리에서 옮긴다(§4.5) — 앞 판 화면이 보낸 것도 받는다.
    /// 모양이 틀리면 <see cref="InvalidOperationException"/>.
    /// </summary>
    public static FlowFormula Parse(string json) => Read(json, identityOnly: false);

    // 진단 때문에 계산할 수 없어도 저장된 식의 식별자는 재사용한다. 식 모양 검증은 그대로 한다.
    internal static FlowFormula ReadForIdentity(string json) => Read(json, identityOnly: true);

    private static FlowFormula Read(string json, bool identityOnly)
    {
        int version;
        try
        {
            using var document = JsonDocument.Parse(json);
            if (document.RootElement.ValueKind != JsonValueKind.Object
                || !document.RootElement.TryGetProperty("version", out var v) || v.ValueKind != JsonValueKind.Number)
                throw new InvalidOperationException("가공식의 판을 알 수 없습니다.");
            version = v.GetInt32();
            // 판올림에서 뜻이 갈린 식은 원문을 보존하고 기존 깨진 열 진단으로 알린다. 사람이 식을 고쳐 저장하면
            // 새 식만 정상 검증하며, 직렬화할 때 이 임시 진단은 떨어진다.
            if (!identityOnly && JsonNode.Parse(json) is JsonObject root && root["migrationError"] is JsonObject error)
            {
                var current = root.DeepClone().AsObject();
                current.Remove("migrationError");
                if (JsonNode.DeepEquals(current, error["original"]))
                    throw new InvalidOperationException(error["message"]?.GetValue<string>() ?? "판올림 뒤 식을 확인하세요.");
            }
        }
        catch (JsonException) { throw new InvalidOperationException("가공식 형식이 올바르지 않습니다."); }
        catch (FormatException) { throw new InvalidOperationException("가공식의 판을 알 수 없습니다."); }

        try
        {
            switch (version)
            {
                case 1:
                    var old = JsonSerializer.Deserialize<ColumnFormula>(json, Json)
                        ?? throw new InvalidOperationException("가공식이 비었습니다.");
                    return MigrateV1(old);
                case 2:
                    var formula = JsonSerializer.Deserialize<FlowFormula>(json, Json)
                        ?? throw new InvalidOperationException("가공식이 비었습니다.");
                    Validate(formula);
                    return formula with { Id = formula.Id ?? "" };
                default:
                    throw new InvalidOperationException("가공식의 판을 알 수 없습니다.");
            }
        }
        catch (JsonException) { throw new InvalidOperationException("가공식 형식이 올바르지 않습니다."); }
    }

    public static string Serialize(FlowFormula formula) => JsonSerializer.Serialize(formula, Json);

    /// <summary>
    /// v1 → v2. v1 은 물품 행 위에서만 돌았다: <c>pick</c> 으로 필드를 고르고 그 투영 위에서 <c>distinct</c> 했다.
    /// 그래서 pick 은 단계로 남기지 않고 뒤 단계의 필드로 흡수한다 — distinct 는 고른 필드를 기준으로,
    /// at+get 은 n번째+꺼내기로. 화면의 <c>migrate.ts</c> 와 스키마 22 의 SQL 이 같은 결과를 낸다.
    /// </summary>
    public static FlowFormula MigrateV1(ColumnFormula formula)
    {
        string[] picked = [];
        var steps = new List<FlowStep>();
        foreach (var s in formula.Steps ?? [])
        {
            if (s is null) throw new InvalidOperationException("가공식 형식이 올바르지 않습니다.");
            switch (s.Op)
            {
                case "pick": picked = [.. s.Fields ?? []]; break;
                case "distinct": steps.Add(picked.Length > 0 ? new FlowStep("distinct", Fields: picked) : new FlowStep("distinct")); break;
                case "at": steps.Add(new FlowStep("nth", N: s.Index ?? 1)); break;
                case "get": steps.Add(new FlowStep("get", Field: s.Text ?? "")); break;
                case "format": steps.Add(new FlowStep("format", Text: s.Text ?? "")); break;
                case "join": steps.Add(new FlowStep("join", Text: s.Text ?? "")); break;
                case "count": steps.Add(new FlowStep("count")); break;
                default: throw new InvalidOperationException($"가공식 v1 에 모르는 연산이 있습니다: {s.Op}");
            }
        }
        return new FlowFormula(2, formula.Id ?? "", "flow", new FlowDefinition(new FlowSource("items"), [.. steps]));
    }

    /// <summary>JSON 이 담을 수 있는 모양인가 — 꼴이 아니라 자리. 꼴은 <see cref="Check"/> 가 본다.</summary>
    private static void Validate(FlowFormula formula)
    {
        static InvalidOperationException Bad(string why) => new($"가공식 형식이 올바르지 않습니다: {why}");
        static void Text(string? value)
        {
            if (value is not null && (value.Length > MaxText || value.Contains('\0')))
                throw new InvalidOperationException("글자 인자는 1000자 이내여야 하며 NUL 문자를 포함할 수 없습니다.");
        }

        switch (formula.Source)
        {
            case "sql":
                if (string.IsNullOrWhiteSpace(formula.Sql)) throw Bad("SQL 이 비었습니다");
                return;
            case "flow":
                break;
            default:
                throw Bad("source 는 flow 또는 sql 입니다");
        }

        var flow = formula.Flow ?? throw Bad("flow 가 없습니다");
        var from = flow.From ?? throw Bad("시작 데이터가 없습니다");
        if (flow.Steps is null) throw Bad("steps 가 없습니다");
        if (flow.Steps.Length > MaxSteps) throw Bad($"단계는 {MaxSteps}개까지입니다");
        switch (from.Kind)
        {
            case "items": case "number": break;
            case "text": if (from.Text is null) throw Bad("글자 시작 데이터가 비었습니다"); Text(from.Text); break;
            case "field": if (from.Field is null) throw Bad("열 시작 데이터가 비었습니다"); break;
            case "linked": if (from.Link is null || !LinkWords.ContainsKey(from.Link)) throw Bad("연결 문서가 올바르지 않습니다"); break;
            default: throw Bad($"알 수 없는 시작 데이터: {from.Kind}");
        }
        if (from.Kind == "number" && (from.Value is not { } n || !double.IsFinite(n))) throw Bad("수 시작 데이터가 비었습니다");

        foreach (var step in flow.Steps)
        {
            if (step?.Op is null) throw Bad("op 가 없는 단계가 있습니다");
            Text(step.Text); Text(step.To); Text(step.Then); Text(step.Else); Text(step.Otherwise);
            if (step.Dir is not null and not "desc") throw Bad("정렬 방향은 desc 만 적습니다");
            if (step.Unit is not null and not ("day" or "month" or "year")) throw Bad("날짜 단위는 day·month·year 입니다");
            if (step.Value is { } v && !double.IsFinite(v)) throw Bad("수 인자가 올바르지 않습니다");
            if (step.Fields?.Any(f => f is null) == true) throw Bad("필드 목록에 빈 자리가 있습니다");
            foreach (var pair in step.Pairs ?? [])
            {
                if (pair is not { Length: 2 } || pair[0] is null || pair[1] is null) throw Bad("대응은 [찾을 값, 바꿀 값] 짝입니다");
                Text(pair[0]); Text(pair[1]);
            }
            CheckClauses(step.Cond);
            if (step.Branches is { } branches)
            {
                // 갈래 수는 꼴 검사가 본다(화면과 같은 말) — 여기서는 담을 수 있는 크기만 막는다.
                if (branches.Length > MaxSteps) throw Bad($"경우는 {MaxSteps}개까지 담습니다");
                foreach (var branch in branches)
                {
                    if (branch?.Cond is null || branch.Then is null) throw Bad("경우는 조건과 글의 짝입니다");
                    Text(branch.Then);
                    CheckClauses(branch.Cond);
                }
            }
        }

        static void CheckClauses(FlowClause[]? cond)
        {
            foreach (var clause in cond ?? [])
            {
                if (clause?.Field is null || clause.Cmp is null || !Compares.Contains(clause.Cmp)) throw Bad("조건이 올바르지 않습니다");
                if (clause.Join is not null and not ("and" or "or")) throw Bad("조건은 and·or 로 잇습니다");
                if (clause.Value is { } value && value.ValueKind is not (JsonValueKind.String or JsonValueKind.Number))
                    throw Bad("조건의 값은 글이나 수입니다");
                if (clause.Value is { ValueKind: JsonValueKind.String } s) Text(s.GetString());
            }
        }
    }

    // ── 꼴 검사 ─────────────────────────────────────────────

    /// <summary>
    /// 이 자료 종류의 재료. 기본 열은 사용자 열을 뺀 면 뷰의 열, 품목은 <see cref="ComputedColumn.Fields"/>,
    /// 연결은 §11.1-2 의 표 — 공고·접수에서 연결계약은 여러 건이다.
    /// </summary>
    public static FlowEnv EnvFor(string entity)
    {
        FlowLinked Link(string kind, bool many) =>
            new(Views.BaseColumns(kind), [.. ComputedColumn.Fields(kind).Keys], many);
        var linked = entity switch
        {
            "contract" => new Dictionary<string, FlowLinked> { ["notice"] = Link("notice", false), ["request"] = Link("request", false) },
            "notice" => new Dictionary<string, FlowLinked> { ["request"] = Link("request", false), ["contract"] = Link("contract", true) },
            "request" => new Dictionary<string, FlowLinked> { ["notice"] = Link("notice", false), ["contract"] = Link("contract", true) },
            _ => throw new InvalidOperationException("가공열의 자료 종류가 올바르지 않습니다."),
        };
        return new FlowEnv(Views.BaseColumns(entity), [.. ComputedColumn.Fields(entity).Keys], linked);
    }

    /// <summary>
    /// 맞춤 뷰의 재료(docs/user-views.md §3) — 「이 행」 이 없고(기본 열·품목이 빈다) 접수·공고·계약이 모두 한 건이다.
    /// 문서마다 그 면 뷰의 열을 모두 읽는다: 기본 열 끝에 사람 열·가공열 <paramref name="own"/>(종류 → 이름, 차례대로)을 더한다.
    /// 맞춤 뷰는 면 뷰 위층이라 그 열을 읽어도 순환이 없다.
    /// </summary>
    public static FlowEnv EnvForView(IReadOnlyDictionary<string, IReadOnlyList<string>> own)
    {
        FlowLinked Link(string kind)
        {
            var columns = Views.BaseColumns(kind);
            var mine = own.GetValueOrDefault(kind) ?? [];
            return new FlowLinked([.. columns, .. mine.Where(n => !columns.Contains(n))], [.. ComputedColumn.Fields(kind).Keys], false);
        }
        return new FlowEnv([], [], new Dictionary<string, FlowLinked>
        {
            ["request"] = Link("request"), ["notice"] = Link("notice"), ["contract"] = Link("contract"),
        }, View: true);
    }

    /// <summary>받침에 맞춰 「은/는」을 붙인다 — 「연결공고은」 같은 문구를 막는다(화면 <c>flow/josa.ts</c> 와 같은 규칙).</summary>
    private static string 은는(string word)
    {
        var last = word.TrimEnd().TrimEnd("’'」)]".ToCharArray());
        var c = last.Length == 0 ? ' ' : last[^1];
        var batchim = c is >= '가' and <= '힣' ? (c - '가') % 28 != 0 : c is '0' or '1' or '3' or '6' or '7' or '8';
        return word + (batchim ? "은" : "는");
    }

    private static string VerbWord(FlowStep step) =>
        step.Op == "nth" ? $"{step.N ?? 1}번째" : ByOp.TryGetValue(step.Op, out var v) ? v.Word : step.Op;

    /// <summary>원천의 꼴과 필드. 오류면 글.</summary>
    public static (string Shape, IReadOnlyList<string> Fields)? SourceShape(FlowSource from, FlowEnv env, out string? error)
    {
        error = null;
        switch (from.Kind)
        {
            case "items" when env.View:
                error = "맞춤 뷰에는 「이 행」 이 없습니다. ‘공고.품목’처럼 문서를 붙여 읽으세요";
                return null;
            case "field" when env.View:
                error = $"맞춤 뷰에는 「이 행」 이 없습니다. ‘공고.{from.Field}’처럼 문서를 붙여 읽으세요";
                return null;
            case "items": return ("rows", env.Items);
            case "text": return ("value", []);
            case "number": return ("number", []);
            case "field":
                if (env.Base.Contains(from.Field ?? "")) return ("value", []);
                error = $"이 행에 없는 열: {from.Field}";
                return null;
            case "linked":
                var word = LinkWord(env, from.Link);
                if (from.Link is null || !env.Linked.TryGetValue(from.Link, out var link))
                {
                    error = $"{은는(word)} 이 자료 종류에서 쓸 수 없습니다";
                    return null;
                }
                if (from.Items == true)
                {
                    if (!link.Many) return ("rows", link.Items);
                    error = $"{은는(word)} 여러 건이라 품목을 바로 읽을 수 없습니다. ‘{word} |> 처음’처럼 한 건을 먼저 고르세요";
                    return null;
                }
                if (from.Field is null) return (link.Many ? "rows" : "row", link.Fields);
                if (link.Many)
                {
                    error = $"{은는(word)} 여러 건입니다. ‘{word} |> 처음 |> 꺼내기 {from.Field}’처럼 한 건을 골라 읽으세요";
                    return null;
                }
                if (link.Fields.Contains(from.Field)) return ("value", []);
                error = $"{word}에 없는 필드: {from.Field}";
                return null;
            default:
                error = $"알 수 없는 시작 데이터: {from.Kind}";
                return null;
        }
    }

    /// <summary>서식의 {필드} 를 순서대로. 짝이 안 맞으면 오류 글.</summary>
    public static IReadOnlyList<string>? Placeholders(string text, out string? error)
    {
        error = null;
        var output = new List<string>();
        for (var i = 0; i < text.Length; i++)
        {
            if (text[i] == '}') { error = "서식의 ‘}’ 짝이 없습니다"; return null; }
            if (text[i] != '{') continue;
            var end = text.IndexOf('}', i + 1);
            var open = text.IndexOf('{', i + 1);
            if (end < 0 || (open >= 0 && open < end)) { error = "서식의 ‘{’ 가 닫히지 않았습니다"; return null; }
            var name = text[(i + 1)..end];
            if (name.Length == 0) { error = "서식에 빈 {} 가 있습니다"; return null; }
            output.Add(name);
            i = end;
        }
        return output;
    }

    /// <summary>
    /// 조건 검사. 행을 견주면 마디의 필드는 그 행의 필드다. 값 하나를 견주면 빈 필드가 지금 값이고, 적힌 필드는 바깥의 열이다 —
    /// <c>비면</c> 이 대신 읽는 열과 같은 자리(가공열은 이 행의 열, 맞춤 뷰는 <c>공고.공고명</c>)이고 같은 검사(<paramref name="outer"/>)를 탄다.
    /// </summary>
    private static string? CondProblem(FlowClause[]? cond, string shape, IReadOnlyList<string> fields, Func<string, string?> outer)
    {
        if (cond is not { Length: > 0 }) return "조건이 비었습니다";
        foreach (var c in cond)
        {
            if (shape == "value")
            {
                if (c.Field.Length > 0 && outer(c.Field) is { } problem) return problem;
                continue;
            }
            if (c.Field.Length == 0) return "견줄 필드를 고르세요";
            if (!fields.Contains(c.Field)) return $"없는 필드: {c.Field}";
        }
        return null;
    }

    private static string? StepProblem(FlowStep step, string input, IReadOnlyList<string> fields, FlowEnv env)
    {
        var rowish = input is "rows" or "row";
        string? NeedField(string? f) => string.IsNullOrEmpty(f) ? "필드를 고르세요" : !fields.Contains(f) ? $"없는 필드: {f}" : null;
        string? BaseField(string f) => env.View ? DocField(f, env) : env.Base.Contains(f) ? null : $"이 행에 없는 열: {f}";
        switch (step.Op)
        {
            case "filter": return CondProblem(step.Cond, input, fields, BaseField);
            case "if": return CondProblem(step.Cond, input == "row" ? "row" : "value", fields, BaseField);
            case "sort": return step.Field is null ? null : rowish ? NeedField(step.Field) : "값목록은 필드 없이 정렬합니다";
            case "distinct":
                if (step.Fields is not { Length: > 0 }) return null;
                if (!rowish) return "값목록은 기준 필드 없이 고유를 셉니다";
                foreach (var f in step.Fields) { var p = NeedField(f); if (p is not null) return p; }
                return null;
            case "take": case "nth": return (step.N ?? 0) >= 1 ? null : "1 이상의 수를 적으세요";
            case "abbrev":
            {
                if ((step.N ?? 0) < 1) return "보일 개수는 1 이상입니다";
                var ph = Placeholders(step.Text ?? "", out var error);
                if (ph is null) return error;
                var bad = ph.FirstOrDefault(p => p is not ("앞" or "나머지"));
                return bad is null ? null : $"줄여쓰기에는 {{앞}}·{{나머지}}만 씁니다: {{{bad}}}";
            }
            case "format":
            {
                var ph = Placeholders(step.Text ?? "", out var error);
                if (ph is null) return error;
                var bad = ph.FirstOrDefault(p => !fields.Contains(p));
                return bad is null ? null : $"서식에 없는 필드: {{{bad}}}";
            }
            case "get": return NeedField(step.Field);
            case "sum": case "min": case "max": case "avg":
                if (input == "rows") return NeedField(step.Field);
                return step.Field is not null ? "값목록에는 필드를 적지 않습니다" : null;
            case "substr": return (step.N ?? 0) >= 1 && (step.Length ?? -1) >= 0 ? null : "시작은 1 이상, 길이는 0 이상입니다";
            case "padStart": return (step.Length ?? 0) >= 1 && (step.Text ?? "").Length >= 1 ? null : "길이와 채울 글자를 적으세요";
            case "ifEmpty": return step.Field is not null ? BaseField(step.Field) : null;
            case "map": return step.Pairs is { Length: > 0 } ? null : "대응할 값을 하나 이상 적으세요";
            case "cases":
            {
                var branches = step.Branches ?? [];
                if (branches.Length == 0) return "경우를 하나 이상 적으세요";
                if (branches.Length > MaxBranches) return $"경우는 {MaxBranches}개까지입니다";
                for (var i = 0; i < branches.Length; i++)
                    if (CondProblem(branches[i].Cond, "value", fields, BaseField) is { } problem) return $"{i + 1}번째 경우: {problem}";
                return null;
            }
            case "add": case "sub": case "mul": case "div":
                if (step.Field is not null) return BaseField(step.Field);
                return step.Op == "div" && step.Value == 0 ? "0 으로 나눌 수 없습니다" : null;
            case "dateDiff": return !string.IsNullOrEmpty(step.Field) ? BaseField(step.Field) : "뺄 날짜 열을 고르세요";
            case "dateFormat": return !string.IsNullOrEmpty(step.Text) ? null : "서식을 적으세요";
            default: return null;
        }
    }

    /// <summary>맞춤 뷰에서 단계가 읽는 행의 열(<c>공고.공고명</c>)이 그 문서에 있는가. 없으면 오류 글.</summary>
    private static string? DocField(string field, FlowEnv env)
    {
        if (Qualified(field) is not { } q) return $"문서를 붙여 적으세요 — ‘공고.{field}’처럼 읽습니다";
        var word = DocWords[q.Kind];
        return env.Linked.TryGetValue(q.Kind, out var link) && link.Fields.Contains(q.Column) ? null : $"{word}에 없는 필드: {q.Column}";
    }

    /// <summary>
    /// 꼴 검사(§4.2·§4.4). 오류는 그것을 만든 단계에 붙고, 첫 오류 뒤의 단계는 꼴을 모른다(null).
    /// 화면의 <c>checkFlow</c> 와 같은 결과를 낸다.
    /// </summary>
    public static FlowChecked Check(FlowDefinition flow, FlowEnv env)
    {
        var n = flow.Steps.Length + 1;
        var shapes = new string?[n];
        var fields = new IReadOnlyList<string>[n];
        for (var i = 0; i < n; i++) fields[i] = [];
        var errors = new List<FlowIssue>();
        var source = SourceShape(flow.From, env, out var sourceError);
        if (source is null)
        {
            errors.Add(new FlowIssue(0, sourceError!));
            return new FlowChecked(shapes, fields, errors, null);
        }
        shapes[0] = source.Value.Shape;
        fields[0] = source.Value.Fields;
        for (var i = 0; i < flow.Steps.Length; i++)
        {
            var step = flow.Steps[i];
            var input = shapes[i]!;
            if (!ByOp.TryGetValue(step.Op, out var verb))
            {
                errors.Add(new FlowIssue(i + 1, $"알 수 없는 연산: {step.Op}"));
                break;
            }
            var match = verb.Io.Where(io => io.In == input).Select(io => io.Out).FirstOrDefault();
            if (match is null)
            {
                var accepts = string.Join("·", verb.Io.Select(io => Shapes[io.In]).Distinct());
                errors.Add(new FlowIssue(i + 1, $"‘{VerbWord(step)}’{은는(VerbWord(step))[VerbWord(step).Length..]} {Shapes[input]}에 쓸 수 없습니다. {accepts}에 쓰는 동사입니다"));
                break;
            }
            var problem = StepProblem(step, input, fields[i], env);
            if (problem is not null)
            {
                errors.Add(new FlowIssue(i + 1, problem));
                break;
            }
            shapes[i + 1] = match;
            fields[i + 1] = match is "rows" or "row" ? fields[i] : [];
        }
        var final = errors.Count > 0 ? null : shapes[n - 1];
        if (final is not null && !Ends.Contains(final))
            errors.Add(new FlowIssue(n - 1, $"마지막 결과가 {Shapes[final]}입니다. 값·수·날짜 중 하나로 끝나야 저장할 수 있습니다", true));
        return new FlowChecked(shapes, fields, errors, errors.Count > 0 ? null : final);
    }

    // ── 수 읽기 ─────────────────────────────────────────────

    private static readonly Regex NumberText = new(@"^[-+]?([0-9]+\.?[0-9]*|\.[0-9]+)$", RegexOptions.CultureInvariant);

    /// <summary>
    /// 글을 수로 — 쉼표를 걷고 앞뒤 공백을 걷는다. 못 읽으면 null. 화면의 <c>toNumber</c>, SQL 의
    /// <see cref="FlowCompiler"/> 숫자 판정과 같은 규칙이다(지수 표기는 받지 않는다).
    /// </summary>
    public static double? ToNumber(string text)
    {
        var t = text.Replace(",", "").Trim();
        if (t.Length == 0 || !NumberText.IsMatch(t)) return null;
        return double.Parse(t, NumberStyles.Float, CultureInfo.InvariantCulture);
    }
}

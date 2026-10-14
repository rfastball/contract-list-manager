using System.Text.Json;

namespace Pclm.Core.Storage;

/// <summary>
/// 가공열 — 식이 채우는 열. 값을 저장하지 않고 순수 SQLite 식으로 계산하므로 앱 전용 함수나 캐시가 없어
/// 외부 읽기 연결에서도 같은 값이 나오며 재수집 즉시 반영된다. 식은 흐름식 v2(<see cref="Flow"/>)이고
/// SQL 로 옮기는 것은 <see cref="FlowCompiler"/> 다. 품목은 부모의 정확한 차수에 묶인다.
/// </summary>
public static class ComputedColumn
{
    /// <summary>v1 가공식(<see cref="ColumnFormula"/>)을 읽고 쓰는 꼴. 앞 판 화면이 이 꼴로 보낸다.</summary>
    public static readonly JsonSerializerOptions Json = new(JsonSerializerDefaults.Web);

    /// <summary>저장된·화면이 보낸 가공식을 v2 로 읽는다(v1 은 옮긴다).</summary>
    public static FlowFormula Parse(string json) => Flow.Parse(json);

    /// <summary>
    /// 화면이 보낸 가공식을 이 자료 종류로 읽는다. v1 이면 옮기기 전에 <c>pick</c> 의 필드를 본다 — 옮기면
    /// 뒤에서 쓰이지 않는 <c>pick</c> 은 사라지므로, 없는 필드를 고른 v1 이 조용히 통과하지 않게 한다.
    /// </summary>
    public static FlowFormula ParseFor(string entity, string json)
    {
        var known = Fields(entity);
        JsonDocument document;
        try { document = JsonDocument.Parse(json); }
        catch (JsonException) { throw new InvalidOperationException("가공식 형식이 올바르지 않습니다."); }
        using (document)
        {
            if (document.RootElement.ValueKind == JsonValueKind.Object
                && document.RootElement.TryGetProperty("version", out var version) && version.ValueKind == JsonValueKind.Number
                && version.GetInt32() == 1
                && document.RootElement.TryGetProperty("steps", out var steps) && steps.ValueKind == JsonValueKind.Array)
            {
                foreach (var step in steps.EnumerateArray())
                {
                    if (step.ValueKind != JsonValueKind.Object || !step.TryGetProperty("op", out var op)
                        || op.ValueKind != JsonValueKind.String || op.GetString() != "pick") continue;
                    var fields = step.TryGetProperty("fields", out var f) && f.ValueKind == JsonValueKind.Array
                        ? f.EnumerateArray().Select(x => x.ValueKind == JsonValueKind.String ? x.GetString()! : "").ToList() : [];
                    if (fields.Count == 0 || fields.Distinct().Count() != fields.Count || fields.Any(x => !known.ContainsKey(x)))
                        throw new InvalidOperationException("선택한 필드가 없거나 중복되었습니다.");
                }
            }
        }
        return Parse(json);
    }

    /// <summary>뷰의 한 칸이 될 식. 바깥 행은 <c>현재</c>. 꼴 검사를 지나지 못하면 단계 번호를 담아 던진다.</summary>
    public static string Expression(string entity, string json) => Expression(entity, Parse(json));

    /// <summary>
    /// 흐름식은 컴파일하고, SQL 열은 모양을 본 뒤 그 글을 감싼다(§11.5). SQL 의 읽기 권한은 여기서 보지 않는다 —
    /// 저장과 뷰 짓기가 <see cref="SqlColumn.Prepare"/> 로 따로 본다.
    /// </summary>
    public static string Expression(string entity, FlowFormula formula, FlowEnv? env = null)
    {
        if (formula.Source != "sql") return FlowCompiler.Expression(entity, formula, FlowCompiler.Target.Internal, env);
        SqlColumn.CheckShape(formula.Sql);
        return SqlColumn.Expression(formula.Sql!);
    }

    /// <summary>끝 꼴 — <c>value</c>·<c>number</c>·<c>date</c>. SQL 열은 글이다.</summary>
    public static string FinalShape(string entity, string json) => FlowCompiler.FinalShape(entity, Parse(json));

    public static IReadOnlyDictionary<string, string> Fields(string entity) => entity switch
    {
        "notice" => new Dictionary<string, string>
        {
            ["세부품명"] = "item_name", ["세부품명번호"] = "detail_item_number",
            ["물품식별번호"] = "item_id_number", ["규격"] = "specification",
            ["수요기관"] = "demand_agency", ["단위"] = "unit", ["수량"] = "quantity",
            ["추정단가"] = "unit_price",
            ["납품장소"] = "delivery_place", ["인도조건"] = "delivery_terms",
        },
        "contract" => new Dictionary<string, string>
        {
            ["품명"] = "item_name", ["물품분류번호"] = "classification_number",
            ["물품식별번호"] = "item_id_number", ["규격"] = "specification",
            ["수요기관"] = "demand_agency", ["단위"] = "unit", ["수량"] = "quantity",
            ["단가"] = "unit_price", ["금액"] = "amount",
            ["인도조건"] = "delivery_terms",
        },
        "request" => new Dictionary<string, string>
        {
            ["세부품명"] = "item_name", ["세부품명번호"] = "detail_item_number",
            ["물품식별번호"] = "item_id_number", ["규격"] = "specification",
            ["단위"] = "unit", ["수량"] = "quantity", ["조달요구번호"] = "request_number",
            ["단가"] = "unit_price", ["금액"] = "amount",
            ["납품장소"] = "delivery_place", ["인도조건"] = "delivery_terms",
        },
        _ => throw new InvalidOperationException("가공열의 자료 종류가 올바르지 않습니다."),
    };

    internal static string Quote(string value) => "'" + value.Replace("'", "''") + "'";
    private static string Identifier(string value) => "\"" + value.Replace("\"", "\"\"") + "\"";

    /// <summary>
    /// 품목 원천 — 이 문서의 정확한 차수에 속한 품목 행. <c>__order</c> 는 순번이다. 미리보기 원본과 실제 계산이
    /// 같은 이름·순서·금액 표기를 쓴다(금액은 계약면 품목 뷰와 같은 자릿점).
    /// </summary>
    internal static string SourceSql(string entity, string baseExpression, string revisionExpression) =>
        "SELECT i.line_no AS __order, " + string.Join(", ", Fields(entity).Select(f =>
            $"{(f.Value is "unit_price" or "amount" ? Views.Money($"i.{f.Value}") : $"coalesce(CAST(i.{f.Value} AS TEXT), '')")} AS {Identifier(f.Key)}")) +
        $" FROM {entity}_item i WHERE i.{entity}_base = {baseExpression} AND i.seq = {revisionExpression}";
}

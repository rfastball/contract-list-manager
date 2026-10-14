using System.Globalization;
using System.Text.Json;
using System.Text.RegularExpressions;

namespace Pclm.Core.Storage;

/// <summary>
/// 흐름식 → 순수 SQLite 스칼라 식(docs/column-management.md §11.3). 앱 전용 함수가 없어 외부 읽기 연결에서도
/// 같은 값이 나온다. 바깥 행은 별칭 <c>현재</c> 로 읽는다 — 정정(<c>field_override</c>)을 마친 면 뷰의 값이다.
///
/// <para>출력기가 둘이다. <see cref="Target.Internal"/> 은 뷰에 들어가는 식으로, 품목을 <b>내부 표의 정확한
/// 차수</b>에서 읽고 연결 문서는 그 개체의 덮개 식을 다시 불러 읽는다 — 공개 뷰를 읽지 않으므로 뷰 사이 순환이
/// 생기지 않고, 옛 차수 행도 제 차수의 품목으로 계산된다. <see cref="Target.Public"/> 은 「SQL로 꺼내기」로,
/// 같은 단계를 계약면 뷰 위에 옮겨 쓴다 — 계약면 뷰가 최신 차수만 내므로 그 기준이다. 둘은 최신 차수에서 같다.</para>
///
/// <para>꼴마다 SQL 조각의 모양이 정해져 있다: 행목록은 <c>(__order, 필드…)</c> 를 내는 부질의, 값목록은
/// <c>(__order, __value)</c>, 행은 0~1행 부질의, 값은 NULL 이 아닌 글, 수·날짜는 NULL 일 수 있는 스칼라.
/// 순서는 언제나 <c>__order</c> 가 쥔다 — 정렬은 그것을 다시 매긴다.</para>
/// </summary>
public static class FlowCompiler
{
    public enum Target { Internal, Public }

    /// <summary>앞부분 컴파일의 결과. 목록·행이면 <see cref="Sql"/> 이 부질의, 값·수·날짜면 스칼라 식이다.</summary>
    public sealed record Part(string Shape, string Sql, IReadOnlyList<string> Fields)
    {
        public bool IsRelation => Shape is "rows" or "values" or "row";
    }

    /// <summary>바깥 행의 별칭(§5).</summary>
    public const string Self = "현재";

    /// <summary>JS 의 <c>trim</c> 이 걷는 공백 — 전각 공백(U+3000)이 문서에 흔하다.</summary>
    private const string Spaces =
        "char(9,10,11,12,13,32,160,5760,8192,8193,8194,8195,8196,8197,8198,8199,8200,8201,8202,8232,8233,8239,8287,12288,65279)";

    // ── 겉 ─────────────────────────────────────────────

    /// <summary>
    /// 뷰의 한 칸이 될 식. 꼴 검사를 지나지 못하면 단계 번호를 담아 던진다(「3단계: …」).
    /// 끝 꼴이 수면 정수는 천단위·소수는 그대로, 날짜는 <c>YYYY/MM/DD</c>(§4.6). 모두 <c>coalesce(…, '')</c>.
    /// </summary>
    /// <param name="env">꼴 검사의 재료. 없으면 <see cref="Flow.EnvFor"/> — 맞춤 뷰는 <see cref="Flow.EnvForView"/> 를 준다(그때 <paramref name="entity"/> 는 쓰지 않는다).</param>
    public static string Expression(string entity, FlowFormula formula, Target target = Target.Internal, FlowEnv? env = null)
    {
        if (formula.Source != "flow" || formula.Flow is null)
            throw new InvalidOperationException("SQL 가공열은 아직 받지 않습니다.");
        var part = Prefix(entity, formula.Flow, formula.Flow.Steps.Length, target, requireEnd: true, env);
        return $"coalesce({Render(part)}, '')";
    }

    /// <summary>「SQL로 꺼내기」 — 계약면 뷰만 읽는 스칼라 부질의 하나(§5).</summary>
    public static string Eject(string entity, FlowFormula formula, FlowEnv? env = null)
    {
        var expression = Expression(entity, formula, Target.Public, env);
        var steps = formula.Flow!.Steps.Select((step, i) =>
            $"  -- {i + 1}단계: {Flow.Verbs.First(v => v.Op == step.Op).Word}");
        var notes = string.Join("\n", steps);
        return "SELECT\n" + (notes.Length == 0 ? "" : notes + "\n") +
               "  " + SqlLine.Replace(expression, m => m.Value == "\n" ? "\n  " : m.Value) + " AS 결과";
    }

    /// <summary>끝 꼴(<c>value</c>·<c>number</c>·<c>date</c>). 검사를 지나지 못하면 던진다.</summary>
    public static string FinalShape(string entity, FlowFormula formula, FlowEnv? env = null)
    {
        if (formula.Source != "flow" || formula.Flow is null) return "value";
        var checkedFlow = Flow.Check(formula.Flow, env ?? Flow.EnvFor(entity));
        if (!checkedFlow.Ok) throw new InvalidOperationException(checkedFlow.Errors[0].Describe());
        return checkedFlow.Final!;
    }

    /// <summary>
    /// 앞부분 컴파일 — 단계 <paramref name="steps"/> 개까지만 잘라 그 시점의 값을 낸다(§11.3, 보표가 쓴다).
    /// 0 이면 원천이다. 그 앞에 꼴 오류가 있으면 단계 번호를 담아 던진다.
    /// </summary>
    public static Part Prefix(string entity, FlowDefinition flow, int steps, Target target = Target.Internal, FlowEnv? env = null) =>
        Prefix(entity, flow, steps, target, requireEnd: false, env);

    private static Part Prefix(string entity, FlowDefinition flow, int steps, Target target, bool requireEnd, FlowEnv? env)
    {
        if (steps < 0 || steps > flow.Steps.Length) throw new ArgumentOutOfRangeException(nameof(steps));
        env ??= Flow.EnvFor(entity);
        var checkedFlow = Flow.Check(flow, env);
        var blocking = checkedFlow.Errors.FirstOrDefault(e => e.Index <= steps && (requireEnd || !e.Final));
        if (blocking is not null) throw new InvalidOperationException(blocking.Describe());

        var source = new Sources(entity, target, env);
        var part = source.From(flow.From, checkedFlow.Fields[0]);
        for (var i = 0; i < steps; i++)
            part = Apply(flow.Steps[i], part, checkedFlow.Shapes[i + 1]!, checkedFlow.Fields[i + 1], source.RowColumn);
        return part;
    }

    /// <summary>끝 값을 열에 찍힐 글로(§4.6). 목록·행이면 쓸 수 없다.</summary>
    public static string Render(Part part) => part.Shape switch
    {
        "value" => part.Sql,
        "number" => $"""
            (SELECT CASE WHEN 원값 IS NULL THEN '' ELSE (
                SELECT CASE WHEN 정밀값 = CAST(정밀값 AS INTEGER) THEN printf('%,d', CAST(정밀값 AS INTEGER)) ELSE printf('%.15g', 정밀값) END
                FROM (SELECT CAST(printf('%.15g', 원값) AS REAL) AS 정밀값)) END
             FROM (SELECT {part.Sql} AS 원값))
            """,
        "date" => $"replace({part.Sql}, '-', '/')",
        _ => throw new InvalidOperationException($"마지막 결과가 {Flow.Shapes[part.Shape]}입니다. 값·수·날짜 중 하나로 끝나야 저장할 수 있습니다"),
    };

    // ── 원천 ─────────────────────────────────────────────

    /// <summary>
    /// 원천을 읽는 길. 출력기 둘이 갈리는 곳은 여기뿐이다 — 단계는 같은 SQL 을 쌓는다.
    ///
    /// <para>맞춤 뷰(<see cref="FlowEnv.View"/>)면 바깥 행이 <c>현재</c> 하나가 아니라 같은 조달 건의 세 문서다
    /// (docs/user-views.md §5). 뷰가 면 뷰를 별칭 <c>접수</c>·<c>공고</c>·<c>계약</c> 으로 걸어 두므로 연결 문서를 찾아가지
    /// 않고 그 별칭을 바로 읽는다 — 문서가 없으면 그 별칭이 NULL 이라 빈 값이 된다. 품목만 그 문서의 본번호·차수로 찾는다.</para>
    /// </summary>
    private sealed class Sources(string entity, Target target, FlowEnv env)
    {
        private static readonly IReadOnlyDictionary<string, (string Key, string Base, string View, string ItemView)> Faces =
            new Dictionary<string, (string, string, string, string)>
            {
                ["notice"] = ("입찰공고번호", "공고본번호", "v_공고", "v_공고품목"),
                ["contract"] = ("계약번호", "계약본번호", "v_계약", "v_계약품목"),
                ["request"] = ("접수번호", "접수본번호", "v_접수", "v_접수품목"),
            };

        private static string Own(string column) => $"{Self}.{Ident(column)}";

        public Part From(FlowSource from, IReadOnlyList<string> fields) => from.Kind switch
        {
            "items" => new Part("rows", Items(), fields),
            "field" => new Part("value", Own(from.Field!), []),
            "text" => new Part("value", Lit(from.Text!), []),
            "number" => new Part("number", Num(from.Value!.Value), []),
            "linked" => Linked(from, fields),
            _ => throw new InvalidOperationException($"알 수 없는 원천: {from.Kind}"),
        };

        /// <summary>
        /// 단계가 읽는 행의 열(<c>비면</c>·사칙·<c>날짜차이</c>). 가공열은 <c>현재</c> 의 열이고, 맞춤 뷰는 <c>공고.공고명</c> 꼴을
        /// 그 문서의 별칭으로 읽는다 — 문서가 없으면 빈 글이다.
        /// </summary>
        public string RowColumn(string field)
        {
            if (!env.View) return Own(field);
            var (kind, column) = Flow.Qualified(field) ?? throw new InvalidOperationException($"문서를 붙여 적으세요: {field}");
            return $"coalesce({Doc(kind)}.{Ident(column)}, '')";
        }

        /// <summary>맞춤 뷰의 바깥 행 별칭 — 그 문서의 면 뷰 한 줄(없으면 NULL).</summary>
        private static string Doc(string kind) => Flow.DocWords[kind];

        private Part Linked(FlowSource from, IReadOnlyList<string> fields)
        {
            var kind = from.Link!;
            if (env.View) return Document(kind, from, fields);
            if (from.Items == true) return new Part("rows", LinkedItems(kind), fields);
            var many = Flow.EnvFor(entity).Linked[kind].Many;
            var rows = LinkedRows(kind);
            if (from.Field is null) return new Part(many ? "rows" : "row", rows, fields);
            return new Part("value", $"coalesce((SELECT {Ident(from.Field)} FROM ({rows})), '')", []);
        }

        /// <summary>
        /// 맞춤 뷰의 문서 하나 — 별칭을 바로 읽는다. 행은 그 문서가 있을 때만 한 줄이고, 품목은 안쪽이면 그 문서의 정확한
        /// 차수에서, 바깥(「SQL로 꺼내기」)이면 계약면 품목 뷰에서 그 문서의 번호로 읽는다.
        /// </summary>
        private Part Document(string kind, FlowSource from, IReadOnlyList<string> fields)
        {
            var doc = Doc(kind);
            var face = Faces[kind];
            if (from.Items == true)
                return new Part("rows", target == Target.Internal
                    ? ComputedColumn.SourceSql(kind, $"{doc}.{Ident(face.Base)}", $"{doc}.{Ident("차수")}")
                    : $"SELECT 순번 AS __order, {Columns(ComputedColumn.Fields(kind).Keys)} FROM {face.ItemView} WHERE {face.Key} = {doc}.{Ident(face.Key)}",
                    fields);
            if (from.Field is not null) return new Part("value", $"coalesce({doc}.{Ident(from.Field)}, '')", []);
            var columns = string.Join(", ", fields.Select(f => $"coalesce({doc}.{Ident(f)}, '') AS {Ident(f)}"));
            return new Part("row", $"SELECT 0 AS __order, {columns} WHERE {doc}.{Ident(face.Base)} IS NOT NULL", fields);
        }

        private string Items()
        {
            if (target == Target.Internal)
                return ComputedColumn.SourceSql(entity, Own(Faces[entity].Base), Own("차수"));
            var face = Faces[entity];
            return $"SELECT 순번 AS __order, {Columns(ComputedColumn.Fields(entity).Keys)} FROM {face.ItemView} " +
                   $"WHERE {face.Key} = {Own(face.Key)}";
        }

        // ── 안쪽: 내부 표 + 덮개 식 ──

        /// <summary>이 행이 속한 공고건. 연결의 끝점은 건이다(스키마 V15).</summary>
        private string Group() => entity switch
        {
            "contract" => $"(SELECT pl.notice_group FROM project_link pl WHERE pl.contract_base = {Own("계약본번호")})",
            "notice" => $"(SELECT s.group_base FROM notice_series s WHERE s.notice_base = {Own("공고본번호")})",
            _ => $"(SELECT rl.notice_group FROM request_link rl WHERE rl.request_base = {Own("접수본번호")})",
        };

        private string RequestBase() => $"(SELECT rl.request_base FROM request_link rl WHERE rl.notice_group = {Group()})";

        private string LinkedRows(string kind)
        {
            if (target == Target.Public) return PublicRows(kind);
            return kind switch
            {
                // 건의 현행 공고 — NoticeGroups.현행공고 하나가 정본이다.
                "notice" => $"""
                    SELECT 0 AS __order, {Views.Face("notice")}
                    FROM notice n
                    JOIN ({NoticeGroups.현행공고}) 현행 ON 현행.notice_base = n.notice_base AND 현행.seq = n.seq
                    WHERE 현행.group_base = {Group()}
                    """,
                "request" => $"""
                    SELECT 0 AS __order, {Views.Face("request")}
                    FROM request r
                    WHERE r.request_base = {RequestBase()}
                      AND CAST(r.seq AS INTEGER) = (SELECT max(CAST(x.seq AS INTEGER)) FROM request x WHERE x.request_base = r.request_base)
                    """,
                _ => $"""
                    SELECT c.contract_base AS __order, {Views.Face("contract")}
                    FROM contract c
                    LEFT JOIN counterparty p ON c.counterparty_number = p.business_number
                    WHERE c.contract_base IN (SELECT pl.contract_base FROM project_link pl WHERE pl.notice_group = {Group()})
                      AND CAST(c.seq AS INTEGER) = (SELECT max(CAST(x.seq AS INTEGER)) FROM contract x WHERE x.contract_base = c.contract_base)
                    """,
            };
        }

        private string LinkedItems(string kind)
        {
            if (target == Target.Public)
                return $"SELECT 순번 AS __order, {Columns(ComputedColumn.Fields(kind).Keys)} FROM {Faces[kind].ItemView} " +
                       $"WHERE {Faces[kind].Key} = ({PublicKey(kind)})";
            return kind switch
            {
                "notice" => ComputedColumn.SourceSql("notice",
                    $"(SELECT 현행.notice_base FROM ({NoticeGroups.현행공고}) 현행 WHERE 현행.group_base = {Group()})",
                    $"(SELECT 현행.seq FROM ({NoticeGroups.현행공고}) 현행 WHERE 현행.group_base = {Group()})"),
                "request" => ComputedColumn.SourceSql("request", RequestBase(),
                    $"(SELECT x.seq FROM request x WHERE x.request_base = {RequestBase()} ORDER BY CAST(x.seq AS INTEGER) DESC LIMIT 1)"),
                _ => throw new InvalidOperationException("연결계약은 여러 건이라 품목을 바로 읽을 수 없습니다. 한 건을 먼저 고르세요."),
            };
        }

        // ── 바깥: 계약면 뷰 ──

        /// <summary>
        /// 연결 문서의 키를 계약면 뷰로 찾는다. 계약 줄과 접수 줄은 <c>v_통합</c> 이 건의 현행 공고에 잇고,
        /// 공고 줄은 그 공고 문서가 몇 번째 공고든 건에 잇기 위해 <c>v_통합차수</c> 를 본다.
        /// </summary>
        private string PublicKey(string kind)
        {
            var own = Faces[entity].Key;
            var unified = entity == "notice" ? "v_통합차수" : "v_통합";
            return $"SELECT {Faces[kind].Key} FROM {unified} WHERE {own} = {Own(own)} AND {Faces[kind].Key} <> '' LIMIT 1";
        }

        private string PublicRows(string kind)
        {
            var columns = Columns(Views.BaseColumns(kind));
            if (kind != "contract")
                return $"SELECT 0 AS __order, {columns} FROM {Faces[kind].View} WHERE {Faces[kind].Key} = ({PublicKey(kind)})";
            var own = Faces[entity].Key;
            var unified = entity == "notice" ? "v_통합차수" : "v_통합";
            return $"SELECT 계약본번호 AS __order, {columns} FROM v_계약 " +
                   $"WHERE 계약번호 IN (SELECT 계약번호 FROM {unified} WHERE {own} = {Own(own)})";
        }
    }

    // ── 단계 ─────────────────────────────────────────────

    private static Part Apply(FlowStep step, Part part, string output, IReadOnlyList<string> fields, Func<string, string> row)
    {
        var rel = part.Sql;
        var cols = part.Shape == "values" ? "__value" : Columns(part.Fields);
        Part Rel(string shape, string sql) => new(shape, sql, shape is "rows" or "row" ? fields : []);
        Part Scalar(string shape, string sql) => new(shape, sql, []);
        // 값 하나를 v 로 한 번만 읽는다 — 식이 v 를 여러 번 불러도 앞 단계 SQL 이 겹쳐 불어나지 않는다.
        Part Over(string shape, string body) => Scalar(shape, $"(SELECT {body} FROM (SELECT {rel} AS v))");

        switch (step.Op)
        {
            case "filter":
                return Rel("rows", $"SELECT * FROM ({rel}) WHERE {Condition(step.Cond!, Ident)}");

            case "sort":
            {
                var desc = step.Dir == "desc" ? " DESC" : "";
                if (part.Shape == "rows" && step.Field is null)
                    return desc.Length == 0 ? part
                        : Rel("rows", $"SELECT row_number() OVER (ORDER BY __order DESC) AS __order, {cols} FROM ({rel})");
                var key = part.Shape == "rows" ? Ident(step.Field!) : "__value";
                // 모두 수로 읽히면 수로, 아니면 글로 견준다. 같으면 원래 차례(안정 정렬).
                return new Part(part.Shape, $"""
                    SELECT row_number() OVER (ORDER BY CASE WHEN __a = 1 THEN __k END{desc}, CASE WHEN __a = 1 THEN NULL ELSE __t END{desc}, __order) AS __order, {cols}
                    FROM (SELECT *, {Number(key)} AS __k, {key} AS __t,
                                 min(CASE WHEN {Number(key)} IS NULL THEN 0 ELSE 1 END) OVER () AS __a
                          FROM ({rel}))
                    """, part.Fields);
            }

            case "distinct":
            {
                var keys = part.Shape == "values" ? "__value"
                    : Columns(step.Fields is { Length: > 0 } chosen ? chosen : part.Fields);
                return new Part(part.Shape, $"""
                    SELECT __order, {cols}
                    FROM (SELECT *, row_number() OVER (PARTITION BY {keys} ORDER BY __order) AS __rn FROM ({rel}))
                    WHERE __rn = 1
                    """, part.Fields);
            }

            case "take":
                return new Part(part.Shape, $"SELECT * FROM ({rel}) ORDER BY __order LIMIT {step.N}", part.Fields);

            case "nth": case "first": case "last":
            {
                var pick = step.Op == "last" ? "ORDER BY __order DESC LIMIT 1"
                    : $"ORDER BY __order LIMIT 1 OFFSET {(step.Op == "nth" ? step.N!.Value - 1 : 0)}";
                return part.Shape == "values"
                    ? Scalar("value", $"coalesce((SELECT __value FROM ({rel}) {pick}), '')")
                    : Rel("row", $"SELECT * FROM ({rel}) {pick}");
            }

            case "format":
            {
                var text = Template(step.Text ?? "");
                return part.Shape == "row"
                    ? Scalar("value", $"coalesce((SELECT {text} FROM ({rel})), '')")
                    : Rel("values", $"SELECT __order, {text} AS __value FROM ({rel})");
            }

            case "get":
                return part.Shape == "row"
                    ? Scalar("value", $"coalesce((SELECT {Ident(step.Field!)} FROM ({rel})), '')")
                    : Rel("values", $"SELECT __order, {Ident(step.Field!)} AS __value FROM ({rel})");

            case "join":
                return Scalar("value", $"coalesce((SELECT group_concat(__value, {Lit(step.Text ?? "")} ORDER BY __order) FROM ({rel})), '')");

            case "abbrev":
            {
                var n = step.N!.Value;
                var head = Lit("{앞}");
                var rest = Lit("{나머지}");
                return Scalar("value", $"""
                    coalesce((SELECT CASE WHEN count(*) <= {n} THEN group_concat(__value, ', ' ORDER BY __order)
                        ELSE replace(replace({Lit(step.Text ?? "")},
                            {head}, group_concat(CASE WHEN __rn <= {n} THEN __value END, ', ' ORDER BY __order)),
                            {rest}, CAST(count(*) - {n} AS TEXT)) END
                      FROM (SELECT __order, __value, row_number() OVER (ORDER BY __order) AS __rn FROM ({rel}))), '')
                    """);
            }

            case "count":
                return Scalar("number", $"(SELECT count(*) FROM ({rel}))");

            case "sum": case "min": case "max": case "avg":
            {
                var column = part.Shape == "rows" ? Ident(step.Field!) : "__value";
                return Scalar("number", $"(SELECT {step.Op}({Number(column)}) FROM ({rel}))");
            }

            case "if" when part.Shape == "row":
                return Scalar("value",
                    $"coalesce((SELECT CASE WHEN {Condition(step.Cond!, Ident)} THEN {Lit(step.Then ?? "")} ELSE {Lit(step.Else ?? "")} END FROM ({rel})), {Lit(step.Else ?? "")})");
        }

        var operand = step.Field is not null ? Number(row(step.Field)) : Num(step.Value ?? 0);
        string Scale()
        {
            var digits = step.N ?? 0;
            if (digits is < -15 or > 15) throw new InvalidOperationException("소수 자리는 -15~15 사이여야 합니다.");
            return Num(Math.Pow(10, digits));
        }
        switch (step.Op)
        {
            case "trim": return Over("value", $"trim(v, {Spaces})");
            case "replace": return Over("value", $"replace(v, {Lit(step.Text ?? "")}, {Lit(step.To ?? "")})");
            case "before":
            {
                var s = Lit(step.Text ?? "");
                return Over("value", $"CASE WHEN instr(v, {s}) > 0 THEN substr(v, 1, instr(v, {s}) - 1) ELSE v END");
            }
            case "after":
            {
                var s = Lit(step.Text ?? "");
                return Over("value", $"CASE WHEN instr(v, {s}) > 0 THEN substr(v, instr(v, {s}) + length({s})) ELSE '' END");
            }
            case "substr": return Over("value", $"substr(v, {step.N}, {step.Length})");
            case "padStart":
            {
                var length = step.Length!.Value;
                return Over("value",
                    $"CASE WHEN length(v) >= {length} THEN v ELSE substr(replace(hex(zeroblob({length})), '00', {Lit(step.Text!)}), 1, {length} - length(v)) || v END");
            }
            case "ifEmpty":
                return Over("value", $"CASE WHEN v = '' THEN {(step.Field is not null ? row(step.Field) : Lit(step.Text ?? ""))} ELSE v END");
            case "map":
                return Over("value",
                    $"CASE v {string.Join(" ", step.Pairs!.Select(p => $"WHEN {Lit(p[0])} THEN {Lit(p[1])}"))} ELSE {(step.Otherwise is not null ? Lit(step.Otherwise) : "v")} END");
            case "if":
                // 빈 필드는 지금 값, 적힌 필드는 바깥의 열이다(비면과 같은 자리).
                return Over("value", $"CASE WHEN {Condition(step.Cond!, f => f.Length == 0 ? "v" : row(f))} THEN {Lit(step.Then ?? "")} ELSE {Lit(step.Else ?? "")} END");
            case "cases":
                // 차례대로 견주어 처음 맞는 갈래가 이긴다. 맞는 것이 없으면 그외(없으면 빈 글).
                return Over("value",
                    $"CASE {string.Join(" ", step.Branches!.Select(b => $"WHEN {Condition(b.Cond, f => f.Length == 0 ? "v" : row(f))} THEN {Lit(b.Then)}"))} ELSE {Lit(step.Otherwise ?? "")} END");
            case "toNumber": return Over("number", Number("v"));
            case "add": return Over("number", $"v + ({operand})");
            case "sub": return Over("number", $"v - ({operand})");
            case "mul": return Over("number", $"v * ({operand})");
            case "div": return Over("number", $"v * 1.0 / ({operand})");
            case "round":
                // Math.round 처럼 절댓값에서 반올림한다(0.5 는 0 에서 멀어지게). 수학 함수를 쓰지 않는다 — 소비자에 없을 수 있다.
            {
                var scale = Scale();
                return Over("number", $"CASE WHEN v IS NULL THEN NULL ELSE (CASE WHEN v < 0 THEN -1 ELSE 1 END) * CAST(abs(v) * {scale} + 0.5 AS INTEGER) * 1.0 / {scale} END");
            }
            case "floor":
            {
                var scale = Scale();
                return Over("number", $"(SELECT CASE WHEN x IS NULL THEN NULL WHEN x < CAST(x AS INTEGER) THEN CAST(x AS INTEGER) - 1 ELSE CAST(x AS INTEGER) END * 1.0 / {scale} FROM (SELECT v * {scale} AS x))");
            }
            case "thousands": return Over("value", "CASE WHEN v IS NULL THEN '' ELSE printf('%,d', v) END");
            case "plain": return Over("value", "CASE WHEN v IS NULL THEN '' ELSE printf('%.15g', v) END");
            case "toDate": return Scalar("date", ToDate(rel));
            case "dateAdd":
            {
                var unit = step.Unit switch { "month" => "months", "year" => "years", _ => "days" };
                var n = step.N ?? 0;
                return Over("date", $"date(v, {Lit((n < 0 ? "-" : "+") + Math.Abs((long)n).ToString(CultureInfo.InvariantCulture) + " " + unit)})");
            }
            case "dateDiff":
                return Over("number", $"CAST(round(julianday(v) - julianday({ToDate(row(step.Field!))})) AS INTEGER)");
            case "year": return Over("number", "CAST(substr(v, 1, 4) AS INTEGER)");
            case "month": return Over("number", "CAST(substr(v, 6, 2) AS INTEGER)");
            case "quarter": return Over("number", "(CAST(substr(v, 6, 2) AS INTEGER) + 2) / 3");
            case "dateFormat": return Over("value", $"CASE WHEN v IS NULL THEN '' ELSE {DatePattern(step.Text ?? "")} END");
        }
        throw new InvalidOperationException($"이 꼴에서 돌 수 없는 연산: {step.Op}");
    }

    // ── 조각 ─────────────────────────────────────────────

    // SQL 글자·식별자 안의 줄바꿈은 결과 값의 일부이므로 들여쓰지 않는다.
    private static readonly Regex SqlLine = new(@"'(?:[^']|'')*'|""(?:[^""]|"""")*""|\n");
    private static readonly Regex Placeholder = new(@"\{([^{}]+)\}");
    private static readonly Regex DateToken = new("YYYY|MM|M|DD|D");

    /// <summary>서식 — <c>{필드}</c> 자리에 그 행의 값. 필드 값은 NULL 이 아니다(원천이 빈 글로 낸다).</summary>
    private static string Template(string text)
    {
        var parts = new List<string>();
        var end = 0;
        foreach (Match m in Placeholder.Matches(text))
        {
            if (m.Index > end) parts.Add(Lit(text[end..m.Index]));
            parts.Add(Ident(m.Groups[1].Value));
            end = m.Index + m.Length;
        }
        if (end < text.Length) parts.Add(Lit(text[end..]));
        return parts.Count == 0 ? "''" : string.Join(" || ", parts);
    }

    private static string DatePattern(string text)
    {
        var parts = new List<string>();
        var end = 0;
        foreach (Match m in DateToken.Matches(text))
        {
            if (m.Index > end) parts.Add(Lit(text[end..m.Index]));
            parts.Add(m.Value switch
            {
                "YYYY" => "substr(v, 1, 4)",
                "MM" => "substr(v, 6, 2)",
                "M" => "CAST(CAST(substr(v, 6, 2) AS INTEGER) AS TEXT)",
                "DD" => "substr(v, 9, 2)",
                _ => "CAST(CAST(substr(v, 9, 2) AS INTEGER) AS TEXT)",
            });
            end = m.Index + m.Length;
        }
        if (end < text.Length) parts.Add(Lit(text[end..]));
        return parts.Count == 0 ? "''" : string.Join(" || ", parts);
    }

    /// <summary>
    /// 글 → 수. 쉼표와 앞뒤 공백을 걷고, <c>[+-]?(숫자+.?숫자*|.숫자+)</c> 꼴만 읽는다 — 못 읽으면 NULL.
    /// <c>CAST('abc' AS REAL)</c> 은 0 을 내므로 꼴을 먼저 본다(<see cref="Flow.ToNumber"/> 와 같은 규칙).
    /// </summary>
    internal static string Number(string text)
    {
        var t = $"trim(replace({text}, ',', ''), {Spaces})";
        var u = $"(CASE WHEN {t} GLOB '[+-]*' THEN substr({t}, 2) ELSE {t} END)";
        return $"(CASE WHEN {u} <> '' AND {u} <> '.' AND {u} NOT GLOB '*[^0-9.]*' AND {u} NOT GLOB '*.*.*' THEN CAST({t} AS REAL) END)";
    }

    /// <summary>글 → 날짜(<c>YYYY-MM-DD</c>). 앞 10자를 보고 <c>/</c> 를 <c>-</c> 로. 없는 날(2월 30일)은 NULL.</summary>
    private static string ToDate(string text) =>
        $"(SELECT CASE WHEN t GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]' AND date(t) = t THEN t END " +
        $"FROM (SELECT replace(substr({text}, 1, 10), '/', '-') AS t))";

    /// <summary>
    /// 조건. 그리고가 또는보다 먼저 묶인다(SQL 과 같다). 수와 수면 수로, 아니면 글로 견준다 —
    /// 비교 값이 글자 그대로 정해져 있으므로 그것이 수인지는 여기서 정한다.
    /// </summary>
    private static string Condition(FlowClause[] cond, Func<string, string> subject)
    {
        var text = new System.Text.StringBuilder("(");
        for (var i = 0; i < cond.Length; i++)
        {
            var c = cond[i];
            if (i > 0) text.Append(c.Join == "or" ? " OR " : " AND ");
            var a = subject(c.Field);
            var (bs, bn) = c.Value switch
            {
                { ValueKind: JsonValueKind.Number } v => (JsNumber(v.GetDouble()), (double?)v.GetDouble()),
                { ValueKind: JsonValueKind.String } v => (v.GetString()!, Flow.ToNumber(v.GetString()!)),
                _ => ("", null),
            };
            var op = c.Cmp switch { "=" => "=", "≠" => "<>", ">" => ">", "<" => "<", "≥" => ">=", "≤" => "<=", _ => null };
            text.Append(c.Cmp switch
            {
                "비어있음" => $"{a} = ''",
                "안비어있음" => $"{a} <> ''",
                "포함" => $"instr({a}, {Lit(bs)}) > 0",
                _ when bn is null => $"{a} {op} {Lit(bs)}",
                _ => $"(CASE WHEN {Number(a)} IS NOT NULL THEN {Number(a)} {op} {Num(bn.Value)} ELSE {a} {op} {Lit(bs)} END)",
            });
        }
        return text.Append(')').ToString();
    }

    private static string Columns(IEnumerable<string> names) => string.Join(", ", names.Select(Ident));

    internal static string Ident(string name) => "\"" + name.Replace("\"", "\"\"") + "\"";

    internal static string Lit(string value) => "'" + value.Replace("'", "''") + "'";

    private static string Num(double value) =>
        value == Math.Floor(value) && Math.Abs(value) < 1e15
            ? ((long)value).ToString(CultureInfo.InvariantCulture)
            : value.ToString("R", CultureInfo.InvariantCulture);

    /// <summary>JS 의 <c>String(수)</c> — 조건의 값이 수일 때 글로 견주는 쪽에 쓴다.</summary>
    private static string JsNumber(double value) =>
        value == Math.Floor(value) && Math.Abs(value) < 1e21
            ? value.ToString("0", CultureInfo.InvariantCulture)
            : value.ToString("R", CultureInfo.InvariantCulture);
}

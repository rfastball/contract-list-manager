using Dapper;
using Microsoft.Data.Sqlite;

namespace Pclm.Core.Storage;

/// <summary>
/// SQL 가공열 — 사람이 고친 스칼라 부질의(docs/column-management.md §5·§11.5). 뷰에 그 글이 그대로 박히므로
/// <b>박기 전에 반드시 여기를 지난다</b>: 저장할 때도, 뷰를 다시 지을 때도.
///
/// <para>막는 것은 두 겹이다. 글의 모양(<see cref="CheckShape"/>)은 부질의 하나가 괄호 밖으로 새지 않음을 본다 —
/// 뷰 정의는 여러 문장을 한 번에 돌리는 자리라, 괄호를 닫고 <c>;</c> 를 치면 그 뒤가 쓰기 연결에서 돈다.
/// 읽는 것(<see cref="Prepare"/>)은 SQLite authorizer 가 본다. authorizer 는 뷰를 펼친 안쪽의 읽기에 그 뷰(또는
/// 뷰 안 CTE)의 이름을 넘긴다 — 실측: <c>notice_item</c> 을 바로 읽으면 빈 이름, <c>v_공고품목</c> 을 거치면
/// <c>v_공고품목</c>, <c>v_통합</c> 안의 CTE 를 거치면 <c>사슬</c>. 그래서 바깥(사용자 글)의 읽기는 계약면 뷰만 통과시키고,
/// 안쪽의 읽기는 계약면이 아닌 뷰(<c>v_가공열</c>·<c>v_계획</c>…) 안이 아니면 통과시킨다. 사용자 글에는
/// <c>WITH</c> 를 받지 않는다 — 제 CTE 를 세우면 그 안의 읽기가 「안쪽」으로 보고되어 이 가름을 비켜 간다.</para>
/// </summary>
public static class SqlColumn
{
    /// <summary>사용자 SQL 이 읽을 수 있는 뷰 — 계약면 뷰 열 개. <c>v_가공열</c> 은 다른 열의 식을 담아 뺀다.</summary>
    public static readonly IReadOnlySet<string> Readable = new HashSet<string>(StringComparer.Ordinal)
    {
        "v_통합", "v_통합차수", "v_접수", "v_접수품목", "v_공고", "v_공고품목", "v_공고차수", "v_계약", "v_계약차수", "v_계약품목",
    };

    /// <summary>
    /// 쓸 수 있는 함수. 소비자 SQLite 에 늘 있는 핵심 함수만 둔다 — 앱 전용·확장·파일 함수(<c>load_extension</c>·
    /// <c>readfile</c>)는 들지 않는다. LIKE·GLOB 연산자도 함수로 불려 함께 둔다.
    /// </summary>
    public static readonly IReadOnlySet<string> Functions = new HashSet<string>(StringComparer.OrdinalIgnoreCase)
    {
        "abs", "avg", "ceil", "char", "coalesce", "count", "date", "datetime", "floor", "glob", "group_concat", "hex",
        "ifnull", "iif", "instr", "julianday", "length", "like", "lower", "ltrim", "max", "min", "nullif", "printf",
        "replace", "round", "rtrim", "strftime", "substr", "substring", "sum", "time", "total", "trim", "typeof",
        "unicode", "upper", "zeroblob", "format", "concat", "concat_ws", "string_agg",
        // 창 함수. 뷰 안의 창 함수는 바깥에서 부른 것처럼 보고된다(실측: v_통합 의 row_number) — 뷰를 읽으려면 둔다.
        "row_number", "rank", "dense_rank", "percent_rank", "cume_dist", "ntile", "lag", "lead",
        "first_value", "last_value", "nth_value",
    };

    private const int MaxLength = 20_000;

    /// <summary>검증용 연결 — 읽기 전용으로 열고 <c>query_only</c> 를 켠다(트랜잭션을 열기 전에).</summary>
    public static SqliteConnection OpenChecked(Database database)
    {
        var connection = database.OpenReadOnly();
        connection.Execute("PRAGMA query_only = ON;");
        return connection;
    }
    private const int Ok = 0, Deny = 1;
    private const int Read = 20, Select = 21, Function = 31;

    /// <summary>뷰의 한 칸이 될 식 — 글이 언제나 글로, 빈 값은 빈 글로 나간다(NULL 이 새지 않게).</summary>
    public static string Expression(string sql) => $"coalesce(CAST((\n{sql}\n) AS TEXT), '')";

    /// <summary>
    /// 글의 모양: 비지 않았고, <c>SELECT</c> 로 시작하며, 문장이 하나이고(<c>;</c> 없음), 괄호·글자·식별자·주석이
    /// 모두 닫혔다. 이것을 지나야 <c>(…)</c> 로 감싼 자리 밖으로 새지 않는다. 글에 적힌 이름(맨 낱말과 따옴표 친
    /// 식별자)을 모아 낸다 — <see cref="Prepare"/> 가 스키마의 표·뷰 이름과 견준다.
    /// </summary>
    public static IReadOnlySet<string> CheckShape(string? sql)
    {
        var names = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        if (string.IsNullOrWhiteSpace(sql)) throw new InvalidOperationException("SQL이 비었습니다.");
        if (sql.Length > MaxLength) throw new InvalidOperationException($"SQL은 {MaxLength:N0}자까지 쓸 수 있습니다.");
        if (sql.Contains('\0')) throw new InvalidOperationException("SQL에 NUL 문자를 쓸 수 없습니다.");
        var depth = 0;
        var firstWord = (string?)null;
        for (var i = 0; i < sql.Length; i++)
        {
            var c = sql[i];
            if (char.IsWhiteSpace(c)) continue;
            if (c == '-' && i + 1 < sql.Length && sql[i + 1] == '-')
            {
                var end = sql.IndexOf('\n', i);
                i = end < 0 ? sql.Length : end;
                continue;
            }
            if (c == '/' && i + 1 < sql.Length && sql[i + 1] == '*')
            {
                var end = sql.IndexOf("*/", i + 2, StringComparison.Ordinal);
                if (end < 0) throw new InvalidOperationException("닫히지 않은 주석이 있습니다.");
                i = end + 1;
                continue;
            }
            if (c is '\'' or '"' or '`' or '[')
            {
                var close = c == '[' ? ']' : c;
                var j = i + 1;
                for (; ; j++)
                {
                    if (j >= sql.Length) throw new InvalidOperationException(c == '\'' ? "닫히지 않은 글자가 있습니다." : "닫히지 않은 이름이 있습니다.");
                    if (sql[j] != close) continue;
                    if (close != ']' && j + 1 < sql.Length && sql[j + 1] == close) { j++; continue; }
                    break;
                }
                if (c != '\'') names.Add(sql[(i + 1)..j].Replace(new string(close, 2), close.ToString()));
                firstWord ??= "";
                i = j;
                continue;
            }
            if (char.IsLetter(c) || c == '_')
            {
                var j = i;
                while (j < sql.Length && (char.IsLetterOrDigit(sql[j]) || sql[j] is '_' or '$')) j++;
                var word = sql[i..j];
                names.Add(word);
                firstWord ??= word;
                if (string.Equals(word, "WITH", StringComparison.OrdinalIgnoreCase))
                    throw new InvalidOperationException("WITH(공통 테이블 식)는 쓸 수 없습니다. 괄호 안의 부질의로 바꿔 쓰세요.");
                i = j - 1;
                continue;
            }
            firstWord ??= c.ToString();
            switch (c)
            {
                case '(': depth++; break;
                case ')':
                    if (--depth < 0) throw new InvalidOperationException("여는 괄호 없이 닫는 괄호가 있습니다.");
                    break;
                case ';': throw new InvalidOperationException("SQL 문장은 하나만 쓸 수 있습니다. ‘;’를 지우세요.");
            }
        }
        if (depth != 0) throw new InvalidOperationException("괄호가 닫히지 않았습니다.");
        if (!string.Equals(firstWord, "SELECT", StringComparison.OrdinalIgnoreCase))
            throw new InvalidOperationException("SELECT로 시작하는 문장 하나만 쓸 수 있습니다.");
        return names;
    }

    /// <summary>
    /// 글에 적힌 이름이 스키마의 표·뷰·색인·트리거이거나 <c>sqlite_</c>·<c>pragma_</c> 로 시작하면 계약면 뷰여야 한다.
    /// authorizer 만으로는 모자란다 — SQLite 가 뷰 위의 <c>count(*)</c> 를 밑 표 읽기로 바꿔 쓰면 그 읽기가 바깥 읽기로
    /// 보고된다(실측: <c>SELECT count(*) FROM v_공고차수</c> → <c>notice</c>). 이름을 적지 않고 표를 읽을 길은
    /// 뷰뿐이고, 뷰는 여기서 걸러진다.
    /// </summary>
    private static void CheckNames(SqliteConnection connection, IReadOnlySet<string> named)
    {
        var schema = connection.Query<string>("SELECT name FROM sqlite_master").ToHashSet(StringComparer.OrdinalIgnoreCase);
        var bad = named.FirstOrDefault(n => !Readable.Contains(n) && (schema.Contains(n)
            || n.StartsWith("sqlite_", StringComparison.OrdinalIgnoreCase) || n.StartsWith("pragma_", StringComparison.OrdinalIgnoreCase)));
        if (bad is not null) throw new InvalidOperationException($"공개 뷰(v_공고 등)만 읽을 수 있습니다: {bad}");
    }

    /// <summary>
    /// authorizer 를 건 채로 <paramref name="sql"/> 을 준비해 본다. 통과는 셋뿐이다 — <c>SQLITE_SELECT</c>,
    /// 계약면 뷰(또는 그 뷰 안쪽)의 <c>SQLITE_READ</c>, 허용 함수의 <c>SQLITE_FUNCTION</c>. 나머지(쓰기·ATTACH·PRAGMA·
    /// CREATE·재귀 CTE…)는 거절한다. 거절하면 까닭을 담아 던진다.
    /// </summary>
    internal static T Guarded<T>(SqliteConnection connection, IReadOnlySet<string> named, Func<T> run)
    {
        // 안쪽 읽기를 막을 뷰 — 계약면이 아닌 뷰. 뷰 안의 CTE 이름은 스키마에 없으므로 여기 들지 않는다.
        var closed = connection.Query<string>("SELECT name FROM sqlite_master WHERE type = 'view'")
            .Where(v => !Readable.Contains(v)).ToHashSet(StringComparer.Ordinal);
        string? refused = null;
        int Authorize(object? _, int action, string? p0, string? p1, string? database, string? inner)
        {
            var inView = !string.IsNullOrEmpty(inner);
            switch (action)
            {
                case Select:
                    return Ok;
                case Read:
                    // 바깥에 보고된 읽기라도 사용자가 그 이름을 적지 않았으면 뷰를 바꿔 쓴 것이다(CheckNames).
                    if (inView ? !closed.Contains(inner!) : Readable.Contains(p0 ?? "") || !named.Contains(p0 ?? "")) return Ok;
                    refused ??= $"공개 뷰(v_공고 등)만 읽을 수 있습니다: {(inView ? inner : p0)}";
                    return Deny;
                case Function:
                    // 뷰 안의 함수는 앱이 지은 것이다. 바깥(사용자 글)의 함수만 허용 목록으로 본다.
                    if (inView || Functions.Contains(p1 ?? "")) return Ok;
                    refused ??= $"이 함수는 쓸 수 없습니다: {p1}";
                    return Deny;
                default:
                    refused ??= "읽기 질의만 받습니다 — 쓰기·ATTACH·PRAGMA·재귀는 쓸 수 없습니다.";
                    return Deny;
            }
        }
        SQLitePCL.strdelegate_authorizer callback = Authorize!;
        SQLitePCL.raw.sqlite3_set_authorizer(connection.Handle, callback, null);
        try
        {
            return run();
        }
        catch (SqliteException error)
        {
            throw new InvalidOperationException(refused ?? $"SQL을 읽지 못했습니다: {error.Message}", error);
        }
        finally
        {
            SQLitePCL.raw.sqlite3_set_authorizer(connection.Handle, (SQLitePCL.strdelegate_authorizer)null!, null);
            GC.KeepAlive(callback);
        }
    }

    /// <summary>
    /// 이 자료 종류의 면 뷰 위에서 준비만 한다(<c>LIMIT 0</c>) — 구문 오류·없는 열·여러 열·순환이 여기서 잡힌다.
    /// 모양 검사를 먼저 한다.
    /// </summary>
    public static void Prepare(SqliteConnection connection, SqliteTransaction? transaction, string entity, string sql) =>
        Prepare(connection, transaction, sql, Frame.Face(entity));

    /// <summary>
    /// SQL 열이 서는 자리 — 고른 것·거르기·꼬리를 받아 문장 하나를 낸다. <see cref="Key"/> 는 표본 열쇠의 식이다.
    /// 가공열은 면 뷰 한 줄(<c>현재</c>)이고, 맞춤 뷰는 사슬 위의 세 문서(<c>접수</c>·<c>공고</c>·<c>계약</c>)다(docs/user-views.md §5).
    /// </summary>
    public sealed record Frame(string Key, Func<string, string?, string, string> Statement)
    {
        /// <summary>이 자료 종류의 면 뷰 한 줄 — 바깥 행은 <c>현재</c>.</summary>
        public static Frame Face(string entity)
        {
            var face = Store.FaceView(entity);
            return new($"{FlowCompiler.Self}.\"{Store.FaceKey(entity)}\"", (select, where, tail) =>
                $"SELECT {select} FROM \"{face}\" AS {FlowCompiler.Self}{(where is null ? "" : $" WHERE {where}")} {tail}");
        }
    }

    /// <summary>준비만 하는 것(<c>LIMIT 0</c>)을 그 자리(<paramref name="frame"/>) 위에서 한다 — 맞춤 뷰가 쓴다.</summary>
    public static void Prepare(SqliteConnection connection, SqliteTransaction? transaction, string sql, Frame frame)
    {
        var named = CheckShape(sql);
        CheckNames(connection, named);
        Guarded(connection, named, () =>
        {
            using var command = connection.CreateCommand();
            // 만들 때 연결의 트랜잭션을 저절로 문다 — null 로 덮으면 트랜잭션 안에서 거절된다.
            if (transaction is not null) command.Transaction = transaction;
            command.CommandText = frame.Statement(Expression(sql), null, "LIMIT 0") + ";";
            using var reader = command.ExecuteReader();
            return 0;
        });
    }

    /// <summary>
    /// 검증(§5)의 전부 — authorizer, 준비, 그리고 표본 실행. 표본에서 한 행이 값을 둘 이상 내면 거절한다(스칼라
    /// 부질의는 첫 줄만 쓰고 나머지를 조용히 버린다). 표본마다 찍힐 값을 낸다. 연결은 <see cref="OpenChecked"/> 로 연다.
    /// </summary>
    public static IReadOnlyList<string> Validate(SqliteConnection connection, string entity, string sql,
        IReadOnlyList<string> sampleKeys, int scan = 20) =>
        Validate(connection, sql, Frame.Face(entity), sampleKeys, scan);

    /// <summary>검증의 전부를 그 자리(<paramref name="frame"/>) 위에서 한다 — 맞춤 뷰가 쓴다.</summary>
    public static IReadOnlyList<string> Validate(SqliteConnection connection, string sql, Frame frame,
        IReadOnlyList<string> sampleKeys, int scan = 20)
    {
        Prepare(connection, null, sql, frame);
        var named = CheckShape(sql);
        var count = $"{frame.Key}, (SELECT count(*) FROM (\n{sql}\n))";
        var counted = Guarded(connection, named, () => connection.Query<(string Key, long Count)>(
                frame.Statement(count, $"{frame.Key} IN @sampleKeys", ""), new { sampleKeys })
            .Concat(connection.Query<(string Key, long Count)>(frame.Statement(count, null, $"LIMIT {scan}"))).ToList());
        var many = counted.FirstOrDefault(c => c.Count > 1);
        if (many.Key is not null)
            throw new InvalidOperationException(
                $"한 행에서 값이 {many.Count}개 나옵니다({many.Key}). group_concat으로 잇거나 LIMIT 1로 하나만 고르세요.");
        return [.. sampleKeys.Select(k => Guarded(connection, named, () => connection.QuerySingleOrDefault<string>(
            frame.Statement(Expression(sql), $"{frame.Key} = @k", "LIMIT 1"), new { k })) ?? "")];
    }
}

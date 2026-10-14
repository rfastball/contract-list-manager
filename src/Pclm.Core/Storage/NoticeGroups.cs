using Dapper;
using Microsoft.Data.Sqlite;
using Pclm.Core.Domain;

namespace Pclm.Core.Storage;

/// <summary>수집된 관련공고·접수 참조로 공고건을 재구성한다.</summary>
public static class NoticeGroups
{
    /// <summary>
    /// 이 본번호가 속한 <b>건</b>. 계열이 없으면 <c>null</c> — 문서가 들어온 적 없는 번호다.
    ///
    /// <para><b>사람이 아는 것은 공고번호이고 DB 가 아는 것은 건이다.</b> 그 사이의 번역은
    /// 여기서만 일어난다 — 링크를 쓰는 자리마다 따로 옮기면 한 군데가 조용히 늙고, 그때
    /// 어긋나는 것은 링크의 <b>끝점</b>이라 오류가 아니라 잘못 이어진 줄로 나온다.</para>
    /// </summary>
    public static string? GroupOf(
        SqliteConnection connection, string noticeBase, SqliteTransaction? transaction = null) =>
        connection.ExecuteScalar<string?>(
            "SELECT group_base FROM notice_series WHERE notice_base = @b;",
            new { b = noticeBase }, transaction);

    /// <summary>명시 대체 관계와 차수로 결정되는 유일한 현행 공고. 여럿이면 선택하지 않는다.</summary>
    public const string 현행공고 = """
        SELECT group_base, notice_base, seq FROM (
            SELECT 계열.group_base AS group_base, 현행.notice_base AS notice_base, 현행.seq AS seq,
                   COUNT(*) OVER (PARTITION BY 계열.group_base) AS 후보수
            FROM notice 현행
            JOIN notice_series 계열 ON 계열.notice_base = 현행.notice_base
            WHERE CAST(현행.seq AS INTEGER) =
                  (SELECT MAX(CAST(x.seq AS INTEGER)) FROM notice x WHERE x.notice_base = 현행.notice_base)
              AND NOT EXISTS (
                  SELECT 1 FROM notice_relation r
                  WHERE r.related_base = 현행.notice_base AND r.related_seq = 현행.seq
                    AND r.notice_base <> 현행.notice_base)
        ) 후보
        WHERE 후보수 = 1
        """;

    internal static void Rebuild(SqliteConnection connection, SqliteTransaction? transaction)
    {
        connection.Execute("DELETE FROM request_item_link; DELETE FROM request_link; DELETE FROM project_link;", transaction: transaction);
        해소(connection, transaction);
        var roots = 성분(connection, transaction);
        foreach (var root in roots.Values.Distinct(StringComparer.Ordinal))
            connection.Execute("INSERT OR IGNORE INTO notice_group(group_base) VALUES(@root)", new { root }, transaction);
        foreach (var (number, root) in roots)
            connection.Execute("UPDATE notice_series SET group_base=@root WHERE notice_base=@number", new { root, number }, transaction);
        connection.Execute("DELETE FROM notice_group WHERE group_base NOT IN (SELECT group_base FROM notice_series)", transaction: transaction);
    }

    /// <summary>
    /// 적힌 관련공고를 <b>본번호와 차수로 갈라 옆 칸에 둔다</b>. 가르는 것은
    /// <see cref="ValueParser.SplitNoticeRef"/> 하나다 — 뷰도 여기 갈라 둔 값만 견주므로
    /// SQL 에 문자열 산술이 없고, 규칙이 두 벌이 되지 않는다.
    /// </summary>
    private static void 해소(SqliteConnection connection, SqliteTransaction? transaction)
    {
        var 관계 = connection.Query<(string 본번호, string 차수, long 줄, string 적힌것, string? 옛본, string? 옛차)>(
            "SELECT notice_base, seq, line_no, related, related_base, related_seq FROM notice_relation;",
            transaction: transaction).ToList();

        foreach (var row in 관계)
        {
            var 갈린것 = ValueParser.SplitNoticeRef(row.적힌것);
            var (본, 차) = (갈린것?.Base, 갈린것?.Seq);

            // 이미 같은 값이면 건드리지 않는다. 관계 표는 공고마다 다시 쓰이는 자리라,
            // 바뀐 것 없이 도는 재계산이 WAL 을 부풀릴 까닭이 없다.
            if (string.Equals(본, row.옛본, StringComparison.Ordinal)
                && string.Equals(차, row.옛차, StringComparison.Ordinal)) continue;

            connection.Execute(
                """
                UPDATE notice_relation SET related_base = @본, related_seq = @차
                WHERE notice_base = @본번호 AND seq = @차수 AND line_no = @줄;
                """,
                new { 본, 차, row.본번호, row.차수, row.줄 }, transaction);
        }
    }

    /// <summary>
    /// 관련공고 간선을 풀어 <b>본번호 → 그 건의 이름</b>을 낸다.
    ///
    /// <para>이름은 그 성분이 아는 본번호 중 <b>Ordinal 최소값</b>이다. 합칠 때 언제나 작은 쪽에
    /// 매다는 것으로 그 성질이 곧바로 선다 — 나중에 다시 훑어 최소값을 고를 것이 없다.</para>
    ///
    /// <para>본번호는 <c>notice_series</c> 에 있는 것과 <c>related_base</c> 로 적힌 것을 함께
    /// 모은다. <b>문서 없는 조상</b>도 이름이 되어야 하기 때문이다 — 가스성분분석기가 가리키는
    /// <c>R26BK09013019</c> 처럼, 재공고 건에서는 가리키는 공고가 DB 에 없는 것이 보통이다.</para>
    /// </summary>
    private static Dictionary<string, string> 성분(
        SqliteConnection connection, SqliteTransaction? transaction)
    {
        var 어미 = new Dictionary<string, string>(StringComparer.Ordinal);

        void 세운다(string 본번호)
        {
            if (!어미.ContainsKey(본번호)) 어미[본번호] = 본번호;
        }

        string 찾는다(string 본번호)
        {
            var root = 본번호;
            while (!string.Equals(어미[root], root, StringComparison.Ordinal)) root = 어미[root];

            // 길을 접어 둔다. 다음에 같은 본번호를 물으면 한 칸이다.
            var 걸음 = 본번호;
            while (!string.Equals(걸음, root, StringComparison.Ordinal))
            {
                var 다음 = 어미[걸음];
                어미[걸음] = root;
                걸음 = 다음;
            }

            return root;
        }

        foreach (var 본번호 in connection.Query<string>(
                     "SELECT notice_base FROM notice_series;", transaction: transaction))
            세운다(본번호);

        foreach (var 본번호 in connection.Query<string>(
                     """
                     SELECT DISTINCT related_base FROM notice_relation WHERE related_base IS NOT NULL;
                     """, transaction: transaction))
            세운다(본번호);

        foreach (var (left, right) in connection.Query<(string Left, string Right)>(
                     """
                     SELECT notice_base, related_base FROM notice_relation WHERE related_base IS NOT NULL
                     UNION
                     SELECT a.notice_base, b.notice_base FROM notice_item a JOIN notice_item b
                       ON a.ref_request_base=b.ref_request_base AND a.notice_base<b.notice_base
                     WHERE coalesce(a.ref_request_base,'')<>''
                       AND CAST(a.seq AS INTEGER)=(SELECT max(CAST(n.seq AS INTEGER)) FROM notice n WHERE n.notice_base=a.notice_base)
                       AND CAST(b.seq AS INTEGER)=(SELECT max(CAST(n.seq AS INTEGER)) FROM notice n WHERE n.notice_base=b.notice_base)
                       AND NOT EXISTS(SELECT 1 FROM notice_item x
                         WHERE x.notice_base IN(a.notice_base,b.notice_base)
                           AND CAST(x.seq AS INTEGER)=(SELECT max(CAST(n.seq AS INTEGER)) FROM notice n WHERE n.notice_base=x.notice_base)
                           AND coalesce(x.ref_request_base,'')<>'' AND x.ref_request_base<>a.ref_request_base)
                     """, transaction: transaction))
        {
            세운다(left);
            세운다(right);
            var a = 찾는다(left);
            var b = 찾는다(right);
            if (string.CompareOrdinal(a, b) <= 0) 어미[b] = a;
            else 어미[a] = b;
        }

        return 어미.Keys.ToDictionary(본번호 => 본번호, 찾는다, StringComparer.Ordinal);
    }

}

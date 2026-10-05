using Dapper;
using Microsoft.Data.Sqlite;
using Pclm.Core.Domain;

namespace Pclm.Core.Storage;

/// <summary>
/// 건을 다시 지으며 <b>사람에게 알려야 할 것</b>.
///
/// <para>이 저장소는 밀어낸 것을 알리지 않은 적이 없다(<see cref="MergeConflict"/> 가 서 있는
/// 자리와 같다). 건이 합쳐지면 가리킬 데를 잃는 링크와, 한 건에 둘이 되어 밀려나는 접수가
/// 생기는데 둘 다 오류를 내지 않는다 — 알리지 않으면 적은 사람은 사라진 줄도 모른다.</para>
/// </summary>
/// <param name="이은건수">본번호가 둘 이상 묶인 건의 수. 재공고로 갈렸던 것이 몇 건 이어졌나.</param>
/// <param name="끊긴링크">가리키던 본번호가 어디에도 없어 걷어낸 링크. 사람이 읽는 한 줄씩.</param>
/// <param name="밀린접수">한 건에 접수가 둘이 되어 밀려난 쪽. 사람이 읽는 한 줄씩.</param>
public sealed record NoticeGroupChange(
    int 이은건수, IReadOnlyList<string> 끊긴링크, IReadOnlyList<string> 밀린접수)
{
    /// <summary>사람에게 보일 것이 있는가. 이은 것만으로는 알릴 일이 아니다 — 그것이 제 일이다.</summary>
    public bool 있나 => 끊긴링크.Count > 0 || 밀린접수.Count > 0;
}

/// <summary>
/// 관련공고를 타고 갈린 본번호들을 <b>공고건 하나</b>로 묶는다.
///
/// <para><b>전체 재계산 하나뿐이다.</b> 증분 갱신을 두지 않는 것이 이 표가 <b>파생</b>이라는
/// 사실을 코드가 잊지 않게 하는 길이다 — 관련공고는 뒤늦게 들어온 공고 한 장이 이미 선 건 둘을
/// 합칠 수 있어, 어디를 고쳤는지로는 무엇이 바뀌는지 셀 수 없다.</para>
///
/// <para><b>묶기는 무방향이다.</b> 화살표를 믿을 수 없기 때문이다 — 절단기 재출력본
/// <c>R26BK09017030-000</c> 의 관련공고는 앞이 아니라 <b>뒤</b>(<c>-001</c>)를 가리킨다.
/// 무엇이 무엇을 대신했는지는 같은 본번호 안에서는 차수로, 본번호 사이에서만 관련공고로
/// 정한다(그쪽은 뷰의 「현행공고」가 한다).</para>
///
/// <para><b>캐스케이드에 기대지 않는다.</b> <c>Store</c> 는 외래키를 켜고(<c>Database.Open</c>)
/// <c>Merger.Write</c> 는 꺼 두어서, 캐스케이드에 기대면 같은 코드가 한쪽에서만 돈다.
/// 모든 삭제와 재지정을 명시 SQL 로 한다.</para>
///
/// <para><b>트랜잭션을 만들지 않는다.</b> 부르는 쪽이 쥔 것 안에서 돌아야, 건이 반쯤 지어진
/// 채로 커밋되는 일이 없다.</para>
/// </summary>
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

    /// <summary>
    /// <see cref="GroupOf"/> 와 같되 없으면 <b>사람이 읽는 말로</b> 던진다. 화면과 명령줄이
    /// 건넨 공고번호를 받는 자리(잇기·물리치기·견주기)에 쓴다.
    ///
    /// <para>여기서 조용히 넘어가면 대신 던지는 것은 외래키인데, 그 말은 사람이 읽을 것이 못 된다.</para>
    /// </summary>
    public static string RequireGroupOf(
        SqliteConnection connection, EntityRef notice, SqliteTransaction? transaction = null) =>
        GroupOf(connection, notice.Base, transaction)
            ?? throw new InvalidOperationException($"그런 공고가 없습니다: {notice.Display}");

    /// <summary>
    /// 건마다 <b>현행 공고 한 장</b>을 내는 SELECT. 열은 <c>group_base · notice_base · seq</c> 다.
    ///
    /// <para><b>대체되었다</b> 는 둘 중 하나다. ① 같은 본번호에 더 높은 차수가 있다,
    /// ② <b>다른 본번호</b>의 공고가 <c>(related_base, related_seq)</c> 로 이 차수를 가리킨다.
    /// 둘 다 아닌 것 가운데 게시가 늦은 것, 같으면 본번호가 큰 것을 낸다.</para>
    ///
    /// <para>②에서 같은 본번호를 일부러 뺀다. 빼지 않으면 절단기 재출력본
    /// <c>R26BK09017030-000</c> 이 자기 뒤차수를 가리키고 있어 <b>최신 차수가 대체된 것</b>이
    /// 된다 — 화살표 방향을 믿을 수 없기 때문이다.</para>
    ///
    /// <para><b>이것이 정본이다.</b> 뷰의 <see cref="Views"/>.건현행 은 이 SELECT 를 파생표로
    /// 감싸 쓸 뿐이라, 규칙은 이 자리 한 벌뿐이다 — 뷰는 건마다 공고 한 줄을 세우는 자리에서,
    /// 링커는 후보를 건 단위로 세는 자리에서 같은 것을 묻는데, 두 벌로 적어 두면 한쪽만 늙어
    /// <b>화면이 보여 준 공고와 기계가 견준 공고가 달라진다</b>. 고칠 곳은 여기 하나다.</para>
    ///
    /// <para>그래서 <c>public</c> 이다. 뷰가 부르고, 둘이 정말 같은 것을 내는지
    /// <c>NoticeGroupTests</c> 가 여기서 직접 읽어 <c>v_공고.현행공고</c> 와 견준다 —
    /// 합쳐 두어도 나중에 한쪽만 고치는 일은 시험이 잡아야 한다.</para>
    ///
    /// <para>건에 남는 것이 하나도 없을 수 있다 — 두 본번호가 서로의 최신 차수를 가리키는
    /// 꼴이다. 그때는 뷰의 <c>현행공고</c> 도 비므로 여기서도 억지로 하나를 세우지 않는다.
    /// 없는 것을 있는 척하는 순간 규칙이 두 벌이 된다.</para>
    /// </summary>
    public const string 현행공고 = """
        SELECT group_base, notice_base, seq FROM (
            SELECT 계열.group_base AS group_base, 현행.notice_base AS notice_base, 현행.seq AS seq,
                   COUNT(*) OVER (PARTITION BY 계열.group_base) AS 후보수,
                   ROW_NUMBER() OVER (
                       PARTITION BY 계열.group_base
                       ORDER BY COALESCE(현행.posted_at, '') DESC, 현행.notice_base DESC) AS 순위
            FROM notice 현행
            JOIN notice_series 계열 ON 계열.notice_base = 현행.notice_base
            WHERE CAST(현행.seq AS INTEGER) =
                  (SELECT MAX(CAST(x.seq AS INTEGER)) FROM notice x WHERE x.notice_base = 현행.notice_base)
              AND NOT EXISTS (
                  SELECT 1 FROM notice_relation r
                  WHERE r.related_base = 현행.notice_base AND r.related_seq = 현행.seq
                    AND r.notice_base <> 현행.notice_base)
        ) 후보
        WHERE 순위 = 1 AND (후보수=1 OR NOT EXISTS (
            SELECT 1 FROM notice_item i JOIN notice_series ns ON ns.notice_base=i.notice_base
            WHERE ns.group_base=후보.group_base AND coalesce(i.ref_request_base,'')<>''))
        """;

    /// <summary>
    /// 건을 처음부터 다시 짓는다. 차례는 <b>넣기 → 재지정 → 지우기</b>다 — 옛 건을 먼저 지우면
    /// 거기 매달린 링크가 (외래키가 켜져 있을 때) 캐스케이드로 함께 간다.
    /// </summary>
    public static NoticeGroupChange Rebuild(SqliteConnection connection, SqliteTransaction? transaction)
    {
        해소(connection, transaction);

        var 뿌리 = 성분(connection, transaction);
        var 이은건수 = 뿌리.Values
            .GroupBy(root => root, StringComparer.Ordinal)
            .Count(g => g.Count() > 1);

        // ① 넣기. 재지정이 가리킬 건이 먼저 서 있어야 한다.
        foreach (var root in 뿌리.Values.Distinct(StringComparer.Ordinal).Order(StringComparer.Ordinal))
            connection.Execute(
                "INSERT OR IGNORE INTO notice_group (group_base) VALUES (@g);",
                new { g = root }, transaction);

        // ② 재지정. 계열 먼저 — 옛 건을 지울 수 있는지가 여기서 정해진다.
        foreach (var (본번호, 지금건) in connection.Query<(string 본번호, string 건)>(
                     "SELECT notice_base, group_base FROM notice_series;", transaction: transaction))
        {
            var root = 뿌리[본번호];
            if (string.Equals(root, 지금건, StringComparison.Ordinal)) continue;

            connection.Execute(
                "UPDATE notice_series SET group_base = @g WHERE notice_base = @b;",
                new { g = root, b = 본번호 }, transaction);
        }

        var 끊긴링크 = new List<string>();
        var 밀린접수 = new List<string>();

        // 접수 링크는 UNIQUE 라 먼저, 그리고 따로 옮긴다 — 옮기다 부딪히면 취합처럼 터진다.
        접수링크(connection, transaction, 뿌리, 끊긴링크, 밀린접수);

        옮긴다(connection, transaction, "project_link", "contract_base", "계약 링크", 뿌리, 끊긴링크);
        옮긴다(connection, transaction, "link_rejection", "contract_base", "계약 물리침", 뿌리, 끊긴링크);
        옮긴다(connection, transaction, "request_link_rejection", "request_base", "접수 물리침", 뿌리, 끊긴링크);

        // ③ 지우기. 아무 계열도 서 있지 않은 건은 이름만 남은 것이다.
        //
        // 링크는 이 시점에 모두 뿌리(계열이 반드시 하나는 서 있는 이름)를 가리키거나 걷혔으므로,
        // 계열만 보고 지워도 링크가 캐스케이드로 딸려 갈 일이 없다.
        connection.Execute(
            """
            DELETE FROM notice_group
            WHERE group_base NOT IN (SELECT group_base FROM notice_series);
            """, transaction: transaction);

        return new NoticeGroupChange(이은건수, 끊긴링크, 밀린접수);
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

        foreach (var (이쪽, 저쪽, erp) in connection.Query<(string 이쪽, string 저쪽, int Erp)>(
                     """
                     SELECT notice_base, related_base, 0 FROM notice_relation WHERE related_base IS NOT NULL
                     UNION
                     SELECT a.notice_base, b.notice_base, 1 FROM notice_item a JOIN notice_item b
                       ON a.ref_request_base=b.ref_request_base AND a.notice_base<b.notice_base
                     WHERE coalesce(a.ref_request_base,'')<>''
                       AND NOT EXISTS(SELECT 1 FROM notice_item x WHERE x.notice_base IN(a.notice_base,b.notice_base)
                         AND coalesce(x.ref_request_base,'')<>'' AND x.ref_request_base<>a.ref_request_base)
                       AND NOT EXISTS(SELECT 1 FROM request_link l JOIN request_link m ON l.request_base<>m.request_base
                         WHERE l.notice_group=(SELECT group_base FROM notice_series WHERE notice_base=a.notice_base)
                           AND m.notice_group=(SELECT group_base FROM notice_series WHERE notice_base=b.notice_base));
                     """, transaction: transaction))
        {
            세운다(이쪽);
            세운다(저쪽);

            var a = 찾는다(이쪽);
            var b = 찾는다(저쪽);
            if (string.Equals(a, b, StringComparison.Ordinal)) continue;

            if (erp == 1)
            {
                // ponytail: 재구성 시 건 단위 충돌 검사. 공고가 매우 많아지면 컴포넌트별 소유자 집합을 캐시한다.
                var members = 어미.Keys.Where(k => 찾는다(k) == a || 찾는다(k) == b).ToArray();
                var groups = connection.Query<string>("SELECT DISTINCT group_base FROM notice_series WHERE notice_base IN @members", new { members }, transaction).ToArray();
                if (connection.ExecuteScalar<int>("SELECT count(DISTINCT request_base) FROM request_link WHERE notice_group IN @groups", new { groups }, transaction) > 1 ||
                    connection.ExecuteScalar<int>("SELECT count(*) FROM project_link l JOIN link_rejection r USING(contract_base) WHERE l.notice_group IN @groups AND r.notice_group IN @groups", new { groups }, transaction) > 0 ||
                    connection.ExecuteScalar<int>("SELECT count(*) FROM request_link l JOIN request_link_rejection r USING(request_base) WHERE l.notice_group IN @groups AND r.notice_group IN @groups", new { groups }, transaction) > 0) continue;
            }

            // 언제나 큰 쪽을 작은 쪽에 매단다 — 그래야 뿌리가 곧 최소 본번호다.
            if (string.CompareOrdinal(a, b) <= 0) 어미[b] = a; else 어미[a] = b;
        }

        return 어미.Keys.ToDictionary(본번호 => 본번호, 찾는다, StringComparer.Ordinal);
    }

    /// <summary>
    /// 링크 한 표를 새 건으로 옮긴다. 가리키던 본번호가 어디에도 없으면 <b>걷어내고 알린다</b>.
    ///
    /// <para><c>UPDATE OR REPLACE</c> 인 것은 물리침 표 때문이다. 한 계약이 원공고와 재공고를
    /// 따로 물리쳐 두었으면 둘이 한 줄이 되는데, 그 둘은 <b>같은 말</b>이라 하나면 족하다.
    /// <c>project_link</c> 는 키가 계약 하나라 여기서 부딪힐 자리가 없다.</para>
    /// </summary>
    private static void 옮긴다(
        SqliteConnection connection, SqliteTransaction? transaction,
        string 표, string 키열, string 종류,
        IReadOnlyDictionary<string, string> 뿌리, List<string> 끊긴링크)
    {
        // 표 이름과 열 이름은 이 자리에서 적는 상수라 주입될 것이 없다.
        var 줄들 = connection.Query<(string 키, string 건)>(
            $"SELECT {키열}, notice_group FROM {표};", transaction: transaction).ToList();

        foreach (var (키, 건) in 줄들)
        {
            if (!뿌리.TryGetValue(건, out var root))
            {
                connection.Execute(
                    $"DELETE FROM {표} WHERE {키열} = @k AND notice_group = @g;",
                    new { k = 키, g = 건 }, transaction);

                끊긴링크.Add($"{종류} {키} — 가리키던 공고건 {건} 이 없어졌습니다");
                continue;
            }

            if (string.Equals(root, 건, StringComparison.Ordinal)) continue;

            connection.Execute(
                $"UPDATE OR REPLACE {표} SET notice_group = @root WHERE {키열} = @k AND notice_group = @g;",
                new { root, k = 키, g = 건 }, transaction);
        }
    }

    /// <summary>
    /// 접수 링크를 옮긴다. <c>notice_group</c> 이 UNIQUE 라 <b>옮기기 전에</b> 한 건에 둘이
    /// 되는 자리를 가려야 한다 — 그대로 밀어 넣으면 UNIQUE 위반으로 통째로 터진다.
    ///
    /// <para>남는 것은 <b>사람이 확정한 것</b>, 그 다음이 <b>늦게 이은 것</b>이다. 기계가 이은
    /// 것이 사람의 판단을 밀어내는 일은 없다(ADR-007 의 규율). 기계가 적은 값은 ERP 명시
    /// 참조의 <c>'explicit'</c> 과 옛 판이 남긴 <c>'auto'</c>(ADR-029) 라, 사람인지는
    /// <c>'human'</c> 인지로 가른다.</para>
    ///
    /// <para>밀린 쪽은 <b>반드시 알린다</b>. 인수인계 직후에 원공고와 재공고에 접수를 각각
    /// 이어 둔 DB 가 판올림에서 곧바로 이 자리에 온다.</para>
    /// </summary>
    private static void 접수링크(
        SqliteConnection connection, SqliteTransaction? transaction,
        IReadOnlyDictionary<string, string> 뿌리, List<string> 끊긴링크, List<string> 밀린접수)
    {
        var 줄들 = connection.Query<(string 접수, string 건, string? 결정, string? 확정)>(
            "SELECT request_base, notice_group, decided_by, confirmed_at FROM request_link;",
            transaction: transaction).ToList();

        var 살아남을것 = new Dictionary<string, List<(string 접수, string 건, string? 결정, string? 확정)>>(
            StringComparer.Ordinal);

        // 링크를 걷으면 품목 짝도 함께 간다. <c>request_item_link</c> 는 <c>request_link</c> 가
        // 아니라 <c>request</c> 를 가리켜 캐스케이드가 데려가지 않으므로, 여기서 명시로 걷지
        // 않으면 <b>어느 조달요구가 어느 공고 품목이 되었는지가 링크 없이 떠 있게 된다</b> —
        // 근거가 아니라 결과인 표라 홀로 남을 뜻이 없다(RequestLinker.Unlink 와 같은 자세).
        void 걷는다(string 접수)
        {
            connection.Execute(
                "DELETE FROM request_item_link WHERE request_base = @r;", new { r = 접수 }, transaction);

            connection.Execute(
                "DELETE FROM request_link WHERE request_base = @r;", new { r = 접수 }, transaction);
        }

        foreach (var 줄 in 줄들)
        {
            if (!뿌리.TryGetValue(줄.건, out var root))
            {
                걷는다(줄.접수);

                끊긴링크.Add($"접수 링크 {줄.접수} — 가리키던 공고건 {줄.건} 이 없어졌습니다");
                continue;
            }

            if (!살아남을것.TryGetValue(root, out var 무리)) 살아남을것[root] = 무리 = [];
            무리.Add(줄);
        }

        foreach (var (root, 무리) in 살아남을것.OrderBy(p => p.Key, StringComparer.Ordinal))
        {
            var 이긴것 = 무리
                .OrderByDescending(x => string.Equals(x.결정, "human", StringComparison.Ordinal))
                .ThenByDescending(x => x.확정 ?? "", StringComparer.Ordinal)
                .ThenBy(x => x.접수, StringComparer.Ordinal)
                .First();

            foreach (var 진것 in 무리.Where(x => !string.Equals(x.접수, 이긴것.접수, StringComparison.Ordinal)))
            {
                걷는다(진것.접수);

                밀린접수.Add(
                    $"접수 {진것.접수} 가 공고건 {root} 에서 밀렸습니다 — 남은 것은 {이긴것.접수}" +
                    $" ({(string.Equals(이긴것.결정, "human", StringComparison.Ordinal) ? "사람 확정" : "기계가 이음")})");
            }

            if (string.Equals(이긴것.건, root, StringComparison.Ordinal)) continue;

            connection.Execute(
                "UPDATE request_link SET notice_group = @root WHERE request_base = @r;",
                new { root, r = 이긴것.접수 }, transaction);
        }
    }
}

using Dapper;
using Microsoft.Data.Sqlite;
using Pclm.Core.Domain;

namespace Pclm.Core.Storage;

/// <summary>
/// 접수와 공고를 나란히 놓고 견준 항목 하나.
/// </summary>
/// <param name="Agrees">맞는가. <c>null</c> 이면 한쪽 값이 없어 <b>판정할 수 없다</b>는 뜻이다.</param>
public sealed record RequestFacet(string Name, string Request, string Notice, bool? Agrees);

/// <summary>접수 한 건에 붙일 공고 후보.</summary>
/// <param name="TitleMatched">요청명과 공고명이 정확히 일치하는가. <b>근거가 아니라 정황</b>이다.</param>
/// <param name="Blocker">잇기 전에 걸리는 점. 없으면 판정을 통과한 유일한 후보다.</param>
public sealed record RequestLinkCandidate(
    EntityRef Request,
    string RequestTitle,
    EntityRef Notice,
    string NoticeTitle,
    double Confidence,
    string Reason,
    long NoticeLinkedCount,
    bool TitleMatched = false,
    string? Blocker = null,
    IReadOnlyList<RequestFacet>? Facets = null)
{
    public IReadOnlyList<RequestFacet> Compared => Facets ?? [];
}

/// <summary>
/// 잇기 화면에 세울 접수 한 줄. <b>이어진 것도 낸다</b> — 잘못 이어진 것을 끊으려면
/// 그 줄이 화면에 서 있어야 한다. 후보가 하나도 없는 것도 낸다.
/// </summary>
public sealed record RequestLinkRow(
    EntityRef Request, string Title, int CandidateCount,
    EntityRef? Notice, string NoticeTitle, string? DecidedBy);

/// <summary>접수 잇기 화면이 한 번에 받아 가는 것.</summary>
public sealed record RequestLinkWork(
    IReadOnlyList<RequestLinkRow> Requests,
    IReadOnlyList<RequestLinkCandidate> Candidates);

/// <summary>
/// 접수와 공고를 잇는다.
///
/// <para><b>접수서·공고서 어디에도 조달요구번호와 공고번호를 잇는 명시적 키가 없다.</b> 그래서 품목으로
/// 잇는다 — 줄 수가 두 곳에서 같고, 각 줄의 <b>수량과 단가</b>가 다중집합으로 완전히 같으면
/// 그 짝이다(ADR-021).</para>
///
/// <para><b>줄 차례가 다르다.</b> 접수는 요청번호순, 공고는 세부품명·수량순이다. 자리끼리
/// 맞추면 틀리므로 다중집합으로 견준다. 통과하면 (수량,단가) 짝이 서로 달라 여덟 쌍이
/// 일대일로 정해진다.</para>
///
/// <para><b>블로킹 키는 건명이 아니라 품목 줄 수다.</b> <see cref="Linker"/> 는 나라장터가
/// 공고명을 계약명으로 그대로 옮긴다는 사실에 기대지만, 요청명과 공고명이 늘 같다는 보장은
/// 없다 — 요청명은 수요기관이 적고 공고명은 조달청이 적는다. 이름은 <b>부수 확인</b>으로만 쓴다.</para>
///
/// <para><b>유일성은 줄 수가 아니라 통과한 후보 사이에서 센다.</b> 줄 수가 같은 공고가 여럿인
/// 것은 흔한 일이고 — 품목 두 줄짜리 공고는 얼마든지 있다 — 그것만으로 사람에게 넘기면 큐가
/// 헛되이 는다. 블로킹 키는 견줄 것을 추리는 자리이지 결정하는 자리가 아니다.</para>
///
/// <para><b>세부품명번호는 근거로 쓰지 않는다.</b> 협의·요청 누락으로 접수와 공고가 다를 수
/// 있다. 나란히 보이기만 하고, 어긋나도 막지 않는다.</para>
///
/// <para><b>기계는 잇지 않는다</b>(ADR-029). 판정은 후보를 추천하는 데까지이고, 링크는 ERP 명시
/// 참조(<c>ExplicitLinks</c>)와 사람의 확정으로만 선다. 옛 판이 남긴 <c>decided_by = 'auto'</c>
/// 줄은 그대로 둔다.</para>
/// </summary>
public sealed class RequestLinker(Database database)
{
    private readonly Database _database = database;

    /// <summary>사람이 볼 후보. 이미 이어진 접수는 여기 오지 않는다.</summary>
    public IReadOnlyList<RequestLinkCandidate> Candidates()
    {
        var work = Work();
        var linked = work.Requests.Where(r => r.Notice is not null).Select(r => r.Request.Base).ToHashSet();

        return [.. work.Candidates.Where(c => !linked.Contains(c.Request.Base))];
    }

    /// <summary>
    /// 잇기 화면이 한 번에 받아 갈 것. 접수를 <b>이어진 것까지</b> 모두 내고, 그 접수마다 후보를 붙인다.
    /// </summary>
    public RequestLinkWork Work()
    {
        using var connection = _database.Open();
        var state = Read(connection);

        var results = new List<RequestLinkCandidate>();
        var rows = new List<RequestLinkRow>();

        foreach (var (request, matches) in CountMatches(state))
        {
            var before = results.Count;
            var self = new EntityRef("request", request.Base, request.Seq);

            state.Links.TryGetValue(request.Base, out var link);

            // 이미 이어진 건은 후보에서 뺀다 — 화면이 「지금 이어진 공고」 카드로 따로 그린다.
            var pool = link.NoticeGroup is null
                ? matches
                : matches.Where(n => n.Group != link.NoticeGroup).ToList();

            // 「여럿입니다」는 통과한 후보가 둘 이상일 때에만 붙인다. 줄 수가 같은 공고가 여럿인
            // 것은 흔한 일이라, 그것만으로 붙이면 유일하게 맞는 짝까지 애매해 보인다.
            var passing = pool.Count(n => Passes(state, request, n));

            var ambiguous = passing > 1
                ? $"수량·단가가 모두 맞는 공고가 {passing}건입니다"
                : null;

            foreach (var notice in pool)
            {
                var verdict = Verify(state, request, notice);
                var taken = Taken(state, request, notice);

                results.Add(new RequestLinkCandidate(
                    self, request.Title ?? string.Empty,
                    new EntityRef("notice", notice.Base, notice.Seq), notice.Title ?? string.Empty,
                    verdict.Passed ? 1.0 : 0.5,
                    verdict.Passed ? verdict.Evidence : $"품목 {request.Items.Count}줄",
                    notice.Linked,
                    TitleMatched: Linker.TitleKey(request.Title) == Linker.TitleKey(notice.Title),
                    // 여럿이라는 말은 통과한 후보에만 붙인다 — 통과하지 못한 후보에는
                    // 제 까닭이 따로 있고, 그것을 덮으면 무엇이 어긋났는지 화면에서 사라진다.
                    Blocker: verdict.Passed && taken is null
                        ? ambiguous
                        : verdict.Blocker ?? taken,
                    Facets: Compare(state, request, notice)));
            }

            // 보이는 것은 <b>현행 공고 번호</b>다. 건 이름은 문서가 없는 번호일 수 있어
            // (가스성분분석기가 가리키는 R26BK09013019) 사람 앞에 낼 것이 못 된다. 이름으로 물러서는 것은
            // 건에 현행이 하나도 없을 때뿐이고, 그 자리는 v_공고.현행공고 도 빈다.
            var 이어진것 = link.NoticeGroup is null ? null : 현행(state, link.NoticeGroup);

            rows.Add(new RequestLinkRow(
                self,
                request.Title ?? string.Empty,
                results.Count - before,
                link.NoticeGroup is null
                    ? null
                    : new EntityRef("notice", 이어진것?.Base ?? link.NoticeGroup, 이어진것?.Seq ?? string.Empty),
                이어진것?.Title ?? string.Empty,
                link.NoticeGroup is null ? null : link.DecidedBy));
        }

        return new RequestLinkWork(
            rows,
            [.. results.OrderBy(r => r.Request.Base, StringComparer.Ordinal)
                       .ThenByDescending(r => r.Confidence)]);
    }

    /// <summary>
    /// 공고를 모두 낸다. <b>추천이 찾지 못한 짝을 사람이 손수 고르는 자리</b>에 쓴다.
    ///
    /// <para>줄은 <b>건마다 하나</b>이고 번호는 그 건의 <b>현행 공고</b>다 — 취소된 옛 본번호가
    /// 따로 서지 않는다. 건 이름을 내지 않는 까닭은 그것이 문서 없는 번호일 수 있어서다.</para>
    /// </summary>
    public IReadOnlyList<NoticeChoice> Notices()
    {
        using var connection = _database.Open();
        var state = Read(connection);

        return
        [
            .. state.Notices.Select(n => new NoticeChoice(
                new EntityRef("notice", n.Base, n.Seq),
                n.Title ?? string.Empty,
                n.PostedAt?.ToString("yyyy-MM-dd") ?? string.Empty,
                n.Linked)),
        ];
    }

    /// <summary>후보에 없는 임의의 짝도 나란히 견준다. <b>잇기 전에 눈으로 볼 재료</b>다.</summary>
    public IReadOnlyList<RequestFacet> Compare(EntityRef request, EntityRef notice)
    {
        using var connection = _database.Open();
        var state = Read(connection);

        var left = state.Requests.FirstOrDefault(r => r.Base == request.Base)
            ?? throw new InvalidOperationException($"그런 접수가 없습니다: {request.Display}");

        // 사람이 건넨 것은 공고번호라 건으로 옮겨 세운다 — 대체된 차수나 취소된 본번호를
        // 골라도 그 건의 현행 공고와 견준다. 견주는 단위가 건이기 때문이다.
        var 건 = NoticeGroups.RequireGroupOf(connection, notice);

        var right = state.Notices.FirstOrDefault(n => n.Group == 건)
            ?? throw new InvalidOperationException($"그런 공고가 없습니다: {notice.Display}");

        return Compare(state, left, right);
    }

    /// <summary>
    /// 사람이 고른 짝을 확정한다. 품목 짝(<c>request_item_link</c>)도 함께 쓴다.
    ///
    /// <para><b>공고건 하나에 접수 하나다.</b> <c>request_link.notice_group</c> 이 UNIQUE 라
    /// 이미 다른 접수가 붙은 건에 또 이으면 DB 가 막는다 — 그 실패를 사람이 읽을 수 있는
    /// 말로 바꿔 여기서 먼저 낸다. 조용히 두 줄이 되는 것보다 시끄럽게 실패하는 편이 낫다.</para>
    ///
    /// <para><b>받는 것은 공고번호이고 매다는 것은 건이다.</b> 사람이 고르는 것은 눈에 보이는
    /// 번호라 재공고 건의 어느 본번호든 올 수 있는데, 링크의 끝점은 그 건의 이름 하나뿐이다 —
    /// 여기서 옮겨 두지 않으면 뿌리가 아닌 본번호로 확정할 때 외래키가 던진다.</para>
    /// </summary>
    public void Confirm(EntityRef request, EntityRef notice, double confidence)
    {
        using var connection = _database.Open();
        var state = Read(connection);
        var 건 = NoticeGroups.RequireGroupOf(connection, notice);

        if (Taken(state, request.Base, 건) is { } why)
            throw new InvalidOperationException(why);

        using var transaction = connection.BeginTransaction();

        // 사람이 손수 이었다면 그 짝에 대한 옛 거부는 뜻을 잃는다. 거부도 건에 매여 있으므로
        // 재공고를 물리쳐 두었다가 원공고로 확정해도 함께 풀린다 — 둘은 같은 말이다.
        connection.Execute(
            "DELETE FROM request_link_rejection WHERE request_base = @r AND notice_group = @n;",
            new { r = request.Base, n = 건 }, transaction);

        Write(connection, transaction, request.Base, 건, confidence);
        WriteItemLinks(connection, transaction, state, request.Base, 건);

        transaction.Commit();
    }

    /// <summary>
    /// 이 짝은 아니라고 판정한다. 물리친 후보가 다음에도 목록 맨 위에 다시 뜨지 않게 남긴다.
    ///
    /// <para><b>물리치는 것도 건이다.</b> 재공고를 아니라고 하면 원공고도 함께 내려간다 —
    /// 둘은 한 조달 건이라 한쪽만 아니라고 할 수 있는 것이 아니다.</para>
    /// </summary>
    public void Reject(EntityRef request, EntityRef notice)
    {
        using var connection = _database.Open();
        var 건 = NoticeGroups.RequireGroupOf(connection, notice);

        using var transaction = connection.BeginTransaction();

        // 그 건으로 이어져 있었다면 함께 푼다 — 아니라고 판정한 짝이 링크로 남을 수는 없다.
        var 풀렸나 = connection.Execute(
            "DELETE FROM request_link WHERE request_base = @r AND notice_group = @n;",
            new { r = request.Base, n = 건 }, transaction) > 0;

        // 품목 짝은 <b>링크가 실제로 풀렸을 때만</b> 걷는다. 그 표는 공고 한 장을 가리키지 건을
        // 가리키지 않아 건으로는 고를 수 없고, 접수번호만 보고 걷으면 <b>다른 건에 이어져 있던
        // 짝까지</b> 함께 사라진다. 링크가 그대로면 짝도 그대로여야 한다.
        if (풀렸나)
            connection.Execute(
                "DELETE FROM request_item_link WHERE request_base = @r;",
                new { r = request.Base }, transaction);

        connection.Execute(
            """
            INSERT INTO request_link_rejection (request_base, notice_group, rejected_at)
            VALUES (@r, @n, @now)
            ON CONFLICT(request_base, notice_group) DO NOTHING;
            """,
            new { r = request.Base, n = 건, now = DateTime.UtcNow.ToString("O") }, transaction);

        transaction.Commit();
    }

    /// <summary>링크를 푼다. 품목 짝도 함께 걷는다 — 그것은 링크의 결과라 홀로 남을 뜻이 없다.</summary>
    public void Unlink(EntityRef request)
    {
        using var connection = _database.Open();
        using var transaction = connection.BeginTransaction();

        connection.Execute(
            "DELETE FROM request_item_link WHERE request_base = @r;", new { r = request.Base }, transaction);

        connection.Execute(
            "DELETE FROM request_link WHERE request_base = @r;", new { r = request.Base }, transaction);

        transaction.Commit();
    }

    // ── 규칙 ─────────────────────────────────────────────

    private sealed record Verdict(bool Passed, string Evidence, string? Blocker);

    /// <summary>
    /// 필수 확인 하나 — <b>(수량, 단가) 다중집합이 완전히 같은가</b>.
    ///
    /// <para>하나라도 어긋나면 통과하지 못한다. 이것은 채점이 아니라 <b>두 문서를 잇는 키를
    /// 세우는 일</b>이라 느슨하게 둘 여지가 없다(ADR-016 은 값의 옳고 그름을 따지지 말라는
    /// 것이지, 잇는 근거를 흐리라는 것이 아니다).</para>
    ///
    /// <para>부수 확인은 <b>근거 문장에 보태기만</b> 한다 — 요청명·금액·날짜는 값이 양쪽에
    /// 있을 때만 보고, 없는 것은 위반이 아니라 판정 불가다.</para>
    /// </summary>
    private static Verdict Verify(Snapshot state, RequestRow request, NoticeRow notice)
    {
        if (request.Items.Count == 0 || notice.Items.Count == 0)
            return new Verdict(false, string.Empty, "품목을 읽지 못했습니다");

        if (request.Items.Count != notice.Items.Count)
            return new Verdict(false, string.Empty,
                $"품목 줄 수가 다릅니다 (접수 {request.Items.Count} · 공고 {notice.Items.Count})");

        if (request.Items.Any(i => i.Incomplete) || notice.Items.Any(i => i.Incomplete))
            return new Verdict(false, string.Empty, "수량·단가를 읽지 못했습니다");

        if (!Pairs(request.Items).SequenceEqual(Pairs(notice.Items)))
            return new Verdict(false, string.Empty, "수량·단가가 다릅니다");

        var reasons = new List<string> { $"품목 {request.Items.Count}줄 · 수량·단가 전부 일치" };

        if (Linker.TitleKey(request.Title).Length > 0
            && Linker.TitleKey(request.Title) == Linker.TitleKey(notice.Title))
            reasons.Add("요청명 일치");

        if (request.GoodsAmount is { } goods && notice.ProjectAmount is { } project && goods == project)
            reasons.Add("품대 = 사업금액");

        if (request.BudgetAmount is { } budget && notice.AllocatedBudget is { } allocated && budget == allocated)
            reasons.Add("예산금액 = 배정예산");

        // 접수가 공고보다 앞선다. 어긋나도 막지는 않는다 — 날짜 하나로 물리치기에는
        // 접수일이 비거나 늦게 정정되는 일이 실재한다.
        if (request.ReceivedOn is { } received && notice.PostedAt is { } posted && received < posted)
            reasons.Add($"접수일 {received:yyyy-MM-dd} < 게시일 {posted:yyyy-MM-dd}");

        return new Verdict(true, string.Join(" · ", reasons), null);
    }

    /// <summary>다중집합 비교용. 정렬해 늘어놓으면 줄 차례가 달라도 같은 것이 같아진다.</summary>
    private static IEnumerable<(int Quantity, decimal UnitPrice)> Pairs(IReadOnlyList<ItemRow> items) =>
        items.Select(i => (i.Quantity!.Value, i.UnitPrice!.Value))
             .OrderBy(p => p.Item1).ThenBy(p => p.Item2);

    /// <summary>
    /// 그 <b>건</b>에 이미 다른 접수가 붙어 있는가. 붙어 있으면 까닭을, 아니면 <c>null</c>.
    /// <c>notice_group</c> UNIQUE 가 DB 에서 막는 것을 <b>그 앞에서 말로 바꾼다</b>.
    ///
    /// <para>말은 「공고」로 한다 — 사람이 아는 것은 공고번호이지 건 이름이 아니다.</para>
    /// </summary>
    private static string? Taken(Snapshot state, string requestBase, string 건)
    {
        var owner = state.Links.FirstOrDefault(l => l.Value.NoticeGroup == 건 && l.Key != requestBase);
        return owner.Key is null ? null : $"그 공고에 이미 다른 접수가 이어져 있습니다 ({owner.Key})";
    }

    private static string? Taken(Snapshot state, RequestRow request, NoticeRow notice) =>
        Taken(state, request.Base, notice.Group);

    /// <summary>
    /// 이을 수 있는 후보인가 — 판정을 통과했고, 그 건에 다른 접수가 붙어 있지 않은가.
    /// <b>유일성을 세는 잣대</b>다.
    /// </summary>
    private static bool Passes(Snapshot state, RequestRow request, NoticeRow notice) =>
        Verify(state, request, notice).Passed && Taken(state, request, notice) is null;

    /// <summary>
    /// 접수마다 <b>품목 줄 수가 같은</b> 건들을 짝지어 돌려준다. 이것이 블로킹 키다.
    /// <see cref="Snapshot.Notices"/> 가 건마다 한 줄이라 재공고와 원공고가 여기서
    /// <b>하나로 셈해진다</b> — 줄 수도 다중집합도 같아 둘 다 통과하던 것이 그것으로 풀린다.
    /// 이어진 접수도 낸다 — 걸러 버리면 잇기 화면이 그 줄을 세울 수 없어 끊을 자리가 사라진다.
    ///
    /// <para><b>여기서 나온 수로 유일성을 세지 않는다.</b> 블로킹 키는 줄 수이지만 유일성은
    /// <see cref="Passes"/> 를 통과한 후보 사이에서 센다 — 줄 수가 같은 공고가 여럿인 것은
    /// 흔한 일이고, 그것만으로 사람에게 넘기면 큐가 헛되이 는다.</para>
    /// </summary>
    private static IEnumerable<(RequestRow Request, List<NoticeRow> Matches)> CountMatches(Snapshot state)
    {
        foreach (var request in state.Requests)
        {
            var matches = request.Items.Count == 0
                ? []
                : state.Notices
                    .Where(n => n.Items.Count == request.Items.Count)
                    .Where(n => !state.Rejections.Contains((request.Base, n.Group)))
                    .ToList();

            yield return (request, matches);
        }
    }

    /// <summary>
    /// 두 쪽을 나란히 놓는다. <see cref="Verify"/> 가 통과 여부만 내는 것과 달리 여기서는
    /// <b>무엇이 어떻게 다른지</b>를 그대로 보여 준다 — 사람이 3초에 판단할 재료다.
    /// </summary>
    private static IReadOnlyList<RequestFacet> Compare(Snapshot state, RequestRow request, NoticeRow notice)
    {
        var requestTitle = request.Title ?? string.Empty;
        var noticeTitle = notice.Title ?? string.Empty;

        var bothCounted = request.Items.Count > 0 && notice.Items.Count > 0;
        var comparable = bothCounted
                         && request.Items.Count == notice.Items.Count
                         && !request.Items.Any(i => i.Incomplete)
                         && !notice.Items.Any(i => i.Incomplete);

        var requestNames = Names(request.Items);
        var noticeNames = Names(notice.Items);
        var requestCodes = Codes(request.Items);
        var noticeCodes = Codes(notice.Items);

        return
        [
            new RequestFacet("건명", requestTitle, noticeTitle,
                Linker.TitleKey(requestTitle) == Linker.TitleKey(noticeTitle)),

            new RequestFacet("품목수",
                request.Items.Count.ToString(), notice.Items.Count.ToString(),
                bothCounted ? request.Items.Count == notice.Items.Count : null),

            new RequestFacet("수량·단가", Show(request.Items), Show(notice.Items),
                comparable ? Pairs(request.Items).SequenceEqual(Pairs(notice.Items)) : null),

            new RequestFacet("세부품명", Join(requestNames), Join(noticeNames),
                requestNames.Count > 0 && noticeNames.Count > 0
                    ? requestNames.SetEquals(noticeNames)
                    : null),

            // 어긋나도 막지 않는다 — 협의·요청 누락으로 접수와 공고가 다를 수 있다.
            new RequestFacet("세부품명번호", Join(requestCodes), Join(noticeCodes),
                requestCodes.Count > 0 && noticeCodes.Count > 0
                    ? requestCodes.SetEquals(noticeCodes)
                    : null),

            new RequestFacet("품대 / 사업금액", Amount(request.GoodsAmount), Amount(notice.ProjectAmount),
                request.GoodsAmount is { } g && notice.ProjectAmount is { } p ? g == p : null),

            new RequestFacet("예산금액 / 배정예산", Amount(request.BudgetAmount), Amount(notice.AllocatedBudget),
                request.BudgetAmount is { } b && notice.AllocatedBudget is { } a ? b == a : null),

            new RequestFacet("수요기관", request.Agency ?? string.Empty, Join(state.NoticeAgencies.GetValueOrDefault(notice.Base) ?? []),
                !string.IsNullOrWhiteSpace(request.Agency)
                && state.NoticeAgencies.GetValueOrDefault(notice.Base) is { Count: > 0 } agencies
                    ? agencies.Contains(request.Agency)
                    : null),

            new RequestFacet("접수일자 / 게시일시",
                request.ReceivedOn?.ToString("yyyy-MM-dd") ?? string.Empty,
                notice.PostedAt?.ToString("yyyy-MM-dd") ?? string.Empty,
                request.ReceivedOn is { } received && notice.PostedAt is { } posted
                    ? received < posted
                    : null),
        ];

        static HashSet<string> Names(IReadOnlyList<ItemRow> items) =>
            [.. items.Select(i => i.ItemName).Where(n => !string.IsNullOrWhiteSpace(n))!];

        static HashSet<string> Codes(IReadOnlyList<ItemRow> items) =>
            [.. items.Select(i => i.DetailItemNumber)
                     .Where(n => !string.IsNullOrWhiteSpace(n) && n != "-")!];

        static string Join(IEnumerable<string> values) => string.Join(", ", values.Order(StringComparer.Ordinal));

        static string Amount(decimal? value) => value?.ToString("#,##0") ?? string.Empty;

        // 줄 차례가 서로 달라 자리끼리 견줄 수 없다. 정렬해 늘어놓아야 사람 눈에도 같은 것이 같아 보인다.
        static string Show(IReadOnlyList<ItemRow> items) =>
            string.Join(", ", items
                .Where(i => !i.Incomplete)
                .Select(i => (i.Quantity!.Value, i.UnitPrice!.Value))
                .OrderBy(p => p.Item1).ThenBy(p => p.Item2)
                .Select(p => $"{p.Item1}×{p.Item2:#,##0}"));
    }

    /// <summary>
    /// 어느 조달요구가 어느 공고 품목이 되었는지를 남긴다.
    ///
    /// <para>양쪽을 <b>(수량, 단가, 순번)</b> 으로 정렬해 지퍼처럼 짝짓는다. 같은 (수량,단가)
    /// 가 둘이면 어느 짝을 지어도 값은 같으므로, 순번으로 못 박아 <b>결과가 늘 같게</b> 한다 —
    /// 같은 자료에서 두 번 돌려 다른 짝이 나오면 그 표를 믿을 수 없다.</para>
    ///
    /// <para>링크는 건에 매달리지만 <b>품목 짝은 차수의 일이다</b>. 짝지을 줄이 있는 것은 문서
    /// 한 장이므로 건의 <b>현행 공고</b>를 가리킨다 — 취소된 옛 차수의 줄과 짝지으면 그 줄이
    /// 어느 문서의 것인지가 뒤섞인다.</para>
    /// </summary>
    private static void WriteItemLinks(
        SqliteConnection connection, SqliteTransaction transaction,
        Snapshot state, string requestBase, string 건)
    {
        connection.Execute(
            "DELETE FROM request_item_link WHERE request_base = @r;", new { r = requestBase }, transaction);

        var request = state.Requests.FirstOrDefault(r => r.Base == requestBase);
        var notice = 현행(state, 건);

        if (request is null || notice is null) return;
        if (request.Items.Count == 0 || request.Items.Count != notice.Items.Count) return;
        if (request.Items.Any(i => i.Incomplete) || notice.Items.Any(i => i.Incomplete)) return;
        if (!Pairs(request.Items).SequenceEqual(Pairs(notice.Items))) return;

        var left = Zipped(request.Items);
        var right = Zipped(notice.Items);

        for (var i = 0; i < left.Count; i++)
            connection.Execute(
                """
                INSERT INTO request_item_link
                    (request_base, request_seq, line_no, notice_base, notice_seq, notice_line_no)
                VALUES (@rb, @rs, @rl, @nb, @ns, @nl);
                """,
                new
                {
                    rb = requestBase, rs = request.Seq, rl = left[i].LineNo,
                    nb = notice.Base, ns = notice.Seq, nl = right[i].LineNo,
                }, transaction);

        static List<ItemRow> Zipped(IReadOnlyList<ItemRow> items) =>
            [.. items.OrderBy(i => i.Quantity!.Value)
                     .ThenBy(i => i.UnitPrice!.Value)
                     .ThenBy(i => i.LineNo)];
    }

    // ── 읽기 ─────────────────────────────────────────────

    /// <param name="Incomplete">수량이나 단가를 읽지 못한 줄. 다중집합을 믿을 수 없게 만든다.</param>
    private sealed record ItemRow(
        int LineNo, int? Quantity, decimal? UnitPrice, string? ItemName, string? DetailItemNumber)
    {
        public bool Incomplete => Quantity is null || UnitPrice is null;
    }

    private sealed record RequestRow(
        string Base, string Seq, string? Title, string? Agency, DateTime? ReceivedOn,
        decimal? GoodsAmount, decimal? BudgetAmount, IReadOnlyList<ItemRow> Items);

    /// <param name="Group">이 줄이 대표하는 <b>건</b>. 링크와 거부가 매달리는 끝점이다.</param>
    /// <param name="Base">그 건의 <b>현행 공고</b> 본번호. 사람에게 보이고 품목을 가져오는 쪽이다.</param>
    private sealed record NoticeRow(
        string Group, string Base, string Seq, string? Title, DateTime? PostedAt,
        decimal? ProjectAmount, decimal? AllocatedBudget, long Linked, IReadOnlyList<ItemRow> Items);

    /// <param name="Notices">
    /// <b>건마다 한 줄</b>이고, 그 줄이 담은 번호·건명·품목은 건의 <b>현행 공고</b>의 것이다.
    /// 세는 단위가 건이라야 재공고 건에서 후보가 둘로 갈리지 않는다.
    /// </param>
    private sealed record Snapshot(
        List<RequestRow> Requests,
        List<NoticeRow> Notices,
        Dictionary<string, (string NoticeGroup, string DecidedBy)> Links,
        Dictionary<string, HashSet<string>> NoticeAgencies,
        HashSet<(string Request, string Group)> Rejections);

    /// <summary>
    /// 판정에 필요한 것을 한 번에 읽어 둔다.
    ///
    /// <para><b>품목은 최신 차수의 것만</b> 읽는다. <see cref="Linker"/> 는 이름·번호를 계열
    /// 전체에서 모으지만(옛 차수에만 찍힌 값을 버리지 않으려고), 여기서 견주는 것은 <b>줄 수와
    /// 다중집합</b>이라 차수를 섞으면 줄이 두 배가 되어 판정이 통째로 무너진다.</para>
    /// </summary>
    private static Snapshot Read(SqliteConnection connection)
    {
        var requestItems = Items(connection,
            """
            SELECT i.request_base AS a, i.line_no, i.quantity, i.unit_price, i.item_name, i.detail_item_number
            FROM request_item i
            JOIN (SELECT request_base AS b, MAX(CAST(seq AS INTEGER)) AS s
                  FROM request GROUP BY request_base) latest
              ON i.request_base = latest.b AND CAST(i.seq AS INTEGER) = latest.s;
            """);

        var noticeItems = Items(connection,
            """
            SELECT i.notice_base AS a, i.line_no, i.quantity, i.unit_price, i.item_name, i.detail_item_number
            FROM notice_item i
            JOIN (SELECT notice_base AS b, MAX(CAST(seq AS INTEGER)) AS s
                  FROM notice GROUP BY notice_base) latest
              ON i.notice_base = latest.b AND CAST(i.seq AS INTEGER) = latest.s;
            """);

        var requests = connection.Query<(string Base, string Seq, string? Title, string? Agency,
                string? ReceivedOn, string? Goods, string? Budget)>(
            """
            SELECT r.request_base, r.seq, r.title, r.demand_agency, r.received_on,
                   r.goods_amount, r.budget_amount
            FROM request r
            JOIN (SELECT request_base AS b, MAX(CAST(seq AS INTEGER)) AS s
                  FROM request GROUP BY request_base) latest
              ON r.request_base = latest.b AND CAST(r.seq AS INTEGER) = latest.s
            ORDER BY r.request_base;
            """)
            .Select(r => new RequestRow(
                r.Base, r.Seq, r.Title, r.Agency, Moment(r.ReceivedOn),
                ValueParser.Money(r.Goods), ValueParser.Money(r.Budget),
                requestItems.GetValueOrDefault(r.Base) ?? []))
            .ToList();

        // 잇는 단위가 <b>건</b>이라 건마다 한 줄만 세운다. 그 줄이 되는 것은 건의
        // <b>현행 공고</b> — 대체되지 않은 한 장이고, 사람에게 보일 번호이자 견줄 품목을
        // 가져올 자리다. 고르는 규칙은 NoticeGroups.현행공고 하나뿐이라 뷰와 갈릴 수 없다.
        //
        // 품목이 현행 공고의 것이라야 다중집합이 성립한다. 건에 선 문서를 다 모으면 재공고 건에서
        // 줄이 두 배가 되어 판정이 통째로 무너진다 — 차수를 섞지 않는 것과 같은 까닭이다.
        var notices = connection.Query<(string Group, string Base, string Seq, string? Title, string? PostedAt,
                string? Project, string? Allocated, long Linked)>(
            $"""
            SELECT 현행.group_base, n.notice_base, n.seq, n.title, n.posted_at,
                   n.project_amount, n.allocated_budget,
                   (SELECT COUNT(*) FROM request_link l WHERE l.notice_group = 현행.group_base) AS linked
            FROM ({NoticeGroups.현행공고}) 현행
            JOIN notice n ON n.notice_base = 현행.notice_base AND n.seq = 현행.seq
            ORDER BY n.notice_base;
            """)
            .Select(n => new NoticeRow(
                n.Group, n.Base, n.Seq, n.Title, Moment(n.PostedAt),
                ValueParser.Money(n.Project), ValueParser.Money(n.Allocated), n.Linked,
                noticeItems.GetValueOrDefault(n.Base) ?? []))
            .ToList();

        var links = connection.Query<(string Request, string Group, string? DecidedBy)>(
            "SELECT request_base, notice_group, decided_by FROM request_link;")
            .ToDictionary(r => r.Request, r => (r.Group, r.DecidedBy ?? string.Empty));

        // 거부도 건에 매여 있다 — 재공고를 물리치면 원공고도 함께 내려간다.
        var rejections = connection.Query<(string Request, string Group)>(
            "SELECT request_base, notice_group FROM request_link_rejection;").ToHashSet();

        var agencies = new Dictionary<string, HashSet<string>>();
        foreach (var row in connection.Query<(string A, string V)>(
                     "SELECT notice_base AS a, demand_agency AS v FROM notice_item WHERE demand_agency <> '';"))
        {
            if (!agencies.TryGetValue(row.A, out var set)) agencies[row.A] = set = [];
            set.Add(row.V);
        }

        return new Snapshot(requests, notices, links, agencies, rejections);
    }

    private static Dictionary<string, IReadOnlyList<ItemRow>> Items(SqliteConnection connection, string sql)
    {
        var map = new Dictionary<string, List<ItemRow>>();

        foreach (var row in connection.Query<(string A, long LineNo, long? Quantity,
                     string? UnitPrice, string? ItemName, string? DetailItemNumber)>(sql))
        {
            if (!map.TryGetValue(row.A, out var list)) map[row.A] = list = [];

            list.Add(new ItemRow(
                (int)row.LineNo,
                row.Quantity is null ? null : (int)row.Quantity.Value,
                ValueParser.Money(row.UnitPrice),
                row.ItemName,
                row.DetailItemNumber));
        }

        return map.ToDictionary(
            kv => kv.Key,
            kv => (IReadOnlyList<ItemRow>)[.. kv.Value.OrderBy(i => i.LineNo)]);
    }

    /// <summary>
    /// 그 건에 선 <b>현행 공고</b>. 없으면 <c>null</c> — 건의 문서가 서로를 가리켜 모두
    /// 대체된 자리이고, 그때는 <c>v_공고.현행공고</c> 도 빈다.
    /// </summary>
    private static NoticeRow? 현행(Snapshot state, string 건) =>
        state.Notices.FirstOrDefault(n => n.Group == 건);

    /// <summary>저장된 시각 문자열을 값으로. 읽지 못하면 null — 없는 값과 같이 다룬다.</summary>
    private static DateTime? Moment(string? text) =>
        DateTime.TryParse(text, out var value) ? value : null;

    /// <summary>
    /// 사람의 확정을 적는다. 옛 링크를 덮으면 그 근거·규칙판도 함께 비운다 — 남기면 사람이 고른
    /// 짝에 기계의 근거가 붙어 보인다. 열은 스키마에 그대로 둔다(옛 <c>auto</c> 줄이 쓴다).
    /// </summary>
    private static void Write(
        SqliteConnection connection, SqliteTransaction transaction,
        string requestBase, string noticeGroup, double confidence) =>
        connection.Execute(
            """
            INSERT INTO request_link
                (request_base, notice_group, confidence, confirmed_at, decided_by, evidence, rule_version)
            VALUES
                (@requestBase, @noticeGroup, @confidence, @now, 'human', NULL, NULL)
            ON CONFLICT(request_base) DO UPDATE SET
                notice_group = excluded.notice_group,
                confidence = excluded.confidence,
                confirmed_at = excluded.confirmed_at,
                decided_by = excluded.decided_by,
                evidence = excluded.evidence,
                rule_version = excluded.rule_version;
            """,
            new
            {
                requestBase, noticeGroup, confidence,
                now = DateTime.UtcNow.ToString("O"),
            }, transaction);
}

using Dapper;
using Microsoft.Data.Sqlite;

namespace Pclm.Core.Storage;

/// <summary>
/// 계약 한 건에 붙일 공고 후보.
/// </summary>
/// <param name="TitleMatched">건명이 정확히 일치하는가. 아니면 약한 근거로 올라온 후보다.</param>
/// <param name="Blocker">잇기 전에 걸리는 점. 없으면 판정을 통과한 유일한 후보다.</param>
public sealed record LinkCandidate(
    EntityRef Contract,
    string ContractTitle,
    EntityRef Notice,
    string NoticeTitle,
    double Confidence,
    string Reason,
    long NoticeContractCount,
    bool TitleMatched = false,
    string? Blocker = null,
    IReadOnlyList<LinkFacet>? Facets = null)
{
    /// <summary>나란히 견준 항목들. 사람이 3초에 판단하게 하는 자리다.</summary>
    public IReadOnlyList<LinkFacet> Compared => Facets ?? [];
}

/// <summary>
/// 두 쪽을 나란히 놓고 견준 항목 하나.
/// </summary>
/// <param name="Agrees">맞는가. <c>null</c> 이면 한쪽 값이 없어 <b>판정할 수 없다</b>는 뜻이다.</param>
public sealed record LinkFacet(string Name, string Contract, string Notice, bool? Agrees);

/// <summary>
/// 잇기 화면에 세울 계약 한 줄. <b>이어진 것도 낸다</b> — 잘못 이어진 것을 끊고 다시 이으려면
/// 그 줄이 화면에 서 있어야 한다. 후보가 하나도 없는 것도 낸다.
/// </summary>
/// <param name="Notice">지금 이어져 있는 공고. <c>null</c> 이면 아직 이어지지 않았다.</param>
/// <param name="DecidedBy">
/// 누가 이었는가 — <c>human</c>·<c>explicit</c>(ERP 명시 참조), 옛 판이 남긴 <c>auto</c>.
/// 이어지지 않았으면 <c>null</c>.
/// </param>
public sealed record LinkRow(
    EntityRef Contract, string Title, int CandidateCount,
    EntityRef? Notice, string NoticeTitle, string? DecidedBy);

/// <summary>사람이 손수 고를 공고 하나. 추천이 찾지 못한 짝을 고르는 자리에 세운다.</summary>
/// <param name="Linked">이 공고에 이미 붙은 계약 수. 한 공고에 계약이 여럿일 수 있어 막지는 않는다.</param>
public sealed record NoticeChoice(EntityRef Notice, string Title, string PostedAt, long Linked);

/// <summary>잇기 화면이 한 번에 받아 가는 것.</summary>
public sealed record LinkWork(
    IReadOnlyList<LinkRow> Contracts,
    IReadOnlyList<LinkCandidate> Candidates);

/// <summary>
/// 공고와 계약을 잇는다.
///
/// <para>계약서 어디에도 공고번호가 찍히지 않는다. 그래도 <b>나라장터가 공고명을 계약명으로
/// 그대로 옮긴다</b> — 전파된 이름의 정확 일치는 정황 증거가 아니라 사실상 파생 키다.
/// 그래서 건명을 <b>블로킹 키</b>로 세우고, 부수 확인을 통과했는지와 후보가 몇인지를
/// 후보마다 적어 <b>추천한다</b>.</para>
///
/// <para><b>기계는 잇지 않는다</b>(ADR-029). 링크는 ERP 명시 참조(<c>ExplicitLinks</c>)와
/// 사람의 확정으로만 선다 — 한때 통과한 유일 후보를 기계가 스스로 이었으나(ADR-015) 걷었다.
/// 옛 판이 남긴 <c>decided_by = 'auto'</c> 줄은 그대로 두고 그대로 보인다.</para>
///
/// <para>한 공고에 계약이 여러 건 달릴 수 있다(분할 낙찰·수요기관 복수). 계약 쪽에서 보면
/// 후보 공고는 여전히 하나다.</para>
///
/// <para><b>세부품명이 달라도 물품분류번호 앞 8자리가 같으면 같은 품목이다</b> — 8자리 기준
/// 이름과 10자리 기준 이름이 문서마다 섞여 찍힌다. 계약서는 물품분류번호(8자리)에 딸린
/// 이름을, 공고서는 세부품명번호(10자리)에 딸린 이름을 적어서, 같은 물건이 「유독또는
/// 가스탐지기」와 「가스탐지기」로 갈라진다. 이름이 겹치지 않을 때 번호의 앞 8자리를
/// 대신 본다 — 이름 대조를 느슨하게 푸는 것이 아니라 <b>더 굳은 근거</b>로 갈아 대는 것이다.</para>
/// </summary>
public sealed class Linker(Database database)
{
    private readonly Database _database = database;

    /// <summary>건명이 맞는 공고가 없을 때 쓰는 약한 점수의 바닥. 이 아래는 후보로도 내놓지 않는다.</summary>
    private const double Floor = 0.3;

    /// <summary>
    /// 건명 대조용 키. <b>공백만 지운다.</b>
    ///
    /// <para>더 다듬고 싶은 유혹이 있지만 다듬을수록 <b>서로 다른 조달 건이 같은 키로 뭉개진다</b>.
    /// 후행 계약유형 낱말(구매·조달·납품)을 떼면 <c>…절단기 구매</c> 와 <c>…절단기 조달</c> 이
    /// 한 키가 되어 유일성 판정이 망가진다. 실측에서도 뗄 이유가 없었다 — 코퍼스 세 쌍 모두
    /// 공백만 지우면 완전히 같다. 한때 어긋나 보였던 한 건은 양식이 아니라
    /// <b>파서가 값을 자르고 있던 것</b>이었다.</para>
    /// </summary>
    public static string TitleKey(string? title)
    {
        if (string.IsNullOrWhiteSpace(title)) return string.Empty;

        Span<char> buffer = stackalloc char[title.Length];
        var n = 0;

        foreach (var ch in title)
            if (!char.IsWhiteSpace(ch)) buffer[n++] = ch;

        return new string(buffer[..n]);
    }

    /// <summary>
    /// 물품분류번호 대조용 키. <b>숫자만 남겨 앞 8자리</b>를 쓰고, 여덟 자리에 못 미치면
    /// 버린다(<c>null</c>).
    ///
    /// <para>계약서에는 물품분류번호 8자리가, 공고서에는 세부품명번호 10자리가 찍힌다.
    /// 뒤 두 자리는 세부품명을 가르는 자리라 계약서 쪽에 아예 없으므로, 자릿수를 맞추는 유일한
    /// 길이 앞 8자리다.</para>
    /// </summary>
    public static string? ClassKey(string? number)
    {
        if (string.IsNullOrWhiteSpace(number)) return null;

        Span<char> buffer = stackalloc char[number.Length];
        var n = 0;

        foreach (var ch in number)
            if (char.IsAsciiDigit(ch)) buffer[n++] = ch;

        return n >= 8 ? new string(buffer[..8]) : null;
    }

    /// <summary>
    /// 사람이 볼 후보. 이미 이어진 계약은 여기 오지 않는다.
    ///
    /// <para>두 갈래다. <b>건명이 같은 공고</b>는 걸리는 점(<c>Blocker</c>)과 함께 내놓고,
    /// <b>건명이 맞는 공고가 아예 없는 계약</b>은 물품식별번호·건명 유사도로 약한 후보를 낸다.</para>
    /// </summary>
    /// <remarks>
    /// <see cref="Work"/> 가 이어진 계약의 <b>갈아탈 후보</b>까지 내므로 여기서 그것을 걸러 낸다 —
    /// 이 목록은 "사람이 아직 결정하지 않은 것" 이고, 그 뜻이 흔들리면 명령줄이 내던 큐가 달라진다.
    /// 이어진 계약에서 다른 공고로 갈아타는 길은 창의 잇기 화면에만 있다.
    /// </remarks>
    public IReadOnlyList<LinkCandidate> Candidates()
    {
        var work = Work();
        var linked = work.Contracts.Where(r => r.Notice is not null).Select(r => r.Contract.Base).ToHashSet();

        return [.. work.Candidates.Where(c => !linked.Contains(c.Contract.Base))];
    }

    /// <summary>
    /// 잇기 화면이 한 번에 받아 갈 것. 계약을 <b>이어진 것까지</b> 모두 내고, 그 계약마다 후보를 붙인다.
    ///
    /// <para>후보 목록만 내면 후보가 없는 계약이 화면에서 통째로 사라져 미연결로 남은 줄도 모르게 되고,
    /// 이어진 계약을 빼면 <b>잘못 이어진 것을 끊을 자리가 없다</b> — 끊기는 그 줄을 골라야 나오는 동작이다.</para>
    ///
    /// <para>이어진 계약의 후보는 <b>건명이 같은 다른 공고</b>까지만 낸다. 약한 점수는 계약×공고
    /// 전수라 이어진 것까지 돌리면 값을 두 배로 치른다 — 거기서 더 나가는 길은 화면의 「직접 찾기」다.</para>
    /// </summary>
    public LinkWork Work()
    {
        using var connection = _database.Open();
        var state = Read(connection);

        var results = new List<LinkCandidate>();
        var contracts = new List<LinkRow>();

        foreach (var (contract, matches) in TitleMatches(state))
        {
            var before = results.Count;
            var self = new EntityRef("contract", contract.Base, contract.Seq);

            if (state.Links.TryGetValue(contract.Base, out var link))
            {
                // 지금 이어진 건은 후보에서 뺀다 — 화면이 「지금 이어진 공고」 카드로 따로 그린다.
                foreach (var notice in matches.Where(n => n.Group != link.NoticeGroup))
                {
                    var verdict = Verify(state, contract, notice);
                    results.Add(Candidate(contract, notice, 0.9,
                        verdict.Passed ? verdict.Evidence : "건명 일치",
                        verdict.Blocker, titleMatched: true,
                        Compare(state, contract, notice)));
                }

                var linked = state.Notices.FirstOrDefault(n => n.Group == link.NoticeGroup);

                contracts.Add(new LinkRow(
                    self,
                    contract.Title ?? string.Empty,
                    results.Count - before,
                    // 보이는 것은 <b>현행 공고 번호</b>다. 건 이름은 문서가 없는 번호일 수 있어
                    // (가스성분분석기가 가리키는 R26BK09013019) 사람 앞에 낼 것이 못 된다. 이름으로 물러서는
                    // 것은 건에 현행이 하나도 없을 때뿐이고, 그 자리는 v_공고.현행공고 도 빈다.
                    new EntityRef("notice", linked?.Base ?? link.NoticeGroup, linked?.Seq ?? string.Empty),
                    linked?.Title ?? string.Empty,
                    link.DecidedBy));

                continue;
            }

            if (matches.Count > 0)
            {
                // 후보가 여럿이면 어느 하나가 통과하더라도 그것만으로는 가를 수 없다.
                var ambiguous = matches.Count > 1
                    ? $"같은 이름 공고가 {matches.Count}건입니다"
                    : null;

                foreach (var notice in matches)
                {
                    var verdict = Verify(state, contract, notice);
                    results.Add(Candidate(contract, notice, 0.9,
                        verdict.Passed ? verdict.Evidence : "건명 일치",
                        ambiguous ?? verdict.Blocker, titleMatched: true,
                        Compare(state, contract, notice)));
                }

                contracts.Add(new LinkRow(
                    self, contract.Title ?? string.Empty, matches.Count,
                    null, string.Empty, null));

                continue;
            }

            foreach (var notice in state.Notices)
            {
                if (state.Rejections.Contains((contract.Base, notice.Group))) continue;

                var (score, reason) = WeakScore(state, contract, notice);
                if (score >= Floor)
                    results.Add(Candidate(contract, notice, score, reason, null, titleMatched: false,
                        Compare(state, contract, notice)));
            }

            contracts.Add(new LinkRow(
                self, contract.Title ?? string.Empty, results.Count - before,
                null, string.Empty, null));
        }

        return new LinkWork(
            contracts,
            [.. results.OrderBy(r => r.Contract.Base).ThenByDescending(r => r.Confidence)]);
    }

    /// <summary>
    /// 공고를 모두 낸다. <b>추천이 찾지 못한 짝을 사람이 손수 고르는 자리</b>에 쓴다.
    ///
    /// <para>줄은 <b>건마다 하나</b>이고 번호는 그 건의 <b>현행 공고</b>다 — 취소된 옛 본번호가
    /// 따로 서지 않는다. 건 이름을 내지 않는 까닭은 그것이 문서 없는 번호일 수 있어서다.</para>
    ///
    /// <para>거르기는 화면이 한다 — 이 저장소의 다른 목록도 다 그렇게 하고, 몇백 줄에 검색
    /// 질의를 새로 파는 것은 값보다 짐이 크다.</para>
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

    /// <summary>
    /// 후보에 없는 임의의 짝도 나란히 견준다. <b>잇기 전에 눈으로 볼 재료</b>다.
    ///
    /// <para>추천 밖에서 고른 짝이라고 근거 없이 잇게 두지 않는다 — 후보 카드와 같은 표를
    /// 보고 누르는 것이 이 화면의 약속이다.</para>
    /// </summary>
    public IReadOnlyList<LinkFacet> Compare(EntityRef contract, EntityRef notice)
    {
        using var connection = _database.Open();
        var state = Read(connection);

        var left = state.Contracts.FirstOrDefault(c => c.Base == contract.Base)
            ?? throw new InvalidOperationException($"그런 계약이 없습니다: {contract.Display}");

        // 사람이 건넨 것은 공고번호라 건으로 옮겨 세운다 — 대체된 차수나 취소된 본번호를
        // 골라도 그 건의 현행 공고와 견준다. 견주는 단위가 건이기 때문이다.
        var 건 = NoticeGroups.RequireGroupOf(connection, notice);

        var right = state.Notices.FirstOrDefault(n => n.Group == 건)
            ?? throw new InvalidOperationException($"그런 공고가 없습니다: {notice.Display}");

        return Compare(state, left, right);
    }

    /// <summary>
    /// 사람이 고른 짝을 확정한다. 같은 계약을 다시 이으면 덮어쓴다.
    ///
    /// <para><b>차수가 아니라 계열끼리 잇는다.</b> 사람이 확정하는 것은 "이 계약 건이 저 공고에서
    /// 나왔다" 는 사실이고, 그 사실은 변경계약이 들어와도 그대로다. 차수에 매달면 변경계약
    /// 한 건에 링크가 끊겨 통합 뷰의 공고 열이 통째로 비었다(ADR-012).</para>
    ///
    /// <para><b>받는 것은 공고번호이고 매다는 것은 건이다.</b> 사람이 고르는 것은 눈에 보이는
    /// 번호라 재공고 건의 어느 본번호든 올 수 있는데, 링크의 끝점은 그 건의 이름 하나뿐이다 —
    /// 여기서 옮겨 두지 않으면 뿌리가 아닌 본번호로 확정할 때 외래키가 던진다.</para>
    /// </summary>
    public void Confirm(EntityRef contract, EntityRef notice, double confidence)
    {
        using var connection = _database.Open();
        var 건 = NoticeGroups.RequireGroupOf(connection, notice);

        // 사람이 손수 이었다면 그 짝에 대한 옛 거부는 뜻을 잃는다. 거부도 건에 매여 있으므로
        // 재공고를 물리쳐 두었다가 원공고로 확정해도 함께 풀린다 — 둘은 같은 말이다.
        connection.Execute(
            "DELETE FROM link_rejection WHERE contract_base = @c AND notice_group = @n;",
            new { c = contract.Base, n = 건 });

        Write(connection, contract.Base, 건, confidence);
    }

    /// <summary>
    /// 이 짝은 아니라고 판정한다.
    ///
    /// <para>거부를 남기지 않으면 물리친 후보가 다음에도 목록 맨 위에 다시 떠서 <b>사람 큐가
    /// 줄지 않는다</b>. 확정만 기록하던 시절의 실제 불편이었다.</para>
    ///
    /// <para><b>물리치는 것도 건이다.</b> 재공고를 아니라고 하면 원공고도 함께 내려간다 —
    /// 둘은 한 조달 건이라 한쪽만 아니라고 할 수 있는 것이 아니다.</para>
    /// </summary>
    public void Reject(EntityRef contract, EntityRef notice)
    {
        using var connection = _database.Open();
        var 건 = NoticeGroups.RequireGroupOf(connection, notice);

        using var transaction = connection.BeginTransaction();

        // 그 건으로 이어져 있었다면 함께 푼다 — 아니라고 판정한 짝이 링크로 남을 수는 없다.
        connection.Execute(
            "DELETE FROM project_link WHERE contract_base = @c AND notice_group = @n;",
            new { c = contract.Base, n = 건 }, transaction);

        connection.Execute(
            """
            INSERT INTO link_rejection (contract_base, notice_group, rejected_at)
            VALUES (@c, @n, @now)
            ON CONFLICT(contract_base, notice_group) DO NOTHING;
            """,
            new { c = contract.Base, n = 건, now = DateTime.UtcNow.ToString("O") }, transaction);

        transaction.Commit();
    }

    /// <summary>링크를 푼다. 거부와 달리 "아니다" 라는 판정은 남기지 않는다.</summary>
    public void Unlink(EntityRef contract)
    {
        using var connection = _database.Open();
        connection.Execute(
            "DELETE FROM project_link WHERE contract_base = @c;",
            new { c = contract.Base });
    }

    // ── 규칙 ─────────────────────────────────────────────

    private sealed record Verdict(bool Passed, string Evidence, string? Blocker);

    /// <summary>
    /// 건명 말고 무엇이 더 맞아야 하는가.
    ///
    /// <para><b>세부품명은 반드시 맞아야 한다</b> — 모든 문서에 있고 반복구매를 가르는 확인이다.
    /// 읽지 못했으면 통과시키지 않는다. <b>수요기관과 시간은 값이 양쪽에 있을 때만 본다</b>:
    /// 없는 값은 위반이 아니라 판정 불가다. 초안 계약서에는 계약일자가 아예 비어 있어서,
    /// 없는 것을 어긋난 것으로 치면 멀쩡한 짝이 통째로 탈락한다.</para>
    ///
    /// <para>이름이 겹치지 않으면 <b>물품분류번호 앞 8자리</b>를 본다. 이름은 8자리 기준의 것과
    /// 10자리 기준의 것이 문서마다 섞여 찍히지만 번호의 앞 8자리는 그렇지 않다. 그래서 이름이
    /// 아예 없어도 번호가 양쪽에 있으면 판정하고, <b>둘 다 없을 때에만</b> 읽지 못했다고 한다.</para>
    /// </summary>
    private static Verdict Verify(Snapshot state, ContractRow contract, NoticeRow notice)
    {
        var reasons = new List<string> { "건명 일치" };

        var contractItems = state.ContractItems.GetValueOrDefault(contract.Base) ?? [];
        var noticeItems = state.NoticeItems.GetValueOrDefault(notice.Base) ?? [];
        var contractClasses = state.ContractClasses.GetValueOrDefault(contract.Base) ?? [];
        var noticeClasses = state.NoticeClasses.GetValueOrDefault(notice.Base) ?? [];

        // 이름도 번호도 없는 쪽이 있으면 견줄 것 자체가 없다.
        if ((contractItems.Count == 0 && contractClasses.Count == 0)
            || (noticeItems.Count == 0 && noticeClasses.Count == 0))
            return new Verdict(false, string.Empty, "세부품명을 읽지 못했습니다");

        if (contractItems.Overlaps(noticeItems))
        {
            reasons.Add("세부품명 일치");
        }
        else if (contractClasses.Intersect(noticeClasses).Order().ToList() is { Count: > 0 } shared)
        {
            reasons.Add($"물품분류번호 앞 8자리 일치 ({string.Join(", ", shared)})");
        }
        else
        {
            return new Verdict(false, string.Empty, "세부품명이 다릅니다");
        }

        var agencies = state.NoticeAgencies.GetValueOrDefault(notice.Base);
        if (!string.IsNullOrWhiteSpace(contract.Agency) && agencies is { Count: > 0 })
        {
            if (!agencies.Contains(contract.Agency))
                return new Verdict(false, string.Empty, $"수요기관이 다릅니다 ({contract.Agency})");

            reasons.Add("수요기관 일치");
        }

        // 계약은 언제나 공고 뒤에 온다. 어긋나면 물리적으로 불가능한 짝이다.
        if (contract.ContractedOn is { } signed && notice.PostedAt is { } posted)
        {
            if (signed <= posted)
                return new Verdict(false, string.Empty,
                    $"계약일 {signed:yyyy-MM-dd} 이 게시일 {posted:yyyy-MM-dd} 보다 앞섭니다");

            reasons.Add($"계약일 {signed:yyyy-MM-dd} > 게시일 {posted:yyyy-MM-dd}");
        }

        return new Verdict(true, string.Join(" · ", reasons), null);
    }

    /// <summary>
    /// 계약마다 건명이 같은 <b>건</b>들을 짝지어 돌려준다. <c>Snapshot.Notices</c> 가 건마다
    /// 한 줄이라 재공고와 원공고가 여기서 <b>하나로 셈해진다</b> — 이름이 같은 두 줄로 서서
    /// 유일성 판정을 막던 것이 그것으로 풀린다. <b>이어진 계약도 낸다</b> —
    /// 이어진 것을 여기서 걸러 버리면 잇기 화면이 그 줄을 세울 수 없어 끊을 자리가 사라진다.
    /// 이어진 것을 어떻게 다룰지는 부르는 쪽이 <see cref="Snapshot.Links"/> 를 보고 정한다.
    /// </summary>
    private static IEnumerable<(ContractRow Contract, List<NoticeRow> Matches)> TitleMatches(Snapshot state)
    {
        var byTitle = new Dictionary<string, List<NoticeRow>>();

        foreach (var notice in state.Notices)
        {
            var key = TitleKey(notice.Title);
            if (key.Length == 0) continue;

            if (!byTitle.TryGetValue(key, out var list)) byTitle[key] = list = [];
            list.Add(notice);
        }

        foreach (var contract in state.Contracts)
        {
            var key = TitleKey(contract.Title);
            var matches = key.Length > 0 && byTitle.TryGetValue(key, out var found)
                ? found.Where(n => !state.Rejections.Contains((contract.Base, n.Group))).ToList()
                : [];

            yield return (contract, matches);
        }
    }

    /// <summary>
    /// 건명이 맞는 공고가 없을 때의 약한 점수. <b>사람에게 보여줄 순서를 정할 뿐</b>이다.
    /// </summary>
    private static (double Score, string Reason) WeakScore(Snapshot state, ContractRow contract, NoticeRow notice)
    {
        var reasons = new List<string>();
        var score = 0.0;

        if (state.ContractItemIds.TryGetValue(contract.Base, out var a)
            && state.NoticeItemIds.TryGetValue(notice.Base, out var b)
            && a.Overlaps(b))
        {
            score += 0.6;
            reasons.Add("물품식별번호 일치");
        }

        var similarity = TitleSimilarity(contract.Title ?? string.Empty, notice.Title ?? string.Empty);
        if (similarity > 0)
        {
            score += similarity * 0.4;
            reasons.Add($"건명 유사 {similarity:P0}");
        }

        return (Math.Min(score, 1.0), string.Join(" · ", reasons));
    }

    private static LinkCandidate Candidate(
        ContractRow contract, NoticeRow notice,
        double confidence, string reason, string? blocker, bool titleMatched,
        IReadOnlyList<LinkFacet> facets) =>
        new(new EntityRef("contract", contract.Base, contract.Seq),
            contract.Title ?? string.Empty,
            new EntityRef("notice", notice.Base, notice.Seq),
            notice.Title ?? string.Empty,
            confidence, reason, notice.Linked, titleMatched, blocker, facets);

    /// <summary>
    /// 두 쪽을 나란히 놓는다. <see cref="Verify"/> 가 통과 여부만 내는 것과 달리 여기서는
    /// <b>무엇이 어떻게 다른지</b>를 그대로 보여 준다 — 사람이 판단할 재료다.
    /// </summary>
    private static IReadOnlyList<LinkFacet> Compare(Snapshot state, ContractRow contract, NoticeRow notice)
    {
        var contractItems = state.ContractItems.GetValueOrDefault(contract.Base) ?? [];
        var noticeItems = state.NoticeItems.GetValueOrDefault(notice.Base) ?? [];
        var contractNumbers = state.ContractClassNumbers.GetValueOrDefault(contract.Base) ?? [];
        var noticeNumbers = state.NoticeClassNumbers.GetValueOrDefault(notice.Base) ?? [];
        var contractClasses = state.ContractClasses.GetValueOrDefault(contract.Base) ?? [];
        var noticeClasses = state.NoticeClasses.GetValueOrDefault(notice.Base) ?? [];
        var contractIds = state.ContractItemIds.GetValueOrDefault(contract.Base) ?? [];
        var noticeIds = state.NoticeItemIds.GetValueOrDefault(notice.Base) ?? [];
        var agencies = state.NoticeAgencies.GetValueOrDefault(notice.Base) ?? [];

        var contractTitle = contract.Title ?? string.Empty;
        var noticeTitle = notice.Title ?? string.Empty;

        return
        [
            new LinkFacet("건명", contractTitle, noticeTitle,
                TitleKey(contractTitle) == TitleKey(noticeTitle)),

            new LinkFacet("세부품명", Join(contractItems), Join(noticeItems),
                Both(contractItems, noticeItems) ? contractItems.Overlaps(noticeItems) : null),

            // 보이는 것은 문서에 찍힌 번호 그대로이고, 맞는지는 앞 8자리로 본다 —
            // 계약서는 8자리, 공고서는 10자리라 통째로 견주면 늘 어긋난다.
            new LinkFacet("물품분류번호", Join(contractNumbers), Join(noticeNumbers),
                Both(contractClasses, noticeClasses) ? contractClasses.Overlaps(noticeClasses) : null),

            new LinkFacet("물품식별번호", Join(contractIds), Join(noticeIds),
                Both(contractIds, noticeIds) ? contractIds.Overlaps(noticeIds) : null),

            new LinkFacet("수요기관", contract.Agency ?? string.Empty, Join(agencies),
                !string.IsNullOrWhiteSpace(contract.Agency) && agencies.Count > 0
                    ? agencies.Contains(contract.Agency)
                    : null),

            new LinkFacet("계약일 / 게시일",
                contract.ContractedOn?.ToString("yyyy-MM-dd") ?? string.Empty,
                notice.PostedAt?.ToString("yyyy-MM-dd") ?? string.Empty,
                contract.ContractedOn is { } signed && notice.PostedAt is { } posted
                    ? signed > posted
                    : null),
        ];

        static bool Both(HashSet<string> a, HashSet<string> b) => a.Count > 0 && b.Count > 0;
        static string Join(IEnumerable<string> values) => string.Join(", ", values.Order());
    }

    /// <summary>낱말 단위로 겹치는 비율. 건명은 표기가 조금씩 달라 글자 단위 비교로는 놓친다.</summary>
    private static double TitleSimilarity(string left, string right)
    {
        var a = Words(left);
        var b = Words(right);
        if (a.Count == 0 || b.Count == 0) return 0;

        var shared = a.Count(w => b.Contains(w));
        return (double)shared / Math.Max(a.Count, b.Count);
    }

    private static HashSet<string> Words(string text) =>
        [.. text.Split([' ', '(', ')', ',', '·', '-'], StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries)
                .Where(w => w.Length > 1)];

    // ── 읽기 ─────────────────────────────────────────────

    private sealed record ContractRow(string Base, string Seq, string? Title, string? Agency, DateTime? ContractedOn);

    /// <param name="Group">이 줄이 대표하는 <b>건</b>. 링크와 거부가 매달리는 끝점이다.</param>
    /// <param name="Base">그 건의 <b>현행 공고</b> 본번호. 사람에게 보이고 품목을 가져오는 쪽이다.</param>
    private sealed record NoticeRow(
        string Group, string Base, string Seq, string? Title, DateTime? PostedAt, long Linked);

    /// <summary>판정에 필요한 것을 한 번에 읽어 둔다. 계약×공고를 전수로 훑으므로 질의가 반복되면 안 된다.</summary>
    /// <param name="Notices">
    /// <b>건마다 한 줄</b>이고, 그 줄이 담은 번호·건명·품목은 건의 <b>현행 공고</b>의 것이다.
    /// 세는 단위가 건이라야 재공고 건에서 후보가 둘로 갈리지 않는다.
    /// </param>
    /// <param name="Links">
    /// 계약 계열마다 지금 이어진 <b>건</b>과 <b>누가 이었는지</b>. 화면이 사람의 확정과
    /// 명시 참조·옛 자동 연결을 갈라 적어야 해서 이어졌다는 사실만으로는 모자라다.
    /// </param>
    /// <param name="ContractClassNumbers">문서에 찍힌 그대로의 물품분류번호. 사람에게 보일 값이다.</param>
    /// <param name="ContractClasses">
    /// 그것을 <see cref="ClassKey"/> 로 깎은 <b>앞 8자리</b>. 판정은 이쪽으로만 한다.
    /// </param>
    private sealed record Snapshot(
        List<ContractRow> Contracts,
        List<NoticeRow> Notices,
        Dictionary<string, (string NoticeGroup, string DecidedBy)> Links,
        Dictionary<string, HashSet<string>> ContractItems,
        Dictionary<string, HashSet<string>> NoticeItems,
        Dictionary<string, HashSet<string>> ContractClassNumbers,
        Dictionary<string, HashSet<string>> NoticeClassNumbers,
        Dictionary<string, HashSet<string>> ContractClasses,
        Dictionary<string, HashSet<string>> NoticeClasses,
        Dictionary<string, HashSet<string>> ContractItemIds,
        Dictionary<string, HashSet<string>> NoticeItemIds,
        Dictionary<string, HashSet<string>> NoticeAgencies,
        HashSet<(string Contract, string Group)> Rejections);

    private static Snapshot Read(SqliteConnection connection)
    {
        // 잇는 단위가 계열이므로 대표로 최신 차수를 세운다(사람에게 보여줄 번호이기도 하다).
        var contracts = connection.Query<(string Base, string Seq, string? Title, string? Agency, string? ContractedOn)>(
            """
            SELECT c.contract_base, c.seq, c.title, c.demand_agency, c.contracted_on
            FROM contract c
            JOIN (SELECT contract_base AS b, MAX(CAST(seq AS INTEGER)) AS s
                  FROM contract GROUP BY contract_base) latest
              ON c.contract_base = latest.b AND CAST(c.seq AS INTEGER) = latest.s
            ORDER BY c.contract_base;
            """)
            .Select(r => new ContractRow(r.Base, r.Seq, r.Title, r.Agency, Moment(r.ContractedOn)))
            .ToList();

        // 잇는 단위가 <b>건</b>이라 건마다 한 줄만 세운다. 그 줄이 되는 것은 건의
        // <b>현행 공고</b> — 대체되지 않은 한 장이고, 사람에게 보일 번호이자 견줄 품목·건명을
        // 가져올 자리다. 고르는 규칙은 NoticeGroups.현행공고 하나뿐이라 뷰와 갈릴 수 없다.
        var notices = connection.Query<(string Group, string Base, string Seq, string? Title, string? PostedAt, long Linked)>(
            $"""
            SELECT 현행.group_base, n.notice_base, n.seq, n.title, n.posted_at,
                   (SELECT COUNT(*) FROM project_link l WHERE l.notice_group = 현행.group_base) AS linked
            FROM ({NoticeGroups.현행공고}) 현행
            JOIN notice n ON n.notice_base = 현행.notice_base AND n.seq = 현행.seq
            ORDER BY n.notice_base;
            """)
            .Select(r => new NoticeRow(r.Group, r.Base, r.Seq, r.Title, Moment(r.PostedAt), r.Linked))
            .ToList();

        var links = connection.Query<(string Contract, string Group, string? DecidedBy)>(
            "SELECT contract_base, notice_group, decided_by FROM project_link;")
            .ToDictionary(r => r.Contract, r => (r.Group, r.DecidedBy ?? string.Empty));

        // 거부도 건에 매여 있다 — 재공고를 물리치면 원공고도 함께 내려간다.
        var rejections = connection.Query<(string Contract, string Group)>(
            "SELECT contract_base, notice_group FROM link_rejection;").ToHashSet();

        // 물품분류번호도 같은 방식으로 계열 전체에서 모은다. 찍힌 그대로와 깎은 것을 나란히
        // 두는 까닭은, 보이는 것은 원본이고 견주는 것은 앞 8자리이기 때문이다.
        var contractNumbers = Lookup(connection,
            "SELECT contract_base AS a, classification_number AS v FROM contract_item WHERE classification_number <> '';");

        var noticeNumbers = Lookup(connection,
            "SELECT notice_base   AS a, detail_item_number AS v FROM notice_item   WHERE detail_item_number <> '';");

        // 근거는 계열 전체에서 모은다. 물품식별번호나 품명은 차수가 올라도 그대로인 것이 보통이라,
        // 옛 차수에만 찍힌 값을 버리면 이을 수 있는 짝을 놓친다.
        //
        // 건 전체가 아니라 <b>현행 공고의 계열</b>까지다. 건의 대표 문서를 현행 공고로 정한 이상
        // 견줄 값도 거기서 와야 한다 — 취소된 옛 본번호의 품목까지 끌어오면 무엇을 보고 이었는지가
        // 화면의 견주기 표와 어긋난다.
        return new Snapshot(
            contracts,
            notices,
            links,
            Lookup(connection, "SELECT contract_base AS a, item_name AS v FROM contract_item WHERE item_name <> '';"),
            Lookup(connection, "SELECT notice_base   AS a, item_name AS v FROM notice_item   WHERE item_name <> '';"),
            contractNumbers,
            noticeNumbers,
            Keys(contractNumbers),
            Keys(noticeNumbers),
            Lookup(connection, "SELECT contract_base AS a, item_id_number AS v FROM contract_item WHERE item_id_number <> '';"),
            Lookup(connection, "SELECT notice_base   AS a, item_id_number AS v FROM notice_item   WHERE item_id_number <> '';"),
            Lookup(connection, "SELECT notice_base   AS a, demand_agency  AS v FROM notice_item   WHERE demand_agency  <> '';"),
            rejections);
    }

    /// <summary>저장된 시각 문자열을 값으로. 읽지 못하면 null — 없는 값과 같이 다룬다.</summary>
    private static DateTime? Moment(string? text) =>
        DateTime.TryParse(text, out var value) ? value : null;

    /// <summary>
    /// 사람의 확정을 적는다. 옛 링크를 덮으면 그 근거·규칙판도 함께 비운다 — 남기면 사람이 고른
    /// 짝에 기계의 근거가 붙어 보인다. 열은 스키마에 그대로 둔다(옛 <c>auto</c> 줄이 쓴다).
    /// </summary>
    private static void Write(
        SqliteConnection connection, string contractBase, string noticeGroup, double confidence) =>
        connection.Execute(
            """
            INSERT INTO project_link
                (contract_base, notice_group, confidence, confirmed_at, decided_by, evidence, rule_version)
            VALUES
                (@contractBase, @noticeGroup, @confidence, @now, 'human', NULL, NULL)
            ON CONFLICT(contract_base) DO UPDATE SET
                notice_group = excluded.notice_group,
                confidence = excluded.confidence,
                confirmed_at = excluded.confirmed_at,
                decided_by = excluded.decided_by,
                evidence = excluded.evidence,
                rule_version = excluded.rule_version;
            """,
            new
            {
                contractBase, noticeGroup, confidence,
                now = DateTime.UtcNow.ToString("O"),
            });

    /// <summary>계열 하나가 가진 값들을 모은다. 차수를 넘나들며 합친다.</summary>
    private static Dictionary<string, HashSet<string>> Lookup(SqliteConnection connection, string sql)
    {
        var map = new Dictionary<string, HashSet<string>>();

        foreach (var row in connection.Query<(string A, string V)>(sql))
        {
            if (!map.TryGetValue(row.A, out var set)) map[row.A] = set = [];
            set.Add(row.V);
        }

        return map;
    }

    /// <summary>모아 둔 번호를 앞 8자리 키로 깎는다. 여덟 자리에 못 미치는 것은 버린다.</summary>
    private static Dictionary<string, HashSet<string>> Keys(Dictionary<string, HashSet<string>> numbers)
    {
        var map = new Dictionary<string, HashSet<string>>();

        foreach (var (@base, values) in numbers)
        {
            var keys = values.Select(ClassKey).OfType<string>().ToHashSet();
            if (keys.Count > 0) map[@base] = keys;
        }

        return map;
    }
}

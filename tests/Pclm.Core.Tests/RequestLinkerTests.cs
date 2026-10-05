using Dapper;
using Microsoft.Data.Sqlite;
using Pclm.Core.Domain;
using Pclm.Core.Storage;
using Xunit;

namespace Pclm.Core.Tests;

/// <summary>
/// 접수와 공고를 <b>품목의 수량·단가로</b> 견준다(ADR-021).
///
/// <para>접수서·공고서 어디에도 조달요구번호와 공고번호를 잇는 키가 없다. 그래서 줄 수가 같고
/// (수량, 단가)가 다중집합으로 완전히 같을 때만 막힘 없는 후보로 선다. <b>줄 차례는 다르다</b> —
/// 접수는 요청번호순, 공고는 세부품명·수량순이라 자리끼리 맞추면 틀린다.</para>
///
/// <para>기계는 잇지 않고 추천만 한다(ADR-029) — 잇는 것은 사람의 확정이다.</para>
/// </summary>
public class RequestLinkerTests : IDisposable
{
    private readonly string _path = Path.Combine(
        Path.GetTempPath(), $"pclm-reqlink-{Guid.NewGuid():N}.db");

    private readonly Database _database;
    private readonly Store _store;
    private readonly RequestLinker _linker;

    public RequestLinkerTests()
    {
        _database = PclmFile.Create(_path, PclmRole.Work);
        _store = new Store(_database);
        _linker = new RequestLinker(_database);
    }

    /// <summary>실측의 모양 그대로(값은 지어냈다 — 품대가 여덟 줄의 합이 되게). 접수는 요청번호순, 공고는 세부품명·수량순이라 <b>차례가 다르다</b>.</summary>
    private static readonly (int Quantity, decimal UnitPrice)[] AsReceived =
    [
        (70, 310_200m), (14, 1_874_000m), (12, 1_652_300m), (12, 2_418_000m),
        (12, 2_307_400m), (14, 1_126_700m), (15, 884_600m), (60, 470_000m),
    ];

    private static readonly (int Quantity, decimal UnitPrice)[] AsPosted =
    [
        (70, 310_200m), (60, 470_000m), (15, 884_600m), (14, 1_874_000m),
        (14, 1_126_700m), (12, 1_652_300m), (12, 2_418_000m), (12, 2_307_400m),
    ];

    [Fact]
    public void 차례가_달라도_다중집합이_같으면_막힘_없는_후보로_선다()
    {
        StoreRequest("MPKPLA26910286", AsReceived);
        StoreNotice("R26BK09018047", AsPosted);

        var candidate = Assert.Single(_linker.Candidates());
        Assert.Equal("MPKPLA26910286-000", candidate.Request.Display);
        Assert.Equal("R26BK09018047-000", candidate.Notice.Display);
        Assert.Null(candidate.Blocker);
        Assert.Contains("품목 8줄 · 수량·단가 전부 일치", candidate.Reason);

        // 추천만 한다 — 판정을 통과한 유일 후보여도 기계는 잇지 않는다(ADR-029).
        Assert.Empty(Links());
    }

    /// <summary>줄 수가 블로킹 키다. 다르면 <b>후보에도 오르지 않는다</b>.</summary>
    [Fact]
    public void 줄_수가_다르면_후보에도_오르지_않는다()
    {
        StoreRequest("MPKPLA26910286", AsReceived);
        StoreNotice("R26BK09018047", [.. AsPosted.Take(7)]);

        var row = Assert.Single(_linker.Work().Requests);
        Assert.Equal(0, row.CandidateCount);
        Assert.Null(row.Notice);
    }

    /// <summary>단가 하나만 달라도 막힌다. 이것은 채점이 아니라 <b>잇는 키</b>라 느슨할 여지가 없다.</summary>
    [Fact]
    public void 단가_하나만_달라도_막힌다()
    {
        StoreRequest("MPKPLA26910286", AsReceived);

        var 어긋난 = AsPosted.ToArray();
        어긋난[3] = (14, 1_874_001m);
        StoreNotice("R26BK09018047", 어긋난);

        // 줄 수는 같으니 후보로는 오른다 — 사람이 무엇이 다른지 보고 판단할 자리다.
        var candidate = Assert.Single(_linker.Work().Candidates);
        Assert.Equal("수량·단가가 다릅니다", candidate.Blocker);

        var facet = Assert.Single(candidate.Compared, f => f.Name == "수량·단가");
        Assert.False(facet.Agrees);
    }

    /// <summary>통과 후보가 둘이면 둘 다 <b>「여럿입니다」</b>를 단다 — 수량·단가로는 가를 수 없다.</summary>
    [Fact]
    public void 통과_후보가_둘이면_둘_다_걸리는_점을_단다()
    {
        StoreRequest("MPKPLA26910286", AsReceived);
        StoreNotice("R26BK09018047", AsPosted);
        StoreNotice("R26BK09019999", AsPosted);

        var row = Assert.Single(_linker.Work().Requests);
        Assert.Equal(2, row.CandidateCount);
        Assert.Null(row.Notice);

        Assert.All(_linker.Work().Candidates,
            c => Assert.Equal("수량·단가가 모두 맞는 공고가 2건입니다", c.Blocker));
    }

    /// <summary>
    /// 블로킹 키는 줄 수이지만 <b>유일성은 통과한 후보 사이에서</b> 센다.
    ///
    /// <para>품목 두 줄짜리 공고가 여럿인 것은 흔한 일이라, 줄 수만으로 세면 수량·단가가 맞는
    /// 공고가 하나뿐인데도 사람 큐로 밀려난다 — 실측에서 그렇게 걸린 것이 셋이었다.</para>
    /// </summary>
    [Fact]
    public void 줄_수가_같은_공고가_여럿이어도_수량단가가_맞는_것이_하나면_막힘_없이_선다()
    {
        StoreRequest("MPKPLA26910286", [(12, 1_652_300m), (14, 1_126_700m)]);
        StoreNotice("R26BK09018047", [(14, 1_126_700m), (12, 1_652_300m)]);
        StoreNotice("R26BK09018048", [(3, 400_000m), (7, 55_000m)]);
        StoreNotice("R26BK09018049", [(12, 1_652_301m), (14, 1_126_700m)]);

        var 맞는것 = Assert.Single(_linker.Work().Candidates, c => c.Blocker is null);
        Assert.Equal("R26BK09018047-000", 맞는것.Notice.Display);

        Confirm("MPKPLA26910286", "R26BK09018047");

        // 이어지고 나면 그 공고는 후보에서 빠지고, 남은 둘은 제 까닭을 그대로 단다.
        var candidates = _linker.Work().Candidates;
        Assert.Equal(2, candidates.Count);
        Assert.All(candidates, c => Assert.Equal("수량·단가가 다릅니다", c.Blocker));
    }

    /// <summary>이어지기 전에도 통과한 후보에만 「여럿입니다」가 붙는다 — 나머지는 제 까닭이 있다.</summary>
    [Fact]
    public void 통과하지_못한_후보는_제_까닭을_그대로_단다()
    {
        StoreRequest("MPKPLA26910286", [(12, 1_652_300m), (14, 1_126_700m)]);
        StoreNotice("R26BK09018047", [(14, 1_126_700m), (12, 1_652_300m)]);
        StoreNotice("R26BK09018048", [(3, 400_000m), (7, 55_000m)]);

        var candidates = _linker.Work().Candidates;

        Assert.Null(Assert.Single(candidates, c => c.Notice.Base == "R26BK09018047").Blocker);
        Assert.Equal("수량·단가가 다릅니다",
            Assert.Single(candidates, c => c.Notice.Base == "R26BK09018048").Blocker);
    }

    /// <summary>
    /// 품목 짝 여덟 쌍이 바르게 남는다. 양쪽을 (수량, 단가, 순번)으로 정렬해 지퍼처럼 짝짓는다 —
    /// 실측 자료에서 접수 1~8 번은 공고 1·4·6·7·8·5·3·2 번이 된다.
    /// </summary>
    [Fact]
    public void 품목_짝_여덟_쌍이_남는다()
    {
        StoreRequest("MPKPLA26910286", AsReceived);
        StoreNotice("R26BK09018047", AsPosted);
        Confirm("MPKPLA26910286", "R26BK09018047");

        Assert.Equal([1, 4, 6, 7, 8, 5, 3, 2], ItemLinks());
    }

    /// <summary>링크를 풀면 품목 짝도 함께 걷힌다 — 그것은 링크의 결과라 홀로 남을 뜻이 없다.</summary>
    [Fact]
    public void 링크를_풀면_품목_짝도_걷힌다()
    {
        StoreRequest("MPKPLA26910286", AsReceived);
        StoreNotice("R26BK09018047", AsPosted);
        Confirm("MPKPLA26910286", "R26BK09018047");

        _linker.Unlink(new EntityRef("request", "MPKPLA26910286", "000"));

        Assert.Empty(ItemLinks());
        Assert.Null(Assert.Single(_linker.Work().Requests).Notice);
    }

    /// <summary>
    /// <b>공고건 하나에 접수 하나다.</b> 이미 접수가 붙은 건에 또 이으면 조용히 두 줄이 되는
    /// 대신 시끄럽게 실패한다(<c>notice_group</c> UNIQUE).
    ///
    /// <para>스키마 V15 에서 <b>뜻이 좁아졌다</b> — 예전에는 공고 하나였고 이제는 건 하나라,
    /// 원공고와 재공고에 접수를 따로 이을 수 없다. 재공고에 별도의 접수서가 오는 사례가
    /// 나오면 그때 다시 판단한다.</para>
    /// </summary>
    [Fact]
    public void 한_공고에_접수_둘은_막힌다()
    {
        StoreRequest("MPKPLA26910286", AsReceived);
        StoreRequest("MPKPLA26920286", AsReceived);
        StoreNotice("R26BK09018047", AsPosted);

        _linker.Confirm(
            new EntityRef("request", "MPKPLA26910286", "000"),
            new EntityRef("notice", "R26BK09018047", "000"), 1.0);

        var ex = Assert.Throws<InvalidOperationException>(() => _linker.Confirm(
            new EntityRef("request", "MPKPLA26920286", "000"),
            new EntityRef("notice", "R26BK09018047", "000"), 1.0));

        Assert.Contains("이미 다른 접수가 이어져 있습니다", ex.Message);
    }

    /// <summary>물리친 짝은 다시 후보로 오르지 않는다 — 그러지 않으면 사람 큐가 줄지 않는다.</summary>
    [Fact]
    public void 물리친_짝은_다시_오르지_않는다()
    {
        StoreRequest("MPKPLA26910286", AsReceived);
        StoreNotice("R26BK09018047", AsPosted);

        _linker.Reject(
            new EntityRef("request", "MPKPLA26910286", "000"),
            new EntityRef("notice", "R26BK09018047", "000"));

        Assert.Empty(_linker.Work().Candidates);
    }

    /// <summary>
    /// 세부품명번호가 어긋나도 <b>막지 않는다</b> — 협의·요청 누락으로 접수와 공고가 다를 수 있다.
    /// 나란히 보이기만 한다.
    /// </summary>
    [Fact]
    public void 세부품명번호가_달라도_막지_않는다()
    {
        StoreRequest("MPKPLA26910286", AsReceived, detailNumber: "2510190301");
        StoreNotice("R26BK09018047", AsPosted, detailNumber: "9999999999");

        Assert.Null(Assert.Single(_linker.Candidates()).Blocker);
    }

    /// <summary>차수가 여럿이어도 <b>최신 것만</b> 견준다. 섞으면 줄이 두 배가 되어 판정이 무너진다.</summary>
    [Fact]
    public void 차수를_섞지_않는다()
    {
        StoreRequest("MPKPLA26910286", AsReceived, seq: "000");
        StoreRequest("MPKPLA26910286", AsReceived, seq: "001");
        StoreNotice("R26BK09018047", AsPosted);

        var candidate = Assert.Single(_linker.Candidates());
        Assert.Equal("MPKPLA26910286-001", candidate.Request.Display);
        Assert.Null(candidate.Blocker);
    }

    /// <summary>접수가 아직 없으면 아무 일도 없다.</summary>
    [Fact]
    public void 빈_DB에서도_돈다()
    {
        Assert.Empty(_linker.Candidates());
        Assert.Empty(_linker.Work().Requests);
    }

    // ── 건 단위로 센다 ───────────────────────────────────

    /// <summary>
    /// <b>이 바뀜의 실질적 값이다.</b> 조립대는 취소 뒤 재채번되어 본번호가 갈렸고, 품목이
    /// 같아 수량·단가가 모두 맞는 공고가 둘로 서 있었다 — 「모두 맞는 공고가 2건입니다」로
    /// 막히던 것이 한 건이 되면서 유일해져 막힘 없는 후보로 선다.
    ///
    /// <para>후보로 오르는 번호도 건 이름(<c>R26BK09011054</c>)이 아니라 <b>현행 공고</b>다.</para>
    /// </summary>
    [Fact]
    public void 재공고_건은_후보가_하나라_막힘_없이_선다()
    {
        조립대();
        StoreRequest("MBKLMH26930006", AsReceived);

        var 후보 = Assert.Single(_linker.Work().Candidates);
        Assert.Equal("R26BK09012082-000", 후보.Notice.Display);
        Assert.Null(후보.Blocker);

        _linker.Confirm(후보.Request, 후보.Notice, 후보.Confidence);

        // 링크가 매달리는 자리는 건이다 — 이름은 가장 이른 본번호다.
        Assert.Equal(["MBKLMH26930006|R26BK09011054"], Links());

        // 품목 짝은 차수의 일이라 <b>현행 공고 한 장</b>을 가리킨다.
        Assert.Equal(["R26BK09012082"], ItemLinkNotices());
    }

    /// <summary>
    /// 사람이 고르는 것은 <b>눈에 보이는 공고번호</b>라 재공고 건에서는 뿌리가 아닌 본번호가
    /// 온다. 그것을 건으로 옮기지 않으면 링크가 없는 이름을 가리켜 <b>외래키가 던진다</b> —
    /// 조립대가 정확히 그 모양이라 이 건은 아예 이을 수 없었다.
    /// </summary>
    [Fact]
    public void 뿌리가_아닌_본번호로_확정해도_건에_매인다()
    {
        조립대();
        StoreRequest("MBKLMH26930006", AsReceived);

        _linker.Confirm(
            new EntityRef("request", "MBKLMH26930006", "000"),
            new EntityRef("notice", "R26BK09012082", "000"), 1.0);

        Assert.Equal(["MBKLMH26930006|R26BK09011054"], Links());

        // 물리치기도 같은 자리를 지난다. 링크가 풀리고, 품목 짝도 함께 걷히고, 거부가 건으로 남는다.
        _linker.Reject(
            new EntityRef("request", "MBKLMH26930006", "000"),
            new EntityRef("notice", "R26BK09012082", "000"));

        Assert.Empty(Links());
        Assert.Empty(ItemLinks());
        Assert.Equal(["MBKLMH26930006|R26BK09011054"], Rejections());
    }

    /// <summary>
    /// <b>물리치는 것도 건이다.</b> 재공고를 아니라고 했는데 원공고가 다시 후보로 오르면
    /// 사람은 같은 판단을 두 번 하게 되고, 그것은 같은 조달 건에 대한 판단이다.
    /// </summary>
    [Fact]
    public void 재공고를_물리치면_원공고도_다시_오르지_않는다()
    {
        조립대();
        StoreRequest("MBKLMH26930006", AsReceived);

        _linker.Reject(
            new EntityRef("request", "MBKLMH26930006", "000"),
            new EntityRef("notice", "R26BK09012082", "000"));

        Assert.Empty(_linker.Work().Candidates);
    }

    /// <summary>
    /// 건이 합쳐져 한 건에 접수가 둘이 되면 하나가 밀려나는데, <b>밀린 쪽의 품목 짝도 함께
    /// 걷혀야 한다</b>. <c>request_item_link</c> 는 <c>request_link</c> 가 아니라
    /// <c>request</c> 를 가리켜 캐스케이드가 데려가지 않으므로, 걷지 않으면 어느 조달요구가
    /// 어느 공고 품목이 되었는지가 <b>링크 없이 떠 있게 된다</b> — 오류는 나지 않는다.
    /// </summary>
    [Fact]
    public void 밀려난_접수는_품목_짝도_함께_걷힌다()
    {
        StoreNotice("R26BK09011054", AsPosted);
        StoreNotice("R26BK09012082", AsPosted);
        StoreRequest("MBKLMH26930006", AsReceived);
        StoreRequest("MBKLMH26930007", AsReceived);

        _linker.Confirm(
            new EntityRef("request", "MBKLMH26930006", "000"),
            new EntityRef("notice", "R26BK09011054", "000"), 1.0);

        _linker.Confirm(
            new EntityRef("request", "MBKLMH26930007", "000"),
            new EntityRef("notice", "R26BK09012082", "000"), 1.0);

        Assert.Equal(8, 품목짝수("MBKLMH26930006"));
        Assert.Equal(8, 품목짝수("MBKLMH26930007"));

        // 한쪽을 기계가 이은 것으로 돌려 둔다 — 밀릴 쪽을 규칙이 정하게 해야 시험이 흔들리지 않는다.
        Execute("UPDATE request_link SET decided_by = 'auto' WHERE request_base = 'MBKLMH26930007';");

        // 뒤늦게 들어온 관련공고 한 줄이 두 건을 하나로 만든다.
        Execute(
            """
            INSERT INTO notice_relation (notice_base, seq, line_no, related)
            VALUES ('R26BK09012082', '000', 1, 'R26BK09011054-000');
            """);

        NoticeGroupChange change;
        using (var connection = _database.Open())
            change = NoticeGroups.Rebuild(connection, transaction: null);

        Assert.Contains("MBKLMH26930007", Assert.Single(change.밀린접수));

        Assert.Equal(["MBKLMH26930006|R26BK09011054"], Links());
        Assert.Equal(8, 품목짝수("MBKLMH26930006"));
        Assert.Equal(0, 품목짝수("MBKLMH26930007"));
    }

    /// <summary>
    /// 직접 찾기 목록은 <b>건마다 한 줄</b>이고 번호는 현행 공고다. 건 이름을 내면 문서가 없는
    /// 번호가 화면에 서는 날이 온다 — 가스성분분석기가 가리키는 <c>R26BK09013019</c> 처럼.
    /// </summary>
    [Fact]
    public void 직접_찾기_목록은_건마다_현행_공고를_낸다()
    {
        조립대();

        var choice = Assert.Single(_linker.Notices());
        Assert.Equal("R26BK09012082-000", choice.Notice.Display);
    }

    // ── 자료 짓기 ────────────────────────────────────────

    /// <summary>
    /// 조립대. 당초 → 취소 → <b>재채번된 재공고</b> 세 장이 한 건이다(실측 코퍼스의 모양이고
    /// 담긴 값은 지어냈다). 현행은 재공고 <c>R26BK09012082-000</c> 다.
    /// </summary>
    private void 조립대()
    {
        StoreNotice("R26BK09011054", AsPosted);
        StoreNotice("R26BK09011054", AsPosted, seq: "001", related: ["R26BK09011054-000"]);
        StoreNotice("R26BK09012082", AsPosted, related: ["R26BK09011054-001", "R26BK09011054-000"]);
    }

    private void Confirm(string requestBase, string noticeBase) =>
        _linker.Confirm(
            new EntityRef("request", requestBase, "000"),
            new EntityRef("notice", noticeBase, "000"), 1.0);

    private List<string> Links() => Read(
        "SELECT request_base || '|' || notice_group FROM request_link ORDER BY 1;");

    private List<string> Rejections() => Read(
        "SELECT request_base || '|' || notice_group FROM request_link_rejection ORDER BY 1;");

    /// <summary>품목 짝이 가리키는 공고 본번호. 건이 아니라 <b>문서 한 장</b>이어야 한다.</summary>
    private List<string> ItemLinkNotices() => Read(
        "SELECT DISTINCT notice_base FROM request_item_link ORDER BY 1;");

    private long 품목짝수(string requestBase)
    {
        using var connection = _database.Open();
        return connection.ExecuteScalar<long>(
            "SELECT COUNT(*) FROM request_item_link WHERE request_base = @r;", new { r = requestBase });
    }

    private List<string> Read(string sql)
    {
        using var connection = _database.Open();
        return [.. connection.Query<string>(sql)];
    }

    private void Execute(string sql)
    {
        using var connection = _database.Open();
        using var command = connection.CreateCommand();
        command.CommandText = sql;
        command.ExecuteNonQuery();
    }

    private List<long> ItemLinks()
    {
        using var connection = _database.Open();
        return [.. connection.Query<long>(
            "SELECT notice_line_no FROM request_item_link ORDER BY line_no;")];
    }

    private void StoreRequest(
        string @base, (int Quantity, decimal UnitPrice)[] items,
        string seq = "000", string detailNumber = "2510190301") =>
        _store.UpsertRequest(new RequestRecord
        {
            RequestBase = @base,
            Seq = seq,
            Title = "26년 한별 윈치 8종 구매",
            ReceivedOn = new DateTime(2026, 6, 9),
            GoodsAmount = 181_725_200m,
            BudgetAmount = 183_408_620m,
            DemandAgency = "한별군수지원단",
            Items = [.. items.Select((p, i) => new RequestItemRecord
            {
                LineNo = i + 1,
                RequestNumber = $"{@base}-{i}",
                ItemName = "전동식윈치",
                DetailItemNumber = detailNumber,
                Quantity = p.Quantity,
                UnitPrice = p.UnitPrice,
            })],
        });

    private void StoreNotice(
        string @base, (int Quantity, decimal UnitPrice)[] items,
        string detailNumber = "2510190301", string seq = "000", string[]? related = null) =>
        _store.UpsertNotice(new NoticeRecord
        {
            NoticeBase = @base,
            Seq = seq,
            Title = "26년 한별 윈치 8종 구매",
            PostedAt = new DateTime(2026, 7, 18),
            RelatedNotices = related ?? [],
            ProjectAmount = 181_725_200m,
            AllocatedBudget = 183_408_620m,
            Items = [.. items.Select((p, i) => new NoticeItemRecord
            {
                LineNo = i + 1,
                ItemName = "전동식윈치",
                DetailItemNumber = detailNumber,
                DemandAgency = "한별군수지원단",
                Quantity = p.Quantity,
                UnitPrice = p.UnitPrice,
            })],
        });

    public void Dispose()
    {
        SqliteConnection.ClearAllPools();
        foreach (var suffix in new[] { "", "-wal", "-shm" })
            File.Delete(_path + suffix);
        GC.SuppressFinalize(this);
    }
}

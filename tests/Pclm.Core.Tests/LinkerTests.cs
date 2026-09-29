using Microsoft.Data.Sqlite;
using Pclm.Core.Domain;
using Pclm.Core.Storage;
using Xunit;

namespace Pclm.Core.Tests;

/// <summary>
/// 자동 확정이 <b>확신할 수 있을 때만</b> 걸리는지 본다.
///
/// <para>이 규칙의 값어치는 많이 잇는 데 있지 않고 <b>위험한 쪽으로 실패하지 않는</b> 데 있다.
/// 애매하면 잇지 않고 사람에게 넘겨야 한다 — 조용히 잘못 이어진 링크 하나가 축적 자산 전체의
/// 신뢰를 무너뜨리기 때문이다. 그래서 여기 시험 대부분은 "이어지는지" 가 아니라
/// <b>"안 이어지는지"</b> 를 지킨다(ADR-015).</para>
/// </summary>
public class LinkerTests : IDisposable
{
    private readonly string _path = Path.Combine(
        Path.GetTempPath(), $"pclm-linker-{Guid.NewGuid():N}.db");

    private readonly Database _database;
    private readonly Store _store;
    private readonly Linker _linker;

    private static readonly DateTime Posted = new(2026, 7, 2, 16, 59, 28);
    private static readonly DateTime Signed = new(2026, 7, 22);

    public LinkerTests()
    {
        _database = new Database(_path);
        _database.Migrate();
        _store = new Store(_database);
        _linker = new Linker(_database);
    }

    [Fact]
    public void 건명이_같고_부수확인을_통과하면_자동으로_이어진다()
    {
        Put(Notice("R26BK09017075", "2026년 가람 수질측정기 조달"));
        Put(Contract("R26TA09110507", "2026년 가람 수질측정기 조달"));

        var result = _linker.AutoConfirm();

        Assert.Equal(1, result.Count);
        Assert.Equal("R26BK09017075", Link("R26TA09110507")["notice_group"]);
        Assert.Equal("auto", Link("R26TA09110507")["decided_by"]);
        Assert.Equal($"{Linker.RuleVersion}", Link("R26TA09110507")["rule_version"]);
        Assert.Contains("세부품명 일치", Link("R26TA09110507")["evidence"]);
    }

    /// <summary>건명은 공백만 지우고 대조하므로 띄어쓰기가 흔들려도 이어져야 한다.</summary>
    [Fact]
    public void 띄어쓰기가_달라도_이어진다()
    {
        Put(Notice("R26BK09017075", "2026년 가람 수질측정기 조달"));
        Put(Contract("R26TA09110507", "2026년가람 수질측정기조달"));

        Assert.Equal(1, _linker.AutoConfirm().Count);
    }

    /// <summary>재공고·반복구매. 이름만으로는 가릴 수 없으니 사람에게 넘어가야 한다.</summary>
    [Fact]
    public void 같은_이름_공고가_둘이면_자동으로_잇지_않는다()
    {
        Put(Notice("R26BK09017075", "26년 가람 절단기 구매"));
        Put(Notice("R26BK09017030", "26년 가람 절단기 구매"));
        Put(Contract("R26TA09110507", "26년 가람 절단기 구매"));

        Assert.Equal(0, _linker.AutoConfirm().Count);

        var candidates = _linker.Candidates();
        Assert.Equal(2, candidates.Count);
        Assert.All(candidates, c => Assert.True(c.TitleMatched));
        Assert.All(candidates, c => Assert.Contains("같은 이름 공고가 2건", c.Blocker));
    }

    [Fact]
    public void 세부품명이_다르면_자동으로_잇지_않는다()
    {
        Put(Notice("R26BK09017075", "2026년 가람 수질측정기 조달", itemName: "플라즈마절단기"));
        Put(Contract("R26TA09110507", "2026년 가람 수질측정기 조달", itemName: "수질검사장치"));

        Assert.Equal(0, _linker.AutoConfirm().Count);
        Assert.Contains("세부품명이 다릅니다", _linker.Candidates()[0].Blocker);
    }

    /// <summary>
    /// 같은 물건인데 이름이 갈린다. 계약서는 물품분류번호(8자리)에 딸린 이름을, 공고서는
    /// 세부품명번호(10자리)에 딸린 이름을 적어서 <b>「유독또는가스탐지기」와 「가스탐지기」</b>
    /// 가 된다 — 이름만 보면 멀쩡한 짝이 통째로 막힌다.
    /// </summary>
    [Fact]
    public void 세부품명이_달라도_물품분류번호_앞_8자리가_같으면_이어진다()
    {
        Put(Notice("R26BK09017075", "26년 가람 가스성분분석기 구매",
            itemName: "가스탐지기", detailNumber: "4715190501"));

        Put(Contract("R26TA09110507", "26년 가람 가스성분분석기 구매",
            itemName: "유독또는가스탐지기", classNumber: "47151905"));

        Assert.Equal(1, _linker.AutoConfirm().Count);

        var evidence = Link("R26TA09110507")["evidence"];
        Assert.Contains("물품분류번호 앞 8자리 일치 (47151905)", evidence);
        Assert.DoesNotContain("세부품명 일치", evidence);
    }

    /// <summary>번호를 대안으로 둔 것이지 이름 대조를 푼 것이 아니다 — 둘 다 어긋나면 막힌다.</summary>
    [Fact]
    public void 세부품명도_분류번호도_다르면_잇지_않는다()
    {
        Put(Notice("R26BK09017075", "26년 가람 가스성분분석기 구매",
            itemName: "가스탐지기", detailNumber: "4715190501"));

        Put(Contract("R26TA09110507", "26년 가람 가스성분분석기 구매",
            itemName: "플라즈마절단기", classNumber: "27112501"));

        Assert.Equal(0, _linker.AutoConfirm().Count);
        Assert.Contains("세부품명이 다릅니다", _linker.Candidates()[0].Blocker);
    }

    /// <summary>보이는 것은 문서에 찍힌 번호 그대로이고, 맞는지는 앞 8자리로 본다.</summary>
    [Fact]
    public void 견주기에_물품분류번호가_선다()
    {
        Put(Notice("R26BK09017075", "26년 가람 가스성분분석기 구매",
            itemName: "가스탐지기", detailNumber: "4715190501"));

        Put(Contract("R26TA09110507", "26년 가람 가스성분분석기 구매",
            itemName: "유독또는가스탐지기", classNumber: "47151905"));

        var facets = _linker.Compare(
            new EntityRef("contract", "R26TA09110507", "00"),
            new EntityRef("notice", "R26BK09017075", "000"));

        var facet = Assert.Single(facets, f => f.Name == "물품분류번호");

        Assert.Equal("47151905", facet.Contract);
        Assert.Equal("4715190501", facet.Notice);
        Assert.True(facet.Agrees);
    }

    [Fact]
    public void 수요기관이_다르면_자동으로_잇지_않는다()
    {
        Put(Notice("R26BK09017075", "2026년 가람 수질측정기 조달", agency: "한별군수지원단"));
        Put(Contract("R26TA09110507", "2026년 가람 수질측정기 조달", agency: "가람군수지원단"));

        Assert.Equal(0, _linker.AutoConfirm().Count);
        Assert.Contains("수요기관이 다릅니다", _linker.Candidates()[0].Blocker);
    }

    /// <summary>계약은 언제나 공고 뒤에 온다. 어긋나면 물리적으로 불가능한 짝이다.</summary>
    [Fact]
    public void 계약일이_게시일보다_앞서면_자동으로_잇지_않는다()
    {
        Put(Notice("R26BK09017075", "2026년 가람 수질측정기 조달"));
        Put(Contract("R26TA09110507", "2026년 가람 수질측정기 조달", signed: Posted.AddDays(-1)));

        Assert.Equal(0, _linker.AutoConfirm().Count);
        Assert.Contains("보다 앞섭니다", _linker.Candidates()[0].Blocker);
    }

    /// <summary>
    /// 초안 계약서에는 계약일자가 아예 비어 있다. <b>없는 값은 위반이 아니라 판정 불가</b>라,
    /// 그 확인만 건너뛰고 나머지로 판단해야 한다 — 없는 것을 어긋난 것으로 치면 멀쩡한 짝이 탈락한다.
    /// </summary>
    [Fact]
    public void 계약일이_없으면_그_확인만_건너뛴다()
    {
        Put(Notice("R26BK09017075", "26년 한별 공압식승강판 20톤 구매"));
        Put(Contract("R26TA09100327", "26년 한별 공압식승강판 20톤 구매") with { ContractedOn = null });

        Assert.Equal(1, _linker.AutoConfirm().Count);
        Assert.DoesNotContain("계약일", Link("R26TA09100327")["evidence"]);
    }

    [Fact]
    public void 거부한_짝은_다시_후보에_오르지_않는다()
    {
        Put(Notice("R26BK09017075", "26년 가람 절단기 구매"));
        Put(Notice("R26BK09017030", "26년 가람 절단기 구매"));
        Put(Contract("R26TA09110507", "26년 가람 절단기 구매"));

        _linker.Reject(
            new EntityRef("contract", "R26TA09110507", "00"),
            new EntityRef("notice", "R26BK09017030", "000"));

        // 물리치고 나면 남은 후보가 하나뿐이라 이제 확신할 수 있다.
        Assert.Equal(1, _linker.AutoConfirm().Count);
        Assert.Equal("R26BK09017075", Link("R26TA09110507")["notice_group"]);
    }

    /// <summary>아니라고 판정한 짝이 이미 이어져 있었다면 함께 풀려야 한다.</summary>
    [Fact]
    public void 거부하면_그_짝의_링크도_함께_풀린다()
    {
        Put(Notice("R26BK09017075", "2026년 가람 수질측정기 조달"));
        Put(Contract("R26TA09110507", "2026년 가람 수질측정기 조달"));
        _linker.AutoConfirm();

        _linker.Reject(
            new EntityRef("contract", "R26TA09110507", "00"),
            new EntityRef("notice", "R26BK09017075", "000"));

        Assert.Empty(Links());
        Assert.Equal(0, _linker.AutoConfirm().Count);
    }

    [Fact]
    public void 재평가는_사람이_확정한_링크를_건드리지_않는다()
    {
        Put(Notice("R26BK09017075", "2026년 가람 수질측정기 조달"));

        // 건명이 전혀 다른 계약. 규칙으로는 절대 이어지지 않으므로 사람이 확정한 것임이 분명하다.
        Put(Contract("R26TA09110507", "손으로 이은 계약"));
        _linker.Confirm(
            new EntityRef("contract", "R26TA09110507", "00"),
            new EntityRef("notice", "R26BK09017075", "000"), 1.0);

        var result = _linker.Reevaluate();

        Assert.Equal(0, result.Released);
        Assert.Equal("human", Link("R26TA09110507")["decided_by"]);
        Assert.Equal("R26BK09017075", Link("R26TA09110507")["notice_group"]);
    }

    [Fact]
    public void 재평가는_기계가_이은_것만_풀고_다시_판정한다()
    {
        Put(Notice("R26BK09017075", "2026년 가람 수질측정기 조달"));
        Put(Contract("R26TA09110507", "2026년 가람 수질측정기 조달"));
        _linker.AutoConfirm();

        // 규칙판을 지난 것으로 돌려 놓으면 재평가가 그 링크를 다시 본다.
        Execute("UPDATE project_link SET rule_version = 0 WHERE decided_by = 'auto';");

        var result = _linker.Reevaluate();

        Assert.Equal(1, result.Released);
        Assert.Equal(1, result.Count);
        Assert.Equal($"{Linker.RuleVersion}", Link("R26TA09110507")["rule_version"]);
    }

    /// <summary>공고 하나에 계약이 여럿 달릴 수 있다. 계약 쪽에서 보면 후보는 여전히 하나다.</summary>
    [Fact]
    public void 분할낙찰이면_계약_셋이_모두_같은_공고에_이어진다()
    {
        Put(Notice("R26BK09017075", "2026년 가람 수질측정기 조달"));
        Put(Contract("R26TA09110501", "2026년 가람 수질측정기 조달"));
        Put(Contract("R26TA09110502", "2026년 가람 수질측정기 조달"));
        Put(Contract("R26TA09110503", "2026년 가람 수질측정기 조달"));

        Assert.Equal(3, _linker.AutoConfirm().Count);
        Assert.All(Links(), row => Assert.Equal("R26BK09017075", row));
    }

    [Fact]
    public void 이미_이어진_계약은_다시_판정하지_않는다()
    {
        Put(Notice("R26BK09017075", "2026년 가람 수질측정기 조달"));
        Put(Contract("R26TA09110507", "2026년 가람 수질측정기 조달"));

        Assert.Equal(1, _linker.AutoConfirm().Count);
        Assert.Equal(0, _linker.AutoConfirm().Count);
    }

    [Fact]
    public void 링크를_풀면_다시_이을_수_있다()
    {
        Put(Notice("R26BK09017075", "2026년 가람 수질측정기 조달"));
        Put(Contract("R26TA09110507", "2026년 가람 수질측정기 조달"));
        _linker.AutoConfirm();

        _linker.Unlink(new EntityRef("contract", "R26TA09110507", "00"));

        Assert.Empty(Links());
        Assert.Equal(1, _linker.AutoConfirm().Count);
    }

    /// <summary>
    /// 정규화를 더 하고 싶은 유혹을 막는 자리다. 계약유형 낱말을 떼면 서로 다른 조달 건이
    /// 한 키로 뭉개져 유일성 판정이 망가진다.
    /// </summary>
    [Fact]
    public void TitleKey_는_공백만_지운다()
    {
        Assert.Equal("26년가람절단기구매", Linker.TitleKey("26년 가람 절단기 구매"));
        Assert.Equal("26년가람절단기구매", Linker.TitleKey(" 26년가람  절단기\t구매 "));

        Assert.NotEqual(Linker.TitleKey("26년 가람 절단기 조달"), Linker.TitleKey("26년 가람 절단기 구매"));
        Assert.Equal(string.Empty, Linker.TitleKey(null));
    }

    /// <summary>건명이 맞는 공고가 없으면 약한 근거로 후보를 내되 자동 확정 대상은 아니다.</summary>
    [Fact]
    public void 건명이_다르면_약한_후보로만_올라온다()
    {
        Put(Notice("R26BK09017075", "수질측정기 구매", itemId: "29478001"));
        Put(Contract("R26TA09110507", "전혀 다른 이름의 계약", itemId: "29478001"));

        Assert.Equal(0, _linker.AutoConfirm().Count);

        var candidates = _linker.Candidates();
        Assert.Single(candidates);
        Assert.False(candidates[0].TitleMatched);
        Assert.Contains("물품식별번호 일치", candidates[0].Reason);
    }

    /// <summary>
    /// 이어진 계약이 잇기 화면 목록에서 사라지면 <b>잘못 이어진 것을 끊을 자리가 없다.</b>
    /// 끊기는 그 줄을 골라야 나오는 동작이라, 줄이 서 있지 않으면 창에서는 되돌릴 길이 없다.
    /// </summary>
    [Fact]
    public void 이어진_계약도_목록에_남고_이어진_공고를_함께_낸다()
    {
        Put(Notice("R26BK09017075", "2026년 가람 수질측정기 조달"));
        Put(Contract("R26TA09110507", "2026년 가람 수질측정기 조달"));
        _linker.AutoConfirm();

        var row = Assert.Single(_linker.Work().Contracts);

        Assert.Equal("R26TA09110507", row.Contract.Base);
        Assert.Equal("R26BK09017075", row.Notice?.Base);
        Assert.Equal("2026년 가람 수질측정기 조달", row.NoticeTitle);
        Assert.Equal("auto", row.DecidedBy);
    }

    /// <summary>다시 이을 길. 지금 이어진 공고는 후보에서 빠진다 — 화면이 따로 그린다.</summary>
    [Fact]
    public void 이어진_계약은_건명이_같은_다른_공고를_후보로_낸다()
    {
        Put(Notice("R26BK09017075", "26년 가람 절단기 구매"));
        Put(Notice("R26BK09017030", "26년 가람 절단기 구매"));
        Put(Contract("R26TA09110507", "26년 가람 절단기 구매"));

        _linker.Confirm(
            new EntityRef("contract", "R26TA09110507", "00"),
            new EntityRef("notice", "R26BK09017030", "000"), 1.0);

        var work = _linker.Work();
        var candidate = Assert.Single(work.Candidates);

        Assert.Equal("R26BK09017075", candidate.Notice.Base);
        Assert.Equal(1, work.Contracts[0].CandidateCount);
    }

    /// <summary>
    /// 갈아탈 후보는 창에만 있다. 명령줄의 큐는 <b>사람이 아직 결정하지 않은 것</b>이라,
    /// 이어진 계약의 대안이 섞이면 그 뜻이 흔들리고 큐가 줄지 않는다.
    /// </summary>
    [Fact]
    public void 이어진_계약의_후보는_pclm_link_목록에_오르지_않는다()
    {
        Put(Notice("R26BK09017075", "26년 가람 절단기 구매"));
        Put(Notice("R26BK09017030", "26년 가람 절단기 구매"));
        Put(Contract("R26TA09110507", "26년 가람 절단기 구매"));

        _linker.Confirm(
            new EntityRef("contract", "R26TA09110507", "00"),
            new EntityRef("notice", "R26BK09017030", "000"), 1.0);

        Assert.Single(_linker.Work().Candidates);
        Assert.Empty(_linker.Candidates());
    }

    /// <summary>추천 밖에서 고른 짝도 근거 없이 잇게 두지 않는다 — 후보 카드와 같은 표를 낸다.</summary>
    [Fact]
    public void 추천에_없는_공고와도_나란히_견줄_수_있다()
    {
        Put(Notice("R26BK09017075", "수질측정기 구매", itemName: "수질검사장치"));
        Put(Contract("R26TA09110507", "전혀 다른 이름의 계약", itemName: "수질검사장치"));

        var facets = _linker.Compare(
            new EntityRef("contract", "R26TA09110507", "00"),
            new EntityRef("notice", "R26BK09017075", "000"));

        Assert.Equal(
            ["건명", "세부품명", "물품분류번호", "물품식별번호", "수요기관", "계약일 / 게시일"],
            facets.Select(f => f.Name));
        Assert.False(facets[0].Agrees);
        Assert.True(facets[1].Agrees);

        Assert.Throws<InvalidOperationException>(() => _linker.Compare(
            new EntityRef("contract", "R26TA09999999", "00"),
            new EntityRef("notice", "R26BK09017075", "000")));
    }

    [Fact]
    public void Notices_는_공고를_최신_차수로_모두_낸다()
    {
        Put(Notice("R26BK09017075", "26년 가람 절단기 구매"));
        Put(Notice("R26BK09017075", "26년 가람 절단기 구매(정정)") with { Seq = "001" });
        Put(Notice("R26BK09017030", "26년 한별 소나 부품 구매"));
        Put(Contract("R26TA09110507", "26년 한별 소나 부품 구매"));
        _linker.AutoConfirm();

        var notices = _linker.Notices();

        Assert.Equal(2, notices.Count);
        Assert.Equal(["R26BK09017030-000", "R26BK09017075-001"], notices.Select(n => n.Notice.Display));
        Assert.Equal("26년 가람 절단기 구매(정정)", notices[1].Title);
        Assert.Equal("2026-07-02", notices[1].PostedAt);

        // 한 공고에 계약이 여럿일 수 있으니 이미 붙은 수를 보여만 주고 막지는 않는다.
        Assert.Equal(1, notices[0].Linked);
        Assert.Equal(0, notices[1].Linked);
    }

    // ── 건 단위로 센다 ───────────────────────────────────

    /// <summary>
    /// <b>이 바뀜의 실질적 값이다.</b> 조립대는 취소 뒤 재채번되어 본번호가 갈렸고, 건명도
    /// 품목도 같아 이름이 같은 공고가 둘로 서 있었다 — 「같은 이름 공고가 2건」으로 막혀
    /// 사람 큐로 밀리던 것이 한 건이 되면서 유일해져 기계가 스스로 잇는다.
    ///
    /// <para>후보로 오르는 번호도 건 이름(<c>R26BK09011054</c>)이 아니라 <b>현행 공고</b>다.</para>
    /// </summary>
    [Fact]
    public void 재공고_건은_후보가_하나라_자동으로_이어진다()
    {
        조립대();
        Put(Contract("R26TA09080469", "26년 한별 조립대 구매"));

        var 후보 = Assert.Single(_linker.Work().Candidates);
        Assert.Equal("R26BK09012082-000", 후보.Notice.Display);
        Assert.Null(후보.Blocker);

        var 이음 = Assert.Single(_linker.AutoConfirm().Linked);
        Assert.Equal("R26BK09012082-000", 이음.Notice.Display);

        // 링크가 매달리는 자리는 건이다 — 이름은 가장 이른 본번호다.
        Assert.Equal("R26BK09011054", Link("R26TA09080469")["notice_group"]);
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
        Put(Contract("R26TA09080469", "손으로 이은 계약"));

        _linker.Confirm(
            new EntityRef("contract", "R26TA09080469", "00"),
            new EntityRef("notice", "R26BK09012082", "000"), 1.0);

        Assert.Equal("R26BK09011054", Link("R26TA09080469")["notice_group"]);

        // 물리치기도 같은 자리를 지난다. 링크가 풀리고 거부가 건으로 남는다.
        _linker.Reject(
            new EntityRef("contract", "R26TA09080469", "00"),
            new EntityRef("notice", "R26BK09012082", "000"));

        Assert.Empty(Links());
        Assert.Equal(["R26TA09080469|R26BK09011054"], Rejections());
    }

    /// <summary>
    /// <b>물리치는 것도 건이다.</b> 재공고를 아니라고 했는데 원공고가 다시 후보로 오르면
    /// 사람은 같은 판단을 두 번 하게 되고, 그것은 같은 조달 건에 대한 판단이다.
    /// </summary>
    [Fact]
    public void 재공고를_물리치면_원공고도_다시_오르지_않는다()
    {
        조립대();
        Put(Contract("R26TA09080469", "26년 한별 조립대 구매"));

        _linker.Reject(
            new EntityRef("contract", "R26TA09080469", "00"),
            new EntityRef("notice", "R26BK09012082", "000"));

        Assert.Equal(0, _linker.AutoConfirm().Count);
        Assert.Empty(_linker.Candidates());
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

    // ── 준비 ─────────────────────────────────────────────

    /// <summary>
    /// 조립대. 당초 → 취소 → <b>재채번된 재공고</b> 세 장이 한 건이다(실측 코퍼스의 모양이고
    /// 담긴 값은 지어냈다). 현행은 재공고 <c>R26BK09012082-000</c> 다.
    /// </summary>
    private void 조립대()
    {
        Put(Notice("R26BK09011054", "26년 한별 조립대 구매"));
        Put(Notice("R26BK09011054", "26년 한별 조립대 구매",
            seq: "001", kind: "취소공고", related: ["R26BK09011054-000"]));
        Put(Notice("R26BK09012082", "26년 한별 조립대 구매",
            related: ["R26BK09011054-001", "R26BK09011054-000"]));
    }

    private void Put(NoticeRecord notice) => _store.UpsertNotice(notice);

    private void Put(ContractRecord contract) => _store.UpsertContract(contract);

    private static NoticeRecord Notice(
        string @base, string title,
        string itemName = "수질검사장치", string itemId = "29478001",
        string agency = "가람군수지원단", string? detailNumber = null,
        string seq = "000", string? kind = null, string[]? related = null) => new()
        {
            NoticeBase = @base,
            Seq = seq,
            Title = title,
            NoticeKind = kind,
            PostedAt = Posted,
            RelatedNotices = related ?? [],
            Items = [new NoticeItemRecord
            {
                LineNo = 1, ItemName = itemName, ItemIdNumber = itemId, DemandAgency = agency,
                DetailItemNumber = detailNumber,
            }],
        };

    private static ContractRecord Contract(
        string @base, string title,
        string itemName = "수질검사장치", string itemId = "29478001",
        string? agency = "가람군수지원단", DateTime? signed = null,
        string? classNumber = null) => new()
        {
            ContractBase = @base,
            Seq = "00",
            Title = title,
            Amount = 164_872_340m,
            DemandAgency = agency,
            ContractedOn = signed ?? Signed,
            Items = [new ContractItemRecord
            {
                LineNo = 1, ItemName = itemName, ItemIdNumber = itemId,
                ClassificationNumber = classNumber,
            }],
        };

    private Dictionary<string, string> Link(string contractBase)
    {
        using var connection = _database.OpenReadOnly();
        using var command = connection.CreateCommand();
        command.CommandText =
            $"SELECT * FROM project_link WHERE contract_base = '{contractBase}';";

        using var reader = command.ExecuteReader();
        Assert.True(reader.Read(), $"링크가 없습니다: {contractBase}");

        var row = new Dictionary<string, string>();
        for (var i = 0; i < reader.FieldCount; i++)
            row[reader.GetName(i)] = reader.IsDBNull(i) ? "" : reader.GetValue(i).ToString() ?? "";

        return row;
    }

    private List<string> Links()
    {
        using var connection = _database.OpenReadOnly();
        using var command = connection.CreateCommand();
        command.CommandText = "SELECT notice_group FROM project_link ORDER BY contract_base;";

        var rows = new List<string>();
        using var reader = command.ExecuteReader();
        while (reader.Read()) rows.Add(reader.GetString(0));
        return rows;
    }

    private List<string> Rejections()
    {
        using var connection = _database.OpenReadOnly();
        using var command = connection.CreateCommand();
        command.CommandText =
            "SELECT contract_base || '|' || notice_group FROM link_rejection ORDER BY 1;";

        var rows = new List<string>();
        using var reader = command.ExecuteReader();
        while (reader.Read()) rows.Add(reader.GetString(0));
        return rows;
    }

    private void Execute(string sql)
    {
        using var connection = _database.Open();
        using var command = connection.CreateCommand();
        command.CommandText = sql;
        command.ExecuteNonQuery();
    }

    public void Dispose()
    {
        SqliteConnection.ClearAllPools();
        foreach (var suffix in new[] { "", "-wal", "-shm" })
            File.Delete(_path + suffix);
        GC.SuppressFinalize(this);
    }
}

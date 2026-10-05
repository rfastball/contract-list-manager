using Microsoft.Data.Sqlite;
using Pclm.Core.Domain;
using Pclm.Core.Storage;
using Xunit;

namespace Pclm.Core.Tests;

/// <summary>
/// 쌓인 것을 <b>생긴 모양대로</b> 내는지 본다 — 접수 → 공고 → 계약 사슬, 계약 아래 명세.
///
/// <para>평평한 표로 펴면 공고 값이 계약 줄마다, 계약 값이 명세 줄마다 되풀이된다.
/// 그 되풀이가 <b>자료가 겹쳐 들어간 것처럼</b> 보여서, 실제로는 업서트가 멀쩡한데도
/// 중복을 의심하게 만들었다. 여기서 지키는 것은 <b>되풀이가 없다</b>는 사실이다.</para>
///
/// <para>사슬은 <b>셋 중 어느 것이 없어도 선다</b>. 접수만 들어온 것·공고만 들어온 것·
/// 어디에도 매달리지 못한 계약이 모두 제 자리에 있어야, 넣은 사람이 빠진 것을 알아챈다.</para>
/// </summary>
public class OutlineTests : IDisposable
{
    private readonly string _path = Path.Combine(
        Path.GetTempPath(), $"pclm-outline-{Guid.NewGuid():N}.db");

    private readonly Database _database;
    private readonly Store _store;

    public OutlineTests()
    {
        _database = PclmFile.Create(_path, PclmRole.Work);
        _store = new Store(_database);
    }

    [Fact]
    public void 공고_아래_계약_아래_명세로_달린다()
    {
        _store.UpsertNotice(Notice("000"));
        _store.UpsertContract(Contract("00", 두줄: true));
        Link();

        var chain = Assert.Single(new Outline(_database).Build().Chains);

        Assert.Null(chain.Request);
        Assert.Equal("R26BK09017075-000", chain.Notice!.Current!.Number);

        var contract = Assert.Single(chain.Contracts);
        Assert.Equal("R26TA0911050700", contract.Number);
        Assert.Equal(2, contract.Items.Count);
    }

    /// <summary>명세가 두 줄이어도 계약은 한 번만 선다. 표에서 계약번호가 두 번 찍히던 자리다.</summary>
    [Fact]
    public void 명세가_여러_줄이어도_계약은_한_번만_선다()
    {
        _store.UpsertContract(Contract("00", 두줄: true));

        var chain = Assert.Single(new Outline(_database).Build().Chains);
        Assert.Equal(2, Assert.Single(chain.Contracts).Items.Count);
    }

    /// <summary>
    /// 공고가 아직 없는 계약. <b>제 사슬로 선다</b> — 옛 <c>loose</c> 자리를 사슬이 대신한다.
    /// </summary>
    [Fact]
    public void 이어지지_않은_계약도_제_사슬로_선다()
    {
        _store.UpsertNotice(Notice("000"));
        _store.UpsertContract(Contract("00"));

        var chains = new Outline(_database).Build().Chains;
        Assert.Equal(2, chains.Count);

        var 공고사슬 = Assert.Single(chains, c => c.Notice is not null);
        Assert.Empty(공고사슬.Contracts);

        var 홀로 = Assert.Single(chains, c => c.Notice is null);
        Assert.Null(홀로.Request);
        Assert.Equal("R26TA0911050700", Assert.Single(홀로.Contracts).Number);
    }

    /// <summary>접수 → 공고 → 계약이 한 사슬로 선다. 이것이 이 자료의 <b>기본 모양</b>이다.</summary>
    [Fact]
    public void 접수와_공고와_계약이_한_사슬로_선다()
    {
        _store.UpsertRequest(Request("000"));
        _store.UpsertNotice(Notice("000"));
        _store.UpsertContract(Contract("00"));
        Link();
        LinkRequest();

        var chain = Assert.Single(new Outline(_database).Build().Chains);

        Assert.Equal("MPKPLA26910286-000", chain.Request!.Number);
        Assert.Equal("R26BK09017075-000", chain.Notice!.Current!.Number);
        Assert.Equal("R26TA0911050700", Assert.Single(chain.Contracts).Number);

        // 사슬의 이름은 있는 것의 본번호를 이어 만든다.
        Assert.Equal("MPKPLA26910286/R26BK09017075", chain.Key);
    }

    /// <summary>접수만 들어온 것도 제 사슬로 선다. 안 보이면 넣은 사람이 넣지 않은 줄 안다.</summary>
    [Fact]
    public void 공고가_없는_접수도_제_사슬로_선다()
    {
        _store.UpsertRequest(Request("000"));

        var chain = Assert.Single(new Outline(_database).Build().Chains);

        Assert.Equal("MPKPLA26910286-000", chain.Request!.Number);
        Assert.Null(chain.Notice);
        Assert.Empty(chain.Contracts);
    }

    /// <summary>조달요구번호는 뷰가 이어 붙인 것을 도로 갈라 낸다 — 화면이 번호마다 따로 세운다.</summary>
    [Fact]
    public void 조달요구번호가_갈라져_온다()
    {
        _store.UpsertRequest(Request("000"));

        var request = Assert.Single(new Outline(_database).Build().Chains).Request!;
        Assert.Equal(["MPKPLA26910286", "MPKPLA26910290"], request.RequestNumbers);
    }

    /// <summary>
    /// 조달요구 하나가 납품장소마다 줄을 갈라 와도 번호는 한 번만 선다. 줄은 품목 잇기의
    /// 재료라 셋 다 남고, 요약만 접힌다 — 차례는 처음 나온 순번이다.
    /// </summary>
    [Fact]
    public void 납지로_갈린_조달요구번호는_한_번만_선다()
    {
        _store.UpsertRequest(Request("000") with
        {
            Items =
            [
                new RequestItemRecord { LineNo = 1, RequestNumber = "MPKPLA26910290", Quantity = 2, DeliveryPlace = "가부대" },
                new RequestItemRecord { LineNo = 2, RequestNumber = "MPKPLA26910286", Quantity = 1, DeliveryPlace = "가부대" },
                new RequestItemRecord { LineNo = 3, RequestNumber = "MPKPLA26910290", Quantity = 3, DeliveryPlace = "나부대" },
            ],
        });

        var request = Assert.Single(new Outline(_database).Build().Chains).Request!;
        Assert.Equal(["MPKPLA26910290", "MPKPLA26910286"], request.RequestNumbers);

        using var connection = _database.Open();
        using var count = connection.CreateCommand();
        count.CommandText = "SELECT COUNT(*) FROM v_접수품목;";
        Assert.Equal(3L, count.ExecuteScalar());
    }

    /// <summary>
    /// 접수도 인도조건은 줄마다 같으면 싣는다. 세부품명이 여럿인 여덟 줄짜리 접수가 모두
    /// 「납품장소 입고도」 인데도 비어 나가던 자리다.
    /// </summary>
    [Fact]
    public void 접수_인도조건은_세부품명이_여럿이어도_같으면_싣는다()
    {
        _store.UpsertRequest(Request("000") with
        {
            Items =
            [
                new RequestItemRecord { LineNo = 1, RequestNumber = "MPKPLA26910286", ItemName = "윈치", DeliveryTerms = "납품장소 입고도" },
                new RequestItemRecord { LineNo = 2, RequestNumber = "MPKPLA26910287", ItemName = "도르래줄", DeliveryTerms = "납품장소 입고도" },
            ],
        });

        using var connection = _database.Open();
        using var read = connection.CreateCommand();
        read.CommandText = "SELECT 인도조건 || '|' || 세부품명 FROM v_접수;";
        Assert.Equal("납품장소 입고도|", read.ExecuteScalar());
    }

    /// <summary>
    /// 접수도 세부품명이 여럿일 때 칸마다 본다. 단위가 모두 같으면 수량을 더하고, 단위가
    /// 갈리면 수량도 단위도 비운다 — 단위가 같아야 더한 수가 뜻을 갖는다.
    /// </summary>
    [Theory]
    [InlineData("대", "대", "3|대")]
    [InlineData("대", "식", "|")]
    public void 접수_수량은_단위가_같을_때만_더한다(string first, string second, string expected)
    {
        _store.UpsertRequest(Request("000") with
        {
            Items =
            [
                new RequestItemRecord { LineNo = 1, RequestNumber = "REQ-A", ItemName = "지어낸 기계", Quantity = 1, Unit = first },
                new RequestItemRecord { LineNo = 2, RequestNumber = "REQ-B", ItemName = "지어낸 부품", Quantity = 2, Unit = second },
            ],
        });

        using var connection = _database.Open();
        using var read = connection.CreateCommand();
        read.CommandText = "SELECT 수량 || '|' || 단위 FROM v_접수;";
        Assert.Equal(expected, read.ExecuteScalar());
    }

    /// <summary>
    /// 접수도 빈 줄을 한 값으로 센다. 한 줄만 재고번호를 적었으면 접수 전체의 재고번호가
    /// 아니고, 수량이 빈 줄이 있으면 합은 모르는 수다.
    /// </summary>
    [Theory]
    [InlineData("STK-1", "STK-1", 5, 3, "STK-1|8")]
    [InlineData("STK-1", null, 5, 3, "|8")]
    [InlineData("STK-1", "", 5, 3, "|8")]
    [InlineData("STK-1", "STK-1", 5, null, "STK-1|")]
    public void 접수도_빈_줄이_섞이면_칸을_비운다(
        string? firstStock, string? secondStock, int? firstQuantity, int? secondQuantity, string expected)
    {
        _store.UpsertRequest(Request("000") with
        {
            Items =
            [
                new RequestItemRecord { LineNo = 1, RequestNumber = "REQ-A", ItemName = "지어낸 기계", Quantity = firstQuantity, Unit = "대", StockNumber = firstStock },
                new RequestItemRecord { LineNo = 2, RequestNumber = "REQ-B", ItemName = "지어낸 부품", Quantity = secondQuantity, Unit = "대", StockNumber = secondStock },
            ],
        });

        using var connection = _database.Open();
        using var read = connection.CreateCommand();
        read.CommandText = "SELECT 재고번호 || '|' || 수량 FROM v_접수;";
        Assert.Equal(expected, read.ExecuteScalar());
    }

    /// <summary>차수는 접는다 — 본문에는 최신 하나가 서고 쌓인 차수는 이름표로 모인다.</summary>
    [Fact]
    public void 차수는_접혀서_최신_하나만_선다()
    {
        _store.UpsertNotice(Notice("000"));
        _store.UpsertNotice(Notice("001"));
        _store.UpsertContract(Contract("00"));
        _store.UpsertContract(Contract("01"));
        Link();

        var chain = Assert.Single(new Outline(_database).Build().Chains);

        Assert.Equal("R26BK09017075-001", chain.Notice!.Current!.Number);
        Assert.Equal(["000", "001"], chain.Notice.Current.Revisions);

        var contract = Assert.Single(chain.Contracts);
        Assert.Equal("R26TA0911050701", contract.Number);
        Assert.Equal(["00", "01"], contract.Revisions);
    }

    /// <summary>금액과 날짜는 뷰가 찍은 그대로 온다 — 표기 규칙이 화면마다 갈리면 안 된다.</summary>
    [Fact]
    public void 금액은_뷰가_찍은_표기로_온다()
    {
        _store.UpsertContract(Contract("00"));

        var chain = Assert.Single(new Outline(_database).Build().Chains);
        Assert.Equal("164,872,340", Assert.Single(chain.Contracts).Amount);
    }

    /// <summary>차례는 가장 이른 단계를 본다 — 공고가 있으면 게시일, 없으면 접수일.</summary>
    [Fact]
    public void 최근_것이_위로_온다()
    {
        _store.UpsertRequest(Request("000"));   // 2026/06/09
        _store.UpsertNotice(Notice("000"));       // 2026/07/18

        var chains = new Outline(_database).Build().Chains;

        Assert.Equal(2, chains.Count);
        Assert.NotNull(chains[0].Notice);    // 게시일이 늦어 위로
        Assert.NotNull(chains[1].Request);
    }

    [Fact]
    public void 빈_DB에서도_돈다() => Assert.Empty(new Outline(_database).Build().Chains);

    private void LinkRequest() => new RequestLinker(_database).Confirm(
        new EntityRef("request", "MPKPLA26910286", "000"),
        new EntityRef("notice", "R26BK09017075", "000"), 1.0);

    private static RequestRecord Request(string seq) => new()
    {
        RequestBase = "MPKPLA26910286",
        Seq = seq,
        Title = "수질측정기 구매",
        ReceivedOn = new DateTime(2026, 6, 9),
        Items =
        [
            new RequestItemRecord { LineNo = 1, RequestNumber = "MPKPLA26910286", ItemName = "수질검사장치", Quantity = 1, UnitPrice = 82_436_170m },
            new RequestItemRecord { LineNo = 2, RequestNumber = "MPKPLA26910290", ItemName = "시약", Quantity = 1, UnitPrice = 82_436_170m },
        ],
    };

    private void Link() => new Linker(_database).Confirm(
        new EntityRef("contract", "R26TA09110507", "00"),
        new EntityRef("notice", "R26BK09017075", "000"), 1.0);

    private static NoticeRecord Notice(string seq) => new()
    {
        NoticeBase = "R26BK09017075",
        Seq = seq,
        Title = "수질측정기 구매",
        PostedAt = new DateTime(2026, 7, 18),
    };

    private static ContractRecord Contract(string seq, bool 두줄 = false) => new()
    {
        ContractBase = "R26TA09110507",
        Seq = seq,
        Title = "2026년 가람 수질측정기 조달",
        Amount = 164_872_340m,
        Items = 두줄
            ? [Item(1, "수질검사장치"), Item(2, "시약")]
            : [],
    };

    private static ContractItemRecord Item(int lineNo, string name) => new()
    {
        LineNo = lineNo,
        ItemName = name,
        Quantity = 1,
        Unit = "대",
        UnitPrice = 82_436_170m,
        Amount = 82_436_170m,
    };

    public void Dispose()
    {
        SqliteConnection.ClearAllPools();
        foreach (var suffix in new[] { "", "-wal", "-shm" })
            File.Delete(_path + suffix);
        GC.SuppressFinalize(this);
    }
}

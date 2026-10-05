using Dapper;
using Microsoft.Data.Sqlite;
using Pclm.Core.Domain;
using Pclm.Core.Storage;
using Xunit;

namespace Pclm.Core.Tests;

/// <summary>
/// <c>v_계획</c> — 계획을 <b>분모</b>로 세운 뷰.
///
/// <para>여기서 붙들어 두는 것은 두 가지다. 첫째, <b>줄이 갈라지지 않는다</b>. 한 공고에 계약이
/// 여럿 달릴 수 있는데 그때 줄을 가르면 계획 건수가 부풀어 세어지고, 분모로 쓰려고 세운 뷰가
/// 분모 노릇을 못 한다. 둘째, <b>하나로 정할 수 없는 것을 하나인 양 내지 않는다</b> —
/// 계약이 둘이면 계약번호·계약일자·계약금액을 비우고 계약건수만 낸다(<c>v_공고</c> 의
/// <c>UniformItem</c> 규율 그대로).</para>
///
/// <para>「단계」는 <b>있는 것을 적는 것이지 판정이 아니다</b>(ADR-016). 늦었다·빠졌다를
/// 기계가 가리지 않는다 — 어디까지 왔는지만 적는다.</para>
///
/// <para>값은 모두 지어낸 것이다. 실제 조달 자료를 시험에 쓰지 않는다.</para>
/// </summary>
public class PlanViewTests : IDisposable
{
    private readonly string _path = Path.Combine(
        Path.GetTempPath(), $"pclm-planview-{Guid.NewGuid():N}.db");

    private readonly Database _database;
    private readonly Store _store;

    public PlanViewTests()
    {
        _database = PclmFile.Create(_path, PclmRole.Work);
        _store = new Store(_database);
    }

    // ── 단계 네 갈래 ─────────────────────────────────────────────

    /// <summary>계획에만 있는 것. 아직 아무 문서도 오지 않았다.</summary>
    [Fact]
    public void 계획에만_있으면_미착수다()
    {
        Plan("MPKPLA26910286");

        var row = Assert.Single(Rows());

        Assert.Equal("미착수", row["단계"]);
        Assert.Equal("", row["접수번호"]);
        Assert.Equal("", row["입찰공고번호"]);
        Assert.Equal("0", row["계약건수"]);
    }

    /// <summary>접수가 그 조달요구번호를 품었으면 접수다. 공고는 아직 없다.</summary>
    [Fact]
    public void 접수까지_왔으면_접수다()
    {
        Plan("MPKPLA26910286");
        _store.UpsertRequest(Request("000"));

        var row = Assert.Single(Rows());

        Assert.Equal("접수", row["단계"]);
        Assert.Equal("MPKPLA26910286-000", row["접수번호"]);
        Assert.Equal("2026/06/09", row["접수일자"]);
        Assert.Equal("", row["입찰공고번호"]);
    }

    /// <summary>접수가 공고로 이어졌으면 공고다. 계약은 아직 없다.</summary>
    [Fact]
    public void 공고까지_이어졌으면_공고다()
    {
        Plan("MPKPLA26910286");
        _store.UpsertRequest(Request("000"));
        _store.UpsertNotice(Notice("000"));
        LinkRequest();

        var row = Assert.Single(Rows());

        Assert.Equal("공고", row["단계"]);
        Assert.Equal("R26BK09017075-000", row["입찰공고번호"]);
        Assert.Equal("2026/07/18", row["게시일시"]);

        // 개찰은 공고 본문이 아니라 일정 표에서 온다 — v_공고 와 같은 자리를 짚는다.
        Assert.Equal("2026/07/25 11:00:00", row["개찰일시"]);

        Assert.Equal("0", row["계약건수"]);
        Assert.Equal("", row["계약번호"]);
    }

    /// <summary>계약이 달렸으면 계약이다. 하나뿐이라 계약 세 칸이 모두 찬다.</summary>
    [Fact]
    public void 계약이_달렸으면_계약이다()
    {
        셋다();

        var row = Assert.Single(Rows());

        Assert.Equal("계약", row["단계"]);
        Assert.Equal("1", row["계약건수"]);
        Assert.Equal("R26TA0911050700", row["계약번호"]);
        Assert.Equal("2026/07/21", row["계약일자"]);
        Assert.Equal("164,872,340", row["계약금액"]);
    }

    // ── 줄이 갈라지지 않는다 ─────────────────────────────────────

    /// <summary>
    /// 한 공고에 계약이 여럿이어도 <b>계획 한 줄은 한 줄이다</b>(분할 낙찰·수요기관 복수).
    /// 갈라지면 계획 건수가 부풀어 세어져, 분모로 쓰려고 세운 뷰가 분모 노릇을 못 한다.
    /// </summary>
    [Fact]
    public void 계약이_여럿이어도_줄이_갈라지지_않는다()
    {
        Plan("MPKPLA26910286");
        둘로_낙찰();

        var row = Assert.Single(Rows());

        Assert.Equal("계약", row["단계"]);
        Assert.Equal("2", row["계약건수"]);
    }

    /// <summary>
    /// 계약이 둘 이상이면 계약번호·계약일자·계약금액을 <b>비운다</b>. 그중 하나를 이 계획의
    /// 계약인 양 내면, 잘린 것은 눈에 띄지 않지만 틀린 것은 그대로 문서에 찍힌다.
    /// </summary>
    [Fact]
    public void 계약이_둘_이상이면_계약_세_칸을_비운다()
    {
        Plan("MPKPLA26910286");
        둘로_낙찰();

        var row = Assert.Single(Rows());

        Assert.Equal("2", row["계약건수"]);
        Assert.Equal("", row["계약번호"]);
        Assert.Equal("", row["계약일자"]);
        Assert.Equal("", row["계약금액"]);
    }

    // ── 분모의 경계 ──────────────────────────────────────────────

    /// <summary>
    /// 계획에 없는 조달요구번호의 접수는 <b>이 뷰에 서지 않는다</b>. 이 뷰는 분모이고,
    /// 계획 밖에서 들어온 것은 <c>v_통합</c> 이 낸다.
    /// </summary>
    [Fact]
    public void 계획에_없는_조달요구번호의_접수는_서지_않는다()
    {
        Plan("MPKPLA26910286");
        _store.UpsertRequest(Request("000"));

        // 계획에 없는 번호로 접수 하나를 더 넣는다.
        _store.UpsertRequest(
            Request("000", "MPKPLA26910999"));

        var row = Assert.Single(Rows());
        Assert.Equal("MPKPLA26910286", row["조달요구번호"]);
    }

    /// <summary>계획이 늘면 줄도 는다 — 접수가 없어도 분모에는 선다.</summary>
    [Fact]
    public void 계획_행마다_한_줄이다()
    {
        Plan("MPKPLA26910286");
        Plan("MPKPLA26910287");
        Plan("MPKPLA26910288");

        Assert.Equal(3, Rows().Count);
    }

    // ── 값 ───────────────────────────────────────────────────────

    /// <summary>
    /// 빈 값은 NULL 이 아니라 <b>빈 문자열</b>이다. 새면 받는 쪽의 빈 값 경고가 죽는다.
    /// 계획 엑셀은 담당자·요청부대부서명 칸이 비어 오는 일이 흔해서 여기가 실제로 걸린다.
    /// </summary>
    [Fact]
    public void 담당자가_비면_NULL_이_아니라_빈_문자열이다()
    {
        Plan("MPKPLA26910286", officer: null, requestingUnit: null);

        var row = Assert.Single(Rows());
        Assert.Equal("", row["담당자"]);
        Assert.Equal("", row["요청부대부서명"]);

        using var connection = _database.OpenReadOnly();
        using var command = connection.CreateCommand();
        command.CommandText = "SELECT * FROM v_계획;";

        using var reader = command.ExecuteReader();
        Assert.True(reader.Read());

        for (var i = 0; i < reader.FieldCount; i++)
            Assert.False(reader.IsDBNull(i), $"{reader.GetName(i)} 가 NULL 입니다.");
    }

    /// <summary>
    /// 계획 제 칸은 계획 표에서 <b>표본의 머리글 이름 그대로</b> 온다(ADR-023 개정).
    /// 지시수량만 INTEGER 라 문자열로 펴서 낸다.
    /// </summary>
    [Fact]
    public void 계획의_제_칸을_그대로_낸다()
    {
        Plan("MPKPLA26910286");

        var row = Assert.Single(Rows());

        Assert.Equal("2590-01-234-5678", row["재고번호"]);
        Assert.Equal("전동 윈치", row["품명"]);
        Assert.Equal("KRW", row["화폐구분"]);
        Assert.Equal("대", row["단위"]);
        Assert.Equal("8", row["지시수량"]);
        Assert.Equal("한별군수지원단 보급창", row["요청부대부서명"]);
        Assert.Equal("홍길동", row["요청부대담당자"]);
        Assert.Equal("051-000-0000", row["요청부대사용자전화번호"]);
        Assert.Equal("계약1과", row["계약부서"]);
        Assert.Equal("김철수", row["담당자"]);
        Assert.Equal("02-000-0000", row["연락처"]);
    }

    /// <summary>
    /// 사람이 <b>계약·공고·접수 탭에서 고친 값</b>이 계획 탭에도 그대로 비친다.
    ///
    /// <para>계획 제 칸(요구명·담당자·예산금액…)은 <c>plan</c> 표에서 그대로 오지만, 이어 온
    /// 여덟 열은 계획의 값이 아니라 <b>접수·공고·계약의 값</b>이라 덮개 자리가 있다. 그래서
    /// 원본 표가 아니라 <c>v_접수</c>·<c>v_공고</c>·<c>v_계약</c> 에서 받아 온다.</para>
    ///
    /// <para>원본에서 끌면 계약 탭에서 고친 계약금액이 계획 탭에는 옛 값으로 떠서, <b>같은
    /// 자료를 두 화면이 다르게 말한다</b> — 파서에서 그토록 경계한 그 일이 화면에 생긴다.</para>
    /// </summary>
    [Fact]
    public void 다른_탭에서_고친_값이_계획_탭에도_비친다()
    {
        셋다();

        _store.SetOverride(new EntityRef("contract", "R26TA09110507", "00"), "계약금액", "170,000,000");
        _store.SetOverride(new EntityRef("notice", "R26BK09017075", "000"), "게시일시", "2026/05/28");
        _store.SetOverride(new EntityRef("request", "MPKPLA26910286", "000"), "접수일자", "2026/04/29");

        var row = Assert.Single(Rows());

        Assert.Equal("170,000,000", row["계약금액"]);
        Assert.Equal("2026/05/28", row["게시일시"]);
        Assert.Equal("2026/04/29", row["접수일자"]);
    }

    [Fact]
    public void 빈_DB에서도_돈다() => Assert.Empty(Rows());

    // ── 짓기 ─────────────────────────────────────────────────────

    private void 셋다()
    {
        Plan("MPKPLA26910286");
        _store.UpsertRequest(Request("000"));
        _store.UpsertNotice(Notice("000"));
        _store.UpsertContract(Contract("00"));
        LinkRequest();
        Link("R26TA09110507");
    }

    /// <summary>한 공고에 계약 둘. 분할 낙찰·수요기관 복수에서 실제로 생기는 모양이다.</summary>
    private void 둘로_낙찰()
    {
        _store.UpsertRequest(Request("000"));
        _store.UpsertNotice(Notice("000"));
        _store.UpsertContract(Contract("00"));
        _store.UpsertContract(Contract("00", "R26TA09110508"));
        LinkRequest();
        Link("R26TA09110507");
        Link("R26TA09110508");
    }

    /// <summary>계획 한 줄. 엑셀을 지어 읽히는 것은 <c>PlanImportTests</c> 의 몫이라 바로 담는다.</summary>
    private void Plan(
        string number, string? officer = "김철수", string? requestingUnit = "한별군수지원단 보급창")
    {
        using var connection = _database.Open();
        connection.Execute(
            """
            INSERT INTO plan (
                request_number, stock_number, item_name, currency, unit, quantity,
                requesting_unit, requesting_officer, requesting_phone,
                contract_department, officer, contact, source_name, imported_at
            ) VALUES (
                @number, '2590-01-234-5678', '전동 윈치', 'KRW', '대', 8,
                @requestingUnit, '홍길동', '051-000-0000',
                '계약1과', @officer, '02-000-0000', '계획.xlsx', '2026-02-14T00:00:00Z'
            );
            """,
            new { number, officer, requestingUnit });
    }

    private List<Dictionary<string, string>> Rows()
    {
        using var connection = _database.OpenReadOnly();

        return [.. connection.Query("SELECT * FROM v_계획;")
            .Cast<IDictionary<string, object>>()
            .Select(row => row.ToDictionary(c => c.Key, c => c.Value?.ToString() ?? ""))];
    }

    private void Link(string contractBase) => new Linker(_database).Confirm(
        new EntityRef("contract", contractBase, "00"),
        new EntityRef("notice", "R26BK09017075", "000"), 1.0);

    private void LinkRequest() => new RequestLinker(_database).Confirm(
        new EntityRef("request", "MPKPLA26910286", "000"),
        new EntityRef("notice", "R26BK09017075", "000"), 1.0);

    private static RequestRecord Request(string seq, string @base = "MPKPLA26910286") => new()
    {
        RequestBase = @base,
        Seq = seq,
        Title = "윈치 8종 구매",
        ReceivedOn = new DateTime(2026, 6, 9),
        Items =
        [
            new RequestItemRecord
            {
                LineNo = 1, RequestNumber = @base, ItemName = "전동 윈치",
                Quantity = 8, UnitPrice = 21_288_647m,
            },
        ],
    };

    private static NoticeRecord Notice(string seq, string @base = "R26BK09017075") => new()
    {
        NoticeBase = @base,
        Seq = seq,
        Title = "윈치 8종 구매",
        PostedAt = new DateTime(2026, 7, 18),
        Schedule =
        [
            new NoticeScheduleRecord
            {
                LineNo = 1, Name = "개찰", StartsAt = new DateTime(2026, 7, 25, 11, 0, 0),
            },
        ],
    };

    private static ContractRecord Contract(string seq, string @base = "R26TA09110507") => new()
    {
        ContractBase = @base,
        Seq = seq,
        Title = "윈치 8종 구매",
        ContractedOn = new DateTime(2026, 7, 21),
        Amount = 164_872_340m,
    };

    public void Dispose()
    {
        SqliteConnection.ClearAllPools();
        foreach (var suffix in new[] { "", "-wal", "-shm" })
            File.Delete(_path + suffix);
        GC.SuppressFinalize(this);
    }
}

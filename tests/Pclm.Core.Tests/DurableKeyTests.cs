using Microsoft.Data.Sqlite;
using Pclm.Core.Domain;
using Pclm.Core.Storage;
using Xunit;

namespace Pclm.Core.Tests;

/// <summary>
/// 변경공고·변경계약이 들어와도 <b>사람이 붙인 것이 뷰에서 사라지지 않는지</b> 본다.
///
/// <para>손으로 적은 값과 확정한 링크가 <c>(본번호, 차수)</c> 에 매달려 있고 뷰는 최신 차수만
/// 내던 시절, 변경계약 한 건이 들어오면 진행상태·담당·메모가 조용히 빈 문자열이 되고 링크가
/// 끊겨 공고 열이 통째로 비었다. 아무 오류도 나지 않아 <b>사람이 알아챌 방법이 없었다</b> —
/// 이 시험이 지키려는 것이 바로 그 자리다(ADR-012).</para>
/// </summary>
public class DurableKeyTests : IDisposable
{
    private readonly string _path = Path.Combine(
        Path.GetTempPath(), $"pclm-durable-{Guid.NewGuid():N}.db");

    private readonly Database _database;
    private readonly Store _store;

    public DurableKeyTests()
    {
        _database = PclmFile.Create(_path, PclmRole.Work);
        _store = new Store(_database);
    }

    [Fact]
    public void 변경계약이_들어와도_손으로_적은_값이_뷰에_남는다()
    {
        _store.UpsertContract(Contract("00"));
        _store.SetUserField(new EntityRef("contract", "R26TA09110507", "00"), "진행상태", "납품");

        // 변경계약. 차수가 오른 별개의 행으로 쌓이고 뷰는 이쪽을 낸다.
        _store.UpsertContract(Contract("01"));

        // 계약의 사용자 열은 계약 뷰에 붙고, 통합은 그 뷰를 통째로 받는다.
        var row = Single("SELECT 차수, 진행상태 FROM v_통합;");

        Assert.Equal("01", row["차수"]);
        Assert.Equal("납품", row["진행상태"]);
    }

    [Fact]
    public void 변경공고가_들어와도_손으로_적은_값이_뷰에_남는다()
    {
        _store.UpsertNotice(Notice("000"));
        _store.SetUserField(new EntityRef("notice", "R26BK09017075", "000"), "검토여부", "참여");

        _store.UpsertNotice(Notice("001"));

        var row = Single("SELECT 차수, 검토여부 FROM v_공고;");

        Assert.Equal("001", row["차수"]);
        Assert.Equal("참여", row["검토여부"]);
    }

    [Fact]
    public void 변경계약이_들어와도_확정한_링크가_끊기지_않는다()
    {
        _store.UpsertNotice(Notice("000"));
        _store.UpsertContract(Contract("00"));

        new Linker(_database).Confirm(
            new EntityRef("contract", "R26TA09110507", "00"),
            new EntityRef("notice", "R26BK09017075", "000"), 1.0);

        _store.UpsertContract(Contract("01"));

        var row = Single("SELECT 계약번호, 공고명 FROM v_통합;");

        Assert.Equal("R26TA0911050701", row["계약번호"]);
        Assert.Equal("수질측정기 구매", row["공고명"]);
    }

    [Fact]
    public void 변경공고가_들어와도_확정한_링크가_끊기지_않는다()
    {
        _store.UpsertNotice(Notice("000"));
        _store.UpsertContract(Contract("00"));

        new Linker(_database).Confirm(
            new EntityRef("contract", "R26TA09110507", "00"),
            new EntityRef("notice", "R26BK09017075", "000"), 1.0);

        _store.UpsertNotice(Notice("001") with { Title = "수질측정기 구매(변경)" });

        var row = Single("SELECT 입찰공고번호, 공고명 FROM v_통합;");

        Assert.Equal("R26BK09017075-001", row["입찰공고번호"]);
        Assert.Equal("수질측정기 구매(변경)", row["공고명"]);
    }

    [Fact]
    public void 변경접수가_들어와도_손으로_적은_값이_뷰에_남는다()
    {
        _store.UpsertRequest(Request("000"));
        _store.AddUserColumn("request", "비고", "text", []);
        _store.SetUserField(new EntityRef("request", "MPKPLA26910286", "000"), "비고", "협의중");

        _store.UpsertRequest(Request("001"));

        var row = Single("SELECT 차수, 비고 FROM v_접수;");

        Assert.Equal("001", row["차수"]);
        Assert.Equal("협의중", row["비고"]);
    }

    [Fact]
    public void 변경접수가_들어와도_확정한_링크가_끊기지_않는다()
    {
        _store.UpsertRequest(Request("000"));
        _store.UpsertNotice(Notice("000"));

        new RequestLinker(_database).Confirm(
            new EntityRef("request", "MPKPLA26910286", "000"),
            new EntityRef("notice", "R26BK09017075", "000"), 1.0);

        _store.UpsertRequest(Request("001"));
        _store.UpsertContract(Contract("00"));

        new Linker(_database).Confirm(
            new EntityRef("contract", "R26TA09110507", "00"),
            new EntityRef("notice", "R26BK09017075", "000"), 1.0);

        // 링크는 계열끼리라 차수가 올라도 통합의 접수 열이 그대로 채워진다.
        var row = Single("SELECT 접수번호, 요청명 FROM v_통합;");

        Assert.Equal("MPKPLA26910286-001", row["접수번호"]);
        Assert.Equal("윈치 8종 구매", row["요청명"]);
    }

    /// <summary>차수는 TEXT 라 사전순 비교는 자릿수가 늘어나는 날 뒤집힌다.</summary>
    [Fact]
    public void 차수가_두_자리를_넘어도_최신이_최신이다()
    {
        _store.UpsertContract(Contract("09"));
        _store.UpsertContract(Contract("10"));

        Assert.Equal("10", Single("SELECT 차수 FROM v_계약;")["차수"]);
    }

    private static ContractRecord Contract(string seq) => new()
    {
        ContractBase = "R26TA09110507",
        Seq = seq,
        Title = "2026년 가람 수질측정기 조달",
        Amount = 164_872_340m,
    };

    private static NoticeRecord Notice(string seq) => new()
    {
        NoticeBase = "R26BK09017075",
        Seq = seq,
        Title = "수질측정기 구매",
    };

    private static RequestRecord Request(string seq) => new()
    {
        RequestBase = "MPKPLA26910286",
        Seq = seq,
        Title = "윈치 8종 구매",
        Items =
        [
            new RequestItemRecord
            {
                LineNo = 1, RequestNumber = "MPKPLA26910286",
                ItemName = "받침목", Quantity = 70, UnitPrice = 310_200m,
            },
        ],
    };

    private Dictionary<string, string> Single(string sql)
    {
        using var connection = _database.OpenReadOnly();
        using var command = connection.CreateCommand();
        command.CommandText = sql;

        using var reader = command.ExecuteReader();
        Assert.True(reader.Read(), $"행이 없습니다: {sql}");

        var row = new Dictionary<string, string>();
        for (var i = 0; i < reader.FieldCount; i++)
            row[reader.GetName(i)] = reader.IsDBNull(i) ? "" : reader.GetValue(i).ToString() ?? "";

        Assert.False(reader.Read(), $"행이 둘 이상입니다: {sql}");
        return row;
    }

    public void Dispose()
    {
        SqliteConnection.ClearAllPools();
        foreach (var suffix in new[] { "", "-wal", "-shm" })
            File.Delete(_path + suffix);
        GC.SuppressFinalize(this);
    }
}

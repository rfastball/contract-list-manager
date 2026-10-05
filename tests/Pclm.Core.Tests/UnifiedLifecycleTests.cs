using Dapper;
using Microsoft.Data.Sqlite;
using Pclm.Core.Domain;
using Pclm.Core.Storage;
using Xunit;

namespace Pclm.Core.Tests;

/// <summary>
/// 통합 시트가 <b>생애주기 전부</b>를 내는지 본다 — 접수만 온 것, 공고까지 온 것,
/// 셋이 다 온 것, 어디에도 매달리지 못한 계약이 모두 한 줄씩 선다.
///
/// <para>예전에는 통합이 <c>v_계약</c> 을 줄기로 세워 <b>계약이 있는 것만</b> 냈다.
/// 그래서 같은 자료를 두 화면이 다르게 셌다 — 구조 보기에는 접수만 온 건이 서 있는데
/// 표에는 없었다. 무엇이 아직 안 들어왔는지는 <b>표에서도</b> 보여야 한다.</para>
///
/// <para>여기서 지키는 것은 그 둘이 <b>같은 것을 센다</b>는 사실이다. 사슬을 세우는 규칙이
/// <see cref="Outline"/> 과 SQL 두 곳에 적혀 있어, 한쪽만 고치면 다시 갈라진다.</para>
/// </summary>
public class UnifiedLifecycleTests : IDisposable
{
    private readonly string _path = Path.Combine(
        Path.GetTempPath(), $"pclm-unified-{Guid.NewGuid():N}.db");

    private readonly Database _database;
    private readonly Store _store;

    public UnifiedLifecycleTests()
    {
        _database = PclmFile.Create(_path, PclmRole.Work);
        _store = new Store(_database);
    }

    /// <summary>접수만 들어온 것. 통합에 서지 않으면 넣은 사람이 넣지 않은 줄 안다.</summary>
    [Fact]
    public void 접수만_들어온_것도_한_줄로_선다()
    {
        _store.UpsertRequest(Request("000"));

        var row = Assert.Single(Rows());

        Assert.Equal("MPKPLA26910286-000", row["접수번호"]);
        Assert.Equal("", row["입찰공고번호"]);
        Assert.Equal("", row["계약번호"]);
    }

    /// <summary>공고까지 온 것. 계약 자리는 비되 줄은 선다.</summary>
    [Fact]
    public void 공고까지만_온_것도_한_줄로_선다()
    {
        _store.UpsertRequest(Request("000"));
        _store.UpsertNotice(Notice("000"));
        LinkRequest();

        var row = Assert.Single(Rows());

        Assert.Equal("MPKPLA26910286-000", row["접수번호"]);
        Assert.Equal("R26BK09017075-000", row["입찰공고번호"]);
        Assert.Equal("", row["계약번호"]);
    }

    /// <summary>접수 → 공고 → 계약. 이 자료의 기본 모양이라 한 줄이어야 한다.</summary>
    [Fact]
    public void 셋이_다_온_것은_한_줄이다()
    {
        셋다();

        var row = Assert.Single(Rows());

        Assert.Equal("MPKPLA26910286-000", row["접수번호"]);
        Assert.Equal("R26BK09017075-000", row["입찰공고번호"]);
        Assert.Equal("R26TA0911050700", row["계약번호"]);
        Assert.Equal("164,872,340", row["계약금액"]);
    }

    /// <summary>어디에도 매달리지 못한 계약. 계약서만 먼저 들어온 흔한 경우다.</summary>
    [Fact]
    public void 이어지지_않은_계약도_한_줄로_선다()
    {
        _store.UpsertNotice(Notice("000"));
        _store.UpsertContract(Contract("00"));

        var rows = Rows();
        Assert.Equal(2, rows.Count);

        var 홀로 = Assert.Single(rows, r => r["계약번호"] != "");
        Assert.Equal("", 홀로["입찰공고번호"]);

        var 공고만 = Assert.Single(rows, r => r["계약번호"] == "");
        Assert.Equal("R26BK09017075-000", 공고만["입찰공고번호"]);
    }

    /// <summary>
    /// 한 공고에 계약이 여럿이면(분할 낙찰·수요기관 복수) 계약마다 한 줄로 갈라진다.
    /// 그때 공고·접수 열이 되풀이되는 것은 옳다 — 줄 하나가 계약 하나라는 약속이 지켜진다.
    /// </summary>
    [Fact]
    public void 계약이_여럿이면_계약마다_한_줄이다()
    {
        _store.UpsertNotice(Notice("000"));
        _store.UpsertContract(Contract("00"));
        _store.UpsertContract(Contract("00", "R26TA09110508"));
        Link("R26TA09110507");
        Link("R26TA09110508");

        var rows = Rows();

        Assert.Equal(2, rows.Count);
        Assert.All(rows, r => Assert.Equal("R26BK09017075-000", r["입찰공고번호"]));
        Assert.Equal(
            ["R26TA0911050700", "R26TA0911050800"],
            rows.Select(r => r["계약번호"]).Order());
    }

    /// <summary>
    /// 구조 보기와 표 보기가 <b>같은 것을 센다</b>. 사슬을 세우는 규칙이 두 곳에 적혀 있어,
    /// 한쪽만 고치면 두 화면이 다시 갈라진다 — 이 시험이 지키려는 자리가 거기다.
    /// </summary>
    [Fact]
    public void 구조와_표가_같은_수를_센다()
    {
        셋다();
        _store.UpsertRequest(Request("000", "MPKPLA26910300"));
        _store.UpsertNotice(Notice("000", "R26BK09017076"));
        _store.UpsertContract(Contract("00", "R26TA09110509"));
        취소되고_재공고된_한_벌();

        // 사슬 하나가 계약 여럿을 품으면 표는 계약마다 한 줄이 되므로, 계약을 펴서 센다.
        var 사슬 = new Outline(_database).Build().Chains;
        var 펴본_수 = 사슬.Sum(c => Math.Max(c.Contracts.Count, 1));

        // 세는 것부터 견준다. 이 시험의 이름이 그것이라, 붙박이가 늘어 절대값이 어긋난 것과
        // 두 화면이 갈린 것을 실패 한 줄로 구별할 수 있어야 한다.
        Assert.Equal(펴본_수, Rows().Count);
        Assert.Equal(5, 사슬.Count);
    }

    /// <summary>빈 칸은 NULL 이 아니라 빈 문자열이다 — 새면 받는 쪽의 빈 값 경고가 죽는다.</summary>
    [Fact]
    public void 빈_칸은_NULL_이_아니라_빈_문자열이다()
    {
        _store.UpsertRequest(Request("000"));

        using var connection = _database.OpenReadOnly();
        using var command = connection.CreateCommand();
        command.CommandText = "SELECT * FROM v_통합;";

        using var reader = command.ExecuteReader();
        Assert.True(reader.Read());

        for (var i = 0; i < reader.FieldCount; i++)
            Assert.False(reader.IsDBNull(i), $"{reader.GetName(i)} 가 NULL 입니다.");
    }

    /// <summary>사람이 세운 계약 열도 계약 없는 줄에서 빈 문자열로 선다.</summary>
    [Fact]
    public void 사람이_세운_계약_열도_함께_온다()
    {
        셋다();
        _store.SetUserField(new EntityRef("contract", "R26TA09110507", "00"), "진행상태", "납품");
        _store.UpsertRequest(Request("000", "MPKPLA26910300"));

        var rows = Rows();

        Assert.Equal("납품", Assert.Single(rows, r => r["계약번호"] != "")["진행상태"]);
        Assert.Equal("", Assert.Single(rows, r => r["계약번호"] == "")["진행상태"]);
    }

    [Fact]
    public void 빈_DB에서도_돈다() => Assert.Empty(Rows());

    private void 셋다()
    {
        _store.UpsertRequest(Request("000"));
        _store.UpsertNotice(Notice("000"));
        _store.UpsertContract(Contract("00"));
        Link("R26TA09110507");
        LinkRequest();
    }

    /// <summary>
    /// 취소 → 재공고 한 벌. 본번호는 갈리지만 <b>조달 건은 하나</b>라, 접수도 계약도 하나이고
    /// 표도 한 줄만 낸다. 사람이 이어 둔 것은 <b>나중에 취소된 원공고</b>다 — 인수인계 직후에
    /// 실제로 이 모양이 나온다.
    ///
    /// <para>붙박이에 이것이 없으면 이 시험은 <b>합쳐진 건을 한 번도 보지 못한다</b>.
    /// 공고 하나가 곧 사슬 하나이던 시절에는 구조와 표가 저절로 같은 수여서, 갈린 본번호가
    /// 서지 않는 붙박이는 지키려던 자리를 그대로 비워 둔다.</para>
    /// </summary>
    private void 취소되고_재공고된_한_벌()
    {
        _store.UpsertNotice(Notice("000", "R26BK09011054"));
        _store.UpsertNotice(
            Notice("001", "R26BK09011054", "R26BK09011054-000"));
        _store.UpsertNotice(
            Notice("000", "R26BK09012082", "R26BK09011054-001", "R26BK09011054-000"));

        _store.UpsertRequest(Request("000", "MBKLMH26930006"));
        _store.UpsertContract(Contract("00", "R26TA09080469"));

        new Linker(_database).Confirm(
            new EntityRef("contract", "R26TA09080469", "00"),
            new EntityRef("notice", "R26BK09011054", "000"), 1.0);

        new RequestLinker(_database).Confirm(
            new EntityRef("request", "MBKLMH26930006", "000"),
            new EntityRef("notice", "R26BK09011054", "000"), 1.0);
    }

    private List<Dictionary<string, string>> Rows()
    {
        using var connection = _database.OpenReadOnly();

        return [.. connection.Query("SELECT * FROM v_통합;")
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
        Title = "수질측정기 구매",
        ReceivedOn = new DateTime(2026, 6, 9),
        Items =
        [
            new RequestItemRecord
            {
                LineNo = 1, RequestNumber = @base, ItemName = "수질검사장치",
                Quantity = 1, UnitPrice = 164_872_340m,
            },
        ],
    };

    /// <param name="related">
    /// 공고서의 「관련공고」 칸. 이것 하나가 취소·재공고로 갈린 본번호를 잇는 단서다(ADR-025).
    /// </param>
    private static NoticeRecord Notice(
        string seq, string @base = "R26BK09017075", params string[] related) => new()
    {
        NoticeBase = @base,
        Seq = seq,
        Title = "수질측정기 구매",
        NoticeKind = seq == "001" ? "취소공고" : "등록공고",
        PostedAt = new DateTime(2026, 7, 18),
        RelatedNotices = related,
    };

    private static ContractRecord Contract(string seq, string @base = "R26TA09110507") => new()
    {
        ContractBase = @base,
        Seq = seq,
        Title = "수질측정기 구매",
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

using Dapper;
using Microsoft.Data.Sqlite;
using Pclm.Core.Domain;
using Pclm.Core.Storage;
using Xunit;

namespace Pclm.Core.Tests;

/// <summary>
/// 하자담보기간이 계약면에 <b>한 가지 모양</b>으로 찍히는가.
///
/// <para>공고는 같은 기간을 세 가지로 받는다 — 화면이 적은 문장, 해·달 두 칸, 총 개월 하나.
/// 자리마다 따로 적던 동안 <c>2년 0개월</c>·<c>2 년 0 개월</c>·<c>2년</c> 이 섞였고, 개월만
/// 받은 자리는 나눗셈으로 달을 버려 18개월이 <c>1년</c> 으로 찍혔다. 값은 지어 넣는다.</para>
/// </summary>
public class WarrantyViewTests : IDisposable
{
    private readonly string _path = Path.Combine(
        Path.GetTempPath(), $"pclm-warranty-{Guid.NewGuid():N}.db");

    private readonly Database _database;
    private readonly Store _store;

    public WarrantyViewTests()
    {
        _database = PclmFile.Create(_path, PclmRole.Work);
        _store = new Store(_database);
    }

    /// <summary>총 개월 하나만 받은 공고. 해로 떨어지지 않는 달을 버리지 않는다.</summary>
    [Theory]
    [InlineData(36, "3년")]
    [InlineData(18, "1년 6개월")]
    [InlineData(6, "6개월")]
    [InlineData(0, "")]
    public void 개월만_받은_공고는_해와_달로_편다(int months, string expected)
    {
        _store.UpsertNotice(Notice() with { WarrantyMonths = months });

        Assert.Equal(expected, Cell("v_공고", "하자담보기간"));
    }

    /// <summary>해·달 두 칸을 받은 공고. 달이 0 이면 붙이지 않는다 — 양식이 <c>2년</c> 으로 적는다.</summary>
    [Theory]
    [InlineData("2", "0", "2년")]
    [InlineData("1", "6", "1년 6개월")]
    [InlineData("0", "6", "6개월")]
    public void 해와_달을_받은_공고는_합쳐서_편다(string years, string months, string expected)
    {
        _store.UpsertNotice(Notice());
        Execute("UPDATE notice SET warranty_years = @years, warranty_month_part = @months", new { years, months });

        Assert.Equal(expected, Cell("v_공고", "하자담보기간"));
    }

    /// <summary>화면이 문장으로 적어 보낸 공고. 셈하지 않고 글자만 다듬는다.</summary>
    [Theory]
    [InlineData("2 년 0 개월", "2년")]
    [InlineData("0 년 6 개월", "6개월")]
    [InlineData("0 년 0 개월", "")]
    [InlineData("2 년 6 개월", "2년 6개월")]
    [InlineData("2년 10개월", "2년 10개월")]
    public void 문장으로_받은_공고는_글자만_다듬는다(string text, string expected)
    {
        _store.UpsertNotice(Notice());
        Execute("UPDATE notice SET warranty_text = @text", new { text });

        Assert.Equal(expected, Cell("v_공고", "하자담보기간"));
    }

    /// <summary>계약도 같은 모양이다.</summary>
    [Theory]
    [InlineData("1", "0", "1년")]
    [InlineData("1", "6", "1년 6개월")]
    [InlineData("0", "0", "")]
    public void 계약의_하자담보책임기간도_같은_모양이다(string years, string months, string expected)
    {
        _store.UpsertContract(new ContractRecord { ContractBase = "R26TA00000001", Seq = "00", Title = "지어낸 계약" });
        Execute("UPDATE contract SET warranty_years = @years, warranty_months = @months", new { years, months });

        Assert.Equal(expected, Cell("v_계약", "하자담보책임기간"));
    }

    // ── 거들기 ───────────────────────────────────────────

    private static NoticeRecord Notice() => new()
    {
        NoticeBase = "R26BK00000001",
        Seq = "000",
        Title = "지어낸 공고",
    };

    private void Execute(string sql, object args)
    {
        using var connection = _database.Open();
        connection.Execute(sql, args);
    }

    private string Cell(string view, string column)
    {
        using var connection = _database.OpenReadOnly();
        return connection.QuerySingle<string?>($"SELECT \"{column}\" FROM \"{view}\";") ?? "";
    }

    public void Dispose()
    {
        SqliteConnection.ClearAllPools();
        GC.SuppressFinalize(this);
        if (File.Exists(_path)) File.Delete(_path);
    }
}

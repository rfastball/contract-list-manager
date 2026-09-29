using Microsoft.Data.Sqlite;
using Pclm.Core.Storage;
using Xunit;

namespace Pclm.Core.Tests;

/// <summary>
/// 현황의 <b>이음매</b>가 섰는지 본다.
///
/// <para>지금 들어 있는 지표는 가상이고 버릴 것이다. 그러므로 여기서 지킬 것은 "무엇이 몇 건
/// 나오는가" 가 아니라 <b>지표를 더하거나 빼는 일이 <see cref="Status.Metrics"/> 배열만 고치는
/// 일인가</b>이다 — 화면·다리·타입이 그대로여야 한다. <c>metrics</c> 인자가 있는 까닭이 그것을
/// 이 시험이 확인하기 위해서다.</para>
///
/// <para>붙박이 지표의 SQL 은 <b>빈 DB 에서도 전부 돌아야 한다</b>. 오타는 세는 자리에서가
/// 아니라 사람이 현황 탭을 여는 자리에서 터지는데, 그때는 무엇이 잘못됐는지 화면에 한 줄도
/// 서지 않는다.</para>
/// </summary>
public class StatusTests : IDisposable
{
    private readonly string _path = Path.Combine(
        Path.GetTempPath(), $"pclm-status-{Guid.NewGuid():N}.db");

    private readonly Database _database;

    public StatusTests()
    {
        _database = new Database(_path);
        _database.Migrate();
    }

    /// <summary>
    /// 붙박이 지표의 SQL 이 하나도 빠짐없이 돈다. 표 이름이나 열 이름의 오타를 여기서 잡는다 —
    /// 빈 DB 라 수는 모두 0 이지만, <b>돌았다는 사실</b>이 판정이다.
    /// </summary>
    [Fact]
    public void 붙박이_지표는_빈_DB에서도_전부_돈다()
    {
        var report = new Status(_database).Report();

        Assert.Equal(
            Status.Metrics.Select(m => m.이름).Count(),
            report.Groups.Sum(g => g.Cells.Count));

        Assert.All(report.Groups, g => Assert.All(g.Cells, c => Assert.Equal(0, c.수)));
    }

    /// <summary>묶음은 배열에 <b>처음 나온 차례</b>로 선다. 사전순이 아니다.</summary>
    [Fact]
    public void 묶음은_배열에_처음_나온_차례로_선다()
    {
        var report = new Status(_database, [
            new("하", "가", "SELECT 1;", null),
            new("가", "나", "SELECT 2;", null),
            new("하", "다", "SELECT 3;", null),
            new("나", "라", "SELECT 4;", null),
        ]).Report();

        Assert.Equal(["하", "가", "나"], report.Groups.Select(g => g.이름));

        // 같은 묶음의 지표는 배열 차례대로 그 묶음 안에 든다.
        Assert.Equal(["가", "다"], report.Groups[0].Cells.Select(c => c.이름));
    }

    /// <summary>
    /// <b>지표를 하나 더하는 일이 배열을 고치는 일이다.</b> 더한 칸이 보고에 뜨고 나머지는
    /// 그대로다 — 이 성질이 무너지면 지표를 갈아끼울 때 화면과 다리를 함께 고쳐야 한다.
    /// </summary>
    [Fact]
    public void 지표를_하나_더하면_그_칸만_는다()
    {
        var 먼저 = new Status(_database).Report();

        var 더한 = new Status(_database, [
            .. Status.Metrics,
            new("쌓인 것", "상대자", "SELECT COUNT(*) FROM counterparty;", null),
        ]).Report();

        Assert.Equal(먼저.Groups.Count, 더한.Groups.Count);
        Assert.Equal(먼저.Groups.Sum(g => g.Cells.Count) + 1, 더한.Groups.Sum(g => g.Cells.Count));

        var 쌓인것 = Assert.Single(더한.Groups, g => g.이름 == "쌓인 것");
        Assert.Equal("상대자", 쌓인것.Cells[^1].이름);

        // 나머지 칸은 이름도 차례도 그대로다 — 더한 것만 빼면 먼저 것과 같다.
        Assert.Equal(
            먼저.Groups.SelectMany(g => g.Cells).Select(c => c.이름),
            더한.Groups.SelectMany(g => g.Cells).Select(c => c.이름).Where(n => n != "상대자"));
    }

    /// <summary>거르개는 손대지 않고 <b>그대로</b> 실어 낸다 — 화면이 한 자리에서 풀어 쓴다.</summary>
    [Fact]
    public void 거르개가_그대로_실려_온다()
    {
        var report = new Status(_database, [
            new("단계", "미착수", "SELECT 7;", "단계=미착수"),
            new("단계", "합", "SELECT 9;", null),
        ]).Report();

        var group = Assert.Single(report.Groups);

        Assert.Equal("단계=미착수", group.Cells[0].거르개);
        Assert.Equal(7, group.Cells[0].수);
        Assert.Null(group.Cells[1].거르개);
    }

    /// <summary>붙박이 지표의 거르개는 <c>"열이름=값"</c> 꼴 하나뿐이다.</summary>
    [Fact]
    public void 붙박이_거르개는_열이름_등호_값_꼴이다()
    {
        foreach (var 거르개 in Status.Metrics.Select(m => m.거르개).OfType<string>())
        {
            var 나뉨 = 거르개.Split('=');
            Assert.Equal(2, 나뉨.Length);
            Assert.NotEmpty(나뉨[0]);
            Assert.NotEmpty(나뉨[1]);
        }
    }

    public void Dispose()
    {
        SqliteConnection.ClearAllPools();
        foreach (var suffix in new[] { "", "-wal", "-shm" })
            File.Delete(_path + suffix);
        GC.SuppressFinalize(this);
    }
}

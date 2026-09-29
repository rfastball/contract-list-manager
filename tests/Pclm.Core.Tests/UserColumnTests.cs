using Microsoft.Data.Sqlite;
using Pclm.Core.Domain;
using Pclm.Core.Storage;
using Xunit;

namespace Pclm.Core.Tests;

/// <summary>
/// 사람이 스스로 만드는 열.
///
/// <para>여기서 가장 조심하는 것은 <b>열 이름이 뷰 SQL 에 그대로 박힌다</b>는 점이다.
/// 뷰는 앱이 열릴 때마다 다시 지어지므로, 깨진 이름 하나가 들어가면 그 뒤로 앱이 아예 뜨지
/// 못한다 — 자료를 잃지는 않지만 사람은 프로그램이 죽었다고 여긴다.</para>
/// </summary>
public class UserColumnTests : IDisposable
{
    private readonly string _path = Path.Combine(
        Path.GetTempPath(), $"pclm-columns-{Guid.NewGuid():N}.db");

    private readonly Database _database;
    private readonly Store _store;

    public UserColumnTests()
    {
        _database = new Database(_path);
        _database.Migrate();
        _store = new Store(_database);
    }

    /// <summary>
    /// 미리 세워 주는 열은 종류마다 둘뿐이고, <b>모두 자유 입력</b>이다.
    /// 정해진 후보로는 실제로 적고 싶은 것이 다 담기지 않아 후보 목록을 걷었다.
    /// </summary>
    [Theory]
    [InlineData("contract", "진행상태")]
    [InlineData("notice", "검토여부")]
    public void 기본_열은_자유_입력_둘뿐이다(string entityType, string first)
    {
        var columns = _store.UserColumns(entityType);

        Assert.Equal([first, "메모"], columns.Select(c => c.FieldName));
        Assert.All(columns, c => Assert.Equal("text", c.Kind));
        Assert.All(columns, c => Assert.Empty(c.Choices));
    }

    /// <summary>
    /// 접수에도 사람 열을 세울 수 있다. <c>user_column</c> 의 <c>CHECK</c> 를 넓히지 않았으면
    /// 여기서 막힌다(스키마 V11).
    /// </summary>
    [Fact]
    public void 접수에도_열을_더할_수_있다()
    {
        _store.AddUserColumn("request", "비고", "text", []);

        Assert.Contains("비고", ViewColumns("v_접수_v1"));
        Assert.Equal(["비고"], _store.UserColumns("request").Select(c => c.FieldName));
    }

    /// <summary>
    /// <c>v_통합_v2</c> 에만 있는 이름과도 부딪히지 않는다. v1 만 보던 시절에는 계약 열에
    /// <c>요청명</c> 을 세울 수 있었고, 그것이 통합 v2 에서 조용히 <c>요청명:1</c> 이 되었다.
    /// </summary>
    [Theory]
    [InlineData("요청명")]
    [InlineData("접수번호")]
    [InlineData("접수수수료")]
    public void 통합_v2_가_쓰는_이름은_계약_열로_쓸_수_없다(string name)
    {
        var ex = Assert.Throws<InvalidOperationException>(
            () => _store.AddUserColumn("contract", name, "text", []));

        Assert.Contains("표가 이미 쓰는 이름입니다", ex.Message);
    }

    [Fact]
    public void 열을_더하면_표에_선다()
    {
        _store.AddUserColumn("contract", "담당", "text", []);

        Assert.Contains("담당", ViewColumns("v_계약_v1"));
        Assert.Contains("담당", ViewColumns("v_통합_v1"));
    }

    [Fact]
    public void 후보를_정한_열도_만들_수_있다()
    {
        _store.AddUserColumn("contract", "우선순위", "choice", ["높음", "보통", "낮음"]);

        var made = _store.UserColumns("contract").Single(c => c.FieldName == "우선순위");

        Assert.Equal("choice", made.Kind);
        Assert.Equal(["높음", "보통", "낮음"], made.Choices);
    }

    /// <summary>후보는 choice 일 때만 뜻이 있다. 남겨 두면 화면이 없는 목록을 들고 있게 된다.</summary>
    [Fact]
    public void 자유_입력_열에는_후보를_담지_않는다()
    {
        _store.AddUserColumn("contract", "담당", "text", ["가", "나"]);

        Assert.Empty(_store.UserColumns("contract").Single(c => c.FieldName == "담당").Choices);
    }

    /// <summary>값은 열 이름으로 매달려 있다 — 이름만 바꾸면 적어 둔 것이 통째로 미아가 된다.</summary>
    [Fact]
    public void 이름을_바꾸면_적어_둔_값이_따라온다()
    {
        Put();
        _store.SetUserField(Contract, "메모", "선금 신청함");

        _store.UpdateUserColumn("contract", "메모", "비고", "text", []);

        Assert.Equal("선금 신청함", Cell("비고"));
    }

    /// <summary>단추 한 번에 백 건의 메모가 사라지면 되돌릴 길이 없다.</summary>
    [Fact]
    public void 열을_지워도_값은_남고_다시_만들면_되살아난다()
    {
        Put();
        _store.SetUserField(Contract, "메모", "선금 신청함");

        _store.RemoveUserColumn("contract", "메모");
        Assert.DoesNotContain("메모", ViewColumns("v_계약_v1"));

        _store.AddUserColumn("contract", "메모", "text", []);
        Assert.Equal("선금 신청함", Cell("메모"));
    }

    [Fact]
    public void 차례를_옮기면_표의_열_순서가_바뀐다()
    {
        Assert.Equal(["진행상태", "메모"], _store.UserColumns("contract").Select(c => c.FieldName));

        _store.MoveUserColumn("contract", "메모", -1);

        Assert.Equal(["메모", "진행상태"], _store.UserColumns("contract").Select(c => c.FieldName));

        var view = ViewColumns("v_계약_v1");
        Assert.True(view.IndexOf("메모") < view.IndexOf("진행상태"));
    }

    // ── 막아야 하는 것 ───────────────────────────────────

    /// <summary>
    /// 따옴표가 든 이름은 뷰 정의를 깨뜨린다. 그리고 뷰는 앱이 열릴 때마다 다시 지어지므로,
    /// 한 번 들어가면 <b>그 뒤로 앱이 뜨지 못한다</b>.
    /// </summary>
    [Theory]
    [InlineData("메모'")]
    [InlineData("메모\"")]
    [InlineData("메모\\")]
    [InlineData("메모', 1) AS x, (SELECT 1")]
    public void 따옴표가_든_이름은_막는다(string name)
    {
        Assert.Throws<InvalidOperationException>(() => _store.AddUserColumn("contract", name, "text", []));

        // 막았으니 뷰는 멀쩡하다 — 다시 지어도 터지지 않아야 한다.
        _database.RefreshViews();
        Assert.DoesNotContain(name, ViewColumns("v_계약_v1"));
    }

    /// <summary>
    /// 겹치는 이름은 SQLite 가 조용히 <c>이름:1</c> 로 바꾼다 — 오류가 나지 않아
    /// 표가 이상해진 뒤에야 알게 된다.
    /// </summary>
    [Theory]
    [InlineData("계약금액")]
    [InlineData("계약건명")]
    [InlineData("공고명")]     // 통합 뷰에서 부딪힌다
    public void 표가_이미_쓰는_이름은_막는다(string name)
    {
        var ex = Assert.Throws<InvalidOperationException>(
            () => _store.AddUserColumn("contract", name, "text", []));

        Assert.Contains(name, ex.Message);
    }

    [Fact]
    public void 이미_있는_열_이름은_막는다()
    {
        Assert.Throws<InvalidOperationException>(() => _store.AddUserColumn("contract", "메모", "text", []));
    }

    /// <summary>이름을 그대로 두고 종류만 고치는 것은 막지 않는다.</summary>
    [Fact]
    public void 제_이름_그대로_고치는_것은_막지_않는다()
    {
        _store.UpdateUserColumn("contract", "진행상태", "진행상태", "choice", ["준비", "종료"]);

        Assert.Equal(["준비", "종료"], _store.UserColumns("contract").Single(c => c.FieldName == "진행상태").Choices);
    }

    [Fact]
    public void 빈_이름과_너무_긴_이름은_막는다()
    {
        Assert.Throws<InvalidOperationException>(() => _store.AddUserColumn("contract", "   ", "text", []));
        Assert.Throws<InvalidOperationException>(
            () => _store.AddUserColumn("contract", new string('가', 21), "text", []));
    }

    [Fact]
    public void 모르는_종류는_막는다()
    {
        Assert.Throws<InvalidOperationException>(() => _store.AddUserColumn("contract", "담당", "색깔", []));
    }

    [Fact]
    public void 없는_열은_고치거나_지울_수_없다()
    {
        Assert.Throws<InvalidOperationException>(() => _store.RemoveUserColumn("contract", "없는열"));
        Assert.Throws<InvalidOperationException>(
            () => _store.UpdateUserColumn("contract", "없는열", "담당", "text", []));
    }

    /// <summary>공고 쪽은 따로 센다 — 같은 이름을 양쪽에 하나씩 두는 것은 막을 이유가 없다.</summary>
    [Fact]
    public void 계약과_공고는_따로_센다()
    {
        _store.AddUserColumn("contract", "검토여부", "text", []);

        Assert.Contains("검토여부", _store.UserColumns("contract").Select(c => c.FieldName));
        Assert.Contains("검토여부", _store.UserColumns("notice").Select(c => c.FieldName));
    }

    // ── 후보 목록 표기 ───────────────────────────────────
    // 세로줄로 나눈다. 개행으로 나누다 거짓 경고가 나던 자리라 시험으로 못 박는다.

    [Fact]
    public void 후보를_세로줄로_나눈다()
    {
        var column = new UserColumnDefinition("contract", "진행상태", "choice", "준비|투찰|낙찰|계약|납품", 10);

        Assert.Equal(["준비", "투찰", "낙찰", "계약", "납품"], column.Choices);
    }

    [Fact]
    public void 후보가_없으면_빈_목록()
    {
        Assert.Empty(new UserColumnDefinition("contract", "메모", "text", null, 10).Choices);
    }

    // ── 준비 ─────────────────────────────────────────────

    private static EntityRef Contract => new("contract", "R26TA09110507", "00");

    private void Put() => _store.UpsertContract(
        new ContractRecord
        {
            ContractBase = "R26TA09110507",
            Seq = "00",
            Title = "2026년 가람 수질측정기 조달",
            Amount = 164_872_340m,
        });

    private List<string> ViewColumns(string view)
    {
        using var connection = _database.OpenReadOnly();
        using var command = connection.CreateCommand();
        command.CommandText = $"PRAGMA table_info(\"{view}\");";

        var names = new List<string>();
        using var reader = command.ExecuteReader();
        while (reader.Read()) names.Add(reader.GetString(1));

        return names;
    }

    private string Cell(string column)
    {
        using var connection = _database.OpenReadOnly();
        using var command = connection.CreateCommand();
        command.CommandText = $"SELECT \"{column}\" FROM v_계약_v1;";

        return command.ExecuteScalar()?.ToString() ?? "";
    }

    public void Dispose()
    {
        SqliteConnection.ClearAllPools();
        foreach (var suffix in new[] { "", "-wal", "-shm" })
            File.Delete(_path + suffix);
        GC.SuppressFinalize(this);
    }
}

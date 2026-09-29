using Dapper;
using Microsoft.Data.Sqlite;
using Pclm.Core.Domain;
using Pclm.Core.Storage;
using Xunit;

namespace Pclm.Core.Tests;

/// <summary>
/// 관련공고를 <b>담되 판정하지 않는다</b>(ADR-025).
///
/// <para>지금까지 아는 축은 차수 하나였다. 변경공고는 그것으로 잡히지만 <b>취소 후 재공고는
/// 본번호가 갈린다</b> — <c>R26BK09011054</c> 가 취소되고 <c>R26BK09012082</c> 로 다시 나간다.
/// 차수로는 영영 이어지지 않고, 둘을 잇는 단서는 공고서의 「관련공고」 칸 하나뿐이다.</para>
///
/// <para>여기서 지키는 것은 <b>적힌 것이 적힌 그대로 나온다</b>는 것뿐이다. 이 값을 타고
/// "이 계열은 죽었다" 를 정하는 코드는 아직 없고, 그것은 이 표가 쌓인 뒤에야 근거를 갖는다.</para>
/// </summary>
public class NoticeRelationTests : IDisposable
{
    private readonly string _path = Path.Combine(
        Path.GetTempPath(), $"pclm-notice-rel-{Guid.NewGuid():N}.db");

    private readonly Database _database;
    private readonly Store _store;

    public NoticeRelationTests()
    {
        _database = new Database(_path);
        _database.Migrate();
        _store = new Store(_database);
    }

    /// <summary>
    /// 뷰는 <b>문서가 찍은 모양 그대로</b> 되돌린다 — 쉼표 뒤에 공백이 없다.
    /// 받는 쪽은 이 칸을 다시 손질하지 않는다.
    /// </summary>
    [Fact]
    public void 뷰가_적힌_그대로_낸다()
    {
        Put("R26BK09012082", "000", "R26BK09011054-001", "R26BK09011054-000");

        Assert.Equal("R26BK09011054-001,R26BK09011054-000", Notice()["관련공고"]);
    }

    /// <summary>없으면 <b>빈 문자열</b>이다. NULL 이 새면 받는 쪽의 빈 값 경고가 죽는다.</summary>
    [Fact]
    public void 없으면_빈_문자열이다()
    {
        Put("R26BK09011054", "000");

        Assert.Equal("", Notice()["관련공고"]);
    }

    /// <summary>
    /// <b>차수에 매단다.</b> 같은 계열이어도 차수마다 가리키는 것이 다르다 — 당초는 자기를
    /// 대신한 것을, 변경분은 자기가 대신한 것을 적는다(실측 <c>R26BK09017030</c>).
    ///
    /// <para>한 차수를 다시 넣어도 다른 차수의 것은 건드리지 않는다. 계열에 매달았다면
    /// 나중에 넣은 문서가 앞 차수의 관련공고를 조용히 덮었을 자리다.</para>
    /// </summary>
    [Fact]
    public void 차수마다_따로_매달린다()
    {
        Put("R26BK09017030", "000", "R26BK09017030-001");
        Put("R26BK09017030", "001", "R26BK09017030-000");

        Assert.Equal(["R26BK09017030-001"], Related("R26BK09017030", "000"));
        Assert.Equal(["R26BK09017030-000"], Related("R26BK09017030", "001"));

        // 변경분을 다시 넣는다 — 이번에는 관련공고가 지워진 판이다.
        Put("R26BK09017030", "001");

        Assert.Empty(Related("R26BK09017030", "001"));
        Assert.Equal(["R26BK09017030-001"], Related("R26BK09017030", "000"));
    }

    /// <summary>취소공고도 여느 공고처럼 줄을 갖는다 — 이 판은 계열의 생사를 정하지 않는다.</summary>
    [Fact]
    public void 취소공고도_여느_공고처럼_선다()
    {
        Put("R26BK09011054", "000");
        Put("R26BK09011054", "001", "R26BK09011054-000");
        Put("R26BK09012082", "000", "R26BK09011054-001", "R26BK09011054-000");

        var rows = Rows("v_공고_v1");

        // 계열 둘이 각각 최신 차수 하나씩. 취소되었다는 이유로 빠지지 않는다.
        Assert.Equal(2, rows.Count);
        Assert.Equal("R26BK09011054-001", rows[0]["입찰공고번호"]);
        Assert.Equal("취소공고", rows[0]["공고종류"]);
        Assert.Equal("R26BK09012082-000", rows[1]["입찰공고번호"]);
    }

    private void Put(string @base, string seq, params string[] related) =>
        _store.UpsertNotice(
            new NoticeRecord
            {
                NoticeBase = @base,
                Seq = seq,
                Title = "지어낸 공고",
                NoticeKind = seq == "001" ? "취소공고" : "등록공고",
                RelatedNotices = related,
            });

    private IReadOnlyList<string> Related(string @base, string seq)
    {
        using var connection = _database.OpenReadOnly();
        return
        [
            .. connection.Query<string>(
                "SELECT related FROM notice_relation WHERE notice_base = @b AND seq = @s ORDER BY line_no;",
                new { b = @base, s = seq }),
        ];
    }

    private Dictionary<string, string> Notice() => Rows("v_공고_v1")[0];

    private List<Dictionary<string, string>> Rows(string view)
    {
        using var connection = _database.OpenReadOnly();
        using var command = connection.CreateCommand();
        command.CommandText = $"SELECT * FROM \"{view}\" ORDER BY 1;";

        using var reader = command.ExecuteReader();
        var rows = new List<Dictionary<string, string>>();

        while (reader.Read())
        {
            var row = new Dictionary<string, string>();
            for (var i = 0; i < reader.FieldCount; i++)
                row[reader.GetName(i)] = reader.IsDBNull(i) ? "" : reader.GetValue(i).ToString() ?? "";
            rows.Add(row);
        }

        return rows;
    }

    public void Dispose()
    {
        SqliteConnection.ClearAllPools();
        GC.SuppressFinalize(this);
        if (File.Exists(_path)) File.Delete(_path);
    }
}

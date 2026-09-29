using Microsoft.Data.Sqlite;
using Pclm.Core.Domain;
using Pclm.Core.Storage;
using Xunit;

namespace Pclm.Core.Tests;

/// <summary>
/// 사람이 손에 쥔 번호로 접수를 찾는다.
///
/// <para>접수의 키는 조달청이 매긴 <b>접수번호</b>다(ADR-027). 수요기관은 자기 조달요구번호를
/// 들고 오므로, 접수번호를 알아야만 찾을 수 있다면 그 키는 절반만 쓸모가 있다.</para>
/// </summary>
public class RequestResolveTests : IDisposable
{
    private readonly string _path = Path.Combine(
        Path.GetTempPath(), $"pclm-resolve-{Guid.NewGuid():N}.db");

    private readonly Database _database;
    private readonly Store _store;

    public RequestResolveTests()
    {
        _database = new Database(_path);
        _database.Migrate();
        _store = new Store(_database);

        _store.UpsertRequest(new RequestRecord
        {
            RequestBase = "R26DC00000001",
            Seq = "000",
            Title = "26년 한별 윈치 8종 구매",
            Items =
            [
                new RequestItemRecord { LineNo = 1, RequestNumber = "MPKPLA26910286" },
                new RequestItemRecord { LineNo = 2, RequestNumber = "MPKPLA26910290" },
                new RequestItemRecord { LineNo = 3, RequestNumber = "MPKPLA26910295" },
            ],
        });
    }

    /// <summary>접수번호로 찾는다 — 있는 행과 글자 그대로 맞춘다.</summary>
    [Fact]
    public void 접수번호로_찾는다()
    {
        var found = _store.Resolve("R26DC00000001-000");

        Assert.NotNull(found);
        Assert.Equal("request", found.EntityType);
        Assert.Equal("R26DC00000001", found.Base);
        Assert.Equal("000", found.Seq);
    }

    /// <summary>접수번호가 아닌 <b>아무 요청번호</b>로도 찾는다. 그 계열의 최신 차수를 준다.</summary>
    [Theory]
    [InlineData("MPKPLA26910286")]
    [InlineData("MPKPLA26910290")]
    [InlineData("MPKPLA26910295")]
    public void 아무_요청번호로도_찾는다(string key)
    {
        var found = _store.Resolve(key);

        Assert.NotNull(found);
        Assert.Equal("request", found.EntityType);
        Assert.Equal("R26DC00000001-000", found.Display);
    }

    /// <summary>차수가 오르면 최신 것을 준다 — 사람이 가리킨 것은 조달요구이지 특정 차수가 아니다.</summary>
    [Fact]
    public void 요청번호는_최신_차수를_준다()
    {
        _store.UpsertRequest(new RequestRecord
        {
            RequestBase = "R26DC00000001",
            Seq = "001",
            Items = [new RequestItemRecord { LineNo = 1, RequestNumber = "MPKPLA26910290" }],
        });

        Assert.Equal("R26DC00000001-001", _store.Resolve("MPKPLA26910290")!.Display);
    }

    [Fact]
    public void 모르는_번호는_찾지_못한다() => Assert.Null(_store.Resolve("MPKPLA99999999"));

    public void Dispose()
    {
        SqliteConnection.ClearAllPools();
        foreach (var suffix in new[] { "", "-wal", "-shm" })
            File.Delete(_path + suffix);
        GC.SuppressFinalize(this);
    }
}

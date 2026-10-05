using Microsoft.Data.Sqlite;
using Pclm.Core.Storage;

namespace Pclm.Core.Tests;

/// <summary>
/// 스키마 기준선 — 빈 파일을 v19 로 <b>곧장</b> 짓는 한 벌(<see cref="Schema.Baseline"/>).
///
/// <para>박제(<c>contract/schema.txt</c>)는 옛 열아홉 단계를 밟아 지은 v19 에서 떴다. 기준선이 그와
/// <b>한 글자라도</b> 다른 표를 지으면, 단계를 밟아 올라온 파일과 갓 지은 파일이 같은 판 번호를 달고
/// 서로 다른 스키마를 갖게 된다 — 어느 쪽도 오류를 내지 않는다. 뷰는 <c>contract/views.txt</c> 가
/// 따로 본다.</para>
///
/// <para>일부러 바꿀 일은 없다. 스키마를 바꾸려면 기준선을 고치는 것이 아니라
/// <see cref="Schema.Steps"/> 에 단계를 덧붙이고, 그때 박제를 눈으로 본 뒤 새로 쓴다. 지금 박제는
/// 기준선에 v20 단계(<c>erp_dataset</c> → <c>pclm_file</c>)를 더한 모습이다 — 갓 지은 파일도 그 단계를 밟는다.</para>
/// </summary>
public sealed class SchemaBaselineTests : IDisposable
{
    private static readonly string GoldenPath =
        Path.Combine(FindUpwards("tests") ?? ".", "Pclm.Core.Tests", "contract", "schema.txt");

    private readonly string _path = Path.Combine(Path.GetTempPath(), $"pclm-baseline-{Guid.NewGuid():N}.db");

    public void Dispose()
    {
        SqliteConnection.ClearAllPools();
        foreach (var suffix in new[] { "", "-wal", "-shm" })
            File.Delete(_path + suffix);
    }

    [Fact]
    public void 새로_지은_스키마가_박제와_같다()
    {
        var database = PclmFile.Create(_path, PclmRole.Work);

        string actual;
        using (var connection = database.OpenReadOnly())
            actual = SchemaDump.Of(connection);

        Assert.True(File.Exists(GoldenPath), $"스키마 박제가 없습니다: {GoldenPath}");
        // 박제 쪽만 줄바꿈을 고른다. 지은 쪽에 CR 이 섞이면 그것은 기준선 원문이 달라진 것이다.
        Assert.Equal(File.ReadAllText(GoldenPath).Replace("\r\n", "\n"), actual);
    }

    [Fact]
    public void 판은_기준선에_단계를_더한_것이다()
    {
        Assert.Equal(Schema.Version, Schema.BaselineVersion + Schema.Steps.Count);
    }

    /// <summary>
    /// 기준선보다 옛 시험판은 <b>한 바이트도 고치지 않고</b> 거절한다. 반쯤 손댄 채 실패하면
    /// 0.7.0 으로 열어 올리는 길마저 흐려진다.
    /// </summary>
    [Theory]
    [MemberData(nameof(옛판들))]
    public void 옛_시험판은_고치지_않고_거절한다(int version)
    {
        using (var connection = new SqliteConnection($"Data Source={_path};Pooling=False"))
        {
            connection.Open();
            using var command = connection.CreateCommand();
            command.CommandText = $"CREATE TABLE notice (notice_base TEXT, seq TEXT); PRAGMA user_version = {version};";
            command.ExecuteNonQuery();
        }

        var before = File.ReadAllBytes(_path);

        var error = Assert.Throws<InvalidOperationException>(() => new Database(_path).Migrate());
        Assert.Contains($"v{version}", error.Message);
        Assert.Contains("0.7.0", error.Message);

        SqliteConnection.ClearAllPools();
        Assert.Equal(before, File.ReadAllBytes(_path));
        Assert.False(File.Exists(_path + "-wal"));
    }

    public static IEnumerable<object[]> 옛판들() =>
        Enumerable.Range(1, Schema.BaselineVersion - 1).Select(v => new object[] { v });

    private static string? FindUpwards(string name)
    {
        for (var directory = new DirectoryInfo(AppContext.BaseDirectory);
             directory is not null;
             directory = directory.Parent)
        {
            var candidate = Path.Combine(directory.FullName, name);
            if (Directory.Exists(candidate)) return candidate;
        }

        return null;
    }
}

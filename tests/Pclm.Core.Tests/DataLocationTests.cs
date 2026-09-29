using Microsoft.Data.Sqlite;
using Pclm.Core.Storage;
using Xunit;

namespace Pclm.Core.Tests;

/// <summary>
/// 자료가 쌓이는 자리를 옮기는 길.
///
/// <para>여기서 붙드는 것은 편의가 아니라 <b>조용한 유실</b>이다. 자리를 고르는 순간에 복사하면,
/// 고른 뒤 재시작 전까지의 편집이 옛 자리에만 쌓이고 다음 실행은 고른 시점의 낡은 사진을
/// 연다 — 며칠치가 오류 하나 없이 사라진다. 그래서 복사는 <b>다음 실행 맨 앞</b>에 하고,
/// 그 순서를 시험이 붙든다.</para>
/// </summary>
public class DataLocationTests : IDisposable
{
    private readonly string _root = Path.Combine(
        Path.GetTempPath(), "pclm-loc-" + Guid.NewGuid().ToString("N")[..8]);

    private string Config => Path.Combine(_root, "config.json");
    private string Dir(string name) => Path.Combine(_root, name);

    public DataLocationTests() => Directory.CreateDirectory(_root);

    public void Dispose()
    {
        // 연결 풀이 파일을 쥐고 있으면 임시 폴더가 지워지지 않는다.
        SqliteConnection.ClearAllPools();
        try { Directory.Delete(_root, recursive: true); } catch { /* 지우지 못해도 시험 결과는 그대로다 */ }
        GC.SuppressFinalize(this);
    }

    /// <summary>공고 하나를 넣어 둔 DB 를 그 폴더에 세운다.</summary>
    private string Seed(string directory)
    {
        var path = DataLocation.DbIn(directory);
        var database = new Database(path);
        database.Migrate();

        using var connection = database.Open();
        using var command = connection.CreateCommand();
        command.CommandText =
            "INSERT INTO notice (notice_base, seq, title, updated_at) VALUES ('N-1', 0, '표식', '2026-08-30');";
        command.ExecuteNonQuery();
        return path;
    }

    private static bool HasMark(string dbPath)
    {
        using var connection = new Database(dbPath).Open();
        using var command = connection.CreateCommand();
        command.CommandText = "SELECT COUNT(*) FROM notice WHERE title = '표식';";
        return Convert.ToInt32(command.ExecuteScalar()) == 1;
    }

    [Fact]
    public void 쪽지가_없으면_기본_자리다()
    {
        var resolved = DataLocation.Resolve(Config);

        Assert.True(resolved.IsDefault);
        Assert.Equal(Database.DefaultPath, resolved.DbPath, ignoreCase: true);
        Assert.Null(resolved.Note);
    }

    [Fact]
    public void 쪽지가_깨져_있어도_기본_자리로_뜬다()
    {
        // 쪽지 하나 때문에 앱이 못 뜨면 되돌릴 길이 화면에 없다.
        File.WriteAllText(Config, "{ 이건 JSON 이 아니다");

        var resolved = DataLocation.Resolve(Config);

        Assert.True(resolved.IsDefault);
    }

    /// <summary>이 저장소가 옮겨 온 설계의 핵심. 고를 때가 아니라 <b>다음 실행에</b> 옮긴다.</summary>
    [Fact]
    public void 이사는_고를_때가_아니라_다음_실행에_치러진다()
    {
        var old = Dir("old");
        Seed(old);

        DataLocation.StageMove(DataLocation.DbIn(old), Dir("new"), Config);

        // 적어 두기만 했을 뿐, 아직 아무것도 옮기지 않았다.
        Assert.False(File.Exists(DataLocation.DbIn(Dir("new"))));

        // 고른 뒤 재시작 전에도 사람은 계속 일한다. 그 편집이 옛 자리에 쌓인다.
        using (var connection = new Database(DataLocation.DbIn(old)).Open())
        {
            using var command = connection.CreateCommand();
            command.CommandText =
                "INSERT INTO notice (notice_base, seq, title, updated_at) VALUES ('N-2', 0, '고른 뒤에 적은 것', '2026-08-30');";
            command.ExecuteNonQuery();
        }

        var resolved = DataLocation.Resolve(Config);

        // 다음 실행이 옮긴 것은 고른 시점의 사진이 아니라 그 뒤의 편집까지 담긴 지금 자료다.
        Assert.Equal(DataLocation.DbIn(Dir("new")), resolved.DbPath);
        using var moved = new Database(resolved.DbPath).Open();
        using var check = moved.CreateCommand();
        check.CommandText = "SELECT COUNT(*) FROM notice;";
        Assert.Equal(2, Convert.ToInt32(check.ExecuteScalar()));
    }

    [Fact]
    public void 옮긴_뒤에도_옛_자리는_남는다()
    {
        var old = Dir("old");
        Seed(old);
        DataLocation.StageMove(DataLocation.DbIn(old), Dir("new"), Config);

        DataLocation.Resolve(Config);

        // 이사가 잘못됐을 때 돌아갈 자리가 된다.
        Assert.True(HasMark(DataLocation.DbIn(old)));
    }

    [Fact]
    public void 이사는_한_번만_치러진다()
    {
        var old = Dir("old");
        Seed(old);
        DataLocation.StageMove(DataLocation.DbIn(old), Dir("new"), Config);

        var first = DataLocation.Resolve(Config);
        Assert.NotNull(first.Note);

        // 두 번째 실행은 옮길 것이 없다 — 남아 있으면 새 자리를 옛 사진으로 덮는다.
        var second = DataLocation.Resolve(Config);
        Assert.Null(second.Note);
        Assert.Equal(first.DbPath, second.DbPath);
    }

    [Fact]
    public void 옮기지_못하면_옛_자리를_계속_쓴다()
    {
        // 옮길 원본이 없는 자리를 가리켜 이사를 실패시킨다 (드라이브가 빠진 경우와 같다).
        var old = Dir("old");
        DataLocation.Save(new DataLocationConfig { DataDir = Dir("new"), PendingMoveFrom = old }, Config);

        var resolved = DataLocation.Resolve(Config);

        // 초기화되지 않은 빈 새 자리에서 뜨느니 옛 자리 위에서 도는 쪽이 낫다.
        Assert.Equal(DataLocation.DbIn(old), resolved.DbPath);
        Assert.Contains("기존 자리를 계속", resolved.Note);

        // 쪽지도 함께 되돌아가야 다음 실행이 또 빈 자리를 열지 않는다.
        Assert.Equal(old, DataLocation.Load(Config).DataDir);
    }

    [Fact]
    public void 이미_자료가_있는_자리로는_옮기지_않는다()
    {
        var busy = Dir("busy");
        Seed(busy);

        // 막는 자리가 여기여야 한다 — 적어 둔 뒤에 알면 다음 실행이 남의 자료를 덮는다.
        var why = DataLocation.WhyNotMoveTo(busy);

        Assert.NotNull(why);
        Assert.Contains("이미 계약 목록 자료가 있습니다", why);
    }

    [Fact]
    public void 빈_자리는_받아들인다()
    {
        Assert.Null(DataLocation.WhyNotMoveTo(Dir("비어있음")));
    }
}

using Microsoft.Data.Sqlite;
using Pclm.Core.Storage;
using Xunit;

namespace Pclm.Core.Tests;

/// <summary>
/// DB 안에 적어 두는 설정 — 제출자 이름과 골라 둔 계획 엑셀.
///
/// <para>여기서 가장 조심하는 것은 <b>그 값을 모르는 저장이 그것을 지우는 일</b>이다.</para>
/// </summary>
public class SettingsTests : IDisposable
{
    private readonly string _path = Path.Combine(
        Path.GetTempPath(), $"pclm-settings-{Guid.NewGuid():N}.db");

    private readonly string _folder = Path.Combine(
        Path.GetTempPath(), $"pclm-plan-{Guid.NewGuid():N}");

    private readonly Database _database;
    private readonly SettingsStore _settings;

    public SettingsTests()
    {
        _database = PclmFile.Create(_path, PclmRole.Work);
        _settings = new SettingsStore(_database);
    }

    /// <summary>
    /// 제출자 이름은 <b>제출본 파일 이름에만</b> 쓰는 값이다(ADR-023). 그 이름을 모르는
    /// 부르는 쪽이 저장할 때마다 조용히 지워지면, 사람은 이름을 적어 둔 줄 알고 이름 없는
    /// 제출본을 보낸다 — 그래서 <c>null</c> 은 "그대로 두라" 는 뜻이다.
    /// </summary>
    [Fact]
    public void 제출자_이름은_주지_않으면_지워지지_않는다()
    {
        _settings.Save(submitterName: "홍길동");
        Assert.Equal("홍길동", _settings.Read().SubmitterName);

        _settings.Save();
        Assert.Equal("홍길동", _settings.Read().SubmitterName);
    }

    /// <summary>한 번도 고른 적이 없으면 빈 문자열이다 — <c>null</c> 이 아니다.</summary>
    [Fact]
    public void 고른_계획_엑셀이_없으면_빈_문자열이다()
    {
        Assert.Equal("", _settings.Read().PlanPath);
    }

    /// <summary>
    /// 고른 자리를 적어 둔다 — 서식이 표본에 고정된 뒤로 계획 가져오기에 남은 결정은
    /// <b>어느 파일인가</b> 하나뿐이라, 경로가 곧 링크다(ADR-023 개정).
    /// </summary>
    [Fact]
    public void 고른_계획_엑셀의_자리가_그대로_돌아온다()
    {
        var excel = Path.Combine(_folder, "조달계획.xlsx");

        Assert.Equal(excel, _settings.SavePlanPath(excel).PlanPath);
        Assert.Equal(excel, _settings.Read().PlanPath);
    }

    /// <summary>
    /// 다른 설정을 저장할 때 골라 둔 엑셀이 조용히 지워지면 안 된다 — 제출자 이름과 같은
    /// 규율이다. 지우려면 빈 문자열을 준다.
    /// </summary>
    [Fact]
    public void 계획_엑셀을_모르는_저장은_그것을_건드리지_않는다()
    {
        var excel = Path.Combine(_folder, "조달계획.xlsx");
        _settings.SavePlanPath(excel);

        _settings.Save(submitterName: "홍길동");

        Assert.Equal(excel, _settings.Read().PlanPath);

        _settings.Save(planPath: "");

        Assert.Equal("", _settings.Read().PlanPath);
    }

    public void Dispose()
    {
        SqliteConnection.ClearAllPools();
        foreach (var suffix in new[] { "", "-wal", "-shm" })
            File.Delete(_path + suffix);

        GC.SuppressFinalize(this);
    }
}

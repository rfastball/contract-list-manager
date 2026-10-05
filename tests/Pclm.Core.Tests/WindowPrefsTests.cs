using Pclm.Core.Storage;
using Xunit;

namespace Pclm.Core.Tests;

/// <summary>
/// 창의 몸가짐(<c>window.json</c>). 읽기가 <b>어떤 꼴에도 넘어지지 않고</b> 기본값(모두 꺼짐)으로 서는지,
/// 쓴 것이 그대로 돌아오는지를 본다. 넘어지면 창이 뜨지 못하고, 기본값이 켜짐 쪽으로 새면 사람이 켠 적 없는
/// 몸가짐이 선다.
///
/// <para>홈은 모두 임시 폴더다.</para>
/// </summary>
public sealed class WindowPrefsTests : IDisposable
{
    private readonly string _root = Path.Combine(Path.GetTempPath(), "pclm-window-" + Guid.NewGuid().ToString("N")[..8]);
    private readonly Home _home;

    public WindowPrefsTests()
    {
        Directory.CreateDirectory(_root);
        _home = new Home(Path.Combine(_root, "홈"));
    }

    public void Dispose()
    {
        try { Directory.Delete(_root, recursive: true); } catch { /* 지우지 못해도 시험 결과는 그대로다 */ }
    }

    [Fact]
    public void 파일이_없으면_모두_꺼짐이다()
    {
        Assert.Equal(new WindowPrefs(false, false), WindowPrefs.Read(_home.WindowPrefsPath));
        Assert.False(File.Exists(_home.WindowPrefsPath));   // 읽기만으로 짓지 않는다
    }

    [Theory]
    [InlineData("")]
    [InlineData("{ 깨진")]
    [InlineData("null")]
    [InlineData("[]")]
    [InlineData("{\"closeToTray\": \"yes\"}")]
    public void 깨졌으면_넘어지지_않고_모두_꺼짐이다(string text)
    {
        Directory.CreateDirectory(_home.Directory);
        File.WriteAllText(_home.WindowPrefsPath, text);

        Assert.Equal(new WindowPrefs(), WindowPrefs.Read(_home.WindowPrefsPath));
    }

    [Fact]
    public void 쓴_것이_그대로_돌아오고_임시_파일을_남기지_않는다()
    {
        new WindowPrefs(CloseToTray: true, TrayNoticeShown: true).Write(_home.WindowPrefsPath);
        Assert.Equal(new WindowPrefs(true, true), WindowPrefs.Read(_home.WindowPrefsPath));

        new WindowPrefs(CloseToTray: false, TrayNoticeShown: true).Write(_home.WindowPrefsPath);
        Assert.Equal(new WindowPrefs(false, true), WindowPrefs.Read(_home.WindowPrefsPath));

        Assert.Equal(["window.json"], Directory.GetFiles(_home.Directory).Select(Path.GetFileName));
    }

    [Fact]
    public void 모르는_칸은_넘기고_빠진_칸은_꺼짐이다()
    {
        Directory.CreateDirectory(_home.Directory);
        File.WriteAllText(_home.WindowPrefsPath, "{\"closeToTray\": true, \"뒷날\": 1}");

        Assert.Equal(new WindowPrefs(true, false), WindowPrefs.Read(_home.WindowPrefsPath));
    }

    /// <summary>창을 불러내는 신호는 잠금처럼 홈마다 하나다 — 표기가 달라도 같은 홈이면 같은 이름이다.</summary>
    [Fact]
    public void 불러내기_신호는_홈마다_하나다()
    {
        var same = new Home(_home.Directory.ToUpperInvariant() + Path.DirectorySeparatorChar);
        var other = new Home(Path.Combine(_root, "다른 홈"));

        Assert.Equal(_home.ActivationName, same.ActivationName);
        Assert.NotEqual(_home.ActivationName, other.ActivationName);
        Assert.StartsWith(@"Local\", _home.ActivationName);
    }
}

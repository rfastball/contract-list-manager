using Pclm.Cli;

namespace Pclm.Core.Tests;

public class CliArgumentsTests
{
    [Theory]
    [InlineData("fix")]
    [InlineData("set")]
    public void 모르는_옵션을_값누락으로_넘기지_않는다(string command)
    {
        var error = Assert.Throws<ArgumentException>(() => CliArguments.Parse([command, "문서번호", "열", "--긴급"]));
        Assert.Contains("--긴급", error.Message);
    }

    [Theory]
    [InlineData("fix")]
    [InlineData("set")]
    public void 종료표식_뒤에는_옵션도_입력값이다(string command)
    {
        var parsed = CliArguments.Parse([command, "--home", "시험홈", "문서번호", "열", "--", "--긴급", "--예"]);
        Assert.Equal("시험홈", parsed.Option("--home"));
        Assert.Equal(["문서번호", "열", "--긴급", "--예"], parsed.Positionals);
        Assert.False(parsed.Flag("--예"));
    }

    [Theory]
    [InlineData("--home")]
    [InlineData("--file")]
    [InlineData("--out")]
    [InlineData("--이름")]
    public void 옵션값이_없거나_다음옵션이면_거절한다(string option)
    {
        Assert.Throws<ArgumentException>(() => CliArguments.Parse(["merge", option]));
        Assert.Throws<ArgumentException>(() => CliArguments.Parse(["merge", option, "--예"]));
    }

    [Fact]
    public void 알려진_옵션과_깃발은_자리인자에서_분리한다()
    {
        var parsed = CliArguments.Parse(["merge", "제출본1", "--home", "시험홈", "--file", "열람본",
            "--out", "취합본", "--이름", "담당자", "--계열", "--예", "제출본2"]);
        Assert.Equal(["제출본1", "제출본2"], parsed.Positionals);
        Assert.Equal("시험홈", parsed.Option("--home"));
        Assert.Equal("열람본", parsed.Option("--file"));
        Assert.Equal("취합본", parsed.Option("--out"));
        Assert.Equal("담당자", parsed.Option("--이름"));
        Assert.True(parsed.Flag("--계열"));
        Assert.True(parsed.Flag("--예"));
        Assert.Null(parsed.Option("--없는옵션"));
    }

    [Fact]
    public void 없어진_db옵션은_실제홈을_열기전에_거절한다() =>
        Assert.Contains("--db 는 없어졌습니다", Assert.Throws<ArgumentException>(() => CliArguments.Parse(["status", "--db", "시험자료"])).Message);
}

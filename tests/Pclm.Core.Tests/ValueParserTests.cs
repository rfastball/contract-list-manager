using Pclm.Core.Domain;
using Xunit;

namespace Pclm.Core.Tests;

/// <summary>
/// 값 변환의 경계. 여기서는 <b>못 읽으면 null</b> 이라는 규약을 특히 지킨다. 0 으로 얼버무리면
/// "0원짜리 계약"과 "금액을 못 읽은 계약"이 구별되지 않는다.
/// </summary>
public class ValueParserTests
{
    [Theory]
    [InlineData("164,872,340 원", "164872340")]
    [InlineData("54,957,446.667", "54957446.667")]
    [InlineData("196,500,000", "196500000")]
    [InlineData("0 원", "0")]
    public void 금액을_읽는다(string input, string expected) =>
        Assert.Equal(decimal.Parse(expected), ValueParser.Money(input));

    [Theory]
    [InlineData("")]
    [InlineData("   ")]
    [InlineData("해당없음")]
    [InlineData("-")]
    public void 금액이_아니면_null(string input) => Assert.Null(ValueParser.Money(input));

    [Fact]
    public void 공고번호는_붙임표로_차수를_가른다()
    {
        var parsed = ValueParser.NoticeNumber("R26BK09017075 - 000");
        Assert.Equal(("R26BK09017075", "000"), parsed);
    }

    [Fact]
    public void 계약번호는_끝_두_자리가_차수다()
    {
        var parsed = ValueParser.ContractNumber("R26TA0911050700");
        Assert.Equal(("R26TA09110507", "00"), parsed);
    }

    /// <summary>
    /// 번호처럼 생기지 않았으면 <c>null</c> 이다.
    ///
    /// <para>이 가드가 없을 때 계약번호는 <b>세 자 이상이면 무엇이든</b> 통과해
    /// 끝 두 자를 차수로 잘랐다. <c>계약번호</c>·<c>계약금액</c> 라벨이 함께 있는
    /// 사내 품의서가 들어오면 <c>연구개발용역</c> 같은 값이 그대로 자연키가 되어
    /// <c>contract</c> 에 <b>조용히 정상 행</b>으로 앉았다 — 남은 유일한 오염 경로였다.</para>
    /// </summary>
    [Theory]
    [InlineData("연구개발용역")]
    [InlineData("계약서 사본")]
    [InlineData("R26TA 091105 00")]
    [InlineData("R26TA091105AB")]
    [InlineData("--")]
    public void 계약번호로_보이지_않으면_번호가_아니다(string input) =>
        Assert.Null(ValueParser.ContractNumber(input));

    [Theory]
    [InlineData("가-나")]
    [InlineData("R26BK09017075 - 변경")]
    [InlineData("R26BK 090170 - 000")]
    public void 공고번호도_같은_잣대를_쓴다(string input) =>
        Assert.Null(ValueParser.NoticeNumber(input));

    /// <summary>
    /// 좌표를 그보다 더 좀히지는 않는다. 지자체·수의계약의 변형을 모르는 채로
    /// 자릿수나 접두사를 박으면 <b>진짜 문서를 튕긴다</b> — 잃는 쪽이 더 크다.
    /// </summary>
    [Theory]
    [InlineData("12300", "123", "00")]
    [InlineData("abc99", "abc", "99")]
    public void 자릿수나_접두사는_따지지_않는다(string input, string @base, string seq) =>
        Assert.Equal((@base, seq), ValueParser.ContractNumber(input));
}

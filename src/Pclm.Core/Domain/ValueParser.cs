using System.Globalization;

namespace Pclm.Core.Domain;

/// <summary>
/// 사람이 친 키와 적힌 문자열을 값으로 바꾼다.
///
/// <para>못 읽으면 <c>null</c>을 돌려준다. 0이나 기본값으로 얼버무리면 "값이 0원인 계약"과
/// "금액을 못 읽은 계약"이 구별되지 않고, 그 차이는 나중에 문서에 그대로 찍힌다.</para>
/// </summary>
public static class ValueParser
{
    /// <summary>금액. <c>164,872,340 원</c> · <c>54,957,446.667</c> 처럼 단위와 자릿점이 섞여 온다.</summary>
    public static decimal? Money(string? text)
    {
        var digits = Digits(text, allowDecimal: true);
        return digits is null ? null : decimal.Parse(digits, CultureInfo.InvariantCulture);
    }

    /// <summary>정수. <c>3</c> · <c>3 대</c>.</summary>
    public static int? Integer(string? text)
    {
        var digits = Digits(text, allowDecimal: false);
        return digits is null ? null : int.Parse(digits, CultureInfo.InvariantCulture);
    }

    /// <summary>
    /// 공고번호 <c>R26BK09017075 - 000</c> 을 본번호와 차수로 가른다.
    /// 계약번호 <c>R26TA0911050700</c> 은 끝 두 자리가 차수다.
    ///
    /// <para><b>번호처럼 생기지 않았으면 번호가 아니다.</b> 사람이 친 키(<c>Store.Resolve</c>)를
    /// 실제 행에 맞출 때 쓴다 — 여기는 검산이 아니라 <b>어느 자리를 가리키느냐</b>를 가리는
    /// 자리다(ADR-016).</para>
    ///
    /// <para>자릿수와 접두사는 <b>박지 않는다</b>. 지자체·수의계약의 변형을 모르는 채로 좁히면
    /// 진짜 번호를 튕긴다. 막는 것은 번호 자리에 <b>번호가 아닌 것</b>이 앉은 경우뿐이다.</para>
    /// </summary>
    public static (string Base, string Seq)? NoticeNumber(string? text)
    {
        if (string.IsNullOrWhiteSpace(text)) return null;

        var parts = text.Split('-', StringSplitOptions.TrimEntries | StringSplitOptions.RemoveEmptyEntries);
        if (parts.Length < 2) return null;

        return Checked(parts[0], parts[1]);
    }

    /// <inheritdoc cref="NoticeNumber"/>
    public static (string Base, string Seq)? ContractNumber(string? text)
    {
        var t = text?.Trim();
        if (string.IsNullOrWhiteSpace(t) || t.Length < 3) return null;

        return Checked(t[..^2], t[^2..]);
    }

    /// <summary>
    /// 관련공고 한 칸을 <b>본번호와 차수로 가른다</b>. <c>R26BK09011054-001</c> →
    /// <c>("R26BK09011054", "001")</c>. 붙임표가 없거나 어느 한쪽이 비면 <c>null</c>.
    ///
    /// <para><b>가르는 규칙을 여기 하나에만 둔다.</b> 이 값으로 공고건을 짓는 쪽
    /// (<c>NoticeGroups.Rebuild</c>)과 담는 쪽이 같은 함수를 부르고,
    /// 뷰는 이미 갈라 둔 열(<c>related_base</c>·<c>related_seq</c>)만 견준다 — SQL 에 문자열
    /// 산술이 한 줄도 없다. 규칙이 두 벌이면 문서가 조금만 달리 찍혀도 둘이 말없이 갈린다.</para>
    ///
    /// <para><see cref="NoticeNumber"/> 와 달리 <see cref="Checked"/> 를 지나지 않는다. 이쪽은
    /// 적힌 그대로 담아 둔 것을 <b>이을 수 있는지</b> 보는 자리다 — 갈라지지 않는 값은
    /// <c>null</c> 로 두고 <c>related</c> 에 그대로 남아 사람 눈에 온다.</para>
    /// </summary>
    public static (string Base, string Seq)? SplitNoticeRef(string? text)
    {
        var t = text?.Trim();
        if (string.IsNullOrEmpty(t)) return null;

        var mark = t.IndexOf('-');
        if (mark < 0) return null;

        var @base = t[..mark].Trim();
        var seq = t[(mark + 1)..].Trim();

        return @base.Length == 0 || seq.Length == 0 ? null : (@base, seq);
    }

    /// <summary>
    /// 본번호는 ASCII 영숫자만, 차수는 숫자만. 어느 하나라도 어긋나면 <c>null</c>.
    ///
    /// <para>공고 <c>R26BK09017075-000</c>·계약 <c>R26TA0911050700</c> 이 이 검사를 그대로
    /// 지난다. 실측으로 확인한 범위가 그만큼이라 그보다 좁히지 않는다.</para>
    /// </summary>
    private static (string Base, string Seq)? Checked(string @base, string seq)
    {
        if (@base.Length == 0 || seq.Length == 0) return null;
        if (!@base.All(char.IsAsciiLetterOrDigit)) return null;
        if (!seq.All(char.IsAsciiDigit)) return null;

        return (@base, seq);
    }

    private static string? Digits(string? text, bool allowDecimal)
    {
        if (string.IsNullOrWhiteSpace(text)) return null;

        Span<char> buffer = stackalloc char[text.Length + 1];
        var n = 0;
        var seenDot = false;

        foreach (var ch in text)
        {
            if (char.IsAsciiDigit(ch)) buffer[n++] = ch;
            else if (ch == '-' && n == 0) buffer[n++] = ch;
            else if (ch == '.' && allowDecimal && !seenDot && n > 0) { buffer[n++] = ch; seenDot = true; }
            else if (ch is ',' or ' ') continue;
            else break; // 숫자 뒤에 붙은 단위에서 멈춘다
        }

        var digits = new string(buffer[..n]);
        return digits.Any(char.IsAsciiDigit) ? digits.TrimEnd('.') : null;
    }
}

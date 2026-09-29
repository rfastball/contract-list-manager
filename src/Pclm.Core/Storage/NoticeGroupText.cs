using System.Text;

namespace Pclm.Core.Storage;

/// <summary>
/// 건을 다시 지으며 <b>밀어낸 것</b>을 글로 짓는다.
///
/// <para><b>글이 한 벌인 까닭</b>은 <see cref="MergeReportText"/> 와 같다 — 창은 상자로 띄우고
/// 명령줄은 화면에 찍는데, 같은 판올림을 두 길로 겪은 사람이 서로 다른 글을 읽으면 어느 것이
/// 진짜인지 가릴 수 없다. 두 벌로 두면 한쪽이 조용히 늙는다.</para>
///
/// <para><b>이은 것만으로는 아무 말도 하지 않는다.</b> 갈렸던 본번호를 잇는 것은 이 판올림이
/// 하러 온 일이라 알릴 거리가 아니다. 알리는 것은 <b>사람이 적어 둔 것이 밀려났을 때</b>뿐이고,
/// 그 자리는 오류를 내지 않으므로 이 글이 아니면 아무도 알아채지 못한다.</para>
/// </summary>
public static class NoticeGroupText
{
    /// <summary>
    /// 알릴 것이 있으면 글을, 없으면 <c>null</c>.
    ///
    /// <para>부르는 쪽은 <c>is { } 알림</c> 으로 받아 <b>그때만</b> 사람 앞에 낸다. 아무 일도
    /// 없었는데 상자가 뜨면 다음에 진짜 뜬 상자도 읽히지 않고 닫힌다.</para>
    /// </summary>
    public static string? Render(NoticeGroupChange? change)
    {
        if (change is null || !change.있나) return null;

        var text = new StringBuilder();
        text.AppendLine("공고건을 다시 지었습니다");

        // 이은 것은 알릴 거리가 아니지만, 밀린 까닭을 짚어 주는 자리라 함께 적는다 —
        // 무엇 때문에 이런 일이 생겼는지가 없으면 밀렸다는 말만 뜬금없이 선다.
        if (change.이은건수 > 0)
            text.AppendLine($"  취소·재공고로 갈렸던 본번호가 공고건 {change.이은건수}개로 이어졌습니다.");

        // 빈 구역은 통째로 뺀다. 없는 일을 적어 두면 실제로 알릴 것이 있는 줄이 거기 묻힌다.
        if (change.밀린접수.Count > 0)
        {
            text.AppendLine();
            text.AppendLine($"  밀린 접수 {change.밀린접수.Count}건 — 한 공고건에 접수는 하나입니다");
            foreach (var line in change.밀린접수) text.AppendLine($"    {line}");
            text.AppendLine("    밀린 쪽이 맞다면 잇기 화면에서 다시 이으세요.");
        }

        if (change.끊긴링크.Count > 0)
        {
            text.AppendLine();
            text.AppendLine($"  끊긴 링크 {change.끊긴링크.Count}건 — 가리키던 공고건이 남지 않았습니다");
            foreach (var line in change.끊긴링크) text.AppendLine($"    {line}");
        }

        return text.ToString();
    }
}

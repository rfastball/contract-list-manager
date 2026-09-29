using System.Globalization;
using System.Text;

namespace Pclm.Core.Storage;

/// <summary>
/// 취합 한 번의 보고를 글로 짓는다.
///
/// <para><b>글이 한 벌인 까닭.</b> 명령줄은 화면에 찍고 창은 취합본 옆에 파일로 남기는데,
/// 같은 취합을 두 길로 한 사람이 서로 다른 글을 읽으면 어느 것이 진짜인지 가릴 수 없다.
/// 두 벌로 두면 한쪽이 조용히 늙는다 — 그래서 짓는 자리를 하나로 둔다.</para>
/// </summary>
public static class MergeReportText
{
    /// <summary>
    /// 취합 한 번의 보고. <b>알림이 본체다</b> — 무엇이 누구와 누구에게서 왔고 어느 것을
    /// 실었는지 한 줄씩 적는다. 마지막은 줄바꿈 하나로 닫는다.
    /// </summary>
    public static string Render(MergeReport report, string 취합본경로)
    {
        var text = new StringBuilder();

        text.AppendLine("취합 결과");
        text.AppendLine(
            $"  제출본 {report.제출본}개 · 조달요구 {report.조달요구} · 접수 {report.접수}" +
            $" · 공고 {report.공고} · 계약 {report.계약}");

        // 빈 구역은 통째로 뺀다. "거절 없음" 은 적을 말이 아니라 없는 일이라, 적어 두면
        // 실제로 알릴 것이 있는 줄이 늘 있는 줄에 묻힌다.
        if (report.거절한제출본.Count > 0)
        {
            text.AppendLine();
            text.AppendLine($"  받지 않은 제출본 {report.거절한제출본.Count}개 — 판이 이 프로그램보다 새롭습니다");
            foreach (var rejected in report.거절한제출본) text.AppendLine($"    {rejected}");
            text.AppendLine("    새 판으로 지은 자료는 내릴 길이 없습니다. 이 프로그램을 새로 받아 다시 합치세요.");
        }

        if (report.겹친것.Count > 0)
        {
            text.AppendLine();
            text.AppendLine($"  겹친 것 {report.겹친것.Count}건 — 늦은 것을 실었습니다");
            foreach (var c in report.겹친것)
                text.AppendLine($"    {c.키,-16} {Side(c.실림)} ◀ 실림   {string.Join("  ", c.밀림.Select(Side))}");
        }

        text.AppendLine();
        text.AppendLine($"  파일  {취합본경로}");

        return text.ToString();
    }

    /// <summary>
    /// 겹친 자리의 한쪽. 시각은 사람이 훑을 것이라 월/일까지만 적는다.
    ///
    /// <para>구분자를 문화권에 맡기지 않는다 — 서식의 <c>/</c> 는 그 문화권의 날짜 구분자로
    /// 바뀌어, 한국어 창에서는 <c>08-28</c> 이 된다.</para>
    /// </summary>
    private static string Side(MergeSide side) =>
        side.시각.Length == 0 ? side.제출자
        : DateTime.TryParse(side.시각, out var moment)
            ? $"{side.제출자}({moment.ToString("MM/dd", CultureInfo.InvariantCulture)})"
            : $"{side.제출자}({side.시각})";
}

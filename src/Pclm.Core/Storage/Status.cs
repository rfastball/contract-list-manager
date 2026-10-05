using Dapper;

namespace Pclm.Core.Storage;

/// <summary>
/// 현황 한 칸의 <b>정의</b>. 무엇을 세고, 눌렀을 때 계획 탭이 무엇으로 걸러야 하는가.
/// </summary>
/// <param name="묶음">
/// 이 칸이 설 자리. 같은 이름이면 한 묶음이고, 묶음은 <see cref="Status.Metrics"/> 배열에
/// <b>처음 나온 차례</b>로 선다.
/// </param>
/// <param name="Sql">
/// 수 하나를 내는 스칼라 조회. <c>COUNT(*)</c> 가 SQLite 에서 64비트라 <c>long</c> 으로 받는다.
///
/// <para><b>금액을 세게 되면 뷰가 아니라 원본 표에서</b> <c>CAST(... AS REAL)</c> 로 낸다 —
/// 뷰의 금액은 자릿점이 찍힌 문자열이라 더할 수 없다. 지금 지표에는 금액이 없다.</para>
/// </param>
/// <param name="거르개">
/// 눌렀을 때 계획 탭이 걸 조건. 꼴은 <c>"열이름=값"</c> <b>하나뿐</b>이다 — 화면이 한 자리에서
/// 풀어 쓰므로, 지표마다 다른 길을 내면 이 구조가 곧바로 무너진다. 비면 누를 수 없는 줄이다.
/// </param>
public sealed record Metric(string 묶음, string 이름, string Sql, string? 거르개);

/// <summary>센 결과 한 칸.</summary>
public sealed record Cell(string 이름, long 수, string? 거르개);

/// <summary>같은 묶음의 칸들.</summary>
public sealed record Group(string 이름, IReadOnlyList<Cell> Cells);

/// <summary>현황 한 벌. 화면과 명령줄은 <b>오는 대로</b> 그린다.</summary>
public sealed record StatusReport(IReadOnlyList<Group> Groups);

/// <summary>
/// 쌓인 것을 세어 현황으로 낸다.
///
/// <para><b>무엇을 보일지가 아니라, 무엇을 보일지 나중에 갈아끼울 수 있게 하는 것이 목표다.</b>
/// 지금 들어 있는 지표는 가상이고 버릴 것이다 — 이 구조가 섰는지 판별하는 기준은
/// <b>지표를 더하거나 빼는 일이 <see cref="Metrics"/> 배열만 고치는 일인가</b> 하나다.
/// 화면·다리·타입이 그대로여야 한다. <c>metrics</c> 인자는 그 성질을 시험이 확인하려고 있다.</para>
///
/// <para>배치를 지표에 맞춰 짜지 않는다. 깔때기·색·그림은 여기에도 저쪽에도 없다 — 지금
/// 필요한 것은 "뒤에서 센 것이 앞에 뜬다" 를 눈으로 보는 것뿐이다.</para>
///
/// <para>읽기만 한다. <c>Reports.StatusSummary</c> 와 이름이 닮았지만 다른 것이다 —
/// 저쪽은 표마다의 행 수를 못 박은 레코드이고, 이쪽은 <b>자료로 된 지표 목록</b>이다.</para>
/// </summary>
public sealed class Status(Database database, IReadOnlyList<Metric>? metrics = null)
{
    /// <summary>
    /// 붙박이 지표. <b>이 배열이 전부다 — 갈아끼울 자리.</b>
    ///
    /// <para>지금 것은 가상이다. 깔때기·담당자별·손볼 것·기한은 여기에 줄을 더하는 일이지
    /// 다른 무엇을 고치는 일이 아니다.</para>
    /// </summary>
    public static IReadOnlyList<Metric> Metrics =>
    [
        // 쌓인 것 — 표를 그대로 센다. 계획만 차수가 없어 행이 곧 건이다.
        new("쌓인 것", "계획", "SELECT COUNT(*) FROM plan;", null),
        new("쌓인 것", "접수", "SELECT COUNT(DISTINCT request_base) FROM request;", null),
        new("쌓인 것", "공고", "SELECT COUNT(DISTINCT notice_base) FROM notice;", null),

        // 「공고」는 본번호를 세므로 취소 뒤 재채번된 건을 <b>둘로 센다</b>. 그 옆에 조달 건이
        // 몇인지를 함께 낸다 — 그것이 접수·계약과 견줄 수 있는 수다(스키마 V15).
        //
        // notice_group 을 그대로 세지 않는다. 문서 없는 조상만으로 이름이 선 건이 있어
        // (가스성분분석기가 가리키는 R26BK09013019 는 코퍼스에 없다), 그것까지 세면
        // 쌓인 것을 세는 자리에서 <b>쌓이지 않은 것</b>이 한 건으로 잡힌다.
        new("쌓인 것", "공고건",
            """
            SELECT COUNT(DISTINCT s.group_base) FROM notice_series s
            JOIN notice n ON n.notice_base = s.notice_base;
            """, null),
        new("쌓인 것", "계약", "SELECT COUNT(DISTINCT contract_base) FROM contract;", null),

        // 단계 — v_계획 이 낸 것을 그대로 센다. 세는 곳과 보는 곳이 같아야
        // 「17건」을 눌러 열었을 때 17줄이 선다.
        new("단계", "미착수", "SELECT COUNT(*) FROM v_계획 WHERE 단계 = '미착수';", "단계=미착수"),
        new("단계", "접수", "SELECT COUNT(*) FROM v_계획 WHERE 단계 = '접수';", "단계=접수"),
        new("단계", "공고", "SELECT COUNT(*) FROM v_계획 WHERE 단계 = '공고';", "단계=공고"),
        new("단계", "계약", "SELECT COUNT(*) FROM v_계획 WHERE 단계 = '계약';", "단계=계약"),
    ];

    private readonly Database _database = database;
    private readonly IReadOnlyList<Metric> _metrics = metrics ?? Metrics;

    /// <summary>지표를 차례대로 세어 묶음으로 접는다.</summary>
    public StatusReport Report()
    {
        using var connection = _database.OpenReadOnly();

        // 묶음은 배열에 처음 나온 차례로 세운다 — 사전순으로 세우면 지표를 더한 사람이
        // 어디에 설지 알 수 없고, 배열의 차례가 곧 화면의 차례라는 약속이 깨진다.
        var order = new List<string>();
        var cells = new Dictionary<string, List<Cell>>(StringComparer.Ordinal);

        foreach (var metric in _metrics)
        {
            if (!cells.TryGetValue(metric.묶음, out var list))
            {
                cells[metric.묶음] = list = [];
                order.Add(metric.묶음);
            }

            list.Add(new Cell(metric.이름, connection.ExecuteScalar<long>(metric.Sql), metric.거르개));
        }

        return new StatusReport([.. order.Select(name => new Group(name, cells[name]))]);
    }
}

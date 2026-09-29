using Dapper;

namespace Pclm.Core.Storage;

/// <summary>명세 한 줄.</summary>
public sealed record OutlineItem(
    int LineNo, string Name, string Specification,
    string Quantity, string Unit, string UnitPrice, string Amount);

/// <summary>
/// 계약 하나. <paramref name="Revisions"/> 는 이 계약번호로 쌓인 차수를 낮은 것부터 늘어놓은 것이다.
/// </summary>
public sealed record OutlineContract(
    string Key,
    string Number,
    string Title,
    string ContractedOn,
    string Amount,
    string Counterparty,
    string DemandAgency,
    IReadOnlyList<string> Revisions,
    IReadOnlyList<OutlineItem> Items);

/// <summary>공고 하나. <b>계약을 품지 않는다</b> — 사슬이 품는다.</summary>
public sealed record OutlineNotice(
    string Key,
    string Number,
    string Title,
    string PostedAt,
    string Agency,
    string EstimatedPrice,
    IReadOnlyList<string> Revisions);

/// <summary>
/// 사슬의 가운데 칸. 공고 하나가 아니라 <b>건 하나</b>다 — 취소·재공고로 갈린 본번호가 여기 모인다.
///
/// <para>공고 하나를 세우던 자리다. 그때는 취소된 원공고와 재공고가 <b>각각</b> 사슬을 세워,
/// 접수와 계약을 안고 서는 것이 죽은 쪽이고 살아 있는 재공고가 빈손으로 옆에 섰다. 건 이름이
/// 문서 없는 조상일 때는 한 조달 건이 사슬 셋으로 흩어졌다. 셋 다 오류가 아니라 <b>수가 늘어난
/// 화면</b>으로만 나타난다.</para>
/// </summary>
/// <param name="GroupBase">
/// 건의 이름. <b>화면에 내지 않는다</b> — 그 건이 아는 본번호 중 가장 이른 것이라 들어온 적
/// 없는 <b>문서 없는 조상</b>일 수 있고, 그것을 보이면 사람이 없는 공고를 찾으러 간다.
/// 그래도 담는 것은 옆판과 시험이 <b>건을 짚을 자리</b>가 필요해서다 — 링크가 가리키는 것이
/// 공고가 아니라 이 이름이라, 이것이 없으면 무엇에 이어졌는지를 되짚을 길이 없다.
/// </param>
/// <param name="Current">
/// 그 건의 살아 있는 공고 한 장. <b><c>null</c> 일 수 있다</b> — 두 본번호가 서로의 최신 차수를
/// 가리키면 대체되지 않은 것이 하나도 남지 않는다(<see cref="NoticeGroups.현행공고"/> 의 주석이
/// 그 갈래를 적는다). 그때 여기서 억지로 하나를 세우지 않는다. 뷰의 「현행공고」도 비는데
/// 이쪽만 세우면 규칙이 두 벌이 되어, <b>화면이 보여 준 공고와 기계가 견준 공고가 갈린다</b>.
/// </param>
/// <param name="Superseded">
/// 대체된 공고. <b>늦은 것부터</b> 선다 — 방금 지나간 것이 맨 앞이라야 사람이 찾는 순서와 같다.
/// <paramref name="Current"/> 가 없으면 그 건의 공고가 모두 여기로 온다.
/// </param>
public sealed record OutlineNoticeGroup(
    string GroupBase,
    OutlineNotice? Current,
    IReadOnlyList<OutlineNotice> Superseded);

/// <summary>
/// 접수 하나. <paramref name="RequestNumbers"/> 는 이 접수서가 묶은 조달요구번호 전부다 —
/// 줄 하나가 조달요구 하나라, 그것이 이 접수서가 무엇을 요청했는지의 요약이다.
/// </summary>
public sealed record OutlineRequest(
    string Key,
    string Number,
    string Title,
    string ReceivedOn,
    string GoodsAmount,
    string BudgetAmount,
    string DemandAgency,
    IReadOnlyList<string> RequestNumbers,
    IReadOnlyList<string> Revisions);

/// <summary>
/// 조달 건 하나 — <b>접수 → 공고 → 계약</b>.
///
/// <para>기본은 1:1:1 이다. 셋 중 어느 것이 없어도 사슬은 선다 — 접수만 들어온 것, 공고만
/// 들어온 것, 어디에도 매달리지 못한 계약이 <b>모두 제 자리에</b> 있어야 넣은 사람이
/// 빠진 것을 알아챈다. 계약이 둘 이상일 때만(분할 낙찰·수요기관 복수) 끝이 갈라진다.</para>
/// </summary>
/// <param name="Key">
/// 사슬을 구별하는 키. <b>있는 것의 본번호</b>를 <c>/</c> 로 이어 만든다.
/// 접수·공고가 하나도 없으면 그 계약의 본번호가 곧 사슬의 이름이다.
/// </param>
public sealed record OutlineChain(
    string Key,
    OutlineRequest? Request,
    OutlineNoticeGroup? Notice,
    IReadOnlyList<OutlineContract> Contracts)
{
    /// <summary>
    /// 차례를 정하는 값. <b>가장 이른 단계를 먼저 본다</b> — 공고가 있으면 게시일, 없으면
    /// 접수일, 그것도 없으면 계약일이다. 표기가 <c>yyyy/MM/dd</c> 라 문자열 비교로 시간순이 된다.
    ///
    /// <para>현행이 정해지지 않은 건은 <b>대체된 것 중 가장 이른 게시일</b>로 대신한다. 공고가
    /// 들어와 있는데도 값이 비면 그 사슬만 맨 아래로 가라앉아, 정작 손볼 것이 눈에서 사라진다.</para>
    /// </summary>
    public string SortsOn =>
        Notice?.Current?.PostedAt is { Length: > 0 } posted ? posted
        : 대체된것의_첫_게시일 is { Length: > 0 } superseded ? superseded
        : Request?.ReceivedOn is { Length: > 0 } received ? received
        : Contracts.Count > 0 ? Contracts[0].ContractedOn
        : string.Empty;

    /// <summary>대체된 것 중 <b>가장 이른</b> 게시일. 건이 시작된 때라 사슬의 나이에 가깝다.</summary>
    private string? 대체된것의_첫_게시일 =>
        Notice?.Superseded
            .Select(n => n.PostedAt)
            .Where(posted => posted.Length > 0)
            .Order(StringComparer.Ordinal)
            .FirstOrDefault();
}

/// <summary>쌓인 것을 생긴 모양대로 낸 것.</summary>
public sealed record OutlineTree(IReadOnlyList<OutlineChain> Chains);

/// <summary>
/// 축적된 것을 <b>생긴 모양 그대로</b> 낸다 — 접수 아래 공고, 공고 아래 계약, 계약 아래 명세.
///
/// <para>표는 줄 하나가 한 가지 것일 때만 읽힌다. 그런데 이 자료는 그렇지 않다 —
/// 계약 하나에 명세가 여러 줄이라, 평평한 표로 펴면 <b>위쪽 값이 아래 줄마다 되풀이된다</b>.
/// 겹친 것이 아니라 <b>펼 수 없는 것을 편 것</b>이다. 그래서 펴지 않고 그대로 낸다.</para>
///
/// <para><b>사슬로 낸다.</b> 예전에는 공고를 줄기로 세우고 매달리지 못한 계약을 <c>loose</c> 로
/// 따로 냈는데, 그러면 접수가 설 자리가 없고 "공고 하나에 계약 여럿" 이 기본인 것처럼
/// 보였다. 실제 기본은 1:1:1 이다(ADR-021).</para>
///
/// <para>값은 <b>계약면 뷰에서 읽는다</b>. 금액 자릿점과 날짜 표기를 여기서 다시 짜면
/// 같은 값이 화면마다 다르게 찍힌다 — 표기 규칙은 <see cref="Views"/> 한 군데에만 둔다.</para>
///
/// <para><b>차수는 접어서 낸다.</b> 본문에 세우는 것은 최신 차수 하나이고, 쌓인 차수는
/// 이름표로 함께 낸다. 변경공고 세 번이 목록에 세 줄로 서면 건수가 부풀어 보인다.</para>
///
/// <para><b>가운데 칸은 공고가 아니라 건이다.</b> 취소·재공고는 차수가 아니라 본번호를 가르므로
/// (ADR-025), 공고마다 사슬을 세우면 접수도 계약도 하나인 조달 건이 둘로 서고 그중 <b>죽은
/// 쪽</b>이 접수와 계약을 안는다. 사슬을 <c>notice_group</c> 단위로 돌아 그 자리를 없앤다 —
/// 링크가 가리키는 것이 이미 건이라, 조회 키도 여기서 건이어야 짝이 맞는다.</para>
/// </summary>
public sealed class Outline(Database database)
{
    private readonly Database _database = database;

    public OutlineTree Build()
    {
        using var connection = _database.Open();

        var revisions = Revisions(connection);
        var items = Items(connection);

        var contracts = Rows(connection,
                """
                SELECT 계약본번호, 계약번호, 계약건명, 계약일자, 계약금액, 계약상대자, 수요기관
                FROM v_계약_v1;
                """)
            .Select(row =>
            {
                var key = Text(row, "계약본번호");
                var number = Text(row, "계약번호");

                return new OutlineContract(
                    key, number, Text(row, "계약건명"), Text(row, "계약일자"), Text(row, "계약금액"),
                    Text(row, "계약상대자"), Text(row, "수요기관"),
                    revisions.GetValueOrDefault($"contract {key}") ?? [],
                    items.GetValueOrDefault(number) ?? []);
            })
            .ToList();

        // 건과 현행도 뷰에서 읽는다. notice_series 를 따로 묻지 않는 것은 이 파일의 규율이다 —
        // 「무엇이 현행인가」의 규칙이 여기 한 벌 더 생기면 화면이 보여 준 공고와 기계가 견준
        // 공고가 갈릴 수 있고, 그때 틀리는 것은 값이 아니라 무엇을 무엇에 이었는가다.
        var 공고줄 = Rows(connection,
                """
                SELECT 공고본번호, 입찰공고번호, 공고명, 게시일시, 공고기관, 추정가격, 공고건, 현행공고
                FROM v_공고_v1;
                """)
            .Select(row =>
            {
                var key = Text(row, "공고본번호");

                return (
                    // 계열이 서지 않아 건 이름이 비면 제 본번호로 선다. 빈 이름으로 묶으면
                    // 그런 공고가 죄다 한 사슬로 뭉치는데, 그것도 오류를 내지 않는다.
                    건: Text(row, "공고건") is { Length: > 0 } group ? group : key,
                    현행: Text(row, "현행공고"),
                    공고: new OutlineNotice(
                        key, Text(row, "입찰공고번호"), Text(row, "공고명"), Text(row, "게시일시"),
                        Text(row, "공고기관"), Text(row, "추정가격"),
                        revisions.GetValueOrDefault($"notice {key}") ?? []));
            })
            .ToList();

        var groups = 공고줄
            .GroupBy(row => row.건, StringComparer.Ordinal)
            .Select(g =>
            {
                var current = g
                    .Where(row => string.Equals(row.공고.Number, row.현행, StringComparison.Ordinal))
                    .Select(row => row.공고)
                    .FirstOrDefault();

                // 늦은 것이 앞. NoticeGroups.현행공고 가 현행을 고르는 차례와 같은 방향이라,
                // 두 자리가 같은 뜻으로 늘어놓는다.
                var superseded = g
                    .Select(row => row.공고)
                    .Where(n => current is null || !string.Equals(n.Key, current.Key, StringComparison.Ordinal))
                    .OrderByDescending(n => n.PostedAt, StringComparer.Ordinal)
                    .ThenByDescending(n => n.Key, StringComparer.Ordinal)
                    .ToList();

                return new OutlineNoticeGroup(g.Key, current, superseded);
            })
            .ToList();

        var requests = Rows(connection,
                """
                SELECT 접수본번호, 접수번호, 요청명, 접수일자, 품대, 예산금액, 수요기관, 조달요구번호
                FROM v_접수_v1;
                """)
            .Select(row =>
            {
                var key = Text(row, "접수본번호");

                return new OutlineRequest(
                    key, Text(row, "접수번호"), Text(row, "요청명"), Text(row, "접수일자"),
                    Text(row, "품대"), Text(row, "예산금액"), Text(row, "수요기관"),
                    // 뷰가 순번대로 이어 붙인 것을 도로 가른다. 화면은 번호마다 따로 세운다.
                    [.. Text(row, "조달요구번호")
                         .Split(',', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries)],
                    revisions.GetValueOrDefault($"request {key}") ?? []);
            })
            .ToList();

        // 링크의 계약 쪽은 계열이라 본번호이고, 공고 쪽은 <b>건</b>이다. 사슬을 건마다 도는
        // 것이 그래서다 — 여기서 공고 본번호로 물으면 재공고 쪽 사슬만 조회가 빗나가, 링크가
        // 멀쩡한데도 접수와 계약이 화면에서 사라진다.
        var contractToNotice = connection.Query<(string ContractBase, string NoticeGroup)>(
                "SELECT contract_base, notice_group FROM project_link;")
            .ToDictionary(l => l.ContractBase, l => l.NoticeGroup);

        // 접수:건이 1:1(notice_group UNIQUE)이라 건에서 접수로 되짚을 수 있다.
        var noticeToRequest = connection.Query<(string RequestBase, string NoticeGroup)>(
                "SELECT request_base, notice_group FROM request_link;")
            .ToDictionary(l => l.NoticeGroup, l => l.RequestBase);

        var byNotice = contracts
            .Where(c => contractToNotice.ContainsKey(c.Key))
            .GroupBy(c => contractToNotice[c.Key])
            .ToDictionary(g => g.Key, g => (IReadOnlyList<OutlineContract>)[.. Ordered(g)]);

        var requestByBase = requests.ToDictionary(r => r.Key);
        var chains = new List<OutlineChain>();
        var placedRequests = new HashSet<string>();

        // 공고가 있는 사슬. <b>건마다 한 번</b> 돈다 — 취소되고 재채번된 본번호는 이미 한
        // 이름 아래 모여 있으므로, 갈린 수만큼 사슬이 늘지 않는다.
        foreach (var group in groups)
        {
            OutlineRequest? request = null;

            if (noticeToRequest.TryGetValue(group.GroupBase, out var requestBase)
                && requestByBase.TryGetValue(requestBase, out var found))
            {
                request = found;
                placedRequests.Add(requestBase);
            }

            chains.Add(new OutlineChain(
                Key(request, group, []),
                request, group,
                byNotice.GetValueOrDefault(group.GroupBase) ?? []));
        }

        // 접수만 있고 공고가 아직 없는 것. 따로 세우지 않으면 넣은 사람이 넣지 않은 줄 안다.
        foreach (var request in requests.Where(r => !placedRequests.Contains(r.Key)))
            chains.Add(new OutlineChain(Key(request, null, []), request, null, []));

        // 어디에도 매달리지 못한 계약. 계약서만 먼저 들어온 흔한 경우라 하나씩 제 사슬로 선다.
        foreach (var contract in Ordered(contracts.Where(c => !contractToNotice.ContainsKey(c.Key))))
            chains.Add(new OutlineChain(Key(null, null, [contract]), null, null, [contract]));

        return new OutlineTree(
            [.. chains
                .OrderByDescending(c => c.SortsOn, StringComparer.Ordinal)
                .ThenBy(c => c.Key, StringComparer.Ordinal)]);
    }

    /// <summary>
    /// 사슬의 이름. <b>있는 것의 본번호를 이어 만든다.</b> 계약은 이름에 넣지 않는다 —
    /// 계약이 한 건 더 들어올 때마다 사슬의 이름이 바뀌면 화면이 접어 둔 자리를 잃는다.
    /// 접수도 공고도 없을 때만 그 계약이 사슬의 이름이 된다.
    ///
    /// <para>공고 자리에 서는 것은 현행 공고가 아니라 <b>건 이름</b>이다. 현행은 재공고가
    /// 나갈 때마다 옮겨 다니는데, 그것을 이름으로 쓰면 같은 조달 건인데도 사슬의 이름이 바뀌어
    /// 화면이 접어 둔 자리를 잃는다. 건 이름은 가장 이른 본번호라 뒤에 무엇이 들어와도 그대로다.</para>
    /// </summary>
    private static string Key(
        OutlineRequest? request, OutlineNoticeGroup? notice, IReadOnlyList<OutlineContract> contracts)
    {
        var parts = new List<string>();
        if (request is not null) parts.Add(request.Key);
        if (notice is not null) parts.Add(notice.GroupBase);
        if (parts.Count == 0) parts.AddRange(contracts.Select(c => c.Key));

        return string.Join("/", parts);
    }

    private static IEnumerable<OutlineContract> Ordered(IEnumerable<OutlineContract> contracts) =>
        contracts
            .OrderByDescending(c => c.ContractedOn, StringComparer.Ordinal)
            .ThenBy(c => c.Number, StringComparer.Ordinal);

    /// <summary>
    /// 쌓인 차수. 접수·공고·계약을 한 사전에 담되 <b>키에 종류를 붙인다</b> —
    /// 본번호 규칙이 서로 달라 부딪힐 일은 없지만, 부딪히지 않는다는 것에 기대지 않는다.
    /// </summary>
    private static Dictionary<string, IReadOnlyList<string>> Revisions(
        Microsoft.Data.Sqlite.SqliteConnection connection)
    {
        var all = new Dictionary<string, IReadOnlyList<string>>();

        void Read(string table, string column, string kind)
        {
            foreach (var group in connection
                         .Query<(string Base, string Seq)>(
                             $"SELECT {column}, seq FROM {table} ORDER BY CAST(seq AS INTEGER);")
                         .GroupBy(r => r.Base))
                all[$"{kind} {group.Key}"] = [.. group.Select(r => r.Seq)];
        }

        Read("request", "request_base", "request");
        Read("notice", "notice_base", "notice");
        Read("contract", "contract_base", "contract");

        return all;
    }

    /// <summary>계약번호로 묶은 명세. 뷰가 이미 최신 차수만 내므로 여기서 다시 고르지 않는다.</summary>
    private static Dictionary<string, IReadOnlyList<OutlineItem>> Items(
        Microsoft.Data.Sqlite.SqliteConnection connection) =>
        Rows(connection,
                """
                SELECT 계약번호, 순번, 품명, 규격, 수량, 단위, 단가, 금액
                FROM v_품목_v1 ORDER BY 계약번호, 순번;
                """)
            .GroupBy(row => Text(row, "계약번호"))
            .ToDictionary(
                g => g.Key,
                g => (IReadOnlyList<OutlineItem>)[.. g.Select(row => new OutlineItem(
                    Convert.ToInt32(row["순번"]), Text(row, "품명"), Text(row, "규격"),
                    Text(row, "수량"), Text(row, "단위"), Text(row, "단가"), Text(row, "금액")))]);

    /// <summary>
    /// 뷰 한 판을 <b>이름표 붙은 칸 묶음</b>으로 읽는다.
    ///
    /// <para><b>레코드로 바로 받지 않는다.</b> 뷰의 열은 죄다 식이라 선언 타입이 없고,
    /// 그러면 SQLite 가 타입을 알려줄 수 없어 매핑기가 전부 <c>byte[]</c> 로 보고 생성자를
    /// 못 찾는다 — <see cref="Reports.Revisions"/> 가 이미 밟은 자리다. 값만 받아 여기서 편다.</para>
    ///
    /// <para><c>dynamic</c> 인 채로 두지도 않는다. 한 칸이라도 <c>dynamic</c> 이면 그 식이
    /// 통째로 런타임 조회가 되어, <b>열 이름 오타가 컴파일에서 걸리지 않고 실행 중에 터진다</b>.
    /// 사전으로 내려 놓으면 적어도 조립하는 코드는 다시 정적으로 검사된다.</para>
    /// </summary>
    private static IEnumerable<IDictionary<string, object>> Rows(
        Microsoft.Data.Sqlite.SqliteConnection connection, string sql) =>
        connection.Query(sql).Cast<IDictionary<string, object>>();

    private static string Text(IDictionary<string, object> row, string column) =>
        row.TryGetValue(column, out var value) ? value?.ToString() ?? "" : "";
}

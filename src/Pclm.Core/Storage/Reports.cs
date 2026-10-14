using System.Globalization;
using Dapper;

namespace Pclm.Core.Storage;

public sealed record StatusSummary(
    int NoticeRows,
    int NoticeBases,
    int ContractRows,
    int ContractBases,
    int Counterparties,
    int UnlinkedContracts,
    int RequestRows,
    int RequestBases,
    int UnlinkedRequests);

/// <summary><c>Revisions</c> 는 SQLite <c>COUNT(*)</c> 가 64비트라 <c>long</c> 이다.</summary>
public sealed record RevisionGroup(string Base, string Kind, long Revisions, string Latest, string? Title);

public sealed record DashboardContract(string Number, string Title, string Date, decimal? Amount, bool Draft, bool Terminated);
public sealed record DeliveryDeadline(string Raw, string? Date, int? DaysRemaining);
public sealed record DeliveryItem(int LineNo, string Name, string Specification, string Agency, string Quantity, string Unit, DeliveryDeadline Due);
public sealed record DeliveryContract(string Number, string Title, string ContractDate, string Party, string Agency,
    bool Terminated, bool Draft, DeliveryDeadline Due, IReadOnlyList<DeliveryItem> Items, IReadOnlyList<string> Issues);
public sealed record DashboardCase(string Key, string Title, string? Stage,
    string RequestNumber, string NoticeNumber, IReadOnlyList<string> NoticeNumbers, string NoticeKind, string RequestDate, string PostedAt,
    string BidStartsAt, string BidClosesAt, string OpeningAt, string Agency,
    IReadOnlyList<DashboardContract> Contracts, decimal? ContractAmount, IReadOnlyList<string> Issues,
    int DraftContracts, bool NoticeCancelled, int TerminatedContracts);
public sealed record DashboardStage(string Key, string Label, int Count);
public sealed record DashboardDocuments(int Requests, int Notices, int Contracts,
    int RequestRevisions, int NoticeRevisions, int ContractRevisions);
public sealed record DashboardReport(string AsOf, int TotalCases, IReadOnlyList<DashboardStage> Stages,
    IReadOnlyList<DashboardCase> Rows, DashboardDocuments Documents, decimal ContractAmount, int ContractsWithAmount,
    int DataCheckCount, IReadOnlyList<DeliveryContract> Deliveries);

/// <summary>축적 상태를 훑어보는 조회들. 쓰기는 하지 않는다.</summary>
public sealed class Reports(Database database)
{
    private readonly Database _database = database;

    /// <summary>
    /// 수집된 조달 건의 브리핑. Outline 과 같은 공고건·명시 링크를 사용하며 품목 목록은 펼치지 않는다.
    /// 문서 부재는 업무 미진행을, 계약 문서는 이행 완료를 뜻하지 않는다. 모든 수는 한 읽기 사진에서 낸다.
    /// </summary>
    public DashboardReport Dashboard(DateTimeOffset? asOf = null)
    {
        var now = (asOf ?? DateTimeOffset.UtcNow).ToOffset(TimeSpan.FromHours(9));
        using var connection = _database.OpenReadOnly();
        using var transaction = connection.BeginTransaction(deferred: true);
        List<IDictionary<string, object>> Read(string sql) => connection.Query(sql, transaction: transaction)
            .Select(row => (IDictionary<string, object>)row).ToList();
        var requests = Read("""
            SELECT 접수본번호, 접수번호, 요청명, 접수일자, 수요기관,
                (SELECT count(*) FROM request_item i WHERE i.request_base = 접수본번호
                 AND i.request_base || '-' || i.seq = 접수번호 AND i.cancelled = 1) AS 취소품목수
            FROM v_접수
            """).ToDictionary(r => Text(r, "접수본번호"), StringComparer.Ordinal);
        var notices = Read("""
            SELECT 공고본번호, 입찰공고번호, 공고명, 공고종류, 게시일시, 공고기관, 수요기관,
                   입찰개시일시, 입찰마감일시, 개찰일시, 공고건, 현행공고 FROM v_공고
            """);
        var contracts = Read("""
            SELECT v.계약본번호, v.계약번호, v.계약건명, v.계약일자, v.계약금액, v.수요기관, v.계약상대자, v.납품기한,
                   c.terminated AS 해지
            FROM v_계약 v JOIN contract c ON v.계약본번호 = c.contract_base AND v.계약번호 = c.contract_base || c.seq
            """).ToDictionary(c => Text(c, "계약본번호"), StringComparer.Ordinal);
        var itemsByContract = Read("""
            SELECT 계약번호, 순번, 품명, 규격, 수요기관, 수량, 단위, 납품기한 FROM v_계약품목
            """).GroupBy(i => Text(i, "계약번호")).ToDictionary(g => g.Key, g => g.ToList(), StringComparer.Ordinal);
        // 기한은 계약과 모든 최신 품목을 따로 보존한다. 품목의 더 이른 날로 계약 기한을 덮지 않는다.
        var deliveries = contracts.Values.OrderBy(c => Text(c, "계약번호"), StringComparer.Ordinal)
            .Select(c => Delivery(c, itemsByContract.GetValueOrDefault(Text(c, "계약번호")) ?? [], now)).ToList();
        var requestLinks = connection.Query<(string Base, string Group)>(
            "SELECT request_base, notice_group FROM request_link", transaction: transaction).ToDictionary(r => r.Base, r => r.Group);
        var contractLinks = connection.Query<(string Base, string Group)>(
            "SELECT contract_base, notice_group FROM project_link", transaction: transaction).ToDictionary(c => c.Base, c => c.Group);
        var noticeGroups = notices.GroupBy(n => First(Text(n, "공고건"), Text(n, "공고본번호")))
            .ToDictionary(g => g.Key, g => g.ToList(), StringComparer.Ordinal);
        var requestsByGroup = requestLinks.Where(l => requests.ContainsKey(l.Key))
            .ToDictionary(l => l.Value, l => requests[l.Key], StringComparer.Ordinal);
        var contractsByGroup = contractLinks.Where(l => contracts.ContainsKey(l.Key)).GroupBy(l => l.Value)
            .ToDictionary(g => g.Key, g => g.Select(l => contracts[l.Key]).ToList(), StringComparer.Ordinal);
        // 참조만 남은 공고건에도 실제 접수·계약 문서가 있으면 보인다. 빈 건 표 자체는 분모에 넣지 않는다.
        var groups = noticeGroups.Keys.Concat(requestsByGroup.Keys).Concat(contractsByGroup.Keys).Distinct(StringComparer.Ordinal);
        var rows = new List<DashboardCase>();
        foreach (var group in groups)
        {
            var captured = noticeGroups.GetValueOrDefault(group) ?? [];
            var current = captured.SingleOrDefault(n => Text(n, "입찰공고번호") == Text(n, "현행공고"));
            var request = requestsByGroup.GetValueOrDefault(group);
            var linkedContracts = contractsByGroup.GetValueOrDefault(group) ?? [];
            rows.Add(Case("group:" + group, request, current, captured, linkedContracts, now));
        }
        foreach (var request in requests.Where(r => !requestLinks.ContainsKey(r.Key)))
            rows.Add(Case("request:" + request.Key, request.Value, null, [], [], now));
        foreach (var contract in contracts.Where(c => !contractLinks.ContainsKey(c.Key)))
            rows.Add(Case("contract:" + contract.Key, null, null, [], [contract.Value], now));

        var stages = new (string Key, string Label)[] {
            ("requestOnly", "공고 대기"), ("open", "공고중"),
            ("awaitingContract", "계약 대기"), ("contracted", "계약 완료") };
        var amounts = contracts.Values.Select(c => Money(Text(c, "계약금액"))).Where(a => a is not null).Select(a => a!.Value).ToList();
        int Count(string table) => connection.ExecuteScalar<int>($"SELECT count(*) FROM {table}", transaction: transaction);
        return new DashboardReport(now.ToString("O", CultureInfo.InvariantCulture), rows.Count,
            [.. stages.Select(s => new DashboardStage(s.Key, s.Label, rows.Count(r => r.Stage == s.Key)))],
            [.. rows.OrderByDescending(r => First(r.PostedAt, r.RequestDate, r.Contracts.FirstOrDefault()?.Date ?? ""), StringComparer.Ordinal)
                .ThenBy(r => r.Key, StringComparer.Ordinal)],
            new(requests.Count, notices.Count, contracts.Count, Count("request"), Count("notice"), Count("contract")),
            amounts.Sum(), amounts.Count, rows.Count(r => r.Stage is null), deliveries);
    }

    private static DashboardCase Case(string key, IDictionary<string, object>? request,
        IDictionary<string, object>? notice, IReadOnlyList<IDictionary<string, object>> notices,
        IReadOnlyList<IDictionary<string, object>> contracts, DateTimeOffset now)
    {
        var issues = new List<string>();
        var ordered = contracts.OrderBy(c => Text(c, "계약번호"), StringComparer.Ordinal).ToList();
        var documents = ordered.Select(c => new DashboardContract(Text(c, "계약번호"), Text(c, "계약건명"),
            Text(c, "계약일자"), Money(Text(c, "계약금액")), Text(c, "계약일자").Length == 0, Text(c, "해지") == "1")).ToList();
        var knownAmounts = documents.Where(c => c.Amount is not null).Select(c => c.Amount!.Value).ToList();
        foreach (var contract in ordered.Where(c => Text(c, "해지") == "1"))
            issues.Add($"해지 표시 계약: {Text(contract, "계약번호")}");
        if (documents.Any(c => c.Amount is null)) issues.Add("금액이 없거나 숫자로 읽히지 않는 계약 문서가 있습니다.");
        if (request is null) issues.Add("접수 문서가 이 건에 연결되어 있지 않습니다.");
        else if (Text(request, "취소품목수") != "0") issues.Add($"취소 표시 접수 품목 {Text(request, "취소품목수")}개가 있습니다.");
        // 접수만 있고 공고가 없는 것은 공고 대기 그 자체라 확인할 거리가 아니다. 계약이 공고 없이 선 것은 확인할 거리다.
        if (notices.Count == 0) { if (contracts.Count > 0) issues.Add("연결된 공고 문서가 없습니다."); }
        else if (notice is null) issues.Add("관련공고 관계로 현행 공고 하나를 정할 수 없습니다.");
        if (notices.Count > 1) issues.Add($"관련공고 {notices.Count}개 계열을 한 건으로 셉니다: {string.Join(", ", notices.Select(n => Text(n, "입찰공고번호")))}");
        var opening = Date(Text(notice, "개찰일시"));
        var posted = Date(Text(notice, "게시일시"));
        var cancelled = Text(notice, "공고종류") == "취소공고";
        var drafts = documents.Count(c => c.Number.Length > 0 && c.Draft);
        // 실공고: 게시일시가 읽히고 이미 지났으며 취소공고가 아닌 공고. 현행을 정하지 못하면 수집된 공고 모두를 본다.
        bool Real(IDictionary<string, object> n) =>
            Date(Text(n, "게시일시")) is { } at && at <= now && Text(n, "공고종류") != "취소공고";
        IReadOnlyList<IDictionary<string, object>> considered = notice is not null ? [notice] : notices;
        string? stage;
        if (documents.Count > 0)
        {
            if (documents.Any(c => c.Number.Length == 0 || (c.Date.Length > 0 && Date(c.Date) is null)))
            {
                stage = null;
                issues.Add("계약번호·계약일자를 확인하세요.");
            }
            else stage = drafts > 0 ? "awaitingContract" : "contracted";
            if (cancelled) issues.Add("취소공고와 계약 문서가 함께 있습니다. 계약 상태를 확인하세요.");
        }
        else if (request is not null && !considered.Any(Real))
        {
            // 접수 후 게시된 현행 공고가 없다 — 공고 문서 없음, 게시 전, 게시일시 미기재, 현행 취소를 모두 포함한다.
            stage = "requestOnly";
            if (considered.Any(n => Text(n, "게시일시").Length > 0 && Date(Text(n, "게시일시")) is null)) issues.Add("게시일시를 확인하세요.");
            if (Text(notice, "개찰일시").Length > 0 && opening is null) issues.Add("개찰일시를 확인하세요.");
            if (cancelled) issues.Add("현행 취소공고가 있습니다. 후속 재공고를 확인하세요.");
        }
        else if (notice is null)
            stage = null;
        else if (cancelled)
        {
            stage = null;
            issues.Add("현행 취소공고가 있습니다.");
        }
        else if (posted is null || (Text(notice, "개찰일시").Length > 0 && opening is null) || opening < posted)
        {
            stage = null;
            issues.Add("게시일시·개찰일시를 확인하세요.");
        }
        else if (posted > now)
        {
            stage = null;
            issues.Add("게시 예정 시각 전 공고이며 접수 문서가 연결되어 있지 않습니다.");
        }
        else if (opening is null)
        {
            stage = null;
            issues.Add("개찰일시를 확인하세요.");
        }
        else stage = opening <= now ? "awaitingContract" : "open";
        var displayNotice = notice ?? notices.OrderByDescending(n => Text(n, "게시일시"), StringComparer.Ordinal).FirstOrDefault();
        return new(key, First(Text(displayNotice, "공고명"), Text(request, "요청명"), documents.FirstOrDefault()?.Title ?? "", "제목 없음"),
            stage, Text(request, "접수번호"), Text(notice, "입찰공고번호"),
            [.. notices.OrderBy(n => Text(n, "입찰공고번호") == Text(notice, "입찰공고번호") ? 0 : 1)
                .ThenBy(n => Text(n, "입찰공고번호"), StringComparer.Ordinal).Select(n => Text(n, "입찰공고번호"))], Text(notice, "공고종류"),
            Text(request, "접수일자"), Text(notice, "게시일시"), Text(notice, "입찰개시일시"), Text(notice, "입찰마감일시"), Text(notice, "개찰일시"),
            First(Text(request, "수요기관"), Text(notice, "수요기관"), Text(notice, "공고기관"), Text(ordered.FirstOrDefault(), "수요기관")),
            documents, knownAmounts.Count > 0 ? knownAmounts.Sum() : null, issues, drafts, cancelled, documents.Count(c => c.Terminated));
    }

    private static DeliveryContract Delivery(IDictionary<string, object> contract,
        IReadOnlyList<IDictionary<string, object>> sourceItems, DateTimeOffset now)
    {
        var due = Deadline(Text(contract, "납품기한"), now);
        var items = sourceItems.OrderBy(i => Convert.ToInt32(i["순번"], CultureInfo.InvariantCulture))
            .Select(i => new DeliveryItem(Convert.ToInt32(i["순번"], CultureInfo.InvariantCulture),
                Text(i, "품명"), Text(i, "규격"), Text(i, "수요기관"), Text(i, "수량"), Text(i, "단위"), Deadline(Text(i, "납품기한"), now))).ToList();
        var issues = new List<string>();
        if (due.Date is null) issues.Add(due.Raw.Length == 0 ? "계약 전체 납품기한이 없습니다." : "계약 전체 납품기한을 날짜로 읽을 수 없습니다.");
        foreach (var item in items.Where(i => i.Due.Date is null && !(i.Due.Raw.Length == 0 && due.Date is not null)))
            issues.Add($"품목 {item.LineNo}: " + (item.Due.Raw.Length == 0 ? "납품기한이 없습니다." : "납품기한을 날짜로 읽을 수 없습니다."));
        if (Text(contract, "계약일자").Length > 0 && Date(Text(contract, "계약일자")) is null) issues.Add("계약일자를 읽을 수 없습니다.");
        return new(Text(contract, "계약번호"), Text(contract, "계약건명"), Text(contract, "계약일자"),
            Text(contract, "계약상대자"), Text(contract, "수요기관"), Text(contract, "해지") == "1", Text(contract, "계약일자").Length == 0,
            due, items, issues);
    }

    private static DeliveryDeadline Deadline(string raw, DateTimeOffset now)
    {
        var parsed = Date(raw);
        if (parsed is null) return new(raw, null, null);
        var date = DateOnly.FromDateTime(parsed.Value.DateTime);
        return new(raw, date.ToString("yyyy-MM-dd", CultureInfo.InvariantCulture), date.DayNumber - DateOnly.FromDateTime(now.DateTime).DayNumber);
    }

    private static string Text(IDictionary<string, object>? row, string key) =>
        row is not null && row.TryGetValue(key, out var value) ? Convert.ToString(value, CultureInfo.InvariantCulture) ?? "" : "";
    private static string First(params string[] values) => values.FirstOrDefault(v => v.Length > 0) ?? "";
    private static decimal? Money(string text) => decimal.TryParse(text, NumberStyles.Number, CultureInfo.InvariantCulture, out var amount) ? amount : null;
    private static DateTimeOffset? Date(string text) => DateTimeOffset.TryParseExact(text + " +09:00",
        ["yyyy/MM/dd zzz", "yyyy/MM/dd HH:mm:ss zzz"], CultureInfo.InvariantCulture, DateTimeStyles.None, out var value)
            ? value : null;

    public StatusSummary Summary()
    {
        using var connection = _database.Open();

        return new StatusSummary(
            connection.ExecuteScalar<int>("SELECT COUNT(*) FROM notice;"),
            connection.ExecuteScalar<int>("SELECT COUNT(DISTINCT notice_base) FROM notice;"),
            connection.ExecuteScalar<int>("SELECT COUNT(*) FROM contract;"),
            connection.ExecuteScalar<int>("SELECT COUNT(DISTINCT contract_base) FROM contract;"),
            connection.ExecuteScalar<int>("SELECT COUNT(*) FROM counterparty;"),
            connection.ExecuteScalar<int>(
                """
                SELECT COUNT(DISTINCT c.contract_base) FROM contract c
                WHERE NOT EXISTS (
                    SELECT 1 FROM project_link l WHERE l.contract_base = c.contract_base
                );
                """),
            connection.ExecuteScalar<int>("SELECT COUNT(*) FROM request;"),
            connection.ExecuteScalar<int>("SELECT COUNT(DISTINCT request_base) FROM request;"),
            connection.ExecuteScalar<int>(
                """
                SELECT COUNT(DISTINCT r.request_base) FROM request r
                WHERE NOT EXISTS (
                    SELECT 1 FROM request_link l WHERE l.request_base = r.request_base
                );
                """));
    }

    /// <summary>
    /// 차수가 둘 이상 쌓인 건. 변경공고·변경계약이 제대로 갈라졌는지 보는 창이다.
    ///
    /// <para><c>COUNT</c>·<c>MAX</c> 로 만든 열을 레코드로 바로 받지 않는다. <b>결과가 한 행도
    /// 없으면 SQLite 가 그 열의 타입을 알려줄 수 없어</b>(식에는 선언 타입이 없다) 매핑기가
    /// 전부 <c>byte[]</c> 로 보고 생성자를 못 찾는다. 자료를 아직 한 건도 넣지 않은 DB에서
    /// <c>status</c> 가 통째로 터졌다 — 갓 설치한 사람이 처음 치는 명령이 그것이다.
    /// 값만 받아 오고 조립은 여기서 한다.</para>
    /// </summary>
    public IReadOnlyList<RevisionGroup> Revisions()
    {
        using var connection = _database.Open();

        // 공고·계약은 표 이름만 다른 같은 물음이다. 종류 이름은 SQL 이 아니라 여기서 붙인다.
        //
        // 제목은 <b>최신 차수의 것</b>이라야 한다. 사전순 최대(MAX)로 고르면 「최신 001」 옆에
        // 000 의 제목이 붙는다 — 변경 뒤에 다시 내려받은 당초 공고는 공고명 끝에
        // 「(최종 공고가 아닙니다.)」 가 붙어 있어, 하필 그것이 사전순으로 크다.
        IEnumerable<RevisionGroup> Groups(string table, string key, string kind) =>
            connection.Query(
                $"""
                SELECT t.{key} AS Base, COUNT(*) AS Revisions,
                       (SELECT x.seq FROM {table} x WHERE x.{key} = t.{key}
                        ORDER BY CAST(x.seq AS INTEGER) DESC LIMIT 1) AS Latest,
                       (SELECT x.title FROM {table} x WHERE x.{key} = t.{key}
                        ORDER BY CAST(x.seq AS INTEGER) DESC LIMIT 1) AS Title
                FROM {table} t GROUP BY t.{key};
                """)
                .Select(row => new RevisionGroup(
                    (string)row.Base, kind, (long)row.Revisions, (string)row.Latest, (string?)row.Title));

        return [.. Groups("request", "request_base", "접수")
            .Concat(Groups("notice", "notice_base", "공고"))
            .Concat(Groups("contract", "contract_base", "계약"))
            .OrderByDescending(g => g.Revisions)
            .ThenBy(g => g.Base, StringComparer.Ordinal)];
    }
}

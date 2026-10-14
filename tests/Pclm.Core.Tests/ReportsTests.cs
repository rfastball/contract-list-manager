using Dapper;
using Microsoft.Data.Sqlite;
using Pclm.Core.Domain;
using Pclm.Core.Storage;
using Xunit;

namespace Pclm.Core.Tests;

/// <summary>
/// 상태 조회가 <b>자료가 한 건도 없는 DB에서도</b> 도는지 본다.
///
/// <para>갓 설치한 사람이 처음 치는 명령이 <c>status</c> 다. 그런데 빈 DB에서 그 명령은 요약을
/// 찍은 뒤 통째로 터졌다 — <c>COUNT</c>·<c>MAX</c> 처럼 식으로 만든 열은 결과가 비면 SQLite 가
/// 타입을 알려줄 수 없어 레코드 매핑이 깨진다. <b>자료가 한 건이라도 있으면 드러나지 않아</b>
/// 개발하는 동안에는 영영 보이지 않는 자리라, 시험으로 못을 박는다.</para>
/// </summary>
public class ReportsTests : IDisposable
{
    private readonly string _path = Path.Combine(
        Path.GetTempPath(), $"pclm-reports-{Guid.NewGuid():N}.db");

    private readonly Database _database;
    private readonly Store _store;

    public ReportsTests()
    {
        _database = PclmFile.Create(_path, PclmRole.Work);
        _store = new Store(_database);
    }

    [Fact]
    public void 빈_DB에서도_조회가_돈다()
    {
        var reports = new Reports(_database);

        Assert.Equal(0, reports.Summary().NoticeRows);
        Assert.Empty(reports.Revisions());
        var briefing = new Reports(new Database(_path, Access.Read)).Dashboard(At);
        Assert.Equal(0, briefing.TotalCases);
        Assert.Empty(briefing.Rows);
        Assert.All(briefing.Stages, s => Assert.Equal(0, s.Count));
        Assert.Equal(0m, briefing.ContractAmount);
        Assert.Equal(new DashboardDocuments(0, 0, 0, 0, 0, 0), briefing.Documents);
        Assert.Equal(0, briefing.DataCheckCount);
        Assert.Empty(briefing.Deliveries);
        Assert.Equal(["공고 대기", "공고중", "계약 대기", "계약 완료"], briefing.Stages.Select(s => s.Label));
    }

    [Fact]
    public void 차수는_공고와_계약을_따로_세고_많이_쌓인_것이_앞에_온다()
    {
        _store.UpsertNotice(Notice("000"));
        _store.UpsertNotice(Notice("001"));
        _store.UpsertContract(Contract("00"));

        var groups = new Reports(_database).Revisions();

        Assert.Collection(
            groups,
            notice =>
            {
                Assert.Equal("공고", notice.Kind);
                Assert.Equal("R26BK09017075", notice.Base);
                Assert.Equal(2, notice.Revisions);
                Assert.Equal("001", notice.Latest);   // 차수 문자열 그대로, 정수로 정렬한 최신
                Assert.Equal("수질측정기 구매", notice.Title);
            },
            contract =>
            {
                Assert.Equal("계약", contract.Kind);
                Assert.Equal(1, contract.Revisions);
            });
    }

    /// <summary>
    /// 제목은 <b>최신 차수의 것</b>이다. 사전순 최대를 고르면 「최신 001」 옆에 000 의 제목이 붙는다.
    ///
    /// <para>변경 뒤에 다시 내려받은 당초 공고는 공고명 끝에 「(최종 공고가 아닙니다.)」 가
    /// 붙어 있는데, 하필 그것이 사전순으로 크다 — <b>대체된 공고의 이름이 최신인 양</b> 찍혔다.
    /// 차수가 같은 이름을 이어 가는 동안에는 드러나지 않던 자리다.</para>
    /// </summary>
    [Fact]
    public void 제목은_최신_차수의_것이다()
    {
        _store.UpsertNotice(
            Notice("000") with { Title = "26년 가람 절단기 구매 (최종 공고가 아닙니다.)" });
        _store.UpsertNotice(
            Notice("001") with { Title = "26년 가람 절단기 구매" });

        var notice = Assert.Single(new Reports(_database).Revisions());

        Assert.Equal("001", notice.Latest);
        Assert.Equal("26년 가람 절단기 구매", notice.Title);
    }

    private static readonly DateTimeOffset At = new(2026, 10, 9, 0, 0, 0, TimeSpan.Zero); // 09:00 KST

    [Theory]
    [InlineData(8, 10, 8, "open")]
    [InlineData(9, 10, 12, "open")]
    [InlineData(8, 9, 12, "awaitingContract")]
    [InlineData(8, 8, 12, "awaitingContract")]
    [InlineData(10, 11, 12, null)]
    [InlineData(8, -1, 8, null)]
    [InlineData(-1, 10, 11, null)]
    [InlineData(10, 9, 8, null)]
    public void 브리핑은_게시와_개찰_KST_경계만_단계로_쓰고_부족한정보를_따로_센다(int posted, int opening, int bidClose, string? stage)
    {
        DateTime? Time(int hour) => hour < 0 ? null : new DateTime(2026, 10, 9, hour, 0, 0);
        _store.UpsertNotice(Notice("000") with { NoticeKind = "등록공고", PostedAt = Time(posted), Schedule = [
            new() { LineNo = 1, Name = "입찰서제출", StartsAt = Time(11), EndsAt = Time(bidClose) },
            new() { LineNo = 2, Name = "개찰", StartsAt = Time(opening) }] });
        var report = new Reports(_database).Dashboard(At);
        var row = Assert.Single(report.Rows);
        Assert.Equal(stage, row.Stage);
        Assert.EndsWith("+09:00", report.AsOf);
        Assert.Equal(1, report.Stages.Sum(s => s.Count) + report.DataCheckCount);
        Assert.Equal(stage is null ? 1 : 0, report.DataCheckCount);
        Assert.Null(row.ContractAmount);
        Assert.Equal(0, report.ContractsWithAmount);
    }

    [Fact]
    public void 브리핑은_차수와_재공고를_접고_계약여럿과_단독문서를_빠뜨리지_않는다()
    {
        _store.UpsertRequest(new RequestRecord { RequestBase = "REQ", Seq = "000", Title = "원제목" });
        _store.UpsertRequest(new RequestRecord { RequestBase = "REQ", Seq = "010", Title = "최신 접수" });
        _store.UpsertRequest(new RequestRecord { RequestBase = "STANDALONE", Seq = "000", Title = "공고 미수집 접수", Items = [new() { LineNo = 1, Cancelled = true }] });
        _store.UpsertNotice(Notice("000") with { NoticeKind = "등록공고" });
        _store.UpsertNotice(Notice("001") with { NoticeKind = "취소공고" });
        _store.UpsertNotice(new NoticeRecord { NoticeBase = "RENOTICE", Seq = "000", Title = "재공고",
            NoticeKind = "등록공고", RelatedNotices = ["R26BK09017075-001"] });
        _store.UpsertContract(Contract("00") with { Amount = 10m, Terminated = true });
        _store.UpsertContract(Contract("10") with { Amount = 25m, Terminated = false, ContractedOn = new DateTime(2026, 10, 8) });
        _store.UpsertContract(new ContractRecord { ContractBase = "SECOND", Seq = "00", Amount = 7m, Terminated = true });
        _store.UpsertContract(new ContractRecord { ContractBase = "STANDALONE", Seq = "00", Terminated = true, ContractedOn = new DateTime(2026, 10, 1) });
        ReferenceFixture.Request(_database, new("request", "REQ", "010"), new("notice", "RENOTICE", "000"));
        ReferenceFixture.Contract(_database, new("contract", "R26TA09110507", "10"), new("notice", "RENOTICE", "000"));
        ReferenceFixture.Contract(_database, new("contract", "SECOND", "00"), new("notice", "RENOTICE", "000"));

        _store.UpsertNotice(new NoticeRecord { NoticeBase = "MISSING", Seq = "000" });
        _store.UpsertContract(new ContractRecord { ContractBase = "MISSING", Seq = "00", Amount = 0m });
        ReferenceFixture.Contract(_database, new("contract", "MISSING", "00"), new("notice", "MISSING", "000"));
        // 문서 없는 계열/링크가 남은 유효한 부분 DB도 계약 문서를 분모에서 잃으면 안 된다.
        using (var c = _database.Open()) c.Execute("DELETE FROM notice WHERE notice_base='MISSING'");
        var report = new Reports(new Database(_path, Access.Read)).Dashboard(At);
        Assert.Equal(4, report.TotalCases);
        Assert.Equal(4, report.Stages.Sum(s => s.Count));
        Assert.Equal(new DashboardDocuments(2, 2, 4, 3, 3, 5), report.Documents);
        Assert.Equal(32m, report.ContractAmount); // 최신 차수 25 + 해지 표시 계약 7, 옛 계약 10은 제외
        Assert.Equal(3, report.ContractsWithAmount);
        var group = Assert.Single(report.Rows, r => r.NoticeNumber == "RENOTICE-000");
        Assert.Equal("awaitingContract", group.Stage);
        Assert.Equal(1, group.DraftContracts);
        Assert.False(group.Contracts.Single(c => c.Number == "R26TA0911050710").Draft);
        Assert.True(group.Contracts.Single(c => c.Number == "SECOND00").Draft);
        Assert.Equal("REQ-010", group.RequestNumber);
        Assert.Equal("재공고", group.Title);
        Assert.Equal(["RENOTICE-000", "R26BK09017075-001"], group.NoticeNumbers);
        Assert.Equal(2, group.Contracts.Count);
        Assert.Equal(32m, group.ContractAmount);
        Assert.Contains(group.Issues, s => s.Contains("SECOND00"));
        Assert.Contains(group.Issues, s => s.Contains("관련공고 2개"));
        Assert.Contains(Assert.Single(report.Rows, r => r.Stage == "requestOnly").Issues, i => i.Contains("취소 표시 접수 품목 1개"));
        Assert.Equal(0m, Assert.Single(report.Rows, r => r.Key == "group:MISSING").ContractAmount);
        var terminated = Assert.Single(report.Rows, r => r.TerminatedContracts == 1 && r.Contracts.Count == 1);
        Assert.Equal("contracted", terminated.Stage);
        Assert.Null(terminated.ContractAmount);
        Assert.Contains(terminated.Issues, s => s.Contains("금액"));
    }

    [Fact]
    public void 브리핑은_현행_취소와_모호한_현행을_가공하지_않는다()
    {
        _store.UpsertNotice(Notice("000") with { NoticeKind = "등록공고" });
        _store.UpsertNotice(Notice("001") with { NoticeKind = "취소공고" });
        _store.UpsertNotice(new NoticeRecord { NoticeBase = "LOOP1", Seq = "000", RelatedNotices = ["LOOP2-000"] });
        _store.UpsertNotice(new NoticeRecord { NoticeBase = "LOOP2", Seq = "000", RelatedNotices = ["LOOP1-000"] });
        var report = new Reports(_database).Dashboard(At);
        Assert.Equal(2, report.TotalCases);
        Assert.Equal("R26BK09017075-001", Assert.Single(report.Rows, r => r.NoticeCancelled).NoticeNumber);
        Assert.All(report.Rows, r => Assert.Null(r.Stage));
        Assert.Equal(2, report.DataCheckCount);
        var unclear = Assert.Single(report.Rows, r => !r.NoticeCancelled);
        Assert.Equal("", unclear.NoticeNumber);
        Assert.Equal(["LOOP1-000", "LOOP2-000"], unclear.NoticeNumbers);
        Assert.Contains(unclear.Issues, s => s.Contains("현행 공고"));
        _store.UpsertContract(Contract("00") with { ContractedOn = new DateTime(2026, 10, 1) });
        ReferenceFixture.Contract(_database, new("contract", "R26TA09110507", "00"), new("notice", "R26BK09017075", "001"));
        var withContract = Assert.Single(new Reports(_database).Dashboard(At).Rows, r => r.Stage == "contracted");
        Assert.Contains(withContract.Issues, i => i.Contains("취소공고와 계약"));
    }

    [Fact]
    public void 브리핑은_정정된_날짜와_금액을_읽되_틀린값을_0으로_바꾸지_않는다()
    {
        _store.UpsertNotice(Notice("000") with { PostedAt = new DateTime(2026, 10, 9, 8, 0, 0), Schedule = [new() { LineNo = 1, Name = "개찰", StartsAt = new DateTime(2026, 10, 9, 11, 0, 0) }] });
        _store.SetOverride(new("notice", "R26BK09017075", "000"), "게시일시", "읽을 수 없는 날");
        _store.UpsertContract(Contract("00") with { Amount = 10m, ContractedOn = new DateTime(2026, 10, 1) });
        _store.SetOverride(new("contract", "R26TA09110507", "00"), "계약금액", "금액 미확인");
        var report = new Reports(_database).Dashboard(At);
        Assert.Contains(Assert.Single(report.Rows, r => r.Stage is null).Issues, i => i.Contains("게시일시·개찰일시"));
        Assert.Null(Assert.Single(report.Rows, r => r.Stage == "contracted").ContractAmount);
        Assert.Equal(0m, report.ContractAmount);
        Assert.Equal(0, report.ContractsWithAmount);
        _store.SetOverride(new("notice", "R26BK09017075", "000"), "게시일시", "0001/01/01");
        Assert.Single(new Reports(_database).Dashboard(At).Rows, r => r.Stage is null);
    }

    [Fact]
    public void 공고_대기는_접수를_필요로_하고_날짜없는_계약은_초안이며_틀린날짜는_확인대상이다()
    {
        _store.UpsertRequest(new RequestRecord { RequestBase = "REQ", Seq = "000" });
        _store.UpsertNotice(Notice("000") with { PostedAt = new DateTime(2026, 10, 10), Schedule = [
            new() { LineNo = 1, Name = "개찰", StartsAt = new DateTime(2026, 10, 11) }] });
        ReferenceFixture.Request(_database, new("request", "REQ", "000"), new("notice", "R26BK09017075", "000"));
        Assert.Equal("requestOnly", Assert.Single(new Reports(_database).Dashboard(At).Rows).Stage);
        _store.SetOverride(new("notice", "R26BK09017075", "000"), "개찰일시", "");
        Assert.Equal("requestOnly", Assert.Single(new Reports(_database).Dashboard(At).Rows).Stage); // 게시 전은 개찰 시각 없이도 알 수 있다.
        _store.SetOverride(new("notice", "R26BK09017075", "000"), "개찰일시", "개찰 미확인");
        var unreadOpening = Assert.Single(new Reports(_database).Dashboard(At).Rows);
        Assert.Equal("requestOnly", unreadOpening.Stage); // 게시 전이면 공고 대기다. 틀린 개찰 시각은 행의 확인 표시로 남는다.
        Assert.Contains(unreadOpening.Issues, i => i.Contains("개찰일시"));
        _store.UpsertContract(Contract("00"));
        ReferenceFixture.Contract(_database, new("contract", "R26TA09110507", "00"), new("notice", "R26BK09017075", "000"));
        var draft = Assert.Single(new Reports(_database).Dashboard(At).Rows);
        Assert.Equal("awaitingContract", draft.Stage);
        Assert.Equal(1, draft.DraftContracts);
        _store.SetOverride(new("contract", "R26TA09110507", "00"), "계약일자", "날짜 미확인");
        var invalid = new Reports(_database).Dashboard(At);
        Assert.Null(Assert.Single(invalid.Rows).Stage);
        Assert.Equal(1, invalid.DataCheckCount);
        Assert.Equal(0, Assert.Single(invalid.Rows).DraftContracts);
    }

    /// <summary>
    /// 공고 대기는 「접수 후 게시된 현행 공고가 없는 건」이다. 게시일시가 비어 있는 ERP 초안 공고,
    /// 게시 전 공고, 현행이 취소공고인 건이 단계 미정으로 빠지면 공고를 기다리는 건이 어디에도 세지 않는다.
    /// </summary>
    [Theory]
    [InlineData("none", "requestOnly", null)]
    [InlineData("draft", "requestOnly", null)]
    [InlineData("future", "requestOnly", null)]
    [InlineData("cancelled", "requestOnly", "취소공고")]
    [InlineData("unreadable", "requestOnly", "게시일시")]
    [InlineData("posted", "open", null)]
    public void 공고_대기는_접수_후_게시된_현행_공고가_없는_건이다(string scenario, string stage, string? issue)
    {
        _store.UpsertRequest(new RequestRecord { RequestBase = "REQTEST01", Seq = "000", Title = "시험 접수" });
        NoticeRecord Made(string seq, DateTime? posted, string kind = "등록공고") => new()
        {
            NoticeBase = "NTCTEST01", Seq = seq, Title = "시험 공고", NoticeKind = kind, PostedAt = posted,
            Schedule = [new() { LineNo = 1, Name = "개찰", StartsAt = new DateTime(2026, 10, 20) }],
        };
        switch (scenario)
        {
            case "draft": _store.UpsertNotice(Made("000", null)); break;
            case "future": _store.UpsertNotice(Made("000", new DateTime(2026, 10, 10))); break;
            case "cancelled":
                _store.UpsertNotice(Made("000", new DateTime(2026, 10, 1)));
                _store.UpsertNotice(Made("001", new DateTime(2026, 10, 2), "취소공고"));
                break;
            case "unreadable":
                _store.UpsertNotice(Made("000", new DateTime(2026, 10, 1)));
                _store.SetOverride(new("notice", "NTCTEST01", "000"), "게시일시", "게시 미확인");
                break;
            case "posted": _store.UpsertNotice(Made("000", new DateTime(2026, 10, 1))); break;
        }
        if (scenario != "none") ReferenceFixture.Request(_database, new("request", "REQTEST01", "000"), new("notice", "NTCTEST01", "000"));
        var report = new Reports(_database).Dashboard(At);
        var row = Assert.Single(report.Rows);
        Assert.Equal(stage, row.Stage);
        Assert.Equal(0, report.DataCheckCount);
        if (issue is null) Assert.Empty(row.Issues);
        else Assert.Contains(row.Issues, i => i.Contains(issue));
    }

    [Theory]
    [InlineData(-1)]
    [InlineData(10)]
    public void 접수가_없는_미게시_공고는_공고_대기로_세지_않는다(int day)
    {
        _store.UpsertNotice(new NoticeRecord { NoticeBase = "NTCTEST02", Seq = "000", Title = "시험 공고", NoticeKind = "등록공고",
            PostedAt = day < 0 ? null : new DateTime(2026, 10, day) });
        var row = Assert.Single(new Reports(_database).Dashboard(At).Rows);
        Assert.Null(row.Stage);
        Assert.NotEmpty(row.Issues);
    }

    [Fact]
    public void 납품기한은_최신_계약과_모든_품목을_따로_내고_KST_날짜로_경과일을_센다()
    {
        _store.UpsertContract(Contract("00") with { DeliveryDue = new DateTime(2020, 1, 1), Items = [
            new() { LineNo = 99, ItemName = "옛 품목", DeliveryDue = new DateTime(2020, 1, 1) }] });
        _store.UpsertContract(Contract("10") with { ContractedOn = new DateTime(2026, 10, 1), DeliveryDue = new DateTime(2026, 10, 16, 23, 59, 59),
            Counterparty = new() { Name = "시험 업체", BusinessNumber = "0000000000" }, DemandAgency = "기관", Items = [
            new() { LineNo = 1, ItemName = "경과", DeliveryDue = new DateTime(2026, 10, 8, 23, 59, 59) },
            new() { LineNo = 2, ItemName = "계약기한 적용" },
            new() { LineNo = 3, ItemName = "오늘", DeliveryDue = new DateTime(2026, 10, 9) },
            new() { LineNo = 4, ItemName = "30일", DeliveryDue = new DateTime(2026, 11, 8) }] });
        _store.SetOverride(new("contract", "R26TA09110507", "10"), "납품기한", "2026/10/16");
        _store.UpsertContract(new ContractRecord { ContractBase = "ENDED", Seq = "00", Terminated = true,
            DeliveryDue = new DateTime(2026, 10, 9) });
        var report = new Reports(new Database(_path, Access.Read)).Dashboard(At);
        Assert.Equal(2, report.Deliveries.Count);
        var delivery = Assert.Single(report.Deliveries, d => d.Number == "R26TA0911050710");
        Assert.Equal("시험 업체", delivery.Party);
        Assert.Equal("기관", delivery.Agency);
        Assert.Equal(new DeliveryDeadline("2026/10/16", "2026-10-16", 7), delivery.Due);
        Assert.Equal([1, 2, 3, 4], delivery.Items.Select(i => i.LineNo));
        Assert.Equal(-1, delivery.Items[0].Due.DaysRemaining);
        Assert.Null(delivery.Items[1].Due.Date); // 원래 품목값은 보존; 조회에서 명시 계약기한을 적용한다.
        Assert.DoesNotContain(delivery.Issues, i => i.StartsWith("품목 2:"));
        Assert.Equal(0, delivery.Items[2].Due.DaysRemaining);
        Assert.Equal(30, delivery.Items[3].Due.DaysRemaining);
        Assert.True(Assert.Single(report.Deliveries, d => d.Number == "ENDED00").Terminated);
        using (var c = _database.Open()) c.Execute("UPDATE contract_item SET delivery_due='잘못된 기한' WHERE contract_base='R26TA09110507' AND seq='10' AND line_no=2");
        var malformedItem = Assert.Single(new Reports(_database).Dashboard(At).Deliveries, d => d.Number == "R26TA0911050710");
        Assert.NotEmpty(malformedItem.Items[1].Due.Raw);
        Assert.Null(malformedItem.Items[1].Due.Date);
        Assert.Contains(malformedItem.Issues, i => i.StartsWith("품목 2:")); // 잘못 적힌 품목 날짜는 계약기한으로 덮지 않는다.
        _store.SetOverride(new("contract", "R26TA09110507", "10"), "납품기한", "기한 미확인");
        var invalid = Assert.Single(new Reports(_database).Dashboard(At).Deliveries, d => d.Number == "R26TA0911050710");
        Assert.Null(invalid.Due.Date);
        Assert.Null(invalid.Due.DaysRemaining);
        Assert.Contains(invalid.Issues, i => i.Contains("날짜로 읽을 수 없습니다"));
        Assert.Contains(invalid.Issues, i => i.StartsWith("품목 2:"));
        Assert.Equal(-1, invalid.Items[0].Due.DaysRemaining);
    }

    private static NoticeRecord Notice(string seq) => new()
    {
        NoticeBase = "R26BK09017075",
        Seq = seq,
        Title = "수질측정기 구매",
    };

    private static ContractRecord Contract(string seq) => new()
    {
        ContractBase = "R26TA09110507",
        Seq = seq,
        Title = "2026년 가람 수질측정기 조달",
        Amount = 164_872_340m,
    };

    public void Dispose()
    {
        SqliteConnection.ClearAllPools();
        foreach (var suffix in new[] { "", "-wal", "-shm" })
            File.Delete(_path + suffix);
        GC.SuppressFinalize(this);
    }
}

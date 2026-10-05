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

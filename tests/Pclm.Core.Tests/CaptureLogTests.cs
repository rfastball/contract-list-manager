using System.Globalization;
using System.Text.Json;
using Dapper;
using Microsoft.Data.Sqlite;
using Pclm.Core.Erp;
using Pclm.Core.Storage;
using Xunit;

namespace Pclm.Core.Tests;

/// <summary>
/// 나라장터 화면의 「들어온 자료」. 수집 기록을 종류·결과·오늘로 갈라 읽는다. 번호·건명은 모두 지어낸 것이다.
/// </summary>
public sealed class CaptureLogTests : IDisposable
{
    private readonly string _root = Path.Combine(Path.GetTempPath(), "pclm-log-" + Guid.NewGuid().ToString("N"));
    private string DbPath => Path.Combine(_root, "pclm.db");

    /// <summary>로컬 지금. 날짜 경계를 세우려고 못 박는다.</summary>
    private static readonly DateTime Now = new(2026, 10, 5, 9, 30, 0, DateTimeKind.Local);

    public CaptureLogTests() => PclmFile.Create(DbPath, PclmRole.Work);

    public void Dispose()
    {
        SqliteConnection.ClearAllPools();
        Directory.Delete(_root, recursive: true);
    }

    /// <summary>저장과 같은 꼴(UTC 왕복 표기)로 적는다 — 로컬 시각을 넘기면 옮겨서 적는다.</summary>
    private static string Stored(DateTime local) => local.ToUniversalTime().ToString("O", CultureInfo.InvariantCulture);

    private static CaptureInput Notice(string title) => new(
        "g2b-notice-a-v1", Mapping.Hash(Mapping.Defaults()), JsonSerializer.SerializeToElement(new
        {
            pointInfo = new { bidPbancNo = "R26BK00000001", bidPbancOrd = "001", bidPbancNm = title },
            tables = new { },
        }), "live", Screen: "01179");

    private void Stamp(string captureId, DateTime local)
    {
        using var c = new Database(DbPath).Open();
        c.Execute("UPDATE erp_capture SET captured_at = @at WHERE capture_id = @captureId", new { captureId, at = Stored(local) });
    }

    private void Fake(string captureId, string profile, string @base, string seq, bool changed, DateTime local)
    {
        using var c = new Database(DbPath).Open();
        c.Execute("""
            INSERT INTO erp_capture(capture_id, request_hash, content_hash, profile, entity_base, entity_seq, snapshot_json, result_json, captured_at)
            VALUES (@captureId, 'h', 'h', @profile, @base, @seq, '{}', @result, @at)
            """, new { captureId, profile, @base, seq, result = JsonSerializer.Serialize(new { changed }), at = Stored(local) });
    }

    [Fact]
    public void 오늘_것만_종류와_결과로_가르고_마지막은_오늘이_아니어도_낸다()
    {
        var db = new Database(DbPath);

        var empty = CaptureLog.Read(db, Now);
        Assert.Equal("2026-10-05", empty.Today);
        Assert.Empty(empty.Entries);
        Assert.Null(empty.Last);

        // 어제 자정 1초 전 — 오늘이 아니다. 그래도 마지막 하나로는 선다.
        Fake(Guid.NewGuid().ToString(), "g2b-contract-v1", "R26TA00000003", "01", true, Now.Date.AddSeconds(-1));
        var yesterday = CaptureLog.Read(db, Now);
        Assert.Empty(yesterday.Entries);
        Assert.Equal(0, yesterday.Contracts);
        Assert.Equal("계약", yesterday.Last!.Kind);
        Assert.Equal("2026-10-04T23:59:59", yesterday.Last.At);

        // 진짜 저장 길로 공고 한 건, 같은 화면을 다시 저장해 「변경 없음」 한 건.
        var host = new ErpCapture(db);
        var first = Guid.NewGuid().ToString();
        host.Save(Notice("지어낸 공고"), host.Inspect(Notice("지어낸 공고")).BaseToken, first, new());
        var again = Guid.NewGuid().ToString();
        host.Save(Notice("지어낸 공고"), host.Inspect(Notice("지어낸 공고")).BaseToken, again, new());
        Stamp(first, Now.Date.AddHours(9));
        Stamp(again, Now.Date.AddHours(9).AddMinutes(5));

        // 자정 30초 뒤 — 오늘이다. 계약 표에 건명이 있으면 그것을 낸다.
        Fake(Guid.NewGuid().ToString(), "g2b-contract-v1", "R26TA00000003", "01", true, Now.Date.AddSeconds(30));
        using (var c = db.Open())
            c.Execute("""
                INSERT INTO contract_series VALUES ('R26TA00000003');
                INSERT INTO contract(contract_base, seq, title, updated_at) VALUES ('R26TA00000003', '01', '지어낸 계약', 'now');
                """);

        var day = CaptureLog.Read(db, Now);
        Assert.Equal(
            new[]
            {
                ("2026-10-05T09:05:00", "공고", "R26BK00000001-001", "지어낸 공고", false, "변경 없음"),
                ("2026-10-05T09:00:00", "공고", "R26BK00000001-001", "지어낸 공고", true, "저장"),
                ("2026-10-05T00:00:30", "계약", "R26TA0000000301", "지어낸 계약", true, "저장"),
            },
            day.Entries.Select(e => (e.At, e.Kind, e.Number, e.Title, e.Changed, e.Result)));
        Assert.Equal((0, 2, 1, 2), (day.Requests, day.Notices, day.Contracts, day.Saved));
        Assert.Equal(again, day.Last!.CaptureId);
        Assert.Equal("live", day.Entries[0].Scope);

        // 건명은 지금 쌓인 것이 먼저다. 비면 수집 사진에서, 사진에도 없으면 빈 문자열이다.
        using (var c = db.Open())
            c.Execute("UPDATE notice SET title = '사람이 본 건명'; DELETE FROM contract; DELETE FROM contract_series;");
        Assert.Equal("사람이 본 건명", CaptureLog.Read(db, Now).Entries[0].Title);
        using (var c = db.Open()) c.Execute("UPDATE notice SET title = NULL;");
        var fallback = CaptureLog.Read(db, Now);
        Assert.Equal("지어낸 공고", fallback.Entries[0].Title);
        Assert.Equal("", fallback.Entries[2].Title);

        // 다음 날 아침에는 어제가 된다 — 경계는 로컬 자정이다.
        Assert.Empty(CaptureLog.Read(db, Now.AddDays(1)).Entries);

        // 열람 창은 읽기 전용 손잡이로 같은 것을 읽는다.
        var viewer = CaptureLog.Read(new Database(DbPath, Access.Read), Now);
        Assert.Equal(3, viewer.Entries.Count);
    }
}

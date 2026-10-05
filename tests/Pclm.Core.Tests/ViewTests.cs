using Microsoft.Data.Sqlite;
using Pclm.Core.Storage;
using Xunit;

namespace Pclm.Core.Tests;

/// <summary>
/// 다른 자료를 열어 보는 길(ADR-031). 지키는 것은 둘이다.
///
/// <para>① <b>원본은 한 바이트도 바뀌지 않는다.</b> 받은 제출본을 열어 본 것만으로 판이 오르거나 뷰가 다시
/// 지어지거나 옆에 <c>-wal</c> 이 서면, 보낸 것과 받은 것이 오류 없이 갈린다.</para>
///
/// <para>② <b>읽는 길은 읽기 손잡이에서 다 돈다.</b> 창은 열람 중 모든 읽기를 <see cref="Access.Read"/> 위에서
/// 부르므로, 읽기 길 어딘가에 숨은 쓰기가 있으면 그 화면이 통째로 SQLITE_READONLY 로 선다. 다리의 읽기 요청
/// 하나하나가 부르는 Core 함수를 여기서 차례로 부른다.</para>
///
/// <para>값은 전부 지어낸 것이다.</para>
/// </summary>
public sealed class ViewTests : IDisposable
{
    private readonly string _root = Path.Combine(Path.GetTempPath(), "pclm-view-" + Guid.NewGuid().ToString("N")[..8]);

    public ViewTests() => Directory.CreateDirectory(_root);

    public void Dispose()
    {
        SqliteConnection.ClearAllPools();
        try { Directory.Delete(_root, recursive: true); } catch { /* 지우지 못해도 시험 결과는 그대로다 */ }
    }

    private string In(string name) => Path.Combine(_root, name);

    private string ViewDirectory => In("홈\\view");

    private static void Run(SqliteConnection connection, string sql)
    {
        using var command = connection.CreateCommand();
        command.CommandText = sql;
        command.ExecuteNonQuery();
    }

    private static long Pragma(string path, string name)
    {
        using var connection = new SqliteConnection($"Data Source={path};Mode=ReadOnly;Pooling=False");
        connection.Open();
        using var command = connection.CreateCommand();
        command.CommandText = $"PRAGMA {name};";
        return Convert.ToInt64(command.ExecuteScalar());
    }

    /// <summary>채운 작업자료에서 뜬 한 파일짜리 PCLM(제출본 따위).</summary>
    private string SingleFile(string name, string role)
    {
        var work = PclmFile.Create(In("작업-" + name), PclmRole.Work);
        using (var connection = work.Open()) Run(connection, PclmFileTests.채움);
        var saved = PclmFile.Snapshot(work, In(name), role, overwrite: false);
        SqliteConnection.ClearAllPools();
        return saved;
    }

    private string[] Copies() =>
        Directory.Exists(ViewDirectory) ? Directory.GetFiles(ViewDirectory) : [];

    // ── 원본 ────────────────────────────────────────────────────────

    [Fact]
    public void 한_파일짜리_원본은_열어_보아도_바이트도_옆_파일도_그대로다()
    {
        var path = SingleFile("제출_홍길동.pclm", PclmRole.Submission);
        var before = File.ReadAllBytes(path);

        using (var viewed = PclmFile.OpenView(path, ViewDirectory))
        {
            Assert.Equal((path, PclmRole.Submission, Schema.Version), (viewed.SourcePath, viewed.Role, viewed.Version));
            Assert.Equal(Access.Read, viewed.Database.Access);
            Assert.NotEqual(path, viewed.Database.Path);
            Assert.Equal(1, new Reports(viewed.Database).Summary().ContractBases);

            // 사본도 한 파일이다 — 읽기 손잡이가 옆에 아무것도 세우지 않는다.
            Assert.False(File.Exists(viewed.Database.Path + "-wal"));
        }

        SqliteConnection.ClearAllPools();
        Assert.Equal(before, File.ReadAllBytes(path));
        Assert.False(File.Exists(path + "-wal"));
        Assert.False(File.Exists(path + "-shm"));
    }

    /// <summary>
    /// 누가 쓰는 중인 작업자료도 열어 본다. 커밋된 것까지가 보이고, 쥐고 있는 쓰기는 보이지 않는다.
    /// </summary>
    [Fact]
    public void 쓰는_중인_작업자료도_커밋된_것까지_보인다()
    {
        var work = PclmFile.Create(In("계약자료.pclm"), PclmRole.Work);
        using var writer = work.Open();
        Run(writer, PclmFileTests.채움);

        // 쓰기 잠금을 쥔 채 아직 커밋하지 않은 계약 하나.
        Run(writer, "BEGIN IMMEDIATE;");
        Run(writer,
            """
            INSERT INTO contract_series (contract_base) VALUES ('C-2');
            INSERT INTO contract (contract_base, seq, title, updated_at)
            VALUES ('C-2', '0', '아직 커밋 안 한 계약', '2026-09-07T09:00:00.0000000Z');
            """);

        try
        {
            using var viewed = PclmFile.OpenView(work.Path, ViewDirectory);
            Assert.Equal(PclmRole.Work, viewed.Role);
            Assert.Equal(1, new Reports(viewed.Database).Summary().ContractBases);
        }
        finally
        {
            Run(writer, "ROLLBACK;");
        }
    }

    /// <summary>표지가 생기기 전의 v19 자료는 <b>사본이</b> 올라간다. 원본의 판도 바이트도 그대로다.</summary>
    [Fact]
    public void 옛_v19_는_사본을_올리고_원본은_그대로다()
    {
        var path = In("옛 계약자료.db");
        PclmFileTests.LegacyV19(path, PclmFileTests.채움);
        SqliteConnection.ClearAllPools();
        var before = File.ReadAllBytes(path);

        using (var viewed = PclmFile.OpenView(path, ViewDirectory))
        {
            Assert.Equal((PclmRole.Work, Schema.BaselineVersion), (viewed.Role, viewed.Version));

            var copy = PclmFile.Inspect(viewed.Database.Path);
            Assert.Equal((PclmKind.Ok, Schema.Version), (copy.Kind, copy.Version));
            Assert.False(File.Exists(viewed.Database.Path + "-wal"));

            // 뷰가 사본 위에 서 있다 — 계약면을 그대로 읽는다.
            using var connection = viewed.Database.Open();
            using var command = connection.CreateCommand();
            command.CommandText = "SELECT COUNT(*) FROM v_계약;";
            Assert.Equal(1L, Convert.ToInt64(command.ExecuteScalar()));
        }

        SqliteConnection.ClearAllPools();
        Assert.Equal(before, File.ReadAllBytes(path));
        Assert.Equal(Schema.BaselineVersion, Pragma(path, "user_version"));
        Assert.Equal(0, Pragma(path, "application_id"));
    }

    [Fact]
    public void 없거나_PCLM_이_아니거나_판이_맞지_않으면_열지_않고_사본도_남기지_않는다()
    {
        var text = In("글.pclm");
        File.WriteAllText(text, "이건 SQLite 가 아니다");

        var old = In("옛판.pclm");
        using (var connection = new SqliteConnection($"Data Source={old};Pooling=False"))
        {
            connection.Open();
            Run(connection, "CREATE TABLE notice (notice_base TEXT, seq TEXT); PRAGMA user_version = 11;");
        }

        var newer = SingleFile("새판.pclm", PclmRole.Submission);
        using (var connection = new SqliteConnection($"Data Source={newer};Pooling=False"))
        {
            connection.Open();
            Run(connection, "PRAGMA user_version = 999;");
        }

        foreach (var (path, kind) in new[]
                 {
                     (In("없는.pclm"), PclmKind.NotFound), (text, PclmKind.NotPclm),
                     (old, PclmKind.TooOld), (newer, PclmKind.TooNew),
                 })
        {
            var error = Assert.Throws<PclmFileException>(() => PclmFile.OpenView(path, ViewDirectory));
            Assert.Equal(kind, error.Kind);
            Assert.Contains(path, error.Message);
        }

        Assert.Empty(Copies());
        Assert.False(File.Exists(In("없는.pclm")));
    }

    [Fact]
    public void 다_보면_사본을_지운다()
    {
        var path = SingleFile("백업.pclm", PclmRole.Backup);

        var viewed = PclmFile.OpenView(path, ViewDirectory);
        var copy = viewed.Database.Path;

        // 풀에 연결이 남은 채여도 지워진다 — Windows 는 열린 파일을 지우지 못한다.
        Assert.NotEmpty(new Pclm.Core.Storage.Status(viewed.Database).Report().Groups);
        Assert.True(File.Exists(copy));

        viewed.Dispose();
        Assert.False(File.Exists(copy));
        Assert.Empty(Copies());
    }

    [Fact]
    public void 열람_사본_정리는_오래된_것만_걷는다()
    {
        var home = new Home(In("홈"));
        Directory.CreateDirectory(home.ViewDirectory);

        var old = Path.Combine(home.ViewDirectory, "열람-옛것.pclm");
        var fresh = Path.Combine(home.ViewDirectory, "열람-방금.pclm");
        File.WriteAllText(old, "");
        File.WriteAllText(fresh, "");
        File.SetLastWriteTimeUtc(old, DateTime.UtcNow - Home.ViewCopiesKept - TimeSpan.FromHours(1));

        home.PruneViews();

        Assert.False(File.Exists(old));
        Assert.True(File.Exists(fresh));

        // 폴더가 없어도 조용히 지나간다.
        new Home(In("빈 홈")).PruneViews();
        Assert.False(Directory.Exists(In("빈 홈")));
    }

    // ── 읽기 길 ─────────────────────────────────────────────────────

    /// <summary>
    /// 창의 읽기 요청(<c>status</c>·<c>현황</c>·<c>sheet</c>·<c>outline</c>·<c>userColumns</c>·<c>settings</c>·
    /// <c>linkCandidates</c>·<c>requestLinkCandidates</c>·<c>compareLink</c>·<c>compareRequestLink</c>·
    /// <c>deletionPlan</c>·<c>export</c>)이 부르는 Core 함수를 열람 손잡이 위에서 차례로 부른다.
    /// 하나라도 몰래 쓰면 SQLITE_READONLY 로 여기서 선다.
    /// </summary>
    [Fact]
    public void 다리의_읽기_길은_열람_손잡이에서_모두_돈다()
    {
        var path = SingleFile("제출.pclm", PclmRole.Submission);
        using var viewed = PclmFile.OpenView(path, ViewDirectory);
        var database = viewed.Database;
        var store = new Store(database);

        // status · 현황 · outline
        var summary = new Reports(database).Summary();
        Assert.Equal((1, 1, 1), (summary.RequestBases, summary.NoticeBases, summary.ContractBases));
        Assert.NotNull(new Reports(database).Revisions());
        Assert.NotEmpty(new Pclm.Core.Storage.Status(database).Report().Groups);
        Assert.NotEmpty(new Outline(database).Build().Chains);

        // sheet — 다리가 하는 그대로: 키 풀기, 사람 열, 덮개, 그리고 뷰 전부.
        var 계약 = store.Resolve("C-1") ?? new EntityRef("contract", "C-1", "0");
        Assert.NotEmpty(store.UserColumns());
        Assert.NotEmpty(store.UserColumns("contract"));
        Assert.Single(store.Overrides("contract"));
        using (var connection = database.OpenReadOnly())
            foreach (var view in Views.Names)
            {
                using var command = connection.CreateCommand();
                command.CommandText = $"SELECT * FROM \"{view}\";";
                using var reader = command.ExecuteReader();
                while (reader.Read()) { }
            }

        // deletionPlan — 묻기만 하고 지우지 않는다.
        Assert.NotNull(store.PlanDeletion(계약, wholeSeries: true));

        // settings
        Assert.NotNull(new SettingsStore(database).Read());

        // linkCandidates · compareLink
        var linker = new Linker(database);
        Assert.NotEmpty(linker.Work().Contracts);
        Assert.NotEmpty(linker.Notices());
        var 공고 = new EntityRef("notice", "N-1", "0");
        Assert.NotEmpty(linker.Compare(계약, 공고));

        // requestLinkCandidates · compareRequestLink
        var requestLinker = new RequestLinker(database);
        Assert.NotEmpty(requestLinker.Work().Requests);
        Assert.NotEmpty(requestLinker.Notices());
        Assert.NotEmpty(requestLinker.Compare(new EntityRef("request", "R-1", "0"), 공고));

        // export — 파일은 밖에 떨어지고 자료에는 쓰지 않는다.
        var saved = new Exporter(database).Export(In("관리대장.xlsx"));
        Assert.True(File.Exists(saved));
    }
}

using System.Security.Cryptography;
using Microsoft.Data.Sqlite;
using Pclm.Core.Erp;
using Pclm.Core.Storage;
using Xunit;

namespace Pclm.Core.Tests;

/// <summary>
/// 홈과 쪽지(ADR-030). 창·명령줄·확장 호스트가 같은 쪽지 하나를 보고, <b>아무것도 조용히 만들거나
/// 바꾸지 않는지</b>를 본다.
///
/// <para>지키는 것은 "오류 없이 다른 자료를 여는 일" 이다. 쪽지가 깨졌을 때 기본 자리로 물러나거나, 가리키는
/// 파일이 없을 때 빈 자료를 세우거나, 옛 자료를 옮겨 오다 원본을 고치는 일 — 어느 것도 그 자리에서 말하지 않는다.</para>
///
/// <para>홈은 모두 임시 폴더다. 실제 <c>%LOCALAPPDATA%\Pclm</c> 은 건드리지 않는다. 값은 전부 지어낸 것이다.</para>
/// </summary>
public sealed class HomeTests : IDisposable
{
    private readonly string _root = Path.Combine(Path.GetTempPath(), "pclm-home-" + Guid.NewGuid().ToString("N")[..8]);
    private readonly Home _home;

    public HomeTests()
    {
        Directory.CreateDirectory(_root);
        _home = new Home(In("홈"), legacyRoamingConfig: In("roaming", "config.json"));
    }

    public void Dispose()
    {
        SqliteConnection.ClearAllPools();
        try { Directory.Delete(_root, recursive: true); } catch { /* 지우지 못해도 시험 결과는 그대로다 */ }
    }

    private string In(params string[] parts) => Path.Combine([_root, .. parts]);

    /// <summary>파일 바이트의 지문. 열린 연결이 쥐고 있어도 읽는다 — 덮였는지만 보면 된다.</summary>
    private static string Hash(string path)
    {
        using var stream = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete);
        return Convert.ToHexString(SHA256.HashData(stream));
    }

    private static PclmInfo Info(string path) => PclmFile.Inspect(path);

    /// <summary>표지가 생기기 전의 v19 작업자료. 접수 계열 한 줄을 담는다.</summary>
    private static string Legacy(string path)
    {
        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        PclmFileTests.LegacyV19(path, "INSERT INTO request_series (request_base) VALUES ('R-지어냄');");
        return path;
    }

    private static long Count(string path, string table)
    {
        using var connection = new SqliteConnection($"Data Source={path};Mode=ReadOnly;Pooling=False");
        connection.Open();
        using var command = connection.CreateCommand();
        command.CommandText = $"SELECT COUNT(*) FROM {table};";
        return Convert.ToInt64(command.ExecuteScalar());
    }

    private string Work(string name) => PclmFile.Create(In(name), PclmRole.Work).Path;

    private string Submission(string name) =>
        PclmFile.Snapshot(new Database(Work("작업-" + name)), In(name), PclmRole.Submission, overwrite: false);

    private static T Is<T>(HomeState state) where T : HomeState => Assert.IsType<T>(state);

    // ── 처음 실행 ────────────────────────────────────────────────────

    [Fact]
    public void 처음_실행은_홈에_작업자료와_쪽지를_짓는다()
    {
        var ready = Is<HomeState.Ready>(_home.Resolve());

        Assert.Equal(_home.DefaultWorkfile, ready.Database.Path, ignoreCase: true);
        Assert.Empty(ready.Notes);

        var info = Info(_home.DefaultWorkfile);
        Assert.Equal((PclmKind.Ok, PclmRole.Work, Schema.Version), (info.Kind, info.Role, info.Version));

        var config = _home.ReadConfig();
        Assert.Equal(ConfigState.Ok, config.State);
        Assert.Equal(_home.DefaultWorkfile, config.Config!.Workfile, ignoreCase: true);
        Assert.Equal(info.DatasetId, config.Config.DatasetId);
        Assert.False(_home.IsDefault);
    }

    /// <summary>
    /// 지금 쓰는 사람의 옛 자료는 <b>사본을 떠서</b> 올린다. 원본은 바이트 하나 바뀌지 않는다 — 옮긴 사본이
    /// 잘못됐을 때 돌아갈 자리이고, 신원은 이어져 확장의 검토 화면이 끊기지 않는다.
    /// </summary>
    [Fact]
    public void 홈의_옛_자료는_사본을_떠_올리고_원본은_그대로_둔다()
    {
        var legacy = Legacy(Path.Combine(_home.Directory, "pclm.db"));
        var before = Hash(legacy);
        var legacyId = Info(legacy).DatasetId;

        var ready = Is<HomeState.Ready>(_home.Resolve());

        Assert.Contains("옛 자료를 옮겨 왔습니다", Assert.Single(ready.Notes));
        Assert.Contains("원본은 그대로", ready.Notes[0]);
        Assert.Equal(_home.DefaultWorkfile, ready.Database.Path, ignoreCase: true);

        var copied = Info(_home.DefaultWorkfile);
        Assert.Equal((PclmKind.Ok, PclmRole.Work, Schema.Version, legacyId),
            (copied.Kind, copied.Role, copied.Version, copied.DatasetId));
        Assert.Equal(1, Count(_home.DefaultWorkfile, "request_series"));
        Assert.Equal(legacyId, _home.ReadConfig().Config!.DatasetId);

        SqliteConnection.ClearAllPools();
        Assert.Equal(before, Hash(legacy));
        Assert.Equal(Schema.BaselineVersion, Info(legacy).Version);

        // 원본이 백업 노릇을 하므로 사본을 다시 백업하지 않는다. 임시 이름도 남지 않는다.
        Assert.False(Directory.Exists(_home.BackupsDirectory));
        Assert.Empty(Directory.GetFiles(_home.Directory, "*.tmp"));
    }

    [Fact]
    public void 옛_쪽지의_dataDir_자료가_홈의_옛_자료보다_먼저다()
    {
        var roaming = Legacy(In("옛 자리", "pclm.db"));
        Legacy(Path.Combine(_home.Directory, "pclm.db"));
        Directory.CreateDirectory(In("roaming"));
        File.WriteAllText(In("roaming", "config.json"),
            System.Text.Json.JsonSerializer.Serialize(new { dataDir = In("옛 자리") }));
        var before = Hash(roaming);

        var ready = Is<HomeState.Ready>(_home.Resolve());

        Assert.Contains(roaming, ready.Notes.Single());
        Assert.Equal(Info(roaming).DatasetId, Info(_home.DefaultWorkfile).DatasetId);
        SqliteConnection.ClearAllPools();
        Assert.Equal(before, Hash(roaming));
    }

    /// <summary>이사가 밀려 있던 옛 쪽지는 dataDir 이 아직 빈 새 자리다. 자료는 옛 자리(pendingMoveFrom)에 있다.</summary>
    [Fact]
    public void 옛_쪽지에_밀린_이사가_있으면_옛_자리의_자료를_옮겨_온다()
    {
        var real = Legacy(In("옛 자리", "pclm.db"));
        Directory.CreateDirectory(In("roaming"));
        File.WriteAllText(In("roaming", "config.json"), System.Text.Json.JsonSerializer.Serialize(
            new { dataDir = In("아직 빈 새 자리"), pendingMoveFrom = In("옛 자리") }));

        var ready = Is<HomeState.Ready>(_home.Resolve());

        Assert.Contains(real, ready.Notes.Single());
        Assert.Equal(Info(real).DatasetId, Info(_home.DefaultWorkfile).DatasetId);
        Assert.False(Directory.Exists(In("아직 빈 새 자리")));
    }

    [Fact]
    public void 홈에_이미_작업자료가_있으면_덮지_않고_그것을_쓴다()
    {
        Directory.CreateDirectory(_home.Directory);
        var existing = PclmFile.Create(_home.DefaultWorkfile, PclmRole.Work).Path;
        var id = Info(existing).DatasetId;
        Legacy(Path.Combine(_home.Directory, "pclm.db"));

        var ready = Is<HomeState.Ready>(_home.Resolve());

        Assert.Empty(ready.Notes);
        Assert.Equal(id, Info(_home.DefaultWorkfile).DatasetId);
        Assert.Equal(id, _home.ReadConfig().Config!.DatasetId);
    }

    /// <summary>받을 수 없는 옛 자료가 있으면 빈 자료를 세우지 않는다 — 사람은 자료가 사라진 줄 안다.</summary>
    [Fact]
    public void 받을_수_없는_옛_자료가_있으면_짓지_않고_멈춘다()
    {
        Directory.CreateDirectory(_home.Directory);
        using (var connection = new SqliteConnection($"Data Source={Path.Combine(_home.Directory, "pclm.db")};Pooling=False"))
        {
            connection.Open();
            using var command = connection.CreateCommand();
            command.CommandText = "CREATE TABLE t (x); PRAGMA user_version = 11;";
            command.ExecuteNonQuery();
        }

        var problem = Is<HomeState.Problem>(_home.Resolve());

        Assert.Equal(ProblemKind.TooOld, problem.Kind);
        Assert.False(File.Exists(_home.DefaultWorkfile));
        Assert.Equal(ConfigState.Missing, _home.ReadConfig().State);
    }

    // ── 쪽지가 있을 때 ───────────────────────────────────────────────

    [Theory]
    [InlineData("{ 깨진")]
    [InlineData("""{ "version": 2, "workfile": "C:\\x.pclm", "datasetId": "ab" }""")]
    [InlineData("""{ "version": 1, "workfile": "상대\\x.pclm", "datasetId": "ab" }""")]
    [InlineData("""{ "version": 1, "datasetId": "ab" }""")]
    public void 깨진_쪽지는_기본_자리로_물러나지_않고_아무것도_짓지_않는다(string text)
    {
        Directory.CreateDirectory(_home.Directory);
        File.WriteAllText(_home.ConfigPath, text);
        Legacy(Path.Combine(_home.Directory, "pclm.db"));

        var problem = Is<HomeState.Problem>(_home.Resolve());

        Assert.Equal(ProblemKind.ConfigCorrupt, problem.Kind);
        Assert.Equal(_home.ConfigPath, problem.Path);
        Assert.False(File.Exists(_home.DefaultWorkfile));
        Assert.Equal(text, File.ReadAllText(_home.ConfigPath));
    }

    [Fact]
    public void 쪽지가_가리키는_파일이_없으면_아무것도_짓지_않는다()
    {
        var missing = In("지워진 자리", "계약자료.pclm");
        _home.WriteConfig(missing, "ab");

        var problem = Is<HomeState.Problem>(_home.Resolve());

        Assert.Equal((ProblemKind.WorkfileMissing, missing), (problem.Kind, problem.Path));
        Assert.Contains(missing, problem.Message);
        Assert.False(Directory.Exists(In("지워진 자리")));
        Assert.False(File.Exists(_home.DefaultWorkfile));
    }

    [Fact]
    public void 쪽지가_제출본을_가리키면_그_자리에서_열지_않는다()
    {
        var submission = Submission("제출.pclm");
        _home.WriteConfig(submission, Info(submission).DatasetId!);
        var before = Hash(submission);

        var problem = Is<HomeState.Problem>(_home.Resolve());

        Assert.Equal(ProblemKind.NotWork, problem.Kind);
        Assert.Contains("제출본", problem.Message);
        Assert.Equal(before, Hash(submission));
    }

    /// <summary>판올림은 되돌릴 수 없다. 올리기 전에 홈의 backups 에 한 벌 뜬다(ADR-033).</summary>
    [Fact]
    public void 판이_낮으면_백업을_뜬_뒤_올린다()
    {
        var legacy = Legacy(In("작업", "옛것.pclm"));
        var id = Info(legacy).DatasetId!;
        _home.WriteConfig(legacy, id);

        var ready = Is<HomeState.Ready>(_home.Resolve());

        var backup = Assert.Single(Directory.GetFiles(_home.BackupsDirectory));
        Assert.Matches(@"계약자료_v19_\d{8}-\d{6}\.pclm$", backup);
        Assert.Equal((PclmKind.Ok, Schema.BaselineVersion, id), (Info(backup).Kind, Info(backup).Version, Info(backup).DatasetId));
        Assert.Equal(1, Count(backup, "request_series"));
        Assert.Contains(backup, Assert.Single(ready.Notes));

        Assert.Equal(Schema.Version, Info(legacy).Version);
        Assert.Equal(id, Info(legacy).DatasetId);
    }

    [Fact]
    public void 백업은_최근_열_벌만_남기고_남이_둔_파일은_건드리지_않는다()
    {
        Directory.CreateDirectory(_home.BackupsDirectory);
        for (var n = 0; n < 12; n++)
        {
            var old = Path.Combine(_home.BackupsDirectory, $"계약자료_v19_2001010{n % 10}-0000{n:00}.pclm");
            File.WriteAllText(old, "옛 백업");
            File.SetLastWriteTimeUtc(old, new DateTime(2001, 1, 1, 0, 0, 0, DateTimeKind.Utc).AddMinutes(n));
        }
        var memo = Path.Combine(_home.BackupsDirectory, "메모.txt");
        File.WriteAllText(memo, "사람이 둔 것");

        var legacy = Legacy(In("작업", "옛것.pclm"));
        _home.WriteConfig(legacy, Info(legacy).DatasetId!);
        Is<HomeState.Ready>(_home.Resolve());

        var kept = Directory.GetFiles(_home.BackupsDirectory, "계약자료_v*_*.pclm");
        Assert.Equal(Home.BackupsKept, kept.Length);
        // 방금 뜬 것은 남고, 가장 오래된 셋이 지워졌다.
        Assert.Contains(kept, f => Info(f).Kind == PclmKind.Ok);
        Assert.DoesNotContain(kept, f => f.EndsWith("-000000.pclm") || f.EndsWith("-000001.pclm") || f.EndsWith("-000002.pclm"));
        Assert.True(File.Exists(memo));
    }

    /// <summary>
    /// 쪽지의 신원이 파일과 다르면 그 자리에 다른 작업자료가 놓인 것이다. 쪽지를 파일의 것으로 고친다 —
    /// 호스트는 쪽지의 신원으로 대상을 확인하므로 옛 검토 화면의 저장이 이로써 거절된다.
    /// </summary>
    [Fact]
    public void 쪽지와_파일의_신원이_다르면_쪽지를_파일의_것으로_고친다()
    {
        var work = Work("작업.pclm");
        _home.WriteConfig(work, "deadbeef");

        Is<HomeState.Ready>(_home.Resolve());

        Assert.Equal(Info(work).DatasetId, _home.ReadConfig().Config!.DatasetId);
    }

    [Fact]
    public void 쪽지는_임시_파일을_남기지_않고_통째로_바뀐다()
    {
        _home.WriteConfig(In("가.pclm"), "aa");
        _home.WriteConfig(In("나.pclm"), "bb");

        var config = _home.ReadConfig().Config!;
        Assert.Equal((In("나.pclm"), "bb"), (config.Workfile, config.DatasetId));
        Assert.Equal([_home.ConfigPath], Directory.GetFiles(_home.Directory));
    }

    // ── 시작 화면의 두 동작 ───────────────────────────────────────────

    [Fact]
    public void 파일_찾기는_작업자료만_받는다()
    {
        var submission = Submission("제출.pclm");
        var error = Assert.Throws<PclmFileException>(() => _home.Adopt(submission));
        Assert.Contains("제출본", error.Message);
        Assert.Throws<PclmFileException>(() => _home.Adopt(In("없음.pclm")));
        Assert.Equal(ConfigState.Missing, _home.ReadConfig().State);

        var work = Work("내 자료.pclm");
        _home.Adopt(work);
        var config = _home.ReadConfig().Config!;
        Assert.Equal((work, Info(work).DatasetId), (config.Workfile, config.DatasetId));
        Assert.Equal(work, Is<HomeState.Ready>(_home.Resolve()).Database.Path);
    }

    [Fact]
    public void 새로_만들기는_있는_파일을_덮지_않는다()
    {
        var existing = Work("있음.pclm");
        var before = Hash(existing);

        Assert.Throws<IOException>(() => _home.CreateNew(existing));
        Assert.Equal(before, Hash(existing));
        Assert.Equal(ConfigState.Missing, _home.ReadConfig().State);

        var created = _home.CreateNew(In("새 자리", "새것.pclm"));
        Assert.Equal((PclmKind.Ok, PclmRole.Work), (Info(created).Kind, Info(created).Role));
        Assert.Equal(created, _home.ReadConfig().Config!.Workfile);
    }

    [Theory]
    [InlineData(@"\\server\share\계약자료.pclm")]
    [InlineData(@"\\?\UNC\server\share\계약자료.pclm")]
    public void 네트워크_자리는_작업자료로_받지_않는다(string path)
    {
        Assert.Contains("네트워크", Assert.Throws<InvalidOperationException>(() => _home.CreateNew(path)).Message);
        Assert.Contains("네트워크", Assert.Throws<InvalidOperationException>(() => _home.Adopt(path)).Message);
        Assert.Equal(ConfigState.Missing, _home.ReadConfig().State);
    }

    // ── 홈 잠금 ──────────────────────────────────────────────────────

    [Fact]
    public void 홈은_한_번에_하나만_쥔다()
    {
        var first = _home.TryLock();
        Assert.NotNull(first);
        Assert.Null(_home.TryLock());
        Assert.Null(new Home(_home.Directory.ToUpperInvariant() + Path.DirectorySeparatorChar).TryLock());

        using (var other = new Home(In("다른 홈")).TryLock())
            Assert.NotNull(other);

        first!.Dispose();
        using var again = _home.TryLock();
        Assert.NotNull(again);
    }

    // ── 확장 호스트 ──────────────────────────────────────────────────

    [Fact]
    public void 호스트는_쪽지가_가리키는_지금_판의_작업자료만_연다()
    {
        Assert.Throws<InvalidOperationException>(() => ErpConnection.OpenBound(_home));   // 쪽지 없음

        var work = Work("작업.pclm");
        var id = Info(work).DatasetId!;
        _home.WriteConfig(work, id);
        Assert.Equal(work, ErpConnection.OpenBound(_home).Path);

        _home.WriteConfig(work, "deadbeef");
        Assert.ThrowsAny<InvalidOperationException>(() => ErpConnection.OpenBound(_home));

        var submission = Submission("제출.pclm");
        _home.WriteConfig(submission, Info(submission).DatasetId!);
        Assert.ThrowsAny<InvalidOperationException>(() => ErpConnection.OpenBound(_home));

        var missing = In("없음", "계약자료.pclm");
        _home.WriteConfig(missing, id);
        Assert.ThrowsAny<InvalidOperationException>(() => ErpConnection.OpenBound(_home));
        Assert.False(Directory.Exists(In("없음")));
    }

    /// <summary>호스트는 판을 올리지 않는다 — 올리는 것은 백업과 함께 창이 한다.</summary>
    [Fact]
    public void 호스트는_판이_낮은_자료를_올리지_않고_거절한다()
    {
        var legacy = Legacy(In("작업", "옛것.pclm"));
        _home.WriteConfig(legacy, Info(legacy).DatasetId!);

        Assert.ThrowsAny<InvalidOperationException>(() => ErpConnection.OpenBound(_home));
        Assert.Equal(Schema.BaselineVersion, Info(legacy).Version);
    }

    [Fact]
    public void 호스트를_부른_확장은_홈에_적힌_출처와_같아야_한다()
    {
        const string origin = "chrome-extension://abcdefghijklmnopabcdefghijklmnop/";
        Assert.False(ErpConnection.IsAllowedOrigin(_home, origin));

        Directory.CreateDirectory(_home.Directory);
        File.WriteAllText(_home.ExtensionOriginPath, origin + "\n");
        Assert.True(ErpConnection.IsAllowedOrigin(_home, origin));
        Assert.False(ErpConnection.IsAllowedOrigin(_home, "chrome-extension://bbcdefghijklmnopabcdefghijklmnop/"));
        Assert.False(ErpConnection.IsAllowedOrigin(new Home(In("다른 홈")), origin));
    }
}

using System.Security.Cryptography;
using System.Text.Json;
using Microsoft.Data.Sqlite;
using Pclm.Core.Erp;
using Pclm.Core.Storage;
using Xunit;

namespace Pclm.Core.Tests;

/// <summary>
/// 작업자료 바꾸기(ADR-032) — 옮기기·다른 작업자료 쓰기·스냅샷에서 새로·새 계약자료·백업.
///
/// <para>지키는 것은 둘이다. 바꾸는 사이 확장이 저장한 것이 <b>사람이 다음에 여는 자료에 있거나, 거절되거나</b>
/// 둘 중 하나여야 하고, 바꾸다 멈추면 <b>쪽지도 파일도 그대로</b>여야 한다.</para>
///
/// <para>홈은 모두 임시 폴더다. 값은 전부 지어낸 것이다.</para>
/// </summary>
public sealed class WorkfileSwitchTests : IDisposable
{
    private readonly string _root = Path.Combine(Path.GetTempPath(), "pclm-switch-" + Guid.NewGuid().ToString("N")[..8]);
    private readonly Home _home;
    private readonly Database _current;

    public WorkfileSwitchTests()
    {
        Directory.CreateDirectory(_root);
        _home = new Home(In("홈"));
        _current = Assert.IsType<HomeState.Ready>(_home.Resolve()).Database;

        using var connection = _current.Open();
        Run(connection, """
            INSERT INTO request_series (request_base) VALUES ('R-지어냄');
            INSERT INTO request (request_base, seq, title, updated_at) VALUES ('R-지어냄', '00', '지어낸 접수', 't');
            INSERT INTO contract_series (contract_base) VALUES ('C-지어냄');
            INSERT INTO contract (contract_base, seq, title, updated_at) VALUES ('C-지어냄', '00', '지어낸 계약', 't');
            INSERT INTO plan (request_number, item_name, source_name, imported_at) VALUES ('P-지어냄', '지어낸 품명', '지어낸.xlsx', 't');
            """);
    }

    public void Dispose()
    {
        SqliteConnection.ClearAllPools();
        try { Directory.Delete(_root, recursive: true); } catch { /* 지우지 못해도 시험 결과는 그대로다 */ }
    }

    private string In(params string[] parts) => Path.Combine([_root, .. parts]);

    private static void Run(SqliteConnection connection, string sql)
    {
        using var command = connection.CreateCommand();
        command.CommandText = sql;
        command.ExecuteNonQuery();
    }

    private static SqliteConnection Reader(string path)
    {
        var connection = new SqliteConnection($"Data Source={path};Mode=ReadOnly;Pooling=False");
        connection.Open();
        return connection;
    }

    private static long Count(string path, string table)
    {
        using var connection = Reader(path);
        using var command = connection.CreateCommand();
        command.CommandText = $"SELECT COUNT(*) FROM \"{table}\";";
        return Convert.ToInt64(command.ExecuteScalar());
    }

    /// <summary>계약면 뷰 전부를 글로 편다. 옮긴 자리의 뷰가 원본과 한 글자도 다르지 않은지 본다.</summary>
    private static List<string> Dump(string path)
    {
        var lines = new List<string>();
        using var connection = Reader(path);
        foreach (var view in Views.Names.Order(StringComparer.Ordinal))
        {
            using var command = connection.CreateCommand();
            command.CommandText = $"SELECT * FROM \"{view}\";";
            using var reader = command.ExecuteReader();
            while (reader.Read())
                lines.Add(view + "|" + string.Join("|", Enumerable.Range(0, reader.FieldCount)
                    .Select(i => reader.IsDBNull(i) ? "<null>" : reader.GetValue(i).ToString())));
        }
        return lines;
    }

    /// <summary>파일 바이트의 지문. 열린 연결이 쥐고 있어도 읽는다.</summary>
    private static string Hash(string path)
    {
        using var stream = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete);
        return Convert.ToHexString(SHA256.HashData(stream));
    }

    private string ConfigBytes() => Convert.ToHexString(File.ReadAllBytes(_home.ConfigPath));

    private string CurrentId => PclmFile.Inspect(_current.Path).DatasetId!;

    // ── 확장 저장 ────────────────────────────────────────────────────

    /// <summary>확장 호스트가 이 파일에 저장한다. 공고 하나, 충돌 없음.</summary>
    private static Func<JsonElement> PrepareCapture(ErpCapture host, string datasetId, string noticeBase)
    {
        var snapshot = new CaptureInput(
            "g2b-notice-a-v1", Mapping.Hash(Mapping.Defaults()), JsonSerializer.SerializeToElement(new
            {
                pointInfo = new { bidPbancNo = noticeBase, bidPbancOrd = "001", bidPbancNm = "지어낸 공고", pbancInstUntyGrpNm = "시험기관" },
                tables = new { },
            }), "live", Screen: "01179");
        var preview = host.Inspect(snapshot);
        var args = new { datasetId, snapshot, captureId = Guid.NewGuid().ToString(), baseToken = preview.BaseToken,
            choices = new Dictionary<string, string>() };

        return () => JsonSerializer.SerializeToElement(host.Dispatch(JsonSerializer.SerializeToElement(new
            { protocolVersion = 2, requestId = Guid.NewGuid().ToString(), method = "capture", @params = args })), Mapping.Json);
    }

    private ErpCapture Host(string path) => new(path, environment: "development", home: _home);

    // ── 옮기기 ───────────────────────────────────────────────────────

    [Fact]
    public void 옮기기는_같은_자료를_새_자리에_세우고_옛_파일을_retired_로_남긴다()
    {
        var old = _current.Path;
        var id = CurrentId;
        var before = Dump(old);
        var target = In("새 자리", "계약자료.pclm");

        Assert.Equal(target, _home.MoveTo(_current, target), ignoreCase: true);

        var moved = PclmFile.Inspect(target);
        Assert.Equal((PclmKind.Ok, PclmRole.Work, id), (moved.Kind, moved.Role, moved.DatasetId));
        Assert.Equal(before, Dump(target));
        foreach (var table in new[] { "request", "contract", "plan" })
            Assert.Equal(1, Count(target, table));

        var config = _home.ReadConfig().Config!;
        Assert.Equal(target, config.Workfile, ignoreCase: true);
        Assert.Equal(id, config.DatasetId);
        Assert.Equal(old, config.Retired, ignoreCase: true);

        Assert.Equal(PclmRole.Retired, PclmFile.Inspect(old).Role);
        // 옮기기 전에 떠 있던 다른 손잡이도 옛 파일에 쓰지 못한다(ADR-031).
        Assert.Contains("옮겨졌습니다", Assert.Throws<InvalidOperationException>(() => new Database(old).Open()).Message);
        Assert.Empty(Directory.GetFiles(In("새 자리"), "*.tmp-*"));
    }

    [Fact]
    public void 옮긴_뒤_다음_시작은_옛_파일을_지우고_쪽지의_옛_자리를_걷는다()
    {
        var old = _current.Path;
        var target = In("새 자리.pclm");
        _home.MoveTo(_current, target);
        SqliteConnection.ClearAllPools();   // 다음 실행

        var ready = Assert.IsType<HomeState.Ready>(_home.Resolve());

        Assert.Equal(target, ready.Database.Path, ignoreCase: true);
        Assert.Contains(ready.Notes, n => n.Contains("옛 파일을 지웠습니다") && n.Contains(old));
        Assert.False(File.Exists(old));
        Assert.False(File.Exists(old + "-wal"));
        Assert.Null(_home.ReadConfig().Config!.Retired);
    }

    /// <summary>쪽지가 가리키는 옛 자리에 retired 가 아닌 것이 놓였으면 남의 것이다 — 지우지 않고 칸만 걷는다.</summary>
    [Fact]
    public void 옛_자리의_파일이_retired_가_아니면_지우지_않는다()
    {
        var other = PclmFile.Create(In("다른 작업자료.pclm"), PclmRole.Work).Path;
        _home.WriteConfig(_current.Path, CurrentId, retired: other);
        SqliteConnection.ClearAllPools();

        var ready = Assert.IsType<HomeState.Ready>(_home.Resolve());

        Assert.True(File.Exists(other));
        Assert.Empty(ready.Notes);
        Assert.Null(_home.ReadConfig().Config!.Retired);
    }

    [Fact]
    public void 옮길_자리에_파일이_있으면_아무것도_바꾸지_않는다()
    {
        var target = In("남의 파일.pclm");
        File.WriteAllText(target, "남의 것");
        var config = ConfigBytes();

        Assert.Throws<IOException>(() => _home.MoveTo(_current, target));

        Assert.Equal("남의 것", File.ReadAllText(target));
        Assert.Equal(config, ConfigBytes());
        Assert.Equal(PclmRole.Work, PclmFile.Inspect(_current.Path).Role);
        Assert.Empty(Directory.GetFiles(_root, "*.tmp-*", SearchOption.AllDirectories));

        // -wal 만 남은 자리도 비지 않은 것으로 본다.
        var walOnly = In("반쯤.pclm");
        File.WriteAllText(walOnly + "-wal", "");
        Assert.Throws<IOException>(() => _home.MoveTo(_current, walOnly));
        Assert.False(File.Exists(walOnly));
        Assert.Equal(config, ConfigBytes());
    }

    [Fact]
    public void 옮기기_전에_커밋된_저장은_새_자리에_있다()
    {
        Assert.True(PrepareCapture(Host(_current.Path), CurrentId, "R26BK00000001")().GetProperty("ok").GetBoolean());

        var target = In("새 자리.pclm");
        _home.MoveTo(_current, target);

        Assert.Equal(1, Count(target, "notice"));
        Assert.Equal(1, Count(target, "erp_capture"));
    }

    /// <summary>
    /// 전환 경합. 옮기는 창이 잠금을 쥔 사이 저장이 줄을 서면, 잠금이 풀린 뒤 옛 파일이 retired 이고 쪽지가
    /// 새 자리를 가리키므로 거절된다 — 옛 파일에도(곧 지워진다) 새 자리에도(사본은 이미 떴다) 쓰이지 않는다.
    /// </summary>
    [Fact]
    public async Task 옮기는_사이_줄_선_저장은_거절되고_어느_파일에도_쓰이지_않는다()
    {
        var old = _current.Path;
        var capture = PrepareCapture(Host(old), CurrentId, "R26BK00000001");
        Task<JsonElement>? pending = null;

        _home.WhileLocked = () =>
        {
            pending = Task.Run(capture);
            Thread.Sleep(500);
            Assert.False(pending.IsCompleted);   // 잠금 앞에 줄 서 있다
        };

        var target = In("새 자리.pclm");
        _home.MoveTo(_current, target);

        var reply = await pending!;
        Assert.False(reply.GetProperty("ok").GetBoolean());
        Assert.Contains("저장 대상이 바뀌었습니다", reply.GetProperty("error").GetProperty("message").GetString());

        foreach (var path in new[] { old, target })
        {
            Assert.Equal(0, Count(path, "notice"));
            Assert.Equal(0, Count(path, "erp_capture"));
        }
    }

    // ── 다른 작업자료 쓰기 ───────────────────────────────────────────

    [Fact]
    public void 다른_작업자료를_쓰면_쪽지만_바뀌고_지금_자료는_작업자료로_남는다()
    {
        var other = PclmFile.Create(In("다른 작업자료.pclm"), PclmRole.Work).Path;
        var otherId = PclmFile.Inspect(other).DatasetId;

        Assert.Equal(other, _home.UseInPlace(_current, other), ignoreCase: true);

        var config = _home.ReadConfig().Config!;
        Assert.Equal((other, otherId), (config.Workfile, config.DatasetId));
        Assert.Null(config.Retired);
        Assert.Equal(PclmRole.Work, PclmFile.Inspect(_current.Path).Role);
        using (new Database(_current.Path).Open()) { }   // 여전히 쓸 수 있는 작업자료다
    }

    [Fact]
    public void 제출본은_그_자리에서_작업자료로_쓰지_않는다()
    {
        var submission = PclmFile.Snapshot(_current, In("제출.pclm"), PclmRole.Submission, overwrite: false);
        var config = ConfigBytes();

        var error = Assert.Throws<PclmFileException>(() => _home.UseInPlace(_current, submission));

        Assert.Contains("그 자리에서 작업자료로 쓸 수 없습니다", error.Message);
        Assert.Equal(config, ConfigBytes());
    }

    [Fact]
    public async Task 다른_작업자료로_바꾸는_사이_줄_선_저장은_거절된다()
    {
        var other = PclmFile.Create(In("다른 작업자료.pclm"), PclmRole.Work).Path;
        var capture = PrepareCapture(Host(_current.Path), CurrentId, "R26BK00000001");
        Task<JsonElement>? pending = null;

        _home.WhileLocked = () =>
        {
            pending = Task.Run(capture);
            Thread.Sleep(500);
            Assert.False(pending.IsCompleted);
        };

        _home.UseInPlace(_current, other);

        Assert.False((await pending!).GetProperty("ok").GetBoolean());
        Assert.Equal(PclmRole.Work, PclmFile.Inspect(_current.Path).Role);
        Assert.Equal(0, Count(_current.Path, "notice"));
        Assert.Equal(0, Count(other, "notice"));
    }

    // ── 스냅샷에서 새로 ──────────────────────────────────────────────

    [Fact]
    public void 제출본에서_새_계약자료를_만들면_새_신원의_작업자료가_서고_원본은_그대로다()
    {
        var submission = PclmFile.Snapshot(_current, In("제출.pclm"), PclmRole.Submission, overwrite: false);
        var submissionId = PclmFile.Inspect(submission).DatasetId;
        var before = Hash(submission);
        var target = In("이어서.pclm");

        _home.CreateFromSnapshot(_current, submission, target);

        var made = PclmFile.Inspect(target);
        Assert.Equal((PclmKind.Ok, PclmRole.Work, Schema.Version), (made.Kind, made.Role, made.Version));
        Assert.NotEqual(submissionId, made.DatasetId);
        Assert.NotEqual(CurrentId, made.DatasetId);
        Assert.Equal(Dump(submission), Dump(target));

        var config = _home.ReadConfig().Config!;
        Assert.Equal((target, made.DatasetId), (config.Workfile, config.DatasetId), new PathAndId());
        Assert.Equal(PclmRole.Work, PclmFile.Inspect(_current.Path).Role);

        SqliteConnection.ClearAllPools();
        Assert.Equal(before, Hash(submission));
        Assert.False(File.Exists(target + "-wal"));
    }

    [Fact]
    public void 옛_v19_파일에서_새_계약자료를_만들면_사본을_올리고_원본은_그대로다()
    {
        var legacy = In("옛 자료.db");
        PclmFileTests.LegacyV19(legacy, """
            INSERT INTO request_series (request_base) VALUES ('R-옛것');
            INSERT INTO request (request_base, seq, updated_at) VALUES ('R-옛것', '00', 't');
            """);
        var legacyId = PclmFile.Inspect(legacy).DatasetId;
        SqliteConnection.ClearAllPools();
        var before = Hash(legacy);
        var target = In("올린 것.pclm");

        _home.CreateFromSnapshot(_current, legacy, target);

        var made = PclmFile.Inspect(target);
        Assert.Equal((PclmKind.Ok, PclmRole.Work, Schema.Version), (made.Kind, made.Role, made.Version));
        Assert.NotEqual(legacyId, made.DatasetId);
        Assert.Equal(1, Count(target, "request"));
        Assert.False(File.Exists(target + "-wal"));
        Assert.Empty(Directory.GetFiles(_root, "*.tmp-*"));

        SqliteConnection.ClearAllPools();
        Assert.Equal(before, Hash(legacy));
        Assert.Equal(Schema.BaselineVersion, PclmFile.Inspect(legacy).Version);
    }

    [Fact]
    public void 새_계약자료를_뜰_자리에_파일이_있으면_덮지_않는다()
    {
        var submission = PclmFile.Snapshot(_current, In("제출.pclm"), PclmRole.Submission, overwrite: false);
        var target = In("있는 것.pclm");
        File.WriteAllText(target, "남의 것");
        var config = ConfigBytes();

        Assert.Throws<IOException>(() => _home.CreateFromSnapshot(_current, submission, target));

        Assert.Equal("남의 것", File.ReadAllText(target));
        Assert.Equal(config, ConfigBytes());
        Assert.Empty(Directory.GetFiles(_root, "*.tmp-*"));
    }

    // ── 새 계약자료 ──────────────────────────────────────────────────

    [Fact]
    public void 새_계약자료는_빈_작업자료를_지어_쪽지를_옮긴다()
    {
        var target = In("빈 것.pclm");

        _home.CreateNewWhileRunning(_current, target);

        var made = PclmFile.Inspect(target);
        Assert.Equal((PclmKind.Ok, PclmRole.Work), (made.Kind, made.Role));
        Assert.Equal(0, Count(target, "request"));
        var config = _home.ReadConfig().Config!;
        Assert.Equal((target, made.DatasetId), (config.Workfile, config.DatasetId), new PathAndId());
        Assert.Equal(PclmRole.Work, PclmFile.Inspect(_current.Path).Role);
    }

    [Fact]
    public void 새_계약자료를_지을_자리에_파일이_있으면_덮지_않는다()
    {
        var target = In("있는 것.pclm");
        File.WriteAllText(target, "남의 것");
        var config = ConfigBytes();

        Assert.Throws<IOException>(() => _home.CreateNewWhileRunning(_current, target));

        Assert.Equal("남의 것", File.ReadAllText(target));
        Assert.Equal(config, ConfigBytes());
    }

    // ── 백업 ─────────────────────────────────────────────────────────

    [Fact]
    public void 백업은_새_신원의_한_파일이고_쪽지는_그대로다()
    {
        var config = ConfigBytes();
        var target = In("백업.pclm");

        Assert.Equal(target, Home.BackupTo(_current, target), ignoreCase: true);

        var backup = PclmFile.Inspect(target);
        Assert.Equal((PclmKind.Ok, PclmRole.Backup), (backup.Kind, backup.Role));
        Assert.NotEqual(CurrentId, backup.DatasetId);
        Assert.False(File.Exists(target + "-wal"));
        Assert.False(File.Exists(target + "-shm"));
        Assert.Equal(1, Count(target, "contract"));
        Assert.Equal(config, ConfigBytes());

        // 사람이 고른 자리의 옛 백업은 덮는다.
        Home.BackupTo(_current, target);
        Assert.Equal(PclmRole.Backup, PclmFile.Inspect(target).Role);
    }

    [Fact]
    public void 백업은_작업자료를_덮지_않는다()
    {
        var other = PclmFile.Create(In("다른 작업자료.pclm"), PclmRole.Work).Path;
        SqliteConnection.ClearAllPools();
        var before = Hash(other);

        Assert.Throws<InvalidOperationException>(() => Home.BackupTo(_current, other));
        Assert.Throws<InvalidOperationException>(() => Home.BackupTo(_current, _current.Path));

        Assert.Equal(before, Hash(other));
        Assert.Equal(PclmRole.Work, PclmFile.Inspect(_current.Path).Role);
    }

    // ── 네트워크 자리 ────────────────────────────────────────────────

    [Fact]
    public void 네트워크_자리는_어느_동작에서도_거절한다()
    {
        const string unc = @"\\지어낸서버\공유\계약자료.pclm";
        var submission = PclmFile.Snapshot(_current, In("제출.pclm"), PclmRole.Submission, overwrite: false);
        var config = ConfigBytes();

        Assert.Throws<InvalidOperationException>(() => _home.MoveTo(_current, unc));
        Assert.Throws<InvalidOperationException>(() => _home.UseInPlace(_current, unc));
        Assert.Throws<InvalidOperationException>(() => _home.CreateFromSnapshot(_current, submission, unc));
        Assert.Throws<InvalidOperationException>(() => _home.CreateNewWhileRunning(_current, unc));
        Assert.Throws<InvalidOperationException>(() => Home.BackupTo(_current, unc));

        Assert.Equal(config, ConfigBytes());
        Assert.Equal(PclmRole.Work, PclmFile.Inspect(_current.Path).Role);
    }

    /// <summary>쪽지의 경로는 온전한 경로로 적히므로 대소문자만 접어 견준다.</summary>
    private sealed class PathAndId : IEqualityComparer<(string, string?)>
    {
        public bool Equals((string, string?) x, (string, string?) y) =>
            string.Equals(x.Item1, y.Item1, StringComparison.OrdinalIgnoreCase) && x.Item2 == y.Item2;

        public int GetHashCode((string, string?) obj) => 0;
    }
}

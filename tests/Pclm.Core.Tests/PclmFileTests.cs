using Microsoft.Data.Sqlite;
using Pclm.Core.Storage;
using Xunit;

namespace Pclm.Core.Tests;

/// <summary>
/// 파일이 스스로 PCLM 임과 역할을 밝히고, 빈 자료가 서는 길이 <see cref="PclmFile.Create"/> 하나뿐인지를 본다
/// (ADR-031).
///
/// <para>지키는 것은 "오류 없이 엉뚱한 파일을 여는 일" 이다. 경로 오타 하나가 빈 자료를 세우거나, 제출본이
/// 작업자료와 같은 신원을 달고 나가거나, 읽으려고 연 남의 파일이 판올림으로 바뀌는 일 — 셋 다 그 자리에서는
/// 아무 말도 하지 않는다.</para>
///
/// <para>값은 전부 지어낸 것이다.</para>
/// </summary>
public sealed class PclmFileTests : IDisposable
{
    private readonly string _root = Path.Combine(Path.GetTempPath(), "pclm-file-" + Guid.NewGuid().ToString("N")[..8]);

    public PclmFileTests() => Directory.CreateDirectory(_root);

    public void Dispose()
    {
        SqliteConnection.ClearAllPools();
        try { Directory.Delete(_root, recursive: true); } catch { /* 지우지 못해도 시험 결과는 그대로다 */ }
    }

    private string In(string name) => Path.Combine(_root, name);

    // ── 지어낸 자료 ──────────────────────────────────────────────────

    /// <summary>접수 → 공고 → 계약 한 줄과 품목·링크·사람 값·덮개·계획. 뷰마다 줄이 서게 채운다.</summary>
    internal const string 채움 =
        """
        INSERT INTO request_series (request_base) VALUES ('R-1');
        INSERT INTO request (request_base, seq, title, received_on, updated_at)
        VALUES ('R-1', '0', '지어낸 접수', '2026/09/01', '2026-09-01T09:00:00.0000000Z');
        INSERT INTO request_item (request_base, seq, line_no, item_name, quantity, unit_price)
        VALUES ('R-1', '0', 1, '지어낸 품목', 10, '1,000');

        INSERT INTO notice_group  (group_base) VALUES ('N-1');
        INSERT INTO notice_series (notice_base, group_base) VALUES ('N-1', 'N-1');
        INSERT INTO notice (notice_base, seq, title, posted_at, updated_at)
        VALUES ('N-1', '0', '지어낸 공고', '2026/09/02', '2026-09-02T09:00:00.0000000Z');
        INSERT INTO notice_item (notice_base, seq, line_no, item_name, quantity, unit_price)
        VALUES ('N-1', '0', 1, '지어낸 품목', 10, '1,000');

        INSERT INTO contract_series (contract_base) VALUES ('C-1');
        INSERT INTO contract (contract_base, seq, title, updated_at)
        VALUES ('C-1', '0', '지어낸 계약', '2026-09-03T09:00:00.0000000Z');
        INSERT INTO contract_item (contract_base, seq, line_no, item_name, quantity)
        VALUES ('C-1', '0', 1, '지어낸 품목', 10);

        INSERT INTO request_link (request_base, notice_group, confidence, confirmed_at)
        VALUES ('R-1', 'N-1', 1.0, '2026-09-04T09:00:00.0000000Z');
        INSERT INTO project_link (contract_base, notice_group, confidence, confirmed_at)
        VALUES ('C-1', 'N-1', 1.0, '2026-09-04T09:00:00.0000000Z');

        INSERT INTO contract_user_field (contract_base, field_name, value, updated_at)
        VALUES ('C-1', '메모', '지어낸 메모', '2026-09-05T09:00:00.0000000Z');
        INSERT INTO field_override (entity_type, base, seq, column_name, value, original, updated_at)
        VALUES ('contract', 'C-1', '0', '계약금액', '1,234', '', '2026-09-05T09:00:00.0000000Z');

        INSERT INTO plan (request_number, item_name, quantity, source_name, imported_at)
        VALUES ('P-1', '지어낸 계획 품목', 3, '지어낸 계획.xlsx', '2026-09-06T09:00:00.0000000Z');
        """;

    /// <summary>
    /// 표지가 생기기 전(v19)의 작업자료. 0.7.0 이 지은 꼴 그대로 — 기준선 스키마에 WAL, 그리고 뷰.
    /// </summary>
    internal static void LegacyV19(string path, string? sql = null)
    {
        using var connection = new SqliteConnection($"Data Source={path};Pooling=False");
        connection.Open();
        Run(connection, "PRAGMA journal_mode = WAL;");
        Run(connection, Schema.Baseline);
        Run(connection, $"PRAGMA user_version = {Schema.BaselineVersion};");
        if (sql is not null) Run(connection, sql);

        var columns = new List<UserColumnDefinition>();
        using (var command = connection.CreateCommand())
        {
            command.CommandText = "SELECT entity_type, field_name, kind, options, sort_order FROM user_column;";
            using var reader = command.ExecuteReader();
            while (reader.Read())
                columns.Add(new(reader.GetString(0), reader.GetString(1), reader.GetString(2),
                    reader.IsDBNull(3) ? null : reader.GetString(3), reader.GetInt64(4)));
        }

        foreach (var definition in Views.Definitions(columns)) Run(connection, definition);
    }

    /// <summary>WAL 이 아닌 한 파일로 떨어진 PCLM. 읽기만 하면 옆에 아무것도 생기지 않아야 하는 파일이다.</summary>
    private string SingleFile(string name, string role)
    {
        var work = PclmFile.Create(In("작업-" + name), PclmRole.Work);
        using (var connection = work.Open()) Run(connection, 채움);
        return PclmFile.Snapshot(work, In(name), role, overwrite: false);
    }

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

    /// <summary>뷰 하나의 줄 전부. 차례에 기대지 않게 글로 바꿔 정렬한다.</summary>
    private static List<string> Rows(SqliteConnection connection, string view)
    {
        using var command = connection.CreateCommand();
        command.CommandText = $"SELECT * FROM \"{view}\";";
        var rows = new List<string>();
        using var reader = command.ExecuteReader();
        while (reader.Read())
            rows.Add(string.Join("\u0001", Enumerable.Range(0, reader.FieldCount)
                .Select(i => reader.IsDBNull(i) ? "<NULL>" : Convert.ToString(reader.GetValue(i)))));
        rows.Sort(StringComparer.Ordinal);
        return rows;
    }

    private static Dictionary<string, List<string>> AllViews(string path)
    {
        using var connection = new SqliteConnection($"Data Source={path};Mode=ReadOnly;Pooling=False");
        connection.Open();
        return Views.Names.ToDictionary(v => v, v => Rows(connection, v));
    }

    /// <summary>SQLite 가 <b>읽기 전용이라서</b> 거절했는가(SQLITE_READONLY = 8). 다른 까닭의 실패는 지나가지 않는다.</summary>
    private static void Refused(Action write) =>
        Assert.Equal(8, Assert.ThrowsAny<SqliteException>(write).SqliteErrorCode);

    private void AssertUntouched(string path, byte[] before)
    {
        SqliteConnection.ClearAllPools();
        Assert.Equal(before, File.ReadAllBytes(path));
        Assert.False(File.Exists(path + "-wal"));
        Assert.False(File.Exists(path + "-shm"));
    }

    // ── 알아보기 ────────────────────────────────────────────────────

    [Fact]
    public void 없는_파일은_NotFound_이고_아무것도_만들지_않는다()
    {
        var path = Path.Combine(_root, "없는 폴더", "계약자료.pclm");

        Assert.Equal(PclmKind.NotFound, PclmFile.Inspect(path).Kind);
        Assert.False(Directory.Exists(Path.GetDirectoryName(path)));
    }

    [Fact]
    public void 텍스트_빈_파일_일반_SQLite_는_PCLM_이_아니다()
    {
        var text = In("글.pclm");
        File.WriteAllText(text, "이건 SQLite 가 아니다");

        var empty = In("빈.pclm");
        File.WriteAllBytes(empty, []);

        var plain = In("일반.db");
        using (var connection = new SqliteConnection($"Data Source={plain};Pooling=False"))
        {
            connection.Open();
            Run(connection, "CREATE TABLE notice (notice_base TEXT, seq TEXT);");
        }

        // 판 번호만 같은 남의 SQLite — 옛 이름표(erp_dataset)가 없으면 알아보지 않는다.
        var lookalike = In("판만같음.db");
        using (var connection = new SqliteConnection($"Data Source={lookalike};Pooling=False"))
        {
            connection.Open();
            Run(connection, $"CREATE TABLE notice (notice_base TEXT); PRAGMA user_version = {Schema.BaselineVersion};");
        }

        foreach (var path in new[] { text, empty, plain, lookalike })
        {
            var before = File.ReadAllBytes(path);
            Assert.Equal(PclmKind.NotPclm, PclmFile.Inspect(path).Kind);
            AssertUntouched(path, before);
        }
    }

    [Fact]
    public void 옛_시험판은_TooOld_새_판은_TooNew()
    {
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

        Assert.Equal(new PclmInfo(PclmKind.TooOld, null, null, 11), PclmFile.Inspect(old));
        Assert.Equal(new PclmInfo(PclmKind.TooNew, null, null, 999), PclmFile.Inspect(newer));
    }

    /// <summary>임시 규칙 — 표지가 생기기 전의 v19 작업자료는 PCLM 으로 알아본다(1.0 전에 걷는다).</summary>
    [Fact]
    public void 표지_없는_v19_작업자료는_작업자료로_알아본다()
    {
        var path = In("옛 계약자료.db");
        LegacyV19(path);

        string datasetId;
        using (var connection = new SqliteConnection($"Data Source={path};Mode=ReadOnly;Pooling=False"))
        {
            connection.Open();
            using var command = connection.CreateCommand();
            command.CommandText = "SELECT dataset_id FROM erp_dataset;";
            datasetId = (string)command.ExecuteScalar()!;
        }

        Assert.Equal(new PclmInfo(PclmKind.Ok, PclmRole.Work, datasetId, Schema.BaselineVersion), PclmFile.Inspect(path));
    }

    [Fact]
    public void 역할마다_제_역할로_알아본다()
    {
        foreach (var role in PclmRole.All)
        {
            var path = In($"{role}.pclm");
            PclmFile.Create(path, role);

            var info = PclmFile.Inspect(path);
            Assert.Equal((PclmKind.Ok, role, Schema.Version), (info.Kind, info.Role, info.Version));
            Assert.Matches("^[0-9a-f]{32}$", info.DatasetId);
            Assert.Equal(PclmFile.ApplicationId, Pragma(path, "application_id"));
        }
    }

    /// <summary>WAL 이 아닌 파일은 들여다본 뒤에도 옆에 아무것도 생기지 않고 한 바이트도 바뀌지 않는다.</summary>
    [Fact]
    public void 들여다보기는_파일을_바꾸지_않는다()
    {
        var path = SingleFile("제출.pclm", PclmRole.Submission);
        SqliteConnection.ClearAllPools();
        var before = File.ReadAllBytes(path);

        Assert.Equal(PclmKind.Ok, PclmFile.Inspect(path).Kind);
        AssertUntouched(path, before);
    }

    // ── 짓기와 열기 ─────────────────────────────────────────────────

    [Fact]
    public void 있는_자리에는_짓지_않고_한_바이트도_건드리지_않는다()
    {
        var other = In("남의 것.pclm");
        File.WriteAllText(other, "남의 것");
        var before = File.ReadAllBytes(other);

        Assert.Throws<IOException>(() => PclmFile.Create(other, PclmRole.Work));
        Assert.Equal(before, File.ReadAllBytes(other));

        // 본체가 없어도 -wal 이 남아 있으면 누가 쓰던 자리다.
        var orphan = In("고아.pclm");
        File.WriteAllText(orphan + "-wal", "");
        Assert.Throws<IOException>(() => PclmFile.Create(orphan, PclmRole.Work));
        Assert.False(File.Exists(orphan));
    }

    [Fact]
    public void 없는_파일은_작업자료로_열지_않고_만들지도_않는다()
    {
        var path = Path.Combine(_root, "오타", "계약자료.pclm");

        var error = Assert.Throws<PclmFileException>(() => PclmFile.OpenWork(path));
        Assert.Equal(PclmKind.NotFound, error.Kind);
        Assert.Contains(path, error.Message);
        Assert.False(Directory.Exists(Path.GetDirectoryName(path)));

        // Database 손잡이도 짓지 않는다 — 열면 SQLite 가 실패하고 끝이다.
        var database = new Database(path);
        Assert.ThrowsAny<SqliteException>(() => database.Open().Dispose());
        Assert.False(Directory.Exists(Path.GetDirectoryName(path)));
    }

    [Fact]
    public void 제출본은_작업자료로_열지_않는다()
    {
        var path = SingleFile("제출.pclm", PclmRole.Submission);

        var error = Assert.Throws<PclmFileException>(() => PclmFile.OpenWork(path));
        Assert.Equal((PclmKind.Ok, PclmRole.Submission), (error.Kind, error.Role));
        Assert.Contains("제출본", error.Message);
    }

    // ── v19 → v20 ───────────────────────────────────────────────────

    /// <summary>
    /// 지금 쓰는 사람의 v19 자료가 그대로 열려 v20 으로 올라간다. 신원이 이어져야 확장의 검토 화면이
    /// 끊기지 않고, 계약면의 줄은 한 칸도 달라지면 안 된다.
    /// </summary>
    [Fact]
    public void v19_작업자료는_신원과_계약면을_지닌_채_v20_이_된다()
    {
        var path = In("옛 계약자료.db");
        LegacyV19(path, 채움);
        var before = AllViews(path);
        var legacy = PclmFile.Inspect(path);

        Assert.Contains(before.Values, rows => rows.Count > 0);

        PclmFile.OpenWork(path).Migrate();

        var after = PclmFile.Inspect(path);
        Assert.Equal(new PclmInfo(PclmKind.Ok, PclmRole.Work, legacy.DatasetId, Schema.Version), after);
        Assert.Equal(PclmFile.ApplicationId, Pragma(path, "application_id"));

        foreach (var view in Views.Names)
            Assert.True(before[view].SequenceEqual(AllViews(path)[view]), $"{view} 의 줄이 판올림 뒤 달라졌습니다");
    }

    // ── 읽기 전용 ───────────────────────────────────────────────────

    /// <summary>
    /// 읽기 손잡이로는 어느 길로도 쓰지 못한다 — SQLite 가 거절한다. 읽는 길은 그대로 돈다.
    /// 끝난 뒤 파일은 한 바이트도 바뀌지 않는다.
    /// </summary>
    [Fact]
    public void 읽기_손잡이는_쓰지_못하고_읽기만_한다()
    {
        var path = SingleFile("열람.pclm", PclmRole.Submission);
        SqliteConnection.ClearAllPools();
        var before = File.ReadAllBytes(path);

        var database = new Database(path, Access.Read);
        var store = new Store(database);
        var 계약 = new EntityRef("contract", "C-1", "0");

        Refused(() => store.SetUserField(계약, "진행상태", "납품"));
        Refused(() => store.SetOverride(계약, "계약금액", "9,999"));
        Refused(() => store.Delete(계약, wholeSeries: false));
        Refused(() => new SettingsStore(database).Save(submitterName: "홍길동"));

        Assert.Equal(1, new Reports(database).Summary().ContractBases);
        Assert.NotEmpty(store.UserColumns());
        using (var connection = database.Open())
            Assert.Single(Rows(connection, "v_계약"));

        // 지금 판이면 아무것도 하지 않는다 — 뷰를 다시 짓는 것도 쓰기다.
        Assert.Null(database.Migrate());

        AssertUntouched(path, before);
    }

    [Fact]
    public void 읽기_손잡이는_판을_올리지_않는다()
    {
        var path = In("옛 계약자료.db");
        LegacyV19(path, 채움);
        using (var connection = new SqliteConnection($"Data Source={path};Pooling=False"))
        {
            connection.Open();
            Run(connection, "PRAGMA journal_mode = DELETE;");
        }

        var before = File.ReadAllBytes(path);

        Assert.Throws<InvalidOperationException>(() => new Database(path, Access.Read).Migrate());
        AssertUntouched(path, before);
        Assert.Equal(Schema.BaselineVersion, Pragma(path, "user_version"));
    }

    // ── 사본 ────────────────────────────────────────────────────────

    /// <summary>
    /// 제출본은 자기 역할과 <b>새 신원</b>을 달고 한 파일로 떨어진다. 작업자료는 그대로다.
    /// </summary>
    [Fact]
    public void 제출본은_새_신원을_달고_한_파일로_떨어진다()
    {
        var work = PclmFile.Create(In("계약자료.pclm"), PclmRole.Work);
        using (var connection = work.Open()) Run(connection, 채움);
        var source = PclmFile.Inspect(work.Path);

        var saved = PclmFile.Snapshot(work, In("제출_홍길동.pclm"), PclmRole.Submission, overwrite: false);
        SqliteConnection.ClearAllPools();

        var info = PclmFile.Inspect(saved);
        Assert.Equal((PclmKind.Ok, PclmRole.Submission, Schema.Version), (info.Kind, info.Role, info.Version));
        Assert.NotEqual(source.DatasetId, info.DatasetId);
        Assert.Equal(PclmFile.ApplicationId, Pragma(saved, "application_id"));
        Assert.False(File.Exists(saved + "-wal"));
        Assert.False(File.Exists(saved + "-shm"));
        Assert.Single(AllViews(saved)["v_계약"]);

        Assert.Equal(source, PclmFile.Inspect(work.Path));
    }

    /// <summary>
    /// 결과물(제출본·취합본·백업)은 덮어쓰기로 뜨지만 <b>작업자료는 덮지 않는다</b>. 저장 창에서 이름을 잘못 고른
    /// 한 번에 일거리가 결과물 사본으로 바뀌면 되돌릴 길이 없다 — 자기 작업자료든, 다른 작업자료든, 표지 전의
    /// v19 작업자료든, 옮겨진 옛 자료든.
    /// </summary>
    [Fact]
    public void 제출본은_작업자료_자리를_덮지_않는다()
    {
        var work = PclmFile.Create(In("계약자료.pclm"), PclmRole.Work);
        using (var connection = work.Open()) Run(connection, 채움);

        var other = PclmFile.Create(In("다른_계약자료.pclm"), PclmRole.Work).Path;
        var legacy = In("pclm.db");
        LegacyV19(legacy);
        var retired = PclmFile.Create(In("옮긴_옛것.pclm"), PclmRole.Retired).Path;
        SqliteConnection.ClearAllPools();

        foreach (var target in new[] { work.Path, other, legacy, retired })
        {
            var before = File.ReadAllBytes(target);
            Assert.Throws<InvalidOperationException>(
                () => PclmFile.Snapshot(work, target, PclmRole.Submission, overwrite: true));
            SqliteConnection.ClearAllPools();
            Assert.Equal(before, File.ReadAllBytes(target));
        }

        // 지난 제출본은 결과물이라 덮는다.
        var old = PclmFile.Snapshot(work, In("제출_지난.pclm"), PclmRole.Submission, overwrite: false);
        var again = PclmFile.Snapshot(work, old, PclmRole.Submission, overwrite: true);
        Assert.Equal(PclmRole.Submission, PclmFile.Inspect(again).Role);
    }

    /// <summary>취합본의 자리가 작업자료면 제출본을 읽기 전에 멈춘다.</summary>
    [Fact]
    public void 취합본은_작업자료_자리에_짓지_않는다()
    {
        var work = PclmFile.Create(In("계약자료.pclm"), PclmRole.Work);
        using (var connection = work.Open()) Run(connection, 채움);
        var submission = PclmFile.Snapshot(work, In("제출_홍길동.pclm"), PclmRole.Submission, overwrite: false);
        SqliteConnection.ClearAllPools();
        var before = File.ReadAllBytes(work.Path);

        Assert.Throws<InvalidOperationException>(() => new Merger().Merge([submission], work.Path));
        SqliteConnection.ClearAllPools();
        Assert.Equal(before, File.ReadAllBytes(work.Path));
    }
}

using Microsoft.Data.Sqlite;

namespace Pclm.Core.Storage;

/// <summary>파일을 들여다본 결과의 갈래.</summary>
public enum PclmKind
{
    /// <summary>그 자리에 파일이 없다.</summary>
    NotFound,

    /// <summary>SQLite 가 아니거나, SQLite 라도 PCLM 표지가 없다. 열지 않는다.</summary>
    NotPclm,

    /// <summary>기준선보다 옛 시험판(v1~v18). 이 프로그램이 올리지 못한다.</summary>
    TooOld,

    /// <summary>이 프로그램보다 새 판. 내릴 길이 없다.</summary>
    TooNew,

    /// <summary>이 프로그램이 받을 수 있는 PCLM 파일. 역할은 <see cref="PclmInfo.Role"/> 에 있다.</summary>
    Ok,
}

/// <summary>
/// <see cref="PclmFile.Inspect"/> 의 결과. <paramref name="Role"/>·<paramref name="DatasetId"/> 는
/// <see cref="PclmKind.Ok"/> 일 때만 찬다. <paramref name="Version"/> 은 읽었으면 그 판, 못 읽었으면 0.
/// </summary>
public sealed record PclmInfo(PclmKind Kind, string? Role, string? DatasetId, int Version);

/// <summary>
/// 파일의 역할. <c>pclm_file.role</c> 에 적히는 글자 그대로다 — 스키마의 CHECK 와 같은 다섯.
///
/// <para>쓰기를 허락하는 것은 <see cref="Work"/> 하나다. 제출본·취합본·백업은 그때의 기록이라
/// 그 자리에서 고치는 길이 없고, 일하려면 사본을 떠서 새 작업자료로 삼는다(ADR-032).</para>
/// </summary>
public static class PclmRole
{
    public const string Work = "work";
    public const string Submission = "submission";
    public const string Merged = "merged";
    public const string Backup = "backup";
    public const string Retired = "retired";

    public static IReadOnlyList<string> All { get; } = [Work, Submission, Merged, Backup, Retired];

    /// <summary>사람에게 보일 이름.</summary>
    public static string Name(string? role) => role switch
    {
        Work => "작업자료",
        Submission => "제출본",
        Merged => "취합본",
        Backup => "백업",
        Retired => "옮겨진 옛 자료",
        _ => $"알 수 없는 역할({role})",
    };

    internal static void Require(string role)
    {
        if (!All.Contains(role))
            throw new ArgumentException($"모르는 역할입니다: {role}", nameof(role));
    }
}

/// <summary>열지 않기로 한 파일. 무엇이 왜 안 되는지를 함께 든다.</summary>
public sealed class PclmFileException(string path, PclmKind kind, string? role, string message)
    : InvalidOperationException(message)
{
    public string Path { get; } = path;
    public PclmKind Kind { get; } = kind;
    public string? Role { get; } = role;
}

/// <summary>
/// 열어 본 파일(<see cref="PclmFile.OpenView"/>). 손잡이는 <b>원본이 아니라 임시 사본</b>을 가리키는 읽기 손잡이다.
///
/// <para>다 보면 <see cref="Dispose"/> 로 사본을 지운다. 지우지 못하고 남은 것은 다음 시작에서
/// <see cref="Home.PruneViews"/> 가 걷는다.</para>
/// </summary>
public sealed class ViewedFile : IDisposable
{
    internal ViewedFile(Database database, string sourcePath, string role, int version)
    {
        Database = database;
        SourcePath = sourcePath;
        Role = role;
        Version = version;
    }

    /// <summary>사본 위의 읽기 손잡이(<see cref="Access.Read"/>).</summary>
    public Database Database { get; }

    /// <summary>사람이 고른 원본의 온전한 경로. 화면에 보이는 것은 이것이다.</summary>
    public string SourcePath { get; }

    /// <summary>원본의 역할(<see cref="PclmRole"/>).</summary>
    public string Role { get; }

    /// <summary>원본의 판. 사본은 이 프로그램의 판까지 올라가 있을 수 있다.</summary>
    public int Version { get; }

    public void Dispose() => Discard(Database.Path);

    /// <summary>
    /// 사본을 지운다. 풀에 남은 연결이 쥐고 있으면 지워지지 않으므로 먼저 놓는다.
    /// 그래도 못 지우면 둔다 — 다음 시작의 <see cref="Home.PruneViews"/> 가 걷는다.
    /// </summary>
    internal static void Discard(string copy)
    {
        new Database(copy, Access.Read).ReleasePool();
        foreach (var suffix in new[] { "", "-wal", "-shm", "-journal" })
        {
            try { File.Delete(copy + suffix); }
            catch (Exception e) when (e is IOException or UnauthorizedAccessException) { /* 다음 시작에서 걷는다 */ }
        }
    }
}

/// <summary>
/// <c>.pclm</c> 파일을 알아보고, 짓고, 연다.
///
/// <para><b>생성과 열기를 가른다</b>(ADR-031). <see cref="Create"/> 는 있는 파일을 덮지 않고,
/// <see cref="OpenWork"/> 는 없는 파일을 짓지 않는다. 그 둘을 한 길로 두면 경로 오타 하나가 빈 자료를
/// 세우고, 그 빈 자료에 확장까지 묶인다.</para>
///
/// <para><b>판별은 한 함수가 낸다</b> — <see cref="Inspect"/>. 창·명령줄·호스트·취합이 저마다 따로 보면
/// 어느 하나가 조용히 다른 것을 받아들인다.</para>
/// </summary>
public static class PclmFile
{
    /// <summary>SQLite 머리의 <c>application_id</c>. <c>'PCLM'</c> 네 글자다.</summary>
    public const int ApplicationId = 0x50434C4D;

    /// <summary>
    /// 파일을 들여다본다. <b>아무것도 쓰지 않고 아무 파일도 만들지 않는다</b> — 읽기 전용 연결이고,
    /// WAL 이 아닌 파일이면 <c>-wal</c>·<c>-shm</c> 도 생기지 않는다.
    /// </summary>
    public static PclmInfo Inspect(string path)
    {
        var full = System.IO.Path.GetFullPath(path);
        if (!File.Exists(full)) return new(PclmKind.NotFound, null, null, 0);

        // 빈 파일도 SQLite 는 "빈 DB" 로 열어 준다. 표지가 없으니 아래에서 NotPclm 으로 떨어지지만,
        // 열어 볼 것도 없어 먼저 거른다.
        if (new FileInfo(full).Length == 0) return new(PclmKind.NotPclm, null, null, 0);

        try
        {
            using var connection = new SqliteConnection(new SqliteConnectionStringBuilder
            {
                DataSource = full,
                Mode = SqliteOpenMode.ReadOnly,
                // 들여다보기만 하고 놓는다. 풀에 남으면 부른 쪽이 그 파일을 지우거나 덮지 못한다.
                Pooling = false,
            }.ToString());
            connection.Open();

            var applicationId = Scalar<long>(connection, "PRAGMA application_id;");
            var version = (int)Scalar<long>(connection, "PRAGMA user_version;");

            if (applicationId == ApplicationId)
            {
                if (version > Schema.Version) return new(PclmKind.TooNew, null, null, version);
                if (Schema.IsPreBaseline(version)) return new(PclmKind.TooOld, null, null, version);
                if (!HasTable(connection, "pclm_file")) return new(PclmKind.NotPclm, null, null, version);

                using var command = connection.CreateCommand();
                command.CommandText = "SELECT role, dataset_id FROM pclm_file WHERE singleton = 1;";
                using var reader = command.ExecuteReader();
                return reader.Read()
                    ? new(PclmKind.Ok, reader.GetString(0), reader.GetString(1), version)
                    : new(PclmKind.NotPclm, null, null, version);
            }

            if (applicationId != 0) return new(PclmKind.NotPclm, null, null, version);

            // 표지가 생기기 전의 파일. 옛 시험판은 표지 없이 판만 달고 있다.
            if (Schema.IsPreBaseline(version)) return new(PclmKind.TooOld, null, null, version);

            // 임시 규칙: 표지가 생기기 전의 v19 작업자료(erp_dataset 이 있다)를 PCLM 으로 알아본다.
            // v20 판올림이 표지를 박으므로 이 규칙은 지금 쓰는 사람의 자료를 한 번 건네주는 다리일 뿐이다.
            // 1.0 전에 걷는다(ADR-031).
            if (version == Schema.BaselineVersion && HasTable(connection, "erp_dataset"))
            {
                var datasetId = Scalar<string>(connection, "SELECT dataset_id FROM erp_dataset WHERE singleton = 1;");
                return new(PclmKind.Ok, PclmRole.Work, datasetId, version);
            }

            return new(PclmKind.NotPclm, null, null, version);
        }
        catch (SqliteException)
        {
            // 26 (not a database) 이 대부분이다. 무엇으로 실패했든 열지 않을 파일이다.
            return new(PclmKind.NotPclm, null, null, 0);
        }
    }

    /// <summary>
    /// 새 PCLM 파일을 짓는다. <b>빈 자료가 서는 길은 이것 하나다</b>(ADR-031).
    ///
    /// <para>그 자리에 파일(이나 <c>-wal</c>)이 이미 있으면 <b>아무것도 하지 않고</b> 실패한다 — 덮는 순간
    /// 남의 자료가 빈 자료로 바뀐다. 폴더는 지어 준다.</para>
    /// </summary>
    /// <returns>쓰기 손잡이. 판올림까지 마친 상태다.</returns>
    public static Database Create(string path, string role)
    {
        PclmRole.Require(role);

        var full = System.IO.Path.GetFullPath(path);
        if (File.Exists(full) || File.Exists(full + "-wal"))
            throw new IOException($"그 자리에 이미 파일이 있어 새로 만들지 않습니다: {full}");

        var directory = System.IO.Path.GetDirectoryName(full);
        if (!string.IsNullOrEmpty(directory)) Directory.CreateDirectory(directory);

        Database.CreateEmptyFile(full);

        var database = new Database(full, Access.Write);
        database.Migrate();

        // 판올림은 언제나 work 로 심는다. 다른 역할로 지을 때만 고쳐 적는다.
        if (role != PclmRole.Work)
        {
            using var connection = database.Open();
            using var command = connection.CreateCommand();
            command.CommandText = "UPDATE pclm_file SET role = $role WHERE singleton = 1;";
            command.Parameters.AddWithValue("$role", role);
            command.ExecuteNonQuery();
        }

        return database;
    }

    /// <summary>
    /// 작업자료로 연다. <b>PCLM 이고 역할이 <c>work</c> 인 파일만</b> 쓰기 손잡이를 받는다.
    /// 없는 파일을 짓지 않는다. 판올림은 부르는 쪽이 <see cref="Database.Migrate"/> 로 한다.
    /// </summary>
    /// <exception cref="PclmFileException">그 밖의 모든 경우. 까닭을 사람 말로 든다.</exception>
    public static Database OpenWork(string path)
    {
        var full = System.IO.Path.GetFullPath(path);
        var info = Inspect(full);

        if (info is { Kind: PclmKind.Ok, Role: PclmRole.Work })
            return new Database(full, Access.Write);

        throw new PclmFileException(full, info.Kind, info.Role, Why(info, full));
    }

    /// <summary>
    /// 남의 것·제출본·취합본·백업을 <b>열어 본다</b>(ADR-031). 원본을 읽기 전용 연결로 <paramref name="workDirectory"/>
    /// 에 한 벌 뜨고, 그 사본을 읽기 손잡이로 건넨다. 역할은 가리지 않는다 — 보는 것은 무엇이든 된다.
    ///
    /// <para><b>원본에는 닿지 않는다.</b> 판올림도 뷰 다시 짓기도 WAL 로 바꾸기도 원본에서 하지 않는다 —
    /// 셋 다 파일을 고치는 쓰기라, 받은 제출본을 한 번 열어 본 것만으로 보낸 것과 받은 것이 갈린다.
    /// 판이 낮으면 <b>사본을</b> 올린다(취합이 하는 일과 같다).</para>
    ///
    /// <para>사본은 WAL 이 아닌 한 파일로 둔다. 그래야 읽기 손잡이가 그 옆에 <c>-wal</c>·<c>-shm</c> 을
    /// 세우지 않고, 다 본 뒤 파일 하나만 지우면 끝난다.</para>
    /// </summary>
    /// <param name="workDirectory">사본을 둘 폴더. 홈의 <see cref="Home.ViewDirectory"/> 다.</param>
    /// <exception cref="PclmFileException">없거나 PCLM 이 아니거나 판이 맞지 않는 파일.</exception>
    public static ViewedFile OpenView(string path, string workDirectory)
    {
        var full = System.IO.Path.GetFullPath(path);
        var info = Inspect(full);
        if (info.Kind != PclmKind.Ok)
            throw new PclmFileException(full, info.Kind, info.Role, Why(info, full));

        Directory.CreateDirectory(workDirectory);
        var copy = System.IO.Path.Combine(
            System.IO.Path.GetFullPath(workDirectory), $"열람-{Guid.NewGuid():N}.pclm");

        try
        {
            new Database(full, Access.Read).Snapshot(copy);

            if (info.Version < Schema.Version)
            {
                // 쓰기 손잡이로 여는 것은 사본이다. 올리는 김에 뷰도 새로 선다.
                var writer = new Database(copy, Access.Write);
                writer.Migrate();

                // 쓰기 손잡이가 WAL 로 바꿔 두었다. 한 파일로 되돌린다 — 풀에 남은 연결이 쥐고 있으면
                // 저널 방식을 바꾸지 못하므로 먼저 놓는다.
                writer.ReleasePool();
                using var connection = new SqliteConnection(new SqliteConnectionStringBuilder
                {
                    DataSource = copy,
                    Mode = SqliteOpenMode.ReadWrite,
                    Pooling = false,
                }.ToString());
                connection.Open();
                using var command = connection.CreateCommand();
                command.CommandText = "PRAGMA journal_mode = DELETE;";
                command.ExecuteNonQuery();
            }

            return new ViewedFile(new Database(copy, Access.Read), full, info.Role!, info.Version);
        }
        catch
        {
            ViewedFile.Discard(copy);
            throw;
        }
    }

    /// <summary>
    /// <paramref name="source"/> 를 <paramref name="target"/> 에 한 벌 뜨고 그 사본에 역할과 <b>새 신원</b>을 적는다.
    ///
    /// <para><b>사본은 언제나 새 <c>dataset_id</c> 를 받는다</b>(ADR-031). 제출본이 작업자료와 같은 신원을
    /// 달고 나가면, 그 파일을 누가 확장 저장 대상으로 묶었을 때 옛 검토 화면이 엉뚱한 파일에 저장된다.
    /// <c>created_at</c> 도 지금으로 고친다 — 이 파일이 생긴 때다.</para>
    ///
    /// <para>고쳐 적는 연결은 <b>WAL 로 바꾸지 않는다</b>(ADR-031). <c>VACUUM INTO</c> 가 떨군 일반 저널 한
    /// 파일을 그대로 두어야 받는 쪽이 파일 하나만 가져가도 알맹이가 빠지지 않는다.</para>
    /// </summary>
    /// <returns>뜬 자리의 온전한 경로.</returns>
    /// <exception cref="InvalidOperationException">덮으려는 자리에 작업자료가 있다(<see cref="RequireOverwritable"/>).</exception>
    public static string Snapshot(Database source, string target, string role, bool overwrite)
    {
        PclmRole.Require(role);
        if (overwrite) RequireOverwritable(target);

        var saved = source.Snapshot(target, overwrite);

        using var connection = new SqliteConnection(new SqliteConnectionStringBuilder
        {
            DataSource = saved,
            Mode = SqliteOpenMode.ReadWrite,
            Pooling = false,
        }.ToString());
        connection.Open();

        using var command = connection.CreateCommand();
        command.CommandText =
            """
            UPDATE pclm_file
               SET role = $role,
                   dataset_id = lower(hex(randomblob(16))),
                   created_at = strftime('%Y-%m-%dT%H:%M:%f0000Z', 'now')
             WHERE singleton = 1;
            """;
        command.Parameters.AddWithValue("$role", role);

        if (command.ExecuteNonQuery() != 1)
            throw new InvalidOperationException($"뜬 사본에 파일 이름표(pclm_file)가 없습니다: {saved}");

        return saved;
    }

    /// <summary>
    /// 열지 않는 까닭. 경로를 함께 적는다 — 어느 파일 이야기인지 모르면 고칠 데를 찾지 못한다.
    /// 창이 고른 파일을 거절할 때도 이 말을 쓴다 — 거절하는 자리마다 따로 적으면 같은 일에 말이 갈린다.
    /// </summary>
    /// <summary>
    /// 결과물(제출본·취합본·백업)로 덮어도 되는 자리인가. 아니면 그 까닭으로 던진다.
    ///
    /// <para><b>작업자료와 옮겨진 옛 자료는 덮지 않는다</b> — 표지 전의 v19 작업자료도 작업자료다. 셋은 저장 창에서
    /// 사람이 자리를 고르고 창이 "바꾸시겠습니까" 를 물은 뒤에 덮는데, 그 한 번의 잘못 고름으로 일거리가 통째로
    /// 결과물 사본으로 바뀌면 되돌릴 길이 없다. 제출본·취합본·백업은 매번 다시 뜨는 결과물이라 덮는다.
    /// PCLM 이 아닌 파일은 사람이 덮기로 고른 것이라 막지 않는다.</para>
    /// </summary>
    public static void RequireOverwritable(string target)
    {
        var full = System.IO.Path.GetFullPath(target);
        var existing = Inspect(full);
        if (existing is { Kind: PclmKind.Ok, Role: PclmRole.Work or PclmRole.Retired })
            throw new InvalidOperationException(
                $"그 자리에는 {PclmRole.Name(existing.Role)}이(가) 있어 덮지 않습니다 — 다른 이름을 고르세요: {full}");
    }

    public static string Why(PclmInfo info, string path) => info.Kind switch
    {
        PclmKind.NotFound => $"파일이 없습니다: {path}",
        PclmKind.NotPclm => $"계약 목록 자료(PCLM) 파일이 아닙니다: {path}",
        PclmKind.TooOld =>
            $"정식판 이전의 옛 시험판(v{info.Version})으로 지은 자료라 이 프로그램이 올리지 못합니다. " +
            $"0.7.0 으로 한 번 열어 v{Schema.BaselineVersion} 로 올린 뒤 다시 여세요: {path}",
        PclmKind.TooNew =>
            $"이 프로그램보다 새 판(v{info.Version})으로 지은 자료입니다. 프로그램을 새로 받아 여세요: {path}",
        _ =>
            $"작업자료가 아니라 {PclmRole.Name(info.Role)}입니다 — 그 자리에서 고치지 않습니다: {path}",
    };

    private static bool HasTable(SqliteConnection connection, string name)
    {
        using var command = connection.CreateCommand();
        command.CommandText = "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = $name;";
        command.Parameters.AddWithValue("$name", name);
        return Convert.ToInt64(command.ExecuteScalar()) > 0;
    }

    private static T Scalar<T>(SqliteConnection connection, string sql)
    {
        using var command = connection.CreateCommand();
        command.CommandText = sql;
        return (T)Convert.ChangeType(command.ExecuteScalar()!, typeof(T), System.Globalization.CultureInfo.InvariantCulture);
    }
}

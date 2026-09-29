using Microsoft.Data.Sqlite;

namespace Pclm.Core.Storage;

/// <summary>
/// SQLite 연결과 스키마 판올림.
///
/// <para>WAL 모드로 연다 — 편집 화면이 쓰는 동안에도 소비 쪽이 막히지 않고 읽을 수 있어야 한다(ADR-006).</para>
/// </summary>
public sealed class Database
{
    /// <summary>
    /// 자료가 쌓이는 자리. <b>창과 명령줄이 같은 곳을 본다.</b>
    ///
    /// <para>한동안 창은 여기를, 명령줄은 일하는 폴더의 <c>artifacts/pclm.db</c> 를 봤다.
    /// 그러면 같은 컴퓨터에 DB 가 여럿 생기는데 — 실제로 배포 폴더에서 <c>pclm.exe</c> 를
    /// 한 번 부른 것만으로 그 옆에 세 번째가 생겼다 — 어느 쪽이 진짜인지는 파일을 열어
    /// 보기 전에는 알 수 없다. 밖에서 읽는 쪽(문서 자동 생성기)에는 <b>가리킬 자리가 하나</b>
    /// 여야 하므로 기본값을 여기로 모은다. 다른 자료를 열 때는 <c>--db</c> 로 짚는다(ADR-017).</para>
    ///
    /// <para>프로그램 폴더가 아니라 사용자 앱 데이터에 두는 까닭은 권한과 개인정보다 —
    /// 프로그램 폴더에는 쓰지 못하고, 바탕화면·문서 폴더는 클라우드 동기화로 흘러들 수 있다.</para>
    /// </summary>
    public static string DefaultPath { get; } = System.IO.Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
        "Pclm", "pclm.db");

    public string Path { get; }

    public Database(string path)
    {
        Path = System.IO.Path.GetFullPath(path);

        var directory = System.IO.Path.GetDirectoryName(Path);
        if (!string.IsNullOrEmpty(directory)) Directory.CreateDirectory(directory);
    }

    /// <summary>읽고 쓰는 연결. 없으면 파일을 만든다.</summary>
    public SqliteConnection Open()
    {
        var connection = new SqliteConnection(new SqliteConnectionStringBuilder
        {
            DataSource = Path,
            Mode = SqliteOpenMode.ReadWriteCreate,
        }.ToString());

        connection.Open();
        Execute(connection, "PRAGMA journal_mode = WAL;");
        Execute(connection, "PRAGMA foreign_keys = ON;");
        return connection;
    }

    /// <summary>읽기 전용 연결. 소비 쪽에 넘길 때 쓴다.</summary>
    public SqliteConnection OpenReadOnly()
    {
        var connection = new SqliteConnection(new SqliteConnectionStringBuilder
        {
            DataSource = Path,
            Mode = SqliteOpenMode.ReadOnly,
        }.ToString());

        connection.Open();
        return connection;
    }

    /// <summary>
    /// 자료 한 벌을 <paramref name="targetPath"/> 로 뜬다. 뜬 자리의 온전한 경로를 낸다.
    ///
    /// <para><c>VACUUM INTO</c> 라 <c>-wal</c>·<c>-shm</c> 을 딸려 보내지 않고도 <b>한 파일로
    /// 정합하게</b> 떨어진다 — 파일 셋을 손으로 복사하다 어긋나는 길을 아예 없앤다. 자리를
    /// 옮길 때(<see cref="DataLocation"/>)도 제출본을 뜰 때(<c>pclm submit</c>)도 이 하나를 쓴다.</para>
    ///
    /// <para><b>원본은 건드리지 않는다.</b> 그런데도 읽기 전용 연결로 열지 않는 것은
    /// <c>VACUUM</c> 이 그 위에서 돌지 않기 때문이다.</para>
    ///
    /// <para><paramref name="overwrite"/> 의 기본이 거짓인 데는 뜻이 있다. 자리를 옮기는 길은
    /// <b>이미 자료가 있는 자리를 덮지 않는 것</b>으로 남의 자료를 지키는데
    /// (<see cref="DataLocation.WhyNotMoveTo"/>), 여기서 말없이 덮으면 그 방벽이 무너진다.
    /// 사람이 자리를 손수 짚어 뜨는 제출본만 참으로 부른다.</para>
    /// </summary>
    public string Snapshot(string targetPath, bool overwrite = false)
    {
        if (!File.Exists(Path))
            throw new FileNotFoundException($"뜰 자료가 없습니다: {Path}");

        var target = System.IO.Path.GetFullPath(targetPath);
        var directory = System.IO.Path.GetDirectoryName(target);
        if (!string.IsNullOrEmpty(directory)) Directory.CreateDirectory(directory);

        // VACUUM INTO 는 이미 있는 파일에 쏟지 않는다. 덮어쓰기로 부른 것만 치운다.
        if (overwrite)
            foreach (var suffix in new[] { "", "-wal", "-shm" })
                File.Delete(target + suffix);

        using var source = new SqliteConnection(new SqliteConnectionStringBuilder
        {
            DataSource = Path,
            Mode = SqliteOpenMode.ReadWrite,   // VACUUM 은 읽기 전용 연결로는 돌지 않는다
        }.ToString());

        source.Open();
        using var command = source.CreateCommand();
        // 경로에 따옴표가 섞일 수 있어 매개변수로 넘긴다.
        command.CommandText = "VACUUM INTO $target;";
        command.Parameters.AddWithValue("$target", target);
        command.ExecuteNonQuery();

        return target;
    }

    /// <summary>
    /// 스키마를 현재 판까지 올리고 계약면 뷰를 새로 짓는다.
    ///
    /// <para>올릴 것이 있었으면 <b>공고건을 다시 지은 결과</b>를 낸다. 판올림이 원공고와
    /// 재공고를 한 건으로 만들면서 링크를 걷어내거나 접수 하나를 밀어낼 수 있는데, 둘 다
    /// 오류를 내지 않으므로 <b>부르는 쪽이 사람에게 전할 자리를 가져야 한다</b>. 올릴 것이
    /// 없었으면 <c>null</c> 이다 — 앱을 열 때마다 전수 재계산이 돌지 않는다.</para>
    /// </summary>
    public NoticeGroupChange? Migrate()
    {
        using var connection = Open();
        var current = UserVersion(connection);
        NoticeGroupChange? change = null;

        if (current < Schema.Version)
        {
            // 판올림이 표를 통째로 갈아 끼우기도 하는데, 뷰가 그 표를 가리키고 있으면
            // SQLite 가 "error in view" 로 막는다. 뷰에는 데이터가 없으니 먼저 치우고 나중에 다시 짓는다.
            DropViews(connection);

            // 외래키 검사는 트랜잭션 안에서 켜고 끌 수 없어 바깥에서 다룬다.
            Execute(connection, "PRAGMA foreign_keys = OFF;");

            using (var transaction = connection.BeginTransaction())
            {
                for (var v = current; v < Schema.Migrations.Count; v++)
                    Execute(connection, Schema.Migrations[v], transaction);

                // PRAGMA 는 매개변수를 받지 않아 값을 직접 넣는다. 상수라 주입 위험이 없다.
                Execute(connection, $"PRAGMA user_version = {Schema.Version};", transaction);
                transaction.Commit();
            }

            Execute(connection, "PRAGMA foreign_keys = ON;");

            // 판올림은 건마다 계열 하나를 씨로 뿌려 둘 뿐이다(스키마 V15) — 관련공고를 타고
            // 실제로 이어 붙이는 것은 여기다. 참조 검사보다 앞서야 옮겨진 링크가 검사를 지난다.
            using (var transaction = connection.BeginTransaction())
            {
                change = NoticeGroups.Rebuild(connection, transaction);
                transaction.Commit();
            }

            // 갈아 끼운 참조가 실제로 맞는지 확인한다. 어긋난 채로 넘어가면 나중에 조용히 곪는다.
            AssertForeignKeysHold(connection);
        }

        RefreshViews(connection);
        return change;
    }

    /// <summary>
    /// 사람이 채우는 열이 바뀌면 뷰를 다시 짓는다. 열은 표가 아니라 정의로 있으므로,
    /// 정의를 고친 쪽이 반드시 이것을 불러야 화면과 계약면에 반영된다.
    /// </summary>
    public void RefreshViews()
    {
        using var connection = Open();
        RefreshViews(connection);
    }

    /// <summary>
    /// 뷰를 지웠다 다시 만든다. 테이블과 달리 뷰에는 데이터가 없어 언제 다시 지어도 안전하고,
    /// 정의를 고쳤을 때 곧바로 반영된다.
    /// </summary>
    private static void RefreshViews(SqliteConnection connection)
    {
        // 사람이 채우는 열은 표가 아니라 정의로 있으므로, 뷰를 지을 때마다 읽어 붙인다.
        var columns = ReadUserColumns(connection);

        using var transaction = connection.BeginTransaction();
        foreach (var definition in Views.Definitions(columns))
            Execute(connection, definition, transaction);
        transaction.Commit();
    }

    private static void DropViews(SqliteConnection connection)
    {
        var names = new List<string>();

        using (var command = connection.CreateCommand())
        {
            command.CommandText = "SELECT name FROM sqlite_master WHERE type = 'view';";
            using var reader = command.ExecuteReader();
            while (reader.Read()) names.Add(reader.GetString(0));
        }

        foreach (var name in names)
            Execute(connection, $"DROP VIEW IF EXISTS \"{name}\";");
    }

    /// <summary>
    /// 외래키가 실제로 성립하는지 본다. 판올림이 참조를 새로 걸었으니 여기서 한 번 확인하고 넘어간다.
    /// </summary>
    private static void AssertForeignKeysHold(SqliteConnection connection)
    {
        using var command = connection.CreateCommand();
        command.CommandText = "PRAGMA foreign_key_check;";

        var broken = new List<string>();
        using var reader = command.ExecuteReader();
        while (reader.Read())
            broken.Add($"{reader.GetValue(0)} 행이 {reader.GetValue(2)} 를 가리키지 못합니다");

        if (broken.Count > 0)
            throw new InvalidOperationException(
                "판올림 뒤 참조가 어긋났습니다: " + string.Join(" / ", broken));
    }

    private static List<UserColumnDefinition> ReadUserColumns(SqliteConnection connection)
    {
        var columns = new List<UserColumnDefinition>();

        using var command = connection.CreateCommand();
        command.CommandText =
            "SELECT entity_type, field_name, kind, options, sort_order FROM user_column ORDER BY entity_type, sort_order;";

        using var reader = command.ExecuteReader();
        while (reader.Read())
            columns.Add(new UserColumnDefinition(
                reader.GetString(0), reader.GetString(1), reader.GetString(2),
                reader.IsDBNull(3) ? null : reader.GetString(3), reader.GetInt64(4)));

        return columns;
    }

    private static int UserVersion(SqliteConnection connection)
    {
        using var command = connection.CreateCommand();
        command.CommandText = "PRAGMA user_version;";
        return Convert.ToInt32(command.ExecuteScalar());
    }

    private static void Execute(SqliteConnection connection, string sql, SqliteTransaction? transaction = null)
    {
        using var command = connection.CreateCommand();
        command.CommandText = sql;
        command.Transaction = transaction;
        command.ExecuteNonQuery();
    }
}

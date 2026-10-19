using Dapper;
using Microsoft.Data.Sqlite;

namespace Pclm.Core.Storage;

/// <summary>
/// <see cref="Database"/> 로 여는 연결이 쓸 수 있는가.
///
/// <para>화면에서 단추를 숨기는 것만으로는 새 쓰기 경로 하나를 빠뜨리는 순간 뚫린다. 손잡이에 붙여 두면
/// 그 손잡이로 여는 모든 연결이 SQLite 수준에서 막힌다(ADR-031).</para>
/// </summary>
public enum Access
{
    Write,
    Read,
}

/// <summary>
/// SQLite 연결과 스키마 판올림.
///
/// <para>WAL 모드로 연다 — 편집 화면이 쓰는 동안에도 소비 쪽이 막히지 않고 읽을 수 있어야 한다(ADR-006).</para>
/// </summary>
public sealed class Database
{
    public string Path { get; }

    /// <summary>이 손잡이로 여는 연결이 쓸 수 있는가. <see cref="Open"/> 이 따른다.</summary>
    public Access Access { get; }

    /// <summary>
    /// 파일을 가리키기만 한다. <b>아무것도 만들지 않는다</b> — 폴더도, 파일도.
    ///
    /// <para>한동안 여기서 폴더를 만들고 <see cref="Open"/> 이 없는 파일을 지었다. 그러면 쪽지가 가리키는
    /// 파일이 지워졌거나 경로에 오타가 난 것만으로 빈 자료가 서고, 앱은 곧바로 확장까지
    /// 그 빈 자료에 묶었다(ADR-030). 빈 자료가 서는 길은 <see cref="PclmFile.Create"/> 하나다.</para>
    /// </summary>
    public Database(string path, Access access = Access.Write)
    {
        Path = System.IO.Path.GetFullPath(path);
        Access = access;
    }

    /// <summary>
    /// 연결. <b>없는 파일은 만들지 않고 실패한다</b>(SQLite 코드 14).
    ///
    /// <para><see cref="Access.Read"/> 면 <c>Mode=ReadOnly</c> 에 <c>query_only</c> 까지 건다 —
    /// <c>Store</c>·<c>SettingsStore</c> 가 어디서 열든 SQLite 가 쓰기를 거절하므로,
    /// 새 쓰기 경로를 빠뜨려도 막힌다(ADR-031). WAL 로 바꾸는 것도 파일 머리를 고치는 쓰기라 하지 않는다.</para>
    ///
    /// <para><see cref="Access.Write"/> 면 <b>옮겨진 옛 파일(<c>retired</c>)을 거절한다</b>(ADR-031). 창이 작업자료를
    /// 옮기기 전에 떠 있던 명령줄이나 두 번째 손잡이는 쪽지를 다시 보지 않으므로, 막지 않으면 아무도 다시 열지
    /// 않을 파일에 계속 쓰고 그것은 다음 시작에 지워진다.</para>
    /// </summary>
    public SqliteConnection Open()
    {
        if (Access == Access.Read)
        {
            var reader = OpenReadOnly();
            Execute(reader, "PRAGMA query_only = ON;");
            return reader;
        }

        var connection = new SqliteConnection(WriteConnectionString);

        connection.Open();
        try
        {
            RefuseRetired(connection);
            Execute(connection, "PRAGMA journal_mode = WAL;");
            Execute(connection, "PRAGMA foreign_keys = ON;");
        }
        catch
        {
            connection.Dispose();
            throw;
        }
        return connection;
    }

    /// <summary>
    /// 역할이 <c>retired</c> 면 던진다. 열 때마다 도는 자리라 물음 하나로 끝낸다 — 이름표가 없는 파일
    /// (판올림 전의 옛 v19, <see cref="CreateEmptyFile"/> 이 막 지은 빈 파일)에서만 실패하고, 그때만 표가
    /// 정말 없는지 한 번 더 보고 넘어간다.
    /// </summary>
    private void RefuseRetired(SqliteConnection connection)
    {
        string? role;
        try
        {
            using var command = connection.CreateCommand();
            command.CommandText = "SELECT role FROM pclm_file WHERE singleton = 1;";
            role = command.ExecuteScalar() as string;
        }
        catch (SqliteException e) when (e.SqliteErrorCode == 1 && !HasPclmFile(connection))
        {
            return;
        }

        if (role == PclmRole.Retired)
            throw new InvalidOperationException(
                $"이 자료는 다른 자리로 옮겨졌습니다: {Path} — 지금 작업자료를 쓰려면 프로그램을 다시 실행하세요.");
    }

    private static bool HasPclmFile(SqliteConnection connection)
    {
        using var command = connection.CreateCommand();
        command.CommandText = "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'pclm_file';";
        return Convert.ToInt64(command.ExecuteScalar()) > 0;
    }

    /// <summary>
    /// 빈 파일 하나를 짓는다. <b><c>ReadWriteCreate</c> 로 여는 곳은 여기 하나뿐이고</b>, 부르는 곳도
    /// <see cref="PclmFile.Create"/> 하나뿐이다 — 그 밖의 길로 빈 자료가 서면 P1 이 되살아난다.
    ///
    /// <para>WAL 로 바꾸는 것까지 해 둔다. 머리를 한 번 적어야 파일이 실제로 디스크에 서고,
    /// 작업자료는 어차피 WAL 로 쓴다.</para>
    /// </summary>
    internal static void CreateEmptyFile(string path)
    {
        using var connection = new SqliteConnection(new SqliteConnectionStringBuilder
        {
            DataSource = path,
            Mode = SqliteOpenMode.ReadWriteCreate,
            Pooling = false,
        }.ToString());

        connection.Open();
        Execute(connection, "PRAGMA journal_mode = WAL;");
    }

    /// <summary>읽기 전용 연결. 소비 쪽에 넘길 때 쓴다.</summary>
    public SqliteConnection OpenReadOnly()
    {
        var connection = new SqliteConnection(ReadOnlyConnectionString);
        connection.Open();
        return connection;
    }

    /// <summary>
    /// 이 파일을 쥐고 풀에 남은 연결을 놓는다. 그 파일을 지우거나 저널 방식을 바꾸기 전에 부른다 —
    /// 풀에 남은 연결이 열려 있으면 Windows 는 지우지 못하게 막는다.
    ///
    /// <para><c>ClearAllPools</c> 를 쓰지 않는다. 그것은 프로세스 전체의 풀을 비워, 다른 실마리가 막 연
    /// 연결까지 폐기한다.</para>
    /// </summary>
    public void ReleasePool()
    {
        foreach (var connectionString in new[] { WriteConnectionString, ReadOnlyConnectionString })
        {
            using var connection = new SqliteConnection(connectionString);
            SqliteConnection.ClearPool(connection);
        }
    }

    private string WriteConnectionString => new SqliteConnectionStringBuilder
    {
        DataSource = Path,
        Mode = SqliteOpenMode.ReadWrite,
    }.ToString();

    private string ReadOnlyConnectionString => new SqliteConnectionStringBuilder
    {
        DataSource = Path,
        Mode = SqliteOpenMode.ReadOnly,
    }.ToString();

    /// <summary>
    /// 자료 한 벌을 <paramref name="targetPath"/> 로 뜬다. 뜬 자리의 온전한 경로를 낸다.
    ///
    /// <para><c>VACUUM INTO</c> 라 <c>-wal</c>·<c>-shm</c> 을 딸려 보내지 않고도 <b>한 파일로
    /// 정합하게</b> 떨어진다 — 파일 셋을 손으로 복사하다 어긋나는 길을 아예 없앤다. 옛 자료를 홈으로
    /// 옮겨 올 때(<see cref="Home"/>)도 제출본을 뜰 때(<see cref="PclmFile.Snapshot"/>)도 이 하나를 쓴다.</para>
    ///
    /// <para><b>원본은 건드리지 않는다 — 읽기 전용 연결로 뜬다.</b> <c>VACUUM INTO</c> 는 원본에 쓰지
    /// 않으므로 읽기 전용 연결에서도 돈다(실측, Microsoft.Data.Sqlite 10.0.11). 한동안 여기 "읽기 전용으로는
    /// 돌지 않는다" 고 적고 쓰기 연결로 열었는데, 틀린 말이었다 — 남의 제출본을 뜰 때(취합)도 그 파일에
    /// 쓰기 잠금을 잡을 까닭이 없다. 뜬 결과는 WAL 이 아닌 일반 저널 한 파일이다.</para>
    ///
    /// <para><paramref name="overwrite"/> 의 기본이 거짓인 데는 뜻이 있다. 작업자료 자리는
    /// <b>이미 자료가 있으면 덮지 않는 것</b>으로 남의 자료를 지키는데(ADR-031), 여기서 말없이 덮으면
    /// 그 방벽이 무너진다. 사람이 자리를 손수 짚어 뜨는 제출본만 참으로 부른다.</para>
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
            Mode = SqliteOpenMode.ReadOnly,
            // 한 번 뜨고 마는 연결이라 풀에 남겨 원본을 쥐고 있을 까닭이 없다 — 남의 파일이면 더욱 그렇다.
            Pooling = false,
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
    /// <para>판올림 뒤에는 명시 참조로 관계를 재계산한다. 현재 판에서는 관계를 다시 짓지 않는다.</para>
    ///
    /// <para>빈 파일(<c>user_version</c> 0)은 <see cref="Schema.Baseline"/> 으로 v19 를 곧장 짓고,
    /// 그 위로 <see cref="Schema.Steps"/> 를 밟는다. 기준선보다 옛 시험판(v1~v18)은 <b>건드리지 않고</b>
    /// 거절한다 — 그 판을 올리던 단계는 0.7.0 까지만 실려 있다.</para>
    ///
    /// <para><see cref="Access.Read"/> 이면 <b>아무것도 쓰지 않는다.</b> 올릴 것이 있으면 예외로 거절하고,
    /// 지금 판이면 뷰도 다시 짓지 않는다 — 뷰는 이미 파일 안에 있고, 다시 짓는 것도 쓰기다.
    /// 읽기로 연 남의 파일을 판올림하려면 사본을 떠서 그것을 올린다(취합이 하는 일).</para>
    /// </summary>
    public void Migrate()
    {
        RefusePreBaseline();

        if (Access == Access.Read)
        {
            int version;
            using (var probe = OpenReadOnly())
                version = UserVersion(probe);

            if (version < Schema.Version)
                throw new InvalidOperationException(
                    $"읽기 전용으로 연 자료라 판을 올리지 않습니다(v{version} → v{Schema.Version}): {Path}");

            return;
        }

        using var connection = Open();
        var current = UserVersion(connection);

        if (current < Schema.Version)
        {
            // 판올림이 표를 통째로 갈아 끼우기도 하는데, 뷰가 그 표를 가리키고 있으면
            // SQLite 가 "error in view" 로 막는다. 뷰에는 데이터가 없으니 먼저 치우고 나중에 다시 짓는다.
            DropViews(connection);

            // 외래키 검사는 트랜잭션 안에서 켜고 끌 수 없어 바깥에서 다룬다.
            Execute(connection, "PRAGMA foreign_keys = OFF;");

            // 이미 있던 작업자료만 예시 보기를 받는다 — 갓 지은 파일(0 에서 온 것)은 PclmFile.Create 가 역할을 보고 심는다.
            var seedExample = current is > 0 and < Schema.ExampleViewVersion;
            var upgradeExample = current is > 0 and < Schema.ExampleViewUpgradeVersion;

            using (var transaction = connection.BeginTransaction())
            {
                if (current == 0)
                {
                    Execute(connection, Schema.Baseline, transaction);
                    current = Schema.BaselineVersion;
                }

                for (var v = current; v < Schema.Version; v++)
                {
                    Execute(connection, Schema.Steps[v - Schema.BaselineVersion], transaction);
                    if (v == 29) SchemaRepairs.Upgrade30(connection, transaction, current);
                }

                // v26 은 그 판의 예시를 심고, v27 은 손대지 않은 그것을 지금 구성으로 갈아 끼운다 — v25 에서 오면 둘 다 밟는다.
                var work = (seedExample || upgradeExample) && PclmFile.IsWork(connection, transaction);
                if (work && seedExample)
                    Store.SeedExampleViews(connection, transaction, Store.ExampleViewPackageV26());
                if (work && upgradeExample)
                    Store.UpgradeExampleViews(connection, transaction);

                // PRAGMA 는 매개변수를 받지 않아 값을 직접 넣는다. 상수라 주입 위험이 없다.
                Execute(connection, $"PRAGMA user_version = {Schema.Version};", transaction);
                transaction.Commit();
            }

            Execute(connection, "PRAGMA foreign_keys = ON;");

            // 건을 관련공고를 타고 실제로 이어 붙이는 것은 판올림이 아니라 여기다.
            // 참조 검사보다 앞서야 옮겨진 링크가 검사를 지난다.
            using (var transaction = connection.BeginTransaction())
            {
                Pclm.Core.Erp.ExplicitLinks.Resolve(connection, transaction);
                transaction.Commit();
            }

            // 갈아 끼운 참조가 실제로 맞는지 확인한다. 어긋난 채로 넘어가면 나중에 조용히 곪는다.
            AssertForeignKeysHold(connection);
        }

        RefreshViews(connection);
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
    internal static IReadOnlyDictionary<(string Entity, string Name), string> RefreshViews(
        SqliteConnection connection, SqliteTransaction? transaction = null) =>
        Refresh(connection, transaction).Columns;

    /// <summary>
    /// <see cref="RefreshViews(SqliteConnection, SqliteTransaction?)"/> 에 맞춤 뷰의 깨진 필드(뷰 식별자, 필드 식별자 → 까닭)까지.
    ///
    /// <para>차례(docs/user-views.md §5): 맞춤 뷰(<c>u_%</c>)를 <b>맨 먼저 모두 치우고</b> 계약면 뷰를 지은 뒤 <b>맨 끝에</b>
    /// 맞춤 뷰를 짓는다 — 면 뷰 위층이라 남아 있으면 면 뷰를 다시 짓는 길을 막고, 지웠거나 이름을 바꾼 것이 남는다.</para>
    /// </summary>
    internal static (IReadOnlyDictionary<(string Entity, string Name), string> Columns,
        IReadOnlyDictionary<(string View, string Column), string> UserViews) Refresh(
        SqliteConnection connection, SqliteTransaction? transaction = null)
    {
        // 사람이 채우는 열은 표가 아니라 정의로 있으므로, 뷰를 지을 때마다 읽어 붙인다.
        var columns = ReadUserColumns(connection);

        using var ownTransaction = transaction is null ? connection.BeginTransaction() : null;
        transaction ??= ownTransaction!;
        UserViewBuilder.DropAll(connection, transaction);
        var broken = IsolateComputed(connection, transaction, columns);
        var views = UserViewBuilder.Build(connection, transaction, columns);
        ownTransaction?.Commit();
        return (broken, views);
    }

    /// <summary>
    /// 가공열 하나가 깨져도 뷰 전체가 서지 못하는 일이 없게 한다(column-management §11.5). 뷰는 앱이 열릴 때마다
    /// 다시 지어지므로, 저장된 식 하나가 깨지면(뷰 이름이 바뀐 SQL 열, 손으로 고친 파일) 앱이 아예 뜨지 않는다.
    ///
    /// <list type="number">
    /// <item>가공열을 모두 빈 글로 둔 뼈대를 짓는다.</item>
    /// <item>그 위에서 열마다 따로 컴파일하고 면 뷰 위에서 돌려 본다(SQL 열은 authorizer 를 건 채 준비한다).</item>
    /// <item>깨진 것만 빈 글로 두고 다시 짓는다. 그래도 뷰가 준비되지 않으면 SQL 열이 서로 도는 것이다(순환) —
    ///   SQL 열을 모두 걷었다가 하나씩 되돌려, 되돌리면 깨지는 것을 깨진 것으로 둔다.</item>
    /// </list>
    /// 깨진 칸은 빈 글이라 소비자의 빈 값 경고가 울린다. 까닭은 뷰 정의 안에 남는다(<see cref="Views.BrokenIn"/>).
    /// </summary>
    private static Dictionary<(string Entity, string Name), string> IsolateComputed(
        SqliteConnection connection, SqliteTransaction transaction, IReadOnlyList<UserColumnDefinition> columns)
    {
        var broken = new Dictionary<(string Entity, string Name), string>();
        var computed = columns.Where(c => c.Kind == "computed").ToList();
        void Build()
        {
            foreach (var definition in Views.Definitions(columns, broken))
                Execute(connection, definition, transaction);
        }
        if (computed.Count == 0)
        {
            Build();
            return broken;
        }

        // 1. 뼈대
        foreach (var c in computed) broken[(c.EntityType, c.FieldName)] = "";
        Build();
        broken.Clear();

        // 2. 하나씩
        foreach (var c in computed)
        {
            try
            {
                var formula = ComputedColumn.Parse(c.Options ?? "");
                _ = ComputedColumn.FinalShape(c.EntityType, c.Options!);
                if (formula.Source == "sql")
                {
                    Execute(connection, "PRAGMA query_only = ON;", transaction);
                    try { SqlColumn.Prepare(connection, transaction, c.EntityType, formula.Sql!); }
                    finally { Execute(connection, "PRAGMA query_only = OFF;", transaction); }
                }
                var expression = ComputedColumn.Expression(c.EntityType, formula);
                using var command = connection.CreateCommand();
                command.Transaction = transaction;
                command.CommandText = $"SELECT {expression} FROM \"{Store.FaceView(c.EntityType)}\" AS {FlowCompiler.Self} LIMIT 20;";
                using var reader = command.ExecuteReader();
                while (reader.Read()) { }
            }
            catch (Exception error) when (error is InvalidOperationException or SqliteException)
            {
                broken[(c.EntityType, c.FieldName)] = error.Message;
            }
        }

        // 3. 깨진 것만 걷고 짓는다. SQL 열끼리의 순환은 준비해 봐야 드러난다.
        Build();
        if (PrepareViews(connection, transaction) is null) return broken;
        var sql = computed.Where(c => !broken.ContainsKey((c.EntityType, c.FieldName))
                                      && ComputedColumn.Parse(c.Options!).Source == "sql").ToList();
        foreach (var c in sql) broken[(c.EntityType, c.FieldName)] = "";
        Build();
        if (PrepareViews(connection, transaction) is { } baseError)
            throw new InvalidOperationException($"계약면 뷰를 짓지 못했습니다: {baseError}");
        foreach (var c in sql)
        {
            broken.Remove((c.EntityType, c.FieldName));
            Build();
            if (PrepareViews(connection, transaction) is { } error)
                broken[(c.EntityType, c.FieldName)] = error;
        }
        Build();
        return broken;
    }

    /// <summary>모든 뷰를 준비만 해 본다. 실패하면 첫 오류 글.</summary>
    private static string? PrepareViews(SqliteConnection connection, SqliteTransaction transaction)
    {
        foreach (var view in connection.Query<string>("SELECT name FROM sqlite_master WHERE type = 'view' ORDER BY name;", transaction: transaction).ToList())
        {
            try
            {
                using var command = connection.CreateCommand();
                command.Transaction = transaction;
                command.CommandText = $"SELECT * FROM \"{view.Replace("\"", "\"\"")}\" LIMIT 0;";
                using var reader = command.ExecuteReader();
            }
            catch (SqliteException error)
            {
                return error.Message;
            }
        }
        return null;
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

    /// <summary>
    /// 기준선보다 옛 시험판이면 거절한다. <b>읽기 전용 연결로 먼저 본다</b> — <see cref="Open"/> 은
    /// WAL 로 바꾸며 파일 머리를 고치므로, 받지 않을 파일에는 그것조차 하지 않는다.
    /// </summary>
    private void RefusePreBaseline()
    {
        if (!File.Exists(Path)) return;

        int version;
        using (var probe = OpenReadOnly())
            version = UserVersion(probe);

        if (Schema.IsPreBaseline(version))
            throw new InvalidOperationException(
                $"정식판 이전의 옛 시험판(v{version})으로 지은 자료라 이 프로그램이 올리지 못합니다. " +
                $"0.7.0 으로 한 번 열어 v{Schema.BaselineVersion} 로 올린 뒤 다시 여세요: {Path}");
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

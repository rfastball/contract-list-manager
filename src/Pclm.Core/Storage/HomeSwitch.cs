using Microsoft.Data.Sqlite;

namespace Pclm.Core.Storage;

/// <summary>
/// 작업자료 바꾸기(ADR-032) — 옮기기·다른 작업자료 쓰기·스냅샷에서 새로·새 계약자료, 그리고 백업.
///
/// <para><b>바꾸는 일은 모두 한 절차(<see cref="Switch"/>)를 탄다.</b> 나가는 작업자료의 쓰기 잠금을 쥔 채 쪽지를
/// 고치고, 잠금을 놓는다. 그래서 확장 호스트의 저장은 "바꾸기 전 옛 자료에" 아니면 "거절" 둘 중 하나다 —
/// 호스트는 같은 잠금을 얻은 뒤 쪽지를 다시 보기 때문이다(<see cref="Erp.ErpCapture.Save"/>).</para>
///
/// <para>바꾼 뒤 창은 스스로를 다시 띄운다. 이 창의 손잡이를 갈아 끼우지 않는다 — 갈아 끼우는 창은 그 사이에
/// 무엇이 어디로 쓰였는지를 따져야 한다.</para>
/// </summary>
public sealed partial class Home
{
    /// <summary>
    /// 옮기기의 사본 검사가 세는 표. 개체 셋과 계획 — 사람이 쌓은 것의 몸통이다. 하나라도 어긋나면 옮기지 않는다.
    /// </summary>
    private static readonly string[] CountedTables = ["request", "notice", "contract", "plan"];

    /// <summary>
    /// 전환이 쓰기 잠금을 쥔 채 쪽지까지 고친 뒤, 잠금을 놓기 직전에 부르는 자리. <b>시험만 건다</b> — 그 사이
    /// 줄 선 저장이 어떻게 되는지 보려고.
    /// </summary>
    internal Action? WhileLocked { get; set; }

    /// <summary>
    /// 작업자료를 다른 자리로 옮긴다. <b>같은 자료다</b> — 신원(<c>dataset_id</c>)을 그대로 가져가므로 확장의 열린
    /// 검토 화면이 새 자리로 이어진다(ADR-032). 옛 파일은 <c>retired</c> 로 표시되고 다음 시작에 지워진다.
    ///
    /// <para>사본은 쓰기 잠금을 쥔 채 뜬다. 잠금이 없으면 뜨는 사이 커밋된 저장이 사본에는 없고 옛 파일에만
    /// 남는다 — 그리고 옛 파일은 지워진다.</para>
    /// </summary>
    /// <returns>새 자리의 온전한 경로.</returns>
    /// <exception cref="InvalidOperationException">네트워크 자리·지금 자리와 같음·검사 실패.</exception>
    /// <exception cref="IOException">그 자리에 이미 파일이 있다.</exception>
    public string MoveTo(Database current, string target)
    {
        var full = CheckNewTarget(current, target);
        var temporary = $"{full}.tmp-{Guid.NewGuid():N}";
        var created = new List<string>();

        Switch(current, (connection, transaction, datasetId) =>
        {
            // 읽기 전용 연결의 VACUUM INTO 다. 쥔 것은 쓰기 잠금이라 읽기는 막히지 않고, 커밋된 것까지만 뜬다 —
            // 이 연결은 아직 아무것도 쓰지 않았으므로 그것이 곧 지금의 전부다.
            created.Add(temporary);
            current.Snapshot(temporary);

            // 이름을 바꾸기 전에 본다(ADR-031). 셈은 잠금 안에서 본 것과 견준다.
            Verify(temporary, PclmRole.Work, id => id == datasetId, Counts(connection, transaction));

            File.Move(temporary, full, overwrite: false);
            created.Add(full);
            return (full, datasetId, current.Path);
        }, retireCurrent: true, created);

        return full;
    }

    /// <summary>
    /// 있는 다른 작업자료를 그 자리에서 쓴다(ADR-032 「내 계약자료로 사용」). <b>역할이 <c>work</c> 인 것만</b> 받는다.
    /// 지금 작업자료는 그대로 작업자료로 남는다 — 사람이 다시 돌아올 수 있는 자료다.
    /// </summary>
    /// <exception cref="PclmFileException">작업자료가 아닌 파일.</exception>
    public string UseInPlace(Database current, string path)
    {
        var full = Path.GetFullPath(path);
        RequireLocal(full);
        if (SamePath(full, current.Path))
            throw new InvalidOperationException($"지금 이 창에서 쓰고 있는 작업자료입니다: {full}");

        var info = RequireWork(full);
        Switch(current, (_, _, _) => (full, info.DatasetId!, null), retireCurrent: false, []);
        return full;
    }

    /// <summary>
    /// 제출본·취합본·백업(무엇이든)의 사본을 떠서 새 작업자료로 삼는다(ADR-032 「이 파일로 새 계약자료 만들기」).
    /// <b>원본은 그대로다</b> — 그때의 기록이라 고치지 않는다. 사본은 <b>새 신원</b>을 받는다: 원본과 같은 신원이면
    /// 둘 중 어느 것이 확장의 저장 대상인지 가릴 수 없다.
    /// </summary>
    /// <exception cref="PclmFileException">없거나 PCLM 이 아니거나 판이 맞지 않는 원본.</exception>
    public string CreateFromSnapshot(Database current, string source, string target)
    {
        var from = Path.GetFullPath(source);
        var full = CheckNewTarget(current, target);

        var info = PclmFile.Inspect(from);
        if (info.Kind != PclmKind.Ok)
            throw new PclmFileException(from, info.Kind, info.Role, Refuse(info, from).Message);

        var temporary = $"{full}.tmp-{Guid.NewGuid():N}";
        var created = new List<string>();
        string datasetId;

        try
        {
            created.Add(temporary);
            var counts = CountsOf(from);
            new Database(from, Access.Read).Snapshot(temporary);

            // 판이 낮으면 사본을 올린다. 이름표가 있으면 먼저 work 로 고쳐 적는다 — 쓰기 손잡이는 retired 를 열지 않는다.
            // 표지가 생기기 전의 v19 는 이름표가 없어, 판올림이 심은 뒤 아래에서 새 신원을 받는다.
            if (info.Version < Schema.Version)
            {
                Stamp(temporary, required: false);
                var writer = new Database(temporary, Access.Write);
                writer.Migrate();
                writer.ReleasePool();
            }
            Stamp(temporary, required: true);

            Verify(temporary, PclmRole.Work, id => id != info.DatasetId, counts);
            datasetId = PclmFile.Inspect(temporary).DatasetId!;

            File.Move(temporary, full, overwrite: false);
            created.Add(full);
        }
        catch
        {
            Discard(created);
            throw;
        }

        Switch(current, (_, _, _) => (full, datasetId, null), retireCurrent: false, created);
        return full;
    }

    /// <summary>
    /// 빈 작업자료를 지어 그것으로 바꾼다(ADR-032 「새 계약자료」). 지금 작업자료는 그대로 남는다.
    /// 시작 화면의 「새로 만들기」(<see cref="CreateNew"/>)와 달리 나가는 작업자료가 있어 전환 절차를 탄다.
    /// </summary>
    public string CreateNewWhileRunning(Database current, string target)
    {
        var full = CheckNewTarget(current, target);
        var created = new List<string>();
        string datasetId;

        try
        {
            var database = PclmFile.Create(full, PclmRole.Work);
            created.Add(full);
            database.ReleasePool();
            datasetId = DatasetIdOf(full);
        }
        catch
        {
            Discard(created);
            throw;
        }

        Switch(current, (_, _, _) => (full, datasetId, null), retireCurrent: false, created);
        return full;
    }

    /// <summary>
    /// 지금 시점을 백업 파일로 뜬다(ADR-032 「백업 만들기」). 역할은 <c>backup</c>, 신원은 새로 — 바꾸는 일이
    /// 아니라 쪽지는 그대로다(ADR-032).
    ///
    /// <para>자리는 사람이 저장 대화상자에서 고른 것이라 그 자리의 옛 백업은 덮는다(ADR-031). 다만 <b>작업자료는
    /// 덮지 않는다</b> — 지금 쓰는 것이든 다른 것이든, 백업 하나가 사람의 자료를 지우게 두지 않는다.</para>
    /// </summary>
    /// <returns>뜬 자리의 온전한 경로.</returns>
    public static string BackupTo(Database current, string target)
    {
        var full = Path.GetFullPath(target);
        RequireLocal(full);
        // 지금 작업자료 자리·다른 작업자료·옮긴 옛 파일을 덮지 않는 검사는 Snapshot 이 쥔다 — 제출·취합과 한 길이다.
        return PclmFile.Snapshot(current, full, PclmRole.Backup, overwrite: true);
    }

    // ── 전환 절차 ────────────────────────────────────────────────────

    /// <summary>
    /// 전환 절차(ADR-032). 바꾸는 일은 모두 이것 하나를 탄다.
    ///
    /// <code>
    /// 1. 나가는 작업자료에 연결 A, BEGIN IMMEDIATE      ← 확장 호스트·명령줄의 쓰기가 여기서 줄 선다
    /// 2. (옮기기) A 를 쥔 채 사본을 뜨고 검사한 뒤 제자리 이름으로
    /// 3. 쪽지를 새 작업자료로 쓴다(원자적)             ← 여기를 지나면 되돌리지 않는다
    /// 4. (옮기기) A 안에서 옛 파일을 retired 로
    /// 5. A 커밋 → 줄 서 있던 저장은 잠금을 얻은 뒤 새 쪽지를 보고 거절된다
    /// </code>
    ///
    /// <para><b>쪽지를 잠금 안에서 쓰는 까닭.</b> 잠금을 놓은 뒤에 쓰면, 놓는 순간 줄 서 있던 저장이 잠금을 얻어
    /// 옛 쪽지를 보고 옛 자료에 쓰고 성공을 알린다 — 그다음 쪽지가 바뀌어 그 저장은 사람이 여는 자료에 없다.
    /// 잠금을 잡기 전에 쓰면, 잠금 앞에서 기다리는 동안 이미 잠금을 쥔 저장이 옛 쪽지를 보고 들어간다.
    /// 잠금 안에서 쓰면 잠금을 얻은 모든 저장이 바꾸기 전(옛 쪽지) 아니면 바꾼 뒤(새 쪽지)만 본다.</para>
    ///
    /// <para>3 앞에서 무엇이 실패하든 이 절차가 지은 파일(<paramref name="created"/>)을 지우고 쪽지는 그대로 둔다.
    /// 3 을 지난 뒤의 실패는 그대로 올라가되 지은 파일을 지우지 않는다 — 쪽지가 이미 그것을 가리킨다.</para>
    /// </summary>
    /// <param name="prepare">잠금 안에서 할 일. A 와 그 트랜잭션, 나가는 자료의 신원을 받아 새 작업자료와 그 신원,
    /// 쪽지에 남길 옛 자리(옮기기만)를 낸다.</param>
    /// <param name="retireCurrent">나가는 파일을 <c>retired</c> 로 표시하나. 옮기기만 참이다.</param>
    /// <param name="created">이 전환이 지은 파일. 3 앞에서 실패하면 지운다.</param>
    private void Switch(
        Database current,
        Func<SqliteConnection, SqliteTransaction, string, (string Workfile, string DatasetId, string? Retired)> prepare,
        bool retireCurrent,
        List<string> created)
    {
        using (var connection = current.Open())
        using (var transaction = connection.BeginTransaction(deferred: false))
        {
            try
            {
                // 잠금을 얻은 뒤에 다시 본다. 이 창이 연 뒤로 다른 손이 쪽지를 바꿨으면 이 창의 자료는 이미
                // 작업자료가 아니다 — 거기서 또 바꾸면 쪽지가 두 번 갈려 어느 것이 진짜인지 모른다.
                var (datasetId, role) = Identity(connection, transaction);
                if (role != PclmRole.Work || !PointsAt(current.Path, datasetId))
                    throw new InvalidOperationException(
                        $"이 창이 연 자료가 더 이상 작업자료가 아닙니다 — 다른 곳에서 바꿨습니다. 프로그램을 다시 실행하세요: {current.Path}");

                var next = prepare(connection, transaction, datasetId);
                WriteConfig(next.Workfile, next.DatasetId, next.Retired);
            }
            catch
            {
                transaction.Rollback();
                Discard(created);
                throw;
            }

            if (retireCurrent)
                Execute(connection, transaction, "UPDATE pclm_file SET role = 'retired' WHERE singleton = 1;");

            WhileLocked?.Invoke();
            transaction.Commit();
        }

        // 풀에 남은 연결이 옛 파일을 쥐고 있으면 다음 시작이 그것을 지우지 못한다.
        current.ReleasePool();
    }

    // ── 도움 ─────────────────────────────────────────────────────────

    /// <summary>
    /// 새 작업자료가 설 자리. 로컬 디스크이고, 지금 자리가 아니고, <b>비어 있어야 한다</b> — 작업자료 자리는
    /// 무엇이 있든 덮지 않는다(ADR-031). <c>-wal</c> 만 남은 자리도 비지 않은 것으로 본다.
    ///
    /// <para>창은 자리를 고른 직후에도 이것을 부른다 — 확인을 받은 뒤에야 거절하면 사람은 한 번 더 고른다.
    /// 바꾸는 동작이 다시 부르므로 그 사이에 무엇이 놓여도 덮지 않는다.</para>
    /// </summary>
    /// <returns>온전한 경로.</returns>
    public static string CheckNewTarget(Database current, string target)
    {
        var full = Path.GetFullPath(target);
        RequireLocal(full);
        if (SamePath(full, current.Path))
            throw new InvalidOperationException($"지금 쓰고 있는 작업자료 자리입니다: {full}");
        if (File.Exists(full) || File.Exists(full + "-wal"))
            throw new IOException($"그 자리에 이미 파일이 있어 덮지 않습니다 — 다른 이름을 고르세요: {full}");
        return full;
    }

    /// <summary>작업자료로 쓸 수 있는 파일인가. 아니면 시작 화면 「파일 찾기」 와 같은 말로 거절한다.</summary>
    private static PclmInfo RequireWork(string full)
    {
        var info = PclmFile.Inspect(full);
        if (info is { Kind: PclmKind.Ok, Role: PclmRole.Work }) return info;

        var message = info.Kind == PclmKind.Ok
            ? $"{PclmRole.Name(info.Role)} 파일은 그 자리에서 작업자료로 쓸 수 없습니다 — 그때의 기록이라 고치지 않습니다. " +
              $"일하려면 사본을 떠서 새 작업자료로 삼으세요: {full}"
            : Refuse(info, full).Message;
        throw new PclmFileException(full, info.Kind, info.Role, message);
    }

    /// <summary>
    /// 사본을 제자리 이름으로 바꾸기 전에 본다(ADR-031): PCLM·역할·신원, <c>quick_check</c>, 표마다 줄 수.
    /// </summary>
    private static void Verify(string path, string role, Func<string?, bool> identity, IReadOnlyDictionary<string, long> counts)
    {
        var info = PclmFile.Inspect(path);
        if (info.Kind != PclmKind.Ok || info.Role != role || !identity(info.DatasetId))
            throw new InvalidOperationException($"뜬 사본의 이름표가 맞지 않아 쓰지 않습니다: {path}");

        using var connection = ReadOnly(path);
        using (var command = connection.CreateCommand())
        {
            command.CommandText = "PRAGMA quick_check;";
            if (command.ExecuteScalar() as string != "ok")
                throw new InvalidOperationException($"뜬 사본이 온전하지 않아 쓰지 않습니다: {path}");
        }

        var copied = Counts(connection, null);
        foreach (var (table, count) in counts)
            if (copied[table] != count)
                throw new InvalidOperationException(
                    $"뜬 사본의 {table} 줄 수가 원본과 다릅니다({count} → {copied[table]}). 쓰지 않습니다: {path}");
    }

    private static Dictionary<string, long> CountsOf(string path)
    {
        using var connection = ReadOnly(path);
        return Counts(connection, null);
    }

    private static Dictionary<string, long> Counts(SqliteConnection connection, SqliteTransaction? transaction)
    {
        var counts = new Dictionary<string, long>(StringComparer.Ordinal);
        foreach (var table in CountedTables)
        {
            using var command = connection.CreateCommand();
            command.Transaction = transaction;
            command.CommandText = $"SELECT COUNT(*) FROM \"{table}\";";
            counts[table] = Convert.ToInt64(command.ExecuteScalar());
        }
        return counts;
    }

    private static (string DatasetId, string Role) Identity(SqliteConnection connection, SqliteTransaction transaction)
    {
        using var command = connection.CreateCommand();
        command.Transaction = transaction;
        command.CommandText = "SELECT dataset_id, role FROM pclm_file WHERE singleton = 1;";
        using var reader = command.ExecuteReader();
        if (!reader.Read())
            throw new InvalidOperationException("작업자료의 이름표(pclm_file)가 비어 있습니다.");
        return (reader.GetString(0), reader.GetString(1));
    }

    /// <summary>
    /// 사본에 작업자료의 이름표를 적는다 — 역할 <c>work</c>, <b>새 신원</b>, 지은 때는 지금.
    /// 판올림이 WAL 로 바꿔 두었으면 한 파일로 되돌린다: 이름을 바꿀 때 <c>-wal</c> 이 옛 이름에 남으면 안 된다.
    /// </summary>
    /// <param name="required">이름표가 없으면 던지나. 판올림 전의 옛 v19 사본은 아직 없다.</param>
    private static void Stamp(string path, bool required)
    {
        using var connection = new SqliteConnection(new SqliteConnectionStringBuilder
        {
            DataSource = path,
            Mode = SqliteOpenMode.ReadWrite,
            Pooling = false,
        }.ToString());
        connection.Open();

        Execute(connection, null, "PRAGMA journal_mode = DELETE;");

        using var exists = connection.CreateCommand();
        exists.CommandText = "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'pclm_file';";
        if (Convert.ToInt64(exists.ExecuteScalar()) == 0)
        {
            if (required) throw new InvalidOperationException($"뜬 사본에 파일 이름표(pclm_file)가 없습니다: {path}");
            return;
        }

        Execute(connection, null,
            """
            UPDATE pclm_file
               SET role = 'work',
                   dataset_id = lower(hex(randomblob(16))),
                   created_at = strftime('%Y-%m-%dT%H:%M:%f0000Z', 'now')
             WHERE singleton = 1;
            """);
    }

    private static SqliteConnection ReadOnly(string path)
    {
        // 풀에 남기지 않는다 — 다 보고 나면 이 파일의 이름을 바꾼다.
        var connection = new SqliteConnection(new SqliteConnectionStringBuilder
        {
            DataSource = path,
            Mode = SqliteOpenMode.ReadOnly,
            Pooling = false,
        }.ToString());
        connection.Open();
        return connection;
    }

    /// <summary>이 전환이 지은 파일을 지운다. 지은 것만 담겨 오므로 남의 파일을 지울 일이 없다.</summary>
    private static void Discard(List<string> created)
    {
        foreach (var path in created) ViewedFile.Discard(path);
    }

    private static void Execute(SqliteConnection connection, SqliteTransaction? transaction, string sql)
    {
        using var command = connection.CreateCommand();
        command.Transaction = transaction;
        command.CommandText = sql;
        command.ExecuteNonQuery();
    }
}

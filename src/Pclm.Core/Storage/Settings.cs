using Dapper;

namespace Pclm.Core.Storage;

/// <summary>
/// DB 안에 적어 두는 설정. 자료를 어디서 가져올지는 여기 없다 — 자료는 확장·ERP JSON·계획
/// 엑셀로 들어오고(ADR-028), 어느 작업자료를 열지는 DB 밖 홈의 <c>config.json</c> 이 적는다(<see cref="Home"/>).
/// </summary>
/// <param name="SubmitterName">
/// 제출본 파일 이름에 쓸 내 이름. <b>쓰임은 그것뿐이다.</b>
///
/// <para><b>자료의 임자를 정하는 값이 아니다.</b> 담당자 신원은 앱이 아니라 계획 엑셀이
/// 쥔다(ADR-023) — 행마다 담당자가 적혀 있어 자료를 따라 움직인다. 앱에 심은 이름은 파일을
/// 따라가지 않으므로, 그것으로 임자를 정하면 여러 사람의 것을 한 파일로 모았을 때 어느 줄이
/// 누구 것인지 알 길이 없다. 여기 있는 것은 <c>제출_홍길동_20260729.pclm</c> 의 가운데 토막
/// 하나다.</para>
///
/// <para>비면 빈 문자열이다 — <c>null</c> 이 아니다.</para>
/// </param>
/// <param name="PlanPath">
/// 마지막으로 고른 연간 조달계획 엑셀의 <b>전체 경로</b>.
///
/// <para><b>경로가 곧 링크다.</b> 계획 서식이 표본에 고정된 뒤(ADR-023 개정) 계획 가져오기에
/// 남은 결정은 "어느 파일인가" 하나뿐이라, 그 하나를 기억해 둔다 — 같은 파일을 매달 다시
/// 고르게 하는 것은 사람이 컴퓨터를 대신하는 일이다.</para>
///
/// <para>여기 있는 것이 <b>지금도 그 자리에 있다는 뜻은 아니다</b>. 파일이 옮겨 가거나
/// 지워졌으면 가져올 때 그 자리에서 알린다 — 고를 때 막으면 고쳐 둔 경로를 쓰지 못한다.</para>
///
/// <para>비면 빈 문자열이다 — <c>null</c> 이 아니다.</para>
/// </param>
public sealed record AppSettings(string SubmitterName = "", string PlanPath = "");

/// <summary>
/// 설정을 읽고 쓴다.
///
/// <para><b>설정은 DB 안에 둔다.</b> 따로 파일로 두면 자료와 설정이 각자 옮겨 다니다 어긋난다 —
/// 다른 작업자료를 열면 그쪽의 계획 엑셀이 따라오는 것이 맞다.</para>
/// </summary>
public sealed class SettingsStore(Database database)
{
    private const string SubmitterKey = "submit.name";
    private const string PlanPathKey = "plan.path";

    private readonly Database _database = database;

    /// <summary>지금 설정. 한 번도 저장한 적이 없으면 빈 값을 낸다.</summary>
    public AppSettings Read()
    {
        using var connection = _database.Open();

        var values = connection.Query<(string Key, string? Value)>(
                "SELECT key, value FROM app_setting;")
            .ToDictionary(r => r.Key, r => r.Value);

        return new AppSettings(
            // 빈 값은 NULL 이 아니라 빈 문자열이다 — 읽는 쪽이 두 가지 빈 것을 가리지 않게.
            values.TryGetValue(SubmitterKey, out var s) ? s ?? "" : "",
            values.TryGetValue(PlanPathKey, out var p) ? p ?? "" : "");
    }

    /// <summary>
    /// 설정을 저장한다.
    ///
    /// <para><paramref name="submitterName"/> 와 <paramref name="planPath"/> 는 <c>null</c> 이면
    /// <b>그대로 둔다</b>. 이 값을 모르는 부르는 쪽이 저장할 때마다 이름이나 골라 둔 엑셀이
    /// 조용히 지워지면 안 된다. 지우려면 빈 문자열을 준다.</para>
    /// </summary>
    public AppSettings Save(string? submitterName = null, string? planPath = null)
    {
        using var connection = _database.Open();
        using var transaction = connection.BeginTransaction();

        if (submitterName is not null) Put(connection, transaction, SubmitterKey, submitterName.Trim());
        if (planPath is not null) Put(connection, transaction, PlanPathKey, planPath.Trim());

        transaction.Commit();
        return Read();
    }

    /// <summary>
    /// 고른 계획 엑셀의 자리만 적어 둔다. 고르기는 다른 설정과 상관없이 혼자 벌어지는 일이다.
    /// </summary>
    public AppSettings SavePlanPath(string path) => Save(planPath: path);

    private static void Put(
        Microsoft.Data.Sqlite.SqliteConnection connection,
        Microsoft.Data.Sqlite.SqliteTransaction transaction,
        string key, string? value) =>
        connection.Execute(
            """
            INSERT INTO app_setting (key, value) VALUES (@key, @value)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value;
            """,
            new { key, value }, transaction);
}

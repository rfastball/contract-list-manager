using System.Text.Json;
using Dapper;
using Microsoft.Data.Sqlite;
using Pclm.Core.Storage;

namespace Pclm.Core.Erp;

public static class ErpConnection
{
    public const string HostName = "kr.rfastball.pclm.erp";
    public static string DirectoryPath => Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Pclm");
    public static string BindingPath => Path.Combine(DirectoryPath, "erp-connection.json");
    public sealed record Binding(string Path, string DatasetId);
    public static void Prepare(Database database)
    {
        using var c = database.Open();
        c.Execute("UPDATE erp_dataset SET environment='production' WHERE singleton=1");
        Directory.CreateDirectory(DirectoryPath);
        var binding = new Binding(database.Path, c.QuerySingle<string>("SELECT dataset_id FROM erp_dataset"));
        var temp = BindingPath + ".tmp";
        File.WriteAllText(temp, JsonSerializer.Serialize(binding));
        File.Move(temp, BindingPath, overwrite: true);
    }
    public static Database OpenBound()
    {
        var binding = JsonSerializer.Deserialize<Binding>(File.ReadAllText(BindingPath))
            ?? throw new InvalidOperationException("메인 프로그램에서 저장 대상을 준비하세요.");
        if (!File.Exists(binding.Path)) throw new InvalidOperationException("연결된 DB가 없습니다. 메인 프로그램을 다시 실행하세요.");
        using var c = new SqliteConnection(new SqliteConnectionStringBuilder { DataSource = binding.Path, Mode = SqliteOpenMode.ReadOnly }.ToString());
        c.Open();
        if (c.ExecuteScalar<int>("PRAGMA user_version") != Schema.Version ||
            c.QuerySingle<string>("SELECT dataset_id FROM erp_dataset") != binding.DatasetId ||
            c.QuerySingle<string>("SELECT environment FROM erp_dataset") != "production")
            throw new InvalidOperationException("연결된 자료가 바뀌었습니다. 메인 프로그램에서 다시 준비하세요.");
        return new Database(binding.Path);
    }
    public static bool IsAllowedOrigin(string origin) =>
        System.Text.RegularExpressions.Regex.IsMatch(origin, @"\Achrome-extension://[a-p]{32}/\z") &&
        File.Exists(Path.Combine(DirectoryPath, "extension-origin.txt")) &&
        File.ReadAllText(Path.Combine(DirectoryPath, "extension-origin.txt")).Trim() == origin;
}

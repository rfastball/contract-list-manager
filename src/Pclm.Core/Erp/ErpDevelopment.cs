using Dapper;
using Pclm.Core.Storage;

namespace Pclm.Core.Erp;

public static class ErpDevelopment
{
    public const string HostName = "kr.rfastball.pclm.erp.dev";
    public static string DirectoryPath => Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Pclm.Erp.Dev");
    public static string DatabasePath => Path.Combine(DirectoryPath, "pclm.db");
    public static string OriginPath => Path.Combine(DirectoryPath, "extension-origin.txt");

    // GUI만 초기화한다. Native host는 아래 경로가 없어도 다른 DB로 우회하지 않는다.
    public static void MarkInitialized(Database database)
    {
        if (!string.Equals(database.Path, Path.GetFullPath(DatabasePath), StringComparison.OrdinalIgnoreCase))
            throw new InvalidOperationException("ERP 개발 DB 경로가 아닙니다.");
        using var connection = database.Open();
        connection.Execute("UPDATE erp_dataset SET environment = 'development' WHERE singleton = 1;");
    }

    public static bool IsAllowedOrigin(string origin) =>
        System.Text.RegularExpressions.Regex.IsMatch(origin, @"\Achrome-extension://[a-p]{32}/\z") &&
        File.Exists(OriginPath) && File.ReadAllText(OriginPath).Trim() == origin;
}

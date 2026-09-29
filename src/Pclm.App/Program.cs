using System.IO;
using Pclm.Core.Erp;

namespace Pclm.App;

public static class Program
{
    [STAThread]
    public static int Main(string[] args)
    {
        // WPF 초기화 전에 분기한다. 브라우저가 실행한 host에서 창·대화상자가 뜨면 안 된다.
        if (args.FirstOrDefault()?.StartsWith("chrome-extension://", StringComparison.Ordinal) == true ||
            args.Contains("--native-dev"))
        {
            try
            {
                var development = args.Contains("--native-dev") || Path.GetFileNameWithoutExtension(Environment.ProcessPath ?? "").EndsWith("-erp-dev", StringComparison.Ordinal);
                if (!args.Contains("--native-dev") && !(development ? ErpDevelopment.IsAllowedOrigin(args[0]) : ErpConnection.IsAllowedOrigin(args[0]))) return 2;
                NativeMessaging.Run(Console.OpenStandardInput(), Console.OpenStandardOutput(), request =>
                {
                    try
                    {
                        var host = development ? new ErpCapture(ErpDevelopment.DatabasePath) : new ErpCapture(ErpConnection.OpenBound(), ErpConnection.DirectoryPath);
                        return host.Dispatch(request);
                    }
                    catch (Exception e) when (e is IOException or InvalidOperationException or System.Text.Json.JsonException or Microsoft.Data.Sqlite.SqliteException)
                    {
                        return new { protocolVersion = 2, requestId = request.TryGetProperty("requestId", out var id) ? id.GetString() : null,
                            ok = false, error = new { code = "setup_required", message = "메인 프로그램에서 저장 대상과 확장 연결을 준비하세요." } };
                    }
                });
                return 0;
            }
            catch (Exception e) when (e is IOException or UnauthorizedAccessException)
            {
                // 길이 오류·끊긴 파이프: stdout에 진단문이나 원문을 섞지 않는다.
                return 1;
            }
        }
        var app = new App();
        return app.Run();
    }
}

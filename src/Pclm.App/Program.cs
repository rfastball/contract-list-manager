using System.IO;
using Pclm.Core.Erp;
using Pclm.Core.Storage;

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
                // 업무·개발 호스트는 홈만 다르다. 쪽지를 읽기만 하고 쓰지 않는다(ErpConnection).
                var home = development ? Home.Development : Home.Default;
                if (!args.Contains("--native-dev") && !ErpConnection.IsAllowedOrigin(home, args[0])) return 2;
                var environment = development ? "development" : "production";
                // 상시 연결 인사(presence)는 작업자료를 열기 전에 Serve 가 받는다 — 포트를 쥔 채 브라우저만큼 사는
                // 호스트가 DB 를 쥐면 안 된다. 포트가 닫히면 Serve 가 끊김을 적고 이 프로세스의 연결 파일을 지운다.
                // 그동안 이 exe 를 붙드므로, 사람이 옛 exe 를 비켜 새 판을 놓으면 그것을 보고 떠난다(ADR-035).
                ExtensionPresence.Serve(Console.OpenStandardInput(), Console.OpenStandardOutput(), home.Directory, request =>
                {
                    try
                    {
                        // 개발용인지는 파일이 아니라 이 호스트가 안다 — 확장이 hello 의 environment 로 팝업에 띄운다.
                        var host = new ErpCapture(ErpConnection.OpenBound(home), home.Directory, environment, home);
                        return host.Dispatch(request);
                    }
                    catch (Exception e) when (e is IOException or InvalidOperationException or System.Text.Json.JsonException or Microsoft.Data.Sqlite.SqliteException)
                    {
                        return new { protocolVersion = 2, requestId = request.TryGetProperty("requestId", out var id) ? id.GetString() : null,
                            ok = false, error = new { code = "setup_required", message = "메인 프로그램에서 저장 대상과 확장 연결을 준비하세요." } };
                    }
                }, ExecutableIdentity.OfThisProcess(),
                    // 손으로 띄운 개발 호스트는 등록으로 뜬 것이 아니라 등록을 보지 않는다.
                    registered: args.Contains("--native-dev") ? null
                        : StillRegistered(development ? ErpDevelopment.HostName : ErpConnection.HostName));
                return 0;
            }
            // InvalidDataException(길이 오류·응답 크기 초과)은 IOException 이 아니라 따로 받는다 — 빠지면 stderr 에 스택이 찍힌다.
            catch (Exception e) when (e is IOException or UnauthorizedAccessException or InvalidDataException)
            {
                // 길이 오류·끊긴 파이프: stdout에 진단문이나 원문을 섞지 않는다.
                return 1;
            }
        }
        var app = new App();
        var code = app.Run();
        app.StartAgainIfAsked();
        return code;
    }

    /// <summary>
    /// Chrome·Edge 의 등록 중 하나라도 이 exe 를 가리키는가. 새 판을 다른 자리에서 켜면 창이 등록을 그리로 고쳐 적는다 —
    /// 그러면 이 호스트는 떠나고 확장이 새 판으로 다시 붙는다(ADR-035). 등록이 하나도 읽히지 않으면 가리킨다고 친다.
    /// </summary>
    private static Func<bool> StillRegistered(string hostName) => () =>
    {
        var self = Environment.ProcessPath;
        if (self is null) return true;
        var seen = false;
        foreach (var browser in new[] { @"Google\Chrome", @"Microsoft\Edge" })
        {
            using var key = Microsoft.Win32.Registry.CurrentUser.OpenSubKey($@"Software\{browser}\NativeMessagingHosts\{hostName}");
            if (key?.GetValue("") is not string manifest || !File.Exists(manifest)) continue;
            seen = true;
            if (ExecutableIdentity.ManifestPointsAt(File.ReadAllText(manifest), self)) return true;
        }
        return !seen;
    };
}

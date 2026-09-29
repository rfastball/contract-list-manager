using System.IO;
using System.Reflection;
using System.Windows;
using Microsoft.Web.WebView2.Core;

namespace Pclm.App;

/// <summary>
/// 화면을 담는 창. 그림은 전부 웹으로 그리고 여기서는 <b>WebView2 를 띄우고 다리를 놓는 일</b>만 한다.
///
/// <para>프론트를 파일 경로가 아니라 <b>가상 호스트</b>로 여는 이유는, <c>file://</c> 로 열면
/// 모듈 스크립트가 origin 규칙에 걸려 막히기 때문이다.</para>
/// </summary>
public partial class MainWindow : Window
{
    private const string VirtualHost = "pclm.local";

    private readonly Bridge _bridge;
    private readonly string? _uiOrigin;
    private readonly string? _profileDirectory;

    /// <param name="uiOrigin">
    /// 화면을 여기서 연다. <c>null</c> 이면 지어 둔 <c>wwwroot</c> 를 가상 호스트로 낸다.
    ///
    /// <para>개발 중에 Vite 서버(<c>http://localhost:5173</c>)를 짚는 자리다. 그러면 화면은
    /// 고칠 때마다 바뀌면서 <b>다리와 DB 는 진짜를 쓴다</b> — 가짜 다리(<c>mock.ts</c>)로는
    /// 드러나지 않는 어긋남이 여기서 잡힌다. <c>chrome.webview</c> 는 출처를 가리지 않고
    /// 주입되므로 다리는 그대로 붙는다.</para>
    /// </param>
    public MainWindow(Bridge bridge, string? uiOrigin = null, string? profileDirectory = null)
    {
        _bridge = bridge;
        _uiOrigin = uiOrigin;
        _profileDirectory = profileDirectory;
        InitializeComponent();
        Loaded += async (_, _) => await StartAsync();
    }

    private async Task StartAsync()
    {
        // 딴 데서 여는 중이면 지어 둔 화면이 없어도 된다 — 그게 -Dev 로 도는 까닭이다.
        if (_uiOrigin is null && Read("wwwroot/index.html") is null)
        {
            ShowFallback("화면이 exe 안에 들어 있지 않습니다.\n\n" +
                         "npm install 뒤 npx vite build 를 돌리고 다시 빌드해 주세요 " +
                         "(run.ps1 · publish.ps1 이 그 순서를 쥡니다).");
            return;
        }

        CoreWebView2Environment environment;

        try
        {
            // 사용자 자료가 아니라 앱 데이터 폴더에 캐시를 둔다.
            var profile = _profileDirectory ?? Path.Combine(
                Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
                "Pclm", "WebView2");

            environment = await CoreWebView2Environment.CreateAsync(userDataFolder: profile);
            await View.EnsureCoreWebView2Async(environment);
        }
        catch (Exception ex)
        {
            ShowFallback("WebView2 런타임을 열지 못했습니다.\n\n" + ex.Message +
                         "\n\nMicrosoft Edge WebView2 런타임을 설치한 뒤 다시 실행해 주세요.");
            return;
        }

        var core = View.CoreWebView2;

        if (_uiOrigin is null)
        {
            core.AddWebResourceRequestedFilter($"https://{VirtualHost}/*", CoreWebView2WebResourceContext.All);
            core.WebResourceRequested += (_, e) => Serve(environment, e);
        }

        // 앱 화면이지 브라우저가 아니다 — 나가는 물건에서는 오른쪽 단추 메뉴와 개발자 도구를 닫는다.
        // 여는 것은 디버그 빌드뿐이고, 깃발이 아니라 빌드 설정에 매단다 — 배포본에는
        // 열 수 있는 문이 아예 없어야 한다.
#if DEBUG
        core.Settings.AreDefaultContextMenusEnabled = true;
        core.Settings.AreDevToolsEnabled = true;
#else
        core.Settings.AreDefaultContextMenusEnabled = false;
        core.Settings.AreDevToolsEnabled = false;
#endif
        core.Settings.IsStatusBarEnabled = false;

        // 파일을 받는 창이 아니다(ADR-028). 막지 않으면 놓인 파일로 창이 통째로 넘어가 화면을 잃는다.
        View.AllowExternalDrop = false;

        core.WebMessageReceived += async (_, e) => await _bridge.HandleAsync(core, e.WebMessageAsJson);

        View.Source = _uiOrigin is null
            ? new Uri($"https://{VirtualHost}/index.html")
            : new Uri(_uiOrigin);
    }

    /// <summary>
    /// 가상 호스트로 들어온 요청을 exe 안의 리소스로 갚는다.
    ///
    /// <para>폴더 매핑을 쓰지 않는 까닭이 여기 있다 — 그쪽은 디스크의 진짜 폴더를 요구해서
    /// 배포 단위가 폴더가 되고, 그러면 <c>wwwroot</c> 를 지운 채로 나가는 사고가 열린다.
    /// 여기서는 화면이 exe 와 한 몸이라 그 사고가 있을 수 없다.</para>
    ///
    /// <para>동기로 처리한다. 리소스는 이미 메모리에 있어 기다릴 것이 없고,
    /// <c>GetDeferral</c> 을 쓰면 놓쳤을 때 화면이 통째로 멈춘다.</para>
    /// </summary>
    private void Serve(CoreWebView2Environment environment, CoreWebView2WebResourceRequestedEventArgs e)
    {
        var path = new Uri(e.Request.Uri).AbsolutePath.TrimStart('/');
        if (path.Length == 0) path = "index.html";

        var content = Read("wwwroot/" + path);
        if (content is null)
        {
            e.Response = environment.CreateWebResourceResponse(null, 404, "Not Found", "");
            return;
        }

        // 글꼴은 5MB 를 넘는다. 매 요청마다 다시 읽지 않게 캐시가 걸리도록 헤더를 붙인다.
        var headers = $"Content-Type: {MimeOf(path)}\r\nCache-Control: public, max-age=31536000";
        e.Response = environment.CreateWebResourceResponse(
            new MemoryStream(content), 200, "OK", headers);
    }

    /// <summary>
    /// 박아 둔 이름을 <b>구분자와 대소문자를 지운 꼴</b>로 미리 훑어 둔다.
    ///
    /// <para>이름 표기에 기대지 않으려고 이렇게 한다. MSBuild 의 <c>%(RecursiveDir)</c> 은
    /// 윈도에서 역슬래시를 내므로 리소스가 <c>wwwroot/assets\index-abc.js</c> 로 박히는데,
    /// 요청은 <c>wwwroot/assets/index-abc.js</c> 로 온다. 앞의 한 겹만 맞아 <c>index.html</c>
    /// 은 찾아지고 그 아래 자산이 전부 404 가 되면 <b>창은 뜨는데 화면만 하얗다</b> —
    /// 오류도 없이. 표기를 맞추는 대신 표기를 무시한다.</para>
    /// </summary>
    private static readonly Lazy<Dictionary<string, string>> Embedded = new(() =>
    {
        var assembly = Assembly.GetExecutingAssembly();
        return assembly.GetManifestResourceNames()
            .ToDictionary(Normalize, name => name, StringComparer.Ordinal);
    });

    private static string Normalize(string name) =>
        name.Replace('\\', '/').ToLowerInvariant();

    /// <summary>exe 안에 박아 둔 파일. 없으면 <c>null</c>.</summary>
    private static byte[]? Read(string resource)
    {
        if (!Embedded.Value.TryGetValue(Normalize(resource), out var name)) return null;

        using var stream = Assembly.GetExecutingAssembly().GetManifestResourceStream(name);
        if (stream is null) return null;

        using var buffer = new MemoryStream();
        stream.CopyTo(buffer);
        return buffer.ToArray();
    }

    /// <summary>
    /// 확장자로 종류를 정한다. <b>틀리면 조용히 안 뜬다</b> — 브라우저는 잘못된 종류의
    /// 모듈 스크립트와 스타일시트를 오류 없이 버린다.
    /// </summary>
    private static string MimeOf(string path) => Path.GetExtension(path).ToLowerInvariant() switch
    {
        ".html" => "text/html; charset=utf-8",
        ".js" or ".mjs" => "text/javascript; charset=utf-8",
        ".css" => "text/css; charset=utf-8",
        ".json" or ".map" => "application/json; charset=utf-8",
        ".woff2" => "font/woff2",
        ".woff" => "font/woff",
        ".svg" => "image/svg+xml",
        ".png" => "image/png",
        ".jpg" or ".jpeg" => "image/jpeg",
        ".ico" => "image/x-icon",
        _ => "application/octet-stream",
    };

    private void ShowFallback(string message)
    {
        View.Visibility = Visibility.Collapsed;
        Fallback.Text = message;
        Fallback.Visibility = Visibility.Visible;
    }
}

using System.Globalization;
using System.Text.Json;
using System.Text.Json.Nodes;

namespace Pclm.Core.Erp;

/// <summary>
/// 확장 ↔ 창의 중계(ADR-036). 호스트와 창은 다른 프로세스라 그 사이에 통로가 필요하다 — <b>홈의 파일로 잇는다.</b>
///
/// <list type="bullet">
/// <item>확장이 상시 포트로 보낸 나라장터 탭 보고를 호스트가 <c>extension-live/&lt;pid&gt;.tabs.json</c> 에 통째로 바꿔
/// 쓴다(임시 파일 + 바꿔치기). 창은 그것을 읽어 들인다.</item>
/// <item>창은 명령을 <c>extension-live/&lt;pid&gt;.cmd/&lt;순번&gt;.json</c> 에 놓고, 호스트가 그 폴더를 지켜 포트로 넘긴 뒤
/// 지운다.</item>
/// </list>
///
/// <para><b>named pipe 를 쓰지 않는 까닭.</b> 창이 없어도 호스트는 홀로 서고(브라우저만큼 산다), 창이 늦게 떠도 마지막
/// 상태를 읽고, 시험·관찰이 파일 하나로 끝난다. 상시 연결(<see cref="ExtensionPresence"/>)이 이미 같은 자리에 같은
/// 방식으로 선다.</para>
///
/// <para>탭 보고에는 <b>실제 조달 자료</b>가 들어 있다. 홈(로컬) 밖으로 내지 않고 로그에 남기지 않는다. 호스트가 끝나면
/// 지우고, 끝난 호스트의 것은 읽는 쪽이 거둔다.</para>
/// </summary>
public static class ExtensionRelay
{
    public const string TabsMethod = "tabs";

    /// <summary>창이 보낼 수 있는 명령. 확장은 이것 말고는 받지 않는다.</summary>
    public static readonly IReadOnlySet<string> Commands = new HashSet<string>(StringComparer.Ordinal)
    {
        // 그 탭에 닿는 것 — 탭 번호가 있어야 한다.
        "focusTab", "read", "export",
        // 확장의 설정(「Chrome 확장 상태」 의 설정). 수집기를 띄우는 방식은 옵션 창과 같은 저장 자리에 쓰고, 단축키는
        // 브라우저의 확장 단축키 화면에서만 바뀌므로 그 화면을 연다.
        "setPanelMode", "openShortcuts",
        // 확장 관리 화면. 브라우저는 명령줄로 받은 chrome://·edge:// 주소를 막고 새 탭만 열어, 붙어 있는 확장이 연다.
        "openExtensions",
    };

    /// <summary>탭 번호가 있어야 하는 명령.</summary>
    private static bool ForTab(string command) => command is "focusTab" or "read" or "export";

    /// <summary>수집기를 띄우는 방식 — 확장의 옵션 창과 같은 낱말.</summary>
    public static readonly IReadOnlySet<string> PanelModes = new HashSet<string>(StringComparer.Ordinal) { "always", "button" };

    /// <summary>탭 하나의 화면 줄 수 상한. 확장은 지원 안 되는 화면에서 40줄까지만 싣는다.</summary>
    public const int MaxScreenRows = 400;
    private const int MaxScreenText = 1000;

    /// <summary>명령 폴더를 다시 보는 간격. 파일 감시가 놓친 것을 줍는다.</summary>
    public static readonly TimeSpan PollInterval = TimeSpan.FromMilliseconds(700);

    public static string TabsPath(string directory, int pid) =>
        Path.Combine(ExtensionPresence.LiveDirectory(directory), pid.ToString(CultureInfo.InvariantCulture) + ".tabs.json");

    public static string CommandDirectory(string directory, int pid) =>
        Path.Combine(ExtensionPresence.LiveDirectory(directory), pid.ToString(CultureInfo.InvariantCulture) + ".cmd");

    public static bool IsTabs(JsonElement request) =>
        request.ValueKind == JsonValueKind.Object && request.TryGetProperty("method", out var method) &&
        method.ValueKind == JsonValueKind.String && method.GetString() == TabsMethod;

    /// <summary>
    /// 탭 보고 한 벌을 적는다. 모양만 본다 — 탭마다 번호가 있고 상태가 알려진 낱말인가, 화면 줄(<c>screenRows</c>)이
    /// 글 넷의 줄인가, 확장 설정(<c>settings</c>)이 아는 낱말인가. 값(스냅샷·화면 줄)은 확장이 읽은 그대로 둔다: 검산하지
    /// 않는다(ADR-016). 크기는 요청 상한(<see cref="NativeMessaging.MaxRequestBytes"/>)이 이미 막았다.
    /// </summary>
    /// <returns>받지 않은 까닭. 받았으면 null.</returns>
    public static string? WriteTabs(string directory, int pid, JsonElement report, DateTime now)
    {
        if (report.ValueKind != JsonValueKind.Object || !report.TryGetProperty("tabs", out var tabs) || tabs.ValueKind != JsonValueKind.Array)
            return "탭 보고의 모양이 다릅니다.";
        var written = new JsonArray();
        foreach (var tab in tabs.EnumerateArray())
        {
            if (tab.ValueKind != JsonValueKind.Object || !tab.TryGetProperty("tabId", out var id) || id.ValueKind != JsonValueKind.Number || !id.TryGetInt32(out _) ||
                !tab.TryGetProperty("state", out var state) || state.ValueKind != JsonValueKind.String ||
                state.GetString() is not ("reading" or "supported" or "unsupported" or "error") ||
                tab.TryGetProperty("screenRows", out var rows) && !ScreenRowsFit(rows) ||
                // 메뉴 번호(ADR-037)는 빈 글이거나 숫자 다섯이다. 옛 확장은 싣지 않는다.
                tab.TryGetProperty("menu", out var menu) && !(menu.ValueKind == JsonValueKind.String && (menu.GetString() == "" || Screens.IsCode(menu.GetString()))))
                return "탭 보고의 모양이 다릅니다.";
            written.Add(JsonNode.Parse(tab.GetRawText()));
        }
        JsonObject? settings = null;
        if (report.TryGetProperty("settings", out var given) && given.ValueKind != JsonValueKind.Null)
        {
            settings = Settings(given);
            if (settings is null) return "탭 보고의 모양이 다릅니다.";
        }
        var root = new JsonObject
        {
            ["pid"] = pid,
            ["receivedAt"] = now.ToString("yyyy-MM-ddTHH:mm:ss", CultureInfo.InvariantCulture),
            ["front"] = report.TryGetProperty("front", out var front) && front.ValueKind == JsonValueKind.Number && front.TryGetInt32(out var frontId) ? frontId : null,
            ["frontAt"] = report.TryGetProperty("frontAt", out var frontAt) && frontAt.ValueKind == JsonValueKind.Number && frontAt.TryGetInt64(out var at) ? at : 0,
            ["tabs"] = written,
            ["settings"] = settings,
        };
        Replace(TabsPath(directory, pid), root.ToJsonString());
        return null;
    }

    private static bool ScreenRowsFit(JsonElement rows) =>
        rows.ValueKind == JsonValueKind.Array && rows.GetArrayLength() <= MaxScreenRows &&
        rows.EnumerateArray().All(row => row.ValueKind == JsonValueKind.Object &&
            new[] { "group", "label", "text", "source" }.All(name => row.TryGetProperty(name, out var v) &&
                v.ValueKind == JsonValueKind.String && v.GetString()!.Length <= MaxScreenText));

    private static readonly System.Text.RegularExpressions.Regex Shortcut = new(@"\A[A-Za-z0-9+ .,\-]{0,40}\z");

    /// <summary>
    /// 확장 설정 — 수집기 띄우는 방식 · 현재 화면 바로 저장의 단축키 · 나라장터 사이트 접근. 아는 모양만 받는다(확장이
    /// 홈에 임의 글을 쓰지 못하게). 틀리면 null.
    /// </summary>
    private static JsonObject? Settings(JsonElement value)
    {
        if (value.ValueKind != JsonValueKind.Object ||
            !value.TryGetProperty("panelMode", out var mode) || mode.ValueKind != JsonValueKind.String || !PanelModes.Contains(mode.GetString()!) ||
            !value.TryGetProperty("shortcut", out var key) || key.ValueKind != JsonValueKind.String || !Shortcut.IsMatch(key.GetString()!) ||
            !value.TryGetProperty("siteAccess", out var access) || access.ValueKind is not (JsonValueKind.True or JsonValueKind.False))
            return null;
        return new JsonObject { ["panelMode"] = mode.GetString(), ["shortcut"] = key.GetString(), ["siteAccess"] = access.GetBoolean() };
    }

    /// <summary>한 호스트가 낸 마지막 탭 보고.</summary>
    /// <param name="Report">확장이 보낸 것에 <c>pid</c>·<c>receivedAt</c> 을 얹은 것.</param>
    /// <param name="ConnectedAt">그 호스트의 포트가 열린 때 — 브라우저가 여럿이면 처음 붙은 쪽을 가린다.</param>
    public sealed record TabReport(int Pid, string Browser, JsonElement Report, string ConnectedAt = "");

    /// <summary>
    /// 지금 살아 있는 호스트가 낸 탭 보고들. 살아 있는지는 상시 연결 파일이 정한다(<see cref="ExtensionPresence.ReadLive"/>
    /// 가 끝난 것을 거두며 그 보고와 명령 폴더도 함께 치운다).
    /// </summary>
    public static List<TabReport> ReadTabs(string directory)
    {
        var reports = new List<TabReport>();
        foreach (var (pid, contact) in ExtensionPresence.LiveProcesses(directory))
        {
            try
            {
                var text = File.ReadAllText(TabsPath(directory, pid));
                using var document = JsonDocument.Parse(text, new JsonDocumentOptions { MaxDepth = 64 });
                reports.Add(new(pid, contact.Browser, document.RootElement.Clone(), contact.ConnectedAt));
            }
            catch (Exception e) when (e is IOException or UnauthorizedAccessException or JsonException) { }
        }
        return reports;
    }

    /// <summary>
    /// 창 → 확장 명령 하나를 그 호스트의 명령 폴더에 놓는다. 살아 있지 않은 호스트에는 놓지 않는다 — 아무도 집어 가지
    /// 않는 파일이 쌓인다.
    /// </summary>
    /// <returns>놓았는가.</returns>
    public static bool Send(string directory, int pid, string command, int tabId) => Send(directory, pid, command, tabId, null);

    /// <summary>탭에 닿지 않는 명령(<c>setPanelMode</c>·<c>openShortcuts</c>·<c>openExtensions</c>).</summary>
    /// <param name="mode"><c>setPanelMode</c> 의 <c>always</c>·<c>button</c>.</param>
    public static bool SendSetting(string directory, int pid, string command, string? mode = null) => Send(directory, pid, command, null, mode);

    private static bool Send(string directory, int pid, string command, int? tabId, string? mode)
    {
        if (!Commands.Contains(command)) throw new InvalidOperationException($"모르는 명령입니다: {command}");
        if (ForTab(command) != tabId.HasValue) throw new InvalidOperationException($"명령의 모양이 다릅니다: {command}");
        if (command == "setPanelMode" && (mode is null || !PanelModes.Contains(mode)))
            throw new InvalidOperationException("수집기를 띄우는 방식은 항상 띄우기·툴바 버튼으로만 중 하나입니다.");
        if (!ExtensionPresence.LiveProcesses(directory).Any(p => p.Pid == pid)) return false;
        var folder = CommandDirectory(directory, pid);
        Directory.CreateDirectory(folder);
        // 이름이 곧 차례다. 같은 틱에 둘이 와도 뒤의 GUID 가 가른다(차례는 그 둘 사이에서만 흐려진다).
        var name = DateTime.UtcNow.Ticks.ToString("D19", CultureInfo.InvariantCulture) + "-" + Guid.NewGuid().ToString("N")[..8] + ".json";
        var body = new JsonObject { ["command"] = command };
        if (tabId is { } tab) body["tabId"] = tab;
        if (command == "setPanelMode") body["mode"] = mode;
        Replace(Path.Combine(folder, name), body.ToJsonString());
        return true;
    }

    /// <summary>이 프로세스가 남긴 탭 보고와 명령 폴더를 치운다. 끝나는 길이라 던지지 않는다.</summary>
    public static void Forget(string directory, int pid)
    {
        try { File.Delete(TabsPath(directory, pid)); } catch { /* 읽는 쪽이 거둔다. */ }
        try { Directory.Delete(CommandDirectory(directory, pid), recursive: true); } catch { /* 같다. */ }
    }

    private static void Replace(string path, string text)
    {
        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        // 임시 파일은 .json 으로 끝나지 않게 한다 — 명령 폴더를 지키는 쪽이 반쯤 쓴 것을 집지 않는다.
        var temporary = path + "." + Guid.NewGuid().ToString("N") + ".tmp";
        try
        {
            File.WriteAllText(temporary, text);
            File.Move(temporary, path, overwrite: true);
        }
        finally { if (File.Exists(temporary)) File.Delete(temporary); }
    }

    /// <summary>
    /// 호스트 쪽: 명령 폴더를 지켜 들어온 것을 차례대로 <c>deliver</c> 에 넘기고 지운다. 파일 감시가 놓칠 수 있어 짧게
    /// 다시 본다. 모양이 틀린 것은 넘기지 않고 지운다.
    /// </summary>
    public sealed class Watcher : IDisposable
    {
        private readonly string _folder;
        private readonly Action<JsonObject> _deliver;
        private readonly FileSystemWatcher? _watcher;
        private readonly Timer _timer;
        private readonly object _gate = new();
        private bool _disposed;

        public Watcher(string directory, int pid, Action<JsonObject> deliver, TimeSpan? interval = null)
        {
            _folder = CommandDirectory(directory, pid);
            _deliver = deliver;
            Directory.CreateDirectory(_folder);
            try
            {
                _watcher = new FileSystemWatcher(_folder, "*.json") { NotifyFilter = NotifyFilters.FileName };
                _watcher.Created += (_, _) => Drain();
                _watcher.Renamed += (_, _) => Drain();
                _watcher.EnableRaisingEvents = true;
            }
            catch (Exception e) when (e is IOException or ArgumentException or PlatformNotSupportedException) { _watcher = null; }
            var every = interval ?? PollInterval;
            _timer = new Timer(_ => Drain(), null, every, every);
            Drain();
        }

        /// <summary>쌓인 명령을 차례대로 넘긴다. 시험이 직접 부르기도 한다.</summary>
        public void Drain()
        {
            lock (_gate)
            {
                if (_disposed) return;
                string[] files;
                try { files = Directory.GetFiles(_folder, "*.json"); }
                catch (Exception e) when (e is IOException or UnauthorizedAccessException) { return; }
                foreach (var file in files.Where(f => Path.GetExtension(f) == ".json").Order(StringComparer.Ordinal))
                {
                    JsonObject? command = null;
                    try
                    {
                        if (JsonNode.Parse(File.ReadAllText(file)) is JsonObject parsed &&
                            parsed["command"]?.GetValueKind() == JsonValueKind.String && Commands.Contains(parsed["command"]!.GetValue<string>()))
                        {
                            var name = parsed["command"]!.GetValue<string>();
                            if (ForTab(name) && parsed["tabId"]?.GetValueKind() == JsonValueKind.Number)
                                command = new JsonObject { ["protocolVersion"] = 2, ["command"] = name, ["tabId"] = parsed["tabId"]!.GetValue<int>() };
                            else if (name == "setPanelMode" && parsed["mode"]?.GetValueKind() == JsonValueKind.String &&
                                PanelModes.Contains(parsed["mode"]!.GetValue<string>()))
                                command = new JsonObject { ["protocolVersion"] = 2, ["command"] = name, ["mode"] = parsed["mode"]!.GetValue<string>() };
                            else if (name is "openShortcuts" or "openExtensions")
                                command = new JsonObject { ["protocolVersion"] = 2, ["command"] = name };
                        }
                    }
                    catch (Exception e) when (e is IOException or UnauthorizedAccessException) { continue; } // 아직 쓰는 중 — 다음에 줍는다.
                    catch (Exception e) when (e is JsonException or FormatException or InvalidOperationException) { }
                    try { File.Delete(file); }
                    catch (Exception e) when (e is IOException or UnauthorizedAccessException) { continue; }
                    if (command is not null)
                    {
                        try { _deliver(command); }
                        catch (Exception e) when (e is IOException or ObjectDisposedException or InvalidDataException) { }
                    }
                }
            }
        }

        public void Dispose()
        {
            lock (_gate) _disposed = true;
            _watcher?.Dispose();
            _timer.Dispose();
        }
    }
}

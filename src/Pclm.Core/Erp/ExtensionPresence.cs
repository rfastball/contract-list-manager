using System.ComponentModel;
using System.Diagnostics;
using System.Globalization;
using System.Text.Json;
using System.Text.Json.Nodes;

namespace Pclm.Core.Erp;

/// <summary>
/// 상시 연결(ADR-035). 확장이 <c>connectNative</c> 포트를 쥐고 있는 동안 브라우저가 띄운 호스트는 살아 있고, 그동안
/// 홈의 <c>extension-live/&lt;pid&gt;.json</c> 이 그 호스트를 알린다. 앱은 그 프로세스가 아직 도는 것만 살아 있는 연결로
/// 센다 — 「마지막 인사」 흔적만으로는 확장이 지금 있는지 몰랐다. 붙고 끊길 때마다 <see cref="ExtensionLog"/> 에 한 줄씩.
///
/// <para>포트를 쥔 호스트는 브라우저가 닫힐 때까지 산다. 그래서 <b>DB 를 열지 않고 잠금도 쥐지 않는다</b>
/// (ADR-032) — 쪽지가 없어도, 작업자료가 바뀌어도 이 길은 홈 폴더에 파일만 쓴다. 그리고 그동안 exe 를 붙든다:
/// 제 실행 파일이 비켜지면(<see cref="ExecutableIdentity"/>) 포트를 닫고 떠나, 확장이 그 자리의 새 exe 로 다시 붙게 한다.</para>
/// </summary>
public static class ExtensionPresence
{
    public const string Method = "presence";

    /// <summary>제 실행 파일을 다시 보는 간격.</summary>
    public static readonly TimeSpan WatchInterval = TimeSpan.FromSeconds(3);

    public static string LiveDirectory(string directory) => Path.Combine(directory, "extension-live");
    public static string LivePath(string directory, int pid) => Path.Combine(LiveDirectory(directory), pid.ToString(CultureInfo.InvariantCulture) + ".json");

    public static bool IsPresence(JsonElement request) => IsMethod(request, Method);

    private static bool IsMethod(JsonElement request, string name) =>
        request.ValueKind == JsonValueKind.Object && request.TryGetProperty("method", out var method) &&
        method.ValueKind == JsonValueKind.String && method.GetString() == name;

    /// <summary>
    /// 호스트의 메시지 고리. 상시 연결 인사는 <paramref name="dispatch"/> 에 닿기 전에 여기서 받는다 — 그쪽은 작업자료를
    /// 연다. 포트가 닫혀(stdin 끝) 고리가 끝나면 끊김을 적고 이 프로세스의 파일을 지운다.
    /// </summary>
    /// <param name="watch">인사를 받은 뒤 지켜볼 실행 파일. 하나라도 비켜지면 끊김을 적고 <paramref name="leave"/> 를 부른다.
    /// 요청 하나만 받고 끝나는 수집 호스트는 인사를 받지 않으므로 지켜보지 않는다 — 저장 도중에 떠나지 않는다.</param>
    /// <param name="leave">떠나는 길. 기본은 프로세스를 끝내는 것이다 — stdout 이 닫혀 브라우저가 포트를 끊는다.</param>
    /// <param name="registered">브라우저의 호스트 등록이 아직 이 실행 파일을 가리키는가. 새 판을 다른 자리에 놓으면 등록이
    /// 그리로 옮겨 가고 제 자리는 그대로다 — 그때도 떠나야 확장이 새 판으로 다시 붙는다. 던지면 가리킨다고 친다.</param>
    public static void Serve(Stream input, Stream output, string directory, Func<JsonElement, object> dispatch,
        IReadOnlyList<ExecutableIdentity>? watch = null, TimeSpan? interval = null, Action? leave = null,
        Func<bool>? registered = null)
    {
        var writing = new object();
        var session = new Session(directory, watch ?? [], interval ?? WatchInterval, leave ?? (() => Environment.Exit(0)), registered,
            command => NativeMessaging.Send(output, command, writing));
        try
        {
            NativeMessaging.Run(input, output, request =>
            {
                // 떠나는 길과 겹치지 않게 — 요청 하나를 다 처리한 뒤에만 떠난다.
                lock (session.Gate)
                    return IsPresence(request) ? session.Hello(request)
                        : ExtensionRelay.IsTabs(request) ? session.Tabs(request)
                        : IsMethod(request, ExtensionLog.EventMethod) ? session.Event(request)
                        : dispatch(request);
            }, writing);
        }
        finally { session.End(); }
    }

    private sealed class Session(string directory, IReadOnlyList<ExecutableIdentity> watch, TimeSpan interval, Action leave,
        Func<bool>? registered, Action<JsonObject> push)
    {
        public readonly object Gate = new();
        private (string Browser, string Version)? _hello;
        private Timer? _timer;
        private ExtensionRelay.Watcher? _commands;
        private bool _ended;

        public object Hello(JsonElement request)
        {
            var reply = Handle(request, directory, out var hello);
            if (hello is { } accepted && _hello is null)
            {
                _hello = accepted;
                try { ExtensionLog.RecordConnected(directory, accepted.Browser, accepted.Version, DateTime.Now); }
                catch (Exception e) when (e is IOException or UnauthorizedAccessException) { }
                if (watch.Count > 0 || registered is not null) _timer = new Timer(_ => Check(), null, interval, interval);
                // 창이 놓는 명령(그 탭을 앞으로·다시 읽기)을 포트로 넘긴다(ADR-036). 인사를 받은 호스트만 — 요청 하나만 받는
                // 수집 호스트는 포트를 쥐지 않아 넘길 데가 없다.
                try { _commands = new ExtensionRelay.Watcher(directory, Environment.ProcessId, push); }
                catch (Exception e) when (e is IOException or UnauthorizedAccessException) { }
            }
            return reply;
        }

        /// <summary>
        /// 나라장터 탭 보고(ADR-036). 인사를 받은 포트에서만 받는다 — 상시 연결 파일이 없으면 창은 이 보고를 산 것으로
        /// 세지 않는다. 받은 것은 홈에만 적고 출력하지 않는다(실제 조달 자료다).
        /// </summary>
        public object Tabs(JsonElement request)
        {
            string? requestId = null;
            if (request.TryGetProperty("requestId", out var id) && id.ValueKind == JsonValueKind.String) requestId = id.GetString();
            if (_hello is null) return Error(requestId, "인사 전에는 탭 보고를 받지 않습니다.");
            if (!request.TryGetProperty("params", out var report)) return Error(requestId, "탭 보고가 비었습니다.");
            try
            {
                var refused = ExtensionRelay.WriteTabs(directory, Environment.ProcessId, report, DateTime.Now);
                if (refused is not null) return Error(requestId, refused);
            }
            catch (Exception e) when (e is IOException or UnauthorizedAccessException) { return Error(requestId, "탭 보고를 적지 못했습니다."); }
            return new { protocolVersion = 2, requestId, ok = true, result = new { status = "received" } };
        }

        /// <summary>
        /// 수집 흐름의 사건(ADR-036) — 오류의 코드, 다시 보내 풀림. 연결 기록에 한 줄로 남긴다. 확장은 코드만 보낸다 —
        /// 값·번호·건명은 싣지 않고, 실려 와도 적지 않는다(아는 칸만 읽는다).
        /// </summary>
        public object Event(JsonElement request)
        {
            string? requestId = null;
            if (request.TryGetProperty("requestId", out var id) && id.ValueKind == JsonValueKind.String) requestId = id.GetString();
            if (_hello is not { } hello) return Error(requestId, "인사 전에는 사건을 받지 않습니다.");
            if (!request.TryGetProperty("params", out var args) || args.ValueKind != JsonValueKind.Object ||
                !args.TryGetProperty("kind", out var kind) || kind.ValueKind != JsonValueKind.String)
                return Error(requestId, "사건의 모양이 다릅니다.");
            string? code = args.TryGetProperty("code", out var c) && c.ValueKind == JsonValueKind.String ? c.GetString() : null;
            try
            {
                if (!ExtensionLog.RecordEvent(directory, hello.Browser, hello.Version, kind.GetString()!, code, DateTime.Now))
                    return Error(requestId, "사건의 모양이 다릅니다.");
            }
            catch (Exception e) when (e is IOException or UnauthorizedAccessException) { return Error(requestId, "사건을 적지 못했습니다."); }
            return new { protocolVersion = 2, requestId, ok = true, result = new { status = "recorded" } };
        }

        private bool StillRegistered()
        {
            try { return registered?.Invoke() ?? true; }
            catch (Exception) { return true; }
        }

        private void Check()
        {
            if (watch.All(w => w.StillHere()) && StillRegistered()) return;
            lock (Gate)
            {
                if (_ended) return;
                Finish();
            }
            leave();
        }

        /// <summary>포트가 닫혔다. 이미 떠났으면 아무것도 하지 않는다.</summary>
        public void End()
        {
            lock (Gate)
            {
                if (_ended) return;
                Finish();
            }
        }

        private void Finish()
        {
            _ended = true;
            _timer?.Dispose();
            _commands?.Dispose();
            ExtensionRelay.Forget(directory, Environment.ProcessId);
            if (_hello is { } hello)
            {
                try { ExtensionLog.RecordDisconnected(directory, hello.Browser, hello.Version, DateTime.Now); }
                catch (Exception e) when (e is IOException or UnauthorizedAccessException) { }
            }
            Forget(directory);
        }
    }

    /// <summary>인사를 받아 이 프로세스의 파일을 쓰고 마지막 연결도 적는다. stdout 에는 응답 말고 아무것도 쓰지 않는다.</summary>
    private static object Handle(JsonElement request, string directory, out (string Browser, string Version)? accepted)
    {
        accepted = null;
        string? requestId = null;
        if (request.TryGetProperty("requestId", out var id) && id.ValueKind == JsonValueKind.String) requestId = id.GetString();
        if (!Guid.TryParse(requestId, out _) || !request.TryGetProperty("protocolVersion", out var protocol) ||
            protocol.ValueKind != JsonValueKind.Number || !protocol.TryGetInt32(out var number) || number != 2)
            return Error(requestId, "앱과 확장을 함께 업데이트하세요. 프로토콜 2가 필요합니다.");
        // 확장이 보낸 값이라 마지막 연결과 같은 모양만 받는다 — 확장이 디스크에 임의 문자열을 쓰지 못하게.
        if (!request.TryGetProperty("params", out var args) || args.ValueKind != JsonValueKind.Object ||
            !args.TryGetProperty("browser", out var browserValue) || browserValue.ValueKind != JsonValueKind.String ||
            !args.TryGetProperty("extensionVersion", out var versionValue) || versionValue.ValueKind != JsonValueKind.String ||
            !ExtensionStatus.Accepts(browserValue.GetString()!, versionValue.GetString()!))
            return Error(requestId, "확장이 보낸 브라우저·판을 읽지 못했습니다.");
        var browser = browserValue.GetString()!;
        var version = versionValue.GetString()!;
        try
        {
            using var self = Process.GetCurrentProcess();
            var live = new JsonObject
            {
                ["browser"] = browser, ["version"] = version, ["pid"] = self.Id,
                ["processStartUtc"] = self.StartTime.ToUniversalTime().ToString("O", CultureInfo.InvariantCulture),
                ["connectedAt"] = DateTime.Now.ToString("yyyy-MM-ddTHH:mm:ss", CultureInfo.InvariantCulture),
            };
            var path = LivePath(directory, self.Id);
            Directory.CreateDirectory(LiveDirectory(directory));
            var temporary = path + "." + Guid.NewGuid().ToString("N") + ".tmp";
            try
            {
                File.WriteAllText(temporary, live.ToJsonString());
                File.Move(temporary, path, overwrite: true);
            }
            finally { if (File.Exists(temporary)) File.Delete(temporary); }
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException or Win32Exception)
        { return Error(requestId, "확장 연결을 적지 못했습니다."); }
        accepted = (browser, version);
        try { ExtensionStatus.RecordContact(directory, version, browser); }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException) { }
        return new { protocolVersion = 2, requestId, ok = true, result = new { status = "present" } };
    }

    /// <summary>이 프로세스의 파일을 지운다. 끝나는 길이라 무슨 일이 있어도 던지지 않고 아무것도 출력하지 않는다.</summary>
    public static void Forget(string directory)
    {
        try { File.Delete(LivePath(directory, Environment.ProcessId)); }
        catch { /* 남은 파일은 읽는 쪽이 프로세스를 보고 거둔다. */ }
    }

    /// <summary>
    /// 살아 있는 연결. 파일의 프로세스가 아직 돌고 <b>그 시작 시각까지 같아야</b> 센다 — 끝난 호스트의 번호를 다른
    /// 프로세스가 물려받을 수 있다. 끝난 것의 파일은 거두고, 모양이 깨진 파일은 건너뛴다.
    /// </summary>
    public static List<ExtensionStatus.LiveContact> ReadLive(string directory) =>
        LiveProcesses(directory).Select(p => p.Contact).OrderBy(c => c.ConnectedAt, StringComparer.Ordinal).ToList();

    /// <summary>
    /// <see cref="ReadLive"/> 와 같되 호스트의 프로세스 번호를 함께 낸다 — 중계(<see cref="ExtensionRelay"/>)가 그 번호로
    /// 탭 보고와 명령 폴더를 찾는다. 끝난 호스트의 연결 파일을 거둘 때 그 보고와 명령 폴더도 함께 치운다.
    /// </summary>
    public static List<(int Pid, ExtensionStatus.LiveContact Contact)> LiveProcesses(string directory)
    {
        var live = new List<(int, ExtensionStatus.LiveContact)>();
        string[] files;
        try { files = Directory.GetFiles(LiveDirectory(directory), "*.json"); }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException) { return live; }
        foreach (var file in files)
        {
            if (!Path.GetExtension(file).Equals(".json", StringComparison.OrdinalIgnoreCase)) continue;
            // 탭 보고(<pid>.tabs.json)는 연결 파일이 아니다 — 크기도 커서 읽지 않고 건너뛴다.
            if (Path.GetFileName(file).EndsWith(".tabs.json", StringComparison.OrdinalIgnoreCase)) continue;
            int pid; DateTime started; string browser, version, connectedAt;
            try
            {
                if (JsonNode.Parse(File.ReadAllText(file)) is not JsonObject entry ||
                    entry["pid"]?.GetValueKind() != JsonValueKind.Number ||
                    entry["processStartUtc"]?.GetValueKind() != JsonValueKind.String ||
                    entry["browser"]?.GetValueKind() != JsonValueKind.String ||
                    entry["version"]?.GetValueKind() != JsonValueKind.String ||
                    entry["connectedAt"]?.GetValueKind() != JsonValueKind.String) continue;
                pid = entry["pid"]!.GetValue<int>();
                browser = entry["browser"]!.GetValue<string>();
                version = entry["version"]!.GetValue<string>();
                connectedAt = entry["connectedAt"]!.GetValue<string>();
                if (Path.GetFileName(file) != Path.GetFileName(LivePath(directory, pid)) || !ExtensionStatus.Accepts(browser, version) ||
                    !DateTime.TryParse(entry["processStartUtc"]!.GetValue<string>(), CultureInfo.InvariantCulture,
                        DateTimeStyles.RoundtripKind, out started)) continue;
            }
            catch (Exception e) when (e is IOException or UnauthorizedAccessException or JsonException or FormatException or InvalidOperationException)
            { continue; }
            switch (Running(pid, started.ToUniversalTime()))
            {
                case true: live.Add((pid, new(browser, version, connectedAt))); break;
                case false:
                    try { File.Delete(file); }
                    catch (Exception e) when (e is IOException or UnauthorizedAccessException) { }
                    ExtensionRelay.Forget(directory, pid);
                    break;
                // null: 그 프로세스를 들여다볼 수 없다. 세지도 거두지도 않는다.
            }
        }
        return live;
    }

    /// <summary>그 번호의 프로세스가 그때 시작한 그것으로 아직 도는가. 알 수 없으면 null.</summary>
    private static bool? Running(int pid, DateTime startedUtc)
    {
        try
        {
            using var process = Process.GetProcessById(pid);
            return Math.Abs((process.StartTime.ToUniversalTime() - startedUtc).TotalSeconds) <= 2;
        }
        catch (ArgumentException) { return false; } // 그 번호의 프로세스가 없다.
        catch (InvalidOperationException) { return false; } // 막 끝났다.
        catch (Win32Exception) { return null; }
        catch (NotSupportedException) { return null; }
    }

    private static object Error(string? requestId, string message) =>
        new { protocolVersion = 2, requestId, ok = false, error = new { code = "invalid_request", message } };
}

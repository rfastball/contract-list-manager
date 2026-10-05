using System.Buffers.Binary;
using System.IO.Pipes;
using System.Text.Json;
using Pclm.Core.Erp;
using Pclm.Core.Storage;
using Xunit;

namespace Pclm.Core.Tests;

/// <summary>
/// 상시 연결(ADR-035) — 확장이 쥔 포트로 뜬 호스트가 홈에 자신을 알리고, 붙고 끊긴 것을 기록하고, 제 exe 가 비켜지면
/// 떠나는지. 브라우저 없이 길이 접두 stdio 를 직접 흘려 넣는다.
/// </summary>
public sealed class ExtensionPresenceTests : IDisposable
{
    private readonly string _root = Path.Combine(Path.GetTempPath(), "pclm-presence-" + Guid.NewGuid().ToString("N"));

    public ExtensionPresenceTests() => Directory.CreateDirectory(_root);

    public void Dispose() => Directory.Delete(_root, recursive: true);

    private static byte[] Frame(byte[] payload)
    {
        var frame = new byte[payload.Length + 4];
        BinaryPrimitives.WriteUInt32LittleEndian(frame, (uint)payload.Length);
        payload.CopyTo(frame, 4); return frame;
    }

    private static byte[] PresenceFrame(string browser = "Chrome", string version = "0.8.0", int protocolVersion = 2, string? requestId = null) =>
        Frame(JsonSerializer.SerializeToUtf8Bytes(new { protocolVersion, requestId = requestId ?? Guid.NewGuid().ToString(),
            method = ExtensionPresence.Method, @params = new { extensionVersion = version, browser } }));

    private static List<JsonElement> Replies(MemoryStream output)
    {
        output.Position = 0;
        var replies = new List<JsonElement>();
        while (output.Position < output.Length)
        {
            var header = new byte[4]; output.ReadExactly(header);
            var body = new byte[BinaryPrimitives.ReadUInt32LittleEndian(header)]; output.ReadExactly(body);
            replies.Add(JsonDocument.Parse(body).RootElement.Clone());
        }
        return replies;
    }

    private static IEnumerable<(string Event, string Browser, string Version, string Previous)> Events(string directory) =>
        ExtensionLog.Read(directory, DateTime.MinValue).Select(e => (e.Event, e.Browser, e.Version, e.Previous));

    [Fact]
    public void 상시_연결_인사는_작업자료가_없는_홈에도_연결_파일만_쓰고_포트가_닫히면_지우며_붙음과_끊김을_적는다()
    {
        // 쪽지도 작업자료도 없는 홈. 인사가 작업자료 쪽(dispatch)으로 새면 여기서 터진다.
        var home = new Home(Path.Combine(_root, "빈 홈"));
        var livePath = ExtensionPresence.LivePath(home.Directory, Environment.ProcessId);
        var hello = Frame(JsonSerializer.SerializeToUtf8Bytes(new { protocolVersion = 2, requestId = Guid.NewGuid().ToString(), method = "hello" }));
        using var input = new MemoryStream(PresenceFrame().Concat(hello).ToArray());
        using var output = new MemoryStream();
        var seen = new List<ExtensionStatus.LiveContact>();
        var log = new List<ExtensionLog.Entry>();
        var dispatched = new List<string?>();
        ExtensionPresence.Serve(input, output, home.Directory, request =>
        {
            dispatched.Add(request.GetProperty("method").GetString());
            Assert.True(File.Exists(livePath)); // 포트가 열린 동안은 파일이 있다.
            var state = ExtensionStatus.Read(home.Directory, "0.8.0");
            seen.AddRange(state.Live);
            log.AddRange(state.Log);
            return new { ok = true };
        });

        Assert.Equal(new[] { "hello" }, dispatched);
        var reply = Replies(output)[0];
        Assert.True(reply.GetProperty("ok").GetBoolean());
        Assert.Equal("present", reply.GetProperty("result").GetProperty("status").GetString());
        var live = Assert.Single(seen);
        Assert.Equal(("Chrome", "0.8.0"), (live.Browser, live.Version));
        Assert.True(DateTime.TryParse(live.ConnectedAt, out _));
        Assert.Equal(ExtensionLog.Connected, Assert.Single(log).Event);

        // 끝나면 이 프로세스의 파일은 없고, 마지막 연결 흔적과 붙음·끊김 기록이 남는다. 작업자료·쪽지는 서지 않았다.
        Assert.False(File.Exists(livePath));
        Assert.Equal(("Chrome", "0.8.0"), ExtensionStatus.Read(home.Directory, "0.8.0").Contacts.Select(c => (c.Browser, c.Version)).Single());
        Assert.Equal(new[] { (ExtensionLog.Connected, "Chrome", "0.8.0", ""), (ExtensionLog.Disconnected, "Chrome", "0.8.0", "") }, Events(home.Directory));
        Assert.Equal(ConfigState.Missing, home.ReadConfig().State);
        Assert.Equal(new[] { "extension-contact.json", "extension-log.txt" },
            Directory.GetFiles(home.Directory, "*", SearchOption.AllDirectories).Select(Path.GetFileName).Order(StringComparer.Ordinal));
    }

    [Fact]
    public void 상시_연결은_모양이_틀린_인사를_적지_않는다()
    {
        var directory = Path.Combine(_root, "상시 연결");
        var frames = new[]
        {
            PresenceFrame(browser: "Firefox"), PresenceFrame(version: "../../x"), PresenceFrame(version: "0.8.0\n"),
            PresenceFrame(version: "1.2.3.4.5"), PresenceFrame(version: "0.8.0\t끊김"), PresenceFrame(protocolVersion: 1), PresenceFrame(requestId: "아님"),
        };
        using var input = new MemoryStream(frames.SelectMany(f => f).ToArray());
        using var output = new MemoryStream();
        ExtensionPresence.Serve(input, output, directory, _ => throw new InvalidOperationException("호출되면 안 됨"));
        var replies = Replies(output);
        Assert.Equal(frames.Length, replies.Count);
        Assert.All(replies, r => Assert.False(r.GetProperty("ok").GetBoolean()));
        Assert.False(Directory.Exists(ExtensionPresence.LiveDirectory(directory)));
        Assert.False(File.Exists(Path.Combine(directory, "extension-contact.json")));
        Assert.False(File.Exists(ExtensionLog.LogPath(directory))); // 인사를 받지 않았으면 끊김도 없다.
    }

    [Fact]
    public void 연결_기록을_쓰지_못해도_인사는_성공한다()
    {
        var directory = Path.Combine(_root, "기록 못 씀");
        Directory.CreateDirectory(ExtensionLog.LogPath(directory)); // 기록 자리에 폴더가 섰다.
        using var input = new MemoryStream(PresenceFrame());
        using var output = new MemoryStream();
        ExtensionPresence.Serve(input, output, directory, _ => throw new InvalidOperationException("호출되면 안 됨"));
        Assert.True(Replies(output).Single().GetProperty("ok").GetBoolean());
        Assert.Single(ExtensionStatus.Read(directory, "0.8.0").Contacts);
        Assert.Empty(ExtensionStatus.Read(directory, "0.8.0").Log);
    }

    [Fact]
    public void 살아_있는_연결은_그_프로세스가_그때_시작한_그대로_돌_때만_센다()
    {
        var directory = Path.Combine(_root, "살아 있는 연결");
        var folder = ExtensionPresence.LiveDirectory(directory);
        Directory.CreateDirectory(folder);
        DateTime started;
        using (var self = System.Diagnostics.Process.GetCurrentProcess()) started = self.StartTime.ToUniversalTime();
        string Entry(int pid, DateTime start) => JsonSerializer.Serialize(new { browser = "Edge", version = "0.8.0", pid,
            processStartUtc = start.ToString("O"), connectedAt = "2026-01-01T09:00:00" });

        // 이 번호를 다른 프로세스가 물려받았다(시작 시각이 다르다). 윈도의 PID 는 4의 배수라 int.MaxValue 는 없다.
        var reused = ExtensionPresence.LivePath(directory, Environment.ProcessId);
        var dead = ExtensionPresence.LivePath(directory, int.MaxValue);
        File.WriteAllText(reused, Entry(Environment.ProcessId, started.AddHours(-1)));
        File.WriteAllText(dead, Entry(int.MaxValue, started));
        File.WriteAllText(Path.Combine(folder, "깨짐.json"), "{ 깨진");
        File.WriteAllText(Path.Combine(folder, "12.json"), Entry(Environment.ProcessId, started)); // 이름과 번호가 다르다.
        Assert.Empty(ExtensionStatus.Read(directory, "0.8.0").Live);
        Assert.False(File.Exists(reused));
        Assert.False(File.Exists(dead));
        Assert.True(File.Exists(Path.Combine(folder, "깨짐.json"))); // 모양이 깨진 것은 건너뛰기만 한다.

        File.WriteAllText(reused, Entry(Environment.ProcessId, started));
        var live = Assert.Single(ExtensionStatus.Read(directory, "0.8.0").Live);
        Assert.Equal(("Edge", "0.8.0", "2026-01-01T09:00:00"), (live.Browser, live.Version, live.ConnectedAt));
        Assert.True(File.Exists(reused));
        ExtensionPresence.Forget(directory);
        Assert.False(File.Exists(reused));
        ExtensionPresence.Forget(directory); // 없어도 던지지 않는다.
    }

    [Fact]
    public void 연결_기록은_브라우저마다_판이_바뀌면_남기고_오늘_것만_읽힌다()
    {
        var directory = Path.Combine(_root, "기록");
        var yesterday = new DateTime(2026, 10, 4, 18, 0, 0);
        var today = new DateTime(2026, 10, 5, 8, 30, 0);
        ExtensionLog.RecordConnected(directory, "Chrome", "0.7.0", yesterday);
        ExtensionLog.RecordDisconnected(directory, "Chrome", "0.7.0", yesterday.AddMinutes(2));
        ExtensionLog.RecordConnected(directory, "Chrome", "0.8.0", today);           // 판이 바뀌었다
        ExtensionLog.RecordConnected(directory, "Edge", "0.8.0", today.AddMinutes(1)); // Edge 는 처음이라 바뀜이 아니다
        ExtensionLog.RecordDisconnected(directory, "Chrome", "0.8.0", today.AddMinutes(27));
        ExtensionLog.RecordConnected(directory, "Chrome", "0.8.0", today.AddMinutes(42)); // 같은 판
        ExtensionLog.RecordConnected(directory, "Firefox", "0.8.0", today); // 모양이 틀린 것은 적지 않는다

        Assert.Equal(new[]
        {
            (ExtensionLog.Updated, "Chrome", "0.8.0", "0.7.0"), (ExtensionLog.Connected, "Chrome", "0.8.0", ""),
            (ExtensionLog.Connected, "Edge", "0.8.0", ""), (ExtensionLog.Disconnected, "Chrome", "0.8.0", ""),
            (ExtensionLog.Connected, "Chrome", "0.8.0", ""),
        }, ExtensionLog.Read(directory, today.Date).Select(e => (e.Event, e.Browser, e.Version, e.Previous)));
        Assert.Equal(new[] { "2026-10-05T08:30:00", "2026-10-05T08:30:00", "2026-10-05T08:31:00", "2026-10-05T08:57:00", "2026-10-05T09:12:00" },
            ExtensionStatus.Read(directory, "0.8.0", today.AddHours(2)).Log.Select(e => e.At));
        Assert.Equal(7, Events(directory).Count());
        Assert.Empty(ExtensionLog.Read(Path.Combine(_root, "없는 홈"), DateTime.MinValue));
    }

    [Fact]
    public void 연결_기록은_이레와_500줄을_넘으면_스스로_줄이고_깨진_줄을_걷는다()
    {
        var directory = Path.Combine(_root, "줄이기");
        Directory.CreateDirectory(directory);
        var now = new DateTime(2026, 10, 5, 12, 0, 0);
        var lines = new List<string> { "깨진 줄", "2026-10-05T09:00:00\tconnected\tChrome" };
        // 여드레 전 것 열 줄, 그다음 이틀 전부터 1분 간격으로 600줄.
        for (var i = 0; i < 10; i++) lines.Add($"{now.AddDays(-8).AddMinutes(i):yyyy-MM-ddTHH:mm:ss}\tconnected\tEdge\t0.7.0\t");
        for (var i = 0; i < 600; i++) lines.Add($"{now.AddDays(-2).AddMinutes(i):yyyy-MM-ddTHH:mm:ss}\tdisconnected\tChrome\t0.8.0\t");
        File.WriteAllLines(ExtensionLog.LogPath(directory), lines);

        ExtensionLog.RecordConnected(directory, "Edge", "0.8.0", now);
        var kept = File.ReadAllLines(ExtensionLog.LogPath(directory));
        Assert.Equal(ExtensionLog.MaxLines, kept.Length);
        Assert.All(kept, l => Assert.Equal(5, l.Split('\t').Length));
        Assert.True(string.CompareOrdinal(kept[0], now.AddDays(-7).ToString("yyyy-MM-ddTHH:mm:ss")) >= 0);
        // Edge 의 옛 판은 이번에 이레 밖으로 걷혀 나가지만, 걷기 전에 보고 「판 바뀜」 을 남긴다.
        Assert.Equal(new[] { (ExtensionLog.Updated, "Edge", "0.8.0", "0.7.0"), (ExtensionLog.Connected, "Edge", "0.8.0", "") },
            Events(directory).TakeLast(2));

        // 줄일 것이 없으면 덧붙이기만 한다 — 앞의 줄은 글자 하나 바뀌지 않는다.
        var before = File.ReadAllText(ExtensionLog.LogPath(directory));
        File.WriteAllText(ExtensionLog.LogPath(directory), string.Join("\n", before.Split('\n').Take(10)) + "\n");
        var head = File.ReadAllText(ExtensionLog.LogPath(directory));
        ExtensionLog.RecordDisconnected(directory, "Edge", "0.8.0", now.AddMinutes(1));
        Assert.StartsWith(head, File.ReadAllText(ExtensionLog.LogPath(directory)));
    }

    [Fact]
    public void 여러_호스트가_함께_써도_줄이_섞이지_않는다()
    {
        var directory = Path.Combine(_root, "함께 씀");
        var now = new DateTime(2026, 10, 5, 9, 0, 0);
        Parallel.For(0, 8, worker =>
        {
            for (var i = 0; i < 25; i++)
                if (i % 2 == 0) ExtensionLog.RecordConnected(directory, worker % 2 == 0 ? "Chrome" : "Edge", "0.8.0", now.AddSeconds(i));
                else ExtensionLog.RecordDisconnected(directory, worker % 2 == 0 ? "Chrome" : "Edge", "0.8.0", now.AddSeconds(i));
        });
        var lines = File.ReadAllLines(ExtensionLog.LogPath(directory));
        Assert.Equal(200, lines.Length);
        Assert.Equal(200, ExtensionLog.Read(directory, now.Date).Count);
    }

    [Fact]
    public void 실행_파일의_정체는_비켜지거나_다른_파일이_서면_달라진다()
    {
        var exe = Path.Combine(_root, "계약목록.exe");
        File.WriteAllText(exe, "옛 판");
        var identity = ExecutableIdentity.Of(exe)!;
        Assert.True(identity.StillHere());
        Assert.Null(ExecutableIdentity.Of(Path.Combine(_root, "없음.exe")));
        Assert.Null(ExecutableIdentity.Of(""));

        // 이름을 바꿔 비켰다.
        var aside = OldExecutables.NameFor(exe, new DateTime(2026, 10, 5, 9, 12, 30));
        Assert.Equal(Path.Combine(_root, "계약목록.old-20261005091230.exe"), aside);
        File.Move(exe, aside);
        Assert.False(identity.StillHere());
        // 그 자리에 새 판이 섰다 — 크기가 같아도 쓴 때가 다르다.
        File.WriteAllText(exe, "새 판");
        File.SetLastWriteTimeUtc(exe, identity.LastWriteUtc.AddSeconds(5));
        Assert.False(identity.StillHere());
        // 같은 파일이 같은 때로 돌아오면 같다.
        File.Delete(exe);
        File.Move(aside, exe);
        Assert.True(identity.StillHere());

        Assert.NotEmpty(ExecutableIdentity.OfThisProcess());
        Assert.All(ExecutableIdentity.OfThisProcess(), i => Assert.True(i.StillHere()));
    }

    [Fact]
    public void 상시_연결_호스트는_제_실행_파일이_비켜지면_끊김을_적고_떠나고_수집_호스트는_떠나지_않는다()
    {
        var directory = Path.Combine(_root, "비키기");
        var exe = Path.Combine(_root, "계약목록.exe");
        File.WriteAllText(exe, "옛 판");
        var watch = new[] { ExecutableIdentity.Of(exe)! };
        var livePath = ExtensionPresence.LivePath(directory, Environment.ProcessId);

        // 수집 호스트: 인사 없이 요청 하나. 그사이 exe 가 비켜져도 떠나지 않는다(저장 도중에 끊지 않는다).
        var collectorLeft = false;
        var request = Frame(JsonSerializer.SerializeToUtf8Bytes(new { protocolVersion = 2, requestId = Guid.NewGuid().ToString(), method = "hello" }));
        using (var input = new MemoryStream(request))
        using (var output = new MemoryStream())
            ExtensionPresence.Serve(input, output, directory, _ =>
            {
                File.Move(exe, exe + ".잠깐");
                Thread.Sleep(200);
                File.Move(exe + ".잠깐", exe);
                return new { ok = true };
            }, watch, TimeSpan.FromMilliseconds(20), () => collectorLeft = true);
        Assert.False(collectorLeft);
        Assert.False(File.Exists(ExtensionLog.LogPath(directory)));

        // 상시 연결 호스트: 포트는 열린 채(쓰는 쪽을 닫지 않는다). exe 를 비키면 떠난다.
        using var left = new ManualResetEventSlim();
        using var browser = new AnonymousPipeServerStream(PipeDirection.Out);
        using var port = new AnonymousPipeClientStream(PipeDirection.In, browser.ClientSafePipeHandle);
        var output2 = new MemoryStream();
        var host = new Thread(() => ExtensionPresence.Serve(port, output2, directory,
            _ => throw new InvalidOperationException("호출되면 안 됨"), watch, TimeSpan.FromMilliseconds(20), left.Set));
        host.Start();
        browser.Write(PresenceFrame(browser: "Edge"));
        browser.Flush();
        Assert.True(SpinWait.SpinUntil(() => File.Exists(livePath), TimeSpan.FromSeconds(5)));
        Thread.Sleep(100);
        Assert.False(left.IsSet); // 제자리에 있는 동안은 머문다.

        File.Move(exe, OldExecutables.NameFor(exe, DateTime.Now));
        Assert.True(left.Wait(TimeSpan.FromSeconds(5)));
        Assert.False(File.Exists(livePath));
        Assert.Equal(new[] { ExtensionLog.Connected, ExtensionLog.Disconnected }, Events(directory).Select(e => e.Event));

        // 그 뒤 포트가 닫혀도 끊김을 두 번 적지 않는다.
        browser.Dispose();
        Assert.True(host.Join(TimeSpan.FromSeconds(5)));
        Assert.Equal(2, Events(directory).Count());
    }

    [Fact]
    public void 상시_연결_호스트는_브라우저_등록이_다른_exe_로_옮겨_가면_떠난다()
    {
        // 새 판을 다른 자리에 놓고 켜면 창이 등록을 그리로 고쳐 적는다. 제 exe 는 그대로라 정체로는 모른다.
        var exe = Path.Combine(_root, "옛 자리", "계약목록.exe");
        var manifest = (string path) => JsonSerializer.Serialize(new { name = "kr.rfastball.pclm.erp", path });
        Assert.True(ExecutableIdentity.ManifestPointsAt(manifest(exe), exe));
        Assert.True(ExecutableIdentity.ManifestPointsAt(manifest(exe.ToUpperInvariant()), exe));
        Assert.False(ExecutableIdentity.ManifestPointsAt(manifest(Path.Combine(_root, "새 자리", "계약목록.exe")), exe));
        // 읽지 못하는 문서는 가리킨다고 친다 — 확실하지 않을 때 연결을 끊지 않는다.
        Assert.True(ExecutableIdentity.ManifestPointsAt("{ 깨진", exe));
        Assert.True(ExecutableIdentity.ManifestPointsAt("{}", exe));

        var directory = Path.Combine(_root, "등록 옮김");
        var ours = true;
        using var left = new ManualResetEventSlim();
        using var browser = new AnonymousPipeServerStream(PipeDirection.Out);
        using var port = new AnonymousPipeClientStream(PipeDirection.In, browser.ClientSafePipeHandle);
        var host = new Thread(() => ExtensionPresence.Serve(port, new MemoryStream(), directory,
            _ => throw new InvalidOperationException("호출되면 안 됨"), [], TimeSpan.FromMilliseconds(20), left.Set,
            registered: () => ours));
        host.Start();
        browser.Write(PresenceFrame());
        browser.Flush();
        Assert.True(SpinWait.SpinUntil(() => File.Exists(ExtensionPresence.LivePath(directory, Environment.ProcessId)), TimeSpan.FromSeconds(5)));
        Thread.Sleep(100);
        Assert.False(left.IsSet);

        ours = false;
        Assert.True(left.Wait(TimeSpan.FromSeconds(5)));
        Assert.Equal(new[] { ExtensionLog.Connected, ExtensionLog.Disconnected }, Events(directory).Select(e => e.Event));
        browser.Dispose();
        Assert.True(host.Join(TimeSpan.FromSeconds(5)));
    }

    [Fact]
    public void 등록을_읽다_실패하면_떠나지_않는다()
    {
        var directory = Path.Combine(_root, "등록 못 읽음");
        using var left = new ManualResetEventSlim();
        using var browser = new AnonymousPipeServerStream(PipeDirection.Out);
        using var port = new AnonymousPipeClientStream(PipeDirection.In, browser.ClientSafePipeHandle);
        var host = new Thread(() => ExtensionPresence.Serve(port, new MemoryStream(), directory,
            _ => throw new InvalidOperationException("호출되면 안 됨"), [], TimeSpan.FromMilliseconds(20), left.Set,
            registered: () => throw new UnauthorizedAccessException()));
        host.Start();
        browser.Write(PresenceFrame());
        browser.Flush();
        Thread.Sleep(200);
        Assert.False(left.IsSet);
        browser.Dispose();
        Assert.True(host.Join(TimeSpan.FromSeconds(5)));
    }

    [Fact]
    public void 비켜_둔_옛_exe_는_이름이_맞는_것만_치우고_쓰는_중인_것은_미룬다()
    {
        var beside = Path.Combine(_root, "exe 옆");
        var home = Path.Combine(_root, "홈");
        Directory.CreateDirectory(beside);
        Directory.CreateDirectory(home);
        string Make(string folder, string name) { var path = Path.Combine(folder, name); File.WriteAllText(path, name); return path; }
        var stamped = Make(beside, "계약목록.old-20261005091230.exe");
        var plain = Make(beside, "계약목록.OLD.exe");
        var renamed = Make(beside, "내 계약목록.old-1.exe"); // 사람이 exe 이름을 바꿔 쓰고 있다
        var inHome = Make(home, "계약목록.old.exe");
        var busy = Make(home, "계약목록.old-2.exe");
        var current = Make(beside, "내 계약목록.exe");
        var others = new[] { Make(beside, "설치.old.exe"), Make(beside, "계약목록.old-x.exe"), Make(beside, "계약목록.old.dll"), Make(beside, "계약목록.exe") };

        using (new FileStream(busy, FileMode.Open, FileAccess.Read, FileShare.None))
            OldExecutables.SweepAround(current, home);
        Assert.All(new[] { stamped, plain, renamed, inHome }, p => Assert.False(File.Exists(p), p));
        Assert.True(File.Exists(busy)); // 쓰는 중 — 다음 번에.
        Assert.True(File.Exists(current));
        Assert.All(others, p => Assert.True(File.Exists(p), p));

        Assert.Equal(1, OldExecutables.Sweep(home, [OldExecutables.ShippedStem]));
        Assert.Equal(0, OldExecutables.Sweep(Path.Combine(_root, "없는 폴더"), [OldExecutables.ShippedStem]));
        Assert.True(OldExecutables.IsOld("계약목록.old-20261005091230.exe", "계약목록"));
        Assert.False(OldExecutables.IsOld("계약목록.exe", "계약목록"));
    }

    // ── 수집 흐름의 사건(ADR-036) ─────────────────────────────

    private static byte[] EventFrame(object args) =>
        Frame(JsonSerializer.SerializeToUtf8Bytes(new { protocolVersion = 2, requestId = Guid.NewGuid().ToString(),
            method = ExtensionLog.EventMethod, @params = args }));

    [Fact]
    public void 확장의_오류와_풀림은_인사_뒤에만_코드만_연결_기록에_남는다()
    {
        var directory = Path.Combine(_root, "홈");
        // 인사 전의 사건은 받지 않는다. 인사 뒤의 사건은 코드만 적는다 — 함께 실려 온 자료는 적지 않는다.
        using var input = new MemoryStream(EventFrame(new { kind = "error", code = "unconfirmed" })
            .Concat(PresenceFrame())
            .Concat(EventFrame(new { kind = "error", code = "unconfirmed", title = "시험 계약 건명", number = "R26TA00000001-00" }))
            .Concat(EventFrame(new { kind = "error", code = "모르는 코드" }))
            .Concat(EventFrame(new { kind = "nonsense" }))
            .Concat(EventFrame(new { kind = "recovered" })).ToArray());
        using var output = new MemoryStream();
        ExtensionPresence.Serve(input, output, directory, _ => throw new InvalidOperationException("호출되면 안 됨"));

        Assert.Equal(new[] { false, true, true, false, false, true }, Replies(output).Select(r => r.GetProperty("ok").GetBoolean()));
        Assert.Equal(new[]
        {
            (ExtensionLog.Connected, "Chrome", "0.8.0", ""), (ExtensionLog.Error, "Chrome", "0.8.0", "unconfirmed"),
            (ExtensionLog.Recovered, "Chrome", "0.8.0", ""), (ExtensionLog.Disconnected, "Chrome", "0.8.0", ""),
        }, Events(directory));
        Assert.DoesNotContain("시험 계약", File.ReadAllText(ExtensionLog.LogPath(directory)));
        Assert.DoesNotContain("R26TA", File.ReadAllText(ExtensionLog.LogPath(directory)));
    }

    [Fact]
    public void 오늘의_오류와_오늘_전의_마지막_오류를_가르고_연결_기록에는_붙음_끊김_판_바뀜만_낸다()
    {
        var directory = Path.Combine(_root, "홈");
        var today = DateTime.Today.AddHours(9);
        var yesterday = today.AddDays(-1).AddHours(7);
        ExtensionLog.RecordConnected(directory, "Chrome", "0.8.0", yesterday);
        Assert.True(ExtensionLog.RecordEvent(directory, "Chrome", "0.8.0", ExtensionLog.Error, "unconfirmed", yesterday.AddMinutes(1)));
        Assert.True(ExtensionLog.RecordEvent(directory, "Edge", "0.8.0", ExtensionLog.Error, "connection", yesterday.AddMinutes(2)));
        Assert.True(ExtensionLog.RecordEvent(directory, "Chrome", "0.8.0", ExtensionLog.Recovered, null, yesterday.AddMinutes(3)));
        ExtensionLog.RecordConnected(directory, "Chrome", "0.8.0", today);
        Assert.True(ExtensionLog.RecordEvent(directory, "Chrome", "0.8.0", ExtensionLog.Error, "setup", today.AddMinutes(5)));
        Assert.True(ExtensionLog.RecordEvent(directory, "Chrome", "0.8.0", ExtensionLog.Error, "rejected", today.AddMinutes(9)));
        Assert.False(ExtensionLog.RecordEvent(directory, "Chrome", "0.8.0", ExtensionLog.Error, "Bad Code!", today.AddMinutes(10)));
        Assert.False(ExtensionLog.RecordEvent(directory, "Chrome", "0.8.0", ExtensionLog.Connected, null, today.AddMinutes(10)));
        // 앞으로 더할 사건도 읽는 쪽은 건너뛴다.
        File.AppendAllText(ExtensionLog.LogPath(directory), today.AddMinutes(11).ToString("yyyy-MM-ddTHH:mm:ss") + "\tfuture\tChrome\t0.8.0\t\n");

        var (errors, earlier) = ExtensionLog.Problems(directory, today.AddHours(1));
        Assert.Equal(new[] { ("setup", false), ("rejected", false) }, errors.Select(e => (e.Code, e.Recovered)));
        // 오늘 전의 마지막은 Edge 의 연결 오류 — Chrome 의 풀림은 Edge 의 것을 풀지 않는다.
        Assert.Equal(("Edge", "connection", false), (earlier!.Browser, earlier.Code, earlier.Recovered));
        Assert.True(ExtensionLog.Problems(directory, today.AddHours(1)).Today.All(p => p.At.StartsWith(today.ToString("yyyy-MM-dd"))));

        var state = ExtensionStatus.Read(directory, "0.8.0", today.AddHours(1));
        Assert.Equal(new[] { ExtensionLog.Connected }, state.Log.Select(e => e.Event));
        Assert.Equal(2, state.Errors.Count);
        Assert.Equal("connection", state.PastError!.Code);

        // 풀린 오류.
        var other = Path.Combine(_root, "다른 홈");
        ExtensionLog.RecordEvent(other, "Chrome", "0.8.0", ExtensionLog.Error, "unconfirmed", yesterday);
        ExtensionLog.RecordEvent(other, "Chrome", "0.8.0", ExtensionLog.Recovered, null, yesterday.AddMinutes(1));
        Assert.True(ExtensionLog.Problems(other, today).Earlier!.Recovered);
        Assert.Empty(ExtensionLog.Problems(other, today).Today);
    }
}

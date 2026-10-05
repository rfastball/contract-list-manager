using System.Buffers.Binary;
using System.Diagnostics;
using System.IO.Pipes;
using System.Text.Json;
using System.Text.Json.Nodes;
using Dapper;
using Microsoft.Data.Sqlite;
using Pclm.Core.Erp;
using Pclm.Core.Storage;
using Xunit;

namespace Pclm.Core.Tests;

/// <summary>
/// 지금 보는 화면(ADR-036) — 확장의 탭 보고가 홈의 파일로 창에 닿고 창의 명령이 같은 길로 돌아가는지, 창이 그 화면을
/// 계약면의 표기 그대로 비추되 작업자료를 건드리지 않는지, 창의 가져오기가 확장과 같은 저장 절차를 타는지.
/// 번호·건명·금액은 모두 지어낸 것이다.
/// </summary>
public sealed class MirrorTests : IDisposable
{
    private readonly string _root = Path.Combine(Path.GetTempPath(), "pclm-mirror-" + Guid.NewGuid().ToString("N"));
    private string DbPath => Path.Combine(_root, "계약자료.pclm");
    private readonly Database _db;
    private const string Items = "mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis";

    public MirrorTests()
    {
        Directory.CreateDirectory(_root);
        _db = PclmFile.Create(DbPath, PclmRole.Work);
    }

    public void Dispose()
    {
        SqliteConnection.ClearAllPools();
        Directory.Delete(_root, recursive: true);
    }

    // ── 중계 ───────────────────────────────────────────────

    /// <summary>이 시험 프로세스를 살아 있는 호스트로 세운다 — 상시 연결 파일을 호스트가 쓰는 모양 그대로.</summary>
    private static void Live(string directory, DateTime? startedUtc = null)
    {
        using var self = Process.GetCurrentProcess();
        Directory.CreateDirectory(ExtensionPresence.LiveDirectory(directory));
        File.WriteAllText(ExtensionPresence.LivePath(directory, self.Id), new JsonObject
        {
            ["browser"] = "Chrome", ["version"] = "0.8.0", ["pid"] = self.Id,
            ["processStartUtc"] = (startedUtc ?? self.StartTime.ToUniversalTime()).ToString("O"),
            ["connectedAt"] = "2026-10-05T09:00:00",
        }.ToJsonString());
    }

    private static JsonElement Report(int front, params object[] tabs) =>
        JsonSerializer.SerializeToElement(new { front, frontAt = 1, tabs });

    [Fact]
    public void 탭_보고는_살아_있는_호스트의_것만_읽히고_끝나면_치워진다()
    {
        var home = Path.Combine(_root, "홈");
        var pid = Environment.ProcessId;
        var report = Report(7, new { tabId = 7, windowId = 1, title = "나라장터", state = "supported", screen = "계약 상세", readAt = "09:12",
            snapshot = new { profile = "g2b-contract-v1" } }, new { tabId = 8, state = "unsupported", screen = "개찰 결과" });

        // 상시 연결이 없으면 적은 보고도 산 것으로 세지 않는다.
        Assert.Null(ExtensionRelay.WriteTabs(home, pid, report, DateTime.Now));
        Assert.Empty(ExtensionRelay.ReadTabs(home));

        Live(home);
        var read = Assert.Single(ExtensionRelay.ReadTabs(home));
        Assert.Equal((pid, "Chrome"), (read.Pid, read.Browser));
        Assert.Equal(7, read.Report.GetProperty("front").GetInt32());
        Assert.Equal(new[] { 7, 8 }, read.Report.GetProperty("tabs").EnumerateArray().Select(t => t.GetProperty("tabId").GetInt32()));
        Assert.Equal("g2b-contract-v1", read.Report.GetProperty("tabs")[0].GetProperty("snapshot").GetProperty("profile").GetString());
        Assert.Empty(Directory.GetFiles(ExtensionPresence.LiveDirectory(home), "*.tmp"));

        // 모양이 틀린 보고는 적지 않는다 — 있던 것을 그대로 둔다.
        Assert.NotNull(ExtensionRelay.WriteTabs(home, pid, JsonSerializer.SerializeToElement(new { tabs = new[] { new { tabId = "일곱", state = "supported" } } }), DateTime.Now));
        Assert.NotNull(ExtensionRelay.WriteTabs(home, pid, JsonSerializer.SerializeToElement(new { tabs = new[] { new { tabId = 1, state = "아무거나" } } }), DateTime.Now));
        Assert.Equal(2, Assert.Single(ExtensionRelay.ReadTabs(home)).Report.GetProperty("tabs").GetArrayLength());

        // 같은 번호를 다른 프로세스가 물려받았으면(시작 시각이 다르다) 끝난 호스트다 — 연결 파일과 함께 보고·명령 폴더를 거둔다.
        Assert.True(ExtensionRelay.Send(home, pid, "read", 7));
        Live(home, DateTime.UtcNow.AddHours(-5));
        Assert.Empty(ExtensionRelay.ReadTabs(home));
        Assert.False(File.Exists(ExtensionRelay.TabsPath(home, pid)));
        Assert.False(Directory.Exists(ExtensionRelay.CommandDirectory(home, pid)));
    }

    [Fact]
    public void 명령은_차례대로_넘기고_모양이_틀린_것은_버리며_끝난_호스트에는_놓지_않는다()
    {
        var home = Path.Combine(_root, "홈");
        var pid = Environment.ProcessId;
        Assert.False(ExtensionRelay.Send(home, pid, "focusTab", 3)); // 아무도 집어 가지 않는다.
        Assert.False(Directory.Exists(ExtensionRelay.CommandDirectory(home, pid)));
        Assert.Throws<InvalidOperationException>(() => ExtensionRelay.Send(home, pid, "deleteEverything", 3));

        Live(home);
        Assert.True(ExtensionRelay.Send(home, pid, "focusTab", 3));
        Assert.True(ExtensionRelay.Send(home, pid, "read", 4));
        Assert.True(ExtensionRelay.Send(home, pid, "export", 5));
        File.WriteAllText(Path.Combine(ExtensionRelay.CommandDirectory(home, pid), "9999999999999999999-zz.json"), """{"command":"rm","tabId":1}""");
        File.WriteAllText(Path.Combine(ExtensionRelay.CommandDirectory(home, pid), "반쯤.json.tmp"), "{");

        var delivered = new List<JsonObject>();
        using (var watcher = new ExtensionRelay.Watcher(home, pid, delivered.Add, TimeSpan.FromHours(1)))
        {
            watcher.Drain();
            Assert.Equal(new[] { ("focusTab", 3), ("read", 4), ("export", 5) },
                delivered.Select(c => (c["command"]!.GetValue<string>(), c["tabId"]!.GetValue<int>())));
            Assert.All(delivered, c => Assert.Equal(2, c["protocolVersion"]!.GetValue<int>()));
            // 넘긴 것과 버린 것은 지웠고, 쓰는 중인 임시 파일은 건드리지 않았다.
            Assert.Equal(new[] { "반쯤.json.tmp" }, Directory.GetFiles(ExtensionRelay.CommandDirectory(home, pid)).Select(Path.GetFileName));
        }
    }

    private static byte[] Frame(object message)
    {
        var payload = JsonSerializer.SerializeToUtf8Bytes(message);
        var frame = new byte[payload.Length + 4];
        BinaryPrimitives.WriteUInt32LittleEndian(frame, (uint)payload.Length);
        payload.CopyTo(frame, 4); return frame;
    }

    private static object Presence() => new { protocolVersion = 2, requestId = Guid.NewGuid().ToString(), method = ExtensionPresence.Method,
        @params = new { extensionVersion = "0.8.0", browser = "Chrome" } };

    private static object Tabs(int tabId) => new { protocolVersion = 2, requestId = Guid.NewGuid().ToString(), method = ExtensionRelay.TabsMethod,
        @params = new { front = tabId, frontAt = 1, tabs = new[] { new { tabId, state = "reading", screen = "계약 상세" } } } };

    private static JsonElement Next(Stream output)
    {
        var header = new byte[4]; output.ReadExactly(header);
        var body = new byte[BinaryPrimitives.ReadUInt32LittleEndian(header)]; output.ReadExactly(body);
        return JsonDocument.Parse(body).RootElement.Clone();
    }

    [Fact]
    public void 상시_호스트는_인사_뒤의_탭_보고를_홈에_적고_창의_명령을_포트로_밀어_보내며_끝나면_둘_다_치운다()
    {
        var home = Path.Combine(_root, "홈");
        var pid = Environment.ProcessId;
        using var browser = new AnonymousPipeServerStream(PipeDirection.Out);
        using var port = new AnonymousPipeClientStream(PipeDirection.In, browser.ClientSafePipeHandle);
        using var hostOut = new AnonymousPipeServerStream(PipeDirection.Out);
        using var fromHost = new AnonymousPipeClientStream(PipeDirection.In, hostOut.ClientSafePipeHandle);
        var host = new Thread(() => ExtensionPresence.Serve(port, hostOut, home, _ => throw new InvalidOperationException("호출되면 안 됨")));
        host.Start();

        // 인사 전의 보고는 받지 않는다 — 상시 연결 파일이 없는 보고는 창이 산 것으로 세지 않는다.
        browser.Write(Frame(Tabs(3)));
        Assert.False(Next(fromHost).GetProperty("ok").GetBoolean());
        browser.Write(Frame(Presence()));
        Assert.True(Next(fromHost).GetProperty("ok").GetBoolean());
        browser.Write(Frame(Tabs(3)));
        Assert.Equal("received", Next(fromHost).GetProperty("result").GetProperty("status").GetString());
        Assert.Equal(3, Assert.Single(ExtensionRelay.ReadTabs(home)).Report.GetProperty("front").GetInt32());

        // 창이 놓은 명령은 요청 없이 포트로 나간다.
        Assert.True(ExtensionRelay.Send(home, pid, "focusTab", 3));
        var pushed = Next(fromHost);
        Assert.Equal(("focusTab", 3), (pushed.GetProperty("command").GetString(), pushed.GetProperty("tabId").GetInt32()));

        browser.Dispose();
        Assert.True(host.Join(TimeSpan.FromSeconds(5)));
        Assert.False(File.Exists(ExtensionRelay.TabsPath(home, pid)));
        Assert.False(Directory.Exists(ExtensionRelay.CommandDirectory(home, pid)));
    }

    // ── 투영 ───────────────────────────────────────────────

    private static CaptureInput Contract(string seq, string title, string date, params (int Line, int Quantity, int Price)[] items) => new(
        "g2b-contract-v1", Mapping.Hash(Mapping.Defaults()), JsonSerializer.SerializeToElement(new
        {
            pointInfo = new Dictionary<string, string>
            {
                ["ctrtNoOrd"] = $"R26TA00000001-{seq}", ["ctrtNm"] = title, ["ctrtDt"] = date,
                ["bidPbancNo"] = "R26BK00000001", ["dmstUntyGrpNm"] = "가온시험기관",
            },
            tables = new Dictionary<string, object>
            {
                [Items] = items.Select(i => new
                {
                    ctrtNo = "R26TA00000001", ctrtChgOrd = seq, ctrtItemSqno = i.Line, ctrtItemNm = $"시험 품목 {i.Line}",
                    ctrtUntVal = "세트", ctrtQty = i.Quantity, ctrtUprc = i.Price, ctrtAmt = i.Quantity * i.Price,
                }).ToArray(),
            },
        }), "live", Screen: "01579");

    private void Save(CaptureInput input)
    {
        var host = new ErpCapture(_db);
        var preview = host.Inspect(input);
        host.Save(input, preview.BaseToken, Guid.NewGuid().ToString(), Mirror.DefaultChoices(preview, new HashSet<string>()));
    }

    /// <summary>투영이 아무것도 남기지 않았는가 — 다른 연결이 본 data_version 과 업무 표의 줄 수.</summary>
    private (long Version, long Rows) Footprint(SqliteConnection watcher) => (
        watcher.ExecuteScalar<long>("PRAGMA data_version"),
        watcher.ExecuteScalar<long>("""
            SELECT (SELECT count(*) FROM contract) + (SELECT count(*) FROM contract_item) + (SELECT count(*) FROM contract_series)
                 + (SELECT count(*) FROM erp_source) + (SELECT count(*) FROM erp_row) + (SELECT count(*) FROM erp_capture)
                 + (SELECT count(*) FROM field_override)
            """));

    private static Dictionary<string, string> ViewRow(SqliteConnection c, string view, string key, string value) =>
        ((IDictionary<string, object>)c.QuerySingle($"SELECT * FROM \"{view}\" WHERE \"{key}\"=@value", new { value }))
            .ToDictionary(p => p.Key, p => Convert.ToString(p.Value) ?? "");

    [Fact]
    public void 수집_안_함의_안내는_매핑에서_화면을_단_프로필을_읽는다()
    {
        // 창이 「수집하는 화면은 아래 …」 로 보이는 목록 — 따로 적어 두지 않고 매핑의 screen 에서 끌어낸다(ADR-037).
        Assert.Equal(new[]
        {
            new MirrorScreen("01117", "request", "접수", "조달요구 접수(목록형)"),
            new MirrorScreen("01179", "notice", "공고", "입찰공고 상세"),
            new MirrorScreen("01579", "contract", "계약", "계약 상세"),
        }, Mirror.SupportedScreens(_db));
    }

    [Fact]
    public void 새_자료는_모든_칸을_새로_담길_것으로_비추고_작업자료에는_아무것도_남기지_않는다()
    {
        using var watcher = _db.OpenReadOnly();
        var before = Footprint(watcher);
        var shot = Mirror.Project(_db, Contract("00", "시험 음향설비 구매", "20260914", (1, 6, 2_000_000), (2, 1, 18_400_000)));

        Assert.Equal(("contract", "계약", "R26TA0000000100", "새 자료"), (shot.EntityType, shot.Kind, shot.Number, shot.Status));
        Assert.Null(shot.CompareSeq);
        Assert.Equal("시험 음향설비 구매", shot.Title);
        Assert.Equal("v_계약", shot.View);
        Assert.Contains(shot.Fields, f => f is { Column: "계약번호", Kind: "id", Value: "R26TA0000000100" });
        var date = Assert.Single(shot.Fields, f => f.Column == "계약일자");
        Assert.Equal(("new", "2026/09/14", null), (date.Kind, date.Value, date.Raw)); // 화면이 이름표를 보내지 않았다.
        Assert.Equal("30,400,000", Assert.Single(shot.Fields, f => f.Column == "계약금액").Value);
        Assert.DoesNotContain(shot.Fields, f => f.Column is "진행상태" or "메모"); // 사람이 채우는 열은 화면에서 오지 않는다.
        Assert.True(shot.Rest > 0);
        Assert.Equal(shot.ViewColumns, shot.Fields.Count + shot.Rest);
        Assert.Equal(new[] { ("1", "6", "세트", "2,000,000", "12,000,000"), ("2", "1", "세트", "18,400,000", "18,400,000") },
            shot.Items.Select(i => (i.Line, i.Quantity, i.Unit, i.Price, i.Amount)));
        Assert.True(shot.ItemsAllRead);
        Assert.Equal(2, shot.ItemRows);
        // 자리: 공고는 원천 참조만 있고 아직 수집되지 않았다. 접수는 아무것도 없다.
        Assert.Equal(new[] { ("접수", "missing", ""), ("공고", "ref", "R26BK00000001"), ("계약", "here", "R26TA0000000100") },
            shot.Place.Select(p => (p.Kind, p.State, p.Number)));
        Assert.False(shot.Place[1].Collected);
        Assert.Equal(new[] { ("00", "ghost") }, shot.Rounds.Select(r => (r.Seq, r.State)));
        Assert.Null(shot.Blocked);

        Assert.Equal(before, Footprint(watcher));
    }

    [Fact]
    public void 새_차수는_가장_늦은_차수와_견주고_칸은_저장한_뒤의_계약면과_같은_표기다()
    {
        Save(Contract("00", "시험 음향설비 구매", "20260914", (1, 6, 2_000_000), (2, 1, 18_400_000)));
        using var watcher = _db.OpenReadOnly();
        var before = Footprint(watcher);
        var next = Contract("01", "시험 음향설비 구매", "20261002", (1, 8, 2_000_000), (2, 1, 18_400_000));
        var shot = Mirror.Project(_db, next);

        Assert.Equal(("새 차수", "00", "00"), (shot.Status, shot.CompareSeq, shot.LatestSeq));
        var date = Assert.Single(shot.Fields, f => f.Column == "계약일자");
        Assert.Equal(("changed", "2026/10/02", "2026/09/14"), (date.Kind, date.Value, date.Old));
        Assert.Equal(("changed", "34,400,000", "30,400,000"), Assert.Single(shot.Fields, f => f.Column == "계약금액") is var amount
            ? (amount.Kind, amount.Value, amount.Old) : default);
        Assert.Equal("same", Assert.Single(shot.Fields, f => f.Column == "계약건명").Kind);
        Assert.Equal(3, shot.Changes); // 칸 둘 + 품목 한 줄
        var changed = Assert.Single(shot.Items, i => i.Before is not null);
        Assert.Equal(("1", "8", "16,000,000", "6", "12,000,000"), (changed.Line, changed.Quantity, changed.Amount, changed.Before!.Quantity, changed.Before.Amount));
        Assert.Equal(new[] { ("00", "stored"), ("01", "ghost") }, shot.Rounds.Select(r => (r.Seq, r.State)));
        Assert.Equal("30,400,000", shot.Rounds[0].Amount);
        Assert.Equal(before, Footprint(watcher));

        // 실제로 저장한 뒤의 계약면과 칸마다 같은 표기다.
        Save(next);
        using var c = _db.OpenReadOnly();
        var face = ViewRow(c, "v_계약", "계약번호", "R26TA0000000101");
        Assert.All(shot.Fields, f => Assert.Equal(face[f.Column], f.Value));
        var items = c.Query<(long, string, string)>("SELECT 순번, 수량, 금액 FROM v_계약품목 WHERE 계약번호='R26TA0000000101' ORDER BY 순번")
            .Select(r => (r.Item1.ToString(), r.Item2, r.Item3));
        Assert.Equal(items, shot.Items.Select(i => (i.Line, i.Quantity, i.Amount)));

        // 같은 화면을 다시 비추면 바뀌는 것이 없다.
        var again = Mirror.Project(_db, next);
        Assert.Equal(("저장됨", 0, "01"), (again.Status, again.Changes, again.CompareSeq));
        Assert.All(again.Fields, f => Assert.Contains(f.Kind, new[] { "id", "same" }));
        Assert.Equal(new[] { ("00", "stored"), ("01", "current") }, again.Rounds.Select(r => (r.Seq, r.State)));
    }

    [Fact]
    public void 옛_차수를_다시_비추면_더_늦은_차수가_아니라_그_차수와_견준다()
    {
        Save(Contract("00", "시험 음향설비 구매", "20260914", (1, 6, 2_000_000)));
        Save(Contract("01", "시험 음향설비 구매 변경", "20261002", (1, 8, 2_000_000)));
        var shot = Mirror.Project(_db, Contract("00", "시험 음향설비 구매", "20260914", (1, 6, 2_000_000)));
        Assert.Equal(("저장됨", "00", "01"), (shot.Status, shot.CompareSeq, shot.LatestSeq));
        Assert.Equal("시험 음향설비 구매", shot.Title);
        Assert.Equal("6", Assert.Single(shot.Items).Quantity);
        Assert.Equal(new[] { ("00", "current"), ("01", "stored") }, shot.Rounds.Select(r => (r.Seq, r.State)));
    }

    [Fact]
    public void 사람이_고친_칸의_아래_값이_바뀌면_검토_대기로_비추고_창의_가져오기는_고른_칸만_복원한다()
    {
        var first = Contract("00", "시험 음향설비 구매", "20260914", (1, 6, 2_000_000));
        Save(first);
        using (var c = _db.Open())
            c.Execute("""
                INSERT INTO field_override(entity_type, base, seq, column_name, value, original, updated_at) VALUES
                    ('contract', 'R26TA00000001', '00', '계약건명', '사람이 고친 건명', '시험 음향설비 구매', 'before'),
                    ('contract', 'R26TA00000001', '00', '수요기관', '사람이 고친 기관', '가온시험기관', 'before');
                """);
        var screen = Contract("00", "화면에서 바뀐 건명", "20260914", (1, 6, 2_000_000));
        var shot = Mirror.Project(_db, screen);
        Assert.Equal("검토 대기", shot.Status);
        var title = Assert.Single(shot.Fields, f => f.Kind == "override");
        Assert.Equal(("계약건명", "화면에서 바뀐 건명", "사람이 고친 건명"), (title.Column, title.Value, title.Human));
        Assert.Equal("same", Assert.Single(shot.Fields, f => f.Column == "수요기관").Kind); // 아래 값이 그대로인 덮개는 묻지 않는다.

        // 쪽지가 다른 작업자료를 가리키면 창도 확장처럼 거절한다(ADR-032).
        var home = new Home(Path.Combine(_root, "홈"));
        var host = new ErpCapture(_db, home: home);
        var preview = new ErpCapture(_db).Inspect(screen);
        Assert.Equal(shot.BaseToken, preview.BaseToken);
        var choices = Mirror.DefaultChoices(preview, new HashSet<string> { title.ChoiceId! });
        home.WriteConfig(Path.Combine(_root, "다른.pclm"), "ffff");
        Assert.Throws<InvalidOperationException>(() => host.Save(screen, shot.BaseToken, Guid.NewGuid().ToString(), choices));

        // 화면이나 자료가 바뀐 뒤의 baseToken 은 받지 않는다.
        string dataset;
        using (var c = _db.OpenReadOnly()) dataset = c.QuerySingle<string>("SELECT dataset_id FROM pclm_file");
        home.WriteConfig(DbPath, dataset);
        var stale = Assert.Throws<InvalidOperationException>(() => host.Save(screen, "낡은 토큰", Guid.NewGuid().ToString(), choices));
        Assert.StartsWith("미리보기 이후", stale.Message);

        // 고른 칸만 덮개를 걷고, 같은 수집 ID 로 다시 보내면 같은 영수증이다.
        var id = Guid.NewGuid().ToString();
        var stored = JsonSerializer.Serialize(host.Save(screen, shot.BaseToken, id, choices));
        Assert.Equal(stored, JsonSerializer.Serialize(host.Save(screen, shot.BaseToken, id, choices)));
        using var read = _db.OpenReadOnly();
        Assert.Equal(new[] { "수요기관" }, read.Query<string>("SELECT column_name FROM field_override"));
        Assert.Equal("화면에서 바뀐 건명", read.QuerySingle<string>("SELECT 계약건명 FROM v_계약"));
        Assert.Equal(2, read.ExecuteScalar<int>("SELECT count(*) FROM erp_capture")); // 처음 저장 + 창의 가져오기 한 번
    }

    [Fact]
    public void 고르지_않으면_사람이_고친_값은_그대로_두고_빈_값으로_지우는_변경은_유지한다()
    {
        Save(Contract("00", "시험 음향설비 구매", "20260914", (1, 6, 2_000_000)));
        var wiped = Contract("00", "", "20261002", (1, 6, 2_000_000));
        var preview = new ErpCapture(_db).Inspect(wiped);
        var choices = Mirror.DefaultChoices(preview, new HashSet<string>());
        var title = Assert.Single(preview.Changes, ch => ch.Field == "title");
        var date = Assert.Single(preview.Changes, ch => ch.Field == "contracted_on");
        Assert.Equal(("keep", "apply"), (choices[title.Id], choices[date.Id]));
        var shot = Mirror.Project(_db, wiped);
        Assert.Equal("same", Assert.Single(shot.Fields, f => f.Column == "계약건명").Kind); // 유지하므로 그대로 보인다.
        Assert.Equal("changed", Assert.Single(shot.Fields, f => f.Column == "계약일자").Kind);
    }

    [Fact]
    public void 품목_대응을_골라야_하는_화면은_창에서_가져오지_않는다()
    {
        Save(Contract("00", "시험 음향설비 구매", "20260914", (1, 6, 2_000_000), (2, 1, 18_400_000)));
        // 품목 하나가 빠진 화면 — 빠진 행을 지울지는 사람이 Chrome 의 수집기에서 고른다.
        var shot = Mirror.Project(_db, Contract("00", "시험 음향설비 구매", "20260914", (1, 6, 2_000_000)));
        Assert.Equal("화면에 없는 품목 행을 지울지 골라야 합니다.", shot.Blocked);
    }

    [Fact]
    public void 수집_규칙에_없는_화면은_투영하지_못하고_까닭을_던진다()
    {
        var input = Contract("00", "시험", "20260914") with { Profile = "g2b-unknown" };
        Assert.Throws<InvalidOperationException>(() => Mirror.Project(_db, input));
        Assert.Throws<InvalidOperationException>(() => Mirror.Project(_db, Contract("00", "시험", "20260914") with { MappingRevision = "옛 매핑" }));
    }

    // ── 화면 이름표 잇기 ─────────────────────────────────────

    [Fact]
    public void 면_열의_재료는_열_정의에서_끌어낸다()
    {
        var contract = Views.FaceReferences("contract");
        Assert.Equal(new[] { "contract.title" }, contract["계약건명"]);
        Assert.Equal(new[] { "contract.contracted_on" }, contract["계약일자"]);
        Assert.Equal(new[] { "contract.amount" }, contract["계약금액"]);
        Assert.Equal(new[] { "erp_partner.name" }, contract["계약상대자"]); // 바깥 LEFT JOIN 의 별칭은 풀리지 않아 세지 않는다.
        Assert.False(contract.ContainsKey("계약번호")); // 키 열은 덮개로 감싸지 않는다.

        var notice = Views.FaceReferences("notice");
        Assert.Equal(new[] { "notice.title" }, notice["공고명"]);
        Assert.Equal(new[] { "notice.posted_at" }, notice["게시일시"]);
        Assert.Equal(new[] { "notice.estimated_price" }, notice["추정가격"]);
        Assert.Equal(new[] { "notice_item.item_name" }, notice["세부품명"]); // 품목 줄을 잇는 조건은 재료가 아니다.
        Assert.Equal(new[] { "notice.contract_kind", "notice.contract_method" }, notice["입찰방법"]); // 덮개 표는 걷는다.
        Assert.Equal(new[] { "notice_schedule.starts_at" }, notice["개찰일시"]);

        var request = Views.FaceReferences("request");
        Assert.Equal(new[] { "request.title" }, request["요청명"]);
        Assert.Equal(new[] { "request.received_on" }, request["접수일자"]);
        Assert.Equal(new[] { "request.goods_amount" }, request["품대"]);
        Assert.Empty(Views.FaceReferences("plan"));
    }

    [Fact]
    public void 칸마다_채우는_화면의_자리와_이름표를_잇고_화면_표기는_보이는_글이_다를_때만_곁에_둔다()
    {
        Save(Contract("00", "시험 음향설비 구매", "20260914", (1, 6, 2_000_000)));
        var screen = new List<ScreenRow>
        {
            new("계약 기본정보", "계약번호", "R26TA00000001-01", "ctrtNoOrd"),
            new("계약 기본정보", "계약명", "시험 음향설비 구매", "ctrtNm"),
            new("계약 기본정보", "계약일자", "2026-10-02", "ctrtDt"),
            new("계약 기본정보", "공고번호", "R26BK00000001", "bidPbancNo"),
            new("품목내역", "1 시험 품목 1", "8세트 · 16000000", "table:contract_item:1"),
            new("품목내역", "2 시험 품목 2", "1세트 · 18400000", "table:contract_item:2"),
        };
        var shot = Mirror.Project(_db, Contract("01", "시험 음향설비 구매", "20261002", (1, 8, 2_000_000), (2, 1, 18_400_000)), screen);

        var date = Assert.Single(shot.Fields, f => f.Column == "계약일자");
        Assert.Equal(new[] { "ctrtDt" }, date.Sources);
        Assert.Equal(("계약일자", "2026-10-02"), (date.From, date.Raw));
        var title = Assert.Single(shot.Fields, f => f.Column == "계약건명");
        Assert.Equal(("계약명", (string?)null), (title.From, title.Raw)); // 보이는 글이 표기와 같으면 곁에 두지 않는다.
        var number = Assert.Single(shot.Fields, f => f.Column == "계약번호");
        Assert.Equal(new[] { "ctrtNoOrd" }, number.Sources);
        Assert.Equal("계약번호", number.From);
        // 품목에서 셈하는 총액은 화면의 한 칸에서 오지 않는다 — 대응을 지어내지 않는다.
        var amount = Assert.Single(shot.Fields, f => f.Column == "계약금액");
        Assert.Equal((0, "", (string?)null), (amount.Sources.Count, amount.From, amount.Raw));
        Assert.Equal(new[] { ("1", "table:contract_item:1"), ("2", "table:contract_item:2") }, shot.Items.Select(i => (i.Line, i.Source)));
        var notice = Assert.Single(shot.Place, p => p.Kind == "공고");
        Assert.Equal(("ref", "bidPbancNo"), (notice.State, notice.Source));
        Assert.Null(Assert.Single(shot.Place, p => p.Kind == "계약").Source);

        // 화면 줄을 함께 실은 탭 보고에서 읽는다. 표 자리는 그 표의 모든 줄을 가리킨다.
        var tab = JsonSerializer.SerializeToElement(new { tabId = 1, state = "supported", screenRows = screen.Select(r =>
            new { group = r.Group, label = r.Label, text = r.Text, source = r.Source }) });
        Assert.Equal(screen, Mirror.ScreenRows(tab));
        Assert.True(Mirror.Points("table:contract_item", "table:contract_item:2"));
        Assert.False(Mirror.Points("table:contract_item", "table:contract_item_x:2"));
        Assert.False(Mirror.Points("", ""));
    }

    [Fact]
    public void 탭_보고의_화면_줄과_확장_설정은_모양만_보고_받는다()
    {
        var home = Path.Combine(_root, "홈");
        var pid = Environment.ProcessId;
        Live(home);
        object Row(object text) => new { group = "기본", label = "계약명", text, source = "ctrtNm" };
        JsonElement Of(object tab, object? settings = null) =>
            JsonSerializer.SerializeToElement(new { front = 1, frontAt = 1, tabs = new[] { tab }, settings });

        Assert.Null(ExtensionRelay.WriteTabs(home, pid, Of(new { tabId = 1, state = "supported", screenRows = new[] { Row("시험") } },
            new { panelMode = "button", shortcut = "Alt+Shift+S", siteAccess = true }), DateTime.Now));
        var read = Assert.Single(ExtensionRelay.ReadTabs(home)).Report;
        Assert.Equal("button", read.GetProperty("settings").GetProperty("panelMode").GetString());
        Assert.Equal("시험", read.GetProperty("tabs")[0].GetProperty("screenRows")[0].GetProperty("text").GetString());

        Assert.NotNull(ExtensionRelay.WriteTabs(home, pid, Of(new { tabId = 1, state = "supported", screenRows = new[] { Row(7) } }), DateTime.Now));
        Assert.NotNull(ExtensionRelay.WriteTabs(home, pid, Of(new { tabId = 1, state = "supported", screenRows = "줄" }), DateTime.Now));
        Assert.NotNull(ExtensionRelay.WriteTabs(home, pid, Of(new { tabId = 1, state = "supported",
            screenRows = Enumerable.Repeat(Row("x"), ExtensionRelay.MaxScreenRows + 1).ToArray() }), DateTime.Now));
        Assert.NotNull(ExtensionRelay.WriteTabs(home, pid, Of(new { tabId = 1, state = "reading" },
            new { panelMode = "sometimes", shortcut = "", siteAccess = true }), DateTime.Now));
        Assert.NotNull(ExtensionRelay.WriteTabs(home, pid, Of(new { tabId = 1, state = "reading" },
            new { panelMode = "always", shortcut = "<script>", siteAccess = true }), DateTime.Now));
        // 메뉴 번호는 빈 글이거나 숫자 다섯만.
        Assert.Null(ExtensionRelay.WriteTabs(home, pid, Of(new { tabId = 1, state = "unsupported", menu = "01175" }), DateTime.Now));
        Assert.Equal("01175", Assert.Single(ExtensionRelay.ReadTabs(home)).Report.GetProperty("tabs")[0].GetProperty("menu").GetString());
        Assert.Null(ExtensionRelay.WriteTabs(home, pid, Of(new { tabId = 1, state = "unsupported", menu = "" }), DateTime.Now));
        Assert.NotNull(ExtensionRelay.WriteTabs(home, pid, Of(new { tabId = 1, state = "unsupported", menu = "<b>메뉴</b>" }), DateTime.Now));
        Assert.NotNull(ExtensionRelay.WriteTabs(home, pid, Of(new { tabId = 1, state = "unsupported", menu = 1175 }), DateTime.Now));
        // 설정 없는 보고(옛 확장)도 받는다.
        Assert.Null(ExtensionRelay.WriteTabs(home, pid, Of(new { tabId = 1, state = "reading" }), DateTime.Now));
        Assert.Equal(JsonValueKind.Null, Assert.Single(ExtensionRelay.ReadTabs(home)).Report.GetProperty("settings").ValueKind);
    }

    [Fact]
    public void 설정_명령은_탭_없이_가고_모양이_틀리면_놓지_않는다()
    {
        var home = Path.Combine(_root, "홈");
        var pid = Environment.ProcessId;
        Live(home);
        Assert.Throws<InvalidOperationException>(() => ExtensionRelay.SendSetting(home, pid, "setPanelMode", "sometimes"));
        Assert.Throws<InvalidOperationException>(() => ExtensionRelay.SendSetting(home, pid, "focusTab"));
        Assert.Throws<InvalidOperationException>(() => ExtensionRelay.Send(home, pid, "openShortcuts", 3));
        Assert.True(ExtensionRelay.SendSetting(home, pid, "setPanelMode", "button"));
        Assert.True(ExtensionRelay.SendSetting(home, pid, "openShortcuts"));
        Assert.True(ExtensionRelay.SendSetting(home, pid, "openExtensions"));
        File.WriteAllText(Path.Combine(ExtensionRelay.CommandDirectory(home, pid), "9999999999999999998-zz.json"), """{"command":"setPanelMode","mode":"never"}""");

        var delivered = new List<JsonObject>();
        using var watcher = new ExtensionRelay.Watcher(home, pid, delivered.Add, TimeSpan.FromHours(1));
        watcher.Drain();
        Assert.Equal(new[] { """{"protocolVersion":2,"command":"setPanelMode","mode":"button"}""", """{"protocolVersion":2,"command":"openShortcuts"}""",
                """{"protocolVersion":2,"command":"openExtensions"}""" },
            delivered.Select(c => c.ToJsonString()));
        Assert.Empty(Directory.GetFiles(ExtensionRelay.CommandDirectory(home, pid)));
    }
}

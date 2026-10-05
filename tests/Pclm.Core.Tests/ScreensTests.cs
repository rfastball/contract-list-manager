using System.Text.Json;
using Microsoft.Data.Sqlite;
using Pclm.Core.Erp;
using Pclm.Core.Storage;
using Xunit;

namespace Pclm.Core.Tests;

/// <summary>
/// 수집하는 화면은 메뉴 번호의 허용 목록으로만 가린다(ADR-037) — 화면 하나에 프로필 하나, 번호를 모르면 받지 않는다.
/// 전에는 번호 칸이 읽히면 어느 화면이든 받았고, 그다음에는 번호를 모르면 예전처럼 번호 칸으로 가렸다. 목록은 매핑의
/// 프로필이 단 <c>screen</c> 이라, 그 화면의 고유키와 한 자리에 있다.
/// 값은 모두 지어낸 것이다.
/// </summary>
public sealed class ScreensTests : IDisposable
{
    private readonly string _root = Path.Combine(Path.GetTempPath(), "pclm-screens-" + Guid.NewGuid().ToString("N"));
    private string DbPath => Path.Combine(_root, "pclm.db");
    private readonly ErpCapture _host;
    private readonly string _dataset;

    public ScreensTests()
    {
        var database = PclmFile.Create(DbPath, PclmRole.Work);
        using var connection = database.Open();
        _dataset = Dapper.SqlMapper.QuerySingle<string>(connection, "SELECT dataset_id FROM pclm_file;");
        _host = new(DbPath, environment: "development");
    }

    public void Dispose()
    {
        SqliteConnection.ClearAllPools();
        Directory.Delete(_root, recursive: true);
    }

    private static JsonElement Point(params (string Name, string Value)[] depths) =>
        JsonSerializer.SerializeToElement(depths.ToDictionary(d => d.Name, d => d.Value));

    /// <summary>접수 목록형 파일. 메뉴 경로(depth)만 갈아 끼운다.</summary>
    private static CaptureInput RequestFile(Dictionary<string, string> depths, string profile = "g2b-request-v1")
    {
        var point = new Dictionary<string, object> { ["ctrtDmndBizNm"] = "시험 구매", ["wrtYmd"] = "20260927", ["totlPymtAmt"] = "0" };
        foreach (var (k, v) in depths) point[k] = v;
        return new(profile, Mapping.Hash(Mapping.Defaults()), JsonSerializer.SerializeToElement(new
        {
            pointInfo = point,
            tables = new Dictionary<string, object> { ["mf_wfm_container_gridView"] = new[] {
                new { ctrtDmndRcptNo = "RC001", ctrtDmndRcptOrd = "000", ctrtDmndRcptItemSqno = "001", ndfsPrcmDmndNo = "REQ1", ctrtDmndQty = "1" },
            } }
        }));
    }

    private static Dictionary<string, string> RequestPage => new() { ["depth1"] = "01001", ["depth2"] = "01117", ["depth3"] = "" };

    /// <summary>공고 실제 화면. 확장이 읽은 메뉴 번호를 싣는다.</summary>
    private static CaptureInput NoticeLive(string? screen, string profile = "g2b-notice-a-v1") => new(
        profile, Mapping.Hash(Mapping.Defaults()), JsonSerializer.SerializeToElement(new
        {
            pointInfo = new { bidPbancNo = "R26BK00000001", bidPbancOrd = "001", bidPbancNm = "시험 공고" },
            tables = new { }
        }), "live", Screen: screen);

    /// <summary>계약 실제 화면.</summary>
    private static CaptureInput ContractLive(string? screen) => new(
        "g2b-contract-v1", Mapping.Hash(Mapping.Defaults()), JsonSerializer.SerializeToElement(new
        {
            pointInfo = new { ctrtNoOrd = "R26TA00000001-00", ctrtNm = "시험 계약" },
            tables = new { }
        }), "live", Screen: screen);

    private static string Reason(Action act) => Assert.Throws<InvalidOperationException>(act).Message;

    [Fact]
    public void 허용_목록은_기본_매핑에서_화면을_단_세_프로필이고_화면마다_프로필_하나다()
    {
        var defaults = Mapping.Defaults();
        Assert.Equal(new[] { ("01117", "g2b-request-v1"), ("01179", "g2b-notice-a-v1"), ("01579", "g2b-contract-v1") },
            Screens.Supported(defaults).Select(p => (p.Screen!.Code, p.Id)).OrderBy(p => p.Code));
        // 번호는 숫자 다섯이고 겹치지 않으며, 이름이 있다.
        Assert.All(Screens.Supported(defaults), p => Assert.True(Screens.IsCode(p.Screen!.Code) && p.Screen.Name.Length > 0, p.Id));
        Assert.Equal(3, Screens.Supported(defaults).Select(p => p.Screen!.Code).Distinct().Count());
        Assert.Equal("g2b-contract-v1", Screens.ProfileFor(defaults, "01579")?.Id);
        Assert.Null(Screens.ProfileFor(defaults, "01175"));
        Assert.Null(Screens.ProfileFor(defaults, null));
    }

    [Fact]
    public void 화면과_고유키는_편집본이_바꿀_수_없다()
    {
        var defaults = Mapping.Defaults();
        MappingSet Edit(Func<MappingProfile, MappingProfile> change) => defaults with
            { Profiles = defaults.Profiles.Select(p => p.Id == "g2b-notice-a-v1" ? change(p) : p).ToArray() };
        const string frozen = "화면 번호와 고유키는 바꿀 수 없습니다.";
        Assert.Equal(frozen, Reason(() => Mapping.Validate(Edit(p => p with { Screen = new("01175", "개찰 결과") }))));
        Assert.Equal(frozen, Reason(() => Mapping.Validate(Edit(p => p with { Screen = null }))));
        Assert.Equal(frozen, Reason(() => Mapping.Validate(Edit(p => p with { BaseField = "bidPbancText" }))));
        Assert.Equal(frozen, Reason(() => Mapping.Validate(Edit(p => p with { SeqField = "bidPbancText" }))));
        Assert.Equal(frozen, Reason(() => Mapping.Validate(Edit(p => p with { IdentitySource = "mf_wfm_container_gridView" }))));
        // 받지 않는 프로필에 화면을 다는 것도 목록을 넓히는 일이라 편집본으로는 하지 않는다.
        Assert.Equal(frozen, Reason(() => Mapping.Validate(defaults with { Profiles = defaults.Profiles
            .Select(p => p.Id == "g2b-notice-b-v1" ? p with { Screen = new("01188", "공고") } : p).ToArray() })));
        Mapping.Validate(Edit(p => p with { Heading = "다른 제목" }));
    }

    [Fact]
    public void 화면을_적기_전에_저장한_매핑은_기본의_화면을_얻고_고유키는_얻지_않는다()
    {
        var node = System.Text.Json.Nodes.JsonNode.Parse(JsonSerializer.Serialize(Mapping.Defaults(), Mapping.Json))!;
        foreach (var profile in node["profiles"]!.AsArray()) profile!.AsObject().Remove("screen");
        var parsed = Mapping.Parse(node.ToJsonString());
        Assert.Equal(Mapping.Hash(Mapping.Defaults()), Mapping.Hash(parsed));

        // 고유키가 기본과 다른 옛 편집본은 조용히 고쳐 쓰지 않고 거절한다.
        node["profiles"]![0]!["baseField"] = "ctrtDmndRcptNoX";
        Assert.Equal("화면 번호와 고유키는 바꿀 수 없습니다.", Reason(() => Mapping.Parse(node.ToJsonString())));
    }

    [Fact]
    public void 메뉴_경로에서_비지_않은_가장_깊은_단을_화면의_번호로_본다()
    {
        Assert.Equal("01579", Screens.Leaf(Point(("depth1", "01570"), ("depth2", "01571"), ("depth3", "01579"))));
        Assert.Equal("01117", Screens.Leaf(Point(("depth1", "01001"), ("depth2", "01117"), ("depth3", ""))));
        Assert.Equal("01001", Screens.Leaf(Point(("depth1", "01001"))));
        Assert.Null(Screens.Leaf(Point(("ctrtNoOrd", "R26TA00000001-00"))));
        Assert.Null(Screens.Leaf(Point(("depth1", "01001"), ("depth2", "메뉴"))));
        Assert.Null(Screens.Leaf(JsonSerializer.SerializeToElement("pointInfo")));
    }

    [Fact]
    public void 파일은_번호가_없거나_목록에_없거나_그_화면의_프로필이_아니면_거절한다()
    {
        Assert.Equal("이 JSON 에는 화면 번호(depth)가 없어 받지 않습니다.", Reason(() => _host.Inspect(RequestFile(new()))));
        Assert.Equal("이 JSON 에는 화면 번호(depth)가 없어 받지 않습니다.", Reason(() => _host.Inspect(RequestFile(new() { ["depth1"] = "메뉴" }))));
        Assert.Equal("메뉴 01175 화면은 수집하지 않습니다.",
            Reason(() => _host.Inspect(RequestFile(new() { ["depth1"] = "01001", ["depth2"] = "01175" }))));
        Assert.Equal("메뉴 01114 화면은 수집하지 않습니다.",
            Reason(() => _host.Inspect(RequestFile(new() { ["depth1"] = "01001", ["depth2"] = "01114" }))));
        var contract = RequestFile(new() { ["depth1"] = "01570", ["depth2"] = "01571", ["depth3"] = "01579" });
        Assert.Equal("계약 화면(메뉴 01579)의 JSON 입니다. 계약 프로필(g2b-contract-v1)로 여세요.", Reason(() => _host.Inspect(contract)));
        // 같은 종류의 다른 프로필도 받지 않는다 — 화면 하나에 프로필 하나다.
        Assert.Equal("접수 화면(메뉴 01117)의 JSON 입니다. 접수 프로필(g2b-request-v1)로 여세요.",
            Reason(() => _host.Inspect(RequestFile(RequestPage, "g2b-request-b-v1"))));

        Assert.Equal("RC001-000", _host.Inspect(RequestFile(RequestPage)).Entity);
    }

    [Fact]
    public void 실제_화면은_확장이_읽은_번호로만_가리고_저장과_창의_투영도_같은_문을_지난다()
    {
        const string unread = "화면 번호를 읽지 못했습니다. 접수·공고·계약 상세 화면에서 다시 읽어 주세요.";
        Assert.Equal(unread, Reason(() => _host.Inspect(NoticeLive(null))));
        Assert.Equal(unread, Reason(() => _host.Inspect(NoticeLive("메뉴 아님"))));
        Assert.Equal("메뉴 01175 화면은 수집하지 않습니다.", Reason(() => _host.Inspect(NoticeLive("01175"))));
        Assert.Equal("메뉴 01188 화면은 수집하지 않습니다.", Reason(() => _host.Inspect(NoticeLive("01188", "g2b-notice-b-v1"))));
        Assert.Equal("계약 화면(메뉴 01579)인데 다른 프로필로 읽혔습니다. 화면을 다시 읽어 주세요.", Reason(() => _host.Inspect(NoticeLive("01579"))));
        Assert.Contains("공고 화면(메뉴 01179)", Reason(() => _host.Inspect(NoticeLive("01179", "g2b-public-notice-header-v1"))));
        Assert.Equal("R26BK00000001-001", _host.Inspect(NoticeLive("01179")).Entity);
        Assert.StartsWith("R26TA00000001", _host.Inspect(ContractLive("01579")).Entity);

        var good = _host.Inspect(NoticeLive("01179")).BaseToken;
        foreach (var bad in new[] { NoticeLive("01175"), NoticeLive(null) })
        {
            Assert.Throws<InvalidOperationException>(() => _host.Save(bad, good, Guid.NewGuid().ToString(), new()));
            Assert.Throws<InvalidOperationException>(() => Pclm.Core.Erp.Mirror.Project(new Database(DbPath), bad));
        }
    }

    [Fact]
    public void 확장은_인사의_매핑에서_목록을_받고_스냅샷의_화면_번호는_모르는_칸이_아니다()
    {
        JsonElement Call(string method, object? args = null) =>
            JsonSerializer.SerializeToElement(_host.Dispatch(JsonSerializer.SerializeToElement(new
                { protocolVersion = 2, requestId = Guid.NewGuid().ToString(), method, @params = args })), Mapping.Json);

        // 목록을 따로 싣지 않는다 — 매핑의 프로필이 단 화면이 그것이다.
        var hello = Call("hello").GetProperty("result");
        Assert.False(hello.TryGetProperty("screens", out _));
        Assert.Equal(new[] { ("01117", "g2b-request-v1"), ("01179", "g2b-notice-a-v1"), ("01579", "g2b-contract-v1") },
            hello.GetProperty("mapping").GetProperty("profiles").EnumerateArray()
                .Where(p => p.TryGetProperty("screen", out var s) && s.ValueKind == JsonValueKind.Object)
                .Select(p => (p.GetProperty("screen").GetProperty("code").GetString()!, p.GetProperty("id").GetString()!)).OrderBy(p => p.Item1));

        var refused = Call("inspect", new { datasetId = _dataset, snapshot = NoticeLive("01175") });
        Assert.False(refused.GetProperty("ok").GetBoolean());
        Assert.Equal("메뉴 01175 화면은 수집하지 않습니다.", refused.GetProperty("error").GetProperty("message").GetString());
        Assert.True(Call("inspect", new { datasetId = _dataset, snapshot = NoticeLive("01179") }).GetProperty("ok").GetBoolean());
    }

    [Fact]
    public void 수집_이력은_화면_번호를_적어_두고_재검토는_그_번호로_가린다()
    {
        // 저장은 걸러 담은 자료만 남겨 메뉴 경로가 지워진다 — 그래서 읽은 화면의 번호를 이력에 함께 적는다.
        var live = NoticeLive("01179");
        _host.Save(live, _host.Inspect(live).BaseToken, Guid.NewGuid().ToString(), new());
        var file = RequestFile(RequestPage);
        _host.Save(file, _host.Inspect(file).BaseToken, Guid.NewGuid().ToString(), new());
        List<(string Profile, JsonElement Snapshot)> stored;
        using (var c = new Database(DbPath).Open())
            stored = Dapper.SqlMapper.Query<(string Profile, string Json)>(c, "SELECT profile, snapshot_json FROM erp_capture ORDER BY profile")
                .Select(r => (r.Profile, JsonDocument.Parse(r.Json).RootElement.Clone())).ToList();
        Assert.Equal(new[] { ("g2b-notice-a-v1", "01179"), ("g2b-request-v1", "01117") },
            stored.Select(s => (s.Profile, s.Snapshot.GetProperty("screen").GetString()!)));

        // 창의 고급 도구는 이력의 자료를 파일로, 이력의 번호를 함께 실어 다시 본다.
        foreach (var (profile, snapshot) in stored)
        {
            var data = snapshot.GetProperty("data");
            Assert.Null(Screens.Leaf(data.GetProperty("pointInfo")));
            var again = new CaptureInput(profile, Mapping.Hash(Mapping.Defaults()), data, "file", Screen: snapshot.GetProperty("screen").GetString());
            Assert.Empty(_host.Inspect(again).Changes);
            // 번호가 없으면(번호를 적기 전의 이력) 받지 않는다. 창은 그런 이력을 부르기 전에 막는다(Nara.test.tsx).
            Assert.Equal("이 JSON 에는 화면 번호(depth)가 없어 받지 않습니다.", Reason(() => _host.Inspect(again with { Screen = null })));
        }
        // 실린 번호가 메뉴 경로보다 앞선다.
        Assert.Contains("계약 프로필", Reason(() => _host.Inspect(RequestFile(RequestPage) with { Screen = "01579" })));
    }

    [Fact]
    public void 화면_번호가_없는_입력은_예전과_같은_해시를_낸다()
    {
        // 재전송 영수증(request_hash)과 baseToken 이 입력의 해시를 쓴다 — 번호 칸이 null 로 실리면 판을 올린 뒤
        // 결과를 확인하지 못한 저장의 재전송이 「같은 수집 ID 에 다른 내용」 으로 거절된다.
        Assert.DoesNotContain("\"screen\"", JsonSerializer.Serialize(RequestFile(RequestPage), Mapping.Json));
        Assert.Contains("\"screen\": \"01179\"", JsonSerializer.Serialize(NoticeLive("01179"), Mapping.Json));
    }
}

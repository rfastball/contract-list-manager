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

    /// <summary>
    /// 변경계약서 작성 실제 화면(메뉴 01608). 번호·차수는 머리에 없고 품목 표에만 있다. 업체는 바뀌기 전 표와 지금 표 둘로
    /// 온다. 품목 표를 내려받기용으로 한 벌 더 싣는다.
    /// </summary>
    private static CaptureInput ChangeLive(string profile = "g2b-contract-change-v1")
    {
        var items = new[] { new { ctrtNo = "R26TA00000005", ctrtChgOrd = "01", ctrtItemSqno = 1, ctrtItemNm = "시험 품목", ctrtQty = 2, ctrtUprc = 1000, ctrtAmt = 2000 } };
        return new(profile, Mapping.Hash(Mapping.Defaults()), JsonSerializer.SerializeToElement(new
        {
            pointInfo = new { ctrtNo = "R26TA00000005", ctrtNm = "시험 변경계약", ctrtMthdNm = "수의계약" },
            tables = new Dictionary<string, object>
            {
                ["mf_wfm_container_grdCtrtLis"] = items,
                ["mf_wfm_container_grdCtrtLisExcel"] = items,
                ["mf_wfm_container_grdBeoeEtpsLst"] = new[] { new { ctrtNo = "R26TA00000005", ctrtChgOrd = "00", ctentUntyGrpNo = "OLD0001", ctentUntyGrpNm = "바뀌기 전 업체" } },
                ["mf_wfm_container_grdCrntEtpsLst"] = new[] { new { ctrtNo = "R26TA00000005", ctrtChgOrd = "01", ctentUntyGrpNo = "NEW0001", ctentUntyGrpNm = "시험 업체" } },
            },
        }), "live", Screen: "01608");
    }

    private static string Reason(Action act) => Assert.Throws<InvalidOperationException>(act).Message;

    [Fact]
    public void 허용_목록은_기본_매핑에서_화면을_단_일곱_프로필이고_화면마다_프로필_하나다()
    {
        var defaults = Mapping.Defaults();
        Assert.Equal(new[] { ("01112", "g2b-request-b-v1"), ("01117", "g2b-request-v1"), ("01179", "g2b-notice-a-v1"), ("01181", "g2b-notice-a-service-v1"), ("01196", "g2b-notice-b-v1"), ("01579", "g2b-contract-v1"), ("01608", "g2b-contract-change-v1") },
            Screens.Supported(defaults).Select(p => (p.Screen!.Code, p.Id)).OrderBy(p => p.Code));
        // 번호는 숫자 다섯이고 겹치지 않으며, 이름이 있다.
        Assert.All(Screens.Supported(defaults), p => Assert.True(Screens.IsCode(p.Screen!.Code) && p.Screen.Name.Length > 0, p.Id));
        Assert.Equal(7, Screens.Supported(defaults).Select(p => p.Screen!.Code).Distinct().Count());
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
            .Select(p => p.Id == "g2b-public-notice-header-v1" ? p with { Screen = new("01188", "공고") } : p).ToArray() })));
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
    public void 프로필이_더해지기_전에_저장한_매핑은_빠진_프로필을_기본에서_얻고_고친_프로필은_그대로다()
    {
        // 프로필 수가 기본과 다르면 검증이 거절한다 — 채우지 않으면 작업자료에 저장·활성화한 편집본을 읽는 수집이 통째로 멈춘다.
        const string added = "g2b-contract-change-v1";
        var node = System.Text.Json.Nodes.JsonNode.Parse(JsonSerializer.Serialize(Mapping.Defaults(), Mapping.Json))!;
        var profiles = node["profiles"]!.AsArray();
        profiles.Remove(profiles.Single(p => (string)p!["id"]! == added));
        Assert.Equal(Mapping.Hash(Mapping.Defaults()), Mapping.Hash(Mapping.Parse(node.ToJsonString())));

        // 고친 다른 프로필은 고친 그대로 남고, 빠진 것만 기본의 것으로 선다.
        profiles.Single(p => (string)p!["id"]! == "g2b-notice-a-v1")!["heading"] = "다른 제목";
        using (var c = new Database(DbPath).Open())
            Dapper.SqlMapper.Execute(c, "INSERT INTO erp_mapping(revision,json,active,validated,created_at) VALUES('old',@json,1,1,'2026-01-01T00:00:00Z')",
                new { json = node.ToJsonString() });
        var active = new MappingStore(new Database(DbPath)).Active();
        Assert.Equal("다른 제목", active.Profiles.Single(p => p.Id == "g2b-notice-a-v1").Heading);
        Assert.Equal(Mapping.Hash(Mapping.Defaults().Profiles.Single(p => p.Id == added)), Mapping.Hash(active.Profiles.Single(p => p.Id == added)));
        Assert.Equal("01608", Screens.ProfileFor(active, "01608")?.Screen!.Code);

        // 모르는 프로필은 채우지 않는다 — 거절은 그대로다.
        var unknown = profiles[0]!.DeepClone(); unknown["id"] = "g2b-unknown-v1"; profiles.Add(unknown);
        Assert.Equal("지원하는 매핑 판·프로필 구성을 확인하세요.", Reason(() => Mapping.Parse(node.ToJsonString())));
    }

    [Fact]
    public void 기본에서_거둔_필드가_남은_편집본은_읽을_때_그_필드만_떨구고_새로_저장하는_길은_거절한다()
    {
        // 뜻이 확정되지 않아 기본 매핑에서 뺀 필드(ADR-043)가 거두기 전에 활성화한 편집본에 남아 있다 — 머리 필드·표 필드·그것만 걸던 선택자.
        const string contract = "g2b-contract-v1";
        var node = System.Text.Json.Nodes.JsonNode.Parse(JsonSerializer.Serialize(Mapping.Defaults(), Mapping.Json))!;
        var profile = node["profiles"]!.AsArray().Single(p => (string)p!["id"]! == contract)!;
        profile["fields"]!.AsArray().Add(System.Text.Json.Nodes.JsonNode.Parse("""{"source":"ctrtDvofNm","target":"contract_officer","transform":"text"}"""));
        profile["tables"]!.AsArray().Single(t => (string)t!["target"]! == "contract_item")!["fields"]!.AsArray()
            .Add(System.Text.Json.Nodes.JsonNode.Parse("""{"source":"spplRgnNm","target":"region","transform":"text"}"""));
        profile["selectors"] = System.Text.Json.Nodes.JsonNode.Parse("""{"ctrtDvofNm":"#officer"}""");
        node["profiles"]!.AsArray().Single(p => (string)p!["id"]! == "g2b-notice-a-v1")!["heading"] = "다른 제목";
        using (var c = new Database(DbPath).Open())
            Dapper.SqlMapper.Execute(c, "INSERT INTO erp_mapping(revision,json,active,validated,created_at) VALUES('old',@json,1,1,'2026-01-01T00:00:00Z')",
                new { json = node.ToJsonString() });

        var active = new MappingStore(new Database(DbPath)).Active();
        Assert.Equal(Mapping.Hash(Mapping.Defaults().Profiles.Single(p => p.Id == contract)), Mapping.Hash(active.Profiles.Single(p => p.Id == contract)));
        Assert.Equal("다른 제목", active.Profiles.Single(p => p.Id == "g2b-notice-a-v1").Heading);

        // 사람이 지금 저장하는 편집본은 말없이 고쳐 쓰지 않는다 — 기본에 없는 대상은 거절이다.
        Assert.Equal("허용되지 않은 대상·변환입니다: contract_officer", Reason(() => Mapping.Parse(node.ToJsonString())));
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
    public void 통합_공고_상세는_공고_B_로_읽고_비어_온_기초금액_표는_걷지_않은_값이다()
    {
        // 메뉴 01196 — 번호와 차수가 한 칸(「본번호 - 차수」)에 온다. 수의시담은 기초금액 표가 비어 온다.
        CaptureInput Live(params object[] baseRows) => new("g2b-notice-b-v1", Mapping.Hash(Mapping.Defaults()), JsonSerializer.SerializeToElement(new
        {
            pointInfo = new { bidPbancText = "R26BK00000002 - 000", bidPbancNm = "시험 공고" },
            tables = new Dictionary<string, object>
            {
                ["mf_wfm_container_mainWframe_gridView5"] = baseRows,
                ["mf_wfm_container_mainWframe_gridView3"] = new[] { new { bidClsfNo = "1", bidPbancItemSqno = 1, dtlsPrnmNm = "시험 품목", prchsDtlItemQty = 1, prchsDtlItemUprc = 1000 } },
            },
        }), "live", Screen: "01196");
        var preview = _host.Inspect(Live());
        Assert.Equal("R26BK00000002-000", preview.Entity);
        Assert.Equal(1, preview.ItemCount);
        Assert.False(preview.Document.Rows[0].Values.ContainsKey("base_price"));
        Assert.Equal("1000", _host.Inspect(Live(new { baseAmt = 1000 })).Document.Rows[0].Values["base_price"]);
        // 분류가 여럿이면 어느 것이 문서 전체의 값인지 모르므로 여전히 막는다.
        Assert.Equal("문서 전체 금액은 단일 분류 표에서만 가져올 수 있습니다.", Reason(() => _host.Inspect(Live(new { baseAmt = 1 }, new { baseAmt = 2 }))));
        Assert.Equal("공고 화면(메뉴 01196)인데 다른 프로필로 읽혔습니다. 화면을 다시 읽어 주세요.", Reason(() => _host.Inspect(NoticeLive("01196"))));
    }

    [Fact]
    public void 변경계약서_작성_화면은_품목_표의_번호로_그_차수의_계약이고_업체는_바뀐_뒤의_표만_읽는다()
    {
        // 메뉴 01608 — 서명 전에 쓰는 화면이지만 읽은 값은 그 차수의 계약으로 담는다. 계약일은 이 화면에 없어 걷지 않는다.
        var preview = _host.Inspect(ChangeLive());
        Assert.Equal("R26TA00000005-01", preview.Entity);
        Assert.Equal(1, preview.ItemCount);
        var head = preview.Document.Rows[0].Values;
        Assert.Equal("2000", head["amount"]); // 계약 상세와 같이 품목에서 셈한다.
        Assert.Equal("수의계약", head["contract_method"]);
        Assert.False(head.ContainsKey("contracted_on"));
        // 바뀌기 전 업체(앞 차수의 것)는 이 차수에 담지 않는다.
        Assert.Equal(new[] { "시험 업체" }, preview.Document.Rows.Where(r => r.Table == "erp_partner").Select(r => r.Values["name"]));
        Assert.Equal("계약 화면(메뉴 01608)인데 다른 프로필로 읽혔습니다. 화면을 다시 읽어 주세요.", Reason(() => _host.Inspect(ChangeLive("g2b-contract-v1"))));

        // 서명 뒤 계약 상세(01579)에서 같은 차수를 다시 읽는 것은 여느 두 번째 수집이다.
        var change = ChangeLive();
        _host.Save(change, _host.Inspect(change).BaseToken, Guid.NewGuid().ToString(), new());
        var signed = new CaptureInput("g2b-contract-v1", Mapping.Hash(Mapping.Defaults()), JsonSerializer.SerializeToElement(new
        {
            pointInfo = new { ctrtNoOrd = "R26TA00000005-01", ctrtNm = "시험 변경계약", ctrtDt = "2026/10/05" },
            tables = new Dictionary<string, object> { ["mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis"] = new[] {
                new { ctrtNo = "R26TA00000005", ctrtChgOrd = "01", ctrtItemSqno = 1, ctrtItemNm = "시험 품목", ctrtQty = 2, ctrtUprc = 1000, ctrtAmt = 2000 } } },
        }), "live", Screen: "01579");
        var again = _host.Inspect(signed);
        Assert.Equal("R26TA00000005-01", again.Entity);
        Assert.Empty(again.Unmatched);
    }

    [Fact]
    public void 일반용역_접수_상세는_접수_B_로_읽고_다른_프로필로_열면_맞는_프로필을_알려_준다()
    {
        // 메뉴 01112 — 번호·차수가 머리에 있고 품목은 gvDmndItem 에 온다. 그 화면이 오기 전에는 접수 B 가 받는 화면이 없었다.
        CaptureInput File(string profile) => new(profile, Mapping.Hash(Mapping.Defaults()), JsonSerializer.SerializeToElement(new
        {
            pointInfo = new
            {
                depth1 = "01001", depth2 = "01112", ctrtDmndRcptNo = "RC012", ctrtDmndRcptOrd = "000",
                ctrtDmndBizNm = "시험 용역", wrtYmd = "20261001", scsbdMthdCd = "낙030005",
            },
            tables = new Dictionary<string, object> { ["mf_wfm_container_gvDmndItem"] = new[] {
                new { ctrtDmndRcptNo = "RC012", ctrtDmndRcptOrd = "000", ctrtDmndRcptItemSqno = 1, dtlsPrnm = "시험 용역", ctrtDmndQty = 1, ctrtDmndUprc = 1000, devyCndtCd = "인010045" },
            } },
        }));
        var preview = _host.Inspect(File("g2b-request-b-v1"));
        Assert.Equal("RC012-000", preview.Entity);
        Assert.Equal(1, preview.ItemCount);
        Assert.Equal("협상에의한계약", preview.Document.Rows[0].Values["award_method"]);
        Assert.Equal("기타사항참조", Assert.Single(preview.Document.Rows, r => r.Table == "request_item").Values["delivery_terms"]);
        Assert.Equal("접수 화면(메뉴 01112)의 JSON 입니다. 접수 프로필(g2b-request-b-v1)로 여세요.", Reason(() => _host.Inspect(File("g2b-request-v1"))));
    }

    [Fact]
    public void 일반용역_공고_상세는_공고_A_의_칸으로_읽고_미리보기가_한_벌_더_실은_첨부는_하나로_본다()
    {
        // 메뉴 01181 — 물품 공고 상세(01179)와 같은 칸이다. 「미리보기」 창이 같은 첨부 표를 한 벌 더 싣고 빈 첨부 표가 하나 더 온다.
        var files = new[] { new { atchFileSqno = 1, atchFileDscr = "공고서", orgnlAtchFileNm = "시험 공고서.hwp" } };
        CaptureInput File(string profile) => new(profile, Mapping.Hash(Mapping.Defaults()), JsonSerializer.SerializeToElement(new
        {
            pointInfo = new
            {
                depth1 = "01170", depth2 = "01181", bidPbancNo = "R26BK00000011", bidPbancOrd = "001", bidPbancNm = "시험 용역 공고",
                pbancKndCd = "공440002", pbancSttsCd = "공400002", scsbdMthdCd = "낙030005", dmstcOvrsSeCd = "국010001", pnprDcsnMthoCd = "예050001", pnprDcsnMthoNm = "비예가",
                slprRcptBgngDay = "20261001", slprRcptBgngTime = "1000", onbsPrnmntDay = "20261008", onbsPrnmntTime = "1100",
            },
            tables = new Dictionary<string, object>
            {
                ["mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List"] = new[] {
                    new { bidClsfNo = "0", bidPbancItemSqno = "1", dtlsPrnmNm = "시험 용역", devyCndtCd = "인010040" } },
                ["mf_wfm_container_tabEmpty_grdFile"] = Array.Empty<object>(),
                ["mf_wfm_container_grdFile"] = files,
                ["mf_wfm_container_preview_grdFile"] = files,
            },
        }));
        var preview = _host.Inspect(File("g2b-notice-a-service-v1"));
        Assert.Equal("R26BK00000011-001", preview.Entity);
        Assert.Equal(1, preview.ItemCount);
        var rows = preview.Document.Rows;
        var head = rows[0].Values;
        // 종류와 상태는 따로 담고, 공고 A 의 후처리로 일정 행을 세운다.
        Assert.Equal(("실공고", "변경공고"), (head["notice_kind"], head["source_status"]));
        Assert.Equal(new[] { "입찰서제출", "개찰" }, rows.Where(r => r.Table == "notice_schedule").Select(r => r.Values["name"]));
        Assert.Equal(("협상에의한계약", "국내입찰", "비예가"), (head["award_method"], head["international_kind"], head["price_estimation_method"]));
        Assert.Equal("과업내역에 따름", Assert.Single(rows, r => r.Table == "notice_item").Values["delivery_terms"]);
        Assert.Single(rows, r => r.Table == "erp_notice_attachment");
        Assert.Equal("공고 화면(메뉴 01181)의 JSON 입니다. 공고 프로필(g2b-notice-a-service-v1)로 여세요.", Reason(() => _host.Inspect(File("g2b-notice-a-v1"))));
    }

    [Theory]
    [InlineData("g2b-contract-v1")]
    [InlineData("g2b-contract-change-v1")]
    public void 계약특성과_계약보증서_제출방법은_사람이_화면에서_확인한_명칭으로_담는다(string profile)
    {
        // 원문에 명칭 칸이 없어 「명칭 미확정」 으로 담기던 코드 — 명칭은 사람이 계약 상세 화면에서 읽어 준 그대로다(2026-10-15).
        var map = Mapping.Defaults().Profiles.Single(p => p.Id == profile).Fields.ToDictionary(f => f.Source);
        Assert.Equal("[국가계약법]용역(물품과 용역을 일괄입찰한 소프트웨어 사업제외)(0.125)", Mapping.ConvertValue("0013", map["ctrtChrsSeCd"]));
        Assert.Equal("지급각서대체", Mapping.ConvertValue("계260001", map["ctwtSbmsnMthdCd"]));
        Assert.Equal("전자접수 및 직접수납", Mapping.ConvertValue("계260002", map["ctwtSbmsnMthdCd"]));
        Assert.Equal("[국가계약법]물품의 제조 · 구매(0.075)", Mapping.ConvertValue("0009", map["ctrtChrsSeCd"]));
        // 사람이 건넨 목록에 없는 번호는 짐작하지 않는다.
        Assert.Equal("0005 (명칭 미확정)", Mapping.ConvertValue("0005", map["ctrtChrsSeCd"]));
    }

    [Fact]
    public void 계약특성_코드_사전은_계약_프로필_모두가_같은_것을_쓴다()
    {
        var dictionaries = Mapping.Defaults().Profiles.SelectMany(p => p.Fields).Where(f => f.Source == "ctrtChrsSeCd")
            .Select(f => JsonSerializer.Serialize(f.Codes!.OrderBy(c => c.Key))).ToArray();
        Assert.Equal(2, dictionaries.Length);
        Assert.Single(dictionaries.Distinct());
    }

    [Fact]
    public void 일반용역_화면이_오기_전에_저장한_매핑은_새_공고_프로필과_접수_B_의_화면을_기본에서_얻는다()
    {
        const string added = "g2b-notice-a-service-v1";
        var node = System.Text.Json.Nodes.JsonNode.Parse(JsonSerializer.Serialize(Mapping.Defaults(), Mapping.Json))!;
        var profiles = node["profiles"]!.AsArray();
        profiles.Remove(profiles.Single(p => (string)p!["id"]! == added));
        profiles.Single(p => (string)p!["id"]! == "g2b-request-b-v1")!.AsObject().Remove("screen");
        var parsed = Mapping.Parse(node.ToJsonString());
        Assert.Equal(Mapping.Hash(Mapping.Defaults().Profiles.Single(p => p.Id == added)), Mapping.Hash(parsed.Profiles.Single(p => p.Id == added)));
        Assert.Equal(added, Screens.ProfileFor(parsed, "01181")?.Id);
        Assert.Equal("g2b-request-b-v1", Screens.ProfileFor(parsed, "01112")?.Id);
        Assert.Equal(Mapping.Hash(Mapping.Defaults()), Mapping.Hash(parsed));
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
        Assert.Equal(new[] { ("01112", "g2b-request-b-v1"), ("01117", "g2b-request-v1"), ("01179", "g2b-notice-a-v1"), ("01181", "g2b-notice-a-service-v1"), ("01196", "g2b-notice-b-v1"), ("01579", "g2b-contract-v1"), ("01608", "g2b-contract-change-v1") },
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

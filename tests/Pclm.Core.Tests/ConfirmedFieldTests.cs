using System.Text.Json;
using System.Text.Json.Nodes;
using Dapper;
using Microsoft.Data.Sqlite;
using Pclm.Core.Erp;
using Pclm.Core.Storage;
using Xunit;

namespace Pclm.Core.Tests;

/// <summary>
/// 확정되고도 담지 않던 필드(ADR-043, 스키마 28)가 수집에서 면 뷰의 새 열까지 닿는가. 뜻을 확인하고도 「쓰지 않는다」 는 이유로
/// 매핑에 들지 못해, 나중에 쓰게 되면 판을 올리고 이미 수집한 건을 다시 수집해야 했다. 새 열은 끝에 덧붙고 표에서는 기본으로
/// 숨는다(<see cref="Views.HiddenByDefault"/>). 값은 모두 지어낸 것이다.
/// </summary>
public sealed class ConfirmedFieldTests : IDisposable
{
    private readonly string _root = Path.Combine(Path.GetTempPath(), "pclm-confirmed-" + Guid.NewGuid().ToString("N"));
    private string DbPath => Path.Combine(_root, "pclm.db");
    private readonly Database _database;
    private readonly ErpCapture _host;

    public ConfirmedFieldTests()
    {
        _database = PclmFile.Create(DbPath, PclmRole.Work);
        _host = new(DbPath, environment: "development");
    }

    public void Dispose()
    {
        SqliteConnection.ClearAllPools();
        Directory.Delete(_root, recursive: true);
    }

    private void Capture(string profile, string screen, object data)
    {
        var input = new CaptureInput(profile, Mapping.Hash(new MappingStore(_database).Active()),
            JsonSerializer.SerializeToElement(data), "live", Screen: screen);
        var preview = _host.Inspect(input);
        _host.Save(input, preview.BaseToken, Guid.NewGuid().ToString(), new());
    }

    private IDictionary<string, object> Row(string sql, object? args = null)
    {
        using var c = _database.OpenReadOnly();
        return (IDictionary<string, object>)c.QuerySingle(sql, args);
    }

    private void Values(IDictionary<string, object> row, params (string Column, string Value)[] expected)
    {
        foreach (var (column, value) in expected) Assert.True(value == (string)row[column], $"{column}: {row[column]}");
    }

    [Fact]
    public void 접수의_공고종류_추정가격과_품목의_물품분류번호_납품부대명이_담긴다()
    {
        Capture("g2b-request-v1", "01117", new
        {
            pointInfo = new { ctrtDmndBizNm = "시험 구매", wrtYmd = "20261001", pbancKndCd = "공440002", prspPrce = "1234500" },
            tables = new Dictionary<string, object> { ["mf_wfm_container_gridView"] = new[] {
                new { ctrtDmndRcptNo = "RC021", ctrtDmndRcptOrd = "000", ctrtDmndRcptItemSqno = "001", itemClsfNo = "00000001", ndfsPrcmItemDlvgdsMtruNm = "시험부대" },
            } },
        });
        Values(Row("SELECT * FROM v_접수 WHERE 접수번호 = 'RC021-000'"), ("공고종류", "실공고"), ("추정가격", "1,234,500"));
        Values(Row("SELECT * FROM v_접수품목 WHERE 접수번호 = 'RC021-000'"), ("물품분류번호", "00000001"), ("납품부대명", "시험부대"));
    }

    [Fact]
    public void 물품_공고의_입찰방식_계약법_제한업종과_품목의_새_칸이_담긴다()
    {
        Capture("g2b-notice-a-v1", "01179", new
        {
            pointInfo = new
            {
                bidPbancNo = "R26BK00000021", bidPbancOrd = "000", bidPbancNm = "시험 공고",
                bidMthdCd = "입180002", bizAmtFeeSumAmt = "1100000", lcrtTyCd = "계040002",
            },
            tables = new Dictionary<string, object>
            {
                ["mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List"] = new[] {
                    new { bidClsfNo = "0", bidPbancItemSqno = "1", dmstUntyGrpNo = "AA0000001", itemClsfNo = "00000001",
                          itemIdnfNm = "시험품, 시험규격", ndfsItemSpecNo = "SPEC-1", ndfsPrcmDmndNo = "REQ21" } },
                // 업종이 여럿이면 줄 차례대로 잇는다.
                ["mf_wfm_container_tabCont_contents_itemTabs2_body_wframe7_grdAliasDmTtl07LeftList"] = new[] {
                    new { bidLmtUntyNm = "시험 업종 가" }, new { bidLmtUntyNm = "시험 업종 나" } },
            },
        });
        Values(Row("SELECT * FROM v_공고 WHERE 입찰공고번호 = 'R26BK00000021-000'"),
            ("입찰방식", "전자입찰"), ("사업금액수수료합계", "1,100,000"), ("계약법구분", "지방계약법"),
            ("입찰참가제한업종", "시험 업종 가, 시험 업종 나"), ("입찰진행상태", ""));
        Values(Row("SELECT * FROM v_공고품목 WHERE 입찰공고번호 = 'R26BK00000021-000'"),
            ("수요기관코드", "AA0000001"), ("물품분류번호", "00000001"), ("물품식별명", "시험품, 시험규격"),
            ("규격번호", "SPEC-1"), ("조달요구번호", "REQ21"),
            // 물품식별명은 새 열이다 — 상세규격(규격)을 덮지 않는다.
            ("규격", ""));
    }

    /// <summary>
    /// 제한 업종 표는 줄 차례가 곧 행이다. 업종명을 키로 삼았을 때는 같은 업종이 두 제한 그룹에 오거나 이름 칸이 비면
    /// 「원천 행 키가 중복됩니다/없습니다」 로 공고 수집이 통째로 멈췄다.
    /// </summary>
    [Fact]
    public void 제한_업종은_같은_이름이_겹치거나_비어도_공고를_받고_줄마다_선다()
    {
        object Notice(params string[] names) => new
        {
            pointInfo = new { bidPbancNo = "R26BK00000023", bidPbancOrd = "000", bidPbancNm = "시험 공고" },
            tables = new Dictionary<string, object>
            {
                ["mf_wfm_container_tabCont_contents_itemTabs2_body_wframe7_grdAliasDmTtl07LeftList"] =
                    names.Select((n, i) => new { lmtGupSqno = i + 1, bidLmtUntyNm = n }).ToArray(),
            },
        };
        Capture("g2b-notice-a-v1", "01179", Notice("시험 업종 가", "시험 업종 가", ""));
        using (var c = _database.OpenReadOnly())
            Assert.Equal(new[] { "시험 업종 가", "시험 업종 가", "" },
                c.Query<string?>("SELECT name FROM notice_restricted_industry ORDER BY line_no").Select(n => n ?? ""));
        Values(Row("SELECT * FROM v_공고 WHERE 입찰공고번호 = 'R26BK00000023-000'"), ("입찰참가제한업종", "시험 업종 가, 시험 업종 가"));

        // 같은 화면을 다시 가져오면 같은 차례의 줄에 얹혀 바뀐 것이 없다.
        var again = new CaptureInput("g2b-notice-a-v1", Mapping.Hash(Mapping.Defaults()),
            JsonSerializer.SerializeToElement(Notice("시험 업종 가", "시험 업종 가", "")), "live", Screen: "01179");
        Assert.Empty(_host.Inspect(again).Changes);
    }

    [Fact]
    public void 키를_비우는_것은_기본이_그렇게_둔_표에서만_된다()
    {
        var node = JsonNode.Parse(JsonSerializer.Serialize(Mapping.Defaults(), Mapping.Json))!;
        var notice = node["profiles"]!.AsArray().Single(p => (string)p!["id"]! == "g2b-notice-a-v1")!;
        notice["tables"]!.AsArray().Single(t => (string)t!["target"]! == "notice_item")!["keys"] = new JsonArray();
        Assert.Equal("원천 행 복합키가 필요합니다.", Assert.Throws<InvalidOperationException>(() => Mapping.Parse(node.ToJsonString())).Message);
    }

    [Fact]
    public void 통합_조회_공고의_입찰방식_진행상태와_품목의_새_칸이_담긴다()
    {
        Capture("g2b-notice-b-v1", "01196", new
        {
            pointInfo = new { bidPbancText = "R26BK00000022-001", bidPbancNm = "시험 공고", bidMthdNm = "전자시담" },
            tables = new Dictionary<string, object>
            {
                ["mf_wfm_container_mainWframe_gridView3"] = new[] {
                    new { bidClsfNo = "0", bidPbancItemSqno = "1", dmstUntyGrpNo = "AA0000002", itemClsfNo = "00000002", itemIdnfNm = "시험품" } },
                ["mf_wfm_container_mainWframe_gridViewPgst"] = new[] { new { bidPgstNm = "개찰완료" } },
            },
        });
        Values(Row("SELECT * FROM v_공고 WHERE 입찰공고번호 = 'R26BK00000022-001'"), ("입찰방식", "전자시담"), ("입찰진행상태", "개찰완료"));
        Values(Row("SELECT * FROM v_공고품목 WHERE 입찰공고번호 = 'R26BK00000022-001'"),
            ("수요기관코드", "AA0000002"), ("물품분류번호", "00000002"), ("물품식별명", "시험품"));
    }

    [Fact]
    public void 계약의_머리_업체_품목의_새_칸이_담기고_통합에도_실린다()
    {
        Capture("g2b-contract-v1", "01579", new
        {
            pointInfo = new
            {
                ctrtNoOrd = "R26TA00000021-00", ctrtNm = "시험 계약", dmstPicId = "test01",
                fprdYn = "N", lcrtTyCd = "계040001", prflcAplcnCrtrCd = "물130002", rlsYn = "Y",
            },
            tables = new Dictionary<string, object>
            {
                ["mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis"] = new[] {
                    new { ctrtNo = "R26TA00000021", ctrtChgOrd = "00", ctrtItemSqno = 1, ctrtAmt = 1000,
                          bidClsfNo = "0", bidPbancItemSqno = 1, dlvgdsPlacNm = "시험 장소", dlvgdsTermNody = 90,
                          dmstUntyGrpNo = "AA0000003", ndfsPrcmDmndNo = "REQ23", odn1PlorNtnNm = "시험국" } },
                ["mf_wfm_container_grdEtpsLst"] = new[] { new { ctentUntyGrpNo = "P001", ctentUntyGrpNm = "시험 업체", jintSyddCmnMthoNm = "단독계약" } },
            },
        });
        (string, string)[] head = [("수요기관담당자ID", "test01"), ("외국산여부", "아니오"), ("계약법구분", "국가계약법"),
            ("물가변동적용기준", "품목조정율"), ("공개여부", "Y"), ("공동수급구성방식", "단독계약")];
        Values(Row("SELECT * FROM v_계약 WHERE 계약번호 = 'R26TA0000002100'"), head);
        Values(Row("SELECT * FROM v_통합 WHERE 계약번호 = 'R26TA0000002100'"), head);
        Values(Row("SELECT * FROM v_계약품목 WHERE 계약번호 = 'R26TA0000002100'"),
            ("입찰분류번호", "0"), ("입찰공고품목순번", "1"), ("납품장소", "시험 장소"), ("납품일수", "90"),
            ("수요기관코드", "AA0000003"), ("조달요구번호", "REQ23"), ("원산지", "시험국"));
    }

    /// <summary>
    /// 같은 ID 는 어느 화면에서도 같은 뜻이다(ADR-043 결정 6) — 다른 화면에서 확정한 필드가 일반용역 접수·일반용역 공고·변경계약서
    /// 화면에도 있으면 거기서도 담는다. 일반용역 공고는 코드 옆의 명칭 칸을 담는다(명칭이 코드 사전의 것과 같았다).
    /// </summary>
    [Fact]
    public void 다른_화면에서_확정한_필드는_일반용역_접수_공고와_변경계약서에서도_담긴다()
    {
        Capture("g2b-request-b-v1", "01112", new
        {
            pointInfo = new { ctrtDmndRcptNo = "RC024", ctrtDmndRcptOrd = "000", ctrtDmndBizNm = "시험 용역", pbancKndCd = "공440002", prspPrce = "3000" },
            tables = new Dictionary<string, object> { ["mf_wfm_container_gvDmndItem"] = new[] {
                new { ctrtDmndRcptNo = "RC024", ctrtDmndRcptOrd = "000", ctrtDmndRcptItemSqno = 1, itemClsfNo = "00000004" } } },
        });
        Values(Row("SELECT * FROM v_접수 WHERE 접수번호 = 'RC024-000'"), ("공고종류", "실공고"), ("추정가격", "3,000"));
        Values(Row("SELECT * FROM v_접수품목 WHERE 접수번호 = 'RC024-000'"), ("물품분류번호", "00000004"));

        Capture("g2b-notice-a-service-v1", "01181", new
        {
            pointInfo = new
            {
                bidPbancNo = "R26BK00000024", bidPbancOrd = "000", bidPbancNm = "시험 용역 공고",
                ctrtTyNm = "시험 계약구분", stdCtrtMthdNm = "시험 계약방법", fprdYnNm = "아니오", prttnDlvgdsPsbltyYnNm = "가능",
                rschDvlpItemYnNm = "아니오", scsbdMthdDtlsNm = "시험 세부기준", bidMthdNm = "전자입찰", bizAmtFeeSumAmt = "2000", lcrtTyCd = "계040001",
            },
            tables = new Dictionary<string, object>
            {
                ["mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List"] = new[] {
                    new { bidClsfNo = "0", bidPbancItemSqno = "1", dmstUntyGrpNo = "AA0000004", itemClsfNo = "00000005" } },
                ["mf_wfm_container_tabCont_contents_itemTabs2_body_wframe7_grdAliasDmTtl07LeftList"] = new[] { new { bidLmtUntyNm = "시험 업종 다" } },
                // 공고 상세 안에 뜬 통합 조회 팝업 — 진행상태는 여기에만 있다.
                ["mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridViewPgst"] = new[] { new { bidPgstNm = "시험 진행상태" } },
            },
        });
        Values(Row("SELECT * FROM v_공고 WHERE 입찰공고번호 = 'R26BK00000024-000'"),
            ("계약구분", "시험 계약구분"), ("계약방법", "시험 계약방법"), ("분할납품", "가능"), ("입찰방식", "전자입찰"),
            ("사업금액수수료합계", "2,000"), ("계약법구분", "국가계약법"), ("입찰참가제한업종", "시험 업종 다"), ("입찰진행상태", "시험 진행상태"));
        using (var c = _database.OpenReadOnly())
            Assert.Equal(("시험 세부기준", 0L, 0L), c.QuerySingle<(string, long, long)>(
                "SELECT award_criteria, foreign_allowed, research_item FROM notice WHERE notice_base = 'R26BK00000024'"));
        Values(Row("SELECT * FROM v_공고품목 WHERE 입찰공고번호 = 'R26BK00000024-000'"), ("수요기관코드", "AA0000004"), ("물품분류번호", "00000005"));

        Capture("g2b-contract-change-v1", "01608", new
        {
            pointInfo = new { ctrtNm = "시험 변경계약", ctrtTyNm = "시험 계약구분" },
            tables = new Dictionary<string, object> { ["mf_wfm_container_grdCtrtLis"] = new[] {
                new { ctrtNo = "R26TA00000024", ctrtChgOrd = "01", ctrtItemSqno = 1, ctrtAmt = 1000, bidClsfNo = "0", bidPbancItemSqno = 1,
                      dlvgdsPlacNm = "시험 장소", dlvgdsTermNody = 30, dmstUntyGrpNo = "AA0000005", ndfsPrcmDmndNo = "REQ24", odn1PlorNtnNm = "시험국" } } },
        });
        Values(Row("SELECT * FROM v_계약 WHERE 계약번호 = 'R26TA0000002401'"), ("계약구분", "시험 계약구분"));
        Values(Row("SELECT * FROM v_계약품목 WHERE 계약번호 = 'R26TA0000002401'"),
            ("입찰분류번호", "0"), ("입찰공고품목순번", "1"), ("납품장소", "시험 장소"), ("납품일수", "30"),
            ("수요기관코드", "AA0000005"), ("조달요구번호", "REQ24"), ("원산지", "시험국"));
    }

    /// <summary>
    /// 「채권자」 한 칸에 재무관(<c>fncrNm</c>, 사람 이름)과 통합 조회의 <c>grnteCrdrNm</c>(늘 기관장 직함 — 뜻 미정 후보)이 섞여
    /// 담기고 있었다(2026-10-16 사용자 결정). 재무관만 담아 「재무관」 열로 내고, 후보는 담지 않는다.
    /// </summary>
    [Fact]
    public void 재무관은_공고_A_에서만_담기고_통합_조회의_채권자_후보는_담지_않는다()
    {
        Capture("g2b-notice-a-v1", "01179", new
        {
            pointInfo = new { bidPbancNo = "R26BK00000025", bidPbancOrd = "000", bidPbancNm = "시험 공고", fncrNm = "홍길동" },
            tables = new Dictionary<string, object>(),
        });
        Capture("g2b-notice-b-v1", "01196", new
        {
            pointInfo = new { bidPbancText = "R26BK00000026-000", bidPbancNm = "시험 공고", grnteCrdrNm = "시험 기관장" },
            tables = new Dictionary<string, object>(),
        });
        Values(Row("SELECT * FROM v_공고 WHERE 입찰공고번호 = 'R26BK00000025-000'"), ("재무관", "홍길동"));
        Values(Row("SELECT * FROM v_공고 WHERE 입찰공고번호 = 'R26BK00000026-000'"), ("재무관", ""));
        Assert.DoesNotContain(Mapping.Defaults().Profiles.SelectMany(p => p.Fields), f => f.Source == "grnteCrdrNm");
    }

    /// <summary>
    /// 대상 이름을 바꾸기 전(<c>creditor</c>)에 저장·활성화한 편집본. 읽을 때 옛 대상은 떨구고 새 대상(<c>finance_officer</c>)은
    /// 기본의 것으로 채운다 — 거절하면 활성 매핑을 읽는 수집이 통째로 멈춘다.
    /// </summary>
    [Fact]
    public void 채권자_대상이_남은_편집본은_읽을_때_떨구고_재무관을_채운다()
    {
        var node = JsonNode.Parse(JsonSerializer.Serialize(Mapping.Defaults(), Mapping.Json))!;
        JsonNode Profile(string id) => node["profiles"]!.AsArray().Single(p => (string)p!["id"]! == id)!;
        Profile("g2b-notice-a-v1")["fields"]!.AsArray().Single(f => (string)f!["target"]! == "finance_officer")!["target"] = "creditor";
        Profile("g2b-notice-b-v1")["fields"]!.AsArray().Add(JsonNode.Parse("""{"source":"grnteCrdrNm","target":"creditor","transform":"text"}"""));
        var json = node.ToJsonString();

        var read = Mapping.Read(json);
        Assert.DoesNotContain(read.Profiles.SelectMany(p => p.Fields), f => f.Target == "creditor");
        Assert.Equal("fncrNm", read.Profiles.Single(p => p.Id == "g2b-notice-a-v1").Fields.Single(f => f.Target == "finance_officer").Source);
        foreach (var original in Mapping.Defaults().Profiles)
            Assert.Equal(original.Fields.Select(f => f.Target).Order(), read.Profiles.Single(p => p.Id == original.Id).Fields.Select(f => f.Target).Order());
    }

    /// <summary>
    /// v29 판올림은 「채권자」 칸의 값 가운데 <b>재무관에서 왔다고 밝힐 수 있는 것만</b> 재무관으로 옮긴다 — 수집 기록이 그 칸을
    /// 공고 A 가 채웠다고 적고 값도 같을 때다. 통합 조회가 채운 기관장 직함, 기록이 없는 옛 값, 기록과 갈린 값은 비운다.
    /// </summary>
    [Fact]
    public void 재무관_판올림은_공고_A_가_채운_값만_옮긴다()
    {
        var path = Path.Combine(_root, "v28.pclm");
        using (var c = new SqliteConnection($"Data Source={path};Pooling=False"))
        {
            c.Open();
            c.Execute(Schema.Baseline);
            foreach (var step in Schema.Steps.Take(28 - Schema.BaselineVersion)) c.Execute(step);
            c.Execute("PRAGMA user_version = 28;");
            string Doc(string profile, string value) => JsonSerializer.Serialize(new
            {
                profile, rows = new[] { new { table = "notice", sourceKey = "", values = new { creditor = value }, origins = new { creditor = profile } } },
            });
            foreach (var (b, value, profile) in new (string, string, string?)[] {
                ("TESTN0001", "홍길동", "g2b-notice-a-v1"), ("TESTN0002", "시험 기관장", "g2b-notice-b-v1"),
                ("TESTN0003", "옛 값", null), ("TESTN0004", "김철수", "g2b-notice-a-service-v1"), ("TESTN0005", "고친 값", "g2b-notice-a-v1") })
            {
                c.Execute("INSERT INTO notice_group VALUES(@b); INSERT INTO notice_series VALUES(@b,@b); " +
                          "INSERT INTO notice(notice_base,seq,creditor,updated_at) VALUES(@b,'000',@value,'2026-10-01')", new { b, value });
                if (profile is not null)
                    c.Execute("INSERT INTO erp_source VALUES('notice',@b,'000',@doc,'r','2026-10-01')",
                        new { b, doc = Doc(profile, b == "TESTN0005" ? "수집한 값" : value) });
            }
        }
        var database = new Database(path);
        database.Migrate();
        using var read = database.OpenReadOnly();
        Assert.Equal(new[] { "홍길동", "", "", "김철수", "" },
            read.Query<string>("SELECT 재무관 FROM v_공고차수 ORDER BY 공고본번호"));
    }

    /// <summary>
    /// 새 열도 사람이 고칠 수 있다 — 덮개가 그 칸을 감싸지 않으면 그 칸만 조용히 고칠 수 없게 된다(ADR-020).
    /// </summary>
    [Fact]
    public void 새_머리_열은_덮개로_고쳐진다()
    {
        var store = new Store(_database);
        Capture("g2b-request-v1", "01117", new
        {
            pointInfo = new { ctrtDmndBizNm = "시험 구매", prspPrce = "1000" },
            tables = new Dictionary<string, object> { ["mf_wfm_container_gridView"] = new[] {
                new { ctrtDmndRcptNo = "RC022", ctrtDmndRcptOrd = "000", ctrtDmndRcptItemSqno = "001" } } },
        });
        store.SetOverride(new EntityRef("request", "RC022", "000"), "추정가격", "2,000");
        Values(Row("SELECT * FROM v_접수 WHERE 접수번호 = 'RC022-000'"), ("추정가격", "2,000"));
    }

    /// <summary>
    /// 새 필드를 기본 매핑에 더하기 전에 저장·활성화한 편집본. 읽을 때 기본에 새로 더해진 대상을 채운다 — 채우지 않으면 그
    /// 작업자료만 새 필드를 영영 수집하지 않는데 편집본은 검증을 통과하므로 아무것도 실패하지 않는다. 편집본이 고친 것은 남는다.
    /// </summary>
    [Fact]
    public void 새_필드를_더하기_전의_편집본은_읽을_때_새_대상을_채우고_고친_것은_남긴다()
    {
        var node = JsonNode.Parse(JsonSerializer.Serialize(Mapping.Defaults(), Mapping.Json))!;
        JsonNode Profile(string id) => node["profiles"]!.AsArray().Single(p => (string)p!["id"]! == id)!;
        // 머리 필드 하나·품목 필드 하나·표 하나가 아직 없던 편집본. 다른 칸의 출처는 고쳐 두었다.
        var request = Profile("g2b-request-v1");
        request["fields"]!.AsArray().Remove(request["fields"]!.AsArray().Single(f => (string)f!["target"]! == "estimated_price"));
        var items = request["tables"]![0]!["fields"]!.AsArray();
        items.Remove(items.Single(f => (string)f!["target"]! == "classification_number"));
        request["fields"]!.AsArray().Single(f => (string)f!["target"]! == "title")!["source"] = "editedTitle";
        var notice = Profile("g2b-notice-a-v1");
        notice["tables"]!.AsArray().Remove(notice["tables"]!.AsArray().Single(t => (string)t!["target"]! == "notice_restricted_industry"));
        using (var c = _database.Open())
            c.Execute("INSERT INTO erp_mapping(revision,json,active,validated,created_at) VALUES('old',@json,1,1,'2026-01-01T00:00:00Z')",
                new { json = node.ToJsonString() });

        var active = new MappingStore(_database).Active();
        var defaults = Mapping.Defaults();
        var activeRequest = active.Profiles.Single(p => p.Id == "g2b-request-v1");
        Assert.Equal("editedTitle", activeRequest.Fields.Single(f => f.Target == "title").Source);
        Assert.Equal("prspPrce", activeRequest.Fields.Single(f => f.Target == "estimated_price").Source);
        Assert.Contains(activeRequest.Tables[0].Fields, f => f.Target == "classification_number");
        Assert.Equal(defaults.Profiles.Single(p => p.Id == "g2b-notice-a-v1").Tables.Select(t => t.Target),
            active.Profiles.Single(p => p.Id == "g2b-notice-a-v1").Tables.Select(t => t.Target));

        // 채운 매핑으로 실제로 담긴다.
        Capture("g2b-request-v1", "01117", new
        {
            pointInfo = new { editedTitle = "시험 구매", prspPrce = "5000" },
            tables = new Dictionary<string, object> { ["mf_wfm_container_gridView"] = new[] {
                new { ctrtDmndRcptNo = "RC023", ctrtDmndRcptOrd = "000", ctrtDmndRcptItemSqno = "001", itemClsfNo = "00000003" } } },
        });
        Values(Row("SELECT * FROM v_접수 WHERE 접수번호 = 'RC023-000'"), ("요청명", "시험 구매"), ("추정가격", "5,000"));
        Values(Row("SELECT * FROM v_접수품목 WHERE 접수번호 = 'RC023-000'"), ("물품분류번호", "00000003"));

        // 기본과 같은 편집본은 기본 그대로다 — 채울 것이 없으면 손대지 않는다.
        Assert.Equal(Mapping.Hash(defaults), Mapping.Hash(Mapping.Read(JsonSerializer.Serialize(defaults, Mapping.Json))));
    }
}

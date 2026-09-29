using System.Buffers.Binary;
using System.Text;
using System.Text.Json;
using Dapper;
using Microsoft.Data.Sqlite;
using Pclm.Core.Erp;
using Pclm.Core.Storage;
using Xunit;

namespace Pclm.Core.Tests;

public sealed class ErpCaptureTests : IDisposable
{
    private readonly string _root = Path.Combine(Path.GetTempPath(), "pclm-erp-" + Guid.NewGuid().ToString("N"));
    private string DbPath => Path.Combine(_root, "pclm.db");
    private readonly ErpCapture _host;
    private readonly string _dataset;

    public ErpCaptureTests()
    {
        var database = new Database(DbPath);
        database.Migrate();
        using var connection = database.Open();
        connection.Execute("UPDATE erp_dataset SET environment = 'development';");
        _dataset = connection.QuerySingle<string>("SELECT dataset_id FROM erp_dataset;");
        _host = new(DbPath);
    }

    public void Dispose()
    {
        foreach (var mode in new[] { SqliteOpenMode.ReadWriteCreate, SqliteOpenMode.ReadOnly, SqliteOpenMode.ReadWrite })
        {
            using var c = new SqliteConnection(new SqliteConnectionStringBuilder { DataSource = DbPath, Mode = mode }.ToString());
            SqliteConnection.ClearPool(c);
        }
        SqliteConnection.ClearAllPools(); // 자리 옮기기 시험이 연 다른 DB 들
        Directory.Delete(_root, recursive: true);
    }

    private JsonElement Call(string method, object? args = null, ErpCapture? host = null) =>
        JsonSerializer.SerializeToElement((host ?? _host).Dispatch(JsonSerializer.SerializeToElement(new
        { protocolVersion = 2, requestId = Guid.NewGuid().ToString(), method, @params = args })), Mapping.Json);

    [Fact]
    public void 내장_확장을_꺼내고_같은_ID로_파일과_exe_연결을_갱신한다()
    {
        var directory = Path.Combine(_root, "사용자 폴더");
        var exe = Path.Combine(_root, "계약목록.exe");
        Assert.Throws<InvalidOperationException>(() => BundledExtension.Prepare(directory, exe));
        Assert.False(Directory.Exists(directory));
        File.WriteAllText(exe, "test executable");
        var prepared = BundledExtension.Prepare(directory, exe);
        foreach (var file in new[] { "manifest.json", "background.js", "capture.js", "export.js", "popup.js", "popup.html", "popup.css", "options.html", "options.js" })
            Assert.NotEmpty(File.ReadAllBytes(Path.Combine(prepared.Folder, file)));
        using var manifest = JsonDocument.Parse(File.ReadAllText(Path.Combine(prepared.Folder, "manifest.json")));
        Assert.Equal(manifest.RootElement.GetProperty("version").GetString(), prepared.Version);
        Assert.Equal("hnfnknkojoobmpcgnimdjkpnmannhigc", prepared.ExtensionId);
        var script = File.ReadAllText(Path.Combine(prepared.Folder, "background.js"));
        File.WriteAllText(Path.Combine(prepared.Folder, "background.js"), "old");
        File.WriteAllText(Path.Combine(directory, "erp-connection.json"), "existing DB binding");
        var moved = Path.Combine(_root, "옮긴 앱.exe");
        File.Move(exe, moved);
        var updated = BundledExtension.Prepare(directory, moved);
        Assert.Equal(prepared.Folder, updated.Folder);
        Assert.Equal(prepared.ExtensionId, updated.ExtensionId);
        Assert.Equal(script, File.ReadAllText(Path.Combine(updated.Folder, "background.js")));
        Assert.Equal("existing DB binding", File.ReadAllText(Path.Combine(directory, "erp-connection.json")));
        using var host = JsonDocument.Parse(File.ReadAllText(updated.ManifestPath));
        Assert.Equal(moved, host.RootElement.GetProperty("path").GetString());
        Assert.Equal(ErpConnection.HostName, host.RootElement.GetProperty("name").GetString());
        Assert.Equal("stdio", host.RootElement.GetProperty("type").GetString());
        var origins = host.RootElement.GetProperty("allowed_origins").EnumerateArray().ToArray();
        Assert.Single(origins);
        Assert.Equal($"chrome-extension://{updated.ExtensionId}/", origins[0].GetString());
        Assert.Equal(origins[0].GetString(), File.ReadAllText(Path.Combine(directory, "extension-origin.txt")));
        Assert.Empty(Directory.GetFiles(directory, "*.tmp", SearchOption.AllDirectories));
        Assert.True(updated.Changed);

        // 앱을 켤 때마다 불린다. 같은 것이면 아무것도 쓰지 않는다.
        var files = Directory.GetFiles(directory, "*", SearchOption.AllDirectories)
            .Where(f => Path.GetFileName(f) != "erp-connection.json").ToArray();
        var old = new DateTime(2020, 1, 1, 0, 0, 0, DateTimeKind.Utc);
        foreach (var file in files) File.SetLastWriteTimeUtc(file, old);
        Assert.False(BundledExtension.Prepare(directory, moved).Changed);
        Assert.All(files, file => Assert.Equal(old, File.GetLastWriteTimeUtc(file)));
        File.WriteAllText(Path.Combine(updated.Folder, "popup.js"), "old");
        Assert.True(BundledExtension.Prepare(directory, moved).Changed);
        Assert.NotEqual("old", File.ReadAllText(Path.Combine(updated.Folder, "popup.js")));
        Assert.Equal(old, File.GetLastWriteTimeUtc(Path.Combine(updated.Folder, "background.js")));
        Assert.Equal(prepared.Version, BundledExtension.EmbeddedVersion());
    }

    [Fact]
    public void 확장_상태는_풀어_둔_판과_브라우저별_마지막_연결을_보인다()
    {
        var directory = Path.Combine(_root, "상태");
        var empty = ExtensionStatus.Read(directory, "0.4.1");
        Assert.False(empty.Prepared);
        Assert.Null(empty.DiskVersion);
        Assert.Empty(empty.Contacts);
        Assert.Equal("0.4.1", empty.EmbeddedVersion);

        Directory.CreateDirectory(Path.Combine(directory, "extension"));
        File.WriteAllText(Path.Combine(directory, "extension", "manifest.json"), "{\"version\":\"0.4.0\"}");
        File.WriteAllText(Path.Combine(directory, "extension-contact.json"), "깨진 파일");
        ExtensionStatus.RecordContact(directory, "0.4.0", "Chrome");
        var state = ExtensionStatus.Read(directory, "0.4.1");
        Assert.True(state.Prepared);
        Assert.Equal("0.4.0", state.DiskVersion);
        var contact = Assert.Single(state.Contacts);
        Assert.Equal(("Chrome", "0.4.0"), (contact.Browser, contact.Version));
        Assert.True(DateTime.TryParse(contact.At, out _));
    }

    [Fact]
    public void 업무_호스트의_인사는_디스크의_확장_판을_알려_주고_브라우저별_연결을_적는다()
    {
        var directory = Path.Combine(_root, "확장 자리");
        Directory.CreateDirectory(Path.Combine(directory, "extension"));
        File.WriteAllText(Path.Combine(directory, "extension", "manifest.json"), "{\"version\":\"0.4.1\"}");
        var host = new ErpCapture(DbPath, extensionDirectory: directory);
        var contactPath = Path.Combine(directory, "extension-contact.json");

        // 옛 확장은 params 없이 인사한다. 그래도 성공이고 적을 것이 없다.
        var hello = Call("hello", host: host);
        Assert.True(hello.GetProperty("ok").GetBoolean());
        Assert.Equal("0.4.1", hello.GetProperty("result").GetProperty("extensionVersion").GetString());
        Assert.False(File.Exists(contactPath));

        Assert.True(Call("hello", new { extensionVersion = "0.4.0", browser = "Edge" }, host).GetProperty("ok").GetBoolean());
        Assert.True(Call("hello", new { extensionVersion = "0.4.1", browser = "Chrome" }, host).GetProperty("ok").GetBoolean());
        // 확장이 보낸 값은 모양이 맞는 것만 받는다.
        foreach (var (version, browser) in new[] { ("0.4.1", "Firefox"), ("../../x", "Edge"), ("0.4.1\n", "Chrome"), ("1.2.3.4.5", "Edge") })
            Assert.True(Call("hello", new { extensionVersion = version, browser }, host).GetProperty("ok").GetBoolean());
        var contacts = ExtensionStatus.Read(directory, "0.4.1").Contacts;
        Assert.Equal(new[] { ("Edge", "0.4.0"), ("Chrome", "0.4.1") }, contacts.Select(c => (c.Browser, c.Version)));
        using (var file = JsonDocument.Parse(File.ReadAllText(contactPath)))
            Assert.Equal(new[] { "Edge", "Chrome" }, file.RootElement.EnumerateObject().Select(p => p.Name));

        // 기록 자리를 쓸 수 없어도 인사는 성공이다.
        File.Delete(contactPath);
        Directory.CreateDirectory(contactPath);
        Assert.True(Call("hello", new { extensionVersion = "0.4.1", browser = "Edge" }, host).GetProperty("ok").GetBoolean());

        // 개발 호스트는 판도 연결도 모른다.
        var plain = Call("hello", new { extensionVersion = "0.4.1", browser = "Edge" });
        Assert.True(plain.GetProperty("ok").GetBoolean());
        Assert.True(!plain.GetProperty("result").TryGetProperty("extensionVersion", out var none) || none.ValueKind == JsonValueKind.Null);
        Assert.Empty(Directory.GetFiles(_root, "extension-contact.json"));
    }

    private static CaptureInput Snapshot(string title = "검증용 공고") => new(
        "g2b-public-notice-header-v1", Mapping.Hash(Mapping.Defaults()), JsonSerializer.SerializeToElement(new
        {
            pointInfo = new { noticeBase = "R26BK00000001", seq = "001", title, notice_kind = "실공고(변경공고)",
                posted_at = "2026/09/24 10:00:00", award_method = "적격심사제", contract_method = "제한경쟁", notice_agency = "시험기관" },
            tables = new { }
        }), "live");

    private object Request(CaptureInput snapshot, string? token = null, string? id = null)
    {
        var preview = _host.Inspect(snapshot);
        return new { datasetId = _dataset, snapshot, captureId = id ?? Guid.NewGuid().ToString(), baseToken = token ?? preview.BaseToken,
            choices = preview.Changes.Where(c => c.Conflict).ToDictionary(c => c.Id, c => c.Field == "__override" ? "keep" : "apply") };
    }

    [Fact]
    public void 기본정보만_저장하고_재전송과_결과조회는_같은_영수증을_반환한다()
    {
        using var connection = new Database(DbPath).Open();
        connection.Execute("""
            INSERT INTO notice_group VALUES ('R26BK00000001');
            INSERT INTO notice_series VALUES ('R26BK00000001', 'R26BK00000001');
            INSERT INTO notice(notice_base, seq, title, notice_officer, updated_at)
                VALUES ('R26BK00000001', '001', '옛 제목', '보존할 담당자', 'before');
            INSERT INTO notice_item(notice_base, seq, line_no, item_name) VALUES ('R26BK00000001', '001', 1, '보존할 품목');
            INSERT INTO field_override(entity_type, base, seq, column_name, value, original, updated_at)
                VALUES ('notice', 'R26BK00000001', '001', '공고명', '사용자 제목', '옛 제목', 'before');
            """);
        var id = Guid.NewGuid().ToString();
        var args = Request(Snapshot(), id: id);
        var saved = Call("capture", args);
        Assert.True(saved.GetProperty("ok").GetBoolean(), saved.ToString());
        var receipt = saved.GetProperty("result").ToString();
        Assert.Equal(receipt, Call("capture", args).GetProperty("result").ToString());
        Assert.Equal(receipt, Call("captureStatus", new { datasetId = _dataset, captureId = id }).GetProperty("result").ToString());
        Assert.Equal("검증용 공고", connection.QuerySingle<string>("SELECT title FROM notice;"));
        Assert.Equal("보존할 담당자", connection.QuerySingle<string>("SELECT notice_officer FROM notice;"));
        Assert.Equal("보존할 품목", connection.QuerySingle<string>("SELECT item_name FROM notice_item;"));
        Assert.Equal("사용자 제목", connection.QuerySingle<string>("SELECT value FROM field_override;"));
        Assert.Equal(1, connection.ExecuteScalar<int>("SELECT count(*) FROM erp_capture;"));
        Assert.Equal("invalid_request", Call("capture", Request(Snapshot("다른 내용"), id: id)).GetProperty("error").GetProperty("code").GetString());
        var timestamp = connection.QuerySingle<string>("SELECT updated_at FROM notice;");
        Assert.False(Call("capture", Request(Snapshot())).GetProperty("result").GetProperty("changed").GetBoolean());
        Assert.Equal(timestamp, connection.QuerySingle<string>("SELECT updated_at FROM notice;"));
    }

    [Fact]
    public void 미리보기_뒤_수정된_자료나_다른_DB에는_저장하지_않는다()
    {
        var args = Request(Snapshot());
        Assert.True(Call("capture", Request(Snapshot("다른 수집"))).GetProperty("ok").GetBoolean());
        Assert.Equal("invalid_request", Call("capture", args).GetProperty("error").GetProperty("code").GetString());
        Assert.Equal("invalid_request", Call("inspect", new { datasetId = "other", snapshot = Snapshot() }).GetProperty("error").GetProperty("code").GetString());
    }

    /// <summary>
    /// 확장 설정 창은 자리를 옮기는 두 번째 입구다. 앱과 같은 검증을 거쳐 <b>예약만</b> 하고,
    /// 옮기는 것은 여전히 다음 앱 실행 맨 앞이다.
    /// </summary>
    [Fact]
    public void 확장은_업무_DB의_자리만_보이고_옮기기는_다음_실행으로_예약만_한다()
    {
        var config = Path.Combine(_root, "config.json");
        var host = new ErpCapture(DbPath, config);
        string? Error(JsonElement reply) => reply.GetProperty("ok").GetBoolean() ? null
            : reply.GetProperty("error").GetProperty("message").GetString();
        JsonElement Stage(string folder, string? dataset = null) =>
            Call("stageDataMove", new { datasetId = dataset ?? _dataset, folder }, host);
        var target = Path.Combine(_root, "새 자리", DataLocation.FolderName);

        // 개발 DB 는 격리된 고정 자리다.
        Assert.Contains("개발 DB", Error(Call("dataLocation", null, host)));
        Assert.Contains("개발 DB", Error(Stage(Path.Combine(_root, "새 자리"))));
        using (var c = new Database(DbPath).Open()) c.Execute("UPDATE erp_dataset SET environment = 'production';");
        var viaFiles = JsonSerializer.SerializeToElement(host.Dispatch(JsonSerializer.SerializeToElement(new
            { protocolVersion = 2, requestId = Guid.NewGuid().ToString(), method = "dataLocation" }), filesAllowed: true), Mapping.Json);
        Assert.Contains("개발 DB", Error(viaFiles));

        // 쪽지가 가리키는 자리(여기서는 쪽지가 없어 기본 자리)가 바인딩된 DB 와 다르면 옮길 자료를 믿을 수 없다.
        Assert.Contains("다시 실행", Error(Stage(Path.Combine(_root, "새 자리"))));
        Assert.False(File.Exists(config));

        DataLocation.Save(new DataLocationConfig { DataDir = _root }, config);
        var shown = Call("dataLocation", null, host).GetProperty("result");
        Assert.Equal(DbPath, shown.GetProperty("path").GetString(), ignoreCase: true);
        Assert.Equal(DataLocation.FolderName, shown.GetProperty("folderName").GetString());
        Assert.Equal(JsonValueKind.Null, shown.GetProperty("pending").ValueKind);

        foreach (var bad in new[] { "", "   ", @"상대\폴더", @"D:상대", "C:\\a|b" })
            Assert.NotNull(Error(Stage(bad)));
        Assert.NotNull(Error(Stage(Path.Combine(_root, "새 자리"), dataset: "other")));
        var busy = Path.Combine(_root, "쓰는 중");
        new Database(DataLocation.DbIn(Path.Combine(busy, DataLocation.FolderName))).Migrate();
        Assert.Contains("이미 계약 목록 자료가 있습니다", Error(Stage(busy)));
        Assert.Equal(_root, DataLocation.Load(config).DataDir);
        Assert.Null(DataLocation.Load(config).PendingMoveFrom);

        var staged = Stage(Path.Combine(_root, "새 자리"));
        Assert.Null(Error(staged));
        Assert.Equal(target, staged.GetProperty("result").GetProperty("folder").GetString(), ignoreCase: true);
        Assert.Equal(target, DataLocation.Load(config).DataDir, ignoreCase: true);
        Assert.Equal(_root, DataLocation.Load(config).PendingMoveFrom, ignoreCase: true);
        Assert.False(File.Exists(DataLocation.DbIn(target))); // 예약만 했다.
        var pending = Call("dataLocation", null, host).GetProperty("result").GetProperty("pending");
        Assert.Equal(_root, pending.GetProperty("from").GetString(), ignoreCase: true);
        Assert.Equal(target, pending.GetProperty("to").GetString(), ignoreCase: true);

        // 다시 고르면 목적지만 바뀐다. 자료는 여전히 바인딩된 옛 자리에 있다.
        var other = Path.Combine(_root, "다른 자리", DataLocation.FolderName);
        Assert.Null(Error(Stage(Path.Combine(_root, "다른 자리"))));
        Assert.Equal(_root, DataLocation.Load(config).PendingMoveFrom, ignoreCase: true);
        Assert.Equal(other, DataLocation.Load(config).DataDir, ignoreCase: true);

        // 다음 앱 실행 맨 앞에서 실제로 옮겨진다.
        var resolved = DataLocation.Resolve(config);
        Assert.Equal(DataLocation.DbIn(other), resolved.DbPath, ignoreCase: true);
        Assert.True(File.Exists(DataLocation.DbIn(other)));
    }

    [Fact]
    public void 영수증_기록에_실패하면_업무자료도_롤백한다()
    {
        using var connection = new Database(DbPath).Open();
        connection.Execute("CREATE TRIGGER reject_receipt BEFORE INSERT ON erp_capture BEGIN SELECT RAISE(ABORT, 'test'); END;");
        Assert.False(Call("capture", Request(Snapshot())).GetProperty("ok").GetBoolean());
        Assert.Equal(0, connection.ExecuteScalar<int>("SELECT count(*) FROM notice;"));
        Assert.Equal(0, connection.ExecuteScalar<int>("SELECT count(*) FROM notice_series;"));
        Assert.Equal(0, connection.ExecuteScalar<int>("SELECT count(*) FROM erp_capture;"));
    }

    [Fact]
    public void 지원하지_않거나_불완전한_화면은_저장을_거부한다()
    {
        foreach (var snapshot in new[] { Snapshot() with { Profile = "unknown" },
            Snapshot() with { MappingRevision = "old" }, Snapshot() with { Scope = "file" } })
            Assert.False(Call("inspect", new { datasetId = _dataset, snapshot }).GetProperty("ok").GetBoolean());
    }

    [Fact]
    public void 호스트는_DB를_만들거나_판올림하거나_운영DB로_우회하지_않는다()
    {
        var absent = Path.Combine(_root, "absent", "pclm.db");
        Assert.Equal("invalid_request", Call("hello", host: new(absent)).GetProperty("error").GetProperty("code").GetString());
        Assert.False(Directory.Exists(Path.GetDirectoryName(absent)));
        using var connection = new Database(DbPath).Open();
        connection.Execute("UPDATE erp_dataset SET environment = 'unbound';");
        Assert.Throws<InvalidOperationException>(() => ErpDevelopment.MarkInitialized(new Database(DbPath)));
        Assert.Equal("invalid_request", Call("hello").GetProperty("error").GetProperty("code").GetString());
        connection.Execute("PRAGMA user_version = 15;");
        Assert.Equal("invalid_request", Call("hello").GetProperty("error").GetProperty("code").GetString());
        Assert.Equal(15, connection.ExecuteScalar<int>("PRAGMA user_version;"));
    }

    [Fact]
    public void 통신은_잘린_읽기와_연속_메시지에서_UTF8_바이트_길이를_지킨다()
    {
        var payload = Encoding.UTF8.GetBytes("{\"value\":\"한글\"}");
        using var input = new OneByteStream(Frame(payload).Concat(Frame(payload)).ToArray());
        using var output = new MemoryStream();
        NativeMessaging.Run(input, output, root => new { value = root.GetProperty("value").GetString() });
        output.Position = 0;
        for (var i = 0; i < 2; i++)
        {
            var header = new byte[4]; output.ReadExactly(header);
            var body = new byte[BinaryPrimitives.ReadUInt32LittleEndian(header)]; output.ReadExactly(body);
            Assert.Equal("한글", JsonDocument.Parse(body).RootElement.GetProperty("value").GetString());
        }
        Assert.Equal(output.Length, output.Position);
    }

    [Theory]
    [InlineData(0)]
    [InlineData(NativeMessaging.MaxRequestBytes + 1)]
    public void 과대한_프레임은_할당과_호출_전에_거부한다(uint length)
    {
        var header = new byte[4]; BinaryPrimitives.WriteUInt32LittleEndian(header, length);
        using var input = new MemoryStream(header);
        using var output = new MemoryStream();
        Assert.Throws<InvalidDataException>(() => NativeMessaging.Run(input, output, _ => throw new Exception("호출되면 안 됨")));
        Assert.Equal(0, output.Length);
    }


    [Fact]
    public void JSON_변경은_검토하고_미수집과_정정값은_보존한다()
    {
        var db = new Database(DbPath);
        var host = new ErpCapture(db);
        var mapping = Mapping.Defaults();
        var input = SampleRequest(mapping, "첫 제목");
        var preview = host.Inspect(input);
        Assert.Equal(2, preview.ItemCount);
        var id = Guid.NewGuid().ToString();
        var stored = JsonSerializer.Serialize(host.Save(input, preview.BaseToken, id, new()));
        Assert.Equal(stored, JsonSerializer.Serialize(host.Save(input, preview.BaseToken, id, new())));
        using var c = db.Open();
        c.Execute("UPDATE request SET remarks='수집에 없는 값' WHERE request_base='RC001'");
        var changed = SampleRequest(mapping, "다른 제목");
        var next = host.Inspect(changed);
        Assert.Throws<InvalidOperationException>(() => host.Save(changed, next.BaseToken, Guid.NewGuid().ToString(), new()));
        var choices = next.Changes.Where(x => x.Conflict).ToDictionary(x => x.Id, _ => "keep");
        host.Save(changed, next.BaseToken, Guid.NewGuid().ToString(), choices);
        Assert.Equal("첫 제목", c.ExecuteScalar<string>("SELECT title FROM request"));
        Assert.Equal("수집에 없는 값", c.ExecuteScalar<string>("SELECT remarks FROM request"));
        Assert.Equal(2, c.ExecuteScalar<int>("SELECT count(*) FROM request_item"));
        c.Execute("UPDATE request SET title='동시 편집'");
        Assert.Throws<InvalidOperationException>(() => host.Save(changed, next.BaseToken, Guid.NewGuid().ToString(), choices));
    }

    [Fact]
    public void 매핑은_검증후_활성화하고_중복키와_민감필드를_거절한다()
    {
        var db = new Database(DbPath); var mappings = new MappingStore(db);
        var set = Mapping.Defaults(); var json = JsonSerializer.Serialize(set, Mapping.Json);
        var revision = mappings.Save(json);
        Assert.Throws<InvalidOperationException>(() => mappings.Activate(revision));
        var sample = SampleRequest(set, "시험");
        mappings.ValidateSample(json, sample.Profile, sample.Data);
        mappings.Activate(revision);
        Assert.Equal(revision, Mapping.Hash(mappings.Active()));
        Assert.Throws<InvalidOperationException>(() => Mapping.Parse(json.Replace("ctrtDmndBizNm", "giveActno")));
        var duplicate = JsonDocument.Parse(sample.Data.GetRawText().Replace("\"002\"", "\"001\"")).RootElement;
        Assert.Throws<InvalidOperationException>(() => Mapping.Map(set, sample.Profile, duplicate));
        // 실제 화면 수집도 건수 선택자 없이 번호만 맞으면 검토로 넘어간다.
        Assert.Equal(2, new ErpCapture(db).Inspect(sample with { Scope = "live" }).ItemCount);
    }

    [Fact]
    public void 번호만_있으면_받고_걷지_않은_제목과_표는_건드리지_않는다()
    {
        var db = new Database(DbPath); var host = new ErpCapture(db); var set = Mapping.Defaults();
        CaptureInput Contract(object tables, string? title) => new("g2b-contract-v1", Mapping.Hash(set), JsonSerializer.SerializeToElement(new
        {
            // 확장은 읽지 못한 칸의 키를 싣지 않는다.
            pointInfo = title is null ? new Dictionary<string, string> { ["ctrtNoOrd"] = "R26TA00000009-00" }
                : new Dictionary<string, string> { ["ctrtNoOrd"] = "R26TA00000009-00", ["ctrtNm"] = title },
            tables,
        }), "live");
        var full = Contract(new Dictionary<string, object> { ["mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis"] = new[] {
            new { ctrtNo = "R26TA00000009", ctrtChgOrd = "00", ctrtItemSqno = 1, ctrtAmt = 1000 } } }, "시험 계약");
        host.Save(full, host.Inspect(full).BaseToken, Guid.NewGuid().ToString(), new());
        // 품목 탭을 열지 않은 화면·제목 칸이 없는 화면: 번호만으로 받되 있던 품목·제목·총액은 그대로다.
        var bare = Contract(new Dictionary<string, object> { ["mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis"] = Array.Empty<object>() }, null);
        var preview = host.Inspect(bare);
        Assert.Equal(0, preview.ItemCount);
        Assert.Empty(preview.Changes);
        Assert.Empty(host.Inspect(Contract(new { }, null)).Changes);
    }

    private static CaptureInput SampleRequest(MappingSet mapping, string title) => new(
        "g2b-request-v1", Mapping.Hash(mapping), JsonSerializer.SerializeToElement(new
        {
            pointInfo = new { ctrtDmndBizNm = title, wrtYmd = "20260927", totlPymtAmt = "0" },
            tables = new Dictionary<string, object> { ["mf_wfm_container_gridView"] = new[] {
                new { ctrtDmndRcptNo = "RC001", ctrtDmndRcptOrd = "000", ctrtDmndRcptItemSqno = "001", ndfsPrcmDmndNo = "REQ1", ctrtDmndQty = "1" },
                new { ctrtDmndRcptNo = "RC001", ctrtDmndRcptOrd = "000", ctrtDmndRcptItemSqno = "002", ndfsPrcmDmndNo = "REQ1", ctrtDmndQty = "2" },
            } }
        }));

    [Fact]
    public void 접수_상세화면은_품목표_이름이_달라도_같은_접수로_변환한다()
    {
        // 접수는 화면이 둘이다: 목록형은 gridView, 상세형은 gvDmndItem 에 품목을 싣는다.
        var set = Mapping.Defaults();
        var input = JsonSerializer.SerializeToElement(new
        {
            pointInfo = new { ctrtDmndBizNm = "시험 구매", wrtYmd = "20260927", ctrtDmndRcptNo = "RC009", ctrtDmndRcptOrd = "000", totlPymtAmt = "1000" },
            tables = new Dictionary<string, object> { ["mf_wfm_container_gvDmndItem"] = new[] {
                new { ctrtDmndRcptNo = "RC009", ctrtDmndRcptOrd = "000", ctrtDmndRcptItemSqno = 1, dtlsPrnm = "시험품", ctrtDmndQty = 1, ctrtDmndUprc = 1000, qtyUntNm = "SET" },
            } }
        });
        var doc = Mapping.Map(set, "g2b-request-b-v1", input);
        Assert.Equal(("RC009", "000"), (doc.Base, doc.Seq));
        Assert.Equal("1000", Assert.Single(doc.Rows, r => r.Table == "request_item").Values["unit_price"]);
        Assert.Throws<InvalidOperationException>(() => Mapping.Map(set, "g2b-request-v1", input));
    }

    [Fact]
    public void 로컬_JSON_코퍼스는_네_화면형을_변환한다()
    {
        var path = Environment.GetEnvironmentVariable("PCLM_JSON_CORPUS");
        if (string.IsNullOrEmpty(path)) return; // 개인 코퍼스는 저장소/CI로 복사하지 않는다.
        using var zip = System.IO.Compression.ZipFile.OpenRead(path);
        var mapping = Mapping.Defaults();
        foreach (var entry in zip.Entries.Where(e => e.Name.EndsWith(".json")))
        {
            using var stream = entry.Open(); using var document = JsonDocument.Parse(stream);
            var point = document.RootElement.GetProperty("pointInfo");
            var profile = point.TryGetProperty("ctrtNoOrd", out _) ? "g2b-contract-v1" :
                point.TryGetProperty("ctrtDmndBizNm", out _) ? document.RootElement.GetProperty("tables").TryGetProperty("mf_wfm_container_gvDmndItem", out _) ? "g2b-request-b-v1" : "g2b-request-v1" :
                point.TryGetProperty("bidPbancText", out _) ? "g2b-notice-b-v1" : "g2b-notice-a-v1";
            try {
                Assert.NotEmpty(Mapping.Map(mapping, profile, document.RootElement).Rows);
                var service = new ErpCapture(new Database(DbPath));
                var input = new CaptureInput(profile, Mapping.Hash(mapping), document.RootElement);
                var preview = service.Inspect(input);
                service.Save(input, preview.BaseToken, Guid.NewGuid().ToString(), preview.Changes.Where(c => c.Conflict).ToDictionary(c => c.Id, _ => "apply"));
                var again = service.Inspect(input);
                Assert.Empty(again.Changes);
            }
            catch (Exception e) { throw new InvalidOperationException(entry.Name + ": " + e.Message, e); }
        }
    }


    [Fact]
    public void 명시키로만_연결하고_접수의_반복요구번호와_공고의_모호한_현행을_보존한다()
    {
        var db = new Database(DbPath); var host = new ErpCapture(db); var set = Mapping.Defaults();
        var request = SampleRequest(set, "같은 이름");
        host.Save(request, host.Inspect(request).BaseToken, Guid.NewGuid().ToString(), new());
        using var c = db.Open();
        Assert.Equal("RC001", new Store(db).Resolve("RC001-000")!.Base);
        for (var i = 1; i <= 2; i++)
        {
            var number = $"R26BK0000000{i}";
            new Store(db).UpsertNotice(new Pclm.Core.Domain.NoticeRecord { NoticeBase = number, Seq = "000", Title = "같은 이름" });
            c.Execute("INSERT INTO notice_item(notice_base,seq,line_no,ref_request_base,ref_request_seq,ref_request_item) VALUES(@number,'000',1,'RC001','000','001')", new { number });
        }
        ExplicitLinks.Resolve(db);
        Assert.Equal(1, c.ExecuteScalar<int>("SELECT count(DISTINCT group_base) FROM notice_series"));
        Assert.Equal(1, c.ExecuteScalar<int>("SELECT count(*) FROM request_link WHERE decided_by='explicit'"));
        Assert.Empty(c.Query(NoticeGroups.현행공고));
        Assert.Equal(0, c.ExecuteScalar<int>("SELECT count(*) FROM request_item_link")); // 차수가 다른 공고 품목 둘이면 고르지 않는다.
        Assert.Equal(2, c.ExecuteScalar<int>("SELECT count(*) FROM request_item WHERE request_number='REQ1'"));
    }

    /// <summary>
    /// 접수의 키는 접수번호·접수차수 그대로다(ADR-027). 한동안 <c>ERP:</c> 를 달고 저장되어
    /// 화면과 계약면의 접수번호 칸에 그 접두어가 그대로 찍혔다.
    /// </summary>
    [Fact]
    public void 접수_수집의_키는_접수번호_그대로다()
    {
        var db = new Database(DbPath); var host = new ErpCapture(db);
        var input = SampleRequest(Mapping.Defaults(), "시험 구매");
        var preview = host.Inspect(input);
        Assert.Equal("RC001-000", preview.Entity);
        var result = JsonSerializer.SerializeToElement(host.Save(input, preview.BaseToken, Guid.NewGuid().ToString(), new()));
        Assert.Equal("RC001-000", result.GetProperty("entity").GetString());
        using var c = db.Open();
        Assert.Equal("RC001", c.QuerySingle<string>("SELECT request_base FROM request"));
        Assert.Equal("RC001", c.QuerySingle<string>("SELECT request_base FROM request_series"));
        Assert.Equal("RC001", c.QuerySingle<string>("SELECT entity_base FROM erp_source WHERE entity_type='request'"));
        Assert.Equal("RC001", c.QuerySingle<string>("SELECT entity_base FROM erp_capture"));
        Assert.Equal("RC001-000", c.QuerySingle<string>("SELECT 접수번호 FROM v_접수_v1"));
        Assert.Equal(0, c.ExecuteScalar<int>("SELECT count(*) FROM request WHERE request_base LIKE 'ERP:%'"));
    }

    private static byte[] Frame(byte[] payload)
    {
        var frame = new byte[payload.Length + 4];
        BinaryPrimitives.WriteUInt32LittleEndian(frame, (uint)payload.Length);
        payload.CopyTo(frame, 4); return frame;
    }
    private sealed class OneByteStream(byte[] bytes) : MemoryStream(bytes)
    {
        public override int Read(Span<byte> buffer) => base.Read(buffer[..Math.Min(buffer.Length, 1)]);
        public override int Read(byte[] buffer, int offset, int count) => base.Read(buffer, offset, Math.Min(count, 1));
    }
}

using System.Text.Json;
using Dapper;
using Microsoft.Data.Sqlite;
using Pclm.Core.Storage;

namespace Pclm.Core.Erp;

/// <param name="Screen">그 자료를 읽은 화면의 메뉴 번호(<see cref="Screens"/>). 확장은 실제 화면에서 읽어 싣고, 창의 고급 도구는
/// 수집 이력을 다시 검토할 때 이력에 적힌 번호를 싣는다. 파일은 비워 두면 원본의 메뉴 경로(<see cref="Screens.Leaf"/>)를 본다.
/// 비었을 때는 직렬화하지 않아, 같은 요청의 해시(재전송 영수증·baseToken)가 예전과 같다.</param>
public sealed record CaptureInput(string Profile, string MappingRevision, JsonElement Data,
    string Scope = "file", Dictionary<string, int>? Totals = null,
    Dictionary<string, int>? RowMatches = null,
    [property: System.Text.Json.Serialization.JsonIgnore(Condition = System.Text.Json.Serialization.JsonIgnoreCondition.WhenWritingNull)]
    string? Screen = null);
public sealed record CaptureChange(string Id, string Table, int Line, string Field,
    string? Before, string? After, bool Conflict, string? Override = null);
public sealed record RowMatch(string Id, string Table, string SourceKey, object[] Existing);
public sealed record CapturePreview(string DatasetId, string Entity, string EntityType, string BaseToken,
    string MappingRevision, MappedDocument Document, List<CaptureChange> Changes,
    List<RowMatch> Unmatched, int ItemCount);

/// <summary>확장과 파일 입력의 동일한 미리보기/커밋 경계. 부분 열 갱신만 허용한다.</summary>
/// <param name="databasePath">바인딩된 DB.</param>
/// <param name="extensionDirectory">확장을 풀어 둔 자리(<see cref="Home.Directory"/>). 호스트만 넘긴다 —
/// 있으면 hello 가 디스크의 확장 판을 알려 주고 브라우저별 연결을 적는다.</param>
/// <param name="environment">이 호스트가 어느 쪽인가 — 개발 호스트면 <c>development</c>, 아니면 <c>production</c>.
/// 파일에 적던 것을 v20 에서 걷었다: 개발용인지는 자료가 아니라 <b>그 자료를 연 호스트</b>의 사정이라
/// <c>App/Program.cs</c> 가 넘긴다. 확장이 hello 의 이 값으로 팝업에 「개발용」 을 띄운다.</param>
/// <param name="home">이 파일을 가리켜야 하는 쪽지의 홈. 확장 호스트만 넘긴다 — 있으면 저장할 때 쓰기 잠금
/// 안에서 쪽지가 아직 이 파일을 가리키는지 다시 본다(<see cref="Save"/>). 창 안의 고급 도구는 창이 연 그 파일에
/// 쓰는 것이라 넘기지 않는다.</param>
public sealed class ErpCapture(string databasePath, string? extensionDirectory = null, string environment = "production", Home? home = null)
{
    public ErpCapture(Database database, string? extensionDirectory = null, string environment = "production", Home? home = null)
        : this(database.Path, extensionDirectory, environment, home) { }
    private Database database => new(databasePath);
    public object Dispatch(JsonElement request, bool filesAllowed = false)
    {
        string? requestId = null;
        try
        {
            requestId = request.GetProperty("requestId").GetString();
            if (!Guid.TryParse(requestId, out _) || request.GetProperty("protocolVersion").GetInt32() != 2)
                throw new InvalidOperationException("앱과 확장을 함께 업데이트하세요. 프로토콜 2가 필요합니다.");
            using var c = Open();
            var datasetId = c.QuerySingle<string>("SELECT dataset_id FROM pclm_file");
            // 확장이 쓰는 것은 작업자료뿐이다. 제출본·취합본·백업은 그때의 기록이라 거기에 수집이 쌓이면
            // 보낸 것과 받은 것이 조용히 갈린다.
            if (!filesAllowed && c.QuerySingle<string>("SELECT role FROM pclm_file") != PclmRole.Work)
                throw new InvalidOperationException("메인 프로그램에서 확장 저장 대상을 준비하세요.");
            var method = request.GetProperty("method").GetString();
            if (method == "hello")
            {
                var mapping = new MappingStore(database).Active();
                string? extensionVersion = null;
                if (extensionDirectory is not null)
                {
                    extensionVersion = ExtensionStatus.DiskVersion(extensionDirectory);
                    // 옛 확장은 params 없이 인사한다. 적지 못해도 인사는 성공이다 — 기록은 설정 패널의 참고일 뿐이다.
                    if (request.TryGetProperty("params", out var hello) && hello.ValueKind == JsonValueKind.Object &&
                        hello.TryGetProperty("extensionVersion", out var version) && version.ValueKind == JsonValueKind.String &&
                        hello.TryGetProperty("browser", out var browser) && browser.ValueKind == JsonValueKind.String)
                        try { ExtensionStatus.RecordContact(extensionDirectory, version.GetString()!, browser.GetString()!); }
                        catch (Exception e) when (e is IOException or UnauthorizedAccessException) { }
                }
                // 수집하는 화면은 mapping 의 프로필이 단 screen 이다(ADR-037). 확장은 그것으로만 화면을 가린다.
                return Reply(new { datasetId, environment,
                    destination = database.Path, mapping,
                    mappingRevision = Mapping.Hash(mapping), maxRequestBytes = NativeMessaging.MaxRequestBytes, extensionVersion });
            }
            // 자료 자리를 보거나 옮기는 요청(dataLocation·stageDataMove)은 받지 않는다 — 브라우저가 띄운 통로가
            // 저장 구조를 고치는 통로를 겸하면 안 된다(ADR-032). 아래 「지원하지 않는 요청」 으로 떨어진다.
            var args = request.GetProperty("params");
            if (args.GetProperty("datasetId").GetString() != datasetId) throw new InvalidOperationException("저장 대상이 바뀌었습니다. 다시 확인하세요.");
            if (method == "captureStatus")
            {
                var json = c.QuerySingleOrDefault<string>("SELECT result_json FROM erp_capture WHERE capture_id=@id", new { id = args.GetProperty("captureId").GetString() });
                return Reply(json is null ? new { status = "not_found" } : JsonSerializer.Deserialize<JsonElement>(json));
            }
            var input = args.GetProperty("snapshot").Deserialize<CaptureInput>(Mapping.Json)!;
            if (!filesAllowed && input.Scope != "live") throw new InvalidOperationException("확장은 실제 화면 수집만 저장할 수 있습니다.");
            if (method == "inspect")
            {
                var view = Inspect(input);
                return Reply(new { view.DatasetId, view.Entity, view.EntityType, view.BaseToken, view.MappingRevision,
                    document = new { rows = view.Document.Rows.Take(1) }, view.ItemCount,
                    changes = view.Changes.Where(ch => ch.Conflict), unmatched = view.Unmatched.Select(r => new { r.Id, r.Table, r.SourceKey }) });
            }
            if (method == "capture") return Reply(Save(input, args.GetProperty("baseToken").GetString()!,
                args.GetProperty("captureId").GetString()!, args.TryGetProperty("choices", out var choices)
                    ? choices.Deserialize<Dictionary<string, string>>(Mapping.Json)! : new()));
            throw new InvalidOperationException("지원하지 않는 요청입니다.");
        }
        catch (Exception e) when (e is InvalidOperationException or JsonException or KeyNotFoundException or SqliteException or FormatException)
        { return new { protocolVersion = 2, requestId, ok = false, error = new { code = "invalid_request", message = e.Message } }; }
        object Reply(object result) => new { protocolVersion = 2, requestId, ok = true, result };
    }
    /// <summary>
    /// 그 입력을 읽은 화면의 메뉴 번호. 실제 화면은 확장이 실은 번호뿐이다. 파일은 실린 번호(수집 이력의 재검토)가 있으면
    /// 그것을, 없으면 원본 JSON 의 메뉴 경로를 본다. 모르면 null.
    /// </summary>
    private static string? ScreenOf(CaptureInput input) =>
        input.Scope == "live" || input.Screen is not null ? input.Screen
        : input.Data.ValueKind == JsonValueKind.Object && input.Data.TryGetProperty("pointInfo", out var point) ? Screens.Leaf(point) : null;

    internal SqliteConnection Open()
    {
        if (!File.Exists(databasePath)) throw new InvalidOperationException("메인 프로그램을 먼저 실행해 저장 대상을 준비하세요.");
        var c = new SqliteConnection(new SqliteConnectionStringBuilder { DataSource = databasePath, Mode = SqliteOpenMode.ReadWrite }.ToString());
        c.Open(); c.Execute("PRAGMA foreign_keys=ON");
        if (c.ExecuteScalar<int>("PRAGMA user_version") == Schema.Version) return c;
        c.Dispose(); throw new InvalidOperationException("메인 프로그램에서 DB 판올림을 완료하세요.");
    }
    public CapturePreview Inspect(CaptureInput input)
    {
        using var c = Open(); using var tx = c.BeginTransaction(deferred: true);
        return Inspect(c, tx, input);
    }
    internal CapturePreview Inspect(SqliteConnection c, SqliteTransaction tx, CaptureInput input)
    {
        var mapping = new MappingStore(database).Active(); var revision = Mapping.Hash(mapping);
        if (revision != input.MappingRevision) throw new InvalidOperationException("매핑이 바뀌었습니다. 다시 확인하세요.");
        if (input.Scope is not ("file" or "live")) throw new InvalidOperationException("수집 범위가 잘못되었습니다.");
        // 받는 화면인가(ADR-037). 번호 칸을 읽기 전에 본다 — 다른 화면이면 번호를 못 찾았다는 말보다 이 말이 맞다.
        // 번호를 모르면 받지 않는다. 프로필을 모르면 아래 Map 이 거절한다.
        if (mapping.Profiles.Any(p => p.Id == input.Profile))
            Screens.Check(mapping, ScreenOf(input), input.Profile, input.Scope == "live");
        var doc = Mapping.Map(mapping, input.Profile, input.Data, input.Scope);
        var changes = new List<CaptureChange>(); var unmatched = new List<RowMatch>();
        var tracked = c.Query<(string Table, string SourceKey, int Line)>("SELECT table_name,source_key,line_no FROM erp_row WHERE entity_type=@EntityType AND entity_base=@Base AND entity_seq=@Seq", doc, tx).ToList();
        var used = new HashSet<string>();
        foreach (var group in doc.Rows.GroupBy(r => r.Table))
        {
            var existing = ReadRows(c, tx, doc, group.Key);
            var next = existing.Select(r => r.TryGetValue("line_no", out var n) ? Convert.ToInt32(n) : 0).DefaultIfEmpty().Max();
            foreach (var row in group)
            {
                var id = RowId(row); var line = 0;
                if (row.Table != doc.EntityType)
                {
                    line = tracked.FirstOrDefault(r => r.Table == row.Table && r.SourceKey == row.SourceKey).Line;
                    if (line == 0 && input.RowMatches?.TryGetValue(id, out var selected) == true)
                    {
                        if (selected < 0 || selected > 0 && !existing.Any(r => Convert.ToInt32(r["line_no"]) == selected))
                            throw new InvalidOperationException("대응 품목 행이 존재하지 않습니다.");
                        line = selected;
                    }
                    else if (line == 0 && existing.Count > 0)
                        unmatched.Add(new(id, row.Table, row.SourceKey, existing.Cast<object>().ToArray()));
                    if (line == 0) line = ++next;
                    if (!used.Add(row.Table + ":" + line)) throw new InvalidOperationException("여러 원천 행을 같은 품목 행으로 합칠 수 없습니다.");
                }
                var before = existing.FirstOrDefault(r => line == 0 || Convert.ToInt32(r["line_no"]) == line);
                foreach (var (field, value) in row.Values)
                {
                    var old = (before is not null && before.TryGetValue(field, out var previous) ? previous?.ToString() : null);
                    // 덮개는 표시 열 이름을 쓰므로 헤더 전체를 별도로 확인시킨다.
                    if (old != value) changes.Add(new(id + "/" + field, row.Table, line, field, old, value,
                        !string.IsNullOrEmpty(old)));
                }
            }
            // pdf: 로 시작하는 행 키는 걷힌 PDF 비교가 남긴 지난 관찰 기록이다(ADR-028). 원천이 다른
            // 행이라 이번 수집에 없다고 지우자고 하지 않는다.
            if (group.Key != doc.EntityType)
                foreach (var missing in tracked.Where(r => r.Table == group.Key && !r.SourceKey.StartsWith("pdf:") && !group.Any(row => row.SourceKey == r.SourceKey)))
                {
                    var old = existing.FirstOrDefault(r => Convert.ToInt32(r["line_no"]) == missing.Line);
                    if (old is not null && !used.Contains(missing.Table + ":" + missing.Line)) changes.Add(new("remove/" + missing.Table + "/" + missing.Line,
                        missing.Table, missing.Line, "__row", JsonSerializer.Serialize(old), null, true));
                }
        }
        var overrides = c.Query<(string Column, string Value)>("SELECT column_name,value FROM field_override WHERE entity_type=@EntityType AND base=@Base AND seq=@Seq", doc, tx).ToList();
        if (overrides.Count > 0)
        {
            // 덮개마다 묻던 것을, 이번 수집이 덮개 아래 값을 바꿀 때만 묻는다 — 바뀐 것이 없는데 다시
            // 수집할 때마다 고르게 하면 단축키 저장도 영영 막힌다. 같은 트랜잭션 안에서 평범한 변경을
            // 모두 적용하고 줄은 지우지 않은 모습을 써 보고, 덮개를 하나씩 걷어 표시 값을 읽은 뒤 되돌린다.
            // 걸린 채로 두는 덮개는 목록에 싣지 않는다: 확장은 충돌만 받고, 싣지 않은 것은 저장 때
            // 건드리지 않는다. 복원하고 싶으면 앱의 칸 편집에서 걷는다(Store.ClearOverride).
            var before = Underlying(c, tx, doc, overrides);
            tx.Save(Preview);
            try
            {
                Write(c, tx, input, doc, changes, ch => ch.Field == "__row" ? "keep" : "apply", DateTime.UtcNow.ToString("O"));
                var after = Underlying(c, tx, doc, overrides);
                foreach (var o in overrides)
                    // 읽지 못한 열(null)은 판단할 수 없으니 예전처럼 묻는다.
                    if (after[o.Column] is null || before[o.Column] != after[o.Column])
                        changes.Add(new("override/" + o.Column, doc.EntityType, 0, "__override", o.Value,
                            after[o.Column], true, o.Column));
            }
            finally { tx.Rollback(Preview); tx.Release(Preview); }
        }
        var state = new {
            mapping = revision, input = Mapping.Hash(input),
            rows = doc.Rows.Select(r => r.Table).Distinct().Select(t => new { table = t, values = ReadRows(c, tx, doc, t) }).ToArray(),
            overrides = overrides.Select(o => new { o.Column, o.Value }).ToArray(),
            tracked = tracked.Select(r => new { r.Table, r.SourceKey, r.Line }).ToArray(),
        };
        return new(c.QuerySingle<string>("SELECT dataset_id FROM pclm_file", transaction: tx),
            doc.Base + "-" + doc.Seq, doc.EntityType, Mapping.Hash(state), revision, doc, changes, unmatched,
            doc.Rows.Count(r => r.Table == doc.EntityType + "_item"));
    }
    private const string Preview = "pclm_capture_preview", Peek = "pclm_capture_peek";
    /// <summary>
    /// 덮개 아래 값을 읽는 뷰. 계약면 뷰(<see cref="Store.FaceView"/>)는 최신 차수만 내므로, 옛 차수를
    /// 다시 수집할 때도 읽히게 열 한 벌을 나눠 쓰는 차수 뷰에서 읽는다. 접수는 차수 뷰가 없다.
    /// </summary>
    internal static string RevisionView(string entityType) => entityType switch
    {
        "notice" => "v_공고차수",
        "contract" => "v_계약차수",
        _ => Store.FaceView(entityType),
    };
    /// <summary>덮개마다 그 덮개 하나만 걷었을 때 계약면에 보이는 값. 읽지 못하면 null.</summary>
    private static Dictionary<string, string?> Underlying(SqliteConnection c, SqliteTransaction tx, MappedDocument doc,
        List<(string Column, string Value)> overrides)
    {
        var key = new EntityRef(doc.EntityType, doc.Base, doc.Seq).Display;
        var values = new Dictionary<string, string?>();
        foreach (var o in overrides)
        {
            tx.Save(Peek);
            try
            {
                c.Execute("DELETE FROM field_override WHERE entity_type=@EntityType AND base=@Base AND seq=@Seq AND column_name=@Column",
                    new { doc.EntityType, doc.Base, doc.Seq, o.Column }, tx);
                // 줄이 없으면(최신 차수만 내는 접수 뷰의 옛 차수) 판단할 수 없어 null 로 둔다.
                var found = c.Query<string?>(
                    $"SELECT \"{o.Column.Replace("\"", "\"\"")}\" FROM \"{RevisionView(doc.EntityType)}\" WHERE \"{Store.FaceKey(doc.EntityType)}\"=@key",
                    new { key }, tx).ToList();
                values[o.Column] = found.Count == 0 ? null : found[0] ?? "";
            }
            catch (SqliteException) { values[o.Column] = null; }
            finally { tx.Rollback(Peek); tx.Release(Peek); }
        }
        return values;
    }
    internal static string RowId(MappedRow row) => row.Table + "/" + row.SourceKey;
    private static List<IDictionary<string, object>> ReadRows(SqliteConnection c, SqliteTransaction tx, MappedDocument doc, string table) =>
        c.Query($"SELECT * FROM {table} WHERE {doc.EntityType}_base=@Base AND seq=@Seq ORDER BY " + (table == doc.EntityType ? "seq" : "line_no"), doc, tx)
            .Select(r => (IDictionary<string, object>)r).ToList();
    public object Save(CaptureInput input, string baseToken, string captureId, Dictionary<string, string> choices)
    {
        if (!Guid.TryParse(captureId, out _)) throw new InvalidOperationException("올바른 수집 ID가 필요합니다.");
        using var c = Open(); using var tx = c.BeginTransaction(deferred: false);
        // 대상 확인을 쓰기 잠금 안에서 다시 한다. 연 뒤 저장까지 사람이 검토하는 만큼 시간이 떨어져, 그 사이
        // 창이 작업자료를 바꿨을 수 있다. 창은 나가는 자료의 쓰기 잠금을 쥔 채 쪽지를 고치므로(ADR-032),
        // 잠금을 얻은 뒤 읽은 쪽지가 지금의 진실이다 — 저장은 "바꾸기 전 옛 자료에" 아니면 "거절" 둘 중 하나다.
        // 옮긴 뒤의 옛 파일은 역할이 retired 라 쪽지를 모르는 쓰기도 여기서 막힌다.
        var file = c.QuerySingle<(string DatasetId, string Role)>("SELECT dataset_id, role FROM pclm_file", transaction: tx);
        if (file.Role != PclmRole.Work || home is not null && !home.PointsAt(databasePath, file.DatasetId))
            throw new InvalidOperationException("저장 대상이 바뀌었습니다. 다시 확인하세요.");
        var hash = Mapping.Hash(new { input, baseToken, choices });
        var receipt = c.QuerySingleOrDefault<(string Hash, string Result)>("SELECT request_hash,result_json FROM erp_capture WHERE capture_id=@captureId", new { captureId }, tx);
        if (receipt.Result is not null)
        {
            if (receipt.Hash != hash) throw new InvalidOperationException("같은 수집 ID에 다른 내용을 보낼 수 없습니다.");
            return JsonSerializer.Deserialize<JsonElement>(receipt.Result);
        }
        var preview = Inspect(c, tx, input); var doc = preview.Document;
        if (preview.BaseToken != baseToken) throw new InvalidOperationException("미리보기 이후 자료가 바뀌었습니다. 다시 확인하세요.");
        if (preview.Unmatched.Count > 0) throw new InvalidOperationException("기존 품목과 원천 행의 대응을 먼저 선택하세요.");
        foreach (var change in preview.Changes.Where(ch => ch.Conflict))
            if (!choices.TryGetValue(change.Id, out var choice) || choice is not ("keep" or "apply"))
                throw new InvalidOperationException("변경값과 사용자 정정값을 모두 검토하세요.");
        var now = DateTime.UtcNow.ToString("O");
        Write(c, tx, input, doc, preview.Changes, ch => choices.GetValueOrDefault(ch.Id), now);
        if (doc.EntityType == "contract")
        {
            var total = c.Query<string>("SELECT amount FROM contract_item WHERE contract_base=@Base AND seq=@Seq", doc, tx)
                .Sum(a => decimal.Parse(a ?? "0", System.Globalization.CultureInfo.InvariantCulture));
            var header = c.ExecuteScalar<string>("SELECT amount FROM contract WHERE contract_base=@Base AND seq=@Seq", doc, tx);
            if (decimal.Parse(header ?? "0", System.Globalization.CultureInfo.InvariantCulture) != decimal.Truncate(total / 10) * 10)
                throw new InvalidOperationException("선택한 품목 금액과 계약 총액이 다릅니다. 품목 유지·삭제와 총액 선택을 함께 확인하세요.");
        }
        foreach (var o in preview.Changes.Where(ch => ch.Field == "__override" && choices.GetValueOrDefault(ch.Id) == "apply"))
            c.Execute("DELETE FROM field_override WHERE entity_type=@EntityType AND base=@Base AND seq=@Seq AND column_name=@column", new { doc.EntityType, doc.Base, doc.Seq, column = o.Override }, tx);
        var changed = preview.Changes.Any(ch => choices.GetValueOrDefault(ch.Id) != "keep");
        if (changed) c.Execute($"UPDATE {doc.EntityType} SET updated_at=@now WHERE {doc.EntityType}_base=@Base AND seq=@Seq", new { doc.Base, doc.Seq, now }, tx);
        var previousDocument = c.QuerySingleOrDefault<string>("SELECT document_json FROM erp_source WHERE entity_type=@EntityType AND entity_base=@Base AND entity_seq=@Seq", doc, tx);
        var aggregate = previousDocument is null ? new List<MappedRow>() : JsonSerializer.Deserialize<MappedDocument>(previousDocument, Mapping.Json)!.Rows;
        var retained = c.Query<(string Table, string Key)>("SELECT table_name,source_key FROM erp_row WHERE entity_type=@EntityType AND entity_base=@Base AND entity_seq=@Seq", doc, tx).ToHashSet();
        aggregate.RemoveAll(r => r.Table != doc.EntityType && !retained.Contains((r.Table, r.SourceKey)));
        foreach (var row in doc.Rows)
        {
            var old = aggregate.FindIndex(r => r.Table == row.Table && r.SourceKey == row.SourceKey);
            if (old < 0) aggregate.Add(row);
            else
            {
                var values = new Dictionary<string, string?>(aggregate[old].Values);
                var origins = new Dictionary<string, string>(aggregate[old].Origins ?? new());
                foreach (var (k,v) in row.Values) { values[k] = v; origins[k] = doc.Profile; }
                aggregate[old] = row with { Values = values, Origins = origins };
            }
        }
        var documentJson = JsonSerializer.Serialize(doc with { Rows = aggregate }, Mapping.Json);
        c.Execute("INSERT INTO erp_source(entity_type,entity_base,entity_seq,document_json,mapping_revision,updated_at) VALUES(@EntityType,@Base,@Seq,@documentJson,@MappingRevision,@now) ON CONFLICT(entity_type,entity_base,entity_seq) DO UPDATE SET document_json=excluded.document_json,mapping_revision=excluded.mapping_revision,updated_at=excluded.updated_at",
            new { doc.EntityType, doc.Base, doc.Seq, documentJson, preview.MappingRevision, now }, tx);
        var links = ExplicitLinks.Resolve(c, tx).Where(r => r.Base == doc.Base || r.Target == doc.Base).ToArray();
        var raw = input with { Data = Mapping.Sanitize(new MappingStore(database).Active(), input.Profile, input.Data) };
        // screen: 읽은 화면의 번호 — 걸러 담은 data 에는 메뉴 경로가 남지 않아, 이력을 다시 검토할 때 이것을 싣는다(ADR-037).
        var snapshotJson = JsonSerializer.Serialize(new { raw.Profile, raw.MappingRevision, raw.Data, raw.Scope, raw.Totals, raw.RowMatches, choices,
            screen = ScreenOf(input) }, Mapping.Json);
        var result = new { captureId, datasetId = preview.DatasetId, status = "stored", entity = preview.Entity,
            entityType = doc.EntityType, changed, itemCount = preview.ItemCount, scope = input.Scope, links };
        c.Execute("INSERT INTO erp_capture(capture_id,request_hash,content_hash,profile,entity_base,entity_seq,snapshot_json,result_json,captured_at) VALUES(@captureId,@hash,@contentHash,@Profile,@Base,@Seq,@snapshotJson,@resultJson,@now)",
            new { captureId, hash, contentHash = Mapping.Hash(doc), doc.Profile, doc.Base, doc.Seq, snapshotJson, resultJson = JsonSerializer.Serialize(result), now }, tx);
        tx.Commit(); return result;
    }
    /// <summary>
    /// 업무 표에 수집값을 쓴다. 저장과, 덮개 아래 값이 바뀌는지 미리 보는 <see cref="Inspect(SqliteConnection, SqliteTransaction, CaptureInput)"/>
    /// 가 함께 쓴다 — 두 벌이면 미리 본 모습과 실제 저장이 갈린다. <c>field_override</c> 에는 쓰지 않는다(ADR-020).
    /// </summary>
    internal static void Write(SqliteConnection c, SqliteTransaction tx, CaptureInput input, MappedDocument doc,
        List<CaptureChange> changes, Func<CaptureChange, string?> choice, string now)
    {
        if (doc.EntityType == "notice") c.Execute("INSERT INTO notice_group(group_base) VALUES(@Base) ON CONFLICT DO NOTHING", doc, tx);
        c.Execute($"INSERT INTO {doc.EntityType}_series({doc.EntityType}_base{(doc.EntityType == "notice" ? ",group_base" : "")}) VALUES(@Base{(doc.EntityType == "notice" ? ",@Base" : "")}) ON CONFLICT DO NOTHING", doc, tx);
        c.Execute($"INSERT INTO {doc.EntityType}({doc.EntityType}_base,seq,updated_at) VALUES(@Base,@Seq,@now) ON CONFLICT DO NOTHING", new { doc.Base, doc.Seq, now }, tx);
        foreach (var row in doc.Rows)
        {
            var id = RowId(row);
            var line = changes.FirstOrDefault(ch => ch.Id.StartsWith(id + "/", StringComparison.Ordinal))?.Line ?? 0;
            if (row.Table != doc.EntityType && line == 0)
                line = c.ExecuteScalar<int>("SELECT line_no FROM erp_row WHERE entity_type=@EntityType AND entity_base=@Base AND entity_seq=@Seq AND table_name=@Table AND source_key=@SourceKey", new { doc.EntityType, doc.Base, doc.Seq, row.Table, row.SourceKey }, tx);
            if (row.Table != doc.EntityType)
            {
                if (line == 0) line = input.RowMatches?.GetValueOrDefault(id) ?? 0;
                if (line == 0) line = c.ExecuteScalar<int>($"SELECT coalesce(max(line_no),0)+1 FROM {row.Table} WHERE {doc.EntityType}_base=@Base AND seq=@Seq", doc, tx);
                c.Execute($"INSERT INTO {row.Table}({doc.EntityType}_base,seq,line_no) VALUES(@Base,@Seq,@line) ON CONFLICT DO NOTHING", new { doc.Base, doc.Seq, line }, tx);
                c.Execute("INSERT INTO erp_row(entity_type,entity_base,entity_seq,table_name,source_key,line_no) VALUES(@EntityType,@Base,@Seq,@Table,@SourceKey,@line) ON CONFLICT DO NOTHING", new { doc.EntityType, doc.Base, doc.Seq, row.Table, row.SourceKey, line }, tx);
            }
            foreach (var change in changes.Where(ch => ch.Table == row.Table && ch.Line == line && ch.Field is not ("__override" or "__row")))
            {
                if (choice(change) == "keep") continue;
                c.Execute($"UPDATE {row.Table} SET {change.Field}=@value WHERE {doc.EntityType}_base=@Base AND seq=@Seq" + (line == 0 ? "" : " AND line_no=@line"), new { doc.Base, doc.Seq, line, value = change.After }, tx);
            }
        }
        foreach (var removed in changes.Where(ch => ch.Field == "__row" && choice(ch) == "apply"))
        {
            c.Execute($"DELETE FROM {removed.Table} WHERE {doc.EntityType}_base=@Base AND seq=@Seq AND line_no=@Line", new { doc.Base, doc.Seq, removed.Line }, tx);
            c.Execute("DELETE FROM erp_row WHERE entity_type=@EntityType AND entity_base=@Base AND entity_seq=@Seq AND table_name=@Table AND line_no=@Line", new { doc.EntityType, doc.Base, doc.Seq, removed.Table, removed.Line }, tx);
        }
    }
}

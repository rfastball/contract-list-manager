using System.Diagnostics;
using System.Globalization;
using System.Text.Json;
using System.Text.Json.Nodes;
using Dapper;
using Microsoft.Data.Sqlite;
using Pclm.Core.Domain;
using Pclm.Core.Storage;
using Xunit.Abstractions;

namespace Pclm.Core.Tests;

/// <summary>
/// 가공식 v2 — 공유 사례표(<c>tests/flow-cases.json</c>, docs/column-management.md §11.7).
///
/// <para>화면의 vitest(<c>frontend/src/flow/cases.test.ts</c>)가 <b>같은 파일</b>을 TypeScript 꼴 검사·평가로 돌린다.
/// 여기서는 C# 꼴 검사와 SQL 컴파일러를 실제 SQLite 위에서 돌린다 — 두 쪽이 갈리면 한쪽이 붉어진다.
/// 값은 모두 지어낸 것이다.</para>
/// </summary>
public sealed class FlowCaseTests : IDisposable
{
    private readonly string _path = Path.Combine(Path.GetTempPath(), $"pclm-flow-{Guid.NewGuid():N}.db");
    private readonly Database _database;
    private readonly Store _store;
    private readonly ITestOutputHelper _output;
    private static readonly JsonObject Fixture = Load();

    public FlowCaseTests(ITestOutputHelper output)
    {
        _output = output;
        _database = PclmFile.Create(_path, PclmRole.Work);
        _store = new Store(_database);
    }

    public void Dispose()
    {
        SqliteConnection.ClearAllPools();
        foreach (var suffix in new[] { "", "-wal", "-shm" })
            File.Delete(_path + suffix);
    }

    private static JsonObject Load()
    {
        for (var directory = new DirectoryInfo(AppContext.BaseDirectory); directory is not null; directory = directory.Parent)
        {
            var candidate = Path.Combine(directory.FullName, "tests", "flow-cases.json");
            if (File.Exists(candidate)) return JsonNode.Parse(File.ReadAllText(candidate))!.AsObject();
        }
        throw new FileNotFoundException("tests/flow-cases.json 을 찾지 못했습니다.");
    }

    private static IEnumerable<JsonObject> Cases => Fixture["cases"]!.AsArray().Select(c => c!.AsObject());
    private static JsonObject Docs => Fixture["docs"]!.AsObject();

    public static IEnumerable<object[]> CaseNames() => Cases.Select(c => new object[] { c["name"]!.GetValue<string>() });

    private static JsonObject Case(string name) => Cases.Single(c => c["name"]!.GetValue<string>() == name);

    // ── 재료 ─────────────────────────────────────────────

    [Theory]
    [InlineData("contract")]
    [InlineData("notice")]
    [InlineData("request")]
    public void 사례표의_재료는_C샵이_쓰는_재료와_같다(string entity)
    {
        var env = Flow.EnvFor(entity);
        var expected = Fixture["env"]![entity]!;
        Assert.Equal(Strings(expected["base"]), env.Base);
        Assert.Equal(Strings(expected["items"]), env.Items);
        var linked = expected["linked"]!.AsObject();
        Assert.Equal(linked.Select(l => l.Key).Order(), env.Linked.Keys.Order());
        foreach (var (kind, link) in linked)
        {
            Assert.Equal(Strings(link!["fields"]), env.Linked[kind].Fields);
            Assert.Equal(Strings(link["items"]), env.Linked[kind].Items);
            Assert.Equal(link["many"]!.GetValue<bool>(), env.Linked[kind].Many);
        }
    }

    [Fact]
    public void 시험_문서는_면_뷰에_적힌_대로_찍힌다()
    {
        Seed();
        using var connection = _database.OpenReadOnly();
        foreach (var (id, node) in Docs)
        {
            var doc = node!.AsObject();
            var entity = doc["entity"]!.GetValue<string>();
            var row = (IDictionary<string, object>)connection.QuerySingle(
                $"SELECT * FROM \"{Store.FaceView(entity)}\" WHERE \"{Store.FaceKey(entity)}\" = @key", new { key = Key(doc) });
            foreach (var (field, value) in doc["fields"]!.AsObject())
                Assert.True(value!.GetValue<string>() == (string)row[field], $"{id}.{field}: {row[field]}");
        }
    }

    // ── 사례 ─────────────────────────────────────────────

    [Theory]
    [MemberData(nameof(CaseNames))]
    public void 사례(string name)
    {
        var c = Case(name);
        var entity = c["entity"]!.GetValue<string>();
        var formula = Flow.Parse(c["formula"]!.ToJsonString());

        if (c["migrated"] is { } migrated)
        {
            Assert.True(JsonNode.DeepEquals(migrated, JsonNode.Parse(Flow.Serialize(formula))), Flow.Serialize(formula));
            Assert.True(JsonNode.DeepEquals(migrated, JsonNode.Parse(MigrateWithSchemaStep(c["formula"]!.ToJsonString()))));
        }

        var checkedFlow = Flow.Check(formula.Flow!, Flow.EnvFor(entity));
        if (c["shapes"] is JsonArray shapes)
            Assert.Equal(shapes.Select(s => s?.GetValue<string>()), checkedFlow.Shapes);
        if (c["error"] is { } error)
        {
            Assert.False(checkedFlow.Ok);
            Assert.Equal(error["index"]!.GetValue<int>(), checkedFlow.Errors[0].Index);
            var refused = Assert.Throws<InvalidOperationException>(() => FlowCompiler.Expression(entity, formula));
            Assert.StartsWith(checkedFlow.Errors[0].Index == 0 ? "시작 데이터: " : $"{checkedFlow.Errors[0].Index}단계: ", refused.Message);
            return;
        }
        Assert.True(checkedFlow.Ok, string.Join(" / ", checkedFlow.Errors.Select(e => e.Describe())));

        Seed();
        var expression = FlowCompiler.Expression(entity, formula);
        var eject = FlowCompiler.Eject(entity, formula);
        using var connection = _database.OpenReadOnly();
        foreach (var (id, value) in c["final"]!.AsObject())
        {
            var doc = Docs[id]!.AsObject();
            Assert.Equal(entity, doc["entity"]!.GetValue<string>());
            var sql = $"FROM \"{Store.FaceView(entity)}\" AS {FlowCompiler.Self} WHERE {FlowCompiler.Self}.\"{Store.FaceKey(entity)}\" = @key";
            var inside = connection.QuerySingle<string>($"SELECT {expression} {sql}", new { key = Key(doc) });
            Assert.True(value!.GetValue<string>() == inside, $"{id}: 기대 {value} · 안쪽 SQL {inside}");
            // 「SQL로 꺼내기」 — 계약면 뷰만 읽는 두 번째 출력기. 최신 차수에서 같아야 한다.
            var outside = connection.QuerySingle<string>($"SELECT ({eject}) {sql}", new { key = Key(doc) });
            Assert.True(inside == outside, $"{id}: 안쪽 {inside} · 꺼낸 SQL {outside}");
        }
    }

    /// <summary>모든 사례를 실제 열로 저장하면 뷰가 같은 값을 낸다 — 감싼 뷰의 바깥 행 <c>현재</c> 를 거친다.</summary>
    [Fact]
    public void 사례를_가공열로_저장하면_뷰가_같은_값을_낸다()
    {
        Seed();
        var saved = new List<(string Column, string Entity, JsonObject Final)>();
        var i = 0;
        foreach (var c in Cases.Where(c => c["final"] is not null))
        {
            var entity = c["entity"]!.GetValue<string>();
            var column = $"사례{++i:D2}";
            _store.SaveComputedColumn(entity, column, c["formula"]!.ToJsonString());
            saved.Add((column, entity, c["final"]!.AsObject()));
        }

        using var connection = _database.OpenReadOnly();
        foreach (var (column, entity, final) in saved)
        foreach (var (id, value) in final)
        {
            var doc = Docs[id]!.AsObject();
            var views = entity switch
            {
                "notice" => new[] { ("v_공고", "입찰공고번호"), ("v_공고차수", "입찰공고번호") },
                "contract" => [("v_계약", "계약번호"), ("v_계약차수", "계약번호"), ("v_통합", "계약번호"), ("v_통합차수", "계약번호")],
                _ => [("v_접수", "접수번호")],
            };
            foreach (var (view, key) in views)
            {
                var got = connection.Query<string>($"SELECT \"{column}\" FROM {view} WHERE {key} = @key", new { key = Key(doc) }).First();
                Assert.True(value!.GetValue<string>() == got, $"{view}.{column} {id}: 기대 {value} · {got}");
            }
        }

        // 결과타입은 끝 꼴에서 온다.
        var types = connection.Query<(string Name, string Type)>("SELECT 열이름, 결과타입 FROM v_가공열").ToDictionary(t => t.Name, t => t.Type);
        Assert.Equal(saved.Count, types.Count);
        Assert.Contains("number", types.Values);
        Assert.Contains("date", types.Values);
        Assert.Contains("text", types.Values);
    }

    // ── 시험 DB ─────────────────────────────────────────────

    private static string Key(JsonObject doc)
    {
        var (b, s) = (doc["base"]!.GetValue<string>(), doc["seq"]!.GetValue<string>());
        return doc["entity"]!.GetValue<string>() == "contract" ? b + s : $"{b}-{s}";
    }

    private void Seed()
    {
        foreach (var (_, node) in Docs)
        {
            var doc = node!.AsObject();
            var f = doc["fields"]!.AsObject();
            var items = doc["items"]!.AsArray().Select(x => x!.AsObject()).ToList();
            string? T(JsonObject o, string key) => o[key]?.GetValue<string>() is { Length: > 0 } v ? v : null;
            DateTime? D(JsonObject o, string key) => T(o, key) is { } v ? DateTime.ParseExact(v, "yyyy/MM/dd", CultureInfo.InvariantCulture) : null;
            decimal? M(JsonObject o, string key) => T(o, key) is { } v ? decimal.Parse(v.Replace(",", ""), CultureInfo.InvariantCulture) : null;
            int? Q(JsonObject o, string key) => T(o, key) is { } v ? int.Parse(v, CultureInfo.InvariantCulture) : null;
            var (b, s) = (doc["base"]!.GetValue<string>(), doc["seq"]!.GetValue<string>());
            switch (doc["entity"]!.GetValue<string>())
            {
                case "notice":
                    _store.UpsertNotice(new NoticeRecord
                    {
                        NoticeBase = b, Seq = s, Title = T(f, "공고명"), PostedAt = D(f, "게시일시"), BasePrice = M(f, "기초금액"),
                        Items = [.. items.Select((x, i) => new NoticeItemRecord
                        {
                            LineNo = i + 1, ItemName = T(x, "세부품명"), DetailItemNumber = T(x, "세부품명번호"), ItemIdNumber = T(x, "물품식별번호"),
                            Specification = T(x, "규격"), DemandAgency = T(x, "수요기관"), Unit = T(x, "단위"), Quantity = Q(x, "수량"),
                            UnitPrice = M(x, "추정단가"), DeliveryPlace = T(x, "납품장소"), DeliveryTerms = T(x, "인도조건"),
                        })],
                    });
                    break;
                case "contract":
                    _store.UpsertContract(new ContractRecord
                    {
                        ContractBase = b, Seq = s, Title = T(f, "계약건명"), ContractedOn = D(f, "계약일자"), Amount = M(f, "계약금액"),
                        DeliveryDue = D(f, "납품기한"),
                        Items = [.. items.Select((x, i) => new ContractItemRecord
                        {
                            LineNo = i + 1, ItemName = T(x, "품명"), ClassificationNumber = T(x, "물품분류번호"), ItemIdNumber = T(x, "물품식별번호"),
                            Specification = T(x, "규격"), DemandAgency = T(x, "수요기관"), Unit = T(x, "단위"), Quantity = Q(x, "수량"),
                            UnitPrice = M(x, "단가"), Amount = M(x, "금액"), DeliveryTerms = T(x, "인도조건"),
                        })],
                    });
                    break;
                default:
                    _store.UpsertRequest(new RequestRecord
                    {
                        RequestBase = b, Seq = s, Title = T(f, "요청명"), ReceivedOn = D(f, "접수일자"), GoodsAmount = M(f, "품대"),
                        Items = [.. items.Select((x, i) => new RequestItemRecord
                        {
                            LineNo = i + 1, ItemName = T(x, "세부품명"), DetailItemNumber = T(x, "세부품명번호"), ItemIdNumber = T(x, "물품식별번호"),
                            Specification = T(x, "규격"), Unit = T(x, "단위"), Quantity = Q(x, "수량"), RequestNumber = T(x, "조달요구번호"),
                            UnitPrice = M(x, "단가"), Amount = M(x, "금액"), DeliveryPlace = T(x, "납품장소"), DeliveryTerms = T(x, "인도조건"),
                        })],
                    });
                    break;
            }
        }

        EntityRef Ref(string id)
        {
            var doc = Docs[id]!.AsObject();
            return new EntityRef(doc["entity"]!.GetValue<string>(), doc["base"]!.GetValue<string>(), doc["seq"]!.GetValue<string>());
        }
        foreach (var (id, node) in Docs)
        {
            var doc = node!.AsObject();
            var links = doc["links"]!.AsObject();
            switch (doc["entity"]!.GetValue<string>())
            {
                case "contract" when links["notice"] is { } notice:
                    ReferenceFixture.Contract(_database, Ref(id), Ref(notice.GetValue<string>()));
                    break;
                case "request" when links["notice"] is { } notice:
                    ReferenceFixture.Request(_database, Ref(id), Ref(notice.GetValue<string>()));
                    break;
            }
        }
    }

    /// <summary>스키마 22 의 SQL 이 v1 을 옮긴 결과. 세 벌(TS·C#·SQL)이 같은 v2 를 내야 한다.</summary>
    private static string MigrateWithSchemaStep(string v1)
    {
        using var connection = new SqliteConnection("Data Source=:memory:");
        connection.Open();
        connection.Execute("CREATE TABLE user_column (entity_type TEXT, field_name TEXT, kind TEXT, options TEXT, sort_order INTEGER);");
        connection.Execute("INSERT INTO user_column VALUES ('notice', 'x', 'computed', @v1, 10);", new { v1 });
        // v21 → v22 단계 — 뒤에 단계가 더해져도(v23·v24 맞춤 뷰) 그 자리다. Steps[0] 이 v19 → v20 이다.
        connection.Execute(Schema.Steps[21 - Schema.BaselineVersion]);
        return connection.QuerySingle<string>("SELECT options FROM user_column");
    }

    private static string[] Strings(JsonNode? node) => [.. node!.AsArray().Select(x => x!.GetValue<string>())];

    // ── 성능 ─────────────────────────────────────────────

    /// <summary>
    /// 계약 3,000건 × 가공열 10개(품목 나열·연결 필드 섞어)에서 <c>SELECT * FROM v_통합</c> 시간(§11.7).
    /// 숫자는 커밋 본문에 적는다. 10초를 넘으면 붉어진다 — 그 아래는 기계마다 다른 숫자라 막지 않는다.
    /// </summary>
    [Fact]
    public void 계약_3000건_가공열_10개의_통합_조회_시간()
    {
        const int contracts = 3000;
        using (var connection = _database.Open())
        using (var transaction = connection.BeginTransaction())
        {
            // 공고 하나에 계약 셋이 매달린다(분할 낙찰처럼). 건은 제 본번호다.
            for (var g = 0; g < contracts / 3; g++)
            {
                var notice = $"PERFN{g:D5}";
                connection.Execute("""
                    INSERT INTO notice_group (group_base) VALUES (@notice);
                    INSERT INTO notice_series (notice_base, group_base) VALUES (@notice, @notice);
                    INSERT INTO notice (notice_base, seq, title, posted_at, base_price, updated_at) VALUES (@notice, '000', @title, '2026-07-01', '12345000', '2026-10-08');
                    """, new { notice, title = $"성능 공고 {g}" }, transaction);
                for (var line = 1; line <= 3; line++)
                    connection.Execute("INSERT INTO notice_item (notice_base, seq, line_no, item_name, detail_item_number, quantity) VALUES (@notice, '000', @line, @name, @no, 2)",
                        new { notice, line, name = $"품목{line}", no = $"{line:D3}" }, transaction);
            }
            for (var k = 0; k < contracts; k++)
            {
                var contract = $"PERFK{k:D5}";
                connection.Execute("""
                    INSERT INTO contract_series (contract_base) VALUES (@contract);
                    INSERT INTO contract (contract_base, seq, title, contracted_on, amount, delivery_due, updated_at) VALUES (@contract, '00', @title, '2026-08-01', '1500000', '2026-09-20', '2026-10-08');
                    INSERT INTO project_link (contract_base, notice_group) VALUES (@contract, @notice);
                    """, new { contract, title = $"성능 계약 {k}", notice = $"PERFN{k / 3:D5}" }, transaction);
                for (var line = 1; line <= 4; line++)
                    connection.Execute("INSERT INTO contract_item (contract_base, seq, line_no, item_name, classification_number, quantity, unit_price, amount) VALUES (@contract, '00', @line, @name, @no, 3, '1000', '3000')",
                        new { contract, line, name = $"품목{line % 3}", no = $"{line:D3}" }, transaction);
            }
            transaction.Commit();
        }

        string Items(params object[] steps) => JsonSerializer.Serialize(new { version = 2, id = "", source = "flow", flow = new { from = new { kind = "items" }, steps } });
        string Linked(object from, params object[] steps) => JsonSerializer.Serialize(new { version = 2, id = "", source = "flow", flow = new { from, steps } });
        var formulas = new[]
        {
            Items(new { op = "distinct", fields = new[] { "품명", "물품분류번호" } }, new { op = "format", text = "{품명}({물품분류번호})" }, new { op = "join", text = ", " }),
            Items(new { op = "get", field = "품명" }, new { op = "distinct" }, new { op = "abbrev", n = 1, text = "{앞} 외 {나머지}종" }),
            Items(new { op = "sum", field = "금액" }, new { op = "thousands" }),
            Items(new { op = "count" }),
            Items(new { op = "sort", field = "단가", dir = "desc" }, new { op = "first" }, new { op = "get", field = "품명" }),
            Linked(new { kind = "linked", link = "notice", field = "기초금액" }),
            Linked(new { kind = "linked", link = "notice" }, new { op = "format", text = "{공고명}({입찰공고번호})" }),
            Linked(new { kind = "linked", link = "notice", items = true }, new { op = "get", field = "세부품명" }, new { op = "join", text = "/" }),
            Linked(new { kind = "linked", link = "notice", field = "게시일시" }, new { op = "toDate" }, new { op = "dateDiff", field = "계약일자" }),
            Linked(new { kind = "field", field = "계약금액" }, new { op = "toNumber" }, new { op = "div", value = 3 }, new { op = "round", n = 2 }),
        };
        long Time()
        {
            using var reader = _database.OpenReadOnly();
            var watch = Stopwatch.StartNew();
            _ = reader.Query("SELECT * FROM v_통합").ToList();
            return watch.ElapsedMilliseconds;
        }
        var before = Time();
        for (var i = 0; i < formulas.Length; i++) _store.SaveComputedColumn("contract", $"성능{i}", formulas[i]);

        using var reader = _database.OpenReadOnly();
        var watch = Stopwatch.StartNew();
        var rows = reader.Query("SELECT * FROM v_통합").ToList();
        watch.Stop();
        var report = $"PERF v_통합 {rows.Count}줄 × 가공열 {formulas.Length}개: {watch.ElapsedMilliseconds} ms (가공열 없이 {before} ms)";
        _output.WriteLine(report);
        Assert.True(rows.Count >= contracts);
        var sample = (IDictionary<string, object>)rows.First(r => ((IDictionary<string, object>)r)["계약번호"] as string == "PERFK0000000");
        Assert.Equal("12,345,000", sample["성능5"]);
        Assert.Equal("-31", sample["성능8"]);
        Assert.True(watch.Elapsed < TimeSpan.FromSeconds(10), $"{watch.ElapsedMilliseconds} ms");
    }
}

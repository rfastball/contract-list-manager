using System.Text.Json;
using System.Text.Json.Nodes;
using Dapper;
using Microsoft.Data.Sqlite;
using Pclm.Core.Domain;
using Pclm.Core.Storage;

namespace Pclm.Core.Tests;

/// <summary>
/// 가공열 v2 가 계약면에 서는 자리(docs/column-management.md §11). 식 하나하나의 값은 <see cref="FlowCaseTests"/> 가
/// 공유 사례표로 본다 — 여기서는 뷰를 감싸는 것, 차수, 정정, 저장 검증, 판올림을 본다. 값은 모두 지어낸 것이다.
/// </summary>
public sealed class FlowColumnTests : IDisposable
{
    private readonly string _path = Path.Combine(Path.GetTempPath(), $"pclm-flowcol-{Guid.NewGuid():N}.db");
    private readonly Database _database;
    private readonly Store _store;

    public FlowColumnTests()
    {
        _database = PclmFile.Create(_path, PclmRole.Work);
        _store = new Store(_database);
    }

    public void Dispose()
    {
        SqliteConnection.ClearAllPools();
        foreach (var suffix in new[] { "", "-wal", "-shm" })
            File.Delete(_path + suffix);
    }

    private static string Flow(object from, params object[] steps) =>
        JsonSerializer.Serialize(new { version = 2, id = "", source = "flow", flow = new { from, steps } });

    private static readonly object Items = new { kind = "items" };

    private string Read(string sql, object? args = null)
    {
        // 앱 함수를 등록하지 않은 독립 읽기 전용 연결이 소비자 경계다.
        using var consumer = new SqliteConnection($"Data Source={_path};Mode=ReadOnly;Pooling=False");
        consumer.Open();
        return consumer.QuerySingle<string>(sql, args);
    }

    private List<string> Columns(string view)
    {
        using var connection = _database.OpenReadOnly();
        return [.. connection.Query<string>($"SELECT name FROM pragma_table_info('{view}')")];
    }

    /// <summary>
    /// 기본 열 목록이 실제 면 뷰와 같다. 가공열이 있으면 이 목록으로 바깥을 다시 적으므로, 어긋나면 감싼 뷰에서
    /// 그 열만 사라진다 — <c>Views.ContractColumns</c> 와 같은 까닭이다.
    /// </summary>
    [Fact]
    public void 기본_열_목록이_면_뷰와_같다()
    {
        foreach (var entity in new[] { "notice", "contract", "request" })
        {
            var people = _store.UserColumns(entity).Select(c => c.FieldName);
            Assert.Equal([.. Views.BaseColumns(entity), .. people], Columns(Store.FaceView(entity)));
        }
    }

    /// <summary>
    /// 가공열이 있으면 뷰를 한 겹 감싸는데, <b>열 이름과 차례는 그대로</b>다 — 사람 열의 sort_order 가 입력열과
    /// 가공열을 섞어도 그 자리에 선다. 차수 뷰와 통합도 같은 이름을 낸다.
    /// </summary>
    [Fact]
    public void 감싼_뷰는_열_차례를_바꾸지_않는다()
    {
        _store.SaveComputedColumn("contract", "품목수", Flow(Items, new { op = "count" }));
        _store.AddUserColumn("contract", "비고", "text", []);
        _store.SaveComputedColumn("notice", "품목수", Flow(Items, new { op = "count" }));
        _store.SaveComputedColumn("request", "품목합계", Flow(Items, new { op = "sum", field = "금액" }));
        _store.MoveUserColumn("contract", "품목수", -1); // 진행상태(10)·품목수·메모 … 사이에 끼운다

        foreach (var entity in new[] { "notice", "contract", "request" })
        {
            var people = _store.UserColumns(entity).Select(c => c.FieldName).ToList();
            Assert.Equal([.. Views.BaseColumns(entity), .. people], Columns(Store.FaceView(entity)));
        }
        Assert.Equal(Columns("v_공고"), Columns("v_공고차수"));
        Assert.Equal(Columns("v_계약"), Columns("v_계약차수"));
        Assert.Equal([.. Columns("v_통합"), "공고건", "현행공고"], Columns("v_통합차수"));
        foreach (var column in Columns("v_계약")) Assert.Contains(column, Columns("v_통합"));

        // 감싸도 값은 모두 문자열이다.
        using var connection = _database.OpenReadOnly();
        foreach (var view in Views.Names)
        foreach (var (name, type) in connection.Query<(string, string)>($"SELECT name, type FROM pragma_table_info('{view}')"))
            Assert.True(name == "순번" || type is "TEXT" or "", $"{view}.{name}: {type}");
    }

    /// <summary>옛 차수 행은 그 차수의 품목으로 계산한다 — 품목 뷰가 최신 차수만 내므로 내부 표를 읽는다(§11.1-1).</summary>
    [Fact]
    public void 차수_뷰의_옛_줄은_제_차수의_품목으로_계산한다()
    {
        var first = new NoticeRecord { NoticeBase = "TESTOLD", Seq = "000", Items = [new() { LineNo = 1, ItemName = "책상" }, new() { LineNo = 2, ItemName = "의자" }] };
        _store.UpsertNotice(first);
        _store.UpsertNotice(first with { Seq = "001", Items = [new() { LineNo = 1, ItemName = "모니터" }] });
        _store.SaveComputedColumn("notice", "품명들", Flow(Items, new { op = "get", field = "세부품명" }, new { op = "join", text = ", " }));

        Assert.Equal("책상, 의자", Read("SELECT 품명들 FROM v_공고차수 WHERE 차수 = '000'"));
        Assert.Equal("모니터", Read("SELECT 품명들 FROM v_공고차수 WHERE 차수 = '001'"));
        Assert.Equal("모니터", Read("SELECT 품명들 FROM v_공고"));

        // 꺼낸 SQL 은 계약면 품목 뷰를 읽으므로 최신 차수 기준이다 — 옛 줄에서는 빈 값.
        var eject = FlowCompiler.Eject("notice", Pclm.Core.Storage.Flow.Parse(Flow(Items, new { op = "get", field = "세부품명" }, new { op = "join", text = ", " })));
        Assert.Contains("-- 1단계: 꺼내기", eject);
        Assert.Contains("AS 결과", eject);
        SqlColumn.CheckShape(eject);
        // 주석과 표시용 별칭이 추가돼도 기존 SQL 저장 검증과 공개 뷰 경계를 통과한다.
        _store.SaveComputedColumn("notice", "꺼낸품명", JsonSerializer.Serialize(new { version = 2, id = "", source = "sql", sql = eject }));
        Assert.Equal("모니터", Read("SELECT 꺼낸품명 FROM v_공고"));
        Assert.Equal("모니터", Read($"SELECT ({eject}) FROM v_공고차수 AS 현재 WHERE 차수 = '001'"));
        Assert.Equal("", Read($"SELECT ({eject}) FROM v_공고차수 AS 현재 WHERE 차수 = '000'"));
    }

    /// <summary>
    /// 자기 행의 열은 정정(<c>field_override</c>)을 마친 값이다(<c>현재."열"</c>). 연결 문서의 필드도 그 개체의 덮개
    /// 식을 다시 불러 읽으므로 정정이 비친다.
    /// </summary>
    [Fact]
    public void 정정한_값을_읽는다()
    {
        _store.UpsertNotice(new NoticeRecord { NoticeBase = "TESTN", Seq = "000", Title = "시험 공고", BasePrice = 1000m });
        _store.UpsertContract(new ContractRecord { ContractBase = "TESTK", Seq = "00", Title = "시험 계약" });
        ReferenceFixture.Contract(_database, new("contract", "TESTK", "00"), new("notice", "TESTN", "000"));
        _store.SaveComputedColumn("contract", "건명", Flow(new { kind = "field", field = "계약건명" }, new { op = "replace", text = "시험", to = "본" }));
        _store.SaveComputedColumn("contract", "공고기초", Flow(new { kind = "linked", link = "notice", field = "기초금액" }));
        Assert.Equal("본 계약", Read("SELECT 건명 FROM v_계약"));
        Assert.Equal("1,000", Read("SELECT 공고기초 FROM v_통합"));

        _store.SetOverride(new("contract", "TESTK", "00"), "계약건명", "시험 고친 계약");
        _store.SetOverride(new("notice", "TESTN", "000"), "기초금액", "2,000");
        Assert.Equal("본 고친 계약", Read("SELECT 건명 FROM v_계약"));
        Assert.Equal("2,000", Read("SELECT 공고기초 FROM v_계약"));

        // 가공열 칸은 여전히 고칠 수 없다.
        Assert.Throws<InvalidOperationException>(() => _store.SetOverride(new("contract", "TESTK", "00"), "건명", "수동값"));
        Assert.Throws<InvalidOperationException>(() => _store.SetUserField(new("contract", "TESTK", "00"), "건명", "수동값"));
    }

    /// <summary>
    /// 저장은 화면을 믿지 않고 다시 검사한다(§11.2). 꼴 오류는 단계 번호를 담고, 실패하면 아무것도 남지 않는다.
    /// v1 이 와도 받아 v2 로 담는다.
    /// </summary>
    [Fact]
    public void 저장은_다시_검사하고_단계_번호로_거절한다()
    {
        var error = Assert.Throws<InvalidOperationException>(() =>
            _store.SaveComputedColumn("contract", "실패열", Flow(Items, new { op = "count" }, new { op = "join", text = ", " })));
        Assert.StartsWith("2단계: ", error.Message);
        error = Assert.Throws<InvalidOperationException>(() =>
            _store.SaveComputedColumn("contract", "실패열", Flow(new { kind = "linked", link = "contract" })));
        Assert.StartsWith("시작 데이터: ", error.Message);
        error = Assert.Throws<InvalidOperationException>(() =>
            _store.SaveComputedColumn("contract", "실패열", Flow(Items, new { op = "distinct" })));
        Assert.StartsWith("1단계: ", error.Message);
        Assert.Throws<InvalidOperationException>(() =>
            _store.SaveComputedColumn("contract", "실패열", """{"version":2,"id":"","source":"sql","sql":"SELECT count(*) FROM contract_item"}"""));
        Assert.Throws<InvalidOperationException>(() =>
            _store.SaveComputedColumn("contract", "실패열", """{"version":2,"id":"","source":"flow","flow":{"from":{"kind":"items"},"steps":[{"op":"filter","cond":[{"field":"품명","cmp":"~"}]}]}}"""));
        Assert.DoesNotContain(_store.UserColumns(), c => c.FieldName == "실패열");
        _database.RefreshViews();

        _store.SaveComputedColumn("contract", "옛식", """{"version":1,"id":"","steps":[{"op":"pick","fields":["품명"]},{"op":"distinct"},{"op":"count"}]}""");
        var stored = JsonNode.Parse(_store.UserColumns("contract").Single(c => c.FieldName == "옛식").Options!)!;
        Assert.Equal(2, stored["version"]!.GetValue<int>());
        Assert.Equal("flow", stored["source"]!.GetValue<string>());
        Assert.NotEmpty(stored["id"]!.GetValue<string>());
        Assert.True(JsonNode.DeepEquals(JsonNode.Parse("""[{"op":"distinct","fields":["품명"]},{"op":"count"}]"""), stored["flow"]!["steps"]));
        Assert.Equal("number", Read("SELECT 결과타입 FROM v_가공열 WHERE 열이름 = '옛식'"));
        Assert.True(JsonNode.DeepEquals(stored, JsonNode.Parse(Read("SELECT 가공식 FROM v_가공열 WHERE 열이름 = '옛식'"))));
    }

    /// <summary>
    /// 경우 — 갈래마다 값 조건(빈 필드는 지금 값, 적힌 필드는 이 행의 열)이고 처음 맞는 것이 이긴다. 그외가 없으면 빈 글이다.
    /// 검사의 말은 화면 check.ts 와 같다(값은 공유 사례표가 실제 SQLite 로 본다).
    /// </summary>
    [Fact]
    public void 경우는_첫_갈래가_이기는_CASE_로_컴파일한다()
    {
        var formula = Pclm.Core.Storage.Flow.Parse(Flow(new { kind = "field", field = "계약건명" }, new
        {
            op = "cases",
            branches = new object[]
            {
                new { cond = new object[] { new { field = "", cmp = "안비어있음" } }, then = "있음" },
                new { cond = new object[] { new { field = "계약금액", cmp = ">", value = 10 } }, then = "큼" },
            },
        }));
        var eject = FlowCompiler.Eject("contract", formula);
        Assert.Contains("-- 1단계: 경우", eject);
        Assert.Contains("CASE WHEN (v <> '') THEN '있음' WHEN ((CASE WHEN", eject);
        Assert.Contains("현재.\"계약금액\"", eject);
        Assert.Contains("THEN '큼' ELSE '' END", eject);

        string? Problem(object step) => Pclm.Core.Storage.Flow.Check(
            Pclm.Core.Storage.Flow.Parse(Flow(new { kind = "field", field = "계약건명" }, step)).Flow!, Pclm.Core.Storage.Flow.EnvFor("contract")).Errors.FirstOrDefault()?.Message;
        Assert.Equal("경우를 하나 이상 적으세요", Problem(new { op = "cases", branches = Array.Empty<object>() }));
        Assert.Equal("2번째 경우: 이 행에 없는 열: 없는열", Problem(new { op = "cases", branches = new object[]
        {
            new { cond = new object[] { new { field = "", cmp = "비어있음" } }, then = "" },
            new { cond = new object[] { new { field = "없는열", cmp = "비어있음" } }, then = "" },
        } }));
        Assert.Equal("경우는 20개까지입니다", Problem(new { op = "cases", branches = Enumerable.Range(0, 21)
            .Select(_ => new { cond = new object[] { new { field = "", cmp = "비어있음" } }, then = "" }).ToArray() }));
        Assert.Throws<InvalidOperationException>(() => Pclm.Core.Storage.Flow.Parse(
            Flow(Items, new { op = "cases", branches = new object[] { new { then = "x" } } })));
    }

    /// <summary>앞부분 컴파일 — 단계 k 까지의 값을 SQL 로(보표가 쓴다). 뒤에 오류가 있어도 앞은 컴파일된다.</summary>
    [Fact]
    public void 앞부분만_컴파일한다()
    {
        _store.UpsertContract(new ContractRecord { ContractBase = "TESTK", Seq = "00", Items = [
            new() { LineNo = 1, ItemName = "책상" }, new() { LineNo = 2, ItemName = "책상" }, new() { LineNo = 3, ItemName = "의자" }] });
        var flow = Pclm.Core.Storage.Flow.Parse(Flow(Items, new { op = "get", field = "품명" }, new { op = "distinct" }, new { op = "count" }, new { op = "join", text = "," })).Flow!;
        var at = (int k) => FlowCompiler.Prefix("contract", flow, k);
        Assert.Equal("rows", at(0).Shape);
        Assert.Equal("3", Read($"SELECT (SELECT count(*) FROM ({at(0).Sql})) FROM v_계약 AS 현재"));
        Assert.Equal("values", at(2).Shape);
        Assert.Equal("책상,의자", Read($"SELECT (SELECT group_concat(__value, ',' ORDER BY __order) FROM ({at(2).Sql})) FROM v_계약 AS 현재"));
        Assert.Equal("number", at(3).Shape);
        Assert.Equal("2", Read($"SELECT CAST({at(3).Sql} AS TEXT) FROM v_계약 AS 현재"));
        Assert.StartsWith("4단계: ", Assert.Throws<InvalidOperationException>(() => at(4)).Message);
    }

    /// <summary>
    /// 스키마 22 — 저장된 v1 가공식을 v2 로 옮겨 쓴다. 다른 사람 열과 손으로 적은 값은 그대로이고, 옮긴 식이
    /// 앞 판과 같은 값을 낸다.
    /// </summary>
    [Fact]
    public void 판올림은_v1_가공식을_v2_로_옮긴다()
    {
        var path = Path.Combine(Path.GetTempPath(), $"pclm-v21-{Guid.NewGuid():N}.db");
        try
        {
            using (var old = new SqliteConnection($"Data Source={path};Pooling=False"))
            {
                old.Open();
                using var command = old.CreateCommand();
                command.CommandText = Schema.Baseline + Schema.Steps[0] + Schema.Steps[1] + """
                    INSERT INTO user_column VALUES ('notice', '물품요약', 'computed',
                      '{"version":1,"id":"keep","steps":[{"op":"pick","fields":["세부품명","세부품명번호"],"text":null,"index":null},{"op":"distinct","fields":null,"text":null,"index":null},{"op":"format","fields":null,"text":"{세부품명}({세부품명번호})","index":null},{"op":"join","fields":null,"text":", ","index":null}]}', 30);
                    PRAGMA user_version=21;
                    """;
                command.ExecuteNonQuery();
            }
            var database = new Database(path);
            database.Migrate();
            var store = new Store(database);
            var column = store.UserColumns("notice").Single(c => c.FieldName == "물품요약");
            Assert.Equal("computed", column.Kind);
            Assert.Equal(
                """{"version":2,"id":"keep","source":"flow","flow":{"from":{"kind":"items"},"steps":[{"op":"distinct","fields":["세부품명","세부품명번호"]},{"op":"format","text":"{세부품명}({세부품명번호})"},{"op":"join","text":", "}]}}""",
                column.Options);
            Assert.Equal(["검토여부", "메모", "물품요약"], store.UserColumns("notice").Select(c => c.FieldName));

            store.UpsertNotice(new NoticeRecord { NoticeBase = "TESTN", Seq = "000", Items = [
                new() { LineNo = 3, ItemName = "책상", DetailItemNumber = "002" },
                new() { LineNo = 1, ItemName = "책상", DetailItemNumber = "001" },
                new() { LineNo = 2, ItemName = "책상", DetailItemNumber = "001" }] });
            using var connection = database.OpenReadOnly();
            Assert.Equal("책상(001), 책상(002)", connection.QuerySingle<string>("SELECT 물품요약 FROM v_공고"));
            Assert.Equal("keep", connection.QuerySingle<string>("SELECT 식별자 FROM v_가공열"));
        }
        finally
        {
            SqliteConnection.ClearAllPools();
            foreach (var suffix in new[] { "", "-wal", "-shm" }) File.Delete(path + suffix);
        }
    }
}

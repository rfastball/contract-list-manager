using System.Text.Json;
using System.Text.Json.Nodes;
using Dapper;
using Microsoft.Data.Sqlite;
using Pclm.Core.Domain;
using Pclm.Core.Storage;

namespace Pclm.Core.Tests;

/// <summary>
/// 열 관리의 다리 쪽(docs/column-management.md §11.4·§11.5) — SQL 열과 그 검증, 깨진 열의 격리, 값 흐름 보표,
/// 값으로 굳히기. 값은 모두 지어낸 것이다.
/// </summary>
public sealed class ColumnBridgeTests : IDisposable
{
    private readonly string _path = Path.Combine(Path.GetTempPath(), $"pclm-colbridge-{Guid.NewGuid():N}.db");
    private readonly Database _database;
    private readonly Store _store;

    public ColumnBridgeTests()
    {
        _database = PclmFile.Create(_path, PclmRole.Work);
        _store = new Store(_database);
        _store.UpsertNotice(new NoticeRecord
        {
            NoticeBase = "TESTN1", Seq = "000", Title = "시험 공고 하나",
            Items = [
                new() { LineNo = 1, ItemName = "책상", DetailItemNumber = "001" },
                new() { LineNo = 2, ItemName = "책상", DetailItemNumber = "001" },
                new() { LineNo = 3, ItemName = "의자", DetailItemNumber = "" }],
        });
        _store.UpsertNotice(new NoticeRecord { NoticeBase = "TESTN2", Seq = "000", Title = "시험 공고 둘", Items = [new() { LineNo = 1, ItemName = "모니터", DetailItemNumber = "003" }] });
        _store.UpsertContract(new ContractRecord { ContractBase = "TESTK1", Seq = "00", Title = "시험 계약", Items = [new() { LineNo = 1, ItemName = "책상" }] });
    }

    public void Dispose()
    {
        SqliteConnection.ClearAllPools();
        foreach (var suffix in new[] { "", "-wal", "-shm" })
            File.Delete(_path + suffix);
    }

    private static string Sql(string sql) => JsonSerializer.Serialize(new { version = 2, id = "", source = "sql", sql });

    private static string Flow(object from, params object[] steps) =>
        JsonSerializer.Serialize(new { version = 2, id = "", source = "flow", flow = new { from, steps } });

    private static readonly object Items = new { kind = "items" };

    private string Read(string sql)
    {
        using var consumer = new SqliteConnection($"Data Source={_path};Mode=ReadOnly;Pooling=False");
        consumer.Open();
        return consumer.QuerySingle<string>(sql);
    }

    private static JsonElement Json(object value) => JsonSerializer.SerializeToElement(value);

    private string Refused(string entity, string sql) =>
        Assert.Throws<InvalidOperationException>(() => _store.SaveComputedColumn(entity, "거절열", Sql(sql))).Message;

    // ── SQL 열 ─────────────────────────────────────────────

    [Fact]
    public void SQL_열은_계약면_뷰를_읽어_뷰에_선다()
    {
        _store.SaveComputedColumn("notice", "품명들", Sql("""
            SELECT group_concat(세부품명, ', ' ORDER BY 순번) FROM v_공고품목 WHERE 입찰공고번호 = 현재.입찰공고번호
            """));
        Assert.Equal("책상, 책상, 의자", Read("SELECT 품명들 FROM v_공고 WHERE 입찰공고번호 = 'TESTN1-000'"));
        Assert.Equal("모니터", Read("SELECT 품명들 FROM v_공고차수 WHERE 입찰공고번호 = 'TESTN2-000'"));
        Assert.Equal("text", Read("SELECT 결과타입 FROM v_가공열 WHERE 열이름 = '품명들'"));
        // 수를 내는 SQL 도 글로 찍히고, 없는 값은 NULL 이 아니라 빈 글이다.
        _store.SaveComputedColumn("notice", "품목수", Sql("SELECT count(*) FROM v_공고품목 WHERE 입찰공고번호 = 현재.입찰공고번호"));
        _store.SaveComputedColumn("notice", "없는값", Sql("SELECT 세부품명 FROM v_공고품목 WHERE 입찰공고번호 = '없음'"));
        Assert.Equal("3", Read("SELECT 품목수 FROM v_공고 WHERE 입찰공고번호 = 'TESTN1-000'"));
        Assert.Equal("", Read("SELECT 없는값 FROM v_공고 WHERE 입찰공고번호 = 'TESTN1-000'"));
        // 문서용 조합은 기본 열 대신 사용자가 정의한 SQL 열로 만든다.
        _store.SetOverride(new EntityRef("notice", "TESTN1", "000"), "계약방법", "일반경쟁");
        _store.SetOverride(new EntityRef("notice", "TESTN1", "000"), "계약구분", "총액계약");
        _store.SaveComputedColumn("notice", "입찰방법", Sql("""
            SELECT substr(현재.계약방법, 1, 2) || '(' || replace(현재.계약구분, '계약', '') || ')'
            """));
        Assert.Equal("일반(총액)", Read("SELECT 입찰방법 FROM v_공고 WHERE 입찰공고번호 = 'TESTN1-000'"));
        Assert.Equal("일반(총액)", Read("SELECT 입찰방법 FROM v_공고차수 WHERE 입찰공고번호 = 'TESTN1-000'"));
        Assert.Empty(_store.BrokenColumns());
    }

    [Fact]
    public void SQL_열_검증은_읽기_밖의_것을_거절한다()
    {
        Assert.Contains("공개 뷰", Refused("notice", "SELECT count(*) FROM notice_item"));
        Assert.Contains("공개 뷰", Refused("notice", "SELECT count(*) FROM v_가공열"));
        Assert.Contains("공개 뷰", Refused("notice", "SELECT count(*) FROM v_ERP원천"));
        Assert.Contains("sqlite_master", Refused("notice", "SELECT count(*) FROM \"sqlite_master\""));
        Assert.Contains("함수", Refused("notice", "SELECT load_extension('x')"));
        Assert.Contains("pragma_table_info", Refused("notice", "SELECT name FROM pragma_table_info('notice')"));
        // 제 CTE 를 세우면 그 안의 읽기가 뷰 안쪽으로 보고되므로 WITH 는 받지 않는다.
        Assert.Contains("WITH", Refused("notice",
            "SELECT (WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM c LIMIT 5) SELECT count(*) FROM c)"));
        Assert.Contains("WITH", Refused("notice", "SELECT (with 사슬 AS (SELECT * FROM notice_item) SELECT count(*) FROM 사슬)"));
        // 괄호 밖으로 새거나 문장을 잇는 글은 준비하기 전에 막는다 — 뷰 정의는 여러 문장을 한 번에 돌린다.
        Assert.Contains("하나만", Refused("notice", "SELECT 1; DELETE FROM notice"));
        Assert.Contains("괄호", Refused("notice", "SELECT 1) AS x FROM v_공고; DROP TABLE notice; SELECT (1"));
        Assert.Contains("SELECT", Refused("notice", "DELETE FROM notice"));
        Assert.Contains("주석", Refused("notice", "SELECT 1 /* 닫지 않음"));
        // 한 행이 값을 여럿 내면 표본 실행이 거절한다.
        Assert.Contains("2개", Refused("notice", "SELECT DISTINCT 세부품명 FROM v_공고품목 WHERE 입찰공고번호 = 현재.입찰공고번호"));
        // 여러 열은 준비에서 걸린다.
        Assert.Throws<InvalidOperationException>(() => _store.SaveComputedColumn("notice", "거절열", Sql("SELECT 1, 2")));
        Assert.DoesNotContain(_store.UserColumns(), c => c.FieldName == "거절열");
        Assert.Equal("2", Read("SELECT count(*) FROM notice"));
    }

    [Fact]
    public void 계약면_뷰는_모두_읽을_수_있다()
    {
        // 뷰 안쪽의 읽기(내부 표·다른 계약면 뷰)를 authorizer 가 바깥 읽기로 잘못 거절하지 않는다.
        foreach (var view in SqlColumn.Readable)
        {
            var result = Json(_store.ValidateComputedSql("contract", $"SELECT count(*) FROM {view}"));
            Assert.True(result.GetProperty("ok").GetBoolean(), $"{view}: {result.GetProperty("error")}");
        }
        var refused = Json(_store.ValidateComputedSql("contract", "SELECT count(*) FROM contract_item"));
        Assert.False(refused.GetProperty("ok").GetBoolean());
        Assert.Contains("contract_item", refused.GetProperty("error").GetString());
    }

    [Fact]
    public void 뷰를_도는_SQL_열은_저장하지_않는다()
    {
        // v_통합 은 v_계약 을 받으므로, 계약 열이 v_통합 을 읽으면 뷰가 제 자신을 부른다.
        var error = Refused("contract", "SELECT count(*) FROM v_통합 WHERE 계약번호 = 현재.계약번호");
        Assert.Contains("circularly", error);
        Assert.DoesNotContain(_store.UserColumns(), c => c.FieldName == "거절열");
        Assert.Equal("3", Read("SELECT count(*) FROM v_통합"));
    }

    // ── 깨진 열 ─────────────────────────────────────────────

    [Fact]
    public void 깨진_정의는_뷰_짓기를_막지_않고_빈_값이_된다()
    {
        _store.SaveComputedColumn("notice", "품목수", Sql("SELECT count(*) FROM v_공고품목 WHERE 입찰공고번호 = 현재.입찰공고번호"));
        _store.SaveComputedColumn("notice", "흐름", Flow(Items, new { op = "count" }));
        _store.SaveComputedColumn("contract", "계약품목수", Flow(Items, new { op = "count" }));
        // 판올림이 뷰 이름을 바꾸었거나 손으로 고친 파일 — 저장된 SQL 이 없는 뷰를 읽는다.
        using (var connection = _database.Open())
        {
            connection.Execute("UPDATE user_column SET options = @options WHERE field_name = '품목수'",
                new { options = Sql("SELECT count(*) FROM v_없는뷰 WHERE 입찰공고번호 = 현재.입찰공고번호") });
            connection.Execute("UPDATE user_column SET options = '{\"version\":2' WHERE field_name = '계약품목수'");
        }
        _database.RefreshViews();

        Assert.Equal("", Read("SELECT 품목수 FROM v_공고 WHERE 입찰공고번호 = 'TESTN1-000'"));
        Assert.Equal("", Read("SELECT 품목수 FROM v_공고차수 WHERE 입찰공고번호 = 'TESTN1-000'"));
        Assert.Equal("3", Read("SELECT 흐름 FROM v_공고 WHERE 입찰공고번호 = 'TESTN1-000'"));
        Assert.Equal("", Read("SELECT 계약품목수 FROM v_통합 WHERE 계약번호 = 'TESTK100'"));
        foreach (var view in Views.Names) Assert.NotNull(Read($"SELECT CAST(count(*) AS TEXT) FROM {view}"));

        var broken = _store.BrokenColumns();
        Assert.Equal(2, broken.Count);
        Assert.Contains("v_없는뷰", broken[("notice", "품목수")]);
        Assert.Contains("형식", broken[("contract", "계약품목수")]);
        Assert.Equal("text", Read("SELECT 결과타입 FROM v_가공열 WHERE 열이름 = '품목수'"));

        // 다른 열을 저장하는 일도 막히지 않는다. 깨진 열을 고쳐 저장하면 깨짐이 걷힌다.
        _store.SaveComputedColumn("notice", "또하나", Flow(Items, new { op = "count" }));
        _store.SaveComputedColumn("notice", "품목수", Sql("SELECT count(*) FROM v_공고품목 WHERE 입찰공고번호 = 현재.입찰공고번호"), "품목수");
        Assert.Equal("3", Read("SELECT 품목수 FROM v_공고 WHERE 입찰공고번호 = 'TESTN1-000'"));
        Assert.Equal([("contract", "계약품목수")], _store.BrokenColumns().Keys);
    }

    // ── 보표·꺼내기 ─────────────────────────────────────────────

    [Fact]
    public void 보표는_단계마다_표본_값을_낸다()
    {
        var trace = Json(_store.TraceComputedColumn("notice", Flow(Items,
            new { op = "distinct", fields = new[] { "세부품명", "세부품명번호" } },
            new { op = "format", text = "{세부품명}({세부품명번호})" },
            new { op = "join", text = ", " })));
        Assert.Equal(2, trace.GetProperty("samples").GetArrayLength());
        Assert.Equal("TESTN1", trace.GetProperty("samples")[0].GetProperty("base").GetString());
        var steps = trace.GetProperty("steps");
        Assert.Equal(4, steps.GetArrayLength());

        var source = steps[0].GetProperty("values")[0];
        Assert.Equal("rows", steps[0].GetProperty("shape").GetString());
        Assert.Equal("list", source.GetProperty("kind").GetString());
        Assert.Equal(3, source.GetProperty("count").GetInt32());
        Assert.Equal("책상 · 001", source.GetProperty("items")[0].GetString());
        Assert.Equal("001", source.GetProperty("rows")[0].GetProperty("세부품명번호").GetString());

        var distinct = steps[1].GetProperty("values")[0];
        Assert.Equal(2, distinct.GetProperty("count").GetInt32());
        Assert.Equal(1, distinct.GetProperty("dropped").GetInt32());

        var format = steps[2].GetProperty("values")[0];
        Assert.Equal("values", steps[2].GetProperty("shape").GetString());
        Assert.Equal(["책상(001)", "의자()"], format.GetProperty("items").EnumerateArray().Select(x => x.GetString()));
        Assert.Equal(1, format.GetProperty("blank").GetInt32());

        Assert.Equal("책상(001), 의자()", steps[3].GetProperty("values")[0].GetProperty("text").GetString());
        Assert.Equal(["책상(001), 의자()", "모니터(003)"], trace.GetProperty("final").EnumerateArray().Select(x => x.GetString()));
        Assert.Equal(trace.GetProperty("sql").GetString(),
            _store.CompileComputedColumn("notice", Flow(Items,
                new { op = "distinct", fields = new[] { "세부품명", "세부품명번호" } },
                new { op = "format", text = "{세부품명}({세부품명번호})" },
                new { op = "join", text = ", " })));
    }

    [Fact]
    public void 보표의_단계_오류는_그_단계에_붙고_뒤는_막힌다()
    {
        var trace = Json(_store.TraceComputedColumn("notice", Flow(Items,
            new { op = "count" }, new { op = "join", text = "," }, new { op = "trim" })));
        var steps = trace.GetProperty("steps");
        Assert.Equal("number", steps[1].GetProperty("shape").GetString());
        Assert.Equal("3", steps[1].GetProperty("values")[0].GetProperty("text").GetString());
        Assert.Contains("잇기", steps[2].GetProperty("error").GetString());
        Assert.True(steps[3].GetProperty("blocked").GetBoolean());
        Assert.False(steps[1].TryGetProperty("blocked", out _));
        Assert.Equal(["", ""], trace.GetProperty("final").EnumerateArray().Select(x => x.GetString()));
        Assert.Equal("", trace.GetProperty("sql").GetString());

        // SQL 열은 원천 한 줄에 표본 결과를 싣는다.
        var sql = Json(_store.TraceComputedColumn("notice", Sql("SELECT count(*) FROM notice_item")));
        Assert.Contains("공개 뷰", sql.GetProperty("steps")[0].GetProperty("error").GetString());
        var ok = Json(_store.TraceComputedColumn("notice", Sql("SELECT count(*) FROM v_공고품목 WHERE 입찰공고번호 = 현재.입찰공고번호")));
        Assert.Equal(["3", "1"], ok.GetProperty("final").EnumerateArray().Select(x => x.GetString()));
    }

    [Fact]
    public void 열_맥락은_원천으로_쓸_재료를_낸다()
    {
        var context = Json(_store.ColumnContext("notice"));
        var available = context.GetProperty("available");
        Assert.Equal("v_공고", available.GetProperty("view").GetString());
        Assert.Equal(Views.BaseColumns("notice"), available.GetProperty("base").EnumerateArray().Select(x => x.GetString()));
        var linked = available.GetProperty("linked").EnumerateArray().ToDictionary(l => l.GetProperty("kind").GetString()!);
        Assert.True(linked["contract"].GetProperty("many").GetBoolean());
        Assert.False(linked["request"].GetProperty("many").GetBoolean());
        Assert.Equal(SqlColumn.Readable.Order(), available.GetProperty("schema").EnumerateObject().Select(p => p.Name).Order());
        Assert.Contains("세부품명", available.GetProperty("schema").GetProperty("v_공고품목").EnumerateArray().Select(x => x.GetString()));
    }

    // ── 값으로 굳히기 ─────────────────────────────────────────────

    [Fact]
    public void 굳히기는_센_만큼_계열의_최신_값을_쓰고_식을_남긴다()
    {
        // 옛 차수가 있는 계열 — 굳히는 값은 최신 차수의 것이다.
        _store.UpsertNotice(new NoticeRecord { NoticeBase = "TESTN1", Seq = "001", Title = "시험 공고 하나 정정", Items = [new() { LineNo = 1, ItemName = "책상", DetailItemNumber = "001" }] });
        _store.UpsertNotice(new NoticeRecord { NoticeBase = "TESTN3", Seq = "000", Title = "품목 없음" });
        _store.AddUserColumn("notice", "요약", "text", []);
        _store.SetUserField(new("notice", "TESTN2", "000"), "요약", "옛 손값");
        _store.SetUserField(new("notice", "TESTN3", "000"), "요약", "숨을 값");
        var hiding = _store.PlanFreezeColumn("notice", "요약");
        Assert.Equal(2, hiding.Hidden);
        Assert.Equal(0, hiding.Cells);

        _store.SaveComputedColumn("notice", "요약", Flow(Items, new { op = "get", field = "세부품명" }, new { op = "join", text = "/" }), "요약");
        var id = JsonNode.Parse(_store.UserColumns("notice").Single(c => c.FieldName == "요약").Options!)!["id"]!.GetValue<string>();
        Assert.Equal("모니터", Read("SELECT 요약 FROM v_공고 WHERE 입찰공고번호 = 'TESTN2-000'"));

        var plan = _store.PlanFreezeColumn("notice", "요약");
        Assert.Equal(3, plan.Series);
        Assert.Equal(2, plan.Cells);   // TESTN3 은 품목이 없어 빈 값
        Assert.Equal(2, plan.Hidden);
        Assert.Throws<InvalidOperationException>(() => _store.FreezeColumn("notice", "요약", plan.Cells + 1));
        Assert.Equal(plan.Cells, _store.FreezeColumn("notice", "요약", plan.Cells));

        var column = _store.UserColumns("notice").Single(c => c.FieldName == "요약");
        Assert.Equal("text", column.Kind);
        Assert.Equal(id, JsonNode.Parse(column.Options!)!["id"]!.GetValue<string>());
        Assert.Equal("책상", Read("SELECT 요약 FROM v_공고 WHERE 입찰공고번호 = 'TESTN1-001'"));
        Assert.Equal("모니터", Read("SELECT 요약 FROM v_공고 WHERE 입찰공고번호 = 'TESTN2-000'"));
        Assert.Equal("", Read("SELECT 요약 FROM v_공고 WHERE 입찰공고번호 = 'TESTN3-000'"));
        Assert.Equal("2", Read("SELECT count(*) FROM notice_user_field WHERE field_name = '요약'"));

        // 굳힌 값은 이제 사람이 고칠 수 있다. 다시 켜면 같은 식별자로 돌아온다.
        _store.SetUserField(new("notice", "TESTN2", "000"), "요약", "손으로");
        _store.UpdateUserColumn("notice", "요약", "요약", "text", []);
        Assert.Equal(id, JsonNode.Parse(_store.UserColumns("notice").Single(c => c.FieldName == "요약").Options!)!["id"]!.GetValue<string>());
        _store.SaveComputedColumn("notice", "요약", Flow(Items, new { op = "count" }), "요약");
        Assert.Equal(id, Read("SELECT 식별자 FROM v_가공열 WHERE 열이름 = '요약'"));
        Assert.Equal("1", Read("SELECT 요약 FROM v_공고 WHERE 입찰공고번호 = 'TESTN2-000'"));
    }
}

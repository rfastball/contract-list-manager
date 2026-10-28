using System.Text.Json;
using System.Text.Json.Nodes;
using Dapper;
using Microsoft.Data.Sqlite;
using Pclm.Core.Domain;
using Pclm.Core.Storage;
using Xunit;

namespace Pclm.Core.Tests;

/// <summary>
/// 맞춤 뷰(docs/user-views.md) — 조달 건마다 한 줄, 접수·공고·계약의 필드를 늘어놓은 <c>u_*</c> 뷰. 저장 왕복, 줄이 서는 꼴(사슬·
/// 거르기·모으기), 깨진 필드, 사람 열과의 맞물림, 미리보기·표본·보표, 판올림, 취합, 메타 뷰를 본다. 번호·건명은 모두 지어낸 것이다.
/// </summary>
public sealed class UserViewTests : IDisposable
{
    private readonly string _path = Path.Combine(Path.GetTempPath(), $"pclm-userview-{Guid.NewGuid():N}.db");
    private readonly Database _database;
    private readonly Store _store;

    private const string N1 = "TESTN1-000", N2 = "TESTN2-000";

    public UserViewTests()
    {
        _database = PclmFile.Create(_path, PclmRole.Work);
        _store = new Store(_database);
        // 빈 작업자료는 서식용 예시 보기를 들고 선다. 여기 시험들은 빈 보기 목록에서 시작한다 — 예시는 따로 본다.
        foreach (var view in _store.UserViews()) _store.DeleteUserView(view.Id);
    }

    public void Dispose()
    {
        SqliteConnection.ClearAllPools();
        foreach (var suffix in new[] { "", "-wal", "-shm" })
            File.Delete(_path + suffix);
    }

    /// <summary>
    /// 사슬 여섯 — (접수1, 공고1, 계약1)·(접수1, 공고1, 계약2) 는 계약이 둘인 건이고, 공고2 는 혼자, 접수2 는 공고가 아직 없고,
    /// 계약3·계약4 는 어디에도 매달리지 못했다. 계약2 를 먼저 넣는다 — 모으기의 차례가 넣은 차례가 아니라 계약본번호 차례인지 본다.
    /// </summary>
    private static void Seed(Database database)
    {
        var store = new Store(database);
        store.UpsertNotice(new NoticeRecord { NoticeBase = "TESTN1", Seq = "000", Title = "시험 공고 하나",
            Items = [new() { LineNo = 1, ItemName = "책상" }, new() { LineNo = 2, ItemName = "의자" }] });
        store.UpsertNotice(new NoticeRecord { NoticeBase = "TESTN2", Seq = "000", Title = "시험 공고 둘" });
        store.UpsertContract(new ContractRecord { ContractBase = "TESTK2", Seq = "00", Title = "시험 계약 둘",
            Items = [new() { LineNo = 1, ItemName = "의자", Quantity = 3, UnitPrice = 1000, Amount = 3000 }] });
        store.UpsertContract(new ContractRecord { ContractBase = "TESTK1", Seq = "00", Title = "시험 계약 하나",
            Items = [new() { LineNo = 1, ItemName = "책상", Quantity = 1, UnitPrice = 5000, Amount = 5000 },
                     new() { LineNo = 2, ItemName = "책상", Quantity = 1, UnitPrice = 5000, Amount = 5000 }] });
        store.UpsertContract(new ContractRecord { ContractBase = "TESTK3", Seq = "00", Title = "시험 계약 셋" });
        store.UpsertContract(new ContractRecord { ContractBase = "TESTK4", Seq = "00", Title = "시험 계약 넷" });
        store.UpsertRequest(new RequestRecord { RequestBase = "TESTR1", Seq = "000", Title = "시험 접수 하나" });
        store.UpsertRequest(new RequestRecord { RequestBase = "TESTR2", Seq = "000", Title = "시험 접수 둘" });
        ReferenceFixture.Contract(database, new("contract", "TESTK2", "00"), new("notice", "TESTN1", "000"));
        ReferenceFixture.Contract(database, new("contract", "TESTK1", "00"), new("notice", "TESTN1", "000"));
        ReferenceFixture.Request(database, new("request", "TESTR1", "000"), new("notice", "TESTN1", "000"));
        store.SetUserField(new("notice", "TESTN1", "000"), "메모", "확인함");
    }

    private void Seed() => Seed(_database);

    // ── 지어내는 도구 ─────────────────────────────────────────

    private static object FlowOf(object from, params object[] steps) => new { version = 2, source = "flow", flow = new { from, steps } };

    /// <summary>문서 하나의 열 — <c>공고.공고명</c>.</summary>
    private static object Doc(string link, string field) => FlowOf(new { kind = "linked", link, field });

    private static object Sql(string sql) => new { version = 2, source = "sql", sql };

    private static string View(string name, string contracts, params (string? Id, string Name, object Formula)[] columns) =>
        JsonSerializer.Serialize(new
        {
            name,
            definition = new { version = 2, contracts, columns = columns.Select(c => new { id = c.Id, name = c.Name, formula = c.Formula }) },
        });

    private static string View(string name, params (string? Id, string Name, object Formula)[] columns) => View(name, "split", columns);

    private static readonly object NoticeItemCount = FlowOf(new { kind = "linked", link = "notice", items = true }, new { op = "count" });

    private string Read(string sql, object? args = null)
    {
        // 앱 함수가 없는 독립 읽기 연결이 소비자 경계다.
        using var consumer = new SqliteConnection($"Data Source={_path};Mode=ReadOnly;Pooling=False");
        consumer.Open();
        return consumer.QuerySingle<string>(sql, args);
    }

    private List<string> Rows(string sql)
    {
        using var consumer = new SqliteConnection($"Data Source={_path};Mode=ReadOnly;Pooling=False");
        consumer.Open();
        return [.. consumer.Query<string>(sql)];
    }

    private long Count(string view) => long.Parse(Read($"SELECT CAST(count(*) AS TEXT) FROM \"{view}\""));

    private List<string> ColumnsOf(string view) => Rows($"SELECT name FROM pragma_table_info('{view}')");

    private List<string> UserViewNames() =>
        Rows("SELECT name FROM sqlite_master WHERE type = 'view' AND name LIKE 'u\\_%' ESCAPE '\\' ORDER BY name");

    private string Master() => string.Join("\n", Rows("SELECT type || ' ' || name || ' ' || coalesce(sql, '') FROM sqlite_master ORDER BY type, name"));

    private static List<string> ColumnIds(Store.UserViewInfo view) =>
        [.. view.Definition.GetProperty("columns").EnumerateArray().Select(c => c.GetProperty("id").GetString()!)];

    private static List<string> ColumnNames(Store.UserViewInfo view) =>
        [.. view.Definition.GetProperty("columns").EnumerateArray().Select(c => c.GetProperty("name").GetString()!)];

    private List<JsonElement> Samples(string view, string? query = null, string? edge = null) =>
        [.. JsonSerializer.SerializeToElement(_store.UserViewSamples(view, query, edge)).EnumerateArray()];

    [Fact]
    public void 서식용_예시는_취소와_재공고가_연결되면_현행_공고명을_낸다()
    {
        var package = Store.BuiltinViewPackage();
        _store.ImportViewPackage(package, _store.PreviewViewPackage(package).Token);
        _store.UpsertNotice(new NoticeRecord { NoticeBase = "TESTN1", Seq = "000", Title = "시험 원공고" });
        _store.UpsertNotice(new NoticeRecord { NoticeBase = "TESTN1", Seq = "001", Title = "시험 취소공고",
            NoticeKind = "취소공고", RelatedNotices = ["TESTN1-000"] });
        var renotice = new NoticeRecord { NoticeBase = "TESTN2", Seq = "000", Title = "시험 재공고",
            NoticeKind = "재공고", RelatedNotices = ["TESTN1-001", "TESTN1-000"] };
        _store.UpsertNotice(renotice);
        _store.UpsertRequest(new RequestRecord { RequestBase = "TESTR1", Seq = "000", Title = "시험 접수" });
        ReferenceFixture.Request(_database, new("request", "TESTR1", "000"), new("notice", "TESTN1", "000"));
        foreach (var number in new[] { "TESTK1", "TESTK2" })
        {
            _store.UpsertContract(new ContractRecord { ContractBase = number, Seq = "00", Title = "시험 계약" });
            ReferenceFixture.Contract(_database, new("contract", number, "00"), new("notice", "TESTN1", "000"));
        }

        List<string> Titles() => Rows("""
            SELECT 건명 || '|' || 입찰공고번호 || '|' || 공고명 || '|' || 계약번호
            FROM "u_서식용 예시" ORDER BY 계약번호
            """);
        string[] current = ["시험 재공고|TESTN2-000|시험 재공고|TESTK100", "시험 재공고|TESTN2-000|시험 재공고|TESTK200"];
        // 관련공고가 본번호를 잇고, 계약 둘은 원공고를 참조해도 현행 재공고의 이름을 읽는다.
        Assert.Equal(current, Titles());

        // ERP처럼 관련공고 없이 같은 접수 품목으로 묶인 경우도 재공고 하나가 현행이다.
        _store.UpsertNotice(renotice with { RelatedNotices = [] });
        ReferenceFixture.Request(_database, new("request", "TESTR1", "000"), new("notice", "TESTN2", "000"));
        Assert.Equal(current, Titles());

        // 현행 후보가 갈리면 임의로 고르지 않는다. 건명만 접수명으로 물러서고 공고명은 빈다.
        _store.SetOverride(new("notice", "TESTN2", "000"), "공고종류", "일반공고");
        Assert.Equal(["시험 접수|||TESTK100", "시험 접수|||TESTK200"], Titles());
    }

    [Fact]
    public void 보기_구성은_자료와_입력값_없이_의존열과_차례를_왕복한다()
    {
        Seed();
        _store.AddUserColumn("notice", "공유확인", "choice", ["대기", "완료"]);
        _store.SetUserField(new("notice", "TESTN1", "000"), "공유확인", "절대내보내지않는셀값");
        _store.SaveComputedColumn("notice", "품목총수", JsonSerializer.Serialize(FlowOf(new { kind = "items" }, new { op = "count" })));
        var view = Assert.Single(_store.SaveUserView(View("공유문서", "fold",
            ("c-1", "검토", Doc("notice", "공유확인")), ("c-2", "수량", Doc("notice", "품목총수"))), null));
        var json = _store.ExportViewPackage([view.Id]);
        Assert.DoesNotContain("절대내보내지않는셀값", json);
        Assert.DoesNotContain("시험 공고 하나", json);
        Assert.DoesNotContain("TESTN1", json);
        var package = JsonNode.Parse(json)!;
        Assert.Equal(2, package["columns"]!.AsArray().Count);
        var preview = _store.PreviewViewPackage(json);
        Assert.Empty(preview.Errors);
        Assert.Equal("공유문서 (복사본)", Assert.Single(preview.Views).ImportName);
        Assert.All(preview.Columns, c => Assert.Equal("reuse", c.Action));
        Assert.Single(_store.UserViews()); // preview writes nothing
        var imported = _store.ImportViewPackage(json, preview.Token).Single(v => v.Name == "공유문서 (복사본)");
        Assert.Equal("fold", imported.Definition.GetProperty("contracts").GetString());
        Assert.Equal(ColumnNames(view), ColumnNames(imported));
        Assert.Equal(ColumnIds(view), ColumnIds(imported));
        Assert.True(JsonNode.DeepEquals(JsonNode.Parse(view.Definition.GetRawText()), JsonNode.Parse(imported.Definition.GetRawText())));
        Assert.Equal("절대내보내지않는셀값", Read("SELECT 공유확인 FROM v_공고 WHERE 입찰공고번호='TESTN1-000'"));
    }

    [Fact]
    public void 보기_구성은_SQL_간접의존열을_포함하고_빈_자료에도_가져온다()
    {
        _store.AddUserColumn("notice", "원수", "number", []);
        _store.SaveComputedColumn("notice", "간접수", JsonSerializer.Serialize(Sql("SELECT 현재.원수")));
        _store.SaveComputedColumn("notice", "다른수", JsonSerializer.Serialize(FlowOf(new { kind = "number", value = 2 })));
        var saved = Assert.Single(_store.SaveUserView(View("SQL문서", ("c-1", "값", Doc("notice", "간접수"))), null));
        var json = _store.ExportViewPackage([saved.Id]);
        Assert.True(JsonNode.Parse(json)!["includesAllColumnsForSql"]!.GetValue<bool>());
        _store.DeleteUserView(saved.Id);
        _store.RemoveUserColumn("notice", "간접수");
        _store.RemoveUserColumn("notice", "원수");
        _store.RemoveUserColumn("notice", "다른수");
        var preview = _store.PreviewViewPackage(json);
        Assert.Empty(preview.Errors);
        Assert.Contains(preview.Columns, c => c.FieldName == "원수" && c.Action == "add");
        Assert.DoesNotContain(_store.UserColumns(), c => c.FieldName == "원수");
        var imported = Assert.Single(_store.ImportViewPackage(json, preview.Token));
        Assert.Empty(imported.Broken);
        Assert.Contains(_store.UserColumns(), c => c.FieldName == "원수");
        Assert.Equal(0, Count("u_SQL문서"));
        var formulaIds = _store.UserColumns("notice").Where(c => c.Kind == "computed").Select(c => ComputedColumn.Parse(c.Options!).Id).ToList();
        Assert.Equal(2, formulaIds.Count);
        Assert.All(formulaIds, id => Assert.False(string.IsNullOrWhiteSpace(id)));
        Assert.Equal(formulaIds.Count, formulaIds.Distinct().Count());
        var reuse = _store.PreviewViewPackage(json);
        Assert.Empty(reuse.Errors);
        _store.ImportViewPackage(json, reuse.Token);
        Assert.Equal(formulaIds, _store.UserColumns("notice").Where(c => c.Kind == "computed").Select(c => ComputedColumn.Parse(c.Options!).Id).ToList());
    }

    [Fact]
    public void 보기_구성의_열정의가_여럿_다르면_받는_사람이_바꿀_내_열을_모두_든다()
    {
        _store.AddUserColumn("notice", "검토자", "text", []);
        _store.AddUserColumn("contract", "확인", "text", []);
        var saved = Assert.Single(_store.SaveUserView(View("둘",
            ("c-1", "검토", Doc("notice", "검토자")), ("c-2", "확인", Doc("contract", "확인"))), null));
        var modified = JsonNode.Parse(_store.ExportViewPackage([saved.Id]))!;
        foreach (var column in modified["columns"]!.AsArray()) column!["kind"] = "number";
        var preview = _store.PreviewViewPackage(modified.ToJsonString());
        Assert.Equal(2, preview.Columns.Count(c => c.Action == "conflict"));
        Assert.Equal(2, preview.Errors.Count);
        Assert.Contains(preview.Errors, e => e.StartsWith("내 공고 열 「검토자」 와 정의가 다릅니다. 열 관리에서 내 열 이름을 바꾸면", StringComparison.Ordinal));
        Assert.Contains(preview.Errors, e => e.StartsWith("내 계약 열 「확인」 과 정의가 다릅니다.", StringComparison.Ordinal));
        Assert.All(preview.Errors, e => Assert.EndsWith("보낸 사람에게 열 이름을 바꿔 다시 내보내 달라고 해도 됩니다.", e));
    }

    [Fact]
    public void 보기_구성의_다른_열정의와_오래된_미리보기는_기존것을_덮어쓰지_않는다()
    {
        _store.AddUserColumn("notice", "공유검토", "text", []);
        var saved = Assert.Single(_store.SaveUserView(View("공유", ("c-1", "검토", Doc("notice", "공유검토"))), null));
        var json = _store.ExportViewPackage([saved.Id]);
        var validPreview = _store.PreviewViewPackage(json);
        var modified = JsonNode.Parse(json)!;
        modified["columns"]![0]!["kind"] = "number";
        var conflict = _store.PreviewViewPackage(modified.ToJsonString());
        Assert.Contains(conflict.Columns, c => c.Action == "conflict");
        Assert.Equal(["내 공고 열 「공유검토」 와 정의가 다릅니다. 열 관리에서 내 열 이름을 바꾸면 이 구성을 가져올 수 있습니다. " +
            "보낸 사람에게 열 이름을 바꿔 다시 내보내 달라고 해도 됩니다."], conflict.Errors);
        Assert.Throws<InvalidOperationException>(() => _store.ImportViewPackage(modified.ToJsonString(), conflict.Token));
        Assert.Equal("text", _store.UserColumns("notice").Single(c => c.FieldName == "공유검토").Kind);
        modified["columns"]![0]!["choices"]!.AsArray().Add((JsonNode?)null);
        Assert.NotEmpty(_store.PreviewViewPackage(modified.ToJsonString()).Errors);
        _store.SaveUserView(View("공유 (복사본)", ("c-1", "번호", Doc("notice", "입찰공고번호"))), null);
        Assert.Throws<InvalidOperationException>(() => _store.ImportViewPackage(json, validPreview.Token));
        Assert.Equal(2, _store.UserViews().Count);
        var reader = new Store(new Database(_path, Access.Read));
        Assert.Throws<InvalidOperationException>(() => reader.ImportViewPackage(json, validPreview.Token));
    }

    /// <summary>
    /// 서식용 예시 보기(docs/user-views.md §13) — 빈 작업자료가 처음부터 들고 서고, 깨진 필드 없이 서식 자리 이름 그대로의 열을 낸다.
    /// 계약마다 한 줄이다. 계약과 공고에 다 있는 자리는 계약 값이 이기고, 계약 값이 비었거나 계약이 없으면 공고 값이다. 공고담당자를
    /// 이름·전화로 가르는 식도 지어낸 값으로 본다. 취합본·제출본으로 짓는 빈 파일에는 들지 않는다.
    /// </summary>
    [Fact]
    public void 빈_작업자료는_서식용_예시_보기를_들고_선다()
    {
        var json = Store.BuiltinViewPackage();
        var package = JsonNode.Parse(json)!;
        Assert.Empty(package["columns"]!.AsArray());
        Assert.DoesNotContain("\"sql\"", json);
        var fields = package["views"]![0]!["definition"]!["columns"]!.AsArray().Select(c => c!["name"]!.GetValue<string>()).ToList();
        // 단계는 「경우」 한 단계다 — 계약번호가 있으면 계약, 없고 입찰공고번호가 있으면 공고, 둘 다 없으면 접수.
        var stage = package["views"]![0]!["definition"]!["columns"]!.AsArray().Single(c => c!["name"]!.GetValue<string>() == "단계")!;
        Assert.Equal("c32", stage["id"]!.GetValue<string>());
        Assert.True(JsonNode.DeepEquals(JsonNode.Parse("""
            {"from":{"kind":"linked","link":"contract","field":"계약번호"},"steps":[{"op":"cases","branches":[
              {"cond":[{"field":"","cmp":"안비어있음"}],"then":"계약"},
              {"cond":[{"field":"공고.입찰공고번호","cmp":"안비어있음"}],"then":"공고"}],"otherwise":"접수"}]}
            """), stage["formula"]!["flow"]));

        var path = Path.Combine(Path.GetTempPath(), $"pclm-example-{Guid.NewGuid():N}.pclm");
        var merged = Path.Combine(Path.GetTempPath(), $"pclm-example-merged-{Guid.NewGuid():N}.pclm");
        try
        {
            var database = PclmFile.Create(path, PclmRole.Work);
            var store = new Store(database);
            var view = Assert.Single(store.UserViews());
            Assert.Equal("서식용 예시", view.Name);
            Assert.Empty(view.Broken);
            Assert.Equal("split", view.Definition.GetProperty("contracts").GetString());

            store.UpsertNotice(new NoticeRecord { NoticeBase = "TESTN5", Seq = "000", Title = "시험 공고 다섯",
                NoticeOfficer = "홍길동 ( ☎ 000-0000-0000 )", PostedAt = new DateTime(2000, 1, 2, 10, 30, 0),
                Items = [new() { LineNo = 1, ItemName = "책상", Unit = "개" }] });
            store.UpsertNotice(new NoticeRecord { NoticeBase = "TESTN6", Seq = "000", Title = "시험 공고 여섯",
                Items = [new() { LineNo = 1, ItemName = "의자", Unit = "식" }] });
            store.UpsertContract(new ContractRecord { ContractBase = "TESTK5", Seq = "00", Title = "시험 계약 다섯", ItemName = "서랍", Unit = "대", Amount = 1000 });
            store.UpsertContract(new ContractRecord { ContractBase = "TESTK6", Seq = "00", Title = "시험 계약 여섯", Amount = 2000 });
            ReferenceFixture.Contract(database, new("contract", "TESTK5", "00"), new("notice", "TESTN5", "000"));
            ReferenceFixture.Contract(database, new("contract", "TESTK6", "00"), new("notice", "TESTN5", "000"));
            // 건명의 갈래 — 접수만 있는 건, 공고명이 빈 공고에 이어진 접수, 공고명이 빈 공고에 이어진 계약, 계약만 있는 건.
            store.UpsertRequest(new RequestRecord { RequestBase = "TESTR5", Seq = "000", Title = "시험 접수 다섯" });
            store.UpsertNotice(new NoticeRecord { NoticeBase = "TESTN7", Seq = "000", Title = "" });
            store.UpsertRequest(new RequestRecord { RequestBase = "TESTR7", Seq = "000", Title = "시험 접수 일곱" });
            ReferenceFixture.Request(database, new("request", "TESTR7", "000"), new("notice", "TESTN7", "000"));
            store.UpsertNotice(new NoticeRecord { NoticeBase = "TESTN8", Seq = "000", Title = "" });
            store.UpsertContract(new ContractRecord { ContractBase = "TESTK8", Seq = "00", Title = "시험 계약 여덟" });
            ReferenceFixture.Contract(database, new("contract", "TESTK8", "00"), new("notice", "TESTN8", "000"));
            store.UpsertContract(new ContractRecord { ContractBase = "TESTK7", Seq = "00", Title = "시험 계약 일곱" });

            using var consumer = new SqliteConnection($"Data Source={path};Mode=ReadOnly;Pooling=False");
            consumer.Open();
            Assert.Equal(fields, consumer.Query<string>("SELECT name FROM pragma_table_info('u_서식용 예시')"));
            string Row(string sql) => string.Join("\n", consumer.Query<string>(sql));
            // 계약이 둘인 건은 계약마다 한 줄 — 계약 값이 이기고, 계약 값이 비면 공고 값이다.
            Assert.Equal("1,000|서랍|대\n2,000|책상|개",
                Row("SELECT 계약금액 || '|' || 세부품명 || '|' || 단위 FROM \"u_서식용 예시\" WHERE 입찰공고번호 = 'TESTN5-000' ORDER BY 1"));
            // 계약이 없는 공고는 공고 값이다. 빈 칸은 NULL 이 아니라 빈 글이다.
            Assert.Equal("의자|식||", Row("SELECT 세부품명 || '|' || 단위 || '|' || 대표계약업체 || '|' || 계약담당자 FROM \"u_서식용 예시\" WHERE 입찰공고번호 = 'TESTN6-000'"));
            Assert.Equal("홍길동|000-0000-0000|2000/01/02", Row(
                "SELECT DISTINCT 계약담당자 || '|' || \"계약담당 전화번호\" || '|' || 게시일 FROM \"u_서식용 예시\" WHERE 입찰공고번호 = 'TESTN5-000'"));

            // 앞의 일곱 필드 — 사람이 알아보는 건명·단계가 먼저다. 건명은 공고명 → 요청명 → 계약건명으로 물러서 비지 않고,
            // 공고명은 공고가 없거나 비었으면 빈 채로 둔다(받는 쪽의 빈 값 경고가 산다). 계약이 둘인 건은 계약번호가 갈린 두 줄이다.
            Assert.Equal(["건명", "단계", "수요기관", "대표계약업체", "조달요구번호", "입찰공고번호", "계약번호"], fields.Take(7));
            Assert.Equal(string.Join("\n",
                    "시험 계약 여덟|계약||TESTK800",
                    "시험 계약 일곱|계약||TESTK700",
                    "시험 공고 다섯|계약|시험 공고 다섯|TESTK500",
                    "시험 공고 다섯|계약|시험 공고 다섯|TESTK600",
                    "시험 공고 여섯|공고|시험 공고 여섯|",
                    "시험 접수 다섯|접수||",
                    "시험 접수 일곱|공고||"),
                Row("SELECT 건명 || '|' || 단계 || '|' || 공고명 || '|' || 계약번호 FROM \"u_서식용 예시\" ORDER BY 1"));

            // 취합본처럼 다른 역할로 짓는 빈 파일에는 들지 않는다.
            Assert.Empty(new Store(PclmFile.Create(merged, PclmRole.Merged)).UserViews());
        }
        finally
        {
            SqliteConnection.ClearAllPools();
            foreach (var file in new[] { path, merged })
                foreach (var suffix in new[] { "", "-wal", "-shm" }) File.Delete(file + suffix);
        }
    }

    /// <summary>
    /// 지운 예시 보기는 「보기 구성」 에서 다시 들인다 — 같은 미리보기 → 추가 흐름이고, 이름이 겹치면 여느 구성처럼 복사본으로 선다.
    /// </summary>
    [Fact]
    public void 예시_보기는_구성_가져오기로_다시_들고_겹치면_복사본이다()
    {
        var json = Store.BuiltinViewPackage();
        var preview = _store.PreviewViewPackage(json);
        Assert.Empty(preview.Errors);
        Assert.Empty(preview.Columns);
        Assert.Equal(["서식용 예시"], preview.Views.Select(v => v.ImportName));
        Assert.Empty(UserViewNames()); // 미리보기는 되돌린다

        var imported = Assert.Single(_store.ImportViewPackage(json, preview.Token));
        Assert.Empty(imported.Broken);
        Assert.Equal(["u_서식용 예시"], UserViewNames());

        var again = _store.PreviewViewPackage(json);
        Assert.Empty(again.Errors);
        Assert.Equal(["서식용 예시 (복사본)"], again.Views.Select(v => v.ImportName));
    }

    [Theory]
    [InlineData("SELECT count(*) FROM notice")]
    [InlineData("SELECT 공고.없는필드")]
    public void 보기_구성의_잘못된_SQL은_추가열까지_전부_취소한다(string sql)
    {
        var saved = Assert.Single(_store.SaveUserView(View("원본", ("c-1", "번호", Doc("notice", "입찰공고번호"))), null));
        var package = JsonNode.Parse(_store.ExportViewPackage([saved.Id]))!;
        package["columns"]!.AsArray().Add(JsonNode.Parse("""{"entityType":"notice","fieldName":"임시공유열","kind":"text","choices":[]}"""));
        package["views"]![0]!["definition"]!["columns"]![0]!["formula"] = JsonSerializer.SerializeToNode(Sql(sql));
        var before = Master();
        var preview = _store.PreviewViewPackage(package.ToJsonString());
        Assert.NotEmpty(preview.Errors);
        Assert.Throws<InvalidOperationException>(() => _store.ImportViewPackage(package.ToJsonString(), preview.Token));
        Assert.Equal(before, Master());
        Assert.Single(_store.UserViews());
        Assert.DoesNotContain(_store.UserColumns(), c => c.FieldName == "임시공유열");
    }

    // ── 저장 ─────────────────────────────────────────────

    /// <summary>
    /// 저장하면 <c>u_이름</c> 이 서고, 이름을 바꿔도 뷰 식별자와 필드 식별자는 남는다. 옛 이름의 뷰는 sqlite_master 에 남지 않는다.
    /// 필드 식별자가 비어 오면 다리가 뽑는다. 정의는 v2 이고 기준이 없다.
    /// </summary>
    [Fact]
    public void 저장_왕복과_식별자는_이름을_바꿔도_남는다()
    {
        Seed();
        var saved = Assert.Single(_store.SaveUserView(View("공고문서",
            ("c-1", "공고번호", Doc("notice", "입찰공고번호")), (null, "공고명", Doc("notice", "공고명"))), null));
        Assert.Equal("공고문서", saved.Name);
        Assert.Empty(saved.Broken);
        Assert.Equal(2, saved.Definition.GetProperty("version").GetInt32());
        Assert.Equal("split", saved.Definition.GetProperty("contracts").GetString());
        var ids = ColumnIds(saved);
        Assert.Equal("c-1", ids[0]);
        Assert.False(string.IsNullOrEmpty(ids[1]));
        Assert.Equal(["u_공고문서"], UserViewNames());
        Assert.Equal("시험 공고 하나", Read("SELECT 공고명 FROM u_공고문서 WHERE 공고번호 = @N1 LIMIT 1", new { N1 }));
        Assert.DoesNotContain("base", Rows("SELECT name FROM pragma_table_info('user_view')"));

        // 저장 꼴의 필드 식에는 식별자를 싣지 않는다(§4).
        Assert.False(saved.Definition.GetProperty("columns")[0].GetProperty("formula").TryGetProperty("id", out _));

        var renamed = Assert.Single(_store.SaveUserView(View("공고서류",
            (ids[0], "번호", Doc("notice", "입찰공고번호")), (ids[1], "공고명", Doc("notice", "공고명"))), saved.Id));
        Assert.Equal(saved.Id, renamed.Id);
        Assert.Equal(ids, ColumnIds(renamed));
        Assert.Equal(["u_공고서류"], UserViewNames());

        Assert.Empty(_store.DeleteUserView(saved.Id));
        Assert.Empty(UserViewNames());
        Assert.Contains("맞춤 뷰가 없습니다", Assert.Throws<InvalidOperationException>(() => _store.DeleteUserView(saved.Id)).Message);
    }

    /// <summary>필드 차례는 정의의 차례다 — 문서의 차례와 상관없다.</summary>
    [Fact]
    public void 필드_차례는_정의의_차례다()
    {
        Seed();
        _store.SaveUserView(View("문서",
            (null, "공고명", Doc("notice", "공고명")), (null, "접수번호", Doc("request", "접수번호")),
            (null, "품목 수", NoticeItemCount), (null, "메모", Doc("notice", "메모"))), null);
        Assert.Equal(["공고명", "접수번호", "품목 수", "메모"], ColumnsOf("u_문서"));
    }

    /// <summary>
    /// 한 뷰에서 접수·공고·계약의 필드를 함께 읽는다(§3) — 세 문서의 사람 열·가공열까지. 맞춤 뷰는 면 뷰 위층이라 순환이 없다.
    /// SQL 필드는 바깥 행의 문서를 <c>접수</c>·<c>공고</c>·<c>계약</c> 으로 부른다.
    /// </summary>
    [Fact]
    public void 세_문서의_사람_열과_가공열을_함께_읽는다()
    {
        Seed();
        _store.AddUserColumn("request", "확인여부", "text", []);
        _store.SetUserField(new("request", "TESTR1", "000"), "확인여부", "예");
        _store.SaveComputedColumn("contract", "품목개수", JsonSerializer.Serialize(FlowOf(new { kind = "items" }, new { op = "count" })));
        _store.SaveUserView(View("문서",
            (null, "계약번호", Doc("contract", "계약번호")), (null, "확인", Doc("request", "확인여부")),
            (null, "메모", Doc("notice", "메모")), (null, "품목개수", Doc("contract", "품목개수")),
            (null, "대표품명", FlowOf(new { kind = "linked", link = "contract", items = true },
                new { op = "get", field = "품명" }, new { op = "distinct" }, new { op = "abbrev", n = 1, text = "{앞} 외 {나머지}종" })),
            (null, "공고명SQL", Sql("SELECT 공고.공고명")),
            (null, "비면", FlowOf(new { kind = "text", text = "" }, new { op = "ifEmpty", field = "공고.메모" }))), null);

        Assert.Equal("예|확인함|2|책상|시험 공고 하나|확인함",
            Read("SELECT 확인 || '|' || 메모 || '|' || 품목개수 || '|' || 대표품명 || '|' || 공고명SQL || '|' || 비면 FROM u_문서 WHERE 계약번호 = 'TESTK100'"));
        Assert.Equal("의자", Read("SELECT 대표품명 FROM u_문서 WHERE 계약번호 = 'TESTK200'"));
    }

    /// <summary>
    /// 줄은 <c>v_통합</c> 과 같은 사슬이다(§2·§5) — 계약마다 한 줄이면 건 수가 담긴 종류로 거른 <c>v_통합</c> 의 줄 수와 같고, 사슬의 글은
    /// 두 뷰에 한 벌로 들어 있다.
    /// </summary>
    [Fact]
    public void 줄은_v_통합과_같은_사슬이다()
    {
        Seed();
        _store.SaveUserView(View("셋", (null, "접수번호", Doc("request", "접수번호")), (null, "공고번호", Doc("notice", "입찰공고번호")),
            (null, "계약번호", Doc("contract", "계약번호"))), null);
        _store.SaveUserView(View("공고만", (null, "공고명", Doc("notice", "공고명"))), null);
        _store.SaveUserView(View("계약만", (null, "계약번호", Doc("contract", "계약번호"))), null);
        _store.SaveUserView(View("접수공고", (null, "접수번호", Doc("request", "접수번호")), (null, "공고명", Doc("notice", "공고명"))), null);

        Assert.Equal(6, Count("v_통합"));
        Assert.Equal(6, Count("u_셋"));
        Assert.Equal(
            Rows("SELECT 접수번호 || '|' || 입찰공고번호 || '|' || 계약번호 FROM v_통합 ORDER BY 1"),
            Rows("SELECT 접수번호 || '|' || 공고번호 || '|' || 계약번호 FROM u_셋 ORDER BY 1"));
        Assert.Equal(long.Parse(Read("SELECT CAST(count(*) AS TEXT) FROM v_통합 WHERE 입찰공고번호 <> ''")), Count("u_공고만"));
        Assert.Equal(long.Parse(Read("SELECT CAST(count(*) AS TEXT) FROM v_통합 WHERE 계약번호 <> ''")), Count("u_계약만"));
        Assert.Equal(long.Parse(Read("SELECT CAST(count(*) AS TEXT) FROM v_통합 WHERE 입찰공고번호 <> '' OR 접수번호 <> ''")), Count("u_접수공고"));

        // 사슬은 한 벌이다 — 두 뷰가 같은 글을 부른다.
        Assert.Contains(Views.사슬, Read("SELECT sql FROM sqlite_master WHERE name = 'v_통합'"));
        Assert.Contains(Views.사슬, Read("SELECT sql FROM sqlite_master WHERE name = 'u_셋'"));
    }

    /// <summary>
    /// 모으기(§2) — 계약이 여럿인 건은 한 줄이고, 계약 필드는 계약마다 셈해 계약본번호 차례로 <c>", "</c> 로 잇는다. 단계가 있는 계약
    /// 필드도 계약마다 셈한다. 계약마다 한 줄로 되돌리면 같은 필드가 그대로 선다.
    /// </summary>
    [Fact]
    public void 모으기는_계약이_여럿인_건을_한_줄로_잇는다()
    {
        Seed();
        var folded = Assert.Single(_store.SaveUserView(View("문서", "fold",
            (null, "공고번호", Doc("notice", "입찰공고번호")), (null, "계약번호", Doc("contract", "계약번호")),
            (null, "품목", FlowOf(new { kind = "linked", link = "contract", items = true }, new { op = "count" })),
            (null, "건명", FlowOf(new { kind = "linked", link = "contract", field = "계약건명" }, new { op = "after", text = "시험 " }))), null));
        Assert.Equal("fold", folded.Definition.GetProperty("contracts").GetString());

        Assert.Equal("TESTK100, TESTK200|2, 1|계약 하나, 계약 둘", Read("SELECT 계약번호 || '|' || 품목 || '|' || 건명 FROM u_문서 WHERE 공고번호 = @N1", new { N1 }));
        Assert.Equal("1", Read("SELECT CAST(count(*) AS TEXT) FROM u_문서 WHERE 공고번호 = @N1", new { N1 }));
        // 공고건 둘 + 매달리지 못한 계약 둘.
        Assert.Equal(4, Count("u_문서"));

        _store.SaveUserView(View("문서", "split",
            (null, "공고번호", Doc("notice", "입찰공고번호")), (null, "계약번호", Doc("contract", "계약번호"))), folded.Id);
        Assert.Equal(["TESTK100", "TESTK200"], Rows($"SELECT 계약번호 FROM u_문서 WHERE 공고번호 = '{N1}' ORDER BY 1"));
        Assert.Empty(_store.UserViews().Single().Broken);
    }

    /// <summary>계약만·접수만인 사슬은 모으기에서 접히지 않는다 — 공고건이 없는 것끼리 한 줄이 되지 않는다.</summary>
    [Fact]
    public void 공고건이_없는_사슬은_접히지_않는다()
    {
        Seed();
        _store.SaveUserView(View("문서", "fold", (null, "접수번호", Doc("request", "접수번호")), (null, "계약번호", Doc("contract", "계약번호"))), null);
        Assert.Equal(["TESTK300", "TESTK400"], Rows("SELECT 계약번호 FROM u_문서 WHERE 접수번호 = '' ORDER BY 1"));
        Assert.Equal(["TESTK100, TESTK200", ""], Rows("SELECT 계약번호 FROM u_문서 WHERE 접수번호 <> '' ORDER BY 1 DESC"));
        Assert.Equal(4, Count("u_문서"));
    }

    /// <summary>
    /// 담긴 흐름식 필드가 읽는 문서 종류가 하나도 없는 건은 줄이 서지 않는다(§2). SQL 필드는 거르지 않고, 문서를 읽지 않는 흐름식뿐이면
    /// 모든 조달 건이 선다.
    /// </summary>
    [Fact]
    public void 담긴_종류가_없는_건은_줄이_서지_않는다()
    {
        Seed();
        _store.SaveUserView(View("접수", (null, "접수번호", Doc("request", "접수번호"))), null);
        _store.SaveUserView(View("SQL", (null, "n", Sql("SELECT count(*) FROM v_공고"))), null);
        _store.SaveUserView(View("글", (null, "t", FlowOf(new { kind = "text", text = "고정" }))), null);
        _store.SaveUserView(View("비면", (null, "t", FlowOf(new { kind = "text", text = "" }, new { op = "ifEmpty", field = "계약.계약번호" }))), null);

        // 접수1 은 계약마다 두 줄이고, 접수2 는 한 줄이다.
        Assert.Equal(["TESTR1-000", "TESTR1-000", "TESTR2-000"], Rows("SELECT 접수번호 FROM u_접수 ORDER BY 1"));
        Assert.Equal(6, Count("u_SQL"));
        Assert.Equal(6, Count("u_글"));
        Assert.Equal(4, Count("u_비면"));
    }

    /// <summary>
    /// 「이 행」 의 원천(<c>field</c>·<c>items</c>)은 맞춤 뷰에 없다 — 문서를 붙여 읽으라고 말하며 거절한다. 이름이 비거나 겹치거나
    /// 큰따옴표가 있어도 거절한다. 뷰끼리는 대소문자도 겹침이다(SQLite 이름이 그렇다).
    /// </summary>
    [Fact]
    public void 이행의_원천과_틀린_이름은_거절한다()
    {
        Seed();
        var field = Assert.Throws<InvalidOperationException>(() => _store.SaveUserView(View("문서",
            (null, "공고명", FlowOf(new { kind = "field", field = "공고명" }))), null));
        Assert.Contains("‘공고명’ 필드를 세울 수 없습니다", field.Message);
        Assert.Contains("‘공고.공고명’", field.Message);
        Assert.Throws<InvalidOperationException>(() => _store.SaveUserView(View("문서", (null, "n", FlowOf(new { kind = "items" }, new { op = "count" }))), null));
        Assert.Contains("공고에 없는 필드", Assert.Throws<InvalidOperationException>(() =>
            _store.SaveUserView(View("문서", (null, "x", Doc("notice", "그런열없음"))), null)).Message);
        Assert.Throws<InvalidOperationException>(() => _store.SaveUserView(View("문서", "many", (null, "a", Doc("notice", "공고명"))), null));

        Assert.Throws<InvalidOperationException>(() => _store.SaveUserView(View(" ", (null, "a", Doc("notice", "공고명"))), null));
        Assert.Throws<InvalidOperationException>(() => _store.SaveUserView(View("a\"b", (null, "a", Doc("notice", "공고명"))), null));
        Assert.Contains("필드를 하나 이상", Assert.Throws<InvalidOperationException>(() => _store.SaveUserView(View("문서"), null)).Message);
        Assert.Contains("겹칩니다", Assert.Throws<InvalidOperationException>(() =>
            _store.SaveUserView(View("문서", (null, "a", Doc("notice", "공고명")), (null, "A", Doc("notice", "입찰공고번호"))), null)).Message);
        Assert.Throws<InvalidOperationException>(() => _store.SaveUserView(View("문서", (null, " ", Doc("notice", "공고명"))), null));
        _store.SaveUserView(View("Doc", (null, "a", Doc("notice", "공고명"))), null);
        Assert.Contains("‘doc’ 맞춤 뷰가 이미 있습니다", Assert.Throws<InvalidOperationException>(() =>
            _store.SaveUserView(View("doc", (null, "a", Doc("contract", "계약번호"))), null)).Message);
        Assert.Equal(["u_Doc"], UserViewNames());
    }

    /// <summary>빈 값은 NULL 이 아니라 빈 글이다 — 받는 쪽의 빈 값 경고가 그것에 걸려 있다. 문서가 없는 건의 칸도 그렇다.</summary>
    [Fact]
    public void 빈_값은_빈_글이다()
    {
        Seed();
        _store.SaveUserView(View("문서",
            (null, "메모", Doc("notice", "메모")), (null, "접수", Doc("request", "요청명")),
            (null, "개수", NoticeItemCount), (null, "SQL", Sql("SELECT 계약건명 FROM v_계약 WHERE 계약번호 = 'NONE'")),
            (null, "계약", FlowOf(new { kind = "linked", link = "contract" }, new { op = "get", field = "계약건명" }))), null);
        foreach (var column in new[] { "메모", "접수", "SQL", "계약" })
            Assert.Equal("text", Read($"SELECT typeof(\"{column}\") FROM u_문서 WHERE \"{column}\" = '' LIMIT 1"));
        Assert.Equal("0", Read("SELECT CAST(count(*) AS TEXT) FROM u_문서 WHERE 메모 IS NULL OR 접수 IS NULL OR 개수 IS NULL OR SQL IS NULL OR 계약 IS NULL"));
        Assert.Equal("0", Read("SELECT 개수 FROM u_문서 WHERE 접수 = 'x' OR 메모 = '' LIMIT 1"));
    }

    /// <summary>
    /// 깨진 필드는 빈 글 + 까닭 주석으로 서고(§5), 같은 뷰의 다른 필드와 다른 뷰는 그대로 선다. 까닭은 뷰 정의 글에서 읽혀
    /// 목록의 <c>broken</c> 에 필드 식별자로 실린다.
    /// </summary>
    [Fact]
    public void 깨진_필드는_빈_글로_서고_다른_것은_선다()
    {
        Seed();
        var good = Assert.Single(_store.SaveUserView(View("멀쩡", (null, "공고명", Doc("notice", "공고명"))), null));
        // 손으로 고친 파일처럼 — 없는 열을 읽는 식과 SQL 이 맞춤 뷰를 읽는 식을 넣는다.
        using (var connection = _database.Open())
            connection.Execute("""
                INSERT INTO user_view (id, name, definition, sort_order) VALUES ('v-broken', '깨짐', @definition, 99)
                """, new { definition = JsonSerializer.Serialize(new { version = 2, contracts = "split", columns = new object[] {
                    new { id = "k1", name = "공고명", formula = Doc("notice", "공고명") },
                    new { id = "k2", name = "없는열", formula = Doc("notice", "그런열없음") },
                    new { id = "k3", name = "남의뷰", formula = Sql("SELECT count(*) FROM u_멀쩡") },
                } }) });
        _database.RefreshViews();

        Assert.Equal(["u_깨짐", "u_멀쩡"], UserViewNames());
        Assert.Equal(["공고명", "없는열", "남의뷰"], ColumnsOf("u_깨짐"));
        Assert.Equal("시험 공고 하나", Read("SELECT 공고명 FROM u_깨짐 WHERE 공고명 LIKE '%하나' LIMIT 1"));
        Assert.Equal("", Read("SELECT 없는열 FROM u_깨짐 LIMIT 1"));
        Assert.Equal("", Read("SELECT 남의뷰 FROM u_깨짐 LIMIT 1"));
        Assert.Equal(3, Count("u_멀쩡"));

        var broken = Views.BrokenIn(Read("SELECT sql FROM sqlite_master WHERE name = 'u_깨짐'"));
        Assert.Equal(["남의뷰", "없는열"], broken.Keys.Order());
        Assert.Contains("그런열없음", broken["없는열"]);

        var listed = _store.UserViews().Single(v => v.Id == "v-broken");
        Assert.Equal(["k2", "k3"], listed.Broken.Keys.Order());
        Assert.Empty(_store.UserViews().Single(v => v.Id == good.Id).Broken);

        // 저장은 이 뷰의 필드가 깨진 식으로 서면 거절한다 — 같은 정의를 저장하려 하면 막힌다.
        Assert.Throws<InvalidOperationException>(() => _store.SaveUserView(View("깨짐",
            ("k1", "공고명", Doc("notice", "공고명")), ("k3", "남의뷰", Sql("SELECT count(*) FROM u_멀쩡"))), "v-broken"));
    }

    // ── 사람 열과 맞물림 ─────────────────────────────────────

    /// <summary>
    /// 사람 열 이름을 바꾸면 그 열을 읽던 맞춤 뷰 필드가 새 이름을 따라간다(§5) — 원천 <c>linked(종류) 열</c>, 단계가 읽는
    /// <c>공고.열</c>, 그 문서 한 행의 첫 단계가 짚는 필드. 다른 종류의 같은 이름은 그대로다. 지우는 확인 문구가 종류마다 셀 수를
    /// 내고, 지우면 그 필드는 빈 글(깨진 식)로 선다 — 조용히 빠지지 않는다.
    /// </summary>
    [Fact]
    public void 사람_열의_이름을_바꾸면_따라가고_지우면_깨진_식이_된다()
    {
        Seed();
        var view = Assert.Single(_store.SaveUserView(View("문서",
            (null, "메모값", Doc("notice", "메모")),
            (null, "비면", FlowOf(new { kind = "text", text = "" }, new { op = "ifEmpty", field = "공고.메모" })),
            (null, "꺼냄", FlowOf(new { kind = "linked", link = "notice" }, new { op = "get", field = "메모" })),
            (null, "계약메모", Doc("contract", "메모")),
            (null, "공고명", Doc("notice", "공고명"))), null));
        _store.SaveUserView(View("계약문서", (null, "메모", Doc("contract", "메모"))), null);

        Assert.Equal(1, _store.UserViewReaders("notice")["메모"]);
        Assert.Equal(2, _store.UserViewReaders("contract")["메모"]);
        Assert.False(_store.UserViewReaders("notice").ContainsKey("검토여부"));

        _store.UpdateUserColumn("notice", "메모", "비고", "text", []);
        var rewritten = _store.UserViews().Single(v => v.Id == view.Id);
        Assert.Empty(rewritten.Broken);
        var columns = rewritten.Definition.GetProperty("columns");
        JsonElement Flow(int i) => columns[i].GetProperty("formula").GetProperty("flow");
        Assert.Equal("비고", Flow(0).GetProperty("from").GetProperty("field").GetString());
        Assert.Equal("공고.비고", Flow(1).GetProperty("steps")[0].GetProperty("field").GetString());
        Assert.Equal("비고", Flow(2).GetProperty("steps")[0].GetProperty("field").GetString());
        Assert.Equal("메모", Flow(3).GetProperty("from").GetProperty("field").GetString());
        Assert.Equal("확인함|확인함|확인함", Read($"SELECT 메모값 || '|' || 비면 || '|' || 꺼냄 FROM u_문서 WHERE 메모값 <> '' LIMIT 1"));

        // 가공열 이름 바꾸기(SaveComputedColumn 의 previousName)도 같은 길이다.
        _store.SaveComputedColumn("notice", "개수", JsonSerializer.Serialize(FlowOf(new { kind = "items" }, new { op = "count" })));
        _store.SaveUserView(View("개수뷰", (null, "n", Doc("notice", "개수"))), null);
        _store.SaveComputedColumn("notice", "품목수", JsonSerializer.Serialize(FlowOf(new { kind = "items" }, new { op = "count" })), "개수");
        Assert.Equal("2", Read("SELECT n FROM u_개수뷰 WHERE n <> '0' LIMIT 1"));

        Assert.Equal(1, _store.UserViewReaders("notice")["비고"]);
        _store.RemoveUserColumn("notice", "비고");
        var after = _store.UserViews().Single(v => v.Id == view.Id);
        Assert.Equal(3, after.Broken.Count);
        Assert.Equal(["메모값", "비면", "꺼냄", "계약메모", "공고명"], ColumnsOf("u_문서"));
        Assert.Equal("", Read("SELECT 메모값 FROM u_문서 LIMIT 1"));
        Assert.Equal("시험 공고 하나", Read("SELECT 공고명 FROM u_문서 WHERE 공고명 LIKE '%하나' LIMIT 1"));
    }

    /// <summary>
    /// 값 하나를 견주는 조건의 마디는 바깥의 열을 읽는다 — 「비면」 과 같은 자리(<c>공고.메모</c>)이고 같은 검사를 탄다. 필드 없는 마디는
    /// 지금 값이다. 그 마디가 읽는 문서도 읽는 종류로 세어 줄을 거르고, 사람 열 이름을 바꾸면 따라간다.
    /// </summary>
    [Fact]
    public void 값_조건의_바깥_열은_비면과_같은_자리를_읽는다()
    {
        Seed();
        object If(object[] cond) => new { op = "if", cond, then = "맞음", @else = "" };
        var view = Assert.Single(_store.SaveUserView(View("조건",
            (null, "계약번호", Doc("contract", "계약번호")),
            (null, "판정", FlowOf(new { kind = "linked", link = "contract", field = "계약번호" }, If([
                new { field = "", cmp = "안비어있음" }, new { join = "and", field = "공고.메모", cmp = "=", value = "확인함" }])))), null));
        Assert.Empty(view.Broken);
        Assert.Equal(["TESTK100|맞음", "TESTK200|맞음", "TESTK300|", "TESTK400|"],
            Rows("SELECT 계약번호 || '|' || 판정 FROM u_조건 WHERE 계약번호 <> '' ORDER BY 1"));

        // 글자 원천에 공고를 견주는 조건만 있으면 공고가 있는 건만 선다(사슬 여섯 중 공고가 있는 셋).
        _store.SaveUserView(View("공고조건", (null, "t", FlowOf(new { kind = "text", text = "" }, If([
            new { field = "공고.입찰공고번호", cmp = "안비어있음" }])))), null);
        Assert.Equal(3, Count("u_공고조건"));
        Assert.Equal(["맞음"], Rows("SELECT DISTINCT t FROM u_공고조건"));

        Assert.Equal(1, _store.UserViewReaders("notice")["메모"]);
        _store.UpdateUserColumn("notice", "메모", "비고", "text", []);
        var rewritten = _store.UserViews().Single(v => v.Id == view.Id);
        Assert.Empty(rewritten.Broken);
        Assert.Equal("공고.비고", rewritten.Definition.GetProperty("columns")[1].GetProperty("formula").GetProperty("flow")
            .GetProperty("steps")[0].GetProperty("cond")[1].GetProperty("field").GetString());
        Assert.Equal("맞음", Read("SELECT 판정 FROM u_조건 WHERE 계약번호 = 'TESTK100'"));

        Assert.Contains("문서를 붙여 적으세요 — ‘공고.비고’처럼 읽습니다", Assert.Throws<InvalidOperationException>(() =>
            _store.SaveUserView(View("틀림", (null, "x", FlowOf(new { kind = "text", text = "" }, If([new { field = "비고", cmp = "비어있음" }])))), null)).Message);
        Assert.Contains("공고에 없는 필드: 없는열", Assert.Throws<InvalidOperationException>(() =>
            _store.SaveUserView(View("틀림", (null, "x", FlowOf(new { kind = "text", text = "" }, If([new { field = "공고.없는열", cmp = "비어있음" }])))), null)).Message);
    }

    // ── 미리보기·표본·보표 ─────────────────────────────────────

    /// <summary>
    /// 필드 목록의 재료 — 접수·공고·계약 셋이 저마다 면 뷰의 열(사람 열·가공열 포함)과 품목 필드를 들고, 「이 행」 은 없다. 조달 건 수는
    /// 계약마다 한 줄로 세고, 계약이 여럿인 건 수를 함께 낸다.
    /// </summary>
    [Fact]
    public void 필드_목록의_재료는_세_문서다()
    {
        Seed();
        _store.SaveComputedColumn("contract", "품목개수", JsonSerializer.Serialize(FlowOf(new { kind = "items" }, new { op = "count" })));
        var context = JsonSerializer.SerializeToElement(_store.UserViewContext());
        var linked = context.GetProperty("available").GetProperty("linked").EnumerateArray().ToList();
        Assert.Equal(["request", "notice", "contract"], linked.Select(l => l.GetProperty("kind").GetString()));
        Assert.Empty(context.GetProperty("available").GetProperty("base").EnumerateArray());
        var notice = linked[1].GetProperty("fields").EnumerateArray().Select(f => f.GetString()).ToList();
        Assert.Equal(["검토여부", "메모"], notice.TakeLast(2));
        Assert.Equal("품목개수", linked[2].GetProperty("fields").EnumerateArray().Last().GetString());
        Assert.Contains("금액", linked[2].GetProperty("items").EnumerateArray().Select(f => f.GetString()));
        Assert.False(linked[0].TryGetProperty("many", out _));
        Assert.Equal(6, context.GetProperty("total").GetInt64());
        Assert.Equal(1, context.GetProperty("manyContracts").GetInt64());
    }

    /// <summary>
    /// 표본 고르개는 <b>그 뷰의 줄</b>(거르기·모으기 뒤)에서 고른다 — 보이는 번호는 입찰공고번호 → 접수번호 → 계약번호 중 처음 있는 것,
    /// 제목은 공고명 → 요청명 → 계약건명. 찾기는 번호와 제목에서, 「계약 여럿」 은 계약이 둘 이상인 건, 「연결 안 된 건」 은 빠진 단계가
    /// 있는 건이다.
    /// </summary>
    [Fact]
    public void 표본은_그_뷰의_줄에서_고른다()
    {
        Seed();
        var notices = View("문서", (null, "공고명", Doc("notice", "공고명")));
        var split = Samples(notices);
        Assert.Equal([N1, N1, N2], split.Select(s => s.GetProperty("number").GetString()));
        Assert.Equal("시험 공고 하나", split[0].GetProperty("title").GetString());
        Assert.Equal(3, split.Select(s => s.GetProperty("key").GetString()).Distinct().Count());

        var folded = Samples(View("문서", "fold", (null, "공고명", Doc("notice", "공고명")), (null, "계약번호", Doc("contract", "계약번호"))));
        Assert.Equal([N1, N2, "TESTK300", "TESTK400"], folded.Select(s => s.GetProperty("number").GetString()));
        Assert.Equal("시험 계약 셋", folded[2].GetProperty("title").GetString());

        var all = Samples(View("빈뷰"));
        Assert.Equal(6, all.Count);
        Assert.Contains("시험 접수 둘", all.Select(s => s.GetProperty("title").GetString()));
        Assert.Equal(["TESTN2-000"], Samples(notices, "공고 둘").Select(s => s.GetProperty("number").GetString()));
        Assert.Equal(["TESTN2-000"], Samples(notices, "testn2").Select(s => s.GetProperty("number").GetString()));
        Assert.Equal([N1, N1], Samples(notices, null, "manyContracts").Select(s => s.GetProperty("number").GetString()));
        Assert.Equal([N1], Samples(View("문서", "fold", (null, "계약번호", Doc("contract", "계약번호"))), null, "manyContracts")
            .Select(s => s.GetProperty("number").GetString()));
        Assert.Equal([N2], Samples(notices, null, "unlinked").Select(s => s.GetProperty("number").GetString()));
        Assert.Throws<InvalidOperationException>(() => _store.UserViewSamples(notices, null, "weird"));
    }

    /// <summary>
    /// 미리보기는 그 표본 한 건의 값과 필드마다 그 뷰의 전체 건 중 빈 값 수, 그리고 거르기·모으기 뒤의 건 수를 낸다. 아무것도 남기지
    /// 않는다 — sqlite_master 가 그대로다. 깨진 필드는 error 를 싣고 다른 필드는 값을 낸다. 이름은 보지 않는다(필드 목록의 후보도 이 길로 센다).
    /// </summary>
    [Fact]
    public void 미리보기는_흔적을_남기지_않는다()
    {
        Seed();
        var before = Master();
        var view = View("", "fold", ("a", "메모", Doc("notice", "메모")), ("b", "계약번호", Doc("contract", "계약번호")),
            ("c", "깨짐", Doc("notice", "그런열없음")), ("d", "메모", NoticeItemCount));
        var key = Samples(view)[0].GetProperty("key").GetString();
        var preview = JsonSerializer.SerializeToElement(_store.PreviewUserView(view, key));
        Assert.Equal(before, Master());

        Assert.Equal(key, preview.GetProperty("sample").GetProperty("key").GetString());
        Assert.Equal(N1, preview.GetProperty("sample").GetProperty("number").GetString());
        Assert.Equal("시험 공고 하나", preview.GetProperty("sample").GetProperty("title").GetString());
        Assert.Equal(4, preview.GetProperty("total").GetInt64());
        var columns = preview.GetProperty("columns");
        Assert.Equal(["a", "b", "c", "d"], columns.EnumerateArray().Select(c => c.GetProperty("id").GetString()));
        Assert.Equal("확인함", columns[0].GetProperty("value").GetString());
        Assert.Equal(3, columns[0].GetProperty("blank").GetInt64());
        Assert.Equal("TESTK100, TESTK200", columns[1].GetProperty("value").GetString());
        Assert.Equal(1, columns[1].GetProperty("blank").GetInt64());
        Assert.Equal("", columns[2].GetProperty("value").GetString());
        Assert.Contains("그런열없음", columns[2].GetProperty("error").GetString());
        Assert.Equal("2", columns[3].GetProperty("value").GetString());
        Assert.False(columns[3].TryGetProperty("error", out _));

        var missing = JsonSerializer.SerializeToElement(_store.PreviewUserView(View("", ("a", "x", Doc("notice", "공고명"))), "NONE"));
        Assert.Equal(JsonValueKind.Null, missing.GetProperty("sample").ValueKind);
        Assert.Equal("", missing.GetProperty("columns")[0].GetProperty("value").GetString());
    }

    /// <summary>
    /// 보표는 맞춤 뷰 환경으로 돈다 — 세 문서의 사람 열을 읽고, 표본은 준 열쇠가 처음이다. 모은 건의 열쇠면 단계는 계약 한 건을 보고
    /// 열에 찍힐 값은 뷰가 낼 값(이어 붙인 것)이다. 「SQL로 꺼내기」 의 SQL 은 바깥 행을 문서 별칭으로 부른다.
    /// </summary>
    [Fact]
    public void 보표는_세_문서를_읽고_준_표본이_처음이다()
    {
        Seed();
        var json = JsonSerializer.Serialize(Doc("notice", "메모"));
        var keys = Samples(View("문서", (null, "메모", Doc("notice", "메모")))).Select(s => s.GetProperty("key").GetString()!).ToList();
        var trace = JsonSerializer.SerializeToElement(_store.TraceUserViewColumn(json, [keys[2], keys[0]]));
        var samples = trace.GetProperty("samples");
        Assert.Equal(N2, samples[0].GetProperty("base").GetString());
        Assert.Equal(N1, samples[1].GetProperty("base").GetString());
        Assert.Equal(["", "확인함"], trace.GetProperty("final").EnumerateArray().Select(x => x.GetString()));
        Assert.Contains("공고.\"메모\"", trace.GetProperty("sql").GetString());

        var contract = JsonSerializer.Serialize(Doc("contract", "계약번호"));
        var foldKey = Samples(View("문서", "fold", (null, "계약번호", Doc("contract", "계약번호"))), null, "manyContracts")[0].GetProperty("key").GetString()!;
        var folded = JsonSerializer.SerializeToElement(_store.TraceUserViewColumn(contract, [foldKey]));
        Assert.Equal("TESTK100", folded.GetProperty("steps")[0].GetProperty("values")[0].GetProperty("text").GetString());
        Assert.Equal("TESTK100, TESTK200", folded.GetProperty("final")[0].GetString());

        var items = JsonSerializer.SerializeToElement(_store.TraceUserViewColumn(JsonSerializer.Serialize(NoticeItemCount), [keys[0]]));
        Assert.Equal(2, items.GetProperty("samples")[0].GetProperty("rows").GetInt64());

        // SQL 필드도 바깥 행의 문서를 별칭으로 읽는다.
        var sql = JsonSerializer.SerializeToElement(_store.TraceUserViewColumn(JsonSerializer.Serialize(Sql("SELECT 공고.공고명")), [keys[0]]));
        Assert.Equal("시험 공고 하나", sql.GetProperty("final")[0].GetString());
        Assert.False(sql.GetProperty("steps")[0].TryGetProperty("error", out _));
    }

    // ── 계약면과 소비자 ─────────────────────────────────────

    /// <summary>
    /// <c>v_맞춤뷰</c> — 필드 하나당 한 줄, 식별자로 이름 바뀜을 따라온다. 결과타입은 끝 꼴이다. 2판의 <c>v_사용자뷰</c> 는 제자리에서
    /// 바꾼 것이라 남아 있으면 열 때 치운다(ADR-034).
    /// </summary>
    [Fact]
    public void 메타_뷰는_필드마다_한_줄이고_옛_이름을_치운다()
    {
        Seed();
        var view = Assert.Single(_store.SaveUserView(View("문서",
            ("c-1", "공고명", Doc("notice", "공고명")), ("c-2", "개수", NoticeItemCount)), null));

        var rows = Rows("SELECT 뷰식별자 || '|' || 뷰이름 || '|' || 열식별자 || '|' || 열이름 || '|' || 차례 || '|' || 결과타입 FROM v_맞춤뷰 ORDER BY 차례");
        Assert.Equal([$"{view.Id}|u_문서|c-1|공고명|1|text", $"{view.Id}|u_문서|c-2|개수|2|number"], rows);
        Assert.Equal(["뷰식별자", "뷰이름", "열식별자", "열이름", "차례", "결과타입", "가공식"], ColumnsOf("v_맞춤뷰"));
        var formula = JsonNode.Parse(Read("SELECT 가공식 FROM v_맞춤뷰 WHERE 열식별자 = 'c-2'"))!;
        Assert.Equal("count", formula["flow"]!["steps"]![0]!["op"]!.GetValue<string>());
        Assert.Contains("v_맞춤뷰", Views.Names);
        Assert.Contains("v_맞춤뷰", Views.ReadOnly);
        Assert.DoesNotContain("v_맞춤뷰", Views.Exported);
        Assert.Contains("v_사용자뷰", Views.LegacyNames);

        using (var connection = _database.Open())
            connection.Execute("CREATE VIEW v_사용자뷰 AS SELECT 1 AS 옛;");
        _database.RefreshViews();
        Assert.Empty(Rows("SELECT name FROM sqlite_master WHERE name = 'v_사용자뷰'"));
    }

    /// <summary>맞춤 뷰는 SQL 필드의 읽을 거리가 아니다(§3) — authorizer 와 이름 검사가 막는다.</summary>
    [Fact]
    public void 맞춤_뷰는_SQL_필드가_읽지_못한다()
    {
        Seed();
        _store.SaveUserView(View("문서", (null, "공고명", Doc("notice", "공고명"))), null);
        Assert.DoesNotContain(SqlColumn.Readable, v => v.StartsWith("u_", StringComparison.Ordinal));

        var refused = JsonSerializer.SerializeToElement(_store.ValidateComputedSql("notice", "SELECT count(*) FROM u_문서"));
        Assert.False(refused.GetProperty("ok").GetBoolean());
        Assert.Contains("u_문서", refused.GetProperty("error").GetString());
        var quoted = JsonSerializer.SerializeToElement(_store.ValidateComputedSql("notice", "SELECT count(*) FROM \"u_문서\""));
        Assert.False(quoted.GetProperty("ok").GetBoolean());
        Assert.Throws<InvalidOperationException>(() => _store.SaveUserView(View("둘", (null, "x", Sql("SELECT count(*) FROM u_문서"))), null));
        Assert.Throws<InvalidOperationException>(() => _store.SaveUserView(View("셋", (null, "x", Sql("SELECT count(*) FROM notice"))), null));
    }

    // ── 판올림 ─────────────────────────────────────────────

    /// <summary>
    /// 스키마 24 — 기준을 걷고 정의를 v2 로 옮긴다(§4). 원천 <c>field</c>·<c>items</c> 는 기준 문서를 붙이고, 단계가 읽는 행의 열은
    /// 문서 이름을 붙인다. 공고 기준의 <c>연결계약 |&gt; 꺼내기 f |&gt; 잇기 ", "</c> 그대로는 <c>계약.f</c> 가 되고 그 뷰는 모으기다 —
    /// 앞 판과 같은 값이다. 그 밖의 여러 건 식은 옮기지 않아 깨진 식으로 선다(조용히 다른 값을 내지 않는다).
    /// </summary>
    [Fact]
    public void 판올림은_기준을_걷고_정의를_옮긴다()
    {
        var path = Path.Combine(Path.GetTempPath(), $"pclm-v23-{Guid.NewGuid():N}.db");
        try
        {
            static string Column(string id, string name, object formula) => JsonSerializer.Serialize(new { id, name, formula });
            var notice = "{\"version\":1,\"columns\":[" + string.Join(",",
                Column("n1", "공고명", new { version = 2, source = "flow", flow = new { from = new { kind = "field", field = "공고명" }, steps = Array.Empty<object>() } }),
                Column("n2", "품목 수", new { version = 2, source = "flow", flow = new { from = new { kind = "items" }, steps = new object[] { new { op = "count" } } } }),
                Column("n3", "비면", new { version = 2, source = "flow", flow = new { from = new { kind = "text", text = "" }, steps = new object[] { new { op = "ifEmpty", field = "메모" } } } }),
                Column("n4", "계약번호", new { version = 2, source = "flow", flow = new { from = new { kind = "linked", link = "contract" },
                    steps = new object[] { new { op = "get", field = "계약번호" }, new { op = "join", text = ", " } } } }),
                Column("n5", "계약 수", new { version = 2, source = "flow", flow = new { from = new { kind = "linked", link = "contract" }, steps = new object[] { new { op = "count" } } } }),
                Column("n6", "접수번호", new { version = 2, source = "flow", flow = new { from = new { kind = "linked", link = "request", field = "접수번호" }, steps = Array.Empty<object>() } })) + "]}";
            var contract = "{\"version\":1,\"columns\":[" +
                Column("k1", "번호", new { version = 2, source = "flow", flow = new { from = new { kind = "field", field = "계약번호" }, steps = Array.Empty<object>() } }) + "]}";
            using (var old = new SqliteConnection($"Data Source={path};Pooling=False"))
            {
                old.Open();
                using var command = old.CreateCommand();
                command.CommandText = Schema.Baseline + string.Concat(Schema.Steps.Take(23 - Schema.BaselineVersion)) + """
                    INSERT INTO user_view (id, name, base, definition, sort_order) VALUES ('v-n', '공고문서', 'notice', @notice, 10);
                    INSERT INTO user_view (id, name, base, definition, sort_order) VALUES ('v-k', '계약문서', 'contract', @contract, 20);
                    PRAGMA user_version=23;
                    """;
                command.Parameters.AddWithValue("@notice", notice);
                command.Parameters.AddWithValue("@contract", contract);
                command.ExecuteNonQuery();
            }
            var database = new Database(path);
            database.Migrate();
            Seed(database);
            var store = new Store(database);

            var views = store.UserViews();
            var n = views.Single(v => v.Name == "공고문서").Definition;
            Assert.Equal(2, n.GetProperty("version").GetInt32());
            Assert.Equal("fold", n.GetProperty("contracts").GetString());
            JsonElement Flow(int i) => n.GetProperty("columns")[i].GetProperty("formula").GetProperty("flow");
            Assert.True(JsonNode.DeepEquals(JsonNode.Parse("""{"kind":"linked","link":"notice","field":"공고명"}"""), JsonNode.Parse(Flow(0).GetProperty("from").GetRawText())));
            Assert.True(JsonNode.DeepEquals(JsonNode.Parse("""{"kind":"linked","link":"notice","items":true}"""), JsonNode.Parse(Flow(1).GetProperty("from").GetRawText())));
            Assert.Equal("공고.메모", Flow(2).GetProperty("steps")[0].GetProperty("field").GetString());
            Assert.True(JsonNode.DeepEquals(JsonNode.Parse("""{"kind":"linked","link":"contract","field":"계약번호"}"""), JsonNode.Parse(Flow(3).GetProperty("from").GetRawText())));
            Assert.Equal(0, Flow(3).GetProperty("steps").GetArrayLength());
            Assert.True(JsonNode.DeepEquals(JsonNode.Parse("""{"kind":"linked","link":"contract"}"""), JsonNode.Parse(Flow(4).GetProperty("from").GetRawText())));
            Assert.True(JsonNode.DeepEquals(JsonNode.Parse("""{"kind":"linked","link":"request","field":"접수번호"}"""), JsonNode.Parse(Flow(5).GetProperty("from").GetRawText())));
            Assert.Equal(["n1", "n2", "n3", "n4", "n5", "n6"], n.GetProperty("columns").EnumerateArray().Select(c => c.GetProperty("id").GetString()));
            Assert.Equal(["n5"], views.Single(v => v.Name == "공고문서").Broken.Keys);

            var k = views.Single(v => v.Name == "계약문서").Definition;
            Assert.Equal("split", k.GetProperty("contracts").GetString());
            Assert.Empty(views.Single(v => v.Name == "계약문서").Broken);

            using var consumer = new SqliteConnection($"Data Source={path};Mode=ReadOnly;Pooling=False");
            consumer.Open();
            Assert.Equal("시험 공고 하나|2|확인함|TESTK100, TESTK200||TESTR1-000",
                consumer.QuerySingle<string>("SELECT 공고명 || '|' || \"품목 수\" || '|' || 비면 || '|' || 계약번호 || '|' || \"계약 수\" || '|' || 접수번호 FROM u_공고문서 WHERE 공고명 LIKE '%하나'"));
            Assert.Equal(4, consumer.QuerySingle<long>("SELECT count(*) FROM u_계약문서"));
            Assert.DoesNotContain("base", consumer.Query<string>("SELECT name FROM pragma_table_info('user_view')"));
        }
        finally
        {
            SqliteConnection.ClearAllPools();
            foreach (var suffix in new[] { "", "-wal", "-shm" }) File.Delete(path + suffix);
        }
    }

    /// <summary>
    /// 스키마 26 — 이미 있던 작업자료에 서식용 예시 보기를 <b>한 번</b> 심는다(docs/user-views.md §13). 같은 이름의 보기가 있으면 그것을
    /// 두고, 사람이 지운 뒤 다시 열어도 되살아나지 않는다. 작업자료가 아닌 파일(제출본을 뜬 사본)에는 심지 않는다.
    /// </summary>
    [Fact]
    public void 판올림은_작업자료에만_예시_보기를_한_번_심는다()
    {
        var root = Path.Combine(Path.GetTempPath(), "pclm-v25-" + Guid.NewGuid().ToString("N")[..8]);
        Directory.CreateDirectory(root);
        try
        {
            string Old(string name, string role, string? ownView)
            {
                var path = Path.Combine(root, name);
                using var old = new SqliteConnection($"Data Source={path};Pooling=False");
                old.Open();
                using var command = old.CreateCommand();
                command.CommandText = Schema.Baseline + string.Concat(Schema.Steps.Take(25 - Schema.BaselineVersion)) + """
                    UPDATE pclm_file SET role = @role;
                    PRAGMA user_version=25;
                    """;
                command.Parameters.AddWithValue("@role", role);
                command.ExecuteNonQuery();
                if (ownView is not null)
                {
                    command.CommandText = "INSERT INTO user_view (id, name, definition, sort_order) VALUES ('v-own', '서식용 예시', @definition, 10);";
                    command.Parameters.AddWithValue("@definition", ownView);
                    command.ExecuteNonQuery();
                }
                return path;
            }

            var plain = Old("작업.pclm", PclmRole.Work, null);
            var database = new Database(plain);
            database.Migrate();
            var store = new Store(database);
            var seeded = Assert.Single(store.UserViews());
            Assert.Equal("서식용 예시", seeded.Name);
            Assert.Empty(seeded.Broken);
            // v26 이 그 판의 예시를 심고 v27 이 손대지 않은 그것을 갈아 끼워, v25 에서 오면 지금 구성이다.
            Assert.Equal(BuiltinFieldNames(), ColumnNames(seeded));

            store.DeleteUserView(seeded.Id);
            new Database(plain).Migrate();
            Assert.Empty(new Store(new Database(plain)).UserViews());

            var mine = JsonSerializer.Serialize(new { version = 2, contracts = "split", columns = new[] { new { id = "c-1", name = "내 필드", formula = Doc("notice", "공고명") } } });
            var owned = Old("내것.pclm", PclmRole.Work, mine);
            var ownedDb = new Database(owned);
            ownedDb.Migrate();
            var kept = Assert.Single(new Store(ownedDb).UserViews());
            Assert.Equal("v-own", kept.Id);
            Assert.Equal(["내 필드"], ColumnNames(kept));

            var submission = Old("제출.pclm", PclmRole.Submission, null);
            var copy = new Database(submission);
            copy.Migrate();
            Assert.Empty(new Store(copy).UserViews());
        }
        finally
        {
            SqliteConnection.ClearAllPools();
            Directory.Delete(root, recursive: true);
        }
    }

    private static List<string> BuiltinFieldNames(string? package = null) =>
        [.. JsonNode.Parse(package ?? Store.BuiltinViewPackage())!["views"]![0]!["definition"]!["columns"]!.AsArray().Select(c => c!["name"]!.GetValue<string>())];

    /// <summary>
    /// 스키마 27 — v26 이 심은 예시 보기를 사람이 손대지 않았으면(저장된 정의가 v26 의 것과 같은 JSON — 키 순서는 보지 않는다) 지금
    /// 구성으로 갈아 끼운다. 고친 보기와 지운 보기는 그대로이고, 작업자료가 아닌 파일은 건드리지 않는다.
    /// </summary>
    [Fact]
    public void 판올림은_손대지_않은_예시_보기만_지금_구성으로_갈아_끼운다()
    {
        var root = Path.Combine(Path.GetTempPath(), "pclm-v26-" + Guid.NewGuid().ToString("N")[..8]);
        Directory.CreateDirectory(root);
        try
        {
            // v26 의 파일 — 그 판이 심은 예시를 들었거나(정의를 고칠 수 있게 받는다), 지워서 없다.
            string Old(string name, string role, Func<string, string>? edit)
            {
                var path = Path.Combine(root, name);
                using var old = new SqliteConnection($"Data Source={path};Pooling=False");
                old.Open();
                using (var command = old.CreateCommand())
                {
                    // 단계마다 줄을 끊는다 — v26 은 주석 한 줄뿐이라 붙여 쓰면 뒤의 UPDATE 가 주석에 먹힌다.
                    command.CommandText = Schema.Baseline + string.Concat(Schema.Steps.Take(26 - Schema.BaselineVersion).Select(s => s + "\n")) + """
                        UPDATE pclm_file SET role = @role;
                        PRAGMA user_version=26;
                        """;
                    command.Parameters.AddWithValue("@role", role);
                    command.ExecuteNonQuery();
                }
                if (edit is not null)
                {
                    using var transaction = old.BeginTransaction();
                    Store.SeedExampleViews(old, transaction, Store.ExampleViewPackageV26());
                    var definition = old.QuerySingle<string>("SELECT definition FROM user_view", transaction: transaction);
                    old.Execute("UPDATE user_view SET definition = @definition", new { definition = edit(definition) }, transaction);
                    transaction.Commit();
                }
                return path;
            }

            var v26 = BuiltinFieldNames(Store.ExampleViewPackageV26());
            Assert.NotEqual(v26, BuiltinFieldNames());
            List<string> Migrated(string path)
            {
                var database = new Database(path);
                database.Migrate();
                var view = Assert.Single(new Store(database).UserViews());
                Assert.Equal("서식용 예시", view.Name);
                Assert.Empty(view.Broken);
                return ColumnNames(view);
            }

            // 손대지 않은 것 — 그대로든, 키 순서만 갈렸든 지금 구성이다.
            Assert.Equal(BuiltinFieldNames(), Migrated(Old("그대로.pclm", PclmRole.Work, d => d)));
            Assert.Equal(BuiltinFieldNames(), Migrated(Old("순서.pclm", PclmRole.Work, d =>
            {
                var node = JsonNode.Parse(d)!.AsObject();
                return new JsonObject(node.Reverse().Select(p => KeyValuePair.Create(p.Key, p.Value?.DeepClone()))).ToJsonString();
            })));

            // 고친 것 — 필드 이름 하나만 바꿔도 사람 것이라 그대로 둔다.
            var edited = Migrated(Old("고침.pclm", PclmRole.Work, d => d.Replace("\"게시일\"", "\"공고게시일\"")));
            Assert.Equal(v26.Select(n => n == "게시일" ? "공고게시일" : n), edited);

            // 지운 것 — 되살리지 않는다.
            var deleted = Old("지움.pclm", PclmRole.Work, null);
            var deletedDb = new Database(deleted);
            deletedDb.Migrate();
            Assert.Empty(new Store(deletedDb).UserViews());

            // 작업자료가 아닌 파일은 손대지 않은 예시도 그대로다.
            var submission = new Database(Old("제출.pclm", PclmRole.Submission, d => d));
            submission.Migrate();
            Assert.Equal(v26, ColumnNames(Assert.Single(new Store(submission).UserViews())));
        }
        finally
        {
            SqliteConnection.ClearAllPools();
            Directory.Delete(root, recursive: true);
        }
    }

    // ── 취합 ─────────────────────────────────────────────

    /// <summary>
    /// 제출본에 실려 취합된다. 이름이 같고 정의가 다르면 먼저 온 제출본이 이기고 갈린 것은 겹침으로 알린다(user_column 과 같은
    /// 정의 규칙). 이름이 다르면 둘 다 선다.
    /// </summary>
    [Fact]
    public void 취합은_이름으로_고르고_정의가_다르면_알린다()
    {
        var root = Path.Combine(Path.GetTempPath(), "pclm-uvmerge-" + Guid.NewGuid().ToString("N")[..8]);
        Directory.CreateDirectory(root);
        try
        {
            string Submission(string who, params string[] views)
            {
                var path = Path.Combine(root, $"제출_{who}.pclm");
                var db = PclmFile.Create(path, PclmRole.Submission);
                using (var connection = db.Open())
                    connection.Execute("INSERT INTO app_setting (key, value) VALUES ('submit.name', @who)", new { who });
                var store = new Store(db);
                foreach (var view in views) store.SaveUserView(view, null);
                return path;
            }
            var first = Submission("홍길동", View("문서", ("c-1", "공고명", Doc("notice", "공고명"))), View("홍길동뷰", ("c-1", "번호", Doc("contract", "계약번호"))));
            var second = Submission("김철수", View("문서", "fold", ("c-1", "공고명", Doc("notice", "공고명"))));
            var output = Path.Combine(root, "취합.pclm");

            var report = new Merger().Merge([first, second], output);

            var conflict = Assert.Single(report.겹친것);
            Assert.Equal("맞춤 뷰", conflict.종류);
            Assert.Equal("문서", conflict.키);
            Assert.Equal("홍길동", conflict.실림.제출자);
            Assert.Equal("김철수", Assert.Single(conflict.밀림).제출자);

            var merged = new Store(new Database(output, Access.Read));
            var views = merged.UserViews();
            Assert.Equal(["문서", "홍길동뷰"], views.Select(v => v.Name).Order());
            Assert.Equal("split", views.Single(v => v.Name == "문서").Definition.GetProperty("contracts").GetString());
            Assert.Equal(["공고명"], ColumnNames(views.Single(v => v.Name == "문서")));
            using var consumer = new SqliteConnection($"Data Source={output};Mode=ReadOnly;Pooling=False");
            consumer.Open();
            Assert.Equal(2, consumer.QuerySingle<long>("SELECT count(*) FROM sqlite_master WHERE name LIKE 'u\\_%' ESCAPE '\\'"));
        }
        finally
        {
            SqliteConnection.ClearAllPools();
            try { Directory.Delete(root, recursive: true); } catch { /* 임시 폴더 */ }
        }
    }
}

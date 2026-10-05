using Microsoft.Data.Sqlite;
using Pclm.Core.Storage;
using Xunit;

namespace Pclm.Core.Tests;

/// <summary>
/// 제출과 취합.
///
/// <para>여기서 붙드는 것은 편의가 아니라 <b>조용한 유실</b>이다. 취합은 사람이 적은 것을 서로
/// 밀어내는 일이라, 밀린 것이 알림 없이 사라지면 두 사람 다 그 사실을 모른다. 그래서 기계는
/// 고르기만 하고 <b>무엇을 밀어냈는지 반드시 알린다</b>(ADR-016 의 결) — 그 규율을 이 시험이
/// 붙든다.</para>
///
/// <para>값은 전부 지어낸 것이다. 실제 조달 자료는 시험에 오지 않는다.</para>
/// </summary>
public class MergeTests : IDisposable
{
    private readonly string _root = Path.Combine(
        Path.GetTempPath(), "pclm-merge-" + Guid.NewGuid().ToString("N")[..8]);

    public MergeTests() => Directory.CreateDirectory(_root);

    public void Dispose()
    {
        // 연결 풀이 파일을 쥐고 있으면 임시 폴더가 지워지지 않는다.
        SqliteConnection.ClearAllPools();
        try { Directory.Delete(_root, recursive: true); } catch { /* 지우지 못해도 시험 결과는 그대로다 */ }
        GC.SuppressFinalize(this);
    }

    private string In(string name) => Path.Combine(_root, name);

    private string Out => In("취합.pclm");

    // ── 지어낸 자료 ──────────────────────────────────────────────────

    /// <summary>
    /// 공고 한 장. <b>계열에 건을 함께 넣는다</b> — <c>group_base</c> 가 NOT NULL 이라
    /// 빠뜨리면 <c>OR IGNORE</c> 가 그 위반을 삼켜 계열이 서지 않고, 공고는 들어갔는데
    /// 사람 값도 링크도 붙지 않는 자리가 조용히 생긴다(<c>Store.UpsertNotice</c> 와 같은 꼴).
    /// </summary>
    private static string 공고(string @base, string title, string updated, string seq = "0") =>
        $"""
         INSERT OR IGNORE INTO notice_group  (group_base) VALUES ('{@base}');
         INSERT OR IGNORE INTO notice_series (notice_base, group_base) VALUES ('{@base}', '{@base}');
         INSERT INTO notice (notice_base, seq, title, updated_at)
         VALUES ('{@base}', '{seq}', '{title}', '{updated}');
         """;

    private static string 계약(string @base, string title, string updated, string seq = "0") =>
        $"""
         INSERT OR IGNORE INTO contract_series (contract_base) VALUES ('{@base}');
         INSERT INTO contract (contract_base, seq, title, updated_at)
         VALUES ('{@base}', '{seq}', '{title}', '{updated}');
         """;

    private static string 접수(string @base, string title, string updated, string seq = "0") =>
        $"""
         INSERT OR IGNORE INTO request_series (request_base) VALUES ('{@base}');
         INSERT INTO request (request_base, seq, title, updated_at)
         VALUES ('{@base}', '{seq}', '{title}', '{updated}');
         """;

    private static string 계약품목(string @base, int line, string name, string seq = "0") =>
        $"""
         INSERT INTO contract_item (contract_base, seq, line_no, item_name, quantity)
         VALUES ('{@base}', '{seq}', {line}, '{name}', {line * 10});
         """;

    /// <summary>
    /// 제출본 하나를 짓는다. 남의 설정도 함께 담아 둔다 — 그것이 취합본으로 새지 않는지를
    /// 시험이 봐야 하기 때문이다.
    /// </summary>
    private string 제출본(string 제출자, params string[] sql)
    {
        var path = In($"제출_{제출자}.pclm");
        var database = PclmFile.Create(path, PclmRole.Submission);

        using var connection = database.Open();
        Run(connection, $"INSERT INTO app_setting (key, value) VALUES ('submit.name', '{제출자}');");
        Run(connection, @"INSERT INTO app_setting (key, value) VALUES ('plan.path', 'C:\남의폴더\조달계획.xlsx');");
        foreach (var statement in sql) Run(connection, statement);

        return path;
    }

    private static void Run(SqliteConnection connection, string sql)
    {
        using var command = connection.CreateCommand();
        command.CommandText = sql;
        command.ExecuteNonQuery();
    }

    private static List<string> Read(string db, string sql)
    {
        using var connection = new Database(db).Open();
        using var command = connection.CreateCommand();
        command.CommandText = sql;

        var rows = new List<string>();
        using var reader = command.ExecuteReader();
        while (reader.Read())
            rows.Add(string.Join("|", Enumerable.Range(0, reader.FieldCount)
                .Select(i => reader.IsDBNull(i) ? "" : reader.GetValue(i).ToString())));

        return rows;
    }

    private static long Count(string db, string sql) => long.Parse(Read(db, sql)[0]);

    [Fact]
    public void ERP_원천키와_업체는_채택된_계약의_것을_취합한다()
    {
        string Evidence(string key) => $$"""
            INSERT INTO erp_source VALUES('contract','C-1','0','{"rows":[]}','{{key}}','2026-09-27');
            INSERT INTO erp_row VALUES('contract','C-1','0','contract_item','{{key}}',1);
            INSERT INTO erp_partner(contract_base,seq,line_no,name) VALUES('C-1','0',1,'{{key}}');
            """;
        var first = 제출본("이전", 계약("C-1", "이전", "2026-09-01"), 계약품목("C-1", 1, "이전"), Evidence("old"));
        var last = 제출본("최신", 계약("C-1", "최신", "2026-09-27"), 계약품목("C-1", 1, "최신"), Evidence("new"));
        new Merger().Merge([first, last], Out);
        Assert.Equal(["new"], Read(Out, "SELECT mapping_revision FROM erp_source"));
        Assert.Equal(["new"], Read(Out, "SELECT source_key FROM erp_row"));
        Assert.Equal(["new"], Read(Out, "SELECT name FROM erp_partner"));
        Assert.Equal(0, Count(Out, "SELECT count(*) FROM erp_capture"));
    }

    // ── 겹친 것을 알린다 ────────────────────────────────────────────

    /// <summary>
    /// <b>알림이 본체다.</b> 밀린 쪽이 어느 제출본에서 온 무엇이었는지 나오지 않으면,
    /// 그 사람은 자기가 적은 것이 사라진 줄도 모른다.
    /// </summary>
    [Fact]
    public void 겹친_키를_알리고_늦은_것을_싣는다()
    {
        var 홍 = 제출본("홍길동", 계약("C-1", "먼저 적은 건명", "2026-08-22T09:00:00.0000000Z"));
        var 김 = 제출본("김철수", 계약("C-1", "나중에 적은 건명", "2026-08-28T09:00:00.0000000Z"));

        var report = new Merger().Merge([홍, 김], Out);

        Assert.Equal(["나중에 적은 건명"], Read(Out, "SELECT title FROM contract;"));

        var 겹침 = Assert.Single(report.겹친것);
        Assert.Equal("계약", 겹침.종류);
        Assert.Equal("C-1·0", 겹침.키);
        Assert.Equal("김철수", 겹침.실림.제출자);
        Assert.Equal("홍길동", Assert.Single(겹침.밀림).제출자);
        Assert.Contains("08-22", Assert.Single(겹침.밀림).시각);
    }

    /// <summary>시각이 같으면 먼저 온 제출본을 싣는다 — 그것도 알린다.</summary>
    [Fact]
    public void 시각이_같으면_먼저_온_제출본을_싣고_그것도_알린다()
    {
        var 같은시각 = "2026-08-25T09:00:00.0000000Z";
        var 홍 = 제출본("홍길동", 계약("C-1", "홍길동의 것", 같은시각));
        var 김 = 제출본("김철수", 계약("C-1", "김철수의 것", 같은시각));

        var report = new Merger().Merge([홍, 김], Out);

        Assert.Equal(["홍길동의 것"], Read(Out, "SELECT title FROM contract;"));
        Assert.Equal("홍길동", Assert.Single(report.겹친것).실림.제출자);
    }

    /// <summary>
    /// <b>같은 값을 들고 있는 것은 겹침이 아니라 일치다.</b> 같은 공고문을 둘이 읽었으면 시각만
    /// 갈리는데, 그것까지 한 줄씩 내면 진짜 갈린 둘이 수백 줄에 묻힌다 — 알림이 본체이므로
    /// 알림이 쓸모를 잃으면 안 된다.
    /// </summary>
    [Fact]
    public void 값이_같으면_겹친_것으로_내지_않는다()
    {
        var 홍 = 제출본("홍길동", 공고("N-1", "같은 공고문", "2026-08-22T09:00:00.0000000Z"));
        var 김 = 제출본("김철수", 공고("N-1", "같은 공고문", "2026-08-28T09:00:00.0000000Z"));

        var report = new Merger().Merge([홍, 김], Out);

        Assert.Empty(report.겹친것);
        Assert.Equal(1, report.공고);

        // 값이 하나라도 갈리면 그때는 반드시 낸다.
        var 박 = 제출본("박영수", 공고("N-1", "다르게 읽힌 공고문", "2026-08-30T09:00:00.0000000Z"));
        Assert.Single(new Merger().Merge([홍, 김, 박], Out).겹친것);
    }

    /// <summary>
    /// 딸린 줄은 <b>부모를 따라 통째로</b> 간다. 반씩 섞으면 품목이 두 제출본에서 뒤섞여
    /// <b>어느 문서에도 없던 표</b>가 선다.
    /// </summary>
    [Fact]
    public void 딸린_줄은_이긴_제출본의_것만_실린다()
    {
        var 홍 = 제출본("홍길동",
            계약("C-1", "옛 차수", "2026-08-22T09:00:00.0000000Z"),
            계약품목("C-1", 1, "홍길동이 읽은 품목"),
            계약품목("C-1", 2, "홍길동만 가진 둘째 줄"));

        var 김 = 제출본("김철수",
            계약("C-1", "새 차수", "2026-08-28T09:00:00.0000000Z"),
            계약품목("C-1", 1, "김철수가 읽은 품목"));

        new Merger().Merge([홍, 김], Out);

        // 이긴 쪽의 줄만 남는다. 진 쪽의 둘째 줄이 따라 들어오면 없던 표가 된다.
        Assert.Equal(["김철수가 읽은 품목"], Read(Out, "SELECT item_name FROM contract_item ORDER BY line_no;"));
    }

    /// <summary>
    /// <c>request_link.notice_group</c> 은 UNIQUE 다. 두 사람이 같은 건에 <b>다른 접수</b>를 이어
    /// 두는 것은 인수인계 직후에 정상이라, 취합이 거기서 터지면 안 된다.
    /// </summary>
    [Fact]
    public void 한_공고에_접수가_둘이어도_터지지_않는다()
    {
        var 홍 = 제출본("홍길동",
            접수("R-1", "홍길동이 맡은 접수", "2026-08-20T09:00:00.0000000Z"),
            공고("N-1", "같은 공고", "2026-08-20T09:00:00.0000000Z"),
            """
            INSERT INTO request_link (request_base, notice_group, confidence, confirmed_at)
            VALUES ('R-1', 'N-1', 1.0, '2026-08-22T09:00:00.0000000Z');
            """);

        var 김 = 제출본("김철수",
            접수("R-2", "김철수가 맡은 접수", "2026-08-20T09:00:00.0000000Z"),
            공고("N-1", "같은 공고", "2026-08-20T09:00:00.0000000Z"),
            """
            INSERT INTO request_link (request_base, notice_group, confidence, confirmed_at)
            VALUES ('R-2', 'N-1', 1.0, '2026-08-28T09:00:00.0000000Z');
            """);

        var report = new Merger().Merge([홍, 김], Out);

        // 늦게 이은 쪽만 실린다. 둘 다 넣으면 UNIQUE 위반으로 취합이 통째로 터진다.
        Assert.Equal(["R-2|N-1"], Read(Out, "SELECT request_base, notice_group FROM request_link;"));

        var 겹침 = Assert.Single(report.겹친것);
        Assert.Equal("공고 하나에 접수 둘", 겹침.종류);
        Assert.Equal("N-1", 겹침.키);
        Assert.Equal("김철수", 겹침.실림.제출자);
        Assert.Equal("홍길동", Assert.Single(겹침.밀림).제출자);
    }

    // ── 판 ──────────────────────────────────────────────────────────

    /// <summary>
    /// 기준선보다 옛 시험판 제출본은 올릴 단계가 이 프로그램에 없다. 판이 새로운 것처럼
    /// <b>거절하되 멈추지는 않는다</b> — <c>Migrate</c> 의 예외에 취합 전체가 멎으면 나머지
    /// 사람의 제출본까지 함께 묶인다.
    /// </summary>
    [Fact]
    public void 옛_시험판_제출본은_거절하고_나머지는_합친다()
    {
        var 옛판 = In("제출_옛판.pclm");
        using (var connection = new SqliteConnection($"Data Source={옛판}"))
        {
            connection.Open();
            Run(connection, "CREATE TABLE notice (notice_base TEXT, seq TEXT);");
            Run(connection, "PRAGMA user_version = 11;");
        }

        var 홍 = 제출본("홍길동", 공고("N-1", "지금 판에서 온 공고", "2026-08-20T09:00:00.0000000Z"));

        var report = new Merger().Merge([옛판, 홍], Out);

        Assert.Equal([new MergeRejection("제출_옛판.pclm", Merger.옛시험판)], report.거절한제출본);
        Assert.Equal(1, report.제출본);
        Assert.Equal(["지금 판에서 온 공고"], Read(Out, "SELECT title FROM notice;"));
        Assert.Contains("0.7.0", MergeReportText.Render(report, Out));
    }

    /// <summary>판이 새로운 것은 내릴 길이 없다. <b>거절하되 멈추지는 않는다.</b></summary>
    [Fact]
    public void 새_판_제출본은_거절하고_나머지는_합친다()
    {
        var 새판 = 제출본("앞서간이", 공고("N-앞", "새 판에서 온 공고", "2026-08-20T09:00:00.0000000Z"));
        using (var connection = new Database(새판).Open())
            Run(connection, $"PRAGMA user_version = {Schema.Version + 7};");

        var 홍 = 제출본("홍길동", 공고("N-1", "지금 판에서 온 공고", "2026-08-20T09:00:00.0000000Z"));

        var report = new Merger().Merge([새판, 홍], Out);

        Assert.Equal([new MergeRejection("제출_앞서간이.pclm", Merger.새판)], report.거절한제출본);
        Assert.Equal(1, report.제출본);
        Assert.Equal(["지금 판에서 온 공고"], Read(Out, "SELECT title FROM notice;"));
    }

    // ── 매번 처음부터 짓는다 ────────────────────────────────────────

    /// <summary>
    /// <b>묘비를 두지 않으므로</b> 취합본을 고쳐 쓸 수 없다. 고쳐 쓰면 누가 자기 DB 에서 지운
    /// 건이 다음 취합본에 영영 남는다.
    /// </summary>
    [Fact]
    public void 지운_것은_다음_취합본에_없다()
    {
        var 처음 = 제출본("홍길동",
            공고("N-1", "남을 공고", "2026-08-20T09:00:00.0000000Z"),
            공고("N-2", "지울 공고", "2026-08-20T09:00:00.0000000Z"));

        Assert.Equal(2, new Merger().Merge([처음], Out).공고);

        // 사람이 자기 DB 에서 N-2 를 지우고 다시 제출했다.
        using (var connection = new Database(처음).Open())
        {
            Run(connection, "DELETE FROM notice WHERE notice_base = 'N-2';");
            Run(connection, "DELETE FROM notice_series WHERE notice_base = 'N-2';");
        }

        var report = new Merger().Merge([처음], Out);

        Assert.Equal(1, report.공고);
        Assert.Equal(["남을 공고"], Read(Out, "SELECT title FROM notice;"));
    }

    // ── 사람이 세운 것 ──────────────────────────────────────────────

    /// <summary>
    /// 사용자 열은 <b>합집합</b>이다 — 사람마다 세운 열이 달라, 이름이 같으면 하나로 다르면
    /// 둘 다 세운다. 값은 계열 키에 매달려 그대로 따라온다.
    /// </summary>
    [Fact]
    public void 사용자_열은_합집합이고_값이_따라온다()
    {
        var 홍 = 제출본("홍길동",
            계약("C-1", "홍길동의 계약", "2026-08-20T09:00:00.0000000Z"),
            """
            INSERT INTO user_column (entity_type, field_name, kind, options, sort_order)
            VALUES ('contract', '홍길동칸', 'text', NULL, 50);
            INSERT INTO contract_user_field (contract_base, field_name, value, updated_at)
            VALUES ('C-1', '홍길동칸', '홍길동이 적은 값', '2026-08-22T09:00:00.0000000Z');
            """);

        var 김 = 제출본("김철수",
            공고("N-1", "김철수의 공고", "2026-08-20T09:00:00.0000000Z"),
            """
            INSERT INTO user_column (entity_type, field_name, kind, options, sort_order)
            VALUES ('notice', '김철수칸', 'choice', '가|나|다', 60);
            INSERT INTO notice_user_field (notice_base, field_name, value, updated_at)
            VALUES ('N-1', '김철수칸', '나', '2026-08-22T09:00:00.0000000Z');
            """);

        new Merger().Merge([홍, 김], Out);

        Assert.Contains("contract|홍길동칸", Read(Out, "SELECT entity_type, field_name FROM user_column;"));
        Assert.Contains("notice|김철수칸", Read(Out, "SELECT entity_type, field_name FROM user_column;"));

        // 뷰는 열 정의를 읽어 짓는 것이라, 다시 짓지 않으면 그 칸이 계약면에 서지 않는다.
        Assert.Equal(["홍길동이 적은 값"], Read(Out, "SELECT 홍길동칸 FROM v_계약;"));
        Assert.Equal(["나"], Read(Out, "SELECT 김철수칸 FROM v_공고;"));
    }

    /// <summary>이름이 같은데 뜻이 다르면 먼저 온 것을 두고 <b>알린다</b> — 누가 옳은지 기계가 가리지 않는다.</summary>
    [Fact]
    public void 같은_이름의_열이_뜻이_다르면_알린다()
    {
        var 홍 = 제출본("홍길동", """
            INSERT INTO user_column (entity_type, field_name, kind, options, sort_order)
            VALUES ('contract', '진행단계', 'choice', '준비|계약', 50);
            """);

        var 김 = 제출본("김철수", """
            INSERT INTO user_column (entity_type, field_name, kind, options, sort_order)
            VALUES ('contract', '진행단계', 'choice', '준비|투찰|낙찰|계약', 50);
            """);

        var report = new Merger().Merge([홍, 김], Out);

        Assert.Equal(["준비|계약"],
            Read(Out, "SELECT options FROM user_column WHERE field_name = '진행단계';"));

        var 겹침 = Assert.Single(report.겹친것);
        Assert.Equal("사용자 열", 겹침.종류);
        Assert.Equal("contract·진행단계", 겹침.키);
    }

    // ── 새지 않는다 ─────────────────────────────────────────────────

    /// <summary>
    /// 제출한 사람의 설정은 <b>넘어오지 않는다.</b> 내 이름·계획 엑셀 자리가 섞이면
    /// 취합본을 연 사람이 남의 설정을 자기 것으로 본다.
    /// </summary>
    /// <summary>
    /// 취합기가 아는 표가 <b>스키마의 표 전부</b>인가.
    ///
    /// <para>표를 하나 세우고 <c>Merger.표들</c> 을 고치지 않으면, 그 표는 취합본에서
    /// <b>말없이 빠진다</b> — 넣은 사람도 받은 사람도 빠진 줄을 모른다. 판올림은 앞으로만 가고
    /// 표는 앞으로도 늘 것이라, 이 어긋남을 사람의 기억이 아니라 시험이 잡는다.</para>
    ///
    /// <para>두고 오는 것(<c>app_setting</c>)은 <see cref="Merger.Untouched"/> 에 적힌 것만이다 —
    /// 새 표를 그냥 빠뜨린 것과 <b>일부러</b> 두고 온 것을 여기서 가른다.</para>
    /// </summary>
    [Fact]
    public void 취합기가_아는_표가_스키마의_표_전부다()
    {
        var database = PclmFile.Create(In("스키마.db"), PclmRole.Work);

        using var connection = database.OpenReadOnly();
        using var command = connection.CreateCommand();
        command.CommandText =
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%';";

        var 실제 = new List<string>();
        using (var reader = command.ExecuteReader())
            while (reader.Read()) 실제.Add(reader.GetString(0));

        var 아는것 = Merger.Tables.Concat(Merger.Untouched).ToHashSet();

        Assert.Empty(실제.Where(t => !아는것.Contains(t)));          // 새 표를 빠뜨렸다
        Assert.Empty(아는것.Where(t => !실제.Contains(t)));          // 없는 표를 나르려 한다
    }

    [Fact]
    public void 남의_설정은_취합본으로_넘어오지_않는다()
    {
        var 홍 = 제출본("홍길동", 공고("N-1", "공고", "2026-08-20T09:00:00.0000000Z"));

        new Merger().Merge([홍], Out);

        Assert.Equal(0, Count(Out, "SELECT COUNT(*) FROM app_setting;"));
    }

    /// <summary>
    /// 취합본은 <b>한 파일</b>이다. 메일에 붙거나 폴더째 복사되는 결과물이라, <c>-wal</c> 이
    /// 옆에 남으면 받는 쪽이 본체만 가져가 마지막 것이 빠진 자료를 연다.
    /// </summary>
    [Fact]
    public void 취합본은_한_파일로_떨어진다()
    {
        var 홍 = 제출본("홍길동", 공고("N-1", "공고", "2026-08-20T09:00:00.0000000Z"));

        new Merger().Merge([홍], Out);

        Assert.True(File.Exists(Out));
        Assert.False(File.Exists(Out + "-wal"));
        Assert.False(File.Exists(Out + "-shm"));
    }

    /// <summary>조용히 깨진 취합본을 내놓지 않는다.</summary>
    [Fact]
    public void 취합_뒤_외래키가_성하다()
    {
        var 홍 = 제출본("홍길동",
            접수("R-1", "접수", "2026-08-20T09:00:00.0000000Z"),
            공고("N-1", "공고", "2026-08-20T09:00:00.0000000Z"),
            계약("C-1", "계약", "2026-08-20T09:00:00.0000000Z"),
            계약품목("C-1", 1, "품목"),
            """
            INSERT INTO request_link (request_base, notice_group, confidence, confirmed_at)
            VALUES ('R-1', 'N-1', 1.0, '2026-08-22T09:00:00.0000000Z');
            INSERT INTO project_link (contract_base, notice_group, confidence, confirmed_at)
            VALUES ('C-1', 'N-1', 1.0, '2026-08-22T09:00:00.0000000Z');
            INSERT INTO contract_user_field (contract_base, field_name, value, updated_at)
            VALUES ('C-1', '메모', '지어낸 메모', '2026-08-22T09:00:00.0000000Z');
            """);

        var 김 = 제출본("김철수",
            공고("N-2", "다른 공고", "2026-08-20T09:00:00.0000000Z"),
            """
            INSERT INTO link_rejection (contract_base, notice_group, rejected_at)
            SELECT 'C-9', 'N-2', '2026-08-22T09:00:00.0000000Z'
            WHERE 0;
            """);

        var report = new Merger().Merge([홍, 김], Out);

        Assert.Empty(Read(Out, "PRAGMA foreign_key_check;"));
        Assert.Equal(2, report.제출본);
    }

    /// <summary>제출자 이름을 정하지 않았으면 파일 이름으로 부른다.</summary>
    [Fact]
    public void 이름이_비면_파일_이름으로_부른다()
    {
        var 이름없음 = In("제출_20260729.pclm");
        var database = PclmFile.Create(이름없음, PclmRole.Work);
        using (var connection = database.Open())
            Run(connection, 계약("C-1", "이름 없는 이의 계약", "2026-08-28T09:00:00.0000000Z"));

        var 홍 = 제출본("홍길동", 계약("C-1", "홍길동의 계약", "2026-08-20T09:00:00.0000000Z"));

        var report = new Merger().Merge([이름없음, 홍], Out);

        Assert.Equal("제출_20260729", Assert.Single(report.겹친것).실림.제출자);
    }

    // ── 걷기와 보고 ─────────────────────────────────────────────────

    /// <summary>
    /// 지난 취합본은 제출본으로 세지 않는다 — <b>취합본이 취합본을 먹으면</b> 지난 취합에서
    /// 밀려난 것이 남의 이름을 달고 되살아난다. 가르는 것은 <b>이름이 아니라 역할</b>이다:
    /// 이름을 바꾼 취합본도 빠지고, <c>취합_</c> 으로 시작하는 제출본도 들어간다.
    /// </summary>
    [Fact]
    public void 취합본은_이름이_아니라_역할로_거절한다()
    {
        var 홍 = 제출본("홍길동", 계약("C-1", "홍길동의 것", "2026-08-22T09:00:00.0000000Z"));
        제출본("김철수", 계약("C-2", "김철수의 것", "2026-08-23T09:00:00.0000000Z"));

        // 지난 취합본이 같은 폴더에 있다 — 누가 이름을 바꿔 두었다.
        new Merger().Merge([홍], In("지난것.pclm"));

        // 이름이 취합_ 으로 시작하는 제출본. 예전에는 말없이 빠졌다.
        var work = PclmFile.Create(Path.Combine(_root, "작업", "내것.pclm"), PclmRole.Work);
        using (var connection = work.Open())
            Run(connection, 계약("C-3", "이름이 헷갈리는 이의 것", "2026-08-24T09:00:00.0000000Z"));
        PclmFile.Snapshot(work, In("취합_이몽룡.pclm"), PclmRole.Submission, overwrite: false);

        Assert.Equal(
            ["제출_김철수.pclm", "제출_홍길동.pclm", "지난것.pclm", "취합_이몽룡.pclm"],
            Merger.FindSubmissions(_root).Select(Path.GetFileName));

        var report = new Merger().Merge(Merger.FindSubmissions(_root), Out);

        Assert.Equal([new MergeRejection("지난것.pclm", "취합본")], report.거절한제출본);
        Assert.Equal(3, report.제출본);
        Assert.Equal(["C-1", "C-2", "C-3"], Read(Out, "SELECT contract_base FROM contract ORDER BY 1;"));
        Assert.Contains("지난것.pclm — 취합본", MergeReportText.Render(report, Out));
    }

    /// <summary>
    /// 남의 파일은 읽기만 한다 — 판을 올리는 것도 사본에서다. PCLM 이 아닌 파일은 까닭을 달아 거절하고
    /// 나머지는 합친다.
    /// </summary>
    [Fact]
    public void 입력_파일은_한_바이트도_바뀌지_않고_PCLM_아닌_것은_까닭과_함께_거절한다()
    {
        var work = PclmFile.Create(Path.Combine(_root, "작업", "내것.pclm"), PclmRole.Work);
        using (var connection = work.Open())
            Run(connection, 계약("C-1", "지어낸 계약", "2026-08-24T09:00:00.0000000Z"));
        var 제출 = PclmFile.Snapshot(work, In("제출_홍길동.pclm"), PclmRole.Submission, overwrite: false);

        var 엉뚱 = In("제출_엉뚱.pclm");
        using (var connection = new SqliteConnection($"Data Source={엉뚱};Pooling=False"))
        {
            connection.Open();
            Run(connection, "CREATE TABLE notice (notice_base TEXT, seq TEXT);");
        }

        SqliteConnection.ClearAllPools();
        var before = (File.ReadAllBytes(제출), File.ReadAllBytes(엉뚱));

        var report = new Merger().Merge([제출, 엉뚱], Out);

        Assert.Equal([new MergeRejection("제출_엉뚱.pclm", Merger.PCLM아님)], report.거절한제출본);
        Assert.Equal(1, report.제출본);
        Assert.Contains("제출_엉뚱.pclm — PCLM 파일이 아님", MergeReportText.Render(report, Out));

        SqliteConnection.ClearAllPools();
        Assert.Equal(before.Item1, File.ReadAllBytes(제출));
        Assert.Equal(before.Item2, File.ReadAllBytes(엉뚱));
        foreach (var file in new[] { 제출, 엉뚱 })
        {
            Assert.False(File.Exists(file + "-wal"));
            Assert.False(File.Exists(file + "-shm"));
        }

        // 취합본은 자기 역할과 자기 신원을 갖는다.
        var info = PclmFile.Inspect(Out);
        Assert.Equal((PclmKind.Ok, PclmRole.Merged), (info.Kind, info.Role));
        Assert.NotEqual(PclmFile.Inspect(제출).DatasetId, info.DatasetId);
    }

    /// <summary>
    /// 보고 <b>글</b>이 알림의 본체다. 창은 이것을 취합본 옆에 파일로 남기고 명령줄은 그대로
    /// 찍는다 — 밀린 쪽이 누구였는지 여기 서지 않으면 볼 데가 없다.
    /// </summary>
    [Fact]
    public void 보고_글에_겹친_것이_한_줄씩_선다()
    {
        var 홍 = 제출본("홍길동", 공고("N-1", "먼저 읽은 공고문", "2026-08-22T09:00:00.0000000Z"));
        var 김 = 제출본("김철수", 공고("N-1", "다르게 읽은 공고문", "2026-08-28T09:00:00.0000000Z"));

        var text = MergeReportText.Render(new Merger().Merge([홍, 김], Out), Out);

        Assert.Contains("겹친 것 1건", text);
        Assert.Contains("김철수", text);
        Assert.Contains("홍길동", text);
        Assert.Contains("◀ 실림", text);
        Assert.Contains(Out, text);
    }

    // ── 제출 ────────────────────────────────────────────────────────

    /// <summary>
    /// 제출본은 <c>VACUUM INTO</c> 로 뜬 <b>한 파일</b>이다 — <c>-wal</c>·<c>-shm</c> 을 딸려
    /// 보내지 않아, 받는 쪽이 파일 하나만 복사해도 알맹이가 빠지지 않는다.
    /// </summary>
    [Fact]
    public void 제출본을_다시_열면_같은_것이_들어_있다()
    {
        var path = In("원본.db");
        var database = PclmFile.Create(path, PclmRole.Work);

        using (var connection = database.Open())
        {
            Run(connection, 계약("C-1", "지어낸 계약", "2026-08-20T09:00:00.0000000Z"));
            Run(connection, 계약품목("C-1", 1, "지어낸 품목"));
        }

        var saved = database.Snapshot(In("제출_홍길동_20260729.pclm"));

        Assert.True(File.Exists(saved));
        Assert.False(File.Exists(saved + "-wal"));
        Assert.False(File.Exists(saved + "-shm"));

        Assert.Equal(["C-1|지어낸 계약"], Read(saved, "SELECT contract_base, title FROM contract;"));
        Assert.Equal(["지어낸 품목"], Read(saved, "SELECT item_name FROM contract_item;"));
        Assert.Equal(Schema.Version.ToString(), Read(saved, "PRAGMA user_version;")[0]);
    }

    /// <summary>덮어쓰기를 부르지 않으면 이미 있는 파일을 지우지 않는다 — 자리 옮기기의 방벽이다.</summary>
    [Fact]
    public void 덮어쓰기를_부르지_않으면_있는_파일을_지키다()
    {
        var path = In("원본.db");
        var database = PclmFile.Create(path, PclmRole.Work);

        var 이미있음 = In("이미있음.pclm");
        File.WriteAllText(이미있음, "남의 것");

        Assert.ThrowsAny<Exception>(() => database.Snapshot(이미있음));
        Assert.Equal("남의 것", File.ReadAllText(이미있음));

        // 사람이 자리를 손수 짚어 뜨는 제출본만 덮는다.
        database.Snapshot(이미있음, overwrite: true);
        Assert.NotEqual("남의 것", File.ReadAllText(이미있음));
    }
}

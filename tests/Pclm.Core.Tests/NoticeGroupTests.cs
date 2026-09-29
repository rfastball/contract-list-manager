using Dapper;
using Microsoft.Data.Sqlite;
using Pclm.Core.Domain;
using Pclm.Core.Storage;
using Xunit;

namespace Pclm.Core.Tests;

/// <summary>
/// 관련공고를 타고 <b>갈린 본번호를 한 건으로 묶는다</b>(스키마 V15).
///
/// <para>ADR-025 는 관련공고를 담되 판정하지 않기로 했다. 여기서 넓힌다 — 담아 둔 것을 이제
/// <b>읽는다</b>. 취소 후 재공고는 본번호가 갈려 차수로는 영영 이어지지 않는데, 접수도 계약도
/// 하나인 조달 건이 둘로 서면 링커의 후보가 둘이 되어 기계가 스스로 잇지 못한다.</para>
///
/// <para>여기서 붙드는 것은 대부분 <b>오류 없이 자료를 잃는 자리</b>다 — 계열이 서지 않는
/// 업서트, 캐스케이드가 데려가는 링크와 사람 메모, 한 건에 둘이 되어 밀려나는 접수.
/// 넷 다 아무것도 실패하지 않으므로 시험이 아니면 아무도 알아채지 못한다.</para>
///
/// <para>번호는 실측 코퍼스의 것이지만 담긴 값은 모두 지어낸 것이다.</para>
/// </summary>
public class NoticeGroupTests : IDisposable
{
    private readonly string _path = Path.Combine(
        Path.GetTempPath(), $"pclm-notice-group-{Guid.NewGuid():N}.db");

    private readonly Database _database;
    private readonly Store _store;

    public NoticeGroupTests()
    {
        _database = new Database(_path);
        _database.Migrate();
        _store = new Store(_database);
    }

    // ── 묶기 ─────────────────────────────────────────────────────

    /// <summary>
    /// 조립대. 당초 → 취소 → <b>재채번된 재공고</b> 세 장이 한 건으로 선다.
    /// 건의 이름은 가장 이른 본번호이고, 사람에게 보일 것은 그 건의 <b>현행 공고</b>다.
    /// </summary>
    [Fact]
    public void 재공고는_원공고와_한_건이_된다()
    {
        Put("R26BK09011054", "000");
        Put("R26BK09011054", "001", "R26BK09011054-000");
        Put("R26BK09012082", "000", "R26BK09011054-001", "R26BK09011054-000");

        Assert.Equal(["R26BK09011054"], 건들());
        Assert.Equal("R26BK09011054", 건("R26BK09011054"));
        Assert.Equal("R26BK09011054", 건("R26BK09012082"));

        // 계열 둘이 각각 줄을 갖되(ADR-025), 두 줄 다 같은 건에 서고 현행은 재공고다.
        var rows = Rows();
        Assert.Equal(2, rows.Count);
        Assert.All(rows, r => Assert.Equal("R26BK09011054", r["공고건"]));
        Assert.All(rows, r => Assert.Equal("R26BK09012082-000", r["현행공고"]));
    }

    /// <summary>
    /// 절단기. 변경 뒤 다시 내려받은 <b>재출력본</b>(<c>-000</c>)이 자기 뒤차수를 가리킨다 —
    /// 화살표 방향을 믿을 수 없다는 실측이다.
    ///
    /// <para>그 앞가리킴이 최신 차수를 <b>대체된 것으로 만들면 안 된다</b>. 같은 본번호 안은
    /// 차수가 이미 정하고 있어서, 현행을 고를 때 같은 본번호의 가리킴은 일부러 뺀다.</para>
    /// </summary>
    [Fact]
    public void 재출력본의_앞가리킴은_최신_차수를_대체하지_않는다()
    {
        Put("R26BK09017030", "000", "R26BK09017030-001");
        Put("R26BK09017030", "001", "R26BK09017030-000");

        Assert.Equal(["R26BK09017030"], 건들());
        Assert.Equal("R26BK09017030-001", Assert.Single(Rows())["현행공고"]);
    }

    /// <summary>
    /// <b>문서 없는 조상이 건의 이름이 된다.</b> 가스성분분석기가 가리키는
    /// <c>R26BK09013019</c> 는 코퍼스에 없다 — 재공고 건에서는 그것이 예외가 아니라 보통이다.
    ///
    /// <para>이름을 문서 있는 쪽으로 잡으면, 나중에 그 조상이 들어오는 날 건의 이름이 바뀌고
    /// 거기 매어 둔 링크가 조용히 옮겨 다닌다.</para>
    /// </summary>
    [Fact]
    public void 문서가_없는_조상도_건의_이름이_된다()
    {
        Put("R26BK09014091", "000", "R26BK09013019-000");

        Assert.Equal(["R26BK09013019"], 건들());
        Assert.Equal("R26BK09013019", 건("R26BK09014091"));

        // 이름은 없는 문서의 것이라도, 보이는 것은 서 있는 공고다.
        Assert.Equal("R26BK09014091-000", Assert.Single(Rows())["현행공고"]);
    }

    /// <summary>
    /// 넣는 차례가 건의 이름을 바꾸지 못한다. 재공고서가 자기가 대신하는 번호를 적어 두므로,
    /// 재공고를 먼저 넣어도 이름은 원공고이고 당초가 나중에 들어와도 <b>그대로다</b>.
    /// </summary>
    [Fact]
    public void 재공고를_먼저_넣어도_건_이름이_흔들리지_않는다()
    {
        Put("R26BK09012082", "000", "R26BK09011054-001", "R26BK09011054-000");
        Assert.Equal("R26BK09011054", 건("R26BK09012082"));

        Put("R26BK09011054", "000");
        Put("R26BK09011054", "001", "R26BK09011054-000");

        Assert.Equal(["R26BK09011054"], 건들());
        Assert.Equal("R26BK09011054", 건("R26BK09012082"));
    }

    /// <summary>
    /// <b>공고를 넣으면 계열이 반드시 선다.</b> <c>group_base</c> 가 NOT NULL 이 된 뒤로
    /// <c>INSERT OR IGNORE INTO notice_series (notice_base)</c> 는 그 위반을 <b>삼킨다</b> —
    /// 오류 한 줄 없이 공고만 들어가고 사람 값도 링크도 붙지 않는 자리가 생긴다.
    /// </summary>
    [Fact]
    public void 공고를_넣으면_계열과_건이_함께_선다()
    {
        Put("R26BK09011054", "000");

        Assert.Equal(1, Count("SELECT COUNT(*) FROM notice_series;"));
        Assert.Equal("R26BK09011054", 건("R26BK09011054"));
        Assert.Equal(["R26BK09011054"], 건들());
    }

    // ── 한 규칙, 한 벌 ───────────────────────────────────────────

    /// <summary>
    /// <b>뷰가 낸 현행과 기계가 견준 현행이 같다.</b> 규칙은 한 벌뿐이라
    /// (<see cref="NoticeGroups.현행공고"/> 가 정본이고 뷰는 그것을 파생표로 감싼다)
    /// 지금은 갈릴 자리가 없지만, 갈라 적는 것이 <b>아무것도 실패시키지 않는</b> 데서 이
    /// 시험이 필요하다 — 갈리면 화면이 보여 준 공고와 링커가 견준 공고가 달라지고,
    /// 그때 틀리는 것은 값이 아니라 <b>무엇을 무엇에 이었는가</b>다.
    ///
    /// <para>표본 셋을 한 DB 에 함께 세운다. 셋이 서로 다른 까닭으로 어려운 자리라
    /// (재채번·앞가리킴·문서 없는 조상) 하나만으로는 규칙이 갈린 것을 놓친다.</para>
    /// </summary>
    [Fact]
    public void 뷰의_현행공고가_정본과_같다()
    {
        // 조립대 — 당초 → 취소 → 재채번된 재공고.
        Put("R26BK09011054", "000");
        Put("R26BK09011054", "001", "R26BK09011054-000");
        Put("R26BK09012082", "000", "R26BK09011054-001", "R26BK09011054-000");

        // 절단기 — 재출력본이 자기 뒤차수를 가리킨다.
        Put("R26BK09017030", "000", "R26BK09017030-001");
        Put("R26BK09017030", "001", "R26BK09017030-000");

        // 가스성분분석기 — 가리키는 조상이 DB 에 없다.
        Put("R26BK09014091", "000", "R26BK09013019-000");

        List<string> 기대 =
        [
            "R26BK09011054|R26BK09012082-000",
            "R26BK09013019|R26BK09014091-000",
            "R26BK09017030|R26BK09017030-001",
        ];

        Assert.Equal(기대, Read(
            $"""
            SELECT group_base, notice_base || '-' || seq
            FROM ({NoticeGroups.현행공고}) ORDER BY group_base;
            """));

        Assert.Equal(기대, Read(
            "SELECT DISTINCT 공고건, 현행공고 FROM v_공고_v1 ORDER BY 공고건;"));
    }

    // ── 차수를 편 표 ─────────────────────────────────────────────

    /// <summary>
    /// <c>v_통합_v3</c> 의 줄 하나는 <b>공고 문서 한 장</b>이다 — 조립대 세 장, 절단기 두 장,
    /// 흔한 1:1:1 은 한 장. 그 사이에 <c>v_통합_v2</c> 의 줄 수는 <b>건 수</b> 그대로다.
    ///
    /// <para>같은 자료를 두 표가 다르게 세는 것이 여기서는 옳다. v2 는 "조달 건이 몇이냐" 를,
    /// v3 은 "그 건에 문서가 몇 장 있었느냐" 를 낸다.</para>
    /// </summary>
    [Fact]
    public void 통합_v3_은_공고_한_장마다_한_줄이다()
    {
        Put("R26BK09011054", "000");
        Put("R26BK09011054", "001", "R26BK09011054-000");
        Put("R26BK09012082", "000", "R26BK09011054-001", "R26BK09011054-000");

        Put("R26BK09017030", "000", "R26BK09017030-001");
        Put("R26BK09017030", "001", "R26BK09017030-000");

        Put("R26BK09017075", "000");

        Assert.Equal(
            ["R26BK09011054|3", "R26BK09017030|2", "R26BK09017075|1"],
            Read("SELECT 공고건, COUNT(*) FROM v_통합_v3 GROUP BY 공고건 ORDER BY 공고건;"));

        Assert.Equal(6, Count("SELECT COUNT(*) FROM v_통합_v3;"));
        Assert.Equal(3, Count("SELECT COUNT(*) FROM v_통합_v2;"));
        Assert.Equal(3, Count("SELECT COUNT(*) FROM notice_group;"));
    }

    /// <summary>
    /// 접수·계약 열은 그 건의 공고 줄마다 <b>되풀이된다</b>. 줄이 공고라 그것이 옳은 모습이다 —
    /// v_통합_v2 에서 계약이 여럿인 건의 공고 열이 되풀이되는 것과 같은 자리다.
    ///
    /// <para>옛 차수 줄에도 「현행공고」는 <b>그 건의 현행</b>이 선다. 그래서 지금 서 있는
    /// 것이 무엇인지가 지나간 줄에서도 보인다.</para>
    /// </summary>
    [Fact]
    public void 통합_v3_에서_접수와_계약이_공고_줄마다_되풀이된다()
    {
        Put("R26BK09011054", "000");
        Put("R26BK09011054", "001", "R26BK09011054-000");
        Put("R26BK09012082", "000", "R26BK09011054-001", "R26BK09011054-000");

        _store.UpsertRequest(접수("MBKLMH26930006"));
        _store.UpsertContract(계약("R26TA09080469"));

        var 원공고 = new EntityRef("notice", "R26BK09011054", "000");
        new RequestLinker(_database).Confirm(
            new EntityRef("request", "MBKLMH26930006", "000"), 원공고, 1.0);
        new Linker(_database).Confirm(
            new EntityRef("contract", "R26TA09080469", "00"), 원공고, 1.0);

        Assert.Equal(
            [
                "R26BK09011054-000|MBKLMH26930006-000|R26TA0908046900|R26BK09012082-000",
                "R26BK09011054-001|MBKLMH26930006-000|R26TA0908046900|R26BK09012082-000",
                "R26BK09012082-000|MBKLMH26930006-000|R26TA0908046900|R26BK09012082-000",
            ],
            Read("""
                 SELECT 입찰공고번호, 접수번호, 계약번호, 현행공고
                 FROM v_통합_v3 ORDER BY 입찰공고번호;
                 """));

        // 줄 하나가 조달 건인 v2 는 그대로 한 줄이다.
        Assert.Equal(1, Count("SELECT COUNT(*) FROM v_통합_v2;"));
    }

    /// <summary>
    /// 차수를 편 표에서도 <b>아직 안 들어온 것</b>이 보여야 한다 — 공고가 없는 접수와
    /// 어디에도 매달리지 못한 계약이 공고 열을 비운 채 제 줄로 선다(v_통합_v2 와 같은 규칙).
    /// 이 갈래를 빠뜨리면 「차수 펴기」를 켠 사람에게 그 줄들이 통째로 사라진다.
    /// </summary>
    [Fact]
    public void 통합_v3_도_공고_없는_접수와_홀로_선_계약을_낸다()
    {
        _store.UpsertRequest(접수("MBKLMH26930006"));
        _store.UpsertContract(계약("R26TA09080469"));

        Assert.Equal(
            ["|MBKLMH26930006-000|", "||R26TA0908046900"],
            Read("""
                 SELECT 입찰공고번호, 접수번호, 계약번호
                 FROM v_통합_v3 ORDER BY 접수번호 DESC;
                 """));
    }

    // ── 사슬도 건 단위다 ─────────────────────────────────────────

    /// <summary>
    /// <b>합쳐진 건은 사슬 하나다.</b> 공고마다 사슬을 세우던 때는 접수와 계약을 안고 서는
    /// 것이 <b>취소된 원공고</b>이고 살아 있는 재공고가 빈손으로 옆에 섰다 — 구조 보기가 센
    /// 수와 표가 센 수가 갈리고, 보는 사람은 같은 조달 건을 둘로 읽는다.
    ///
    /// <para>둘 다 아무것도 실패시키지 않는다. 링크는 이미 건을 가리키고 있어서 SQL 은
    /// 멀쩡했고, 갈린 것은 <b>조회하는 키</b>뿐이었다.</para>
    /// </summary>
    [Fact]
    public void 합쳐진_건은_사슬_하나로_서고_현행이_본문에_선다()
    {
        Put("R26BK09011054", "000");
        Put("R26BK09011054", "001", "R26BK09011054-000");
        Put("R26BK09012082", "000", "R26BK09011054-001", "R26BK09011054-000");

        접수와_계약을_잇는다("R26BK09011054", "000");

        var chain = Assert.Single(new Outline(_database).Build().Chains);

        // 본문에 서는 것은 살아 있는 재공고다. 취소된 원공고는 대체된 것으로 물러난다.
        Assert.Equal("R26BK09012082-000", chain.Notice!.Current!.Number);
        Assert.Equal("R26BK09011054-001", Assert.Single(chain.Notice.Superseded).Number);

        // 접수와 계약은 원공고에 이어 두었는데도 그 사슬에 그대로 붙는다.
        Assert.Equal("MBKLMH26930006-000", chain.Request!.Number);
        Assert.Equal("R26TA0908046900", Assert.Single(chain.Contracts).Number);

        // 사슬의 이름은 건 이름이라, 재공고가 한 번 더 나가도 흔들리지 않는다.
        Assert.Equal("MBKLMH26930006/R26BK09011054", chain.Key);
    }

    /// <summary>
    /// <b>문서 없는 조상이 건 이름일 때가 가장 나빴다.</b> 공고 본번호로 사슬을 세우면 그
    /// 이름으로는 링크가 하나도 걸리지 않아, 접수·공고·계약이 <b>사슬 셋으로 흩어진다</b> —
    /// 한 벌이 셋으로 보이는데 어느 줄에도 잘못된 값이 없다.
    ///
    /// <para>재공고 건에서는 가리키는 조상이 DB 에 없는 것이 예외가 아니라 보통이다.</para>
    /// </summary>
    [Fact]
    public void 문서_없는_조상이_건_이름이어도_사슬이_흩어지지_않는다()
    {
        Put("R26BK09014091", "000", "R26BK09013019-000");
        접수와_계약을_잇는다("R26BK09014091", "000");

        var chain = Assert.Single(new Outline(_database).Build().Chains);

        Assert.Equal("R26BK09014091-000", chain.Notice!.Current!.Number);
        Assert.Equal("MBKLMH26930006-000", chain.Request!.Number);
        Assert.Equal("R26TA0908046900", Assert.Single(chain.Contracts).Number);

        // 이름은 들어온 적 없는 번호다. 그래도 사슬은 하나다.
        Assert.Equal("MBKLMH26930006/R26BK09013019", chain.Key);
        Assert.Equal("R26BK09013019", chain.Notice.GroupBase);
    }

    /// <summary>
    /// 대체된 공고는 <b>늦은 것부터</b> 선다 — 방금 지나간 것이 맨 앞이라야 사람이 찾는
    /// 차례와 같다. <see cref="NoticeGroups.현행공고"/> 가 현행을 고르는 방향과 같은 방향이다.
    /// </summary>
    [Fact]
    public void 대체된_공고는_늦은_것부터_선다()
    {
        Put("R26BK09011054", "000", new DateTime(2026, 7, 7));
        Put("R26BK09012082", "000", new DateTime(2026, 7, 17), "R26BK09011054-000");
        Put("R26BK09014091", "000", new DateTime(2026, 7, 25), "R26BK09012082-000");

        var chain = Assert.Single(new Outline(_database).Build().Chains);

        Assert.Equal("R26BK09014091-000", chain.Notice!.Current!.Number);
        Assert.Equal(
            ["R26BK09012082-000", "R26BK09011054-000"],
            chain.Notice.Superseded.Select(n => n.Number));
    }

    // ── 다시 지어도 잃지 않는다 ──────────────────────────────────

    /// <summary>
    /// 건을 다시 짓는 동안 <b>확정한 링크가 살아남는다</b>. 옛 건을 먼저 지우면 거기 매달린
    /// 링크가 캐스케이드로 함께 가는데, 그 길에는 오류가 없다 — 넣기 → 재지정 → 지우기 차례를
    /// 지키는 것과 삭제를 명시 SQL 로 하는 것이 그것을 막는다.
    /// </summary>
    [Fact]
    public void 건이_다시_지어져도_확정한_링크가_살아남는다()
    {
        Put("R26BK09011054", "000");
        _store.UpsertContract(계약("R26TA09080469"));

        new Linker(_database).Confirm(
            new EntityRef("contract", "R26TA09080469", "00"),
            new EntityRef("notice", "R26BK09011054", "000"), 1.0);

        // 재공고가 들어오며 건이 합쳐진다.
        Put("R26BK09012082", "000", "R26BK09011054-000");

        Assert.Equal(
            ["R26TA09080469|R26BK09011054"],
            Read("SELECT contract_base, notice_group FROM project_link;"));
    }

    /// <summary>
    /// 사람이 적은 검토여부·메모는 <b>계열</b>에 매달려 있다. 건을 다시 지을 때도, 재공고를
    /// 지울 때도 그 값은 남는다 — <c>notice_group</c> → <c>notice_series</c> →
    /// <c>notice_user_field</c> 로 캐스케이드가 두 칸 이어지면 여기서 조용히 지워진다.
    /// </summary>
    [Fact]
    public void 사람이_적은_값이_건_재계산과_삭제에서_살아남는다()
    {
        Put("R26BK09011054", "000");
        _store.SetUserField(new EntityRef("notice", "R26BK09011054", "000"), "메모", "지어낸 메모");

        Put("R26BK09012082", "000", "R26BK09011054-000");
        Assert.Equal(["지어낸 메모"], Read("SELECT value FROM notice_user_field;"));

        _store.Delete(new EntityRef("notice", "R26BK09012082", "000"), wholeSeries: true);

        Assert.Equal(["지어낸 메모"], Read("SELECT value FROM notice_user_field;"));
    }

    // ── 밀어낸 것은 알린다 ───────────────────────────────────────

    /// <summary>
    /// 원공고와 재공고에 접수를 각각 이어 둔 자료가 한 건이 되면, <c>notice_group</c> 이
    /// UNIQUE 라 <b>하나는 밀려난다</b>. 남는 것은 사람이 확정한 쪽이고, 밀린 쪽은
    /// <see cref="NoticeGroupChange"/> 에 담겨 <b>반드시 사람 눈에 닿는다</b>.
    ///
    /// <para>인수인계 직후에 실제로 생기는 모양이라 판올림에서도 곧바로 이 자리에 온다.</para>
    /// </summary>
    [Fact]
    public void 한_건에_접수가_둘이면_사람_확정이_남고_밀린_쪽을_알린다()
    {
        Put("R26BK09011054", "000");
        Put("R26BK09012082", "000");

        _store.UpsertRequest(접수("MBKLMH26930006"));
        _store.UpsertRequest(접수("MBKLMH26930007"));

        // 사람이 원공고에 이었다. 기계는 재공고에 이었고 시각은 그쪽이 늦다.
        new RequestLinker(_database).Confirm(
            new EntityRef("request", "MBKLMH26930006", "000"),
            new EntityRef("notice", "R26BK09011054", "000"), 1.0);

        Execute(
            """
            UPDATE request_link SET confirmed_at = '2026-06-28T00:00:00.0000000Z'
            WHERE request_base = 'MBKLMH26930006';

            INSERT INTO request_link (request_base, notice_group, confidence, confirmed_at, decided_by)
            VALUES ('MBKLMH26930007', 'R26BK09012082', 0.9, '2026-07-29T00:00:00.0000000Z', 'auto');
            """);

        // 이제 둘이 한 건이 된다. 관계는 손으로 넣고 재계산을 직접 부른다 —
        // 밀린 쪽이 담겨 나오는지가 여기서 볼 것이라, 결과를 받는 자리가 필요하다.
        Execute(
            """
            INSERT INTO notice_relation (notice_base, seq, line_no, related)
            VALUES ('R26BK09012082', '000', 1, 'R26BK09011054-000');
            """);

        NoticeGroupChange change;
        using (var connection = _database.Open())
            change = NoticeGroups.Rebuild(connection, transaction: null);

        Assert.True(change.있나);
        Assert.Equal(1, change.이은건수);
        Assert.Empty(change.끊긴링크);

        var 밀림 = Assert.Single(change.밀린접수);
        Assert.Contains("MBKLMH26930007", 밀림);
        Assert.Contains("MBKLMH26930006", 밀림);

        // 사람이 확정한 쪽이 남는다. 늦게 이었다는 것만으로 기계가 사람을 밀어내지 않는다.
        Assert.Equal(
            ["MBKLMH26930006|R26BK09011054"],
            Read("SELECT request_base, notice_group FROM request_link;"));
    }

    /// <summary>
    /// 밀어낸 것은 <b>글이 되어야</b> 사람에게 닿는다. 창은 상자로 띄우고 명령줄은 그대로
    /// 찍으므로 글은 한 벌이다 — <see cref="MergeReportText"/> 와 같은 자세다.
    ///
    /// <para><b>이은 것만으로는 아무 말도 하지 않는다.</b> 여기서 글이 서면 판올림마다 상자가
    /// 떠서, 정작 밀린 날의 상자도 읽히지 않고 닫힌다.</para>
    /// </summary>
    [Fact]
    public void 밀어낸_것이_있을_때만_글이_선다()
    {
        Assert.Null(NoticeGroupText.Render(null));
        Assert.Null(NoticeGroupText.Render(new NoticeGroupChange(3, [], [])));

        var text = NoticeGroupText.Render(new NoticeGroupChange(
            1,
            ["계약 링크 R26TA09110501 — 가리키던 공고건 R26BK09011054 이 없어졌습니다"],
            ["접수 MBKLMH26930007 가 공고건 R26BK09011054 에서 밀렸습니다 — 남은 것은 MBKLMH26930006"]));

        Assert.NotNull(text);
        Assert.Contains("공고건 1개로 이어졌습니다", text);

        Assert.Contains("밀린 접수 1건", text);
        Assert.Contains("MBKLMH26930007", text);

        Assert.Contains("끊긴 링크 1건", text);
        Assert.Contains("R26TA09110501", text);
    }

    // ── 지우기 ───────────────────────────────────────────────────

    /// <summary>
    /// 재공고를 지우면 건이 갈리고, <b>링크는 원공고 쪽에 남는다</b>. 건 이름이 원공고라
    /// 옮겨 붙일 것이 없다 — 이름을 가장 이른 본번호로 둔 값어치가 여기서 나온다.
    /// </summary>
    [Fact]
    public void 재공고를_지우면_건이_갈리고_링크는_원공고에_남는다()
    {
        Put("R26BK09011054", "000");
        Put("R26BK09011054", "001", "R26BK09011054-000");
        Put("R26BK09012082", "000", "R26BK09011054-001", "R26BK09011054-000");

        _store.UpsertContract(계약("R26TA09080469"));
        new Linker(_database).Confirm(
            new EntityRef("contract", "R26TA09080469", "00"),
            new EntityRef("notice", "R26BK09011054", "000"), 1.0);

        _store.Delete(new EntityRef("notice", "R26BK09012082", "000"), wholeSeries: true);

        Assert.Equal(["R26BK09011054"], 건들());
        Assert.Equal(["R26BK09011054"], Read("SELECT notice_base FROM notice_series;"));
        Assert.Equal(
            ["R26TA09080469|R26BK09011054"],
            Read("SELECT contract_base, notice_group FROM project_link;"));

        // 남은 계열의 현행은 취소공고다. 취소되었다는 이유로 빼지 않는다(ADR-025).
        Assert.Equal("R26BK09011054-001", Assert.Single(Rows())["현행공고"]);
    }

    /// <summary>
    /// 공고가 통째로 사라지면 그 건도 사라지고, 가리키던 링크는 <b>걷어내고 알린다</b>.
    /// 링크가 매달린 곳이 계열이 아니라 건이라, 계열을 지우는 것만으로는 따라가지 않는다.
    /// </summary>
    [Fact]
    public void 공고가_다_사라지면_건과_링크가_함께_걷힌다()
    {
        Put("R26BK09011054", "000");
        _store.UpsertContract(계약("R26TA09080469"));
        new Linker(_database).Confirm(
            new EntityRef("contract", "R26TA09080469", "00"),
            new EntityRef("notice", "R26BK09011054", "000"), 1.0);

        _store.Delete(new EntityRef("notice", "R26BK09011054", "000"), wholeSeries: true);

        Assert.Empty(건들());
        Assert.Equal(0, Count("SELECT COUNT(*) FROM project_link;"));

        // 계약은 남는다. 공고가 사라졌다고 계약이 없던 일이 되지는 않는다.
        Assert.Equal(1, Count("SELECT COUNT(*) FROM v_계약_v1;"));
    }

    /// <summary>
    /// 공고를 지우기 전에 <b>링크를 세어 보인다</b>. 링크의 끝점이 건이라 링크 표에는
    /// <c>notice_base</c> 열이 아예 없어, 다른 종류와 같은 SQL 을 조립하면 <b>세는 자리에서
    /// 터진다</b> — 사람이 지우기 단추를 누르기도 전에 확인 창이 통째로 죽는 길이었다.
    ///
    /// <para>계약 링크와 <b>접수 링크를 함께</b> 센다. 예전에는 <c>project_link</c> 만 보아,
    /// 접수만 이어져 있으면 「링크 없음」으로 보이고는 지운 뒤에 조용히 사라졌다.</para>
    /// </summary>
    [Fact]
    public void 공고의_지울_것을_셀_때_접수_링크도_함께_센다()
    {
        Put("R26BK09011054", "000");
        _store.UpsertRequest(접수("MBKLMH26930006"));

        var 공고 = new EntityRef("notice", "R26BK09011054", "000");
        Assert.False(_store.PlanDeletion(공고, wholeSeries: true).Linked);

        new RequestLinker(_database).Confirm(
            new EntityRef("request", "MBKLMH26930006", "000"), 공고, 1.0);

        Assert.True(_store.PlanDeletion(공고, wholeSeries: true).Linked);
    }

    /// <summary>
    /// 옛 판으로 쌓아 둔 자료가 판올림을 타고 건까지 갖춘다. <b>남이 외래키로 가리키는 부모
    /// 표를 갈아 끼우는 첫 판올림</b>이라(스키마 V15), 여기서 링크나 사람 값이 조용히 사라지면
    /// 쓰던 사람은 앱을 다시 연 것밖에 한 일이 없다.
    ///
    /// <para>재공고에 매어 두었던 링크는 <b>건의 이름인 원공고 쪽으로</b> 따라온다.</para>
    /// </summary>
    [Fact]
    public void 옛_판에서_올라와도_링크와_사람_값이_건으로_따라온다()
    {
        var 옛판 = Path.Combine(Path.GetTempPath(), $"pclm-v14-{Guid.NewGuid():N}.db");
        var database = new Database(옛판);

        try
        {
            using (var connection = database.Open())
            {
                // 판올림이 표를 갈아 끼우므로 그동안은 참조 검사를 끈다(Database.Migrate 와 같은 자세).
                Run(connection, "PRAGMA foreign_keys = OFF;");
                for (var v = 0; v < 14; v++) Run(connection, Schema.Migrations[v]);
                Run(connection, "PRAGMA user_version = 14;");

                Run(connection,
                    """
                    INSERT INTO notice_series (notice_base)
                    VALUES ('R26BK09011054'), ('R26BK09012082');
                    INSERT INTO notice (notice_base, seq, title, updated_at) VALUES
                        ('R26BK09011054', '000', '지어낸 공고', '2026-07-17T09:00:00.0000000Z'),
                        ('R26BK09012082', '000', '지어낸 재공고', '2026-07-17T09:00:00.0000000Z');
                    INSERT INTO notice_relation (notice_base, seq, line_no, related)
                    VALUES ('R26BK09012082', '000', 1, 'R26BK09011054-000');

                    INSERT INTO contract_series (contract_base) VALUES ('R26TA09080469');
                    INSERT INTO contract (contract_base, seq, title, updated_at)
                    VALUES ('R26TA09080469', '00', '지어낸 계약', '2026-07-17T09:00:00.0000000Z');

                    INSERT INTO project_link (contract_base, notice_base, confidence, confirmed_at)
                    VALUES ('R26TA09080469', 'R26BK09012082', 1.0, '2026-07-19T09:00:00.0000000Z');
                    INSERT INTO notice_user_field (notice_base, field_name, value, updated_at)
                    VALUES ('R26BK09012082', '메모', '옛 판에서 적은 메모', '2026-07-19T09:00:00.0000000Z');
                    """);
            }

            var change = database.Migrate();

            Assert.NotNull(change);
            Assert.Equal(1, change.이은건수);
            Assert.False(change.있나);

            using var 열림 = database.OpenReadOnly();
            Assert.Empty(Read(열림, "PRAGMA foreign_key_check;"));
            Assert.Equal(["R26BK09011054"], Read(열림, "SELECT group_base FROM notice_group;"));
            Assert.Equal(
                ["R26TA09080469|R26BK09011054"],
                Read(열림, "SELECT contract_base, notice_group FROM project_link;"));
            Assert.Equal(["옛 판에서 적은 메모"], Read(열림, "SELECT value FROM notice_user_field;"));
        }
        finally
        {
            SqliteConnection.ClearAllPools();
            foreach (var suffix in new[] { "", "-wal", "-shm" })
                File.Delete(옛판 + suffix);
        }
    }

    // ── 지어낸 자료 ──────────────────────────────────────────────

    private void Put(string @base, string seq, params string[] related) =>
        Put(@base, seq, new DateTime(2026, 7, 18), related);

    /// <summary>
    /// 게시일시까지 정해 넣는다. <b>늦고 이른 것이 갈려야 하는 자리에서만</b> 쓴다 —
    /// 나머지 시험에서는 날짜가 뜻을 갖지 않아 한 날로 두는 편이 읽기 쉽다.
    /// </summary>
    private void Put(string @base, string seq, DateTime posted, params string[] related) =>
        _store.UpsertNotice(
            new NoticeRecord
            {
                NoticeBase = @base,
                Seq = seq,
                Title = "지어낸 공고",
                NoticeKind = seq == "001" ? "취소공고" : "등록공고",
                PostedAt = posted,
                RelatedNotices = related,
            });

    /// <summary>
    /// 접수와 계약을 하나씩 넣고 <b>이 공고에</b> 잇는다. 링크가 실제로 무엇을 가리키는지를
    /// 여기서 정한다 — 사람이 짚는 것은 공고이고, 표에 남는 것은 그 공고가 속한 <b>건</b>이다.
    /// </summary>
    private void 접수와_계약을_잇는다(string noticeBase, string seq)
    {
        _store.UpsertRequest(접수("MBKLMH26930006"));
        _store.UpsertContract(계약("R26TA09080469"));

        new Linker(_database).Confirm(
            new EntityRef("contract", "R26TA09080469", "00"),
            new EntityRef("notice", noticeBase, seq), 1.0);

        new RequestLinker(_database).Confirm(
            new EntityRef("request", "MBKLMH26930006", "000"),
            new EntityRef("notice", noticeBase, seq), 1.0);
    }

    private static ContractRecord 계약(string @base) => new()
    {
        ContractBase = @base,
        Seq = "00",
        Title = "지어낸 계약",
        ContractedOn = new DateTime(2026, 7, 21),
        Amount = 164_872_340m,
    };

    private static RequestRecord 접수(string @base) => new()
    {
        RequestBase = @base,
        Seq = "000",
        Title = "지어낸 접수",
        ReceivedOn = new DateTime(2026, 6, 9),
        Items =
        [
            new RequestItemRecord
            {
                LineNo = 1, RequestNumber = @base, ItemName = "지어낸 품목",
                Quantity = 1, UnitPrice = 164_872_340m,
            },
        ],
    };

    // ── 읽기 ─────────────────────────────────────────────────────

    /// <summary>서 있는 건 이름 전부.</summary>
    private List<string> 건들() => Read("SELECT group_base FROM notice_group ORDER BY group_base;");

    /// <summary>이 계열이 속한 건.</summary>
    private string 건(string noticeBase)
    {
        using var connection = _database.OpenReadOnly();
        return connection.ExecuteScalar<string?>(
            "SELECT group_base FROM notice_series WHERE notice_base = @b;", new { b = noticeBase }) ?? "";
    }

    private List<Dictionary<string, string>> Rows()
    {
        using var connection = _database.OpenReadOnly();
        using var command = connection.CreateCommand();
        command.CommandText = "SELECT * FROM v_공고_v1 ORDER BY 1;";

        using var reader = command.ExecuteReader();
        var rows = new List<Dictionary<string, string>>();

        while (reader.Read())
        {
            var row = new Dictionary<string, string>();
            for (var i = 0; i < reader.FieldCount; i++)
                row[reader.GetName(i)] = reader.IsDBNull(i) ? "" : reader.GetValue(i).ToString() ?? "";
            rows.Add(row);
        }

        return rows;
    }

    private List<string> Read(string sql)
    {
        using var connection = _database.OpenReadOnly();
        return Read(connection, sql);
    }

    private static List<string> Read(SqliteConnection connection, string sql)
    {
        using var command = connection.CreateCommand();
        command.CommandText = sql;

        var rows = new List<string>();
        using var reader = command.ExecuteReader();
        while (reader.Read())
            rows.Add(string.Join("|", Enumerable.Range(0, reader.FieldCount)
                .Select(i => reader.IsDBNull(i) ? "" : reader.GetValue(i).ToString())));

        return rows;
    }

    private static void Run(SqliteConnection connection, string sql)
    {
        using var command = connection.CreateCommand();
        command.CommandText = sql;
        command.ExecuteNonQuery();
    }

    private long Count(string sql)
    {
        using var connection = _database.OpenReadOnly();
        return connection.ExecuteScalar<long>(sql);
    }

    private void Execute(string sql)
    {
        using var connection = _database.Open();
        using var command = connection.CreateCommand();
        command.CommandText = sql;
        command.ExecuteNonQuery();
    }

    public void Dispose()
    {
        SqliteConnection.ClearAllPools();
        foreach (var suffix in new[] { "", "-wal", "-shm" })
            File.Delete(_path + suffix);
        GC.SuppressFinalize(this);
    }
}

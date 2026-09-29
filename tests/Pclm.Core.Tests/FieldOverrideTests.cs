using Microsoft.Data.Sqlite;
using Pclm.Core.Domain;
using Pclm.Core.Storage;
using Xunit;

namespace Pclm.Core.Tests;

/// <summary>
/// 파서가 읽은 값을 사람이 고치는 자리.
///
/// <para>검산을 하지 않기로 한 이상(ADR-016) 틀린 값을 바로잡는 것은 오직 사람이다. 그런데
/// 고친 값을 <c>notice</c>·<c>contract</c> 에 직접 쓰면 <b>기계가 읽은 것과 사람이 붙인 것이
/// 한 칸에 섞여</b> 그 뒤로 아무도 가릴 수 없다. 그래서 덮개 표에 담고 뷰가 씌운다(ADR-020).</para>
///
/// <para>여기서 가장 조심하는 것은 <b>정정이 남의 차수로 새는 일</b>이다. 1차에서 고친 금액이
/// 2차 변경계약에 그대로 얹히면, 확인한 적 없는 숫자가 확인된 얼굴로 문서에 찍힌다 —
/// 오류 없이. 사람 값이 계열에 붙는 것과 정확히 반대 방향의 처방이다(ADR-012).</para>
/// </summary>
public class FieldOverrideTests : IDisposable
{
    private readonly string _path = Path.Combine(
        Path.GetTempPath(), $"pclm-override-{Guid.NewGuid():N}.db");

    private readonly Database _database;
    private readonly Store _store;

    private static readonly EntityRef 계약 = new("contract", "R26TA09110507", "00");
    private static readonly EntityRef 공고 = new("notice", "R26BK09017075", "000");
    private static readonly EntityRef 접수 = new("request", "MPKPLA26910286", "000");

    public FieldOverrideTests()
    {
        _database = new Database(_path);
        _database.Migrate();
        _store = new Store(_database);
    }

    [Fact]
    public void 고친_값이_계약면에_뜬다()
    {
        _store.UpsertContract(Contract("00"));
        Assert.Equal("164,872,340", Cell("v_계약_v1", "계약금액"));

        _store.SetOverride(계약, "계약금액", "164,872,341");

        Assert.Equal("164,872,341", Cell("v_계약_v1", "계약금액"));
    }

    /// <summary>통합은 계약 뷰를 통째로 받으므로 저절로 따라와야 한다 — 두 번 씌우면 열이 둘이 된다.</summary>
    [Fact]
    public void 통합_뷰에도_같은_값이_뜬다()
    {
        _store.UpsertContract(Contract("00"));
        _store.SetOverride(계약, "계약건명", "수질측정기 구매(정정)");

        Assert.Equal("수질측정기 구매(정정)", Cell("v_통합_v1", "계약건명"));
    }

    [Fact]
    public void 되돌리면_파서가_읽은_값이_돌아온다()
    {
        _store.UpsertContract(Contract("00"));
        _store.SetOverride(계약, "계약금액", "0");

        _store.ClearOverride(계약, "계약금액");

        Assert.Equal("164,872,340", Cell("v_계약_v1", "계약금액"));
        Assert.Empty(_store.Overrides("contract"));
    }

    /// <summary>
    /// 잘못 읽힌 칸을 <b>비우는 것</b>과 파서 값으로 <b>돌아가는 것</b>은 다른 일이다.
    /// 빈 문자열에 삭제 의미를 주면 앞의 것을 할 수 없다.
    /// </summary>
    [Fact]
    public void 빈_값도_유효한_덮개다()
    {
        _store.UpsertContract(Contract("00"));
        _store.SetOverride(계약, "계약금액", "");

        Assert.Equal("", Cell("v_계약_v1", "계약금액"));
        Assert.Single(_store.Overrides("contract"));
    }

    /// <summary>두 번째로 고칠 때 다시 적으면 "직전에 내가 적은 값" 이 되어 되돌리기 안내가 거짓이 된다.</summary>
    [Fact]
    public void 원래_값은_처음_고칠_때만_담긴다()
    {
        _store.UpsertContract(Contract("00"));

        _store.SetOverride(계약, "계약금액", "1");
        _store.SetOverride(계약, "계약금액", "2");

        Assert.Equal("164,872,340", Assert.Single(_store.Overrides("contract")).Original);
    }

    /// <summary>
    /// <b>이 시험이 이 표의 존재 이유다.</b> 정정은 "이 문서의 이 칸" 에 대한 것이라 차수를
    /// 따라가지 않는다 — 따라가면 확인한 적 없는 숫자가 확인된 얼굴로 새 차수에 얹힌다.
    /// </summary>
    [Fact]
    public void 한_차수의_정정이_다음_차수로_새지_않는다()
    {
        _store.UpsertContract(Contract("00"));
        _store.SetOverride(계약, "계약금액", "999,999,999");

        _store.UpsertContract(Contract("01"));

        Assert.Equal("01", Cell("v_계약_v1", "차수"));
        Assert.Equal("164,872,340", Cell("v_계약_v1", "계약금액"));
    }

    /// <summary>고친 것은 옛 차수 자리에 그대로 남는다. 새지 않는 것과 잃는 것은 다르다.</summary>
    [Fact]
    public void 새_차수를_지워도_옛_차수의_정정은_살아_있다()
    {
        _store.UpsertContract(Contract("00"));
        _store.SetOverride(계약, "계약금액", "999,999,999");
        _store.UpsertContract(Contract("01"));

        _store.Delete(new EntityRef("contract", "R26TA09110507", "01"), wholeSeries: false);

        Assert.Equal("00", Cell("v_계약_v1", "차수"));
        Assert.Equal("999,999,999", Cell("v_계약_v1", "계약금액"));
    }

    /// <summary>
    /// 집계로 나오는 열도 열린다. 뷰가 표기까지 마친 값 <b>바깥</b>에 씌우므로 안쪽이 무엇이든
    /// 상관하지 않는다 — 세부품명이 여럿이라 비워 둔 칸을 사람이 채울 수 있다.
    /// </summary>
    [Fact]
    public void 품목_집계로_나오는_열도_고칠_수_있다()
    {
        _store.UpsertNotice(
            Notice("000") with
            {
                Items =
                [
                    new NoticeItemRecord { LineNo = 1, ItemName = "수질측정기" },
                    new NoticeItemRecord { LineNo = 2, ItemName = "시약" },
                ],
            });

        // 세부품명이 두 가지라 뷰는 비운다 — 첫 물건의 값을 공고 전체의 값처럼 낼 수 없어서다.
        Assert.Equal("", Cell("v_공고_v1", "세부품명"));

        _store.SetOverride(공고, "세부품명", "수질측정기 외 1종");

        Assert.Equal("수질측정기 외 1종", Cell("v_공고_v1", "세부품명"));
    }

    /// <summary>
    /// 공고의 <b>모든 열</b>이 감싸여 있다. 접수판(<see cref="접수의_모든_열이_감싸여_있다"/>)의
    /// 짝이다 — 지금까지 공고에는 이 시험이 없어, 뷰에 열을 더하면서 덮개(<c>Views.Face</c>)
    /// 로 감싸기를 빠뜨려도 <b>아무도 보지 않았다</b>. 빠뜨리면 그 칸만 조용히 고칠 수 없게
    /// 된다(ADR-020).
    /// </summary>
    [Fact]
    public void 공고의_모든_열이_감싸여_있다()
    {
        _store.UpsertNotice(Notice("000"));

        var 사람열 = _store.UserColumns("notice").Select(c => c.FieldName).ToHashSet();

        foreach (var column in ViewColumns("v_공고_v1"))
        {
            if (Views.KeyColumns.Contains(column) || 사람열.Contains(column)) continue;

            _store.SetOverride(공고, column, "고침");
            Assert.Equal("고침", Cell("v_공고_v1", column));
        }
    }

    /// <summary>
    /// 새로 더한 「공고건」·「현행공고」는 <b>키라 감싸지 않는다</b>(스키마 V15).
    ///
    /// <para>둘 다 관련공고에서 지어지는 파생값이라, 덮개를 씌우면 화면의 글자만 바뀌고 실제
    /// 묶임은 그대로다 — 고칠 수 있는 것처럼 보이는 편이 나쁘다. 위 시험이 이 둘을 건너뛰는
    /// 것이 <b>빠뜨린 것이 아니라 뜻한 것</b>임을 여기서 못 박는다.</para>
    /// </summary>
    [Fact]
    public void 건과_현행공고는_키라_고칠_수_없다()
    {
        _store.UpsertNotice(Notice("000"));

        foreach (var column in new[] { "공고건", "현행공고" })
        {
            Assert.Contains(column, Views.KeyColumns);

            var ex = Assert.Throws<InvalidOperationException>(
                () => _store.SetOverride(공고, column, "R26BK00000000"));

            Assert.Contains(column, ex.Message);
        }
    }

    // ── 입찰방법 ─────────────────────────────────────────
    // 계약방법과 계약구분에서 짓는 칸. 재료는 사람이 고친 뒤의 값이고, 재료가 표에 없으면
    // 통째로 비운다 — 틀린 글자는 그대로 문서에 찍히지만 빈칸은 받는 쪽이 알아챈다.

    private static readonly EntityRef 지은공고 = new("notice", "R26BK00000001", "000");

    [Theory]
    [InlineData("제한경쟁", "총액계약", "제한(총액)")]
    [InlineData("일반경쟁", "일반단가계약", "일반(단가)")]
    [InlineData("지명경쟁", "단가계약", "지명(단가)")]
    [InlineData("수의계약", "제3자단가계약", "수의(제3자단가)")]
    public void 입찰방법은_계약방법과_계약구분을_줄여_잇는다(string method, string kind, string expected)
    {
        _store.UpsertNotice(지은(method, kind));

        Assert.Equal(expected, Cell("v_공고_v1", "입찰방법"));
        Assert.Equal(expected, Cell("v_공고차수_v1", "입찰방법"));
    }

    /// <summary>한쪽이라도 모르면 「제한()」 도 코드 그대로도 아니라 빈 문자열 — NULL 도 아니다.</summary>
    [Theory]
    [InlineData("제한경쟁", "")]
    [InlineData("", "총액계약")]
    [InlineData("제한경쟁", "계999999 (명칭 미확정)")]
    [InlineData(null, null)]
    public void 입찰방법은_한쪽이라도_모르면_비운다(string? method, string? kind)
    {
        _store.UpsertNotice(지은(method, kind));

        using var connection = _database.OpenReadOnly();
        using var command = connection.CreateCommand();
        command.CommandText = "SELECT 입찰방법 FROM v_공고_v1;";
        using var reader = command.ExecuteReader();
        Assert.True(reader.Read());

        Assert.False(reader.IsDBNull(0), "빈 값이 NULL 로 새면 받는 쪽의 빈 값 경고가 죽는다.");
        Assert.Equal("", reader.GetString(0));
    }

    /// <summary>사람이 계약방법을 고치면 입찰방법도 그 값을 따른다 — 두 칸이 서로 다른 말을 하지 않는다.</summary>
    [Fact]
    public void 입찰방법은_고친_계약방법을_따른다()
    {
        _store.UpsertNotice(지은("제한경쟁", "총액계약"));

        _store.SetOverride(지은공고, "계약방법", "일반경쟁");

        Assert.Equal("일반(총액)", Cell("v_공고_v1", "입찰방법"));
    }

    [Fact]
    public void 입찰방법을_직접_고치면_그것이_이긴다()
    {
        _store.UpsertNotice(지은("제한경쟁", "총액계약"));
        _store.SetOverride(지은공고, "계약방법", "일반경쟁");

        _store.SetOverride(지은공고, "입찰방법", "지명(총액)");

        Assert.Equal("지명(총액)", Cell("v_공고_v1", "입찰방법"));
    }

    private static NoticeRecord 지은(string? method, string? kind) => new()
    {
        NoticeBase = "R26BK00000001",
        Seq = "000",
        Title = "시험용 물품 구매",
        ContractMethod = method,
        ContractKind = kind,
    };

    // ── 막는 것들 ────────────────────────────────────────
    // 화면이 걸러도 다리와 명령줄로 직접 부를 수 있다. 검사는 Store 하나에 모여 있다.

    [Fact]
    public void 레코드를_가리키는_열은_고칠_수_없다()
    {
        _store.UpsertContract(Contract("00"));

        var ex = Assert.Throws<InvalidOperationException>(
            () => _store.SetOverride(계약, "계약번호", "R26TA0911050799"));

        Assert.Contains("계약번호", ex.Message);
    }

    [Fact]
    public void 손으로_채우는_열은_덮개로_받지_않는다()
    {
        _store.UpsertContract(Contract("00"));

        var ex = Assert.Throws<InvalidOperationException>(
            () => _store.SetOverride(계약, "진행상태", "납품"));

        Assert.Contains("손으로 채우는 열", ex.Message);
    }

    /// <summary>열 이름이 뒤이어 SQL 에 박힌다. 계약면에 없는 이름은 여기서 멎어야 한다.</summary>
    [Fact]
    public void 계약면에_없는_열은_거절한다()
    {
        _store.UpsertContract(Contract("00"));

        Assert.Throws<InvalidOperationException>(
            () => _store.SetOverride(계약, "amount\"; DROP TABLE contract; --", "0"));
    }

    /// <summary>공고 열은 공고에만, 계약 열은 계약에만. 뷰가 갈려 있으므로 여기서도 갈린다.</summary>
    [Fact]
    public void 다른_종류의_열은_거절한다()
    {
        _store.UpsertContract(Contract("00"));

        Assert.Throws<InvalidOperationException>(() => _store.SetOverride(계약, "공고종류", "일반"));
    }

    // ── 접수 ─────────────────────────────────────────────

    /// <summary>
    /// 접수의 칸도 고칠 수 있다. <c>field_override</c> 의 <c>CHECK</c> 를 넓히지 않았으면
    /// 여기서 막힌다(스키마 V11).
    /// </summary>
    [Fact]
    public void 접수의_고친_값이_계약면에_뜬다()
    {
        _store.UpsertRequest(Request("000"));
        Assert.Equal("181,725,200", Cell("v_접수_v1", "품대"));

        _store.SetOverride(접수, "품대", "181,725,201");

        Assert.Equal("181,725,201", Cell("v_접수_v1", "품대"));
    }

    /// <summary>
    /// 접수의 <b>모든 열</b>이 감싸여 있다. 새 열을 더할 때 감싸기를 빠뜨리면 그 칸만
    /// 조용히 고칠 수 없게 되므로, 열 목록을 돌며 통째로 확인한다(ADR-020).
    /// </summary>
    [Fact]
    public void 접수의_모든_열이_감싸여_있다()
    {
        _store.UpsertRequest(Request("000"));

        var 사람열 = _store.UserColumns("request").Select(c => c.FieldName).ToHashSet();

        foreach (var column in ViewColumns("v_접수_v1"))
        {
            if (Views.KeyColumns.Contains(column) || 사람열.Contains(column)) continue;

            _store.SetOverride(접수, column, "고침");
            Assert.Equal("고침", Cell("v_접수_v1", column));
        }
    }

    /// <summary>덮개는 차수에 매달린다 — 변경접수가 들어오면 따라가지 않는다.</summary>
    [Fact]
    public void 접수의_덮개는_차수를_따라가지_않는다()
    {
        _store.UpsertRequest(Request("000"));
        _store.SetOverride(접수, "품대", "181,725,201");

        _store.UpsertRequest(Request("001"));

        Assert.Equal("181,725,200", Cell("v_접수_v1", "품대"));
    }

    private List<string> ViewColumns(string view)
    {
        using var connection = _database.OpenReadOnly();
        using var command = connection.CreateCommand();
        command.CommandText = $"PRAGMA table_info(\"{view}\");";

        var names = new List<string>();
        using var reader = command.ExecuteReader();
        while (reader.Read()) names.Add(reader.GetString(1));

        return names;
    }

    private static RequestRecord Request(string seq) => new()
    {
        RequestBase = "MPKPLA26910286",
        Seq = seq,
        Title = "윈치 8종 구매",
        ReceivedOn = new DateTime(2026, 6, 9),
        GoodsAmount = 181_725_200m,
        BudgetAmount = 183_408_620m,
        DemandAgency = "한별군수지원단",
        Items =
        [
            new RequestItemRecord
            {
                LineNo = 1, RequestNumber = "MPKPLA26910286",
                ItemName = "받침목", Quantity = 70, UnitPrice = 310_200m,
            },
        ],
    };

    private static ContractRecord Contract(string seq) => new()
    {
        ContractBase = "R26TA09110507",
        Seq = seq,
        Title = "수질측정기 구매",
        Amount = 164_872_340m,
    };

    private static NoticeRecord Notice(string seq) => new()
    {
        NoticeBase = "R26BK09017075",
        Seq = seq,
        Title = "수질측정기 구매",
    };

    /// <summary>뷰에서 칸 하나. 줄이 하나뿐인 시험이라 첫 줄을 본다.</summary>
    private string Cell(string view, string column)
    {
        using var connection = _database.OpenReadOnly();
        using var command = connection.CreateCommand();
        command.CommandText = $"SELECT \"{column}\" FROM \"{view}\";";

        using var reader = command.ExecuteReader();
        Assert.True(reader.Read(), $"{view} 에 줄이 없습니다.");

        return reader.IsDBNull(0) ? "" : reader.GetValue(0).ToString() ?? "";
    }

    public void Dispose()
    {
        SqliteConnection.ClearAllPools();
        foreach (var suffix in new[] { "", "-wal", "-shm" })
            File.Delete(_path + suffix);
        GC.SuppressFinalize(this);
    }
}

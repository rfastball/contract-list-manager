using Microsoft.Data.Sqlite;
using Pclm.Core.Domain;
using Pclm.Core.Storage;
using Xunit;

namespace Pclm.Core.Tests;

/// <summary>
/// 걷어내는 길.
///
/// <para>한동안 이 저장소에는 <b>쌓는 길만 있었다</b>. 업서트는 같은 것을 다시 넣어도 행을
/// 늘리지 않지만, 잘못 읽힌 문서·남의 공고·이제 볼 일 없는 건은 들어온 이상 영원히 남았다.
/// 차수가 자연키의 일부라 변경공고·변경계약도 계속 쌓인다 — 뷰가 최신만 내므로 눈에 띄지도 않는다.</para>
///
/// <para>여기서 조심하는 것은 <b>지운 뒤에 남는 부스러기</b>다. 덮개는 공고·계약을 한 표에 담아
/// 외래키를 걸지 못했으니 손으로 지우지 않으면 조용히 남는다. 계열을 남기면 사람이 적은 값과
/// 링크가 가리킬 것 없이 떠돈다.</para>
/// </summary>
public class DeletionTests : IDisposable
{
    private readonly string _path = Path.Combine(
        Path.GetTempPath(), $"pclm-delete-{Guid.NewGuid():N}.db");

    private readonly Database _database;
    private readonly Store _store;

    private static readonly EntityRef 계약 = new("contract", "R26TA09110507", "00");
    private static readonly EntityRef 공고 = new("notice", "R26BK09017075", "000");
    private static readonly EntityRef 접수 = new("request", "MPKPLA26910286", "000");

    public DeletionTests()
    {
        _database = PclmFile.Create(_path, PclmRole.Work);
        _store = new Store(_database);
    }

    [Fact]
    public void 지우면_계약면에서_사라진다()
    {
        _store.UpsertContract(Contract("00"));
        Assert.Equal(1, Count("SELECT COUNT(*) FROM v_계약;"));

        _store.Delete(계약, wholeSeries: true);

        Assert.Equal(0, Count("SELECT COUNT(*) FROM v_계약;"));
    }

    /// <summary>딸린 줄은 외래키 캐스케이드가 데려간다. 남으면 다음 투입에서 옛 줄이 되살아난다.</summary>
    [Fact]
    public void 딸린_줄도_함께_간다()
    {
        _store.UpsertContract(
            Contract("00") with
            {
                Items = [new ContractItemRecord { LineNo = 1, ItemName = "수질측정기" }],
                Attachments = [new ContractAttachmentRecord { LineNo = 1, FileName = "규격서.hwp" }],
            });

        _store.Delete(계약, wholeSeries: true);

        Assert.Equal(0, Count("SELECT COUNT(*) FROM contract_item;"));
        Assert.Equal(0, Count("SELECT COUNT(*) FROM contract_attachment;"));
    }

    /// <summary>
    /// 계열이 걷혀야 사람이 적은 값과 링크가 함께 간다. 남기면 가리킬 것 없이 떠돈다.
    /// </summary>
    [Fact]
    public void 계열을_지우면_사람이_적은_값과_링크도_간다()
    {
        _store.UpsertNotice(Notice("000"));
        _store.UpsertContract(Contract("00"));
        _store.SetUserField(계약, "진행상태", "납품");
        new Linker(_database).Confirm(계약, 공고, 1.0);

        _store.Delete(계약, wholeSeries: true);

        Assert.Equal(0, Count("SELECT COUNT(*) FROM contract_user_field;"));
        Assert.Equal(0, Count("SELECT COUNT(*) FROM project_link;"));
        Assert.Equal(0, Count("SELECT COUNT(*) FROM contract_series;"));

        // 공고는 그대로 남는다 — 계약 하나를 지운 것이 공고를 지운 것은 아니다.
        Assert.Equal(1, Count("SELECT COUNT(*) FROM v_공고;"));
    }

    /// <summary>덮개는 외래키가 없다. 손으로 지우지 않으면 조용히 남는다.</summary>
    [Fact]
    public void 손으로_고친_것도_함께_간다()
    {
        _store.UpsertContract(Contract("00"));
        _store.SetOverride(계약, "계약금액", "164,872,341");

        _store.Delete(계약, wholeSeries: true);

        Assert.Empty(_store.Overrides("contract"));
    }

    /// <summary>아무 계약도 가리키지 않게 된 상대자. 남기면 status 의 상대자 수가 실물과 어긋난다.</summary>
    [Fact]
    public void 남은_계약이_없는_상대자는_함께_간다()
    {
        _store.UpsertContract(
            Contract("00") with
            {
                Counterparty = new CounterpartyRecord { BusinessNumber = "123-45-67890", Name = "가나다상사" },
            });

        Assert.Equal(1, Count("SELECT COUNT(*) FROM counterparty;"));

        _store.Delete(계약, wholeSeries: true);

        Assert.Equal(0, Count("SELECT COUNT(*) FROM counterparty;"));
    }

    // ── 차수 하나만 지우기 ───────────────────────────────

    /// <summary>잘못 읽힌 변경계약 하나만 걷어 내고 그 앞 차수는 살린다.</summary>
    [Fact]
    public void 차수_하나를_지우면_이전_차수가_다시_최신이_된다()
    {
        _store.UpsertContract(Contract("00"));
        _store.UpsertContract(Contract("01"));

        _store.Delete(계약 with { Seq = "01" }, wholeSeries: false);

        Assert.Equal(1, Count("SELECT COUNT(*) FROM contract;"));
        Assert.Equal("00", Text("SELECT 차수 FROM v_계약;"));
    }

    /// <summary>
    /// 차수 하나만 지웠는데 그것이 마지막이면 <b>계열도 함께 걷힌다</b> — 남을 자리가 없다.
    /// 계열만 남기면 사람이 적은 값이 어느 레코드에도 붙지 않은 채 표에 남는다.
    /// </summary>
    [Fact]
    public void 마지막_차수를_지우면_계열도_간다()
    {
        _store.UpsertContract(Contract("00"));
        _store.SetUserField(계약, "진행상태", "납품");

        _store.Delete(계약, wholeSeries: false);

        Assert.Equal(0, Count("SELECT COUNT(*) FROM contract_series;"));
        Assert.Equal(0, Count("SELECT COUNT(*) FROM contract_user_field;"));
    }

    /// <summary>차수 하나를 지울 때 계열이 남으면 사람이 적은 값은 그대로 살아 있어야 한다.</summary>
    [Fact]
    public void 계열이_남으면_사람이_적은_값도_남는다()
    {
        _store.UpsertContract(Contract("00"));
        _store.UpsertContract(Contract("01"));
        _store.SetUserField(계약, "진행상태", "납품");

        _store.Delete(계약 with { Seq = "01" }, wholeSeries: false);

        Assert.Equal("납품", Text("SELECT 진행상태 FROM v_계약;"));
    }

    // ── 묻기 전에 세어 보이기 ────────────────────────────

    [Fact]
    public void 무엇이_사라지는지_먼저_센다()
    {
        _store.UpsertNotice(Notice("000"));
        _store.UpsertContract(
            Contract("00") with
            {
                Items = [new ContractItemRecord { LineNo = 1, ItemName = "수질측정기" }],
            });

        _store.SetUserField(계약, "진행상태", "납품");
        _store.SetOverride(계약, "계약금액", "164,872,341");
        new Linker(_database).Confirm(계약, 공고, 1.0);

        var plan = _store.PlanDeletion(계약, wholeSeries: true);

        Assert.Equal("R26TA0911050700", plan.Display);
        Assert.Equal("수질측정기 구매", plan.Title);
        Assert.Equal(["00"], plan.Revisions);
        Assert.True(plan.SeriesGoes);
        Assert.Equal(1, plan.ChildRows);
        Assert.Equal(1, plan.UserFields);
        Assert.Equal(1, plan.Overrides);
        Assert.True(plan.Linked);
    }

    /// <summary>차수 하나만 지울 때는 그 차수에 걸린 것만 센다 — 계열의 것은 남는다.</summary>
    [Fact]
    public void 차수_하나만_지울_때는_계열의_것을_세지_않는다()
    {
        _store.UpsertContract(Contract("00"));
        _store.UpsertContract(Contract("01"));
        _store.SetUserField(계약, "진행상태", "납품");
        _store.SetOverride(계약, "계약금액", "111");

        var plan = _store.PlanDeletion(계약 with { Seq = "01" }, wholeSeries: false);

        Assert.False(plan.SeriesGoes);
        Assert.Equal(["00", "01"], plan.Revisions);
        Assert.Equal(0, plan.UserFields);
        Assert.Equal(0, plan.Overrides);
    }

    [Fact]
    public void 없는_레코드는_셀_수_없다()
    {
        var ex = Assert.Throws<InvalidOperationException>(
            () => _store.PlanDeletion(계약, wholeSeries: true));

        Assert.Contains("R26TA0911050700", ex.Message);
    }

    /// <summary>공고를 지우면 거기 매달렸던 계약은 남되 미연결로 돌아간다.</summary>
    [Fact]
    public void 공고를_지우면_계약은_남고_링크만_풀린다()
    {
        _store.UpsertNotice(Notice("000"));
        _store.UpsertContract(Contract("00"));
        new Linker(_database).Confirm(계약, 공고, 1.0);

        _store.Delete(공고, wholeSeries: true);

        Assert.Equal(1, Count("SELECT COUNT(*) FROM v_계약;"));
        Assert.Equal(0, Count("SELECT COUNT(*) FROM project_link;"));
        Assert.Equal("", Text("SELECT 공고명 FROM v_통합;"));
    }

    // ── 접수 ─────────────────────────────────────────────

    /// <summary>접수도 지울 수 있고, 지우면 계약면에서 사라진다.</summary>
    [Fact]
    public void 접수를_지우면_계약면에서_사라진다()
    {
        _store.UpsertRequest(Request("000"));
        Assert.Equal(1, Count("SELECT COUNT(*) FROM v_접수;"));

        _store.Delete(접수, wholeSeries: true);

        Assert.Equal(0, Count("SELECT COUNT(*) FROM v_접수;"));
        Assert.Equal(0, Count("SELECT COUNT(*) FROM request_item;"));
    }

    /// <summary>
    /// 지우기 전에 <b>무엇이 함께 사라지는지 센다</b>. 세지 않고 지우는 길을 새로 내지 않는다.
    /// 접수의 <c>Linked</c> 는 <c>request_link</c> 를 봐야 한다 — <c>project_link</c> 에는
    /// <c>request_base</c> 열이 아예 없어, 가리지 않으면 SQL 이 터진다.
    /// </summary>
    [Fact]
    public void 접수의_지울_것을_세어_보인다()
    {
        _store.UpsertRequest(Request("000"));
        _store.UpsertNotice(Notice("000"));

        new RequestLinker(_database).Confirm(접수, 공고, 1.0);
        _store.AddUserColumn("request", "비고", "text", []);
        _store.SetUserField(접수, "비고", "협의중");

        var plan = _store.PlanDeletion(접수, wholeSeries: true);

        Assert.Equal("request", plan.EntityType);
        Assert.Equal("MPKPLA26910286-000", plan.Display);
        Assert.Equal(["000"], plan.Revisions);
        Assert.True(plan.SeriesGoes);
        Assert.Equal(1, plan.ChildRows);
        Assert.Equal(1, plan.UserFields);
        Assert.True(plan.Linked);
    }

    /// <summary>계열이 걷히면 사람 값·링크·품목 짝이 캐스케이드로 함께 간다.</summary>
    [Fact]
    public void 접수를_지우면_링크와_품목_짝도_함께_간다()
    {
        _store.UpsertRequest(Request("000"));
        _store.UpsertNotice(NoticeWithItem());
        new RequestLinker(_database).Confirm(접수, 공고, 1.0);

        Assert.Equal(1, Count("SELECT COUNT(*) FROM request_link;"));
        Assert.Equal(1, Count("SELECT COUNT(*) FROM request_item_link;"));

        _store.Delete(접수, wholeSeries: true);

        Assert.Equal(0, Count("SELECT COUNT(*) FROM request_link;"));
        Assert.Equal(0, Count("SELECT COUNT(*) FROM request_item_link;"));
        Assert.Equal(0, Count("SELECT COUNT(*) FROM request_series;"));
    }

    /// <summary>공고를 지우면 거기 붙은 접수 링크도 함께 걷힌다 — 가리킬 것이 없어진다.</summary>
    [Fact]
    public void 공고를_지우면_접수_링크도_걷힌다()
    {
        _store.UpsertRequest(Request("000"));
        _store.UpsertNotice(NoticeWithItem());
        new RequestLinker(_database).Confirm(접수, 공고, 1.0);

        _store.Delete(공고, wholeSeries: true);

        Assert.Equal(0, Count("SELECT COUNT(*) FROM request_link;"));
        Assert.Equal(0, Count("SELECT COUNT(*) FROM request_item_link;"));

        // 접수 자체는 남는다. 공고가 사라졌다고 요청이 없던 일이 되지는 않는다.
        Assert.Equal(1, Count("SELECT COUNT(*) FROM v_접수;"));
    }

    /// <summary>덮개도 손으로 지운다 — 세 종류를 한 표에 담아 외래키를 걸지 못했다.</summary>
    [Fact]
    public void 접수를_지우면_덮개도_간다()
    {
        _store.UpsertRequest(Request("000"));
        _store.SetOverride(접수, "품대", "181,725,201");

        Assert.Equal(1, Count("SELECT COUNT(*) FROM field_override;"));

        _store.Delete(접수, wholeSeries: true);

        Assert.Equal(0, Count("SELECT COUNT(*) FROM field_override;"));
    }

    /// <summary>접수와 (수량, 단가)가 맞는 공고. 그래야 품목 짝이 선다.</summary>
    private static NoticeRecord NoticeWithItem() =>
        Notice("000") with
        {
            Items =
            [
                new NoticeItemRecord
                {
                    LineNo = 1, ItemName = "받침목", Quantity = 70, UnitPrice = 310_200m,
                },
            ],
        };

    private static RequestRecord Request(string seq) => new()
    {
        RequestBase = "MPKPLA26910286",
        Seq = seq,
        Title = "윈치 8종 구매",
        GoodsAmount = 181_725_200m,
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

    private int Count(string sql)
    {
        using var connection = _database.OpenReadOnly();
        using var command = connection.CreateCommand();
        command.CommandText = sql;
        return Convert.ToInt32(command.ExecuteScalar());
    }

    private string Text(string sql)
    {
        using var connection = _database.OpenReadOnly();
        using var command = connection.CreateCommand();
        command.CommandText = sql;

        using var reader = command.ExecuteReader();
        Assert.True(reader.Read(), $"행이 없습니다: {sql}");

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

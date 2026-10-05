using Microsoft.Data.Sqlite;
using Pclm.Core.Domain;
using Pclm.Core.Storage;
using Xunit;

namespace Pclm.Core.Tests;

/// <summary>
/// 공고 품목이 <c>v_공고</c> 에 어떻게 비치는가.
///
/// <para>줄 하나가 공고 하나라는 약속을 지키면서 품목을 실으려면 <b>하나로 정할 수 있을 때만</b>
/// 실어야 한다. 첫 줄을 공고 전체의 값인 양 내면 잘린 것은 눈에 띄지 않는데 틀린 값은 그대로
/// 문서에 찍힌다 — 그 경계를 여기에 박아 둔다.</para>
///
/// <para>실제 문서로는 이 경계를 시험할 수 없다. 세부품명이 여럿인 공고가 표본에 없어서다.
/// 그래서 값을 지어 넣는다 — 여기서 보는 것은 파싱이 아니라 <b>뷰의 셈</b>이다.</para>
/// </summary>
public class NoticeItemViewTests : IDisposable
{
    private readonly string _path = Path.Combine(
        Path.GetTempPath(), $"pclm-notice-items-{Guid.NewGuid():N}.db");

    private readonly Database _database;
    private readonly Store _store;

    public NoticeItemViewTests()
    {
        _database = PclmFile.Create(_path, PclmRole.Work);
        _store = new Store(_database);
    }

    /// <summary>
    /// 같은 물건을 여러 수요기관에 나눠 넣느라 줄만 늘어난 공고. 물건은 하나이므로 실리고,
    /// <b>수량은 합</b>이다 — 승강판 1대 + 2대는 3대다.
    /// </summary>
    [Fact]
    public void 세부품명이_하나면_수량을_합쳐_싣는다()
    {
        Put(
            Item(1, "승강판", "2510190201", quantity: 1, place: "한별기동지원단"),
            Item(2, "승강판", "2510190201", quantity: 2, place: "누림항공지원단"));

        var row = Notice();

        Assert.Equal("승강판", row["세부품명"]);
        Assert.Equal("2510190201", row["세부품명번호"]);
        Assert.Equal("3", row["수량"]);
        Assert.Equal("대", row["단위"]);
    }

    /// <summary>
    /// 세부품명이 여럿이어도 <b>칸마다</b> 본다. 줄마다 같은 수요기관·단위·납품기한은 싣고,
    /// 갈리는 세부품명은 비운다. 단위가 같으니 <b>수량은 합</b>이다 — 세부품명 한 가지라는
    /// 조건에 묶여 하나로 정해진 칸까지 모두 비던 자리다.
    /// </summary>
    [Fact]
    public void 세부품명이_여럿이어도_줄마다_같은_칸은_싣는다()
    {
        Put(
            Item(1, "승강판", "2510190201", quantity: 1, due: new DateTime(2026, 9, 26)),
            Item(2, "여과망", "1210190101", quantity: 5, due: new DateTime(2026, 9, 26)));

        var row = Notice();

        Assert.Equal("한별군수지원단", row["수요기관"]);
        Assert.Equal("대", row["단위"]);
        Assert.Equal("2026/09/26", row["납품기한"]);
        Assert.Equal("6", row["수량"]);
        Assert.Equal("", row["세부품명"]);
        Assert.Equal("", row["세부품명번호"]);

        // 비운 것은 공고 뷰뿐이다. 품목 뷰에는 두 줄이 그대로 있다.
        Assert.Equal(2, ItemRows().Count);
    }

    /// <summary>
    /// 단위가 갈리면 수량을 더하지 않는다. 「대」 와 「식」 을 더한 수는 아무것도 말하지
    /// 않으면서 문서에는 그럴듯하게 찍힌다.
    /// </summary>
    [Fact]
    public void 단위가_갈리면_수량을_비운다()
    {
        Put(
            Item(1, "승강판", "2510190201", quantity: 1),
            Item(2, "여과망", "1210190101", quantity: 5, unit: "식"));

        var row = Notice();

        Assert.Equal("", row["수량"]);
        Assert.Equal("", row["단위"]);
    }

    /// <summary>
    /// 인도조건은 물건이 아니라 납품 방식이다. 세부품명이 여럿이어도 줄마다 같으면 싣고,
    /// 갈리면 비운다 — 세부품명 규칙에 묶여 하나로 정해진 값까지 버리던 자리다.
    /// </summary>
    [Theory]
    [InlineData("현장설치도", "현장설치도", "현장설치도")]
    [InlineData("현장설치도", "납품장소 입고도", "")]
    public void 인도조건은_세부품명이_여럿이어도_같으면_싣는다(string first, string second, string expected)
    {
        Put(
            Item(1, "승강판", "2510190201", quantity: 1, terms: first),
            Item(2, "여과망", "1210190101", quantity: 5, terms: second));

        Assert.Equal(expected, Notice()["인도조건"]);
    }

    /// <summary>
    /// 세부품명이 하나여도 <b>칸마다</b> 본다. 수요기관이 줄마다 다르면 그 칸만 비고,
    /// 하나로 정할 수 있는 칸은 그대로 실린다.
    /// </summary>
    [Fact]
    public void 세부품명이_하나여도_갈리는_칸은_비운다()
    {
        Put(
            Item(1, "승강판", "2510190201", quantity: 1, agency: "한별군수지원단"),
            Item(2, "승강판", "2510190201", quantity: 2, agency: "누림군수지원단"));

        var row = Notice();

        Assert.Equal("", row["수요기관"]);
        Assert.Equal("승강판", row["세부품명"]);
        Assert.Equal("3", row["수량"]);
    }

    /// <summary>
    /// <b>빈 줄도 한 값이다.</b> 한 줄만 수요기관을 적고 다른 줄이 비었으면 하나로 정한 것이
    /// 아니다 — <c>COUNT(DISTINCT)</c> 가 NULL 을 세지 않아 그 한 줄의 값이 공고 전체의 값처럼
    /// 찍히던 자리다. 옛 줄에는 NULL 대신 빈 문자열이 남아 있을 수 있어 둘 다 본다.
    /// </summary>
    [Theory]
    [InlineData("한별군수지원단", "한별군수지원단", "한별군수지원단")]
    [InlineData("한별군수지원단", null, "")]
    [InlineData("한별군수지원단", "", "")]
    [InlineData(null, "", "")]
    public void 빈_줄이_섞이면_칸을_비운다(string? first, string? second, string expected)
    {
        Put(
            Item(1, "승강판", "2510190201", quantity: 1, agency: first),
            Item(2, "승강판", "2510190201", quantity: 2, agency: second));

        var row = Notice();

        Assert.Equal(expected, row["수요기관"]);
        Assert.Equal("승강판", row["세부품명"]);
    }

    /// <summary>
    /// 수량이 빈 줄이 있으면 더하지 않는다. <c>SUM</c> 은 NULL 을 건너뛰어 5 와 빈 칸의 합을
    /// 5 로 냈다 — 모르는 수량을 0 으로 친 합이 계약 수량처럼 찍힌다.
    /// </summary>
    [Theory]
    [InlineData(5, 3, "8")]
    [InlineData(5, null, "")]
    [InlineData(null, null, "")]
    public void 수량이_빈_줄이_있으면_더하지_않는다(int? first, int? second, string expected)
    {
        Put(
            Item(1, "승강판", "2510190201", quantity: first),
            Item(2, "여과망", "1210190101", quantity: second));

        var row = Notice();

        Assert.Equal(expected, row["수량"]);
        Assert.Equal("대", row["단위"]);
    }

    /// <summary>
    /// 담당자도 칸이 빈 사람을 한 사람으로 센다. 전화를 적은 사람 하나와 비운 사람 하나를
    /// 두고 앞사람의 전화를 공고의 담당자전화로 낼 수는 없다.
    /// </summary>
    [Theory]
    [InlineData("010-0000-0000", "010-0000-0000", "010-0000-0000")]
    [InlineData("010-0000-0000", null, "")]
    [InlineData("010-0000-0000", "", "")]
    public void 담당자_칸도_빈_사람이_섞이면_비운다(string? first, string? second, string expected)
    {
        Put(
            [Item(1, "승강판", "2510190201", quantity: 1)],
            [
                new NoticeContactRecord { LineNo = 1, DemandAgency = "한별군수지원단", Officer = "홍길동", Phone = first },
                new NoticeContactRecord { LineNo = 2, DemandAgency = "한별군수지원단", Officer = "홍길동", Phone = second },
            ]);

        var row = Notice();

        Assert.Equal(expected, row["담당자전화"]);
        Assert.Equal("홍길동", row["담당자"]);
    }

    /// <summary>
    /// 납품기한은 한 칸이다. 날짜가 박혀 있으면 그 날, 일수만 있으면 계약일로부터 센다.
    ///
    /// <para>날짜가 있는 공고는 일수를 <c>0</c> 으로 적어 두므로 날짜가 먼저다 —
    /// 그 <c>0</c> 을 풀면 "계약 후 0일 이내"라는 없는 말이 나온다.</para>
    /// </summary>
    [Theory]
    [InlineData(90, null, "계약 후 90일 이내")]
    [InlineData(0, "2026-09-26T00:00:00", "2026/09/26")]
    [InlineData(0, null, "")]
    public void 납품기한은_날짜가_먼저다(int days, string? due, string expected)
    {
        Put(Item(1, "여과망", "1210190101", quantity: 3, days: days,
            due: due is null ? null : DateTime.Parse(due)));

        Assert.Equal(expected, Notice()["납품기한"]);
        Assert.Equal(expected, ItemRows()[0]["납품기한"]);
    }

    // ── 거들기 ───────────────────────────────────────────

    private static NoticeItemRecord Item(
        int lineNo, string name, string number, int? quantity,
        string? agency = "한별군수지원단", string place = "어딘가",
        int? days = 300, DateTime? due = null, string terms = "현장설치도", string unit = "대") =>
        new()
        {
            LineNo = lineNo,
            DemandAgency = agency,
            ItemName = name,
            DetailItemNumber = number,
            Quantity = quantity,
            Unit = unit,
            DeliveryDays = days,
            DeliveryDue = due,
            DeliveryPlace = place,
            DeliveryTerms = terms,
        };

    private void Put(params NoticeItemRecord[] items) => Put(items, []);

    private void Put(NoticeItemRecord[] items, NoticeContactRecord[] contacts) =>
        _store.UpsertNotice(
            new NoticeRecord
            {
                NoticeBase = "R26BK09999999",
                Seq = "000",
                Title = "지어낸 공고",
                Items = items,
                Contacts = contacts,
            });

    private Dictionary<string, string> Notice() => Rows("v_공고")[0];

    private List<Dictionary<string, string>> ItemRows() => Rows("v_공고품목");

    private List<Dictionary<string, string>> Rows(string view)
    {
        using var connection = _database.OpenReadOnly();
        using var command = connection.CreateCommand();
        command.CommandText = $"SELECT * FROM \"{view}\" ORDER BY 1;";

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

    public void Dispose()
    {
        SqliteConnection.ClearAllPools();
        GC.SuppressFinalize(this);
        if (File.Exists(_path)) File.Delete(_path);
    }
}

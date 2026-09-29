using ClosedXML.Excel;
using Dapper;
using Microsoft.Data.Sqlite;
using Pclm.Core.Storage;
using Xunit;

namespace Pclm.Core.Tests;

/// <summary>
/// 연간 조달계획 엑셀을 읽어 넣는다.
///
/// <para>여기서 붙들어 두는 것은 <b>표본 서식 그대로 읽히는가</b>이다. 서식은 표본
/// (「조달계획 (양식 표본).xlsx」)에 고정되어 있고 열 대응표는 걷었다(ADR-023 개정) — 짚을
/// 것이 없으니 갓 깐 사람도 첫 가져오기에서 막히지 않아야 한다.</para>
///
/// <para>그다음으로 조심하는 것은 <b>절반만 들어가는 일</b>이다. 필수 머리글이 없는데도
/// 들어갈 수 있는 것만 들어가면, 그 뒤로 무엇이 덜 왔는지 아무도 모른다.</para>
///
/// <para>시험용 엑셀은 <b>그 자리에서 지어 쓴다</b> — 표본 파일은 실제 조달 자료라 시험에
/// 쓰지 않는다. 값은 모두 지어낸 것이다.</para>
/// </summary>
public class PlanImportTests : IDisposable
{
    private readonly string _path = Path.Combine(
        Path.GetTempPath(), $"pclm-plan-{Guid.NewGuid():N}.db");

    private readonly string _folder = Path.Combine(
        Path.GetTempPath(), $"pclm-plan-{Guid.NewGuid():N}");

    private readonly Database _database;
    private readonly PlanImport _import;

    /// <summary>표본의 첫 줄. 열두 개, 이 차례 그대로다.</summary>
    private static readonly string[] 표본머리글 =
    [
        "조달요구번호", "재고번호", "품명", "화폐구분", "단위", "지시수량",
        "요청부대부서명", "요청부대담당자", "요청부대사용자전화번호",
        "계약부서", "담당자", "연락처",
    ];

    /// <summary>표본 한 줄. 값은 모두 지어낸 것이다.</summary>
    private static readonly string[] 표본줄 =
    [
        "MPKPLA26910286", "2590-01-234-5678", "전동 윈치", "KRW", "대", "8",
        "한별군수지원단 보급창", "홍길동", "051-000-0000",
        "계약1과", "김철수", "02-000-0000",
    ];

    public PlanImportTests()
    {
        _database = new Database(_path);
        _database.Migrate();
        _import = new PlanImport(_database);
        Directory.CreateDirectory(_folder);
    }

    /// <summary>표본 그대로면 짚어 줄 것 없이 잡힌다 — 갓 깐 사람이 바로 넣을 수 있어야 한다.</summary>
    [Fact]
    public void 표본_머리글을_그대로_잡는다()
    {
        var result = _import.Import(Book(표본머리글, [표본줄]));

        Assert.True(result.Ok);
        Assert.Equal(1, result.Rows);
        Assert.Equal(1, result.Created);

        Assert.Equal("2590-01-234-5678", Cell("MPKPLA26910286", "stock_number"));
        Assert.Equal("전동 윈치", Cell("MPKPLA26910286", "item_name"));
        Assert.Equal("KRW", Cell("MPKPLA26910286", "currency"));
        Assert.Equal("대", Cell("MPKPLA26910286", "unit"));
        Assert.Equal("8", Cell("MPKPLA26910286", "quantity"));
        Assert.Equal("한별군수지원단 보급창", Cell("MPKPLA26910286", "requesting_unit"));
        Assert.Equal("홍길동", Cell("MPKPLA26910286", "requesting_officer"));
        Assert.Equal("051-000-0000", Cell("MPKPLA26910286", "requesting_phone"));
        Assert.Equal("계약1과", Cell("MPKPLA26910286", "contract_department"));
        Assert.Equal("김철수", Cell("MPKPLA26910286", "officer"));
        Assert.Equal("02-000-0000", Cell("MPKPLA26910286", "contact"));
    }

    /// <summary>
    /// 필수 머리글이 없으면 <b>한 줄도 들어가지 않는다</b>. 잡을 수 있었던 열까지 함께
    /// 막아야, 사람이 파일을 고치기 전에 반쪽짜리 계획을 분모로 삼는 일이 없다.
    ///
    /// <para>없는 것은 <b>머리글 이름</b>으로 적는다 — 사람이 고칠 것은 엑셀의 첫 줄이지
    /// DB 의 열 이름이 아니다.</para>
    /// </summary>
    [Fact]
    public void 필수_머리글이_없으면_한_줄도_들어가지_않는다()
    {
        var book = Book(["조달요구번호", "품명", "책임자"],
        [
            ["MPKPLA26910286", "전동 윈치", "김철수"],
            ["MPKPLA26910290", "전동 권양기", "김철수"],
        ]);

        var result = _import.Import(book);

        Assert.False(result.Ok);
        Assert.Equal(["담당자"], result.MissingRequired);
        Assert.Equal(0, result.Created);
        Assert.Equal(0, Rows());
    }

    /// <summary>무엇이 들어갔는지는 보여야 한다 — 잡힌 머리글을 표본 차례대로 낸다.</summary>
    [Fact]
    public void 실제로_잡힌_머리글을_표본_차례로_낸다()
    {
        // 표본에 없는 열이 섞여 있어도 그냥 읽는다. 그 열은 잡히지 않을 뿐이다.
        var book = Book(["담당자", "품명", "비고", "조달요구번호"],
        [
            ["김철수", "전동 윈치", "1분기", "MPKPLA26910286"],
        ]);

        var result = _import.Import(book);

        Assert.True(result.Ok);
        Assert.Equal(["조달요구번호", "품명", "담당자"], result.Found);
    }

    /// <summary>
    /// 계획변경은 차수를 올리지 않고 덮어쓴다(스키마 V12). 같은 계획을 다시 넣어도 행이 늘지 않는다.
    /// </summary>
    [Fact]
    public void 다시_가져오면_조달요구번호_기준으로_갱신된다()
    {
        string[] 고친줄 =
        [
            "MPKPLA26910286", "2590-01-234-5678", "전동 윈치", "KRW", "대", "6",
            "한별군수지원단 보급창", "홍길동", "051-000-0000",
            "계약2과", "이영희", "02-000-0000",
        ];

        _import.Import(Book(표본머리글, [표본줄]));
        var again = _import.Import(Book(표본머리글, [고친줄]));

        Assert.Equal(0, again.Created);
        Assert.Equal(1, again.Updated);
        Assert.Equal(1, Rows());
        Assert.Equal("6", Cell("MPKPLA26910286", "quantity"));
        Assert.Equal("이영희", Cell("MPKPLA26910286", "officer"));
        Assert.Equal("계약2과", Cell("MPKPLA26910286", "contract_department"));
    }

    /// <summary>
    /// 지시수량은 <b>INTEGER</b> 로 담는다 — <c>request_item.quantity</c> 와 형을 맞춰야
    /// 계획을 분모로 놓고 견줄 수 있다. 자릿점이 찍혀 와도 셈할 수 있는 꼴로 들어간다.
    /// </summary>
    [Theory]
    [InlineData("8", 8L)]
    [InlineData("1,200", 1200L)]
    public void 지시수량은_정수로_담긴다(string written, long stored)
    {
        var book = Book(["조달요구번호", "담당자", "지시수량"],
        [
            ["MPKPLA26910286", "김철수", written],
        ]);

        _import.Import(book);

        using var connection = _database.OpenReadOnly();
        Assert.Equal(stored, connection.ExecuteScalar<long>(
            "SELECT quantity FROM plan WHERE request_number = 'MPKPLA26910286';"));
    }

    /// <summary>
    /// 번호가 빈 줄은 담을 자리가 없다. <b>조용히 버리지 않고 세어 낸다</b> — 계획 엑셀에는
    /// 소계·머리말 줄이 섞여 있어, 몇 줄이 빠졌는지 사람이 알아야 한다.
    /// </summary>
    [Fact]
    public void 조달요구번호가_빈_줄은_건너뛰고_세어_낸다()
    {
        string[] 소계 =
        [
            "", "", "소계", "KRW", "", "11",
            "", "", "", "", "김철수", "",
        ];

        string[] 둘째 =
        [
            "MPKPLA26910290", "2590-01-234-9999", "전동 권양기", "KRW", "대", "3",
            "한별군수지원단 보급창", "홍길동", "051-000-0000",
            "계약1과", "김철수", "02-000-0000",
        ];

        var result = _import.Import(Book(표본머리글, [표본줄, 소계, 둘째]));

        Assert.Equal(3, result.Rows);
        Assert.Equal(2, result.Created);
        Assert.Equal(1, result.Skipped);
        Assert.Equal(2, Rows());
    }

    /// <summary>명령줄이 무엇을 읽었는지 보여 주려면 머리글이 차례대로 나와야 한다.</summary>
    [Fact]
    public void 첫_줄의_머리글을_차례대로_낸다()
    {
        var book = Book(["조달요구번호", "품명", "담당자"], []);

        Assert.Equal(["조달요구번호", "품명", "담당자"], PlanImport.Headers(book));
    }

    /// <summary>지어낸 계획 엑셀 한 권. 값은 모두 문자열로 적어 엑셀이 재해석하지 않게 한다.</summary>
    private string Book(IReadOnlyList<string> headers, IReadOnlyList<string[]> rows)
    {
        var path = Path.Combine(_folder, $"계획-{Guid.NewGuid():N}.xlsx");

        using var workbook = new XLWorkbook();
        var sheet = workbook.Worksheets.Add("계획");

        for (var c = 0; c < headers.Count; c++)
            sheet.Cell(1, c + 1).Value = headers[c];

        for (var r = 0; r < rows.Count; r++)
            for (var c = 0; c < rows[r].Length; c++)
                sheet.Cell(r + 2, c + 1).Value = rows[r][c];

        workbook.SaveAs(path);
        return path;
    }

    private int Rows()
    {
        using var connection = _database.OpenReadOnly();
        return connection.ExecuteScalar<int>("SELECT COUNT(*) FROM plan;");
    }

    /// <summary>수량은 INTEGER 로 담기므로 문자열로 못 박지 않고 받아 적는다.</summary>
    private string Cell(string requestNumber, string column)
    {
        using var connection = _database.OpenReadOnly();
        return connection.ExecuteScalar<object?>(
            $"SELECT \"{column}\" FROM plan WHERE request_number = @n;",
            new { n = requestNumber })?.ToString() ?? "";
    }

    public void Dispose()
    {
        SqliteConnection.ClearAllPools();
        foreach (var suffix in new[] { "", "-wal", "-shm" })
            File.Delete(_path + suffix);

        Directory.Delete(_folder, recursive: true);
        GC.SuppressFinalize(this);
    }
}

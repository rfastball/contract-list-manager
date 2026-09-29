using System.Globalization;
using ClosedXML.Excel;
using Dapper;
using Pclm.Core.Domain;

namespace Pclm.Core.Storage;

/// <summary>
/// 계획 엑셀 한 권을 읽은 결과.
///
/// <para><b>던지지 않고 담아 낸다.</b> 머리글이 어긋난 것은 고장이 아니라 <b>사람이 파일을
/// 다시 보면 되는 일</b>이라, 예외로 올리면 화면과 명령줄이 사람에게 무엇을 짚어야 하는지
/// 보여 줄 수 없다. 무엇이 없었는지와 실제로 무엇을 읽었는지를 함께 낸다.</para>
/// </summary>
/// <param name="Rows">머리글 아래에서 읽은 줄 수. 통째로 빈 줄은 세지 않는다.</param>
/// <param name="Skipped">조달요구번호가 빈 줄. 계획의 자연키라 그 줄은 담을 자리가 없다.</param>
/// <param name="MissingRequired">
/// 없던 필수 <b>머리글 이름</b>(<c>조달요구번호</c>·<c>담당자</c>). 자리 이름이 아니라 머리글로
/// 내는 까닭은, 사람이 고칠 것이 엑셀의 첫 줄이라서다.
/// <b>하나라도 있으면 한 줄도 들어가지 않았다</b>.
/// </param>
/// <param name="Found">
/// 실제로 잡힌 머리글 목록. 사람이 <b>"어느 열이 들어갔나"</b> 를 눈으로 확인하는 자리다 —
/// 표본에 없는 열이 섞인 파일도 그냥 읽히므로, 무엇이 들어왔는지는 보여야 한다.
/// </param>
public sealed record PlanImportResult(
    int Rows,
    int Created,
    int Updated,
    int Skipped,
    IReadOnlyList<string> MissingRequired,
    IReadOnlyList<string> Found)
{
    /// <summary>필수 머리글이 다 있었는가. 거짓이면 DB 는 손대지 않은 채다.</summary>
    public bool Ok => MissingRequired.Count == 0;
}

/// <summary>
/// 연간 조달계획 엑셀을 읽어 <c>plan</c> 에 넣는다.
///
/// <para><b>서식은 표본에 고정한다</b>(ADR-023 개정, 스키마 V13). 한때는 서식이 해마다·부서마다
/// 달라진다고 보고 열 대응표를 설정에 두었는데, 실제로 도는 서식은 「조달계획 (양식 표본).xlsx」
/// 하나로 굳었다. 대응표는 값을 하는 대신 <b>갓 깐 사람이 첫 가져오기에서 막히는 자리</b>가
/// 되었다 — 짚기 전에는 한 줄도 들어가지 않는데, 짚을 것은 표본 그대로였다.</para>
///
/// <para>그래서 열은 <see cref="Columns"/> 하나로 못 박고 <b>머리글 이름이 정확히 같은
/// 열</b>만 잡는다. 비슷한 이름까지 잡지 않는 까닭은, 틀리게 잡힌 열은 빈 열보다 고치기
/// 어렵기 때문이다 — 비면 사람 눈에 띄지만, 엉뚱한 값이 들어차면 그대로 문서에 실린다.</para>
///
/// <para><b>필수 둘을 못 찾으면 한 줄도 넣지 않는다.</b> 조달요구번호는 자연키이고 담당자는
/// 계획을 사람에게 돌려주는 유일한 손잡이다. 둘 중 하나가 비면 들어간 자료로 할 수 있는 일이
/// 없는데, 절반만 들어가면 그 뒤로 <b>무엇이 덜 들어왔는지</b>를 아무도 모른다.</para>
///
/// <para>값은 <b>적힌 그대로</b> 담는다(ADR-016) — 표본의 열은 지시수량 하나만 셈하는 값이라
/// <see cref="ValueParser.Integer"/> 를 타고, 나머지는 문자열이다. <b>여기서 꾸미지 않는다</b>.
/// 문서에 찍힐 표기는 뷰가 씌우는 것이라, 원본 표에는 온 그대로 둔다.</para>
/// </summary>
public sealed class PlanImport(Database database)
{
    /// <summary>
    /// 담을 자리 → 표본의 머리글. <b>차례가 곧 표본의 차례</b>이고, 뷰와 명령줄이 보여 주는
    /// 차례이기도 하다.
    /// </summary>
    public static readonly IReadOnlyDictionary<string, string> Columns =
        new Dictionary<string, string>
        {
            ["request_number"] = "조달요구번호",
            ["stock_number"] = "재고번호",
            ["item_name"] = "품명",
            ["currency"] = "화폐구분",
            ["unit"] = "단위",
            ["quantity"] = "지시수량",
            ["requesting_unit"] = "요청부대부서명",
            ["requesting_officer"] = "요청부대담당자",
            ["requesting_phone"] = "요청부대사용자전화번호",
            ["contract_department"] = "계약부서",
            ["officer"] = "담당자",
            ["contact"] = "연락처",
        };

    /// <summary>이것을 못 찾으면 한 줄도 넣지 않는다.</summary>
    public static readonly IReadOnlyList<string> Required = ["request_number", "officer"];

    private readonly Database _database = database;

    /// <summary>
    /// 첫 줄의 머리글. <c>pclm plan</c> 이 <b>무엇을 읽었는지 보여 주려고</b> 쓴다.
    ///
    /// <para>빈 칸도 자리를 지켜 그대로 낸다 — 차례가 곧 열 번호라, 걷어 내면 사람이 보는
    /// 번호와 실제 열이 어긋난다.</para>
    /// </summary>
    public static IReadOnlyList<string> Headers(string xlsxPath)
    {
        using var workbook = new XLWorkbook(xlsxPath);
        return HeadersOf(workbook.Worksheets.FirstOrDefault()?.FirstRowUsed());
    }

    /// <summary>
    /// 엑셀 한 권을 읽어 <c>request_number</c> 기준으로 업서트한다.
    ///
    /// <para>계획변경은 차수를 올리지 않고 <b>덮어쓴다</b>(스키마 V12) — 같은 파일을 몇 번
    /// 넣어도, 고쳐진 계획을 다시 넣어도 행이 늘지 않는다.</para>
    /// </summary>
    public PlanImportResult Import(string xlsxPath)
    {
        using var workbook = new XLWorkbook(xlsxPath);
        var sheet = workbook.Worksheets.FirstOrDefault();
        var headerRow = sheet?.FirstRowUsed();
        var headers = HeadersOf(headerRow);

        var found = Resolve(headers);
        var missing = Required
            .Where(t => !found.ContainsKey(t))
            .Select(t => Columns[t])
            .ToList();

        // 못 찾았으면 줄을 세지도 않는다. "300줄을 읽었지만 0줄이 들어갔다" 는 말은
        // 사람에게 무엇이 잘못됐는지를 알려 주는 대신 자료가 나쁘다는 오해만 남긴다.
        var read = Columns.Keys.Where(found.ContainsKey).Select(t => Columns[t]).ToList();
        if (missing.Count > 0 || headerRow is null || sheet is null)
            return new PlanImportResult(0, 0, 0, 0, missing, read);

        var lastRow = sheet.LastRowUsed()?.RowNumber() ?? headerRow.RowNumber();
        var now = DateTime.UtcNow.ToString("O");
        var sourceName = Path.GetFileName(xlsxPath);

        using var connection = _database.Open();
        using var transaction = connection.BeginTransaction();

        // 이미 있던 번호를 미리 걷어 신규와 갱신을 가른다. 한 파일 안에 같은 번호가 두 번
        // 나오면 두 번째는 갱신이라, 넣으면서 이 집합도 함께 불린다.
        var seen = connection
            .Query<string>("SELECT request_number FROM plan;", transaction: transaction)
            .ToHashSet(StringComparer.Ordinal);

        int rows = 0, created = 0, updated = 0, skipped = 0;

        for (var r = headerRow.RowNumber() + 1; r <= lastRow; r++)
        {
            var row = sheet.Row(r);
            var values = Columns.Keys.ToDictionary(t => t, t => Cell(row, found, t));

            // 통째로 빈 줄은 자료가 아니다. 세면 엑셀 끝에 남은 빈 줄이 건너뜀으로 잡혀,
            // 진짜로 번호가 빠진 줄이 그 숫자에 묻힌다.
            if (values.Values.All(v => v.Length == 0)) continue;

            rows++;

            var number = values["request_number"];
            if (number.Length == 0) { skipped++; continue; }

            if (seen.Add(number)) created++; else updated++;

            connection.Execute(
                """
                INSERT INTO plan (
                    request_number, stock_number, item_name, currency, unit, quantity,
                    requesting_unit, requesting_officer, requesting_phone,
                    contract_department, officer, contact, source_name, imported_at
                ) VALUES (
                    @Number, @StockNumber, @ItemName, @Currency, @Unit, @Quantity,
                    @RequestingUnit, @RequestingOfficer, @RequestingPhone,
                    @ContractDepartment, @Officer, @Contact, @SourceName, @ImportedAt
                )
                ON CONFLICT(request_number) DO UPDATE SET
                    stock_number        = excluded.stock_number,
                    item_name           = excluded.item_name,
                    currency            = excluded.currency,
                    unit                = excluded.unit,
                    quantity            = excluded.quantity,
                    requesting_unit     = excluded.requesting_unit,
                    requesting_officer  = excluded.requesting_officer,
                    requesting_phone    = excluded.requesting_phone,
                    contract_department = excluded.contract_department,
                    officer             = excluded.officer,
                    contact             = excluded.contact,
                    source_name         = excluded.source_name,
                    imported_at         = excluded.imported_at;
                """,
                new
                {
                    Number = number,
                    StockNumber = Text(values["stock_number"]),
                    ItemName = Text(values["item_name"]),
                    Currency = Text(values["currency"]),
                    Unit = Text(values["unit"]),
                    Quantity = ValueParser.Integer(values["quantity"]),
                    RequestingUnit = Text(values["requesting_unit"]),
                    RequestingOfficer = Text(values["requesting_officer"]),
                    RequestingPhone = Text(values["requesting_phone"]),
                    ContractDepartment = Text(values["contract_department"]),
                    Officer = Text(values["officer"]),
                    Contact = Text(values["contact"]),
                    SourceName = sourceName,
                    ImportedAt = now,
                },
                transaction);
        }

        transaction.Commit();
        return new PlanImportResult(rows, created, updated, skipped, [], read);
    }

    /// <summary>
    /// 자리 이름 → 열 번호. 머리글 이름이 <b>정확히 같은</b> 열만 잡고, 같은 이름이 둘이면
    /// <b>앞의 것</b>을 쓴다.
    /// </summary>
    private static Dictionary<string, int> Resolve(IReadOnlyList<string> headers)
    {
        var byName = new Dictionary<string, int>(StringComparer.Ordinal);
        for (var i = 0; i < headers.Count; i++)
            if (headers[i].Length > 0 && !byName.ContainsKey(headers[i]))
                byName[headers[i]] = i + 1;

        var columns = new Dictionary<string, int>(StringComparer.Ordinal);

        foreach (var (target, header) in Columns)
            if (byName.TryGetValue(header, out var column))
                columns[target] = column;

        return columns;
    }

    private static IReadOnlyList<string> HeadersOf(IXLRow? row)
    {
        if (row is null) return [];

        var last = row.LastCellUsed()?.Address.ColumnNumber ?? 0;
        var headers = new List<string>(last);

        for (var c = 1; c <= last; c++) headers.Add(Raw(row.Cell(c)));
        return headers;
    }

    private static string Cell(IXLRow row, Dictionary<string, int> columns, string target) =>
        columns.TryGetValue(target, out var column) ? Raw(row.Cell(column)) : string.Empty;

    /// <summary>
    /// 칸을 문자열로. <b>엑셀이 보여 주는 모양이 아니라 담긴 값</b>을 집는다 — 표시 서식이
    /// 반올림해 둔 수를 그대로 믿으면 원본에 없던 값이 계획에 앉는다.
    /// </summary>
    private static string Raw(IXLCell cell) => cell.Value.Type switch
    {
        XLDataType.Blank => string.Empty,

        // 지수 표기로 도망가지 않게 자릿수를 넉넉히 편다. 소수부는 있는 만큼만 남는다.
        XLDataType.Number => cell.Value.GetNumber()
            .ToString("0.###########################", CultureInfo.InvariantCulture),

        // 시각이 붙어 있으면 버리지 않는다 — 사람이 적어 둔 것이라 지울 근거가 없다.
        XLDataType.DateTime => cell.Value.GetDateTime() is var d && d.TimeOfDay == TimeSpan.Zero
            ? d.ToString("yyyy-MM-dd", CultureInfo.InvariantCulture)
            : d.ToString("yyyy-MM-dd HH:mm:ss", CultureInfo.InvariantCulture),

        _ => cell.GetString().Trim(),
    };

    /// <summary>
    /// 빈 칸은 NULL 로 담는다. 계획 표는 사람이 보는 계약면이 아니라 <c>request</c>·
    /// <c>notice</c> 와 나란한 <b>원본 표</b>라, 빈 문자열을 씌우는 일은 뷰가 맡는다.
    /// </summary>
    private static string? Text(string value) => value.Length == 0 ? null : value;
}

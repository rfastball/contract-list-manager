using ClosedXML.Excel;
using Microsoft.Data.Sqlite;

namespace Pclm.Core.Storage;

/// <summary>
/// 계약면 뷰를 엑셀 한 권으로 내보낸다.
///
/// <para>이 파일은 <b>그 시점의 사진</b>이지 원본이 아니다. 언제든 다시 뽑을 수 있고,
/// 여기에 손으로 적은 것은 DB로 돌아오지 않는다 — 되돌리는 경로는 만들지 않는다.
/// 쓰임새는 보관·공유·다른 도구로의 반출이다.</para>
///
/// <para>시트를 뷰마다 하나씩 두는 이유는 <b>한 줄이 뜻하는 것이 시트마다 다르기</b> 때문이다.
/// 받는 쪽이 한 줄로 문서 한 장을 만드는데, 계약 한 건이 한 줄인 시트와 품목 한 줄이
/// 한 줄인 시트를 섞으면 안 된다.</para>
/// </summary>
public sealed class Exporter(Database database)
{
    private readonly Database _database = database;

    /// <summary>기본 파일 이름. 날짜를 박아 사진임을 드러낸다.</summary>
    public static string DefaultFileName(DateTime? on = null) =>
        $"관리대장_{(on ?? DateTime.Now):yyyyMMdd}.xlsx";

    public string Export(string path)
    {
        var directory = Path.GetDirectoryName(Path.GetFullPath(path));
        if (!string.IsNullOrEmpty(directory)) Directory.CreateDirectory(directory);

        using var connection = _database.OpenReadOnly();
        using var workbook = new XLWorkbook();

        // Names 가 아니라 Exported 를 돈다. 통합을 v1·v2 두 장으로 내면 받는 쪽이 어느 것을
        // 볼지 헷갈린다 — 파일에는 v2 만 싣는다.
        foreach (var view in Views.Exported)
            WriteSheet(workbook, connection, view);

        workbook.SaveAs(path);
        return Path.GetFullPath(path);
    }

    private static void WriteSheet(XLWorkbook workbook, SqliteConnection connection, string view)
    {
        // 시트 이름에는 뷰 판을 남기지 않는다 — 사람이 고르는 이름이라 짧을수록 낫다.
        var sheet = workbook.Worksheets.Add(Views.SheetName(view));

        using var command = connection.CreateCommand();
        command.CommandText = $"SELECT * FROM \"{view}\";";
        using var reader = command.ExecuteReader();

        for (var c = 0; c < reader.FieldCount; c++)
        {
            var cell = sheet.Cell(1, c + 1);
            cell.Value = reader.GetName(c);
            cell.Style.Font.Bold = true;
            cell.Style.Fill.BackgroundColor = XLColor.FromArgb(0xEE, 0xF1, 0xF4);
        }

        var row = 2;
        while (reader.Read())
        {
            for (var c = 0; c < reader.FieldCount; c++)
            {
                // 값은 이미 표시용 문자열이다. 엑셀이 숫자로 재해석해 자릿점을 지우거나
                // 긴 번호를 지수로 바꾸지 않도록 문자열로 못 박는다.
                var cell = sheet.Cell(row, c + 1);
                cell.SetValue(reader.IsDBNull(c) ? string.Empty : reader.GetValue(c).ToString());
                cell.Style.NumberFormat.Format = "@";
            }
            row++;
        }

        sheet.SheetView.FreezeRows(1);
        if (row > 1) sheet.RangeUsed()?.SetAutoFilter();
        sheet.Columns().AdjustToContents(1, 40d, 60d);
    }
}

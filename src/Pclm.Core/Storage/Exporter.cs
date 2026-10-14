using ClosedXML.Excel;
using Microsoft.Data.Sqlite;

namespace Pclm.Core.Storage;

public sealed record SheetSnapshot(string[] Columns, string[][] Rows);

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

        // Names 가 아니라 Exported 를 돈다. 차수를 편 뷰까지 함께 내면 같은 건이 여러 장에
        // 되풀이되어 받는 쪽이 어느 것을 셀지 헷갈린다.
        foreach (var view in Views.Exported)
            WriteSheet(workbook, connection, view);

        workbook.SaveAs(path);
        return Path.GetFullPath(path);
    }

    /// <summary>화면에서 확정한 열/행 순서 그대로 한 장을 낸다. 전체 자료 내보내기와 별개의 사진이다.</summary>
    public static string ExportSheet(string path, string title, SheetSnapshot snapshot)
    {
        ValidateSnapshot(title, snapshot);
        using var workbook = new XLWorkbook();
        var sheet = workbook.Worksheets.Add(title);
        for (var c = 0; c < snapshot.Columns.Length; c++)
        {
            sheet.Cell(1, c + 1).SetValue(snapshot.Columns[c]);
            sheet.Cell(1, c + 1).Style.Font.Bold = true;
            sheet.Cell(1, c + 1).Style.Fill.BackgroundColor = XLColor.FromArgb(0xEE, 0xF1, 0xF4);
        }
        for (var r = 0; r < snapshot.Rows.Length; r++)
            for (var c = 0; c < snapshot.Columns.Length; c++)
            {
                sheet.Cell(r + 2, c + 1).SetValue(snapshot.Rows[r][c]);
                sheet.Cell(r + 2, c + 1).Style.NumberFormat.Format = "@";
            }
        sheet.SheetView.FreezeRows(1);
        sheet.RangeUsed()?.SetAutoFilter();
        sheet.Columns().AdjustToContents(1, 40d, 60d);
        workbook.SaveAs(path);
        return Path.GetFullPath(path);
    }

    public static void ValidateSnapshot(string title, SheetSnapshot snapshot)
    {
        if (string.IsNullOrWhiteSpace(title) || title.Length > 31 || title.IndexOfAny(['[', ']', ':', '*', '?', '/', '\\']) >= 0
            || title.StartsWith('\'') || title.EndsWith('\'') || title.Any(char.IsControl))
            throw new InvalidOperationException("엑셀 시트 이름이 올바르지 않습니다.");
        if (snapshot?.Columns is null || snapshot.Rows is null || snapshot.Columns.Length is < 1 or > 16384
            || snapshot.Rows.Length > 1048575 || (long)snapshot.Columns.Length * snapshot.Rows.Length > 2_000_000
            || snapshot.Columns.Any(c => string.IsNullOrEmpty(c) || c.Length > 32767)
            || snapshot.Columns.Distinct().Count() != snapshot.Columns.Length
            || snapshot.Rows.Any(r => r is null || r.Length != snapshot.Columns.Length || r.Any(v => v is null || v.Length > 32767)))
            throw new InvalidOperationException("내보낼 표의 크기나 셀 값이 올바르지 않습니다. 최대 2,000,000셀을 내보낼 수 있습니다.");
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

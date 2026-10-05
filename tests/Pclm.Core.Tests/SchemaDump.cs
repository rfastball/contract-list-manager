using System.Text;
using Microsoft.Data.Sqlite;

namespace Pclm.Core.Tests;

/// <summary>
/// 스키마를 글 한 벌로 뜬다 — <c>contract/schema.txt</c> 박제와 그 대조가 이 하나를 쓴다.
///
/// <para>뜨는 것은 <c>sqlite_master</c> 의 표·색인·방아쇠 <b>정의 원문 그대로</b>다. 열 이름만 보는
/// 것이 아니라 SQLite 가 담아 둔 글자를 보므로, 기준선 스크립트가 옛 판올림 열아홉 단계와 한 글자라도
/// 다른 표를 지으면 여기서 갈린다. 뷰는 <c>contract/views.txt</c> 가 따로 본다.</para>
///
/// <para>뒤에는 갓 지은 DB 에 이미 들어 있는 행(씨앗)을 붙인다. DB 마다 무작위로 정해지는 값은
/// 자리표로 바꾼다 — 박제가 실행마다 흔들리면 박제가 아니다.</para>
/// </summary>
internal static class SchemaDump
{
    /// <summary>DB 마다 새로 뽑히는 값. 이름이 아니라 자리표를 적는다.</summary>
    private static readonly HashSet<(string Table, string Column)> Random =
    [
        ("pclm_file", "dataset_id"),
        ("pclm_file", "created_at"),
    ];

    public static string Of(SqliteConnection connection)
    {
        var text = new StringBuilder();

        var definitions = new List<(string Type, string Name, string Sql)>();
        using (var command = connection.CreateCommand())
        {
            command.CommandText =
                """
                SELECT type, name, sql FROM sqlite_master
                WHERE type IN ('table', 'index', 'trigger') AND name NOT LIKE 'sqlite\_%' ESCAPE '\'
                ORDER BY type, name;
                """;
            using var reader = command.ExecuteReader();
            while (reader.Read())
                definitions.Add((reader.GetString(0), reader.GetString(1), reader.IsDBNull(2) ? "" : reader.GetString(2)));
        }

        foreach (var (type, name, sql) in definitions)
        {
            text.Append(type).Append(' ').Append(name).Append('\n');
            // 줄바꿈도 고르지 않는다 — 기준선 원문에 CR 이 섞이면 그것도 다른 글자다.
            text.Append(sql).Append("\n\n");
        }

        text.Append("== 씨앗 ==\n");

        foreach (var (type, table, _) in definitions)
        {
            if (type != "table") continue;

            var columns = new List<string>();
            using (var command = connection.CreateCommand())
            {
                command.CommandText = $"PRAGMA table_info(\"{table}\");";
                using var reader = command.ExecuteReader();
                while (reader.Read()) columns.Add(reader.GetString(1));
            }

            var rows = new List<string>();
            using (var command = connection.CreateCommand())
            {
                var order = string.Join(", ", Enumerable.Range(1, columns.Count));
                command.CommandText = $"SELECT * FROM \"{table}\" ORDER BY {order};";
                using var reader = command.ExecuteReader();
                while (reader.Read())
                    rows.Add(string.Join(" | ", columns.Select((column, i) =>
                        $"{column}={(Random.Contains((table, column)) ? "<무작위>" : Literal(reader.GetValue(i)))}")));
            }

            if (rows.Count == 0) continue;

            text.Append('\n').Append(table).Append('\n');
            foreach (var row in rows) text.Append("  ").Append(row).Append('\n');
        }

        return text.ToString();
    }

    private static string Literal(object value) => value switch
    {
        DBNull => "NULL",
        string s => $"'{s}'",
        _ => Convert.ToString(value, System.Globalization.CultureInfo.InvariantCulture) ?? "",
    };
}

using System.Text;
using Microsoft.Data.Sqlite;
using Pclm.Core.Storage;
using Xunit;

namespace Pclm.Core.Tests;

/// <summary>
/// 바깥에 내놓는 <b>계약면을 박제해 두고 대조한다</b>.
///
/// <para>한동안 골든 시험이 파서를 지키는 동안 계약면은 무방비였다. 뷰의 열 이름 하나가 바뀌면
/// 소비자 쪽에 저장된 필드 연결이 끊기는데, 그것은 파서 회귀보다 비싸면서 아무 시험도
/// 걸리지 않았다. <c>docs/dataset-contract.md</c> 가 적어 둔 약속과 SQL 이 따로 놀 수 있었다(ADR-014).</para>
///
/// <para>여기 박제하는 것은 <b>열 이름과 순서</b>다. 값이 아니라 표면이라 실제 문서가 필요 없고,
/// 개인정보도 들어가지 않는다 — 그래서 이 박제는 저장소에 함께 둔다.</para>
///
/// <para>일부러 바꿨다면 <c>PCLM_UPDATE_GOLDEN=1</c> 로 다시 쓰고, <b>같은 커밋에서
/// 규약 문서도 함께 고친다</b>. 뷰·열 이름은 제자리에서 바꾼다 — 옛 이름을 나란히 남기지
/// 않는다(ADR-034). 박제는 바뀐 것이 <b>의도한 것뿐인지</b> 눈으로 보게 하는 장치다.</para>
/// </summary>
public class DatasetContractTests : IDisposable
{
    /// <summary>계약면 박제가 사는 곳. 열 이름만 담아 개인정보가 없고, 모두가 같은 것을 봐야 한다.</summary>
    private static readonly string ContractDirectory =
        Path.Combine(FindUpwards("tests") ?? ".", "Pclm.Core.Tests", "contract");

    private static readonly string GoldenPath = Path.Combine(ContractDirectory, "views.txt");

    /// <summary>박제를 다시 쓰는 모드. <c>PCLM_UPDATE_GOLDEN=1</c> 로 켠다 — 눈으로 본 뒤에만.</summary>
    private static bool UpdateMode =>
        Environment.GetEnvironmentVariable("PCLM_UPDATE_GOLDEN") is "1" or "true";

    private static string? FindUpwards(string name)
    {
        for (var directory = new DirectoryInfo(AppContext.BaseDirectory);
             directory is not null;
             directory = directory.Parent)
        {
            var candidate = Path.Combine(directory.FullName, name);
            if (Directory.Exists(candidate)) return candidate;
        }

        return null;
    }

    private readonly string _path = Path.Combine(
        Path.GetTempPath(), $"pclm-contract-{Guid.NewGuid():N}.db");

    private readonly Database _database;

    public DatasetContractTests()
    {
        _database = PclmFile.Create(_path, PclmRole.Work);
    }

    [Fact]
    public void 뷰_표면이_박제와_같다()
    {
        var actual = Surface();

        if (UpdateMode)
        {
            Directory.CreateDirectory(ContractDirectory);
            File.WriteAllText(GoldenPath, actual);
            return;
        }

        Assert.True(File.Exists(GoldenPath),
            $"계약면 박제가 없습니다: {GoldenPath}\nPCLM_UPDATE_GOLDEN=1 로 한 번 돌려 만드세요.");

        Assert.Equal(
            File.ReadAllText(GoldenPath).Replace("\r\n", "\n").TrimEnd(),
            actual.Replace("\r\n", "\n").TrimEnd());
    }

    /// <summary>약속한 뷰가 실제로 다 있는가. 하나라도 빠지면 소비자가 붙지 못한다.</summary>
    [Fact]
    public void 약속한_뷰가_모두_있다()
    {
        using var connection = _database.OpenReadOnly();

        var present = new HashSet<string>();
        using (var command = connection.CreateCommand())
        {
            command.CommandText = "SELECT name FROM sqlite_master WHERE type = 'view';";
            using var reader = command.ExecuteReader();
            while (reader.Read()) present.Add(reader.GetString(0));
        }

        foreach (var name in Views.Names) Assert.Contains(name, present);
    }

    /// <summary>
    /// 값은 모두 문서에 그대로 찍힐 문자열이다 — NULL 이 새어 나가면 받는 쪽의 빈 값 처리가
    /// 깨진다. 빈 DB 에서도 타입만은 확인할 수 있어 여기서 함께 본다.
    /// </summary>
    [Fact]
    public void 뷰의_열은_모두_문자열이다()
    {
        var 예외 = new HashSet<string> { "순번" };  // 품목 순번만 정수다.

        using var connection = _database.OpenReadOnly();

        foreach (var view in Views.Names)
        {
            using var command = connection.CreateCommand();
            command.CommandText = $"PRAGMA table_info(\"{view}\");";
            using var reader = command.ExecuteReader();

            while (reader.Read())
            {
                var name = reader.GetString(1);
                var type = reader.GetString(2);
                if (예외.Contains(name)) continue;

                Assert.True(type is "TEXT" or "",
                    $"{view}.{name} 의 타입이 {type} 입니다 — 계약면의 값은 문자열이어야 합니다.");
            }
        }
    }

    /// <summary>
    /// <see cref="Views.ContractColumns"/> 가 실제 <c>v_계약</c> 과 같은가.
    ///
    /// <para><c>v_통합</c> 은 계약 없는 줄도 내느라 계약 뷰를 <c>k.*</c> 로 받지 못하고
    /// 열 이름을 따로 들고 있다. 두 벌이라 어긋날 수 있는데, 어긋나면 <b>통합에서 그 열만
    /// 조용히 사라진다</b> — 아무 오류도 나지 않으므로 여기서 붙잡는다.</para>
    /// </summary>
    [Fact]
    public void 계약_열_목록이_계약_뷰와_같다()
    {
        var 사람열 = new Store(_database).UserColumns("contract").Select(c => c.FieldName);

        Assert.Equal([.. Views.ContractColumns, .. 사람열], ColumnsOf("v_계약"));
    }

    /// <summary><c>v_통합</c> 이 계약 뷰의 열을 하나도 빠뜨리지 않는가.</summary>
    [Fact]
    public void 통합이_계약_뷰의_열을_모두_담는다()
    {
        var 통합 = ColumnsOf("v_통합").ToHashSet();

        foreach (var column in ColumnsOf("v_계약")) Assert.Contains(column, 통합);
    }

    /// <summary>
    /// 차수 뷰의 열이 본 뷰와 <b>한 글자도 다르지 않다</b>. 둘의 차이는 최신 차수로 좁히는
    /// <c>FROM</c> 절뿐이라는 것이 이 뷰의 정의다.
    ///
    /// <para>지금은 <c>Views.공고열</c> 하나를 나눠 써서 갈릴 자리가 없지만, 갈라 적는 것이
    /// <b>아무것도 실패시키지 않는다</b>는 데서 이 시험이 필요하다 — 한쪽에만 열을 더하면
    /// 저쪽에서 그 열만 조용히 사라지고, 차수 뷰를 붙여 둔 쪽은 표가 이상해진 뒤에야 안다.
    /// <see cref="계약_열_목록이_계약_뷰와_같다"/> 와 나란한 자리다.</para>
    /// </summary>
    [Fact]
    public void 차수_뷰의_열이_본_뷰와_같다()
    {
        Assert.Equal(ColumnsOf("v_공고"), ColumnsOf("v_공고차수"));
        Assert.Equal(ColumnsOf("v_계약"), ColumnsOf("v_계약차수"));
    }

    /// <summary>
    /// <c>v_통합차수</c> 는 <c>v_통합</c> 의 열을 <b>그대로 두고</b> 끝에 둘만 더한 것이다.
    ///
    /// <para>두 뷰가 공고·접수 열 이름을 각자 적고 있어(SQL 에는 "이 뷰의 열을 다 가져오되
    /// 하나만 갈아 끼워라" 라고 적을 말이 없다) 한쪽만 고치면 갈린다. 갈려도 아무것도
    /// 실패하지 않으므로 여기서 붙든다 — 열 차례까지 본다.</para>
    /// </summary>
    [Fact]
    public void 통합차수는_통합의_열에_둘만_더한_것이다()
    {
        Assert.Equal([.. ColumnsOf("v_통합"), "공고건", "현행공고"], ColumnsOf("v_통합차수"));
    }

    /// <summary>
    /// 판을 붙여 부르던 옛 이름의 뷰가 남은 DB 를 열면 <b>하나도 남지 않는다</b>(ADR-034).
    ///
    /// <para>남으면 받는 쪽이 그것을 시트로 늘어놓고, 옛 뷰가 가리키던 표가 바뀌는 순간 깨진
    /// 뷰가 된다 — 깨진 뷰 하나가 SQLite 의 표 고치기(<c>ALTER TABLE</c>)까지 막는다.
    /// 옛 뷰끼리 서로 가리키던 모양(통합이 계약·공고·접수 위에 얹힌 것)까지 세워 두고 본다.</para>
    /// </summary>
    [Fact]
    public void 옛_이름의_뷰는_열면_모두_치워진다()
    {
        using (var connection = new SqliteConnection($"Data Source={_path}"))
        {
            connection.Open();
            using var command = connection.CreateCommand();
            command.CommandText = """
                CREATE VIEW v_공고_v1 AS SELECT * FROM v_공고;
                CREATE VIEW v_계약_v1 AS SELECT * FROM v_계약;
                CREATE VIEW v_접수_v1 AS SELECT * FROM v_접수;
                CREATE VIEW v_공고품목_v1 AS SELECT * FROM v_공고품목;
                CREATE VIEW v_품목_v1 AS SELECT * FROM v_계약품목;
                CREATE VIEW v_접수품목_v1 AS SELECT * FROM v_접수품목;
                CREATE VIEW v_공고차수_v1 AS SELECT * FROM v_공고차수;
                CREATE VIEW v_계약차수_v1 AS SELECT * FROM v_계약차수;
                CREATE VIEW v_ERP원천_v1 AS SELECT * FROM v_ERP원천;
                CREATE VIEW v_계약업체_v1 AS SELECT * FROM v_계약업체;
                CREATE VIEW v_ERP접수_v1 AS SELECT * FROM v_ERP접수;
                CREATE VIEW v_통합_v1 AS
                    SELECT k.*, g.공고명 FROM v_계약_v1 k LEFT JOIN v_공고_v1 g ON 0;
                CREATE VIEW v_통합_v2 AS
                    SELECT k.계약번호, r.요청명 FROM v_통합_v1 k LEFT JOIN v_접수_v1 r ON 0;
                CREATE VIEW v_통합_v3 AS
                    SELECT k.*, g.차수 AS 공고차수 FROM v_통합_v2 k LEFT JOIN v_공고차수_v1 g ON 0;
                CREATE VIEW v_계획_v1 AS
                    SELECT r.접수번호, g.입찰공고번호, k.계약번호
                    FROM v_접수_v1 r LEFT JOIN v_공고_v1 g ON 0 LEFT JOIN v_계약_v1 k ON 0;
                """;
            command.ExecuteNonQuery();
        }

        Assert.Equal(Views.LegacyNames.Order(), ViewNames().Intersect(Views.LegacyNames).Order());

        SqliteConnection.ClearAllPools();
        new Database(_path).Migrate();

        var present = ViewNames();
        Assert.Empty(present.Intersect(Views.LegacyNames));
        foreach (var name in Views.Names) Assert.Contains(name, present);
    }

    /// <summary>옛 이름이 지금 이름과 겹치면 막 지은 뷰를 맨 앞의 치우기가 지운다.</summary>
    [Fact]
    public void 옛_이름은_지금_이름과_겹치지_않는다()
    {
        Assert.Empty(Views.LegacyNames.Intersect(Views.Names));
        Assert.All(Views.LegacyNames, name => Assert.Matches(@"_v\d+$", name));
    }

    /// <summary>
    /// 엑셀 시트 이름. 받는 쪽이 시트 이름으로 표를 짚으므로 이것도 계약면이다 — 이름이
    /// 제자리에서 바뀐 뒤에도 머리 <c>v_</c> 만 떼어 이 차례로 나간다. 엑셀은 시트 이름을
    /// 31자까지만 받고 같은 이름 둘을 받지 않는다.
    /// </summary>
    [Fact]
    public void 엑셀_시트_이름은_머리만_뗀_뷰_이름이다()
    {
        var sheets = Views.Exported.Select(Views.SheetName).ToList();

        Assert.Equal(
            ["통합", "접수", "접수품목", "공고", "공고품목", "계약", "계약품목", "ERP원천", "계약업체", "ERP접수"],
            sheets);
        Assert.Equal(sheets.Count, sheets.Distinct().Count());
        Assert.All(sheets, sheet => Assert.InRange(sheet.Length, 1, 31));
    }

    private List<string> ViewNames()
    {
        using var connection = _database.OpenReadOnly();
        using var command = connection.CreateCommand();
        command.CommandText = "SELECT name FROM sqlite_master WHERE type = 'view';";

        using var reader = command.ExecuteReader();
        var names = new List<string>();
        while (reader.Read()) names.Add(reader.GetString(0));

        return names;
    }

    private List<string> ColumnsOf(string view)
    {
        using var connection = _database.OpenReadOnly();
        using var command = connection.CreateCommand();
        command.CommandText = $"PRAGMA table_info(\"{view}\");";

        using var reader = command.ExecuteReader();
        var columns = new List<string>();
        while (reader.Read()) columns.Add(reader.GetString(1));

        return columns;
    }

    private string Surface()
    {
        var text = new StringBuilder();
        using var connection = _database.OpenReadOnly();

        foreach (var view in Views.Names)
        {
            var columns = new List<string>();

            using (var command = connection.CreateCommand())
            {
                command.CommandText = $"PRAGMA table_info(\"{view}\");";
                using var reader = command.ExecuteReader();
                while (reader.Read()) columns.Add(reader.GetString(1));
            }

            text.AppendLine($"{view}  ({columns.Count}열)");
            foreach (var column in columns) text.AppendLine($"  {column}");
            text.AppendLine();
        }

        return text.ToString();
    }

    public void Dispose()
    {
        SqliteConnection.ClearAllPools();
        foreach (var suffix in new[] { "", "-wal", "-shm" })
            File.Delete(_path + suffix);
        GC.SuppressFinalize(this);
    }
}

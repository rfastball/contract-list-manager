using System.Text.Json;
using System.Text.Json.Nodes;
using Dapper;
using Microsoft.Data.Sqlite;
using Pclm.Core.Erp;
using Pclm.Core.Storage;
using Xunit;

namespace Pclm.Core.Tests;

/// <summary>
/// 판올림 V19 — <b>PDF 입구를 걷는다</b>(ADR-028).
///
/// <para>걷는 것은 가져오기 기록(<c>document</c>)과 PDF 가져오기에만 쓰이던 설정, 저장해 둔
/// 수집 매핑의 <c>pdf-*</c> 프로필이다. PDF 로 들어온 공고·계약은 진짜 키로 선 자료라
/// <b>링크·메모·덮개와 함께 남는다</b> — 걷는 김에 그것이 쓸려 가면 오류 없이 자료를 잃는다.</para>
/// </summary>
public sealed class SchemaV19Tests : IDisposable
{
    private readonly string _path = Path.Combine(Path.GetTempPath(), $"pclm-v19-{Guid.NewGuid():N}.db");

    public void Dispose()
    {
        SqliteConnection.ClearAllPools();
        foreach (var suffix in new[] { "", "-wal", "-shm" })
            File.Delete(_path + suffix);
    }

    /// <summary>v18 까지만 올린 자료를 짓는다. 뷰는 짓지 않는다 — 판올림이 어차피 다시 짓는다.</summary>
    private void SeedV18(string sql, object? parameters = null)
    {
        using var connection = new SqliteConnection($"Data Source={_path}");
        connection.Open();
        using var transaction = connection.BeginTransaction();
        for (var v = 0; v < 18; v++) connection.Execute(Schema.Migrations[v], transaction: transaction);
        connection.Execute("PRAGMA user_version = 18;", transaction: transaction);
        connection.Execute(sql, parameters, transaction);
        transaction.Commit();
    }

    private const string Seed = """
        INSERT INTO notice_group VALUES ('R26BK00000001');
        INSERT INTO notice_series VALUES ('R26BK00000001', 'R26BK00000001');
        INSERT INTO notice (notice_base, seq, title, updated_at) VALUES ('R26BK00000001', '000', 'PDF 공고', 't');
        INSERT INTO notice_user_field VALUES ('R26BK00000001', '메모', '공고 메모', 't');

        INSERT INTO contract_series VALUES ('R26TA00000001');
        INSERT INTO contract (contract_base, seq, title, amount, updated_at)
            VALUES ('R26TA00000001', '00', 'PDF 계약', '164872340', 't');
        INSERT INTO contract_user_field VALUES ('R26TA00000001', '진행상태', '납품', 't');
        INSERT INTO field_override VALUES ('contract', 'R26TA00000001', '00', '계약금액', '999', '164,872,340', 't');
        INSERT INTO project_link (contract_base, notice_group, confidence, confirmed_at, decided_by)
            VALUES ('R26TA00000001', 'R26BK00000001', 1, 't', 'human');

        -- 지난 PDF 비교가 남긴 관찰 기록. 고치지 않는다.
        INSERT INTO erp_source VALUES ('notice', 'R26BK00000001', '000',
            '{"profile":"pdf-notice-v1","entityType":"notice","base":"R26BK00000001","seq":"000","rows":[],"scope":"file"}', 'rev', 't');

        INSERT INTO document VALUES ('sha-n', '물품입찰공고서.pdf', 'C:\내려받기\물품입찰공고서.pdf', 'notice', 'R26BK00000001-000', 'ok', NULL, 't');
        INSERT INTO document VALUES ('sha-x', '깨진것.pdf', NULL, 'unknown', NULL, 'failed', '못 읽음', 't');

        INSERT INTO app_setting VALUES ('source_folder', 'C:\내려받기'), ('import_keywords', '공고서|계약서'),
            ('overwrite_on_reingest', '1'), ('submit.name', '홍길동'), ('plan.path', 'C:\계획.xlsx');

        INSERT INTO erp_mapping (revision, json, active, validated, samples, created_at)
            VALUES ('rev-old', @mapping, 1, 1, '[]', 't');
        """;

    /// <summary>옛 판처럼 <c>pdf-*</c> 프로필을 함께 실은 매핑. 기본 매핑에 셋을 덧붙인다.</summary>
    private static string OldMapping()
    {
        var json = JsonNode.Parse(JsonSerializer.Serialize(Mapping.Defaults(), Mapping.Json))!.AsObject();
        var profiles = json["profiles"]!.AsArray();
        var template = profiles.First(p => p!["id"]!.GetValue<string>() == "g2b-public-notice-header-v1")!;

        foreach (var (id, kind) in new[] { ("pdf-request-v1", "request"), ("pdf-notice-v1", "notice"), ("pdf-contract-v1", "contract") })
        {
            var copy = template.DeepClone().AsObject();
            copy["id"] = id;
            copy["entityType"] = kind;
            profiles.Add(copy);
        }

        return json.ToJsonString();
    }

    [Fact]
    public void 가져오기_기록과_PDF_설정을_걷고_자료는_남긴다()
    {
        SeedV18(Seed, new { mapping = OldMapping() });

        new Database(_path).Migrate();

        using var c = new Database(_path).Open();

        Assert.Equal(Schema.Version, c.ExecuteScalar<int>("PRAGMA user_version;"));
        Assert.Empty(c.Query("PRAGMA foreign_key_check;"));

        // ── 걷힌 것 ──
        Assert.Equal(0, c.ExecuteScalar<int>("SELECT count(*) FROM sqlite_master WHERE name = 'document';"));
        Assert.Equal(0, c.ExecuteScalar<int>("SELECT count(*) FROM sqlite_master WHERE name = 'ix_document_entity';"));
        Assert.Equal(
            ["plan.path", "submit.name"],
            c.Query<string>("SELECT key FROM app_setting ORDER BY key;"));

        // ── 남은 자료 — 링크·메모·덮개까지 ──
        Assert.Equal("PDF 공고", c.QuerySingle<string>("SELECT 공고명 FROM v_공고_v1 WHERE 입찰공고번호 = 'R26BK00000001-000';"));
        Assert.Equal("999", c.QuerySingle<string>("SELECT 계약금액 FROM v_계약_v1 WHERE 계약번호 = 'R26TA0000000100';"));
        Assert.Equal("공고 메모", c.QuerySingle<string>("SELECT value FROM notice_user_field;"));
        Assert.Equal("납품", c.QuerySingle<string>("SELECT value FROM contract_user_field;"));
        Assert.Equal("R26BK00000001", c.QuerySingle<string>("SELECT notice_group FROM project_link WHERE contract_base = 'R26TA00000001';"));
        Assert.Equal(1, c.ExecuteScalar<int>("SELECT count(*) FROM field_override;"));

        // ── 지난 관찰 기록은 그대로 ──
        Assert.Contains("pdf-notice-v1", c.QuerySingle<string>("SELECT document_json FROM erp_source;"));

        // ── 저장해 둔 매핑은 pdf-* 를 벗고 다시 읽힌다 ──
        Assert.Equal(
            Mapping.Defaults().Profiles.Select(p => p.Id),
            new MappingStore(new Database(_path)).Active().Profiles.Select(p => p.Id));
    }
}

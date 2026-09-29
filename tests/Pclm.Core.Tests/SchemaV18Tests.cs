using Dapper;
using Microsoft.Data.Sqlite;
using Pclm.Core.Storage;
using Xunit;

namespace Pclm.Core.Tests;

/// <summary>
/// 판올림 V18 — <b>접수의 키를 접수번호로</b>(ADR-027).
///
/// <para>v17 자료에는 접수가 세 갈래로 서 있다. 확장이 수집한 <c>ERP:X</c>, 별칭으로 ERP 접수에
/// 이어진 PDF 접수, 그리고 PDF 에서만 온 접수. 앞의 둘은 접수번호로 <b>옮기되 딸린 것을 잃지
/// 않고</b>, 마지막 것은 옮길 자리가 없어 <b>지운다</b>(그 접수서의 document 까지).</para>
///
/// <para>판올림은 끝까지 오르므로 <c>document</c> 는 V19 가 표째 걷는다(ADR-028) — 여기서는
/// V18 이 그 표를 딛고도 돈다는 것까지만 본다. 걷힌 것은 <see cref="SchemaV19Tests"/> 가 본다.</para>
/// </summary>
public sealed class SchemaV18Tests : IDisposable
{
    private readonly string _path = Path.Combine(Path.GetTempPath(), $"pclm-v18-{Guid.NewGuid():N}.db");

    public void Dispose()
    {
        SqliteConnection.ClearAllPools();
        foreach (var suffix in new[] { "", "-wal", "-shm" })
            File.Delete(_path + suffix);
    }

    /// <summary>v17 까지만 올린 자료를 짓는다. 뷰는 짓지 않는다 — 판올림이 어차피 다시 짓는다.</summary>
    private void SeedV17(string sql)
    {
        using var connection = new SqliteConnection($"Data Source={_path}");
        connection.Open();
        using var transaction = connection.BeginTransaction();
        for (var v = 0; v < 17; v++) connection.Execute(Schema.Migrations[v], transaction: transaction);
        connection.Execute("PRAGMA user_version = 17;", transaction: transaction);
        connection.Execute(sql, transaction: transaction);
        transaction.Commit();
    }

    private const string Seed = """
        INSERT INTO notice_group VALUES ('R26BK00000001'), ('R26BK00000002');
        INSERT INTO notice_series VALUES ('R26BK00000001', 'R26BK00000001'), ('R26BK00000002', 'R26BK00000002');
        INSERT INTO notice (notice_base, seq, title, updated_at) VALUES
            ('R26BK00000001', '000', '수집 공고', 't'), ('R26BK00000002', '000', 'PDF 공고', 't');

        -- 확장이 수집한 접수. 키에 ERP: 가 붙어 있다.
        INSERT INTO request_series VALUES ('ERP:RC001');
        INSERT INTO request (request_base, seq, title, updated_at) VALUES ('ERP:RC001', '000', '수집 접수', 't');
        INSERT INTO request_item (request_base, seq, line_no, request_number, ref_request_base)
            VALUES ('ERP:RC001', '000', 1, 'REQ1', 'RC001');
        INSERT INTO request_user_field VALUES ('ERP:RC001', '메모', '확인함', 't');
        INSERT INTO field_override VALUES ('request', 'ERP:RC001', '000', '요청명', '고친 접수', '수집 접수', 't');
        INSERT INTO request_link (request_base, notice_group, confidence, confirmed_at, decided_by)
            VALUES ('ERP:RC001', 'R26BK00000001', 1, 't', 'explicit');
        INSERT INTO notice_item (notice_base, seq, line_no, item_name) VALUES ('R26BK00000001', '000', 1, '품목');
        INSERT INTO request_item_link VALUES ('ERP:RC001', '000', 1, 'R26BK00000001', '000', 1);
        INSERT INTO erp_row VALUES ('request', 'ERP:RC001', '000', 'request_item', '["RC001","000","001"]', 1);
        INSERT INTO erp_source VALUES ('request', 'ERP:RC001', '000',
            '{"profile":"g2b-request-v1","entityType":"request","base":"ERP:RC001","seq":"000","rows":[],"scope":"live"}', 'rev', 't');
        INSERT INTO erp_capture VALUES ('c1', 'h', 'h', 'g2b-request-v1', 'ERP:RC001', '000', '{}', '{"entity":"ERP:RC001-000"}', 't');

        -- 별칭으로 ERP 접수 RC002 에 이어진 PDF 접수.
        INSERT INTO request_series VALUES ('MPKPLA26910286');
        INSERT INTO request (request_base, seq, title, updated_at) VALUES ('MPKPLA26910286', '000', '이어진 접수', 't');
        INSERT INTO request_item (request_base, seq, line_no, request_number) VALUES ('MPKPLA26910286', '000', 1, 'MPKPLA26910286');
        INSERT INTO request_user_field VALUES ('MPKPLA26910286', '메모', '별칭 메모', 't');
        INSERT INTO field_override VALUES ('request', 'MPKPLA26910286', '000', '요청명', '고친 이름', '이어진 접수', 't');
        INSERT INTO erp_request_alias VALUES ('ERP:RC002', 'MPKPLA26910286', 't');
        INSERT INTO erp_source VALUES ('request', 'MPKPLA26910286', '000',
            '{"profile":"g2b-request-v1","entityType":"request","base":"MPKPLA26910286","seq":"000","rows":[],"scope":"live"}', 'rev', 't');
        INSERT INTO document VALUES ('sha-alias', '접수서1.pdf', NULL, 'request', 'MPKPLA26910286-000', 'ok', NULL, 't');

        -- PDF 에서만 온 접수. 접수번호를 모르니 옮길 자리가 없다.
        INSERT INTO request_series VALUES ('MPKPLA26910300');
        INSERT INTO request (request_base, seq, title, updated_at) VALUES ('MPKPLA26910300', '000', 'PDF 접수', 't');
        INSERT INTO request_item (request_base, seq, line_no, request_number) VALUES ('MPKPLA26910300', '000', 1, 'MPKPLA26910300');
        INSERT INTO request_user_field VALUES ('MPKPLA26910300', '메모', '사라질 메모', 't');
        INSERT INTO field_override VALUES ('request', 'MPKPLA26910300', '000', '요청명', '사라질 정정', 'PDF 접수', 't');
        INSERT INTO request_link (request_base, notice_group, confidence, confirmed_at, decided_by)
            VALUES ('MPKPLA26910300', 'R26BK00000002', 1, 't', 'human');
        INSERT INTO request_link_rejection VALUES ('MPKPLA26910300', 'R26BK00000001', 't');
        INSERT INTO document VALUES ('sha-pdf', '접수서2.pdf', NULL, 'request', 'MPKPLA26910300-000', 'ok', NULL, 't');

        -- 다른 종류의 기록은 건드리지 않는다.
        INSERT INTO document VALUES ('sha-notice', '공고서.pdf', NULL, 'notice', 'R26BK00000002-000', 'ok', NULL, 't');
        INSERT INTO document VALUES ('sha-failed', '깨진것.pdf', NULL, 'request', NULL, 'failed', '못 읽음', 't');
        """;

    [Fact]
    public void 접수번호로_옮기고_PDF에서만_온_접수는_지운다()
    {
        SeedV17(Seed);

        new Database(_path).Migrate();

        using var c = new Database(_path).Open();

        Assert.Equal(Schema.Version, c.ExecuteScalar<int>("PRAGMA user_version;"));
        Assert.Empty(c.Query("PRAGMA foreign_key_check;"));
        Assert.Equal(0, c.ExecuteScalar<int>("SELECT count(*) FROM sqlite_master WHERE name = 'erp_request_alias';"));

        // ── 남은 접수는 둘이고, 키는 접수번호다 ──
        Assert.Equal(["RC001", "RC002"], c.Query<string>("SELECT request_base FROM request_series ORDER BY 1;"));
        Assert.Equal(["RC001", "RC002"], c.Query<string>("SELECT request_base FROM request ORDER BY 1;"));
        Assert.Equal(["RC001-000", "RC002-000"], c.Query<string>("SELECT 접수번호 FROM v_접수_v1 ORDER BY 1;"));

        // ── ERP:X → X. 딸린 것이 함께 온다 ──
        Assert.Equal("확인함", c.QuerySingle<string>("SELECT value FROM request_user_field WHERE request_base = 'RC001';"));
        Assert.Equal("고친 접수", c.QuerySingle<string>("SELECT 요청명 FROM v_접수_v1 WHERE 접수번호 = 'RC001-000';"));
        Assert.Equal("R26BK00000001", c.QuerySingle<string>("SELECT notice_group FROM request_link WHERE request_base = 'RC001';"));
        Assert.Equal(1, c.ExecuteScalar<int>("SELECT count(*) FROM request_item_link WHERE request_base = 'RC001';"));
        Assert.Equal(1, c.ExecuteScalar<int>("SELECT count(*) FROM request_item WHERE request_base = 'RC001';"));
        Assert.Equal(1, c.ExecuteScalar<int>("SELECT count(*) FROM erp_row WHERE entity_base = 'RC001';"));
        Assert.Equal("RC001", c.QuerySingle<string>(
            "SELECT json_extract(document_json, '$.base') FROM erp_source WHERE entity_base = 'RC001';"));
        Assert.Equal("RC001", c.QuerySingle<string>("SELECT entity_base FROM erp_capture WHERE capture_id = 'c1';"));

        // ── 별칭으로 이어진 PDF 접수 → ERP 접수번호 ──
        Assert.Equal("별칭 메모", c.QuerySingle<string>("SELECT value FROM request_user_field WHERE request_base = 'RC002';"));
        Assert.Equal("고친 이름", c.QuerySingle<string>("SELECT 요청명 FROM v_접수_v1 WHERE 접수번호 = 'RC002-000';"));
        Assert.Equal("RC002", c.QuerySingle<string>(
            "SELECT json_extract(document_json, '$.base') FROM erp_source WHERE entity_base = 'RC002';"));

        // ── PDF 에서만 온 접수는 흔적 없이 ──
        foreach (var sql in new[]
                 {
                     "SELECT count(*) FROM request_user_field WHERE request_base LIKE 'MPKPLA%'",
                     "SELECT count(*) FROM request_item WHERE request_base LIKE 'MPKPLA%'",
                     "SELECT count(*) FROM request_link WHERE request_base LIKE 'MPKPLA%'",
                     "SELECT count(*) FROM request_link_rejection",
                     "SELECT count(*) FROM field_override WHERE base LIKE 'MPKPLA%'",
                     "SELECT count(*) FROM erp_source WHERE entity_base LIKE 'MPKPLA%'",
                     "SELECT count(*) FROM request WHERE request_base LIKE 'ERP:%'",
                     "SELECT count(*) FROM field_override WHERE base LIKE 'ERP:%'",
                 })
            Assert.True(c.ExecuteScalar<int>(sql) == 0, sql);
    }
}

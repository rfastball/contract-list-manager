using Dapper;
using Microsoft.Data.Sqlite;
using Pclm.Core.Storage;

namespace Pclm.Core.Erp;

public sealed record ReferenceState(string EntityType, string Base, string Target, string State);
public static class ExplicitLinks
{
    public static List<ReferenceState> Resolve(SqliteConnection c, SqliteTransaction tx)
    {
        NoticeGroups.Rebuild(c, tx);
        var result = new List<ReferenceState>();
        foreach (var n in c.Query<(string Notice, string Request)>("""
            SELECT notice_base, min(ref_request_base) FROM notice_item
            WHERE coalesce(ref_request_base,'')<>'' GROUP BY notice_base
            HAVING count(DISTINCT ref_request_base)=1
            """, transaction: tx))
        {
            // 공고 품목이 적은 접수번호가 곧 접수의 키다(ADR-027).
            var group = NoticeGroups.GroupOf(c, n.Notice, tx)!;
            result.Add(new("request", n.Request, n.Notice, Connect("request", n.Request, group)));
        }
        foreach (var k in c.Query<(string Base, string Notice, string Request)>("""
            SELECT contract_base, coalesce(ref_notice_base,''), coalesce(ref_request_base,'') FROM contract k
            WHERE CAST(seq AS INTEGER)=(SELECT max(CAST(x.seq AS INTEGER)) FROM contract x WHERE x.contract_base=k.contract_base)
            """, transaction: tx))
        {
            var group = string.IsNullOrEmpty(k.Notice) ? null : NoticeGroups.GroupOf(c, k.Notice, tx);
            if (group is null && string.IsNullOrEmpty(k.Notice) && !string.IsNullOrEmpty(k.Request))
            {
                var groups = c.Query<string>("SELECT DISTINCT s.group_base FROM notice_item i JOIN notice_series s USING(notice_base) WHERE i.ref_request_base=@Request", new { k.Request }, tx).ToList();
                if (groups.Count == 1) group = groups[0];
            }
            result.Add(new("contract", k.Base, k.Notice, group is null
                ? string.IsNullOrEmpty(k.Notice) && string.IsNullOrEmpty(k.Request) ? "참조 없음" : "미수집"
                : Connect("contract", k.Base, group)));
        }
        // 정확한 접수 품목 복합키가 있는 공고 행만 잇는다. 계약 품목의 다른 순번은 사용하지 않는다.
        foreach (var r in c.Query<(string Base, string Seq, int Line, string Notice, string NoticeSeq, int NoticeLine)>("""
            SELECT r.request_base,r.seq,r.line_no,n.notice_base,n.seq,n.line_no
            FROM request_item r JOIN notice_item n ON r.ref_request_base=n.ref_request_base
              AND r.ref_request_seq=n.ref_request_seq AND r.ref_request_item=n.ref_request_item
            JOIN notice_series s ON s.notice_base=n.notice_base
            JOIN request_link l ON l.request_base=r.request_base AND l.notice_group=s.group_base
            WHERE r.ref_request_base<>'' AND r.ref_request_seq<>'' AND r.ref_request_item<>''
            """, transaction: tx).GroupBy(r => (r.Base, r.Seq, r.Line)).Where(g => g.Count() == 1).Select(g => g.Single()))
            c.Execute("INSERT INTO request_item_link(request_base,request_seq,line_no,notice_base,notice_seq,notice_line_no) VALUES(@Base,@Seq,@Line,@Notice,@NoticeSeq,@NoticeLine) ON CONFLICT DO NOTHING", new { r.Base, r.Seq, r.Line, r.Notice, r.NoticeSeq, r.NoticeLine }, tx);
        return result;

        string Connect(string kind, string b, string group)
        {
            var table = kind == "request" ? "request_link" : "project_link";
            var rejection = kind == "request" ? "request_link_rejection" : "link_rejection";
            if (c.ExecuteScalar<int>($"SELECT count(*) FROM {kind}_series WHERE {kind}_base=@b", new { b }, tx) == 0) return "미수집";
            var existing = c.ExecuteScalar<string>($"SELECT notice_group FROM {table} WHERE {kind}_base=@b", new { b }, tx);
            if (existing is not null) return existing == group ? "연결됨" : "연결 확인 필요";
            if (c.ExecuteScalar<int>($"SELECT count(*) FROM {rejection} WHERE {kind}_base=@b AND notice_group=@group", new { b, group }, tx) > 0 ||
                kind == "request" && c.ExecuteScalar<int>("SELECT count(*) FROM request_link WHERE notice_group=@group", new { group }, tx) > 0)
                return "연결 확인 필요";
            c.Execute($"INSERT INTO {table}({kind}_base,notice_group,confidence,confirmed_at,decided_by,evidence) VALUES(@b,@group,1,@now,'explicit','ERP 명시 참조')",
                new { b, group, now = DateTime.UtcNow.ToString("O") }, tx);
            return "연결됨";
        }
    }
    public static int Resolve(Database database)
    {
        using var c = database.Open(); using var tx = c.BeginTransaction();
        var before = c.ExecuteScalar<int>("SELECT (SELECT count(*) FROM project_link)+(SELECT count(*) FROM request_link)", transaction: tx);
        Resolve(c, tx);
        var after = c.ExecuteScalar<int>("SELECT (SELECT count(*) FROM project_link)+(SELECT count(*) FROM request_link)", transaction: tx);
        tx.Commit(); return after - before;
    }
}

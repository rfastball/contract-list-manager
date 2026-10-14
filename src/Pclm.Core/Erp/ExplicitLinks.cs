using Dapper;
using Microsoft.Data.Sqlite;
using Pclm.Core.Storage;

namespace Pclm.Core.Erp;

public sealed record ReferenceState(string EntityType, string Base, string Target, string State);

/// <summary>연결은 수집된 명시 참조의 파생값이다. 재수집·삭제·취합마다 같은 입력에서 다시 짓는다.</summary>
public static class ExplicitLinks
{
    public static List<ReferenceState> Resolve(SqliteConnection c, SqliteTransaction? tx)
    {
        NoticeGroups.Rebuild(c, tx);
        var result = new List<ReferenceState>();
        var references = c.Query<(string Group, string Request)>("""
            SELECT DISTINCT s.group_base, i.ref_request_base
            FROM notice_item i JOIN notice_series s USING(notice_base)
            WHERE coalesce(i.ref_request_base,'')<>''
              AND CAST(i.seq AS INTEGER)=(SELECT max(CAST(n.seq AS INTEGER)) FROM notice n WHERE n.notice_base=i.notice_base)
            """, transaction: tx).ToList();
        foreach (var (group, request) in references)
        {
            var conflict = references.Count(r => r.Group == group) != 1 || references.Count(r => r.Request == request) != 1;
            var collected = c.ExecuteScalar<int>("SELECT count(*) FROM request_series WHERE request_base=@request", new { request }, tx) > 0;
            var state = conflict ? "참조 충돌" : !collected ? "미수집" : "연결됨";
            if (state == "연결됨")
                c.Execute("INSERT INTO request_link(request_base,notice_group) VALUES(@request,@group)", new { request, group }, tx);
            result.Add(new("request", request, group, state));
        }
        foreach (var k in c.Query<(string Base, string Notice, string Request)>("""
            SELECT contract_base, coalesce(ref_notice_base,''), coalesce(ref_request_base,'') FROM contract k
            WHERE CAST(seq AS INTEGER)=(SELECT max(CAST(x.seq AS INTEGER)) FROM contract x WHERE x.contract_base=k.contract_base)
            """, transaction: tx))
        {
            var groups = k.Notice.Length > 0
                ? c.Query<string>("""
                    SELECT group_base FROM notice_series WHERE notice_base=@Notice
                    UNION
                    SELECT s.group_base FROM notice_relation r JOIN notice_series s USING(notice_base) WHERE r.related_base=@Notice
                    """, new { k.Notice }, tx).ToArray()
                : references.Where(r => r.Request == k.Request).Select(r => r.Group).Distinct().ToArray();
            var inconsistent = k.Request.Length > 0 && groups.Length == 1
                && references.Any(r => r.Group == groups[0] && r.Request != k.Request);
            var state = groups.Length > 1 || inconsistent ? "참조 충돌" : groups.Length == 1 ? "연결됨"
                : k.Notice.Length == 0 && k.Request.Length == 0 ? "참조 없음" : "미수집";
            if (state == "연결됨")
                c.Execute("INSERT INTO project_link(contract_base,notice_group) VALUES(@Base,@group)", new { k.Base, group = groups[0] }, tx);
            result.Add(new("contract", k.Base, k.Notice.Length > 0 ? k.Notice : k.Request, state));
        }
        // 품목은 정확한 접수번호·차수·품목순번이 일치하고 대상이 유일할 때만 연결한다.
        foreach (var r in c.Query<(string Base, string Seq, int Line, string Notice, string NoticeSeq, int NoticeLine)>($"""
            SELECT r.request_base,r.seq,r.line_no,n.notice_base,n.seq,n.line_no
            FROM request_item r JOIN notice_item n ON r.ref_request_base=n.ref_request_base
              AND r.ref_request_seq=n.ref_request_seq AND r.ref_request_item=n.ref_request_item
            JOIN ({NoticeGroups.현행공고}) current ON current.notice_base=n.notice_base AND current.seq=n.seq
            JOIN request_link l ON l.request_base=r.request_base AND l.notice_group=current.group_base
            WHERE r.ref_request_base<>'' AND r.ref_request_seq<>'' AND r.ref_request_item<>''
            """, transaction: tx).GroupBy(r => (r.Base, r.Seq, r.Line)).Where(g => g.Count() == 1).Select(g => g.Single()))
            c.Execute("INSERT INTO request_item_link(request_base,request_seq,line_no,notice_base,notice_seq,notice_line_no) VALUES(@Base,@Seq,@Line,@Notice,@NoticeSeq,@NoticeLine)", new { r.Base, r.Seq, r.Line, r.Notice, r.NoticeSeq, r.NoticeLine }, tx);
        return result;
    }

    public static int Resolve(Database database)
    {
        using var c = database.Open();
        using var tx = c.BeginTransaction();
        var count = Resolve(c, tx).Count(r => r.State == "연결됨");
        tx.Commit();
        return count;
    }
}

using Dapper;
using Pclm.Core.Erp;
using Pclm.Core.Storage;

namespace Pclm.Core.Tests;

/// <summary>뷰·삭제 시험의 문서에 명시 참조를 넣는다. 관계 테이블에는 직접 쓰지 않는다.</summary>
internal static class ReferenceFixture
{
    internal static void Contract(Database database, EntityRef contract, EntityRef notice)
    {
        using var c = database.Open();
        using var tx = c.BeginTransaction();
        c.Execute("UPDATE contract SET ref_notice_base=@notice WHERE contract_base=@contract",
            new { notice = notice.Base, contract = contract.Base }, tx);
        ExplicitLinks.Resolve(c, tx);
        tx.Commit();
    }

    internal static void Request(Database database, EntityRef request, EntityRef notice)
    {
        using var c = database.Open();
        using var tx = c.BeginTransaction();
        c.Execute("""
            UPDATE request_item SET ref_request_base=request_base, ref_request_seq=seq,
                ref_request_item=CAST(line_no AS TEXT) WHERE request_base=@request;
            UPDATE notice_item SET ref_request_base=@request, ref_request_seq=@seq,
                ref_request_item=CAST(line_no AS TEXT) WHERE notice_base=@notice;
            INSERT INTO notice_item(notice_base,seq,line_no,ref_request_base)
                SELECT notice_base,seq,1,@request FROM notice n WHERE notice_base=@notice
                AND NOT EXISTS(SELECT 1 FROM notice_item i WHERE i.notice_base=n.notice_base AND i.seq=n.seq);
            """, new { request = request.Base, seq = request.Seq, notice = notice.Base }, tx);
        ExplicitLinks.Resolve(c, tx);
        tx.Commit();
    }
}

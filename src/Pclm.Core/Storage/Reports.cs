using Dapper;

namespace Pclm.Core.Storage;

public sealed record StatusSummary(
    int NoticeRows,
    int NoticeBases,
    int ContractRows,
    int ContractBases,
    int Counterparties,
    int UnlinkedContracts,
    int RequestRows,
    int RequestBases,
    int UnlinkedRequests);

/// <summary><c>Revisions</c> 는 SQLite <c>COUNT(*)</c> 가 64비트라 <c>long</c> 이다.</summary>
public sealed record RevisionGroup(string Base, string Kind, long Revisions, string Latest, string? Title);

/// <summary>축적 상태를 훑어보는 조회들. 쓰기는 하지 않는다.</summary>
public sealed class Reports(Database database)
{
    private readonly Database _database = database;

    public StatusSummary Summary()
    {
        using var connection = _database.Open();

        return new StatusSummary(
            connection.ExecuteScalar<int>("SELECT COUNT(*) FROM notice;"),
            connection.ExecuteScalar<int>("SELECT COUNT(DISTINCT notice_base) FROM notice;"),
            connection.ExecuteScalar<int>("SELECT COUNT(*) FROM contract;"),
            connection.ExecuteScalar<int>("SELECT COUNT(DISTINCT contract_base) FROM contract;"),
            connection.ExecuteScalar<int>("SELECT COUNT(*) FROM counterparty;"),
            connection.ExecuteScalar<int>(
                """
                SELECT COUNT(DISTINCT c.contract_base) FROM contract c
                WHERE NOT EXISTS (
                    SELECT 1 FROM project_link l WHERE l.contract_base = c.contract_base
                );
                """),
            connection.ExecuteScalar<int>("SELECT COUNT(*) FROM request;"),
            connection.ExecuteScalar<int>("SELECT COUNT(DISTINCT request_base) FROM request;"),
            connection.ExecuteScalar<int>(
                """
                SELECT COUNT(DISTINCT r.request_base) FROM request r
                WHERE NOT EXISTS (
                    SELECT 1 FROM request_link l WHERE l.request_base = r.request_base
                );
                """));
    }

    /// <summary>
    /// 차수가 둘 이상 쌓인 건. 변경공고·변경계약이 제대로 갈라졌는지 보는 창이다.
    ///
    /// <para><c>COUNT</c>·<c>MAX</c> 로 만든 열을 레코드로 바로 받지 않는다. <b>결과가 한 행도
    /// 없으면 SQLite 가 그 열의 타입을 알려줄 수 없어</b>(식에는 선언 타입이 없다) 매핑기가
    /// 전부 <c>byte[]</c> 로 보고 생성자를 못 찾는다. 자료를 아직 한 건도 넣지 않은 DB에서
    /// <c>status</c> 가 통째로 터졌다 — 갓 설치한 사람이 처음 치는 명령이 그것이다.
    /// 값만 받아 오고 조립은 여기서 한다.</para>
    /// </summary>
    public IReadOnlyList<RevisionGroup> Revisions()
    {
        using var connection = _database.Open();

        // 공고·계약은 표 이름만 다른 같은 물음이다. 종류 이름은 SQL 이 아니라 여기서 붙인다.
        //
        // 제목은 <b>최신 차수의 것</b>이라야 한다. 사전순 최대(MAX)로 고르면 「최신 001」 옆에
        // 000 의 제목이 붙는다 — 변경 뒤에 다시 내려받은 당초 공고는 공고명 끝에
        // 「(최종 공고가 아닙니다.)」 가 붙어 있어, 하필 그것이 사전순으로 크다.
        IEnumerable<RevisionGroup> Groups(string table, string key, string kind) =>
            connection.Query(
                $"""
                SELECT t.{key} AS Base, COUNT(*) AS Revisions,
                       (SELECT x.seq FROM {table} x WHERE x.{key} = t.{key}
                        ORDER BY CAST(x.seq AS INTEGER) DESC LIMIT 1) AS Latest,
                       (SELECT x.title FROM {table} x WHERE x.{key} = t.{key}
                        ORDER BY CAST(x.seq AS INTEGER) DESC LIMIT 1) AS Title
                FROM {table} t GROUP BY t.{key};
                """)
                .Select(row => new RevisionGroup(
                    (string)row.Base, kind, (long)row.Revisions, (string)row.Latest, (string?)row.Title));

        return [.. Groups("request", "request_base", "접수")
            .Concat(Groups("notice", "notice_base", "공고"))
            .Concat(Groups("contract", "contract_base", "계약"))
            .OrderByDescending(g => g.Revisions)
            .ThenBy(g => g.Base, StringComparer.Ordinal)];
    }
}

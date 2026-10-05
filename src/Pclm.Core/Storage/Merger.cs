using System.Globalization;
using Microsoft.Data.Sqlite;

namespace Pclm.Core.Storage;

/// <summary>겹친 자리의 한쪽. 누구의 제출본에서 왔고 그 값이 언제 적힌 것인가.</summary>
/// <param name="시각">표에 적힌 그대로의 문자열. 시각 열이 없는 표에서는 빈 문자열이다.</param>
public sealed record MergeSide(string 제출자, string 시각);

/// <summary>
/// 같은 키가 둘 이상의 제출본에 있었다는 <b>사실</b>.
///
/// <para><b>기계는 고를 뿐 판정하지 않는다</b>(ADR-016 의 결). 무엇이 옳은지는 두 사람이 안다.
/// 그래서 취합은 하나를 싣되 <b>무엇을 밀어냈는지 반드시 알린다</b> — 세기만 하고 어느 것인지
/// 내지 못하면 밀려난 쪽은 오류 없이 사라진다.</para>
/// </summary>
/// <param name="종류">어느 표에서 겹쳤나 — 사람이 읽을 이름(<c>접수</c>·<c>덮개</c>…).</param>
/// <param name="키">그 표의 자연키. 열이 여럿이면 <c>·</c> 로 잇는다.</param>
public sealed record MergeConflict(string 종류, string 키, MergeSide 실림, IReadOnlyList<MergeSide> 밀림);

/// <summary>취합 한 번의 결과. 수는 <b>다 지은 취합본에서</b> 세어 <c>pclm 현황</c> 과 맞춘다.</summary>
/// <param name="제출본">실제로 합친 제출본 수. 거절한 것은 여기 들지 않는다.</param>
/// <param name="거절한제출본">받지 않은 파일과 그 까닭 — PCLM 이 아니거나, 판이 맞지 않거나, 제출본이
/// 아닌 역할(취합본·백업·옮겨진 옛 자료)이다.</param>
public sealed record MergeReport(
    int 제출본, int 조달요구, int 접수, int 공고, int 계약,
    IReadOnlyList<MergeRejection> 거절한제출본,
    IReadOnlyList<MergeConflict> 겹친것);

/// <summary>
/// 취합에 넣지 않은 파일 하나. <b>까닭을 함께 든다</b> — 이름만 적으면 받는 사람은 프로그램을 새로
/// 받아야 하는지, 파일을 잘못 골랐는지 가리지 못한다.
/// </summary>
/// <param name="파일">파일 이름(경로 없이).</param>
/// <param name="까닭">사람이 읽을 짧은 까닭 — <see cref="Merger.옛시험판"/>·<see cref="Merger.새판"/> 따위.</param>
public sealed record MergeRejection(string 파일, string 까닭);

/// <summary>
/// 제출본을 모아 취합본 하나로 짓는다.
///
/// <para><b>왜 공유 DB 하나가 아닌가.</b> 담당자마다 자기 DB 가 하나씩 있는데 그것들을 모을 길이
/// 없었다. 그렇다고 네트워크 드라이브에 DB 하나를 두고 여럿이 붙을 수는 없다 — WAL 은
/// 네트워크 파일시스템에서 돌지 않고(ADR-006), 계약면이 <b>쓰는 주체는 하나</b>라고 약속했다.
/// 웹ERP 에 닿을 수 없어 시작된 저장소라, 취합도 파일로 한다.</para>
///
/// <para><b>왜 매번 처음부터 짓는가.</b> 이 저장소는 <b>묘비를 두지 않는다</b>
/// (<see cref="Store.Delete"/>). 누가 자기 DB 에서 지운 건은 다음 취합본에 그냥 없어야 하는데,
/// 지난 취합본을 고쳐 쓰면 지운 것이 영영 남는다. 그래서 취합본은 결과물이지 자료가 아니다 —
/// 언제든 버리고 다시 지어도 같은 것이 나온다.</para>
///
/// <para><b>제출본은 남의 파일이라 손대지 않는다.</b> 읽기 전용 연결의 <c>VACUUM INTO</c> 로 임시 폴더에
/// 한 벌 떠서 열고, 끝나면 지운다. 옛 판 제출본은 그 사본 위에서 지금 판으로 올려 받는다(마이그레이션은
/// 앞으로만 간다). 판이 지금보다 새로운 것은 내릴 길이 없고, 기준선보다 옛 시험판은 올릴 단계가 이
/// 프로그램에 없어 <b>거절하되 멈추지는 않는다</b> — 나머지는 합친다.</para>
///
/// <para><b>무엇을 받을지는 파일 이름이 아니라 역할이 정한다</b>(<see cref="PclmFile.Inspect"/>).
/// 제출본과 작업자료(사람이 자기 파일을 그대로 보내는 일이 있다)만 받는다. 취합본·백업·옮겨진 옛 자료는
/// 이름이 무엇이든 거절한다 — 한동안 <c>취합_</c> 접두사로 가렸는데, 이름을 바꾼 취합본은 그대로 먹혔고
/// 그 글자로 시작하는 제출본은 말없이 빠졌다.</para>
/// </summary>
public sealed class Merger
{
    /// <summary>겹쳤을 때 무엇을 싣는가.</summary>
    private enum 규칙
    {
        /// <summary>시각이 늦은 쪽. 같으면 먼저 온 제출본. 둘 다 <b>겹친 것으로 알린다</b>.</summary>
        늦은쪽,

        /// <summary>합집합. 같은 키는 같은 것이라 고를 일이 없다(계열 키).</summary>
        합집합,

        /// <summary>
        /// 부모를 따라 통째로. 딸린 줄에는 시각이 없어 낱개로 고를 수 없고, 반씩 섞으면
        /// <b>어느 문서에도 없던 표</b>가 생긴다.
        /// </summary>
        부모따라,
        엔티티부모따라,

        /// <summary>
        /// 합집합이되 <b>뜻이 다르면 알린다</b>. 사람마다 세운 열이 달라 이름이 같으면 하나로,
        /// 다르면 둘 다 세운다. 이름이 같은데 <c>kind</c>·<c>options</c> 가 다르면 먼저 온 것을
        /// 두고 겹친 것으로 알린다 — 누가 옳은지 기계가 판정하지 않는다.
        /// </summary>
        정의,
    }

    /// <summary>
    /// 취합이 <b>나르는</b> 표. 시험이 이것과 실제 스키마를 견줘, 표가 하나 생겼는데 여기를
    /// 고치지 않은 것을 잡는다 — 빠뜨리면 그 표가 취합본에서 <b>조용히</b> 사라진다.
    /// </summary>
    public static IReadOnlyList<string> Tables => [.. 표들.Select(t => t.이름)];

    /// <summary>
    /// 취합이 <b>일부러</b> 두고 오는 표. 늘리려면 까닭이 있어야 한다.
    ///
    /// <para><c>notice_group</c> 은 <b>파생 표</b>라 나르지 않는다. 제출본마다 가진 문서가 달라
    /// 같은 건이 제각각 이름을 달고 오는데, 그것은 참고값일 뿐 취합본의 건이 아니다 —
    /// 값은 취합 끝의 <see cref="NoticeGroups.Rebuild"/> 가 짓는다.</para>
    /// </summary>
    // 수집 영수증의 datasetId·충돌 토큰은 원래 DB에서만 유효하다.
    // 취합본은 새 datasetId를 가지며 남의 영수증으로 저장 성공을 응답하지 않는다.
    // 파일 이름표(pclm_file)는 취합본이 스스로 짓는다 — 역할도 신원도 제출본의 것이 아니다.
    public static IReadOnlyList<string> Untouched => ["app_setting", "notice_group", "pclm_file", "erp_capture", "erp_mapping"];

    /// <param name="부모">딸린 줄일 때, 따라갈 표. 부모 키는 이 표 키의 <b>앞 두 열</b>이다.</param>
    private sealed record 표(string 이름, string[] 키, string? 시각, 규칙 규칙, string 종류, string? 부모 = null);

    /// <summary>
    /// 살아 있는 표와 그 규칙. <b><c>app_setting</c> 은 여기 없다 — 싣지 않는다.</b>
    ///
    /// <para>제출한 사람의 이름·골라 둔 계획 엑셀이 취합본에 섞이면, 취합본을 연 사람이
    /// 남의 설정을 자기 것으로 보게 된다. 제출자 이름만 읽고 옮기지는 않는다.</para>
    ///
    /// <para>차례는 <b>부모가 먼저</b>다. 딸린 줄이 부모의 승자를 보고 따라가므로, 뒤집히면
    /// 따라갈 것이 아직 없다.</para>
    /// </summary>
    private static readonly IReadOnlyList<표> 표들 =
    [
        new("counterparty", ["business_number"], "updated_at", 규칙.늦은쪽, "상대자"),

        // 계획 엑셀은 부서마다 따로 돌아 같은 조달요구번호가 다른 제출본에서 온다.
        new("plan", ["request_number"], "imported_at", 규칙.늦은쪽, "계획"),

        new("request_series", ["request_base"], null, 규칙.합집합, "접수 계열"),
        // group_base 는 파생이라 여기서 고르지 않는다 — 제출본마다 다른 이름이 와도
        // 합집합은 겹침을 알리지 않고, 취합 뒤 Rebuild 가 어차피 다시 계산한다.
        new("notice_series", ["notice_base"], null, 규칙.합집합, "공고 계열"),
        new("contract_series", ["contract_base"], null, 규칙.합집합, "계약 계열"),

        new("request", ["request_base", "seq"], "updated_at", 규칙.늦은쪽, "접수"),
        new("notice", ["notice_base", "seq"], "updated_at", 규칙.늦은쪽, "공고"),
        new("contract", ["contract_base", "seq"], "updated_at", 규칙.늦은쪽, "계약"),

        new("erp_source", ["entity_type", "entity_base", "entity_seq"], null, 규칙.엔티티부모따라, "ERP 출처"),
        new("erp_row", ["entity_type", "entity_base", "entity_seq", "table_name", "source_key"], null, 규칙.엔티티부모따라, "원천 행 대응"),
        new("request_item", ["request_base", "seq", "line_no"], null, 규칙.부모따라, "접수 품목", "request"),
        new("notice_item", ["notice_base", "seq", "line_no"], null, 규칙.부모따라, "공고 품목", "notice"),
        new("notice_schedule", ["notice_base", "seq", "line_no"], null, 규칙.부모따라, "공고 일정", "notice"),
        new("notice_officer_contact", ["notice_base", "seq", "line_no"], null, 규칙.부모따라, "수요기관 담당자", "notice"),
        new("notice_relation", ["notice_base", "seq", "line_no"], null, 규칙.부모따라, "관련공고", "notice"),
        new("contract_item", ["contract_base", "seq", "line_no"], null, 규칙.부모따라, "계약 품목", "contract"),
        new("erp_notice_attachment", ["notice_base", "seq", "line_no"], null, 규칙.부모따라, "공고 첨부", "notice"),
        new("erp_partner", ["contract_base", "seq", "line_no"], null, 규칙.부모따라, "계약 업체", "contract"),
        new("contract_attachment", ["contract_base", "seq", "line_no"], null, 규칙.부모따라, "계약 첨부", "contract"),

        // 사람 값은 계열 키라 차수를 타지 않는다. 그대로 따라온다.
        new("request_user_field", ["request_base", "field_name"], "updated_at", 규칙.늦은쪽, "접수 사람값"),
        new("notice_user_field", ["notice_base", "field_name"], "updated_at", 규칙.늦은쪽, "공고 사람값"),
        new("contract_user_field", ["contract_base", "field_name"], "updated_at", 규칙.늦은쪽, "계약 사람값"),

        new("field_override", ["entity_type", "base", "seq", "column_name"], "updated_at", 규칙.늦은쪽, "덮개"),

        new("user_column", ["entity_type", "field_name"], null, 규칙.정의, "사용자 열"),

        new("project_link", ["contract_base"], "confirmed_at", 규칙.늦은쪽, "계약 링크"),
        new("request_link", ["request_base"], "confirmed_at", 규칙.늦은쪽, "접수 링크"),
        new("request_item_link", ["request_base", "request_seq", "line_no"], null, 규칙.합집합, "품목 짝"),

        new("link_rejection", ["contract_base", "notice_group"], "rejected_at", 규칙.늦은쪽, "계약 물리침"),
        new("request_link_rejection", ["request_base", "notice_group"], "rejected_at", 규칙.늦은쪽, "접수 물리침"),
    ];

    /// <summary>제출본에서 읽은 줄 하나. 어느 제출본에서 왔는지를 값과 함께 든다.</summary>
    private sealed record 후보(int 제출본, string 시각, object?[] 값);

    /// <summary>표 안에서 키를 잇는 글자. 사람에게 보일 때는 <c>·</c> 로 바꾼다.</summary>
    private const char 키이음 = '\u0001';

    /// <summary>
    /// 폴더에서 <c>.pclm</c> 을 걷는다. 걷는 자리가 명령줄과 창 두 곳이라 여기 둔다.
    ///
    /// <para><b>여기서는 가리지 않는다.</b> 지난 취합본이 함께 걸려도 <see cref="Merge"/> 가 역할을 보고
    /// 거절하며 보고에 적는다 — 취합본이 취합본을 먹으면 지난 취합에서 밀려난 것이 남의 이름을 달고
    /// 되살아나는데, 그것을 막는 것은 이름이 아니라 <c>pclm_file.role</c> 이다.</para>
    ///
    /// <para>폴더가 없으면 빈 목록이다 — 부르는 쪽이 "제출본이 없다" 는 말을 하게 한다.</para>
    /// </summary>
    public static IReadOnlyList<string> FindSubmissions(string folder)
    {
        if (!Directory.Exists(folder)) return [];

        return
        [
            .. Directory.GetFiles(folder, "*.pclm")
                .Select(Path.GetFullPath)
                .Distinct(StringComparer.OrdinalIgnoreCase)
                .Order(StringComparer.OrdinalIgnoreCase),
        ];
    }

    /// <summary>
    /// 제출본들을 모아 <paramref name="outPath"/> 에 취합본 하나를 짓는다.
    ///
    /// <para><paramref name="outPath"/> 에 파일이 있으면 지우고 처음부터 짓는다 — 고쳐 쓰지 않는
    /// 까닭은 이 클래스 머리에 있다. 제출본 목록에 취합본 자리가 섞여 있으면 그것은 뺀다.</para>
    /// </summary>
    public MergeReport Merge(IEnumerable<string> sources, string outPath)
    {
        var target = Path.GetFullPath(outPath);

        // 다 지은 뒤 덮는 자리에서도 막히지만, 그때는 제출본을 다 읽은 뒤다. 덮을 수 없는 자리면 먼저 멈춘다.
        PclmFile.RequireOverwritable(target);

        var files = sources
            .Select(Path.GetFullPath)
            .Where(f => !string.Equals(f, target, StringComparison.OrdinalIgnoreCase))
            .ToList();

        var work = Path.Combine(Path.GetTempPath(), "pclm-merge-" + Guid.NewGuid().ToString("N")[..8]);
        Directory.CreateDirectory(work);

        try
        {
            return Build(files, target, work);
        }
        finally
        {
            // 지우려면 잠금이 풀려야 한다. 연결은 닫혔어도 풀이 파일을 쥐고 있다.
            SqliteConnection.ClearAllPools();
            try { Directory.Delete(work, recursive: true); }
            catch { /* 임시 복사본을 못 지워도 취합본은 성하다. 다음 청소에 걷힌다. */ }
        }
    }

    private static MergeReport Build(IReadOnlyList<string> files, string target, string work)
    {
        // 임시 폴더에서 짓고 마지막에 한 벌 떠서 자리에 놓는다. 제출본과 같은 길이다
        // (PclmFile.Snapshot 의 VACUUM INTO) — 그래야 취합본도 -wal 없는 한 파일로 떨어진다.
        // 자리에 바로 지으면 짓는 동안의 WAL 이 그 옆에 남고, 그것을 치우려면 프로세스 전체의
        // 연결 풀을 비워야 한다 — 남의 연결까지 닫는 일이라 그 자리에서 하지 않는다.
        var database = PclmFile.Create(Path.Combine(work, "취합.pclm"), PclmRole.Merged);

        // 열 목록은 취합본에서 읽는다. 제출본은 올려서 여니 같은 판이고, 어긋나면 읽는 자리에서
        // 시끄럽게 실패한다 — 조용히 한 열을 빠뜨리는 것보다 낫다.
        var columns = 표들.ToDictionary(t => t.이름, t => Columns(database, t.이름), StringComparer.Ordinal);
        var pools = 표들.ToDictionary(
            t => t.이름, _ => new Dictionary<string, List<후보>>(StringComparer.Ordinal), StringComparer.Ordinal);

        var 제출자 = new List<string>();
        var 거절 = new List<MergeRejection>();

        // 복사본 이름은 제출본의 차례로 짓는다. 거절한 것도 한 자리를 쓴다 — 이름을 다시 쓰면
        // 앞의 것을 열었던 연결이 아직 파일을 쥐고 있을 때 덮어쓰기가 막힌다.
        var 차례 = 0;

        foreach (var file in files)
        {
            // 받을 것인지는 원본을 읽기만 해서 정한다 — Migrate 보다 먼저다. 마이그레이션은 앞으로만
            // 가므로 새 판을 내릴 길이 없고, 기준선보다 옛 시험판은 Migrate 가 예외로 거절하는데 그 예외에
            // 취합 전체가 멎지 않게 여기서 먼저 걸러 낸다.
            if (Rejected(PclmFile.Inspect(file)) is { } 까닭)
            {
                거절.Add(new MergeRejection(Path.GetFileName(file), 까닭));
                continue;
            }

            // 남의 파일이다. 읽기 전용 연결로 한 벌 떠서 열고, 옛 판이면 사본 위에서 올린다.
            //
            // 파일 셋(본체·-wal·-shm)을 따로 복사하지 않는다. 사람이 자기 작업자료를 이름만 바꿔
            // 보내면서 아직 그 파일을 쓰고 있으면, 셋을 하나씩 나르는 사이에 어긋난 셋이 된다.
            // VACUUM INTO 는 WAL 에 남은 마지막 것까지 커밋된 한 시점으로 뜬다.
            var copy = Path.Combine(work, $"{차례++:D3}.pclm");
            new Database(file, Access.Read).Snapshot(copy);

            var source = new Database(copy);
            source.Migrate();

            using var connection = source.OpenReadOnly();
            var index = 제출자.Count;
            제출자.Add(Submitter(connection) is { Length: > 0 } name ? name : Path.GetFileNameWithoutExtension(file));

            foreach (var t in 표들) Collect(connection, t, columns[t.이름], index, pools[t.이름]);
        }

        var 겹침 = new List<MergeConflict>();
        var 임자 = new Dictionary<string, Dictionary<string, int>>(StringComparer.Ordinal);
        var 실린것 = new Dictionary<string, List<후보>>(StringComparer.Ordinal);

        foreach (var t in 표들)
            실린것[t.이름] = Resolve(t, columns[t.이름], pools[t.이름], 제출자, 임자, 겹침);

        // request_link.notice_group 은 UNIQUE 다. PK 충돌과 별개로 여기서 한 번 더 고른다.
        실린것["request_link"] = OnePerNotice(columns["request_link"], 실린것["request_link"], 제출자, 겹침);

        Write(database, columns, 실린것);

        // 사람이 세운 열이 제출본에서 왔다. 뷰는 그 정의를 읽어 짓는 것이라 다시 지어야
        // 취합본의 계약면에 그 칸이 선다.
        database.RefreshViews();

        // 수는 다 지은 취합본에서 센다 — pclm 현황 의 「쌓인 것」과 같은 셈이라 어긋나면 안 된다.
        MergeReport report;

        using (var opened = database.Open())
        {
            report = new MergeReport(
                제출자.Count,
                Count(opened, "SELECT COUNT(*) FROM plan;"),
                Count(opened, "SELECT COUNT(DISTINCT request_base) FROM request;"),
                Count(opened, "SELECT COUNT(DISTINCT notice_base) FROM notice;"),
                Count(opened, "SELECT COUNT(DISTINCT contract_base) FROM contract;"),
                거절, 겹침);

            // WAL 을 본체로 접는다. 뒤이어 뜨는 한 벌은 어차피 정합하지만, 접어 두면 임시
            // 복사본을 지우는 자리에서 걸릴 것이 하나 줄어든다.
            Execute(opened, "PRAGMA wal_checkpoint(TRUNCATE);");
        }

        // 사람이 자리를 짚어 만든 결과물이라 이미 있으면 덮는다 — 취합본은 매번 처음부터
        // 짓는 것이므로, 지난 것을 남겨 둘 까닭이 없다.
        // 뜬 사본은 자기 신원을 새로 받는다 — 임시로 지은 것의 신원이 결과물에 남을 까닭이 없다.
        PclmFile.Snapshot(database, target, PclmRole.Merged, overwrite: true);
        return report;
    }

    /// <summary>거절 까닭. 보고 글이 그대로 적는다.</summary>
    public const string PCLM아님 = "PCLM 파일이 아님";
    public const string 옛시험판 = "옛 시험판";
    public const string 새판 = "새 판";
    public const string 파일없음 = "파일이 없음";

    /// <summary>
    /// 받지 않을 까닭. 받으면 <c>null</c>.
    ///
    /// <para>받는 것은 제출본과 작업자료다. 취합본을 받으면 지난 취합에서 밀려난 것이 되살아나고,
    /// 백업과 옮겨진 옛 자료는 어느 작업자료의 옛 모습이라 같은 사람의 것이 두 번 들어간다.
    /// 그 셋의 까닭은 역할의 이름(취합본·백업·옮겨진 옛 자료) 그대로다.</para>
    /// </summary>
    private static string? Rejected(PclmInfo info) => info switch
    {
        { Kind: PclmKind.Ok, Role: PclmRole.Submission or PclmRole.Work } => null,
        { Kind: PclmKind.Ok } => PclmRole.Name(info.Role),
        { Kind: PclmKind.TooOld } => 옛시험판,
        { Kind: PclmKind.TooNew } => 새판,
        { Kind: PclmKind.NotFound } => 파일없음,
        _ => PCLM아님,
    };

    /// <summary>제출자 이름. 제출본의 설정에서 읽되 <b>그것만</b> 읽는다.</summary>
    private static string Submitter(SqliteConnection connection)
    {
        using var command = connection.CreateCommand();
        command.CommandText = "SELECT value FROM app_setting WHERE key = 'submit.name';";
        return command.ExecuteScalar() as string ?? "";
    }

    private static string[] Columns(Database database, string table)
    {
        using var connection = database.OpenReadOnly();
        using var command = connection.CreateCommand();
        command.CommandText = $"PRAGMA table_info(\"{table}\");";

        var names = new List<string>();
        using var reader = command.ExecuteReader();
        while (reader.Read()) names.Add(reader.GetString(1));
        return [.. names];
    }

    /// <summary>제출본 하나의 그 표를 통째로 읽어 후보로 쌓는다.</summary>
    private static void Collect(
        SqliteConnection connection, 표 t, string[] columns, int 제출본, Dictionary<string, List<후보>> pool)
    {
        var keyAt = t.키.Select(k => Array.IndexOf(columns, k)).ToArray();
        var timeAt = t.시각 is null ? -1 : Array.IndexOf(columns, t.시각);

        using var command = connection.CreateCommand();
        command.CommandText = $"SELECT {string.Join(", ", columns.Select(c => $"\"{c}\""))} FROM \"{t.이름}\";";

        using var reader = command.ExecuteReader();
        while (reader.Read())
        {
            var values = new object?[columns.Length];
            for (var i = 0; i < columns.Length; i++) values[i] = reader.GetValue(i);

            var key = string.Join(키이음, keyAt.Select(i => Text(values[i])));
            if (!pool.TryGetValue(key, out var list)) pool[key] = list = [];
            list.Add(new 후보(제출본, timeAt < 0 ? "" : Text(values[timeAt]), values));
        }
    }

    /// <summary>키마다 하나를 고르고, 밀어낸 것이 있으면 알린다.</summary>
    private static List<후보> Resolve(
        표 t, string[] columns, Dictionary<string, List<후보>> pool,
        IReadOnlyList<string> 제출자, Dictionary<string, Dictionary<string, int>> 임자,
        List<MergeConflict> 겹침)
    {
        var 실린것 = new List<후보>(pool.Count);
        var mine = 임자[t.이름] = new Dictionary<string, int>(StringComparer.Ordinal);

        foreach (var (key, candidates) in pool.OrderBy(p => p.Key, StringComparer.Ordinal))
        {
            후보 winner;

            if (t.규칙 is 규칙.부모따라 or 규칙.엔티티부모따라)
            {
                // 부모의 승자만 딸린 줄을 낸다. 부모가 어느 제출본에도 없으면(고아 줄) 먼저 온
                // 제출본을 임자로 삼는다 — 그래도 한 제출본의 줄만 실린다.
                var parts = key.Split(키이음);
                var parent = string.Join(키이음, t.규칙 == 규칙.엔티티부모따라 ? parts.Skip(1).Take(2) : parts.Take(2));
                var parentTable = t.규칙 == 규칙.엔티티부모따라 ? parts[0] : t.부모!;
                if (!임자.ContainsKey(parentTable)) continue;
                var owner = 임자[parentTable].TryGetValue(parent, out var o)
                    ? o
                    : mine.TryGetValue(parent, out var seen) ? seen : candidates.Min(c => c.제출본);

                mine[parent] = owner;
                if (candidates.FirstOrDefault(c => c.제출본 == owner) is not { } follower) continue;
                winner = follower;
            }
            else
            {
                winner = t.규칙 switch
                {
                    // 고를 것이 없는 표는 먼저 온 것을 둔다.
                    규칙.합집합 or 규칙.정의 => candidates.MinBy(c => c.제출본)!,

                    // 늦은 쪽이 이긴다. 시각이 같으면 먼저 온 제출본이다.
                    _ => candidates
                        .OrderByDescending(c => c.시각, StringComparer.Ordinal)
                        .ThenBy(c => c.제출본)
                        .First(),
                };

                if (Reportable(t, columns, candidates, winner) is { Count: > 0 } losers)
                    겹침.Add(new MergeConflict(
                        t.종류, key.Replace(키이음, '·'),
                        new MergeSide(제출자[winner.제출본], winner.시각),
                        [.. losers.Select(c => new MergeSide(제출자[c.제출본], c.시각))]));

                mine[key] = winner.제출본;
            }

            실린것.Add(winner);
        }

        return 실린것;
    }

    /// <summary>
    /// 밀어낸 것 가운데 <b>사람에게 알릴</b> 것.
    ///
    /// <para><b>알림이 본체다</b> — 사람이 적은 것이 알림 없이 밀려나면 그 사람은 사라진 줄도
    /// 모른다. 그래서 <b>값이 다른 것은 하나도 빠짐없이 낸다.</b> 다만 알릴 것이 없는 자리에서는
    /// 침묵한다: 두 사람이 <b>같은 값을 들고 있는 것은 겹침이 아니라 일치</b>라, 가릴 것이 없다.
    /// 같은 공고문·같은 계획 줄·같은 상대자는 여러 제출본에 있는 것이 정상이고 실제로 대부분이
    /// 그렇다 — 그것까지 한 줄씩 내면 <b>진짜 갈린 둘이 수백 줄에 묻힌다</b>. 그래서 시각 열을
    /// 뺀 나머지가 이긴 것과 똑같은 줄은 내지 않는다. (기계가 판정한 것이 아니다. 판정할 것이
    /// 없는 자리를 가린 것뿐이다.)</para>
    ///
    /// <para><c>합집합</c> 은 키가 같으면 같은 것이라 애초에 고를 일이 없다(계열 키·
    /// 품목 짝). <c>정의</c> 는 <b>뜻이 어긋날 때만</b> 알린다 — 차례(<c>sort_order</c>)는 뜻이
    /// 아니라 <b>보이는 자리</b>라 어긋나도 알리지 않고 먼저 온 것의 차례를 쓴다.</para>
    /// </summary>
    private static List<후보> Reportable(표 t, string[] columns, List<후보> candidates, 후보 winner)
    {
        if (candidates.Count < 2 || t.규칙 == 규칙.합집합) return [];

        var losers = candidates.Where(c => !ReferenceEquals(c, winner)).OrderBy(c => c.제출본);

        if (t.규칙 == 규칙.정의)
        {
            var kind = Array.IndexOf(columns, "kind");
            var options = Array.IndexOf(columns, "options");

            return [.. losers.Where(c =>
                Text(c.값[kind]) != Text(winner.값[kind]) || Text(c.값[options]) != Text(winner.값[options]))];
        }

        var 시각열 = t.시각 is null ? -1 : Array.IndexOf(columns, t.시각);
        return [.. losers.Where(c => Differs(columns.Length, c, winner, 시각열))];
    }

    /// <summary>시각 한 열을 빼고 두 줄이 다른가. 같은 문서를 둘이 읽으면 그 열만 갈린다.</summary>
    private static bool Differs(int width, 후보 a, 후보 b, int 시각열)
    {
        for (var i = 0; i < width; i++)
            if (i != 시각열 && Text(a.값[i]) != Text(b.값[i])) return true;

        return false;
    }

    /// <summary>
    /// <c>request_link.notice_group</c> 은 UNIQUE 다.
    ///
    /// <para>두 사람이 <b>같은 공고건에 다른 접수</b>를 이어 두었으면 그대로 넣을 때 UNIQUE 위반으로
    /// 취합이 터진다. 인수인계 직후에는 그것이 정상이라 멈출 자리가 아니다 — 늦게 이은 쪽만
    /// 싣고 겹친 것으로 알린다. 접수:건을 1:1 로 못 박은 스키마의 뜻(ADR-021)은 그대로다.</para>
    ///
    /// <para><b>여기서 고르는 것과 <see cref="NoticeGroups.Rebuild"/> 가 미는 것은 다른 자리다.</b>
    /// 이쪽은 제출본들이 <b>이미 같은 이름으로</b> 이어 둔 것을 고르고, 저쪽은 취합 뒤 건이
    /// 합쳐지며 <b>비로소</b> 한 건이 된 둘을 민다 — 여기서 걸리지 않은 짝이 저기서 걸린다.</para>
    /// </summary>
    private static List<후보> OnePerNotice(
        string[] columns, List<후보> rows, IReadOnlyList<string> 제출자, List<MergeConflict> 겹침)
    {
        // 링크의 끝점은 건이다(스키마 V15). 옛 이름을 그대로 두면 −1 이 되어 취합이
        // 인덱스 예외로 죽는다 — 열 이름을 문자열로 짚는 자리라 컴파일러가 잡아 주지 않는다.
        var noticeAt = Array.IndexOf(columns, "notice_group");
        var kept = new List<후보>(rows.Count);

        foreach (var group in rows.GroupBy(r => Text(r.값[noticeAt])).OrderBy(g => g.Key, StringComparer.Ordinal))
        {
            var winner = group
                .OrderByDescending(c => c.시각, StringComparer.Ordinal)
                .ThenBy(c => c.제출본)
                .First();

            kept.Add(winner);

            var losers = group.Where(c => !ReferenceEquals(c, winner)).OrderBy(c => c.제출본).ToList();
            if (losers.Count == 0) continue;

            겹침.Add(new MergeConflict(
                "공고 하나에 접수 둘", group.Key,
                new MergeSide(제출자[winner.제출본], winner.시각),
                [.. losers.Select(c => new MergeSide(제출자[c.제출본], c.시각))]));
        }

        return kept;
    }

    /// <summary>고른 것을 취합본에 넣는다.</summary>
    private static void Write(
        Database database,
        IReadOnlyDictionary<string, string[]> columns,
        IReadOnlyDictionary<string, List<후보>> 실린것)
    {
        using var connection = database.Open();

        // 넣는 차례가 어긋나면 외래키가 걸린다. 끄고 넣되 끝에 반드시 확인한다 —
        // 조용히 깨진 취합본을 내놓지 않는다. (PRAGMA 는 트랜잭션 안에서 켜고 끌 수 없다.)
        Execute(connection, "PRAGMA foreign_keys = OFF;");

        using (var transaction = connection.BeginTransaction())
        {
            // 갓 지은 DB 에는 기본 사용자 열이 심겨 있다(스키마 V2). 취합본에 서야 할 것은
            // 제출본들이 실제로 들고 있는 열이므로, 심긴 것을 치우고 그 자리에 넣는다 —
            // 남겨 두면 사람이 고쳐 둔 후보 목록이 기본값에 밀린다.
            Execute(connection, "DELETE FROM user_column;", transaction);

            // 넣는 차례는 표들 의 차례다 — 부모가 먼저 선다. 외래키를 꺼 두었어도 사람이
            // 나중에 취합본을 들여다볼 때 읽히는 차례가 낫다.
            foreach (var t in 표들)
            {
                var rows = 실린것[t.이름];
                if (rows.Count == 0) continue;

                var names = columns[t.이름];
                using var command = connection.CreateCommand();
                command.Transaction = transaction;
                command.CommandText =
                    $"INSERT INTO \"{t.이름}\" ({string.Join(", ", names.Select(c => $"\"{c}\""))}) " +
                    $"VALUES ({string.Join(", ", names.Select((_, i) => $"$p{i}"))});";

                // 형을 짚지 않는다. 짚으면 INTEGER 칸에 든 수가 문자열로 바뀌어 들어간다 —
                // 값의 형은 읽어 온 그대로가 맞다.
                for (var i = 0; i < names.Length; i++) command.Parameters.AddWithValue($"$p{i}", DBNull.Value);

                foreach (var row in rows)
                {
                    for (var i = 0; i < names.Length; i++) command.Parameters[i].Value = row.값[i] ?? DBNull.Value;
                    command.ExecuteNonQuery();
                }
            }

            transaction.Commit();
        }

        Execute(connection, "PRAGMA foreign_keys = ON;");

        // 취합본의 건을 정하는 <b>유일한 자리</b>다. notice_group 은 나르지 않으므로 여기까지
        // 취합본에는 건이 하나도 없고, 계열과 링크가 아직 없는 이름을 가리키고 있다 —
        // 참조 검사보다 앞서 지어야 그 검사를 지난다.
        NoticeGroups.Rebuild(connection, transaction: null);

        AssertForeignKeysHold(connection);
    }

    private static void AssertForeignKeysHold(SqliteConnection connection)
    {
        using var command = connection.CreateCommand();
        command.CommandText = "PRAGMA foreign_key_check;";

        var broken = new List<string>();
        using var reader = command.ExecuteReader();
        while (reader.Read())
            broken.Add($"{reader.GetValue(0)} 행이 {reader.GetValue(2)} 를 가리키지 못합니다");

        if (broken.Count > 0)
            throw new InvalidOperationException(
                "취합 뒤 참조가 어긋났습니다: " + string.Join(" / ", broken.Take(5)));
    }

    /// <summary>값을 문자열로 본다. 빈 것은 <c>null</c> 이 아니라 빈 문자열이다.</summary>
    private static string Text(object? value) =>
        value is null or DBNull ? "" : Convert.ToString(value, CultureInfo.InvariantCulture) ?? "";

    private static int Count(SqliteConnection connection, string sql)
    {
        using var command = connection.CreateCommand();
        command.CommandText = sql;
        return Convert.ToInt32(command.ExecuteScalar());
    }

    private static void Execute(SqliteConnection connection, string sql, SqliteTransaction? transaction = null)
    {
        using var command = connection.CreateCommand();
        command.CommandText = sql;
        command.Transaction = transaction;
        command.ExecuteNonQuery();
    }
}

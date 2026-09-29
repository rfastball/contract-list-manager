namespace Pclm.Core.Storage;

/// <summary>
/// 소비자에게 내보이는 계약면.
///
/// <para>이 뷰들이 <b>다른 프로그램과의 유일한 약속</b>이다. 컬럼 이름을 바꾸면 저쪽에 저장된
/// 필드 연결이 끊기므로, 고칠 일이 생기면 <c>_v2</c> 를 새로 만들어 나란히 두고 <c>_v1</c> 은
/// 그대로 남긴다.</para>
///
/// <para>컬럼 이름을 <b>한글로 둔다.</b> 받는 쪽이 어휘를 따로 선언하지 않으면 키를 그대로
/// 사람이 읽는 이름으로 쓰기 때문에, 한글이면 엑셀 머리글과 똑같이 동작한다.</para>
///
/// <para>값은 모두 <b>문서에 그대로 찍힐 문자열</b>이다. 금액에 자릿점을 넣는 것도 그래서다 —
/// 사용자 정의 함수는 다른 프로세스에서 보이지 않으므로 순수 SQL로 짠다.</para>
/// </summary>
public static class Views
{
    public const string Version = "v1";

    /// <summary>
    /// 계약면 전부. <b>다리가 열어 줄 표를 가리는 허용목록</b>이자 뷰 정의 목록이다.
    ///
    /// <para>차례는 <b>먼저 있던 것을 앞에</b> 둔다 — <c>contract/views.txt</c> 가 이 차례로
    /// 박제되어 있어, 새 뷰를 사이에 끼우면 바뀐 것이 없는 절까지 통째로 다시 쓰인다.</para>
    /// </summary>
    public static IReadOnlyList<string> Names =>
    [
        "v_통합_v1", "v_공고_v1", "v_공고품목_v1", "v_계약_v1", "v_품목_v1",
        "v_접수_v1", "v_접수품목_v1", "v_통합_v2", "v_계획_v1",

        // 차수를 펴서 내는 것들(스키마 V15). <b>끝에 붙인다</b> — 사이에 끼우면 박제가
        // 바뀐 것 없는 절까지 통째로 다시 쓰여, 무엇이 실제로 달라졌는지 보이지 않는다.
        "v_공고차수_v1", "v_통합_v3", "v_계약차수_v1",
        "v_ERP원천_v1", "v_계약업체_v1", "v_ERP접수_v1",
    ];

    /// <summary>
    /// <b>고칠 수 없는 표.</b> 화면이 값을 고치는 길을 아예 열지 않는다.
    ///
    /// <para><c>v_계획_v1</c> 은 <c>plan</c> 이 개체가 아니라서다 —
    /// <c>field_override</c>·<c>user_column</c> 의 <c>entity_type</c> 에 자리가 없어 고친 값을
    /// 걸 데가 없다. 값을 고치는 길은 엑셀을 고쳐 다시 넣는 것 하나다.</para>
    ///
    /// <para><b>차수 뷰 둘과 <c>v_통합_v3</c> 은 까닭이 다르다.</b>
    /// <see cref="Store.FaceView"/> 가 종류당 뷰를 <b>하나</b>만 안다 — 공고는 <c>v_공고_v1</c>,
    /// 계약은 <c>v_계약_v1</c> 이다. 그래서 옛 차수의 칸을 고치면 덮개가 담을
    /// <c>original</c>(고치기 전에 그 자리에 있던 값)을 그 뷰에서 찾지 못해 <b>빈 문자열로
    /// 박히고</b>, "무엇을 무엇으로 고쳤나" 가 오류 없이 거짓이 된다. 되돌리기 안내도 함께
    /// 거짓이 된다. 고치는 자리는 공고 탭·계약 탭이고, 차수 뷰는 <b>보는</b> 자리다.</para>
    ///
    /// <para>이 목록과 다리(<c>Bridge.ReadSheet</c>)의 잠금 목록이 갈리면 나중에 편집을 켤 때 조용히
    /// 샌다 — 여기 하나만 본다.</para>
    /// </summary>
    public static IReadOnlyList<string> ReadOnly =>
    [
        "v_계획_v1", "v_통합_v3", "v_공고차수_v1", "v_계약차수_v1",
        "v_ERP원천_v1", "v_계약업체_v1", "v_ERP접수_v1",
    ];

    /// <summary>
    /// 엑셀로 나가는 것. <b><see cref="Names"/> 와 일부러 다르다.</b>
    ///
    /// <para>통합을 <c>v1</c>·<c>v2</c> 두 장으로 내면 받는 쪽이 어느 것을 볼지 헷갈린다 —
    /// 한 권 안에 같은 뜻의 시트가 둘 있으면 그 자체가 오답의 원인이다. 파일에는 <b>v2 만</b>
    /// 싣고, <c>v_통합_v1</c> 은 뷰로는 그대로 남겨 저쪽에 저장된 연결을 지킨다.</para>
    ///
    /// <para>차례는 <b>흐름 그대로</b> — 통합 다음에 접수 → 공고 → 계약이다.</para>
    /// </summary>
    public static IReadOnlyList<string> Exported =>
    [
        "v_통합_v2",
        "v_접수_v1", "v_접수품목_v1",
        "v_공고_v1", "v_공고품목_v1",
        "v_계약_v1", "v_품목_v1",
        "v_ERP원천_v1", "v_계약업체_v1", "v_ERP접수_v1",
    ];

    /// <summary>
    /// 시트·화면에 쓸 짧은 이름. <c>v_접수품목_v1</c> → <c>접수품목</c>.
    /// <b>판을 떼는 규칙을 여기 하나에만 둔다</b> — 엑셀과 명령줄이 같은 이름을 불러야 한다.
    /// </summary>
    public static string SheetName(string view)
    {
        var name = view.StartsWith("v_", StringComparison.Ordinal) ? view[2..] : view;
        var mark = name.LastIndexOf("_v", StringComparison.Ordinal);

        return mark > 0 && name[(mark + 2)..].All(char.IsAsciiDigit) ? name[..mark] : name;
    }

    /// <summary>
    /// 이 뷰의 줄 하나는 무엇인가. 덮개와 사람 열이 어느 표에 담길지가 여기서 갈린다.
    ///
    /// <para>다리가 쓰던 <c>view.Contains("공고") ? notice : contract</c> 는 접수가 들어오는
    /// 순간 틀린다 — <c>v_접수_v1</c> 이 계약으로 읽혀 고친 값이 엉뚱한 표에 담긴다.
    /// <b>접수를 먼저 본다</b>: 통합(v2)은 셋을 다 이름에 담지 않으므로 계약으로 떨어진다.</para>
    ///
    /// <para><b>계획을 맨 앞에 본다.</b> <c>v_계획_v1</c> 은 접수도 공고도 아니라 그대로 두면
    /// 계약으로 떨어지는데, 그러면 계획 뷰의 모든 열이 <b>고칠 수 있는 것</b>으로 잡혀 덮개가
    /// 아무도 읽지 않는 자리(계약번호가 아닌 키)에 걸린다. <c>plan</c> 은 개체가 아니라
    /// <c>field_override</c>·<c>user_column</c> 의 <c>entity_type</c> 에 자리가 없다 —
    /// 이 이름은 "고칠 것이 없다" 를 뜻한다.</para>
    /// </summary>
    /// <remarks>
    /// <para><c>v_통합_v3</c> 은 <b>이름 규칙으로 표현되지 않아</b> 따로 적는다. 줄 하나가
    /// 공고 문서 한 장인데 이름에 「공고」가 없어 그대로 두면 계약으로 떨어지고, 그러면 고친
    /// 값이 <c>contract</c> 자리에 담겨 <b>아무도 읽지 않는 곳</b>에 걸린다 — 위 주석이
    /// <c>v_접수_v1</c> 에 대해 경고하던 바로 그 실패다.</para>
    /// </remarks>
    public static string EntityTypeOf(string view) =>
        view == "v_통합_v3" ? "notice"
        : view.Contains("계획", StringComparison.Ordinal) ? "plan"
        : view.Contains("접수", StringComparison.Ordinal) ? "request"
        : view.Contains("공고", StringComparison.Ordinal) ? "notice"
        : "contract";

    /// <summary>
    /// 손으로 고칠 수 없는 열. <b>레코드가 어디에 있는지를 정하는 것들</b>이라, 고치면 값이
    /// 바뀌는 것이 아니라 레코드가 다른 자리로 옮겨간다 — 덮개로 씌울 수 있는 것이 아니다.
    /// </summary>
    public static IReadOnlyList<string> KeyColumns =>
    [
        "입찰공고번호", "공고본번호", "계약번호", "계약본번호",
        "접수번호", "접수본번호", "차수", "순번",

        // 건과 그 건의 현행 공고. 둘 다 관련공고에서 지어지는 파생값이라, 덮개를 씌우면
        // 화면의 글자만 바뀌고 실제 묶임은 그대로다 — 고칠 수 있는 것처럼 보이는 편이 나쁘다.
        "공고건", "현행공고",
    ];

    /// <summary>
    /// <c>v_통합_v1</c> 이 <c>v_공고_v1</c> 에서 붙여 오는 열.
    ///
    /// <para>통합의 줄 하나는 <b>계약</b>이라 줄 키가 계약번호다. 이 열들은 거기 이어진
    /// <b>다른 레코드</b>의 것이어서 그 키로는 주소가 잡히지 않는다 — 통합 시트에서는 잠그고,
    /// 공고 시트에서 고치게 한다. 같은 뷰를 타므로 고친 것은 통합에도 그대로 비친다.</para>
    /// </summary>
    public static IReadOnlyList<string> UnifiedNoticeColumns =>
    [
        "입찰공고번호", "공고명", "공고종류", "게시일시", "입찰방식", "낙찰방법", "낙찰하한율",
        "사업예산", "배정예산", "추정가격", "기초금액", "개찰일시", "입찰개시일시", "입찰마감일시",
        "등록마감일시", "공고담당자", "사전규격등록번호",
    ];

    /// <summary>
    /// <c>v_통합_v2</c> 가 <c>v_접수_v1</c> 에서 붙여 오는 열.
    /// <see cref="UnifiedNoticeColumns"/> 와 같은 까닭으로 통합에서는 잠근다 —
    /// 줄 키가 계약번호라 <b>이어진 다른 레코드</b>의 칸에는 주소가 잡히지 않는다.
    /// </summary>
    public static IReadOnlyList<string> UnifiedRequestColumns =>
    [
        "접수번호", "조달요구번호", "요청명", "접수일자",
        "품대", "접수수수료", "예산금액", "기관담당자", "기관담당자전화",
    ];

    /// <summary>
    /// <c>v_계약_v1</c> 이 내는 열 차례(사람이 세운 열 앞까지).
    ///
    /// <para><b>이름이 한 벌 더 필요한 까닭.</b> <c>v_통합_v2</c> 는 계약이 아직 없는 줄도 내므로
    /// 계약 뷰를 <c>k.*</c> 로 통째로 받을 수 없다 — LEFT JOIN 이 NULL 을 내는데 계약면의 빈
    /// 값은 빈 문자열이라(받는 쪽의 빈 값 경고가 그것에 걸려 있다), 열마다 감싸려면 이름을
    /// 알아야 한다. SQL 에는 "모든 열을 COALESCE" 라고 적을 말이 없다.</para>
    ///
    /// <para>두 벌이 되면 한쪽이 조용히 늙는다. 계약 뷰에 열을 더하고 여기를 빠뜨리면
    /// <b>통합에서 그 열만 사라지는데</b> 아무것도 실패하지 않는다 — 그래서
    /// <c>DatasetContractTests</c> 가 이 목록과 실제 <c>v_계약_v1</c> 이 같은지 본다.</para>
    /// </summary>
    public static IReadOnlyList<string> ContractColumns =>
    [
        "계약번호", "계약본번호", "차수",
        "계약건명", "계약일자", "계약방법", "계약구분",
        "품명", "수량", "단위", "계약금액", "수수료",
        "지체상금률", "하자보수보증금률", "하자담보책임기간", "계약기간",
        "납품기한", "인도조건", "납품장소", "분할납품", "지급방법",
        "수요기관", "검사기관", "검수기관",
        "계약상대자", "대표자", "사업자등록번호", "상대자주소", "상대자전화", "상대자팩스",
    ];

    /// <summary>빈 값은 NULL 이 아니라 빈 문자열로 낸다 — 받는 쪽의 빈 값 경고가 살아나게.</summary>
    private static string Text(string column) => $"COALESCE({column}, '')";

    private static string YesNo(string column) =>
        $"CASE {column} WHEN 1 THEN '가능' WHEN 0 THEN '불가' ELSE '' END";

    /// <summary>
    /// <c>예</c>·<c>아니오</c> 로 낸다. <see cref="YesNo"/> 의 <c>가능</c>·<c>불가</c> 와 갈리는
    /// 까닭은 <b>양식이 그렇게 적기 때문</b>이다 — 접수서의 외국산여부·선급금 칸에는
    /// 아니오가 찍힌다. 값은 문서에 그대로 찍힐 문자열이라 표기를 옮겨 적지 않는다.
    /// </summary>
    private static string YesNoWord(string column) =>
        $"CASE {column} WHEN 1 THEN '예' WHEN 0 THEN '아니오' ELSE '' END";

    /// <summary>
    /// 뜻 없는 소수부를 턴다. <c>0.0</c> → <c>0</c>, <c>0.075</c> 는 그대로.
    /// decimal 을 담으면 정수도 <c>.0</c> 을 달고 나오는데, 그대로 문서에 찍히면 어색하다.
    /// </summary>
    private static string Number(string column) => $"""
        CASE
            WHEN {column} IS NULL OR {column} = '' THEN ''
            WHEN instr({column},'.') = 0 THEN {column}
            WHEN rtrim({column},'0') LIKE '%.' THEN rtrim(rtrim({column},'0'),'.')
            ELSE rtrim({column},'0')
        END
        """;

    /// <summary>
    /// 날짜를 양식이 쓰는 표기로 되돌린다. <c>2026-09-20 00:00:00</c> → <c>2026/09/20</c>.
    /// 자정이면 시각을 떼는데, 납품기한처럼 날짜만 있는 값이 그렇게 저장되기 때문이다.
    /// </summary>
    private static string DateText(string column) => $"""
        CASE
            WHEN {column} IS NULL OR {column} = '' THEN ''
            WHEN substr({column},11) IN ('', 'T00:00:00', ' 00:00:00', 'T00:00:00.0000000')
                THEN replace(substr({column},1,10),'-','/')
            ELSE replace(substr({column},1,10),'-','/') || ' ' || substr({column},12,8)
        END
        """;

    /// <summary>
    /// 금액에 세 자리마다 자릿점을 넣는다. 소수부는 건드리지 않는다 — 단가에 소수점이 실재한다.
    /// 조 단위까지면 충분해서 재귀 없이 자릿수별로 편다.
    /// </summary>
    private static string Money(string column)
    {
        // 뜻 없는 .0 을 먼저 턴 값 위에서 자릿점을 찍는다.
        var trimmed = Number(column);
        var whole = $"CASE WHEN instr(t,'.')>0 THEN substr(t,1,instr(t,'.')-1) ELSE t END";
        var fraction = $"CASE WHEN instr(t,'.')>0 THEN substr(t,instr(t,'.')) ELSE '' END";

        const string grouped = """
            CASE
                WHEN length(w) <= 3  THEN w
                WHEN length(w) <= 6  THEN substr(w,1,length(w)-3) || ',' || substr(w,-3)
                WHEN length(w) <= 9  THEN substr(w,1,length(w)-6) || ',' || substr(w,-6,3) || ',' || substr(w,-3)
                WHEN length(w) <= 12 THEN substr(w,1,length(w)-9) || ',' || substr(w,-9,3) || ',' || substr(w,-6,3) || ',' || substr(w,-3)
                ELSE w
            END
            """;

        return $"""
            COALESCE((
                SELECT (SELECT {grouped} FROM (SELECT {whole} AS w)) || ({fraction})
                FROM (SELECT {trimmed} AS t)
                WHERE t <> ''
            ), '')
            """;
    }

    /// <summary>
    /// 이 공고의 품목이 <b>세부품명 한 가지</b>인가. 세부품명과 세부품명번호를 함께 본다.
    ///
    /// <para>줄이 하나인가가 아니다 — 같은 물건을 여러 수요기관에 나눠 넣느라 줄만 늘어난
    /// 공고가 흔하다(절단기 12줄, 승강판 2줄). 그런 건은 물건이 하나이므로 공고 뷰에 실을 수 있다.</para>
    /// </summary>
    private const string SoleItemKind = """
        (SELECT COUNT(DISTINCT COALESCE(x.item_name,'') || char(31) || COALESCE(x.detail_item_number,''))
         FROM notice_item x WHERE x.notice_base = n.notice_base AND x.seq = n.seq) = 1
        """;

    /// <summary>
    /// 품목이 세부품명 한 가지일 때만 그 칸을 낸다. 아니면 빈 문자열이다.
    ///
    /// <para><b>왜 첫 줄이 아닌가.</b> 첫 줄만 비치면 세부품명이 여럿인 공고에서 <b>첫 물건의
    /// 값이 공고 전체의 값처럼</b> 보인다. 잘린 것은 눈에 띄지 않지만 틀린 것은 그대로 문서에
    /// 찍힌다 — 없는 편이 낫다. 여러 가지인 공고는 <c>v_공고품목_v1</c> 이 전부 낸다.</para>
    ///
    /// <para>한 가지여도 <b>칸마다 값이 갈리면</b> 그 칸은 비운다(<c>COUNT(DISTINCT)</c>).
    /// 같은 물건이라도 수요기관은 줄마다 다를 수 있어서다. 하나로 정할 수 없는 것을
    /// 하나인 양 내지 않는다.</para>
    /// </summary>
    private static string SoleItem(string column) => $"""
        (SELECT CASE WHEN COUNT(DISTINCT {column}) = 1 THEN MIN({column}) END
         FROM notice_item i
         WHERE i.notice_base = n.notice_base AND i.seq = n.seq AND {SoleItemKind})
        """;

    /// <summary>
    /// <b>모든 줄이 같은 값</b>일 때 그 값. <see cref="SoleItem"/> 과 달리 세부품명이 여럿이어도 된다.
    ///
    /// <para>인도조건처럼 물건이 아니라 <b>납품 방식</b>에 달린 칸이 그렇다. 세부품명이 여럿인
    /// 공고도 줄마다 같은 인도조건을 적는 일이 흔한데, 물건 기준으로 비우면 하나로 정해진 값을
    /// 버리게 된다. 줄마다 갈리면 여전히 비운다.</para>
    /// </summary>
    private static string UniformItem(string column) => $"""
        (SELECT CASE WHEN COUNT(DISTINCT {column}) = 1 THEN MIN({column}) END
         FROM notice_item i
         WHERE i.notice_base = n.notice_base AND i.seq = n.seq)
        """;

    /// <summary>
    /// 품목이 세부품명 한 가지일 때 그 <b>수량을 모두 더한다</b>.
    ///
    /// <para>줄이 나뉜 것은 받는 곳이 여럿이라서지 물건이 여럿이라서가 아니다. 승강판 1대 + 2대는
    /// 3대다 — 첫 줄만 보면 1대가 되어 계약 수량과 어긋난다.</para>
    /// </summary>
    private static string SoleQuantity() => $"""
        (SELECT CAST(SUM(i.quantity) AS TEXT) FROM notice_item i
         WHERE i.notice_base = n.notice_base AND i.seq = n.seq AND {SoleItemKind})
        """;

    /// <summary>
    /// <see cref="SoleItemKind"/> 의 접수판.
    ///
    /// <para><b>몸통을 함께 쓰지 않는다.</b> 저쪽은 <c>notice_item</c> 에 못 박힌 조각이라
    /// 표 이름을 인자로 빼려면 공고 뷰가 함께 흔들린다 — 계약면 다섯의 열은 한 글자도
    /// 건드리지 않기로 한 이상, 닮았다는 이유로 묶는 것이 위험을 더 만든다. 나란히 둔다.</para>
    /// </summary>
    private const string SoleRequestItemKind = """
        (SELECT COUNT(DISTINCT COALESCE(x.item_name,'') || char(31) || COALESCE(x.detail_item_number,''))
         FROM request_item x WHERE x.request_base = r.request_base AND x.seq = r.seq) = 1
        """;

    /// <inheritdoc cref="SoleItem"/>
    private static string SoleRequestItem(string column) => $"""
        (SELECT CASE WHEN COUNT(DISTINCT {column}) = 1 THEN MIN({column}) END
         FROM request_item i
         WHERE i.request_base = r.request_base AND i.seq = r.seq AND {SoleRequestItemKind})
        """;

    /// <inheritdoc cref="UniformItem"/>
    private static string UniformRequestItem(string column) => $"""
        (SELECT CASE WHEN COUNT(DISTINCT {column}) = 1 THEN MIN({column}) END
         FROM request_item i
         WHERE i.request_base = r.request_base AND i.seq = r.seq)
        """;

    /// <inheritdoc cref="SoleQuantity"/>
    private static string SoleRequestQuantity() => $"""
        (SELECT CAST(SUM(i.quantity) AS TEXT) FROM request_item i
         WHERE i.request_base = r.request_base AND i.seq = r.seq AND {SoleRequestItemKind})
        """;

    /// <summary>
    /// 접수의 납품기한. 공고와 달리 <b>일수 칸에 문장이 온다</b> — <c>계약후 90일 이내</c> 가
    /// 그대로 찍혀 있어 셈해서 풀 것이 없다. 날짜가 박혀 있으면 그 날이 기한이고,
    /// 없으면 적힌 문장을 그대로 낸다(ADR-016).
    /// </summary>
    private static string RequestDeadline(string dueColumn, string daysColumn) => $"""
        CASE
            WHEN {dueColumn} IS NOT NULL AND {dueColumn} <> '' THEN {DateText(dueColumn)}
            ELSE COALESCE({daysColumn}, '')
        END
        """;

    /// <summary>
    /// 이 접수서가 묶은 조달요구번호 전부를 <b>순번대로</b> 이어 붙인다.
    /// 줄 하나가 조달요구 하나라, 이것이 그 접수서가 무엇을 요청했는지의 요약이다.
    ///
    /// <para><b>번호마다 한 번만 낸다.</b> 조달요구 하나가 납품장소마다 줄을 갈라 오면 같은
    /// 번호가 줄 수만큼 되풀이되어, 접수서가 요청을 여럿 묶은 것처럼 보이고 칸이 목록을
    /// 밀어냈다. 줄은 품목 잇기(ADR-021)의 재료라 그대로 두고, 요약에서만 접는다 — 차례는
    /// 그 번호가 처음 나온 순번이다.</para>
    /// </summary>
    private const string RequestNumbers = """
        (SELECT group_concat(v, ', ') FROM (
            SELECT x.request_number AS v FROM request_item x
            WHERE x.request_base = r.request_base AND x.seq = r.seq
              AND x.request_number IS NOT NULL AND x.request_number <> ''
            GROUP BY x.request_number
            ORDER BY MIN(x.line_no)))
        """;

    /// <summary>
    /// 담당자가 <b>한 사람일 때만</b> 그 칸을 낸다. <see cref="SoleItem"/> 과 같은 까닭이다 —
    /// 수요기관이 여럿인 공고에서 첫 사람을 공고의 담당자인 양 낼 수는 없다.
    /// </summary>
    private static string SoleContact(string column) => $"""
        (SELECT CASE WHEN COUNT(DISTINCT {column}) = 1 THEN MIN({column}) END
         FROM notice_officer_contact t
         WHERE t.notice_base = n.notice_base AND t.seq = n.seq)
        """;

    /// <summary>
    /// 관련공고를 <b>적힌 차례대로</b> 이어 붙인다. 이 공고가 대신하거나 이 공고를 대신하는
    /// 공고의 번호다 — 재공고는 취소공고와 그 당초를 함께 적어 둘이 온다.
    ///
    /// <para>이음쇠가 <c>,</c> 인 것은 <b>문서가 그렇게 찍기 때문</b>이다
    /// (<c>R26BK09011054-001,R26BK09011054-000</c>). 접수의 <see cref="RequestNumbers"/> 가
    /// <c>', '</c> 를 쓰는 것과 일부러 다르다 — 저쪽은 흩어진 요청번호를 이 저장소가 모아
    /// 세운 요약이고, 이쪽은 한 칸에 적혀 있던 것을 그대로 되돌리는 것이다.</para>
    /// </summary>
    private const string RelatedNotices = """
        (SELECT group_concat(v, ',') FROM (
            SELECT x.related AS v FROM notice_relation x
            WHERE x.notice_base = n.notice_base AND x.seq = n.seq
            ORDER BY x.line_no))
        """;

    /// <summary>
    /// 납품기한을 <b>한 칸으로</b> 낸다. 양식은 <c>납품일수</c>·<c>납품기한</c> 두 칸으로 적지만
    /// 뜻하는 것은 하나다 — 언제까지 넣어야 하는가.
    ///
    /// <list type="bullet">
    /// <item>날짜가 박혀 있으면 그 날이 기한이다 — <c>2026/09/26</c>.</item>
    /// <item>날짜가 없고 일수만 있으면 계약일로부터 센다 — <c>계약 후 90일 이내</c>.</item>
    /// </list>
    ///
    /// <para>날짜가 있는 공고는 일수를 <c>0</c> 으로 적어 두므로 날짜를 먼저 본다.
    /// 그 <c>0</c> 을 그대로 풀면 <c>계약 후 0일 이내</c> 라는 없는 말이 나온다.</para>
    /// </summary>
    private static string DeliveryDeadline(string dueColumn, string daysColumn) => $"""
        CASE
            WHEN {dueColumn} IS NOT NULL AND {dueColumn} <> '' THEN {DateText(dueColumn)}
            WHEN {daysColumn} IS NOT NULL AND {daysColumn} > 0
                THEN '계약 후 ' || CAST({daysColumn} AS TEXT) || '일 이내'
            ELSE ''
        END
        """;

    /// <summary>하자담보기간은 개월로 담고 해로 낸다. 양식이 "2년" 으로 적기 때문이다.</summary>
    private const string WarrantyYears = """
        CASE WHEN n.warranty_text IS NOT NULL THEN n.warranty_text
             WHEN n.warranty_years IS NOT NULL THEN n.warranty_years || '년 ' || coalesce(n.warranty_month_part,'0') || '개월'
             WHEN n.warranty_months IS NULL THEN ''
             ELSE CAST(n.warranty_months / 12 AS TEXT) || '년' END
        """;

    /// <summary>공고 일정에서 한 단계의 시각을 꺼낸다.</summary>
    private static string Step(string column, string keyword) =>
        $"(SELECT {column} FROM notice_schedule s " +
        $"WHERE s.notice_base = n.notice_base AND s.seq = n.seq AND s.name LIKE '%{keyword}%')";

    /// <summary>
    /// 최신 차수만 고른다. <b>수로 비교한다</b> — 차수는 TEXT 라 <c>MAX(seq)</c> 는 사전순이고,
    /// 자릿수가 늘어나는 날(<c>9</c> 다음 <c>10</c>) 조용히 뒤집힌다. 지금 양식은 자릿수가
    /// 고정이라 무사하지만, 무사한 이유가 표기 습관인 것을 코드가 알고 있어야 한다.
    /// </summary>
    private const string LatestNotice = """
        JOIN (SELECT notice_base AS b, MAX(CAST(seq AS INTEGER)) AS s FROM notice GROUP BY notice_base) latest
          ON n.notice_base = latest.b AND CAST(n.seq AS INTEGER) = latest.s
        """;

    /// <summary>
    /// 이 공고가 속한 <b>건</b>. 취소·재공고로 본번호가 갈린 것들이 한 이름 아래 모인다
    /// (스키마 V15). 이름은 그 건이 아는 본번호 중 가장 이른 것이라 <b>문서 없는 조상</b>일 수도
    /// 있다 — 그래서 화면이 사람에게 보이는 것은 이 이름이 아니라 <see cref="건현행"/> 이다.
    ///
    /// <para>윈도 함수를 쓰지 않고 상관 스칼라 서브쿼리로 짠다. 낸 값이 <b>문자열</b>이어야
    /// 계약면의 약속을 지킨다 — <c>DatasetContractTests.뷰의_열은_모두_문자열이다</c> 가 그것을
    /// 붙든다.</para>
    /// </summary>
    private static string 공고건(string noticeBaseExpr) =>
        $"(SELECT s.group_base FROM notice_series s WHERE s.notice_base = {noticeBaseExpr})";

    /// <summary>
    /// 그 건의 <b>대체되지 않은 공고 하나</b>. 건마다 공고 한 줄을 세울 자리에서 이것을 쓴다
    /// (ADR-025 를 넓힌다 — 담기만 하던 관련공고를 이제 읽는다).
    ///
    /// <para><b>규칙은 여기 있지 않다.</b> 무엇이 현행인가의 정본은
    /// <see cref="NoticeGroups.현행공고"/> 하나이고, 이 함수는 그 SELECT 를 파생표로 감싸
    /// <b>한 건의 것만 집어 낼</b> 뿐이다. 같은 뜻을 두 벌로 적어 두었더니 뷰는 상관 스칼라
    /// 서브쿼리로, 링커는 <c>ROW_NUMBER</c> 로 물어 <b>둘이 갈리면 화면이 보여 준 공고와 기계가
    /// 견준 공고가 달라지는</b> 자리였다 — 갈릴 수 있는 것을 없앤다.</para>
    ///
    /// <para>파생표 안에 바깥을 가리키는 것이 없으므로(상관은 <c>WHERE</c> 한 줄뿐이다)
    /// SQLite 가 그것을 한 번 펴 놓고 건마다 집어 간다.</para>
    /// </summary>
    /// <param name="낼것"><c>현행.notice_base</c> 또는 <c>현행.notice_base || '-' || 현행.seq</c>.</param>
    private static string 건현행(string groupExpr, string 낼것) => $"""
        (SELECT {낼것}
         FROM ({NoticeGroups.현행공고}) 현행
         WHERE 현행.group_base = {groupExpr})
        """;

    /// <inheritdoc cref="LatestNotice"/>
    private const string LatestContract = """
        JOIN (SELECT contract_base AS b, MAX(CAST(seq AS INTEGER)) AS s FROM contract GROUP BY contract_base) latest
          ON c.contract_base = latest.b AND CAST(c.seq AS INTEGER) = latest.s
        """;

    /// <inheritdoc cref="LatestNotice"/>
    private const string LatestRequest = """
        JOIN (SELECT request_base AS b, MAX(CAST(seq AS INTEGER)) AS s FROM request GROUP BY request_base) latest
          ON r.request_base = latest.b AND CAST(r.seq AS INTEGER) = latest.s
        """;

    /// <summary>
    /// <c>v_계획_v1</c> 이 쓰는 미리 접은 표들. <b>줄이 갈라지지 않게 하는 것이 전부다.</b>
    ///
    /// <para>계획 뷰의 줄 하나는 <c>plan</c> 한 행이어야 한다 — 갈라지면 계획 건수가 부풀어
    /// 세어지고, 분모로 쓰려고 세운 뷰가 분모 노릇을 못 한다. 그래서 이어 오는 것은 모두
    /// <b>본번호마다 한 줄인 표</b>로 미리 접어 둔 뒤에 <c>LEFT JOIN</c> 한다.</para>
    ///
    /// <list type="bullet">
    /// <item><c>최신접수</c>·<c>최신공고</c>·<c>최신계약</c> — 본번호마다 최신 차수 한 줄.</item>
    /// <item><c>접수짝</c> — 조달요구번호마다 접수 본번호 하나. <b>둘 이상 걸리면
    /// <c>request_base</c> 가 작은 것</b>을 집는다(<c>MIN</c>). 같은 조달요구번호가 접수
    /// 둘에 실리는 일이 있는데(다시 접수된 건), 고르는 기준이 값이어야 다시 지어도 같은 것을
    /// 집는다. 무엇을 집었는지 화면에서 보이도록 접수번호를 열로 함께 낸다.</item>
    /// <item><c>계약묶음</c> — <b>공고건</b>마다 달린 계약 수와, 하나뿐일 때 쓸 계약 본번호.
    /// 링크의 끝점이 건이라(스키마 V15) 본번호로 접으면 재공고 건에서 계약이 갈려 세어진다.</item>
    /// </list>
    /// </summary>
    private const string PlanFolds = """
        최신접수 AS (
            SELECT r.* FROM request r
            JOIN (SELECT request_base AS b, MAX(CAST(seq AS INTEGER)) AS s FROM request GROUP BY request_base) t
              ON r.request_base = t.b AND CAST(r.seq AS INTEGER) = t.s
        ),
        최신공고 AS (
            SELECT n.* FROM notice n
            JOIN (SELECT notice_base AS b, MAX(CAST(seq AS INTEGER)) AS s FROM notice GROUP BY notice_base) t
              ON n.notice_base = t.b AND CAST(n.seq AS INTEGER) = t.s
        ),
        최신계약 AS (
            SELECT c.* FROM contract c
            JOIN (SELECT contract_base AS b, MAX(CAST(seq AS INTEGER)) AS s FROM contract GROUP BY contract_base) t
              ON c.contract_base = t.b AND CAST(c.seq AS INTEGER) = t.s
        ),
        접수짝 AS (
            SELECT i.request_number AS 번호, MIN(i.request_base) AS 본번호
            FROM request_item i
            JOIN 최신접수 r ON r.request_base = i.request_base AND r.seq = i.seq
            WHERE i.request_number IS NOT NULL AND i.request_number <> ''
            GROUP BY i.request_number
        ),
        계약묶음 AS (
            SELECT l.notice_group AS 공고건,
                   COUNT(DISTINCT l.contract_base) AS 건수,
                   MIN(l.contract_base) AS 본번호
            FROM project_link l
            GROUP BY l.notice_group
        )
        """;

    /// <summary>
    /// 계약이 <b>하나일 때만</b> 그 칸을 낸다. 아니면 빈 문자열이다.
    ///
    /// <para><see cref="SoleItem"/> 의 규율 그대로다 — 한 공고에 계약이 여럿 달릴 수 있는데
    /// (분할 낙찰·수요기관 복수) 그중 하나를 계획 한 줄의 계약인 양 내면, 잘린 것은 눈에
    /// 띄지 않지만 틀린 것은 그대로 문서에 찍힌다. 몇 건인지는 <c>계약건수</c> 가 낸다.</para>
    /// </summary>
    private static string SoleContract(string expression) =>
        $"CASE WHEN m.건수 = 1 THEN {expression} ELSE '' END";

    /// <summary>
    /// 사람이 채우는 열 하나를 뷰에 붙이는 SQL 조각. 값은 <c>user_field</c> 에서 끌어온다.
    ///
    /// <para>파서가 쓰는 표와 물리적으로 떨어져 있어서, 문서를 다시 넣어도 손으로 적은 값이
    /// 지워질 수 없다 — 안전을 화면이 아니라 구조가 보장한다.</para>
    ///
    /// <para>붙는 자리는 <b>계열</b>이다. 차수로 이으면 변경계약이 들어온 순간 값이 뷰에서
    /// 사라진다 — 지워지지 않는 것과 보이는 것은 다른 문제이고, 둘 다 지켜야 한다(ADR-012).</para>
    ///
    /// <para>계약 쪽 열은 <c>v_계약_v1</c> 에 한 번만 붙인다. <c>v_통합_v1</c> 은 그 뷰를
    /// <c>k.*</c> 로 통째로 받으므로 저쪽에서 또 붙이면 <b>같은 이름의 열이 둘</b>이 된다.</para>
    /// </summary>
    private static string UserColumn(string entityType, string keyExpression, string fieldName)
    {
        var (table, baseColumn) = entityType switch
        {
            "contract" => ("contract_user_field", "contract_base"),
            "request" => ("request_user_field", "request_base"),
            _ => ("notice_user_field", "notice_base"),
        };

        return $"COALESCE((SELECT value FROM {table} u WHERE u.{baseColumn} = {keyExpression} " +
               $"AND u.field_name = '{fieldName}'), '') AS \"{fieldName}\"";
    }

    /// <summary>
    /// 사람이 고친 값이 있으면 <b>그것을</b>, 없으면 파서가 읽은 것을 낸다.
    ///
    /// <para>씌우는 자리는 표기식의 <b>바깥</b>이다. 금액에 자릿점을 찍고 날짜를 되돌린 뒤에
    /// 덮으므로, 사람이 적은 글자가 변환 없이 그대로 나간다 — 계약면의 값은 원래 "문서에
    /// 그대로 찍힐 문자열" 이라 이것이 맞다. 안쪽이 집계든 조인이든 상관하지 않는 것도
    /// 바깥에서 씌우기 때문이다.</para>
    ///
    /// <para>덮개는 <b>차수</b>에 매달린다. 계열에 매달면 1차에서 고친 금액이 2차 변경계약에
    /// 얹혀, 확인한 적 없는 숫자가 확인된 얼굴로 문서에 찍힌다(<see cref="Schema"/> V10).</para>
    ///
    /// <para><c>column_name</c> 은 우리가 여기서 적는 상수라 따옴표가 섞일 수 없다 —
    /// 사용자 열 이름과 달리 걸러 낼 것이 없다.</para>
    /// </summary>
    private static string Face(
        string entityType, string baseColumn, string seqColumn, string name, string expression) =>
        $"{Faced(entityType, baseColumn, seqColumn, name, expression)} AS \"{name}\"";

    /// <summary>
    /// <see cref="Face"/> 의 값만 — 이름을 붙이지 않는다. 다른 열이 <b>사람이 고친 뒤의</b>
    /// 값을 재료로 삼을 때 쓴다. 파서가 읽은 것을 재료로 삼으면 고친 칸과 거기서 지은 칸이
    /// 서로 다른 말을 한다.
    /// </summary>
    private static string Faced(
        string entityType, string baseColumn, string seqColumn, string name, string expression) =>
        $"COALESCE((SELECT o.value FROM field_override o WHERE o.entity_type = '{entityType}' " +
        $"AND o.base = {baseColumn} AND o.seq = {seqColumn} AND o.column_name = '{name}'), " +
        $"{expression})";

    private static string UserColumns(
        IEnumerable<UserColumnDefinition> columns, string entityType, string keyExpression)
    {
        var wanted = columns.Where(c => c.EntityType == entityType).ToList();
        if (wanted.Count == 0) return string.Empty;

        const string separator = ",\n            ";
        return separator + string.Join(separator,
            wanted.Select(c => UserColumn(entityType, keyExpression, c.FieldName)));
    }

    /// <summary>열마다 쓰는 것이라 짧아야 읽힌다 — <c>N</c> 은 공고의 덮개다.</summary>
    private static string N(string name, string expression) =>
        Face("notice", "n.notice_base", "n.seq", name, expression);

    /// <inheritdoc cref="N"/>
    private static string C(string name, string expression) =>
        Face("contract", "c.contract_base", "c.seq", name, expression);

    /// <summary>
    /// 공고 뷰가 내는 열 한 벌. <c>v_공고_v1</c> 과 <c>v_공고차수_v1</c> 이 <b>같이 쓴다</b>.
    ///
    /// <para>두 뷰의 차이는 <c>FROM</c> 절 하나뿐이다 — <c>v_공고_v1</c> 은
    /// <see cref="LatestNotice"/> 로 최신 차수만 세우고, <c>v_공고차수_v1</c> 은 문서 한 장마다
    /// 줄을 세운다. 열은 <b>한 글자도 다르지 않아야</b> 하므로 두 벌로 적지 않는다.
    /// <see cref="ContractColumns"/> 가 이미 그 값을 치르고 있다 — 두 벌이면 한쪽에만 열을
    /// 더해도 아무것도 실패하지 않고 저쪽에서 그 열만 사라진다.</para>
    ///
    /// <para>모든 비키 열을 <see cref="Face"/> 로 감싼다. 새 열을 더할 때 그 자리도 함께
    /// 감싸지 않으면 그 칸만 조용히 고칠 수 없게 된다(ADR-020).</para>
    /// </summary>
    /// <param name="noticeColumns">사람이 세운 공고 열. 뒤에 그대로 이어 붙는다.</param>
    private static string 공고열(string noticeColumns) => $"""
            -- 키 열은 감싸지 않는다. 고치면 값이 바뀌는 것이 아니라 레코드가 옮겨간다.
            n.notice_base || '-' || n.seq AS 입찰공고번호,
            n.notice_base                 AS 공고본번호,
            n.seq                         AS 차수,

            {N("공고명", Text("n.title"))},
            {N("공고종류", Text("n.notice_kind"))},
            {N("게시일시", DateText("n.posted_at"))},
            {N("입찰방식", Text("n.bid_method"))},
            {N("낙찰방법", Text("n.award_method"))},
            {N("낙찰하한율", Number("n.lowest_bid_ratio"))},
            {N("계약방법", Text("n.contract_method"))},
            {N("계약구분", Text("n.contract_kind"))},
            {N("사업예산", Money("n.project_amount"))},
            {N("배정예산", Money("n.allocated_budget"))},
            {N("추정가격", Money("n.estimated_price"))},
            {N("기초금액", Money("n.base_price"))},
            {N("분할납품", YesNo("n.partial_delivery_allowed"))},
            {N("하자담보기간", WarrantyYears)},
            {N("공고기관", Text("n.notice_agency"))},
            {N("공고담당자", Text("n.notice_officer"))},
            {N("집행관", Text("n.executive_officer"))},
            {N("지역제한", Text("n.region_restriction"))},
            {N("사전규격등록번호", Text("n.prior_spec_number"))},
            {N("개찰일시", DateText(Step("starts_at", "개찰")))},
            {N("입찰개시일시", DateText(Step("starts_at", "입찰서제출")))},
            {N("입찰마감일시", DateText(Step("ends_at", "입찰서제출")))},
            {N("등록마감일시", DateText(Step("ends_at", "자격등록")))},

            -- 세부품명이 한 가지인 공고만 채운다. 여럿이면 비우고 v_공고품목_v1 이 낸다.
            {N("수요기관", Text(SoleItem("i.demand_agency")))},
            {N("세부품명", Text(SoleItem("i.item_name")))},
            {N("세부품명번호", Text(SoleItem("i.detail_item_number")))},
            {N("수량", Text(SoleQuantity()))},
            {N("단위", Text(SoleItem("i.unit")))},
            {N("납품기한", Text(SoleItem(DeliveryDeadline("i.delivery_due", "i.delivery_days"))))},
            -- 인도조건은 물건이 아니라 납품 방식이라 세부품명이 여럿이어도 줄마다 같으면 싣는다.
            {N("인도조건", Text(UniformItem("i.delivery_terms")))},

            -- 수요기관 담당자. 공고기관 쪽 공고담당자와 다른 사람이다.
            {N("담당부서", Text(SoleContact("t.department")))},
            {N("담당자", Text(SoleContact("t.officer")))},
            {N("담당자전화", Text(SoleContact("t.phone")))},
            {N("담당자팩스", Text(SoleContact("t.fax")))},

            -- 이 공고가 대신하거나 이 공고를 대신하는 공고. 담기만 한다 — 계열의 생사를
            -- 여기서 정하지 않는다(ADR-025). 취소공고도 재공고도 여느 공고처럼 줄을 갖는다.
            {N("관련공고", Text(RelatedNotices))},

            -- 관련공고를 타고 이어 붙인 건과 그 건의 현행 공고. <b>끝에 덧붙인다</b> —
            -- 더하는 것이라 저쪽에 저장된 필드 연결이 끊기지 않는다.
            --
            -- 키 열이라 Face 로 감싸지 않는다. 고치면 값이 바뀌는 것이 아니라 이 공고가 다른
            -- 건에 속한 것처럼 보이기만 한다(Views.KeyColumns).
            {Text(공고건("n.notice_base"))} AS 공고건,
            {Text(건현행(공고건("n.notice_base"), "현행.notice_base || '-' || 현행.seq"))} AS 현행공고,

            -- 계약방법과 계약구분을 줄여 한 칸에 적는다 — 「제한(총액)」. 재료는 사람이 고친 뒤의
            -- 값이다. 둘 중 하나라도 표에 없으면 통째로 비운다: 틀린 글자는 그대로 문서에
            -- 찍히지만 빈칸은 받는 쪽이 알아챈다. 「제한()」 도, 코드 그대로도 내지 않는다.
            {N("입찰방법", 입찰방법)}{noticeColumns}
        """;

    /// <summary><see cref="공고열"/> 의 「입찰방법」 — 줄임표에 없는 값이 하나라도 있으면 빈 문자열.</summary>
    private static string 입찰방법 => $"""
        COALESCE(
                (CASE {Faced("notice", "n.notice_base", "n.seq", "계약방법", Text("n.contract_method"))}
                    WHEN '일반경쟁' THEN '일반' WHEN '제한경쟁' THEN '제한'
                    WHEN '지명경쟁' THEN '지명' WHEN '수의계약' THEN '수의' END)
                || '(' ||
                (CASE {Faced("notice", "n.notice_base", "n.seq", "계약구분", Text("n.contract_kind"))}
                    WHEN '총액계약' THEN '총액' WHEN '일반단가계약' THEN '단가'
                    WHEN '단가계약' THEN '단가' WHEN '제3자단가계약' THEN '제3자단가' END)
                || ')', '')
        """;

    /// <summary>
    /// 계약 뷰가 내는 열 한 벌. <c>v_계약_v1</c> 과 <c>v_계약차수_v1</c> 이 <b>같이 쓴다</b> —
    /// <see cref="공고열"/> 와 같은 까닭이다.
    /// </summary>
    /// <param name="contractColumns">사람이 세운 계약 열. 뒤에 그대로 이어 붙는다.</param>
    private static string 계약열(string contractColumns) => $"""
            c.contract_base || c.seq AS 계약번호,
            c.contract_base          AS 계약본번호,
            c.seq                    AS 차수,

            {C("계약건명", Text("c.title"))},
            {C("계약일자", DateText("c.contracted_on"))},
            {C("계약방법", Text("c.contract_method"))},
            {C("계약구분", Text("c.contract_kind"))},
            {C("품명", Text("c.item_name"))},
            {C("수량", Text("CAST(c.quantity AS TEXT)"))},
            {C("단위", Text("c.unit"))},
            {C("계약금액", Money("c.amount"))},
            {C("수수료", Money("c.fee"))},
            {C("지체상금률", Number("c.delay_penalty_rate"))},
            {C("하자보수보증금률", Number("c.warranty_bond_rate"))},
            {C("하자담보책임기간", Text("CASE WHEN c.warranty_years IS NOT NULL THEN c.warranty_years || '년 ' || coalesce(c.warranty_months,'0') || '개월' ELSE c.warranty_period END"))},
            {C("계약기간", Text("c.contract_period"))},
            {C("납품기한", DateText("c.delivery_due"))},
            {C("인도조건", Text("c.delivery_terms"))},
            {C("납품장소", Text("c.delivery_place"))},
            {C("분할납품", YesNo("c.partial_delivery_allowed"))},
            {C("지급방법", Text("c.payment_method"))},
            {C("수요기관", Text("c.demand_agency"))},
            {C("검사기관", Text("c.inspection_agency"))},
            {C("검수기관", Text("c.acceptance_agency"))},

            -- 상대자는 사업자등록번호로 이어 온 다른 표의 것이지만, 덮개는 이 계약의 이 칸에
            -- 걸린다 — 한 계약의 정정이 같은 상대자를 쓰는 다른 계약에 번지지 않는다.
            {C("계약상대자", Text(PartyField("name")))},
            {C("대표자", Text(PartyField("representative")))},
            {C("사업자등록번호", Text(PartyField("business_number")))},
            {C("상대자주소", Text(PartyField("address")))},
            {C("상대자전화", Text(PartyField("phone")))},
            {C("상대자팩스", Text(PartyField("fax")))}{contractColumns}
        """;

    private static string PartyField(string column) =>
        $"CASE (SELECT count(*) FROM erp_partner e WHERE e.contract_base=c.contract_base AND e.seq=c.seq) " +
        $"WHEN 0 THEN p.{column} WHEN 1 THEN (SELECT e.{column} FROM erp_partner e WHERE e.contract_base=c.contract_base AND e.seq=c.seq) ELSE '' END";

    public static IReadOnlyList<string> Definitions(IEnumerable<UserColumnDefinition> userColumns)
    {
        var columns = userColumns.ToList();
        var contractColumns = UserColumns(columns, "contract", "c.contract_base");
        var noticeColumns = UserColumns(columns, "notice", "n.notice_base");
        var requestColumns = UserColumns(columns, "request", "r.request_base");

        // 통합 v2 가 계약 뷰에서 받아 오는 열. 사람이 세운 계약 열도 함께 온다 —
        // v_계약_v1 이 그 열까지 낸 뒤이므로 이름만 이어 붙이면 된다.
        var unifiedContract = string.Join(",\n            ",
            ContractColumns
                .Concat(columns.Where(c => c.EntityType == "contract").Select(c => c.FieldName))
                .Select(name => $"COALESCE(k.\"{name}\", '') AS \"{name}\""));

        // 접수 열은 이 뷰 하나에만 쓰여 여기 둔다. 공고·계약은 차수 뷰와 나눠 쓰므로
        // 열 목록째 밖으로 나가 있다(공고열·계약열).
        static string R(string name, string expression) =>
            Face("request", "r.request_base", "r.seq", name, expression);

        return
    [
        // 통합은 계약 뷰 위에 얹히므로, 계약을 다시 짓기 전에 먼저 치운다. v2 는 접수까지
        // 얹으므로 접수 뷰보다도 앞서 치워야 한다 — 가리키는 뷰가 살아 있으면 SQLite 가 막는다.
        // 계획은 접수·공고·계약 셋을 다 가리키므로 그 셋보다 앞선다.
        //
        // v3 이 <b>맨 앞</b>이다. 계약·접수에 더해 v_공고차수_v1 까지 얹혀 이 뭉치에서 가장
        // 위에 있다. 뷰 하나가 실패하면 RefreshViews 가 던져 <b>앱이 아예 뜨지 않는다</b>.
        "DROP VIEW IF EXISTS v_통합_v3;",
        "DROP VIEW IF EXISTS v_계획_v1;",
        "DROP VIEW IF EXISTS v_통합_v2;",
        "DROP VIEW IF EXISTS v_통합_v1;",

        // 짓다 만 중간 뷰의 잔재. 예전 DB 에 남아 있으면 치운다.
        "DROP VIEW IF EXISTS v_계약_기본;",
        "DROP VIEW IF EXISTS _계약_기본;",

        $"""
        DROP VIEW IF EXISTS v_공고_v1;
        CREATE VIEW v_공고_v1 AS
        SELECT
        {공고열(noticeColumns)}
        FROM notice n
        {LatestNotice};
        """,

        // ── 공고 차수 ────────────────────────────────────────────────
        //
        // 줄 하나가 <b>공고 문서 한 장</b>이다. v_공고_v1 과 열이 한 글자도 다르지 않고
        // (같은 공고열 하나를 쓴다) LatestNotice 조인만 없다 — 최신 차수로 좁히지 않으므로
        // 취소공고도, 변경 전 차수도 제 줄로 선다.
        //
        // 「공고건」·「현행공고」는 여기서도 그 <b>건</b>의 것이다. 그래서 이 뷰는 "이 건에
        // 무엇 무엇이 있었고 지금 무엇이 서 있는가" 를 한 표로 낸다.
        //
        // <b>읽기 전용이다</b>(Views.ReadOnly). Store.FaceView 가 공고 뷰를 v_공고_v1 하나로
        // 알아, 옛 차수의 칸을 여기서 고치면 덮개의 original 을 찾지 못해 빈 문자열로 박힌다.
        $"""
        DROP VIEW IF EXISTS v_공고차수_v1;
        CREATE VIEW v_공고차수_v1 AS
        SELECT
        {공고열(noticeColumns)}
        FROM notice n;
        """,

        // 공고 품목. v_품목_v1 이 계약에 대해 하는 것을 공고에 대해 한다 — 줄 하나가 품목 한 줄이다.
        // 수요기관이 여럿인 공고에서는 줄마다 수요기관이 다르므로, 그 기관의 담당자를 이름으로 이어 붙인다.
        $"""
        DROP VIEW IF EXISTS v_공고품목_v1;
        CREATE VIEW v_공고품목_v1 AS
        SELECT
            n.notice_base || '-' || n.seq         AS 입찰공고번호,
            {Text("n.title")}                     AS 공고명,
            i.line_no                             AS 순번,
            {Text("i.demand_agency")}             AS 수요기관,
            {Text("i.item_name")}                 AS 세부품명,
            {Text("i.detail_item_number")}        AS 세부품명번호,
            {Text("i.item_id_number")}            AS 물품식별번호,
            {Text("i.specification")}             AS 규격,
            {Text("CAST(i.quantity AS TEXT)")}    AS 수량,
            {Text("i.unit")}                      AS 단위,
            {Money("i.unit_price")}               AS 추정단가,
            {DeliveryDeadline("i.delivery_due", "i.delivery_days")} AS 납품기한,
            {Text("i.delivery_place")}            AS 납품장소,
            {Text("i.delivery_terms")}            AS 인도조건,
            {Text("t.department")}                AS 담당부서,
            {Text("t.officer")}                   AS 담당자,
            {Text("t.phone")}                     AS 담당자전화,
            {Text("t.fax")}                       AS 담당자팩스
        FROM notice_item i
        JOIN notice n ON n.notice_base = i.notice_base AND n.seq = i.seq
        {LatestNotice}
        LEFT JOIN notice_officer_contact t
               ON t.notice_base = i.notice_base AND t.seq = i.seq
              AND t.demand_agency = i.demand_agency;
        """,

        $"""
        DROP VIEW IF EXISTS _계약_기본;
        DROP VIEW IF EXISTS v_계약_v1;
        CREATE VIEW v_계약_v1 AS
        SELECT
        {계약열(contractColumns)}
        FROM contract c
        {LatestContract}
        LEFT JOIN counterparty p ON c.counterparty_number = p.business_number;
        """,

        // ── 계약 차수 ────────────────────────────────────────────────
        //
        // 줄 하나가 <b>계약 문서 한 장</b>이다. v_계약_v1 과 열이 같고 LatestContract 만 없다 —
        // 변경계약이 들어와도 앞차수가 제 줄로 남는다. v_공고차수_v1 과 같은 까닭으로
        // <b>읽기 전용</b>이다.
        $"""
        DROP VIEW IF EXISTS v_계약차수_v1;
        CREATE VIEW v_계약차수_v1 AS
        SELECT
        {계약열(contractColumns)}
        FROM contract c
        LEFT JOIN counterparty p ON c.counterparty_number = p.business_number;
        """,

        $"""
        DROP VIEW IF EXISTS v_품목_v1;
        CREATE VIEW v_품목_v1 AS
        SELECT
            c.contract_base || c.seq              AS 계약번호,
            {Text("c.title")}                     AS 계약건명,
            i.line_no                             AS 순번,
            {Text("i.item_name")}                 AS 품명,
            {Text("i.specification")}             AS 규격,
            {Text("i.classification_number")}     AS 물품분류번호,
            {Text("i.item_id_number")}            AS 물품식별번호,
            {Text("CAST(i.quantity AS TEXT)")}    AS 수량,
            {Text("i.unit")}                      AS 단위,
            {Money("i.unit_price")}               AS 단가,
            {Money("i.amount")}                   AS 금액,
            {Text("i.demand_agency")}             AS 수요기관,
            {DateText("i.delivery_due")}          AS 납품기한,
            {Text("i.delivery_terms")}            AS 인도조건
        FROM contract_item i
        JOIN contract c ON c.contract_base = i.contract_base AND c.seq = i.seq
        {LatestContract};
        """,

        // 접수. 줄 하나가 접수서 하나(최신 차수)다 — 공고와 1:1 이라 그 줄이 조달 건 하나를 가리킨다.
        // 모든 열을 Face 로 감싼다: 새 열을 더할 때 그 자리도 함께 감싸지 않으면 그 칸만
        // 조용히 고칠 수 없게 된다(ADR-020).
        $"""
        DROP VIEW IF EXISTS v_접수_v1;
        CREATE VIEW v_접수_v1 AS
        SELECT
            -- 키 열은 감싸지 않는다. 고치면 값이 바뀌는 것이 아니라 레코드가 옮겨간다.
            -- 이 두 이름은 대표조달요구번호·대표조달요구본번호에서 제자리로 바꿨다(ADR-027,
            -- ADR-014 의 두 번째 예외). 키가 접수번호가 되어 옛 이름이 거짓이 되었다.
            r.request_base || '-' || r.seq AS 접수번호,
            r.request_base                 AS 접수본번호,
            r.seq                          AS 차수,

            {R("조달요구번호", Text(RequestNumbers))},
            {R("품목수", Text($"CAST((SELECT COUNT(*) FROM request_item x WHERE x.request_base = r.request_base AND x.seq = r.seq) AS TEXT)"))},

            {R("요청명", Text("r.title"))},
            {R("접수일자", DateText("r.received_on"))},
            {R("업무구분", Text("r.business_kind"))},
            {R("계약법구분", Text("r.contract_law"))},
            {R("계약방법", Text("r.contract_method"))},
            {R("계약유형", Text("r.contract_kind"))},
            {R("낙찰방법", Text("r.award_method"))},
            {R("회계구분", Text("r.accounting_kind"))},
            {R("지급방법", Text("r.payment_method"))},

            {R("품대", Money("r.goods_amount"))},
            {R("수수료", Money("r.fee"))},
            {R("부가가치세액", Money("r.vat"))},
            {R("예산금액", Money("r.budget_amount"))},

            {R("외국산여부", YesNoWord("r.foreign_allowed"))},
            {R("요청구분", Text("r.request_kind"))},
            {R("공개여부", Text("r.disclosure"))},
            {R("전담관", Text("r.executive_officer"))},
            {R("담당과", Text("r.department"))},
            {R("담당자", Text("r.officer"))},
            {R("선급금선고지", YesNoWord("r.advance_notice"))},
            {R("선급금지급가능", YesNoWord("r.advance_payment"))},
            {R("기타사항", Text("r.remarks"))},

            {R("수요기관", Text("r.demand_agency"))},
            {R("수요기관코드", Text("r.demand_agency_code"))},
            {R("기관담당자", Text("r.agency_officer"))},
            {R("기관담당자전화", Text("r.agency_phone"))},
            {R("기관담당자팩스", Text("r.agency_fax"))},

            -- 세부품명이 한 가지인 접수만 채운다. 여럿이면 비우고 v_접수품목_v1 이 낸다.
            -- v_공고_v1 의 규율을 그대로 따른다 — 하나로 정할 수 없는 것을 하나인 양 내지 않는다.
            {R("세부품명", Text(SoleRequestItem("i.item_name")))},
            {R("세부품명번호", Text(SoleRequestItem("i.detail_item_number")))},
            {R("수량", Text(SoleRequestQuantity()))},
            {R("단위", Text(SoleRequestItem("i.unit")))},
            {R("인도조건", Text(UniformRequestItem("i.delivery_terms")))},
            {R("납품기한", Text(SoleRequestItem(RequestDeadline("i.delivery_due", "i.delivery_days"))))},
            -- 재고번호도 같은 규칙이지만 세부품명 옆이 아니라 끝에 붙인다 — 계약면은
            -- 늘리기만 하는 자리라, 사이에 끼우면 뒤 열의 엑셀 자리가 한 칸씩 밀린다.
            {R("재고번호", Text(SoleRequestItem("i.stock_number")))}{requestColumns}
        FROM request r
        {LatestRequest};
        """,

        // 접수 품목. 줄 하나가 조달요구 하나다.
        // 「입찰공고번호」·「공고순번」은 request_item_link 에서 온다 — 어느 조달요구가 어느
        // 공고 품목이 되었는지, 수량·단가로 지은 짝의 결과다. 이어지지 않았으면 빈 문자열이다.
        $"""
        DROP VIEW IF EXISTS v_접수품목_v1;
        CREATE VIEW v_접수품목_v1 AS
        SELECT
            r.request_base || '-' || r.seq        AS 접수번호,
            {Text("r.title")}                     AS 요청명,
            i.line_no                             AS 순번,
            {Text("i.request_number")}            AS 조달요구번호,
            {Text("i.plan_year")}                 AS 국방조달계획년도,
            {Text("i.change_seq")}                AS 변경차수,
            {Text("i.item_name")}                 AS 세부품명,
            {Text("i.detail_item_number")}        AS 세부품명번호,
            {Text("i.item_id_number")}            AS 물품식별번호,
            {Text("i.specification")}             AS 규격,
            {Text("i.inspection_kind")}           AS 검사형태,
            {YesNoWord("i.cancelled")}            AS 취소여부,
            {Text("CAST(i.quantity AS TEXT)")}    AS 수량,
            {Text("i.unit")}                      AS 단위,
            {Money("i.unit_price")}               AS 단가,
            {Money("i.amount")}                   AS 금액,
            {Text("CAST(i.pages AS TEXT)")}       AS 면수,
            {Text("i.delivery_terms")}            AS 인도조건,
            {Text("i.delivery_days")}             AS 납품일수,
            {DateText("i.delivery_due")}          AS 납품기한,
            {Text("i.delivery_place")}            AS 납품장소,
            {Text("i.stock_number")}              AS 재고번호,
            {Text("i.spec_number")}               AS 규격번호,
            {Text("i.expense_request_number")}    AS 지출요청번호,
            COALESCE(k.notice_base || '-' || k.notice_seq, '') AS 입찰공고번호,
            {Text("CAST(k.notice_line_no AS TEXT)")}           AS 공고순번
        FROM request_item i
        JOIN request r ON r.request_base = i.request_base AND r.seq = i.seq
        {LatestRequest}
        LEFT JOIN request_item_link k
               ON k.request_base = i.request_base AND k.request_seq = i.seq
              AND k.line_no = i.line_no;
        """,

        // 조인 시트. 계약에 공고를 붙여 낸다. 옆의 공고·계약·품목이 쌓인 그대로를 내는 것과 달리
        // 이쪽은 링크가 확정돼야 공고 열이 채워진다 — 역할이 달라서 둘 다 둔다.
        //
        // 한 공고에 계약이 여럿 달릴 수 있다(분할 낙찰·수요기관 복수). 그럴 때 같은 공고 열이
        // 여러 줄에 반복되는데, 줄 하나가 계약 하나라는 약속이 지켜지므로 그게 옳은 모습이다.
        //
        // 계약은 언제나 공고 뒤에 오고, 둘을 잇는 것은 사람이 확정한 project_link 뿐이다
        // (계약서에 공고번호가 찍히지 않는다). 링크 전에는 공고 열이 빈 문자열이다.
        $"""
        DROP VIEW IF EXISTS v_통합_v1;
        CREATE VIEW v_통합_v1 AS
        SELECT
            k.*,
            COALESCE(g.입찰공고번호, '')     AS 입찰공고번호,
            COALESCE(g.공고명, '')           AS 공고명,
            COALESCE(g.공고종류, '')         AS 공고종류,
            COALESCE(g.게시일시, '')         AS 게시일시,
            COALESCE(g.입찰방식, '')         AS 입찰방식,
            COALESCE(g.낙찰방법, '')         AS 낙찰방법,
            COALESCE(g.낙찰하한율, '')       AS 낙찰하한율,
            COALESCE(g.사업예산, '')         AS 사업예산,
            COALESCE(g.배정예산, '')         AS 배정예산,
            COALESCE(g.추정가격, '')         AS 추정가격,
            COALESCE(g.기초금액, '')         AS 기초금액,
            COALESCE(g.개찰일시, '')         AS 개찰일시,
            COALESCE(g.입찰개시일시, '')     AS 입찰개시일시,
            COALESCE(g.입찰마감일시, '')     AS 입찰마감일시,
            COALESCE(g.등록마감일시, '')     AS 등록마감일시,
            COALESCE(g.공고담당자, '')       AS 공고담당자,
            COALESCE(g.사전규격등록번호, '') AS 사전규격등록번호
        FROM v_계약_v1 k
        LEFT JOIN project_link l ON l.contract_base = k.계약본번호
        -- 링크가 가리키는 것은 건이라 공고 한 장을 골라야 한다. 그 건의 <b>현행 공고</b>다 —
        -- 재공고 건에서 취소된 원공고의 값이 계약 옆에 서지 않게(ADR-025 를 넓힌다).
        LEFT JOIN v_공고_v1 g    ON g.공고본번호 = {건현행("l.notice_group", "현행.notice_base")};
        """,

        // v_통합_v1 위에 접수 아홉 열을 더한 것. v1 은 손대지 않고 나란히 세운다 —
        // 열 이름은 소비자에 저장된 필드 연결의 열쇠라, 고치면 저쪽 설정이 조용히 끊긴다(ADR-014).
        //
        // 「수수료」는 계약 쪽에 이미 있어 「접수수수료」로 낸다. 같은 이름이 둘이면 SQLite 가
        // 오류 없이 뒤엣것을 「수수료:1」로 바꿔, 표가 이상해진 뒤에야 알게 된다.
        //
        // ── 줄 하나는 계약이 아니라 조달 건이다 ──────────────────────────
        //
        // 예전에는 v_계약_v1 을 줄기로 세워 <b>계약이 있는 것만</b> 나왔다. 그래서 같은 자료를
        // 구조 보기와 표 보기가 다르게 셌다 — 접수만 들어온 건, 공고까지만 온 건이 구조에는
        // 서 있는데 표에는 없었다. 무엇이 아직 안 들어왔는지는 표에서도 보여야 한다.
        //
        // 그래서 접수·공고·계약 본번호의 짝(사슬)을 먼저 세우고 거기에 세 뷰를 건다.
        // Outline 이 세우는 사슬과 <b>같은 규칙</b>이다:
        //   · 공고가 있는 것 — 접수는 되짚어(1:1) 붙이고, 계약은 매달린 것마다 한 줄
        //   · 공고가 아직 없는 접수 — 한 줄
        //   · 어디에도 매달리지 못한 계약 — 한 줄
        // 계약이 여럿인 공고(분할 낙찰·수요기관 복수)에서만 줄이 갈라지므로, 흔한 1:1:1 은
        // 그대로 한 줄이다. 접수:건이 1:1(request_link.notice_group UNIQUE)이라 접수 조인은
        // 줄을 늘리지 않는다.
        //
        // 계약 열을 k.* 로 받지 못하는 것은 이 때문이다 — 계약이 없는 줄에서 LEFT JOIN 이
        // NULL 을 내는데 계약면의 빈 값은 빈 문자열이라, 열마다 감싸야 한다(ContractColumns).
        $"""
        DROP VIEW IF EXISTS v_통합_v2;
        CREATE VIEW v_통합_v2 AS
        WITH 사슬(접수, 공고, 계약) AS (
            -- 공고가 선 것. 줄기가 본번호가 아니라 <b>건</b>이라, 취소 후 재공고로 본번호가
            -- 갈렸던 것이 여기서 한 줄로 합쳐진다 — 줄 하나는 조달 건이라는 이 뷰의 정의가
            -- 그것을 요구한다(ADR-022). 계약이 없으면 계약 자리가 NULL 인 채로 한 줄이 남는다.
            --
            -- 건 표를 그대로 훑지 않고 <b>공고가 실제로 서 있는 건</b>만 세우는 것은, 계열만
            -- 남고 문서가 없는 자리(취합본에서 생길 수 있다)가 빈 줄로 서지 않게 하려는 것이다.
            SELECT rl.request_base, 건.group_base, pl.contract_base
            FROM (SELECT DISTINCT s.group_base
                  FROM notice_series s
                  JOIN notice n ON n.notice_base = s.notice_base) 건
            LEFT JOIN request_link rl ON rl.notice_group = 건.group_base
            LEFT JOIN project_link pl ON pl.notice_group = 건.group_base

            UNION ALL

            -- 공고가 아직 없는 접수. 세우지 않으면 넣은 사람이 넣지 않은 줄 안다.
            -- 링크가 있는지가 아니라 <b>그 건에 공고가 실제로 서 있는지</b>를 본다 — 링크만 남고
            -- 그 건에 공고 줄이 하나도 없으면 위 갈래가 그 접수를 집지 않아, 접수가 통째로 사라진다.
            SELECT r.request_base, NULL, NULL
            FROM (SELECT DISTINCT request_base FROM request) r
            WHERE NOT EXISTS (
                SELECT 1 FROM request_link rl
                JOIN notice_series s ON s.group_base = rl.notice_group
                JOIN notice n2 ON n2.notice_base = s.notice_base
                WHERE rl.request_base = r.request_base)

            UNION ALL

            -- 어디에도 매달리지 못한 계약. 계약서만 먼저 들어온 흔한 경우다.
            SELECT NULL, NULL, c.contract_base
            FROM (SELECT DISTINCT contract_base FROM contract) c
            WHERE NOT EXISTS (SELECT 1 FROM project_link pl WHERE pl.contract_base = c.contract_base)
        )
        SELECT
            {unifiedContract},
            COALESCE(g.입찰공고번호, '')     AS 입찰공고번호,
            COALESCE(g.공고명, '')           AS 공고명,
            COALESCE(g.공고종류, '')         AS 공고종류,
            COALESCE(g.게시일시, '')         AS 게시일시,
            COALESCE(g.입찰방식, '')         AS 입찰방식,
            COALESCE(g.낙찰방법, '')         AS 낙찰방법,
            COALESCE(g.낙찰하한율, '')       AS 낙찰하한율,
            COALESCE(g.사업예산, '')         AS 사업예산,
            COALESCE(g.배정예산, '')         AS 배정예산,
            COALESCE(g.추정가격, '')         AS 추정가격,
            COALESCE(g.기초금액, '')         AS 기초금액,
            COALESCE(g.개찰일시, '')         AS 개찰일시,
            COALESCE(g.입찰개시일시, '')     AS 입찰개시일시,
            COALESCE(g.입찰마감일시, '')     AS 입찰마감일시,
            COALESCE(g.등록마감일시, '')     AS 등록마감일시,
            COALESCE(g.공고담당자, '')       AS 공고담당자,
            COALESCE(g.사전규격등록번호, '') AS 사전규격등록번호,
            COALESCE(r.접수번호, '') AS 접수번호,
            COALESCE(r.조달요구번호, '')     AS 조달요구번호,
            COALESCE(r.요청명, '')           AS 요청명,
            COALESCE(r.접수일자, '')         AS 접수일자,
            COALESCE(r.품대, '')             AS 품대,
            COALESCE(r.수수료, '')           AS 접수수수료,
            COALESCE(r.예산금액, '')         AS 예산금액,
            COALESCE(r.기관담당자, '')       AS 기관담당자,
            COALESCE(r.기관담당자전화, '')   AS 기관담당자전화
        FROM 사슬 s
        LEFT JOIN v_계약_v1 k ON k.계약본번호 = s.계약
        -- 사슬의 공고 자리는 건 이름이다. 공고 열은 그 건의 현행 공고에서 낸다 — 건마다
        -- 공고가 여럿일 수 있으므로 여기서 하나로 좁히지 않으면 줄이 갈라진다.
        LEFT JOIN v_공고_v1 g ON g.공고본번호 = {건현행("s.공고", "현행.notice_base")}
        LEFT JOIN v_접수_v1 r ON r.접수본번호 = s.접수;
        """,

        // ── 통합 v3 — 차수를 편 것 ───────────────────────────────────
        //
        // 줄 하나가 <b>공고건 × 공고 레코드</b>다. v2 가 건마다 현행 공고 한 장으로 접어 낸
        // 자리를, 여기서는 그 건이 가진 공고를 한 장씩 편다 — 취소된 원공고와 재공고가
        // 나란히 서고, 변경공고는 앞차수와 함께 선다. 접수·계약 열은 그 줄들에 되풀이된다.
        //
        // 열은 v2 의 것을 그대로 두고 끝에 「공고건」·「현행공고」 둘만 더한다. 공고 열은
        // v_공고_v1 이 아니라 <b>v_공고차수_v1</b> 에서 받는다 — 저쪽은 최신 차수만 내므로
        // 옛 차수 줄의 공고 열이 통째로 비어 버린다.
        //
        // 사슬의 규칙은 v2 와 <b>같다</b>. 공고가 아직 없는 접수와 어디에도 매달리지 못한
        // 계약은 여기서도 제 줄로 선다(공고 열이 빈 채로) — 무엇이 아직 안 들어왔는지는
        // 차수를 편 표에서도 보여야 한다.
        //
        // <b>읽기 전용이다</b>(Views.ReadOnly). 줄이 공고라 EntityTypeOf 도 notice 로 따로
        // 적어 두었다 — 이름에 「공고」가 없어 규칙으로는 잡히지 않는다.
        $"""
        DROP VIEW IF EXISTS v_통합_v3;
        CREATE VIEW v_통합_v3 AS
        WITH 사슬(접수, 공고, 계약) AS (
            -- 공고 문서 한 장이 한 줄이다. 건에 매달린 접수·계약을 그 줄들에 되풀이해 붙인다.
            -- 접수:건이 1:1(request_link.notice_group UNIQUE)이라 접수 조인은 줄을 늘리지 않고,
            -- 계약이 여럿인 건에서만 공고 한 장이 계약 수만큼 갈라진다(v2 와 같다).
            SELECT rl.request_base, n.notice_base || '-' || n.seq, pl.contract_base
            FROM notice n
            JOIN notice_series 계열 ON 계열.notice_base = n.notice_base
            LEFT JOIN request_link rl ON rl.notice_group = 계열.group_base
            LEFT JOIN project_link pl ON pl.notice_group = 계열.group_base

            UNION ALL

            -- 공고가 아직 없는 접수. 링크가 있는지가 아니라 <b>그 건에 공고가 실제로 서 있는지</b>
            -- 를 본다 — v_통합_v2 의 같은 갈래와 한 글자도 다르지 않다.
            SELECT r.request_base, NULL, NULL
            FROM (SELECT DISTINCT request_base FROM request) r
            WHERE NOT EXISTS (
                SELECT 1 FROM request_link rl
                JOIN notice_series s ON s.group_base = rl.notice_group
                JOIN notice n2 ON n2.notice_base = s.notice_base
                WHERE rl.request_base = r.request_base)

            UNION ALL

            -- 어디에도 매달리지 못한 계약.
            SELECT NULL, NULL, c.contract_base
            FROM (SELECT DISTINCT contract_base FROM contract) c
            WHERE NOT EXISTS (SELECT 1 FROM project_link pl WHERE pl.contract_base = c.contract_base)
        )
        SELECT
            {unifiedContract},
            COALESCE(g.입찰공고번호, '')     AS 입찰공고번호,
            COALESCE(g.공고명, '')           AS 공고명,
            COALESCE(g.공고종류, '')         AS 공고종류,
            COALESCE(g.게시일시, '')         AS 게시일시,
            COALESCE(g.입찰방식, '')         AS 입찰방식,
            COALESCE(g.낙찰방법, '')         AS 낙찰방법,
            COALESCE(g.낙찰하한율, '')       AS 낙찰하한율,
            COALESCE(g.사업예산, '')         AS 사업예산,
            COALESCE(g.배정예산, '')         AS 배정예산,
            COALESCE(g.추정가격, '')         AS 추정가격,
            COALESCE(g.기초금액, '')         AS 기초금액,
            COALESCE(g.개찰일시, '')         AS 개찰일시,
            COALESCE(g.입찰개시일시, '')     AS 입찰개시일시,
            COALESCE(g.입찰마감일시, '')     AS 입찰마감일시,
            COALESCE(g.등록마감일시, '')     AS 등록마감일시,
            COALESCE(g.공고담당자, '')       AS 공고담당자,
            COALESCE(g.사전규격등록번호, '') AS 사전규격등록번호,
            COALESCE(r.접수번호, '') AS 접수번호,
            COALESCE(r.조달요구번호, '')     AS 조달요구번호,
            COALESCE(r.요청명, '')           AS 요청명,
            COALESCE(r.접수일자, '')         AS 접수일자,
            COALESCE(r.품대, '')             AS 품대,
            COALESCE(r.수수료, '')           AS 접수수수료,
            COALESCE(r.예산금액, '')         AS 예산금액,
            COALESCE(r.기관담당자, '')       AS 기관담당자,
            COALESCE(r.기관담당자전화, '')   AS 기관담당자전화,

            -- 끝에 더한 둘. 어느 건의 줄이고, 그 건에서 지금 서 있는 것이 무엇인가 —
            -- 옛 차수 줄에서도 「현행공고」는 그 건의 현행이라, 이 줄이 지나간 것인지가 보인다.
            COALESCE(g.공고건, '')           AS 공고건,
            COALESCE(g.현행공고, '')         AS 현행공고
        FROM 사슬 s
        LEFT JOIN v_계약_v1 k     ON k.계약본번호 = s.계약
        -- 사슬의 공고 자리는 건이 아니라 <b>공고 한 장</b>이라 차수까지 붙은 번호로 맞춘다.
        LEFT JOIN v_공고차수_v1 g ON g.입찰공고번호 = s.공고
        LEFT JOIN v_접수_v1 r     ON r.접수본번호 = s.접수;
        """,

        // ── 계획 ─────────────────────────────────────────────────────
        //
        // 줄기는 plan 이다. 줄 하나가 plan 한 행(조달요구번호 하나)이고, 계획에 없는
        // 조달요구번호의 접수는 여기 서지 않는다 — 그것은 v_통합_v2 가 낸다. 이 뷰는 분모다.
        //
        // 「줄이 갈라지지 않는 것」이 이 뷰의 약속이다. 한 공고에 계약이 여럿 달릴 수 있고
        // (분할 낙찰·수요기관 복수) 그럴 때 줄을 가르면 계획 건수가 부풀어 센다. 그래서
        // 이어 오는 것은 모두 본번호마다 한 줄인 표로 미리 접어 두고 건다(PlanFolds).
        // 조인마다 왼쪽이 늘어날 수 없음을 확인해 두었다:
        //   접수짝 GROUP BY 번호 · 최신접수 본번호당 한 줄 · request_link.request_base 는 PK
        //   · 최신공고 본번호당 한 줄 · 계약묶음 GROUP BY 공고본번호 · 최신계약 본번호당 한 줄
        //
        // 계획 자신의 값(조달요구번호 … 연락처)은 원본 표에서 낸다 — plan 은 접수·공고·계약과
        // 나란한 개체가 아니라 field_override 의 entity_type 에 자리가 없고, 계획 탭은 읽기
        // 전용이다. 계획의 값을 고치는 길은 엑셀을 고쳐 다시 넣는 것 하나다(덮어쓴다).
        //
        // 그러나 이어 온 여덟 열은 계획의 값이 아니라 접수·공고·계약의 값이라 덮개 자리가
        // 있다. 그래서 원본 표가 아니라 v_접수_v1·v_공고_v1·v_계약_v1 에서 받아 온다 —
        // v_통합_v1·v2 가 하는 것과 같다. 원본에서 끌면 계약 탭에서 고친 계약금액이 여기에는
        // 옛 값으로 떠서, 같은 자료를 두 화면이 다르게 말한다. 덮개는 저쪽 뷰가 이미 한 번
        // 씌웠으므로 여기서 또 씌우지 않는다(v_통합_v1 이 k.* 를 그대로 받는 것과 같은 까닭).
        $"""
        DROP VIEW IF EXISTS v_계획_v1;
        CREATE VIEW v_계획_v1 AS
        WITH {PlanFolds}
        SELECT
            -- 계획 자신의 열은 표본(「조달계획 (양식 표본).xlsx」)의 머리글 그대로,
            -- 표본의 차례 그대로 낸다(ADR-023 개정, 스키마 V13).
            {Text("p.request_number")}          AS 조달요구번호,
            {Text("p.stock_number")}            AS 재고번호,
            {Text("p.item_name")}               AS 품명,
            {Text("p.currency")}                AS 화폐구분,
            {Text("p.unit")}                    AS 단위,
            {Text("CAST(p.quantity AS TEXT)")}  AS 지시수량,
            {Text("p.requesting_unit")}         AS 요청부대부서명,
            {Text("p.requesting_officer")}      AS 요청부대담당자,
            {Text("p.requesting_phone")}        AS 요청부대사용자전화번호,
            {Text("p.contract_department")}     AS 계약부서,
            {Text("p.officer")}                 AS 담당자,
            {Text("p.contact")}                 AS 연락처,

            -- 어디까지 왔는가. <b>있는 것을 적는 것이지 판정이 아니다</b>(ADR-016) —
            -- 늦었다·빠졌다를 기계가 가리지 않는다. 계약이 달렸으면 「계약」, 공고까지
            -- 이어졌으면 「공고」, 접수가 잡혔으면 「접수」, 계획에만 있으면 「미착수」다.
            --
            -- 공고는 링크가 아니라 <b>그 공고가 실제로 서 있는지</b>를 본다(g 는 v_공고_v1).
            -- 링크만 남고 공고 줄이 없으면 「공고」라고 적을 것이 없다.
            CASE
                WHEN COALESCE(m.건수, 0) > 0         THEN '계약'
                WHEN g.공고본번호 IS NOT NULL        THEN '공고'
                WHEN r.접수본번호 IS NOT NULL THEN '접수'
                ELSE '미착수'
            END                                 AS 단계,

            COALESCE(r.접수번호, '')    AS 접수번호,
            COALESCE(r.접수일자, '')            AS 접수일자,
            COALESCE(g.입찰공고번호, '')        AS 입찰공고번호,
            COALESCE(g.게시일시, '')            AS 게시일시,
            COALESCE(g.개찰일시, '')            AS 개찰일시,

            -- 언제나 채운다. v_접수_v1 의 「품목수」와 같은 꼴이라 없으면 0 이다.
            {Text("CAST(COALESCE(m.건수, 0) AS TEXT)")} AS 계약건수,

            -- 계약이 하나일 때만. 여럿이면 비우고 계약건수가 몇인지만 알린다.
            {SoleContract("COALESCE(k.계약번호, '')")}  AS 계약번호,
            {SoleContract("COALESCE(k.계약일자, '')")}  AS 계약일자,
            {SoleContract("COALESCE(k.계약금액, '')")}  AS 계약금액
        FROM plan p
        LEFT JOIN 접수짝 q         ON q.번호 = p.request_number
        LEFT JOIN v_접수_v1 r      ON r.접수본번호 = q.본번호
        LEFT JOIN request_link rl  ON rl.request_base = q.본번호
        -- 건마다 공고가 여럿일 수 있다(취소·재공고). <b>현행 하나로 좁히지 않으면</b> 여기서
        -- 줄이 갈라져 계획 건수가 부풀어 세어지고, 분모로 쓰려고 세운 뷰가 분모 노릇을 못 한다.
        LEFT JOIN v_공고_v1 g      ON g.공고본번호 = {건현행("rl.notice_group", "현행.notice_base")}
        LEFT JOIN 계약묶음 m       ON m.공고건 = rl.notice_group
        LEFT JOIN v_계약_v1 k      ON k.계약본번호 = m.본번호;
        """,
        """
        DROP VIEW IF EXISTS v_ERP원천_v1;
        CREATE VIEW v_ERP원천_v1 AS
        SELECT s.entity_type AS 자료종류, s.entity_base AS 내부키, s.entity_seq AS 차수,
          coalesce(json_extract(r.value,'$.origins.' || f.key),json_extract(s.document_json,'$.profile')) AS 수집프로필,
          json_extract(r.value,'$.table') AS 원천표, json_extract(r.value,'$.sourceKey') AS 원천행키,
          CAST(f.key AS TEXT) AS 필드, CAST(coalesce(f.value,'') AS TEXT) AS 수집값, s.mapping_revision AS 매핑판, s.updated_at AS 수집시각
        FROM erp_source s, json_each(s.document_json,'$.rows') r, json_each(r.value,'$.values') f;
        DROP VIEW IF EXISTS v_계약업체_v1;
        CREATE VIEW v_계약업체_v1 AS
        SELECT contract_base || seq AS 계약번호, CAST(line_no AS TEXT) AS 순번,
          coalesce(source_id,'') AS 원천업체번호, coalesce(name,'') AS 상호,
          coalesce(business_number,'') AS 사업자등록번호, coalesce(representative,'') AS 대표자,
          coalesce(share_rate,'') AS 계약지분율, coalesce(share_amount,'') AS 계약지분금액
        FROM erp_partner;
        DROP VIEW IF EXISTS v_ERP접수_v1;
        CREATE VIEW v_ERP접수_v1 AS
        SELECT r.request_base AS 내부키,r.seq AS 접수차수,
          coalesce((SELECT min(ref_request_base) FROM request_item i WHERE i.request_base=r.request_base AND i.seq=r.seq),'') AS ERP접수번호,
          coalesce((SELECT min(request_number) FROM request_item i WHERE i.request_base=r.request_base AND i.seq=r.seq),'') AS 대표조달요구본번호,
          coalesce(r.title,'') AS 요청명,coalesce(r.received_on,'') AS 접수일자
        FROM request r WHERE EXISTS(SELECT 1 FROM erp_source s WHERE s.entity_type='request' AND s.entity_base=r.request_base AND s.entity_seq=r.seq);
        """,
    ];
    }
}

/// <summary>사람이 채우는 열 하나의 정의.</summary>
/// <param name="Kind">text · choice · date · number. 편집 화면이 무엇을 띄울지 정한다.</param>
/// <param name="Options">choice 일 때 고를 값들. 세로줄로 나눈다.</param>
/// <param name="SortOrder">SQLite INTEGER 가 64비트라 long 이다.</param>
public sealed record UserColumnDefinition(
    string EntityType,
    string FieldName,
    string Kind,
    string? Options,
    long SortOrder)
{
    public IReadOnlyList<string> Choices =>
        Options is null ? [] : [.. Options.Split('|', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries)];
}

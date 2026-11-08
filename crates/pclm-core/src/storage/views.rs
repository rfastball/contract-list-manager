//! 계약면 뷰(C# `Storage/Views.cs`) — 소비자에게 내보이는 계약면. 이 뷰들이 **다른 프로그램과의 유일한 약속**이다.
//!
//! **SQL 글이 C# 과 바이트까지 같아야 한다**(`sqlite_master.sql` — 계약면 박제와 동등성 계기가 견준다). 그래서 C# 의
//! 원시 문자열을 **고쳐 쓰지 않고 그대로** 둔다: 들여쓰기까지 C# 원본의 것이고, 걷는 것은 [`raw_with`] 가 C# 처럼 한다.
//! 자리(`{Text("n.title")}`)도 C# 의 글 그대로다 — [`eval`] 이 그 꼴의 부름(이 모듈의 표기 함수와 상수)을 푼다. 손으로
//! 옮겨 적은 글이 한 칸 어긋나면 아무것도 실패하지 않고 박제만 갈리므로, 옮겨 적을 자리를 남기지 않는 쪽을 골랐다.
//!
//! 뷰를 짓는 차례(`Database.Refresh`)는 [`crate::storage::database::refresh`] 에 있다.

use std::cell::RefCell;
use std::sync::OnceLock;

use crate::dict::Dict;
use crate::error::{Error, Result};
use crate::storage::flow_compiler::{self, SELF};
use crate::storage::user_view_builder::UserViewFrame;
use crate::storage::{computed_column, notice_groups};
use crate::text::{cmp_ordinal, raw, raw_with, replace_line_endings};

/// 깨진 가공열 — (종류, 이름) → 까닭. C# `IReadOnlyDictionary<(string Entity, string Name), string>`.
pub type BrokenColumns = Dict<(String, String), String>;

/// `Views.Names` — 계약면 전부. 다리가 열어 줄 표를 가리는 허용목록이자 뷰 정의 목록이다. 차례는 먼저 있던 것을
/// 앞에 둔다(`contract/views.txt` 가 이 차례로 박제되어 있다).
pub const NAMES: &[&str] = &[
    "v_공고",
    "v_공고품목",
    "v_계약",
    "v_계약품목",
    "v_접수",
    "v_접수품목",
    "v_통합",
    "v_계획",
    // 차수를 펴서 내는 것들(스키마 V15). 끝에 붙인다.
    "v_공고차수",
    "v_통합차수",
    "v_계약차수",
    "v_ERP원천",
    "v_계약업체",
    "v_ERP접수",
    "v_가공열",
    // 맞춤 뷰의 메타(docs/user-views.md §6). 맞춤 뷰(`u_*`) 자체는 설치마다 달라 여기 들지 않는다.
    "v_맞춤뷰",
];

/// `Views.ReadOnly` — 고칠 수 없는 표. `v_계획` 은 `plan` 이 개체가 아니라서, 차수 뷰 셋은 `Store.FaceView` 가 종류당
/// 뷰를 하나만 알아 옛 차수의 칸을 고치면 덮개의 `original` 이 빈 문자열로 박히기 때문이다.
pub const READ_ONLY: &[&str] = &[
    "v_계획",
    "v_통합차수",
    "v_공고차수",
    "v_계약차수",
    "v_ERP원천",
    "v_계약업체",
    "v_ERP접수",
    "v_가공열",
    "v_맞춤뷰",
];

/// `Views.Exported` — 엑셀로 나가는 것. [`NAMES`] 와 일부러 다르다(차수를 편 셋은 같은 건을 여러 줄로 다시 낸다).
pub const EXPORTED: &[&str] = &[
    "v_통합",
    "v_접수",
    "v_접수품목",
    "v_공고",
    "v_공고품목",
    "v_계약",
    "v_계약품목",
    "v_ERP원천",
    "v_계약업체",
    "v_ERP접수",
];

/// `Views.SheetName` — 시트·화면에 쓸 짧은 이름. `v_접수품목` → `접수품목`.
pub fn sheet_name(view: &str) -> &str {
    view.strip_prefix("v_").unwrap_or(view)
}

/// `Views.EntityTypeOf` — 이 뷰의 줄 하나는 무엇인가. 덮개와 사람 열이 어느 표에 담길지가 여기서 갈린다.
/// `v_통합차수` 는 이름 규칙으로 표현되지 않아 따로 적고, 계획을 맨 앞에, 접수를 공고보다 먼저 본다.
pub fn entity_type_of(view: &str) -> &'static str {
    if view == "v_통합차수" {
        "notice"
    } else if view.contains("계획") {
        "plan"
    } else if view.contains("접수") {
        "request"
    } else if view.contains("공고") {
        "notice"
    } else {
        "contract"
    }
}

/// `Views.KeyColumns` — 손으로 고칠 수 없는 열. 레코드가 어디에 있는지를 정하는 것들이다.
pub const KEY_COLUMNS: &[&str] = &[
    "입찰공고번호",
    "공고본번호",
    "계약번호",
    "계약본번호",
    "접수번호",
    "접수본번호",
    "차수",
    "순번",
    // 건과 그 건의 현행 공고 — 관련공고에서 지어지는 파생값이다.
    "공고건",
    "현행공고",
];

/// `Views.UnifiedNoticeColumns` — `v_통합` 이 `v_공고` 에서 붙여 오는 열(통합 시트에서는 잠근다).
pub const UNIFIED_NOTICE_COLUMNS: &[&str] = &[
    "입찰공고번호",
    "공고명",
    "공고종류",
    "게시일시",
    "입찰방식",
    "낙찰방법",
    "낙찰하한율",
    "사업예산",
    "배정예산",
    "추정가격",
    "기초금액",
    "개찰일시",
    "입찰개시일시",
    "입찰마감일시",
    "등록마감일시",
    "공고담당자",
    "사전규격등록번호",
];

/// `Views.UnifiedRequestColumns` — `v_통합` 이 `v_접수` 에서 붙여 오는 열.
pub const UNIFIED_REQUEST_COLUMNS: &[&str] = &[
    "접수번호",
    "조달요구번호",
    "요청명",
    "접수일자",
    "품대",
    "접수수수료",
    "예산금액",
    "기관담당자",
    "기관담당자전화",
];

/// `Views.ContractColumns` — `v_계약` 이 내는 열 차례(사람이 세운 열 앞까지). `v_통합` 이 열마다 COALESCE 로 감싸려고
/// 이름을 한 벌 더 든다 — 어긋나면 통합에서 그 열만 사라지므로 시험이 실제 뷰와 대조한다.
pub const CONTRACT_COLUMNS: &[&str] = &[
    "계약번호",
    "계약본번호",
    "차수",
    "계약건명",
    "계약일자",
    "계약방법",
    "계약구분",
    "품명",
    "수량",
    "단위",
    "계약금액",
    "계약보증금률(%)",
    "계약보증금(원)",
    "수수료",
    "지체상금률",
    "하자보수보증금률",
    "하자담보책임기간",
    "계약기간",
    "납품기한",
    "인도조건",
    "납품장소",
    "분할납품",
    "지급방법",
    "수요기관",
    "검사기관",
    "검수기관",
    "계약상대자",
    "대표자",
    "사업자등록번호",
    "상대자주소",
    "상대자전화",
    "상대자팩스",
    // 확정되어 담기 시작한 필드(ADR-043, 스키마 V28). 기존 필드 순서를 지키도록 끝에 덧붙인다.
    "수요기관담당자ID",
    "외국산여부",
    "계약법구분",
    "물가변동적용기준",
    "공개여부",
    "공동수급구성방식",
];

/// `Views.NoticeColumns` — `v_공고` 가 내는 열 차례(사람이 세운 열 앞까지). 가공열이 있으면 뷰를 한 겹 감싸 바깥에서
/// 열을 하나씩 다시 적어야 해서 이름을 한 벌 더 든다.
pub const NOTICE_COLUMNS: &[&str] = &[
    "입찰공고번호",
    "공고본번호",
    "차수",
    "공고명",
    "공고종류",
    "게시일시",
    "입찰방식",
    "낙찰방법",
    "낙찰하한율",
    "계약방법",
    "계약구분",
    "사업예산",
    "배정예산",
    "추정가격",
    "기초금액",
    "분할납품",
    "하자담보기간",
    "공고기관",
    "공고담당자",
    "집행관",
    "지역제한",
    "사전규격등록번호",
    "개찰일시",
    "입찰개시일시",
    "입찰마감일시",
    "등록마감일시",
    "수요기관",
    "세부품명",
    "세부품명번호",
    "수량",
    "단위",
    "납품기한",
    "인도조건",
    "담당부서",
    "담당자",
    "담당자전화",
    "담당자팩스",
    "관련공고",
    "공고건",
    "현행공고",
    // 확정되어 담기 시작한 필드(ADR-043, 스키마 V28). 기존 필드 순서를 지키도록 끝에 덧붙인다.
    "계약법구분",
    "사업금액수수료합계",
    "입찰참가제한업종",
    "입찰진행상태",
    // 재무관(fncrNm). 「채권자」 로 담던 칸을 바로 읽은 것이다(스키마 V29).
    "재무관",
];

/// `Views.RequestColumns` — `v_접수` 가 내는 열 차례(사람이 세운 열 앞까지).
pub const REQUEST_COLUMNS: &[&str] = &[
    "접수번호",
    "접수본번호",
    "차수",
    "조달요구번호",
    "품목수",
    "요청명",
    "접수일자",
    "업무구분",
    "계약법구분",
    "계약방법",
    "계약유형",
    "낙찰방법",
    "회계구분",
    "지급방법",
    "품대",
    "수수료",
    "부가가치세액",
    "예산금액",
    "외국산여부",
    "요청구분",
    "공개여부",
    "전담관",
    "담당과",
    "담당자",
    "선급금선고지",
    "선급금지급가능",
    "기타사항",
    "수요기관",
    "수요기관코드",
    "기관담당자",
    "기관담당자전화",
    "기관담당자팩스",
    "세부품명",
    "세부품명번호",
    "수량",
    "단위",
    "인도조건",
    "납품기한",
    "재고번호",
    // 확정되어 담기 시작한 필드(ADR-043, 스키마 V28). 기존 필드 순서를 지키도록 끝에 덧붙인다.
    "공고종류",
    "추정가격",
];

const HIDDEN_NOTICE: &[&str] = &[
    "계약법구분",
    "사업금액수수료합계",
    "입찰참가제한업종",
    "입찰진행상태",
    "재무관",
];
const HIDDEN_CONTRACT: &[&str] = &[
    "수요기관담당자ID",
    "외국산여부",
    "계약법구분",
    "물가변동적용기준",
    "공개여부",
    "공동수급구성방식",
];

/// `Views.HiddenByDefault` — V28 이후 확정되어 도입한 열(ADR-043). 뷰 이름으로 찾는다. 판올림 보정이 사용자 열과
/// 겹치는 새 열을 가리는 데 쓴다.
pub fn hidden_by_default(view: &str) -> Option<&'static [&'static str]> {
    Some(match view {
        "v_공고" | "v_공고차수" => HIDDEN_NOTICE,
        "v_공고품목" => &[
            "수요기관코드",
            "물품분류번호",
            "물품식별명",
            "규격번호",
            "조달요구번호",
        ],
        "v_계약" | "v_계약차수" | "v_통합" | "v_통합차수" => HIDDEN_CONTRACT,
        "v_계약품목" => &[
            "입찰분류번호",
            "입찰공고품목순번",
            "납품장소",
            "납품일수",
            "수요기관코드",
            "조달요구번호",
            "원산지",
        ],
        "v_접수" => &["공고종류", "추정가격"],
        "v_접수품목" => &["물품분류번호", "납품부대명"],
        _ => return None,
    })
}

/// `Views.BaseColumns` — 이 자료 종류의 기본 열(사람이 세운 열을 뺀 면 뷰의 열). 가공열의 원천으로 쓸 수 있는 것이
/// 이것뿐이라 뷰 사이 순환이 구조적으로 막힌다(§4.1).
pub fn base_columns(entity_type: &str) -> Result<&'static [&'static str]> {
    match entity_type {
        "notice" => Ok(NOTICE_COLUMNS),
        "contract" => Ok(CONTRACT_COLUMNS),
        "request" => Ok(REQUEST_COLUMNS),
        _ => Err(Error::invalid("가공열의 자료 종류가 올바르지 않습니다.")),
    }
}

/// `Views.Face(string)` — 이 자료 종류의 덮개 열 한 벌(사람 열 없이). 가공열이 연결 문서의 필드를 읽을 때 상대 뷰를
/// 읽지 않고 이것을 그 개체의 행 별칭(`n`·`c`·`r`)으로 다시 부른다 — 뷰 사이 순환이 생기지 않는다(§11.1-3).
pub(crate) fn face(entity_type: &str) -> Result<String> {
    match entity_type {
        "notice" => Ok(notice_columns_sql("")),
        "contract" => Ok(contract_columns_sql("")),
        "request" => Ok(request_columns_sql("")),
        _ => Err(Error::invalid("가공열의 자료 종류가 올바르지 않습니다.")),
    }
}

/// `Views.LegacyNames` — 판을 붙여 부르던 옛 이름(ADR-034 전). [`definitions`] 가 맨 앞에서 모두 치운다. 차례는 얹힌
/// 것부터다.
pub const LEGACY_NAMES: &[&str] = &[
    "v_통합_v3",
    "v_계획_v1",
    "v_통합_v2",
    "v_통합_v1",
    "v_공고차수_v1",
    "v_계약차수_v1",
    "v_공고품목_v1",
    "v_품목_v1",
    "v_접수품목_v1",
    "v_공고_v1",
    "v_계약_v1",
    "v_접수_v1",
    "v_ERP원천_v1",
    "v_계약업체_v1",
    "v_ERP접수_v1",
    // 맞춤 뷰의 메타가 2판에서 서던 이름(스키마 23). 3판에서 제자리에서 v_맞춤뷰 로 바꾸었다(ADR-034).
    "v_사용자뷰",
];

/// `Views.UserViewPrefix` — 맞춤 뷰가 SQLite 에 서는 이름의 머리.
pub const USER_VIEW_PREFIX: &str = "u_";

/// C# `UserColumnDefinition` — 사람이 채우는 열 하나의 정의. `kind` 는 text · choice · date · number · computed,
/// `options` 는 choice 면 세로줄로 나눈 후보, computed 면 가공식 v2 JSON.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserColumnDefinition {
    pub entity_type: String,
    pub field_name: String,
    pub kind: String,
    pub options: Option<String>,
    pub sort_order: i64,
}

impl UserColumnDefinition {
    /// `UserColumnDefinition.Choices` — `Split('|', RemoveEmptyEntries | TrimEntries)`.
    pub fn choices(&self) -> Vec<String> {
        match (&*self.kind, &self.options) {
            ("choice", Some(options)) => options
                .split('|')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect(),
            _ => Vec::new(),
        }
    }
}

// ── 표기 조각 ─────────────────────────────────────────────
//
// C# 의 private 조각을 같은 이름 자리에서 옮긴다. 원시 문자열 몸은 C# 원본 그대로다(위 모듈 설명).

/// `Views.Text` — 빈 값은 NULL 이 아니라 빈 문자열로 낸다.
fn text(column: &str) -> String {
    format!("COALESCE({column}, '')")
}

/// `Views.YesNo`.
fn yes_no(column: &str) -> String {
    format!("CASE {column} WHEN 1 THEN '가능' WHEN 0 THEN '불가' ELSE '' END")
}

/// `Views.YesNoWord` — 양식이 `예`·`아니오` 로 적는 칸.
fn yes_no_word(column: &str) -> String {
    format!("CASE {column} WHEN 1 THEN '예' WHEN 0 THEN '아니오' ELSE '' END")
}

/// `Views.Number` — 뜻 없는 소수부를 턴다.
fn number(column: &str) -> String {
    csharp(
        r#"
        CASE
            WHEN {column} IS NULL OR {column} = '' THEN ''
            WHEN instr({column},'.') = 0 THEN {column}
            WHEN rtrim({column},'0') LIKE '%.' THEN rtrim(rtrim({column},'0'),'.')
            ELSE rtrim({column},'0')
        END
        "#,
        &[("column", column)],
    )
}

/// `Views.DateText` — 날짜를 양식이 쓰는 표기로 되돌린다(`2026-09-20 00:00:00` → `2026/09/20`).
fn date_text(column: &str) -> String {
    csharp(
        r#"
        CASE
            WHEN {column} IS NULL OR {column} = '' THEN ''
            WHEN substr({column},11) IN ('', 'T00:00:00', ' 00:00:00', 'T00:00:00.0000000')
                THEN replace(substr({column},1,10),'-','/')
            ELSE replace(substr({column},1,10),'-','/') || ' ' || substr({column},12,8)
        END
        "#,
        &[("column", column)],
    )
}

/// `Views.Money` — 금액에 세 자리마다 자릿점을 넣는다. 소수부는 건드리지 않는다.
pub(crate) fn money(column: &str) -> String {
    // 뜻 없는 .0 을 먼저 턴 값 위에서 자릿점을 찍는다.
    let trimmed = number(column);
    let whole = "CASE WHEN instr(t,'.')>0 THEN substr(t,1,instr(t,'.')-1) ELSE t END";
    let fraction = "CASE WHEN instr(t,'.')>0 THEN substr(t,instr(t,'.')) ELSE '' END";
    let grouped = raw(
        r#"
            CASE
                WHEN length(w) <= 3  THEN w
                WHEN length(w) <= 6  THEN substr(w,1,length(w)-3) || ',' || substr(w,-3)
                WHEN length(w) <= 9  THEN substr(w,1,length(w)-6) || ',' || substr(w,-6,3) || ',' || substr(w,-3)
                WHEN length(w) <= 12 THEN substr(w,1,length(w)-9) || ',' || substr(w,-9,3) || ',' || substr(w,-6,3) || ',' || substr(w,-3)
                ELSE w
            END
            "#,
        &[],
    );
    csharp(
        r#"
            COALESCE((
                SELECT (SELECT {grouped} FROM (SELECT {whole} AS w)) || ({fraction})
                FROM (SELECT {trimmed} AS t)
                WHERE t <> ''
            ), '')
            "#,
        &[
            ("grouped", &grouped),
            ("whole", whole),
            ("fraction", fraction),
            ("trimmed", &trimmed),
        ],
    )
}

/// `Views.UniformItem` — 이 공고의 모든 품목 줄이 같은 값일 때 그 값(빈 줄도 한 값으로 센다).
fn uniform_item(column: &str) -> String {
    csharp(
        r#"
        (SELECT CASE WHEN COUNT(DISTINCT COALESCE({column}, '')) = 1 THEN MIN(COALESCE({column}, '')) END
         FROM notice_item i
         WHERE i.notice_base = n.notice_base AND i.seq = n.seq)
        "#,
        &[("column", column)],
    )
}

/// `Views.UniformQuantity` — 단위가 모두 같고 수량이 빈 줄이 없을 때만 수량을 더한다.
fn uniform_quantity() -> String {
    raw(
        r#"
        (SELECT CASE WHEN COUNT(DISTINCT COALESCE(i.unit,'')) = 1
                   AND COUNT(NULLIF(i.quantity,'')) = COUNT(*)
              THEN CAST(SUM(i.quantity) AS TEXT) END
         FROM notice_item i
         WHERE i.notice_base = n.notice_base AND i.seq = n.seq)
        "#,
        &[],
    )
}

/// `Views.UniformRequestItem` — [`uniform_item`] 의 접수판(몸통을 함께 쓰지 않는다).
fn uniform_request_item(column: &str) -> String {
    csharp(
        r#"
        (SELECT CASE WHEN COUNT(DISTINCT COALESCE({column}, '')) = 1 THEN MIN(COALESCE({column}, '')) END
         FROM request_item i
         WHERE i.request_base = r.request_base AND i.seq = r.seq)
        "#,
        &[("column", column)],
    )
}

/// `Views.UniformRequestQuantity`.
fn uniform_request_quantity() -> String {
    raw(
        r#"
        (SELECT CASE WHEN COUNT(DISTINCT COALESCE(i.unit,'')) = 1
                   AND COUNT(NULLIF(i.quantity,'')) = COUNT(*)
              THEN CAST(SUM(i.quantity) AS TEXT) END
         FROM request_item i
         WHERE i.request_base = r.request_base AND i.seq = r.seq)
        "#,
        &[],
    )
}

/// `Views.RequestDeadline` — 접수의 납품기한. 날짜가 있으면 그 날, 없으면 적힌 문장 그대로(ADR-016).
fn request_deadline(due_column: &str, days_column: &str) -> String {
    csharp(
        r#"
        CASE
            WHEN {dueColumn} IS NOT NULL AND {dueColumn} <> '' THEN {DateText(dueColumn)}
            ELSE COALESCE({daysColumn}, '')
        END
        "#,
        &[("dueColumn", due_column), ("daysColumn", days_column)],
    )
}

/// `Views.RequestNumbers` — 이 접수서가 묶은 조달요구번호를 번호마다 한 번, 처음 나온 순번 차례로 잇는다.
/// 하위 질의의 차례는 SQLite 가 바깥 집계에 보장하지 않아 집계 안에서 차례를 정한다.
fn request_numbers() -> String {
    raw(
        r#"
        (SELECT group_concat(v, ', ' ORDER BY k) FROM (
            SELECT x.request_number AS v, MIN(x.line_no) AS k FROM request_item x
            WHERE x.request_base = r.request_base AND x.seq = r.seq
              AND x.request_number IS NOT NULL AND x.request_number <> ''
            GROUP BY x.request_number))
        "#,
        &[],
    )
}

/// `Views.SoleContact` — 담당자가 한 사람일 때만 그 칸.
fn sole_contact(column: &str) -> String {
    csharp(
        r#"
        (SELECT CASE WHEN COUNT(DISTINCT COALESCE({column}, '')) = 1 THEN MIN(COALESCE({column}, '')) END
         FROM notice_officer_contact t
         WHERE t.notice_base = n.notice_base AND t.seq = n.seq)
        "#,
        &[("column", column)],
    )
}

/// `Views.RelatedNotices` — 관련공고를 적힌 차례대로 `,` 로 잇는다(문서가 그렇게 찍는다). 차례는 집계 안에서 정한다.
fn related_notices() -> String {
    raw(
        r#"
        (SELECT group_concat(x.related, ',' ORDER BY x.line_no) FROM notice_relation x
         WHERE x.notice_base = n.notice_base AND x.seq = n.seq)
        "#,
        &[],
    )
}

/// `Views.RestrictedIndustries` — 입찰참가 제한 업종명을 줄 차례대로 `", "` 로 잇는다. 차례는 집계 안에서 정한다.
fn restricted_industries() -> String {
    raw(
        r#"
        (SELECT group_concat(x.name, ', ' ORDER BY x.line_no) FROM notice_restricted_industry x
         WHERE x.notice_base = n.notice_base AND x.seq = n.seq
           AND x.name IS NOT NULL AND x.name <> '')
        "#,
        &[],
    )
}

/// `Views.DeliveryDeadline` — 납품기한을 한 칸으로(날짜가 먼저, 없으면 `계약 후 N일 이내`).
fn delivery_deadline(due_column: &str, days_column: &str) -> String {
    csharp(
        r#"
        CASE
            WHEN {dueColumn} IS NOT NULL AND {dueColumn} <> '' THEN {DateText(dueColumn)}
            WHEN {daysColumn} IS NOT NULL AND {daysColumn} > 0
                THEN '계약 후 ' || CAST({daysColumn} AS TEXT) || '일 이내'
            ELSE ''
        END
        "#,
        &[("dueColumn", due_column), ("daysColumn", days_column)],
    )
}

/// `Views.WarrantyText` — 하자담보기간을 한 가지 모양으로(총 개월 수 → `3년`·`1년 6개월`·`6개월`).
fn warranty_text(total_months: &str) -> String {
    csharp(
        r#"
        (SELECT CASE WHEN t IS NULL OR t <= 0 THEN ''
                     WHEN t % 12 = 0 THEN (t / 12) || '년'
                     WHEN t >= 12 THEN (t / 12) || '년 ' || (t % 12) || '개월'
                     ELSE t || '개월' END
         FROM (SELECT CAST({totalMonths} AS INTEGER) AS t))
        "#,
        &[("totalMonths", total_months)],
    )
}

/// `Views.WarrantyPhrase` — 화면이 문장으로 적어 보낸 하자담보기간을 [`warranty_text`] 의 모양으로 다듬는다.
fn warranty_phrase(column: &str) -> String {
    csharp(
        r#"
        (SELECT CASE WHEN c GLOB '*년[0-9]*개월'
                     THEN substr(c, 1, instr(c, '년')) || ' ' || substr(c, instr(c, '년') + 1)
                     ELSE c END
         FROM (SELECT CASE WHEN b GLOB '0년*' THEN substr(b, 3) ELSE b END AS c
               FROM (SELECT CASE WHEN a GLOB '*년0개월' THEN substr(a, 1, length(a) - 3) ELSE a END AS b
                     FROM (SELECT replace({column}, ' ', '') AS a))))
        "#,
        &[("column", column)],
    )
}

/// `Views.WarrantyYears` — 공고의 하자담보기간.
fn warranty_years() -> String {
    csharp(
        r#"
        CASE WHEN n.warranty_text IS NOT NULL THEN {WarrantyPhrase("n.warranty_text")}
             WHEN n.warranty_years IS NOT NULL
                 THEN {WarrantyText("CAST(n.warranty_years AS INTEGER) * 12 + CAST(coalesce(n.warranty_month_part,'0') AS INTEGER)")}
             WHEN n.warranty_months IS NULL THEN ''
             ELSE {WarrantyText("n.warranty_months")} END
        "#,
        &[],
    )
}

/// `Views.Step` — 공고 일정에서 한 단계의 시각을 꺼낸다.
fn step(column: &str, keyword: &str) -> String {
    format!(
        "(SELECT {column} FROM notice_schedule s WHERE s.notice_base = n.notice_base AND s.seq = n.seq AND s.name LIKE '%{keyword}%')"
    )
}

/// `Views.LatestNotice` — 최신 차수만 고른다(차수는 TEXT 라 수로 비교한다).
fn latest_notice() -> String {
    raw(
        r#"
        JOIN (SELECT notice_base AS b, MAX(CAST(seq AS INTEGER)) AS s FROM notice GROUP BY notice_base) latest
          ON n.notice_base = latest.b AND CAST(n.seq AS INTEGER) = latest.s
        "#,
        &[],
    )
}

/// `Views.공고건` — 이 공고가 속한 건.
fn notice_group(notice_base_expr: &str) -> String {
    format!("(SELECT s.group_base FROM notice_series s WHERE s.notice_base = {notice_base_expr})")
}

/// `Views.건현행` — 그 건의 대체되지 않은 공고 하나. 규칙의 정본은 [`notice_groups::current_notice`] 하나이고 이것은
/// 한 건의 것만 집어 낸다. `pick` 은 `현행.notice_base` 또는 `현행.notice_base || '-' || 현행.seq`.
pub(crate) fn group_current(group_expr: &str, pick: &str) -> String {
    csharp(
        r#"
        (SELECT {낼것}
         FROM ({NoticeGroups.현행공고}) 현행
         WHERE 현행.group_base = {groupExpr})
        "#,
        &[("낼것", pick), ("groupExpr", group_expr)],
    )
}

/// `Views.사슬` — 조달 건의 사슬(접수·공고건·계약 본번호의 짝, ADR-022). `v_통합` 과 맞춤 뷰가 이 한 벌을 부른다.
pub(crate) fn chain() -> String {
    raw(
        r#"
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
        "#,
        &[],
    )
}

/// `Views.LatestContract`.
fn latest_contract() -> String {
    raw(
        r#"
        JOIN (SELECT contract_base AS b, MAX(CAST(seq AS INTEGER)) AS s FROM contract GROUP BY contract_base) latest
          ON c.contract_base = latest.b AND CAST(c.seq AS INTEGER) = latest.s
        "#,
        &[],
    )
}

/// `Views.LatestRequest`.
fn latest_request() -> String {
    raw(
        r#"
        JOIN (SELECT request_base AS b, MAX(CAST(seq AS INTEGER)) AS s FROM request GROUP BY request_base) latest
          ON r.request_base = latest.b AND CAST(r.seq AS INTEGER) = latest.s
        "#,
        &[],
    )
}

/// `Views.PlanFolds` — `v_계획` 이 쓰는 미리 접은 표들. 줄이 갈라지지 않게 하는 것이 전부다.
fn plan_folds() -> String {
    raw(
        r#"
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
        "#,
        &[],
    )
}

/// `Views.SoleContract` — 계약이 하나일 때만 그 칸.
fn sole_contract(expression: &str) -> String {
    format!("CASE WHEN m.건수 = 1 THEN {expression} ELSE '' END")
}

/// `Views.UserColumn` — 사람이 채우는 열 하나(값은 `*_user_field`, 계열에 붙는다). 열 이름을 거르지 않는다(C# 그대로).
fn user_column(entity_type: &str, key_expression: &str, field_name: &str) -> String {
    let (table, base_column) = match entity_type {
        "contract" => ("contract_user_field", "contract_base"),
        "request" => ("request_user_field", "request_base"),
        _ => ("notice_user_field", "notice_base"),
    };
    format!(
        "COALESCE((SELECT value FROM {table} u WHERE u.{base_column} = {key_expression} AND u.field_name = '{field_name}'), '') AS \"{field_name}\""
    )
}

/// (종류, 바깥 키 열, 열 이름, 표기식) — [`face_references`] 가 뷰를 지어 보는 동안만 모은다(C# `[ThreadStatic] _faces`).
type FaceRecord = (String, String, String, String);

thread_local! {
    static FACES: RefCell<Option<Vec<FaceRecord>>> = const { RefCell::new(None) };
}

/// `Views.Face(entity, base, seq, name, expression)` — 사람이 고친 값이 있으면 그것을, 없으면 파서가 읽은 것을 낸다.
/// 씌우는 자리는 표기식의 바깥이고, 덮개는 차수에 매달린다.
fn face_column(
    entity_type: &str,
    base_column: &str,
    seq_column: &str,
    name: &str,
    expression: &str,
) -> String {
    FACES.with(|faces| {
        if let Some(faces) = faces.borrow_mut().as_mut() {
            faces.push((
                entity_type.into(),
                base_column.into(),
                name.into(),
                expression.into(),
            ));
        }
    });
    format!(
        "{} AS \"{name}\"",
        faced(entity_type, base_column, seq_column, name, expression)
    )
}

/// `Views.Faced` — [`face_column`] 의 값만(이름을 붙이지 않는다).
fn faced(
    entity_type: &str,
    base_column: &str,
    seq_column: &str,
    name: &str,
    expression: &str,
) -> String {
    format!(
        "COALESCE((SELECT o.value FROM field_override o WHERE o.entity_type = '{entity_type}' AND o.base = {base_column} AND o.seq = {seq_column} AND o.column_name = '{name}'), {expression})"
    )
}

/// `Views.UserColumns` — 사람이 채우는 열(가공열 빼고). 가공열은 감싼 바깥([`wrap`])에서 덮개를 마친 값을 읽는다.
fn user_columns(columns: &[UserColumnDefinition], entity_type: &str, key: &str) -> String {
    let wanted: Vec<String> = columns
        .iter()
        .filter(|c| c.entity_type == entity_type && c.kind != "computed")
        .map(|c| user_column(entity_type, key, &c.field_name))
        .collect();
    if wanted.is_empty() {
        return String::new();
    }
    const SEPARATOR: &str = ",\n            ";
    format!("{SEPARATOR}{}", wanted.join(SEPARATOR))
}

/// `Views.Wrap` — 가공열이 하나라도 있으면 뷰를 한 겹 감싼다(§11.1-3). 바깥 목록은 지금 열 순서를 그대로 하나씩
/// 적는다. 가공열이 없으면 감싸지 않는다.
fn wrap(
    body: &str,
    entity_type: &str,
    columns: &[UserColumnDefinition],
    broken: &BrokenColumns,
) -> Result<String> {
    let mine: Vec<&UserColumnDefinition> = columns
        .iter()
        .filter(|c| c.entity_type == entity_type)
        .collect();
    if !mine.iter().any(|c| c.kind == "computed") {
        return Ok(body.to_string());
    }
    let mut outer: Vec<String> = base_columns(entity_type)?
        .iter()
        .map(|name| format!("{SELF}.{}", flow_compiler::ident(name)))
        .collect();
    for c in mine {
        let ident = flow_compiler::ident(&c.field_name);
        outer.push(if c.kind != "computed" {
            format!("{SELF}.{ident}")
        } else if let Some(why) = broken.get(&(entity_type.to_string(), c.field_name.clone())) {
            format!("{} AS {ident}", broken_marker(why))
        } else {
            let formula = computed_column::parse(c.options.as_deref().unwrap_or(""))?;
            format!(
                "{} AS {ident}",
                computed_column::expression(entity_type, &formula, None)?
            )
        });
    }
    Ok(format!(
        "SELECT\n    {}\nFROM (\n{body}\n) AS {SELF}",
        outer.join(",\n    ")
    ))
}

/// `Views.BrokenMarker` — 깨진 가공열의 자리(빈 글). 까닭은 뷰 정의 안 주석으로 남겨 [`broken_in`] 이 읽는다.
pub(crate) fn broken_marker(why: &str) -> String {
    format!(
        "'' /* 깨진 가공식: {} */",
        replace_line_endings(&why.replace("*/", "* /"), " ")
    )
}

/// `Views.BrokenIn` — 면 뷰의 정의 글에서 깨진 가공열(이름 → 까닭)을 읽는다. C# 정규식
/// `'' /\* 깨진 가공식: (.*?) \*/ AS "((?:[^"]|"")+)"`(Singleline)을 손으로 훑는다. 같은 이름이 또 나오면 값만 바꾼다.
pub fn broken_in(view_sql: Option<&str>) -> Vec<(String, String)> {
    const HEAD: &str = "'' /* 깨진 가공식: ";
    const TAIL: &str = " */ AS \"";
    let mut found: Dict<String, String> = Dict::new();
    let Some(sql) = view_sql else {
        return Vec::new();
    };
    let mut at = 0;
    while let Some(start) = sql[at..].find(HEAD).map(|i| at + i) {
        let reason_start = start + HEAD.len();
        // (.*?) — 가장 짧은 까닭부터, 뒤의 이름이 맞는 첫 자리.
        let mut matched = None;
        let mut search = reason_start;
        while let Some(tail) = sql[search..].find(TAIL).map(|i| search + i) {
            if let Some(end) = quoted_name_end(sql, tail + TAIL.len()) {
                matched = Some((tail, end));
                break;
            }
            search = tail + 1;
        }
        match matched {
            Some((tail, end)) => {
                let name = &sql[tail + TAIL.len()..end];
                found.insert(
                    name.replace("\"\"", "\""),
                    sql[reason_start..tail].to_string(),
                );
                at = end + 1;
            }
            None => at = start + 1,
        }
    }
    found.into_vec()
}

/// `((?:[^"]|"")+)"` 가 `from` 에서 맞으면 이름의 끝(닫는 따옴표 자리). 탐욕으로 먹고, 끝까지 닫지 못하면 마지막 `""`
/// 를 내놓아 그 첫 따옴표로 닫는다(정규식의 되짚기와 같다).
fn quoted_name_end(sql: &str, from: usize) -> Option<usize> {
    let bytes = sql.as_bytes();
    let mut i = from;
    let mut tokens = 0;
    let mut last_pair = None;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            if bytes.get(i + 1) == Some(&b'"') {
                last_pair = Some((i, tokens));
                tokens += 1;
                i += 2;
                continue;
            }
            return (tokens > 0).then_some(i);
        }
        tokens += 1;
        i += sql[i..].chars().next().map_or(1, char::len_utf8);
    }
    last_pair.and_then(|(i, before)| (before > 0).then_some(i))
}

/// `Views.N` — 공고의 덮개.
fn n(name: &str, expression: &str) -> String {
    face_column("notice", "n.notice_base", "n.seq", name, expression)
}

/// `Views.C` — 계약의 덮개.
fn c(name: &str, expression: &str) -> String {
    face_column("contract", "c.contract_base", "c.seq", name, expression)
}

/// `Views.R` — 접수의 덮개.
fn r(name: &str, expression: &str) -> String {
    face_column("request", "r.request_base", "r.seq", name, expression)
}

/// `Views.공고열` — 공고 뷰가 내는 열 한 벌. `v_공고` 와 `v_공고차수` 가 같이 쓴다(두 벌로 적지 않는다).
fn notice_columns_sql(notice_columns: &str) -> String {
    csharp(
        r#"
            -- 키 열은 감싸지 않는다. 고치면 값이 바뀌는 것이 아니라 레코드가 옮겨간다.
            n.notice_base || '-' || n.seq AS 입찰공고번호,
            n.notice_base                 AS 공고본번호,
            n.seq                         AS 차수,

            {N("공고명", Text("n.title"))},
            {N("공고종류", Text(NoticeGroups.Kind("n.notice_kind", "n.source_status")))},
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

            -- 칸마다 모든 품목 줄이 같으면 싣는다. 갈리면 비우고 v_공고품목 이 줄마다 낸다.
            -- 수량은 더하되 단위가 모두 같을 때만 — 단위가 갈린 합은 뜻이 없다.
            {N("수요기관", Text(UniformItem("i.demand_agency")))},
            {N("세부품명", Text(UniformItem("i.item_name")))},
            {N("세부품명번호", Text(UniformItem("i.detail_item_number")))},
            {N("수량", Text(UniformQuantity()))},
            {N("단위", Text(UniformItem("i.unit")))},
            {N("납품기한", Text(UniformItem(DeliveryDeadline("i.delivery_due", "i.delivery_days"))))},
            -- 인도조건도 같은 규칙이다. 물건이 아니라 납품 방식이라 줄마다 같은 일이 가장 흔하다.
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

            -- 확정되어 담기 시작한 필드(ADR-043, 스키마 V28). 끝에 덧붙인다 — 저쪽에 저장된 필드 연결이
            -- 끊기지 않는다. 제한 업종은 여럿이라 줄 차례대로 잇는다.
            {N("계약법구분", Text("n.contract_law"))},
            {N("사업금액수수료합계", Money("n.project_fee_total"))},
            {N("입찰참가제한업종", Text(RestrictedIndustries))},
            {N("입찰진행상태", Text("n.bid_status"))},
            -- 재무관(fncrNm). 공고담당자·집행관과 다른 사람이다. 「채권자」 로 섞여 담기던 것을 갈랐다(스키마 V29).
            {N("재무관", Text("n.finance_officer"))}{noticeColumns}
        "#,
        &[("noticeColumns", notice_columns)],
    )
}

/// `Views.계약열` — 계약 뷰가 내는 열 한 벌. `v_계약` 과 `v_계약차수` 가 같이 쓴다.
fn contract_columns_sql(contract_columns: &str) -> String {
    csharp(
        r#"
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
            {C("계약보증금률(%)", Number("c.guarantee_rate"))},
            {C("계약보증금(원)", Money("c.guarantee_amount"))},
            {C("수수료", Money("c.fee"))},
            {C("지체상금률", Number("c.delay_penalty_rate"))},
            {C("하자보수보증금률", Number("c.warranty_bond_rate"))},
            {C("하자담보책임기간", Text($"CASE WHEN c.warranty_years IS NOT NULL THEN {WarrantyText("CAST(c.warranty_years AS INTEGER) * 12 + CAST(coalesce(c.warranty_months,'0') AS INTEGER)")} ELSE c.warranty_period END"))},
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
            {C("상대자팩스", Text(PartyField("fax")))},

            -- 확정되어 담기 시작한 필드(ADR-043, 스키마 V28). 기존 필드 순서를 지키도록 끝에 덧붙인다.
            {C("수요기관담당자ID", Text("c.agency_officer_id"))},
            {C("외국산여부", YesNoWord("c.foreign_allowed"))},
            {C("계약법구분", Text("c.contract_law"))},
            {C("물가변동적용기준", Text("c.price_adjustment_basis"))},
            {C("공개여부", Text("c.disclosure"))},
            -- 공동수급 구성방식은 수집한 업체 줄에만 있다. 업체가 하나일 때만 싣는다 — 상대자 칸과 같은 규칙이다.
            {C("공동수급구성방식", Text(PartnerOnly("joint_method")))}{contractColumns}
        "#,
        &[("contractColumns", contract_columns)],
    )
}

/// `Views.접수열` — 접수 뷰가 내는 열 한 벌. 가공열이 연결 접수의 필드를 읽을 때 같은 덮개 식을 다시 부른다.
fn request_columns_sql(request_columns: &str) -> String {
    csharp(
        r#"
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

            -- 칸마다 모든 품목 줄이 같으면 싣는다. 갈리면 비우고 v_접수품목 이 줄마다 낸다.
            -- v_공고 의 규율을 그대로 따른다 — 하나로 정할 수 없는 것을 하나인 양 내지 않는다.
            {R("세부품명", Text(UniformRequestItem("i.item_name")))},
            {R("세부품명번호", Text(UniformRequestItem("i.detail_item_number")))},
            {R("수량", Text(UniformRequestQuantity()))},
            {R("단위", Text(UniformRequestItem("i.unit")))},
            {R("인도조건", Text(UniformRequestItem("i.delivery_terms")))},
            {R("납품기한", Text(UniformRequestItem(RequestDeadline("i.delivery_due", "i.delivery_days"))))},
            -- 재고번호도 같은 규칙이지만 세부품명 옆이 아니라 끝에 붙인다 — 계약면은
            -- 늘리기만 하는 자리라, 사이에 끼우면 뒤 열의 엑셀 자리가 한 칸씩 밀린다.
            {R("재고번호", Text(UniformRequestItem("i.stock_number")))},

            -- 확정되어 담기 시작한 필드(ADR-043, 스키마 V28). 기존 필드 순서를 지키도록 끝에 덧붙인다.
            {R("공고종류", Text("r.notice_kind"))},
            {R("추정가격", Money("r.estimated_price"))}{requestColumns}
        "#,
        &[("requestColumns", request_columns)],
    )
}

/// `Views.PartnerOnly` — 수집한 업체 줄에만 있는 칸. 업체 줄이 하나일 때만 그 값이다.
fn partner_only(column: &str) -> String {
    format!(
        "CASE (SELECT count(*) FROM erp_partner e WHERE e.contract_base=c.contract_base AND e.seq=c.seq) WHEN 1 THEN (SELECT e.{column} FROM erp_partner e WHERE e.contract_base=c.contract_base AND e.seq=c.seq) ELSE '' END"
    )
}

/// `Views.PartyField` — 상대자 칸. 업체 줄이 없으면 상대자 표, 하나면 그 줄, 여럿이면 빈 값.
fn party_field(column: &str) -> String {
    format!(
        "CASE (SELECT count(*) FROM erp_partner e WHERE e.contract_base=c.contract_base AND e.seq=c.seq) WHEN 0 THEN p.{column} WHEN 1 THEN (SELECT e.{column} FROM erp_partner e WHERE e.contract_base=c.contract_base AND e.seq=c.seq) ELSE '' END"
    )
}

/// `Views.Definitions` — 계약면을 짓는 문장들(차례대로 돌린다). `broken` 은 빈 글로 세울 가공열(종류·이름 → 까닭).
/// `v_맞춤뷰` 는 여기 없다 — 결과타입이 맞춤 뷰를 지어 본 뒤에야 정해져 맨 끝에 짓는다([`user_view_meta`]).
pub fn definitions(
    user_columns_in: &[UserColumnDefinition],
    broken: &BrokenColumns,
) -> Result<Vec<String>> {
    let columns = user_columns_in;
    let contract_columns = user_columns(columns, "contract", "c.contract_base");
    let notice_columns = user_columns(columns, "notice", "n.notice_base");
    let request_columns = user_columns(columns, "request", "r.request_base");

    // v_통합 이 계약 뷰에서 받아 오는 열. 사람이 세운 계약 열도 함께 온다.
    let unified_contract = CONTRACT_COLUMNS
        .iter()
        .map(|s| s.to_string())
        .chain(
            columns
                .iter()
                .filter(|c| c.entity_type == "contract")
                .map(|c| c.field_name.clone()),
        )
        .map(|name| format!("COALESCE(k.\"{name}\", '') AS \"{name}\""))
        .collect::<Vec<_>>()
        .join(",\n            ");

    // 뷰 본문. 공고·계약은 최신 차수로 좁히는 조인만 두 뷰가 다르다.
    let notice_body = format!(
        "SELECT\n{}\nFROM notice n\n",
        notice_columns_sql(&notice_columns)
    );
    let contract_body = format!(
        "SELECT\n{}\nFROM contract c\n",
        contract_columns_sql(&contract_columns)
    );
    const PARTY: &str = "\nLEFT JOIN counterparty p ON c.counterparty_number = p.business_number";
    let request_body = format!(
        "SELECT\n{}\nFROM request r\n{}",
        request_columns_sql(&request_columns),
        latest_request()
    );

    // 옛 이름(ADR-034 전)을 모두 치운다. 차례는 얹힌 것부터다.
    let mut list: Vec<String> = LEGACY_NAMES
        .iter()
        .map(|name| format!("DROP VIEW IF EXISTS {name};"))
        .collect();
    // 통합차수가 맨 앞 — 이 뭉치에서 가장 위에 얹혀 있다. 계획은 접수·공고·계약 셋을 가리키므로 그 셋보다 앞선다.
    list.extend(
        [
            "DROP VIEW IF EXISTS v_통합차수;",
            "DROP VIEW IF EXISTS v_계획;",
            "DROP VIEW IF EXISTS v_통합;",
            // 짓다 만 중간 뷰의 잔재.
            "DROP VIEW IF EXISTS v_계약_기본;",
            "DROP VIEW IF EXISTS _계약_기본;",
        ]
        .map(str::to_string),
    );

    list.push(csharp(
        r#"
        DROP VIEW IF EXISTS v_공고;
        CREATE VIEW v_공고 AS
        {Wrap};
        "#,
        &[(
            "Wrap",
            &wrap(
                &format!("{notice_body}{}", latest_notice()),
                "notice",
                columns,
                broken,
            )?,
        )],
    ));

    // 공고 차수 — 줄 하나가 공고 문서 한 장(같은 공고열, LatestNotice 만 없다). 읽기 전용이다.
    list.push(csharp(
        r#"
        DROP VIEW IF EXISTS v_공고차수;
        CREATE VIEW v_공고차수 AS
        {Wrap};
        "#,
        &[("Wrap", &wrap(&notice_body, "notice", columns, broken)?)],
    ));

    // 공고 품목 — 줄 하나가 품목 한 줄.
    list.push(csharp(
        r#"
        DROP VIEW IF EXISTS v_공고품목;
        CREATE VIEW v_공고품목 AS
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
            {Text("t.fax")}                       AS 담당자팩스,
            -- 확정되어 담기 시작한 필드(ADR-043, 스키마 V28). 기존 필드 순서를 지키도록 끝에 덧붙인다.
            {Text("i.demand_agency_code")}        AS 수요기관코드,
            {Text("i.classification_number")}     AS 물품분류번호,
            {Text("i.item_identification_name")}  AS 물품식별명,
            {Text("i.spec_number")}               AS 규격번호,
            {Text("i.request_number")}            AS 조달요구번호
        FROM notice_item i
        JOIN notice n ON n.notice_base = i.notice_base AND n.seq = i.seq
        {LatestNotice}
        LEFT JOIN notice_officer_contact t
               ON t.notice_base = i.notice_base AND t.seq = i.seq
              AND t.demand_agency = i.demand_agency;
        "#,
        &[],
    ));

    list.push(csharp(
        r#"
        DROP VIEW IF EXISTS _계약_기본;
        DROP VIEW IF EXISTS v_계약;
        CREATE VIEW v_계약 AS
        {Wrap};
        "#,
        &[(
            "Wrap",
            &wrap(
                &format!("{contract_body}{}{PARTY}", latest_contract()),
                "contract",
                columns,
                broken,
            )?,
        )],
    ));

    // 계약 차수 — 줄 하나가 계약 문서 한 장. 읽기 전용이다.
    list.push(csharp(
        r#"
        DROP VIEW IF EXISTS v_계약차수;
        CREATE VIEW v_계약차수 AS
        {Wrap};
        "#,
        &[(
            "Wrap",
            &wrap(
                &format!("{contract_body}{PARTY}"),
                "contract",
                columns,
                broken,
            )?,
        )],
    ));

    list.push(csharp(
        r#"
        DROP VIEW IF EXISTS v_계약품목;
        CREATE VIEW v_계약품목 AS
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
            {Text("i.delivery_terms")}            AS 인도조건,
            -- 확정되어 담기 시작한 필드(ADR-043, 스키마 V28). 기존 필드 순서를 지키도록 끝에 덧붙인다.
            -- 입찰분류번호·입찰공고품목순번은 적힌 그대로의 참조다. 잇는 데 쓰지 않는다(ADR-040).
            {Text("i.ref_notice_class")}          AS 입찰분류번호,
            {Text("i.ref_notice_item")}           AS 입찰공고품목순번,
            {Text("i.delivery_place")}            AS 납품장소,
            {Text("CAST(i.delivery_days AS TEXT)")} AS 납품일수,
            {Text("i.demand_agency_code")}        AS 수요기관코드,
            {Text("i.request_number")}            AS 조달요구번호,
            {Text("i.origin_country")}            AS 원산지
        FROM contract_item i
        JOIN contract c ON c.contract_base = i.contract_base AND c.seq = i.seq
        {LatestContract};
        "#,
        &[],
    ));

    // 접수 — 줄 하나가 접수서 하나(최신 차수).
    list.push(csharp(
        r#"
        DROP VIEW IF EXISTS v_접수;
        CREATE VIEW v_접수 AS
        {Wrap};
        "#,
        &[("Wrap", &wrap(&request_body, "request", columns, broken)?)],
    ));

    // 접수 품목 — 줄 하나가 조달요구 하나.
    list.push(csharp(
        r#"
        DROP VIEW IF EXISTS v_접수품목;
        CREATE VIEW v_접수품목 AS
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
            {Text("CAST(k.notice_line_no AS TEXT)")}           AS 공고순번,
            -- 확정되어 담기 시작한 필드(ADR-043, 스키마 V28). 기존 필드 순서를 지키도록 끝에 덧붙인다.
            {Text("i.classification_number")}     AS 물품분류번호,
            {Text("i.delivery_unit")}             AS 납품부대명
        FROM request_item i
        JOIN request r ON r.request_base = i.request_base AND r.seq = i.seq
        {LatestRequest}
        LEFT JOIN request_item_link k
               ON k.request_base = i.request_base AND k.request_seq = i.seq
              AND k.line_no = i.line_no;
        "#,
        &[],
    ));

    // 조인 시트 — 줄 하나는 계약이 아니라 조달 건이다(사슬).
    list.push(csharp(
        r#"
        DROP VIEW IF EXISTS v_통합;
        CREATE VIEW v_통합 AS
        {사슬}
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
        LEFT JOIN v_계약 k ON k.계약본번호 = s.계약
        -- 사슬의 공고 자리는 건 이름이다. 공고 열은 그 건의 현행 공고에서 낸다 — 건마다
        -- 공고가 여럿일 수 있으므로 여기서 하나로 좁히지 않으면 줄이 갈라진다.
        LEFT JOIN v_공고 g ON g.공고본번호 = {건현행("s.공고", "현행.notice_base")}
        LEFT JOIN v_접수 r ON r.접수본번호 = s.접수;
        "#,
        &[("unifiedContract", &unified_contract)],
    ));

    // 통합 차수 — 줄 하나가 공고건 × 공고 레코드. 읽기 전용이다.
    list.push(csharp(
        r#"
        DROP VIEW IF EXISTS v_통합차수;
        CREATE VIEW v_통합차수 AS
        WITH 사슬(접수, 공고, 계약) AS (
            -- 공고 문서 한 장이 한 줄이다. 건에 매달린 접수·계약을 그 줄들에 되풀이해 붙인다.
            -- 접수:건이 1:1(request_link.notice_group UNIQUE)이라 접수 조인은 줄을 늘리지 않고,
            -- 계약이 여럿인 건에서만 공고 한 장이 계약 수만큼 갈라진다(v_통합과 같다).
            SELECT rl.request_base, n.notice_base || '-' || n.seq, pl.contract_base
            FROM notice n
            JOIN notice_series 계열 ON 계열.notice_base = n.notice_base
            LEFT JOIN request_link rl ON rl.notice_group = 계열.group_base
            LEFT JOIN project_link pl ON pl.notice_group = 계열.group_base

            UNION ALL

            -- 공고가 아직 없는 접수. 링크가 있는지가 아니라 <b>그 건에 공고가 실제로 서 있는지</b>
            -- 를 본다 — v_통합 의 같은 갈래와 한 글자도 다르지 않다.
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
        LEFT JOIN v_계약 k     ON k.계약본번호 = s.계약
        -- 사슬의 공고 자리는 건이 아니라 <b>공고 한 장</b>이라 차수까지 붙은 번호로 맞춘다.
        LEFT JOIN v_공고차수 g ON g.입찰공고번호 = s.공고
        LEFT JOIN v_접수 r     ON r.접수본번호 = s.접수;
        "#,
        &[("unifiedContract", &unified_contract)],
    ));

    // 계획 — 줄기는 plan. 이어 오는 것은 모두 본번호마다 한 줄인 표로 미리 접어 둔다.
    list.push(csharp(
        r#"
        DROP VIEW IF EXISTS v_계획;
        CREATE VIEW v_계획 AS
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
            -- 공고는 링크가 아니라 <b>그 공고가 실제로 서 있는지</b>를 본다(g 는 v_공고).
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

            -- 언제나 채운다. v_접수 의 「품목수」와 같은 꼴이라 없으면 0 이다.
            {Text("CAST(COALESCE(m.건수, 0) AS TEXT)")} AS 계약건수,

            -- 계약이 하나일 때만. 여럿이면 비우고 계약건수가 몇인지만 알린다.
            {SoleContract("COALESCE(k.계약번호, '')")}  AS 계약번호,
            {SoleContract("COALESCE(k.계약일자, '')")}  AS 계약일자,
            {SoleContract("COALESCE(k.계약금액, '')")}  AS 계약금액
        FROM plan p
        LEFT JOIN 접수짝 q         ON q.번호 = p.request_number
        LEFT JOIN v_접수 r      ON r.접수본번호 = q.본번호
        LEFT JOIN request_link rl  ON rl.request_base = q.본번호
        -- 건마다 공고가 여럿일 수 있다(취소·재공고). <b>현행 하나로 좁히지 않으면</b> 여기서
        -- 줄이 갈라져 계획 건수가 부풀어 세어지고, 분모로 쓰려고 세운 뷰가 분모 노릇을 못 한다.
        LEFT JOIN v_공고 g      ON g.공고본번호 = {건현행("rl.notice_group", "현행.notice_base")}
        LEFT JOIN 계약묶음 m       ON m.공고건 = rl.notice_group
        LEFT JOIN v_계약 k      ON k.계약본번호 = m.본번호;
        "#,
        &[],
    ));

    list.push(csharp(
        r#"
        DROP VIEW IF EXISTS v_ERP원천;
        CREATE VIEW v_ERP원천 AS
        SELECT s.entity_type AS 자료종류, s.entity_base AS 내부키, s.entity_seq AS 차수,
          coalesce(json_extract(r.value,'$.origins.' || f.key),json_extract(s.document_json,'$.profile')) AS 수집프로필,
          json_extract(r.value,'$.table') AS 원천표, json_extract(r.value,'$.sourceKey') AS 원천행키,
          CAST(f.key AS TEXT) AS 필드, CAST(coalesce(f.value,'') AS TEXT) AS 수집값, s.mapping_revision AS 매핑판, s.updated_at AS 수집시각
        FROM erp_source s,
          json_each(CASE WHEN json_valid(s.document_json) THEN s.document_json ELSE json_object() END,'$.rows') r,
          json_each(CASE WHEN r.type = 'object' THEN r.value ELSE json_object() END,'$.values') f;
        DROP VIEW IF EXISTS v_계약업체;
        CREATE VIEW v_계약업체 AS
        SELECT contract_base || seq AS 계약번호, CAST(line_no AS TEXT) AS 순번,
          coalesce(source_id,'') AS 원천업체번호, coalesce(name,'') AS 상호,
          coalesce(business_number,'') AS 사업자등록번호, coalesce(representative,'') AS 대표자,
          coalesce(share_rate,'') AS 계약지분율, coalesce(share_amount,'') AS 계약지분금액
        FROM erp_partner;
        DROP VIEW IF EXISTS v_가공열;
        CREATE VIEW v_가공열 AS
        SELECT CAST(CASE WHEN json_valid(options) THEN json_extract(options, '$.id') ELSE '' END AS TEXT) AS 식별자,
               entity_type AS 자료종류,
               CASE entity_type WHEN 'notice' THEN 'v_공고' WHEN 'contract' THEN 'v_계약' ELSE 'v_접수' END AS 대상뷰,
               field_name AS 열이름,
               {ResultTypes} AS 결과타입,
               options AS 가공식
        FROM user_column WHERE kind = 'computed';
        DROP VIEW IF EXISTS v_ERP접수;
        CREATE VIEW v_ERP접수 AS
        SELECT r.request_base AS 내부키,r.seq AS 접수차수,
          coalesce((SELECT min(ref_request_base) FROM request_item i WHERE i.request_base=r.request_base AND i.seq=r.seq),'') AS ERP접수번호,
          coalesce((SELECT min(request_number) FROM request_item i WHERE i.request_base=r.request_base AND i.seq=r.seq),'') AS 대표조달요구본번호,
          coalesce(r.title,'') AS 요청명,coalesce(r.received_on,'') AS 접수일자
        FROM request r WHERE EXISTS(SELECT 1 FROM erp_source s WHERE s.entity_type='request' AND s.entity_base=r.request_base AND s.entity_seq=r.seq);
        "#,
        &[("ResultTypes", &result_types(columns, broken)?)],
    ));

    Ok(list)
}

/// `Views.UserViewMeta` — `v_맞춤뷰`(docs/user-views.md §6). `types` 는 (뷰 식별자, 필드 식별자) → 끝 꼴. 없는 필드(깨진
/// 식 포함)는 글이다. 갈래 차례는 사전의 차례 그대로다([`Dict`]).
pub(crate) fn user_view_meta(types: &Dict<(String, String), String>) -> String {
    /// 객체인 필드만 — 글·수 원소는 빈 칸이다(C# `Field`).
    const FIELD: &str = "CASE WHEN c.type = 'object' THEN c.value END";
    let cases: Vec<String> = types
        .iter()
        .filter(|(_, v)| matches!(v.as_str(), "number" | "date"))
        .map(|((view, column), value)| {
            format!(
                "WHEN v.id = {} AND json_extract({FIELD}, '$.id') = {} THEN '{value}'",
                flow_compiler::lit(view),
                flow_compiler::lit(column)
            )
        })
        .collect();
    let r#type = if cases.is_empty() {
        "'text'".to_string()
    } else {
        format!("CASE {} ELSE 'text' END", cases.join(" "))
    };
    csharp(
        r#"
            DROP VIEW IF EXISTS v_맞춤뷰;
            CREATE VIEW v_맞춤뷰 AS
            SELECT v.id AS 뷰식별자,
                   '{UserViewPrefix}' || v.name AS 뷰이름,
                   CAST(coalesce(json_extract({Field}, '$.id'), '') AS TEXT) AS 열식별자,
                   CAST(coalesce(json_extract({Field}, '$.name'), '') AS TEXT) AS 열이름,
                   CAST(c.key + 1 AS TEXT) AS 차례,
                   {type} AS 결과타입,
                   CAST(coalesce(json_extract({Field}, '$.formula'), '') AS TEXT) AS 가공식
            -- 손으로 고친 파일의 깨진 정의 하나가 이 뷰 전체를 읽지 못하게 하지 않는다. 객체가 아닌 필드(글·수)는 빈 칸이다 —
            -- 글 원소를 json_extract 에 넘기면 「malformed JSON」 으로 뷰 전체를 읽지 못한다.
            FROM user_view v, json_each(CASE WHEN json_valid(v.definition) THEN v.definition ELSE json_object() END, '$.columns') c;
            "#,
        &[("type", &r#type), ("Field", FIELD)],
    )
}

/// `Views.UserView` — 맞춤 뷰 하나(docs/user-views.md §5). 깨진 필드(`broken`, 필드 식별자 → 까닭)나 식이 없는 필드는
/// 가공열처럼 빈 글 + 까닭 주석으로 선다.
pub(crate) fn user_view(
    name: &str,
    frame: &UserViewFrame,
    columns: &[(String, String, Option<String>)],
    broken: &Dict<String, String>,
) -> String {
    let view = flow_compiler::ident(&format!("{USER_VIEW_PREFIX}{name}"));
    let list: Vec<String> = columns
        .iter()
        .map(
            |(id, name, expression)| match (broken.get(id), expression) {
                (None, Some(expression)) => {
                    format!("{expression} AS {}", flow_compiler::ident(name))
                }
                (why, _) => format!(
                    "{} AS {}",
                    broken_marker(why.map_or("", String::as_str)),
                    flow_compiler::ident(name)
                ),
            },
        )
        .collect();
    format!(
        "DROP VIEW IF EXISTS {view};\nCREATE VIEW {view} AS\n{};",
        frame.statement(&list.join(",\n    "), None, "")
    )
}

/// `Views.ResultTypes` — `v_가공열.결과타입`. 꼴은 꼴 검사가 정하므로 뷰를 지을 때 열마다 적어 둔다. 깨진 열은 글이다.
fn result_types(columns: &[UserColumnDefinition], broken: &BrokenColumns) -> Result<String> {
    let mut cases = Vec::new();
    for c in columns.iter().filter(|c| {
        c.kind == "computed" && !broken.contains_key(&(c.entity_type.clone(), c.field_name.clone()))
    }) {
        let shape =
            match computed_column::final_shape(&c.entity_type, c.options.as_deref().unwrap_or(""))?
            {
                "number" => "number",
                "date" => "date",
                _ => "text",
            };
        cases.push(format!(
            "WHEN entity_type = {} AND field_name = {} THEN '{shape}'",
            flow_compiler::lit(&c.entity_type),
            flow_compiler::lit(&c.field_name)
        ));
    }
    Ok(if cases.is_empty() {
        "'text'".to_string()
    } else {
        format!("CASE {} ELSE 'text' END", cases.join(" "))
    })
}

// ── 덮개의 재료(FaceReferences) ─────────────────────────────

/// `Views.FaceReferences` — 면 열이 본 표의 어느 열에서 오는가(`계약금액 → [contract.amount]`). 열 정의에서 끌어낸다:
/// 덮개로 감싼 열마다 그 표기식이 가리키는 `별칭.열` 을 모으고, 별칭은 바깥 표와 식 안의 `FROM`·`JOIN` 으로 푼다.
/// 잇는 조건과 덮개 표는 재료가 아니라 걷는다. 차례는 열이 처음 지어진 차례, 재료는 서수 차례다.
pub fn face_references(entity_type: &str) -> Vec<(String, Vec<String>)> {
    static SOURCES: OnceLock<FaceSources> = OnceLock::new();
    SOURCES
        .get_or_init(read_face_sources)
        .iter()
        .find(|(e, _)| e == entity_type)
        .map(|(_, columns)| columns.clone())
        .unwrap_or_default()
}

/// 종류마다 (면 열 → 재료 `표.열`들).
type FaceSources = Vec<(String, Vec<(String, Vec<String>)>)>;

fn read_face_sources() -> FaceSources {
    FACES.with(|f| *f.borrow_mut() = Some(Vec::new()));
    let built = definitions(&[], &BrokenColumns::new());
    let faces = FACES.with(|f| f.borrow_mut().take()).unwrap_or_default();
    built.expect("빈 사람 열로는 뷰가 언제나 선다");

    let mut result: FaceSources = Vec::new();
    for (entity, base_column, name, expression) in faces {
        let index = match result.iter().position(|(e, _)| *e == entity) {
            Some(i) => i,
            None => {
                result.push((entity.clone(), Vec::new()));
                result.len() - 1
            }
        };
        let columns = &mut result[index].1;
        if columns.iter().any(|(n, _)| *n == name) {
            continue; // 차수 뷰가 같은 열 한 벌을 다시 짓는다.
        }
        let mut aliases: Vec<(String, String)> = vec![(
            base_column.split('.').next().unwrap_or("").to_string(),
            entity.clone(),
        )];
        for (table, alias) in regex::declared(&expression) {
            match aliases.iter_mut().find(|(a, _)| *a == alias) {
                Some(entry) => entry.1 = table,
                None => aliases.push((alias, table)),
            }
        }
        let body = regex::strip_joins(&expression);
        let mut references: Vec<String> = regex::references(&body)
            .into_iter()
            .filter_map(|(alias, column)| {
                aliases
                    .iter()
                    .find(|(a, _)| *a == alias)
                    .map(|(_, table)| format!("{table}.{column}"))
            })
            .chain(
                regex::bare(&body)
                    .into_iter()
                    .map(|(column, table)| format!("{table}.{column}")),
            )
            .filter(|r| !r.starts_with("field_override."))
            .collect();
        let mut seen = std::collections::HashSet::new();
        references.retain(|r| seen.insert(r.clone()));
        references.sort_by(|a, b| cmp_ordinal(a, b));
        columns.push((name, references));
    }
    result
}

/// [`read_face_sources`] 의 정규식 넷을 손으로 훑는 것. 모두 탐욕 조각이 되짚어도 달라지지 않는 꼴이라 왼쪽부터 한 번
/// 훑어 같은 짝을 얻는다. `\w` 는 .NET 처럼 유니코드 글자·숫자·`_`, `\s` 는 유니코드 공백이다.
mod regex {
    fn word(c: char) -> bool {
        c.is_alphanumeric() || c == '_'
    }

    /// `\b` — 앞이 글자가 아니고 뒤가 글자거나, 그 반대.
    fn boundary(chars: &[char], i: usize) -> bool {
        let before = i > 0 && word(chars[i - 1]);
        let after = i < chars.len() && word(chars[i]);
        before != after
    }

    fn spaces(chars: &[char], mut i: usize) -> usize {
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        i
    }

    /// `[a-z_][a-z0-9_]*` — `fold` 면 대소문자를 가리지 않는다. 끝 자리.
    fn ascii_ident(chars: &[char], i: usize, fold: bool) -> Option<usize> {
        let first =
            |c: char| c == '_' || c.is_ascii_lowercase() || (fold && c.is_ascii_uppercase());
        let rest = |c: char| first(c) || c.is_ascii_digit();
        if i >= chars.len() || !first(chars[i]) {
            return None;
        }
        let mut j = i + 1;
        while j < chars.len() && rest(chars[j]) {
            j += 1;
        }
        Some(j)
    }

    /// `[a-z_]\w*` — 끝 자리.
    fn word_ident(chars: &[char], i: usize, fold: bool) -> Option<usize> {
        let first =
            |c: char| c == '_' || c.is_ascii_lowercase() || (fold && c.is_ascii_uppercase());
        if i >= chars.len() || !first(chars[i]) {
            return None;
        }
        let mut j = i + 1;
        while j < chars.len() && word(chars[j]) {
            j += 1;
        }
        Some(j)
    }

    fn keyword(chars: &[char], i: usize, word: &str) -> Option<usize> {
        let n = word.chars().count();
        if i + n > chars.len() {
            return None;
        }
        chars[i..i + n]
            .iter()
            .zip(word.chars())
            .all(|(a, b)| a.eq_ignore_ascii_case(&b))
            .then_some(i + n)
    }

    fn text(chars: &[char], from: usize, to: usize) -> String {
        chars[from..to].iter().collect()
    }

    /// `\b(?:FROM|JOIN)\s+([a-z_][a-z0-9_]*)\s+(?!ON\b|WHERE\b)([a-z_][a-z0-9_]*)\b`(IgnoreCase) — (표, 별칭).
    pub(super) fn declared(expression: &str) -> Vec<(String, String)> {
        let chars: Vec<char> = expression.chars().collect();
        let mut found = Vec::new();
        let mut i = 0;
        while i < chars.len() {
            let hit = (|| {
                if !boundary(&chars, i) {
                    return None;
                }
                let k = keyword(&chars, i, "FROM").or_else(|| keyword(&chars, i, "JOIN"))?;
                let t0 = spaces(&chars, k);
                if t0 == k {
                    return None;
                }
                let t1 = ascii_ident(&chars, t0, true)?;
                let a0 = spaces(&chars, t1);
                if a0 == t1 {
                    return None;
                }
                for stop in ["ON", "WHERE"] {
                    if let Some(e) = keyword(&chars, a0, stop)
                        && boundary(&chars, e)
                    {
                        return None;
                    }
                }
                let a1 = ascii_ident(&chars, a0, true)?;
                if !boundary(&chars, a1) {
                    return None;
                }
                Some(((text(&chars, t0, t1), text(&chars, a0, a1)), a1))
            })();
            match hit {
                Some((pair, end)) => {
                    found.push(pair);
                    i = end;
                }
                None => i += 1,
            }
        }
        found
    }

    /// `\b[a-z_]\w*\.[a-z_]\w*\s*=\s*[a-z_]\w*\.[a-z_]\w*|\b[a-z_]\w*\.[a-z_]\w*\s+LIKE\s+'[^']*'`(IgnoreCase)를 `" "` 로.
    pub(super) fn strip_joins(expression: &str) -> String {
        let chars: Vec<char> = expression.chars().collect();
        let dotted = |i: usize| -> Option<usize> {
            let a = word_ident(&chars, i, true)?;
            if chars.get(a) != Some(&'.') {
                return None;
            }
            word_ident(&chars, a + 1, true)
        };
        let mut out = String::with_capacity(expression.len());
        let mut i = 0;
        while i < chars.len() {
            let hit = (|| {
                if !boundary(&chars, i) {
                    return None;
                }
                let left = dotted(i)?;
                // 첫 갈래 — 같음으로 잇는 것.
                let eq = spaces(&chars, left);
                if chars.get(eq) == Some(&'=')
                    && let Some(end) = dotted(spaces(&chars, eq + 1))
                {
                    return Some(end);
                }
                // 둘째 갈래 — LIKE '…'.
                let k0 = spaces(&chars, left);
                if k0 == left {
                    return None;
                }
                let k1 = keyword(&chars, k0, "LIKE")?;
                let q = spaces(&chars, k1);
                if q == k1 || chars.get(q) != Some(&'\'') {
                    return None;
                }
                let close = chars[q + 1..].iter().position(|&c| c == '\'')?;
                Some(q + 1 + close + 1)
            })();
            match hit {
                Some(end) => {
                    out.push(' ');
                    i = end;
                }
                None => {
                    out.push(chars[i]);
                    i += 1;
                }
            }
        }
        out
    }

    /// `\b([a-z_]\w*)\.([a-z_][a-z0-9_]*)\b` — (별칭, 열).
    pub(super) fn references(body: &str) -> Vec<(String, String)> {
        let chars: Vec<char> = body.chars().collect();
        let mut found = Vec::new();
        let mut i = 0;
        while i < chars.len() {
            let hit = (|| {
                if !boundary(&chars, i) {
                    return None;
                }
                let a = word_ident(&chars, i, false)?;
                if chars.get(a) != Some(&'.') {
                    return None;
                }
                let c = ascii_ident(&chars, a + 1, false)?;
                if !boundary(&chars, c) {
                    return None;
                }
                Some(((text(&chars, i, a), text(&chars, a + 1, c)), c))
            })();
            match hit {
                Some((pair, end)) => {
                    found.push(pair);
                    i = end;
                }
                None => i += 1,
            }
        }
        found
    }

    /// `\bSELECT\s+([a-z_][a-z0-9_]*)\s+FROM\s+([a-z_][a-z0-9_]*)\b` — (열, 표).
    pub(super) fn bare(body: &str) -> Vec<(String, String)> {
        let chars: Vec<char> = body.chars().collect();
        let exact = |i: usize, w: &str| -> Option<usize> {
            let n = w.chars().count();
            (i + n <= chars.len() && chars[i..i + n].iter().copied().eq(w.chars())).then_some(i + n)
        };
        let mut found = Vec::new();
        let mut i = 0;
        while i < chars.len() {
            let hit = (|| {
                if !boundary(&chars, i) {
                    return None;
                }
                let s = exact(i, "SELECT")?;
                let c0 = spaces(&chars, s);
                if c0 == s {
                    return None;
                }
                let c1 = ascii_ident(&chars, c0, false)?;
                let f0 = spaces(&chars, c1);
                if f0 == c1 {
                    return None;
                }
                let f1 = exact(f0, "FROM")?;
                let t0 = spaces(&chars, f1);
                if t0 == f1 {
                    return None;
                }
                let t1 = ascii_ident(&chars, t0, false)?;
                if !boundary(&chars, t1) {
                    return None;
                }
                Some(((text(&chars, c0, c1), text(&chars, t0, t1)), t1))
            })();
            match hit {
                Some((pair, end)) => {
                    found.push(pair);
                    i = end;
                }
                None => i += 1,
            }
        }
        found
    }
}

// ── C# 자리 글 풀기 ─────────────────────────────────────────

/// C# 원시 문자열 몸을 걷고 채운다 — `args` 에 없는 자리는 [`eval`] 이 C# 의 부름으로 푼다.
fn csharp(template: &str, args: &[(&str, &str)]) -> String {
    raw_with(template, args, &|key| eval(key, args))
}

/// C# 자리 글 하나를 푼다 — 이 모듈의 표기 함수 부름(`Text("n.title")`), 글자(`"…"`), 보간 글(`$"…{…}…"`), 이름(인자나
/// 상수)뿐이다. 풀 수 없으면 `None`(짓는 쪽의 잘못이라 [`raw_with`] 가 멈춘다).
fn eval(expr: &str, args: &[(&str, &str)]) -> Option<String> {
    let mut parser = Parser {
        chars: expr.chars().collect(),
        i: 0,
        args,
    };
    let value = parser.expr()?;
    parser.space();
    (parser.i == parser.chars.len()).then_some(value)
}

struct Parser<'a> {
    chars: Vec<char>,
    i: usize,
    args: &'a [(&'a str, &'a str)],
}

impl Parser<'_> {
    fn space(&mut self) {
        while self.chars.get(self.i).is_some_and(|c| c.is_whitespace()) {
            self.i += 1;
        }
    }

    fn eat(&mut self, c: char) -> bool {
        self.space();
        if self.chars.get(self.i) == Some(&c) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn expr(&mut self) -> Option<String> {
        self.space();
        match self.chars.get(self.i)? {
            '$' => {
                self.i += 1;
                if self.chars.get(self.i) != Some(&'"') {
                    return None;
                }
                self.i += 1;
                self.interpolated()
            }
            '"' => {
                self.i += 1;
                let start = self.i;
                while *self.chars.get(self.i)? != '"' {
                    self.i += 1;
                }
                self.i += 1;
                Some(self.chars[start..self.i - 1].iter().collect())
            }
            _ => {
                let start = self.i;
                while self
                    .chars
                    .get(self.i)
                    .is_some_and(|&c| c.is_alphanumeric() || c == '_' || c == '.')
                {
                    self.i += 1;
                }
                let name: String = self.chars[start..self.i].iter().collect();
                if name.is_empty() {
                    return None;
                }
                if !self.eat('(') {
                    return self.name(&name);
                }
                let mut values = Vec::new();
                if !self.eat(')') {
                    loop {
                        values.push(self.expr()?);
                        if self.eat(')') {
                            break;
                        }
                        if !self.eat(',') {
                            return None;
                        }
                    }
                }
                call(&name, &values)
            }
        }
    }

    /// `$"…"` 의 몸 — `{…}` 를 풀어 끼운다.
    fn interpolated(&mut self) -> Option<String> {
        let mut out = String::new();
        loop {
            let c = *self.chars.get(self.i)?;
            self.i += 1;
            match c {
                '"' => return Some(out),
                '{' => {
                    let start = self.i;
                    let mut depth = 1;
                    while depth > 0 {
                        match self.chars.get(self.i)? {
                            '{' => depth += 1,
                            '}' => depth -= 1,
                            _ => {}
                        }
                        self.i += 1;
                    }
                    let inner: String = self.chars[start..self.i - 1].iter().collect();
                    out.push_str(&eval(&inner, self.args)?);
                }
                _ => out.push(c),
            }
        }
    }

    fn name(&self, name: &str) -> Option<String> {
        if let Some((_, value)) = self.args.iter().find(|(n, _)| *n == name) {
            return Some(value.to_string());
        }
        Some(match name {
            "LatestNotice" => latest_notice(),
            "LatestContract" => latest_contract(),
            "LatestRequest" => latest_request(),
            "PlanFolds" => plan_folds(),
            "사슬" => chain(),
            "RequestNumbers" => request_numbers(),
            "RelatedNotices" => related_notices(),
            "RestrictedIndustries" => restricted_industries(),
            "WarrantyYears" => warranty_years(),
            "NoticeGroups.현행공고" => notice_groups::current_notice(),
            "UserViewPrefix" => USER_VIEW_PREFIX.to_string(),
            _ => return None,
        })
    }
}

/// C# 표기 함수 부름 하나.
fn call(name: &str, a: &[String]) -> Option<String> {
    Some(match (name, a) {
        ("Text", [x]) => text(x),
        ("YesNo", [x]) => yes_no(x),
        ("YesNoWord", [x]) => yes_no_word(x),
        ("Number", [x]) => number(x),
        ("DateText", [x]) => date_text(x),
        ("Money", [x]) => money(x),
        ("UniformItem", [x]) => uniform_item(x),
        ("UniformQuantity", []) => uniform_quantity(),
        ("UniformRequestItem", [x]) => uniform_request_item(x),
        ("UniformRequestQuantity", []) => uniform_request_quantity(),
        ("RequestDeadline", [due, days]) => request_deadline(due, days),
        ("DeliveryDeadline", [due, days]) => delivery_deadline(due, days),
        ("SoleContact", [x]) => sole_contact(x),
        ("WarrantyText", [x]) => warranty_text(x),
        ("WarrantyPhrase", [x]) => warranty_phrase(x),
        ("Step", [column, keyword]) => step(column, keyword),
        ("공고건", [x]) => notice_group(x),
        ("건현행", [group, pick]) => group_current(group, pick),
        ("SoleContract", [x]) => sole_contract(x),
        ("PartyField", [x]) => party_field(x),
        ("PartnerOnly", [x]) => partner_only(x),
        ("NoticeGroups.Kind", [kind, status]) => notice_groups::kind(kind, status),
        ("N", [name, x]) => n(name, x),
        ("C", [name, x]) => c(name, x),
        ("R", [name, x]) => r(name, x),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eval_reads_csharp_holes() {
        assert_eq!(
            eval("Text(\"n.title\")", &[]).as_deref(),
            Some("COALESCE(n.title, '')")
        );
        assert_eq!(
            eval("Text($\"a {Text(\"b\")} c\")", &[]).as_deref(),
            Some("COALESCE(a COALESCE(b, '') c, '')")
        );
        assert_eq!(
            eval("SoleContract(\"COALESCE(k.x, '')\")", &[]).as_deref(),
            Some("CASE WHEN m.건수 = 1 THEN COALESCE(k.x, '') ELSE '' END")
        );
        assert_eq!(eval("Nope(\"x\")", &[]), None);
    }

    #[test]
    fn broken_in_reads_markers_like_the_regex() {
        let sql = format!(
            "SELECT {} AS \"가\"\"나\", {} AS \"다\" FROM x",
            broken_marker("까닭 */ 하나\n둘"),
            broken_marker("")
        );
        assert_eq!(
            broken_in(Some(&sql)),
            [
                ("가\"나".to_string(), "까닭 * / 하나 둘".to_string()),
                ("다".to_string(), String::new())
            ]
        );
        assert!(broken_in(None).is_empty());
    }

    #[test]
    fn choices_split_and_trim() {
        let c = UserColumnDefinition {
            entity_type: "notice".into(),
            field_name: "f".into(),
            kind: "choice".into(),
            options: Some(" 가 | |나|".into()),
            sort_order: 0,
        };
        assert_eq!(c.choices(), ["가", "나"]);
    }
}

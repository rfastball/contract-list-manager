//! 제출본을 모아 취합본 하나로 짓는다(C# `Storage/Merger.cs`, ADR-024·031).
//!
//! **왜 공유 DB 하나가 아닌가.** 담당자마다 자기 DB 가 하나씩 있는데 그것들을 모을 길이 없었다. 그렇다고 네트워크
//! 드라이브에 DB 하나를 두고 여럿이 붙을 수는 없다 — WAL 은 네트워크 파일시스템에서 돌지 않고(ADR-006), 계약면이
//! **쓰는 주체는 하나**라고 약속했다. 웹ERP 에 닿을 수 없어 시작된 저장소라, 취합도 파일로 한다.
//!
//! **왜 매번 처음부터 짓는가.** 이 저장소는 **묘비를 두지 않는다**. 누가 자기 DB 에서 지운 건은 다음 취합본에 그냥
//! 없어야 하는데, 지난 취합본을 고쳐 쓰면 지운 것이 영영 남는다. 그래서 취합본은 결과물이지 자료가 아니다 — 언제든
//! 버리고 다시 지어도 같은 것이 나온다.
//!
//! **제출본은 남의 파일이라 손대지 않는다.** 읽기 전용 연결의 `VACUUM INTO` 로 작업 폴더에 한 벌 떠서 열고, 끝나면
//! 지운다. 옛 판 제출본은 그 사본 위에서 지금 판으로 올려 받는다(판올림은 앞으로만 간다). 판이 지금보다 새로운 것은
//! 내릴 길이 없고, 기준선보다 옛 시험판은 올릴 단계가 이 프로그램에 없어 **거절하되 멈추지는 않는다** — 나머지는
//! 합친다.
//!
//! **무엇을 받을지는 파일 이름이 아니라 역할이 정한다**([`pclm_file::inspect`]). 제출본과 작업자료(사람이 자기 파일을
//! 그대로 보내는 일이 있다)만 받는다. 취합본·백업·옮겨진 옛 자료는 이름이 무엇이든 거절한다 — 한동안 `취합_` 접두사로
//! 가렸는데, 이름을 바꾼 취합본은 그대로 먹혔고 그 글자로 시작하는 제출본은 말없이 빠졌다.
//!
//! **C# 과 다른 자리(#8).** C# 은 임시 폴더(`Path.GetTempPath()` 아래 `pclm-merge-…`)를 스스로 짓고 지운다. 이
//! 크레이트는 환경을 보지 않으므로 작업 폴더를 부르는 쪽이 준다 — 취합은 그 안에 사본을 짓고 끝나면 그 파일들만
//! 지운다. 경로도 C# 처럼 `Path.GetFullPath` 로 고치지 않는다 — 온전한 경로는 부르는 쪽이 준다.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, params_from_iter};

use crate::env::Env;
use crate::erp::explicit_links;
use crate::error::{Error, Result};
use crate::json::format_double;
use crate::storage::database::{Access, Database, begin, with_suffix};
use crate::storage::pclm_file::{self, PclmInfo, PclmKind, PclmRole};
use crate::text::{cmp_ordinal, eq_ignore_case, ignore_case_key};

/// C# `MergeSide` — 겹친 자리의 한쪽. 누구의 제출본에서 왔고 그 값이 언제 적힌 것인가.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MergeSide {
    /// `제출자`.
    pub submitter: String,
    /// `시각` — 표에 적힌 그대로의 문자열. 시각 열이 없는 표에서는 빈 문자열이다.
    pub time: String,
}

/// C# `MergeConflict` — 같은 키가 둘 이상의 제출본에 있었다는 **사실**.
///
/// **기계는 고를 뿐 판정하지 않는다**(ADR-016 의 결). 무엇이 옳은지는 두 사람이 안다. 그래서 취합은 하나를 싣되
/// **무엇을 밀어냈는지 반드시 알린다** — 세기만 하고 어느 것인지 내지 못하면 밀려난 쪽은 오류 없이 사라진다.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MergeConflict {
    /// `종류` — 어느 표에서 겹쳤나. 사람이 읽을 이름(`접수`·`덮개`…).
    pub kind: String,
    /// `키` — 그 표의 자연키. 열이 여럿이면 `·` 로 잇는다.
    pub key: String,
    /// `실림`.
    pub kept: MergeSide,
    /// `밀림`.
    pub pushed: Vec<MergeSide>,
}

/// C# `MergeRejection` — 취합에 넣지 않은 파일 하나. **까닭을 함께 든다** — 이름만 적으면 받는 사람은 프로그램을
/// 새로 받아야 하는지, 파일을 잘못 골랐는지 가리지 못한다.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MergeRejection {
    /// `파일` — 파일 이름(경로 없이).
    pub file: String,
    /// `까닭` — 사람이 읽을 짧은 까닭([`TOO_OLD`]·[`TOO_NEW`] 따위).
    pub reason: String,
}

/// C# `MergeReport` — 취합 한 번의 결과. 수는 **다 지은 취합본에서** 세어 `pclm 현황` 과 맞춘다.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MergeReport {
    /// `제출본` — 실제로 합친 제출본 수. 거절한 것은 여기 들지 않는다.
    pub submissions: i32,
    /// `조달요구`.
    pub plans: i32,
    /// `접수`.
    pub requests: i32,
    /// `공고`.
    pub notices: i32,
    /// `계약`.
    pub contracts: i32,
    /// `거절한제출본` — 받지 않은 파일과 그 까닭. PCLM 이 아니거나, 판이 맞지 않거나, 제출본이 아닌 역할(취합본·
    /// 백업·옮겨진 옛 자료)이다.
    pub rejected: Vec<MergeRejection>,
    /// `겹친것`.
    pub conflicts: Vec<MergeConflict>,
}

/// 거절 까닭(`Merger.PCLM아님`). 보고 글이 그대로 적는다.
pub const NOT_PCLM: &str = "PCLM 파일이 아님";
/// `Merger.옛시험판`.
pub const TOO_OLD: &str = "옛 시험판";
/// `Merger.새판`.
pub const TOO_NEW: &str = "새 판";
/// `Merger.파일없음`.
pub const NOT_FOUND: &str = "파일이 없음";

/// 겹쳤을 때 무엇을 싣는가.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Rule {
    /// 시각이 늦은 쪽. 같으면 먼저 온 제출본. 둘 다 **겹친 것으로 알린다**.
    Later,
    /// 합집합. 같은 키는 같은 것이라 고를 일이 없다(계열 키).
    Union,
    /// 부모를 따라 통째로. 딸린 줄에는 시각이 없어 낱개로 고를 수 없고, 반씩 섞으면 **어느 문서에도 없던 표**가
    /// 생긴다.
    FollowParent,
    FollowEntity,
    /// 합집합이되 **뜻이 다르면 알린다**. 사람마다 세운 열이 달라 이름이 같으면 하나로, 다르면 둘 다 세운다. 이름이
    /// 같은데 `kind`·`options` 가 다르면 먼저 온 것을 두고 겹친 것으로 알린다 — 누가 옳은지 기계가 판정하지 않는다.
    Definition,
}

/// 나르는 표 하나. `parent` 는 딸린 줄일 때 따라갈 표다 — 부모 키는 이 표 키의 **앞 두 열**이다.
struct Table {
    name: &'static str,
    key: &'static [&'static str],
    time: Option<&'static str>,
    rule: Rule,
    kind: &'static str,
    parent: Option<&'static str>,
}

const fn table(
    name: &'static str,
    key: &'static [&'static str],
    time: Option<&'static str>,
    rule: Rule,
    kind: &'static str,
) -> Table {
    Table {
        name,
        key,
        time,
        rule,
        kind,
        parent: None,
    }
}

const fn child(
    name: &'static str,
    key: &'static [&'static str],
    kind: &'static str,
    parent: &'static str,
) -> Table {
    Table {
        name,
        key,
        time: None,
        rule: Rule::FollowParent,
        kind,
        parent: Some(parent),
    }
}

const UPDATED: Option<&str> = Some("updated_at");

/// 살아 있는 표와 그 규칙(`Merger.표들`). **`app_setting` 은 여기 없다 — 싣지 않는다.**
///
/// 제출한 사람의 이름·골라 둔 계획 엑셀이 취합본에 섞이면, 취합본을 연 사람이 남의 설정을 자기 것으로 보게 된다.
/// 제출자 이름만 읽고 옮기지는 않는다.
///
/// 차례는 **부모가 먼저**다. 딸린 줄이 부모의 승자를 보고 따라가므로, 뒤집히면 따라갈 것이 아직 없다.
const TABLES: &[Table] = &[
    table(
        "counterparty",
        &["business_number"],
        UPDATED,
        Rule::Later,
        "상대자",
    ),
    // 계획 엑셀은 부서마다 따로 돌아 같은 조달요구번호가 다른 제출본에서 온다.
    table(
        "plan",
        &["request_number"],
        Some("imported_at"),
        Rule::Later,
        "계획",
    ),
    table(
        "request_series",
        &["request_base"],
        None,
        Rule::Union,
        "접수 계열",
    ),
    // group_base 는 파생이라 여기서 고르지 않는다 — 제출본마다 다른 이름이 와도 합집합은 겹침을 알리지 않고, 취합
    // 뒤 Rebuild 가 어차피 다시 계산한다.
    table(
        "notice_series",
        &["notice_base"],
        None,
        Rule::Union,
        "공고 계열",
    ),
    table(
        "contract_series",
        &["contract_base"],
        None,
        Rule::Union,
        "계약 계열",
    ),
    table(
        "request",
        &["request_base", "seq"],
        UPDATED,
        Rule::Later,
        "접수",
    ),
    table(
        "notice",
        &["notice_base", "seq"],
        UPDATED,
        Rule::Later,
        "공고",
    ),
    table(
        "contract",
        &["contract_base", "seq"],
        UPDATED,
        Rule::Later,
        "계약",
    ),
    table(
        "erp_source",
        &["entity_type", "entity_base", "entity_seq"],
        None,
        Rule::FollowEntity,
        "ERP 출처",
    ),
    table(
        "erp_row",
        &[
            "entity_type",
            "entity_base",
            "entity_seq",
            "table_name",
            "source_key",
        ],
        None,
        Rule::FollowEntity,
        "원천 행 대응",
    ),
    child(
        "request_item",
        &["request_base", "seq", "line_no"],
        "접수 품목",
        "request",
    ),
    child(
        "notice_item",
        &["notice_base", "seq", "line_no"],
        "공고 품목",
        "notice",
    ),
    child(
        "notice_schedule",
        &["notice_base", "seq", "line_no"],
        "공고 일정",
        "notice",
    ),
    child(
        "notice_officer_contact",
        &["notice_base", "seq", "line_no"],
        "수요기관 담당자",
        "notice",
    ),
    child(
        "notice_relation",
        &["notice_base", "seq", "line_no"],
        "관련공고",
        "notice",
    ),
    child(
        "notice_restricted_industry",
        &["notice_base", "seq", "line_no"],
        "입찰참가 제한 업종",
        "notice",
    ),
    child(
        "contract_item",
        &["contract_base", "seq", "line_no"],
        "계약 품목",
        "contract",
    ),
    child(
        "erp_notice_attachment",
        &["notice_base", "seq", "line_no"],
        "공고 첨부",
        "notice",
    ),
    child(
        "erp_partner",
        &["contract_base", "seq", "line_no"],
        "계약 업체",
        "contract",
    ),
    child(
        "contract_attachment",
        &["contract_base", "seq", "line_no"],
        "계약 첨부",
        "contract",
    ),
    // 사람 값은 계열 키라 차수를 타지 않는다. 그대로 따라온다.
    table(
        "request_user_field",
        &["request_base", "field_name"],
        UPDATED,
        Rule::Later,
        "접수 사람값",
    ),
    table(
        "notice_user_field",
        &["notice_base", "field_name"],
        UPDATED,
        Rule::Later,
        "공고 사람값",
    ),
    table(
        "contract_user_field",
        &["contract_base", "field_name"],
        UPDATED,
        Rule::Later,
        "계약 사람값",
    ),
    table(
        "field_override",
        &["entity_type", "base", "seq", "column_name"],
        UPDATED,
        Rule::Later,
        "덮개",
    ),
    table(
        "user_column",
        &["entity_type", "field_name"],
        None,
        Rule::Definition,
        "사용자 열",
    ),
    // 맞춤 뷰도 정의라 같은 규칙이다 — 열쇠는 이름(docs/user-views.md §4).
    table("user_view", &["name"], None, Rule::Definition, "맞춤 뷰"),
];

/// `Merger.Tables` — 취합이 **나르는** 표. 시험이 이것과 실제 스키마를 견줘, 표가 하나 생겼는데 여기를 고치지 않은
/// 것을 잡는다 — 빠뜨리면 그 표가 취합본에서 **조용히** 사라진다.
pub fn tables() -> Vec<&'static str> {
    TABLES.iter().map(|t| t.name).collect()
}

/// `Merger.Untouched` — 취합이 **일부러** 두고 오는 표. 늘리려면 까닭이 있어야 한다.
///
/// `notice_group` 은 **파생 표**라 나르지 않는다. 제출본마다 가진 문서가 달라 같은 건이 제각각 이름을 달고 오는데,
/// 그것은 참고값일 뿐 취합본의 건이 아니다 — 값은 취합 끝의 명시 참조 해소가 짓는다. 수집 영수증의 datasetId·충돌
/// 토큰은 원래 DB 에서만 유효하다 — 취합본은 새 datasetId 를 가지며 남의 영수증으로 저장 성공을 응답하지 않는다.
/// 파일 이름표(`pclm_file`)는 취합본이 스스로 짓는다 — 역할도 신원도 제출본의 것이 아니다.
pub const UNTOUCHED: [&str; 8] = [
    "app_setting",
    "notice_group",
    "project_link",
    "request_link",
    "request_item_link",
    "pclm_file",
    "erp_capture",
    "erp_mapping",
];

/// 제출본에서 읽은 줄 하나(`후보`). 어느 제출본에서 왔는지를 값과 함께 든다.
#[derive(Clone)]
struct Candidate {
    submission: usize,
    time: String,
    values: Vec<Value>,
}

/// 표 안에서 키를 잇는 글자. 사람에게 보일 때는 `·` 로 바꾼다.
const KEY_JOIN: char = '\u{1}';

/// `Merger.FindSubmissions` — 폴더에서 `.pclm` 을 걷는다. 걷는 자리가 명령줄과 창 두 곳이라 여기 둔다.
///
/// **여기서는 가리지 않는다.** 지난 취합본이 함께 걸려도 [`merge`] 가 역할을 보고 거절하며 보고에 적는다 — 취합본이
/// 취합본을 먹으면 지난 취합에서 밀려난 것이 남의 이름을 달고 되살아나는데, 그것을 막는 것은 이름이 아니라
/// `pclm_file.role` 이다.
///
/// 폴더가 없으면 빈 목록이다 — 부르는 쪽이 "제출본이 없다" 는 말을 하게 한다. 경로는 `folder` 에 이름을 붙인 것이다
/// (C# 은 `Path.GetFullPath` 로 온전하게 고친다 — 온전한 폴더는 부르는 쪽이 준다). 차례와 겹침은 C# 처럼 대소문자를
/// 가리지 않는다(`StringComparer.OrdinalIgnoreCase`). 확장자도 Windows 의 `*.pclm` 처럼 대소문자를 가리지 않는다.
pub fn find_submissions(folder: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut found: Vec<(String, PathBuf)> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if !name.ends_with(".pclm") || !path.is_file() {
            continue;
        }
        let key = ignore_case_key(&path.to_string_lossy());
        if found.iter().all(|(k, _)| k != &key) {
            found.push((key, path));
        }
    }
    found.sort_by(|a, b| cmp_ordinal(&a.0, &b.0));
    found.into_iter().map(|(_, path)| path).collect()
}

/// `Merger.Merge` — 제출본들을 모아 `out_path` 에 취합본 하나를 짓는다.
///
/// `out_path` 에 파일이 있으면 지우고 처음부터 짓는다 — 고쳐 쓰지 않는 까닭은 이 모듈 머리에 있다. 제출본 목록에
/// 취합본 자리가 섞여 있으면 그것은 뺀다.
///
/// `work` 는 부르는 쪽이 지어 준 폴더다. 취합은 그 안에 `취합.pclm`·`000.pclm`… 을 짓고 끝나면(실패해도) 그 파일들을
/// 지운다 — 폴더는 부르는 쪽이 치운다. 같은 폴더를 두 취합이 함께 쓰면 안 된다. 취합본의 신원·시각·새 식별자는
/// `env` 에서 받는다.
///
/// 받아들인 제출본이 하나도 없으면 **결과물에 닿기 전에** 까닭을 들어 멈춘다([`Error::InvalidOperation`]).
pub fn merge<P: AsRef<Path>>(
    sources: &[P],
    out_path: &Path,
    work: &Path,
    env: &dyn Env,
) -> Result<MergeReport> {
    // 다 지은 뒤 덮는 자리에서도 막히지만, 그때는 제출본을 다 읽은 뒤다. 덮을 수 없는 자리면 먼저 멈춘다.
    pclm_file::require_overwritable(out_path)?;

    let target = out_path.to_string_lossy();
    let files: Vec<&Path> = sources
        .iter()
        .map(AsRef::as_ref)
        .filter(|f| !eq_ignore_case(&f.to_string_lossy(), &target))
        .collect();

    let mut made: Vec<PathBuf> = Vec::new();
    let result = build(&files, out_path, work, env, &mut made);

    // rusqlite 에는 연결 풀이 없다 — 연결은 여기까지 모두 놓였다. 못 지운 사본은 둔다(취합본은 성하다).
    for path in &made {
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let _ = std::fs::remove_file(with_suffix(path, suffix));
        }
    }
    result
}

fn build(
    files: &[&Path],
    target: &Path,
    work: &Path,
    env: &dyn Env,
    made: &mut Vec<PathBuf>,
) -> Result<MergeReport> {
    // 작업 폴더에서 짓고 마지막에 한 벌 떠서 자리에 놓는다. 제출본과 같은 길이다(pclm_file::snapshot 의 VACUUM
    // INTO) — 그래야 취합본도 -wal 없는 한 파일로 떨어진다.
    let building = work.join("취합.pclm");
    made.push(building.clone());
    let database = pclm_file::create(&building, PclmRole::MERGED, env)?;

    // 열 목록은 취합본에서 읽는다. 제출본은 올려서 여니 같은 판이고, 어긋나면 읽는 자리에서 시끄럽게 실패한다.
    let columns: Vec<Vec<String>> = TABLES
        .iter()
        .map(|t| columns_of(&database, t.name))
        .collect::<Result<_>>()?;
    let mut pools: Vec<HashMap<String, Vec<Candidate>>> =
        TABLES.iter().map(|_| HashMap::new()).collect();

    let mut submitters: Vec<String> = Vec::new();
    let mut rejected: Vec<MergeRejection> = Vec::new();

    // 복사본 이름은 제출본의 차례로 짓는다. 거절한 것도 한 자리를 쓴다(C# 과 같다).
    let mut order = 0;

    for file in files {
        // 받을 것인지는 원본을 읽기만 해서 정한다 — 판올림보다 먼저다.
        if let Some(reason) = rejected_reason(&pclm_file::inspect(file)) {
            rejected.push(MergeRejection {
                file: file_name(file),
                reason,
            });
            continue;
        }

        // 남의 파일이다. 읽기 전용 연결로 한 벌 떠서 열고, 옛 판이면 사본 위에서 올린다. 파일 셋(본체·-wal·-shm)을
        // 따로 복사하지 않는다 — VACUUM INTO 는 WAL 에 남은 마지막 것까지 커밋된 한 시점으로 뜬다.
        let copy = work.join(format!("{order:03}.pclm"));
        order += 1;
        made.push(copy.clone());
        Database::new(*file, Access::Read).snapshot(&copy, false)?;

        let source = Database::new(&copy, Access::Write);
        source.migrate(env)?;

        let connection = source.open_read_only()?;
        let index = submitters.len();
        let name = submitter(&connection)?;
        submitters.push(if name.is_empty() {
            file_stem(file)
        } else {
            name
        });

        for (i, t) in TABLES.iter().enumerate() {
            collect(&connection, t, &columns[i], index, &mut pools[i])?;
        }
    }

    // 조용히 빈 취합본을 만들지 않는다 — 받는 사람은 그것을 취합 결과로 안다. 결과물에 닿기 전에 멈추고 까닭을 든다.
    if submitters.is_empty() {
        return Err(Error::invalid(if rejected.is_empty() {
            "합칠 수 있는 제출본이 없습니다 — 넣을 파일이 없습니다.".to_string()
        } else {
            format!(
                "합칠 수 있는 제출본이 없습니다 — 파일 {}개를 모두 넣지 않았습니다: {}",
                rejected.len(),
                rejected
                    .iter()
                    .map(|r| format!("{}({})", r.file, r.reason))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }));
    }

    let mut conflicts: Vec<MergeConflict> = Vec::new();
    let mut owners: HashMap<&'static str, HashMap<String, usize>> = HashMap::new();
    let mut kept: Vec<Vec<Candidate>> = Vec::with_capacity(TABLES.len());

    for (i, t) in TABLES.iter().enumerate() {
        let pool = std::mem::take(&mut pools[i]);
        kept.push(resolve(
            t,
            &columns[i],
            pool,
            &submitters,
            &mut owners,
            &mut conflicts,
        ));
    }

    // user_view.id 는 PRIMARY KEY 다. 한 사람이 이름을 바꾼 뷰가 다른 제출본에 옛 이름으로 있으면 같은 식별자가 둘이 된다.
    let view = TABLES
        .iter()
        .position(|t| t.name == "user_view")
        .expect("user_view 는 나르는 표다");
    let views = std::mem::take(&mut kept[view]);
    kept[view] = one_id_per_view(&columns[view], views, env);

    write(&database, &columns, &kept)?;

    // 사람이 세운 열이 제출본에서 왔다. 뷰는 그 정의를 읽어 짓는 것이라 다시 지어야 취합본의 계약면에 그 칸이 선다.
    database.refresh_views()?;

    // 수는 다 지은 취합본에서 센다 — pclm 현황 의 「쌓인 것」과 같은 셈이라 어긋나면 안 된다.
    let report = {
        let opened = database.open()?;
        let report = MergeReport {
            submissions: submitters.len() as i32,
            plans: count(&opened, "SELECT COUNT(*) FROM plan;")?,
            requests: count(&opened, "SELECT COUNT(DISTINCT request_base) FROM request;")?,
            notices: count(&opened, "SELECT COUNT(DISTINCT notice_base) FROM notice;")?,
            contracts: count(
                &opened,
                "SELECT COUNT(DISTINCT contract_base) FROM contract;",
            )?,
            rejected,
            conflicts,
        };
        // WAL 을 본체로 접는다. 뒤이어 뜨는 한 벌은 어차피 정합하지만, 접어 두면 사본을 지울 때 걸릴 것이 줄어든다.
        opened.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        report
    };

    // 사람이 자리를 짚어 만든 결과물이라 이미 있으면 덮는다. 뜬 사본은 자기 신원을 새로 받는다.
    pclm_file::snapshot(&database, target, PclmRole::MERGED, true, env)?;
    Ok(report)
}

/// `Merger.Rejected` — 받지 않을 까닭. 받으면 `None`.
///
/// 받는 것은 제출본과 작업자료다. 취합본을 받으면 지난 취합에서 밀려난 것이 되살아나고, 백업과 옮겨진 옛 자료는
/// 어느 작업자료의 옛 모습이라 같은 사람의 것이 두 번 들어간다. 그 셋의 까닭은 역할의 이름 그대로다.
fn rejected_reason(info: &PclmInfo) -> Option<String> {
    match info.kind {
        PclmKind::Ok => match info.role.as_deref() {
            Some(PclmRole::SUBMISSION | PclmRole::WORK) => None,
            role => Some(PclmRole::name(role)),
        },
        PclmKind::TooOld => Some(TOO_OLD.into()),
        PclmKind::TooNew => Some(TOO_NEW.into()),
        PclmKind::NotFound => Some(NOT_FOUND.into()),
        PclmKind::NotPclm => Some(NOT_PCLM.into()),
    }
}

/// `Path.GetFileName`.
fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// `Path.GetFileNameWithoutExtension` — 마지막 `.` 부터 뗀다(`.pclm` 은 빈 이름이다).
fn file_stem(path: &Path) -> String {
    let name = file_name(path);
    match name.rfind('.') {
        Some(i) => name[..i].to_string(),
        None => name,
    }
}

/// `Merger.Submitter` — 제출자 이름. 제출본의 설정에서 읽되 **그것만** 읽는다. 글이 아니면 없는 것이다.
fn submitter(connection: &Connection) -> Result<String> {
    let value: Option<Value> = connection
        .query_row(
            "SELECT value FROM app_setting WHERE key = 'submit.name';",
            [],
            |r| r.get(0),
        )
        .optional()?;
    Ok(match value {
        Some(Value::Text(name)) => name,
        _ => String::new(),
    })
}

fn columns_of(database: &Database, table: &str) -> Result<Vec<String>> {
    let connection = database.open_read_only()?;
    let mut statement = connection.prepare(&format!("PRAGMA table_info(\"{table}\");"))?;
    let names = statement
        .query_map([], |r| r.get::<_, String>(1))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(names)
}

/// 제출본 하나의 그 표를 통째로 읽어 후보로 쌓는다.
fn collect(
    connection: &Connection,
    t: &Table,
    columns: &[String],
    submission: usize,
    pool: &mut HashMap<String, Vec<Candidate>>,
) -> Result<()> {
    let key_at: Vec<usize> = t
        .key
        .iter()
        .map(|k| column_at(columns, k))
        .collect::<Result<_>>()?;
    let time_at = t.time.map(|c| column_at(columns, c)).transpose()?;

    let sql = format!(
        "SELECT {} FROM \"{}\";",
        columns
            .iter()
            .map(|c| format!("\"{c}\""))
            .collect::<Vec<_>>()
            .join(", "),
        t.name
    );
    let mut statement = connection.prepare(&sql)?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let values: Vec<Value> = (0..columns.len())
            .map(|i| row.get::<_, Value>(i))
            .collect::<rusqlite::Result<_>>()?;
        let key = key_at
            .iter()
            .map(|&i| text(&values[i]))
            .collect::<Vec<_>>()
            .join(&KEY_JOIN.to_string());
        let time = time_at.map(|i| text(&values[i])).unwrap_or_default();
        pool.entry(key).or_default().push(Candidate {
            submission,
            time,
            values,
        });
    }
    Ok(())
}

/// `Array.IndexOf(columns, name)` — 나르는 표의 키·시각 열은 스키마에 있다. 없으면 C# 은 -1 로 읽다 터진다.
fn column_at(columns: &[String], name: &str) -> Result<usize> {
    columns
        .iter()
        .position(|c| c == name)
        .ok_or_else(|| Error::invalid(format!("취합할 표에 {name} 열이 없습니다.")))
}

/// 키마다 하나를 고르고, 밀어낸 것이 있으면 알린다.
fn resolve(
    t: &Table,
    columns: &[String],
    pool: HashMap<String, Vec<Candidate>>,
    submitters: &[String],
    owners: &mut HashMap<&'static str, HashMap<String, usize>>,
    conflicts: &mut Vec<MergeConflict>,
) -> Vec<Candidate> {
    let mut entries: Vec<(String, Vec<Candidate>)> = pool.into_iter().collect();
    entries.sort_by(|a, b| cmp_ordinal(&a.0, &b.0));

    let mut kept = Vec::with_capacity(entries.len());
    // C# 은 이 표의 임자 사전을 처음부터 임자[t.이름] 에 걸어 둔다. 딸린 줄이 보는 부모 표는 언제나 다른 표라
    // 끝에 거는 것과 같다.
    let mut mine: HashMap<String, usize> = HashMap::new();

    for (key, mut candidates) in entries {
        let winner: usize;

        if matches!(t.rule, Rule::FollowParent | Rule::FollowEntity) {
            // 부모의 승자만 딸린 줄을 낸다. 부모가 어느 제출본에도 없으면(고아 줄) 먼저 온 제출본을 임자로 삼는다 —
            // 그래도 한 제출본의 줄만 실린다.
            let parts: Vec<&str> = key.split(KEY_JOIN).collect();
            let (parent, parent_table) = if t.rule == Rule::FollowEntity {
                (
                    parts.iter().skip(1).take(2).copied().collect::<Vec<_>>(),
                    parts[0],
                )
            } else {
                (
                    parts.iter().take(2).copied().collect::<Vec<_>>(),
                    t.parent.expect("딸린 표에는 부모가 있다"),
                )
            };
            let parent = parent.join(&KEY_JOIN.to_string());
            let Some(parent_owners) = owners.get(parent_table) else {
                continue;
            };
            let owner = match parent_owners.get(&parent) {
                Some(&o) => o,
                None => match mine.get(&parent) {
                    Some(&seen) => seen,
                    None => candidates
                        .iter()
                        .map(|c| c.submission)
                        .min()
                        .expect("후보는 하나 이상이다"),
                },
            };
            mine.insert(parent, owner);
            let Some(follower) = candidates.iter().position(|c| c.submission == owner) else {
                continue;
            };
            winner = follower;
        } else {
            winner = match t.rule {
                // 고를 것이 없는 표는 먼저 온 것을 둔다.
                Rule::Union | Rule::Definition => first_min(&candidates, |c| c.submission),
                // 늦은 쪽이 이긴다. 시각이 같으면 먼저 온 제출본이다.
                _ => {
                    let mut order: Vec<usize> = (0..candidates.len()).collect();
                    order.sort_by(|&a, &b| {
                        cmp_ordinal(&candidates[b].time, &candidates[a].time)
                            .then(candidates[a].submission.cmp(&candidates[b].submission))
                    });
                    order[0]
                }
            };

            let losers = reportable(t, columns, &candidates, winner);
            if !losers.is_empty() {
                let w = &candidates[winner];
                conflicts.push(MergeConflict {
                    kind: t.kind.to_string(),
                    key: key.replace(KEY_JOIN, "·"),
                    kept: MergeSide {
                        submitter: submitters[w.submission].clone(),
                        time: w.time.clone(),
                    },
                    pushed: losers
                        .iter()
                        .map(|&i| MergeSide {
                            submitter: submitters[candidates[i].submission].clone(),
                            time: candidates[i].time.clone(),
                        })
                        .collect(),
                });
            }

            mine.insert(key.clone(), candidates[winner].submission);
        }

        kept.push(candidates.swap_remove(winner));
    }

    owners.insert(t.name, mine);
    kept
}

/// `MinBy` — 가장 작은 것 가운데 먼저 온 것.
fn first_min(candidates: &[Candidate], by: impl Fn(&Candidate) -> usize) -> usize {
    let mut best = 0;
    for (i, c) in candidates.iter().enumerate() {
        if by(c) < by(&candidates[best]) {
            best = i;
        }
    }
    best
}

/// 밀어낸 것 가운데 **사람에게 알릴** 것(후보 자리, 제출본 차례로).
///
/// **알림이 본체다** — 사람이 적은 것이 알림 없이 밀려나면 그 사람은 사라진 줄도 모른다. 그래서 **값이 다른 것은 하나도
/// 빠짐없이 낸다.** 다만 두 사람이 **같은 값을 들고 있는 것은 겹침이 아니라 일치**라 침묵한다 — 같은 공고문·같은 계획
/// 줄·같은 상대자는 여러 제출본에 있는 것이 정상이고, 그것까지 한 줄씩 내면 진짜 갈린 둘이 수백 줄에 묻힌다. 그래서
/// 시각 열을 뺀 나머지가 이긴 것과 똑같은 줄은 내지 않는다.
///
/// `합집합` 은 키가 같으면 같은 것이라 애초에 고를 일이 없다. `정의` 는 **뜻이 어긋날 때만** 알린다 — 차례
/// (`sort_order`)는 뜻이 아니라 보이는 자리라 어긋나도 알리지 않고 먼저 온 것의 차례를 쓴다.
fn reportable(
    t: &Table,
    columns: &[String],
    candidates: &[Candidate],
    winner: usize,
) -> Vec<usize> {
    if candidates.len() < 2 || t.rule == Rule::Union {
        return Vec::new();
    }

    let mut losers: Vec<usize> = (0..candidates.len()).filter(|&i| i != winner).collect();
    losers.sort_by_key(|&i| candidates[i].submission);
    let w = &candidates[winner];

    if t.rule == Rule::Definition {
        // 뜻은 차례(sort_order)와 불투명한 식별자(user_view.id)를 뺀 나머지다.
        let meaning: Vec<usize> = (0..columns.len())
            .filter(|&i| columns[i] != "sort_order" && columns[i] != "id")
            .collect();
        return losers
            .into_iter()
            .filter(|&l| {
                meaning
                    .iter()
                    .any(|&i| text(&candidates[l].values[i]) != text(&w.values[i]))
            })
            .collect();
    }

    let time_at = t.time.and_then(|c| columns.iter().position(|x| x == c));
    losers
        .into_iter()
        .filter(|&l| differs(&candidates[l], w, time_at))
        .collect()
}

/// 시각 한 열을 빼고 두 줄이 다른가. 같은 문서를 둘이 읽으면 그 열만 갈린다.
fn differs(a: &Candidate, b: &Candidate, time_at: Option<usize>) -> bool {
    (0..a.values.len()).any(|i| Some(i) != time_at && text(&a.values[i]) != text(&b.values[i]))
}

/// 이름이 다른 맞춤 뷰 둘이 같은 식별자를 들고 오면(한 사람이 이름을 바꾼 뒤 다른 사람의 제출본에 옛 이름이 남은 것)
/// 먼저 온 제출본의 것이 식별자를 지키고 나머지는 새 식별자를 받는다 — 둘 다 싣되 PRIMARY KEY 위반으로 취합이 터지지
/// 않게. 차례는 제출본 차례가 된다(C# 의 `OrderBy` 와 같다).
fn one_id_per_view(columns: &[String], mut rows: Vec<Candidate>, env: &dyn Env) -> Vec<Candidate> {
    let Some(id_at) = columns.iter().position(|c| c == "id") else {
        return rows;
    };
    rows.sort_by_key(|r| r.submission);
    let mut seen: Vec<String> = Vec::new();
    for row in &mut rows {
        let id = text(&row.values[id_at]);
        if !seen.contains(&id) {
            seen.push(id);
            continue;
        }
        let fresh = env.new_id();
        seen.push(fresh.clone());
        row.values[id_at] = Value::Text(fresh);
    }
    rows
}

/// 고른 것을 취합본에 넣는다.
fn write(database: &Database, columns: &[Vec<String>], kept: &[Vec<Candidate>]) -> Result<()> {
    let connection = database.open()?;

    // 넣는 차례가 어긋나면 외래키가 걸린다. 끄고 넣되 끝에 반드시 확인한다. (PRAGMA 는 트랜잭션 안에서 켜고 끌 수 없다.)
    connection.execute_batch("PRAGMA foreign_keys = OFF;")?;

    {
        let transaction = begin(&connection)?;
        // 갓 지은 DB 에는 기본 사용자 열이 심겨 있다. 취합본에 서야 할 것은 제출본들이 실제로 들고 있는 열이므로,
        // 심긴 것을 치우고 그 자리에 넣는다.
        transaction.execute_batch("DELETE FROM user_column;")?;

        // 넣는 차례는 TABLES 의 차례다 — 부모가 먼저 선다.
        for (i, t) in TABLES.iter().enumerate() {
            let rows = &kept[i];
            if rows.is_empty() {
                continue;
            }
            let names = &columns[i];
            let sql = format!(
                "INSERT INTO \"{}\" ({}) VALUES ({});",
                t.name,
                names
                    .iter()
                    .map(|c| format!("\"{c}\""))
                    .collect::<Vec<_>>()
                    .join(", "),
                (0..names.len())
                    .map(|i| format!("$p{i}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            let mut statement = transaction.prepare(&sql)?;
            // 형을 짚지 않는다 — 값의 형은 읽어 온 그대로가 맞다.
            for row in rows {
                statement.execute(params_from_iter(row.values.iter()))?;
            }
        }
        transaction.commit()?;
    }

    connection.execute_batch("PRAGMA foreign_keys = ON;")?;

    // 취합본의 건을 정하는 유일한 자리다. notice_group 은 나르지 않으므로 여기까지 취합본에는 건이 하나도 없다 —
    // 참조 검사보다 앞서 지어야 그 검사를 지난다. C# 처럼 트랜잭션 없이 돈다.
    explicit_links::resolve(&connection)?;

    assert_foreign_keys_hold(&connection)
}

fn assert_foreign_keys_hold(connection: &Connection) -> Result<()> {
    let broken: Vec<String> = connection
        .prepare("PRAGMA foreign_key_check;")?
        .query_map([], |r| {
            Ok(format!(
                "{} 행이 {} 를 가리키지 못합니다",
                text(&r.get::<_, Value>(0)?),
                text(&r.get::<_, Value>(2)?)
            ))
        })?
        .collect::<rusqlite::Result<_>>()?;
    if broken.is_empty() {
        return Ok(());
    }
    Err(Error::invalid(format!(
        "취합 뒤 참조가 어긋났습니다: {}",
        broken.into_iter().take(5).collect::<Vec<_>>().join(" / ")
    )))
}

/// `Merger.Text` — 값을 문자열로 본다. 빈 것은 NULL 이 아니라 빈 문자열이다. C# `Convert.ToString(value,
/// InvariantCulture)` 와 같게 — 실수는 왕복 최단 꼴, BLOB 은 형 이름(`System.Byte[]`)이다.
fn text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Integer(i) => i.to_string(),
        Value::Real(f) => format_double(*f),
        Value::Text(s) => s.clone(),
        Value::Blob(_) => "System.Byte[]".into(),
    }
}

fn count(connection: &Connection, sql: &str) -> Result<i32> {
    let n: i64 = connection.query_row(sql, [], |r| r.get(0))?;
    Ok(n as i32)
}

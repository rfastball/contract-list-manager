//! SQL 가공열(C# `Storage/SqlColumn.cs`) — 사람이 고친 스칼라 부질의(docs/column-management.md §5·§11.5). 뷰에 그 글이
//! 그대로 박히므로 박기 전에 반드시 여기를 지난다.
//!
//! 막는 것은 두 겹이다. 글의 모양([`check_shape`])은 부질의 하나가 괄호 밖으로 새지 않음을 보고, 읽는 것([`prepare`])은
//! SQLite authorizer 가 본다 — 바깥(사용자 글)의 읽기는 계약면 뷰만, 안쪽의 읽기는 계약면이 아닌 뷰 안이 아니면 통과.
//!
//! authorizer 는 `sqlite3_set_authorizer` 를 바로 건다. rusqlite 의 `Connection::authorizer` 는 `&mut Connection` 을
//! 받아 트랜잭션(`Transaction` 은 연결을 빌린다) 안에서 부를 수 없는데, 뷰 짓기는 언제나 트랜잭션 안이다.
//!
//! `Validate`(저장 전 표본 실행)는 저장 단계가 더한다 — 뷰 짓기는 `Prepare` 만 부른다.

use std::collections::HashSet;
use std::ffi::{CStr, c_char, c_int, c_void};

use rusqlite::{Connection, ffi};

use crate::error::{Error, Result};
use crate::storage::database::Database;
use crate::storage::flow_compiler::SELF;
use crate::storage::store;
use crate::storage::user_view_builder::UserViewFrame;
use crate::text::{eq_ignore_case, ignore_case_key, utf16_len};

/// `SqlColumn.Readable` — 사용자 SQL 이 읽을 수 있는 뷰(계약면 뷰 열 개). `v_가공열` 은 다른 열의 식을 담아 뺀다.
pub const READABLE: &[&str] = &[
    "v_통합",
    "v_통합차수",
    "v_접수",
    "v_접수품목",
    "v_공고",
    "v_공고품목",
    "v_공고차수",
    "v_계약",
    "v_계약차수",
    "v_계약품목",
];

/// `SqlColumn.Functions` — 쓸 수 있는 함수(대소문자를 가리지 않는다). 소비자 SQLite 에 늘 있는 핵심 함수만 둔다.
/// `ceil`·`floor` 는 소비자에 수학 함수가 없으면 그 뷰를 읽지 못한다 — [`crate::sqlite::match_csharp`] 의 설명.
pub const FUNCTIONS: &[&str] = &[
    "abs",
    "avg",
    "ceil",
    "char",
    "coalesce",
    "count",
    "date",
    "datetime",
    "floor",
    "glob",
    "group_concat",
    "hex",
    "ifnull",
    "iif",
    "instr",
    "julianday",
    "length",
    "like",
    "lower",
    "ltrim",
    "max",
    "min",
    "nullif",
    "printf",
    "replace",
    "round",
    "rtrim",
    "strftime",
    "substr",
    "substring",
    "sum",
    "time",
    "total",
    "trim",
    "typeof",
    "unicode",
    "upper",
    "zeroblob",
    "format",
    "concat",
    "concat_ws",
    "string_agg",
    // 창 함수. 뷰 안의 창 함수는 바깥에서 부른 것처럼 보고된다(실측: v_통합 의 row_number) — 뷰를 읽으려면 둔다.
    "row_number",
    "rank",
    "dense_rank",
    "percent_rank",
    "cume_dist",
    "ntile",
    "lag",
    "lead",
    "first_value",
    "last_value",
    "nth_value",
];

/// `SqlColumn.Expression` — 뷰의 한 칸이 될 식. 글이 언제나 글로, 빈 값은 빈 글로 나간다.
pub fn expression(sql: &str) -> String {
    format!("coalesce(CAST((\n{sql}\n) AS TEXT), '')")
}

/// `SqlColumn.OpenChecked` — 검증용 연결. 읽기 전용으로 열고 `query_only` 를 켠다.
pub fn open_checked(database: &Database) -> Result<Connection> {
    let connection = database.open_read_only()?;
    connection.execute_batch("PRAGMA query_only = ON;")?;
    Ok(connection)
}

/// `SqlColumn.Frame` — SQL 열이 서는 자리. 가공열은 면 뷰 한 줄(`현재`)이고, 맞춤 뷰는 사슬 위의 세 문서다.
pub enum Frame<'a> {
    /// `Frame.Face(entity)`.
    Face(&'a str),
    /// `UserViewFrame.Sql`.
    UserView(&'a UserViewFrame),
}

impl Frame<'_> {
    /// `Frame.Key` — 표본 열쇠의 식.
    pub fn key(&self) -> String {
        match self {
            Frame::Face(entity) => format!("{SELF}.\"{}\"", store::face_key(entity)),
            Frame::UserView(_) => crate::storage::user_view_builder::KEY.to_string(),
        }
    }

    /// `Frame.Statement(select, where, tail)` — 문장 하나.
    pub fn statement(&self, select: &str, where_: Option<&str>, tail: &str) -> String {
        match self {
            Frame::Face(entity) => format!(
                "SELECT {select} FROM \"{}\" AS {SELF}{} {tail}",
                store::face_view(entity),
                where_.map_or_else(String::new, |w| format!(" WHERE {w}"))
            ),
            Frame::UserView(frame) => frame.statement(select, where_, tail),
        }
    }
}

/// `SqlColumn.CheckNames` — 글에 적힌 이름이 스키마의 표·뷰·색인·트리거이거나 `sqlite_`·`pragma_` 로 시작하면 계약면
/// 뷰여야 한다. authorizer 만으로는 모자란다 — 뷰 위의 `count(*)` 를 밑 표 읽기로 바꿔 쓰면 바깥 읽기로 보고된다.
fn check_names(connection: &Connection, named: &NameSet) -> Result<()> {
    let schema: HashSet<String> = connection
        .prepare("SELECT name FROM sqlite_master")?
        .query_map([], |r| r.get::<_, String>(0))?
        .map(|n| n.map(|n| ignore_case_key(&n)))
        .collect::<rusqlite::Result<_>>()?;
    let bad = named.names().iter().find(|n| {
        let key = ignore_case_key(n);
        !READABLE.contains(&n.as_str())
            && (schema.contains(&key) || key.starts_with("SQLITE_") || key.starts_with("PRAGMA_"))
    });
    match bad {
        Some(bad) => Err(Error::InvalidOperation(format!(
            "공개 뷰(v_공고 등)만 읽을 수 있습니다: {bad}"
        ))),
        None => Ok(()),
    }
}

/// authorizer 가 보는 것 — 거절하면 첫 까닭을 남긴다.
struct Gate<'a> {
    named: &'a NameSet,
    closed: HashSet<String>,
    refused: Option<String>,
}

const OK: c_int = 0;
const DENY: c_int = 1;
const READ: c_int = 20;
const SELECT: c_int = 21;
const FUNCTION: c_int = 31;

impl Gate<'_> {
    fn authorize(
        &mut self,
        action: c_int,
        p0: Option<&str>,
        p1: Option<&str>,
        inner: Option<&str>,
    ) -> c_int {
        let in_view = inner.is_some_and(|s| !s.is_empty());
        match action {
            SELECT => OK,
            READ => {
                // 바깥에 보고된 읽기라도 사용자가 그 이름을 적지 않았으면 뷰를 바꿔 쓴 것이다(check_names).
                let p0 = p0.unwrap_or("");
                let ok = if in_view {
                    !self.closed.contains(inner.unwrap_or(""))
                } else {
                    READABLE.contains(&p0) || !self.named.contains(p0)
                };
                if ok {
                    return OK;
                }
                self.refused.get_or_insert_with(|| {
                    format!(
                        "공개 뷰(v_공고 등)만 읽을 수 있습니다: {}",
                        if in_view { inner.unwrap_or("") } else { p0 }
                    )
                });
                DENY
            }
            FUNCTION => {
                // 뷰 안의 함수는 앱이 지은 것이다. 바깥(사용자 글)의 함수만 허용 목록으로 본다.
                let name = p1.unwrap_or("");
                if in_view || FUNCTIONS.iter().any(|f| eq_ignore_case(f, name)) {
                    return OK;
                }
                self.refused
                    .get_or_insert_with(|| format!("이 함수는 쓸 수 없습니다: {name}"));
                DENY
            }
            _ => {
                self.refused.get_or_insert_with(|| {
                    "읽기 질의만 받습니다 — 쓰기·ATTACH·PRAGMA·재귀는 쓸 수 없습니다.".to_string()
                });
                DENY
            }
        }
    }
}

/// SQLite 가 넘긴 글 — NULL 이면 `None`. 깨진 UTF-8 은 바꿔 읽는다.
///
/// # Safety
/// `p` 는 NULL 이거나 NUL 로 끝나는 글이다(authorizer 의 인자).
unsafe fn arg<'a>(p: *const c_char) -> Option<std::borrow::Cow<'a, str>> {
    if p.is_null() {
        None
    } else {
        // SAFETY: 위의 약속.
        Some(unsafe { CStr::from_ptr(p) }.to_string_lossy())
    }
}

unsafe extern "C" fn authorize(
    gate: *mut c_void,
    action: c_int,
    p0: *const c_char,
    p1: *const c_char,
    _database: *const c_char,
    inner: *const c_char,
) -> c_int {
    // 풀림이 C 로 새면 정의되지 않은 동작이다 — 거절로 막는다.
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // SAFETY: `guarded` 가 살아 있는 `Gate` 를 넘기고, 끝나기 전에 authorizer 를 걷는다. 인자는 SQLite 가 준 글이다.
        let gate = unsafe { &mut *gate.cast::<Gate<'_>>() };
        let (p0, p1, inner) = unsafe { (arg(p0), arg(p1), arg(inner)) };
        gate.authorize(action, p0.as_deref(), p1.as_deref(), inner.as_deref())
    }))
    .unwrap_or(DENY)
}

/// `SqlColumn.Guarded` — authorizer 를 건 채로 `run` 을 돌린다. 통과는 셋뿐이다 — `SQLITE_SELECT`, 계약면 뷰(또는 그
/// 뷰 안쪽)의 `SQLITE_READ`, 허용 함수의 `SQLITE_FUNCTION`. SQLite 가 실패하면 거절의 까닭(없으면 SQLite 의 말)을
/// 담아 [`Error::InvalidOperation`] 으로 낸다.
pub(crate) fn guarded<T>(
    connection: &Connection,
    named: &NameSet,
    run: impl FnOnce() -> Result<T>,
) -> Result<T> {
    // 안쪽 읽기를 막을 뷰 — 계약면이 아닌 뷰. 뷰 안의 CTE 이름은 스키마에 없으므로 여기 들지 않는다.
    let closed: HashSet<String> = connection
        .prepare("SELECT name FROM sqlite_master WHERE type = 'view'")?
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?
        .into_iter()
        .filter(|v| !READABLE.contains(&v.as_str()))
        .collect();
    let mut gate = Gate {
        named,
        closed,
        refused: None,
    };
    /// authorizer 를 걷는 일을 Drop 에 맡긴다 — `run` 이 panic 해도 연결에 `gate` 를 가리키는 authorizer 가
    /// 남지 않는다(남으면 다음 문장이 사라진 지역 변수를 읽는다).
    struct Lift<'c>(&'c Connection);
    impl Drop for Lift<'_> {
        fn drop(&mut self) {
            // SAFETY: 연결 손잡이는 `'c` 동안 살아 있다. 걷는 것이라 가리킬 것이 없다.
            unsafe {
                ffi::sqlite3_set_authorizer(self.0.handle(), None, std::ptr::null_mut());
            }
        }
    }
    // SAFETY: 연결 손잡이는 이 호출 동안 살아 있고, `gate` 는 `lift` 보다 먼저 선언되어 authorizer 가 걷힌 뒤에야
    // 놓인다(지역 변수는 선언의 역순으로 놓인다).
    unsafe {
        ffi::sqlite3_set_authorizer(
            connection.handle(),
            Some(authorize),
            (&raw mut gate).cast::<c_void>(),
        );
    }
    let lift = Lift(connection);
    let result = run();
    drop(lift);
    match result {
        Err(Error::Sqlite(error)) => {
            Err(Error::InvalidOperation(gate.refused.take().unwrap_or_else(
                || format!("SQL을 읽지 못했습니다: {}", dotnet_message(&error)),
            )))
        }
        other => other,
    }
}

/// `SqlColumn.Prepare(connection, transaction, entity, sql)` — 이 자료 종류의 면 뷰 위에서 준비만 한다(`LIMIT 0`).
/// 구문 오류·없는 열·여러 열·순환이 여기서 잡힌다. 부르는 쪽의 트랜잭션이 있으면 그 안에서 돈다.
pub fn prepare(connection: &Connection, entity: &str, sql: &str) -> Result<()> {
    prepare_on(connection, sql, &Frame::Face(entity))
}

/// `SqlColumn.Prepare(connection, transaction, sql, frame)` — 준비만 하는 것을 그 자리 위에서 한다(맞춤 뷰가 쓴다).
pub fn prepare_on(connection: &Connection, sql: &str, frame: &Frame<'_>) -> Result<()> {
    let named = check_shape(Some(sql))?;
    check_names(connection, &named)?;
    guarded(connection, &named, || {
        let statement = frame.statement(&expression(sql), None, "LIMIT 0") + ";";
        // ExecuteReader 처럼 준비하고 한 번 돌린다.
        let mut prepared = connection.prepare(&statement)?;
        let mut rows = prepared.query([])?;
        rows.next()?;
        Ok(())
    })
}

/// `SqlColumn.Validate(connection, entity, sql, sampleKeys, scan)` — 면 뷰 위의 [`validate_on`].
pub fn validate(
    connection: &Connection,
    entity: &str,
    sql: &str,
    sample_keys: &[String],
) -> Result<Vec<String>> {
    validate_on(connection, sql, &Frame::Face(entity), sample_keys, 20)
}

/// `SqlColumn.Validate(connection, sql, frame, sampleKeys, scan)` — 검증(§5)의 전부: authorizer, 준비, 그리고 표본
/// 실행. 표본 열쇠의 줄과 앞 `scan` 줄에서 한 행이 값을 둘 이상 내면 거절한다(스칼라 부질의는 첫 줄만 쓰고 나머지를
/// 조용히 버린다). 표본마다 찍힐 값을 낸다. 연결은 [`open_checked`] 로 연다.
///
/// C# 은 표본 열쇠를 Dapper 의 목록 펼치기(`IN @sampleKeys`)로 넘긴다 — 원소마다 매개변수 하나, 빈 목록이면
/// `IN (SELECT @sampleKeys WHERE 1 = 0)`. 같은 꼴로 짓는다.
pub fn validate_on(
    connection: &Connection,
    sql: &str,
    frame: &Frame<'_>,
    sample_keys: &[String],
    scan: usize,
) -> Result<Vec<String>> {
    prepare_on(connection, sql, frame)?;
    let named = check_shape(Some(sql))?;
    let key = frame.key();
    let count = format!("{key}, (SELECT count(*) FROM (\n{sql}\n))");
    let list = if sample_keys.is_empty() {
        "(SELECT NULL WHERE 1 = 0)".to_string()
    } else {
        format!(
            "({})",
            (1..=sample_keys.len())
                .map(|i| format!("?{i}"))
                .collect::<Vec<_>>()
                .join(",")
        )
    };
    let counted: Vec<(Option<String>, i64)> = guarded(connection, &named, || {
        let mut rows = Vec::new();
        let mut first =
            connection.prepare(&frame.statement(&count, Some(&format!("{key} IN {list}")), ""))?;
        let found = first.query_map(rusqlite::params_from_iter(sample_keys), |r| {
            Ok((r.get(0)?, r.get(1)?))
        })?;
        for row in found {
            rows.push(row?);
        }
        let mut second =
            connection.prepare(&frame.statement(&count, None, &format!("LIMIT {scan}")))?;
        let found = second.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        for row in found {
            rows.push(row?);
        }
        Ok(rows)
    })?;
    // C# 은 처음 걸린 줄의 열쇠가 NULL 이면(`many.Key is not null`) 거절하지 않는다 — 그대로 옮긴다.
    if let Some((Some(many_key), many)) = counted.iter().find(|(_, c)| *c > 1) {
        return Err(Error::InvalidOperation(format!(
            "한 행에서 값이 {many}개 나옵니다({many_key}). group_concat으로 잇거나 LIMIT 1로 하나만 고르세요."
        )));
    }
    let statement = frame.statement(&expression(sql), Some(&format!("{key} = ?1")), "LIMIT 1");
    sample_keys
        .iter()
        .map(|k| {
            guarded(connection, &named, || {
                let mut prepared = connection.prepare(&statement)?;
                let mut rows = prepared.query([k])?;
                Ok(match rows.next()? {
                    Some(row) => row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    None => String::new(),
                })
            })
        })
        .collect()
}

/// Microsoft.Data.Sqlite 의 `SqliteException.Message` — `SQLite Error {기본 코드}: '{sqlite3_errmsg}'.`. 깨진 가공열의
/// 까닭이 이 글로 뷰 정의에 박히므로(C# 과 같은 바이트) 같은 꼴로 짓는다.
pub(crate) fn dotnet_message(error: &rusqlite::Error) -> String {
    match error {
        rusqlite::Error::SqliteFailure(failure, message) => format!(
            "SQLite Error {}: '{}'.",
            failure.extended_code & 0xff,
            message.clone().unwrap_or_else(|| failure.to_string())
        ),
        rusqlite::Error::SqlInputError { error, msg, .. } => {
            format!("SQLite Error {}: '{msg}'.", error.extended_code & 0xff)
        }
        other => other.to_string(),
    }
}

/// [`Error`] 의 C# `Exception.Message`.
pub(crate) fn error_message(error: &Error) -> String {
    match error {
        Error::Sqlite(e) => dotnet_message(e),
        other => other.to_string(),
    }
}

/// `SqlColumn.MaxLength`.
const MAX_LENGTH: usize = 20_000;

/// `SqlColumn.CheckShape` — 부질의 하나가 괄호 밖으로 새지 않는가. 글에 적힌 이름(식별자·따옴표 이름)을 낸다
/// — 대소문자를 가리지 않는 집합이라 [`NameSet::contains`] 로 묻는다.
///
/// 글자 갈래는 .NET `char.IsWhiteSpace`·`IsLetter`·`IsLetterOrDigit` 대신 Rust 의 유니코드 속성으로 본다
/// (`is_whitespace`·`is_alphabetic`·`is_alphanumeric`). 한글·영숫자에서는 같고, 결합 문자 같은 드문 글자에서만
/// 이름의 경계가 갈릴 수 있다.
pub fn check_shape(sql: Option<&str>) -> Result<NameSet> {
    let mut names = NameSet::default();
    let sql = match sql {
        Some(s) if !s.chars().all(char::is_whitespace) => s,
        _ => return Err(Error::invalid("SQL이 비었습니다.")),
    };
    if utf16_len(sql) > MAX_LENGTH {
        return Err(Error::invalid("SQL은 20,000자까지 쓸 수 있습니다."));
    }
    if sql.contains('\0') {
        return Err(Error::invalid("SQL에 NUL 문자를 쓸 수 없습니다."));
    }
    let chars: Vec<char> = sql.chars().collect();
    let mut depth = 0i32;
    let mut first_word: Option<String> = None;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c == '-' && chars.get(i + 1) == Some(&'-') {
            match chars[i..].iter().position(|&x| x == '\n') {
                Some(end) => i += end + 1,
                None => i = chars.len(),
            }
            continue;
        }
        if c == '/' && chars.get(i + 1) == Some(&'*') {
            let end = (i + 2..chars.len().saturating_sub(1))
                .find(|&j| chars[j] == '*' && chars[j + 1] == '/');
            match end {
                Some(end) => i = end + 2,
                None => return Err(Error::invalid("닫히지 않은 주석이 있습니다.")),
            }
            continue;
        }
        if matches!(c, '\'' | '"' | '`' | '[') {
            let close = if c == '[' { ']' } else { c };
            let mut j = i + 1;
            loop {
                if j >= chars.len() {
                    return Err(Error::invalid(if c == '\'' {
                        "닫히지 않은 글자가 있습니다."
                    } else {
                        "닫히지 않은 이름이 있습니다."
                    }));
                }
                if chars[j] != close {
                    j += 1;
                    continue;
                }
                if close != ']' && chars.get(j + 1) == Some(&close) {
                    j += 2;
                    continue;
                }
                break;
            }
            if c != '\'' {
                let inner: String = chars[i + 1..j].iter().collect();
                let doubled: String = [close, close].iter().collect();
                names.insert(inner.replace(&doubled, &close.to_string()));
            }
            first_word.get_or_insert_with(String::new);
            i = j + 1;
            continue;
        }
        if c.is_alphabetic() || c == '_' {
            let mut j = i;
            while j < chars.len() && (chars[j].is_alphanumeric() || matches!(chars[j], '_' | '$')) {
                j += 1;
            }
            let word: String = chars[i..j].iter().collect();
            names.insert(word.clone());
            if eq_ignore_case(&word, "WITH") {
                return Err(Error::invalid(
                    "WITH(공통 테이블 식)는 쓸 수 없습니다. 괄호 안의 부질의로 바꿔 쓰세요.",
                ));
            }
            first_word.get_or_insert(word);
            i = j;
            continue;
        }
        first_word.get_or_insert_with(|| c.to_string());
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth < 0 {
                    return Err(Error::invalid("여는 괄호 없이 닫는 괄호가 있습니다."));
                }
            }
            ';' => {
                return Err(Error::invalid(
                    "SQL 문장은 하나만 쓸 수 있습니다. ‘;’를 지우세요.",
                ));
            }
            _ => {}
        }
        i += 1;
    }
    if depth != 0 {
        return Err(Error::invalid("괄호가 닫히지 않았습니다."));
    }
    if !first_word
        .as_deref()
        .is_some_and(|w| eq_ignore_case(w, "SELECT"))
    {
        return Err(Error::invalid(
            "SELECT로 시작하는 문장 하나만 쓸 수 있습니다.",
        ));
    }
    Ok(names)
}

/// 대소문자를 가리지 않는 이름 집합(`HashSet<string>(StringComparer.OrdinalIgnoreCase)`). 처음 넣은 꼴을 들고 있다.
#[derive(Clone, Debug, Default)]
pub struct NameSet {
    keys: std::collections::HashSet<String>,
    names: Vec<String>,
}

impl NameSet {
    fn insert(&mut self, name: String) {
        if self.keys.insert(ignore_case_key(&name)) {
            self.names.push(name);
        }
    }

    /// 대소문자를 가리지 않고 들어 있는가.
    pub fn contains(&self, name: &str) -> bool {
        self.keys.contains(&ignore_case_key(name))
    }

    /// 넣은 차례대로의 이름(처음 넣은 꼴).
    pub fn names(&self) -> &[String] {
        &self.names
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_names_and_refuses_escapes() {
        let names = check_shape(Some("select \"재무관\", [공고명] from v_공고 -- 끝")).unwrap();
        assert!(names.contains("재무관") && names.contains("V_공고") && names.contains("SELECT"));
        assert!(check_shape(Some("SELECT 1; DELETE FROM notice")).is_err());
        assert!(check_shape(Some("WITH x AS (SELECT 1) SELECT * FROM x")).is_err());
        assert!(check_shape(Some("(SELECT 1")).is_err());
        assert!(check_shape(Some("SELECT 'a")).is_err());
        assert!(check_shape(Some("/* x */ SELECT 1")).is_ok());
        assert!(check_shape(Some("  ")).is_err());
    }
}

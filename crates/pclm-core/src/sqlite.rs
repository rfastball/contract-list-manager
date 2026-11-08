//! 링크된 SQLite 에 대한 사실.

/// 가공식이 내는 SQL 이 요구하는 가장 낮은 SQLite 판(`group_concat(... ORDER BY ...)` 이 3.44 에 들어왔다).
pub const MINIMUM_VERSION_NUMBER: i32 = 3_044_000;

/// 링크된 SQLite 의 판 문자열(`3.54.1` 꼴).
pub fn version() -> &'static str {
    rusqlite::version()
}

/// 링크된 SQLite 가 [`MINIMUM_VERSION_NUMBER`] 이상인가.
pub fn meets_minimum() -> bool {
    rusqlite::version_number() >= MINIMUM_VERSION_NUMBER
}

/// C# 이 쓰는 e_sqlite3 와 같은 SQL 을 받게 연결을 맞춘다. [`crate::storage::database`] 가 연결마다 부른다.
///
/// - **큰따옴표 글자를 받지 않는다**(`SQLITE_DQS=0` 으로 지은 e_sqlite3 와 같다). bundled SQLite 는 받아 주므로, 끄지
///   않으면 C# 에서 깨진(없는 열을 큰따옴표로 적은) SQL 열이 Rust 에서는 글로 읽혀 멀쩡히 선다.
/// - **수학 함수를 모두 둔다**(`SQLITE_ENABLE_MATH_FUNCTIONS` 로 지은 e_sqlite3 와 같다 — [`MATH_FUNCTIONS`]). bundled 는
///   이것 없이 짓는다. SQL 열의 허용 함수(`SqlColumn.Functions`)에 든 `ceil`·`floor` 만으로는 모자란다 — 허용 목록
///   밖의 `sqrt` 를 쓴 SQL 열은 C# 에서 authorizer 가 「이 함수는 쓸 수 없습니다: sqrt」 로 막는데, 함수가 없으면
///   SQLite 가 그보다 먼저 「no such function」 으로 멎는다. 그 말이 깨진 열의 까닭으로 뷰 정의에 박히고 사람에게
///   보이므로, 같은 말이 나오려면 함수가 있어야 한다.
///
/// 뷰에 박힌 사용자 SQL 이 `ceil`·`floor` 를 부르면 수학 함수 없이 지은 SQLite 로 읽는 소비자는 그 뷰를 읽지 못한다 —
/// C# 판부터 있던 소비자 쪽 위험이고, 여기서 바꾸지 않는다.
pub(crate) fn match_csharp(connection: &rusqlite::Connection) -> rusqlite::Result<()> {
    use rusqlite::config::DbConfig;
    use rusqlite::functions::FunctionFlags;

    connection.set_db_config(DbConfig::SQLITE_DBCONFIG_DQS_DML, false)?;
    connection.set_db_config(DbConfig::SQLITE_DBCONFIG_DQS_DDL, false)?;
    let flags = FunctionFlags::SQLITE_UTF8
        | FunctionFlags::SQLITE_DETERMINISTIC
        | FunctionFlags::SQLITE_INNOCUOUS;
    for &(name, function) in MATH_FUNCTIONS {
        match function {
            Math::Ceiling(round) => {
                connection.create_scalar_function(name, 1, flags, move |ctx| {
                    Ok(ceiling(ctx.get_raw(0), round))
                })?
            }
            Math::Log(base) => connection.create_scalar_function(name, 1, flags, move |ctx| {
                Ok(log_one(ctx.get_raw(0), base))
            })?,
            Math::LogBase => connection.create_scalar_function(name, 2, flags, |ctx| {
                Ok(log_two(ctx.get_raw(0), ctx.get_raw(1)))
            })?,
            Math::One(f) => connection
                .create_scalar_function(name, 1, flags, move |ctx| Ok(math1(ctx.get_raw(0), f)))?,
            Math::Two(f) => connection.create_scalar_function(name, 2, flags, move |ctx| {
                Ok(math2(ctx.get_raw(0), ctx.get_raw(1), f))
            })?,
            Math::Pi => connection.create_scalar_function(name, 0, flags, |_| {
                Ok(rusqlite::types::Value::Real(std::f64::consts::PI))
            })?,
        }
    }
    Ok(())
}

/// 수학 함수 하나의 몸 — SQLite `func.c` 의 구현 갈래(`ceilingFunc`·`logFunc`·`math1Func`·`math2Func`·`piFunc`).
#[derive(Clone, Copy)]
enum Math {
    /// `ceilingFunc` — 정수는 그대로, 실수만 돌린다.
    Ceiling(fn(f64) -> f64),
    /// `logFunc` 인자 하나 — 0 은 자연로그, 1 은 상용로그, 2 는 이진로그(`sqlite3_user_data` 의 자리).
    Log(u8),
    /// `logFunc` 인자 둘 — `log(B, X)`.
    LogBase,
    /// `math1Func`.
    One(fn(f64) -> f64),
    /// `math2Func`.
    Two(fn(f64, f64) -> f64),
    /// `piFunc`.
    Pi,
}

/// SQLite `func.c` 의 `SQLITE_ENABLE_MATH_FUNCTIONS` 묶음 그대로(이름·인자 수·차례). `SQLITE_HAVE_C99_MATH_FUNCS` 는
/// MSVC 2013 이상에서 1 이라 `trunc`·`acosh`·`asinh`·`atanh` 도 든다. 셈은 Rust 의 `f64` 함수인데, Windows MSVC
/// 대상에서는 그것이 e_sqlite3 와 같은 C 런타임 함수를 부른다.
const MATH_FUNCTIONS: &[(&str, Math)] = &[
    ("ceil", Math::Ceiling(f64::ceil)),
    ("ceiling", Math::Ceiling(f64::ceil)),
    ("floor", Math::Ceiling(f64::floor)),
    ("trunc", Math::Ceiling(f64::trunc)),
    ("ln", Math::Log(0)),
    ("log", Math::Log(1)),
    ("log10", Math::Log(1)),
    ("log2", Math::Log(2)),
    ("log", Math::LogBase),
    ("exp", Math::One(f64::exp)),
    ("pow", Math::Two(f64::powf)),
    ("power", Math::Two(f64::powf)),
    ("mod", Math::Two(fmod)),
    ("acos", Math::One(f64::acos)),
    ("asin", Math::One(f64::asin)),
    ("atan", Math::One(f64::atan)),
    ("atan2", Math::Two(f64::atan2)),
    ("cos", Math::One(f64::cos)),
    ("sin", Math::One(f64::sin)),
    ("tan", Math::One(f64::tan)),
    ("cosh", Math::One(f64::cosh)),
    ("sinh", Math::One(f64::sinh)),
    ("tanh", Math::One(f64::tanh)),
    ("acosh", Math::One(f64::acosh)),
    ("asinh", Math::One(f64::asinh)),
    ("atanh", Math::One(f64::atanh)),
    ("sqrt", Math::One(f64::sqrt)),
    ("radians", Math::One(deg_to_rad)),
    ("degrees", Math::One(rad_to_deg)),
    ("pi", Math::Pi),
];

/// C `fmod` — Rust 의 `%` 가 곧 그것이다(부호는 나뉘는 수를 따른다).
fn fmod(x: f64, y: f64) -> f64 {
    x % y
}

/// `degToRad` — `x*(M_PI/180.0)`, 괄호 차례 그대로.
fn deg_to_rad(x: f64) -> f64 {
    x * (std::f64::consts::PI / 180.0)
}

/// `radToDeg` — `x*(180.0/M_PI)`.
fn rad_to_deg(x: f64) -> f64 {
    x * (180.0 / std::f64::consts::PI)
}

/// `sqlite3_value_numeric_type` 이 수로 보는 값의 `sqlite3_value_double`. 수가 아니면(NULL·수로 읽히지 않는 글·BLOB)
/// `None`.
fn number(value: rusqlite::types::ValueRef<'_>) -> Option<f64> {
    use rusqlite::types::ValueRef;
    match value {
        ValueRef::Integer(i) => Some(i as f64),
        ValueRef::Real(r) => Some(r),
        ValueRef::Text(t) => match numeric_affinity(t)? {
            Numeric::Integer(i) => Some(i as f64),
            Numeric::Real(r) => Some(r),
        },
        ValueRef::Null | ValueRef::Blob(_) => None,
    }
}

/// `sqlite3_result_double` — NaN 은 NULL 로 남는다(`sqlite3VdbeMemSetDouble`).
fn real(value: f64) -> rusqlite::types::Value {
    if value.is_nan() {
        rusqlite::types::Value::Null
    } else {
        rusqlite::types::Value::Real(value)
    }
}

/// `math1Func`.
fn math1(value: rusqlite::types::ValueRef<'_>, f: fn(f64) -> f64) -> rusqlite::types::Value {
    number(value).map_or(rusqlite::types::Value::Null, |x| real(f(x)))
}

/// `math2Func` — 둘 다 수여야 한다.
fn math2(
    a: rusqlite::types::ValueRef<'_>,
    b: rusqlite::types::ValueRef<'_>,
    f: fn(f64, f64) -> f64,
) -> rusqlite::types::Value {
    match (number(a), number(b)) {
        (Some(x), Some(y)) => real(f(x, y)),
        _ => rusqlite::types::Value::Null,
    }
}

/// `logFunc` 인자 하나 — 0 이하는 NULL.
fn log_one(value: rusqlite::types::ValueRef<'_>, base: u8) -> rusqlite::types::Value {
    match number(value) {
        Some(x) if x > 0.0 => real(match base {
            1 => x.log10(),
            2 => x.log2(),
            _ => x.ln(),
        }),
        _ => rusqlite::types::Value::Null,
    }
}

/// `logFunc` 인자 둘 — `log(B, X)` = `ln X / ln B`. 밑이 0 이하거나 그 로그가 0 이하(밑이 1 이하)거나 X 가 0 이하면
/// NULL. func.c 는 둘째 검사에서 첫 인자의 꼴을 다시 보는데, 첫 검사를 지난 뒤라 결과는 같다.
fn log_two(
    base: rusqlite::types::ValueRef<'_>,
    value: rusqlite::types::ValueRef<'_>,
) -> rusqlite::types::Value {
    let Some(b) = number(base).filter(|b| *b > 0.0).map(f64::ln) else {
        return rusqlite::types::Value::Null;
    };
    if b <= 0.0 {
        return rusqlite::types::Value::Null;
    }
    match number(value) {
        Some(x) if x > 0.0 => real(x.ln() / b),
        _ => rusqlite::types::Value::Null,
    }
}

/// SQLite `func.c` 의 `ceilingFunc` — `sqlite3_value_numeric_type` 으로 꼴을 본다. 정수는 그대로(글이었으면 정수로),
/// 실수는 올림·내림한 실수, 그 밖(NULL·수로 읽히지 않는 글·BLOB)은 NULL.
fn ceiling(value: rusqlite::types::ValueRef<'_>, round: fn(f64) -> f64) -> rusqlite::types::Value {
    use rusqlite::types::{Value, ValueRef};
    match value {
        ValueRef::Integer(i) => Value::Integer(i),
        ValueRef::Real(r) => real(round(r)),
        ValueRef::Text(t) => match numeric_affinity(t) {
            Some(Numeric::Integer(i)) => Value::Integer(i),
            Some(Numeric::Real(r)) => real(round(r)),
            None => Value::Null,
        },
        ValueRef::Null | ValueRef::Blob(_) => Value::Null,
    }
}

#[derive(Debug, PartialEq)]
enum Numeric {
    Integer(i64),
    Real(f64),
}

/// SQLite `applyNumericAffinity(pMem, 0)` — 글이 통째로 수(`sqlite3AtoF` 가 양수를 내는 꼴: 앞뒤 공백, 부호, 숫자,
/// 소수점, 지수)면 수로 바꾼다. 소수점·지수가 없고 64비트에 들면 정수, 아니면 실수다.
fn numeric_affinity(text: &[u8]) -> Option<Numeric> {
    let space = |b: &u8| matches!(b, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r');
    let start = text.iter().position(|b| !space(b))?;
    let end = text.iter().rposition(|b| !space(b))? + 1;
    let s = &text[start..end];
    let mut i = 0;
    if matches!(s.first(), Some(b'+' | b'-')) {
        i += 1;
    }
    let mut digits = 0;
    while s.get(i).is_some_and(u8::is_ascii_digit) {
        i += 1;
        digits += 1;
    }
    let mut integer = true;
    if s.get(i) == Some(&b'.') {
        integer = false;
        i += 1;
        while s.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return None;
    }
    if matches!(s.get(i), Some(b'e' | b'E')) {
        integer = false;
        i += 1;
        if matches!(s.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        let mut exponent = 0;
        while s.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
            exponent += 1;
        }
        if exponent == 0 {
            return None;
        }
    }
    if i != s.len() {
        return None;
    }
    let s = std::str::from_utf8(s).ok()?;
    if integer && let Ok(n) = s.parse::<i64>() {
        return Some(Numeric::Integer(n));
    }
    s.parse::<f64>().ok().map(Numeric::Real)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_affinity_follows_sqlite() {
        assert_eq!(numeric_affinity(b" 12 "), Some(Numeric::Integer(12)));
        assert_eq!(numeric_affinity(b"-1.5"), Some(Numeric::Real(-1.5)));
        assert_eq!(numeric_affinity(b"1."), Some(Numeric::Real(1.0)));
        assert_eq!(numeric_affinity(b".5e1"), Some(Numeric::Real(5.0)));
        assert_eq!(numeric_affinity(b"1,234"), None);
        assert_eq!(numeric_affinity(b"1e"), None);
        assert_eq!(numeric_affinity(b"."), None);
        assert_eq!(numeric_affinity(b""), None);
        assert_eq!(
            numeric_affinity(b"9223372036854775808"),
            Some(Numeric::Real(9223372036854775808.0))
        );
    }

    fn eval(c: &rusqlite::Connection, sql: &str) -> rusqlite::types::Value {
        c.query_row(&format!("SELECT {sql}"), [], |r| r.get(0))
            .unwrap()
    }

    /// func.c 의 뜻 — NULL·글·정수의 갈래와 몇 값.
    #[test]
    fn math_functions_follow_func_c() {
        use rusqlite::types::Value::{Integer, Null, Real};
        let c = rusqlite::Connection::open_in_memory().unwrap();
        match_csharp(&c).unwrap();
        assert_eq!(eval(&c, "sqrt(16)"), Real(4.0));
        assert_eq!(eval(&c, "sqrt(-1)"), Null);
        assert_eq!(eval(&c, "sqrt('9')"), Real(3.0));
        assert_eq!(eval(&c, "sqrt('x')"), Null);
        assert_eq!(eval(&c, "sqrt(NULL)"), Null);
        assert_eq!(eval(&c, "pi()"), Real(std::f64::consts::PI));
        assert_eq!(eval(&c, "mod(7, 3)"), Real(1.0));
        assert_eq!(eval(&c, "mod(-7, 3)"), Real(-1.0));
        assert_eq!(eval(&c, "mod(1, 0)"), Null);
        assert_eq!(eval(&c, "trunc(-2.7)"), Real(-2.0));
        assert_eq!(eval(&c, "trunc(5)"), Integer(5));
        assert_eq!(eval(&c, "log(100)"), Real(2.0));
        assert_eq!(eval(&c, "log(2, 8)"), Real(3.0));
        assert_eq!(eval(&c, "log(1, 8)"), Null);
        assert_eq!(eval(&c, "log(0)"), Null);
        assert_eq!(eval(&c, "log2(8)"), Real(3.0));
        assert_eq!(eval(&c, "ln(1)"), Real(0.0));
        assert_eq!(eval(&c, "pow(2, 10)"), Real(1024.0));
        assert_eq!(eval(&c, "power(2, '3')"), Real(8.0));
        assert_eq!(eval(&c, "power(2, 'x')"), Null);
        assert_eq!(eval(&c, "degrees(pi())"), Real(180.0));
        assert_eq!(eval(&c, "radians(180)"), Real(std::f64::consts::PI));
        assert_eq!(eval(&c, "ceil(1.2)"), Real(2.0));
        assert_eq!(eval(&c, "floor('-1.5')"), Real(-2.0));
        assert_eq!(eval(&c, "exp(0)"), Real(1.0));
        assert_eq!(eval(&c, "atan2(0, 1)"), Real(0.0));
        assert_eq!(eval(&c, "acosh(1)"), Real(0.0));
    }
}

//! 「서식」 의 수 · 날짜 코드 — 엑셀 표시 형식의 부분집합(ADR-053). 꼴은 앞 단계가 정하고(수면 수 코드, 날짜면 날짜
//! 코드), 코드로 꼴을 짐작하지 않는다. 꼴 검사와 컴파일이 같은 해석을 쓴다.
//!
//! **화면의 `frontend/src/flow/format.ts` 와 한 벌이다.** 받는 코드 · 오류 글 · 값이 갈리면 화면에서 되던 식이 저장에서
//! 막히거나 미리보기와 열의 값이 다르다. 공유 사례표(`tests/flow-cases.json`)가 둘을 함께 돈다.
//!
//! 코드는 컴파일 때 정해진 글이라 순수 SQL(`printf` · `substr` · `||`)로 푼다 — 가공열은 면 뷰에 박혀 바깥 프로그램이
//! 순수 SQLite 로 읽는다. 앱 함수를 부르는 것은 `[DBNum4]`(`한글금액`) 하나이고, 그래서 맞춤 뷰에서만 받는다.

use crate::storage::flow_compiler::lit;

/// 해석한 코드.
#[derive(Clone, Debug, PartialEq)]
pub enum Code {
    /// `[앞 글자] 숫자 자리 [뒤 글자]`. `zeros` 는 정수부 최소 자릿수, `dot` 은 소수점을 찍는가(소수 자리가 없어도 `0.` 이면 찍는다).
    Number {
        prefix: String,
        zeros: usize,
        comma: bool,
        dot: bool,
        decimals: usize,
        suffix: String,
    },
    /// `[DBNum4]` — 정수부를 한글 금액으로(앱 함수 `한글금액`).
    Korean {
        suffix: String,
    },
    Date(Vec<Piece>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Piece {
    Text(String),
    Year4,
    Year2,
    Month,
    Month2,
    Day,
    Day2,
    Hour,
    Hour2,
    Minute,
    Minute2,
    Second,
    Second2,
}

enum Tok {
    Text(String),
    Digits(String),
    Korean,
    /// (소문자 자리 글자, 길이, 적힌 그대로).
    Date(char, usize, String),
}

/// 따옴표 없이도 글자로 읽는 ASCII 기호(엑셀과 같다). 날짜 코드에서는 `.` · `,` 도 글자다.
const LITERAL: &str = "$-+/():!^&'~{}<>=";
/// 정수부 · 소수부 자릿수 상한 — 10 의 거듭제곱이 SQLite 정수에 들어가야 한다.
const MAX_DIGITS: usize = 15;

/// 코드를 해석한다. `date` 면 날짜 코드, 아니면 수 코드. 지원하지 않으면 오류 글.
pub fn parse(code: &str, date: bool) -> Result<Code, String> {
    if code.is_empty() {
        return Err("서식을 적으세요".into());
    }
    let tokens = tokens(code, date)?;
    if date {
        date_code(tokens)
    } else {
        number_code(tokens)
    }
}

fn tokens(code: &str, date: bool) -> Result<Vec<Tok>, String> {
    let chars: Vec<char> = code.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let close = |target: char| chars[i + 1..].iter().position(|&d| d == target);
        if c == '"' {
            let end = close('"').ok_or("서식의 따옴표가 닫히지 않았습니다")?;
            out.push(Tok::Text(chars[i + 1..i + 1 + end].iter().collect()));
            i += end + 2;
        } else if c == '\\' {
            let next = chars.get(i + 1).ok_or("서식의 ‘\\’ 뒤에 글자가 없습니다")?;
            out.push(Tok::Text(next.to_string()));
            i += 2;
        } else if c == '[' {
            let end = close(']').ok_or("지원하지 않는 서식 글자: [")?;
            let inner: String = chars[i + 1..i + 1 + end].iter().collect();
            if !inner.eq_ignore_ascii_case("DBNum4") {
                return Err(format!("지원하지 않는 서식: [{inner}]"));
            }
            if date {
                return Err("[DBNum4] 는 수에만 씁니다".into());
            }
            out.push(Tok::Korean);
            i += end + 2;
        } else if !c.is_ascii()
            || c == ' '
            || LITERAL.contains(c)
            || (date && matches!(c, '.' | ','))
        {
            out.push(Tok::Text(c.to_string()));
            i += 1;
        } else if !date && matches!(c, '0' | '#' | ',' | '.') {
            let run = chars[i..]
                .iter()
                .take_while(|d| matches!(d, '0' | '#' | ',' | '.'))
                .count();
            out.push(Tok::Digits(chars[i..i + run].iter().collect()));
            i += run;
        } else if date && matches!(c, '0' | '#') {
            return Err("날짜에는 수 자리(0 · #)를 쓸 수 없습니다".into());
        } else if "ymdhs".contains(c.to_ascii_lowercase()) {
            if !date {
                return Err("수에는 날짜 자리(y · m · d · h · s)를 쓸 수 없습니다".into());
            }
            let letter = c.to_ascii_lowercase();
            let run = chars[i..]
                .iter()
                .take_while(|d| d.to_ascii_lowercase() == letter)
                .count();
            out.push(Tok::Date(letter, run, chars[i..i + run].iter().collect()));
            i += run;
        } else {
            return Err(format!("지원하지 않는 서식 글자: {c}"));
        }
    }
    Ok(out)
}

fn number_code(tokens: Vec<Tok>) -> Result<Code, String> {
    let (mut prefix, mut suffix) = (String::new(), String::new());
    let mut digits: Option<String> = None;
    let mut korean = false;
    for (i, tok) in tokens.into_iter().enumerate() {
        match tok {
            Tok::Text(t) if digits.is_none() && !korean => prefix.push_str(&t),
            Tok::Text(t) => suffix.push_str(&t),
            Tok::Korean if i > 0 => return Err("[DBNum4] 는 서식 맨 앞에만 씁니다".into()),
            Tok::Korean => korean = true,
            Tok::Digits(_) if korean => return Err("[DBNum4] 뒤에는 글자만 씁니다".into()),
            Tok::Digits(_) if digits.is_some() => {
                return Err("숫자 자리는 한 덩이로 씁니다".into());
            }
            Tok::Digits(run) => digits = Some(run),
            Tok::Date(..) => unreachable!("수 코드에는 날짜 자리가 없다"),
        }
    }
    if korean {
        return Ok(Code::Korean { suffix });
    }
    let run = digits.ok_or("숫자 자리(0)가 없습니다")?;
    let mut parts = run.split('.');
    let whole = parts.next().unwrap_or("");
    let fraction = parts.next();
    if parts.next().is_some() {
        return Err("소수점은 하나만 씁니다".into());
    }
    if fraction.is_some_and(|f| f.chars().any(|c| c != '0')) {
        return Err("소수 자리에는 0 만 씁니다".into());
    }
    if whole.ends_with(',') {
        return Err("숫자 자리 끝의 쉼표는 쓸 수 없습니다".into());
    }
    let zeros = whole.chars().filter(|&c| c == '0').count();
    if zeros == 0 {
        return Err("정수 자리에 0 이 하나 이상 있어야 합니다".into());
    }
    let decimals = fraction.map_or(0, str::len);
    if zeros > MAX_DIGITS || decimals > MAX_DIGITS {
        return Err(format!("자릿수는 {MAX_DIGITS}개까지입니다"));
    }
    Ok(Code::Number {
        prefix,
        zeros,
        comma: whole.contains(','),
        dot: fraction.is_some(),
        decimals,
        suffix,
    })
}

fn date_code(tokens: Vec<Tok>) -> Result<Code, String> {
    let mut pieces: Vec<Piece> = Vec::new();
    for tok in tokens {
        let piece = match tok {
            Tok::Text(t) => {
                if let Some(Piece::Text(last)) = pieces.last_mut() {
                    last.push_str(&t);
                    continue;
                }
                Piece::Text(t)
            }
            Tok::Date(letter, n, written) => match (letter, n) {
                ('y', 4) => Piece::Year4,
                ('y', 2) => Piece::Year2,
                ('m', 1) => Piece::Month,
                ('m', 2) => Piece::Month2,
                ('d', 1) => Piece::Day,
                ('d', 2) => Piece::Day2,
                ('h', 1) => Piece::Hour,
                ('h', 2) => Piece::Hour2,
                ('s', 1) => Piece::Second,
                ('s', 2) => Piece::Second2,
                _ => return Err(format!("지원하지 않는 날짜 자리: {written}")),
            },
            Tok::Digits(_) | Tok::Korean => unreachable!("날짜 코드에는 수 자리가 없다"),
        };
        pieces.push(piece);
    }
    // 엑셀처럼: 글자를 건너뛴 바로 앞 자리가 시이거나 바로 뒤 자리가 초면 m 은 분이다.
    let spots: Vec<usize> = (0..pieces.len())
        .filter(|&i| !matches!(pieces[i], Piece::Text(_)))
        .collect();
    for (k, &i) in spots.iter().enumerate() {
        let after_hour = k > 0 && matches!(pieces[spots[k - 1]], Piece::Hour | Piece::Hour2);
        let before_second = spots
            .get(k + 1)
            .is_some_and(|&j| matches!(pieces[j], Piece::Second | Piece::Second2));
        if after_hour || before_second {
            pieces[i] = match pieces[i] {
                Piece::Month => Piece::Minute,
                Piece::Month2 => Piece::Minute2,
                ref other => other.clone(),
            };
        }
    }
    Ok(Code::Date(pieces))
}

/// 코드를 값 하나 `v`(수: REAL/INTEGER, 날짜: `YYYY-MM-DD[ HH:MM:SS]`) 위의 SQL 식으로. 값이 없으면(NULL) 빈 글이다.
pub fn sql(code: &Code) -> String {
    let body = match code {
        Code::Number {
            prefix,
            zeros,
            comma,
            dot,
            decimals,
            suffix,
        } => {
            let p = 10_i64.pow(*decimals as u32);
            let int = if *decimals == 0 {
                "r".to_string()
            } else {
                format!("r / {p}")
            };
            let digits = if *comma && *zeros > 1 {
                // 0 을 채운 뒤 쉼표를 찍는다: 10^zeros 를 더해 찍고 맨 앞의 1(과 그 뒤 쉼표)을 뗀다.
                let top = 10_i64.pow(*zeros as u32);
                let drop = if zeros % 3 == 0 { 3 } else { 2 };
                format!(
                    "CASE WHEN {int} < {top} THEN substr(printf('%,d', {int} + {top}), {drop}) ELSE printf('%,d', {int}) END"
                )
            } else if *comma {
                format!("printf('%,d', {int})")
            } else {
                format!("printf('%0{zeros}d', {int})")
            };
            let mut parts = vec!["CASE WHEN v < 0 AND r > 0 THEN '-' ELSE '' END".to_string()];
            if !prefix.is_empty() {
                parts.push(lit(prefix));
            }
            parts.push(digits);
            if *decimals > 0 {
                parts.push(format!("'.' || printf('%0{decimals}d', r % {p})"));
            } else if *dot {
                parts.push("'.'".into());
            }
            if !suffix.is_empty() {
                parts.push(lit(suffix));
            }
            let scaled = if p == 1 {
                "abs(v)".to_string()
            } else {
                format!("abs(v) * {p}")
            };
            // 사사오입(0 에서 먼 쪽) — 절댓값을 정수로 올려 셈하고 부호는 따로 붙인다.
            format!(
                "(SELECT {} FROM (SELECT CAST({scaled} + 0.5 AS INTEGER) AS r))",
                parts.join(" || ")
            )
        }
        Code::Korean { suffix } => {
            let amount = "한글금액(CAST(CAST(v AS INTEGER) AS TEXT))".to_string();
            if suffix.is_empty() {
                amount
            } else {
                format!("{amount} || {}", lit(suffix))
            }
        }
        Code::Date(pieces) => {
            let parts: Vec<String> = pieces.iter().map(date_piece).collect();
            if parts.is_empty() {
                "''".into()
            } else {
                parts.join(" || ")
            }
        }
    };
    format!("CASE WHEN v IS NULL THEN '' ELSE {body} END")
}

fn date_piece(piece: &Piece) -> String {
    // 시각이 없는 값은 시 · 분 · 초가 0 이다(엑셀과 같다) — 뒤에 덧붙인 ' 00:00:00' 이 그 자리를 채운다.
    let at = |from: u8| format!("substr(v || ' 00:00:00', {from}, 2)");
    let bare = |two: String| format!("CAST(CAST({two} AS INTEGER) AS TEXT)");
    match piece {
        Piece::Text(t) => lit(t),
        Piece::Year4 => "substr(v, 1, 4)".into(),
        Piece::Year2 => "substr(v, 3, 2)".into(),
        Piece::Month2 => "substr(v, 6, 2)".into(),
        Piece::Month => bare("substr(v, 6, 2)".into()),
        Piece::Day2 => "substr(v, 9, 2)".into(),
        Piece::Day => bare("substr(v, 9, 2)".into()),
        Piece::Hour2 => at(12),
        Piece::Hour => bare(at(12)),
        Piece::Minute2 => at(15),
        Piece::Minute => bare(at(15)),
        Piece::Second2 => at(18),
        Piece::Second => bare(at(18)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn number(code: &str) -> Code {
        parse(code, false).unwrap()
    }

    #[test]
    fn number_codes_read_zeros_commas_and_decimals() {
        assert_eq!(
            number("#,##0원"),
            Code::Number {
                prefix: String::new(),
                zeros: 1,
                comma: true,
                dot: false,
                decimals: 0,
                suffix: "원".into()
            }
        );
        assert_eq!(
            number("\"약 \"000.00"),
            Code::Number {
                prefix: "약 ".into(),
                zeros: 3,
                comma: false,
                dot: true,
                decimals: 2,
                suffix: String::new()
            }
        );
        assert_eq!(
            number("[dbnum4]\\원"),
            Code::Korean {
                suffix: "원".into()
            }
        );
        for (code, why) in [
            ("", "서식을 적으세요"),
            ("#,##", "정수 자리에 0 이 하나 이상 있어야 합니다"),
            ("0.#", "소수 자리에는 0 만 씁니다"),
            ("0,", "숫자 자리 끝의 쉼표는 쓸 수 없습니다"),
            ("0 0", "숫자 자리는 한 덩이로 씁니다"),
            ("0%", "지원하지 않는 서식 글자: %"),
            (
                "yyyy",
                "수에는 날짜 자리(y · m · d · h · s)를 쓸 수 없습니다",
            ),
            ("원", "숫자 자리(0)가 없습니다"),
            ("[Red]0", "지원하지 않는 서식: [Red]"),
            ("0[DBNum4]", "[DBNum4] 는 서식 맨 앞에만 씁니다"),
            ("[DBNum4]0", "[DBNum4] 뒤에는 글자만 씁니다"),
            ("0.0.0", "소수점은 하나만 씁니다"),
            ("\"원", "서식의 따옴표가 닫히지 않았습니다"),
        ] {
            assert_eq!(parse(code, false), Err(why.to_string()), "{code}");
        }
    }

    #[test]
    fn date_codes_tell_minutes_from_months() {
        use Piece::*;
        assert_eq!(
            parse("yyyy. m. d. hh:mm", true).unwrap(),
            Code::Date(vec![
                Year4,
                Text(". ".into()),
                Month,
                Text(". ".into()),
                Day,
                Text(". ".into()),
                Hour2,
                Text(":".into()),
                Minute2
            ])
        );
        assert_eq!(
            parse("MM:SS", true).unwrap(),
            Code::Date(vec![Minute2, Text(":".into()), Second2])
        );
        for (code, why) in [
            ("mmm", "지원하지 않는 날짜 자리: mmm"),
            ("yyy", "지원하지 않는 날짜 자리: yyy"),
            ("0", "날짜에는 수 자리(0 · #)를 쓸 수 없습니다"),
            ("hh AM", "지원하지 않는 서식 글자: A"),
            ("[DBNum4]", "[DBNum4] 는 수에만 씁니다"),
        ] {
            assert_eq!(parse(code, true), Err(why.to_string()), "{code}");
        }
    }
}

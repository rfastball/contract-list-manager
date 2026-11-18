//! 금액을 한글로, 날짜를 「년 월 일」 · 공문서 표기로 — 「문서 서식」 이 부르는 SQL 함수의 몸([`crate::sqlite`], ADR-051).
//! 읽는 꼴은 면 뷰가 내는 그대로다 — 금액 `30,400,000`(소수부가 있으면 `54,957,446.667`), 날짜 `2026/09/14`,
//! 일시 `2026/09/14 10:00` · `2026/09/14 10:00:00`. SQLite 의 `date('now', 'localtime')`(`2026-11-09`)도 읽는다.

const DIGITS: [&str; 10] = ["영", "일", "이", "삼", "사", "오", "육", "칠", "팔", "구"];
const SMALL: [&str; 4] = ["", "십", "백", "천"];
const LARGE: [&str; 5] = ["", "만", "억", "조", "경"];

/// 정수 부분을 한글로. 공문서 작성 편람의 꼴처럼 「일」을 살린다(114,500 → 일십일만사천오백).
/// 숫자가 아니면 `None`. 쉼표는 보지 않고, 소수 부분은 버린다(원 아래는 쓰지 않는다).
pub fn amount(text: &str) -> Option<String> {
    let plain: String = text.trim().chars().filter(|c| *c != ',').collect();
    let (negative, body) = match plain.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, plain.as_str()),
    };
    let whole = body.split('.').next()?;
    if whole.is_empty() || !whole.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let whole = whole.trim_start_matches('0');
    if whole.is_empty() {
        return Some("영".into());
    }
    if whole.len() > 20 {
        return None;
    }
    let digits: Vec<usize> = whole.bytes().map(|b| usize::from(b - b'0')).collect();
    let mut out = String::new();
    // 뒤에서부터 네 자리씩 묶는다.
    let groups = digits.len().div_ceil(4);
    for g in (0..groups).rev() {
        let end = digits.len() - g * 4;
        let start = end.saturating_sub(4);
        let chunk = &digits[start..end];
        let mut part = String::new();
        for (i, d) in chunk.iter().enumerate() {
            if *d == 0 {
                continue;
            }
            part.push_str(DIGITS[*d]);
            part.push_str(SMALL[chunk.len() - 1 - i]);
        }
        if !part.is_empty() {
            out.push_str(&part);
            out.push_str(LARGE[g]);
        }
    }
    Some(if negative {
        format!("마이너스 {out}")
    } else {
        out
    })
}

/// 글 앞머리의 날짜 `YYYY/MM/DD`(또는 `-`·`.` 로 나눈 것) → (년, 월, 일). 뒤에 시각이 붙어도 된다. 날짜가 아니면 `None`.
pub fn date(text: &str) -> Option<(u32, u32, u32)> {
    let text = text.trim();
    let head = text.split_whitespace().next()?;
    let mut parts = head.split(['/', '-', '.']);
    let (y, m, d) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some_and(|rest| !rest.is_empty()) {
        return None;
    }
    let number = |s: &str, width: std::ops::RangeInclusive<usize>| {
        (width.contains(&s.len()) && s.bytes().all(|b| b.is_ascii_digit()))
            .then(|| s.parse::<u32>().ok())
            .flatten()
    };
    let (y, m, d) = (number(y, 4..=4)?, number(m, 1..=2)?, number(d, 1..=2)?);
    ((1..=12).contains(&m) && (1..=31).contains(&d)).then_some((y, m, d))
}

/// 날짜 → 「2026년 9월 14일」. 날짜로 읽지 못하면 `None`(지어 쓰지 않는다).
pub fn ymd(text: &str) -> Option<String> {
    let (y, m, d) = date(text)?;
    Some(format!("{y}년 {m}월 {d}일"))
}

/// 면 뷰의 날짜 · 일시(글 전체가 `2026/01/01`, `2026/10/20 09:30`, `2026/10/20 09:30:00`) → 공문서 표기
/// `2026. 1. 1.` · `2026. 10. 20. 09:30`(초가 `00` 이면 뗀다). 그 꼴이 아니면 `None`.
pub fn official(text: &str) -> Option<String> {
    let (head, time) = match text.split_once(' ') {
        Some((head, time)) => (head, Some(time)),
        None => (text, None),
    };
    let b = head.as_bytes();
    if b.len() != 10 || b[4] != b'/' || b[7] != b'/' {
        return None;
    }
    let (y, m, d) = date(head)?;
    let day = format!("{y}. {m}. {d}.");
    let Some(time) = time else { return Some(day) };
    let parts: Vec<&str> = time.split(':').collect();
    let two = |s: &&str| s.len() == 2 && s.bytes().all(|c| c.is_ascii_digit());
    if !(2..=3).contains(&parts.len()) || !parts.iter().all(two) {
        return None;
    }
    let shown = if parts.get(2) == Some(&"00") { &parts[..2] } else { &parts[..] };
    Some(format!("{day} {}", shown.join(":")))
}

/// 날짜 → 「2026. 9.」(연 · 월, 월은 0 을 채우지 않는다). 날짜로 읽지 못하면 `None`.
pub fn ym(text: &str) -> Option<String> {
    let (y, m, _) = date(text)?;
    Some(format!("{y}. {m}."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 금액을_공문서처럼_읽는다() {
        for (number, text) in [
            ("0", "영"),
            ("7", "칠"),
            ("10", "일십"),
            ("114500", "일십일만사천오백"),
            ("123,450,000", "일억이천삼백사십오만"),
            ("100000000", "일억"),
            ("1000010000", "일십억일만"),
            ("9180000.00", "구백일십팔만"),
            ("12,345,678.9", "일천이백삼십사만오천육백칠십팔"),
            ("2000000000000", "이조"),
        ] {
            assert_eq!(amount(number).as_deref(), Some(text), "{number}");
        }
        assert_eq!(amount("abc"), None);
        assert_eq!(amount(""), None);
    }

    #[test]
    fn 면_뷰의_날짜와_일시를_읽는다() {
        assert_eq!(ymd("2026/09/14").as_deref(), Some("2026년 9월 14일"));
        assert_eq!(ymd("2026/09/14 10:00").as_deref(), Some("2026년 9월 14일"));
        assert_eq!(
            ymd("2026/09/14 10:00:00").as_deref(),
            Some("2026년 9월 14일")
        );
        assert_eq!(ymd("2026-11-09").as_deref(), Some("2026년 11월 9일"));
        assert_eq!(ymd("계약 후 90일 이내"), None);
        assert_eq!(ymd("2026/13/01"), None);
        assert_eq!(ymd(""), None);
    }

    #[test]
    fn 면_뷰의_날짜와_일시를_공문서_표기로() {
        assert_eq!(official("2026/01/01").as_deref(), Some("2026. 1. 1."));
        assert_eq!(official("2026/10/20 09:30:00").as_deref(), Some("2026. 10. 20. 09:30"));
        assert_eq!(official("2026/10/20 09:30").as_deref(), Some("2026. 10. 20. 09:30"));
        assert_eq!(official("2026/10/20 09:30:15").as_deref(), Some("2026. 10. 20. 09:30:15"));
        for not in [
            "계약 후 90일 이내",
            "2026/13/01",
            "2026/10/20 까지",
            "2026/10/20 09:30 이후",
            "2026-10-20",
            "2026.10.20",
            "2026/1/1",
            " 2026/10/20",
            "",
        ] {
            assert_eq!(official(not), None, "{not}");
        }
    }

    #[test]
    fn 날짜를_연_월로_읽는다() {
        assert_eq!(ym("2026-11-09").as_deref(), Some("2026. 11."));
        assert_eq!(ym("2026-09-01").as_deref(), Some("2026. 9."));
        assert_eq!(ym("2026/09/14 10:00:00").as_deref(), Some("2026. 9."));
        assert_eq!(ym("어제"), None);
        assert_eq!(ym(""), None);
    }
}

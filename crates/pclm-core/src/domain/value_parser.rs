//! 값 읽기(C# `Domain/ValueParser.cs`) 가운데 공고건 재구성이 부르는 `SplitNoticeRef` 와, 사람이 친 키를 실제 행에
//! 맞추는 `NoticeNumber`·`ContractNumber`(`Store.Resolve`), 계획 엑셀의 지시수량을 읽는 `Integer`.

/// `ValueParser.NoticeNumber` — 공고번호 `R26BK00000001 - 000` 을 본번호와 차수로 가른다. `-` 로 나눈 조각이
/// 정확히 둘이어야 하고(조각마다 앞뒤 공백을 걷는다), [`checked`] 를 지나야 한다. **번호처럼 생기지 않았으면
/// 번호가 아니다** — 어느 자리를 가리키는지 가리는 자리라 검산은 하지 않는다(ADR-016).
pub fn notice_number(text: Option<&str>) -> Option<(String, String)> {
    let text = text?;
    if text.chars().all(char::is_whitespace) {
        return None;
    }
    let parts: Vec<&str> = text.split('-').map(str::trim).collect();
    let [base, seq] = parts.as_slice() else {
        return None;
    };
    checked(base, seq)
}

/// `ValueParser.ContractNumber` — 계약번호는 끝 두 자리가 차수다(`R26TA0000000100`). 세 글자보다 짧으면 번호가 아니다.
pub fn contract_number(text: Option<&str>) -> Option<(String, String)> {
    let t = text?.trim();
    // C# 의 길이는 UTF-16 단위다. 검사([`checked`])가 ASCII 만 받으므로 그 밖의 글자가 끼면 어차피 번호가 아니다.
    if t.is_empty() || crate::text::utf16_len(t) < 3 {
        return None;
    }
    let chars: Vec<char> = t.chars().collect();
    let cut = chars.len().checked_sub(2)?;
    let base: String = chars[..cut].iter().collect();
    let seq: String = chars[cut..].iter().collect();
    checked(&base, &seq)
}

/// `ValueParser.Checked` — 본번호는 ASCII 영숫자만, 차수는 숫자만.
fn checked(base: &str, seq: &str) -> Option<(String, String)> {
    if base.is_empty() || seq.is_empty() {
        return None;
    }
    if !base.chars().all(|c| c.is_ascii_alphanumeric()) || !seq.chars().all(|c| c.is_ascii_digit())
    {
        return None;
    }
    Some((base.to_string(), seq.to_string()))
}

/// `ValueParser.SplitNoticeRef` — 적힌 관련공고(`R26BK00000001-000` 꼴)를 본번호와 차수로 가른다. 첫 `-` 에서
/// 가르고 양쪽 공백을 걷는다. 어느 쪽이 비거나 `-` 가 없으면 `None` — 갈라지지 않는 값은 적힌 그대로 사람 눈에 온다.
pub fn split_notice_ref(text: Option<&str>) -> Option<(String, String)> {
    let t = text?.trim();
    if t.is_empty() {
        return None;
    }
    let (base, seq) = t.split_once('-')?;
    let (base, seq) = (base.trim(), seq.trim());
    if base.is_empty() || seq.is_empty() {
        return None;
    }
    Some((base.to_string(), seq.to_string()))
}

/// `ValueParser.Integer` — 정수. `3` · `3 대` · `1,200`. 소수점이 있으면 정수가 아니다. 32비트를 넘으면 `None`.
pub fn integer(text: Option<&str>) -> Option<i32> {
    // int.TryParse(…, NumberStyles.Integer, Invariant) — Digits 가 지은 글은 부호와 숫자뿐이다.
    digits(text, false)?.parse().ok()
}

/// `ValueParser.Digits` — 숫자·맨 앞의 `-`·(허락하면) 첫 소수점만 모으고, 자릿점과 공백은 건너뛰고, 그 밖의 글자(단위)에서
/// 멈춘다. 숫자가 하나도 없으면 `None`. 끝에 남은 소수점은 지운다.
fn digits(text: Option<&str>, allow_decimal: bool) -> Option<String> {
    let text = text?;
    if text.chars().all(char::is_whitespace) {
        return None;
    }
    let mut out = String::new();
    let mut seen_dot = false;
    for ch in text.chars() {
        if ch.is_ascii_digit() || (ch == '-' && out.is_empty()) {
            out.push(ch);
        } else if ch == '.' && !allow_decimal {
            return None;
        } else if ch == '.' && allow_decimal && !seen_dot && !out.is_empty() {
            out.push(ch);
            seen_dot = true;
        } else if ch == ',' || ch == ' ' {
            continue;
        } else {
            break;
        }
    }
    out.bytes()
        .any(|b| b.is_ascii_digit())
        .then(|| out.trim_end_matches('.').to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_at_first_dash() {
        assert_eq!(
            split_notice_ref(Some(" N-1 - 000 ")),
            Some(("N".into(), "1 - 000".into()))
        );
        assert_eq!(
            split_notice_ref(Some("A0001-000")),
            Some(("A0001".into(), "000".into()))
        );
        assert_eq!(split_notice_ref(Some("A0001")), None);
        assert_eq!(split_notice_ref(Some("-000")), None);
        assert_eq!(split_notice_ref(None), None);
    }

    #[test]
    fn numbers_split_like_dotnet() {
        assert_eq!(
            notice_number(Some(" A26BK00000001 - 000 ")),
            Some(("A26BK00000001".into(), "000".into()))
        );
        assert_eq!(notice_number(Some("A-B-C")), None);
        assert_eq!(notice_number(Some("A1-0x")), None);
        assert_eq!(notice_number(Some("  ")), None);
        assert_eq!(
            contract_number(Some(" A26TA0000000100 ")),
            Some(("A26TA00000001".into(), "00".into()))
        );
        assert_eq!(contract_number(Some("A1")), None);
        assert_eq!(contract_number(Some("A-100")), None);
        assert_eq!(contract_number(Some("가나100")), None);
    }

    #[test]
    fn integers_like_value_parser() {
        assert_eq!(integer(Some("8")), Some(8));
        assert_eq!(integer(Some("1,200")), Some(1200));
        assert_eq!(integer(Some("3 대")), Some(3));
        assert_eq!(integer(Some("-5")), Some(-5));
        assert_eq!(integer(Some("1.5")), None);
        assert_eq!(integer(Some("대")), None);
        assert_eq!(integer(Some("")), None);
        assert_eq!(integer(Some("  ")), None);
        assert_eq!(integer(None), None);
        assert_eq!(integer(Some("99999999999")), None);
    }

    /// C# `ValueParserTests` 의 번호·수량 사례 — 번호처럼 생기지 않았으면 번호가 아니다. 이 가드가 없을 때 계약번호는 세 자
    /// 이상이면 무엇이든 통과해 끝 두 자를 차수로 잘랐고, 사내 품의서의 `연구개발용역` 같은 값이 자연키가 되어 `contract` 에
    /// 조용히 정상 행으로 앉았다. 반대로 자릿수·접두사는 박지 않는다 — 지자체·수의계약의 변형을 튕긴다.
    /// (`ValueParser.Money` 사례는 옮기지 않는다 — C# 에서도 프로덕션 호출자가 없던 함수라 Rust 에 짝이 없다.)
    #[test]
    fn csharp_value_parser_cases() {
        let pair = |b: &str, s: &str| Some((b.to_string(), s.to_string()));
        // 공고번호는_붙임표로_차수를_가른다
        assert_eq!(
            notice_number(Some("R26BK98000575 - 000")),
            pair("R26BK98000575", "000")
        );
        assert_eq!(notice_number(Some("abc - 9")), pair("abc", "9"));
        // 계약번호는_끝_두_자리가_차수다
        assert_eq!(
            contract_number(Some("R26TA9800070700")),
            pair("R26TA98000707", "00")
        );
        // 계약번호로_보이지_않으면_번호가_아니다
        for input in [
            "연구개발용역",
            "계약서 사본",
            "R26TA 980007 00",
            "R26TA980007AB",
            "--",
        ] {
            assert_eq!(contract_number(Some(input)), None, "{input}");
        }
        // 공고번호도_같은_잣대를_쓴다
        for input in [
            "가-나",
            "R26BK98000575 - 변경",
            "R26BK 980005 - 000",
            "R26BK98000575-000-extra",
            "R26BK98000575--000",
            "-R26BK98000575-000",
            "R26BK98000575-000-",
            "R26BK98000575-",
        ] {
            assert_eq!(notice_number(Some(input)), None, "{input}");
        }
        // 소수나_정수범위_밖의_수량은_잘라_읽지_않는다
        for input in ["1.5", "-1.5 대", "999999999999"] {
            assert_eq!(integer(Some(input)), None, "{input}");
        }
        // 자릿수나_접두사는_따지지_않는다
        assert_eq!(contract_number(Some("12300")), pair("123", "00"));
        assert_eq!(contract_number(Some("abc99")), pair("abc", "99"));
    }
}

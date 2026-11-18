//! 확장과 주고받는 글의 꼴(네이티브 메시징·확장 인사·중계의 JSON 쪽). 환경 효과가 없다 —
//! 길이 머리 프레이밍과 홈의 파일은 바깥 층(`pclm-host`)이 다루고, 여기는 글을 읽고 쓰는 뜻만 둔다(#10).
//!
//! - 메시지 읽기는 깊이 32 한도의 JSON 읽기 다음 같은 키 검사다. 둘 중 하나라도 걸리면 읽지
//!   못한 글이다(`invalid_json`).
//! - 답 쓰기는 한 줄 JSON 이다 — 인코더는
//!   기본 인코더(비 ASCII·`"&'+<>`` ` `` 를 `\uXXXX` 로 이스케이프)이고 수는 읽은 원문 그대로다.

use serde_json::Value;

use crate::json::{self, Encoder};

/// 확장이 한 번에 보낼 수 있는 요청의 크기.
pub const MAX_REQUEST_BYTES: usize = 768 * 1024;

/// 호스트가 한 번에 보낼 수 있는 답의 크기. Chrome 의 1MB 한도에 여유를 둔다.
pub const MAX_RESPONSE_BYTES: usize = 1024 * 1024 - 1024;

/// 확장 메시지 한 벌을 읽는다. UTF-8 이 아니거나, JSON 이 아니거나, 32 단계보다 깊거나, 어느 객체에든 같은 키가 두 번
/// 들었으면 `None` — 부르는 쪽이 `invalid_json` 으로 답한다.
///
/// 문자열 안의 잘못된 UTF-8 은 Rust 의 값
/// 모형이 담을 수 없어 읽지 못한 글로 친다 — 브라우저는 UTF-8 만 보낸다.
pub fn parse_message(payload: &[u8]) -> Option<Value> {
    let text = std::str::from_utf8(payload).ok()?;
    let value = json::parse_with_depth(text, 32).ok()?;
    (!json::has_duplicate_keys(text)).then_some(value)
}

/// 홈의 쪽지 한 벌(깊이 64)을 읽는다. 같은 키가 두 번 든 글은 `None` — 부르는 쪽이
/// 모두 읽지 못한 쪽지로 친다.
pub fn parse_node(text: &str) -> Option<Value> {
    let value = json::parse(text).ok()?;
    (!json::has_duplicate_keys(text)).then_some(value)
}

/// 한 줄로 쓴다 — 기본 인코더(비 ASCII·`"&'+<>`` ` `` 를 이스케이프).
pub fn to_web_json(value: &Value) -> String {
    json::to_compact(value, Encoder::Default)
}

/// 들여 쓴다(줄바꿈 `\r\n`) — 기본 인코더.
pub fn to_indented_json(value: &Value) -> String {
    json::to_indented(value, Encoder::Default)
}

/// 들여 쓴다(줄바꿈 `\r\n`, 모든 유니코드 범위를 이스케이프 없이). 홈의 쪽지의 꼴이다 — 사람이 열어 보는 파일이라 한글 경로를
/// `\uXXXX` 로 감추지 않는다.
pub fn to_indented_unicode_json(value: &Value) -> String {
    json::to_indented(value, Encoder::AllRanges)
}

/// 식별자 읽기 — `N`(32자)·`D`(줄표)·`B`(중괄호)·`P`(괄호) 꼴. 앞뒤 공백은 걷는다.
pub fn is_guid(text: Option<&str>) -> bool {
    let Some(text) = text else { return false };
    let t = text.trim();
    let hex = |s: &str| s.bytes().all(|b| b.is_ascii_hexdigit());
    let dashed = |s: &str| {
        let parts: Vec<&str> = s.split('-').collect();
        parts.len() == 5
            && parts
                .iter()
                .zip([8, 4, 4, 4, 12])
                .all(|(p, n)| p.len() == n && hex(p))
    };
    (t.len() == 32 && hex(t))
        || dashed(t)
        || t.strip_prefix('{')
            .and_then(|s| s.strip_suffix('}'))
            .is_some_and(dashed)
        || t.strip_prefix('(')
            .and_then(|s| s.strip_suffix(')'))
            .is_some_and(dashed)
}

/// 수이고 원문이 `int` 에 드는 정수 꼴(`1.0`·`1e0` 은 아니다, `-0` 은 0)일 때만.
pub fn try_get_int32(value: &Value) -> Option<i32> {
    integer_text(value)?.parse().ok()
}

/// [`try_get_int32`] 와 같되 `long`.
pub fn try_get_int64(value: &Value) -> Option<i64> {
    integer_text(value)?.parse().ok()
}

fn integer_text(value: &Value) -> Option<String> {
    let Value::Number(n) = value else { return None };
    let text = n.to_string();
    let body = text.strip_prefix('-').unwrap_or(&text);
    (!body.is_empty() && body.bytes().all(|b| b.is_ascii_digit())).then_some(text)
}

/// `string.Length` — UTF-16 코드 단위 수.
pub fn utf16_len(s: &str) -> usize {
    crate::text::utf16_len(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_depth_limit_is_thirty_two() {
        let nest = |n: usize| "[".repeat(n) + &"]".repeat(n);
        assert!(parse_message(nest(32).as_bytes()).is_some());
        assert!(parse_message(nest(33).as_bytes()).is_none());
        // 쪽지는 64 단계까지 읽는다.
        assert!(parse_node(&nest(64)).is_some());
        assert!(parse_node(&nest(65)).is_none());
    }

    #[test]
    fn message_refuses_duplicates_bom_and_bad_utf8() {
        assert!(parse_message(br#"{"a":1,"a":1}"#).is_none());
        assert!(parse_message(br#"{"a":{"b":1,"b":2}}"#).is_none());
        assert!(parse_message(b"\xEF\xBB\xBF{}").is_none());
        assert!(parse_message(b"\"\xFF\"").is_none());
        assert_eq!(
            to_web_json(&parse_message("{\"n\":1.50,\"s\":\"<é>\"}".as_bytes()).unwrap()),
            r#"{"n":1.50,"s":"\u003C\u00E9\u003E"}"#
        );
    }

    #[test]
    fn int32_follows_try_get_int32() {
        let n = |t: &str| parse_node(t).unwrap();
        assert_eq!(try_get_int32(&n("-0")), Some(0));
        assert_eq!(try_get_int32(&n("1.0")), None);
        assert_eq!(try_get_int32(&n("1e0")), None);
        assert_eq!(try_get_int32(&n("2147483648")), None);
        assert_eq!(try_get_int64(&n("2147483648")), Some(2_147_483_648));
        assert_eq!(try_get_int32(&n("\"1\"")), None);
    }
}

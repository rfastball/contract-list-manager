//! 확장과 주고받는 글의 꼴(C# `NativeMessaging`·`ExtensionPresence`·`ExtensionRelay` 의 JSON 쪽). 환경 효과가 없다 —
//! 길이 머리 프레이밍과 홈의 파일은 바깥 층(`pclm-host`)이 다루고, 여기는 글을 읽고 쓰는 뜻만 둔다(#10).
//!
//! - 메시지 읽기는 `JsonDocument.Parse(payload, MaxDepth = 32)` 다음 `Flow.HasDuplicateKeys` 다. 둘 중 하나라도 걸리면 읽지
//!   못한 글이다(`invalid_json`).
//! - 답 쓰기는 `JsonSerializer.SerializeToUtf8Bytes(…, JsonSerializerDefaults.Web)` 와 같은 바이트다 — 인코더는
//!   `JavaScriptEncoder.Default`(비 ASCII·`"&'+<>`` ` `` 를 `\uXXXX`)이고 수는 읽은 원문 그대로다.

use serde_json::Value;

use crate::json::{self, Encoder};

/// 확장이 한 번에 보낼 수 있는 요청의 크기(C# `NativeMessaging.MaxRequestBytes`).
pub const MAX_REQUEST_BYTES: usize = 768 * 1024;

/// 호스트가 한 번에 보낼 수 있는 답의 크기(C# `NativeMessaging.MaxResponseBytes`). Chrome 의 1MB 한도에 여유를 둔다.
pub const MAX_RESPONSE_BYTES: usize = 1024 * 1024 - 1024;

/// 확장 메시지 한 벌을 읽는다. UTF-8 이 아니거나, JSON 이 아니거나, 32 단계보다 깊거나, 어느 객체에든 같은 키가 두 번
/// 들었으면 `None` — C# 은 `invalid_json` 으로 답한다.
///
/// C# 의 `JsonDocument` 는 문자열 안의 잘못된 UTF-8 을 읽을 때 받아들이고 그 칸을 글로 꺼낼 때에야 던진다. Rust 의 값
/// 모형은 그런 글을 담을 수 없어 여기서 읽지 못한 글로 친다 — 브라우저는 UTF-8 만 보낸다.
pub fn parse_message(payload: &[u8]) -> Option<Value> {
    let text = std::str::from_utf8(payload).ok()?;
    let value = json::parse_with_depth(text, 32).ok()?;
    (!json::has_duplicate_keys(text)).then_some(value)
}

/// 홈의 쪽지 한 벌(`JsonNode.Parse`, 깊이 64)을 읽는다. 같은 키가 두 번 든 글은 `None` — C# 은 그 객체를 열 때
/// `ArgumentException` 을 던지고, 부르는 쪽이 모두 읽지 못한 쪽지로 친다.
pub fn parse_node(text: &str) -> Option<Value> {
    let value = json::parse(text).ok()?;
    (!json::has_duplicate_keys(text)).then_some(value)
}

/// 같은 키가 두 번 든 객체가 어디든 있는가(C# `Flow.HasDuplicateKeys`). JSON 으로 읽히는 글에만 쓴다.
pub fn has_duplicate_keys(text: &str) -> bool {
    json::has_duplicate_keys(text)
}

/// 한 줄로 쓴다 — `JsonSerializerDefaults.Web` 의 직렬화와 인자 없는 `JsonNode.ToJsonString()` 이 같은 바이트다
/// (`JavaScriptEncoder.Default`).
pub fn to_web_json(value: &Value) -> String {
    json::to_compact(value, Encoder::Default)
}

/// 들여 쓴다(`WriteIndented = true`, 줄바꿈 `\r\n`) — `JavaScriptEncoder.Default`.
pub fn to_indented_json(value: &Value) -> String {
    json::to_indented(value, Encoder::Default)
}

/// 들여 쓴다(`WriteIndented = true`, 줄바꿈 `\r\n`) — `JavaScriptEncoder.Create(UnicodeRanges.All)`. 홈의 쪽지(C#
/// `Home.WriteConfig`)의 꼴이다 — 사람이 열어 보는 파일이라 한글 경로를 `\uXXXX` 로 감추지 않는다.
pub fn to_indented_unicode_json(value: &Value) -> String {
    json::to_indented(value, Encoder::AllRanges)
}

/// `Guid.TryParse` — `N`·`D`·`B`·`P` 꼴, 앞뒤 공백은 걷는다.
pub fn is_guid(text: Option<&str>) -> bool {
    super::capture::is_guid(text)
}

/// `JsonElement.TryGetInt32` — 수이고 원문이 `int` 에 드는 정수 꼴(`1.0`·`1e0` 은 아니다, `-0` 은 0)일 때만.
pub fn try_get_int32(value: &Value) -> Option<i32> {
    integer_text(value)?.parse().ok()
}

/// `JsonElement.TryGetInt64` — [`try_get_int32`] 와 같되 `long`.
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

//! System.Text.Json 과 **바이트까지 같은** JSON 읽기·쓰기.
//!
//! 저장되는 JSON(맞춤 뷰 정의·가공식·판올림이 고쳐 쓰는 식)은 C# 이 쓴 것과 한 글자도 갈리면 안 된다 —
//! 대조(#8 E1·E3)가 저장 바이트를 견주고, 소비자는 그 글을 그대로 읽는다. serde_json 의 직렬화는 이스케이프·
//! 수 표기가 .NET 과 달라서 값 모형(`serde_json::Value`)만 빌리고 쓰는 것은 여기서 한다.
//!
//! - **인코더 둘.** `Flow.Json`(가공식·맞춤 뷰)은 `UnsafeRelaxedJsonEscaping`, 그 밖의 기본 옵션(`Mapping.Json`,
//!   인자 없는 `ToJsonString()`)은 `JavaScriptEncoder.Default` 다. 실측(.NET 8, scratch 콘솔):
//!   - 둘 다 `\\`·`\b`·`\t`·`\n`·`\f`·`\r` 은 짧은 꼴, 그 밖의 이스케이프는 `\uXXXX`(대문자 16진),
//!     BMP 밖 글자는 서로게이트 둘(`\uD83D\uDE00`)이다.
//!   - 느슨한 쪽은 `"` 를 `\"` 로 쓰고 `<>&'+` 와 `` ` ``·한글을 그대로 둔다. 다만 제어 문자·U+007F..U+00A0·
//!     U+2028/2029·U+FEFF·사용 영역·정의되지 않은 코드 포인트·BMP 밖 전부를 이스케이프한다 — 그 표가
//!     [`relaxed_table`] 이다.
//!   - 기본 쪽은 `"&'+<>` 와 `` ` `` 를 `\u0022` 꼴로, U+007F 이상을 모두 이스케이프한다.
//! - **수.** 읽은 수는 원문 그대로 다시 쓴다(`JsonNode` 와 같다 — `1.50` 은 `1.50`, `1E+2` 는 `1E+2`). 코드가 지은
//!   `double` 은 .NET 의 왕복 최단 꼴로 쓴다([`format_double`]).
//! - **읽기.** `JsonSerializerDefaults.Web` 은 속성 이름의 대소문자를 가리지 않고 수를 글로도 받는다
//!   (`"n": "3"`). 같은 이름이 둘이면 뒤의 것이 이긴다.

mod relaxed_table;

use std::collections::HashSet;

use serde_json::{Map, Number, Value};

use crate::text::eq_ignore_case;

/// 문자열을 쓸 때의 인코더.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Encoder {
    /// `JavaScriptEncoder.UnsafeRelaxedJsonEscaping` — `Flow.Json`.
    Relaxed,
    /// `JavaScriptEncoder.Default` — 옵션 없는 직렬화·`Mapping.Json`.
    Default,
    /// `JavaScriptEncoder.Create(UnicodeRanges.All)` — 홈의 쪽지(`Home.Json`). 허락 표는 느슨한 쪽과 같고(둘 다
    /// `UnicodeRanges.All` 에서 정의되지 않은 글자를 뺀다), 느슨하지 않아 `"&'+<>` 와 `` ` `` 를 기본 쪽처럼 `"`
    /// 꼴로 쓴다.
    AllRanges,
}

fn escaped(c: char, encoder: Encoder) -> bool {
    let cp = c as u32;
    match encoder {
        Encoder::Default => {
            !(0x20..0x7F).contains(&cp)
                || matches!(c, '"' | '&' | '\'' | '+' | '<' | '>' | '`' | '\\')
        }
        Encoder::AllRanges => {
            matches!(c, '"' | '&' | '\'' | '+' | '<' | '>' | '`' | '\\')
                || escaped(c, Encoder::Relaxed)
        }
        Encoder::Relaxed => relaxed_table::RELAXED_ESCAPED
            .binary_search_by(|&(start, end)| {
                if end < cp {
                    std::cmp::Ordering::Less
                } else if start > cp {
                    std::cmp::Ordering::Greater
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .is_ok(),
    }
}

/// 따옴표까지 붙여 문자열 하나를 쓴다.
pub(crate) fn write_string(out: &mut String, s: &str, encoder: Encoder) {
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' if encoder == Encoder::Relaxed => out.push_str("\\\""),
            '\u{8}' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\u{c}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            c if escaped(c, encoder) => {
                let mut units = [0u16; 2];
                for unit in c.encode_utf16(&mut units) {
                    out.push_str(&format!("\\u{unit:04X}"));
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// 한 줄로 쓴다 — `JsonNode.ToJsonString(options)`·`JsonSerializer.Serialize` 의 들여쓰지 않은 꼴.
pub(crate) fn to_compact(value: &Value, encoder: Encoder) -> String {
    let mut out = String::new();
    write_compact(&mut out, value, encoder);
    out
}

fn write_compact(out: &mut String, value: &Value, encoder: Encoder) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(n) => out.push_str(&n.to_string()),
        Value::String(s) => write_string(out, s, encoder),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_compact(out, item, encoder);
            }
            out.push(']');
        }
        Value::Object(map) => {
            out.push('{');
            for (i, (key, item)) in map.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_string(out, key, encoder);
                out.push(':');
                write_compact(out, item, encoder);
            }
            out.push('}');
        }
    }
}

/// 들여 쓴다 — `WriteIndented = true`. 줄바꿈은 언제나 `\r\n` 이다. .NET 8 의 들여쓰기 줄바꿈은 `Environment.NewLine`
/// 이라, 이 글로 판을 뜨는 C# `Mapping.HashText` 가 `\r\n` 으로 박아 맞춘다.
pub(crate) fn to_indented(value: &Value, encoder: Encoder) -> String {
    let mut out = String::new();
    write_indented(&mut out, value, encoder, 0);
    out
}

const NEWLINE: &str = "\r\n";

fn indent(out: &mut String, depth: usize) {
    out.push_str(NEWLINE);
    for _ in 0..depth {
        out.push_str("  ");
    }
}

fn write_indented(out: &mut String, value: &Value, encoder: Encoder, depth: usize) {
    match value {
        Value::Array(items) if !items.is_empty() => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                indent(out, depth + 1);
                write_indented(out, item, encoder, depth + 1);
            }
            indent(out, depth);
            out.push(']');
        }
        Value::Object(map) if !map.is_empty() => {
            out.push('{');
            for (i, (key, item)) in map.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                indent(out, depth + 1);
                write_string(out, key, encoder);
                out.push_str(": ");
                write_indented(out, item, encoder, depth + 1);
            }
            indent(out, depth);
            out.push('}');
        }
        other => write_compact(out, other, encoder),
    }
}

/// 코드가 지은 `double` 의 JSON 꼴 — `Utf8JsonWriter.WriteNumberValue(double)` 와 같다(실측).
///
/// 왕복 최단 자릿수를 쓰고, 소수점 자리(scale)가 17 을 넘거나 -3 보다 작으면 지수 꼴(`1E+21`·`1E-05`·
/// `1.2345678901234568E+17`)이다. 그 사이는 그냥 적는다(`10000000000000000`·`0.0001`). `-0` 은 `-0`.
/// 유한하지 않은 값은 .NET 이 쓰기를 거절하므로 부르는 쪽이 먼저 막는다.
pub(crate) fn format_double(d: f64) -> String {
    let s = format!("{d:e}");
    let (mantissa, exponent) = s.split_once('e').unwrap_or((&s, "0"));
    let negative = mantissa.starts_with('-');
    let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    let exponent: i32 = exponent.parse().unwrap_or(0);
    let mut out = String::new();
    if negative {
        out.push('-');
    }
    if digits == "0" {
        out.push('0');
        return out;
    }
    let scale = exponent + 1;
    if !(-3..=17).contains(&scale) {
        out.push_str(&digits[..1]);
        if digits.len() > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        out.push('E');
        out.push(if exponent < 0 { '-' } else { '+' });
        out.push_str(&format!("{:02}", exponent.unsigned_abs()));
    } else if scale <= 0 {
        out.push_str("0.");
        for _ in 0..-scale {
            out.push('0');
        }
        out.push_str(&digits);
    } else {
        let scale = scale as usize;
        if digits.len() <= scale {
            out.push_str(&digits);
            for _ in digits.len()..scale {
                out.push('0');
            }
        } else {
            out.push_str(&digits[..scale]);
            out.push('.');
            out.push_str(&digits[scale..]);
        }
    }
    out
}

/// 원문을 그대로 들고 있는 수 — `arbitrary_precision` 이라 다시 쓸 때 이 글 그대로 나간다.
///
/// serde_json 은 수를 읽으며 지수를 고쳐 적는다(`1E+2` → `1e+2`, `1e5` → `1e+5`) — .NET `JsonNode` 는 원문을
/// 그대로 들고 다시 쓰므로 그 길로는 같은 바이트가 나오지 않는다. 그래서 JSON 수로 맞는지만 serde_json 으로 보고,
/// 값은 원문 글자로 짓는다(`Number::from_string_unchecked` — 숨은 API 라 serde_json 판을 `=` 로 묶어 둔다).
pub(crate) fn number(text: &str) -> Value {
    match serde_json::from_str::<Number>(text) {
        Ok(_) => Value::Number(Number::from_string_unchecked(text.to_string())),
        Err(_) => Value::Null,
    }
}

/// JSON 으로 읽기 — `JsonNode.Parse`/`JsonDocument.Parse` 처럼 수의 원문과 키의 차례를 지킨다.
///
/// serde_json 이 고쳐 적은 지수(위 [`number`])는 글에서 수의 원문을 차례대로 다시 주워 되돌린다. 같은 키가 두 번
/// 나와 값 하나가 빠지면 차례가 어긋날 수 있으므로, 주운 원문을 serde_json 식으로 고쳐 적은 것이 읽은 값과 하나도
/// 빠짐없이 같을 때만 되돌린다 — 아니면 고쳐 적힌 채로 둔다(같은 수다).
pub(crate) fn parse(text: &str) -> Result<Value, JsonException> {
    parse_with_depth(text, MAX_DEPTH)
}

/// [`parse`] 를 깊이 한도 `max_depth` 로 — `JsonDocumentOptions.MaxDepth` 를 따로 준 자리(확장 메시지는 32).
pub(crate) fn parse_with_depth(text: &str, max_depth: usize) -> Result<Value, JsonException> {
    let mut value: Value = serde_json::from_str(text).map_err(|_| JsonException)?;
    // .NET 은 64 단계보다 깊은 글을 읽지 않는다 — serde_json 의 한도(128)로 두면 65~128 단계를 Rust 만 읽는다.
    if too_deep(text, max_depth) {
        return Err(JsonException);
    }
    let raws = raw_numbers(text);
    let mut numbers = Vec::new();
    collect_numbers(&mut value, &mut numbers);
    if numbers.len() == raws.len()
        && numbers
            .iter()
            .zip(&raws)
            .all(|(n, raw)| n.as_str() == serde_spelling(raw))
    {
        for (n, raw) in numbers.into_iter().zip(raws) {
            if n.as_str() != raw {
                *n = Number::from_string_unchecked(raw.to_string());
            }
        }
    }
    Ok(value)
}

/// .NET `JsonReaderOptions.MaxDepth` 의 기본값 — `JsonDocument`·`JsonNode`·`Utf8JsonReader`·`JsonSerializer` 가 같다.
const MAX_DEPTH: usize = 64;

/// 객체·배열이 `max_depth` 단계보다 깊게 들었는가. 이미 serde_json 이 맞는 JSON 으로 읽은 글에만 쓴다.
fn too_deep(text: &str, max_depth: usize) -> bool {
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                i += 1;
                while i < bytes.len() && bytes[i] != b'"' {
                    i += if bytes[i] == b'\\' { 2 } else { 1 };
                }
            }
            b'{' | b'[' => {
                depth += 1;
                if depth > max_depth {
                    return true;
                }
            }
            b'}' | b']' => depth = depth.saturating_sub(1),
            _ => {}
        }
        i += 1;
    }
    false
}

/// 글 안 어느 객체에든 같은 키가 두 번 나오는가. 키는 이스케이프를 푼 UTF-16 단위로 견준다 — `JsonObject` 의 열쇠
/// (대소문자를 가린다)와 같다. C# 의 `JsonNode` 는 그런 객체를 열 때 `ArgumentException` 을 던지고, 역직렬화와
/// serde_json 은 뒤의 것을 조용히 골라 읽는 쪽마다 값이 갈리므로, 저장된 식에서는 형식 오류로 막는다.
///
/// serde_json 은 겹친 키를 하나로 접어 값만 봐서는 알 수 없어 글을 직접 훑는다. 이미 [`parse`] 를 지난 글에만 쓴다.
pub(crate) fn has_duplicate_keys(text: &str) -> bool {
    first_duplicate_key(text).is_some()
}

/// 글의 차례로 처음 두 번째 나온 키(이스케이프를 푼 것). C# `JsonObject` 가 그 객체를 열며 던지는 `ArgumentException`
/// 의 `Key:` 와 같다. 이미 [`parse`] 를 지난 글에만 쓴다.
pub(crate) fn first_duplicate_key(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    // 열린 묶음마다 — 객체면 본 키들, 배열이면 `None`.
    let mut open: Vec<Option<HashSet<Vec<u16>>>> = Vec::new();
    let mut expect_key = false;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => {
                open.push(Some(HashSet::new()));
                expect_key = true;
            }
            b'[' => {
                open.push(None);
                expect_key = false;
            }
            b'}' | b']' => {
                open.pop();
                expect_key = false;
            }
            b',' => expect_key = matches!(open.last(), Some(Some(_))),
            b'"' => {
                let start = i + 1;
                i = start;
                while i < bytes.len() && bytes[i] != b'"' {
                    i += if bytes[i] == b'\\' { 2 } else { 1 };
                }
                if expect_key {
                    expect_key = false;
                    let key = unescape_utf16(&text[start..i]);
                    if let Some(Some(seen)) = open.last_mut()
                        && !seen.insert(key.clone())
                    {
                        return Some(String::from_utf16_lossy(&key));
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// JSON 문자열 안쪽(따옴표 뺀 것)의 이스케이프를 풀어 UTF-16 단위로(서로게이트 짝 이스케이프는 단위 둘이 되어 그 글자와 같다).
fn unescape_utf16(inner: &str) -> Vec<u16> {
    let mut out = Vec::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            let mut buffer = [0u16; 2];
            out.extend_from_slice(c.encode_utf16(&mut buffer));
            continue;
        }
        match chars.next() {
            Some('b') => out.push(0x08),
            Some('f') => out.push(0x0C),
            Some('n') => out.push(0x0A),
            Some('r') => out.push(0x0D),
            Some('t') => out.push(0x09),
            Some('u') => {
                let hex: String = chars.by_ref().take(4).collect();
                out.push(u16::from_str_radix(&hex, 16).unwrap_or(0));
            }
            // 따옴표·역빗금·빗금
            Some(other) => out.push(other as u16),
            None => {}
        }
    }
    out
}

/// 글에 적힌 수의 원문을 나온 차례대로. 따옴표 안은 건너뛴다. 이미 serde_json 이 맞는 JSON 으로 읽은 글에만 쓴다.
fn raw_numbers(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                i += 1;
                while i < bytes.len() && bytes[i] != b'"' {
                    i += if bytes[i] == b'\\' { 2 } else { 1 };
                }
                i += 1;
            }
            b'-' | b'0'..=b'9' => {
                let start = i;
                while i < bytes.len()
                    && matches!(bytes[i], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9')
                {
                    i += 1;
                }
                out.push(&text[start..i]);
            }
            _ => i += 1,
        }
    }
    out
}

/// serde_json 이 수를 읽고 남기는 꼴 — 지수 글자는 `e`, 부호가 없으면 `+`. 정수는 `u64`·`i64` 에 들면 그 수로
/// 바꿔 다시 적으므로 `-0` 이 `0` 이 된다(.NET 은 `-0` 그대로).
fn serde_spelling(raw: &str) -> String {
    if !raw.contains(['.', 'e', 'E'])
        && let Ok(n) = raw.parse::<i64>()
    {
        return n.to_string();
    }
    match raw.find(['e', 'E']) {
        Some(at) => {
            let (head, tail) = raw.split_at(at);
            let tail = &tail[1..];
            if tail.starts_with(['+', '-']) {
                format!("{head}e{tail}")
            } else {
                format!("{head}e+{tail}")
            }
        }
        None => raw.to_string(),
    }
}

/// 값 안의 수를 글에 나온 차례(객체는 키의 차례, 배열은 칸의 차례)로.
fn collect_numbers<'a>(value: &'a mut Value, out: &mut Vec<&'a mut Number>) {
    match value {
        Value::Number(n) => out.push(n),
        Value::Array(items) => items.iter_mut().for_each(|v| collect_numbers(v, out)),
        Value::Object(map) => map.values_mut().for_each(|v| collect_numbers(v, out)),
        _ => {}
    }
}

/// C# `JsonException` — 글이 JSON 이 아니거나, 받을 자리와 꼴이 맞지 않는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct JsonException;

pub(crate) type JsonResult<T> = Result<T, JsonException>;

// ── 받는 자리의 꼴로 읽기(역직렬화) ─────────────────────────────────────

/// 객체 자리. JSON `null` 은 `None`.
pub(crate) fn read_object(value: &Value) -> JsonResult<Option<&Map<String, Value>>> {
    match value {
        Value::Null => Ok(None),
        Value::Object(map) => Ok(Some(map)),
        _ => Err(JsonException),
    }
}

/// 문자열 자리. 수·참거짓은 받지 않는다.
pub(crate) fn read_string(value: &Value) -> JsonResult<Option<String>> {
    match value {
        Value::Null => Ok(None),
        Value::String(s) => Ok(Some(s.clone())),
        _ => Err(JsonException),
    }
}

/// `int` 자리. 수는 정수 꼴(`3`)만 — `3.0`·`1e0` 은 거절한다(실측). 글로 온 수(`"3"`)도 받는다.
pub(crate) fn read_i32(value: &Value) -> JsonResult<Option<i32>> {
    let text = match value {
        Value::Null => return Ok(None),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        _ => return Err(JsonException),
    };
    let body = text.strip_prefix(['-', '+']).unwrap_or(&text);
    if body.is_empty() || !body.bytes().all(|b| b.is_ascii_digit()) {
        return Err(JsonException);
    }
    text.parse::<i32>().map(Some).map_err(|_| JsonException)
}

/// 정수 자리(`int`, null 을 받지 않는다).
pub(crate) fn read_required_i32(value: &Value) -> JsonResult<i32> {
    read_i32(value)?.ok_or(JsonException)
}

/// `double` 자리. 수는 무엇이든(넘치면 무한), 글로 온 수도 받는다.
pub(crate) fn read_f64(value: &Value) -> JsonResult<Option<f64>> {
    let text = match value {
        Value::Null => return Ok(None),
        Value::Number(n) => n.to_string(),
        Value::String(s) => {
            if s.is_empty()
                || !s
                    .bytes()
                    .all(|b| b.is_ascii_digit() || matches!(b, b'+' | b'-' | b'.' | b'e' | b'E'))
            {
                return Err(JsonException);
            }
            s.clone()
        }
        _ => return Err(JsonException),
    };
    text.parse::<f64>().map(Some).map_err(|_| JsonException)
}

/// `bool` 자리. 글은 받지 않는다(실측: `"true"` 는 거절).
pub(crate) fn read_bool(value: &Value) -> JsonResult<Option<bool>> {
    match value {
        Value::Null => Ok(None),
        Value::Bool(b) => Ok(Some(*b)),
        _ => Err(JsonException),
    }
}

/// 배열 자리. 원소는 `item` 으로 읽는다.
pub(crate) fn read_array<T>(
    value: &Value,
    item: impl Fn(&Value) -> JsonResult<T>,
) -> JsonResult<Option<Vec<T>>> {
    match value {
        Value::Null => Ok(None),
        Value::Array(items) => items
            .iter()
            .map(item)
            .collect::<JsonResult<Vec<T>>>()
            .map(Some),
        _ => Err(JsonException),
    }
}

/// 객체의 속성을 대소문자 가리지 않고 받을 자리로 나눈다. `names` 의 몇째 이름인지와 값을 차례대로 넘긴다 —
/// 같은 이름이 두 번 오면 두 번 넘어가므로 뒤의 것이 이긴다. 모르는 속성은 `strict` 면 거절한다
/// (`JsonUnmappedMemberHandling.Disallow`), 아니면 건너뛴다.
pub(crate) fn for_each_property(
    map: &Map<String, Value>,
    names: &[&str],
    strict: bool,
    mut set: impl FnMut(usize, &Value) -> JsonResult<()>,
) -> JsonResult<()> {
    for (key, value) in map {
        match names.iter().position(|name| eq_ignore_case(name, key)) {
            Some(index) => set(index, value)?,
            None if strict => return Err(JsonException),
            None => {}
        }
    }
    Ok(())
}

// ── JsonNode 로 다니기 ──────────────────────────────────────────────

/// C# 이 `JsonNode` 를 다니다 낸 `InvalidOperationException` — 배열·값에 이름으로 물었거나, 글이 아닌 것을 글로
/// 꺼냈다. 판올림 보정은 이것을 잡아 그 줄을 건너뛴다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NodeError;

/// `node?["key"]`. 없는 속성과 JSON `null` 은 `None`(C# 의 `null` 노드). 객체가 아닌 노드에 물으면 오류다.
pub(crate) fn index<'a>(
    node: Option<&'a Value>,
    key: &str,
) -> Result<Option<&'a Value>, NodeError> {
    match node {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Object(map)) => Ok(map.get(key).filter(|v| !v.is_null())),
        Some(_) => Err(NodeError),
    }
}

/// `(string?)node`. 글이 아닌 값이면 오류다.
pub(crate) fn node_string(node: Option<&Value>) -> Result<Option<&str>, NodeError> {
    match node {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s)),
        Some(_) => Err(NodeError),
    }
}

/// `node?.AsArray()`. 배열이 아니면 오류다.
pub(crate) fn as_array(node: Option<&Value>) -> Result<Option<&Vec<Value>>, NodeError> {
    match node {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(items)) => Ok(Some(items)),
        Some(_) => Err(NodeError),
    }
}

/// `JsonNode.DeepEquals`(.NET 8) — 객체는 키 차례를 보지 않고, 값은 쓴 글로 견준다(수는 원문 그대로).
pub(crate) fn deep_equals(a: Option<&Value>, b: Option<&Value>) -> bool {
    let a = a.filter(|v| !v.is_null());
    let b = b.filter(|v| !v.is_null());
    match (a, b) {
        (None, None) => true,
        (Some(Value::Object(x)), Some(Value::Object(y))) => {
            x.len() == y.len()
                && x.iter().all(|(key, value)| match y.get(key) {
                    Some(other) => deep_equals(Some(value), Some(other)),
                    None => false,
                })
        }
        (Some(Value::Array(x)), Some(Value::Array(y))) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| deep_equals(Some(p), Some(q)))
        }
        (Some(Value::Object(_) | Value::Array(_)), _)
        | (_, Some(Value::Object(_) | Value::Array(_))) => false,
        (Some(p), Some(q)) => to_compact(p, Encoder::Default) == to_compact(q, Encoder::Default),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 같은 키는 어느 깊이의 객체에서든, 이스케이프를 풀어 같으면 겹친 것이다. 대소문자가 다르면 다른 키다.
    #[test]
    fn duplicate_keys_are_found_at_any_depth() {
        let cases: &[(&str, bool)] = &[
            (r#"{"a":1,"b":2}"#, false),
            (r#"{"a":1,"a":1}"#, true),
            (r#"{"a":1,"A":1}"#, false),
            (r#"{"a":{"b":1},"c":{"b":1}}"#, false),
            (r#"{"a":[{"b":1,"b":2}]}"#, true),
            (r#"[{"x":1},{"x":1}]"#, false),
            (r#"{"a":"a","b":"a"}"#, false),
            (r#"{"k":"a,\"b \":1","b":1}"#, false),
            (r#"{"a":1,"\u0061":2}"#, true),
            (r#"{"\"":1,"\u0022":2}"#, true),
            (r#"{"\ud83d\ude00":1,"😀":2}"#, true),
            (r#"{"a":{},"b":[],"a ":1}"#, false),
        ];
        for (text, expected) in cases {
            assert!(parse(text).is_ok(), "{text}");
            assert_eq!(has_duplicate_keys(text), *expected, "{text}");
        }
    }

    fn s(text: &str, encoder: Encoder) -> String {
        let mut out = String::new();
        write_string(&mut out, text, encoder);
        out
    }

    /// 기대값은 .NET 8 의 `JsonSerializer.Serialize(string, options)` 가 낸 글이다(scratch 콘솔 실측).
    #[test]
    fn relaxed_escaping_matches_dotnet() {
        let cases = [
            ("\u{0}", "\"\\u0000\""),
            ("\u{8}", "\"\\b\""),
            ("\u{1F}", "\"\\u001F\""),
            ("\"", "\"\\\"\""),
            ("&'+<>`", "\"&'+<>`\""),
            ("\\", "\"\\\\\""),
            ("\u{7F}\u{80}\u{A0}", "\"\\u007F\\u0080\\u00A0\""),
            ("\u{AD}", "\"\u{AD}\""),
            ("\u{2028}\u{2029}", "\"\\u2028\\u2029\""),
            ("한\u{FFFD}", "\"한\u{FFFD}\""),
            ("\u{FEFF}", "\"\\uFEFF\""),
            ("\u{1F600}", "\"\\uD83D\\uDE00\""),
        ];
        for (input, expected) in cases {
            assert_eq!(s(input, Encoder::Relaxed), expected, "{input:?}");
        }
    }

    #[test]
    fn default_escaping_matches_dotnet() {
        let cases = [
            ("\"", "\"\\u0022\""),
            ("&'+<>`", "\"\\u0026\\u0027\\u002B\\u003C\\u003E\\u0060\""),
            ("\\\n", "\"\\\\\\n\""),
            ("é한", "\"\\u00E9\\uD55C\""),
            ("\u{AD}", "\"\\u00AD\""),
            ("\u{1F600}", "\"\\uD83D\\uDE00\""),
        ];
        for (input, expected) in cases {
            assert_eq!(s(input, Encoder::Default), expected, "{input:?}");
        }
    }

    /// 쪽지의 인코더 — 한글은 그대로, HTML 에 민감한 글자와 느슨한 쪽이 이스케이프하는 글자는 `\uXXXX`.
    #[test]
    fn all_ranges_escaping_keeps_hangul() {
        let cases = [
            ("\"", r#""\u0022""#),
            ("&'+<>`", r#""\u0026\u0027\u002B\u003C\u003E\u0060""#),
            (r"C:\계약\é.pclm", r#""C:\\계약\\é.pclm""#),
            ("\u{7F}\u{A0}\u{AD}", "\"\\u007F\\u00A0\u{AD}\""),
            ("\u{1F600}", r#""\uD83D\uDE00""#),
        ];
        for (input, expected) in cases {
            assert_eq!(s(input, Encoder::AllRanges), expected, "{input:?}");
        }
    }

    /// 기대값은 `Flow.Serialize` 가 `FlowSource.Value` 를 쓴 글이다(실측).
    #[test]
    fn doubles_match_dotnet() {
        let cases = [
            (0.0, "0"),
            (-0.0, "-0"),
            (1.0, "1"),
            (-1.0, "-1"),
            (1.5, "1.5"),
            (0.1, "0.1"),
            (0.30000000000000004, "0.30000000000000004"),
            (100.0, "100"),
            (1e14, "100000000000000"),
            (1e15, "1000000000000000"),
            (1e16, "10000000000000000"),
            (123456789012345678.0, "1.2345678901234568E+17"),
            (1e21, "1E+21"),
            (1e22, "1E+22"),
            (0.001, "0.001"),
            (0.0001, "0.0001"),
            (0.00001, "1E-05"),
            (1e-7, "1E-07"),
            (1.7976931348623157e308, "1.7976931348623157E+308"),
            (5e-324, "5E-324"),
            (12345.678, "12345.678"),
            (2.5e-5, "2.5E-05"),
            (1.0 / 3.0, "0.3333333333333333"),
        ];
        for (input, expected) in cases {
            assert_eq!(format_double(input), expected, "{input:?}");
        }
    }

    /// `JsonNode.Parse(…).ToJsonString(Flow.Json)` 의 실측 — 수 원문과 키 차례가 남고 글은 다시 이스케이프된다.
    #[test]
    fn node_round_trip_matches_dotnet() {
        let node = parse(r#"{"a":"\u0041<>&'+\u00e9\ud55c","b":1.50,"c":null,"d":1E+2,"e":true}"#)
            .unwrap();
        assert_eq!(
            to_compact(&node, Encoder::Relaxed),
            "{\"a\":\"A<>&'+é한\",\"b\":1.50,\"c\":null,\"d\":1E+2,\"e\":true}"
        );
        assert_eq!(
            to_compact(&node, Encoder::Default),
            "{\"a\":\"A\\u003C\\u003E\\u0026\\u0027\\u002B\\u00E9\\uD55C\",\"b\":1.50,\"c\":null,\"d\":1E+2,\"e\":true}"
        );
        // serde_json 이 고쳐 적는 지수(부호 없는 것, 대문자)도 원문 그대로. 따옴표 안의 수 같은 글은 수가 아니다.
        let node = parse(r#"["1e5 \"2\"",1e5,-0,2E-3,[0.0]]"#).unwrap();
        assert_eq!(
            to_compact(&node, Encoder::Relaxed),
            r#"["1e5 \"2\"",1e5,-0,2E-3,[0.0]]"#
        );
        assert_eq!(to_compact(&number("1E+21"), Encoder::Relaxed), "1E+21");
    }

    #[test]
    fn indented_matches_dotnet() {
        let node = parse(r#"{"a":1,"b":[1],"c":[],"d":{}}"#).unwrap();
        assert_eq!(
            to_indented(&node, Encoder::Default),
            "{\r\n  \"a\": 1,\r\n  \"b\": [\r\n    1\r\n  ],\r\n  \"c\": [],\r\n  \"d\": {}\r\n}"
        );
    }

    #[test]
    fn integers_read_like_dotnet() {
        assert_eq!(read_i32(&parse("3").unwrap()), Ok(Some(3)));
        assert_eq!(read_i32(&parse("\"3\"").unwrap()), Ok(Some(3)));
        assert_eq!(read_i32(&parse("3.0").unwrap()), Err(JsonException));
        assert_eq!(read_i32(&parse("1e0").unwrap()), Err(JsonException));
    }
}

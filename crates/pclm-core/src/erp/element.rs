//! 수집 입력의 JSON 을 걷는 손 — 칸이 없거나 꼴이 다르면 그 까닭을 [`Error::InvalidOperation`] 으로 낸다.
//!
//! 확장 호스트는 이 글을 그대로 응답에 싣는다.

use serde_json::Value;

use crate::error::Error;

/// 값의 꼴 이름. 참과 거짓은 다른 꼴로 친다.
pub(crate) fn kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(true) => "참",
        Value::Bool(false) => "거짓",
        Value::Number(_) => "수",
        Value::String(_) => "글",
        Value::Array(_) => "배열",
        Value::Object(_) => "객체",
    }
}

/// 값이 바란 꼴이 아니다.
pub(crate) fn wrong_type(expected: &str, actual: &Value) -> Error {
    Error::InvalidOperation(format!(
        "값의 꼴이 다릅니다(바란 것: {expected}, 받은 것: {}).",
        kind(actual)
    ))
}

/// 객체에 그 이름의 칸이 없다.
pub(crate) fn key_not_found(name: &str) -> Error {
    Error::InvalidOperation(format!("{name} 칸이 없습니다."))
}

/// 수집 입력에 `data` 가 없다.
pub(crate) fn undefined() -> Error {
    Error::InvalidOperation("수집 입력에 data 가 없습니다.".into())
}

/// `element.GetProperty(name)`.
pub(crate) fn get<'a>(value: &'a Value, name: &str) -> Result<&'a Value, Error> {
    try_get(value, name)?.ok_or_else(|| key_not_found(name))
}

/// `element.TryGetProperty(name, out _)`. 객체가 아니면 던진다.
pub(crate) fn try_get<'a>(value: &'a Value, name: &str) -> Result<Option<&'a Value>, Error> {
    match value {
        Value::Object(map) => Ok(map.get(name)),
        other => Err(wrong_type("객체", other)),
    }
}

/// `element.EnumerateArray()`. 배열이 아니면 던진다.
pub(crate) fn array(value: &Value) -> Result<&Vec<Value>, Error> {
    match value {
        Value::Array(items) => Ok(items),
        other => Err(wrong_type("배열", other)),
    }
}

/// `element.GetString()`. 글·null 이 아니면 던진다.
pub(crate) fn string(value: &Value) -> Result<Option<&str>, Error> {
    match value {
        Value::Null => Ok(None),
        Value::String(s) => Ok(Some(s)),
        other => Err(wrong_type("글", other)),
    }
}

/// `TryGetProperty(name, out v) && v.ValueKind == String ? v.GetString() : null` — 객체가 아니면 `None`.
pub(crate) fn text_property<'a>(value: &'a Value, name: &str) -> Option<&'a str> {
    value.as_object()?.get(name)?.as_str()
}

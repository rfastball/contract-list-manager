//! `JsonElement` 로 다니며 C# 이 던지던 글 — 수집 입력을 걷는 자리가 .NET 의 예외 글을 그대로 낸다.
//!
//! 확장 호스트(`ErpCapture.Dispatch`)는 `InvalidOperationException`·`KeyNotFoundException`·`FormatException` 의 글을 그대로
//! 응답에 싣는다. 그래서 같은 자리에서 같은 글이 나와야 한다 — 이 크레이트는 그 셋을 모두
//! [`Error::InvalidOperation`] 으로 낸다(부르는 쪽들이 셋을 같이 잡는다).

use serde_json::Value;

use crate::error::Error;

/// `JsonElement.ValueKind` 의 이름.
pub(crate) fn kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "Null",
        Value::Bool(true) => "True",
        Value::Bool(false) => "False",
        Value::Number(_) => "Number",
        Value::String(_) => "String",
        Value::Array(_) => "Array",
        Value::Object(_) => "Object",
    }
}

/// 꼴이 틀린 원소에 물었을 때의 `InvalidOperationException`.
pub(crate) fn wrong_type(expected: &str, actual: &Value) -> Error {
    Error::InvalidOperation(format!(
        "The requested operation requires an element of type '{expected}', but the target element has type '{}'.",
        kind(actual)
    ))
}

/// `KeyNotFoundException` — `GetProperty` 가 이름을 못 찾았다.
pub(crate) fn key_not_found() -> Error {
    Error::InvalidOperation("The given key was not present in the dictionary.".into())
}

/// 기본값 `JsonElement`(Undefined)에 물었을 때의 `InvalidOperationException`.
pub(crate) fn undefined() -> Error {
    Error::InvalidOperation("Operation is not valid due to the current state of the object.".into())
}

/// `element.GetProperty(name)`.
pub(crate) fn get<'a>(value: &'a Value, name: &str) -> Result<&'a Value, Error> {
    try_get(value, name)?.ok_or_else(key_not_found)
}

/// `element.TryGetProperty(name, out _)`. 객체가 아니면 던진다.
pub(crate) fn try_get<'a>(value: &'a Value, name: &str) -> Result<Option<&'a Value>, Error> {
    match value {
        Value::Object(map) => Ok(map.get(name)),
        other => Err(wrong_type("Object", other)),
    }
}

/// `element.EnumerateArray()`. 배열이 아니면 던진다.
pub(crate) fn array(value: &Value) -> Result<&Vec<Value>, Error> {
    match value {
        Value::Array(items) => Ok(items),
        other => Err(wrong_type("Array", other)),
    }
}

/// `element.GetString()`. 글·null 이 아니면 던진다.
pub(crate) fn string(value: &Value) -> Result<Option<&str>, Error> {
    match value {
        Value::Null => Ok(None),
        Value::String(s) => Ok(Some(s)),
        other => Err(wrong_type("String", other)),
    }
}

/// `TryGetProperty(name, out v) && v.ValueKind == String ? v.GetString() : null` — 객체가 아니면 `None`.
pub(crate) fn text_property<'a>(value: &'a Value, name: &str) -> Option<&'a str> {
    value.as_object()?.get(name)?.as_str()
}

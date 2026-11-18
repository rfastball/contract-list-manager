//! wasm-bindgen 없이 C ABI 로 내보낸다. 입구는 하나다:
//!
//! `engine_call(op, 요청 JSON, 바이트) -> 0 | 1`, 출력은 `[응답 JSON 길이 u32 LE][응답 JSON][바이트]`.
//! 실패(1)면 출력은 사람이 읽을 까닭 글뿐이다.
//!
//! 부르는 쪽(확장의 엔진 모듈)의 차례: `alloc` → 입력 복사 → 호출 → `out_ptr`/`out_len` 읽기 → 입력 `dealloc`.
//! 요청은 패널이 부르는 것만 둔다. `case` 는 호스트 `document {op:"read"}` 의 답이다.

use serde_json::{Map, Value, json};
use std::cell::RefCell;

thread_local! {
    static OUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

#[unsafe(no_mangle)]
pub extern "C" fn alloc(len: usize) -> *mut u8 {
    let mut buf = Vec::<u8>::with_capacity(len);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

/// # Safety
/// `ptr` 은 같은 `len` 으로 [`alloc`] 이 준 것이어야 한다.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dealloc(ptr: *mut u8, len: usize) {
    unsafe { drop(Vec::from_raw_parts(ptr, 0, len)) };
}

#[unsafe(no_mangle)]
pub extern "C" fn out_ptr() -> *const u8 {
    OUT.with(|o| o.borrow().as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn out_len() -> usize {
    OUT.with(|o| o.borrow().len())
}

fn object<'a>(request: &'a Value, key: &str) -> &'a Map<String, Value> {
    static EMPTY: std::sync::OnceLock<Map<String, Value>> = std::sync::OnceLock::new();
    request[key]
        .as_object()
        .unwrap_or_else(|| EMPTY.get_or_init(Map::new))
}

fn dispatch(op: &str, request: &Value, bytes: &[u8]) -> Result<(Value, Vec<u8>), String> {
    Ok(match op {
        "vocabulary" => (
            serde_json::from_str(crate::vocab::SOURCE).map_err(|e| e.to_string())?,
            vec![],
        ),
        "resolve" => (
            json!({ "values": crate::values::resolve(&request["case"], &request["context"]) }),
            vec![],
        ),
        // 이름을 짓지 못해도 실패(1)로 내지 않는다 — 패널이 없는 칸을 구조로 받아 한 줄을 짓는다.
        "file_name" => (
            match crate::naming::file_name(
                &request["case"],
                request["template"].as_str().unwrap_or_default(),
                request["pattern"].as_str(),
                request["today"].as_str(),
            ) {
                Ok(name) => json!({ "name": name }),
                Err(crate::naming::NameError::Missing(missing)) => json!({ "missing": missing }),
                Err(crate::naming::NameError::Pattern(error)) => json!({ "error": error }),
            },
            vec![],
        ),
        "prepare" => {
            let (doc, report) = crate::template::prepare(bytes)?;
            (report, doc)
        }
        "make" => {
            let (doc, report) = crate::template::make(
                bytes,
                &request["case"],
                &request["context"],
                object(request, "selections"),
            )?;
            (report, doc)
        }
        "inspect_text" => (
            crate::text::inspect(request["text"].as_str().unwrap_or_default()),
            vec![],
        ),
        "make_text" => (
            crate::text::make(
                request["text"].as_str().unwrap_or_default(),
                &request["case"],
                &request["context"],
                object(request, "selections"),
            )?,
            vec![],
        ),
        other => return Err(format!("모르는 요청입니다: {other}")),
    })
}

/// # Safety
/// 세 쌍 모두 [`alloc`] 으로 받아 채운 바이트여야 한다(요청 이름, 요청 JSON, 바이트 — 비면 길이 0).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn engine_call(
    op_ptr: *const u8,
    op_len: usize,
    json_ptr: *const u8,
    json_len: usize,
    bytes_ptr: *const u8,
    bytes_len: usize,
) -> u32 {
    let slice = |ptr: *const u8, len: usize| {
        if len == 0 {
            &[][..]
        } else {
            unsafe { std::slice::from_raw_parts(ptr, len) }
        }
    };
    let op = String::from_utf8_lossy(slice(op_ptr, op_len)).into_owned();
    let request: Result<Value, String> = if json_len == 0 {
        Ok(Value::Null)
    } else {
        serde_json::from_slice(slice(json_ptr, json_len)).map_err(|e| format!("요청 JSON: {e}"))
    };
    let result = request.and_then(|request| dispatch(&op, &request, slice(bytes_ptr, bytes_len)));
    let (status, out) = match result {
        Ok((response, bytes)) => {
            let response = serde_json::to_vec(&response).expect("응답");
            let mut out = (response.len() as u32).to_le_bytes().to_vec();
            out.extend(response);
            out.extend(bytes);
            (0, out)
        }
        Err(e) => (1, e.into_bytes()),
    };
    OUT.with(|o| *o.borrow_mut() = out);
    status
}

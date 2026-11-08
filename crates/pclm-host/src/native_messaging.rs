//! Chrome/Edge Native Messaging(C# `Erp/NativeMessaging.cs`) — 4바이트 little-endian 길이 머리와 JSON 몸. stdout 에는
//! 그것만 쓴다. 글의 뜻(깊이 32·겹친 키·Web 직렬화)은 Core 의 [`wire`] 가 쥔다.

use std::io::{self, Read, Write};
use std::sync::Mutex;

use pclm_core::erp::wire::{self, MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES};
use serde_json::{Map, Value};

/// 고리가 멈춘 까닭. 둘 다 호스트를 1 로 끝낸다(C# 은 `IOException`·`InvalidDataException` 을 함께 잡는다).
#[derive(Debug)]
pub enum Error {
    /// 끊긴 파이프·중간에 끝난 메시지(C# `EndOfStreamException` 등).
    Io(io::Error),
    /// 길이 머리가 0 이거나 요청 한도를 넘었다, 또는 답이 응답 한도를 넘었다(C# `InvalidDataException`).
    InvalidData(&'static str),
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(e) => write!(f, "{e}"),
            Error::InvalidData(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for Error {}

/// stdout 을 줄 세우는 자물쇠 — 답 말고도 밀어 보내는 것(호스트 → 확장 명령)이 있어, 두 실마리가 한 메시지의 길이와 몸을
/// 섞어 쓰지 않게 한다.
pub fn lock<W>(output: &Mutex<W>) -> std::sync::MutexGuard<'_, W> {
    output
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// 메시지 고리(C# `NativeMessaging.Run`). 입력이 메시지 경계에서 끝나면 `Ok`. 읽지 못한 글은 `dispatch` 에 넘기지 않고
/// `invalid_json` 으로 답한다.
pub fn run<R: Read, W: Write>(
    mut input: R,
    output: &Mutex<W>,
    mut dispatch: impl FnMut(&Value) -> Value,
) -> Result<(), Error> {
    let mut header = [0u8; 4];
    loop {
        if !read_first(&mut input, &mut header[0])? {
            return Ok(());
        }
        input.read_exact(&mut header[1..])?;
        let length = u32::from_le_bytes(header) as usize;
        if length == 0 || length > MAX_REQUEST_BYTES {
            return Err(Error::InvalidData("잘못된 메시지 길이"));
        }
        let mut payload = vec![0u8; length];
        input.read_exact(&mut payload)?;
        let response = match wire::parse_message(&payload) {
            Some(request) => dispatch(&request),
            None => invalid_json(),
        };
        send(output, &response)?;
    }
}

/// 첫 바이트 — 입력이 끝났으면 `false`(C# `ReadByte() < 0`).
fn read_first<R: Read>(input: &mut R, byte: &mut u8) -> io::Result<bool> {
    loop {
        match input.read(std::slice::from_mut(byte)) {
            Ok(0) => return Ok(false),
            Ok(_) => return Ok(true),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
}

/// 메시지 하나를 길이 머리와 함께 쓴다(C# `NativeMessaging.Send`). 응답 한도를 넘으면 쓰지 않고 멈춘다.
pub fn send<W: Write>(output: &Mutex<W>, message: &Value) -> Result<(), Error> {
    let bytes = wire::to_web_json(message).into_bytes();
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err(Error::InvalidData("응답 크기 초과"));
    }
    let header = (bytes.len() as u32).to_le_bytes();
    let mut out = lock(output);
    out.write_all(&header)?;
    out.write_all(&bytes)?;
    out.flush()?;
    Ok(())
}

/// 읽지 못한 글의 답.
pub fn invalid_json() -> Value {
    envelope(None, "invalid_json", "JSON 메시지를 읽지 못했습니다.")
}

/// 실패한 답의 꼴 — `{protocolVersion: 2, requestId, ok: false, error: {code, message}}`.
pub fn envelope(request_id: Option<&str>, code: &str, message: &str) -> Value {
    let mut error = Map::new();
    error.insert("code".into(), Value::String(code.into()));
    error.insert("message".into(), Value::String(message.into()));
    let mut out = Map::new();
    out.insert("protocolVersion".into(), 2.into());
    out.insert(
        "requestId".into(),
        request_id.map_or(Value::Null, |id| Value::String(id.into())),
    );
    out.insert("ok".into(), Value::Bool(false));
    out.insert("error".into(), Value::Object(error));
    Value::Object(out)
}

/// 성공한 답의 꼴 — `{protocolVersion: 2, requestId, ok: true, result: {status}}`.
pub fn status_reply(request_id: Option<&str>, status: &str) -> Value {
    let mut result = Map::new();
    result.insert("status".into(), Value::String(status.into()));
    let mut out = Map::new();
    out.insert("protocolVersion".into(), 2.into());
    out.insert(
        "requestId".into(),
        request_id.map_or(Value::Null, |id| Value::String(id.into())),
    );
    out.insert("ok".into(), Value::Bool(true));
    out.insert("result".into(), Value::Object(result));
    Value::Object(out)
}

/// `request.TryGetProperty("requestId", out var id) && id.ValueKind == String ? id.GetString() : null`.
pub fn request_id(request: &Value) -> Option<&str> {
    request.get("requestId").and_then(Value::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn frame(payload: &[u8]) -> Vec<u8> {
        let mut out = (payload.len() as u32).to_le_bytes().to_vec();
        out.extend_from_slice(payload);
        out
    }

    fn replies(bytes: &[u8]) -> Vec<Value> {
        let mut out = Vec::new();
        let mut at = 0;
        while at < bytes.len() {
            let n = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
            out.push(serde_json::from_slice(&bytes[at + 4..at + 4 + n]).unwrap());
            at += 4 + n;
        }
        out
    }

    /// 한 바이트씩만 내주는 입력 — 잘린 읽기에서도 길이를 지키는가.
    struct OneByte(std::io::Cursor<Vec<u8>>);

    impl Read for OneByte {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let n = buf.len().min(1);
            self.0.read(&mut buf[..n])
        }
    }

    #[test]
    fn 통신은_잘린_읽기와_연속_메시지에서_utf8_바이트_길이를_지킨다() {
        let payload = "{\"value\":\"한글\"}".as_bytes();
        let input = OneByte(std::io::Cursor::new(
            [frame(payload), frame(payload)].concat(),
        ));
        let output = Mutex::new(Vec::new());
        run(
            input,
            &output,
            |root| serde_json::json!({ "value": root["value"] }),
        )
        .unwrap();
        let out = output.into_inner().unwrap();
        let replies = replies(&out);
        assert_eq!(replies.len(), 2);
        assert!(replies.iter().all(|r| r["value"] == "한글"));
        // 비 ASCII 는 \uXXXX 로 나간다(Web 기본 인코더).
        assert!(
            std::str::from_utf8(&out[4..])
                .unwrap()
                .starts_with(r#"{"value":"\uD55C\uAE00"}"#)
        );
    }

    #[test]
    fn 과대한_프레임은_할당과_호출_전에_거부한다() {
        for length in [0u32, MAX_REQUEST_BYTES as u32 + 1] {
            let output = Mutex::new(Vec::new());
            let result = run(&length.to_le_bytes()[..], &output, |_| {
                panic!("호출되면 안 됨")
            });
            assert!(matches!(result, Err(Error::InvalidData(_))));
            assert!(output.into_inner().unwrap().is_empty());
        }
    }

    #[test]
    fn 중간에_끝난_메시지는_입출력_오류다() {
        let output = Mutex::new(Vec::new());
        let mut truncated = frame(b"{}");
        truncated.pop();
        assert!(matches!(
            run(&truncated[..], &output, |_| Value::Null),
            Err(Error::Io(_))
        ));
        assert!(matches!(
            run(&[1u8, 0][..], &output, |_| Value::Null),
            Err(Error::Io(_))
        ));
        assert!(run(&[][..], &output, |_| Value::Null).is_ok());
    }

    #[test]
    fn 읽지_못한_글은_넘기지_않고_invalid_json_으로_답한다() {
        let input = [frame(b"{ broken"), frame(br#"{"a":1,"a":2}"#)].concat();
        let output = Mutex::new(Vec::new());
        run(&input[..], &output, |_| panic!("호출되면 안 됨")).unwrap();
        let out = output.into_inner().unwrap();
        for reply in replies(&out) {
            assert_eq!(reply["error"]["code"], "invalid_json");
            assert_eq!(reply["requestId"], Value::Null);
        }
    }

    #[test]
    fn 응답_한도를_넘으면_쓰지_않고_멈춘다() {
        let output = Mutex::new(Vec::new());
        let big = Value::String("a".repeat(MAX_RESPONSE_BYTES));
        assert!(matches!(send(&output, &big), Err(Error::InvalidData(_))));
        assert!(output.into_inner().unwrap().is_empty());
    }
}

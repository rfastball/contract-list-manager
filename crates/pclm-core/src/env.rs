//! 시계와 식별자 — 호출자가 넣어 준다(#8).
//!
//! C# 은 `DateTime.UtcNow`·`Guid.NewGuid()` 를 그 자리에서 부르고, SQL 안에서도 `strftime('now')`·
//! `randomblob(16)` 로 값을 지었다. 이 크레이트는 그 어느 것도 스스로 부르지 않는다 — 같은 입력에서 같은
//! 파일이 서야 대조(#8 E1~E4)가 되고, 문서나르미가 제 시계·식별자를 쓸 수 있다.

/// 시각과 식별자를 내주는 쪽.
pub trait Env {
    /// 지금 UTC 시각. .NET `DateTime.UtcNow.ToString("O")` 꼴 — `2026-11-07T03:04:05.1234567Z`(소수 일곱 자리).
    fn now_utc(&self) -> String;

    /// 새 식별자. .NET `Guid.NewGuid().ToString("N")` 꼴 — 소문자 16진 32자.
    fn new_id(&self) -> String;
}

/// C# 이 SQL 의 `strftime('%Y-%m-%dT%H:%M:%f0000Z', 'now')` 로 짓던 시각을 [`Env::now_utc`] 로 짓는다.
///
/// SQLite 의 `%f` 는 `SS.SSS`(밀리초 셋째 자리)까지라 C# 은 뒤에 `0000` 을 붙여 일곱 자리를 맞췄다
/// (Schema.V20 · PclmFile.Snapshot). 그 꼴을 그대로 지키려고 **밀리초 뒤를 버리고 `0000` 을 붙인다** —
/// `2026-11-07T03:04:05.1234567Z` → `2026-11-07T03:04:05.1230000Z`. 그래서 이 자리의 값은 C# 과 같이
/// 마지막 네 자리가 늘 0 이다. 꼴이 다른 값이 오면 손대지 않고 그대로 쓴다.
pub(crate) fn sqlite_now(env: &dyn Env) -> String {
    let now = env.now_utc();
    let bytes = now.as_bytes();
    // "yyyy-MM-ddTHH:mm:ss.fffffffZ" 는 28 바이트다. 앞 23 바이트가 밀리초까지.
    if bytes.len() == 28 && bytes[19] == b'.' && bytes[27] == b'Z' && now.is_ascii() {
        format!("{}0000Z", &now[..23])
    } else {
        now
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed;
    impl Env for Fixed {
        fn now_utc(&self) -> String {
            "2026-11-07T03:04:05.1234567Z".into()
        }
        fn new_id(&self) -> String {
            "0".repeat(32)
        }
    }

    #[test]
    fn sqlite_now_keeps_milliseconds_and_pads() {
        assert_eq!(sqlite_now(&Fixed), "2026-11-07T03:04:05.1230000Z");
    }
}

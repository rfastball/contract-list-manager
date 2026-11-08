//! C# `HomeTests` 의 호스트 몫(쪽지 읽기·호스트의 묶기·출처·잠금)과 저장의 잠금 안 대상 확인을 옮긴 것. 시작은
//! `home_tests.rs`, 바꾸는 사이의 경합은 `switch_tests.rs` 에 있다.
//! 값은 모두 지어낸 것이다.

use std::path::{Path, PathBuf};

use std::time::Duration;

use pclm_core::erp::capture::{CaptureInput, ErpCapture};
use pclm_core::erp::mapping;
use pclm_core::storage::pclm_file;
use pclm_core::{Access, Database, PclmRole};
use serde_json::{Value, json};

use super::*;

struct Root(PathBuf);

impl Root {
    fn new() -> Root {
        let path = std::env::temp_dir().join(format!("pclm-home-{}", win::new_guid_n()));
        std::fs::create_dir_all(&path).unwrap();
        Root(path)
    }

    fn join(&self, part: &str) -> PathBuf {
        self.0.join(part)
    }
}

impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn work(path: &Path) -> String {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    pclm_file::create(path, PclmRole::WORK, &SystemEnv).unwrap();
    pclm_file::inspect(path).dataset_id.unwrap()
}

/// 창이 쓰는 쪽지와 같은 꼴(C# `Home.WriteConfig` — 들여 쓴 JSON). 호스트 쪽 크레이트에는 쓰는 길이 없어 시험이 짓는다.
fn write_config(home: &Home, workfile: &Path, dataset_id: &str) {
    std::fs::create_dir_all(home.directory()).unwrap();
    let text = serde_json::to_string_pretty(&json!({
        "version": 1, "workfile": workfile.to_string_lossy(), "datasetId": dataset_id,
    }))
    .unwrap();
    std::fs::write(home.config_path(), text).unwrap();
}

#[test]
fn 깨진_쪽지는_기본_자리로_물러나지_않고_아무것도_짓지_않는다() {
    let root = Root::new();
    let home = Home::new(root.join("홈")).unwrap();
    assert_eq!(home.read_config().state, ConfigState::Missing);
    assert!(home.open_bound().is_err());
    std::fs::create_dir_all(home.directory()).unwrap();
    for text in [
        "{ 깨진",
        r#"{ "version": 2, "workfile": "C:\\x.pclm", "datasetId": "ab" }"#,
        r#"{ "version": 1, "workfile": "상대\\x.pclm", "datasetId": "ab" }"#,
        r#"{ "version": 1, "datasetId": "ab" }"#,
        r#"{ "version": 1, "workfile": "   ", "datasetId": "ab" }"#,
        r#"{ "Version": 1, "workfile": "C:\\x.pclm" }"#,
        r#"{ "version": 1.0, "workfile": "C:\\x.pclm" }"#,
        r#"{ "version": "1", "workfile": "C:\\x.pclm" }"#,
        r#"{ "version": 1, "workfile": 5 }"#,
        r#"{ "version": 1, "workfile": "C:\\x.pclm", "retired": 3 }"#,
        "[]",
        "null",
        "",
    ] {
        std::fs::write(home.config_path(), text).unwrap();
        let read = home.read_config();
        assert_eq!(read.state, ConfigState::Corrupt, "{text}");
        assert!(read.config.is_none());
        assert_eq!(
            home.open_bound().unwrap_err().0,
            "메인 프로그램에서 저장 대상을 준비하세요."
        );
        assert_eq!(std::fs::read_to_string(home.config_path()).unwrap(), text);
    }
    assert_eq!(
        std::fs::read_dir(home.directory()).unwrap().count(),
        1,
        "쪽지 말고 아무것도 짓지 않는다"
    );
}

#[test]
fn 쪽지는_대소문자를_가린_칸만_읽고_겹치면_뒤의_것이다() {
    let root = Root::new();
    let home = Home::new(root.join("홈")).unwrap();
    std::fs::create_dir_all(home.directory()).unwrap();
    std::fs::write(
        home.config_path(),
        "\u{feff}{\"version\":2,\"version\":1,\"workfile\":\"C:/a\",\"workfile\":\"C:/b\",\"datasetId\":null,\"x\":{\"y\":[1]}}",
    )
    .unwrap();
    let read = home.read_config();
    assert_eq!(read.state, ConfigState::Ok);
    assert_eq!(
        read.config.unwrap(),
        HomeConfig {
            version: 1,
            workfile: Some("C:/b".into()),
            dataset_id: None,
            retired: None
        }
    );
}

#[test]
fn 호스트는_쪽지가_가리키는_지금_판의_작업자료만_연다() {
    let root = Root::new();
    let home = Home::new(root.join("홈")).unwrap();
    assert!(home.open_bound().is_err()); // 쪽지 없음

    let path = root.join("작업.pclm");
    let id = work(&path);
    write_config(&home, &path, &id);
    assert_eq!(home.open_bound().unwrap().path(), path);
    assert!(home.points_at(&path, &id));
    assert!(!home.points_at(&path, "deadbeef"));
    assert!(home.points_at(Path::new(&path.to_string_lossy().to_uppercase()), &id));

    write_config(&home, &path, "deadbeef");
    assert_eq!(
        home.open_bound().unwrap_err().0,
        "연결된 자료가 바뀌었습니다. 메인 프로그램을 다시 실행하세요."
    );

    let submission = root.join("제출.pclm");
    pclm_file::snapshot(
        &Database::new(&path, Access::Read),
        &submission,
        PclmRole::SUBMISSION,
        false,
        &SystemEnv,
    )
    .unwrap();
    let submission_id = pclm_file::inspect(&submission).dataset_id.unwrap();
    write_config(&home, &submission, &submission_id);
    assert!(home.open_bound().is_err());

    let missing = root.join("없음").join("계약자료.pclm");
    write_config(&home, &missing, &id);
    assert!(home.open_bound().is_err());
    assert!(!root.join("없음").exists());
}

#[test]
fn 호스트는_판이_낮은_자료를_올리지_않고_거절한다() {
    let root = Root::new();
    let home = Home::new(root.join("홈")).unwrap();
    let path = root.join("옛것.pclm");
    let id = work(&path);
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute_batch("PRAGMA user_version = 31;")
        .unwrap();
    write_config(&home, &path, &id);
    assert!(home.open_bound().is_err());
    assert_eq!(pclm_file::inspect(&path).version, 31);
}

#[test]
fn 호스트를_부른_확장은_홈에_적힌_출처와_같아야_한다() {
    let root = Root::new();
    let home = Home::new(root.join("홈")).unwrap();
    let origin = "chrome-extension://abcdefghijklmnopabcdefghijklmnop/";
    assert!(!home.is_allowed_origin(origin));

    std::fs::create_dir_all(home.directory()).unwrap();
    std::fs::write(home.extension_origin_path(), format!("{origin}\n")).unwrap();
    assert!(home.is_allowed_origin(origin));
    assert!(!home.is_allowed_origin("chrome-extension://bbcdefghijklmnopabcdefghijklmnop/"));
    assert!(!home.is_allowed_origin("chrome-extension://abcdefghijklmnopabcdefghijklmnoq/"));
    assert!(!home.is_allowed_origin("chrome-extension://abcdefghijklmnopabcdefghijklmnop"));
    assert!(
        !Home::new(root.join("다른 홈"))
            .unwrap()
            .is_allowed_origin(origin)
    );
}

#[test]
fn 홈은_한_번에_하나만_쥔다() {
    let root = Root::new();
    let home = Home::new(root.join("홈")).unwrap();
    let first = home.try_lock(Duration::ZERO).unwrap();
    assert!(first.is_some());
    assert!(home.try_lock(Duration::ZERO).unwrap().is_none());
    let upper = format!(
        "{}\\",
        dotnet::to_upper_invariant(&home.directory().to_string_lossy())
    );
    assert!(
        Home::new(upper)
            .unwrap()
            .try_lock(Duration::ZERO)
            .unwrap()
            .is_none()
    );
    {
        let other = Home::new(root.join("다른 홈")).unwrap();
        assert!(other.try_lock(Duration::ZERO).unwrap().is_some());
    }
    drop(first);
    assert!(home.try_lock(Duration::ZERO).unwrap().is_some());
}

#[test]
fn 잠금_이름은_폴더의_대문자_지문이다() {
    // C# 의 Convert.ToHexString(SHA256(UTF8("C:\TEMP\계약")), 0, 16) — 실측값.
    let home = Home::new(r"c:\temp\계약\").unwrap();
    assert_eq!(home.directory(), Path::new(r"c:\temp\계약"));
    let key = home
        .lock_name()
        .trim_start_matches("Local\\Pclm-")
        .to_string();
    assert_eq!(key.len(), 32);
    assert!(
        key.bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_lowercase())
    );
    assert_eq!(home.activation_name(), format!("Local\\Pclm.Show.{key}"));
    assert_eq!(
        Home::new(r"C:\TEMP\계약").unwrap().lock_name(),
        home.lock_name()
    );
}

#[test]
fn 자동_수집_설정은_홈의_파일에_통째로_쓴다() {
    let root = Root::new();
    let home = Home::new(root.join("홈")).unwrap();
    assert_eq!(auto_capture::read(&home), AutoCapturePreferences::default());
    let preferences = AutoCapturePreferences {
        enabled: true,
        name: "홍길동".into(),
    };
    auto_capture::write(&home, &preferences).unwrap();
    assert_eq!(auto_capture::read(&home), preferences);
    let names: Vec<_> = std::fs::read_dir(home.directory())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["auto-capture.json"]);
    let long = AutoCapturePreferences {
        enabled: true,
        name: "가".repeat(201),
    };
    assert!(auto_capture::write(&home, &long).is_err());
    assert_eq!(auto_capture::read(&home), preferences);
}

// ── 잠금 안의 대상 확인(ADR-032, C# WorkfileSwitchTests) ─────────────────

fn snapshot(base: &str) -> CaptureInput {
    let mut input = CaptureInput::new(
        "g2b-notice-a-v1",
        &mapping::revision(&mapping::defaults()),
        json!({
            "pointInfo": { "bidPbancNo": base, "bidPbancOrd": "001", "bidPbancNm": "지어낸 공고",
                "pbancInstUntyGrpNm": "시험기관" },
            "tables": {},
        }),
    );
    input.scope = Some("live".into());
    input.screen = Some("01179".into());
    input
}

/// 확장 호스트가 이 파일에 저장할 요청. 공고 하나, 충돌 없음.
fn capture_request(path: &Path, home: &Home, dataset_id: &str, base: &str) -> Value {
    let host = ErpCapture::new(path, &SystemEnv).with_destination(home);
    let input = snapshot(base);
    let preview = host.inspect(&input).unwrap();
    json!({
        "protocolVersion": 2, "requestId": "00000000-0000-0000-0000-0000000000aa", "method": "capture",
        "params": { "datasetId": dataset_id, "snapshot": input.to_value(),
            "captureId": "00000000-0000-0000-0000-0000000000bb", "baseToken": preview.base_token, "choices": {} },
    })
}

fn count(path: &Path, table: &str) -> i64 {
    rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .unwrap()
        .query_row(&format!("SELECT COUNT(*) FROM \"{table}\""), [], |r| {
            r.get(0)
        })
        .unwrap()
}

#[test]
fn 저장은_쪽지가_다른_파일을_가리키면_거절하고_쓰지_않는다() {
    let root = Root::new();
    let home = Home::new(root.join("홈")).unwrap();
    let current = root.join("작업.pclm");
    let id = work(&current);
    write_config(&home, &current, &id);
    let request = capture_request(&current, &home, &id, "R26BK00000002");

    let other = root.join("다른 작업자료.pclm");
    let other_id = work(&other);
    write_config(&home, &other, &other_id);
    let host = ErpCapture::new(&current, &SystemEnv).with_destination(&home);
    let reply = host.dispatch(&request, false);
    assert_eq!(reply["ok"], false);
    assert!(
        reply["error"]["message"]
            .as_str()
            .unwrap()
            .contains("저장 대상이 바뀌었습니다")
    );
    assert_eq!(count(&current, "erp_capture"), 0);

    // 쪽지가 이 파일로 돌아오면 같은 자료가 들어간다.
    write_config(&home, &current, &id);
    assert_eq!(host.dispatch(&request, false)["ok"], true);
    assert_eq!(count(&current, "erp_capture"), 1);
}

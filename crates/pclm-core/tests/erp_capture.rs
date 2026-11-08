//! C# `ErpCaptureTests` · `ConfirmedFieldTests`(수집·매핑 몫) · `FieldRegistryTests` — 수집 경계(`ErpCapture`)와 매핑
//! (`Mapping`·`MappingStore`)이 같은 미리보기·저장·판 계약을 지키는가. 값은 모두 지어낸 것이다(CLAUDE.md).
//!
//! 홈의 쪽지는 [`Destination`] 대역(`Note`)으로, 확장 폴더는 [`ExtensionHello`] 대역(`Ext`)으로 세운다. Core 밖의 시험은
//! 제 크레이트에 있다 — 내장 확장 풀기는 pclm-bridge(`bundled`), 확장 상태·네이티브 메시징 틀은 pclm-host.

mod common;

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use common::TestEnv;
use common::store_fixture::Fixture;
use pclm_core::erp::capture::{
    CaptureChange, CaptureInput, Choices, Destination, ErpCapture, ExtensionHello,
};
use pclm_core::erp::explicit_links;
use pclm_core::erp::mapping::{self, MappedDocument, MappingStore};
use pclm_core::storage::schema;
use pclm_core::storage::store::{CellChange, EntityRef, error_message};
use pclm_core::{Database, Error};
use serde_json::{Map, Value, json};

// ── 대역과 공통 ─────────────────────────────────────────────

/// 쪽지(C# `Home` 의 config.json) 대역 — 어느 파일·어느 작업자료를 가리키는가.
struct Note {
    target: Mutex<Option<(PathBuf, String)>>,
}

impl Note {
    fn pointing(path: &Path, dataset: &str) -> Self {
        Note {
            target: Mutex::new(Some((path.to_path_buf(), dataset.to_string()))),
        }
    }

    fn none() -> Self {
        Note {
            target: Mutex::new(None),
        }
    }

    fn write(&self, path: &Path, dataset: &str) {
        *self.target.lock().unwrap() = Some((path.to_path_buf(), dataset.to_string()));
    }
}

impl Destination for Note {
    fn points_at(&self, path: &Path, dataset_id: &str) -> bool {
        match &*self.target.lock().unwrap() {
            Some((p, d)) => p == path && d == dataset_id,
            None => false,
        }
    }
}

/// 확장 폴더(C# `ExtensionStatus`) 대역 — 디스크의 판과 브라우저별로 받은 연결.
struct Ext {
    version: Option<String>,
    contacts: Mutex<Vec<(String, String)>>,
}

impl ExtensionHello for Ext {
    fn disk_version(&self) -> Option<String> {
        self.version.clone()
    }

    fn record_contact(&self, version: &str, browser: &str) {
        self.contacts
            .lock()
            .unwrap()
            .push((browser.to_string(), version.to_string()));
    }
}

/// 시험 하나의 작업자료와 그 위의 수집 경계.
struct T {
    f: Fixture,
    dataset: String,
}

impl T {
    fn new(name: &str) -> Self {
        let f = Fixture::new(name);
        let dataset = f.scalar("SELECT dataset_id FROM pclm_file").unwrap();
        T { f, dataset }
    }

    /// 시험은 개발 호스트처럼 돈다 — 확장 판·연결 기록 같은 업무 호스트만의 일은 그 시험이 따로 세운다.
    fn host(&self) -> ErpCapture<'static> {
        ErpCapture::new(&self.f.path, self.f.env).with_environment("development")
    }

    /// `new ErpCapture(db)` — 업무 호스트(환경을 정하지 않는다).
    fn plain(&self) -> ErpCapture<'static> {
        ErpCapture::new(&self.f.path, self.f.env)
    }

    fn db(&self) -> Database {
        self.f.database()
    }

    fn id(&self) -> String {
        use pclm_core::Env;
        self.f.env.new_id()
    }

    fn call(&self, method: &str, args: Option<Value>) -> Value {
        self.call_on(&self.host(), method, args)
    }

    fn call_on(&self, host: &ErpCapture<'_>, method: &str, args: Option<Value>) -> Value {
        dispatch(host, &self.id(), method, args)
    }

    /// 미리보기를 뜨고 그 지문과 충돌마다의 고름(덮개는 `keep`, 나머지는 `apply`)을 실은 `capture` 요청의 인자.
    fn request(&self, snapshot: &CaptureInput, token: Option<&str>, id: Option<&str>) -> Value {
        let preview = self.host().inspect(snapshot).unwrap();
        let choices: Map<String, Value> = preview
            .changes
            .iter()
            .filter(|c| c.conflict)
            .map(|c| {
                let pick = if c.field == "__override" {
                    "keep"
                } else {
                    "apply"
                };
                (c.id.clone(), Value::String(pick.into()))
            })
            .collect();
        json!({
            "datasetId": self.dataset,
            "snapshot": snapshot.to_value(),
            "captureId": id.map_or_else(|| self.id(), str::to_string),
            "baseToken": token.unwrap_or(&preview.base_token),
            "choices": choices,
        })
    }

    fn n(&self, sql: &str) -> i64 {
        self.f.scalar(sql).unwrap().parse().unwrap()
    }

    /// 지문이 맞게 미리보기를 떠 곧장 저장한다(충돌은 고르지 않는다).
    fn save_plain(&self, host: &ErpCapture<'_>, input: &CaptureInput) {
        let preview = host.inspect(input).unwrap();
        host.save(
            input,
            Some(&preview.base_token),
            Some(&self.id()),
            &Vec::new(),
        )
        .unwrap();
    }
}

fn dispatch(host: &ErpCapture<'_>, request_id: &str, method: &str, args: Option<Value>) -> Value {
    host.dispatch(
        &json!({"protocolVersion": 2, "requestId": request_id, "method": method,
                "params": args.unwrap_or(Value::Null)}),
        false,
    )
}

fn ok(reply: &Value) -> bool {
    reply["ok"].as_bool().unwrap()
}

fn code(reply: &Value) -> &str {
    reply["error"]["code"].as_str().unwrap()
}

fn is_invalid<V>(result: &pclm_core::Result<V>) -> bool {
    matches!(result, Err(Error::InvalidOperation(_)))
}

fn message<V>(result: pclm_core::Result<V>) -> String {
    match result {
        Err(Error::InvalidOperation(m)) => m,
        Err(other) => panic!("InvalidOperation 이 아닌 오류: {}", error_message(&other)),
        Ok(_) => panic!("오류가 나야 한다"),
    }
}

fn defaults_revision() -> String {
    mapping::revision(&mapping::defaults())
}

/// 실제 화면 수집 입력(`"live"` 범위와 메뉴 번호).
fn live(profile: &str, revision: &str, data: Value, screen: &str) -> CaptureInput {
    let mut input = CaptureInput::new(profile, revision, data);
    input.scope = Some("live".into());
    input.screen = Some(screen.into());
    input
}

fn snapshot(title: &str) -> CaptureInput {
    live(
        "g2b-notice-a-v1",
        &defaults_revision(),
        json!({
            "pointInfo": {"bidPbancNo": "R26BK00000001", "bidPbancOrd": "001", "bidPbancNm": title,
                "pbancKndCd": "공440002", "scsbdMthdCd": "낙030001", "stdCtrtMthdCd": "계030003",
                "pbancInstUntyGrpNm": "시험기관"},
            "tables": {}
        }),
        "01179",
    )
}

/// 접수 목록형 화면(메뉴 01117)의 JSON — 받는 화면은 메뉴 경로로 가린다(ADR-037).
fn sample_request(revision: &str, title: &str) -> CaptureInput {
    CaptureInput::new(
        "g2b-request-v1",
        revision,
        json!({
            "pointInfo": {"ctrtDmndBizNm": title, "wrtYmd": "20260927", "totlPymtAmt": "0",
                "depth1": "01001", "depth2": "01117", "depth3": ""},
            "tables": {"mf_wfm_container_gridView": [
                {"ctrtDmndRcptNo": "RC001", "ctrtDmndRcptOrd": "000", "ctrtDmndRcptItemSqno": "001",
                 "ndfsPrcmDmndNo": "REQ1", "ctrtDmndQty": "1"},
                {"ctrtDmndRcptNo": "RC001", "ctrtDmndRcptOrd": "000", "ctrtDmndRcptItemSqno": "002",
                 "ndfsPrcmDmndNo": "REQ1", "ctrtDmndQty": "2"},
            ]}
        }),
    )
}

/// 확인한 충돌마다 같은 값을 고른다(C# `ToDictionary(c => c.Id, _ => pick)`).
fn pick_all(changes: &[CaptureChange], pick: &str) -> Choices {
    changes
        .iter()
        .filter(|c| c.conflict)
        .map(|c| (c.id.clone(), pick.to_string()))
        .collect()
}

/// `ExplicitLinks.Resolve(db)` — 한 트랜잭션에서 다시 짓는다.
fn resolve(c: &rusqlite::Connection) -> Vec<explicit_links::ReferenceState> {
    let tx = c.unchecked_transaction().unwrap();
    let result = explicit_links::resolve(&tx).unwrap();
    tx.commit().unwrap();
    result
}

/// `NoticeGroups.Kind`(pub(crate) 라 같은 글을 여기에 둔다).
fn kind_sql(kind: &str, status: &str) -> String {
    format!(
        "CASE WHEN NULLIF({kind}, '') IS NOT NULL AND {status} IN ('일반공고','변경공고','취소공고','재공고')\n  \
         AND {kind} NOT IN ('일반공고','변경공고','취소공고','재공고')\n  \
         AND {kind} NOT LIKE '%(일반공고)' AND {kind} NOT LIKE '%(변경공고)'\n  \
         AND {kind} NOT LIKE '%(취소공고)' AND {kind} NOT LIKE '%(재공고)'\n\
         THEN {kind} || '(' || {status} || ')' ELSE COALESCE(NULLIF({kind}, ''), {status}) END"
    )
}

/// `NoticeGroups.현행공고`(pub(crate) 라 같은 글을 여기에 둔다).
fn current_notice_sql() -> String {
    format!(
        "SELECT group_base, notice_base, seq FROM (
    SELECT 유효.*, COUNT(*) OVER (PARTITION BY group_base) AS 후보수,
           SUM(재공고) OVER (PARTITION BY group_base) AS 재공고수
    FROM (
        SELECT 계열.group_base, 현행.notice_base, 현행.seq,
               CASE WHEN COALESCE(
                   (SELECT o.value FROM field_override o
                    WHERE o.entity_type = 'notice' AND o.base = 현행.notice_base
                      AND o.seq = 현행.seq AND o.column_name = '공고종류'),
                   {})
                   IN ('재공고', '실공고(재공고)') THEN 1 ELSE 0 END AS 재공고
        FROM notice 현행
        JOIN notice_series 계열 ON 계열.notice_base = 현행.notice_base
        WHERE CAST(현행.seq AS INTEGER) =
              (SELECT MAX(CAST(x.seq AS INTEGER)) FROM notice x WHERE x.notice_base = 현행.notice_base)
          AND NOT EXISTS (
              SELECT 1 FROM notice_relation r
              WHERE r.related_base = 현행.notice_base AND r.related_seq = 현행.seq
                AND r.notice_base <> 현행.notice_base)
    ) 유효
) 후보
WHERE 후보수 = 1 OR (후보수 > 1 AND 재공고수 = 1 AND 재공고 = 1)",
        kind_sql("현행.notice_kind", "현행.source_status")
    )
}

/// 현행 공고의 본번호들.
fn current(t: &T) -> Vec<Option<String>> {
    t.f.column(&format!(
        "SELECT notice_base FROM ({})",
        current_notice_sql()
    ))
}

/// 값을 가진 칸(Dapper 의 동적 행) 하나의 글.
fn text_of(value: rusqlite::types::Value) -> String {
    match value {
        rusqlite::types::Value::Text(t) => t,
        other => format!("{other:?}"),
    }
}

// ── ErpCaptureTests ─────────────────────────────────────────

/// C# `ErpCaptureTests.매핑의_판은_어느_운영체제에서나_같다` — 판은 고정값이고, 저장하는 들여 쓴 글의 줄바꿈은 늘
/// `\r\n` 이다(`Mapping.HashText` 는 Core 밖이라 저장된 글로 본다).
#[test]
fn mapping_revision_is_the_same_on_every_os() {
    let t = T::new("erp-revision");
    assert_eq!(
        "BCBF796437854FE6E3EB9224297B14A8100CE17AD501FCA5B489790D48F912D8",
        defaults_revision()
    );
    let db = t.db();
    let mappings = MappingStore::new(&db, t.f.env);
    mappings
        .save(&serde_json::to_string(&mapping::defaults().to_value()).unwrap())
        .unwrap();
    let text = t.f.scalar("SELECT json FROM erp_mapping").unwrap();
    assert!(text.contains("\r\n"));
    let bytes = text.as_bytes();
    assert!(
        !bytes
            .iter()
            .enumerate()
            .any(|(i, b)| *b == b'\n' && (i == 0 || bytes[i - 1] != b'\r'))
    );
}

/// C# `ErpCaptureTests.업무_호스트의_인사는_디스크의_확장_판을_알려_주고_브라우저별_연결을_적는다` — **일부만 옮긴다.**
/// 확장이 보낸 값의 모양 거르기(`ExtensionStatus.RecordContact`)와 연결 기록 파일은 Core 밖이다. 여기서는 인사가
/// 디스크의 판을 알리고, 연결을 확장 쪽에 넘기며, 확장이 없는 호스트는 판도 연결도 모르는 것만 본다.
#[test]
fn business_host_hello_reports_extension_version_and_hands_over_contacts() {
    let t = T::new("erp-hello");
    let ext = Ext {
        version: Some("0.4.1".into()),
        contacts: Mutex::new(Vec::new()),
    };
    let host = t.plain().with_extension(&ext);

    // 옛 확장은 params 없이 인사한다. 그래도 성공이고 적을 것이 없다.
    let hello = t.call_on(&host, "hello", None);
    assert!(ok(&hello));
    assert_eq!(
        "0.4.1",
        hello["result"]["extensionVersion"].as_str().unwrap()
    );
    assert!(ext.contacts.lock().unwrap().is_empty());

    assert!(ok(&t.call_on(
        &host,
        "hello",
        Some(json!({"extensionVersion": "0.4.0", "browser": "Edge"}))
    )));
    assert!(ok(&t.call_on(
        &host,
        "hello",
        Some(json!({"extensionVersion": "0.4.1", "browser": "Chrome"}))
    )));
    assert_eq!(
        vec![
            ("Edge".to_string(), "0.4.0".to_string()),
            ("Chrome".to_string(), "0.4.1".to_string())
        ],
        *ext.contacts.lock().unwrap()
    );

    // 개발 호스트는 판도 연결도 모른다.
    let plain = t.call(
        "hello",
        Some(json!({"extensionVersion": "0.4.1", "browser": "Edge"})),
    );
    assert!(ok(&plain));
    assert!(plain["result"]["extensionVersion"].is_null());
}

/// C# `ErpCaptureTests.기본정보만_저장하고_재전송과_결과조회는_같은_영수증을_반환한다`
#[test]
fn saves_only_basics_and_resend_or_status_returns_the_same_receipt() {
    let t = T::new("erp-receipt");
    t.f.sql(
        "INSERT INTO notice_group VALUES ('R26BK00000001');
         INSERT INTO notice_series VALUES ('R26BK00000001', 'R26BK00000001');
         INSERT INTO notice(notice_base, seq, title, notice_officer, updated_at)
             VALUES ('R26BK00000001', '001', '옛 제목', '보존할 담당자', 'before');
         INSERT INTO notice_item(notice_base, seq, line_no, item_name) VALUES ('R26BK00000001', '001', 1, '보존할 품목');
         INSERT INTO field_override(entity_type, base, seq, column_name, value, original, updated_at)
             VALUES ('notice', 'R26BK00000001', '001', '공고명', '사용자 제목', '옛 제목', 'before');",
    );
    let id = t.id();
    let args = t.request(&snapshot("검증용 공고"), None, Some(&id));
    let saved = t.call("capture", Some(args.clone()));
    assert!(ok(&saved), "{saved}");
    let receipt = saved["result"].to_string();
    assert_eq!(receipt, t.call("capture", Some(args))["result"].to_string());
    assert_eq!(
        receipt,
        t.call(
            "captureStatus",
            Some(json!({"datasetId": t.dataset, "captureId": id}))
        )["result"]
            .to_string()
    );
    assert_eq!(
        "검증용 공고",
        t.f.scalar("SELECT title FROM notice").unwrap()
    );
    assert_eq!(
        "보존할 담당자",
        t.f.scalar("SELECT notice_officer FROM notice").unwrap()
    );
    assert_eq!(
        "보존할 품목",
        t.f.scalar("SELECT item_name FROM notice_item").unwrap()
    );
    assert_eq!(
        "사용자 제목",
        t.f.scalar("SELECT value FROM field_override").unwrap()
    );
    assert_eq!(1, t.n("SELECT count(*) FROM erp_capture"));
    let other = t.call(
        "capture",
        Some(t.request(&snapshot("다른 내용"), None, Some(&id))),
    );
    assert_eq!("invalid_request", code(&other));
    let timestamp = t.f.scalar("SELECT updated_at FROM notice").unwrap();
    let again = t.call(
        "capture",
        Some(t.request(&snapshot("검증용 공고"), None, None)),
    );
    assert!(!again["result"]["changed"].as_bool().unwrap());
    assert_eq!(
        timestamp,
        t.f.scalar("SELECT updated_at FROM notice").unwrap()
    );
}

/// C# `ErpCaptureTests.정정값은_수집이_그_아래_값을_바꿀_때만_묻고_복원은_그_차수의_덮개만_걷는다` — 사람이 고친 칸은 이번 수집이
/// 그 아래 값을 바꿀 때만 다시 묻는다. 복원은 그 차수의 덮개만 걷는다(ADR-020).
#[test]
fn override_asks_only_when_capture_changes_value_below_and_restore_removes_that_seq_only() {
    let t = T::new("erp-override");
    assert!(ok(&t.call(
        "capture",
        Some(t.request(&snapshot("검증용 공고"), None, None))
    )));
    t.f.sql(
        "INSERT INTO notice(notice_base, seq, title, updated_at) VALUES ('R26BK00000001', '002', '다음 차수 제목', 'before');
         INSERT INTO field_override(entity_type, base, seq, column_name, value, original, updated_at) VALUES
             ('notice', 'R26BK00000001', '001', '공고명', '사람이 고친 제목', '검증용 공고', 'before'),
             ('notice', 'R26BK00000001', '001', '공고기관', '사람이 고친 기관', '시험기관', 'before'),
             ('notice', 'R26BK00000001', '002', '공고명', '다음 차수 정정', '다음 차수 제목', 'before');",
    );
    let overrides = || t.n("SELECT count(*) FROM field_override");
    let host = t.host();
    // 002 가 서 있어 001 은 옛 차수다 — 최신 차수만 내는 v_공고 에는 없어도 읽혀야 한다.

    // 같은 화면을 다시 수집 — 덮개 아래 값이 그대로라 고를 것이 없고, 선택 없이 저장된다.
    let same = host.inspect(&snapshot("검증용 공고")).unwrap();
    assert!(!same.changes.iter().any(|ch| ch.field == "__override"));
    let inspected = t.call(
        "inspect",
        Some(json!({"datasetId": t.dataset, "snapshot": snapshot("검증용 공고").to_value()})),
    );
    assert!(
        inspected["result"]["changes"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    host.save(
        &snapshot("검증용 공고"),
        Some(&same.base_token),
        Some(&t.id()),
        &Vec::new(),
    )
    .unwrap();
    assert_eq!(3, overrides());
    assert_eq!(
        "사람이 고친 제목",
        t.f.scalar("SELECT 공고명 FROM v_공고차수 WHERE 입찰공고번호 = 'R26BK00000001-001'")
            .unwrap()
    );

    // 제목이 바뀌면 그 칸만 묻고, 새로 읽은 값을 보인다. 미리 보느라 쓴 것은 남지 않는다.
    let changed = host.inspect(&snapshot("바뀐 공고명")).unwrap();
    let asks: Vec<_> = changed
        .changes
        .iter()
        .filter(|ch| ch.field == "__override")
        .collect();
    assert_eq!(1, asks.len());
    let ask = asks[0];
    assert!(ask.conflict);
    assert_eq!(
        (
            Some("공고명".to_string()),
            Some("사람이 고친 제목".to_string()),
            Some("바뀐 공고명".to_string())
        ),
        (
            ask.override_column.clone(),
            ask.before.clone(),
            ask.after.clone()
        )
    );
    assert_eq!(
        "검증용 공고",
        t.f.scalar("SELECT title FROM notice WHERE seq = '001'")
            .unwrap()
    );
    assert_eq!(3, overrides());
    let plain: Choices = changed
        .changes
        .iter()
        .filter(|ch| ch.conflict && ch.field != "__override")
        .map(|ch| (ch.id.clone(), "apply".to_string()))
        .collect();
    assert!(is_invalid(&host.save(
        &snapshot("바뀐 공고명"),
        Some(&changed.base_token),
        Some(&t.id()),
        &plain
    )));

    // 복원은 이 차수의 이 칸 덮개만 걷는다.
    let mut with_ask = plain.clone();
    with_ask.push((ask.id.clone(), "apply".to_string()));
    host.save(
        &snapshot("바뀐 공고명"),
        Some(&changed.base_token),
        Some(&t.id()),
        &with_ask,
    )
    .unwrap();
    let remaining = t
        .f
        .column("SELECT seq || '/' || column_name FROM field_override ORDER BY seq, column_name");
    assert_eq!(
        vec![
            Some("001/공고기관".to_string()),
            Some("002/공고명".to_string())
        ],
        remaining
    );
    assert_eq!(
        "바뀐 공고명",
        t.f.scalar("SELECT 공고명 FROM v_공고차수 WHERE 입찰공고번호 = 'R26BK00000001-001'")
            .unwrap()
    );
}

/// C# `ErpCaptureTests.미리보기_뒤_수정된_자료나_다른_DB에는_저장하지_않는다`
#[test]
fn does_not_save_after_stale_preview_or_to_another_database() {
    let t = T::new("erp-stale");
    let args = t.request(&snapshot("검증용 공고"), None, None);
    assert!(ok(&t.call(
        "capture",
        Some(t.request(&snapshot("다른 수집"), None, None))
    )));
    assert_eq!("invalid_request", code(&t.call("capture", Some(args))));
    let other = t.call(
        "inspect",
        Some(json!({"datasetId": "other", "snapshot": snapshot("검증용 공고").to_value()})),
    );
    assert_eq!("invalid_request", code(&other));
}

/// C# `ErpCaptureTests.확장_호스트는_자료_자리를_보거나_옮기는_요청을_받지_않는다` — 확장은 자료 자리를 보지도 옮기지도 못한다
/// (ADR-032).
#[test]
fn extension_host_does_not_accept_requests_to_see_or_move_the_data_location() {
    let t = T::new("erp-location");
    let host = t.plain();
    let folder = t.f.temp.path("새 자리");
    for method in ["dataLocation", "stageDataMove"] {
        let reply = t.call_on(
            &host,
            method,
            Some(json!({"datasetId": t.dataset, "folder": folder.display().to_string()})),
        );
        assert!(!ok(&reply));
    }
    assert!(!folder.exists());
}

/// C# `ErpCaptureTests.쪽지가_다른_작업자료를_가리키면_저장을_거절한다` — 쓰기 잠금을 쥔 뒤 쪽지를 다시 읽는다.
#[test]
fn rejects_save_when_note_points_to_another_workfile() {
    let t = T::new("erp-note");
    let note = Note::none();
    let host = t.host().with_destination(&note);
    let request = t.request(&snapshot("검증용 공고"), None, None);

    note.write(&t.f.temp.path("다른.pclm"), "ffff");
    assert!(!ok(&t.call_on(&host, "capture", Some(request.clone()))));

    note.write(&t.f.path, &t.dataset);
    assert!(ok(&t.call_on(&host, "capture", Some(request))));
}

/// C# `ErpCaptureTests.전환_중에_줄_선_저장은_잠금이_풀린_뒤_새_쪽지를_보고_거절한다` — 잠금을 얻기 전에 읽은 쪽지로 판단하면 옛
/// 자료에 쓰고 성공을 알린다.
#[test]
fn save_queued_during_switch_sees_the_new_note_after_the_lock_is_released() {
    let t = T::new("erp-switch");
    let note = Note::pointing(&t.f.path, &t.dataset);
    let request = t.request(&snapshot("검증용 공고"), None, None);

    let holder = t.f.conn();
    holder.execute_batch("BEGIN IMMEDIATE").unwrap();
    let path = t.f.path.clone();
    let reply = std::thread::scope(|s| {
        let pending = s.spawn(|| {
            let env = TestEnv::new();
            let host = ErpCapture::new(&path, &env)
                .with_environment("development")
                .with_destination(&note);
            dispatch(
                &host,
                "00000000000000000000000000000001",
                "capture",
                Some(request.clone()),
            )
        });
        std::thread::sleep(Duration::from_millis(500));
        assert!(!pending.is_finished()); // 잠금 앞에 줄 서 있다

        note.write(&t.f.temp.path("새 자리.pclm"), &t.dataset);
        holder.execute_batch("COMMIT").unwrap();

        pending.join().unwrap()
    });
    assert!(!ok(&reply));

    assert_eq!(
        0,
        holder
            .query_row("SELECT count(*) FROM notice", [], |r| r.get::<_, i64>(0))
            .unwrap()
    );
    assert_eq!(
        0,
        holder
            .query_row("SELECT count(*) FROM erp_capture", [], |r| r
                .get::<_, i64>(0))
            .unwrap()
    );
}

/// C# `ErpCaptureTests.잠금을_쥔_사이_옮겨진_옛_파일이_되면_줄_선_저장을_거절한다` — 옮긴 뒤의 옛 파일(retired)에는 쪽지를 모르는
/// 쓰기도 잠금 안에서 막힌다.
#[test]
fn save_queued_while_file_becomes_retired_is_rejected() {
    let t = T::new("erp-retired");
    let request = t.request(&snapshot("검증용 공고"), None, None);

    let holder = t.f.conn();
    holder.execute_batch("BEGIN IMMEDIATE").unwrap();
    let path = t.f.path.clone();
    let reply = std::thread::scope(|s| {
        let pending = s.spawn(|| {
            let env = TestEnv::new();
            let host = ErpCapture::new(&path, &env).with_environment("development");
            dispatch(
                &host,
                "00000000000000000000000000000002",
                "capture",
                Some(request.clone()),
            )
        });
        std::thread::sleep(Duration::from_millis(500));

        holder
            .execute_batch("UPDATE pclm_file SET role = 'retired';")
            .unwrap();
        holder.execute_batch("COMMIT").unwrap();

        pending.join().unwrap()
    });
    assert!(!ok(&reply));

    assert_eq!(
        0,
        holder
            .query_row("SELECT count(*) FROM notice", [], |r| r.get::<_, i64>(0))
            .unwrap()
    );
}

/// C# `ErpCaptureTests.영수증_기록에_실패하면_업무자료도_롤백한다`
#[test]
fn failing_to_record_the_receipt_rolls_back_the_business_data() {
    let t = T::new("erp-rollback");
    t.f.sql("CREATE TRIGGER reject_receipt BEFORE INSERT ON erp_capture BEGIN SELECT RAISE(ABORT, 'test'); END;");
    assert!(!ok(&t.call(
        "capture",
        Some(t.request(&snapshot("검증용 공고"), None, None))
    )));
    assert_eq!(0, t.n("SELECT count(*) FROM notice"));
    assert_eq!(0, t.n("SELECT count(*) FROM notice_series"));
    assert_eq!(0, t.n("SELECT count(*) FROM erp_capture"));
}

/// C# `ErpCaptureTests.지원하지_않거나_불완전한_화면은_저장을_거부한다`
#[test]
fn refuses_unsupported_or_incomplete_screens() {
    let t = T::new("erp-unsupported");
    let mut unknown = snapshot("검증용 공고");
    unknown.profile = Some("unknown".into());
    let mut old = snapshot("검증용 공고");
    old.mapping_revision = Some("old".into());
    let mut file = snapshot("검증용 공고");
    file.scope = Some("file".into());
    for input in [unknown, old, file] {
        let reply = t.call(
            "inspect",
            Some(json!({"datasetId": t.dataset, "snapshot": input.to_value()})),
        );
        assert!(!ok(&reply));
    }
}

/// C# `ErpCaptureTests.호스트는_DB를_만들거나_판올림하거나_운영DB로_우회하지_않는다`
#[test]
fn host_does_not_create_migrate_or_detour_to_a_production_database() {
    let t = T::new("erp-no-create");
    let absent = t.f.temp.path("absent").join("pclm.db");
    let host = ErpCapture::new(&absent, t.f.env);
    assert_eq!("invalid_request", code(&t.call_on(&host, "hello", None)));
    assert!(!absent.parent().unwrap().exists());
    // 작업자료가 아닌 파일에는 쓰지 않는다 — 그 자리에 제출본이 놓였어도 그렇다.
    t.f.sql("UPDATE pclm_file SET role = 'submission';");
    assert_eq!("invalid_request", code(&t.call("hello", None)));
    t.f.sql("UPDATE pclm_file SET role = 'work';");
    t.f.sql("PRAGMA user_version = 15;");
    assert_eq!("invalid_request", code(&t.call("hello", None)));
    assert_eq!(15, t.n("PRAGMA user_version"));
}

/// C# `ErpCaptureTests.계약방법은_코드로_읽는_화면과_이름으로_읽는_화면이_같은_이름을_담는다` — 계약 상세(01579)는 코드,
/// 변경계약서 작성(01608)은 이름으로 온다. 코드표에 없으면 같은 차수를 두 화면에서 읽을 때 변경이 검토에 뜬다.
#[test]
fn contract_method_code_and_name_screens_hold_the_same_name() {
    let set = mapping::defaults();
    for p in set.profiles_iter() {
        let mut found = p.fields_iter().filter(|f| f.target() == "contract_method");
        let Some(f) = found.next() else { continue };
        if f.transform.as_deref() != Some("code") {
            continue;
        }
        assert_eq!(
            Some("수의계약".to_string()),
            mapping::convert_value(Some("계030004"), f).unwrap()
        );
        assert_eq!(
            Some("일반경쟁".to_string()),
            mapping::convert_value(Some("계030001"), f).unwrap()
        );
        assert_eq!(
            Some("제한경쟁".to_string()),
            mapping::convert_value(Some("계030003"), f).unwrap()
        );
    }
}

/// C# `ErpCaptureTests.머리가_T_인_시험용_번호도_받는다`(Theory) — 머리글자는 R·T 둘뿐이다.
fn accepts_t_headed_test_numbers(
    profile: &str,
    base_key: &str,
    base_value: &str,
    seq: Option<(&str, &str)>,
) {
    let mut point = Map::new();
    point.insert(base_key.into(), Value::String(base_value.into()));
    if let Some((k, v)) = seq {
        point.insert(k.into(), Value::String(v.into()));
    }
    let set = mapping::defaults();
    let input = json!({"pointInfo": point.clone(), "tables": {}});
    let doc = mapping::map(&set, profile, &input, "file").unwrap();
    assert!(doc.base.starts_with("T26"));
    // 머리글자는 R·T 둘뿐이다.
    point.insert(
        base_key.into(),
        Value::String(format!("X{}", &base_value[1..])),
    );
    let bad = json!({"pointInfo": point, "tables": {}});
    assert!(is_invalid(&mapping::map(&set, profile, &bad, "file")));
}

/// C# `ErpCaptureTests.머리가_T_인_시험용_번호도_받는다("g2b-notice-a-v1", …)`
#[test]
fn t_headed_number_notice_a() {
    accepts_t_headed_test_numbers(
        "g2b-notice-a-v1",
        "bidPbancNo",
        "T26BK00000001",
        Some(("bidPbancOrd", "000")),
    );
}

/// C# `ErpCaptureTests.머리가_T_인_시험용_번호도_받는다("g2b-notice-b-v1", …)`
#[test]
fn t_headed_number_notice_b() {
    accepts_t_headed_test_numbers("g2b-notice-b-v1", "bidPbancText", "T26BK00000001-000", None);
}

/// C# `ErpCaptureTests.머리가_T_인_시험용_번호도_받는다("g2b-contract-v1", …)`
#[test]
fn t_headed_number_contract() {
    accepts_t_headed_test_numbers("g2b-contract-v1", "ctrtNoOrd", "T26TA00000001-00", None);
}

/// C# `ErpCaptureTests.JSON_변경은_검토하고_미수집과_정정값은_보존한다`
#[test]
fn json_changes_are_reviewed_and_uncollected_and_corrected_values_are_kept() {
    let t = T::new("erp-json");
    let host = t.plain();
    let set = mapping::defaults();
    let revision = mapping::revision(&set);
    let input = sample_request(&revision, "첫 제목");
    let preview = host.inspect(&input).unwrap();
    assert_eq!(2, preview.item_count);
    let id = t.id();
    let stored = host
        .save(&input, Some(&preview.base_token), Some(&id), &Vec::new())
        .unwrap()
        .to_value()
        .to_string();
    assert_eq!(
        stored,
        host.save(&input, Some(&preview.base_token), Some(&id), &Vec::new())
            .unwrap()
            .to_value()
            .to_string()
    );
    t.f.sql("UPDATE request SET remarks='수집에 없는 값' WHERE request_base='RC001'");
    let changed = sample_request(&revision, "다른 제목");
    let next = host.inspect(&changed).unwrap();
    assert!(is_invalid(&host.save(
        &changed,
        Some(&next.base_token),
        Some(&t.id()),
        &Vec::new()
    )));
    let choices = pick_all(&next.changes, "keep");
    host.save(&changed, Some(&next.base_token), Some(&t.id()), &choices)
        .unwrap();
    assert_eq!("첫 제목", t.f.scalar("SELECT title FROM request").unwrap());
    assert_eq!(
        "수집에 없는 값",
        t.f.scalar("SELECT remarks FROM request").unwrap()
    );
    assert_eq!(2, t.n("SELECT count(*) FROM request_item"));
    t.f.sql("UPDATE request SET title='동시 편집'");
    assert!(is_invalid(&host.save(
        &changed,
        Some(&next.base_token),
        Some(&t.id()),
        &choices
    )));
}

/// C# `ErpCaptureTests.로컬_JSON_코퍼스는_네_화면형을_변환한다` — `PCLM_JSON_CORPUS` 가 가리키는 개인 코퍼스(zip)의 JSON 을 모두
/// 옮겨 본다. 개인 코퍼스는 저장소/CI 로 복사하지 않으므로 그 변수가 없으면 아무것도 하지 않는다. 화면 번호가 있으면 그 화면의
/// 프로필이다(ADR-037) — 키로 짐작하면 변경계약서 작성(01608)이 공고로 간다. 저장까지 가는 것은 허용 목록의 화면뿐이고,
/// 저장한 것을 다시 미리 보면 바뀔 것이 없다.
#[test]
fn local_json_corpus_maps_every_screen_kind() {
    let Some(path) = std::env::var_os("PCLM_JSON_CORPUS").filter(|p| !p.is_empty()) else {
        return;
    };
    let mut zip = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
    let set = mapping::defaults();
    let revision = defaults_revision();
    let t = T::new("erp-corpus");
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).unwrap();
        let name = entry.name().unwrap().into_owned();
        if !name.ends_with(".json") {
            continue;
        }
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut entry, &mut bytes).unwrap();
        let document: Value =
            serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        let point = &document["pointInfo"];
        let screen = pclm_core::erp::screens::profile_for(
            &set,
            pclm_core::erp::screens::leaf(point).as_deref(),
        )
        .map(|p| p.id().to_string());
        let has = |key: &str| point.get(key).is_some();
        let profile = screen.clone().unwrap_or_else(|| {
            if has("ctrtNoOrd") {
                "g2b-contract-v1"
            } else if has("ctrtDmndBizNm") {
                if document["tables"]
                    .get("mf_wfm_container_gvDmndItem")
                    .is_some()
                {
                    "g2b-request-b-v1"
                } else {
                    "g2b-request-v1"
                }
            } else if has("bidPbancText") {
                "g2b-notice-b-v1"
            } else {
                "g2b-notice-a-v1"
            }
            .to_string()
        });
        let mapped = mapping::map(&set, &profile, &document, "file")
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(!mapped.rows.is_empty(), "{name}");
        if screen.as_deref() != Some(profile.as_str()) {
            continue;
        }
        let host = t.plain();
        let input = CaptureInput::new(&profile, &revision, document.clone());
        let preview = host
            .inspect(&input)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let choices = pick_all(&preview.changes, "apply");
        host.save(&input, Some(&preview.base_token), Some(&t.id()), &choices)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let again = host
            .inspect(&input)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(again.changes.is_empty(), "{name}");
    }
}

/// C# `ErpCaptureTests.계약_품목은_모두_같은_계약과_변경차수에_속해야_한다`(Theory)
fn contract_items_must_belong_to_the_same_contract(
    profile: &str,
    other_base: &str,
    other_seq: &str,
) {
    let set = mapping::defaults();
    let p = set.profile(profile).unwrap();
    let table = p
        .tables_iter()
        .find(|t| t.target() == "contract_item")
        .unwrap();
    let mut tables = Map::new();
    tables.insert(
        table.source().to_string(),
        json!([
            {"ctrtNo": "R26TA00000001", "ctrtChgOrd": "00", "ctrtItemSqno": 1, "ctrtAmt": 1000},
            {"ctrtNo": other_base, "ctrtChgOrd": other_seq, "ctrtItemSqno": 2, "ctrtAmt": 2000},
        ]),
    );
    let data = json!({"pointInfo": {"ctrtNoOrd": "R26TA00000001-00"}, "tables": tables});
    assert_eq!(
        "서로 다른 계약이나 변경차수의 품목을 한 문서로 저장할 수 없습니다.",
        message(mapping::map(&set, profile, &data, "file"))
    );
}

/// C# `ErpCaptureTests.계약_품목은_모두_같은_계약과_변경차수에_속해야_한다("g2b-contract-v1", 다른 계약)`
#[test]
fn contract_items_same_contract_a() {
    contract_items_must_belong_to_the_same_contract("g2b-contract-v1", "R26TA00000002", "00");
}

/// C# `ErpCaptureTests.계약_품목은_모두_같은_계약과_변경차수에_속해야_한다("g2b-contract-v1", 다른 차수)`
#[test]
fn contract_items_same_contract_b() {
    contract_items_must_belong_to_the_same_contract("g2b-contract-v1", "R26TA00000001", "01");
}

/// C# `ErpCaptureTests.계약_품목은_모두_같은_계약과_변경차수에_속해야_한다("g2b-contract-change-v1", 다른 계약)`
#[test]
fn contract_items_same_contract_c() {
    contract_items_must_belong_to_the_same_contract(
        "g2b-contract-change-v1",
        "R26TA00000002",
        "00",
    );
}

/// C# `ErpCaptureTests.계약_품목은_모두_같은_계약과_변경차수에_속해야_한다("g2b-contract-change-v1", 다른 차수)`
#[test]
fn contract_items_same_contract_d() {
    contract_items_must_belong_to_the_same_contract(
        "g2b-contract-change-v1",
        "R26TA00000001",
        "01",
    );
}

/// C# `ErpCaptureTests.매핑은_검증후_활성화하고_중복키와_민감필드를_거절한다`
#[test]
fn mapping_is_activated_after_validation_and_rejects_duplicate_keys_and_sensitive_fields() {
    let t = T::new("erp-mapping");
    let db = t.db();
    let mappings = MappingStore::new(&db, t.f.env);
    let set = mapping::defaults();
    let json_text = serde_json::to_string(&set.to_value()).unwrap();
    let revision = mappings.save(&json_text).unwrap();
    assert!(is_invalid(&mappings.activate(&revision)));
    let sample = sample_request(&mapping::revision(&set), "시험");
    mappings
        .validate_sample(&json_text, sample.profile(), sample.data.as_ref().unwrap())
        .unwrap();
    mappings.activate(&revision).unwrap();
    assert_eq!(revision, mapping::revision(&mappings.active().unwrap()));
    assert!(is_invalid(&mapping::parse(
        &json_text.replace("ctrtDmndBizNm", "giveActno")
    )));
    let raw = serde_json::to_string(sample.data.as_ref().unwrap())
        .unwrap()
        .replace("\"002\"", "\"001\"");
    let duplicate: Value = serde_json::from_str(&raw).unwrap();
    assert!(is_invalid(&mapping::map(
        &set,
        sample.profile(),
        &duplicate,
        "file"
    )));
    // 실제 화면 수집도 건수 선택자 없이 번호만 맞으면 검토로 넘어간다.
    let mut live_sample = sample.clone();
    live_sample.scope = Some("live".into());
    live_sample.screen = Some("01117".into());
    assert_eq!(2, t.plain().inspect(&live_sample).unwrap().item_count);
}

/// C# `ErpCaptureTests.번호만_있으면_받고_걷지_않은_제목과_표는_건드리지_않는다`
#[test]
fn number_alone_is_accepted_and_uncollected_title_and_tables_are_left_alone() {
    let t = T::new("erp-number-only");
    let host = t.plain();
    let revision = defaults_revision();
    let contract = |tables: Value, title: Option<&str>| {
        // 확장은 읽지 못한 칸의 키를 싣지 않는다.
        let mut point = Map::new();
        point.insert("ctrtNoOrd".into(), json!("R26TA00000009-00"));
        if let Some(title) = title {
            point.insert("ctrtNm".into(), json!(title));
        }
        live(
            "g2b-contract-v1",
            &revision,
            json!({"pointInfo": point, "tables": tables}),
            "01579",
        )
    };
    let grid = "mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis";
    let full = contract(
        json!({grid: [
            {"ctrtNo": "R26TA00000009", "ctrtChgOrd": "00", "ctrtItemSqno": 1, "ctrtAmt": 1000}]}),
        Some("시험 계약"),
    );
    t.save_plain(&host, &full);
    // 품목 탭을 열지 않은 화면·제목 칸이 없는 화면: 번호만으로 받되 있던 품목·제목·총액은 그대로다.
    let bare = contract(json!({grid: []}), None);
    let preview = host.inspect(&bare).unwrap();
    assert_eq!(0, preview.item_count);
    assert!(preview.changes.is_empty());
    assert!(
        host.inspect(&contract(json!({}), None))
            .unwrap()
            .changes
            .is_empty()
    );
}

/// C# `ErpCaptureTests.접수_상세화면은_품목표_이름이_달라도_같은_접수로_변환한다` — 접수는 화면이 둘이다: 목록형은 gridView,
/// 상세형은 gvDmndItem 에 품목을 싣는다.
#[test]
fn request_detail_screen_maps_to_the_same_request_despite_different_item_table_name() {
    let set = mapping::defaults();
    let input = json!({
        "pointInfo": {"ctrtDmndBizNm": "시험 구매", "wrtYmd": "20260927", "ctrtDmndRcptNo": "RC009",
            "ctrtDmndRcptOrd": "000", "totlPymtAmt": "1000"},
        "tables": {"mf_wfm_container_gvDmndItem": [
            {"ctrtDmndRcptNo": "RC009", "ctrtDmndRcptOrd": "000", "ctrtDmndRcptItemSqno": 1,
             "dtlsPrnm": "시험품", "ctrtDmndQty": 1, "ctrtDmndUprc": 1000, "qtyUntNm": "SET"}]}
    });
    let doc = mapping::map(&set, "g2b-request-b-v1", &input, "file").unwrap();
    assert_eq!(("RC009", "000"), (doc.base.as_str(), doc.seq.as_str()));
    let items: Vec<_> = doc
        .rows
        .iter()
        .filter(|r| r.table == "request_item")
        .collect();
    assert_eq!(1, items.len());
    assert_eq!(Some("1000"), items[0].value("unit_price"));
    assert!(is_invalid(&mapping::map(
        &set,
        "g2b-request-v1",
        &input,
        "file"
    )));
}

/// C# `ErpCaptureTests.명시접수로_묶고_유일한_재공고와_정정에_따라_현행과_품목연결을_계산한다`
#[test]
fn groups_by_explicit_request_and_computes_current_notice_and_item_links() {
    let t = T::new("erp-explicit");
    let host = t.plain();
    let set = mapping::defaults();
    let request = sample_request(&mapping::revision(&set), "같은 이름");
    t.save_plain(&host, &request);
    let c = t.f.conn();
    assert_eq!(
        "RC001",
        t.f.store().resolve("RC001-000").unwrap().unwrap().base
    );
    for i in 1..=2 {
        let number = format!("R26BK0000000{i}");
        t.f.notice(&number, "000", &[("title", "같은 이름")], &[], &[]);
        c.execute(
            "INSERT INTO notice_item(notice_base,seq,line_no,ref_request_base,ref_request_seq,ref_request_item) VALUES(?1,'000',1,'RC001','000','001')",
            [&number],
        )
        .unwrap();
    }
    resolve(&c);
    assert_eq!(
        1,
        t.n("SELECT count(DISTINCT group_base) FROM notice_series")
    );
    assert_eq!(1, t.n("SELECT count(*) FROM request_link"));
    assert!(current(&t).is_empty());
    // 차수가 다른 공고 품목 둘이면 고르지 않는다.
    assert_eq!(0, t.n("SELECT count(*) FROM request_item_link"));
    assert_eq!(
        2,
        t.n("SELECT count(*) FROM request_item WHERE request_number='REQ1'")
    );

    // 종류와 상태를 따로 걷어도 기존 종류에 낡은 상태가 붙어 남지 않는다.
    let revision = mapping::revision(&set);
    let initial = CaptureInput::new(
        "g2b-notice-a-v1",
        &revision,
        json!({"pointInfo": {"depth3": "01179", "bidPbancNo": "R26BK00000002", "bidPbancOrd": "000",
            "pbancKndCd": "공440002", "pbancSttsCd": "공400001"}, "tables": {}}),
    );
    t.save_plain(&host, &initial);
    // 원공고번호 없이 상태만 재공고로 바뀐 부분 입력.
    let renotice = CaptureInput::new(
        "g2b-notice-a-v1",
        &revision,
        json!({"pointInfo": {"depth3": "01179", "bidPbancNo": "R26BK00000002", "bidPbancOrd": "000",
            "pbancSttsCd": "공400004"}, "tables": {}}),
    );
    let changed = host.inspect(&renotice).unwrap();
    host.save(
        &renotice,
        Some(&changed.base_token),
        Some(&t.id()),
        &pick_all(&changed.changes, "apply"),
    )
    .unwrap();
    assert_eq!(
        "실공고",
        t.f.scalar("SELECT notice_kind FROM notice WHERE notice_base='R26BK00000002'")
            .unwrap()
    );
    let kind_of = || {
        t.f.scalar("SELECT 공고종류 FROM v_공고 WHERE 공고본번호='R26BK00000002'")
            .unwrap()
    };
    assert_eq!("실공고(재공고)", kind_of());
    let mut kind_only = initial.clone();
    kind_only.data = Some(
        json!({"pointInfo": {"depth3": "01179", "bidPbancNo": "R26BK00000002", "bidPbancOrd": "000",
            "pbancKndCd": "공440002"}, "tables": {}}),
    );
    t.save_plain(&host, &kind_only);
    assert_eq!("실공고(재공고)", kind_of());
    let current_is = |expected: Option<&str>| {
        let rows = current(&t);
        assert!(rows.len() <= 1);
        assert_eq!(
            expected.map(str::to_string),
            rows.into_iter().next().flatten()
        );
    };
    let item_links = || t.n("SELECT count(*) FROM request_item_link");
    current_is(Some("R26BK00000002"));
    assert_eq!(1, item_links());
    assert_eq!(0, t.n("SELECT count(*) FROM notice_relation"));

    // v30 에서 미정으로 남은 품목 연결도 다시 수집하지 않고 판올림만으로 복구한다.
    t.f.sql("DELETE FROM request_item_link; PRAGMA user_version=30;");
    t.db().migrate(t.f.env).unwrap();
    assert_eq!(i64::from(schema::VERSION), t.n("PRAGMA user_version"));
    assert_eq!(1, item_links());

    let store = t.f.store();
    let entity = EntityRef::new("notice", "R26BK00000002", "000");
    store.set_override(&entity, "공고종류", "").unwrap(); // 빈 정정도 source_status 로 되살리지 않는다.
    current_is(None);
    assert_eq!(0, item_links());
    store.clear_override(&entity, "공고종류").unwrap();
    current_is(Some("R26BK00000002"));
    assert_eq!(1, item_links());

    let edit = |value: &str, original: &str, operation: &str| CellChange {
        row_key: Some(entity.display()),
        field: Some("공고종류".into()),
        value: Some(value.into()),
        original: Some(original.into()),
        operation: Some(operation.into()),
    };
    store
        .batch_edit("v_공고", &[edit("일반공고", "실공고(재공고)", "set")])
        .unwrap();
    current_is(None);
    assert_eq!(0, item_links());
    store
        .batch_edit("v_공고", &[edit("", "일반공고", "revert")])
        .unwrap();
    current_is(Some("R26BK00000002"));
    assert_eq!(1, item_links());

    let original = EntityRef::new("notice", "R26BK00000001", "000");
    store.set_override(&original, "공고종류", "재공고").unwrap();
    current_is(None); // 재공고가 여럿이면 임의로 고르지 않는다.
    assert_eq!(0, item_links());
    store.clear_override(&original, "공고종류").unwrap();
    current_is(Some("R26BK00000002"));
    assert_eq!(1, item_links());
}

/// C# `ErpCaptureTests.명시참조_변경과_삭제는_문서와_품목_연결을_재계산한다`
#[test]
fn explicit_reference_changes_and_deletes_recompute_document_and_item_links() {
    let t = T::new("erp-refs");
    let c = t.f.conn();
    c.execute_batch(
        "INSERT INTO request_series(request_base) VALUES('TESTR1'),('TESTR2');
         INSERT INTO request(request_base,seq,title,updated_at) VALUES('TESTR1','000','같은 이름','2026-01-01'),('TESTR2','000','같은 이름','2026-01-01');
         INSERT INTO request_item(request_base,seq,line_no,ref_request_base,ref_request_seq,ref_request_item)
             VALUES('TESTR1','000',1,'TESTR1','000','001'),('TESTR2','000',1,'TESTR2','000','001');
         INSERT INTO notice_group(group_base) VALUES('TESTN1'),('TESTN2');
         INSERT INTO notice_series(notice_base,group_base) VALUES('TESTN1','TESTN1'),('TESTN2','TESTN2');
         INSERT INTO notice(notice_base,seq,title,updated_at) VALUES('TESTN1','000','같은 이름','2026-01-01'),('TESTN2','000','같은 이름','2026-01-01');
         INSERT INTO notice_item(notice_base,seq,line_no) VALUES('TESTN1','000',1),('TESTN2','000',1);
         INSERT INTO contract_series(contract_base) VALUES('TESTC');
         INSERT INTO contract(contract_base,seq,title,updated_at) VALUES('TESTC','00','같은 이름','2026-01-01');",
    )
    .unwrap();
    resolve(&c);
    let links =
        "SELECT count(*) FROM (SELECT * FROM project_link UNION ALL SELECT * FROM request_link)";
    assert_eq!(0, t.n(links));

    c.execute_batch(
        "UPDATE notice_item SET ref_request_base='TESTR1',ref_request_seq='000',ref_request_item='001' WHERE notice_base='TESTN1';
         UPDATE contract SET ref_notice_base='TESTN1';",
    )
    .unwrap();
    resolve(&c);
    assert_eq!(
        "TESTR1",
        t.f.scalar("SELECT request_base FROM request_link").unwrap()
    );
    assert_eq!(
        "TESTR1",
        t.f.scalar("SELECT request_base FROM request_item_link")
            .unwrap()
    );
    assert_eq!(
        "TESTN1",
        t.f.scalar("SELECT notice_group FROM project_link").unwrap()
    );

    c.execute_batch(
        "INSERT INTO notice(notice_base,seq,title,updated_at) VALUES('TESTN1','001','변경공고','2026-01-02');
         INSERT INTO notice_item(notice_base,seq,line_no,ref_request_base,ref_request_seq,ref_request_item)
             VALUES('TESTN1','001',1,'TESTR1','000','001');",
    )
    .unwrap();
    resolve(&c);
    assert_eq!(
        "001",
        t.f.scalar("SELECT notice_seq FROM request_item_link")
            .unwrap()
    );

    // 서로 모순되는 명시 참조는 어느 쪽도 임의로 고르지 않는다.
    c.execute_batch("UPDATE contract SET ref_request_base='TESTR2';")
        .unwrap();
    let states = resolve(&c);
    assert!(
        states
            .iter()
            .any(|r| r.base == "TESTC" && r.state == "참조 충돌")
    );
    assert_eq!(0, t.n("SELECT count(*) FROM project_link"));
    c.execute_batch("UPDATE contract SET ref_request_base=NULL;")
        .unwrap();

    c.execute_batch(
        "UPDATE notice_item SET ref_request_base='TESTR2' WHERE notice_base='TESTN1';
         INSERT INTO contract(contract_base,seq,title,ref_notice_base,updated_at) VALUES('TESTC','01','같은 이름','TESTN2','2026-01-02');",
    )
    .unwrap();
    resolve(&c);
    resolve(&c); // 같은 자료로 다시 계산해도 중복이나 옛 연결이 남지 않는다.
    assert_eq!(
        "TESTR2",
        t.f.scalar("SELECT request_base FROM request_link").unwrap()
    );
    assert_eq!(
        "TESTR2",
        t.f.scalar("SELECT request_base FROM request_item_link")
            .unwrap()
    );
    assert_eq!(
        "TESTN2",
        t.f.scalar("SELECT notice_group FROM project_link").unwrap()
    );

    t.f.store()
        .delete(&EntityRef::new("contract", "TESTC", "01"), false)
        .unwrap();
    assert_eq!(
        "TESTN1",
        t.f.scalar("SELECT notice_group FROM project_link").unwrap()
    );
    c.execute_batch(
        "UPDATE notice_item SET ref_request_base=NULL; UPDATE contract SET ref_notice_base=NULL;",
    )
    .unwrap();
    resolve(&c);
    assert_eq!(0, t.n(links));
    assert_eq!(0, t.n("SELECT count(*) FROM request_item_link"));
}

/// C# `ErpCaptureTests.공고종류_판올림은_원천과_당시매핑이_입증한_합성값만_복구하고_정정을_보존한다`
#[test]
fn notice_kind_migration_restores_only_proven_synthetic_values_and_keeps_corrections() {
    let t = T::new("erp-kind-migration");
    let host = t.plain();
    let set = mapping::defaults();
    let revision = mapping::revision(&set);
    let c = t.f.conn();
    for i in 1..=6 {
        let b = format!("R26BK0000000{i}");
        let input = CaptureInput::new(
            "g2b-notice-a-v1",
            &revision,
            json!({"pointInfo": {"depth3": "01179", "bidPbancNo": b, "bidPbancOrd": "000",
                "pbancKndCd": "공440002", "pbancSttsCd": "공400001"}, "tables": {}}),
        );
        t.save_plain(&host, &input);
        // v31의 종류는 일반공고로 남고 상태만 재공고로 바뀐 모습.
        let text: String = c
            .query_row(
                "SELECT document_json FROM erp_source WHERE entity_base=?1",
                [&b],
                |r| r.get(0),
            )
            .unwrap();
        let mut document: Value = serde_json::from_str(&text).unwrap();
        let head = &mut document["rows"][0];
        head["values"]["notice_kind"] = json!("실공고(일반공고)");
        if i == 2 {
            head["origins"]
                .as_object_mut()
                .unwrap()
                .remove("notice_kind");
        }
        let kind = if i == 3 {
            "실공고(변경공고)"
        } else {
            "실공고(일반공고)"
        };
        let stored = if i == 6 {
            "null".to_string()
        } else {
            document.to_string()
        };
        c.execute(
            "UPDATE erp_source SET document_json=?1 WHERE entity_base=?2",
            [&stored, &b],
        )
        .unwrap();
        c.execute(
            "UPDATE notice SET notice_kind=?1,source_status='재공고' WHERE notice_base=?2",
            [kind, &b],
        )
        .unwrap();
        if i == 4 || i == 5 {
            let text: String = c
                .query_row(
                    "SELECT snapshot_json FROM erp_capture WHERE entity_base=?1",
                    [&b],
                    |r| r.get(0),
                )
                .unwrap();
            let mut snapshot: Value = serde_json::from_str(&text).unwrap();
            snapshot["mappingRevision"] = json!(if i == 4 { "missing" } else { "saved-custom" });
            c.execute(
                "UPDATE erp_capture SET snapshot_json=?1 WHERE entity_base=?2",
                [&snapshot.to_string(), &b],
            )
            .unwrap();
        }
    }
    c.execute(
        "INSERT INTO erp_mapping(revision,json,created_at) VALUES('saved-custom',?1,'2026-10-01')",
        [&serde_json::to_string(&set.to_value()).unwrap()],
    )
    .unwrap();
    let store = t.f.store();
    let entity = EntityRef::new("notice", "R26BK00000001", "000");
    store
        .set_override(&entity, "공고종류", "사람 확정")
        .unwrap();
    c.execute_batch("PRAGMA user_version=31").unwrap();
    t.db().migrate(t.f.env).unwrap();
    let kinds: Vec<_> =
        t.f.column("SELECT notice_kind FROM notice ORDER BY notice_base")
            .into_iter()
            .map(Option::unwrap)
            .collect();
    assert_eq!(
        vec![
            "실공고",
            "실공고(일반공고)",
            "실공고(변경공고)",
            "실공고(일반공고)",
            "실공고",
            "실공고(일반공고)"
        ],
        kinds
    );
    assert_eq!(
        "사람 확정",
        t.f.scalar("SELECT 공고종류 FROM v_공고 WHERE 공고본번호='R26BK00000001'")
            .unwrap()
    );
    let document: Value = serde_json::from_str(
        &t.f.scalar("SELECT document_json FROM erp_source WHERE entity_base='R26BK00000001'")
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        "실공고",
        document["rows"][0]["values"]["notice_kind"]
            .as_str()
            .unwrap()
    );
    store.clear_override(&entity, "공고종류").unwrap();
    let after_clear = "SELECT 공고종류 FROM v_공고 WHERE 공고본번호='R26BK00000001'";
    assert_eq!("실공고(재공고)", t.f.scalar(after_clear).unwrap());
    t.db().migrate(t.f.env).unwrap();
    assert_eq!("실공고(재공고)", t.f.scalar(after_clear).unwrap());
}

/// C# `ErpCaptureTests.접수_수집의_키는_접수번호_그대로다` — 접수의 키는 접수번호·접수차수 그대로다(ADR-027).
#[test]
fn request_capture_key_is_the_request_number_as_is() {
    let t = T::new("erp-request-key");
    let host = t.plain();
    let input = sample_request(&defaults_revision(), "시험 구매");
    let preview = host.inspect(&input).unwrap();
    assert_eq!("RC001-000", preview.entity);
    let result = host
        .save(
            &input,
            Some(&preview.base_token),
            Some(&t.id()),
            &Vec::new(),
        )
        .unwrap()
        .to_value();
    assert_eq!("RC001-000", result["entity"].as_str().unwrap());
    assert_eq!(
        "RC001",
        t.f.scalar("SELECT request_base FROM request").unwrap()
    );
    assert_eq!(
        "RC001",
        t.f.scalar("SELECT request_base FROM request_series")
            .unwrap()
    );
    assert_eq!(
        "RC001",
        t.f.scalar("SELECT entity_base FROM erp_source WHERE entity_type='request'")
            .unwrap()
    );
    assert_eq!(
        "RC001",
        t.f.scalar("SELECT entity_base FROM erp_capture").unwrap()
    );
    assert_eq!(
        "RC001-000",
        t.f.scalar("SELECT 접수번호 FROM v_접수").unwrap()
    );
    assert_eq!(
        0,
        t.n("SELECT count(*) FROM request WHERE request_base LIKE 'ERP:%'")
    );
}

/// C# `ErpCaptureTests.공고_품목의_인도조건은_코드에서_이름을_얻는다` — 공고 화면은 인도조건의 이름 칸을 비워 보내고 코드만
/// 채운다.
#[test]
fn notice_item_delivery_terms_get_their_name_from_the_code() {
    let t = T::new("erp-delivery");
    let host = t.plain();
    let input = live(
        "g2b-notice-a-v1",
        &defaults_revision(),
        json!({
            "pointInfo": {"bidPbancNo": "R26BK00000077", "bidPbancOrd": "000", "bidPbancNm": "시험 공고"},
            "tables": {"mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List": [
                {"bidClsfNo": "0", "bidPbancItemSqno": "1", "dtlsPrnmNm": "시험품", "devyCndtNm": "", "devyCndtCd": "인010004"}]}
        }),
        "01179",
    );
    t.save_plain(&host, &input);
    assert_eq!(
        "납품장소 입고도",
        t.f.scalar("SELECT delivery_terms FROM notice_item")
            .unwrap()
    );
}

/// C# `ErpCaptureTests.입찰공고_상세의_미리보기에서_게시일시를_읽고_닫힌_채_다시_읽어도_지우지_않는다` — 미리보기를 닫고 다시 읽으면
/// 걷지 않은 칸이라 그대로 둔다.
#[test]
fn posted_at_is_read_from_the_notice_preview_and_kept_when_read_again_closed() {
    let t = T::new("erp-posted");
    let host = t.plain();
    let revision = defaults_revision();
    let read = |point: Value| {
        live(
            "g2b-notice-a-v1",
            &revision,
            json!({"pointInfo": point, "tables": {}}),
            "01179",
        )
    };
    let preview = read(json!({"bidPbancNo": "R26BK00000088", "bidPbancOrd": "000",
        "bidPbancNm": "시험 공고", "pbancPstgDt": "2026/09/01 10:30:00"}));
    t.save_plain(&host, &preview);
    assert_eq!(
        "2026-09-01 10:30:00",
        t.f.scalar("SELECT posted_at FROM notice").unwrap()
    );

    let closed = read(
        json!({"bidPbancNo": "R26BK00000088", "bidPbancOrd": "000", "bidPbancNm": "시험 공고"}),
    );
    assert!(
        !host
            .inspect(&closed)
            .unwrap()
            .changes
            .iter()
            .any(|ch| ch.field == "posted_at")
    );
    t.save_plain(&host, &closed);
    assert_eq!(
        "2026-09-01 10:30:00",
        t.f.scalar("SELECT posted_at FROM notice").unwrap()
    );
}

fn attachments(doc: &MappedDocument) -> usize {
    doc.rows
        .iter()
        .filter(|r| r.table == "erp_notice_attachment")
        .count()
}

/// C# `ErpCaptureTests.와일드카드가_같은_내용의_표_여럿에_맞으면_하나로_보고_다르면_거절한다` — 빈 표를 버리고 남은 것이 내용까지 같으면
/// 하나로 보고, 서로 다르면 어느 것이 대표인지 모르므로 거절한다.
#[test]
fn wildcard_matching_several_identical_tables_counts_as_one_and_different_ones_are_refused() {
    let set = mapping::defaults();
    let profile = "g2b-notice-a-v1";
    let files = |second: &str| {
        json!([
            {"atchFileSqno": 1, "atchFileDscr": "공고서", "orgnlAtchFileNm": "시험 공고서.hwp"},
            {"atchFileSqno": 2, "atchFileDscr": "과업지시서", "orgnlAtchFileNm": second},
        ])
    };
    // 같은 행이라도 키의 순서는 다를 수 있다.
    let reordered = |second: &str| {
        json!([
            {"orgnlAtchFileNm": "시험 공고서.hwp", "atchFileDscr": "공고서", "atchFileSqno": 1},
            {"orgnlAtchFileNm": second, "atchFileDscr": "과업지시서", "atchFileSqno": 2},
        ])
    };
    let input = |tables: Vec<(&str, Value)>| {
        let mut map = Map::new();
        for (id, rows) in tables {
            map.insert(id.into(), rows);
        }
        json!({
            "pointInfo": {"bidPbancNo": "R26BK00000099", "bidPbancOrd": "000", "bidPbancNm": "시험 공고"},
            "tables": map,
        })
    };

    let same = input(vec![
        ("mf_wfm_container_tabEmpty_grdFile", json!([])),
        ("mf_wfm_container_grdFile", files("시험 과업지시서.pdf")),
        (
            "mf_wfm_container_preview_grdFile",
            reordered("시험 과업지시서.pdf"),
        ),
    ]);
    assert_eq!(
        2,
        attachments(&mapping::map(&set, profile, &same, "file").unwrap())
    );
    assert_eq!(2, mapping::table(&same, "*_grdFile").unwrap().len());
    // 걸러 담은 자료에는 고른 표 하나만 남는다 — 빈 표가 아니라 앞의 비지 않은 표다.
    let sanitized = mapping::sanitize(&set, profile, &same).unwrap();
    let kept: Vec<_> = sanitized["tables"]
        .as_object()
        .unwrap()
        .keys()
        .filter(|k| k.ends_with("_grdFile"))
        .collect();
    assert_eq!(vec!["mf_wfm_container_grdFile"], kept);
    assert_eq!(
        2,
        attachments(&mapping::map(&set, profile, &sanitized, "file").unwrap())
    );

    let different = input(vec![
        ("mf_wfm_container_grdFile", files("시험 과업지시서.pdf")),
        ("mf_wfm_container_preview_grdFile", files("다른 첨부.pdf")),
    ]);
    let ambiguous = "대표 표를 하나로 확인하지 못했습니다: *_grdFile";
    assert_eq!(
        ambiguous,
        message(mapping::map(&set, profile, &different, "file"))
    );
    assert_eq!(
        ambiguous,
        message(mapping::sanitize(&set, profile, &different))
    );

    // 모두 비었으면 빈 표 하나다 — 덜 불러온 탭일 수 있어 걷지 않는다.
    let empty = input(vec![
        ("mf_wfm_container_grdFile", json!([])),
        ("mf_wfm_container_preview_grdFile", json!([])),
    ]);
    assert_eq!(
        0,
        attachments(&mapping::map(&set, profile, &empty, "file").unwrap())
    );
    let sanitized_empty = mapping::sanitize(&set, profile, &empty).unwrap();
    assert_eq!(
        1,
        sanitized_empty["tables"]
            .as_object()
            .unwrap()
            .keys()
            .filter(|k| k.ends_with("_grdFile"))
            .count()
    );
}

// ── ConfirmedFieldTests (수집·매핑 몫) ─────────────────────────

impl T {
    /// C# `ConfirmedFieldTests.Capture` — 활성 매핑의 판으로 실제 화면을 담는다.
    fn capture(&self, profile: &str, screen: &str, data: Value) {
        let db = self.db();
        let active = MappingStore::new(&db, self.f.env).active().unwrap();
        let input = live(profile, &mapping::revision(&active), data, screen);
        self.save_plain(&self.host(), &input);
    }

    /// C# `ConfirmedFieldTests.Row` — 뷰에서 한 줄(칸 이름과 값).
    fn row(&self, sql: &str) -> Vec<(String, String)> {
        let c = self.db().open_read_only().unwrap();
        let mut statement = c.prepare(sql).unwrap();
        let names: Vec<String> = statement.column_names().map(|n| n.to_string()).collect();
        statement
            .query_row([], |r| {
                names
                    .iter()
                    .enumerate()
                    .map(|(i, n): (usize, &String)| Ok((n.clone(), text_of(r.get(i)?))))
                    .collect()
            })
            .unwrap()
    }
}

/// C# `ConfirmedFieldTests.Values` — 칸마다 글이 같은가.
fn values(row: &[(String, String)], expected: &[(&str, &str)]) {
    for (column, value) in expected {
        let actual = row
            .iter()
            .find(|(n, _)| n == column)
            .map(|(_, v)| v.as_str())
            .unwrap_or_else(|| panic!("칸이 없다: {column}"));
        assert!(*value == actual, "{column}: {actual}");
    }
}

fn find_mut<'v>(array: &'v mut Value, key: &str, wanted: &str) -> &'v mut Value {
    array
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|x| x[key].as_str() == Some(wanted))
        .unwrap()
}

/// C# `ConfirmedFieldTests.접수의_공고종류_추정가격과_품목의_물품분류번호_납품부대명이_담긴다`
#[test]
fn request_notice_kind_estimated_price_and_item_classification_and_unit_name_are_stored() {
    let t = T::new("cf-request");
    t.capture(
        "g2b-request-v1",
        "01117",
        json!({
            "pointInfo": {"ctrtDmndBizNm": "시험 구매", "wrtYmd": "20261001", "pbancKndCd": "공440002", "prspPrce": "1234500"},
            "tables": {"mf_wfm_container_gridView": [
                {"ctrtDmndRcptNo": "RC021", "ctrtDmndRcptOrd": "000", "ctrtDmndRcptItemSqno": "001",
                 "itemClsfNo": "00000001", "ndfsPrcmItemDlvgdsMtruNm": "시험부대"}]},
        }),
    );
    values(
        &t.row("SELECT * FROM v_접수 WHERE 접수번호 = 'RC021-000'"),
        &[("공고종류", "실공고"), ("추정가격", "1,234,500")],
    );
    values(
        &t.row("SELECT * FROM v_접수품목 WHERE 접수번호 = 'RC021-000'"),
        &[("물품분류번호", "00000001"), ("납품부대명", "시험부대")],
    );
}

/// C# `ConfirmedFieldTests.물품_공고의_입찰방식_계약법_제한업종과_품목의_새_칸이_담긴다`
#[test]
fn goods_notice_bid_method_contract_law_restricted_industry_and_new_item_columns_are_stored() {
    let t = T::new("cf-goods-notice");
    t.capture(
        "g2b-notice-a-v1",
        "01179",
        json!({
            "pointInfo": {"bidPbancNo": "R26BK00000021", "bidPbancOrd": "000", "bidPbancNm": "시험 공고",
                "bidMthdCd": "입180002", "bizAmtFeeSumAmt": "1100000", "lcrtTyCd": "계040002"},
            "tables": {
                "mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List": [
                    {"bidClsfNo": "0", "bidPbancItemSqno": "1", "dmstUntyGrpNo": "AA0000001", "itemClsfNo": "00000001",
                     "itemIdnfNm": "시험품, 시험규격", "ndfsItemSpecNo": "SPEC-1", "ndfsPrcmDmndNo": "REQ21"}],
                // 업종이 여럿이면 줄 차례대로 잇는다.
                "mf_wfm_container_tabCont_contents_itemTabs2_body_wframe7_grdAliasDmTtl07LeftList": [
                    {"bidLmtUntyNm": "시험 업종 가"}, {"bidLmtUntyNm": "시험 업종 나"}],
            },
        }),
    );
    values(
        &t.row("SELECT * FROM v_공고 WHERE 입찰공고번호 = 'R26BK00000021-000'"),
        &[
            ("입찰방식", "전자입찰"),
            ("사업금액수수료합계", "1,100,000"),
            ("계약법구분", "지방계약법"),
            ("입찰참가제한업종", "시험 업종 가, 시험 업종 나"),
            ("입찰진행상태", ""),
        ],
    );
    values(
        &t.row("SELECT * FROM v_공고품목 WHERE 입찰공고번호 = 'R26BK00000021-000'"),
        &[
            ("수요기관코드", "AA0000001"),
            ("물품분류번호", "00000001"),
            ("물품식별명", "시험품, 시험규격"),
            ("규격번호", "SPEC-1"),
            ("조달요구번호", "REQ21"),
            // 물품식별명은 새 열이다 — 상세규격(규격)을 덮지 않는다.
            ("규격", ""),
        ],
    );
}

/// C# `ConfirmedFieldTests.제한_업종은_같은_이름이_겹치거나_비어도_공고를_받고_줄마다_선다` — 제한 업종 표는 줄 차례가 곧 행이다.
#[test]
fn restricted_industries_are_one_row_per_line_even_when_names_repeat_or_are_empty() {
    let t = T::new("cf-industry");
    let notice = |names: &[&str]| {
        let rows: Vec<Value> = names
            .iter()
            .enumerate()
            .map(|(i, n)| json!({"lmtGupSqno": i + 1, "bidLmtUntyNm": n}))
            .collect();
        json!({
            "pointInfo": {"bidPbancNo": "R26BK00000023", "bidPbancOrd": "000", "bidPbancNm": "시험 공고"},
            "tables": {"mf_wfm_container_tabCont_contents_itemTabs2_body_wframe7_grdAliasDmTtl07LeftList": rows},
        })
    };
    t.capture(
        "g2b-notice-a-v1",
        "01179",
        notice(&["시험 업종 가", "시험 업종 가", ""]),
    );
    let names: Vec<String> =
        t.f.column("SELECT name FROM notice_restricted_industry ORDER BY line_no")
            .into_iter()
            .map(Option::unwrap_or_default)
            .collect();
    assert_eq!(vec!["시험 업종 가", "시험 업종 가", ""], names);
    values(
        &t.row("SELECT * FROM v_공고 WHERE 입찰공고번호 = 'R26BK00000023-000'"),
        &[("입찰참가제한업종", "시험 업종 가, 시험 업종 가")],
    );

    // 같은 화면을 다시 가져오면 같은 차례의 줄에 얹혀 바뀐 것이 없다.
    let again = live(
        "g2b-notice-a-v1",
        &defaults_revision(),
        notice(&["시험 업종 가", "시험 업종 가", ""]),
        "01179",
    );
    assert!(t.host().inspect(&again).unwrap().changes.is_empty());
}

/// C# `ConfirmedFieldTests.키를_비우는_것은_기본이_그렇게_둔_표에서만_된다`
#[test]
fn emptying_keys_is_allowed_only_in_tables_the_default_leaves_that_way() {
    let mut node = mapping::defaults().to_value();
    let profile = find_mut(&mut node["profiles"], "id", "g2b-notice-a-v1");
    find_mut(&mut profile["tables"], "target", "notice_item")["keys"] = json!([]);
    assert_eq!(
        "원천 행 복합키가 필요합니다.",
        message(mapping::parse(&node.to_string()))
    );
}

/// C# `ConfirmedFieldTests.통합_조회_공고의_입찰방식_진행상태와_품목의_새_칸이_담긴다`
#[test]
fn unified_lookup_notice_bid_method_progress_and_new_item_columns_are_stored() {
    let t = T::new("cf-notice-b");
    t.capture(
        "g2b-notice-b-v1",
        "01196",
        json!({
            "pointInfo": {"bidPbancText": "R26BK00000022-001", "bidPbancNm": "시험 공고", "bidMthdNm": "전자시담"},
            "tables": {
                "mf_wfm_container_mainWframe_gridView3": [
                    {"bidClsfNo": "0", "bidPbancItemSqno": "1", "dmstUntyGrpNo": "AA0000002",
                     "itemClsfNo": "00000002", "itemIdnfNm": "시험품"}],
                "mf_wfm_container_mainWframe_gridViewPgst": [{"bidPgstNm": "개찰완료"}],
            },
        }),
    );
    values(
        &t.row("SELECT * FROM v_공고 WHERE 입찰공고번호 = 'R26BK00000022-001'"),
        &[("입찰방식", "전자시담"), ("입찰진행상태", "개찰완료")],
    );
    values(
        &t.row("SELECT * FROM v_공고품목 WHERE 입찰공고번호 = 'R26BK00000022-001'"),
        &[
            ("수요기관코드", "AA0000002"),
            ("물품분류번호", "00000002"),
            ("물품식별명", "시험품"),
        ],
    );
}

/// C# `ConfirmedFieldTests.계약의_머리_업체_품목의_새_칸이_담기고_통합에도_실린다`
#[test]
fn contract_head_company_and_item_new_columns_are_stored_and_carried_into_unified() {
    let t = T::new("cf-contract");
    t.capture(
        "g2b-contract-v1",
        "01579",
        json!({
            "pointInfo": {"ctrtNoOrd": "R26TA00000021-00", "ctrtNm": "시험 계약", "dmstPicId": "test01",
                "fprdYn": "N", "lcrtTyCd": "계040001", "prflcAplcnCrtrCd": "물130002", "rlsYn": "Y"},
            "tables": {
                "mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis": [
                    {"ctrtNo": "R26TA00000021", "ctrtChgOrd": "00", "ctrtItemSqno": 1, "ctrtAmt": 1000,
                     "bidClsfNo": "0", "bidPbancItemSqno": 1, "dlvgdsPlacNm": "시험 장소", "dlvgdsTermNody": 90,
                     "dmstUntyGrpNo": "AA0000003", "ndfsPrcmDmndNo": "REQ23", "odn1PlorNtnNm": "시험국"}],
                "mf_wfm_container_grdEtpsLst": [
                    {"ctentUntyGrpNo": "P001", "ctentUntyGrpNm": "시험 업체", "jintSyddCmnMthoNm": "단독계약"}],
            },
        }),
    );
    let head = [
        ("수요기관담당자ID", "test01"),
        ("외국산여부", "아니오"),
        ("계약법구분", "국가계약법"),
        ("물가변동적용기준", "품목조정율"),
        ("공개여부", "Y"),
        ("공동수급구성방식", "단독계약"),
    ];
    values(
        &t.row("SELECT * FROM v_계약 WHERE 계약번호 = 'R26TA0000002100'"),
        &head,
    );
    values(
        &t.row("SELECT * FROM v_통합 WHERE 계약번호 = 'R26TA0000002100'"),
        &head,
    );
    values(
        &t.row("SELECT * FROM v_계약품목 WHERE 계약번호 = 'R26TA0000002100'"),
        &[
            ("입찰분류번호", "0"),
            ("입찰공고품목순번", "1"),
            ("납품장소", "시험 장소"),
            ("납품일수", "90"),
            ("수요기관코드", "AA0000003"),
            ("조달요구번호", "REQ23"),
            ("원산지", "시험국"),
        ],
    );
}

/// C# `ConfirmedFieldTests.다른_화면에서_확정한_필드는_일반용역_접수_공고와_변경계약서에서도_담긴다` — 같은 ID 는 어느 화면에서도 같은
/// 뜻이다(ADR-043 결정 6).
#[test]
fn fields_confirmed_on_other_screens_are_also_stored_for_service_request_notice_and_contract_change()
 {
    let t = T::new("cf-service");
    t.capture(
        "g2b-request-b-v1",
        "01112",
        json!({
            "pointInfo": {"ctrtDmndRcptNo": "RC024", "ctrtDmndRcptOrd": "000", "ctrtDmndBizNm": "시험 용역",
                "pbancKndCd": "공440002", "prspPrce": "3000"},
            "tables": {"mf_wfm_container_gvDmndItem": [
                {"ctrtDmndRcptNo": "RC024", "ctrtDmndRcptOrd": "000", "ctrtDmndRcptItemSqno": 1, "itemClsfNo": "00000004"}]},
        }),
    );
    values(
        &t.row("SELECT * FROM v_접수 WHERE 접수번호 = 'RC024-000'"),
        &[("공고종류", "실공고"), ("추정가격", "3,000")],
    );
    values(
        &t.row("SELECT * FROM v_접수품목 WHERE 접수번호 = 'RC024-000'"),
        &[("물품분류번호", "00000004")],
    );

    t.capture(
        "g2b-notice-a-service-v1",
        "01181",
        json!({
            "pointInfo": {"bidPbancNo": "R26BK00000024", "bidPbancOrd": "000", "bidPbancNm": "시험 용역 공고",
                "ctrtTyNm": "시험 계약구분", "stdCtrtMthdNm": "시험 계약방법", "fprdYnNm": "아니오",
                "prttnDlvgdsPsbltyYnNm": "가능", "rschDvlpItemYnNm": "아니오", "scsbdMthdDtlsNm": "시험 세부기준",
                "bidMthdNm": "전자입찰", "bizAmtFeeSumAmt": "2000", "lcrtTyCd": "계040001"},
            "tables": {
                "mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List": [
                    {"bidClsfNo": "0", "bidPbancItemSqno": "1", "dmstUntyGrpNo": "AA0000004", "itemClsfNo": "00000005"}],
                "mf_wfm_container_tabCont_contents_itemTabs2_body_wframe7_grdAliasDmTtl07LeftList": [
                    {"bidLmtUntyNm": "시험 업종 다"}],
                // 공고 상세 안에 뜬 통합 조회 팝업 — 진행상태는 여기에만 있다.
                "mf_wfm_container_PNPE027_01_wframe_popupCnts_mainWframe_gridViewPgst": [{"bidPgstNm": "시험 진행상태"}],
            },
        }),
    );
    values(
        &t.row("SELECT * FROM v_공고 WHERE 입찰공고번호 = 'R26BK00000024-000'"),
        &[
            ("계약구분", "시험 계약구분"),
            ("계약방법", "시험 계약방법"),
            ("분할납품", "가능"),
            ("입찰방식", "전자입찰"),
            ("사업금액수수료합계", "2,000"),
            ("계약법구분", "국가계약법"),
            ("입찰참가제한업종", "시험 업종 다"),
            ("입찰진행상태", "시험 진행상태"),
        ],
    );
    let (criteria, foreign, research): (String, i64, i64) = t
        .f
        .conn()
        .query_row(
            "SELECT award_criteria, foreign_allowed, research_item FROM notice WHERE notice_base = 'R26BK00000024'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(
        ("시험 세부기준".to_string(), 0, 0),
        (criteria, foreign, research)
    );
    values(
        &t.row("SELECT * FROM v_공고품목 WHERE 입찰공고번호 = 'R26BK00000024-000'"),
        &[("수요기관코드", "AA0000004"), ("물품분류번호", "00000005")],
    );

    t.capture(
        "g2b-contract-change-v1",
        "01608",
        json!({
            "pointInfo": {"ctrtNm": "시험 변경계약", "ctrtTyNm": "시험 계약구분"},
            "tables": {"mf_wfm_container_grdCtrtLis": [
                {"ctrtNo": "R26TA00000024", "ctrtChgOrd": "01", "ctrtItemSqno": 1, "ctrtAmt": 1000,
                 "bidClsfNo": "0", "bidPbancItemSqno": 1, "dlvgdsPlacNm": "시험 장소", "dlvgdsTermNody": 30,
                 "dmstUntyGrpNo": "AA0000005", "ndfsPrcmDmndNo": "REQ24", "odn1PlorNtnNm": "시험국"}]},
        }),
    );
    values(
        &t.row("SELECT * FROM v_계약 WHERE 계약번호 = 'R26TA0000002401'"),
        &[("계약구분", "시험 계약구분")],
    );
    values(
        &t.row("SELECT * FROM v_계약품목 WHERE 계약번호 = 'R26TA0000002401'"),
        &[
            ("입찰분류번호", "0"),
            ("입찰공고품목순번", "1"),
            ("납품장소", "시험 장소"),
            ("납품일수", "30"),
            ("수요기관코드", "AA0000005"),
            ("조달요구번호", "REQ24"),
            ("원산지", "시험국"),
        ],
    );
}

/// C# `ConfirmedFieldTests.재무관은_공고_A_에서만_담기고_통합_조회의_채권자_후보는_담지_않는다`
#[test]
fn finance_officer_is_stored_from_notice_a_only_and_the_creditor_candidate_is_not() {
    let t = T::new("cf-finance");
    t.capture(
        "g2b-notice-a-v1",
        "01179",
        json!({
            "pointInfo": {"bidPbancNo": "R26BK00000025", "bidPbancOrd": "000", "bidPbancNm": "시험 공고", "fncrNm": "홍길동"},
            "tables": {},
        }),
    );
    t.capture(
        "g2b-notice-b-v1",
        "01196",
        json!({
            "pointInfo": {"bidPbancText": "R26BK00000026-000", "bidPbancNm": "시험 공고", "grnteCrdrNm": "시험 기관장"},
            "tables": {},
        }),
    );
    values(
        &t.row("SELECT * FROM v_공고 WHERE 입찰공고번호 = 'R26BK00000025-000'"),
        &[("재무관", "홍길동")],
    );
    values(
        &t.row("SELECT * FROM v_공고 WHERE 입찰공고번호 = 'R26BK00000026-000'"),
        &[("재무관", "")],
    );
    assert!(
        !mapping::defaults()
            .profiles_iter()
            .flat_map(|p| p.fields_iter())
            .any(|f| f.source() == "grnteCrdrNm")
    );
}

/// C# `ConfirmedFieldTests.채권자_대상이_남은_편집본은_읽을_때_떨구고_재무관을_채운다` — 거절하면 활성 매핑을 읽는 수집이 통째로 멈춘다.
#[test]
fn edited_mapping_with_creditor_target_drops_it_on_read_and_fills_finance_officer() {
    let mut node = mapping::defaults().to_value();
    find_mut(
        &mut find_mut(&mut node["profiles"], "id", "g2b-notice-a-v1")["fields"],
        "target",
        "finance_officer",
    )["target"] = json!("creditor");
    find_mut(&mut node["profiles"], "id", "g2b-notice-a-v1")["selectors"] =
        json!({"fncrNm": "#custom-finance"});
    let b = find_mut(&mut node["profiles"], "id", "g2b-notice-b-v1");
    b["selectors"] = json!({"grnteCrdrNm": "#retired-creditor"});
    b["fields"]
        .as_array_mut()
        .unwrap()
        .push(json!({"source": "grnteCrdrNm", "target": "creditor", "transform": "text"}));
    let text = node.to_string();

    let read = mapping::read(&text).unwrap();
    assert!(
        !read
            .profiles_iter()
            .flat_map(|p| p.fields_iter())
            .any(|f| f.target() == "creditor")
    );
    let a = read.profile("g2b-notice-a-v1").unwrap();
    let selector_a = a
        .selectors
        .as_ref()
        .unwrap()
        .iter()
        .find(|(k, _)| k == "fncrNm")
        .and_then(|(_, v)| v.as_deref());
    assert_eq!(Some("#custom-finance"), selector_a);
    let selectors_b = read
        .profile("g2b-notice-b-v1")
        .unwrap()
        .selectors
        .as_ref()
        .unwrap();
    assert!(!selectors_b.iter().any(|(k, _)| k == "grnteCrdrNm"));
    let officers: Vec<_> = a
        .fields_iter()
        .filter(|f| f.target() == "finance_officer")
        .collect();
    assert_eq!(1, officers.len());
    assert_eq!("fncrNm", officers[0].source());
    for original in mapping::defaults().profiles_iter() {
        let mut expected: Vec<_> = original
            .fields_iter()
            .map(|f| f.target().to_string())
            .collect();
        let mut actual: Vec<_> = read
            .profile(original.id())
            .unwrap()
            .fields_iter()
            .map(|f| f.target().to_string())
            .collect();
        expected.sort();
        actual.sort();
        assert_eq!(expected, actual);
    }
}

/// C# `ConfirmedFieldTests.새_머리_열은_덮개로_고쳐진다` — 새 열도 사람이 고칠 수 있다(ADR-020). 여기서는 접수 화면을 실제로 수집해
/// 자료를 세운다.
#[test]
fn new_head_column_is_corrected_by_override_after_a_real_capture() {
    let t = T::new("cf-override");
    t.capture(
        "g2b-request-v1",
        "01117",
        json!({
            "pointInfo": {"ctrtDmndBizNm": "시험 구매", "prspPrce": "1000"},
            "tables": {"mf_wfm_container_gridView": [
                {"ctrtDmndRcptNo": "RC022", "ctrtDmndRcptOrd": "000", "ctrtDmndRcptItemSqno": "001"}]},
        }),
    );
    t.f.store()
        .set_override(
            &EntityRef::new("request", "RC022", "000"),
            "추정가격",
            "2,000",
        )
        .unwrap();
    values(
        &t.row("SELECT * FROM v_접수 WHERE 접수번호 = 'RC022-000'"),
        &[("추정가격", "2,000")],
    );
}

/// C# `ConfirmedFieldTests.새_필드를_더하기_전의_편집본은_읽을_때_새_대상을_채우고_고친_것은_남긴다` — 채우지 않으면 그 작업자료만 새
/// 필드를 영영 수집하지 않는데 편집본은 검증을 통과하므로 아무것도 실패하지 않는다.
#[test]
fn mapping_edited_before_a_new_field_is_filled_on_read_and_keeps_edits() {
    let t = T::new("cf-fill");
    let mut node = mapping::defaults().to_value();
    // 머리 필드 하나·품목 필드 하나·표 하나가 아직 없던 편집본. 다른 칸의 출처는 고쳐 두었다.
    {
        let request = find_mut(&mut node["profiles"], "id", "g2b-request-v1");
        request["fields"]
            .as_array_mut()
            .unwrap()
            .retain(|f| f["target"] != "estimated_price");
        request["tables"][0]["fields"]
            .as_array_mut()
            .unwrap()
            .retain(|f| f["target"] != "classification_number");
        find_mut(&mut request["fields"], "target", "title")["source"] = json!("editedTitle");
    }
    {
        let notice = find_mut(&mut node["profiles"], "id", "g2b-notice-a-v1");
        notice["tables"]
            .as_array_mut()
            .unwrap()
            .retain(|x| x["target"] != "notice_restricted_industry");
    }
    t.f.conn()
        .execute(
            "INSERT INTO erp_mapping(revision,json,active,validated,created_at) VALUES('old',?1,1,1,'2026-01-01T00:00:00Z')",
            [&node.to_string()],
        )
        .unwrap();

    let db = t.db();
    let active = MappingStore::new(&db, t.f.env).active().unwrap();
    let defaults = mapping::defaults();
    let active_request = active.profile("g2b-request-v1").unwrap();
    let source_of = |target: &str| {
        active_request
            .fields_iter()
            .find(|f| f.target() == target)
            .unwrap()
            .source()
            .to_string()
    };
    assert_eq!("editedTitle", source_of("title"));
    assert_eq!("prspPrce", source_of("estimated_price"));
    let first_table = active_request.tables_iter().next().unwrap();
    assert!(
        first_table
            .fields_iter()
            .any(|f| f.target() == "classification_number")
    );
    let targets = |set: &mapping::MappingSet| -> Vec<String> {
        set.profile("g2b-notice-a-v1")
            .unwrap()
            .tables_iter()
            .map(|x| x.target().to_string())
            .collect()
    };
    assert_eq!(targets(&defaults), targets(&active));

    // 채운 매핑으로 실제로 담긴다.
    t.capture(
        "g2b-request-v1",
        "01117",
        json!({
            "pointInfo": {"editedTitle": "시험 구매", "prspPrce": "5000"},
            "tables": {"mf_wfm_container_gridView": [
                {"ctrtDmndRcptNo": "RC023", "ctrtDmndRcptOrd": "000", "ctrtDmndRcptItemSqno": "001",
                 "itemClsfNo": "00000003"}]},
        }),
    );
    values(
        &t.row("SELECT * FROM v_접수 WHERE 접수번호 = 'RC023-000'"),
        &[("요청명", "시험 구매"), ("추정가격", "5,000")],
    );
    values(
        &t.row("SELECT * FROM v_접수품목 WHERE 접수번호 = 'RC023-000'"),
        &[("물품분류번호", "00000003")],
    );

    // 기본과 같은 편집본은 기본 그대로다 — 채울 것이 없으면 손대지 않는다.
    assert_eq!(
        mapping::revision(&defaults),
        mapping::revision(&mapping::read(&defaults.to_value().to_string()).unwrap())
    );
}

// ── FieldRegistryTests ─────────────────────────────────────

mod registry {
    //! C# `FieldRegistryTests` — 수집 필드 목록(`docs/erp/fields.md`)과 기본 매핑을 대조한다(ADR-043).

    use std::collections::{BTreeSet, HashSet};

    use pclm_core::erp::mapping;

    type Key = (String, String, String);

    const STATUSES: [&str; 5] = [
        "수집",
        "제외: 개인정보",
        "제외: 다른 출처",
        "제외: 뜻 없음",
        "후보",
    ];

    /// 뜻이 확정되어 `수집` 인데 아직 매핑에 담지 않은 필드. 줄기만 한다 — 매핑에 담으면 여기서 지운다.
    const NOT_YET_MAPPED: [(&str, &str, &str); 0] = [];

    const HEADER: &str = "| 프로필 | 출처 | ID | 뜻 | 상태 | 근거 |";

    struct Row {
        line: usize,
        profile: String,
        source: String,
        id: String,
        status: String,
    }

    impl Row {
        fn key(&self) -> Key {
            (self.profile.clone(), self.source.clone(), self.id.clone())
        }
    }

    /// 머리줄이 [`HEADER`] 인 표의 행만 읽는다 — 상태 설명 같은 다른 표는 건너뛴다.
    fn registry() -> Vec<Row> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("docs")
            .join("erp")
            .join("fields.md");
        let text = std::fs::read_to_string(path).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        let mut rows = Vec::new();
        let mut in_table = false;
        let mut i = 0;
        while i < lines.len() {
            let line = lines[i].trim();
            if line == HEADER {
                in_table = true;
                i += 2; // 구분줄을 건너뛴다
                continue;
            }
            if in_table {
                if !line.starts_with('|') {
                    in_table = false;
                } else {
                    let cells: Vec<&str> = line
                        .trim_matches('|')
                        .split('|')
                        .map(|c| c.trim().trim_matches('`'))
                        .collect();
                    assert!(
                        cells.len() == 6,
                        "fields.md {}행: 칸이 여섯이 아닙니다 — {line}",
                        i + 1
                    );
                    rows.push(Row {
                        line: i + 1,
                        profile: cells[0].into(),
                        source: cells[1].into(),
                        id: cells[2].into(),
                        status: cells[4].into(),
                    });
                }
            }
            i += 1;
        }
        assert!(!rows.is_empty());
        rows
    }

    fn head(profile: &str, source: &str) -> Key {
        if !source.starts_with("tables.") {
            return (profile.into(), "pointInfo".into(), source.into());
        }
        let parts: Vec<&str> = source.split('.').collect();
        (profile.into(), parts[1].into(), parts[3..].join("."))
    }

    /// 기본 매핑이 읽는 (프로필, 출처, ID). 식별 필드와 secondarySource 도 넣는다 — 매핑이 읽어 담는 값이라, 빼면 목록에
    /// 없는 칸이 조용히 담긴다.
    fn mapped() -> HashSet<Key> {
        let mut keys = HashSet::new();
        for p in mapping::defaults().profiles_iter() {
            let id = p.id().to_string();
            for f in p.fields_iter() {
                keys.insert(head(&id, f.source()));
                if let Some(s) = &f.secondary_source {
                    keys.insert(head(&id, s));
                }
            }
            keys.insert((
                id.clone(),
                p.identity_source().into(),
                p.base_field().into(),
            ));
            keys.insert((id.clone(), p.identity_source().into(), p.seq_field().into()));
            for t in p.tables_iter() {
                for f in t.fields_iter() {
                    keys.insert((id.clone(), t.source().into(), f.source().into()));
                    if let Some(s) = &f.secondary_source {
                        keys.insert((id.clone(), t.source().into(), s.clone()));
                    }
                }
                for k in t.keys_iter() {
                    keys.insert((id.clone(), t.source().into(), k.into()));
                }
            }
        }
        keys
    }

    fn show(k: &Key) -> String {
        format!("{} · {} · {}", k.0, k.1, k.2)
    }

    fn not_yet() -> HashSet<Key> {
        NOT_YET_MAPPED
            .iter()
            .map(|(a, b, c)| (a.to_string(), b.to_string(), c.to_string()))
            .collect()
    }

    /// C# `FieldRegistryTests.매핑이_읽는_필드는_모두_수집으로_적혀_있다`
    #[test]
    fn every_field_the_mapping_reads_is_listed_as_collected() {
        let collected: HashSet<Key> = registry()
            .iter()
            .filter(|r| r.status == "수집")
            .map(Row::key)
            .collect();
        let missing: BTreeSet<String> = mapped()
            .iter()
            .filter(|k| !collected.contains(*k))
            .map(show)
            .collect();
        assert!(
            missing.is_empty(),
            "매핑이 읽는데 fields.md 에 「수집」 으로 없는 필드:\n  {}\n목록에 「수집」 으로 적거나, 뜻이 확정되지 않았으면 매핑에서 빼세요(ADR-043).",
            missing.into_iter().collect::<Vec<_>>().join("\n  ")
        );
    }

    /// C# `FieldRegistryTests.수집으로_적힌_필드는_매핑에_있거나_아직_담지_않은_목록에_있다`
    #[test]
    fn fields_listed_as_collected_are_mapped_or_in_the_not_yet_mapped_list() {
        let mapped = mapped();
        let pending = not_yet();
        let stray: Vec<String> = registry()
            .iter()
            .filter(|r| {
                r.status == "수집" && !mapped.contains(&r.key()) && !pending.contains(&r.key())
            })
            .map(|r| format!("{}행 {}", r.line, show(&r.key())))
            .collect();
        assert!(
            stray.is_empty(),
            "fields.md 에 「수집」 인데 매핑에 없는 필드:\n  {}\n매핑에 담으세요. 확정이면 담는다(ADR-043).",
            stray.join("\n  ")
        );
    }

    /// C# `FieldRegistryTests.아직_담지_않은_목록은_줄기만_한다`
    #[test]
    fn the_not_yet_mapped_list_only_shrinks() {
        let mapped = mapped();
        let collected: HashSet<Key> = registry()
            .iter()
            .filter(|r| r.status == "수집")
            .map(Row::key)
            .collect();
        let done: Vec<String> = not_yet()
            .iter()
            .filter(|k| mapped.contains(*k))
            .map(show)
            .collect();
        assert!(
            done.is_empty(),
            "이미 매핑에 담겼으니 NotYetMapped 에서 지우세요:\n  {}",
            done.join("\n  ")
        );
        let orphan: Vec<String> = not_yet()
            .iter()
            .filter(|k| !collected.contains(*k))
            .map(show)
            .collect();
        assert!(
            orphan.is_empty(),
            "fields.md 에 「수집」 으로 없는 NotYetMapped 항목:\n  {}",
            orphan.join("\n  ")
        );
        assert_eq!(NOT_YET_MAPPED.len(), not_yet().len());
    }

    /// C# `FieldRegistryTests.한_ID_에는_뜻이_하나다` — 같은 문자열의 ID 는 어느 화면·표에서도 같은 뜻이다(ADR-043 결정 6).
    #[test]
    fn one_id_has_one_meaning() {
        let rows = registry();
        let mut ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        let split: Vec<String> = ids
            .into_iter()
            .filter_map(|id| {
                let group: Vec<&Row> = rows.iter().filter(|r| r.id == id).collect();
                let unsettled = |r: &&Row| r.status == "후보" || r.status == "제외: 뜻 없음";
                (group.iter().any(|r| r.status == "수집") && group.iter().any(unsettled)).then(
                    || {
                        format!(
                            "{id} — {}",
                            group
                                .iter()
                                .filter(|r| r.status == "수집" || unsettled(r))
                                .map(|r| format!(
                                    "{}행 {} · {} 「{}」",
                                    r.line, r.profile, r.source, r.status
                                ))
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    },
                )
            })
            .collect();
        assert!(
            split.is_empty(),
            "같은 ID 가 한 자리에서는 「수집」 이고 다른 자리에서는 「후보」·「제외: 뜻 없음」 입니다:\n  {}\n같은 ID 는 어느 화면·표에서도 같은 뜻이다 — 한 번 확정한 ID 는 다른 자리에서도 확정이고, 자리마다 정하는 것은 사본인지(「제외: 다른 출처」)뿐이다(ADR-043 결정 6).",
            split.join("\n  ")
        );
    }

    /// C# `FieldRegistryTests.상태는_다섯_가지이고_필드마다_한_줄이다`
    #[test]
    fn status_is_one_of_five_and_each_field_has_one_line() {
        let rows = registry();
        let profiles: HashSet<String> = mapping::defaults()
            .profiles_iter()
            .map(|p| p.id().to_string())
            .collect();
        let bad: Vec<String> = rows
            .iter()
            .filter(|r| {
                !STATUSES.contains(&r.status.as_str())
                    || !profiles.contains(&r.profile)
                    || r.source.is_empty()
                    || r.id.is_empty()
            })
            .map(|r| format!("{}행 {} — 「{}」", r.line, show(&r.key()), r.status))
            .collect();
        assert!(
            bad.is_empty(),
            "상태가 다섯 가지 밖이거나 프로필을 모르는 행:\n  {}",
            bad.join("\n  ")
        );
        let mut seen: std::collections::HashMap<Key, Vec<usize>> = std::collections::HashMap::new();
        for r in &rows {
            seen.entry(r.key()).or_default().push(r.line);
        }
        let twice: Vec<String> = seen
            .iter()
            .filter(|(_, lines)| lines.len() > 1)
            .map(|(k, lines)| {
                format!(
                    "{} — {}",
                    show(k),
                    lines
                        .iter()
                        .map(|l| format!("{l}행"))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
            .collect();
        assert!(
            twice.is_empty(),
            "같은 필드가 두 번 적혔습니다:\n  {}",
            twice.join("\n  ")
        );
    }
}

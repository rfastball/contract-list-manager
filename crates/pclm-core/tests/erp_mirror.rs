//! 지금 보는 화면(ADR-036)·수집하는 화면(ADR-037)·들어온 자료 시험.
//!
//! 옮기지 않은 것: 홈의 파일에 닿는 중계(`ExtensionRelay`·`ExtensionPresence` — 탭 보고·명령 폴더·상시 호스트 파이프)와
//! 자동 수집 설정의 **파일** 읽고 쓰기(`AutoCapturePreferences.Read/Write(home)`)는 이 크레이트에 없다. 설정은 글(`from_json`·
//! `to_json`)만, 자동 수집은 판단(`AutoCapture::try_save`)만 옮긴다. 쪽지(`Home`)는 [`Pointing`] 이 대신한다.
//!
//! 시각은 이 기계의 시간대가 아니라 한국 표준시(+9) 고정 시간대로 읽는다 — 기대값은 그 시간대의 자정 경계로 셈한다. 번호·건명·금액은 모두 지어낸 것이다.

mod common;

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use common::{Temp, TestEnv};
use pclm_core::erp::auto_capture::{AutoCapture, AutoCapturePreferences, AutoCaptureResult};
use pclm_core::erp::capture::{
    CaptureInput, CapturePreview, Choices, Destination, ErpCapture, SaveResult,
};
use pclm_core::erp::mapping::{self, MappingProfile, MappingScreen, MappingSet, MappingStore};
use pclm_core::erp::mirror::{
    self, MirrorExport, MirrorField, MirrorScreen, MirrorShot, ScreenRow, default_choices,
};
use pclm_core::erp::{capture_log, screens};
use pclm_core::local_time::{FixedZone, LocalDateTime, to_utc_text};
use pclm_core::storage::store::{DOCUMENT_VIEW_ID, EntityRef, Store};
use pclm_core::storage::views;
use pclm_core::{Access, Database, Env, Error, Result};
use rusqlite::types::ValueRef;
use rusqlite::{Connection, params};
use serde_json::{Map, Value, json};

const ZONE: FixedZone = FixedZone(9 * 3600);
const ITEMS: &str = "mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis";

// ── 같이 쓰는 것 ───────────────────────────────────────────────

/// 거절 사유(던진 오류의 글).
fn reason<T>(result: Result<T>) -> String {
    match result {
        Err(Error::InvalidOperation(message)) => message,
        Err(other) => panic!("InvalidOperation 이 아닌 오류: {other:?}"),
        Ok(_) => panic!("거절되지 않았다"),
    }
}

fn s(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| v.to_string()).collect()
}

fn single<'a, T>(items: impl Iterator<Item = &'a T>) -> &'a T {
    let mut found: Vec<&T> = items.collect();
    assert_eq!(found.len(), 1, "정확히 하나여야 한다");
    found.remove(0)
}

fn cell(value: ValueRef<'_>) -> String {
    match value {
        ValueRef::Null | ValueRef::Blob(_) => String::new(),
        ValueRef::Integer(i) => i.to_string(),
        ValueRef::Real(f) => f.to_string(),
        ValueRef::Text(t) => String::from_utf8_lossy(t).into_owned(),
    }
}

/// 첫 줄 첫 칸의 글 — NULL 이면 시험이 깨지게 `String` 으로 읽는다.
fn text_of(c: &Connection, sql: &str) -> String {
    c.query_row(sql, [], |r| r.get::<_, String>(0)).unwrap()
}

fn count(c: &Connection, table: &str) -> i64 {
    common::count(c, table)
}

/// 계약 한 건의 화면(메뉴 01579). `items` 는 (순번, 수량, 단가).
fn contract(seq: &str, title: &str, date: &str, items: &[(i64, i64, i64)]) -> CaptureInput {
    let rows: Vec<Value> = items
        .iter()
        .map(|&(line, quantity, price)| {
            json!({
                "ctrtNo": "R26TA00000001", "ctrtChgOrd": seq, "ctrtItemSqno": line,
                "ctrtItemNm": format!("시험 품목 {line}"), "ctrtUntVal": "세트",
                "ctrtQty": quantity, "ctrtUprc": price, "ctrtAmt": quantity * price,
            })
        })
        .collect();
    let mut tables = Map::new();
    tables.insert(ITEMS.into(), Value::Array(rows));
    let data = json!({
        "pointInfo": {
            "ctrtNoOrd": format!("R26TA00000001-{seq}"), "ctrtNm": title, "ctrtDt": date,
            "ctrtGtnrt": "5", "ctrtGrnteAmt": "8094520",
            "bidPbancNo": "R26BK00000001", "dmstUntyGrpNm": "가온시험기관",
        },
        "tables": Value::Object(tables),
    });
    live("g2b-contract-v1", data, Some("01579"))
}

/// 확장이 읽은 실제 화면.
fn live(profile: &str, data: Value, screen: Option<&str>) -> CaptureInput {
    let mut input = CaptureInput::new(profile, &mapping::revision(&mapping::defaults()), data);
    input.scope = Some("live".into());
    input.screen = screen.map(str::to_string);
    input
}

/// 파일로 받은 자료(범위 `file`).
fn file_input(profile: &str, data: Value) -> CaptureInput {
    CaptureInput::new(profile, &mapping::revision(&mapping::defaults()), data)
}

fn with_data(input: &CaptureInput, change: impl FnOnce(&mut Value)) -> CaptureInput {
    let mut out = input.clone();
    change(out.data.as_mut().unwrap());
    out
}

fn choice<'a>(choices: &'a Choices, id: &str) -> &'a str {
    choices
        .iter()
        .find(|(k, _)| k == id)
        .map(|(_, v)| v.as_str())
        .unwrap()
}

fn field<'a>(shot: &'a MirrorShot, column: &str) -> &'a MirrorField {
    single(shot.fields.iter().filter(|f| f.column == column))
}

/// 작업자료 하나와 그 위의 수집 경계.
struct Harness {
    _temp: Temp,
    path: PathBuf,
    db: Database,
    env: TestEnv,
}

impl Harness {
    fn new(name: &str) -> Self {
        let temp = Temp::new(name);
        let path = temp.path("계약자료.pclm");
        let env = TestEnv::new();
        let db = common::workfile(&path, &env);
        Harness {
            _temp: temp,
            path,
            db,
            env,
        }
    }

    fn host(&self) -> ErpCapture<'_> {
        ErpCapture::new(&self.path, &self.env)
    }

    fn project(&self, input: &CaptureInput) -> Result<MirrorShot> {
        self.project_with(input, &[])
    }

    fn project_with(&self, input: &CaptureInput, screen: &[ScreenRow]) -> Result<MirrorShot> {
        mirror::project(&self.host(), &self.env, &ZONE, input, screen)
    }

    /// 확장과 같은 길로 저장한다.
    fn save(&self, input: &CaptureInput) {
        let host = self.host();
        let preview = host.inspect(input).unwrap();
        host.save(
            input,
            Some(&preview.base_token),
            Some(&self.env.new_id()),
            &default_choices(&preview, &[]),
        )
        .unwrap();
    }

    fn read(&self) -> Connection {
        self.db.open_read_only().unwrap()
    }

    fn write(&self) -> Connection {
        self.db.open().unwrap()
    }

    fn dataset(&self) -> String {
        text_of(&self.read(), "SELECT dataset_id FROM pclm_file")
    }
}

/// 투영이 아무것도 남기지 않았는가 — 다른 연결이 본 data_version 과 업무 표의 줄 수.
fn footprint(watcher: &Connection) -> (i64, i64) {
    let version = watcher
        .query_row("PRAGMA data_version", [], |r| r.get(0))
        .unwrap();
    let rows = watcher
        .query_row(
            "SELECT (SELECT count(*) FROM contract) + (SELECT count(*) FROM contract_item) + (SELECT count(*) FROM contract_series)
                  + (SELECT count(*) FROM erp_source) + (SELECT count(*) FROM erp_row) + (SELECT count(*) FROM erp_capture)
                  + (SELECT count(*) FROM field_override)",
            [],
            |r| r.get(0),
        )
        .unwrap();
    (version, rows)
}

fn view_row(c: &Connection, view: &str, key: &str, value: &str) -> HashMap<String, String> {
    let mut statement = c
        .prepare(&format!("SELECT * FROM \"{view}\" WHERE \"{key}\"=?1"))
        .unwrap();
    let names: Vec<String> = statement.column_names().map(str::to_string).collect();
    statement
        .query_row([value], |r| {
            Ok(names
                .iter()
                .enumerate()
                .map(|(i, name)| (name.clone(), cell(r.get_ref(i).unwrap())))
                .collect())
        })
        .unwrap()
}

/// 홈의 쪽지 자리 — 지금 가리키는 (파일, 자료 ID).
struct Pointing(RefCell<Option<(PathBuf, String)>>);

impl Pointing {
    fn none() -> Self {
        Pointing(RefCell::new(None))
    }

    fn write_config(&self, path: &Path, dataset: &str) {
        *self.0.borrow_mut() = Some((path.to_path_buf(), dataset.to_string()));
    }
}

impl Destination for Pointing {
    fn points_at(&self, database_path: &Path, dataset_id: &str) -> bool {
        self.0
            .borrow()
            .as_ref()
            .is_some_and(|(path, id)| path == database_path && id == dataset_id)
    }
}

// ── 자동 수집 ───────────────────────────────────────────────

/// 설정은 글로만 다닌다 — 파일 읽고 쓰기는 홈의 것이다. 담당자 이름은 설정이 아니다(확장이 보고에 싣는 내 정보).
#[test]
fn 자동수집_설정은_로컬에서_기본꺼짐이며_켜기만_저장한다() {
    // 파일이 없을 때의 읽기는 글이 없는 것과 같다.
    assert!(!AutoCapturePreferences::from_json("").enabled);
    let settings = AutoCapturePreferences { enabled: true };
    assert_eq!(AutoCapturePreferences::from_json(&settings.to_json()), settings);
    assert_eq!(settings.to_value(), json!({ "enabled": true }));
    // 옛 파일의 이름 · 규칙은 모르는 칸이라 건너뛴다.
    let legacy = r#"{"enabled":true,"name":"합성 담당자","rules":[{"screen":"01579","enabled":true,"source":"ctrtNm","text":"옛 조건"}]}"#;
    assert_eq!(AutoCapturePreferences::from_json(legacy), settings);
    assert_eq!(
        AutoCapturePreferences::from_json(r#"{"enabled":true,"name":null}"#),
        settings
    );
    for invalid in ["{", r#"{"enabled":"yes"}"#, "[]"] {
        assert!(!AutoCapturePreferences::from_json(invalid).enabled);
    }
}

#[derive(Clone, Copy)]
struct TabOptions<'a> {
    state: &'a str,
    stable: bool,
    age: i64,
    target: Option<&'a str>,
}

impl Default for TabOptions<'_> {
    fn default() -> Self {
        TabOptions {
            state: "supported",
            stable: true,
            age: 0,
            target: None,
        }
    }
}

/// 확장의 탭 보고 한 탭 — 스냅샷과 새로 읽었다는 증거(readiness)를 싣는다.
fn auto_tab(snapshot: &CaptureInput, dataset: &str, now: i64, options: TabOptions<'_>) -> Value {
    json!({
        "state": options.state,
        "menu": snapshot.screen,
        "datasetId": options.target.unwrap_or(dataset),
        "snapshot": snapshot.to_value(),
        "readiness": { "stable": options.stable, "observedAt": now - options.age, "documentId": "synthetic-document" },
    })
}

fn state_of(result: Result<AutoCaptureResult>) -> String {
    result.unwrap().state
}

fn with_staff(input: &CaptureInput, staff: &str) -> CaptureInput {
    with_data(input, |data| data["pointInfo"]["ctrtPicNm"] = json!(staff))
}

#[test]
fn 자동수집은_현재담당자_부분일치와_새읽기_대상_규칙을_모두_확인한다() {
    let h = Harness::new("mirror-auto");
    let home = Pointing::none();
    let c = h.read();
    let dataset = h.dataset();
    home.write_config(&h.path, &dataset);
    let now = 1_800_000_000_000_i64;
    let input = with_staff(
        &contract("00", "합성 담당자", "20260914", &[(1, 1, 1000)]),
        " 합성 담당자 ",
    );
    let preferences = AutoCapturePreferences { enabled: true };
    let tab = |snapshot: &CaptureInput, options: TabOptions<'_>| {
        auto_tab(snapshot, &dataset, now, options)
    };
    let plain = TabOptions::default();
    let mut automatic = AutoCapture::new(&h.db, &home, &h.env);
    // 이름은 확장이 보고에 실은 패널의 내 정보 이름이다.
    let judge = |automatic: &mut AutoCapture<'_>, tab: &Value, name: &str| {
        automatic.try_save(tab, &preferences, name, now, None).unwrap()
    };
    let save = |automatic: &mut AutoCapture<'_>, tab: &Value, name: &str| {
        judge(automatic, tab, name).state
    };
    let me = "합성 담당자";

    assert_eq!(
        state_of(automatic.try_save(
            &tab(&input, plain),
            &AutoCapturePreferences::default(),
            me,
            now,
            None
        )),
        "hold"
    );
    for options in [
        TabOptions {
            state: "reading",
            ..plain
        },
        TabOptions {
            stable: false,
            ..plain
        },
        TabOptions { age: 8001, ..plain },
        TabOptions {
            target: Some("other-dataset"),
            ..plain
        },
    ] {
        assert_eq!(save(&mut automatic, &tab(&input, options), me), "hold");
    }
    let old_rule = CaptureInput {
        mapping_revision: Some("old".into()),
        ..input.clone()
    };
    assert_eq!(save(&mut automatic, &tab(&old_rule, plain), me), "hold");
    let without_staff = contract("01", "담당자 없는 합성 계약", "20260915", &[(1, 1, 1000)]);
    assert_eq!(save(&mut automatic, &tab(&without_staff, plain), me), "hold");
    assert_eq!(save(&mut automatic, &tab(&input, plain), "합성 담당자2"), "hold");
    for staff in ["다른 담당자(010-0000-0000)", ""] {
        assert_eq!(
            save(&mut automatic, &tab(&with_staff(&input, staff), plain), me),
            "hold"
        );
    }
    // 이름이 비면 어느 화면도 자동으로 수집하지 않는다.
    for empty in ["", "  "] {
        let held = judge(&mut automatic, &tab(&input, plain), empty);
        assert_eq!(
            (held.state.as_str(), held.message.as_str()),
            ("hold", "패널 설정의 내 정보에 이름이 없어 자동으로 수집하지 않습니다.")
        );
        assert_eq!(save(&mut automatic, &tab(&without_staff, plain), empty), "hold");
    }
    assert_eq!(count(&c, "erp_capture"), 0);
    assert_eq!(
        save(&mut automatic, &tab(&input, plain), "  합성 담당자  "),
        "stored"
    );
    assert_eq!(count(&c, "erp_capture"), 1);
    // 새 coordinator도 같은 내용을 보고 영수증을 늘리지 않는다.
    let mut second = AutoCapture::new(&h.db, &home, &h.env);
    assert_eq!(save(&mut second, &tab(&input, plain), me), "unchanged");
    assert_eq!(save(&mut automatic, &tab(&input, plain), "담당자"), "unchanged");
    // 이름이 바뀌면 같은 읽기라도 다시 판단한다 — 앞서 낸 결과를 돌려주지 않는다.
    assert_eq!(save(&mut automatic, &tab(&input, plain), "홍길동"), "hold");
    assert_eq!(count(&c, "erp_capture"), 1);
    let mut table = Map::new();
    table.insert(
        "mf_wfm_container_gridView".into(),
        json!([{
            "ctrtDmndRcptNo": "RC001", "ctrtDmndRcptOrd": "000", "ctrtDmndRcptItemSqno": "001",
            "ndfsPrcmDmndNo": "REQ1", "ctrtDmndQty": "1",
        }]),
    );
    let request = live(
        "g2b-request-v1",
        json!({
            "pointInfo": { "ctrtDmndBizNm": "합성 접수", "wrtYmd": "20260927", "dmndInstBizPicNm": "합성 담당자",
                "depth1": "01001", "depth2": "01117", "depth3": "" },
            "tables": Value::Object(table),
        }),
        Some("01117"),
    );
    // 접수 화면은 이름과 무관하게 자동으로 수집하지 않는다(이름 판정보다 먼저 본다).
    for name in [me, ""] {
        let held = judge(&mut automatic, &tab(&request, plain), name);
        assert_eq!(
            (held.state.as_str(), held.message.as_str()),
            ("hold", "접수 화면은 자동으로 수집하지 않습니다.")
        );
    }
    assert_eq!(count(&c, "erp_capture"), 1);
    let matching_staff = [
        "합성 담당자(010-0000-0000)",
        " 합성 담당자 ( ☎ 000-0000-0000 ) ",
        "계약팀 합성 담당자 대리",
        "합성 담당자2",
    ];
    for staff in matching_staff {
        let snapshot = with_staff(&input, staff);
        assert_eq!(save(&mut automatic, &tab(&snapshot, plain), me), "stored");
        assert_eq!(
            text_of(&c, "SELECT staff FROM contract WHERE seq = '00'"),
            staff.trim()
        );
    }
    assert_eq!(count(&c, "erp_capture"), 1 + matching_staff.len() as i64);
    home.write_config(&h._temp.path("다른.pclm"), "other");
    assert_eq!(save(&mut automatic, &tab(&input, plain), me), "hold");
}

#[test]
fn 자동수집은_정상변경과_되돌아온값을_저장하고_누락행_빈칸_사람정정은_유지한다() {
    let h = Harness::new("mirror-auto-change");
    let home = Pointing::none();
    let c = h.write();
    let dataset = h.dataset();
    home.write_config(&h.path, &dataset);
    let now = 1_800_000_000_000_i64;
    let preferences = AutoCapturePreferences { enabled: true };
    let mut automatic = AutoCapture::new(&h.db, &home, &h.env);
    let with_staff_or_not = |input: &CaptureInput, include: bool| {
        with_data(input, |data| {
            if include {
                data["pointInfo"]["ctrtPicNm"] = json!(" 합성 담당자 ");
            } else {
                data["pointInfo"]
                    .as_object_mut()
                    .unwrap()
                    .remove("ctrtPicNm");
            }
        })
    };
    let tab = |input: &CaptureInput| {
        // 메뉴는 01579 로 못 박는다.
        let mut tab = auto_tab(input, &dataset, now, TabOptions::default());
        tab["menu"] = json!("01579");
        tab
    };
    let first = with_staff_or_not(
        &contract("00", "합성 A", "20260914", &[(1, 1, 1000), (2, 1, 1000)]),
        true,
    );
    assert_eq!(
        state_of(automatic.try_save(&tab(&first), &preferences, "합성 담당자", now, None)),
        "stored"
    );
    let second = with_staff_or_not(
        &contract("00", "합성 B", "20260914", &[(1, 1, 1000), (2, 1, 1000)]),
        true,
    );
    assert_eq!(
        state_of(automatic.try_save(&tab(&second), &preferences, "합성 담당자", now, None)),
        "stored"
    );
    assert_eq!(text_of(&c, "SELECT title FROM contract"), "합성 B");
    assert_eq!(
        state_of(automatic.try_save(&tab(&first), &preferences, "합성 담당자", now, None)),
        "stored"
    );
    assert_eq!(text_of(&c, "SELECT title FROM contract"), "합성 A");
    assert_eq!(count(&c, "erp_capture"), 3);
    automatic
        .try_save(&tab(&first), &preferences, "합성 담당자", now, None)
        .unwrap();
    assert_eq!(count(&c, "erp_capture"), 3);
    // DB의 담당자를 조건으로 보충하지 않는다.
    assert_eq!(
        state_of(automatic.try_save(
            &tab(&with_staff_or_not(&first, false)),
            &preferences,
            "합성 담당자",
            now,
            None
        )),
        "hold"
    );
    c.execute(
        "INSERT INTO field_override(entity_type,base,seq,column_name,value,original,updated_at) VALUES('contract','R26TA00000001','00','계약건명','사람 정정','합성 A','before')",
        [],
    )
    .unwrap();
    let partial = with_data(&first, |data| {
        data["pointInfo"]["ctrtNm"] = json!("");
        data["pointInfo"]["dmstUntyGrpNm"] = json!("추가 합성 기관");
        data["tables"] = json!({});
    });
    assert_eq!(
        state_of(automatic.try_save(&tab(&partial), &preferences, "합성 담당자", now, None)),
        "stored"
    );
    assert_eq!(count(&c, "contract_item"), 2);
    assert_eq!(text_of(&c, "SELECT title FROM contract"), "합성 A");
    assert_eq!(text_of(&c, "SELECT value FROM field_override"), "사람 정정");
    assert_eq!(
        text_of(&c, "SELECT demand_agency FROM contract"),
        "추가 합성 기관"
    );
}

// ── 투영 ───────────────────────────────────────────────

#[test]
fn 수집_안_함의_안내는_매핑에서_화면을_단_프로필을_읽는다() {
    // 창이 「수집하는 화면은 아래 …」 로 보이는 목록 — 따로 적어 두지 않고 매핑의 screen 에서 끌어낸다(ADR-037).
    let h = Harness::new("mirror-screens");
    let screen = |code: &str, entity_type: &str, kind: &str, name: &str| MirrorScreen {
        code: code.into(),
        entity_type: entity_type.into(),
        kind: kind.into(),
        name: name.into(),
    };
    assert_eq!(
        vec![
            screen("01117", "request", "접수", "계약요청접수상세조회-국방부"),
            screen("01112", "request", "접수", "계약요청접수상세조회-일반용역"),
            screen("01114", "request", "접수", "계약요청접수상세조회-물품"),
            screen("01179", "notice", "공고", "입찰공고작성[물품]"),
            screen("01181", "notice", "공고", "입찰공고작성[일반용역]"),
            screen("01196", "notice", "공고", "입찰공고상세"),
            screen("01579", "contract", "계약", "물품·일반용역 계약서"),
            screen("01608", "contract", "계약", "물품·일반용역 변경계약서 작성"),
        ],
        mirror::supported_screens(&h.db, &h.env)
    );
}

#[test]
fn 새_자료는_모든_칸을_새로_담길_것으로_비추고_작업자료에는_아무것도_남기지_않는다() {
    let h = Harness::new("mirror-new");
    let watcher = h.read();
    let before = footprint(&watcher);
    let shot = h
        .project(&contract(
            "00",
            "시험 음향설비 구매",
            "20260914",
            &[(1, 6, 2_000_000), (2, 1, 18_400_000)],
        ))
        .unwrap();

    assert_eq!(
        (
            shot.entity_type.as_str(),
            shot.kind.as_str(),
            shot.number.as_str(),
            shot.status.as_str()
        ),
        ("contract", "계약", "R26TA0000000100", "새 자료")
    );
    assert_eq!(shot.compare_seq, None);
    assert_eq!(shot.title, "시험 음향설비 구매");
    assert_eq!(shot.view, "v_계약");
    assert!(
        shot.fields
            .iter()
            .any(|f| f.column == "계약번호" && f.kind == "id" && f.value == "R26TA0000000100")
    );
    let date = field(&shot, "계약일자");
    assert_eq!(
        (date.kind.as_str(), date.value.as_str(), date.raw.clone()),
        ("new", "2026/09/14", None)
    ); // 화면이 이름표를 보내지 않았다.
    assert_eq!(field(&shot, "계약금액").value, "30,400,000");
    assert_eq!(field(&shot, "계약보증금률(%)").value, "5");
    assert_eq!(field(&shot, "계약보증금(원)").value, "8,094,520");
    // 사람이 채우는 열은 화면에서 오지 않는다.
    assert!(
        !shot
            .fields
            .iter()
            .any(|f| f.column == "진행상태" || f.column == "메모")
    );
    assert!(shot.rest > 0);
    assert_eq!(shot.view_columns, shot.fields.len() as i64 + shot.rest);
    let items: Vec<(&str, &str, &str, &str, &str)> = shot
        .items
        .iter()
        .map(|i| {
            (
                i.line.as_str(),
                i.quantity.as_str(),
                i.unit.as_str(),
                i.price.as_str(),
                i.amount.as_str(),
            )
        })
        .collect();
    assert_eq!(
        items,
        vec![
            ("1", "6", "세트", "2,000,000", "12,000,000"),
            ("2", "1", "세트", "18,400,000", "18,400,000")
        ]
    );
    assert!(shot.items_all_read);
    assert_eq!(shot.item_rows, 2);
    // 자리: 공고는 원천 참조만 있고 아직 수집되지 않았다. 접수는 아무것도 없다.
    let place: Vec<(&str, &str, &str)> = shot
        .place
        .iter()
        .map(|p| (p.kind.as_str(), p.state.as_str(), p.number.as_str()))
        .collect();
    assert_eq!(
        place,
        vec![
            ("접수", "missing", ""),
            ("공고", "ref", "R26BK00000001"),
            ("계약", "here", "R26TA0000000100")
        ]
    );
    assert!(!shot.place[1].collected);
    let rounds: Vec<(&str, &str)> = shot
        .rounds
        .iter()
        .map(|r| (r.seq.as_str(), r.state.as_str()))
        .collect();
    assert_eq!(rounds, vec![("00", "ghost")]);
    assert!(shot.missing.is_empty());
    assert_eq!((shot.kept_cells, shot.kept_in.as_str()), (0, ""));
    assert_eq!(
        (shot.totals_with_defaults, shot.totals_without_missing),
        (Some(true), Some(true))
    );

    assert_eq!(before, footprint(&watcher));
}

///
/// `NoticeGroups.현행공고` 가 비었다는 줄은 보지 않는다 — 그 SQL 은 크레이트 안(`pub(crate)`)이다.
/// 비추는 자리(`place`)가 같은 규칙 위에 서므로 아래 단언이 그것을 간접으로 지킨다.
#[test]
fn 현행이_미정이면_공고_후보를_모두_비추고_정해지면_그_공고만_비춘다() {
    let h = Harness::new("mirror-notice");
    let c = h.write();
    for number in ["R26BK00000001", "R26BK00000002"] {
        c.execute("INSERT INTO notice_group VALUES(?1)", params![number])
            .unwrap();
        c.execute("INSERT INTO notice_series VALUES(?1,?1)", params![number])
            .unwrap();
        c.execute(
            "INSERT INTO notice(notice_base,seq,title,updated_at) VALUES(?1,'000','시험 공고','2026-10-01')",
            params![number],
        )
        .unwrap();
        c.execute(
            "INSERT INTO notice_item(notice_base,seq,line_no,ref_request_base) VALUES(?1,'000',1,'RC001')",
            params![number],
        )
        .unwrap();
    }
    let input = contract("00", "시험 구매", "20261001", &[(1, 1, 1000)]);
    h.save(&input);
    let shot = h.project(&input).unwrap();
    let notice = single(shot.place.iter().filter(|p| p.kind == "공고"));
    assert_eq!(
        (notice.state.as_str(), notice.number.as_str()),
        ("linked", "R26BK00000002-000 · R26BK00000001-000")
    );
    Store::new(Database::new(&h.path, Access::Write), &h.env)
        .set_override(
            &EntityRef::new("notice", "R26BK00000002", "000"),
            "공고종류",
            "재공고",
        )
        .unwrap();
    let shot = h.project(&input).unwrap();
    assert_eq!(
        single(shot.place.iter().filter(|p| p.kind == "공고")).number,
        "R26BK00000002-000"
    );
}

#[test]
fn 새_차수는_가장_늦은_차수와_견주고_칸은_저장한_뒤의_계약면과_같은_표기다() {
    let h = Harness::new("mirror-next-round");
    h.save(&contract(
        "00",
        "시험 음향설비 구매",
        "20260914",
        &[(1, 6, 2_000_000), (2, 1, 18_400_000)],
    ));
    let watcher = h.read();
    let before = footprint(&watcher);
    let next = contract(
        "01",
        "시험 음향설비 구매",
        "20261002",
        &[(1, 8, 2_000_000), (2, 1, 18_400_000)],
    );
    let shot = h.project(&next).unwrap();

    assert_eq!(
        (
            shot.status.as_str(),
            shot.compare_seq.as_deref(),
            shot.latest_seq.as_deref()
        ),
        ("새 차수", Some("00"), Some("00"))
    );
    let date = field(&shot, "계약일자");
    assert_eq!(
        (date.kind.as_str(), date.value.as_str(), date.old.as_deref()),
        ("changed", "2026/10/02", Some("2026/09/14"))
    );
    let amount = field(&shot, "계약금액");
    assert_eq!(
        (
            amount.kind.as_str(),
            amount.value.as_str(),
            amount.old.as_deref()
        ),
        ("changed", "34,400,000", Some("30,400,000"))
    );
    assert_eq!(field(&shot, "계약건명").kind, "same");
    assert_eq!(shot.changes, 3); // 칸 둘 + 품목 한 줄
    let changed = single(shot.items.iter().filter(|i| i.before.is_some()));
    let before_item = changed.before.as_ref().unwrap();
    assert_eq!(
        (
            changed.line.as_str(),
            changed.quantity.as_str(),
            changed.amount.as_str(),
            before_item.quantity.as_str(),
            before_item.amount.as_str()
        ),
        ("1", "8", "16,000,000", "6", "12,000,000")
    );
    let rounds: Vec<(&str, &str)> = shot
        .rounds
        .iter()
        .map(|r| (r.seq.as_str(), r.state.as_str()))
        .collect();
    assert_eq!(rounds, vec![("00", "stored"), ("01", "ghost")]);
    assert_eq!(shot.rounds[0].amount, "30,400,000");
    assert_eq!(before, footprint(&watcher));

    // 실제로 저장한 뒤의 계약면과 칸마다 같은 표기다.
    h.save(&next);
    let c = h.read();
    let face = view_row(&c, "v_계약", "계약번호", "R26TA0000000101");
    for f in &shot.fields {
        assert_eq!(face[&f.column], f.value, "{}", f.column);
    }
    let mut statement = c
        .prepare("SELECT 순번, 수량, 금액 FROM v_계약품목 WHERE 계약번호='R26TA0000000101' ORDER BY 순번")
        .unwrap();
    let stored: Vec<(String, String, String)> = statement
        .query_map([], |r| {
            Ok((
                cell(r.get_ref(0)?),
                cell(r.get_ref(1)?),
                cell(r.get_ref(2)?),
            ))
        })
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    let projected: Vec<(String, String, String)> = shot
        .items
        .iter()
        .map(|i| (i.line.clone(), i.quantity.clone(), i.amount.clone()))
        .collect();
    assert_eq!(stored, projected);

    // 같은 화면을 다시 비추면 바뀌는 것이 없다.
    let again = h.project(&next).unwrap();
    assert_eq!(
        (
            again.status.as_str(),
            again.changes,
            again.compare_seq.as_deref()
        ),
        ("저장됨", 0, Some("01"))
    );
    assert!(
        again
            .fields
            .iter()
            .all(|f| f.kind == "id" || f.kind == "same")
    );
    let rounds: Vec<(&str, &str)> = again
        .rounds
        .iter()
        .map(|r| (r.seq.as_str(), r.state.as_str()))
        .collect();
    assert_eq!(rounds, vec![("00", "stored"), ("01", "current")]);
}

#[test]
fn 옛_차수를_다시_비추면_더_늦은_차수가_아니라_그_차수와_견준다() {
    let h = Harness::new("mirror-old-round");
    h.save(&contract(
        "00",
        "시험 음향설비 구매",
        "20260914",
        &[(1, 6, 2_000_000)],
    ));
    h.save(&contract(
        "01",
        "시험 음향설비 구매 변경",
        "20261002",
        &[(1, 8, 2_000_000)],
    ));
    let shot = h
        .project(&contract(
            "00",
            "시험 음향설비 구매",
            "20260914",
            &[(1, 6, 2_000_000)],
        ))
        .unwrap();
    assert_eq!(
        (
            shot.status.as_str(),
            shot.compare_seq.as_deref(),
            shot.latest_seq.as_deref()
        ),
        ("저장됨", Some("00"), Some("01"))
    );
    assert_eq!(shot.title, "시험 음향설비 구매");
    assert_eq!(shot.items.len(), 1);
    assert_eq!(shot.items[0].quantity, "6");
    let rounds: Vec<(&str, &str)> = shot
        .rounds
        .iter()
        .map(|r| (r.seq.as_str(), r.state.as_str()))
        .collect();
    assert_eq!(rounds, vec![("00", "current"), ("01", "stored")]);
}

#[test]
fn 사람이_고친_칸의_아래_값이_바뀌면_검토_대기로_비추고_창의_가져오기는_고른_칸만_복원한다() {
    let h = Harness::new("mirror-override");
    h.save(&contract(
        "00",
        "시험 음향설비 구매",
        "20260914",
        &[(1, 6, 2_000_000)],
    ));
    h.write()
        .execute_batch(
            "INSERT INTO field_override(entity_type, base, seq, column_name, value, original, updated_at) VALUES
                ('contract', 'R26TA00000001', '00', '계약건명', '사람이 고친 건명', '시험 음향설비 구매', 'before'),
                ('contract', 'R26TA00000001', '00', '수요기관', '사람이 고친 기관', '가온시험기관', 'before');",
        )
        .unwrap();
    let screen = contract("00", "화면에서 바뀐 건명", "20260914", &[(1, 6, 2_000_000)]);
    let shot = h.project(&screen).unwrap();
    assert_eq!(shot.status, "검토 대기");
    let title = single(shot.fields.iter().filter(|f| f.kind == "override"));
    assert_eq!(
        (
            title.column.as_str(),
            title.value.as_str(),
            title.human.as_deref()
        ),
        ("계약건명", "화면에서 바뀐 건명", Some("사람이 고친 건명"))
    );
    // 아래 값이 그대로인 덮개는 묻지 않는다.
    assert_eq!(field(&shot, "수요기관").kind, "same");

    // 쪽지가 다른 작업자료를 가리키면 창도 확장처럼 거절한다(ADR-032).
    let home = Pointing::none();
    let host = h.host().with_destination(&home);
    let preview = h.host().inspect(&screen).unwrap();
    assert_eq!(shot.base_token, preview.base_token);
    let choices = default_choices(&preview, &[title.choice_id.clone().unwrap()]);
    home.write_config(&h._temp.path("다른.pclm"), "ffff");
    reason(host.save(
        &screen,
        Some(&shot.base_token),
        Some(&h.env.new_id()),
        &choices,
    ));

    // 화면이나 자료가 바뀐 뒤의 baseToken 은 받지 않는다.
    home.write_config(&h.path, &h.dataset());
    let stale = reason(host.save(&screen, Some("낡은 토큰"), Some(&h.env.new_id()), &choices));
    assert!(stale.starts_with("미리보기 이후"), "{stale}");

    // 고른 칸만 덮개를 걷고, 같은 수집 ID 로 다시 보내면 같은 영수증이다.
    let id = h.env.new_id();
    let stored = host
        .save(&screen, Some(&shot.base_token), Some(&id), &choices)
        .unwrap();
    let replayed = host
        .save(&screen, Some(&shot.base_token), Some(&id), &choices)
        .unwrap();
    assert!(matches!(stored, SaveResult::Stored(_)));
    assert!(matches!(replayed, SaveResult::Replayed(_)));
    // 처음 저장은 다리 꼴(연결 칸이 camelCase)이고 재전송은 적어 둔
    // 글 그대로(PascalCase)라, 연결 칸의 이름 첫 글자만 맞춰 견준다 — 값은 그대로 견준다.
    let mut replayed_value = replayed.to_value();
    for link in replayed_value["links"].as_array_mut().unwrap() {
        let renamed: Map<String, Value> = link
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| {
                let mut chars = k.chars();
                let first = chars.next().unwrap().to_lowercase().collect::<String>();
                (first + chars.as_str(), v.clone())
            })
            .collect();
        *link = Value::Object(renamed);
    }
    assert_eq!(stored.to_value(), replayed_value);
    let read = h.read();
    let mut statement = read
        .prepare("SELECT column_name FROM field_override")
        .unwrap();
    let left: Vec<String> = statement
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(left, s(&["수요기관"]));
    assert_eq!(
        text_of(&read, "SELECT 계약건명 FROM v_계약"),
        "화면에서 바뀐 건명"
    );
    // 처음 저장 + 창의 가져오기 한 번
    assert_eq!(count(&read, "erp_capture"), 2);
}

#[test]
fn 바뀌는_값은_늘_적용하고_빈_값으로_지우는_변경은_고를_때만_비운다() {
    let h = Harness::new("mirror-wipe");
    h.save(&contract(
        "00",
        "시험 음향설비 구매",
        "20260914",
        &[(1, 6, 2_000_000)],
    ));
    let wiped = contract("00", "", "20261002", &[(1, 6, 2_000_000)]);
    let preview = h.host().inspect(&wiped).unwrap();
    let choices = default_choices(&preview, &[]);
    let title = single(preview.changes.iter().filter(|ch| ch.field == "title"));
    let date = single(
        preview
            .changes
            .iter()
            .filter(|ch| ch.field == "contracted_on"),
    );
    assert_eq!(
        (choice(&choices, &title.id), choice(&choices, &date.id)),
        ("keep", "apply")
    );
    // 다른 값으로 바뀌는 것은 고른 것과 상관없이 늘 적용한다.
    assert_eq!(
        choice(
            &default_choices(&preview, std::slice::from_ref(&date.id)),
            &date.id
        ),
        "apply"
    );

    // 유지하므로 남는 값을 보이고, 비우기를 고르는 자리를 단다.
    let shot = h.project(&wiped).unwrap();
    let blank = field(&shot, "계약건명");
    assert_eq!(
        (
            blank.kind.as_str(),
            blank.value.as_str(),
            blank.choice_id.as_deref()
        ),
        ("blank", "시험 음향설비 구매", Some(title.id.as_str()))
    );
    assert_eq!(field(&shot, "계약일자").kind, "changed");

    // 고르면 비운다 — 빈 값은 NULL 이 아니라 빈 문자열로 계약면에 선다.
    h.host()
        .save(
            &wiped,
            Some(&shot.base_token),
            Some(&h.env.new_id()),
            &default_choices(&preview, &[blank.choice_id.clone().unwrap()]),
        )
        .unwrap();
    let c = h.read();
    let (title, date): (String, String) = c
        .query_row("SELECT 계약건명, 계약일자 FROM v_계약", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!((title.as_str(), date.as_str()), ("", "2026/10/02"));
}

#[test]
fn 기본_선택은_고를_것이_있는_충돌마다_빠짐없이_하나씩_낸다() {
    let h = Harness::new("mirror-default-choices");
    h.save(&contract(
        "00",
        "시험 음향설비 구매",
        "20260914",
        &[(1, 6, 2_000_000), (2, 1, 18_400_000)],
    ));
    h.write()
        .execute_batch(
            "INSERT INTO field_override(entity_type, base, seq, column_name, value, original, updated_at) VALUES
                ('contract', 'R26TA00000001', '00', '계약건명', '사람이 고친 건명', '시험 음향설비 구매', 'before');",
        )
        .unwrap();
    // 건명이 바뀌고(덮개 아래), 날짜가 바뀌고, 품목 2가 빠진 화면.
    let screen = contract("00", "화면에서 바뀐 건명", "20261002", &[(1, 6, 2_000_000)]);
    let preview = h.host().inspect(&screen).unwrap();
    let conflicts: Vec<_> = preview.changes.iter().filter(|ch| ch.conflict).collect();
    assert!(conflicts.iter().any(|ch| ch.field == "__override"));
    assert!(conflicts.iter().any(|ch| ch.field == "__row"));
    let none = default_choices(&preview, &[]);
    let mut expected: Vec<&str> = conflicts.iter().map(|ch| ch.id.as_str()).collect();
    expected.sort_unstable();
    let mut got: Vec<&str> = none.iter().map(|(k, _)| k.as_str()).collect();
    got.sort_unstable();
    assert_eq!(expected, got);
    let ids: Vec<String> = conflicts.iter().map(|ch| ch.id.clone()).collect();
    let all = default_choices(&preview, &ids);
    for change in conflicts {
        let opt_in =
            change.field == "__override" || change.field == "__row" || mirror::wipes(change);
        assert_eq!(
            choice(&none, &change.id),
            if opt_in { "keep" } else { "apply" }
        );
        assert_eq!(choice(&all, &change.id), "apply");
    }
}

#[test]
fn 화면에_없는_행은_그대로_두는_것이_기본이고_고르면_지우며_계약_총액이_맞는지_미리_알린다() {
    let h = Harness::new("mirror-missing");
    h.save(&contract(
        "00",
        "시험 음향설비 구매",
        "20260914",
        &[(1, 6, 2_000_000), (2, 1, 18_400_000)],
    ));
    // 품목 하나가 빠진 화면 — 머리 총액은 화면의 품목에서 셈하므로 남은 행과 맞지 않는다.
    let screen = contract("00", "시험 음향설비 구매", "20260914", &[(1, 6, 2_000_000)]);
    let shot = h.project(&screen).unwrap();
    assert_eq!(shot.missing.len(), 1);
    let gone = &shot.missing[0];
    assert_eq!(
        (
            gone.table.as_str(),
            gone.table_name.as_str(),
            gone.line,
            gone.label.as_str(),
            gone.amount.as_str()
        ),
        ("contract_item", "품목", 2, "시험 품목 2", "18,400,000")
    );
    assert_eq!(shot.items.len(), 2); // 그대로 두므로 품목 전체에는 남는다.
    assert_eq!(
        (shot.totals_with_defaults, shot.totals_without_missing),
        (Some(false), Some(true))
    );

    // 기본으로는 저장이 총액을 거절한다 — 투영이 미리 말한 그대로.
    let host = h.host();
    let (input, preview) = mirror::prepare(&host, &screen).unwrap();
    assert_eq!(shot.base_token, preview.base_token);
    let refused = reason(host.save(
        &input,
        Some(&preview.base_token),
        Some(&h.env.new_id()),
        &default_choices(&preview, &[]),
    ));
    assert!(
        refused.starts_with("선택한 품목 금액과 계약 총액이 다릅니다"),
        "{refused}"
    );

    // 지우기를 고르면 그 행과 추적을 지운다.
    host.save(
        &input,
        Some(&preview.base_token),
        Some(&h.env.new_id()),
        &default_choices(&preview, std::slice::from_ref(&gone.choice_id)),
    )
    .unwrap();
    let c = h.read();
    let mut statement = c.prepare("SELECT line_no FROM contract_item").unwrap();
    let lines: Vec<i64> = statement
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(lines, vec![1]);
    assert_eq!(
        c.query_row(
            "SELECT count(*) FROM erp_row WHERE table_name='contract_item'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
}

#[test]
fn 작업자료의_줄과_짝이_없는_화면의_줄은_새_줄로_가져온다() {
    let h = Harness::new("mirror-new-rows");
    h.save(&contract(
        "00",
        "시험 음향설비 구매",
        "20260914",
        &[(1, 6, 2_000_000)],
    ));
    // 같은 차수에 품목 줄이 하나 더 선 화면 — 예전에는 옛 줄과 짝지으라며 창에서 막았다.
    let screen = contract(
        "00",
        "시험 음향설비 구매",
        "20260914",
        &[(1, 6, 2_000_000), (2, 1, 18_400_000)],
    );
    assert!(!h.host().inspect(&screen).unwrap().unmatched.is_empty());

    let shot = h.project(&screen).unwrap();
    let added: Vec<(&str, bool)> = shot
        .items
        .iter()
        .map(|i| (i.line.as_str(), i.added))
        .collect();
    assert_eq!(added, vec![("1", false), ("2", true)]);
    assert_eq!(shot.totals_with_defaults, Some(true));

    // 가져오기는 같은 입력을 지어 같은 토큰을 얻고, 막히지 않고 저장한다.
    let host = h.host();
    let (input, preview) = mirror::prepare(&host, &screen).unwrap();
    assert!(preview.unmatched.is_empty());
    assert_eq!(shot.base_token, preview.base_token);
    host.save(
        &input,
        Some(&preview.base_token),
        Some(&h.env.new_id()),
        &default_choices(&preview, &[]),
    )
    .unwrap();
    let c = h.read();
    let mut statement = c
        .prepare("SELECT CAST(순번 AS TEXT), 금액 FROM v_계약품목 ORDER BY 순번")
        .unwrap();
    let rows: Vec<(String, String)> = statement
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(
        rows,
        vec![
            ("1".to_string(), "12,000,000".to_string()),
            ("2".to_string(), "18,400,000".to_string())
        ]
    );
}

#[test]
fn 수집_규칙에_없는_화면은_투영하지_못하고_까닭을_던진다() {
    let h = Harness::new("mirror-unknown");
    let input = CaptureInput {
        profile: Some("g2b-unknown".into()),
        ..contract("00", "시험", "20260914", &[])
    };
    reason(h.project(&input));
    reason(h.project(&CaptureInput {
        mapping_revision: Some("옛 매핑".into()),
        ..contract("00", "시험", "20260914", &[])
    }));
}

// ── 화면 이름표 잇기 ─────────────────────────────────────

fn references(entity_type: &str) -> Vec<(String, Vec<String>)> {
    views::face_references(entity_type)
}

fn refs<'a>(all: &'a [(String, Vec<String>)], column: &str) -> Vec<&'a str> {
    all.iter()
        .find(|(c, _)| c == column)
        .unwrap_or_else(|| panic!("{column} 열이 없다"))
        .1
        .iter()
        .map(String::as_str)
        .collect()
}

#[test]
fn 면_열의_재료는_열_정의에서_끌어낸다() {
    let contract = references("contract");
    assert_eq!(refs(&contract, "계약건명"), ["contract.title"]);
    assert_eq!(refs(&contract, "계약일자"), ["contract.contracted_on"]);
    assert_eq!(refs(&contract, "계약금액"), ["contract.amount"]);
    // 바깥 LEFT JOIN 의 별칭은 풀리지 않아 세지 않는다.
    assert_eq!(refs(&contract, "계약상대자"), ["erp_partner.name"]);
    // 키 열은 덮개로 감싸지 않는다.
    assert!(!contract.iter().any(|(c, _)| c == "계약번호"));

    let notice = references("notice");
    assert_eq!(refs(&notice, "공고명"), ["notice.title"]);
    assert_eq!(refs(&notice, "게시일시"), ["notice.posted_at"]);
    assert_eq!(refs(&notice, "추정가격"), ["notice.estimated_price"]);
    // 품목 줄을 잇는 조건은 재료가 아니다.
    assert_eq!(refs(&notice, "세부품명"), ["notice_item.item_name"]);
    assert_eq!(refs(&notice, "계약방법"), ["notice.contract_method"]);
    assert_eq!(refs(&notice, "계약구분"), ["notice.contract_kind"]);
    assert_eq!(refs(&notice, "개찰일시"), ["notice_schedule.starts_at"]);

    let request = references("request");
    assert_eq!(refs(&request, "요청명"), ["request.title"]);
    assert_eq!(refs(&request, "접수일자"), ["request.received_on"]);
    assert_eq!(refs(&request, "품대"), ["request.goods_amount"]);
    assert!(references("plan").is_empty());
}

fn screen_row(group: &str, label: &str, text: &str, source: &str) -> ScreenRow {
    ScreenRow {
        group: group.into(),
        label: label.into(),
        text: text.into(),
        source: source.into(),
    }
}

#[test]
fn 칸마다_채우는_화면의_자리와_이름표를_잇고_화면_표기는_보이는_글이_다를_때만_곁에_둔다() {
    let h = Harness::new("mirror-sources");
    h.save(&contract(
        "00",
        "시험 음향설비 구매",
        "20260914",
        &[(1, 6, 2_000_000)],
    ));
    let screen = vec![
        screen_row("계약 기본정보", "계약번호", "R26TA00000001-01", "ctrtNoOrd"),
        screen_row("계약 기본정보", "계약명", "시험 음향설비 구매", "ctrtNm"),
        screen_row("계약 기본정보", "계약일자", "2026-10-02", "ctrtDt"),
        screen_row("계약 기본정보", "공고번호", "R26BK00000001", "bidPbancNo"),
        screen_row(
            "품목내역",
            "1 시험 품목 1",
            "8세트 · 16000000",
            "table:contract_item:1",
        ),
        screen_row(
            "품목내역",
            "2 시험 품목 2",
            "1세트 · 18400000",
            "table:contract_item:2",
        ),
    ];
    let shot = h
        .project_with(
            &contract(
                "01",
                "시험 음향설비 구매",
                "20261002",
                &[(1, 8, 2_000_000), (2, 1, 18_400_000)],
            ),
            &screen,
        )
        .unwrap();

    let date = field(&shot, "계약일자");
    assert_eq!(date.sources, s(&["ctrtDt"]));
    assert_eq!(
        (date.from.as_str(), date.raw.as_deref()),
        ("계약일자", Some("2026-10-02"))
    );
    let title = field(&shot, "계약건명");
    // 보이는 글이 표기와 같으면 곁에 두지 않는다.
    assert_eq!((title.from.as_str(), title.raw.clone()), ("계약명", None));
    let number = field(&shot, "계약번호");
    assert_eq!(number.sources, s(&["ctrtNoOrd"]));
    assert_eq!(number.from, "계약번호");
    // 품목에서 셈하는 총액은 화면의 한 칸에서 오지 않는다 — 대응을 지어내지 않는다.
    let amount = field(&shot, "계약금액");
    assert_eq!(
        (
            amount.sources.len(),
            amount.from.as_str(),
            amount.raw.clone()
        ),
        (0, "", None)
    );
    let sources: Vec<(&str, &str)> = shot
        .items
        .iter()
        .map(|i| (i.line.as_str(), i.source.as_str()))
        .collect();
    assert_eq!(
        sources,
        vec![
            ("1", "table:contract_item:1"),
            ("2", "table:contract_item:2")
        ]
    );
    let notice = single(shot.place.iter().filter(|p| p.kind == "공고"));
    assert_eq!(
        (notice.state.as_str(), notice.source.as_deref()),
        ("ref", Some("bidPbancNo"))
    );
    assert_eq!(
        single(shot.place.iter().filter(|p| p.kind == "계약")).source,
        None
    );

    // 화면 줄을 함께 실은 탭 보고에서 읽는다. 표 자리는 그 표의 모든 줄을 가리킨다.
    let tab = json!({
        "tabId": 1, "state": "supported",
        "screenRows": screen.iter().map(ScreenRow::to_value).collect::<Vec<_>>(),
    });
    assert_eq!(mirror::screen_rows(&tab), screen);
    assert_eq!(mirror::export(&tab), None); // 내보내기를 부르지 않은 탭은 그 결과를 싣지 않는다.
    assert!(mirror::points(
        "table:contract_item",
        "table:contract_item:2"
    ));
    assert!(!mirror::points(
        "table:contract_item",
        "table:contract_item_x:2"
    ));
    assert!(!mirror::points("", ""));
}

/// 탭 보고가 홈의 파일(`ExtensionRelay`)로 건너오는 길은 옮기지 않았다 — 탭 하나의 `export` 를 읽는 모양 검사만 옮긴다.
#[test]
fn 앱이_부른_내보내기의_결과는_탭_보고에_실려_그대로_건너오고_모양이_틀리면_모르는_것으로_둔다() {
    let tab = json!({ "tabId": 1, "state": "unsupported",
        "export": { "at": 1_700_000_000_123_i64, "ok": false, "message": "표를 찾지 못했습니다." } });
    assert_eq!(
        mirror::export(&tab),
        Some(MirrorExport {
            at: 1_700_000_000_123,
            ok: false,
            message: "표를 찾지 못했습니다.".into()
        })
    );
    assert_eq!(
        mirror::export(&json!({ "export": { "at": 5, "ok": true } })),
        Some(MirrorExport {
            at: 5,
            ok: true,
            message: String::new()
        })
    );
    // 모양이 틀리면 결과를 모르는 것이다 — 성공으로 읽지 않는다.
    assert_eq!(
        mirror::export(&json!({ "export": { "at": "어제", "ok": true } })),
        None
    );
    assert_eq!(
        mirror::export(&json!({ "export": { "at": 5, "ok": "예" } })),
        None
    );
    assert_eq!(mirror::export(&json!({ "export": "성공" })), None);
}

// ── 수집하는 화면 ─────────────────────────────────────────────

struct ScreensHarness {
    h: Harness,
    dataset: String,
}

impl ScreensHarness {
    fn new() -> Self {
        let h = Harness::new("screens");
        let dataset = h.dataset();
        ScreensHarness { h, dataset }
    }

    fn host(&self) -> ErpCapture<'_> {
        self.h.host().with_environment("development")
    }

    fn inspect(&self, input: &CaptureInput) -> Result<CapturePreview> {
        self.host().inspect(input)
    }

    fn save(&self, input: &CaptureInput, token: &str) -> Result<SaveResult> {
        self.host()
            .save(input, Some(token), Some(&self.h.env.new_id()), &Vec::new())
    }
}

fn point(depths: &[(&str, &str)]) -> Value {
    Value::Object(
        depths
            .iter()
            .map(|(k, v)| (k.to_string(), json!(v)))
            .collect(),
    )
}

/// 접수 목록형 파일. 메뉴 경로(depth)만 갈아 끼운다.
fn request_file(depths: &[(&str, &str)], profile: &str) -> CaptureInput {
    let mut point = Map::new();
    point.insert("ctrtDmndBizNm".into(), json!("시험 구매"));
    point.insert("wrtYmd".into(), json!("20260927"));
    point.insert("totlPymtAmt".into(), json!("0"));
    for (k, v) in depths {
        point.insert(k.to_string(), json!(v));
    }
    let mut tables = Map::new();
    tables.insert(
        "mf_wfm_container_gridView".into(),
        json!([{ "ctrtDmndRcptNo": "RC001", "ctrtDmndRcptOrd": "000", "ctrtDmndRcptItemSqno": "001",
            "ndfsPrcmDmndNo": "REQ1", "ctrtDmndQty": "1" }]),
    );
    file_input(
        profile,
        json!({ "pointInfo": Value::Object(point), "tables": Value::Object(tables) }),
    )
}

const REQUEST_PAGE: [(&str, &str); 3] = [("depth1", "01001"), ("depth2", "01117"), ("depth3", "")];

/// 공고 실제 화면. 확장이 읽은 메뉴 번호를 싣는다.
fn notice_live(screen: Option<&str>, profile: &str) -> CaptureInput {
    live(
        profile,
        json!({
            "pointInfo": { "bidPbancNo": "R26BK00000001", "bidPbancOrd": "001", "bidPbancNm": "시험 공고" },
            "tables": {},
        }),
        screen,
    )
}

/// 계약 실제 화면.
fn contract_live(screen: Option<&str>) -> CaptureInput {
    live(
        "g2b-contract-v1",
        json!({
            "pointInfo": { "ctrtNoOrd": "R26TA00000001-00", "ctrtNm": "시험 계약" },
            "tables": {},
        }),
        screen,
    )
}

/// 변경계약서 작성 실제 화면(메뉴 01608). 번호·차수는 머리에 없고 품목 표에만 있다. 업체는 바뀌기 전 표와 지금 표 둘로
/// 온다. 품목 표를 내려받기용으로 한 벌 더 싣는다.
fn change_live(profile: &str) -> CaptureInput {
    let items = json!([{ "ctrtNo": "R26TA00000005", "ctrtChgOrd": "01", "ctrtItemSqno": 1, "ctrtItemNm": "시험 품목",
        "ctrtQty": 2, "ctrtUprc": 1000, "ctrtAmt": 2000 }]);
    let mut tables = Map::new();
    tables.insert("mf_wfm_container_grdCtrtLis".into(), items.clone());
    tables.insert("mf_wfm_container_grdCtrtLisExcel".into(), items);
    tables.insert(
        "mf_wfm_container_grdBeoeEtpsLst".into(),
        json!([{ "ctrtNo": "R26TA00000005", "ctrtChgOrd": "00", "ctentUntyGrpNo": "OLD0001", "ctentUntyGrpNm": "바뀌기 전 업체" }]),
    );
    tables.insert(
        "mf_wfm_container_grdCrntEtpsLst".into(),
        json!([{ "ctrtNo": "R26TA00000005", "ctrtChgOrd": "01", "ctentUntyGrpNo": "NEW0001", "ctentUntyGrpNm": "시험 업체" }]),
    );
    live(
        profile,
        json!({
            "pointInfo": { "ctrtNo": "R26TA00000005", "ctrtNm": "시험 변경계약", "ctrtMthdNm": "수의계약" },
            "tables": Value::Object(tables),
        }),
        Some("01608"),
    )
}

/// 매핑 글을 노드로 — 기본 매핑의 꼴 그대로.
fn defaults_node() -> Value {
    mapping::defaults().to_value()
}

fn profile_node<'a>(node: &'a mut Value, id: &str) -> &'a mut Value {
    node["profiles"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|p| p["id"] == id)
        .unwrap()
}

fn node_text(node: &Value) -> String {
    serde_json::to_string(node).unwrap()
}

fn default_profile(id: &str) -> MappingProfile {
    mapping::defaults()
        .profiles_iter()
        .find(|p| p.id() == id)
        .unwrap()
        .clone()
}

fn profile_hash(profile: &MappingProfile) -> String {
    mapping::hash(&profile.to_value())
}

fn insert_old_mapping(h: &Harness, node: &Value) {
    h.write()
        .execute(
            "INSERT INTO erp_mapping(revision,json,active,validated,created_at) VALUES('old',?1,1,1,'2026-01-01T00:00:00Z')",
            [node_text(node)],
        )
        .unwrap();
}

#[test]
fn 읽지_못하는_매핑_글은_한국어_사유로_거절한다() {
    // 매핑 편집기의 저장·샘플 검증·활성화가 영어 예외 글 대신 이 사유를 본다.
    const BROKEN: &str = "매핑 JSON 형식이 올바르지 않습니다.";
    assert_eq!(
        BROKEN,
        reason(mapping::parse("{\"version\":1,\"profiles\":["))
    );
    assert_eq!(BROKEN, reason(mapping::parse("")));
    assert_eq!(BROKEN, reason(mapping::parse("[]")));
    assert_eq!(
        BROKEN,
        reason(mapping::parse(&format!(
            "{}{}",
            "[".repeat(70),
            "]".repeat(70)
        )))
    );
    assert_eq!("매핑이 비어 있습니다.", reason(mapping::parse("null")));

    let json = node_text(&defaults_node());
    // 같은 키가 두 번 든 글은 읽을 수 없는 글이다 — 뒤 값을 조용히 고르지 않는다.
    let doubled = format!("{{\"version\":1,{}", &json[1..]);
    assert_eq!(BROKEN, reason(mapping::parse(&doubled)));
    assert_eq!(BROKEN, reason(mapping::read(&doubled)));

    let mut node = defaults_node();
    node["profiles"] = json!({});
    assert_eq!(
        "매핑의 ‘profiles’ 값 형식이 올바르지 않습니다.",
        reason(mapping::parse(&node_text(&node)))
    );

    let mut node = defaults_node();
    node["profiles"][0]["extra"] = json!(1);
    assert_eq!(
        "매핑에 모르는 칸이 있습니다: extra",
        reason(mapping::parse(&node_text(&node)))
    );
}

#[test]
fn 허용_목록은_기본_매핑에서_화면을_단_여덟_프로필이고_화면마다_프로필_하나다() {
    let defaults = mapping::defaults();
    let mut listed: Vec<(String, String)> = screens::supported(&defaults)
        .map(|p| {
            (
                p.screen.as_ref().unwrap().code().to_string(),
                p.id().to_string(),
            )
        })
        .collect();
    listed.sort();
    let expected: Vec<(String, String)> = [
        ("01112", "g2b-request-b-v1"),
        ("01114", "g2b-request-goods-v1"),
        ("01117", "g2b-request-v1"),
        ("01179", "g2b-notice-a-v1"),
        ("01181", "g2b-notice-a-service-v1"),
        ("01196", "g2b-notice-b-v1"),
        ("01579", "g2b-contract-v1"),
        ("01608", "g2b-contract-change-v1"),
    ]
    .iter()
    .map(|(a, b)| (a.to_string(), b.to_string()))
    .collect();
    assert_eq!(expected, listed);
    // 번호는 숫자 다섯이고 겹치지 않으며, 이름이 있다.
    for p in screens::supported(&defaults) {
        let screen = p.screen.as_ref().unwrap();
        assert!(
            screens::is_code(Some(screen.code())) && !screen.name().is_empty(),
            "{}",
            p.id()
        );
    }
    let mut codes: Vec<&str> = screens::supported(&defaults)
        .map(|p| p.screen.as_ref().unwrap().code())
        .collect();
    codes.sort_unstable();
    codes.dedup();
    assert_eq!(8, codes.len());
    assert_eq!(
        Some("g2b-contract-v1"),
        screens::profile_for(&defaults, Some("01579")).map(|p| p.id())
    );
    assert!(screens::profile_for(&defaults, Some("01175")).is_none());
    assert!(screens::profile_for(&defaults, None).is_none());
}

#[test]
fn 화면과_고유키는_편집본이_바꿀_수_없다() {
    let edit = |change: &dyn Fn(&mut MappingProfile)| -> MappingSet {
        let mut set = mapping::defaults();
        for p in set.profiles.as_mut().unwrap().iter_mut().flatten() {
            if p.id() == "g2b-notice-a-v1" {
                change(p);
            }
        }
        set
    };
    const FROZEN: &str = "화면 번호와 고유키는 바꿀 수 없습니다.";
    let screen = |code: &str, name: &str| {
        Some(MappingScreen {
            code: Some(code.into()),
            name: Some(name.into()),
        })
    };
    assert_eq!(
        FROZEN,
        reason(mapping::validate(&edit(
            &|p| p.screen = screen("01175", "개찰 결과")
        )))
    );
    assert_eq!(
        FROZEN,
        reason(mapping::validate(&edit(&|p| p.screen = None)))
    );
    assert_eq!(
        FROZEN,
        reason(mapping::validate(&edit(
            &|p| p.base_field = Some("bidPbancText".into())
        )))
    );
    assert_eq!(
        FROZEN,
        reason(mapping::validate(&edit(
            &|p| p.seq_field = Some("bidPbancText".into())
        )))
    );
    assert_eq!(
        FROZEN,
        reason(mapping::validate(&edit(
            &|p| p.identity_source = Some("mf_wfm_container_gridView".into())
        )))
    );
    // 받지 않는 프로필에 화면을 다는 것도 목록을 넓히는 일이라 편집본으로는 하지 않는다.
    let mut widened = mapping::defaults();
    for p in widened.profiles.as_mut().unwrap().iter_mut().flatten() {
        if p.id() == "g2b-public-notice-header-v1" {
            p.screen = screen("01188", "공고");
        }
    }
    assert_eq!(FROZEN, reason(mapping::validate(&widened)));
    mapping::validate(&edit(&|p| p.heading = Some("다른 제목".into()))).unwrap();
}

#[test]
fn 옛_매핑은_기본의_화면과_현재_이름을_얻고_고유키는_얻지_않는다() {
    let mut node = defaults_node();
    for profile in node["profiles"].as_array_mut().unwrap() {
        profile.as_object_mut().unwrap().remove("screen");
    }
    let parsed = mapping::parse(&node_text(&node)).unwrap();
    assert_eq!(
        mapping::revision(&mapping::defaults()),
        mapping::revision(&parsed)
    );

    // 고유키가 기본과 다른 옛 편집본은 조용히 고쳐 쓰지 않고 거절한다.
    node["profiles"][0]["baseField"] = json!("ctrtDmndRcptNoX");
    assert_eq!(
        "화면 번호와 고유키는 바꿀 수 없습니다.",
        reason(mapping::parse(&node_text(&node)))
    );

    let mut node = defaults_node();
    for (id, previous) in [
        ("g2b-request-v1", "조달요구 접수(목록형)"),
        ("g2b-request-b-v1", "조달요구 접수 상세(일반용역)"),
        ("g2b-notice-a-v1", "입찰공고 상세"),
        ("g2b-notice-a-service-v1", "입찰공고 상세(일반용역)"),
        ("g2b-notice-b-v1", "입찰공고상세조회(통합)"),
        ("g2b-contract-v1", "계약 상세"),
    ] {
        profile_node(&mut node, id)["screen"]["name"] = json!(previous);
    }
    assert_eq!(
        mapping::revision(&mapping::defaults()),
        mapping::revision(&mapping::parse(&node_text(&node)).unwrap())
    );
    let h = Harness::new("screen-names");
    profile_node(&mut node, "g2b-notice-a-v1")["heading"] = json!("고친 제목");
    insert_old_mapping(&h, &node);
    let active = MappingStore::new(&h.db, &h.env).active().unwrap();
    assert_eq!(
        Some("고친 제목"),
        active
            .profile("g2b-notice-a-v1")
            .unwrap()
            .heading
            .as_deref()
    );
    for p in screens::supported(&active) {
        assert_eq!(
            mapping::defaults().profile(p.id()).unwrap().screen,
            p.screen
        );
    }

    // 이전 이름이어도 번호가 다르거나 이름이 임의 값이면 고쳐서 받아들이지 않는다.
    profile_node(&mut node, "g2b-notice-a-v1")["screen"]["code"] = json!("01175");
    assert_eq!(
        "화면 번호와 고유키는 바꿀 수 없습니다.",
        reason(mapping::read(&node_text(&node)))
    );
    profile_node(&mut node, "g2b-notice-a-v1")["screen"] =
        json!({"code": "01179", "name": "임의 화면"});
    assert_eq!(
        "화면 번호와 고유키는 바꿀 수 없습니다.",
        reason(mapping::parse(&node_text(&node)))
    );
}

#[test]
fn 프로필이_더해지기_전에_저장한_매핑은_빠진_프로필을_기본에서_얻고_고친_프로필은_그대로다() {
    // 프로필 수가 기본과 다르면 검증이 거절한다 — 채우지 않으면 작업자료에 저장·활성화한 편집본을 읽는 수집이 통째로 멈춘다.
    let h = Harness::new("screens-added");
    const ADDED: &str = "g2b-contract-change-v1";
    let mut node = defaults_node();
    node["profiles"]
        .as_array_mut()
        .unwrap()
        .retain(|p| p["id"] != ADDED);
    assert_eq!(
        mapping::revision(&mapping::defaults()),
        mapping::revision(&mapping::parse(&node_text(&node)).unwrap())
    );

    // 고친 다른 프로필은 고친 그대로 남고, 빠진 것만 기본의 것으로 선다.
    profile_node(&mut node, "g2b-notice-a-v1")["heading"] = json!("다른 제목");
    insert_old_mapping(&h, &node);
    let active = MappingStore::new(&h.db, &h.env).active().unwrap();
    let find = |id: &str| active.profiles_iter().find(|p| p.id() == id).unwrap();
    assert_eq!(
        Some("다른 제목"),
        find("g2b-notice-a-v1").heading.as_deref()
    );
    assert_eq!(
        profile_hash(&default_profile(ADDED)),
        profile_hash(find(ADDED))
    );
    assert_eq!(
        Some("01608"),
        screens::profile_for(&active, Some("01608")).map(|p| p.screen.as_ref().unwrap().code())
    );

    // 모르는 프로필은 채우지 않는다 — 거절은 그대로다.
    let mut unknown = node["profiles"][0].clone();
    unknown["id"] = json!("g2b-unknown-v1");
    node["profiles"].as_array_mut().unwrap().push(unknown);
    assert_eq!(
        "지원하는 매핑 판·프로필 구성을 확인하세요.",
        reason(mapping::parse(&node_text(&node)))
    );
}

#[test]
fn 기본에서_거둔_필드가_남은_편집본은_읽을_때_그_필드만_떨구고_새로_저장하는_길은_거절한다() {
    // 뜻이 확정되지 않아 기본 매핑에서 뺀 필드(ADR-043)가 거두기 전에 활성화한 편집본에 남아 있다 — 머리 필드·표 필드·그것만 걸던 선택자.
    let h = Harness::new("screens-retired");
    const CONTRACT: &str = "g2b-contract-v1";
    let mut node = defaults_node();
    let profile = profile_node(&mut node, CONTRACT);
    profile["fields"]
        .as_array_mut()
        .unwrap()
        .push(json!({ "source": "ctrtDvofNm", "target": "contract_officer", "transform": "text" }));
    profile["tables"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|t| t["target"] == "contract_item")
        .unwrap()["fields"]
        .as_array_mut()
        .unwrap()
        .push(json!({ "source": "spplRgnNm", "target": "region", "transform": "text" }));
    profile["selectors"] = json!({ "ctrtDvofNm": "#officer" });
    profile_node(&mut node, "g2b-notice-a-v1")["heading"] = json!("다른 제목");
    insert_old_mapping(&h, &node);

    let active = MappingStore::new(&h.db, &h.env).active().unwrap();
    let find = |id: &str| active.profiles_iter().find(|p| p.id() == id).unwrap();
    assert_eq!(
        profile_hash(&default_profile(CONTRACT)),
        profile_hash(find(CONTRACT))
    );
    assert_eq!(
        Some("다른 제목"),
        find("g2b-notice-a-v1").heading.as_deref()
    );

    // 사람이 지금 저장하는 편집본은 말없이 고쳐 쓰지 않는다 — 기본에 없는 대상은 거절이다.
    assert_eq!(
        "허용되지 않은 대상·변환입니다: contract_officer",
        reason(mapping::parse(&node_text(&node)))
    );
}

#[test]
fn 메뉴_경로에서_비지_않은_가장_깊은_단을_화면의_번호로_본다() {
    assert_eq!(
        screens::leaf(&point(&[
            ("depth1", "01570"),
            ("depth2", "01571"),
            ("depth3", "01579")
        ]))
        .as_deref(),
        Some("01579")
    );
    assert_eq!(
        screens::leaf(&point(&[
            ("depth1", "01001"),
            ("depth2", "01117"),
            ("depth3", "")
        ]))
        .as_deref(),
        Some("01117")
    );
    assert_eq!(
        screens::leaf(&point(&[("depth1", "01001")])).as_deref(),
        Some("01001")
    );
    assert_eq!(
        screens::leaf(&point(&[("ctrtNoOrd", "R26TA00000001-00")])),
        None
    );
    assert_eq!(
        screens::leaf(&point(&[("depth1", "01001"), ("depth2", "메뉴")])),
        None
    );
    assert_eq!(screens::leaf(&json!("pointInfo")), None);
}

#[test]
fn 파일은_번호가_없거나_목록에_없거나_그_화면의_프로필이_아니면_거절한다() {
    let t = ScreensHarness::new();
    const NONE: &str = "이 JSON 에는 화면 번호(depth)가 없어 받지 않습니다.";
    assert_eq!(
        NONE,
        reason(t.inspect(&request_file(&[], "g2b-request-v1")))
    );
    assert_eq!(
        NONE,
        reason(t.inspect(&request_file(&[("depth1", "메뉴")], "g2b-request-v1")))
    );
    assert_eq!(
        "메뉴 01175 화면은 수집하지 않습니다.",
        reason(t.inspect(&request_file(
            &[("depth1", "01001"), ("depth2", "01175")],
            "g2b-request-v1"
        )))
    );
    let contract = request_file(
        &[
            ("depth1", "01570"),
            ("depth2", "01571"),
            ("depth3", "01579"),
        ],
        "g2b-request-v1",
    );
    assert_eq!(
        "계약 화면(메뉴 01579)의 JSON 입니다. 계약 프로필(g2b-contract-v1)로 여세요.",
        reason(t.inspect(&contract))
    );
    // 같은 종류의 다른 프로필도 받지 않는다 — 화면 하나에 프로필 하나다.
    assert_eq!(
        "접수 화면(메뉴 01117)의 JSON 입니다. 접수 프로필(g2b-request-v1)로 여세요.",
        reason(t.inspect(&request_file(&REQUEST_PAGE, "g2b-request-b-v1")))
    );

    assert_eq!(
        "RC001-000",
        t.inspect(&request_file(&REQUEST_PAGE, "g2b-request-v1"))
            .unwrap()
            .entity
    );
}

#[test]
fn 실제_화면은_확장이_읽은_번호로만_가리고_저장과_창의_투영도_같은_문을_지난다() {
    let t = ScreensHarness::new();
    const UNREAD: &str = "화면 번호를 읽지 못했습니다. 접수·공고·계약서 화면에서 다시 읽어 주세요.";
    assert_eq!(
        UNREAD,
        reason(t.inspect(&notice_live(None, "g2b-notice-a-v1")))
    );
    assert_eq!(
        UNREAD,
        reason(t.inspect(&notice_live(Some("메뉴 아님"), "g2b-notice-a-v1")))
    );
    assert_eq!(
        "메뉴 01175 화면은 수집하지 않습니다.",
        reason(t.inspect(&notice_live(Some("01175"), "g2b-notice-a-v1")))
    );
    assert_eq!(
        "메뉴 01188 화면은 수집하지 않습니다.",
        reason(t.inspect(&notice_live(Some("01188"), "g2b-notice-b-v1")))
    );
    assert_eq!(
        "계약 화면(메뉴 01579)인데 다른 프로필로 읽혔습니다. 화면을 다시 읽어 주세요.",
        reason(t.inspect(&notice_live(Some("01579"), "g2b-notice-a-v1")))
    );
    assert!(
        reason(t.inspect(&notice_live(Some("01179"), "g2b-public-notice-header-v1")))
            .contains("공고 화면(메뉴 01179)")
    );
    assert_eq!(
        "R26BK00000001-001",
        t.inspect(&notice_live(Some("01179"), "g2b-notice-a-v1"))
            .unwrap()
            .entity
    );
    assert!(
        t.inspect(&contract_live(Some("01579")))
            .unwrap()
            .entity
            .starts_with("R26TA00000001")
    );

    let good = t
        .inspect(&notice_live(Some("01179"), "g2b-notice-a-v1"))
        .unwrap()
        .base_token;
    for bad in [
        notice_live(Some("01175"), "g2b-notice-a-v1"),
        notice_live(None, "g2b-notice-a-v1"),
    ] {
        reason(t.save(&bad, &good));
        reason(t.h.project(&bad));
    }
}

#[test]
fn 통합_공고_상세는_공고_b_로_읽고_비어_온_기초금액_표는_걷지_않은_값이다() {
    // 메뉴 01196 — 번호와 차수가 한 칸(「본번호 - 차수」)에 온다. 수의시담은 기초금액 표가 비어 온다.
    let t = ScreensHarness::new();
    let live_b = |base_rows: Vec<Value>| {
        let mut tables = Map::new();
        tables.insert(
            "mf_wfm_container_mainWframe_gridView5".into(),
            Value::Array(base_rows),
        );
        tables.insert(
            "mf_wfm_container_mainWframe_gridView3".into(),
            json!([{ "bidClsfNo": "1", "bidPbancItemSqno": 1, "dtlsPrnmNm": "시험 품목", "prchsDtlItemQty": 1, "prchsDtlItemUprc": 1000 }]),
        );
        live(
            "g2b-notice-b-v1",
            json!({
                "pointInfo": { "bidPbancText": "R26BK00000002 - 000", "bidPbancNm": "시험 공고" },
                "tables": Value::Object(tables),
            }),
            Some("01196"),
        )
    };
    let preview = t.inspect(&live_b(vec![])).unwrap();
    assert_eq!("R26BK00000002-000", preview.entity);
    assert_eq!(1, preview.item_count);
    assert!(
        !preview.document.rows[0]
            .values
            .iter()
            .any(|(k, _)| k == "base_price")
    );
    assert_eq!(
        Some("1000"),
        t.inspect(&live_b(vec![json!({ "baseAmt": 1000 })]))
            .unwrap()
            .document
            .rows[0]
            .value("base_price")
    );
    // 분류가 여럿이면 어느 것이 문서 전체의 값인지 모르므로 여전히 막는다.
    assert_eq!(
        "문서 전체 금액은 단일 분류 표에서만 가져올 수 있습니다.",
        reason(t.inspect(&live_b(vec![
            json!({ "baseAmt": 1 }),
            json!({ "baseAmt": 2 })
        ])))
    );
    assert_eq!(
        "공고 화면(메뉴 01196)인데 다른 프로필로 읽혔습니다. 화면을 다시 읽어 주세요.",
        reason(t.inspect(&notice_live(Some("01196"), "g2b-notice-a-v1")))
    );
}

#[test]
fn 변경계약서_작성_화면은_품목_표의_번호로_그_차수의_계약이고_업체는_바뀐_뒤의_표만_읽는다() {
    // 메뉴 01608 — 서명 전에 쓰는 화면이지만 읽은 값은 그 차수의 계약으로 담는다. 계약일은 이 화면에 없어 걷지 않는다.
    let t = ScreensHarness::new();
    let preview = t.inspect(&change_live("g2b-contract-change-v1")).unwrap();
    assert_eq!("R26TA00000005-01", preview.entity);
    assert_eq!(1, preview.item_count);
    let head = &preview.document.rows[0];
    assert_eq!(Some("2000"), head.value("amount")); // 계약 상세와 같이 품목에서 셈한다.
    assert_eq!(Some("수의계약"), head.value("contract_method"));
    assert!(!head.values.iter().any(|(k, _)| k == "contracted_on"));
    // 바뀌기 전 업체(앞 차수의 것)는 이 차수에 담지 않는다.
    let partners: Vec<Option<&str>> = preview
        .document
        .rows
        .iter()
        .filter(|r| r.table == "erp_partner")
        .map(|r| r.value("name"))
        .collect();
    assert_eq!(vec![Some("시험 업체")], partners);
    assert_eq!(
        "계약 화면(메뉴 01608)인데 다른 프로필로 읽혔습니다. 화면을 다시 읽어 주세요.",
        reason(t.inspect(&change_live("g2b-contract-v1")))
    );

    // 서명 뒤 계약 상세(01579)에서 같은 차수를 다시 읽는 것은 여느 두 번째 수집이다.
    let change = change_live("g2b-contract-change-v1");
    t.save(&change, &t.inspect(&change).unwrap().base_token)
        .unwrap();
    let mut tables = Map::new();
    tables.insert(
        ITEMS.into(),
        json!([{ "ctrtNo": "R26TA00000005", "ctrtChgOrd": "01", "ctrtItemSqno": 1, "ctrtItemNm": "시험 품목",
            "ctrtQty": 2, "ctrtUprc": 1000, "ctrtAmt": 2000 }]),
    );
    let signed = live(
        "g2b-contract-v1",
        json!({
            "pointInfo": { "ctrtNoOrd": "R26TA00000005-01", "ctrtNm": "시험 변경계약", "ctrtDt": "2026/10/05" },
            "tables": Value::Object(tables),
        }),
        Some("01579"),
    );
    let again = t.inspect(&signed).unwrap();
    assert_eq!("R26TA00000005-01", again.entity);
    assert!(again.unmatched.is_empty());
}

#[test]
fn 일반용역_접수_상세는_접수_b_로_읽고_다른_프로필로_열면_맞는_프로필을_알려_준다() {
    // 메뉴 01112 — 번호·차수가 머리에 있고 품목은 gvDmndItem 에 온다. 그 화면이 오기 전에는 접수 B 가 받는 화면이 없었다.
    let t = ScreensHarness::new();
    let file = |profile: &str| {
        file_input(
            profile,
            json!({
                "pointInfo": {
                    "depth1": "01001", "depth2": "01112", "ctrtDmndRcptNo": "RC012", "ctrtDmndRcptOrd": "000",
                    "ctrtDmndBizNm": "시험 용역", "wrtYmd": "20261001", "scsbdMthdCd": "낙030005",
                },
                "tables": { "mf_wfm_container_gvDmndItem": [{
                    "ctrtDmndRcptNo": "RC012", "ctrtDmndRcptOrd": "000", "ctrtDmndRcptItemSqno": 1, "dtlsPrnm": "시험 용역",
                    "ctrtDmndQty": 1, "ctrtDmndUprc": 1000, "devyCndtCd": "인010045" }] },
            }),
        )
    };
    let preview = t.inspect(&file("g2b-request-b-v1")).unwrap();
    assert_eq!("RC012-000", preview.entity);
    assert_eq!(1, preview.item_count);
    assert_eq!(
        Some("협상에의한계약"),
        preview.document.rows[0].value("award_method")
    );
    let item = single(
        preview
            .document
            .rows
            .iter()
            .filter(|r| r.table == "request_item"),
    );
    assert_eq!(Some("기타사항참조"), item.value("delivery_terms"));
    assert_eq!(
        "접수 화면(메뉴 01112)의 JSON 입니다. 접수 프로필(g2b-request-b-v1)로 여세요.",
        reason(t.inspect(&file("g2b-request-v1")))
    );
}

#[test]
fn 물품_접수_상세는_접수_b_의_칸을_옮긴_프로필로_읽고_다른_프로필로_열면_맞는_프로필을_알려_준다() {
    // 메뉴 01114 — 머리는 01112 와 같은 고유키다. 화면 하나에 프로필 하나라 접수 B 의 칸을 새 프로필로 옮겼다.
    let t = ScreensHarness::new();
    let file = |profile: &str| {
        file_input(
            profile,
            json!({
                "pointInfo": {
                    "depth1": "01001", "depth2": "01114", "ctrtDmndRcptNo": "RC014", "ctrtDmndRcptOrd": "000",
                    "ctrtDmndBizNm": "시험 물품", "wrtYmd": "20261001", "stdCtrtMthdCd": "계030003",
                },
                "tables": { "mf_wfm_container_gvDmndItem": [{
                    "ctrtDmndRcptNo": "RC014", "ctrtDmndRcptOrd": "000", "ctrtDmndRcptItemSqno": 1, "dtlsPrnm": "시험 물품",
                    "ctrtDmndQty": 3, "ctrtDmndUprc": 1000, "devyCndtCd": "인010001" }] },
            }),
        )
    };
    let preview = t.inspect(&file("g2b-request-goods-v1")).unwrap();
    assert_eq!("RC014-000", preview.entity);
    assert_eq!(1, preview.item_count);
    assert_eq!(
        Some("제한경쟁"),
        preview.document.rows[0].value("contract_method")
    );
    let item = single(
        preview
            .document
            .rows
            .iter()
            .filter(|r| r.table == "request_item"),
    );
    assert_eq!(Some("납품장소도"), item.value("delivery_terms"));
    assert_eq!(
        "접수 화면(메뉴 01114)의 JSON 입니다. 접수 프로필(g2b-request-goods-v1)로 여세요.",
        reason(t.inspect(&file("g2b-request-b-v1")))
    );
}

#[test]
fn 일반용역_공고_상세는_공고_a_의_칸으로_읽고_미리보기가_한_벌_더_실은_첨부는_하나로_본다() {
    // 메뉴 01181 — 물품 공고 상세(01179)와 같은 칸이다. 「미리보기」 창이 같은 첨부 표를 한 벌 더 싣고 빈 첨부 표가 하나 더 온다.
    let t = ScreensHarness::new();
    let files = json!([{ "atchFileSqno": 1, "atchFileDscr": "공고서", "orgnlAtchFileNm": "시험 공고서.hwp" }]);
    let file = |profile: &str| {
        let mut tables = Map::new();
        tables.insert(
            "mf_wfm_container_tabCont_contents_itemTabs6_body_wframe1_grdAliasDmTtl01List".into(),
            json!([{ "bidClsfNo": "0", "bidPbancItemSqno": "1", "dtlsPrnmNm": "시험 용역", "devyCndtCd": "인010040" }]),
        );
        tables.insert("mf_wfm_container_tabEmpty_grdFile".into(), json!([]));
        tables.insert("mf_wfm_container_grdFile".into(), files.clone());
        tables.insert("mf_wfm_container_preview_grdFile".into(), files.clone());
        file_input(
            profile,
            json!({
                "pointInfo": {
                    "depth1": "01170", "depth2": "01181", "bidPbancNo": "R26BK00000011", "bidPbancOrd": "001", "bidPbancNm": "시험 용역 공고",
                    "pbancKndCd": "공440002", "pbancSttsCd": "공400002", "scsbdMthdCd": "낙030005", "dmstcOvrsSeCd": "국010001",
                    "pnprDcsnMthoCd": "예050001", "pnprDcsnMthoNm": "비예가",
                    "slprRcptBgngDay": "20261001", "slprRcptBgngTime": "1000", "onbsPrnmntDay": "20261008", "onbsPrnmntTime": "1100",
                },
                "tables": Value::Object(tables),
            }),
        )
    };
    let preview = t.inspect(&file("g2b-notice-a-service-v1")).unwrap();
    assert_eq!("R26BK00000011-001", preview.entity);
    assert_eq!(1, preview.item_count);
    let rows = &preview.document.rows;
    let head = &rows[0];
    // 종류와 상태는 따로 담고, 공고 A 의 후처리로 일정 행을 세운다.
    assert_eq!(
        (head.value("notice_kind"), head.value("source_status")),
        (Some("실공고"), Some("변경공고"))
    );
    let schedule: Vec<Option<&str>> = rows
        .iter()
        .filter(|r| r.table == "notice_schedule")
        .map(|r| r.value("name"))
        .collect();
    assert_eq!(vec![Some("입찰서제출"), Some("개찰")], schedule);
    assert_eq!(
        (
            head.value("award_method"),
            head.value("international_kind"),
            head.value("price_estimation_method")
        ),
        (Some("협상에의한계약"), Some("국내입찰"), Some("비예가"))
    );
    assert_eq!(
        Some("과업내역에 따름"),
        single(rows.iter().filter(|r| r.table == "notice_item")).value("delivery_terms")
    );
    single(rows.iter().filter(|r| r.table == "erp_notice_attachment"));
    assert_eq!(
        "공고 화면(메뉴 01181)의 JSON 입니다. 공고 프로필(g2b-notice-a-service-v1)로 여세요.",
        reason(t.inspect(&file("g2b-notice-a-v1")))
    );
}

/// 원문에 명칭 칸이 없어 「명칭 미확정」 으로 담기던 코드 — 명칭은 사람이 계약 상세 화면에서 읽어 준 그대로다(2026-10-15).
fn contract_trait_names(profile: &str) {
    let profile = default_profile(profile);
    let convert = |value: &str, source: &str| {
        let map = profile
            .fields_iter()
            .find(|f| f.source() == source)
            .unwrap();
        mapping::convert_value(Some(value), map).unwrap().unwrap()
    };
    assert_eq!(
        "[국가계약법]용역(물품과 용역을 일괄입찰한 소프트웨어 사업제외)(0.125)",
        convert("0013", "ctrtChrsSeCd")
    );
    assert_eq!("지급각서대체", convert("계260001", "ctwtSbmsnMthdCd"));
    assert_eq!(
        "전자접수 및 직접수납",
        convert("계260002", "ctwtSbmsnMthdCd")
    );
    assert_eq!(
        "[국가계약법]물품의 제조 · 구매(0.075)",
        convert("0009", "ctrtChrsSeCd")
    );
    // 사람이 건넨 목록에 없는 번호는 짐작하지 않는다.
    assert_eq!("0005 (명칭 미확정)", convert("0005", "ctrtChrsSeCd"));
}

#[test]
fn 계약특성과_계약보증서_제출방법은_사람이_화면에서_확인한_명칭으로_담는다_계약() {
    contract_trait_names("g2b-contract-v1");
}

#[test]
fn 계약특성과_계약보증서_제출방법은_사람이_화면에서_확인한_명칭으로_담는다_변경계약() {
    contract_trait_names("g2b-contract-change-v1");
}

#[test]
fn 계약특성_코드_사전은_계약_프로필_모두가_같은_것을_쓴다() {
    let defaults = mapping::defaults();
    let dictionaries: Vec<String> = defaults
        .profiles_iter()
        .flat_map(|p| p.fields_iter())
        .filter(|f| f.source() == "ctrtChrsSeCd")
        .map(|f| {
            let mut codes = f.codes.clone().unwrap();
            codes.sort_by(|a, b| a.0.cmp(&b.0));
            format!("{codes:?}")
        })
        .collect();
    assert_eq!(2, dictionaries.len());
    let mut distinct = dictionaries.clone();
    distinct.dedup();
    assert_eq!(1, distinct.len());
}

#[test]
fn 확장은_인사의_매핑에서_목록을_받고_스냅샷의_화면_번호는_모르는_칸이_아니다() {
    let t = ScreensHarness::new();
    let host = t.host();
    let call = |method: &str, args: Value| {
        host.dispatch(
            &json!({ "protocolVersion": 2, "requestId": t.h.env.new_id(), "method": method, "params": args }),
            false,
        )
    };

    // 목록을 따로 싣지 않는다 — 매핑의 프로필이 단 화면이 그것이다.
    let hello = call("hello", Value::Null)["result"].clone();
    assert!(hello.get("screens").is_none());
    let mut listed: Vec<(String, String)> = hello["mapping"]["profiles"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["screen"].is_object())
        .map(|p| {
            (
                p["screen"]["code"].as_str().unwrap().to_string(),
                p["id"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    listed.sort();
    let expected: Vec<(String, String)> = [
        ("01112", "g2b-request-b-v1"),
        ("01114", "g2b-request-goods-v1"),
        ("01117", "g2b-request-v1"),
        ("01179", "g2b-notice-a-v1"),
        ("01181", "g2b-notice-a-service-v1"),
        ("01196", "g2b-notice-b-v1"),
        ("01579", "g2b-contract-v1"),
        ("01608", "g2b-contract-change-v1"),
    ]
    .iter()
    .map(|(a, b)| (a.to_string(), b.to_string()))
    .collect();
    assert_eq!(expected, listed);

    let refused = call(
        "document",
        json!({ "datasetId": t.dataset, "op": "check", "snapshot": notice_live(Some("01175"), "g2b-notice-a-v1").to_value() }),
    );
    assert_eq!(refused["result"]["state"], json!("blocked"));
    assert_eq!(
        refused["result"]["reason"],
        json!("메뉴 01175 화면은 수집하지 않습니다.")
    );
    let checked = call(
        "document",
        json!({ "datasetId": t.dataset, "op": "check", "snapshot": notice_live(Some("01179"), "g2b-notice-a-v1").to_value() }),
    );
    assert_eq!(checked["ok"], json!(true));
    assert_ne!(checked["result"]["state"], json!("blocked"));
}

#[test]
fn 수집_이력은_화면_번호를_적어_두고_재검토는_그_번호로_가린다() {
    // 저장은 걸러 담은 자료만 남겨 메뉴 경로가 지워진다 — 그래서 읽은 화면의 번호를 이력에 함께 적는다.
    let t = ScreensHarness::new();
    let notice = notice_live(Some("01179"), "g2b-notice-a-v1");
    t.save(&notice, &t.inspect(&notice).unwrap().base_token)
        .unwrap();
    let file = request_file(&REQUEST_PAGE, "g2b-request-v1");
    t.save(&file, &t.inspect(&file).unwrap().base_token)
        .unwrap();
    let c = t.h.read();
    let mut statement = c
        .prepare("SELECT profile, snapshot_json FROM erp_capture ORDER BY profile")
        .unwrap();
    let stored: Vec<(String, Value)> = statement
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .unwrap()
        .map(|row| {
            let (profile, json) = row.unwrap();
            (profile, serde_json::from_str(&json).unwrap())
        })
        .collect();
    let shown: Vec<(&str, &str)> = stored
        .iter()
        .map(|(p, snapshot)| (p.as_str(), snapshot["screen"].as_str().unwrap()))
        .collect();
    assert_eq!(
        vec![("g2b-notice-a-v1", "01179"), ("g2b-request-v1", "01117")],
        shown
    );

    // 창의 고급 도구는 이력의 자료를 파일로, 이력의 번호를 함께 실어 다시 본다.
    for (profile, snapshot) in &stored {
        let data = snapshot["data"].clone();
        assert_eq!(None, screens::leaf(&data["pointInfo"]));
        let again = CaptureInput {
            screen: snapshot["screen"].as_str().map(str::to_string),
            ..file_input(profile, data)
        };
        assert!(t.inspect(&again).unwrap().changes.is_empty());
        // 번호가 없으면(번호를 적기 전의 이력) 받지 않는다. 창은 그런 이력을 부르기 전에 막는다(Nara.test.tsx).
        assert_eq!(
            "이 JSON 에는 화면 번호(depth)가 없어 받지 않습니다.",
            reason(t.inspect(&CaptureInput {
                screen: None,
                ..again
            }))
        );
    }
    // 실린 번호가 메뉴 경로보다 앞선다.
    assert!(
        reason(t.inspect(&CaptureInput {
            screen: Some("01579".into()),
            ..request_file(&REQUEST_PAGE, "g2b-request-v1")
        }))
        .contains("계약 프로필")
    );
}

#[test]
fn 화면_번호가_없는_입력은_예전과_같은_해시를_낸다() {
    // 재전송 영수증(request_hash)과 baseToken 이 입력의 해시를 쓴다 — 번호 칸이 null 로 실리면 판을 올린 뒤
    // 결과를 확인하지 못한 저장의 재전송이 「같은 수집 ID 에 다른 내용」 으로 거절된다.
    let plain =
        serde_json::to_string_pretty(&request_file(&REQUEST_PAGE, "g2b-request-v1").to_value())
            .unwrap();
    assert!(!plain.contains("\"screen\""));
    let noticed =
        serde_json::to_string_pretty(&notice_live(Some("01179"), "g2b-notice-a-v1").to_value())
            .unwrap();
    assert!(noticed.contains("\"screen\": \"01179\""));
}

// ── 들어온 자료(수집 기록) ───────────────────────────────────────

fn ldt(year: i64, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> LocalDateTime {
    LocalDateTime {
        year,
        month,
        day,
        hour,
        minute,
        second,
        ticks: 0,
    }
}

fn notice(title: &str) -> CaptureInput {
    live(
        "g2b-notice-a-v1",
        json!({
            "pointInfo": { "bidPbancNo": "R26BK00000001", "bidPbancOrd": "001", "bidPbancNm": title },
            "tables": {},
        }),
        Some("01179"),
    )
}

/// 저장과 같은 꼴(UTC 왕복 표기)로 적는다 — 로컬 시각을 넘기면 옮겨서 적는다.
fn stored(local: LocalDateTime) -> String {
    to_utc_text(local, &ZONE)
}

fn stamp(c: &Connection, capture_id: &str, local: LocalDateTime) {
    c.execute(
        "UPDATE erp_capture SET captured_at = ?1 WHERE capture_id = ?2",
        params![stored(local), capture_id],
    )
    .unwrap();
}

fn fake(
    c: &Connection,
    capture_id: &str,
    profile: &str,
    base: &str,
    seq: &str,
    changed: bool,
    local: LocalDateTime,
) {
    c.execute(
        "INSERT INTO erp_capture(capture_id, request_hash, content_hash, profile, entity_base, entity_seq, snapshot_json, result_json, captured_at)
         VALUES (?1, 'h', 'h', ?2, ?3, ?4, '{}', ?5, ?6)",
        params![
            capture_id,
            profile,
            base,
            seq,
            json!({ "changed": changed }).to_string(),
            stored(local)
        ],
    )
    .unwrap();
}

///
/// 시각은 한국 표준시(+9)로 못 박고, 경계(어제 자정
/// 1초 전·오늘 자정 30초 뒤)를 같은 방식으로 세운다.
#[test]
fn 오늘_것만_종류와_결과로_가르고_마지막은_오늘이_아니어도_낸다() {
    let h = Harness::new("capture-log");
    let now = ldt(2026, 10, 5, 9, 30, 0);
    let db = &h.db;
    let read = |at: LocalDateTime| capture_log::read(db, at, &ZONE).unwrap();

    let empty = read(now);
    assert_eq!("2026-10-05", empty.today);
    assert!(empty.entries.is_empty());
    assert!(empty.last.is_none());

    // 어제 자정 1초 전 — 오늘이 아니다. 그래도 마지막 하나로는 선다.
    let c = h.write();
    fake(
        &c,
        &h.env.new_id(),
        "g2b-contract-v1",
        "R26TA00000003",
        "01",
        true,
        ldt(2026, 10, 4, 23, 59, 59),
    );
    let yesterday = read(now);
    assert!(yesterday.entries.is_empty());
    assert_eq!(0, yesterday.contracts);
    let last = yesterday.last.as_ref().unwrap();
    assert_eq!("계약", last.kind);
    assert_eq!("2026-10-04T23:59:59", last.at);

    // 진짜 저장 길로 공고 한 건, 같은 화면을 다시 저장해 「변경 없음」 한 건.
    let host = h.host();
    let first = h.env.new_id();
    host.save(
        &notice("지어낸 공고"),
        Some(&host.inspect(&notice("지어낸 공고")).unwrap().base_token),
        Some(&first),
        &Vec::new(),
    )
    .unwrap();
    let again = h.env.new_id();
    host.save(
        &notice("지어낸 공고"),
        Some(&host.inspect(&notice("지어낸 공고")).unwrap().base_token),
        Some(&again),
        &Vec::new(),
    )
    .unwrap();
    stamp(&c, &first, ldt(2026, 10, 5, 9, 0, 0));
    stamp(&c, &again, ldt(2026, 10, 5, 9, 5, 0));

    // 자정 30초 뒤 — 오늘이다. 계약 표에 건명이 있으면 그것을 낸다.
    fake(
        &c,
        &h.env.new_id(),
        "g2b-contract-v1",
        "R26TA00000003",
        "01",
        true,
        ldt(2026, 10, 5, 0, 0, 30),
    );
    c.execute_batch(
        "INSERT INTO contract_series VALUES ('R26TA00000003');
         INSERT INTO contract(contract_base, seq, title, updated_at) VALUES ('R26TA00000003', '01', '지어낸 계약', 'now');",
    )
    .unwrap();

    let day = read(now);
    let entries: Vec<(&str, &str, &str, &str, bool, &str)> = day
        .entries
        .iter()
        .map(|e| {
            (
                e.at.as_str(),
                e.kind.as_str(),
                e.number.as_str(),
                e.title.as_str(),
                e.changed,
                e.result.as_str(),
            )
        })
        .collect();
    assert_eq!(
        vec![
            (
                "2026-10-05T09:05:00",
                "공고",
                "R26BK00000001-001",
                "지어낸 공고",
                false,
                "변경 없음"
            ),
            (
                "2026-10-05T09:00:00",
                "공고",
                "R26BK00000001-001",
                "지어낸 공고",
                true,
                "저장"
            ),
            (
                "2026-10-05T00:00:30",
                "계약",
                "R26TA0000000301",
                "지어낸 계약",
                true,
                "저장"
            ),
        ],
        entries
    );
    assert_eq!(
        (0, 2, 1, 2),
        (day.requests, day.notices, day.contracts, day.saved)
    );
    assert_eq!(again, day.last.as_ref().unwrap().capture_id);
    assert_eq!("live", day.entries[0].scope);

    // 건명은 지금 쌓인 것이 먼저다. 비면 수집 사진에서, 사진에도 없으면 빈 문자열이다.
    c.execute_batch(
        "UPDATE notice SET title = '사람이 본 건명'; DELETE FROM contract; DELETE FROM contract_series;",
    )
    .unwrap();
    assert_eq!("사람이 본 건명", read(now).entries[0].title);
    c.execute_batch("UPDATE notice SET title = NULL;").unwrap();
    let fallback = read(now);
    assert_eq!("지어낸 공고", fallback.entries[0].title);
    assert_eq!("", fallback.entries[2].title);

    // 다음 날 아침에는 어제가 된다 — 경계는 로컬 자정이다.
    assert!(read(now.add_days(1)).entries.is_empty());

    // 열람 창은 읽기 전용 손잡이로 같은 것을 읽는다.
    let viewer = capture_log::read(&Database::new(&h.path, Access::Read), now, &ZONE).unwrap();
    assert_eq!(3, viewer.entries.len());
}

// ── 문서 만들기 패널(ADR-045) ───────────────────────────────

const DOCUMENT_REQUEST: &str = "00000000-0000-4000-8000-0000000000d0";

/// 패널이 호스트에 보내는 `document` 한 번 — 답 전체(봉투째).
fn document_reply(h: &Harness, home: &Pointing, op: &str, extra: Value) -> Value {
    let mut params = json!({ "datasetId": h.dataset(), "op": op });
    params
        .as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    ErpCapture::new(&h.path, &h.env)
        .with_destination(home)
        .dispatch(
            &json!({ "protocolVersion": 2, "requestId": DOCUMENT_REQUEST, "method": "document", "params": params }),
            false,
        )
}

fn document(h: &Harness, home: &Pointing, op: &str, extra: Value) -> Value {
    let reply = document_reply(h, home, op, extra);
    assert_eq!(reply["ok"], true, "{reply}");
    reply["result"].clone()
}

fn check(h: &Harness, home: &Pointing, input: &CaptureInput) -> Value {
    document(h, home, "check", json!({ "snapshot": input.to_value() }))
}

fn store(h: &Harness, home: &Pointing, input: &CaptureInput) -> Value {
    document(h, home, "save", json!({ "snapshot": input.to_value() }))
}

fn read_document(h: &Harness, home: &Pointing, entity: &str, entity_type: &str) -> Value {
    document(
        h,
        home,
        "read",
        json!({ "entity": entity, "entityType": entity_type }),
    )
}

/// 쪽지가 이 작업자료를 가리키는 하네스.
fn pointed(name: &str) -> (Harness, Pointing) {
    let h = Harness::new(name);
    let home = Pointing::none();
    home.write_config(&h.path, &h.dataset());
    (h, home)
}

fn sample_contract() -> CaptureInput {
    with_staff(
        &contract(
            "00",
            "합성 음향설비 구매",
            "20260914",
            &[(1, 6, 2_000_000), (2, 1, 18_400_000)],
        ),
        "홍길동",
    )
}

fn keys(row: &Value) -> Vec<String> {
    row.as_object().unwrap().keys().cloned().collect()
}

/// 줄의 값이 모두 글인가 — 빈 값이 null 로 새면 받는 쪽의 빈 값 경고가 죽는다.
fn all_text(row: &Value) -> bool {
    row.as_object().unwrap().values().all(Value::is_string)
}

fn auto_preferences() -> AutoCapturePreferences {
    AutoCapturePreferences { enabled: true }
}

#[test]
fn 문서_패널은_새_건을_다르다고_하고_저장한_뒤에는_같다고_한다() {
    let (h, home) = pointed("document-new");
    let c = h.read();
    let input = sample_contract();

    let first = check(&h, &home, &input);
    assert_eq!(first["state"], "differs");
    assert_eq!(first["isNew"], true);
    assert_eq!(first["entity"], "R26TA00000001-00");
    assert_eq!(first["entityType"], "contract");
    assert!(first["changedCount"].as_u64().unwrap() > 0);
    assert!(first.get("reason").is_none());
    // 무엇이 다른지 — 앞에서 스무 개까지, 이름표와 값은 모두 글이다(빈 값은 빈 글).
    let listed = first["changes"].as_array().unwrap();
    assert!(!listed.is_empty() && listed.len() <= 20, "{listed:?}");
    assert!(listed.len() as u64 <= first["changedCount"].as_u64().unwrap());
    assert!(
        listed
            .iter()
            .all(|ch| keys(ch) == ["label", "before", "after"] && all_text(ch)),
        "{listed:?}"
    );
    assert!(
        listed
            .contains(&json!({ "label": "계약건명", "before": "", "after": "합성 음향설비 구매" })),
        "{listed:?}"
    );
    assert_eq!(count(&c, "erp_capture"), 0); // 묻기만 하고 쓰지 않는다.

    let saved = store(&h, &home, &input);
    assert_eq!(saved["saved"], true);
    assert_eq!(saved["state"], "same");
    assert_eq!(saved["isNew"], false);
    assert_eq!(saved["changedCount"], 0);
    assert_eq!(saved["changes"], json!([]));
    assert_eq!(count(&c, "erp_capture"), 1);

    // 같은 화면이면 다시 쓰지 않는다.
    let again = store(&h, &home, &input);
    assert_eq!(
        (&again["saved"], &again["state"]),
        (&json!(false), &json!("same"))
    );
    assert_eq!(count(&c, "erp_capture"), 1);
    assert_eq!(check(&h, &home, &input)["state"], "same");

    // 자동 수집과 한 판단이다 — 패널이 같다고 한 화면을 자동 수집도 같다고 한다.
    let now = 1_800_000_000_000_i64;
    let mut automatic = AutoCapture::new(&h.db, &home, &h.env);
    assert_eq!(
        state_of(automatic.try_save(
            &auto_tab(&input, &h.dataset(), now, TabOptions::default()),
            &auto_preferences(),
            "홍길동",
            now,
            None
        )),
        "unchanged"
    );

    // 새 차수는 새 것이다.
    let next = contract(
        "01",
        "합성 음향설비 구매",
        "20260914",
        &[(1, 6, 2_000_000), (2, 1, 18_400_000)],
    );
    let round = check(&h, &home, &next);
    assert_eq!(
        (&round["state"], &round["isNew"]),
        (&json!("differs"), &json!(true))
    );
}

#[test]
fn 문서_패널의_저장은_사람이_고친_칸과_화면에_빈_칸을_지키고_읽기에_그대로_보인다() {
    let (h, home) = pointed("document-keep");
    let input = sample_contract();
    store(&h, &home, &input);
    Store::new(Database::new(&h.path, Access::Write), &h.env)
        .set_override(
            &EntityRef::new("contract", "R26TA00000001", "00"),
            "계약건명",
            "사람 정정",
        )
        .unwrap();

    // 화면이 비워 보낸 칸만 다르면 저장된 것과 같다 — 있던 값을 지우지 않는다.
    let blank = with_data(&input, |data| data["pointInfo"]["ctrtGtnrt"] = json!(""));
    assert_eq!(check(&h, &home, &blank)["state"], "same");

    // 덮개 아래 값이 바뀌어도 덮개는 지킨다.
    let renamed = with_data(&blank, |data| {
        data["pointInfo"]["ctrtNm"] = json!("바뀐 합성 건명");
    });
    let before = check(&h, &home, &renamed);
    assert_eq!(before["state"], "differs");
    assert_eq!(before["isNew"], false);
    // 다른 것은 건명 하나 — 창이 같은 칸에 다는 면 열 이름으로, 값은 그 면 열의 표기로. 지키는 덮개 · 화면에 빈 칸은
    // 바뀌지 않으므로 싣지 않는다.
    assert_eq!(
        before["changes"],
        json!([{ "label": "계약건명", "before": "합성 음향설비 구매", "after": "바뀐 합성 건명" }])
    );
    assert_eq!(before["changedCount"], 1);
    let saved = store(&h, &home, &renamed);
    assert_eq!(
        (&saved["saved"], &saved["state"]),
        (&json!(true), &json!("same"))
    );
    let c = h.read();
    assert_eq!(text_of(&c, "SELECT title FROM contract"), "바뀐 합성 건명");
    assert_eq!(text_of(&c, "SELECT value FROM field_override"), "사람 정정");

    // 통합 뷰의 흐름식이 면 뷰를 읽으므로 덮개와 표기가 그대로 온다(공고 · 접수가 없어 건명은 계약건명이다).
    let read = read_document(&h, &home, "R26TA00000001-00", "contract");
    assert_eq!(read["values"]["건명"], "사람 정정");
    assert_eq!(read["values"]["계약보증금"], "8,094,520원");
}

/// 면 뷰의 한 줄을 열 이름 → 값(글)으로.
fn face_values(c: &Connection, sql: &str) -> HashMap<String, String> {
    let mut statement = c.prepare(sql).unwrap();
    let names: Vec<String> = statement.column_names().map(str::to_string).collect();
    statement
        .query_row([], |r| {
            Ok(names
                .iter()
                .enumerate()
                .map(|(i, n)| (n.clone(), cell(r.get_ref(i).unwrap())))
                .collect())
        })
        .unwrap()
}

#[test]
fn 문서_패널의_다른_칸은_면_뷰의_표기와_이름으로_보인다() {
    let (h, home) = pointed("document-notation");
    store(&h, &home, &sample_contract());

    // 보증금 · 계약일자 · 1번 품목의 수량과 금액이 바뀐 화면.
    let changed = with_data(
        &contract(
            "00",
            "합성 음향설비 구매",
            "20261030",
            &[(1, 7, 2_000_000), (2, 1, 18_400_000)],
        ),
        |data| data["pointInfo"]["ctrtGrnteAmt"] = json!("9123450"),
    );
    let before = check(&h, &home, &changed);
    assert_eq!(before["state"], "differs", "{before}");
    let listed = before["changes"].as_array().unwrap().clone();
    // 금액은 자릿점, 날짜는 YYYY/MM/DD, 품목 칸은 품목 뷰의 열 이름 — 업무 표의 글(9123450 · 2026-10-30 · quantity)이
    // 새지 않는다.
    for expected in [
        json!({ "label": "계약보증금(원)", "before": "8,094,520", "after": "9,123,450" }),
        json!({ "label": "계약일자", "before": "2026/09/14", "after": "2026/10/30" }),
        json!({ "label": "품목 1 · 수량", "before": "6", "after": "7" }),
        json!({ "label": "품목 1 · 금액", "before": "12,000,000", "after": "14,000,000" }),
    ] {
        assert!(listed.contains(&expected), "{expected} 가 없다: {listed:?}");
    }

    // 보장 — 저장한 뒤 면 뷰가 그 칸에 내는 글이 패널이 미리 보인 `after` 와 바이트까지 같다.
    let saved = store(&h, &home, &changed);
    assert_eq!(saved["saved"], true, "{saved}");
    let c = h.read();
    let head = face_values(&c, "SELECT * FROM v_계약");
    let line = face_values(&c, "SELECT * FROM v_계약품목 WHERE 순번 = 1");
    for ch in &listed {
        let label = ch["label"].as_str().unwrap();
        let shown = match label.strip_prefix("품목 1 · ") {
            Some(column) => &line[column],
            None => &head[label],
        };
        assert_eq!(ch["after"].as_str().unwrap(), shown, "{label}");
    }
}

#[test]
fn 문서_패널은_저장하지_못하는_화면의_까닭을_싣는다() {
    let (h, home) = pointed("document-blocked");
    let c = h.write();
    let input = sample_contract();

    // 수집 규칙이 바뀐 화면은 짝짓지도 못한다.
    let old_rule = CaptureInput {
        mapping_revision: Some("old".into()),
        ..input.clone()
    };
    let stale = check(&h, &home, &old_rule);
    assert_eq!(stale["state"], "blocked");
    assert_eq!(stale["reason"], "수집 규칙이 바뀌었습니다. 다시 확인하세요.");
    assert_eq!(stale["entity"], Value::Null);
    assert_eq!(stale["changes"], json!([]));
    let refused = store(&h, &home, &old_rule);
    assert_eq!(
        (&refused["state"], &refused["saved"]),
        (&json!("blocked"), &json!(false))
    );

    // 수집이 모르는 품목 줄이 이미 있으면 화면의 줄은 대응을 고르지 않은 채 새 줄이 되고(자동 수집과 같다), 그러면 총액이
    // 맞지 않아 저장이 거절된다. 묻기(check)는 저장해 보지 않으므로 다르다고만 한다.
    c.execute_batch(
        "INSERT INTO contract_series(contract_base) VALUES('R26TA00000001');
         INSERT INTO contract(contract_base,seq,amount,updated_at) VALUES('R26TA00000001','00','30400000','before');
         INSERT INTO contract_item(contract_base,seq,line_no,item_name,quantity,unit_price,amount)
             VALUES('R26TA00000001','00',1,'손으로 넣은 품목',1,'30400000','30400000');",
    )
    .unwrap();
    let pending = check(&h, &home, &input);
    assert_eq!(pending["state"], "differs");
    assert_eq!(pending["isNew"], false);
    let blocked = store(&h, &home, &input);
    assert_eq!(blocked["state"], "blocked");
    assert_eq!(blocked["saved"], false);
    assert_eq!(blocked["entity"], "R26TA00000001-00");
    // 거절된 저장도 무엇이 다른지 싣는다 — 짝짓지 못한 품목 줄은 창의 표 이름과 줄 번호를 단다.
    let refused = blocked["changes"].as_array().unwrap();
    assert!(!refused.is_empty(), "{blocked}");
    assert!(refused.iter().all(all_text), "{refused:?}");
    let reason = blocked["reason"].as_str().unwrap().to_string();
    assert!(reason.contains("계약 총액이 다릅니다"), "{reason}");
    assert_eq!(count(&c, "erp_capture"), 0);
    assert_eq!(count(&c, "contract_item"), 1);
    // 자동 수집도 같은 글로 멈춘다.
    let now = 1_800_000_000_000_i64;
    let held = AutoCapture::new(&h.db, &home, &h.env)
        .try_save(
            &auto_tab(&input, &h.dataset(), now, TabOptions::default()),
            &auto_preferences(),
            "홍길동",
            now,
            None,
        )
        .unwrap();
    assert_eq!(
        (held.state.as_str(), held.message.as_str()),
        ("hold", reason.as_str())
    );

    // 모르는 op · 모르는 종류 · 차수 없는 번호는 요청 오류다.
    for (op, extra) in [
        ("print", json!({})),
        (
            "read",
            json!({ "entity": "R26TA00000001-00", "entityType": "plan" }),
        ),
        (
            "read",
            json!({ "entity": "R26TA0000000100", "entityType": "contract" }),
        ),
    ] {
        let reply = document_reply(&h, &home, op, extra);
        assert_eq!(reply["ok"], false, "{reply}");
    }
}

/// 접수 → 공고 → 계약의 사슬. 접수와 공고는 명시 참조를 단 채 들이고, 계약은 패널로 저장한다(저장이 연결을 다시 짓는다).
/// `others` 는 같은 공고를 가리키는 계약을 더 넣는다.
fn chain(h: &Harness, home: &Pointing, others: &[&str]) {
    let c = h.write();
    c.execute_batch(
        "INSERT INTO request_series(request_base) VALUES('RC001');
         INSERT INTO request(request_base,seq,title,updated_at) VALUES('RC001','000','합성 접수','before');
         INSERT INTO request_item(request_base,seq,line_no,item_name,ref_request_base,ref_request_seq,ref_request_item)
             VALUES('RC001','000',1,'합성 품목','RC001','000','001');
         INSERT INTO notice_group(group_base) VALUES('R26BK00000001');
         INSERT INTO notice_series(notice_base,group_base) VALUES('R26BK00000001','R26BK00000001');
         INSERT INTO notice(notice_base,seq,title,updated_at) VALUES('R26BK00000001','000','합성 공고','before');
         INSERT INTO notice_item(notice_base,seq,line_no,ref_request_base,ref_request_seq,ref_request_item)
             VALUES('R26BK00000001','000',1,'RC001','000','001');",
    )
    .unwrap();
    for other in others {
        c.execute(
            "INSERT INTO contract_series(contract_base) VALUES(?1)",
            [other],
        )
        .unwrap();
        c.execute(
            "INSERT INTO contract(contract_base,seq,title,ref_notice_base,updated_at) VALUES(?1,'00','다른 합성 계약','R26BK00000001','before')",
            [other],
        )
        .unwrap();
    }
    assert_eq!(store(h, home, &sample_contract())["saved"], true);
}

/// exe 에 실린 통합 뷰의 열 이름들, 차례대로.
fn builtin_columns() -> Vec<String> {
    let package: Value = serde_json::from_str(pclm_core::storage::store::builtin_view_package())
        .unwrap();
    package["views"][0]["definition"]["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn 문서_패널의_읽기는_exe_에_실린_통합_뷰의_한_줄이다() {
    let (h, home) = pointed("document-read");
    chain(&h, &home, &[]);
    // 작업자료에 「문서 서식」 줄이 없으면 exe 의 정의를 셈하고, 깔린 u_* 는 읽지 않는다.
    h.write()
        .execute_batch("DELETE FROM user_view; DROP VIEW IF EXISTS \"u_문서 서식\";")
        .unwrap();

    let read = read_document(&h, &home, "R26TA00000001-00", "contract");
    assert_eq!(read["entityType"], "contract");
    assert_eq!(read["entity"], "R26TA0000000100"); // 탭의 번호를 면 뷰의 꼴로
    assert_eq!(keys(&read["values"]), builtin_columns());
    assert!(all_text(&read["values"])); // 빈 값은 빈 글이다.
    let v = &read["values"];
    assert_eq!(v["계약번호"], "R26TA0000000100");
    assert_eq!(v["계약금액"], "30,400,000원");
    assert_eq!(v["계약금액_한글"], "삼천사십만");
    assert_eq!(v["입찰공고번호"], "R26BK00000001-000");
    assert_eq!(v["공고명"], "합성 공고");
    assert_eq!(v["건명"], "합성 공고");
    // 계약 품목이 먼저다 — 서로 다르면 「첫 품명 외 n건」.
    assert_eq!(v["세부품명"], "시험 품목 1 외 1건");
    assert_eq!(v["납품기한"], "");
    assert_eq!(v["계약보증금납부기한"], official_day(&h, "+10 days"));
    assert_eq!(read["rows"], 1);
    assert_eq!(read["older"], false);
    assert_eq!(
        read["caseKeys"],
        json!(["request:RC001", "notice:R26BK00000001", "contract:R26TA00000001"])
    );

    // 접수 탭에서도 같은 사슬의 같은 줄이다.
    let from_request = read_document(&h, &home, "RC001-000", "request");
    assert_eq!(from_request["values"], read["values"]);
    assert_eq!(from_request["caseKeys"], read["caseKeys"]);

    // 품목 납품기한은 가장 늦은 날, 날짜가 아닌 글이 섞이면 모두 같을 때만 그 값이다.
    let c = h.write();
    c.execute_batch("UPDATE contract_item SET delivery_due = CASE line_no WHEN 1 THEN '2027-03-31' ELSE '2026-12-01' END;")
        .unwrap();
    let due_of = || read_document(&h, &home, "R26TA00000001-00", "contract")["values"]["납품기한"].clone();
    assert_eq!(due_of(), "2027. 3. 31.");
    c.execute_batch("UPDATE contract_item SET delivery_due = '수시 납품' WHERE line_no = 1;")
        .unwrap();
    assert_eq!(due_of(), "");
    c.execute_batch("UPDATE contract_item SET delivery_due = '수시 납품', item_name = '시험 품목';")
        .unwrap();
    assert_eq!(due_of(), "수시 납품");
    // 품명이 모두 같으면 그 품명이다.
    assert_eq!(
        read_document(&h, &home, "R26TA00000001-00", "contract")["values"]["세부품명"],
        "시험 품목"
    );

    // 쌓이지 않은 건은 null — 짐작해 채우지 않는다.
    assert_eq!(
        read_document(&h, &home, "R26TA00000009-00", "contract"),
        Value::Null
    );
}

/// 사람이 「문서 서식」 을 고쳐 저장하면 패널의 읽기가 그 정의를 셈한다(ADR-052). 나중에 깨진 식의 열은 빈 글이고, 다른 열은
/// 그대로다 — 식 하나가 읽기 전체를 막지 않는다.
#[test]
fn 문서_패널의_읽기는_사람이_고친_문서_서식을_셈한다() {
    let (h, home) = pointed("document-edited");
    chain(&h, &home, &[]);
    let s = Store::new(Database::new(&h.path, Access::Write), &h.env);
    let view = s
        .user_views()
        .unwrap()
        .into_iter()
        .find(|v| v.id == DOCUMENT_VIEW_ID)
        .unwrap();
    let mut definition = view.definition.clone();
    let set = |definition: &mut Value, name: &str, sql: &str| {
        let columns = definition["columns"].as_array_mut().unwrap();
        let column = columns.iter_mut().find(|c| c["name"] == name).unwrap();
        column["formula"] = json!({ "version": 2, "source": "sql", "sql": sql });
    };
    set(&mut definition, "건명", "SELECT '사람이 고친 건명'");
    s.save_user_view(
        &json!({ "name": view.name, "definition": definition }).to_string(),
        Some(DOCUMENT_VIEW_ID),
    )
    .unwrap();
    let v = read_document(&h, &home, "R26TA00000001-00", "contract")["values"].clone();
    assert_eq!(v["건명"], "사람이 고친 건명");
    assert_eq!(v["계약금액"], "30,400,000원");

    // 저장한 뒤에 깨진 식(저장은 깨진 식을 받지 않으므로 표에 바로 쓴다).
    set(&mut definition, "계약금액", "SELECT 없는_열");
    h.write()
        .execute(
            "UPDATE user_view SET definition = ?1 WHERE id = ?2",
            params![definition.to_string(), DOCUMENT_VIEW_ID],
        )
        .unwrap();
    let read = read_document(&h, &home, "R26TA00000001-00", "contract");
    let v = &read["values"];
    assert_eq!(keys(v), builtin_columns());
    assert!(all_text(v));
    assert_eq!(v["계약금액"], "");
    assert_eq!(v["건명"], "사람이 고친 건명");
    assert_eq!(v["계약번호"], "R26TA0000000100");
}

/// SQLite 가 보는 오늘(`date('now', 'localtime', shift)`)의 (년, 월, 일).
fn today_parts(h: &Harness, shift: &str) -> (u32, u32, u32) {
    let day: String = h
        .read()
        .query_row("SELECT date('now', 'localtime', ?1)", [shift], |r| r.get(0))
        .unwrap();
    let mut parts = day.split('-').map(|p| p.parse::<u32>().unwrap());
    (parts.next().unwrap(), parts.next().unwrap(), parts.next().unwrap())
}

/// 오늘에서 `shift` 만큼 옮긴 날의 공문서 표기 `2026. 1. 5.`.
fn official_day(h: &Harness, shift: &str) -> String {
    let (y, m, d) = today_parts(h, shift);
    format!("{y}. {m}. {d}.")
}

/// 엔진이 문서에 넣을 때 꾸미던 글(옛 `pclm-doc-engine` `values::resolve` — 「원」 · 공문서 날짜 · 파생 값)을 이제 뷰가
/// 낸다. 기대값은 그 엔진 시험의 기대값이다(새 구현의 출력에서 베끼지 않았다) — 문서에 찍히는 글이 바이트까지 같아야 한다.
#[test]
fn 문서_패널의_읽기는_옛_엔진이_문서에_넣던_글_그대로다() {
    let (h, home) = pointed("document-decorated");
    chain(&h, &home, &[]);
    h.write()
        .execute_batch(
            "UPDATE notice SET estimated_price = '24545454', project_amount = '별도 공지',
                 posted_at = '2026-10-20 09:30:00';
             INSERT INTO notice_schedule (notice_base, seq, line_no, name, starts_at, ends_at)
                 VALUES ('R26BK00000001', '000', 1, '개찰', '2026-10-27 10:00:15', NULL);
             UPDATE contract SET amount = '114500', guarantee_amount = ' ';
             UPDATE contract_item SET delivery_due = '수시 납품', item_name = '시험 품목';",
        )
        .unwrap();

    let v = read_document(&h, &home, "R26TA00000001-00", "contract")["values"].clone();
    for (name, text) in [
        ("추정가격", "24,545,454원"),
        ("계약금액", "114,500원"),
        ("계약금액_한글", "일십일만사천오백"),
        ("추정가격_한글", "이천사백오십사만오천사백오십사"),
        ("게시일시", "2026. 10. 20. 09:30"),
        ("게시일", "2026. 10. 20."),
        ("개찰일시", "2026. 10. 27. 10:00"), // 초는 찍지 않는다(ADR-053)
        ("공고일시_년월일", "2026년 10월 20일"),
        ("납품기한", "수시 납품"), // 날짜가 아닌 글은 그대로
    ] {
        assert_eq!(v[name], text, "{name}");
    }
    // 수로 읽히지 않는 금액 글은 빈 글이다(ADR-053 — 옛 원표기는 원문을 그대로 냈다).
    assert_eq!(v["사업예산"], "");
    // 빈 값은 계속 없는 값이다(엔진이 공백뿐인 글을 걷는다).
    assert_eq!(v["계약보증금"].as_str().map(str::trim), Some(""));
    // 쓰는 날 — 옛 엔진은 패널이 넘긴 오늘을, 이제 뷰는 호스트가 읽는 날을 쓴다.
    let (y, m, d) = today_parts(&h, "+0 days");
    assert_eq!(v["오늘_년월일"], format!("{y}년 {m}월 {d}일").as_str());
    assert_eq!(v["오늘_년월"], format!("{y}. {m}.").as_str());
    assert_eq!(v["계약보증금납부기한"], official_day(&h, "+10 days"));

    // 날짜 · 금액이 비면 파생 값도 빈 글이다(그 칸은 【확인 필요】).
    h.write()
        .execute_batch("UPDATE notice SET estimated_price = NULL, posted_at = NULL;")
        .unwrap();
    let v = read_document(&h, &home, "R26TA00000001-00", "contract")["values"].clone();
    for name in ["추정가격", "추정가격_한글", "게시일", "게시일시", "공고일시_년월일"] {
        assert_eq!(v[name], "", "{name}");
    }
    assert!(all_text(&v));

    // 시각 없는 날짜 — 일시 열은 00:00 을 찍는다(ADR-053).
    h.write()
        .execute_batch("UPDATE notice SET posted_at = '2026-01-01';")
        .unwrap();
    let v = read_document(&h, &home, "R26TA00000001-00", "contract")["values"].clone();
    assert_eq!(v["게시일"], "2026. 1. 1.");
    assert_eq!(v["게시일시"], "2026. 1. 1. 00:00");
    assert_eq!(v["공고일시_년월일"], "2026년 1월 1일");
}

#[test]
fn 문서_패널은_계약이_여럿인_공고에서_계약을_고르지_않는다() {
    let (h, home) = pointed("document-many");
    chain(&h, &home, &["R26TA00000002"]);

    let from_notice = read_document(&h, &home, "R26BK00000001-000", "notice");
    assert_eq!(from_notice["rows"], 2);
    let v = &from_notice["values"];
    assert_eq!(v["입찰공고번호"], "R26BK00000001-000");
    assert_eq!(v["조달요구번호"], read_document(&h, &home, "RC001-000", "request")["values"]["조달요구번호"]);
    // 줄마다 다른 칸(계약 쪽)은 비운다.
    assert_eq!(v["계약번호"], "");
    assert_eq!(v["단계"], "계약");

    // 계약 탭은 제 계약이다.
    let from_contract = read_document(&h, &home, "R26TA00000001-00", "contract");
    assert_eq!(from_contract["rows"], 1);
    assert_eq!(from_contract["values"]["계약번호"], "R26TA0000000100");
    assert_eq!(from_contract["values"]["입찰공고번호"], "R26BK00000001-000");
}

#[test]
fn 문서_패널은_공고_탭의_공고를_읽는다_재공고로_번호가_둘이어도() {
    let (h, home) = pointed("document-rebid");
    // 한 건에 공고번호가 둘 — 유찰된 공고와 그것을 대체한 재공고. 건의 현행 공고는 재공고다.
    h.write()
        .execute_batch(
            "INSERT INTO notice_group(group_base) VALUES('R26BK00000001');
             INSERT INTO notice_series(notice_base,group_base) VALUES('R26BK00000001','R26BK00000001'),('R26BK00000002','R26BK00000001');
             INSERT INTO notice(notice_base,seq,title,updated_at) VALUES
                 ('R26BK00000001','000','유찰된 합성 공고','before'),
                 ('R26BK00000002','000','옛 차수의 재공고','before'),
                 ('R26BK00000002','001','다시 낸 합성 공고','before');
             INSERT INTO notice_item(notice_base,seq,line_no,item_name,delivery_due) VALUES
                 ('R26BK00000001','000',1,'유찰 품목','2026-12-01'),
                 ('R26BK00000002','001',1,'다시 낸 품목','2027-01-15'),
                 ('R26BK00000002','001',2,'다시 낸 다른 품목','2027-02-28');
             INSERT INTO notice_relation(notice_base,seq,line_no,related,related_base,related_seq)
                 VALUES('R26BK00000002','001',1,'R26BK00000001-000','R26BK00000001','000');",
        )
        .unwrap();

    // 옛 공고 탭은 옛 공고의 값 — 품목(세부품명 · 납품기한)도 그 공고의 것이다.
    let old = read_document(&h, &home, "R26BK00000001-000", "notice");
    let v = &old["values"];
    assert_eq!(v["입찰공고번호"], "R26BK00000001-000");
    assert_eq!(v["공고명"], "유찰된 합성 공고");
    assert_eq!(v["세부품명"], "유찰 품목");
    assert_eq!(v["납품기한"], "2026. 12. 1.");
    assert_eq!(old["older"], false);
    assert_eq!(old["caseKeys"], json!(["notice:R26BK00000001"]));

    let current = read_document(&h, &home, "R26BK00000002-001", "notice");
    let v = &current["values"];
    assert_eq!(v["입찰공고번호"], "R26BK00000002-001");
    assert_eq!(v["공고명"], "다시 낸 합성 공고");
    assert_eq!(v["세부품명"], "다시 낸 품목 외 1건");
    assert_eq!(v["납품기한"], "2027. 2. 28.");
    assert_eq!(current["caseKeys"], old["caseKeys"]);
    // 공고서는 「외 n건」을 쓰지 않고 품목을 모두 나열한다. 하나로 정할 수 없는 줄(번호 · 수량이 빈 줄)이 있으면 그 열 전체가 빈 글이다.
    assert_eq!(v["세부품명_목록"], "다시 낸 품목, 다시 낸 다른 품목");
    assert_eq!(v["세부품명_번호목록"], "");
    assert_eq!(v["세부품명_수량목록"], "");
    // 같은 (품명, 번호) 의 줄은 한 항목으로 묶어 수량을 더하고, 첫 순번 차례로 잇는다.
    h.write()
        .execute_batch(
            "UPDATE notice_item SET detail_item_number = CASE line_no WHEN 1 THEN '0000000001' ELSE '0000000002' END,
                 quantity = CASE line_no WHEN 1 THEN 3 ELSE 5 END, unit = CASE line_no WHEN 1 THEN '개' ELSE '대' END
                 WHERE notice_base = 'R26BK00000002';
             INSERT INTO notice_item(notice_base,seq,line_no,item_name,detail_item_number,quantity,unit)
                 VALUES('R26BK00000002','001',3,'다시 낸 품목','0000000001',4,'개');",
        )
        .unwrap();
    let lists = || read_document(&h, &home, "R26BK00000002-001", "notice")["values"].clone();
    let v = lists();
    assert_eq!(v["세부품명_목록"], "다시 낸 품목, 다시 낸 다른 품목");
    assert_eq!(v["세부품명_번호목록"], "다시 낸 품목(0000000001), 다시 낸 다른 품목(0000000002)");
    assert_eq!(v["세부품명_수량목록"], "다시 낸 품목 / 7 개, 다시 낸 다른 품목 / 5 대");
    // 묶음 안에서 단위가 갈리면 수량목록만 빈 글이다.
    h.write()
        .execute_batch("UPDATE notice_item SET unit = '상자' WHERE notice_base = 'R26BK00000002' AND line_no = 3;")
        .unwrap();
    let v = lists();
    assert_eq!(v["세부품명_수량목록"], "");
    assert_eq!(v["세부품명_번호목록"], "다시 낸 품목(0000000001), 다시 낸 다른 품목(0000000002)");
    assert!(all_text(&v));
    // 품목이 하나면 수량목록은 「세부품명 / 수량 단위」 와 같은 글이다.
    h.write()
        .execute_batch("UPDATE notice_item SET quantity = 12, unit = '개' WHERE notice_base = 'R26BK00000001';")
        .unwrap();
    let v = read_document(&h, &home, "R26BK00000001-000", "notice")["values"].clone();
    assert_eq!(v["세부품명_목록"], "유찰 품목");
    assert_eq!(v["세부품명_수량목록"], "유찰 품목 / 12 개");
    assert_eq!(
        v["세부품명_수량목록"].as_str().unwrap(),
        format!("{} / {} {}", v["세부품명"].as_str().unwrap(), v["수량"].as_str().unwrap(), v["단위"].as_str().unwrap())
    );

    // 같은 공고번호의 옛 차수 탭은 옛 차수라고 알리고, 값은 최신 차수다.
    let older = read_document(&h, &home, "R26BK00000002-000", "notice");
    assert_eq!(older["older"], true);
    assert_eq!(older["values"]["공고명"], "다시 낸 합성 공고");

    // 창의 통합 · 맞춤 뷰는 건의 현행 공고 그대로다.
    let c = h.read();
    assert_eq!(
        text_of(&c, "SELECT 입찰공고번호 FROM v_통합"),
        "R26BK00000002-001"
    );
    assert_eq!(
        text_of(&c, "SELECT 공고명 FROM \"u_문서 서식\""),
        "다시 낸 합성 공고"
    );
}


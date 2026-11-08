//! 지금 보는 화면(ADR-036)·수집하는 화면(ADR-037)·들어온 자료 시험 — C# `MirrorTests`·`ScreensTests`·
//! `CaptureLogTests`.
//!
//! 옮기지 않은 것: 홈의 파일에 닿는 중계(`ExtensionRelay`·`ExtensionPresence` — 탭 보고·명령 폴더·상시 호스트 파이프)와
//! 자동 수집 설정의 **파일** 읽고 쓰기(`AutoCapturePreferences.Read/Write(home)`)는 이 크레이트에 없다. 설정은 글(`from_json`·
//! `to_json`)만, 자동 수집은 판단(`AutoCapture::try_save`)만 옮긴다. 쪽지(`Home`)는 [`Pointing`] 이 대신한다.
//!
//! C# 이 이 기계의 시간대(`DateTime.Now`·`ToLocalTime`)를 읽던 자리는 한국 표준시(+9) 고정 시간대로 바꿨다 — 기대값은 C# 시험이
//! 세운 것과 같은 방식(로컬 자정 경계)으로 셈한다. 번호·건명·금액은 모두 지어낸 것이다.

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
use pclm_core::storage::store::{EntityRef, Store};
use pclm_core::storage::{pclm_file, views};
use pclm_core::{Access, Database, Env, Error, PclmRole, Result};
use rusqlite::types::ValueRef;
use rusqlite::{Connection, params};
use serde_json::{Map, Value, json};

const ZONE: FixedZone = FixedZone(9 * 3600);
const ITEMS: &str = "mf_wfm_container_tacCtrt_contents_content2_body_grdCtrtLis";

// ── 같이 쓰는 것 ───────────────────────────────────────────────

/// 거절 사유(C# `Assert.Throws<InvalidOperationException>(..).Message`).
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

/// `Assert.Single(items, predicate)`.
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
        let db = pclm_file::create(&path, PclmRole::WORK, &env).unwrap();
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

    /// 확장과 같은 길로 저장한다(C# `Save(input)`).
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

/// C# `Home` 의 쪽지 자리 — 지금 가리키는 (파일, 자료 ID).
struct Pointing(RefCell<Option<(PathBuf, String)>>);

impl Pointing {
    fn none() -> Self {
        Pointing(RefCell::new(None))
    }

    /// C# `Home.WriteConfig(path, dataset)`.
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

/// 설정은 글로만 다닌다 — 파일 읽고 쓰기는 홈의 것이다.
/// C# MirrorTests.자동수집_설정은_로컬에서_기본꺼짐이며_이름을_선택적으로_저장한다 (글 쪽만)
#[test]
fn 자동수집_설정은_로컬에서_기본꺼짐이며_이름을_선택적으로_저장한다() {
    // 파일이 없을 때의 읽기는 글이 없는 것과 같다.
    assert!(!AutoCapturePreferences::from_json("").enabled);
    let settings = AutoCapturePreferences {
        enabled: true,
        name: "합성 담당자".into(),
    };
    assert_eq!(
        AutoCapturePreferences::from_json(&settings.to_json().unwrap()).name,
        "합성 담당자"
    );
    let unnamed = AutoCapturePreferences {
        name: String::new(),
        ..settings.clone()
    };
    assert_eq!(
        AutoCapturePreferences::from_json(&unnamed.to_json().unwrap()),
        AutoCapturePreferences {
            enabled: true,
            name: String::new()
        }
    );
    let legacy = r#"{"enabled":true,"name":"합성 담당자","rules":[{"screen":"01579","enabled":true,"source":"ctrtNm","text":"옛 조건"}]}"#;
    assert_eq!(AutoCapturePreferences::from_json(legacy), settings);
    let too_long = AutoCapturePreferences {
        name: "가".repeat(201),
        ..settings.clone()
    };
    assert!(matches!(
        too_long.to_json(),
        Err(Error::InvalidOperation(_))
    ));
    for invalid in [
        "{".to_string(),
        r#"{"enabled":true,"name":null}"#.to_string(),
        serde_json::to_string(&too_long.to_value()).unwrap(),
    ] {
        assert!(!AutoCapturePreferences::from_json(&invalid).enabled);
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

/// C# MirrorTests.자동수집은_현재담당자_부분일치와_새읽기_대상_규칙을_모두_확인한다
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
    let preferences = AutoCapturePreferences {
        enabled: true,
        name: "합성 담당자".into(),
    };
    let named = |name: &str| AutoCapturePreferences {
        name: name.into(),
        ..preferences.clone()
    };
    let tab = |snapshot: &CaptureInput, options: TabOptions<'_>| {
        auto_tab(snapshot, &dataset, now, options)
    };
    let plain = TabOptions::default();
    let mut automatic = AutoCapture::new(&h.db, &home, &h.env);
    let save = |automatic: &mut AutoCapture<'_>, tab: &Value, p: &AutoCapturePreferences| {
        state_of(automatic.try_save(tab, p, now, None))
    };

    assert_eq!(
        save(
            &mut automatic,
            &tab(&input, plain),
            &AutoCapturePreferences::default()
        ),
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
        assert_eq!(
            save(&mut automatic, &tab(&input, options), &preferences),
            "hold"
        );
    }
    let old_rule = CaptureInput {
        mapping_revision: Some("old".into()),
        ..input.clone()
    };
    assert_eq!(
        save(&mut automatic, &tab(&old_rule, plain), &preferences),
        "hold"
    );
    let without_staff = contract("01", "담당자 없는 합성 계약", "20260915", &[(1, 1, 1000)]);
    assert_eq!(
        save(&mut automatic, &tab(&without_staff, plain), &preferences),
        "hold"
    );
    assert_eq!(
        save(&mut automatic, &tab(&input, plain), &named("합성 담당자2")),
        "hold"
    );
    for staff in ["다른 담당자(010-0000-0000)", ""] {
        assert_eq!(
            save(
                &mut automatic,
                &tab(&with_staff(&input, staff), plain),
                &preferences
            ),
            "hold"
        );
    }
    assert_eq!(count(&c, "erp_capture"), 0);
    assert_eq!(
        save(
            &mut automatic,
            &tab(&input, plain),
            &named("  합성 담당자  ")
        ),
        "stored"
    );
    assert_eq!(count(&c, "erp_capture"), 1);
    // 새 coordinator도 같은 내용을 보고 영수증을 늘리지 않는다.
    let mut second = AutoCapture::new(&h.db, &home, &h.env);
    assert_eq!(
        save(&mut second, &tab(&input, plain), &preferences),
        "unchanged"
    );
    assert_eq!(
        save(&mut automatic, &tab(&input, plain), &named("담당자")),
        "unchanged"
    );
    assert_eq!(count(&c, "erp_capture"), 1);
    assert_eq!(
        save(
            &mut automatic,
            &tab(
                &without_staff,
                TabOptions {
                    stable: false,
                    ..plain
                }
            ),
            &named("")
        ),
        "hold"
    );
    assert_eq!(
        save(
            &mut automatic,
            &tab(
                &without_staff,
                TabOptions {
                    target: Some("other"),
                    ..plain
                }
            ),
            &named("")
        ),
        "hold"
    );
    assert_eq!(
        save(&mut automatic, &tab(&without_staff, plain), &named("  ")),
        "stored"
    );
    assert_eq!(count(&c, "erp_capture"), 2);
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
    // 기관담당자로 업무 담당자를 대신하지 않는다.
    assert_eq!(
        save(&mut automatic, &tab(&request, plain), &preferences),
        "hold"
    );
    assert_eq!(
        save(&mut automatic, &tab(&request, plain), &named("")),
        "stored"
    );
    assert_eq!(count(&c, "erp_capture"), 3);
    let matching_staff = [
        "합성 담당자(010-0000-0000)",
        " 합성 담당자 ( ☎ 000-0000-0000 ) ",
        "계약팀 합성 담당자 대리",
        "합성 담당자2",
    ];
    for staff in matching_staff {
        let snapshot = with_staff(&input, staff);
        assert_eq!(
            save(&mut automatic, &tab(&snapshot, plain), &preferences),
            "stored"
        );
        assert_eq!(
            text_of(&c, "SELECT staff FROM contract WHERE seq = '00'"),
            staff.trim()
        );
    }
    assert_eq!(count(&c, "erp_capture"), 3 + matching_staff.len() as i64);
    home.write_config(&h._temp.path("다른.pclm"), "other");
    assert_eq!(
        save(&mut automatic, &tab(&input, plain), &preferences),
        "hold"
    );
}

/// C# MirrorTests.자동수집은_정상변경과_되돌아온값을_저장하고_누락행_빈칸_사람정정은_유지한다
#[test]
fn 자동수집은_정상변경과_되돌아온값을_저장하고_누락행_빈칸_사람정정은_유지한다() {
    let h = Harness::new("mirror-auto-change");
    let home = Pointing::none();
    let c = h.write();
    let dataset = h.dataset();
    home.write_config(&h.path, &dataset);
    let now = 1_800_000_000_000_i64;
    let preferences = AutoCapturePreferences {
        enabled: true,
        name: "합성 담당자".into(),
    };
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
        // 메뉴는 01579 로 못 박는다(C# 도 그렇다).
        let mut tab = auto_tab(input, &dataset, now, TabOptions::default());
        tab["menu"] = json!("01579");
        tab
    };
    let first = with_staff_or_not(
        &contract("00", "합성 A", "20260914", &[(1, 1, 1000), (2, 1, 1000)]),
        true,
    );
    assert_eq!(
        state_of(automatic.try_save(&tab(&first), &preferences, now, None)),
        "stored"
    );
    let second = with_staff_or_not(
        &contract("00", "합성 B", "20260914", &[(1, 1, 1000), (2, 1, 1000)]),
        true,
    );
    assert_eq!(
        state_of(automatic.try_save(&tab(&second), &preferences, now, None)),
        "stored"
    );
    assert_eq!(text_of(&c, "SELECT title FROM contract"), "합성 B");
    assert_eq!(
        state_of(automatic.try_save(&tab(&first), &preferences, now, None)),
        "stored"
    );
    assert_eq!(text_of(&c, "SELECT title FROM contract"), "합성 A");
    assert_eq!(count(&c, "erp_capture"), 3);
    automatic
        .try_save(&tab(&first), &preferences, now, None)
        .unwrap();
    assert_eq!(count(&c, "erp_capture"), 3);
    // DB의 담당자를 조건으로 보충하지 않는다.
    assert_eq!(
        state_of(automatic.try_save(
            &tab(&with_staff_or_not(&first, false)),
            &preferences,
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
        state_of(automatic.try_save(&tab(&partial), &preferences, now, None)),
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

/// C# MirrorTests.수집_안_함의_안내는_매핑에서_화면을_단_프로필을_읽는다
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
            screen("01117", "request", "접수", "조달요구 접수(목록형)"),
            screen("01112", "request", "접수", "조달요구 접수 상세(일반용역)"),
            screen("01179", "notice", "공고", "입찰공고 상세"),
            screen("01181", "notice", "공고", "입찰공고 상세(일반용역)"),
            screen("01196", "notice", "공고", "입찰공고상세조회(통합)"),
            screen("01579", "contract", "계약", "계약 상세"),
            screen("01608", "contract", "계약", "물품·일반용역 변경계약서 작성"),
        ],
        mirror::supported_screens(&h.db, &h.env)
    );
}

/// C# MirrorTests.새_자료는_모든_칸을_새로_담길_것으로_비추고_작업자료에는_아무것도_남기지_않는다
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

/// C# MirrorTests.현행이_미정이면_공고_후보를_모두_비추고_정해지면_그_공고만_비춘다
///
/// C# 이 먼저 보는 `NoticeGroups.현행공고` 가 비었다는 줄은 옮기지 못했다 — 그 SQL 은 크레이트 안(`pub(crate)`)이다.
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

/// C# MirrorTests.새_차수는_가장_늦은_차수와_견주고_칸은_저장한_뒤의_계약면과_같은_표기다
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

/// C# MirrorTests.옛_차수를_다시_비추면_더_늦은_차수가_아니라_그_차수와_견준다
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

/// C# MirrorTests.사람이_고친_칸의_아래_값이_바뀌면_검토_대기로_비추고_창의_가져오기는_고른_칸만_복원한다
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
    // C# 은 두 영수증을 같은 옵션으로 직렬화해 견준다. Rust 의 처음 저장은 다리 꼴(연결 칸이 camelCase)이고 재전송은 적어 둔
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

/// C# MirrorTests.바뀌는_값은_늘_적용하고_빈_값으로_지우는_변경은_고를_때만_비운다
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

/// C# MirrorTests.기본_선택은_고를_것이_있는_충돌마다_빠짐없이_하나씩_낸다
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

/// C# MirrorTests.화면에_없는_행은_그대로_두는_것이_기본이고_고르면_지우며_계약_총액이_맞는지_미리_알린다
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

/// C# MirrorTests.작업자료의_줄과_짝이_없는_화면의_줄은_새_줄로_가져온다
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

/// C# MirrorTests.수집_규칙에_없는_화면은_투영하지_못하고_까닭을_던진다
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

/// C# MirrorTests.면_열의_재료는_열_정의에서_끌어낸다
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

/// C# MirrorTests.칸마다_채우는_화면의_자리와_이름표를_잇고_화면_표기는_보이는_글이_다를_때만_곁에_둔다
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
    assert_eq!(mirror::export(&tab), None); // 옛 확장은 내보내기 결과를 싣지 않는다.
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
/// C# MirrorTests.앱이_부른_내보내기의_결과는_탭_보고에_실려_그대로_건너오고_모양이_틀리면_모르는_것으로_둔다 (Mirror.Export 쪽)
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

/// C# ScreensTests.읽지_못하는_매핑_글은_한국어_사유로_거절한다
#[test]
fn 읽지_못하는_매핑_글은_한국어_사유로_거절한다() {
    // 매핑 편집기의 저장·샘플 검증·활성화가 .NET 의 영어 예외 대신 이 사유를 본다.
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

/// C# ScreensTests.허용_목록은_기본_매핑에서_화면을_단_일곱_프로필이고_화면마다_프로필_하나다
#[test]
fn 허용_목록은_기본_매핑에서_화면을_단_일곱_프로필이고_화면마다_프로필_하나다() {
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
    assert_eq!(7, codes.len());
    assert_eq!(
        Some("g2b-contract-v1"),
        screens::profile_for(&defaults, Some("01579")).map(|p| p.id())
    );
    assert!(screens::profile_for(&defaults, Some("01175")).is_none());
    assert!(screens::profile_for(&defaults, None).is_none());
}

/// C# ScreensTests.화면과_고유키는_편집본이_바꿀_수_없다
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

/// C# ScreensTests.화면을_적기_전에_저장한_매핑은_기본의_화면을_얻고_고유키는_얻지_않는다
#[test]
fn 화면을_적기_전에_저장한_매핑은_기본의_화면을_얻고_고유키는_얻지_않는다() {
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
}

/// C# ScreensTests.프로필이_더해지기_전에_저장한_매핑은_빠진_프로필을_기본에서_얻고_고친_프로필은_그대로다
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

/// C# ScreensTests.기본에서_거둔_필드가_남은_편집본은_읽을_때_그_필드만_떨구고_새로_저장하는_길은_거절한다
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

/// C# ScreensTests.메뉴_경로에서_비지_않은_가장_깊은_단을_화면의_번호로_본다
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

/// C# ScreensTests.파일은_번호가_없거나_목록에_없거나_그_화면의_프로필이_아니면_거절한다
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
    assert_eq!(
        "메뉴 01114 화면은 수집하지 않습니다.",
        reason(t.inspect(&request_file(
            &[("depth1", "01001"), ("depth2", "01114")],
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

/// C# ScreensTests.실제_화면은_확장이_읽은_번호로만_가리고_저장과_창의_투영도_같은_문을_지난다
#[test]
fn 실제_화면은_확장이_읽은_번호로만_가리고_저장과_창의_투영도_같은_문을_지난다() {
    let t = ScreensHarness::new();
    const UNREAD: &str =
        "화면 번호를 읽지 못했습니다. 접수·공고·계약 상세 화면에서 다시 읽어 주세요.";
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

/// C# ScreensTests.통합_공고_상세는_공고_B_로_읽고_비어_온_기초금액_표는_걷지_않은_값이다
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

/// C# ScreensTests.변경계약서_작성_화면은_품목_표의_번호로_그_차수의_계약이고_업체는_바뀐_뒤의_표만_읽는다
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

/// C# ScreensTests.일반용역_접수_상세는_접수_B_로_읽고_다른_프로필로_열면_맞는_프로필을_알려_준다
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

/// C# ScreensTests.일반용역_공고_상세는_공고_A_의_칸으로_읽고_미리보기가_한_벌_더_실은_첨부는_하나로_본다
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

/// C# ScreensTests.계약특성과_계약보증서_제출방법은_사람이_화면에서_확인한_명칭으로_담는다 (g2b-contract-v1)
#[test]
fn 계약특성과_계약보증서_제출방법은_사람이_화면에서_확인한_명칭으로_담는다_계약() {
    contract_trait_names("g2b-contract-v1");
}

/// C# ScreensTests.계약특성과_계약보증서_제출방법은_사람이_화면에서_확인한_명칭으로_담는다 (g2b-contract-change-v1)
#[test]
fn 계약특성과_계약보증서_제출방법은_사람이_화면에서_확인한_명칭으로_담는다_변경계약() {
    contract_trait_names("g2b-contract-change-v1");
}

/// C# ScreensTests.계약특성_코드_사전은_계약_프로필_모두가_같은_것을_쓴다
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

/// C# ScreensTests.일반용역_화면이_오기_전에_저장한_매핑은_새_공고_프로필과_접수_B_의_화면을_기본에서_얻는다
#[test]
fn 일반용역_화면이_오기_전에_저장한_매핑은_새_공고_프로필과_접수_b_의_화면을_기본에서_얻는다() {
    const ADDED: &str = "g2b-notice-a-service-v1";
    let mut node = defaults_node();
    node["profiles"]
        .as_array_mut()
        .unwrap()
        .retain(|p| p["id"] != ADDED);
    profile_node(&mut node, "g2b-request-b-v1")
        .as_object_mut()
        .unwrap()
        .remove("screen");
    let parsed = mapping::parse(&node_text(&node)).unwrap();
    let find = |id: &str| parsed.profiles_iter().find(|p| p.id() == id).unwrap();
    assert_eq!(
        profile_hash(&default_profile(ADDED)),
        profile_hash(find(ADDED))
    );
    assert_eq!(
        Some(ADDED),
        screens::profile_for(&parsed, Some("01181")).map(|p| p.id())
    );
    assert_eq!(
        Some("g2b-request-b-v1"),
        screens::profile_for(&parsed, Some("01112")).map(|p| p.id())
    );
    assert_eq!(
        mapping::revision(&mapping::defaults()),
        mapping::revision(&parsed)
    );
}

/// C# ScreensTests.확장은_인사의_매핑에서_목록을_받고_스냅샷의_화면_번호는_모르는_칸이_아니다
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
        "inspect",
        json!({ "datasetId": t.dataset, "snapshot": notice_live(Some("01175"), "g2b-notice-a-v1").to_value() }),
    );
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(
        refused["error"]["message"],
        json!("메뉴 01175 화면은 수집하지 않습니다.")
    );
    assert_eq!(
        call(
            "inspect",
            json!({ "datasetId": t.dataset, "snapshot": notice_live(Some("01179"), "g2b-notice-a-v1").to_value() })
        )["ok"],
        json!(true)
    );
}

/// C# ScreensTests.수집_이력은_화면_번호를_적어_두고_재검토는_그_번호로_가린다
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

/// C# ScreensTests.화면_번호가_없는_입력은_예전과_같은_해시를_낸다
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

/// C# CaptureLogTests.오늘_것만_종류와_결과로_가르고_마지막은_오늘이_아니어도_낸다
///
/// C# 은 `DateTime.Now` 꼴의 로컬 시각을 이 기계의 시간대로 읽었다 — 여기서는 한국 표준시(+9)로 못 박고, 경계(어제 자정
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

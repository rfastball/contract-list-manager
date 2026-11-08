//! 다리의 읽기 요청과 문지기. C# 쪽에는 다리 시험이 없다(창 프로젝트에 시험이 없다) — 같은 바이트인지는 동등성
//! 계기의 `bridge` 가 본다(crates/pclm-parity/README.md). 여기서는 갈래마다 한 번씩 선다. 값은 지어낸 것이다.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use pclm_core::storage::pclm_file;
use pclm_core::{Access, Database, Env};
use pclm_home::Home;
use serde_json::Value;

use super::*;

struct Fixed;

impl Env for Fixed {
    fn now_utc(&self) -> String {
        "2026-09-10T00:00:00.0000000Z".into()
    }
    fn new_id(&self) -> String {
        "0".repeat(32)
    }
}

static ENV: Fixed = Fixed;
/// 「오늘」 은 KST — 2026-09-10 09:00.
static ZONE: pclm_core::local_time::FixedZone = pclm_core::local_time::FixedZone(9 * 3600);

struct Temp(PathBuf);

impl Temp {
    fn new(name: &str) -> Temp {
        let dir = std::env::temp_dir().join(format!(
            "pclm-bridge-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Temp(dir)
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// 작업자료 하나와 그 위의 다리. 공고 하나·계약 하나를 심는다.
fn work(temp: &Temp) -> (PathBuf, Home) {
    let file = temp.0.join("work.pclm");
    pclm_file::create(&file, "work", &ENV).unwrap();
    let c = rusqlite::Connection::open(&file).unwrap();
    c.execute_batch(
        "INSERT INTO notice_group (group_base) VALUES ('TNOTICE1');
         INSERT INTO notice_series (notice_base, group_base) VALUES ('TNOTICE1', 'TNOTICE1');
         INSERT INTO notice (notice_base, seq, title, updated_at) VALUES ('TNOTICE1', '000', '시험 공고', 'x');
         INSERT INTO contract_series (contract_base) VALUES ('TCONTRACT1');
         INSERT INTO contract (contract_base, seq, title, amount, updated_at) VALUES ('TCONTRACT1', '00', '시험 계약', 1234, 'x');",
    )
    .unwrap();
    drop(c);
    let home = Home::new(temp.0.join("home")).unwrap();
    (file, home)
}

fn bridge(file: &std::path::Path, home: Home) -> Bridge<'static> {
    Bridge::new(Database::new(file, Access::Write), home, None, &ENV, &ZONE).unwrap()
}

fn args(values: &[&str]) -> Vec<Option<String>> {
    values.iter().map(|v| Some(v.to_string())).collect()
}

#[test]
fn 답_글은_csharp_꼴이다() {
    let result = serde_json::json!({ "이름": "가+<b>" });
    assert_eq!(
        reply_json(3, true, Some(&result), None),
        r#"{"id":3,"ok":true,"result":{"\uC774\uB984":"\uAC00\u002B\u003Cb\u003E"},"error":null}"#
    );
    assert_eq!(
        reply_json(4, false, None, Some("모르는 요청입니다: x")),
        r#"{"id":4,"ok":false,"result":null,"error":"\uBAA8\uB974\uB294 \uC694\uCCAD\uC785\uB2C8\uB2E4: x"}"#
    );
}

#[test]
fn 모르는_요청과_옮기지_않은_요청과_빠진_값을_가려_거절한다() {
    let temp = Temp::new("refuse");
    let (file, home) = work(&temp);
    let b = bridge(&file, home);
    assert_eq!(
        b.invoke("nope", &[]).unwrap_err(),
        "모르는 요청입니다: nope"
    );
    assert_eq!(
        b.invoke("setField", &[]).unwrap_err(),
        "1번째 값이 없습니다."
    );
    assert_eq!(b.invoke("sheet", &[]).unwrap_err(), "1번째 값이 없습니다.");
    assert_eq!(
        b.invoke("sheet", &[None]).unwrap_err(),
        "1번째 값이 없습니다."
    );
    assert_eq!(
        b.invoke("sheet", &args(&["notice"])).unwrap_err(),
        "내보이지 않는 표입니다: notice"
    );
    assert_eq!(
        b.invoke("sheet", &args(&["u_없는 뷰"])).unwrap_err(),
        "내보이지 않는 표입니다: u_없는 뷰"
    );
}

/// 문지기의 두 목록은 C# Dispatch 가 아는 요청 안에 들고, 서로 겹치지 않는다. 아는 요청은 모두 「모르는 요청」 이 아니다.
#[test]
fn 문지기의_목록은_다리가_아는_요청_안에_든다() {
    assert!(WRITES.iter().all(|m| METHODS.contains(m)));
    assert!(VIEWER_ALLOWED.iter().all(|m| METHODS.contains(m)));
    assert!(WRITES.iter().all(|m| !VIEWER_ALLOWED.contains(m)));
    let temp = Temp::new("known");
    let (file, home) = work(&temp);
    let b = bridge(&file, home);
    for method in METHODS {
        // 값이 없어도 되는 요청은 실제로 돈다 — 창을 띄우거나 등록부에 닿는 것은 빼고 본다.
        if matches!(*method, "openOther" | "revealDataFolder" | "prepareExtension") {
            continue;
        }
        let unknown = format!("모르는 요청입니다: {method}");
        if let Err(e) = b.invoke(method, &[]) {
            assert_ne!(e, unknown);
        }
    }
}

#[test]
fn 읽기_요청이_선다() {
    let temp = Temp::new("reads");
    let (file, home) = work(&temp);
    let b = bridge(&file, home);
    assert!(!b.read_only());
    assert_eq!(b.invoke("status", &[]).unwrap()["noticeRows"], 1);
    assert_eq!(
        b.invoke("dashboard", &[]).unwrap()["asOf"],
        "2026-09-10T09:00:00.0000000+09:00"
    );
    assert!(b.invoke("dataVersion", &[]).unwrap().is_i64());
    assert!(b.invoke("현황", &[]).unwrap()["groups"].is_array());
    assert_eq!(
        b.invoke("outline", &[]).unwrap()["chains"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

/// 다른 연결이 쓰면 다리가 쥔 연결의 data_version 이 바뀐다 — 화면이 새로 읽을 때를 안다.
#[test]
fn 자료가_바뀌면_data_version_이_바뀐다() {
    let temp = Temp::new("version");
    let (file, home) = work(&temp);
    let b = bridge(&file, home);
    let before = b.invoke("dataVersion", &[]).unwrap();
    assert_eq!(before, b.invoke("dataVersion", &[]).unwrap());
    rusqlite::Connection::open(&file)
        .unwrap()
        .execute_batch("UPDATE notice SET title = '바뀐 공고';")
        .unwrap();
    assert_ne!(before, b.invoke("dataVersion", &[]).unwrap());
}

#[test]
fn 표는_열과_줄과_고칠_자리를_낸다() {
    let temp = Temp::new("sheet");
    let (file, home) = work(&temp);
    let b = bridge(&file, home);
    let sheet = b.invoke("sheet", &args(&["v_계약"])).unwrap();
    assert_eq!(sheet["name"], "v_계약");
    let columns = sheet["columns"].as_array().unwrap();
    assert!(columns.iter().any(|c| c == "계약번호"));
    let rows = sheet["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["계약번호"], "TCONTRACT100");
    assert_eq!(rows[0]["계약금액"], "1,234");
    // 열쇠 열은 고칠 수 없다.
    let correctable = sheet["correctable"].as_array().unwrap();
    assert!(!correctable.iter().any(|c| c == "계약번호"));
    assert!(correctable.iter().any(|c| c == "계약건명"));
    assert_eq!(sheet["overrides"], Value::Object(Map::new()));

    // 읽기 전용 뷰는 고칠 자리가 없다.
    let read_only = b.invoke("sheet", &args(&["v_계약차수"])).unwrap();
    assert_eq!(read_only["correctable"], serde_json::json!([]));
    assert_eq!(read_only["editable"], serde_json::json!([]));
}

#[test]
fn 덮개는_줄_키로_묶여_온다() {
    let temp = Temp::new("overrides");
    let (file, home) = work(&temp);
    Store::new(Database::new(&file, Access::Write), &ENV)
        .set_override(
            &EntityRef::new("contract", "TCONTRACT1", "00"),
            "계약건명",
            "고친 계약",
        )
        .unwrap();
    let b = bridge(&file, home);
    let sheet = b.invoke("sheet", &args(&["v_계약"])).unwrap();
    assert_eq!(sheet["overrides"]["TCONTRACT100"]["계약건명"], "시험 계약");
    assert_eq!(sheet["rows"][0]["계약건명"], "고친 계약");
}

#[test]
fn 무한대와_blob_은_csharp_의_글로_적는다() {
    assert_eq!(cell(ValueRef::Real(f64::INFINITY)), "∞");
    assert_eq!(cell(ValueRef::Real(f64::NEG_INFINITY)), "-∞");
    assert_eq!(cell(ValueRef::Real(1e15)), "1000000000000000");
    assert_eq!(cell(ValueRef::Real(0.5)), "0.5");
    assert_eq!(cell(ValueRef::Blob(b"x")), "System.Byte[]");
    assert_eq!(cell(ValueRef::Null), "");
    assert_eq!(cell(ValueRef::Integer(-3)), "-3");
}

#[test]
fn 열람_중에는_읽기만_받고_고치는_요청은_사람_말로_거절한다() {
    let temp = Temp::new("viewing");
    let (file, home) = work(&temp);
    let view = home.view_directory();
    std::fs::create_dir_all(&view).unwrap();
    let viewed = pclm_file::open_view(&file, &view.join("열람-시험.pclm"), &ENV).unwrap();
    let database = viewed.database().clone();
    let b = Bridge::new(database, home, Some(viewed), &ENV, &ZONE).unwrap();
    assert!(b.read_only());
    let refusal =
        "열람 중인 자료라 고칠 수 없습니다 — 작업자료 「work.pclm」 는 이 창에서 읽기만 합니다.";
    assert_eq!(b.invoke("setField", &[]).unwrap_err(), refusal);
    assert_eq!(b.invoke("nope", &[]).unwrap_err(), refusal);
    // 열람 중이면 모든 표가 읽기 전용이다.
    let sheet = b.invoke("sheet", &args(&["v_계약"])).unwrap();
    assert_eq!(sheet["correctable"], serde_json::json!([]));
    assert_eq!(sheet["rows"].as_array().unwrap().len(), 1);
    assert_eq!(b.invoke("status", &[]).unwrap()["contractRows"], 1);
    // 다리를 놓으면 열람 사본이 걷힌다 — 연결이 열린 채 걷으면 Windows 가 지우지 못해 남는다.
    drop(b);
    assert_eq!(std::fs::read_dir(&view).unwrap().count(), 0);
}

/// 작업자료 위의 다리를 놓으면 곁파일(-wal·-shm)이 남지 않는다 — C# 은 풀의 쓰기 연결이 마지막에 닫혀 걷는다.
#[test]
fn 다리를_놓으면_곁파일이_남지_않는다() {
    let temp = Temp::new("sidecars");
    let (file, home) = work(&temp);
    let b = bridge(&file, home);
    b.invoke("sheet", &args(&["v_계약"])).unwrap();
    b.invoke("dataVersion", &[]).unwrap();
    drop(b);
    for suffix in ["-wal", "-shm"] {
        let sidecar = PathBuf::from(format!("{}{suffix}", file.display()));
        assert!(!sidecar.exists(), "{}", sidecar.display());
    }
}

#[test]
fn 파일_이름은_마지막_구분자_뒤다() {
    assert_eq!(file_name(std::path::Path::new(r"C:\a\b.pclm")), "b.pclm");
    assert_eq!(file_name(std::path::Path::new("a/b/c.pclm")), "c.pclm");
    assert_eq!(file_name(std::path::Path::new("c.pclm")), "c.pclm");
}

// ── 설정 · 엑셀(#8 ⑥) ─────────────────────────────────────────────

/// 대본이 고른 자리를 내주고, 건네받은 것을 적어 둔다.
#[derive(Default)]
struct Picker {
    save: Mutex<Option<String>>,
    plan: Mutex<Option<String>>,
    prompts: Mutex<Vec<SavePrompt>>,
    plan_asked: Mutex<u32>,
}

impl Host for Picker {
    fn pick_save_path(&self, prompt: &SavePrompt) -> Option<String> {
        self.prompts.lock().unwrap().push(prompt.clone());
        self.save.lock().unwrap().clone()
    }

    fn pick_plan_excel(&self, prompt: &OpenPrompt) -> Option<String> {
        assert_eq!(prompt.title, "계획 엑셀 고르기");
        *self.plan_asked.lock().unwrap() += 1;
        self.plan.lock().unwrap().clone()
    }
}

fn path_text(p: &std::path::Path) -> String {
    p.to_string_lossy().into_owned()
}

fn snapshot_json() -> String {
    r#"{"columns":["번호","메모"],"rows":[["0002","=1+1"],["0001","둘째"]]}"#.into()
}

#[test]
fn 설정을_읽고_이름은_주지_않으면_그대로_둔다() {
    let temp = Temp::new("settings");
    let (file, home) = work(&temp);
    let b = bridge(&file, home);
    assert_eq!(
        b.invoke("settings", &[]).unwrap(),
        serde_json::json!({ "submitterName": "", "planPath": "" })
    );
    assert_eq!(
        b.invoke("saveSettings", &args(&[" 홍길동 "])).unwrap()["submitterName"],
        "홍길동"
    );
    assert_eq!(
        b.invoke("saveSettings", &[]).unwrap()["submitterName"],
        "홍길동"
    );
    assert_eq!(
        b.invoke("saveSettings", &[None]).unwrap()["submitterName"],
        "홍길동"
    );
}

/// C# 에서 고르기 창의 속성이 비었을 때처럼 — 그만둔 것으로 보고 null 을 낸다.
#[test]
fn 고르기_창이_없으면_그만둔다() {
    let temp = Temp::new("nopick");
    let (file, home) = work(&temp);
    let b = bridge(&file, home);
    assert_eq!(b.invoke("export", &[]).unwrap(), Value::Null);
    assert_eq!(
        b.invoke(
            "exportSheet",
            &[Some("선택 자료".into()), Some(snapshot_json())]
        )
        .unwrap(),
        Value::Null
    );
    assert_eq!(b.invoke("pickPlanExcel", &[]).unwrap(), Value::Null);
}

#[test]
fn 내보내기는_고른_자리에_쓰고_전체_경로를_낸다() {
    let temp = Temp::new("export");
    let (file, home) = work(&temp);
    let picker = Picker::default();
    let target = temp.0.join("out").join("관리대장.xlsx");
    *picker.save.lock().unwrap() = Some(path_text(&target));
    let mut b = bridge(&file, home);
    b.set_host(&picker);

    assert_eq!(b.invoke("export", &[]).unwrap(), path_text(&target));
    assert!(target.exists());
    assert_eq!(
        picker.prompts.lock().unwrap()[0],
        SavePrompt {
            title: "엑셀로 내보내기".into(),
            filter: "엑셀 통합 문서 (*.xlsx)|*.xlsx".into(),
            default_ext: ".xlsx".into(),
            file_name: "관리대장_20260910.xlsx".into(),
            allow_overwrite: true,
        }
    );

    // 빈 글은 그만둔 것이다.
    *picker.save.lock().unwrap() = Some(String::new());
    assert_eq!(b.invoke("export", &[]).unwrap(), Value::Null);
}

/// 검사가 고르기 창보다 먼저다 — 거절할 표를 두고 자리부터 묻지 않는다.
#[test]
fn 화면_사진은_검사한_뒤에_자리를_묻는다() {
    let temp = Temp::new("sheetexport");
    let (file, home) = work(&temp);
    let picker = Picker::default();
    let target = temp.0.join("선택.xlsx");
    *picker.save.lock().unwrap() = Some(path_text(&target));
    let mut b = bridge(&file, home);
    b.set_host(&picker);

    let call = |title: &str, json: Option<&str>| {
        b.invoke(
            "exportSheet",
            &[Some(title.to_string()), json.map(String::from)],
        )
    };
    let size =
        "내보낼 표의 크기나 셀 값이 올바르지 않습니다. 최대 2,000,000셀을 내보낼 수 있습니다.";
    assert_eq!(
        call("a/b", Some(&snapshot_json())).unwrap_err(),
        "엑셀 시트 이름이 올바르지 않습니다."
    );
    assert_eq!(
        call("선택 자료", Some("null")).unwrap_err(),
        "작업할 자료가 없습니다."
    );
    assert_eq!(call("선택 자료", None).unwrap_err(), "2번째 값이 없습니다.");
    assert_eq!(
        call(
            "선택 자료",
            Some(r#"{"columns":["번호"],"rows":[["1","2"]]}"#)
        )
        .unwrap_err(),
        size
    );
    // 빠진 속성은 null 이라 검사가 거절한다.
    assert_eq!(
        call("선택 자료", Some(r#"{"columns":["번호"]}"#)).unwrap_err(),
        size
    );
    assert!(call("선택 자료", Some("[]")).is_err());
    assert!(picker.prompts.lock().unwrap().is_empty());

    assert_eq!(
        call("선택 자료", Some(&snapshot_json())).unwrap(),
        path_text(&target)
    );
    assert!(target.exists());
    assert_eq!(
        picker.prompts.lock().unwrap()[0].title,
        "현재 표를 엑셀로 내보내기"
    );
    assert_eq!(
        picker.prompts.lock().unwrap()[0].file_name,
        "선택 자료_20260910.xlsx"
    );
}

/// 엑셀이 붙든 파일(IOException)은 사람 말로, 폴더인 자리(UnauthorizedAccessException)는 .NET 의 글로.
#[cfg(windows)]
#[test]
fn 쓰지_못하면_csharp_과_같은_글로_거절한다() {
    use std::os::windows::fs::OpenOptionsExt as _;
    let temp = Temp::new("writefail");
    let (file, home) = work(&temp);
    let picker = Picker::default();
    let mut b = bridge(&file, home);
    b.set_host(&picker);

    let locked = temp.0.join("열린.xlsx");
    std::fs::write(&locked, "").unwrap();
    let hold = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(0)
        .open(&locked)
        .unwrap();
    *picker.save.lock().unwrap() = Some(path_text(&locked));
    assert_eq!(
        b.invoke("export", &[]).unwrap_err(),
        format!(
            "파일을 쓰지 못했습니다 — 엑셀에서 열려 있다면 닫고 다시 해 주세요.  {}",
            locked.display()
        )
    );
    drop(hold);

    let folder = temp.0.join("폴더.xlsx");
    std::fs::create_dir_all(&folder).unwrap();
    *picker.save.lock().unwrap() = Some(path_text(&folder));
    assert_eq!(
        b.invoke(
            "exportSheet",
            &[Some("선택 자료".into()), Some(snapshot_json())]
        )
        .unwrap_err(),
        format!("Access to the path '{}' is denied.", folder.display())
    );
}

#[test]
fn 계획_엑셀을_골라_두고_넣는다() {
    let temp = Temp::new("plan");
    let (file, home) = work(&temp);
    let picker = Picker::default();
    let mut b = bridge(&file, home);
    b.set_host(&picker);

    assert_eq!(
        b.invoke("importPlan", &[]).unwrap_err(),
        "가져올 계획 엑셀을 고르지 않았습니다."
    );

    // 열지 못하는 파일은 골라 둔 것으로 남기지 않는다.
    let missing = temp.0.join("없는.xlsx");
    *picker.plan.lock().unwrap() = Some(path_text(&missing));
    assert_eq!(
        b.invoke("pickPlanExcel", &[]).unwrap_err(),
        format!("Could not find file '{}'.", missing.display())
    );
    assert_eq!(b.invoke("settings", &[]).unwrap()["planPath"], "");

    let plan = temp.0.join("계획.xlsx");
    let text = |v: &str| pclm_excel::writer::Cell {
        value: pclm_excel::writer::Value::Text(v.into()),
        style: pclm_excel::writer::Style::DEFAULT,
    };
    let sheet = pclm_excel::writer::Sheet {
        name: "계획".into(),
        rows: vec![
            vec![text("조달요구번호"), text("담당자"), text("지시수량")],
            vec![text("PLAN-0001"), text("김철수"), text("1,200")],
        ],
        ..Default::default()
    };
    pclm_excel::writer::write(std::fs::File::create(&plan).unwrap(), &[sheet]).unwrap();
    *picker.plan.lock().unwrap() = Some(path_text(&plan));
    assert_eq!(
        b.invoke("pickPlanExcel", &[]).unwrap(),
        serde_json::json!({ "name": "계획.xlsx", "path": path_text(&plan) })
    );
    assert_eq!(
        b.invoke("settings", &[]).unwrap()["planPath"],
        path_text(&plan)
    );

    let result = b.invoke("importPlan", &[]).unwrap();
    assert_eq!(
        // 선언 차례 그대로 — rows · created · updated · skipped · missingRequired · found · errors · ok.
        bridge_json(&result),
        bridge_json(&serde_json::json!({
            "rows": 1, "created": 1, "updated": 0, "skipped": 0, "missingRequired": [],
            "found": ["조달요구번호", "지시수량", "담당자"], "errors": [], "ok": true
        }))
    );
    assert_eq!(b.invoke("importPlan", &[]).unwrap()["updated"], 1);

    std::fs::remove_file(&plan).unwrap();
    assert_eq!(
        b.invoke("importPlan", &[]).unwrap_err(),
        format!("그런 파일이 없습니다: {}", plan.display())
    );
}

/// 열람 중에는 내보내기와 설정 읽기만 받는다 — 계획 고르기 · 넣기 · 설정 저장은 고치는 요청이다.
#[test]
fn 열람_중에도_내보내기는_받는다() {
    let temp = Temp::new("viewexport");
    let (file, home) = work(&temp);
    let view = home.view_directory();
    std::fs::create_dir_all(&view).unwrap();
    let viewed = pclm_file::open_view(&file, &view.join("열람-시험.pclm"), &ENV).unwrap();
    let database = viewed.database().clone();
    let picker = Picker::default();
    let target = temp.0.join("열람.xlsx");
    *picker.save.lock().unwrap() = Some(path_text(&target));
    *picker.plan.lock().unwrap() = Some(path_text(&target));
    let mut b = Bridge::new(database, home, Some(viewed), &ENV, &ZONE).unwrap();
    b.set_host(&picker);

    assert_eq!(b.invoke("export", &[]).unwrap(), path_text(&target));
    assert!(b.invoke("settings", &[]).is_ok());
    let refusal =
        "열람 중인 자료라 고칠 수 없습니다 — 작업자료 「work.pclm」 는 이 창에서 읽기만 합니다.";
    for method in ["pickPlanExcel", "importPlan", "saveSettings"] {
        assert_eq!(b.invoke(method, &[]).unwrap_err(), refusal, "{method}");
    }
    assert_eq!(*picker.plan_asked.lock().unwrap(), 0);
}

// ── 창 · 운영체제 · 깃발(#8 ⑧a) ────────────────────────────────────

/// 시킨 것을 적어 두는 운영체제 — 진짜 등록부·탐색기·브라우저에 닿지 않는다.
#[derive(Default)]
struct FakeOs {
    exe: Option<PathBuf>,
    default_home: PathBuf,
    calls: Mutex<Vec<String>>,
    registry: Mutex<Vec<(String, String, String)>>,
    signal: bool,
}

impl FakeOs {
    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
    fn note(&self, call: String) {
        self.calls.lock().unwrap().push(call);
    }
}

impl Os for FakeOs {
    fn process_path(&self) -> Option<PathBuf> {
        self.exe.clone()
    }
    fn default_home(&self) -> std::io::Result<PathBuf> {
        Ok(self.default_home.clone())
    }
    fn special_folder(&self, _: SpecialFolder) -> PathBuf {
        self.default_home.join("special")
    }
    fn temp_path(&self) -> PathBuf {
        self.default_home.join("temp\\")
    }
    fn registry_get(
        &self,
        _: RegistryRoot,
        key: &str,
        name: &str,
    ) -> std::io::Result<Option<RegistryValue>> {
        Ok(self
            .registry
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|(k, n, _)| k == key && n == name)
            .map(|(_, _, v)| RegistryValue::String(v.clone())))
    }
    fn registry_set_string(&self, key: &str, name: &str, value: &str) -> std::io::Result<()> {
        self.note(format!("set {key} [{name}] = {value}"));
        self.registry
            .lock()
            .unwrap()
            .push((key.into(), name.into(), value.into()));
        Ok(())
    }
    fn registry_delete_value(&self, key: &str, name: &str) -> std::io::Result<()> {
        self.note(format!("delete {key} [{name}]"));
        self.registry
            .lock()
            .unwrap()
            .retain(|(k, n, _)| !(k == key && n == name));
        Ok(())
    }
    fn shell_open(&self, path: &Path) -> std::io::Result<()> {
        self.note(format!("open {}", path.display()));
        Ok(())
    }
    fn start_process(&self, program: &Path, args: &[String], shell: bool) -> std::io::Result<()> {
        self.note(format!("start {} {:?} {shell}", program.display(), args));
        Ok(())
    }
    fn signal_activation(&self, name: &str) -> bool {
        self.note(format!("signal {name}"));
        self.signal
    }
    fn browser_windows(&self, _: &str) -> Vec<isize> {
        Vec::new()
    }
    fn raise_new_window(&self, _: &str, _: &[isize], _: std::time::Duration) -> bool {
        false
    }
    fn navigate_new_window(&self, process: &str, _: &[isize], url: &str) -> bool {
        self.note(format!("navigate {process} {url}"));
        false
    }
    fn set_clipboard(&self, text: &str) {
        self.note(format!("clipboard {text}"));
    }
    fn file_version(&self, _: &Path) -> Option<String> {
        None
    }
}

/// 다시 띄우기·닫기·알림 영역을 받은 차례대로 적는다. 고르기는 `open` 이 답한다.
#[derive(Default)]
struct Window {
    events: Mutex<Vec<String>>,
    open: Mutex<Option<String>>,
}

impl Host for Window {
    fn pick_open_file(&self, prompt: &OpenPrompt) -> Option<String> {
        self.events
            .lock()
            .unwrap()
            .push(format!("open {} {}", prompt.title, prompt.filter));
        self.open.lock().unwrap().clone()
    }
    fn restart(&self) {
        self.events.lock().unwrap().push("restart".into());
    }
    fn close_window(&self) {
        self.events.lock().unwrap().push("close".into());
    }
    fn apply_close_to_tray(&self, on: bool) {
        self.events.lock().unwrap().push(format!("tray {on}"));
    }
}

#[test]
fn 닫기는_답을_보낸_뒤에_창에_넘기고_깃발은_한_번만_넘긴다() {
    let temp = Temp::new("close");
    let (file, home) = work(&temp);
    let window = Window::default();
    let mut b = bridge(&file, home);
    b.set_host(&window);
    let posted = Mutex::new(Vec::new());
    let mut post = |reply: String| {
        window.events.lock().unwrap().push("reply".into());
        posted.lock().unwrap().push(reply);
    };

    b.handle(r#"{"id":1,"method":"setUnsaved","args":["true"]}"#, &mut post);
    assert!(b.unsaved());
    assert_eq!(
        b.ask_before_closing().unwrap(),
        r#"{"notice":"closeRequested"}"#
    );
    b.page_failed();
    assert!(!b.unsaved());
    assert_eq!(b.ask_before_closing(), None);
    b.handle(r#"{"id":2,"method":"setUnsaved","args":["예"]}"#, &mut post);
    b.handle(r#"{"id":3,"method":"setUnsaved","args":["true"]}"#, &mut post);
    b.handle(r#"{"id":4,"method":"closeWindow"}"#, &mut post);
    assert!(!b.unsaved());
    b.handle(r#"{"id":5,"method":"status"}"#, &mut post);

    let replies = posted.lock().unwrap().clone();
    assert_eq!(replies[0], r#"{"id":1,"ok":true,"result":true,"error":null}"#);
    assert_eq!(
        replies[1],
        reply_json(2, false, None, Some("켜짐·꺼짐이 아닙니다: 예"))
    );
    assert_eq!(replies[3], r#"{"id":4,"ok":true,"result":null,"error":null}"#);
    assert_eq!(
        *window.events.lock().unwrap(),
        ["reply", "reply", "reply", "reply", "close", "reply"]
    );
}

#[test]
fn 깨진_글은_번호를_읽은_만큼_답한다() {
    let temp = Temp::new("malformed");
    let (file, home) = work(&temp);
    let b = bridge(&file, home);
    let mut replies = Vec::new();
    let mut post = |r: String| replies.push(r);
    b.handle(r#"{"method":"status"}"#, &mut post);
    b.handle(r#"{"id":7,"method":3}"#, &mut post);
    b.handle(r#"{"id":8,"method":"sheet","args":[{"a":1}]}"#, &mut post);
    assert_eq!(
        replies[0],
        reply_json(
            0,
            false,
            None,
            Some("The given key was not present in the dictionary.")
        )
    );
    assert!(replies[1].starts_with(r#"{"id":7,"ok":false"#));
    // 글이 아닌 인자는 JSON 글로 넘긴다.
    assert_eq!(
        replies[2],
        reply_json(8, false, None, Some(r#"내보이지 않는 표입니다: {"a":1}"#))
    );
}

#[test]
fn 작업자료를_바꾸면_답을_보낸_뒤에_다시_띄운다() {
    let temp = Temp::new("switch");
    let (file, home) = work(&temp);
    // 쪽지가 이 작업자료를 가리키게 한다.
    let dataset: String = rusqlite::Connection::open(&file)
        .unwrap()
        .query_row("SELECT dataset_id FROM pclm_file", [], |r| r.get(0))
        .unwrap();
    home.write_config(&file, &dataset, None).unwrap();
    let window = Window::default();
    let mut b = bridge(&file, home);
    b.set_host(&window);
    let target = temp.0.join("새 자료").join("계약자료.pclm");
    let message =
        serde_json::json!({ "id": 1, "method": "newWorkfile", "args": [path_text(&target)] });
    let mut replies = Vec::new();
    b.handle(&message.to_string(), &mut |r| {
        window.events.lock().unwrap().push("reply".into());
        replies.push(r);
    });
    let reply: Value = serde_json::from_str(&replies[0]).unwrap();
    assert_eq!(reply["result"]["restart"], true, "{reply}");
    assert_eq!(reply["result"]["path"], path_text(&target));
    assert_eq!(*window.events.lock().unwrap(), ["reply", "restart"]);
    assert!(!b.take_restart());
}

#[test]
fn 업무_홈이_아니면_등록부에_닿지_않는다() {
    let temp = Temp::new("gate");
    let (file, home) = work(&temp);
    let exe = temp.0.join("계약목록.exe");
    std::fs::write(&exe, "fake").unwrap();
    let os = FakeOs {
        exe: Some(exe.clone()),
        default_home: temp.0.join("업무 홈"),
        ..FakeOs::default()
    };
    let mut b = bridge(&file, home);
    b.set_os(&os);
    let production = "내장 확장 연결은 업무 홈에서만 준비합니다. --home 이나 --erp-dev 없이 앱을 다시 실행하세요.";
    assert_eq!(b.invoke("prepareExtension", &[]).unwrap_err(), production);
    assert_eq!(b.invoke("extensionStatus", &[]).unwrap_err(), production);
    let reason =
        "시험 홈(--home)으로 띄운 창이라 자동 실행을 바꾸지 않습니다. 업무 홈의 창에서 바꾸세요.";
    let prefs = b.invoke("windowPrefs", &[]).unwrap();
    assert_eq!(prefs["autostartAvailable"], false);
    assert_eq!(prefs["autostartReason"], reason);
    assert_eq!(prefs["autostartCopyTarget"], Value::Null);
    assert_eq!(
        b.invoke("saveWindowPrefs", &args(&["true", "true"]))
            .unwrap_err(),
        reason
    );
    // 바꾸지 않는 자동 실행이면 알림 영역만 적는다.
    assert_eq!(
        b.invoke("saveWindowPrefs", &args(&["true", "false"]))
            .unwrap()["closeToTray"],
        true
    );
    assert!(os.calls().is_empty(), "{:?}", os.calls());

    b.set_default_home(true);
    let prepared = b.invoke("prepareExtension", &[]).unwrap();
    let manifest = temp.0.join("업무 홈").join("kr.rfastball.pclm.erp.json");
    assert_eq!(prepared["manifestPath"], path_text(&manifest));
    assert_eq!(prepared["extensionId"], "hnfnknkojoobmpcgnimdjkpnmannhigc");
    assert_eq!(
        os.calls(),
        [
            format!(
                r"set Software\Google\Chrome\NativeMessagingHosts\kr.rfastball.pclm.erp [] = {}",
                manifest.display()
            ),
            format!(
                r"set Software\Microsoft\Edge\NativeMessagingHosts\kr.rfastball.pclm.erp [] = {}",
                manifest.display()
            ),
        ]
    );
    let status = b.invoke("extensionStatus", &[]).unwrap();
    assert_eq!(status["prepared"], false, "확장 상태는 이 다리의 홈을 읽는다");

    os.calls.lock().unwrap().clear();
    let saved = b
        .invoke("saveWindowPrefs", &args(&["false", "true"]))
        .unwrap();
    assert_eq!(saved["autostart"], true);
    assert_eq!(saved["autostartAvailable"], true);
    assert_eq!(
        os.calls(),
        [
            format!(
                r#"set Software\Microsoft\Windows\CurrentVersion\Run [계약목록] = "{}" --background"#,
                exe.display()
            ),
            r"delete Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run [계약목록]"
                .to_string(),
        ]
    );
}

#[test]
fn 다른_자료는_새_프로세스로_열고_내_작업자료는_거절한다() {
    let temp = Temp::new("other");
    let (file, home) = work(&temp);
    let exe = temp.0.join("app.exe");
    let os = FakeOs {
        exe: Some(exe.clone()),
        ..FakeOs::default()
    };
    let window = Window::default();
    let mut b = bridge(&file, home.clone());
    b.set_os(&os);
    b.set_host(&window);
    b.set_ui_origin(Some("http://localhost:5173".into()));
    assert_eq!(b.invoke("openOther", &[]).unwrap(), Value::Null);
    *window.open.lock().unwrap() = Some(path_text(&file));
    assert_eq!(
        b.invoke("openOther", &[]).unwrap_err(),
        format!(
            "지금 이 창에서 쓰고 있는 작업자료입니다: {}",
            file.display()
        )
    );
    let other = temp.0.join("다른.pclm");
    *window.open.lock().unwrap() = Some(path_text(&other));
    assert_eq!(
        b.invoke("openOther", &[]).unwrap(),
        serde_json::json!({ "path": path_text(&other) })
    );
    assert_eq!(
        os.calls(),
        [format!(
            "start {} {:?} false",
            exe.display(),
            [
                "--view".to_string(),
                path_text(&other),
                "--home".into(),
                path_text(home.directory()),
                "--ui".into(),
                "http://localhost:5173".into()
            ]
        )]
    );
    assert_eq!(
        window.events.lock().unwrap()[0],
        "open 열어 볼 자료 고르기 계약 목록 자료 (*.pclm)|*.pclm"
    );
}

#[test]
fn 확장_설정_화면은_준비하지_않았으면_거절하고_못_찾으면_주소를_남긴다() {
    let temp = Temp::new("setup");
    let (file, home) = work(&temp);
    let os = FakeOs {
        default_home: temp.0.join("업무 홈"),
        ..FakeOs::default()
    };
    let mut b = bridge(&file, home);
    b.set_os(&os);
    assert_eq!(
        b.invoke("openExtensionSetup", &args(&["folder"]))
            .unwrap_err(),
        "먼저 내장 확장 준비를 누르세요."
    );
    assert_eq!(
        b.invoke("openExtensionSetup", &args(&["firefox"]))
            .unwrap_err(),
        "Edge 또는 Chrome을 선택하세요."
    );
    assert_eq!(
        b.invoke("openExtensionSetup", &args(&["edge"]))
            .unwrap_err(),
        "Edge를 찾지 못했습니다. 브라우저를 직접 열고 주소창에 edge://extensions/를 입력하세요."
    );
    let browser = temp
        .0
        .join("업무 홈")
        .join("special")
        .join(r"Google\Chrome\Application\chrome.exe");
    std::fs::create_dir_all(browser.parent().unwrap()).unwrap();
    std::fs::write(&browser, "fake").unwrap();
    assert_eq!(
        b.invoke("openExtensionSetup", &args(&["chrome"]))
            .unwrap_err(),
        "브라우저 창에 주소를 넣지 못했습니다. chrome://extensions/ 을 복사해 두었으니 주소창에 붙여넣고 Enter 를 누르세요."
    );
    assert_eq!(
        os.calls(),
        [
            format!("start {} [\"--new-window\"] true", browser.display()),
            "navigate chrome chrome://extensions/".to_string(),
            "clipboard chrome://extensions/".to_string(),
        ]
    );
    std::fs::create_dir_all(temp.0.join("업무 홈").join("extension")).unwrap();
    os.calls.lock().unwrap().clear();
    assert_eq!(
        b.invoke("openExtensionSetup", &args(&["folder"])).unwrap(),
        Value::Null
    );
    assert_eq!(
        os.calls(),
        [format!(
            "open {}",
            temp.0.join("업무 홈").join("extension").display()
        )]
    );
}

#[test]
fn 자료의_자리와_세션과_판() {
    let temp = Temp::new("location");
    let (file, home) = work(&temp);
    let os = FakeOs::default();
    let mut b = bridge(&file, home.clone());
    b.set_os(&os);
    assert_eq!(
        b.invoke("session", &[]).unwrap(),
        serde_json::json!({ "readOnly": false, "role": "work", "roleName": "작업자료", "path": path_text(&file) })
    );
    let location = b.invoke("dataLocation", &[]).unwrap();
    assert_eq!(location["folder"], path_text(&temp.0));
    assert!(location["sizeBytes"].as_u64().unwrap() > 0);
    assert_eq!(location["configPath"], path_text(&home.config_path()));
    assert_eq!(
        b.invoke("revealDataFolder", &[]).unwrap(),
        serde_json::json!({ "folder": path_text(&temp.0) })
    );
    assert_eq!(os.calls(), [format!("open {}", temp.0.display())]);
    let about = b.invoke("about", &[]).unwrap();
    assert_eq!(about["version"], env!("CARGO_PKG_VERSION"));
    assert!(about["license"].as_str().unwrap().starts_with("MIT"));
    assert!(!about["notices"].as_str().unwrap().starts_with('\u{feff}'));
}

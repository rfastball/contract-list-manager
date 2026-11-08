//! C# `SettingsTests` — DB 안에 적어 두는 설정(제출자 이름과 골라 둔 계획 엑셀). 가장 조심하는 것은 **그 값을 모르는
//! 저장이 그것을 지우는 일**이다. 값은 지어낸 것이다.

mod common;

use common::*;
use pclm_core::PclmRole;
use pclm_core::storage::pclm_file;
use pclm_core::storage::settings::SettingsStore;

fn fixture(t: &Temp) -> pclm_core::Database {
    pclm_file::create(&t.path("설정.db"), PclmRole::WORK, &TestEnv::new()).unwrap()
}

/// 제출자 이름은 제출본 파일 이름에만 쓰는 값이다(ADR-023). `None` 은 "그대로 두라" 는 뜻이다.
#[test]
fn 제출자_이름은_주지_않으면_지워지지_않는다() {
    let t = Temp::new("settings");
    let database = fixture(&t);
    let settings = SettingsStore::new(&database);
    settings.save(Some("홍길동"), None).unwrap();
    assert_eq!(settings.read().unwrap().submitter_name, "홍길동");

    settings.save(None, None).unwrap();
    assert_eq!(settings.read().unwrap().submitter_name, "홍길동");
}

/// 한 번도 고른 적이 없으면 빈 문자열이다 — `None` 이 아니다.
#[test]
fn 고른_계획_엑셀이_없으면_빈_문자열이다() {
    let t = Temp::new("settings");
    let database = fixture(&t);
    assert_eq!(SettingsStore::new(&database).read().unwrap().plan_path, "");
}

#[test]
fn 고른_계획_엑셀의_자리가_그대로_돌아온다() {
    let t = Temp::new("settings");
    let database = fixture(&t);
    let settings = SettingsStore::new(&database);
    let excel = t.path("plan").join("조달계획.xlsx");
    let excel = excel.to_str().unwrap();

    assert_eq!(settings.save_plan_path(excel).unwrap().plan_path, excel);
    assert_eq!(settings.read().unwrap().plan_path, excel);
}

/// 다른 설정을 저장할 때 골라 둔 엑셀이 조용히 지워지면 안 된다. 지우려면 빈 문자열을 준다.
#[test]
fn 계획_엑셀을_모르는_저장은_그것을_건드리지_않는다() {
    let t = Temp::new("settings");
    let database = fixture(&t);
    let settings = SettingsStore::new(&database);
    let excel = t.path("plan").join("조달계획.xlsx");
    let excel = excel.to_str().unwrap();
    settings.save_plan_path(excel).unwrap();

    settings.save(Some("홍길동"), None).unwrap();
    assert_eq!(settings.read().unwrap().plan_path, excel);

    settings.save(None, Some("")).unwrap();
    assert_eq!(settings.read().unwrap().plan_path, "");
}

/// 저장은 앞뒤 공백을 걷고, 빈 칸(NULL)은 빈 문자열로 읽는다.
#[test]
fn 저장은_공백을_걷고_null_은_빈_문자열로_읽는다() {
    let t = Temp::new("settings");
    let database = fixture(&t);
    let settings = SettingsStore::new(&database);
    assert_eq!(
        settings
            .save(Some("\u{3000} 홍길동\t"), None)
            .unwrap()
            .submitter_name,
        "홍길동"
    );
    raw(database.path())
        .execute_batch("UPDATE app_setting SET value = NULL;")
        .unwrap();
    assert_eq!(settings.read().unwrap().submitter_name, "");
}

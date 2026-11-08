//! DB 안에 적어 두는 설정(C# `Storage/Settings.cs`). 자료를 어디서 가져올지는 여기 없다 — 자료는 확장·ERP JSON·계획
//! 엑셀로 들어오고(ADR-028), 어느 작업자료를 열지는 DB 밖 홈의 `config.json` 이 적는다(`pclm-home`).
//!
//! **설정은 DB 안에 둔다.** 따로 파일로 두면 자료와 설정이 각자 옮겨 다니다 어긋난다 — 다른 작업자료를 열면 그쪽의
//! 계획 엑셀이 따라오는 것이 맞다.

use rusqlite::types::Value;

use crate::error::Result;
use crate::storage::database::{Database, begin};

const SUBMITTER_KEY: &str = "submit.name";
const PLAN_PATH_KEY: &str = "plan.path";

/// C# `AppSettings`. 비면 빈 문자열이다 — `None`(NULL)이 아니다.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AppSettings {
    /// 제출본 파일 이름에 쓸 내 이름. **쓰임은 그것뿐이다** — 자료의 임자를 정하는 값이 아니다(ADR-023).
    pub submitter_name: String,
    /// 마지막으로 고른 연간 조달계획 엑셀의 전체 경로. 지금도 그 자리에 있다는 뜻은 아니다.
    pub plan_path: String,
}

/// C# `SettingsStore` — 설정을 읽고 쓴다.
pub struct SettingsStore<'a> {
    database: &'a Database,
}

impl<'a> SettingsStore<'a> {
    pub fn new(database: &'a Database) -> Self {
        SettingsStore { database }
    }

    /// 지금 설정. 한 번도 저장한 적이 없으면 빈 값을 낸다.
    pub fn read(&self) -> Result<AppSettings> {
        let connection = self.database.open()?;
        let mut statement = connection.prepare("SELECT key, value FROM app_setting;")?;
        let rows =
            statement.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Value>(1)?)))?;
        let mut settings = AppSettings::default();
        for row in rows {
            let (key, value) = row?;
            // 빈 값은 NULL 이 아니라 빈 문자열이다 — 읽는 쪽이 두 가지 빈 것을 가리지 않게.
            let value = match value {
                Value::Text(text) => text,
                _ => String::new(),
            };
            match key.as_str() {
                SUBMITTER_KEY => settings.submitter_name = value,
                PLAN_PATH_KEY => settings.plan_path = value,
                _ => {}
            }
        }
        Ok(settings)
    }

    /// 설정을 저장한다. `None` 이면 **그대로 둔다** — 이 값을 모르는 부르는 쪽이 저장할 때마다 이름이나 골라 둔 엑셀이
    /// 조용히 지워지면 안 된다. 지우려면 빈 문자열을 준다. 앞뒤 공백은 걷는다(`string.Trim`).
    pub fn save(
        &self,
        submitter_name: Option<&str>,
        plan_path: Option<&str>,
    ) -> Result<AppSettings> {
        let connection = self.database.open()?;
        let transaction = begin(&connection)?;
        for (key, value) in [(SUBMITTER_KEY, submitter_name), (PLAN_PATH_KEY, plan_path)] {
            if let Some(value) = value {
                transaction.execute(
                    "INSERT INTO app_setting (key, value) VALUES (?1, ?2)\nON CONFLICT(key) DO UPDATE SET value = excluded.value;",
                    [key, value.trim()],
                )?;
            }
        }
        transaction.commit()?;
        self.read()
    }

    /// 고른 계획 엑셀의 자리만 적어 둔다. 고르기는 다른 설정과 상관없이 혼자 벌어지는 일이다.
    pub fn save_plan_path(&self, path: &str) -> Result<AppSettings> {
        self.save(None, Some(path))
    }
}

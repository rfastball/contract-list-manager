//! 홈(C# `Storage/Home.cs`·`HomeSwitch.cs`·`WindowPrefs.cs`) — 앱 상태가 사는 폴더 하나.
//!
//! - 쪽지(`config.json`)는 작업자료 하나를 가리키는 유일한 포인터다(ADR-030). **쪽지는 창과 명령줄만 쓴다** — 확장
//!   호스트는 [`Home::open_bound`] 로 읽기만 하고, 판을 올리지 않고, 빈 작업자료를 짓지 않는다(ADR-031·032). 쪽지가
//!   없거나 깨졌으면 기본 자리로 물러나지 않고 거절한다.
//! - 시작([`Home::resolve`])·시작 화면의 두 동작([`Home::adopt`]·[`Home::create_new`])은 `resolve`, 작업자료 바꾸기
//!   (ADR-032 — 옮기기·다른 작업자료 쓰기·스냅샷에서 새로·새 계약자료·백업)는 `switch` 에 있다.
//! - 쓰기 잠금 안에서 쪽지가 아직 그 파일을 가리키는지(ADR-032)는 [`Destination`] 으로 Core 의 저장에 넣는다.
//! - 창의 한 홈 한 창 잠금(이름 붙은 뮤텍스)은 C# 과 **같은 커널 객체 이름**을 쓴다 — C# 창과 Rust 쪽이 서로를 막는다.
//!
//! 이 층은 환경 효과를 가진다 — 진짜 시계·난수([`SystemEnv`])로 Core 를 부르고, 폴더를 짓는다(C# 의 `Database.Snapshot`·
//! `PclmFile.Create` 가 짓던 폴더는 Core 가 짓지 않으므로 여기서 짓는다).

pub mod dotnet;
mod resolve;
mod switch;
mod upper_table;
pub mod win;
pub mod window_prefs;

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use pclm_core::erp::auto_capture::AutoCapturePreferences;
use pclm_core::erp::capture::Destination;
use pclm_core::erp::wire;
use pclm_core::storage::{pclm_file, schema};
use pclm_core::{Database, PclmKind};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub use resolve::{
    BACKUPS_KEPT, DEFAULT_WORKFILE_NAME, HomeState, ProblemKind, VIEW_COPIES_KEPT, require_local,
};
pub use switch::{backup_to, check_new_target};
pub use win::NamedMutex;

/// 쪽지(C# `HomeConfig`). 칸은 C# 이 역직렬화한 그대로다 — `workfile` 이 비었을 수 있다(그러면 깨진 쪽지다).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HomeConfig {
    pub version: i32,
    pub workfile: Option<String>,
    pub dataset_id: Option<String>,
    pub retired: Option<String>,
}

/// 쪽지를 읽은 결과의 갈래(C# `ConfigState`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigState {
    /// 쪽지가 없다.
    Missing,
    /// 있는데 읽을 수 없다. 기본 자리로 물러나지 않는다(ADR-030).
    Corrupt,
    Ok,
}

/// [`Home::read_config`] 의 결과(C# `ConfigRead`). `config` 는 [`ConfigState::Ok`] 일 때만 찬다.
///
/// `error` 의 글은 C# 과 같은 자리(빈 쪽지·모르는 형식·경로 없음·전체 경로 아님)에서는 같은 글이고, JSON·입출력 오류는
/// .NET 예외 글 대신 이 크레이트의 글이다 — 호스트는 그 글을 내보내지 않는다.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigRead {
    pub state: ConfigState,
    pub config: Option<HomeConfig>,
    pub error: Option<String>,
}

impl ConfigRead {
    fn corrupt(error: impl Into<String>) -> ConfigRead {
        ConfigRead {
            state: ConfigState::Corrupt,
            config: None,
            error: Some(error.into()),
        }
    }
}

/// 호스트가 저장 대상을 열지 않기로 했다(C# `ErpConnection.OpenBound` 의 `InvalidOperationException`). 확장에는
/// `setup_required` 로 답한다 — 글은 메인 프로그램을 다시 띄우라고 말한다.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BindError(pub String);

impl fmt::Display for BindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for BindError {}

/// 홈 하나. 폴더는 온전한 경로로 바꿔 쥔다(`Path.TrimEndingDirectorySeparator(Path.GetFullPath(directory))`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Home {
    directory: PathBuf,
    /// 옛 `%APPDATA%\Pclm\config.json`. 처음 실행에 그 `dataDir` 의 자료를 옮겨 오려고 본다. 업무 홈만 갖는다.
    legacy_roaming_config: Option<PathBuf>,
    /// 전환이 쓰기 잠금을 쥔 채 쪽지까지 고친 뒤, 잠금을 놓기 직전에 부르는 자리(C# `Home.WhileLocked`). 시험만 건다.
    #[cfg(test)]
    pub(crate) while_locked: switch::Hook,
}

impl Home {
    /// 쪽지 형식의 판. 다른 판이면 깨진 것으로 본다.
    pub const CONFIG_VERSION: i32 = 1;

    /// `new Home(directory)`.
    pub fn new(directory: impl AsRef<Path>) -> io::Result<Home> {
        let full = dotnet::full_path(directory)?;
        let text = full.to_string_lossy();
        Ok(Home {
            directory: PathBuf::from(dotnet::trim_ending_separator(&text)),
            legacy_roaming_config: None,
            #[cfg(test)]
            while_locked: switch::Hook::default(),
        })
    }

    /// `new Home(directory, legacyRoamingConfig)` — 처음 실행에 옛 쪽지의 `pendingMoveFrom`·`dataDir` 를 본다. 업무 홈만
    /// 넘긴다 — 개발·시험 홈이 사람의 업무 자료를 끌어오면 격리가 아니다.
    pub fn with_legacy_roaming_config(
        directory: impl AsRef<Path>,
        legacy_roaming_config: impl Into<PathBuf>,
    ) -> io::Result<Home> {
        Ok(Home {
            legacy_roaming_config: Some(legacy_roaming_config.into()),
            ..Home::new(directory)?
        })
    }

    /// 업무 홈 `%LOCALAPPDATA%\Pclm`(C# `Home.Default`). 옛 쪽지 `%APPDATA%\Pclm\config.json` 을 함께 쥔다 — `APPDATA` 가
    /// 없으면 옛 쪽지 없이.
    pub fn default_home() -> io::Result<Home> {
        let directory = local_app_data()?.join("Pclm");
        match std::env::var_os("APPDATA").filter(|v| !v.is_empty()) {
            Some(roaming) => Home::with_legacy_roaming_config(
                directory,
                PathBuf::from(roaming).join("Pclm").join("config.json"),
            ),
            None => Home::new(directory),
        }
    }

    /// 개발 홈 `%LOCALAPPDATA%\Pclm.Erp.Dev`(C# `Home.Development`). 개발 호스트(`*-erp-dev.exe`·`--native-dev`)가 본다.
    pub fn development() -> io::Result<Home> {
        Home::new(local_app_data()?.join("Pclm.Erp.Dev"))
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn config_path(&self) -> PathBuf {
        self.directory.join("config.json")
    }

    /// 확장의 출처. 내장 확장을 준비할 때 창이 적고, 호스트가 부른 쪽을 가린다.
    pub fn extension_origin_path(&self) -> PathBuf {
        self.directory.join("extension-origin.txt")
    }

    /// 업무 홈인가.
    pub fn is_default(&self) -> bool {
        Home::default_home().is_ok_and(|d| {
            dotnet::same_path(
                &self.directory.to_string_lossy(),
                &d.directory.to_string_lossy(),
            )
        })
    }

    // ── 쪽지 ─────────────────────────────────────────────────────

    /// 쪽지를 읽는다(C# `Home.ReadConfig`). 던지지 않는다 — 깨졌으면 깨졌다고 답한다.
    ///
    /// C# 은 `JsonSerializer.Deserialize<HomeConfig>` 다(실측, .NET 8): 칸 이름은 대소문자를 가리고, 같은 이름이 둘이면 뒤의
    /// 것이 이기고, 모르는 칸은 건너뛴다. `version` 은 `int` 꼴의 수만(`1.0`·`"1"`·`null` 은 오류), 경로·신원은 글이나 `null`
    /// 만 받는다. 뿌리가 `null` 이면 빈 쪽지다.
    pub fn read_config(&self) -> ConfigRead {
        let path = self.config_path();
        if !path.is_file() {
            return ConfigRead {
                state: ConfigState::Missing,
                config: None,
                error: None,
            };
        }
        let text = match dotnet::read_all_text(&path) {
            Ok(text) => text,
            Err(e) => return ConfigRead::corrupt(e.to_string()),
        };
        let config = match deserialize_config(&text) {
            Ok(Some(config)) => config,
            Ok(None) => return ConfigRead::corrupt("비어 있습니다."),
            Err(message) => return ConfigRead::corrupt(message),
        };
        if config.version != Home::CONFIG_VERSION {
            return ConfigRead::corrupt(format!("모르는 형식(version {})입니다.", config.version));
        }
        let Some(workfile) = config.workfile.as_deref().filter(|w| !w.trim().is_empty()) else {
            return ConfigRead::corrupt("작업자료 경로(workfile)가 없습니다.");
        };
        // 상대 경로는 받지 않는다. 창과 명령줄의 작업 폴더가 달라 같은 쪽지가 서로 다른 파일을 가리킨다.
        if !dotnet::is_path_fully_qualified(workfile) {
            return ConfigRead::corrupt(format!(
                "작업자료 경로가 전체 경로가 아닙니다: {workfile}"
            ));
        }
        ConfigRead {
            state: ConfigState::Ok,
            config: Some(config),
            error: None,
        }
    }

    /// 쪽지가 지금 바로 이 파일, 이 신원을 가리키는가(C# `Home.PointsAt`). 쪽지가 없거나 깨졌으면 아니다. 확장
    /// 호스트의 저장이 쓰기 잠금을 쥔 뒤에 묻는다.
    pub fn points_at(&self, workfile: &Path, dataset_id: &str) -> bool {
        let read = self.read_config();
        let Some(config) = read.config.filter(|_| read.state == ConfigState::Ok) else {
            return false;
        };
        let Ok(full) = dotnet::full_path(workfile) else {
            return false;
        };
        config
            .workfile
            .as_deref()
            .is_some_and(|w| dotnet::same_path(w, &full.to_string_lossy()))
            && config.dataset_id.as_deref() == Some(dataset_id)
    }

    /// 쪽지를 쓴다(C# `Home.WriteConfig`). **같은 폴더의 임시 파일에 쓴 뒤 이름을 바꾼다** — 쓰다 끊기면 반쯤 쓴 쪽지가
    /// 남아 다음 실행이 그것을 깨진 쪽지로 보고 멈춘다. 글은 C# 의 `JsonSerializer`(들여쓰기, `UnicodeRanges.All`)와
    /// 바이트까지 같다 — 경로는 온전한 경로로 적고, `retired` 가 없으면 칸을 쓰지 않는다.
    pub fn write_config(
        &self,
        workfile: &Path,
        dataset_id: &str,
        retired: Option<&Path>,
    ) -> io::Result<()> {
        std::fs::create_dir_all(&self.directory)?;
        let text = config_json(workfile, dataset_id, retired)?;
        dotnet::replace_file(&self.config_path(), &text)
    }

    // ── 확장 호스트 ─────────────────────────────────────────────

    /// 쪽지가 가리키는 작업자료를 연다(C# `ErpConnection.OpenBound`). 쪽지가 멀쩡하고, 파일이 지금 판의 작업자료이고,
    /// 쪽지에 적힌 신원과 같아야만 연다. **판을 올리지 않고 아무 파일도 짓지 않는다** — 그 둘은 창의 일이다.
    pub fn open_bound(&self) -> Result<Database, BindError> {
        let read = self.read_config();
        let config = match (read.state, read.config) {
            (ConfigState::Ok, Some(config)) => config,
            _ => {
                return Err(BindError(
                    "메인 프로그램에서 저장 대상을 준비하세요.".into(),
                ));
            }
        };
        let workfile = config.workfile.as_deref().unwrap_or_default();
        let full = dotnet::full_path(workfile).map_err(|e| BindError(e.to_string()))?;
        // 판별은 PclmFile 한 곳이 낸다. 작업자료가 아니면 여기서 멈춘다.
        let database = pclm_file::open_work(&full).map_err(|e| BindError(e.to_string()))?;
        // 판이 낮으면 창이 아직 올리지 않은 것이다. 쪽지의 신원과 다르면 그 자리에 다른 작업자료가 놓였다.
        let info = pclm_file::inspect(&full);
        if info.kind != PclmKind::Ok
            || info.version != schema::VERSION
            || info.dataset_id != config.dataset_id
        {
            return Err(BindError(
                "연결된 자료가 바뀌었습니다. 메인 프로그램을 다시 실행하세요.".into(),
            ));
        }
        Ok(database)
    }

    /// 호스트를 부른 확장이 우리가 풀어 둔 그 확장인가(C# `ErpConnection.IsAllowedOrigin`).
    pub fn is_allowed_origin(&self, origin: &str) -> bool {
        let shaped = origin
            .strip_prefix("chrome-extension://")
            .and_then(|rest| rest.strip_suffix('/'))
            .is_some_and(|id| id.len() == 32 && id.bytes().all(|b| (b'a'..=b'p').contains(&b)));
        let path = self.extension_origin_path();
        shaped
            && path.is_file()
            && dotnet::read_all_text(&path).is_ok_and(|text| text.trim() == origin)
    }

    // ── 홈 잠금 ──────────────────────────────────────────────────

    /// 이 홈을 한 창만 쥐게 한다(C# `Home.TryLock`, ADR-030). 이미 누가 쥐었으면 `None`. `wait` 동안은 100ms 마다 다시
    /// 본다. 이름 붙은 뮤텍스가 **있는가**만 본다 — 손잡이가 열려 있는 동안 이름이 살아 있다.
    pub fn try_lock(&self, wait: Duration) -> io::Result<Option<NamedMutex>> {
        let until = Instant::now() + wait;
        loop {
            if let Some(mutex) = win::create_mutex(&self.lock_name())? {
                return Ok(Some(mutex));
            }
            if Instant::now() >= until {
                return Ok(None);
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    /// 잠금의 커널 객체 이름 — `Local\Pclm-<열쇠>`. C# 은 감춰 두었지만 같은 이름이어야 서로 막으므로 드러낸다.
    pub fn lock_name(&self) -> String {
        format!("Local\\Pclm-{}", self.directory_key())
    }

    /// 이 홈의 창을 앞으로 불러내는 신호의 이름(C# `Home.ActivationName`).
    pub fn activation_name(&self) -> String {
        format!("Local\\Pclm.Show.{}", self.directory_key())
    }

    /// `Convert.ToHexString(SHA256(UTF8(Directory.ToUpperInvariant())), 0, 16)` — 대문자 16진 32자.
    fn directory_key(&self) -> String {
        let upper = dotnet::to_upper_invariant(&self.directory.to_string_lossy());
        Sha256::digest(upper.as_bytes())[..16]
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect()
    }
}

impl Destination for Home {
    fn points_at(&self, database_path: &Path, dataset_id: &str) -> bool {
        Home::points_at(self, database_path, dataset_id)
    }
}

/// 자동 수집 설정 파일(C# `AutoCapturePreferences.PathFor/Read/Write`). 글의 뜻은 Core 가 쥔다.
pub mod auto_capture {
    use super::*;

    pub fn path_for(home: &Home) -> PathBuf {
        home.directory().join("auto-capture.json")
    }

    /// 읽지 못하면 기본값(꺼짐).
    pub fn read(home: &Home) -> AutoCapturePreferences {
        dotnet::read_all_text(&path_for(home))
            .map(|text| AutoCapturePreferences::from_json(&text))
            .unwrap_or_default()
    }

    /// 통째로 바꿔 쓴다. 이름이 200자를 넘으면 거절한다(`InvalidOperationException`).
    pub fn write(home: &Home, preferences: &AutoCapturePreferences) -> pclm_core::Result<()> {
        let text = preferences.to_json()?;
        std::fs::create_dir_all(home.directory())
            .and_then(|()| dotnet::replace_file(&path_for(home), &text))
            .map_err(pclm_core::Error::Io)
    }
}

/// 이 층이 Core 에 넣어 주는 시계·식별자 — 진짜 시계와 운영체제의 난수.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemEnv;

impl pclm_core::Env for SystemEnv {
    fn now_utc(&self) -> String {
        let since = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        win::format_round_trip(since.as_secs() as i64, since.subsec_nanos() / 100)
    }

    fn new_id(&self) -> String {
        win::new_guid_n()
    }
}

/// 이 기계의 시간대 — C# 이 `DateTime.Now`·`ToLocalTime()` 으로 읽던 것(명령줄의 「오늘」, 취합 보고의 월/일).
impl pclm_core::local_time::LocalZone for SystemEnv {
    fn offset_seconds(&self, utc_unix_seconds: i64) -> i64 {
        win::local_offset_seconds(utc_unix_seconds)
    }
}

fn local_app_data() -> io::Result<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "LOCALAPPDATA 가 없습니다"))
}

/// `JsonSerializer.Serialize(new HomeConfig(…), Home.Json)` 의 글.
pub fn config_json(
    workfile: &Path,
    dataset_id: &str,
    retired: Option<&Path>,
) -> io::Result<String> {
    let full = |p: &Path| -> io::Result<Value> {
        Ok(Value::String(
            dotnet::full_path(p)?.to_string_lossy().into_owned(),
        ))
    };
    let mut map = serde_json::Map::new();
    map.insert("version".into(), Value::from(Home::CONFIG_VERSION));
    map.insert("workfile".into(), full(workfile)?);
    map.insert("datasetId".into(), Value::String(dataset_id.into()));
    if let Some(retired) = retired {
        map.insert("retired".into(), full(retired)?);
    }
    Ok(wire::to_indented_unicode_json(&Value::Object(map)))
}

/// `JsonSerializer.Deserialize<HomeConfig>` 의 뜻. `Ok(None)` 은 뿌리가 `null`.
fn deserialize_config(text: &str) -> Result<Option<HomeConfig>, String> {
    let bad = || "쪽지의 JSON 을 읽지 못했습니다.".to_string();
    // 같은 이름이 둘이면 뒤의 것 — serde_json 의 객체도 뒤의 값을 남긴다. 그래서 겹친 키를 막지 않는다.
    let value = pclm_core::erp::parse_json(text).ok_or_else(bad)?;
    let map = match value {
        Value::Null => return Ok(None),
        Value::Object(map) => map,
        _ => return Err(bad()),
    };
    let text_or_null = |v: &Value| match v {
        Value::Null => Ok(None),
        Value::String(s) => Ok(Some(s.clone())),
        _ => Err(bad()),
    };
    let mut config = HomeConfig {
        version: 0,
        workfile: None,
        dataset_id: None,
        retired: None,
    };
    for (key, v) in &map {
        match key.as_str() {
            "version" => config.version = wire::try_get_int32(v).ok_or_else(bad)?,
            "workfile" => config.workfile = text_or_null(v)?,
            "datasetId" => config.dataset_id = text_or_null(v)?,
            "retired" => config.retired = text_or_null(v)?,
            _ => {}
        }
    }
    Ok(Some(config))
}

#[cfg(test)]
mod home_tests;
#[cfg(test)]
mod switch_tests;
#[cfg(test)]
mod tests;

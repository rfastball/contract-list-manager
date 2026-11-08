//! 파일을 뜨고 바꾸는 요청 — 제출본 · 취합본 · 백업 · 작업자료 바꾸기 · 자료의 자리(C# `Bridge` 의 「작업자료 바꾸기」 와
//! `Submit` · `MergeSubmissions` · `BackupWorkfile` · `DataLocationInfo` · `RevealDataFolder`).
//!
//! 바꾸는 일의 뜻(쓰기 잠금 · 쪽지 · 역할)은 모두 [`Home`](pclm_home::Home) 의 전환 절차가 쥔다(ADR-032). 여기는 자리를
//! 사람에게 묻고, 실패를 사람 말로 바꾸고, 바꾼 뒤 창을 다시 띄울 깃발을 세운다.

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use pclm_core::storage::merge_report_text;
use pclm_core::storage::merger;
use pclm_core::storage::pclm_file::{self, PclmRole};
use pclm_core::{Database, Env, PclmKind};
use pclm_home::{DEFAULT_WORKFILE_NAME, backup_to, check_new_target, dotnet, require_local};
use serde_json::Value;

use crate::{
    Bridge, FolderPrompt, OpenPrompt, SavePrompt, core, directory_name, file_name, object,
    path_text, text_or_null,
};

const WORKFILE_FILTER: &str = "계약 목록 자료 (*.pclm)|*.pclm";

impl Bridge<'_> {
    /// C# `Submit` — 제출본을 뜬다. 자리를 사람이 고른다. 이름이 비었으면 화면이 알린다(`nameMissing`).
    pub(crate) fn submit(&self) -> Result<Value, String> {
        let name = self.settings().read().map_err(core)?.submitter_name;
        let stamp = self.stamp()?;
        let suggested = if name.is_empty() {
            format!("제출_{stamp}.pclm")
        } else {
            format!("제출_{name}_{stamp}.pclm")
        };
        let prompt = SavePrompt {
            title: "제출본 만들기".into(),
            filter: "제출본 (*.pclm)|*.pclm".into(),
            default_ext: ".pclm".into(),
            file_name: suggested,
            allow_overwrite: true,
        };
        // 그만두면 화면은 아무 말도 하지 않는다.
        let Some(target) = self.pick_save_path(prompt) else {
            return Ok(Value::Null);
        };
        let path = snapshot(&self.database, &target, PclmRole::SUBMISSION, self.env)
            .map_err(|e| writing_error(e, &target))?;
        Ok(object([
            ("path", Value::String(path_text(&path))),
            ("nameMissing", Value::Bool(name.is_empty())),
        ]))
    }

    /// C# `MergeSubmissions` — 제출본을 모아 취합본 하나로 짓고, 보고를 그 옆에 글로 남긴다(BOM 을 붙인 UTF-8).
    pub(crate) fn merge_submissions(&self) -> Result<Value, String> {
        let prompt = FolderPrompt {
            title: "제출본이 든 폴더 고르기".into(),
            initial: None,
        };
        let Some(folder) = self.host.pick_folder(&prompt).filter(|f| !f.is_empty()) else {
            return Ok(Value::Null);
        };
        let files = merger::find_submissions(Path::new(&folder));
        // 조용히 빈 취합본을 만들지 않는다 — 받는 사람은 그것을 취합 결과로 안다.
        if files.is_empty() {
            return Err(format!(
                "합칠 제출본을 찾지 못했습니다 — 이 폴더에 제출본(.pclm)이 없습니다.  {folder}"
            ));
        }
        let stamp = self.stamp()?;
        let path = combine(&folder, &format!("취합_{stamp}.pclm"));
        // C# Merger 처럼 임시 폴더 아래 pclm-merge-<8자> 에서 짓고 지운다 — Core 는 폴더를 짓지 않는다.
        let work = std::env::temp_dir().join(format!("pclm-merge-{}", &self.env.new_id()[..8]));
        std::fs::create_dir_all(&work).map_err(|e| core(e.into()))?;
        let result = merger::merge(&files, Path::new(&path), &work, self.env);
        let _ = std::fs::remove_dir_all(&work);
        let report = result.map_err(core)?;

        let report_path = combine(&folder, &format!("취합_{stamp}.txt"));
        let text = merge_report_text::render(&report, Path::new(&path), self.zone);
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(text.as_bytes());
        std::fs::write(&report_path, bytes).map_err(|e| core(e.into()))?;

        Ok(object([
            ("path", Value::String(path)),
            ("reportPath", Value::String(report_path)),
            ("submissions", Value::from(report.submissions)),
            ("plans", Value::from(report.plans)),
            ("requests", Value::from(report.requests)),
            ("notices", Value::from(report.notices)),
            ("contracts", Value::from(report.contracts)),
            ("conflicts", Value::from(report.conflicts.len())),
            ("rejected", Value::from(report.rejected.len())),
        ]))
    }

    /// C# `DataLocationInfo` — 열람 중이면 사람이 고른 원본을 보인다.
    pub(crate) fn data_location(&self) -> Value {
        let path = self.shown_path();
        let size = std::fs::metadata(&path)
            .ok()
            .filter(|m| m.is_file())
            .map_or(0, |m| m.len());
        object([
            ("path", Value::String(path_text(&path))),
            ("folder", text_or_null(directory_name(&path))),
            ("sizeBytes", Value::from(size)),
            ("home", Value::String(path_text(self.home.directory()))),
            ("configPath", Value::String(path_text(&self.home.config_path()))),
        ])
    }

    /// C# `RevealDataFolder` — 그 폴더를 탐색기로 연다. 없으면 짓는다.
    pub(crate) fn reveal_data_folder(&self) -> Result<Value, String> {
        let shown = self.shown_path();
        let folder = shown.parent().map(Path::to_path_buf).unwrap_or_default();
        std::fs::create_dir_all(&folder).map_err(|e| core(e.into()))?;
        self.os.shell_open(&folder).map_err(|e| core(e.into()))?;
        Ok(object([("folder", Value::String(path_text(&folder)))]))
    }

    /// C# `PickWorkfile` — 무엇으로 바꿀지 고르게 한다. **아무것도 바꾸지 않는다.** 쓸 수 없는 자리는 여기서 거절한다.
    pub(crate) fn pick_workfile(&self, kind: &str) -> Result<Value, String> {
        let save = |title: &str, file_name: String| {
            self.pick_save_path(SavePrompt {
                title: title.into(),
                filter: WORKFILE_FILTER.into(),
                default_ext: ".pclm".into(),
                file_name,
                allow_overwrite: false,
            })
        };
        let current = Value::String(path_text(self.database.path()));
        let plan = |action: &str,
                    target: &str,
                    source: Option<String>,
                    source_role: Option<String>|
         -> Result<Value, String> {
            let path = check_new_target(&self.database, Path::new(target)).map_err(core)?;
            Ok(object([
                ("action", Value::String(action.into())),
                ("path", Value::String(path_text(&path))),
                ("source", text_or_null(source)),
                ("sourceRoleName", text_or_null(source_role)),
                ("current", current.clone()),
            ]))
        };
        match kind {
            "move" => match save("작업자료 옮기기", file_name(self.database.path())) {
                None => Ok(Value::Null),
                Some(target) => plan(
                    "move",
                    &target,
                    Some(path_text(self.database.path())),
                    None,
                ),
            },
            "new" => match save("새 계약자료", DEFAULT_WORKFILE_NAME.into()) {
                None => Ok(Value::Null),
                Some(target) => plan("new", &target, None, None),
            },
            "switch" => {
                let prompt = OpenPrompt {
                    title: "작업자료로 쓸 파일 고르기".into(),
                    filter: WORKFILE_FILTER.into(),
                };
                let Some(picked) = self.host.pick_open_file(&prompt).filter(|p| !p.is_empty())
                else {
                    return Ok(Value::Null);
                };
                let full = dotnet::full_path(&picked).map_err(|e| core(e.into()))?;
                let full_text = path_text(&full);
                if dotnet::eq_ignore_case(&full_text, &path_text(self.database.path())) {
                    return Err(format!(
                        "지금 이 창에서 쓰고 있는 작업자료입니다: {full_text}"
                    ));
                }
                let info = pclm_file::inspect(&full);
                if info.kind != PclmKind::Ok {
                    return Err(pclm_file::why(&info, &full));
                }
                // 작업자료면 그 자리를 쓴다. 제출본·취합본·백업은 그때의 기록이라 고치지 않는다 — 사본을 뜰 자리를 한 번 더 묻는다.
                if info.role.as_deref() == Some(PclmRole::WORK) {
                    require_local(&full).map_err(core)?;
                    return Ok(object([
                        ("action", Value::String("use".into())),
                        ("path", Value::String(full_text)),
                        ("source", Value::Null),
                        ("sourceRoleName", Value::Null),
                        ("current", current.clone()),
                    ]));
                }
                match save("새 계약자료로 만들 자리", DEFAULT_WORKFILE_NAME.into()) {
                    None => Ok(Value::Null),
                    Some(target) => plan(
                        "snapshot",
                        &target,
                        Some(full_text),
                        Some(PclmRole::name(info.role.as_deref())),
                    ),
                }
            }
            _ => Err(format!("모르는 바꾸기입니다: {kind}")),
        }
    }

    /// C# `Switching` — 작업자료를 바꾸고, 답을 보낸 뒤 창을 다시 띄우게 한다. 실패해도 쪽지가 이미 다른 파일을 가리키면
    /// 다시 띄운다 — 그대로 두면 사람의 편집이 다음 시작에 열리지 않을 파일에 쌓인다.
    pub(crate) fn switching(
        &self,
        work: impl FnOnce() -> Result<PathBuf, String>,
    ) -> Result<Value, String> {
        let result = work();
        if !self.home.is_active_workfile(self.database.path()) {
            self.restart.store(true, Ordering::SeqCst);
        }
        let path = result?;
        Ok(object([
            ("restart", Value::Bool(true)),
            ("path", Value::String(path_text(&path))),
        ]))
    }

    /// C# `BackupWorkfile` — 지금 시점을 백업으로 뜬다. 자리를 사람이 고른다. 쪽지는 그대로다.
    pub(crate) fn backup_workfile(&self) -> Result<Value, String> {
        let prompt = SavePrompt {
            title: "백업 만들기".into(),
            filter: "백업 (*.pclm)|*.pclm".into(),
            default_ext: ".pclm".into(),
            file_name: format!("계약자료_백업_{}.pclm", self.stamp()?),
            allow_overwrite: true,
        };
        let Some(target) = self.pick_save_path(prompt) else {
            return Ok(Value::Null);
        };
        let path = backup_to(&self.database, Path::new(&target))
            .map_err(|e| writing_error(e, &target))?;
        Ok(object([("path", Value::String(path_text(&path)))]))
    }
}

/// `PclmFile.Snapshot(database, target, role, overwrite: true)` — 덮을 수 있는 자리인지 먼저 보고, 온전한 경로의 폴더를
/// 지어 뜬다(C# `Database.Snapshot` 이 폴더를 짓는다). 뜬 온전한 경로를 낸다.
fn snapshot(
    database: &Database,
    target: &str,
    role: &str,
    env: &dyn Env,
) -> pclm_core::Result<PathBuf> {
    let full = dotnet::full_path(target)?;
    pclm_file::require_overwritable(&full)?;
    if let Some(parent) = full.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    pclm_file::snapshot(database, &full, role, true, env)
}

/// C# `Writing` — 사람이 고른 자리에 쓰지 못하면 .NET 의 영어 원문 대신 할 일을 말한다. 앱이 스스로 거절한 것(작업자료를
/// 덮지 않는다 따위)은 이미 사람 말이라 그대로 지나간다.
pub(crate) fn writing_error(error: pclm_core::Error, target: &str) -> String {
    match &error {
        pclm_core::Error::Io(e) if e.kind() == std::io::ErrorKind::PermissionDenied => format!(
            "그 자리에 쓸 권한이 없습니다 — 다른 폴더를 골라 다시 해 주세요.  {target}"
        ),
        pclm_core::Error::Io(_) => format!(
            "파일을 쓰지 못했습니다 — 다른 프로그램에서 열려 있다면 닫고 다시 하거나, 다른 자리를 골라 주세요.  {target}"
        ),
        _ => core(error),
    }
}

/// `Path.Combine(first, second)` — 둘째가 뿌리부터면 둘째, 아니면 사이에 구분자가 없을 때만 `\` 를 넣는다.
fn combine(first: &str, second: &str) -> String {
    if first.is_empty() || Path::new(second).has_root() {
        return second.to_string();
    }
    if first.ends_with(['\\', '/']) {
        format!("{first}{second}")
    } else {
        format!("{first}\\{second}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 폴더와_이름을_csharp_처럼_잇는다() {
        assert_eq!(combine(r"C:\a", "b.txt"), r"C:\a\b.txt");
        assert_eq!(combine(r"C:\a\", "b.txt"), r"C:\a\b.txt");
        assert_eq!(combine("", "b.txt"), "b.txt");
    }

    #[test]
    fn 쓰지_못한_까닭을_사람_말로_바꾼다() {
        let denied = pclm_core::Error::Io(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
        assert_eq!(
            writing_error(denied, "X"),
            "그 자리에 쓸 권한이 없습니다 — 다른 폴더를 골라 다시 해 주세요.  X"
        );
        let busy = pclm_core::Error::Io(std::io::Error::other("busy"));
        assert!(writing_error(busy, "X").starts_with("파일을 쓰지 못했습니다"));
        let refused = pclm_core::Error::InvalidOperation("덮지 않습니다".into());
        assert_eq!(writing_error(refused, "X"), "덮지 않습니다");
    }
}

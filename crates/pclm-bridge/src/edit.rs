//! 고치는 요청 — 사람이 채우는 열 · 가공열 · 덮개 · 지우기 · 여러 건 · 맞춤 뷰와 보기 구성(C# `Bridge` 의 「열 고치기」 ·
//! 「수집한 값 고치기」 · 「지우기」 와 보기 구성 갈래). 뜻은 모두 [`Store`] 가 쥔다 — 여기는 인자를 풀고 답의 꼴을 짓는다.

use std::path::Path;

use pclm_core::storage::flow::Flow;
use pclm_core::storage::store::{self, CellChange, EntityRef, UserViewInfo, user_column_json};
use pclm_core::storage::views::UserColumnDefinition;
use serde_json::{Map, Value};

use crate::dotnet_json::{self, Step};

use crate::{
    Bridge, NOTHING_TO_DO, OpenPrompt, SavePrompt, core, file_name, malformed, object, parse_int,
    text_or_null,
};

/// 보기 구성 파일의 거르개.
const VIEW_PACKAGE_FILTER: &str = "보기 구성 (*.json)|*.json";

impl Bridge<'_> {
    /// C# `UserColumns` — 열 관리가 읽는 목록. 식을 읽지 못하는 가공열은 깨진 열로 알린다.
    pub(crate) fn user_columns(&self, entity_type: Option<&str>) -> Result<Value, String> {
        let store = self.store();
        let broken = store.broken_columns().map_err(core)?;
        // 그 열을 쓰는 맞춤 뷰 수 — 지우는 확인 문구가 「맞춤 뷰 n곳이 이 열을 씁니다」 로 센다.
        let mut readers = Vec::new();
        for entity in ["notice", "contract", "request"] {
            readers.push((entity, store.user_view_readers(entity).map_err(core)?));
        }
        let columns = store.user_columns(entity_type).map_err(core)?;
        let mut out = Vec::new();
        for c in &columns {
            let mut formula = Value::Null;
            let mut error = broken
                .get(&(c.entity_type.clone(), c.field_name.clone()))
                .cloned();
            if c.kind != "choice"
                && let Some(options) = &c.options
                && options.trim_start().starts_with('{')
            {
                match read_for_editing(options) {
                    Ok(value) => formula = value,
                    Err(pclm_core::Error::InvalidOperation(message)) => {
                        if c.kind == "computed" && error.is_none() {
                            error = Some(message);
                        }
                    }
                    Err(other) => return Err(core(other)),
                }
            }
            // C# `readers[c.EntityType]` — 셋 밖의 종류면 KeyNotFoundException 이다.
            let view_readers = readers
                .iter()
                .find(|(e, _)| *e == c.entity_type)
                .ok_or_else(|| {
                    format!(
                        "The given key '{}' was not present in the dictionary.",
                        c.entity_type
                    )
                })?
                .1
                .get(&c.field_name)
                .copied()
                .unwrap_or(0);
            out.push(column_json(c, formula, error, view_readers));
        }
        Ok(Value::Array(out))
    }

    pub(crate) fn save_computed_column(
        &self,
        entity_type: &str,
        name: &str,
        formula: &str,
        previous_name: Option<&str>,
    ) -> Result<Value, String> {
        self.store()
            .save_computed_column(entity_type, name, formula, previous_name)
            .map_err(core)?;
        self.user_columns(None)
    }

    /// 값으로 굳히기. 화면이 센 수를 함께 보내면 그 사이 자료가 바뀐 경우 쓰지 않는다.
    pub(crate) fn freeze_column(
        &self,
        entity_type: &str,
        name: &str,
        expected: Option<&str>,
    ) -> Result<Value, String> {
        self.store()
            .freeze_column(entity_type, name, expected.and_then(try_parse_long))
            .map_err(core)?;
        self.user_columns(None)
    }

    pub(crate) fn add_column(
        &self,
        entity_type: &str,
        field_name: &str,
        kind: &str,
        choices: Option<&str>,
    ) -> Result<Value, String> {
        self.store()
            .add_user_column(entity_type, field_name, kind, &split(choices))
            .map_err(core)?;
        self.user_columns(None)
    }

    pub(crate) fn update_column(
        &self,
        entity_type: &str,
        field_name: &str,
        new_name: &str,
        kind: &str,
        choices: Option<&str>,
    ) -> Result<Value, String> {
        self.store()
            .update_user_column(entity_type, field_name, new_name, kind, &split(choices))
            .map_err(core)?;
        self.user_columns(None)
    }

    pub(crate) fn remove_column(&self, entity_type: &str, field_name: &str) -> Result<Value, String> {
        self.store()
            .remove_user_column(entity_type, field_name)
            .map_err(core)?;
        self.user_columns(None)
    }

    pub(crate) fn move_column(
        &self,
        entity_type: &str,
        field_name: &str,
        delta: &str,
    ) -> Result<Value, String> {
        let delta = parse_int(delta)?;
        self.store()
            .move_user_column(entity_type, field_name, delta)
            .map_err(core)?;
        self.user_columns(None)
    }

    /// C# `SetField` — 손으로 채우는 열만 받는다.
    pub(crate) fn set_field(
        &self,
        key: &str,
        field: &str,
        value: Option<&str>,
    ) -> Result<Value, String> {
        let store = self.store();
        let entity = self.target(key)?;
        let known = store
            .user_columns(Some(&entity.entity_type))
            .map_err(core)?;
        if !known.iter().any(|c| c.field_name == field) {
            return Err(format!("손으로 채우는 열이 아닙니다: {field}"));
        }
        store.set_user_field(&entity, field, value).map_err(core)?;
        Ok(Value::Null)
    }

    /// 계약면의 칸 하나에 사람이 적은 값을 씌운다. 값이 오지 않으면 빈 글이다.
    pub(crate) fn set_override(
        &self,
        key: &str,
        column: &str,
        value: Option<&str>,
    ) -> Result<Value, String> {
        let entity = self.target(key)?;
        self.store()
            .set_override(&entity, column, value.unwrap_or(""))
            .map_err(core)?;
        Ok(Value::Null)
    }

    pub(crate) fn clear_override(&self, key: &str, column: &str) -> Result<Value, String> {
        let entity = self.target(key)?;
        self.store().clear_override(&entity, column).map_err(core)?;
        Ok(Value::Null)
    }

    /// 지우면 무엇이 사라지는지. **묻기 전에 세어 보인다.**
    pub(crate) fn deletion_plan(&self, key: &str, scope: &str) -> Result<Value, String> {
        let entity = self.target(key)?;
        let whole = whole(scope)?;
        self.store()
            .plan_deletion(&entity, whole)
            .map(|p| p.to_json())
            .map_err(core)
    }

    pub(crate) fn delete_entity(&self, key: &str, scope: &str) -> Result<Value, String> {
        let entity = self.target(key)?;
        let whole = whole(scope)?;
        self.store().delete(&entity, whole).map_err(core)?;
        Ok(Value::Null)
    }

    /// C# `Target` — 열쇠를 풀지 못하면 사람 말로.
    fn target(&self, key: &str) -> Result<EntityRef, String> {
        self.store()
            .resolve(key)
            .map_err(core)?
            .ok_or_else(|| format!("그런 공고·계약이 없습니다: {key}"))
    }

    // ── 맞춤 뷰 · 보기 구성 ────────────────────────────────

    pub(crate) fn user_views(&self) -> Result<Value, String> {
        Ok(views_json(&self.store().user_views().map_err(core)?))
    }

    pub(crate) fn save_user_view(&self, json: &str, previous: Option<&str>) -> Result<Value, String> {
        Ok(views_json(
            &self.store().save_user_view(json, previous).map_err(core)?,
        ))
    }

    pub(crate) fn delete_user_view(&self, id: &str) -> Result<Value, String> {
        Ok(views_json(&self.store().delete_user_view(id).map_err(core)?))
    }

    pub(crate) fn import_view_package(&self, json: &str, token: &str) -> Result<Value, String> {
        Ok(views_json(
            &self
                .store()
                .import_view_package(json, token)
                .map_err(core)?,
        ))
    }

    /// C# `ExportViewPackage` — 구성을 먼저 짓고 자리를 묻는다. 고른 자리 그대로를 낸다(온전한 경로로 바꾸지 않는다).
    pub(crate) fn export_view_package(&self, ids: &str) -> Result<Value, String> {
        let ids = read_strings(ids)?;
        let json = self.store().export_view_package(&ids).map_err(core)?;
        let prompt = SavePrompt {
            title: "보기 구성 내보내기".into(),
            filter: VIEW_PACKAGE_FILTER.into(),
            default_ext: ".json".into(),
            file_name: format!("보기구성_{}.json", self.stamp()?),
            allow_overwrite: true,
        };
        let Some(target) = self.pick_save_path(prompt) else {
            return Ok(Value::Null);
        };
        std::fs::write(&target, json.as_bytes())
            .map_err(|e| crate::files::writing_error(pclm_core::Error::Io(e), &target))?;
        Ok(object([("path", Value::String(target))]))
    }

    /// C# `PickViewPackage` — 고른 파일을 읽어 미리보기와 함께 낸다. 2MB 를 넘으면 읽지 않는다.
    pub(crate) fn pick_view_package(&self) -> Result<Value, String> {
        let prompt = OpenPrompt {
            title: "보기 구성 가져오기".into(),
            filter: VIEW_PACKAGE_FILTER.into(),
        };
        let Some(path) = self.host.pick_open_json(&prompt).filter(|p| !p.is_empty()) else {
            return Ok(Value::Null);
        };
        let length = std::fs::metadata(&path)
            .ok()
            .filter(|m| m.is_file())
            .map(|m| m.len())
            .ok_or_else(|| {
                let full = pclm_home::dotnet::full_path(&path)
                    .map(|p| crate::path_text(&p))
                    .unwrap_or_else(|_| path.clone());
                format!("Could not find file '{full}'.")
            })?;
        if length > 2_000_000 {
            return Err("보기 구성 파일은 2MB까지 가져올 수 있습니다.".into());
        }
        let json = pclm_home::dotnet::read_all_text(Path::new(&path))
            .map_err(|e| core(pclm_core::Error::Io(e)))?;
        let preview = self.store().preview_view_package(&json).map_err(core)?;
        Ok(object([
            ("name", Value::String(file_name(Path::new(&path)))),
            ("json", Value::String(json)),
            ("preview", preview.to_json()),
        ]))
    }

    /// 서식용 예시 보기를 고른 파일처럼 내민다.
    pub(crate) fn builtin_view_package(&self) -> Result<Value, String> {
        let json = store::builtin_view_package();
        let preview = self.store().preview_view_package(json).map_err(core)?;
        Ok(object([
            ("name", Value::String("예시 보기".into())),
            ("json", Value::String(json.into())),
            ("preview", preview.to_json()),
        ]))
    }
}

/// 열 관리의 열 하나 — `{entityType, fieldName, kind, choices, formula, broken, viewReaders}`.
fn column_json(
    c: &UserColumnDefinition,
    formula: Value,
    error: Option<String>,
    view_readers: i32,
) -> Value {
    let plain = user_column_json(c);
    object([
        ("entityType", Value::String(c.entity_type.clone())),
        ("fieldName", Value::String(c.field_name.clone())),
        ("kind", Value::String(c.kind.clone())),
        ("choices", plain.get("choices").cloned().unwrap_or(Value::Null)),
        ("formula", formula),
        (
            "broken",
            if c.kind == "computed" {
                text_or_null(error)
            } else {
                Value::Null
            },
        ),
        ("viewReaders", Value::from(view_readers)),
    ])
}

fn views_json(views: &[UserViewInfo]) -> Value {
    Value::Array(views.iter().map(UserViewInfo::to_json).collect())
}

/// C# `ComputedColumn.ReadForEditing` — 편집 화면은 격리된 식도 원문과 진단을 함께 읽는다(`migrationError` 가 있으면
/// 원문, 없으면 `Flow.Json` 꼴의 v2).
fn read_for_editing(json: &str) -> pclm_core::Result<Value> {
    let formula = Flow::read_for_identity(json)?;
    let root: Value = serde_json::from_str(json)
        .map_err(|e| pclm_core::Error::Argument(format!("가공식 형식이 올바르지 않습니다: {e}")))?;
    if root.get("migrationError").is_some() {
        Ok(root)
    } else {
        let text = Flow::serialize(&formula)?;
        serde_json::from_str(&text)
            .map_err(|e| pclm_core::Error::Argument(format!("가공식을 다시 읽지 못했습니다: {e}")))
    }
}

/// C# `Whole` — 계열 전체인가 이 차수 하나인가. 모르는 말이 오면 **넓은 쪽으로 넘기지 않는다**.
pub(crate) fn whole(scope: &str) -> Result<bool, String> {
    match scope {
        "series" => Ok(true),
        "seq" => Ok(false),
        _ => Err(format!("모르는 삭제 범위입니다: {scope}")),
    }
}

/// C# `Split` — 화면은 후보를 세로줄로 이어 보낸다. 빈 것은 버리고 앞뒤 공백을 걷는다.
fn split(choices: Option<&str>) -> Vec<String> {
    choices.map_or_else(Vec::new, |c| {
        c.split('|')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    })
}

/// `long.TryParse(text, out var n)` — `NumberStyles.Integer`.
fn try_parse_long(text: &str) -> Option<i64> {
    let trimmed =
        text.trim_matches(|c: char| matches!(c, ' ' | '\t' | '\n' | '\u{b}' | '\u{c}' | '\r'));
    let (negative, digits) = match trimmed.as_bytes().first() {
        Some(b'-') => (true, &trimmed[1..]),
        Some(b'+') => (false, &trimmed[1..]),
        _ => (false, trimmed),
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let significant = digits.trim_start_matches('0');
    if significant.len() > 19 {
        return None;
    }
    let magnitude: i128 = significant.parse().unwrap_or(0);
    i64::try_from(if negative { -magnitude } else { magnitude }).ok()
}

/// C# `ReadBatch<string[]>` — JSON 의 null 이면 「작업할 자료가 없습니다.」. 꼴이 다른 값은 .NET 의 글로 거절한다
/// ([`dotnet_json`]). null 원소는 .NET 이 받지만(그 뒤 Core 가 무엇을 하는지는 견주지 않았다) 여기서는 거절한다.
pub(crate) fn read_strings(json: &str) -> Result<Vec<String>, String> {
    let value: Value = serde_json::from_str(json).map_err(|e| malformed(e.to_string()))?;
    match value {
        Value::Null => Err(NOTHING_TO_DO.into()),
        Value::Array(items) => items
            .into_iter()
            .enumerate()
            .map(|(i, x)| match x {
                Value::String(s) => Ok(s),
                _ => Err(dotnet_json::not_converted(json, &[Step::Index(i)], "System.String")),
            })
            .collect(),
        _ => Err(dotnet_json::not_converted(json, &[], "System.String[]")),
    }
}

/// C# `ReadBatch<CellChange[]>` — camelCase 이름(대소문자를 가린다)의 글 칸 다섯. 모르는 칸은 버리고, null 원소 · 빠진 칸은
/// 비워 둬 Core 가 거절하게 한다. 꼴이 다른 값은 .NET 의 글로 거절한다([`dotnet_json`]) — 생성자로 짓는 기록이라 칸의 꼴이
/// 틀려도 기록의 이름(`CellChange`)이 든다.
pub(crate) fn read_changes(json: &str) -> Result<Vec<CellChange>, String> {
    const TYPE: &str = "Pclm.Core.Storage.CellChange";
    let value: Value = serde_json::from_str(json).map_err(|e| malformed(e.to_string()))?;
    let items = match value {
        Value::Null => return Err(NOTHING_TO_DO.into()),
        Value::Array(items) => items,
        _ => return Err(dotnet_json::not_converted(json, &[], &format!("{TYPE}[]"))),
    };
    items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let map = match item {
                Value::Object(map) => map,
                Value::Null => return Ok(CellChange::default()),
                _ => return Err(dotnet_json::not_converted(json, &[Step::Index(i)], TYPE)),
            };
            let text = |map: &Map<String, Value>, key: &str| match map.get(key) {
                None | Some(Value::Null) => Ok(None),
                Some(Value::String(s)) => Ok(Some(s.clone())),
                Some(_) => Err(dotnet_json::not_converted(
                    json,
                    &[Step::Index(i), Step::Key(key.into())],
                    TYPE,
                )),
            };
            Ok(CellChange {
                row_key: text(map, "rowKey")?,
                field: text(map, "field")?,
                value: text(map, "value")?,
                original: text(map, "original")?,
                operation: text(map, "operation")?,
            })
        })
        .collect()
}

/// C# `SampleKeys` — 보표의 표본 열쇠(JSON 글 배열). 비었으면 `None`(최신 문서 셋).
pub(crate) fn sample_keys(json: Option<&str>) -> Result<Option<Vec<String>>, String> {
    let Some(json) = json.filter(|j| !j.is_empty()) else {
        return Ok(None);
    };
    string_array(json).map_err(|_| "표본 열쇠 형식이 올바르지 않습니다.".to_string())
}

/// `JsonSerializer.Deserialize<string[]>(json)` — JSON 의 null 이면 `None`. 글이 아닌 원소는 거절한다.
pub(crate) fn string_array(json: &str) -> Result<Option<Vec<String>>, ()> {
    match serde_json::from_str::<Value>(json).map_err(|_| ())? {
        Value::Null => Ok(None),
        Value::Array(items) => items
            .into_iter()
            .map(|x| match x {
                Value::String(s) => Ok(s),
                _ => Err(()),
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Some),
        _ => Err(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 후보는_세로줄로_가르고_빈_것을_버린다() {
        assert_eq!(split(Some(" 가 | |나|")), vec!["가", "나"]);
        assert!(split(None).is_empty());
    }

    #[test]
    fn 센_수는_long_으로_읽고_못_읽으면_없다() {
        assert_eq!(try_parse_long(" 12 "), Some(12));
        assert_eq!(try_parse_long("-0"), Some(0));
        assert_eq!(try_parse_long("9223372036854775807"), Some(i64::MAX));
        assert_eq!(try_parse_long("-9223372036854775808"), Some(i64::MIN));
        assert_eq!(try_parse_long("9223372036854775808"), None);
        assert_eq!(try_parse_long("1.0"), None);
        assert_eq!(try_parse_long(""), None);
    }

    #[test]
    fn 범위는_둘만_받는다() {
        assert_eq!(whole("series"), Ok(true));
        assert_eq!(whole("seq"), Ok(false));
        assert_eq!(whole("all").unwrap_err(), "모르는 삭제 범위입니다: all");
    }

    #[test]
    fn 여러_건의_값은_null_이면_거절한다() {
        assert_eq!(read_strings("null").unwrap_err(), NOTHING_TO_DO);
        assert_eq!(read_strings(r#"["a","b"]"#).unwrap(), vec!["a", "b"]);
        assert_eq!(read_changes("null").unwrap_err(), NOTHING_TO_DO);
        let changes = read_changes(r#"[{"rowKey":"k","field":"f","Value":"무시","extra":1}]"#).unwrap();
        assert_eq!(changes[0].row_key.as_deref(), Some("k"));
        assert_eq!(changes[0].value, None);
        assert_eq!(
            sample_keys(Some("{")).unwrap_err(),
            "표본 열쇠 형식이 올바르지 않습니다."
        );
        assert_eq!(sample_keys(Some("")).unwrap(), None);
        assert_eq!(sample_keys(Some("null")).unwrap(), None);
    }
}

//! 편집 대본(#9 E4) — `run <file> <script.json>`. C# `tests/Pclm.Parity/Script.cs` 를 옮긴 것이고, 꼴은
//! `crates/pclm-parity/README.md` 가 정본이다. 대본의 연산을 차례로 `pclm-core` 의 [`Store`] 에 시키고, 연산마다
//! 결과나 오류 글을 `<file>.out.json` 에 한 줄씩 적는다 — C# 쪽과 같은 대본에서 같은 바이트를 내야 한다.
//!
//! 결과는 다리(`Bridge.Json`)의 꼴([`store::bridge_json`])로 적는다. 실패한 연산(C# 의
//! `InvalidOperationException`·`ArgumentException`·`SqliteException` 에 해당하는 [`Error`])은 메시지를 적고 다음으로
//! 간다. 모르는 연산·대본 꼴의 잘못은 대본의 버그라 적지 않고 멎는다. 인자의 형이 틀린 것(글 배열의 수, 불리언 자리의
//! 글 따위)은 C# 에서 `JsonElement` 가 `InvalidOperationException` 을 던져 오류 줄이 되므로 같은 글로 적는다([`wrong_type`]).

use pclm_core::storage::store::{self, CellChange, EntityRef, Store};
use pclm_core::{Access, Database, Error};
use serde_json::{Map, Value};

use crate::system_env::SystemEnv;

/// 대본 꼴의 잘못 — 적지 않고 멎는다(C# 의 `InvalidDataException`).
type Fatal = String;

/// C# `Script.PinClockSql` — 사람 값·덮개의 시각을 한 값으로 못박는다.
pub(crate) const PIN_CLOCK_SQL: &str =
    "UPDATE contract_user_field SET updated_at = '2000-01-01T00:00:00.0000000Z';
UPDATE notice_user_field SET updated_at = '2000-01-01T00:00:00.0000000Z';
UPDATE request_user_field SET updated_at = '2000-01-01T00:00:00.0000000Z';
UPDATE field_override SET updated_at = '2000-01-01T00:00:00.0000000Z';";

#[derive(Default)]
struct State {
    token: Option<String>,
    fingerprint: Option<String>,
    cells: Option<i64>,
}

/// 연산 하나의 끝 — 결과, 적을 오류, 멎을 잘못.
enum Outcome {
    Error(Error),
    Fatal(Fatal),
}

impl From<Error> for Outcome {
    fn from(error: Error) -> Self {
        Outcome::Error(error)
    }
}

impl From<Fatal> for Outcome {
    fn from(message: Fatal) -> Self {
        Outcome::Fatal(message)
    }
}

type Step<T> = std::result::Result<T, Outcome>;

/// C# `Script.Run`.
pub fn run(file: &str, script_path: &str) -> Result<(), String> {
    let env = SystemEnv;
    let database = Database::new(file, Access::Write);
    let store = Store::new(database, &env);
    let text = std::fs::read_to_string(script_path).map_err(|e| e.to_string())?;
    let script: Value =
        serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| e.to_string())?;
    // serde_json 은 겹친 키를 읽으며 뒤 값 하나로 접는다 — C# 계기는 날것 값 인자를 원문 그대로(겹친 키째) 넘기므로 같은
    // 대본이 두 Core 에 다른 입력이 된다. 조용히 접지 않고 멎는다: 겹친 키 입력은 글(string) 인자로 적는다.
    if has_duplicate_keys(&text) {
        return Err(
            "대본에 겹친 키가 있습니다 — 겹친 키 입력은 글(string) 인자로 적습니다(Rust 는 날것 값의 겹친 키를 접습니다)."
                .into(),
        );
    }
    let ops = script.as_array().ok_or("대본은 연산의 배열입니다.")?;
    let mut state = State::default();
    let mut lines = Vec::new();
    for op in ops {
        let op = op.as_object().ok_or("연산은 객체입니다.")?;
        let name = op
            .get("op")
            .and_then(Value::as_str)
            .ok_or("op 가 없습니다.")?;
        let head = format!(
            "{{\"op\":{}",
            store::bridge_json(&Value::String(name.into()))
        );
        match run_op(&store, name, op, &mut state) {
            Ok(result) => lines.push(format!(
                "{head},\"result\":{}}}",
                store::bridge_json(&result)
            )),
            Err(Outcome::Error(error)) => lines.push(format!(
                "{head},\"error\":{}}}",
                store::bridge_json(&Value::String(store::error_message(&error)))
            )),
            Err(Outcome::Fatal(message)) => return Err(message),
        }
    }
    let out = if lines.is_empty() {
        "[\n]\n".to_string()
    } else {
        format!("[\n{}\n]\n", lines.join(",\n"))
    };
    std::fs::write(format!("{file}.out.json"), out).map_err(|e| e.to_string())
}

struct Args<'a, 's> {
    store: &'a Store<'s>,
    name: &'a str,
    op: &'a Map<String, Value>,
}

impl Args<'_, '_> {
    /// C# `Script.Text` — 없거나 null 이면 `None`, 글이면 자리표시를 푼 것, 그 밖의 JSON 값은 기본 인코더의 압축 글.
    fn text(&self, key: &str, state: &State) -> Step<Option<String>> {
        match self.op.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(s)) => placeholder(self.store, s, state).map(Some),
            Some(other) => Ok(Some(store::bridge_json(other))),
        }
    }

    fn need(&self, key: &str, state: &State) -> Step<String> {
        self.text(key, state)?
            .ok_or_else(|| Outcome::Fatal(format!("{}: {key} 가 없습니다.", self.name)))
    }

    fn strings(&self, key: &str, state: &State) -> Step<Vec<String>> {
        match self.op.get(key) {
            Some(Value::Array(items)) => items
                .iter()
                .map(|item| match item {
                    Value::String(s) => placeholder(self.store, s, state),
                    // C# 은 null 을 자리표시로 풀다 NullReferenceException 으로 멎는다.
                    Value::Null => Err(Outcome::Fatal(format!(
                        "{}: {key} 는 글의 배열입니다.",
                        self.name
                    ))),
                    other => Err(wrong_type("String", other)),
                })
                .collect(),
            _ => Ok(Vec::new()),
        }
    }

    fn strings_or_none(&self, key: &str, state: &State) -> Step<Option<Vec<String>>> {
        match self.op.get(key) {
            Some(Value::Array(_)) => self.strings(key, state).map(Some),
            _ => Ok(None),
        }
    }

    /// C# `GetBoolean` — 없으면 거짓, 불리언이 아니면(null 도) 오류 줄.
    fn whole(&self) -> Step<bool> {
        match self.op.get("wholeSeries") {
            None => Ok(false),
            Some(Value::Bool(b)) => Ok(*b),
            Some(other) => Err(wrong_type("Boolean", other)),
        }
    }

    fn entity(&self, state: &State) -> Step<EntityRef> {
        let key = self.need("key", state)?;
        resolve(self.store, &key)
    }
}

fn resolve(store: &Store<'_>, key: &str) -> Step<EntityRef> {
    store.resolve(key)?.ok_or_else(|| {
        Outcome::Error(Error::InvalidOperation(format!(
            "그런 공고·계약이 없습니다: {key}"
        )))
    })
}

fn list<T>(items: &[T], json: impl Fn(&T) -> Value) -> Value {
    Value::Array(items.iter().map(json).collect())
}

fn run_op(
    store: &Store<'_>,
    name: &str,
    op: &Map<String, Value>,
    state: &mut State,
) -> Step<Value> {
    let a = Args { store, name, op };
    let s = &*state;
    let value = match name {
        "UserColumns" => list(
            &store.user_columns(a.text("entityType", s)?.as_deref())?,
            store::user_column_json,
        ),
        "AddUserColumn" => {
            store.add_user_column(
                &a.need("entityType", s)?,
                &a.need("fieldName", s)?,
                &a.need("kind", s)?,
                &a.strings("choices", s)?,
            )?;
            Value::Null
        }
        "SaveComputedColumn" => {
            store.save_computed_column(
                &a.need("entityType", s)?,
                &a.need("fieldName", s)?,
                &a.need("json", s)?,
                a.text("previousName", s)?.as_deref(),
            )?;
            Value::Null
        }
        "PreviewComputedColumn" => {
            store.preview_computed_column(&a.need("entityType", s)?, &a.need("json", s)?)?
        }
        "ColumnContext" => store.column_context(&a.need("entityType", s)?)?,
        "UpdateUserColumn" => {
            store.update_user_column(
                &a.need("entityType", s)?,
                &a.need("fieldName", s)?,
                &a.need("newName", s)?,
                &a.need("kind", s)?,
                &a.strings("choices", s)?,
            )?;
            Value::Null
        }
        "RemoveUserColumn" => {
            store.remove_user_column(&a.need("entityType", s)?, &a.need("fieldName", s)?)?;
            Value::Null
        }
        "MoveUserColumn" => {
            // C# 과 같은 차례 — 이름 둘을 먼저 푼다. 수가 아니면 GetInt32 의 오류 줄, 없거나 int 밖이면 멎는다.
            let entity_type = a.need("entityType", s)?;
            let field_name = a.need("fieldName", s)?;
            let delta = match op.get("delta") {
                None => return Err(Outcome::Fatal(format!("{name}: delta 가 없습니다."))),
                Some(Value::Number(n)) => n
                    .as_i64()
                    .and_then(|d| i32::try_from(d).ok())
                    .ok_or_else(|| Outcome::Fatal(format!("{name}: delta 는 int 정수입니다.")))?,
                Some(other) => return Err(wrong_type("Number", other)),
            };
            store.move_user_column(&entity_type, &field_name, delta)?;
            Value::Null
        }
        "SetUserField" => {
            store.set_user_field(
                &a.entity(s)?,
                &a.need("fieldName", s)?,
                a.text("value", s)?.as_deref(),
            )?;
            Value::Null
        }
        "SetOverride" => {
            store.set_override(
                &a.entity(s)?,
                &a.need("columnName", s)?,
                &a.need("value", s)?,
            )?;
            Value::Null
        }
        "ClearOverride" => {
            store.clear_override(&a.entity(s)?, &a.need("columnName", s)?)?;
            Value::Null
        }
        "CheckCorrectable" => {
            store.check_correctable(&a.entity(s)?, &a.need("columnName", s)?)?;
            Value::Null
        }
        "FaceValue" => Value::String(store.face_value(&a.entity(s)?, &a.need("columnName", s)?)?),
        "Overrides" => list(
            &store.overrides(&a.need("entityType", s)?)?,
            store::FieldOverride::to_json,
        ),
        "PlanDeletion" => store.plan_deletion(&a.entity(s)?, a.whole()?)?.to_json(),
        "Delete" => {
            store.delete(&a.entity(s)?, a.whole()?)?;
            Value::Null
        }
        "Resolve" => store
            .resolve(&a.need("key", s)?)?
            .map_or(Value::Null, |entity| entity.to_json()),
        // 열쇠가 튜플이라 다리 꼴이 없다 — [종류, 이름, 까닭] 의 목록으로 적는다(C# 쪽과 같다).
        "BrokenColumns" => Value::Array(
            store
                .broken_columns()?
                .iter()
                .map(|((entity, name), why)| {
                    Value::Array(vec![
                        Value::String(entity.clone()),
                        Value::String(name.clone()),
                        Value::String(why.clone()),
                    ])
                })
                .collect(),
        ),
        "TraceComputedColumn" => {
            store.trace_computed_column(&a.need("entityType", s)?, &a.need("json", s)?)?
        }
        "CompileComputedColumn" => Value::String(
            store.compile_computed_column(&a.need("entityType", s)?, &a.need("json", s)?)?,
        ),
        "ValidateComputedSql" => {
            store.validate_computed_sql(&a.need("entityType", s)?, &a.need("sql", s)?)?
        }
        "PlanFreezeColumn" => {
            let plan =
                store.plan_freeze_column(&a.need("entityType", s)?, &a.need("fieldName", s)?)?;
            state.cells = Some(plan.cells);
            plan.to_json()
        }
        "FreezeColumn" => {
            let expected = match op.get("expected") {
                Some(Value::Number(n)) => {
                    Some(n.as_i64().ok_or_else(|| {
                        Outcome::Fatal(format!("{name}: expected 는 정수입니다."))
                    })?)
                }
                Some(Value::String(t)) if t == "$cells" => Some(state.cells.ok_or_else(|| {
                    Outcome::Fatal("$cells 앞에 PlanFreezeColumn 이 없습니다.".to_string())
                })?),
                _ => None,
            };
            Value::from(store.freeze_column(
                &a.need("entityType", s)?,
                &a.need("fieldName", s)?,
                expected,
            )?)
        }
        "UserViews" => list(&store.user_views()?, store::UserViewInfo::to_json),
        "UserViewContext" => store.user_view_context()?,
        "TraceUserViewColumn" => {
            let keys = a.strings_or_none("sampleKeys", s)?;
            store.trace_user_view_column(&a.need("json", s)?, keys.as_deref())?
        }
        "UserViewSamples" => Value::Array(store.user_view_samples(
            &a.need("viewJson", s)?,
            a.text("query", s)?.as_deref(),
            a.text("edge", s)?.as_deref(),
        )?),
        "PreviewUserView" => {
            store.preview_user_view(&a.need("viewJson", s)?, a.text("sampleKey", s)?.as_deref())?
        }
        "SaveUserView" => list(
            &store.save_user_view(&a.need("viewJson", s)?, a.text("previousId", s)?.as_deref())?,
            store::UserViewInfo::to_json,
        ),
        "DeleteUserView" => list(
            &store.delete_user_view(&a.need("id", s)?)?,
            store::UserViewInfo::to_json,
        ),
        "UserViewReaders" => {
            let readers = store.user_view_readers(&a.need("entityType", s)?)?;
            Value::Object(
                readers
                    .iter()
                    .map(|(k, v)| (k.clone(), Value::from(*v)))
                    .collect(),
            )
        }
        "ExportViewPackage" => Value::String(store.export_view_package(&a.strings("ids", s)?)?),
        "BuiltinViewPackage" => Value::String(store::builtin_view_package().to_string()),
        "PreviewViewPackage" => {
            let preview = store.preview_view_package(&a.need("json", s)?)?;
            state.token = Some(preview.token.clone());
            preview.to_json()
        }
        "ImportViewPackage" => list(
            &store.import_view_package(&a.need("json", s)?, &a.need("token", s)?)?,
            store::UserViewInfo::to_json,
        ),
        "BatchEdit" => {
            let cells = match op.get("changes") {
                None => return Err(Outcome::Fatal(format!("{name}: changes 가 없습니다."))),
                Some(Value::Array(cells)) => cells,
                Some(other) => return Err(wrong_type("Array", other)),
            };
            let mut changes = Vec::new();
            for cell in cells {
                if !cell.is_object() {
                    return Err(wrong_type("Object", cell));
                }
                let cell_text = |key: &str| -> Step<Option<String>> {
                    match cell.get(key) {
                        Some(Value::String(t)) => placeholder(store, t, s).map(Some),
                        _ => Ok(None),
                    }
                };
                let mut original = cell_text("original")?;
                if original.as_deref() == Some("$face") {
                    let key = cell_text("rowKey")?.unwrap_or_default();
                    let entity = resolve(store, &key)?;
                    original =
                        Some(store.face_value(&entity, &cell_text("field")?.unwrap_or_default())?);
                }
                changes.push(CellChange {
                    row_key: cell_text("rowKey")?,
                    field: cell_text("field")?,
                    value: cell_text("value")?,
                    original,
                    operation: cell_text("operation")?,
                });
            }
            Value::from(store.batch_edit(&a.need("view", s)?, &changes)?)
        }
        "PlanBatchDeletion" => {
            let plan = store.plan_batch_deletion(&a.strings("keys", s)?, a.whole()?)?;
            state.fingerprint = Some(plan.fingerprint.clone());
            plan.to_json()
        }
        "BatchDelete" => store
            .batch_delete(
                &a.strings("keys", s)?,
                a.whole()?,
                &a.need("fingerprint", s)?,
            )?
            .to_json(),
        // 계기의 연산(Store 가 아니다) — C# 쪽 주석.
        "PinClock" => {
            store
                .database()
                .open()?
                .execute_batch(PIN_CLOCK_SQL)
                .map_err(Error::Sqlite)?;
            Value::Null
        }
        other => return Err(Outcome::Fatal(format!("모르는 연산입니다: {other}"))),
    };
    Ok(value)
}

/// C# 계기가 형이 틀린 인자에서 적는 오류 줄 — `JsonElement` 의 `InvalidOperationException` 글 그대로. 형 이름은
/// `JsonValueKind` 의 것이다(불리언은 값에 따라 `True`·`False`).
fn wrong_type(expected: &str, actual: &Value) -> Outcome {
    let kind = match actual {
        Value::Null => "Null",
        Value::Bool(true) => "True",
        Value::Bool(false) => "False",
        Value::Number(_) => "Number",
        Value::String(_) => "String",
        Value::Array(_) => "Array",
        Value::Object(_) => "Object",
    };
    Outcome::Error(Error::InvalidOperation(format!(
        "The requested operation requires an element of type '{expected}', but the target element has type '{kind}'."
    )))
}

/// 글 안 어느 객체에든 같은 키(이스케이프를 푼 글자가 같은 것)가 두 번 나오는가. 이미 읽힌 글에만 쓴다.
fn has_duplicate_keys(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut open: Vec<Option<std::collections::HashSet<String>>> = Vec::new();
    let mut expect_key = false;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => {
                open.push(Some(std::collections::HashSet::new()));
                expect_key = true;
            }
            b'[' => {
                open.push(None);
                expect_key = false;
            }
            b'}' | b']' => {
                open.pop();
                expect_key = false;
            }
            b',' => expect_key = matches!(open.last(), Some(Some(_))),
            b'"' => {
                let start = i;
                i += 1;
                while i < bytes.len() && bytes[i] != b'"' {
                    i += if bytes[i] == b'\\' { 2 } else { 1 };
                }
                if expect_key {
                    expect_key = false;
                    let key: String = serde_json::from_str(&text[start..=i]).unwrap_or_default();
                    if let Some(Some(seen)) = open.last_mut()
                        && !seen.insert(key)
                    {
                        return true;
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    false
}

/// C# `Script.Placeholder` — 글 전체가 자리표시일 때만 푼다. 실제 파일의 값은 대본에 적지 않고 이것으로 집어 온다.
fn placeholder(store: &Store<'_>, text: &str, state: &State) -> Step<String> {
    match text {
        "$firstContract" => first(
            store,
            "SELECT \"계약번호\" FROM \"v_계약\" ORDER BY 1 LIMIT 1;",
            None,
            text,
        ),
        "$firstNotice" => first(
            store,
            "SELECT \"입찰공고번호\" FROM \"v_공고\" ORDER BY 1 LIMIT 1;",
            None,
            text,
        ),
        "$firstRequest" => first(
            store,
            "SELECT \"접수번호\" FROM \"v_접수\" ORDER BY 1 LIMIT 1;",
            None,
            text,
        ),
        "$token" => state
            .token
            .clone()
            .ok_or_else(|| Outcome::Fatal("$token 앞에 PreviewViewPackage 가 없습니다.".into())),
        "$fingerprint" => state.fingerprint.clone().ok_or_else(|| {
            Outcome::Fatal("$fingerprint 앞에 PlanBatchDeletion 이 없습니다.".into())
        }),
        _ => match text.strip_prefix("$viewId:") {
            Some(name) => first(
                store,
                "SELECT id FROM user_view WHERE name = ?1 ORDER BY id LIMIT 1;",
                Some(name),
                text,
            ),
            None => Ok(text.to_string()),
        },
    }
}

fn first(store: &Store<'_>, sql: &str, name: Option<&str>, placeholder: &str) -> Step<String> {
    let connection = store.database().open()?;
    let mut statement = connection.prepare(sql).map_err(Error::Sqlite)?;
    let mut rows = match name {
        Some(name) => statement.query([name]),
        None => statement.query([]),
    }
    .map_err(Error::Sqlite)?;
    let value = match rows.next().map_err(Error::Sqlite)? {
        Some(row) => match row.get_ref(0).map_err(Error::Sqlite)? {
            rusqlite::types::ValueRef::Text(t) => Some(String::from_utf8_lossy(t).into_owned()),
            rusqlite::types::ValueRef::Integer(i) => Some(i.to_string()),
            // 자리표시는 글·정수 열만 집는다(번호·식별자) — 실수 열은 없다.
            rusqlite::types::ValueRef::Real(r) => Some(r.to_string()),
            _ => None,
        },
        None => None,
    };
    value.ok_or_else(|| {
        Outcome::Error(Error::InvalidOperation(format!(
            "자리표시 값이 없습니다: {placeholder}"
        )))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_duplicate_keys_at_any_depth_after_unescaping() {
        assert!(!has_duplicate_keys(
            r#"[{"op":"x","json":"{\"a\":1,\"a\":2}"}]"#
        ));
        assert!(has_duplicate_keys(r#"[{"op":"x","op":"y"}]"#));
        assert!(has_duplicate_keys(
            r#"[{"op":"x","json":{"a":[{"b":1,"b":2}]}}]"#
        ));
        assert!(!has_duplicate_keys(r#"[{"a":{"a":1}},{"a":2}]"#));
    }

    #[test]
    fn wrong_type_writes_the_dotnet_message() {
        let Outcome::Error(Error::InvalidOperation(message)) = wrong_type("Boolean", &Value::Null)
        else {
            panic!("오류 줄이어야 한다");
        };
        assert_eq!(
            message,
            "The requested operation requires an element of type 'Boolean', but the target element has type 'Null'."
        );
    }
}

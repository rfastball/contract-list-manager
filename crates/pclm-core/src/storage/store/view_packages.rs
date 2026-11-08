//! C# `StoreViewPackages.cs` — 보기 구성(맞춤 뷰 몇 개와 그것이 읽는 맞춤 열)을 파일로 내보내고, 미리 보고, 가져온다.
//! 판올림과 빈 작업자료 짓기가 부르는 서식용 예시 보기 심기·갈아 끼우기도 여기 있다.
//!
//! 구성 파일은 `PackageJson`(= `Flow.Json` + 모르는 속성 거절 + 들여쓰기)으로 쓰고 읽는다. 가져오기는 미리보기와 같은
//! 일을 한 트랜잭션에서 실제로 해 본 뒤, 미리보기의 지문(`token`)이 같을 때만 커밋한다.

use std::collections::{HashSet, VecDeque};

use rusqlite::{Connection, params};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use super::user_views::{
    USER_VIEW_NAME_LENGTH, UserViewInfo, ViewInput, clean_view_name, parse_view_object,
    stored_view_definition, usable_id,
};
use super::{
    Store, begin_deferred, check_free, check_kind, clean_name, object, pack, query,
    read_user_columns, refuse_read, single_or_default, text_of,
};
use crate::env::Env;
use crate::error::{Error, Result};
use crate::json::{self, Encoder, JsonResult, for_each_property, read_array, read_object};
use crate::storage::database::{self, refresh};
use crate::storage::flow::{Flow, FlowFormula, formula_value};
use crate::storage::flow_compiler::{self, Target};
use crate::storage::sql_column::{self, Frame};
use crate::storage::{computed_column, user_view_builder};
use crate::text::{cmp_ordinal, ignore_case_key, utf16_len, utf16_prefix};

/// `Store.BuiltinViewPackage` — exe 에 실린 서식용 예시 보기 구성(docs/user-views.md §13). `presets/` 의
/// 파일을 그 자리에서 읽어 넣는다 — 두 벌로 옮겨 적지 않는다(화면의 목업도 같은 파일을 읽는다).
pub fn builtin_view_package() -> &'static str {
    include_str!("../presets/builtin-views.json")
}

/// `Store.ExampleViewPackageV26` — 스키마 26 이 심었던 예시 보기를 얼려 둔 사본. 고치지 않는다.
pub fn example_view_package_v26() -> &'static str {
    include_str!("../presets/example-view-v26.json")
}

/// C# `Store.ViewPackageView(Name, Definition)` — 보기 구성 안의 보기 하나.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewPackageView {
    pub name: Option<String>,
    pub definition: Value,
}

/// C# `Store.ViewPackageColumn(EntityType, FieldName, Kind, Choices, Formula)` — 보기가 읽는 맞춤 열 하나. 자리마다
/// `Option` 인 것은 파일에서 빠지거나 `null` 로 올 수 있어서다 — 그것을 가르는 것은 가져오기의 검사다.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ViewPackageColumn {
    pub entity_type: Option<String>,
    pub field_name: Option<String>,
    pub kind: Option<String>,
    pub choices: Option<Vec<Option<String>>>,
    pub formula: Option<Value>,
}

/// C# `Store.ViewPackage(Format, Version, IncludesAllColumnsForSql, Views, Columns)`. 열 원소가 `null` 일 수 있다.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewPackage {
    pub format: Option<String>,
    pub version: i32,
    pub includes_all_columns_for_sql: bool,
    pub views: Vec<ViewPackageView>,
    pub columns: Vec<Option<ViewPackageColumn>>,
}

/// C# `Store.ViewPackagePlannedView(Name, ImportName)` — 가져오면 설 보기(겹치면 「(복사본)」 이름).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewPackagePlannedView {
    pub name: String,
    pub import_name: String,
}

/// C# `Store.ViewPackagePlannedColumn(EntityType, FieldName, Action)` — `add`·`reuse`·`conflict`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewPackagePlannedColumn {
    pub entity_type: String,
    pub field_name: String,
    pub action: String,
}

/// C# `Store.ViewPackagePreview` — 미리보기. `token` 은 이 미리보기의 지문이다(가져오기가 견준다).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewPackagePreview {
    pub views: Vec<ViewPackagePlannedView>,
    pub columns: Vec<ViewPackagePlannedColumn>,
    pub errors: Vec<String>,
    pub includes_all_columns_for_sql: bool,
    pub token: String,
}

impl ViewPackagePlannedView {
    fn to_json(&self) -> Value {
        object([
            ("name", Value::String(self.name.clone())),
            ("importName", Value::String(self.import_name.clone())),
        ])
    }
}

impl ViewPackagePlannedColumn {
    fn to_json(&self) -> Value {
        object([
            ("entityType", Value::String(self.entity_type.clone())),
            ("fieldName", Value::String(self.field_name.clone())),
            ("action", Value::String(self.action.clone())),
        ])
    }
}

impl ViewPackagePreview {
    /// 다리가 쓰는 꼴.
    pub fn to_json(&self) -> Value {
        object([
            (
                "views",
                Value::Array(
                    self.views
                        .iter()
                        .map(ViewPackagePlannedView::to_json)
                        .collect(),
                ),
            ),
            (
                "columns",
                Value::Array(
                    self.columns
                        .iter()
                        .map(ViewPackagePlannedColumn::to_json)
                        .collect(),
                ),
            ),
            (
                "errors",
                Value::Array(self.errors.iter().cloned().map(Value::String).collect()),
            ),
            (
                "includesAllColumnsForSql",
                Value::Bool(self.includes_all_columns_for_sql),
            ),
            ("token", Value::String(self.token.clone())),
        ])
    }
}

/// `ViewPackage` 를 `Flow.Json`(`WhenWritingNull`) 꼴의 값으로 — 빈 자리는 적지 않는다.
fn package_value(package: &ViewPackage) -> Value {
    let mut root = Map::new();
    if let Some(format) = &package.format {
        root.insert("format".into(), Value::String(format.clone()));
    }
    root.insert("version".into(), package.version.into());
    root.insert(
        "includesAllColumnsForSql".into(),
        Value::Bool(package.includes_all_columns_for_sql),
    );
    let views = package
        .views
        .iter()
        .map(|v| {
            let mut view = Map::new();
            if let Some(name) = &v.name {
                view.insert("name".into(), Value::String(name.clone()));
            }
            view.insert("definition".into(), v.definition.clone());
            Value::Object(view)
        })
        .collect();
    root.insert("views".into(), Value::Array(views));
    let columns = package
        .columns
        .iter()
        .map(|c| match c {
            None => Value::Null,
            Some(c) => {
                let mut column = Map::new();
                for (k, v) in [
                    ("entityType", &c.entity_type),
                    ("fieldName", &c.field_name),
                    ("kind", &c.kind),
                ] {
                    if let Some(v) = v {
                        column.insert(k.into(), Value::String(v.clone()));
                    }
                }
                if let Some(choices) = &c.choices {
                    column.insert(
                        "choices".into(),
                        Value::Array(
                            choices
                                .iter()
                                .map(|x| x.clone().map_or(Value::Null, Value::String))
                                .collect(),
                        ),
                    );
                }
                if let Some(formula) = &c.formula {
                    column.insert("formula".into(), formula.clone());
                }
                Value::Object(column)
            }
        })
        .collect();
    root.insert("columns".into(), Value::Array(columns));
    Value::Object(root)
}

/// `Store.ReadViewPackage` — `PackageJson`(= `Flow.Json` + 모르는 속성 거절)으로 읽고 판·수를 본다.
pub fn read_view_package(json: &str) -> Result<ViewPackage> {
    let bad_json = || {
        Error::invalid(
            "보기 구성 JSON 형식이 올바르지 않습니다. 자료가 아닌 구성 파일을 골라 주세요.",
        )
    };
    if utf16_len(json) > 2_000_000 {
        return Err(Error::invalid(
            "보기 구성 파일은 2MB까지 가져올 수 있습니다.",
        ));
    }
    let root = json::parse(json).map_err(|_| bad_json())?;
    // 같은 키가 두 번 든 구성 파일은 읽을 수 없는 글이다 — 뒤의 값으로 접어 읽으면 안쪽(정의·식)의 겹침이 지워져 C#(막는다)과
    // 갈렸다.
    if json::has_duplicate_keys(json) {
        return Err(bad_json());
    }
    let package = read_package(&root).map_err(|_| bad_json())?;
    let unsupported = || Error::invalid("지원하는 보기 구성 파일이 아닙니다(구성 판 1).");
    let Some(package) = package else {
        return Err(unsupported());
    };
    match &package {
        ViewPackage {
            format: Some(format),
            version: 1,
            views,
            columns,
            ..
        } if format == "pclm-view-config"
            && (1..=100).contains(&views.len())
            && columns.len() <= 500 => {}
        _ => return Err(unsupported()),
    }
    Ok(package)
}

fn read_package(value: &Value) -> JsonResult<Option<ViewPackage>> {
    let Some(map) = read_object(value)? else {
        return Ok(None);
    };
    let mut format = None;
    let mut version = 0;
    let mut includes = false;
    let mut views: Option<Vec<ViewPackageView>> = None;
    let mut columns: Option<Vec<Option<ViewPackageColumn>>> = None;
    for_each_property(
        map,
        &[
            "format",
            "version",
            "includesAllColumnsForSql",
            "views",
            "columns",
        ],
        true,
        |i, v| {
            match i {
                0 => format = json::read_string(v)?,
                1 => version = json::read_required_i32(v)?,
                2 => includes = json::read_bool(v)?.ok_or(json::JsonException)?,
                3 => views = read_array(v, read_view)?,
                _ => columns = read_array(v, read_column)?,
            }
            Ok(())
        },
    )?;
    // 보기·열 목록이 없으면(null) C# 은 판 검사에서 거절한다 — 같은 말이 나가게 빈 목록 대신 0 판으로 둔다.
    let (Some(views), Some(columns)) = (views, columns) else {
        return Ok(None);
    };
    Ok(Some(ViewPackage {
        format,
        version,
        includes_all_columns_for_sql: includes,
        views,
        columns,
    }))
}

fn read_view(value: &Value) -> JsonResult<ViewPackageView> {
    // 원소가 null 인 보기는 C# 에서 뒤에 NullReference 로 터진다 — 여기서는 꼴 오류로 둔다.
    let map = read_object(value)?.ok_or(json::JsonException)?;
    let mut view = ViewPackageView {
        name: None,
        definition: Value::Null,
    };
    for_each_property(map, &["name", "definition"], true, |i, v| {
        match i {
            0 => view.name = json::read_string(v)?,
            _ => view.definition = v.clone(),
        }
        Ok(())
    })?;
    Ok(view)
}

fn read_column(value: &Value) -> JsonResult<Option<ViewPackageColumn>> {
    let Some(map) = read_object(value)? else {
        return Ok(None);
    };
    let mut column = ViewPackageColumn::default();
    for_each_property(
        map,
        &["entityType", "fieldName", "kind", "choices", "formula"],
        true,
        |i, v| {
            match i {
                0 => column.entity_type = json::read_string(v)?,
                1 => column.field_name = json::read_string(v)?,
                2 => column.kind = json::read_string(v)?,
                3 => column.choices = read_array(v, json::read_string)?,
                // `JsonElement?` — JSON null 은 없는 것이다.
                _ => column.formula = (!v.is_null()).then(|| v.clone()),
            }
            Ok(())
        },
    )?;
    Ok(Some(column))
}

/// 보기 하나를 `JsonSerializer.Serialize(view, Flow.Json)` 한 뒤 `ParseViewInput` 으로 읽은 것 — 이름이 없으면 빈 글.
fn view_input(view: &ViewPackageView) -> Result<ViewInput> {
    let mut map = Map::new();
    if let Some(name) = &view.name {
        map.insert("name".into(), Value::String(name.clone()));
    }
    map.insert("definition".into(), view.definition.clone());
    parse_view_object(&map)
}

/// `StoreUserViews.StoredFormula` — 저장 꼴의 필드 식: 가공식 v2 그대로이되 식 식별자는 싣지 않는다.
pub(crate) fn stored_formula(formula: &FlowFormula) -> Result<Value> {
    let mut node = formula_value(formula)?;
    if let Value::Object(map) = &mut node {
        map.shift_remove("id");
    }
    Ok(node)
}

/// `Store.StoredDefinition` — 구성의 보기 하나를 저장 꼴 정의로(화면이 저장하는 것과 같은 꼴).
pub fn stored_definition(view: &ViewPackageView) -> Result<String> {
    let input = view_input(view)?;
    let mut columns = Vec::with_capacity(input.columns.len());
    for (id, name, formula) in input.columns {
        let mut column = Map::new();
        column.insert("id".into(), id.map_or(Value::Null, Value::String));
        column.insert("name".into(), Value::String(name));
        column.insert("formula".into(), stored_formula(&Flow::parse(&formula)?)?);
        columns.push(Value::Object(column));
    }
    let mut root = Map::new();
    root.insert("version".into(), json::number("2"));
    root.insert(
        "contracts".into(),
        Value::String(if input.fold { "fold" } else { "split" }.into()),
    );
    root.insert("columns".into(), Value::Array(columns));
    Ok(json::to_compact(&Value::Object(root), Encoder::Relaxed))
}

/// `Store.SameJson` — 객체 키 차례는 보지 않고 배열 차례와 값은 본다. 읽을 수 없는 글과 같은 키가 두 번 든 글은 다르다.
pub(crate) fn same_json(a: &str, b: &str) -> bool {
    fn canon(node: &Value) -> String {
        match node {
            Value::Object(o) => {
                let mut entries: Vec<_> = o.iter().collect();
                entries.sort_by(|x, y| cmp_ordinal(x.0, y.0));
                let parts: Vec<String> = entries
                    .into_iter()
                    .map(|(k, v)| {
                        let mut key = String::new();
                        json::write_string(&mut key, k, Encoder::Default);
                        format!("{key}:{}", canon(v))
                    })
                    .collect();
                format!("{{{}}}", parts.join(","))
            }
            Value::Array(items) => {
                format!(
                    "[{}]",
                    items.iter().map(canon).collect::<Vec<_>>().join(",")
                )
            }
            other => json::to_compact(other, Encoder::Default),
        }
    }
    // 같은 키가 두 번 든 글은 어디에 들었든 같지 않다 — 손댄 정의이고, C# 은 그것을 JsonNode 로 열다 던진다.
    match (json::parse(a), json::parse(b)) {
        (Ok(x), Ok(y)) => {
            !json::has_duplicate_keys(a) && !json::has_duplicate_keys(b) && canon(&x) == canon(&y)
        }
        _ => false,
    }
}

/// `Store.SeedExampleViews` — 예시 보기를 작업자료에 심는다. 같은 이름(대소문자 무시)의 보기가 이미 있으면 그것을
/// 두고 넘어간다. `package` 가 없으면 지금 구성([`builtin_view_package`])이다. 뷰를 짓는 것은 부르는 쪽이다.
///
/// 보기 식별자는 C# 의 `Guid.NewGuid().ToString("N")` 자리라 [`Env::new_id`] 로 짓는다.
pub(crate) fn seed_example_views(
    connection: &Connection,
    package: Option<&str>,
    env: &dyn Env,
) -> Result<()> {
    let mut taken: HashSet<String> = connection
        .prepare("SELECT name FROM user_view")?
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?
        .iter()
        .map(|n| ignore_case_key(n))
        .collect();
    for view in read_view_package(package.unwrap_or(builtin_view_package()))?.views {
        let name = view.name.clone().unwrap_or_default();
        if !taken.insert(ignore_case_key(&name)) {
            continue;
        }
        connection.execute(
            "INSERT INTO user_view (id, name, definition, sort_order)\n\
             VALUES (?1, ?2, ?3, coalesce((SELECT max(sort_order) FROM user_view), 0) + 10)",
            params![env.new_id(), name, stored_definition(&view)?],
        )?;
    }
    Ok(())
}

/// `Store.UpgradeExampleViews` — 스키마 27. v26 이 심은 예시 보기를 사람이 손대지 않았으면(이름이 같고 저장된
/// 정의가 v26 의 것과 같은 JSON) 지금 구성으로 갈아 끼운다. 고쳤거나 지웠거나 이름을 바꾼 보기는 그대로 둔다.
pub(crate) fn upgrade_example_views(connection: &Connection) -> Result<()> {
    let current = read_view_package(builtin_view_package())?.views;
    for old in read_view_package(example_view_package_v26())?.views {
        let Some(next) = current.iter().find(|v| v.name == old.name) else {
            continue;
        };
        let stored: Vec<(String, String)> = connection
            .prepare("SELECT id, definition FROM user_view WHERE name = ?1")?
            .query_map([&old.name], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let [(id, definition)] = stored.as_slice() else {
            continue;
        };
        if !same_json(definition, &stored_definition(&old)?) {
            continue;
        }
        connection.execute(
            "UPDATE user_view SET definition = ?1 WHERE id = ?2",
            params![stored_definition(next)?, id],
        )?;
    }
    Ok(())
}

/// `Store.CopyViewName` — 이미 있는 이름이면 「(복사본)」·「(복사본 2)」… 를 붙인다. 길이 한도 안으로 앞을 자른다.
fn copy_view_name(name: &str, taken: &mut HashSet<String>) -> String {
    if taken.insert(ignore_case_key(name)) {
        return name.to_string();
    }
    let mut i = 1;
    loop {
        let suffix = if i == 1 {
            " (복사본)".to_string()
        } else {
            format!(" (복사본 {i})")
        };
        let keep = utf16_len(name).min(USER_VIEW_NAME_LENGTH - utf16_len(&suffix));
        let candidate = format!("{}{suffix}", utf16_prefix(name, keep));
        if taken.insert(ignore_case_key(&candidate)) {
            return candidate;
        }
        i += 1;
    }
}

/// `Store.ConflictMessage` — 받는 사람이 할 수 있는 길을 먼저 말한다. 조사(과/와)는 이름 끝 글자의 받침을 본다.
fn conflict_message(entity_type: &str, mine: &str) -> String {
    let kind = match entity_type {
        "request" => "접수",
        "notice" => "공고",
        _ => "계약",
    };
    // C# `mine[^1]` — 끝 UTF-16 단위. BMP 밖 글자면 뒤 서로게이트라 받침이 없다.
    let c = mine.chars().last().unwrap_or(' ');
    let batchim = if ('가'..='힣').contains(&c) {
        !(c as u32 - '가' as u32).is_multiple_of(28)
    } else {
        matches!(c, '0' | '1' | '3' | '6' | '7' | '8')
    };
    format!(
        "내 {kind} 열 「{mine}」 {} 정의가 다릅니다. 열 관리에서 내 열 이름을 바꾸면 이 구성을 가져올 수 있습니다. \
         보낸 사람에게 열 이름을 바꿔 다시 내보내 달라고 해도 됩니다.",
        if batchim { "과" } else { "와" }
    )
}

/// `JsonElement.GetRawText()` 자리 — 다시 읽기만 하므로 같은 값의 JSON 글이면 된다.
fn raw_text(value: Option<&Value>) -> String {
    value.map_or_else(String::new, |v| json::to_compact(v, Encoder::Relaxed))
}

/// 맞춤 열 하나를 넣어 본 결과 — 계획과, 겹쳤으면 그 말.
type StagedColumn = (ViewPackagePlannedColumn, Option<String>);

impl Store<'_> {
    /// `Store.ExportViewPackage` — 고른 보기와 그것이 (가공식을 타고) 읽는 맞춤 열을 구성 파일 글로. SQL 필드가
    /// 하나라도 있으면 맞춤 열을 모두 싣는다 — SQL 은 간접 참조를 숨길 수 있어 글에서 짐작하지 않는다.
    pub fn export_view_package(&self, ids: &[String]) -> Result<String> {
        let connection = self.database.open_read_only()?;
        let _transaction = begin_deferred(&connection)?;
        let saved = user_view_builder::read_definitions(&connection)?;
        let chosen: Vec<_> = saved.iter().filter(|v| ids.contains(&v.id)).collect();
        let distinct: HashSet<&String> = ids.iter().collect();
        if chosen.is_empty() || distinct.len() != chosen.len() {
            return Err(Error::invalid(
                "내보낼 보기를 하나 이상 고르세요. 삭제된 보기는 목록을 새로 읽어 주세요.",
            ));
        }
        let all = read_user_columns(
            &connection,
            "SELECT entity_type, field_name, kind, options, sort_order FROM user_column ORDER BY entity_type, sort_order",
            [],
        )?;
        let mut needed: HashSet<(String, String)> = HashSet::new();
        let mut formulas: VecDeque<(Option<String>, String)> = chosen
            .iter()
            .flat_map(|v| v.columns.iter().map(|c| (None, c.formula.clone())))
            .collect();
        let mut sql = false;
        while let Some((entity, text)) = formulas.pop_front() {
            let formula = Flow::parse(&text)?;
            if formula.source.as_deref() == Some("sql") {
                sql = true;
                for c in &all {
                    if needed.insert((c.entity_type.clone(), c.field_name.clone()))
                        && c.kind == "computed"
                    {
                        formulas.push_back((
                            Some(c.entity_type.clone()),
                            c.options.clone().unwrap_or_default(),
                        ));
                    }
                }
            } else if entity.is_none() {
                let mut node = json::parse(&text).ok();
                for c in &all {
                    if user_view_builder::touch(node.as_mut(), &c.entity_type, &c.field_name, None)
                        && needed.insert((c.entity_type.clone(), c.field_name.clone()))
                        && c.kind == "computed"
                    {
                        formulas.push_back((
                            Some(c.entity_type.clone()),
                            c.options.clone().unwrap_or_default(),
                        ));
                    }
                }
            }
        }
        let views = chosen
            .iter()
            .map(|v| {
                Ok(ViewPackageView {
                    name: Some(v.name.clone()),
                    // 읽지 못하는 정의(깨진 글·같은 키가 두 번 든 글)는 형식 오류다(C# `StoredDefinition`).
                    definition: json::parse(&v.definition)
                        .ok()
                        .filter(|_| !json::has_duplicate_keys(&v.definition))
                        .ok_or_else(|| Error::invalid("맞춤 뷰 형식이 올바르지 않습니다."))?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let columns = all
            .iter()
            .filter(|c| needed.contains(&(c.entity_type.clone(), c.field_name.clone())))
            .map(|c| {
                Ok(Some(ViewPackageColumn {
                    entity_type: Some(c.entity_type.clone()),
                    field_name: Some(c.field_name.clone()),
                    kind: Some(c.kind.clone()),
                    choices: Some(c.choices().into_iter().map(Some).collect()),
                    formula: if c.kind == "computed" {
                        Some(formula_value(&computed_column::parse(
                            c.options.as_deref().unwrap_or(""),
                        )?)?)
                    } else {
                        None
                    },
                }))
            })
            .collect::<Result<Vec<_>>>()?;
        let package = ViewPackage {
            format: Some("pclm-view-config".into()),
            version: 1,
            includes_all_columns_for_sql: sql,
            views,
            columns,
        };
        Ok(json::to_indented(
            &package_value(&package),
            Encoder::Relaxed,
        ))
    }

    /// `Store.PreviewViewPackage` — 가져오면 무엇이 서는지 한 트랜잭션에서 실제로 해 보고 되돌린다.
    pub fn preview_view_package(&self, json: &str) -> Result<ViewPackagePreview> {
        refuse_read(
            &self.database,
            "열람 중에는 보기 구성을 가져올 수 없습니다.",
        )?;
        let connection = self.database.open()?;
        let transaction = database::begin(&connection)?;
        // 놓으면 되돌린다 — 미리보기는 아무것도 남기지 않는다.
        self.stage_view_package(&transaction, &read_view_package(json)?)
    }

    /// `Store.ImportViewPackage` — 미리보기와 같은 일을 하고, 오류가 없고 지문이 미리보기의 것과 같을 때만 커밋한다.
    pub fn import_view_package(&self, json: &str, token: &str) -> Result<Vec<UserViewInfo>> {
        refuse_read(
            &self.database,
            "열람 중에는 보기 구성을 가져올 수 없습니다.",
        )?;
        {
            let connection = self.database.open()?;
            let transaction = database::begin(&connection)?;
            let preview = self.stage_view_package(&transaction, &read_view_package(json)?)?;
            if !preview.errors.is_empty() {
                return Err(Error::InvalidOperation(preview.errors.join("\n")));
            }
            if preview.token != token {
                return Err(Error::invalid(
                    "미리보기 뒤 구성이 바뀌었습니다. 파일을 다시 골라 확인하세요. 가져온 것은 없습니다.",
                ));
            }
            transaction.commit()?;
        }
        self.user_views()
    }

    /// `Store.StageViewPackage` — 맞춤 열과 보기를 넣어 보고 뷰를 다시 지어 본다. 오류는 모아 낸다.
    fn stage_view_package(
        &self,
        connection: &Connection,
        package: &ViewPackage,
    ) -> Result<ViewPackagePreview> {
        let mut errors: Vec<String> = Vec::new();
        let mut planned_columns = Vec::new();
        let mut planned_views = Vec::new();
        let mut view_ids: Vec<String> = Vec::new();
        let mut seen_columns: HashSet<(String, String)> = HashSet::new();
        for c in &package.columns {
            match self.stage_column(connection, c.as_ref(), &mut seen_columns) {
                Ok((planned, conflict)) => {
                    planned_columns.push(planned);
                    errors.extend(conflict);
                }
                Err(e @ (Error::InvalidOperation(_) | Error::Sqlite(_) | Error::Argument(_))) => {
                    errors.push(format!(
                        "{}.{}: {}",
                        c.as_ref()
                            .and_then(|c| c.entity_type.as_deref())
                            .unwrap_or(""),
                        c.as_ref()
                            .and_then(|c| c.field_name.as_deref())
                            .unwrap_or(""),
                        sql_column::error_message(&e)
                    ));
                }
                Err(other) => return Err(other),
            }
        }
        let mut taken: HashSet<String> =
            query(connection, "SELECT name FROM user_view", [], |r| {
                Ok(ignore_case_key(&text_of(r.get_ref(0)?).unwrap_or_default()))
            })?
            .into_iter()
            .collect();
        for v in &package.views {
            match self.stage_view(connection, v, &mut taken) {
                Ok((view_id, planned)) => {
                    view_ids.push(view_id);
                    planned_views.push(planned);
                }
                Err(e @ (Error::InvalidOperation(_) | Error::Sqlite(_) | Error::Argument(_))) => {
                    errors.push(format!(
                        "{}: {}",
                        v.name.as_deref().unwrap_or(""),
                        sql_column::error_message(&e)
                    ));
                }
                Err(other) => return Err(other),
            }
        }
        if errors.is_empty() {
            let checked = (|| -> Result<()> {
                let broken = refresh(connection)?;
                for c in package.columns.iter().flatten() {
                    let key = (
                        c.entity_type.clone().unwrap_or_default(),
                        c.field_name.clone().unwrap_or_default(),
                    );
                    if let Some(why) = broken.columns.get(&key) {
                        errors.push(format!("{}.{}: {why}", key.0, key.1));
                    }
                }
                for ((view, _), why) in broken.user_views.iter() {
                    if view_ids.contains(view) {
                        errors.push(format!("보기 필드: {why}"));
                    }
                }
                // 보통 저장과 같은 한 행 값 수 검사를 들인 정의 위에서 한다.
                connection.execute_batch("PRAGMA query_only=ON")?;
                let validated = validate_staged(connection, package, &view_ids);
                connection.execute_batch("PRAGMA query_only=OFF")?;
                validated
            })();
            match checked {
                Ok(()) => {}
                Err(e @ (Error::InvalidOperation(_) | Error::Sqlite(_))) => {
                    errors.push(sql_column::error_message(&e));
                }
                Err(other) => return Err(other),
            }
        }
        let stamp = object([
            ("package", package_value(package)),
            (
                "plannedViews",
                Value::Array(
                    planned_views
                        .iter()
                        .map(ViewPackagePlannedView::to_json)
                        .collect(),
                ),
            ),
            (
                "plannedColumns",
                Value::Array(
                    planned_columns
                        .iter()
                        .map(ViewPackagePlannedColumn::to_json)
                        .collect(),
                ),
            ),
            (
                "errors",
                Value::Array(errors.iter().cloned().map(Value::String).collect()),
            ),
        ]);
        let token = Sha256::digest(json::to_compact(&stamp, Encoder::Relaxed).as_bytes())
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect();
        Ok(ViewPackagePreview {
            views: planned_views,
            columns: planned_columns,
            errors,
            includes_all_columns_for_sql: package.includes_all_columns_for_sql,
            token,
        })
    }

    /// 구성의 맞춤 열 하나를 넣어 본다 — `add`·`reuse`·`conflict`. 겹쳤으면 그 말도 낸다.
    fn stage_column(
        &self,
        connection: &Connection,
        c: Option<&ViewPackageColumn>,
        seen_columns: &mut HashSet<(String, String)>,
    ) -> Result<StagedColumn> {
        let bad = || Error::invalid("맞춤 열의 자료 종류 또는 후보 형식이 올바르지 않습니다.");
        let c = c.ok_or_else(bad)?;
        let choices = c.choices.as_ref().ok_or_else(bad)?;
        if choices.iter().any(Option::is_none) {
            return Err(bad());
        }
        let entity_type = match c.entity_type.as_deref() {
            Some(e @ ("notice" | "contract" | "request")) => e,
            _ => return Err(bad()),
        };
        let choices: Vec<String> = choices.iter().flatten().cloned().collect();
        let name = clean_name(c.field_name.as_deref().unwrap_or(""))?;
        if !seen_columns.insert((entity_type.to_string(), name.clone())) {
            return Err(Error::InvalidOperation(format!(
                "구성 안에서 열이 겹칩니다: {name}"
            )));
        }
        let kind = c.kind.as_deref();
        let mut options: Option<String>;
        if kind == Some("computed") {
            let Some(f) = &c.formula else {
                return Err(Error::invalid("가공식이 없습니다."));
            };
            let mut formula = computed_column::parse_for(entity_type, &raw_text(Some(f)))?;
            if formula.source.as_deref() == Some("sql") {
                sql_column::check_shape(formula.sql.as_deref())?;
            } else {
                flow_compiler::expression(entity_type, &formula, Target::Internal, None)?;
            }
            formula.id = Some(String::new());
            options = Some(Flow::serialize(&formula)?);
        } else {
            check_kind(kind.unwrap_or(""))?;
            if c.formula.is_some() {
                return Err(Error::invalid("입력열에는 활성 가공식을 넣을 수 없습니다."));
            }
            options = pack(kind.unwrap_or(""), &choices);
        }
        let planned = |action: &str| ViewPackagePlannedColumn {
            entity_type: entity_type.to_string(),
            field_name: name.clone(),
            action: action.into(),
        };
        let existing = single_or_default(
            connection,
            "SELECT field_name, kind, options FROM user_column WHERE entity_type=?1 AND field_name=?2 COLLATE NOCASE",
            params![entity_type, name],
            |r| {
                Ok((
                    text_of(r.get_ref(0)?).unwrap_or_default(),
                    text_of(r.get_ref(1)?).unwrap_or_default(),
                    text_of(r.get_ref(2)?),
                ))
            },
        )?;
        if let Some((old_name, old_kind, old_options)) = existing {
            let old_options = if old_kind == "computed" {
                let mut f = computed_column::parse(old_options.as_deref().unwrap_or(""))?;
                f.id = Some(String::new());
                Some(Flow::serialize(&f)?)
            } else if old_kind == "choice" {
                old_options
            } else {
                None
            };
            if old_name != name || Some(old_kind.as_str()) != kind || old_options != options {
                // 받는 사람이 할 수 있는 길을 먼저 말한다 — 원본을 고치는 것은 보낸 사람의 일이다.
                return Ok((
                    planned("conflict"),
                    Some(conflict_message(entity_type, &old_name)),
                ));
            }
            return Ok((planned("reuse"), None));
        }
        check_free(connection, entity_type, &name, None)?;
        if kind == Some("computed") {
            let mut f = Flow::parse(options.as_deref().unwrap_or(""))?;
            f.id = Some(self.env.new_id());
            options = Some(Flow::serialize(&f)?);
        }
        connection.execute(
            "INSERT INTO user_column(entity_type,field_name,kind,options,sort_order)\n\
             VALUES(?1,?2,?3,?4,coalesce((SELECT max(sort_order) FROM user_column WHERE entity_type=?1),0)+10)",
            params![entity_type, name, kind, options],
        )?;
        Ok((planned("add"), None))
    }

    /// 구성의 보기 하나를 넣어 본다. (보기 식별자, 계획) 을 낸다.
    fn stage_view(
        &self,
        connection: &Connection,
        v: &ViewPackageView,
        taken: &mut HashSet<String>,
    ) -> Result<(String, ViewPackagePlannedView)> {
        let version_two = match &v.definition {
            Value::Object(d) => matches!(
                d.get("version"),
                Some(Value::Number(n)) if n.to_string().parse::<i32>().ok() == Some(2)
            ),
            _ => false,
        };
        if !version_two {
            return Err(Error::invalid("보기 정의는 판 2여야 합니다."));
        }
        let input = view_input(v)?;
        let name = clean_view_name(&input.name, "보기 이름")?;
        if name.is_empty() || input.columns.is_empty() {
            return Err(Error::invalid("보기 이름과 필드가 필요합니다."));
        }
        let mut seen_names = HashSet::new();
        let mut ids = HashSet::new();
        let mut columns = Vec::new();
        for (id, column_name, formula) in &input.columns {
            let field = clean_view_name(column_name, "필드 이름")?;
            if field.is_empty() || !seen_names.insert(ignore_case_key(&field)) {
                return Err(Error::invalid("필드 이름이 비었거나 겹칩니다."));
            }
            // C# 객체 초기자의 차례 — 식별자를 먼저 짓고 식을 읽는다.
            let id = match usable_id(id.as_deref(), &mut ids) {
                Some(id) => id,
                None => self.new_column_id(&mut ids),
            };
            columns.push((id, field, stored_formula(&Flow::parse(formula)?)?));
        }
        let import_name = copy_view_name(&name, taken);
        let definition = stored_view_definition(input.fold, columns);
        let view_id = self.env.new_id();
        connection.execute(
            "INSERT INTO user_view(id,name,definition,sort_order)\n\
             VALUES(?1,?2,?3,coalesce((SELECT max(sort_order) FROM user_view),0)+10)",
            params![view_id, import_name, definition],
        )?;
        Ok((view_id, ViewPackagePlannedView { name, import_name }))
    }
}

/// 들인 정의 위에서 SQL 열·필드의 한 행 값 수를 본다(`query_only` 를 건 채).
fn validate_staged(
    connection: &Connection,
    package: &ViewPackage,
    view_ids: &[String],
) -> Result<()> {
    for c in package.columns.iter().flatten() {
        if c.kind.as_deref() != Some("computed") {
            continue;
        }
        let f = Flow::parse(&raw_text(c.formula.as_ref()))?;
        if f.source.as_deref() == Some("sql") {
            sql_column::validate(
                connection,
                c.entity_type.as_deref().unwrap_or(""),
                f.sql.as_deref().unwrap_or(""),
                &[],
            )?;
        }
    }
    for v in user_view_builder::read_definitions(connection)?
        .into_iter()
        .filter(|v| view_ids.contains(&v.id))
    {
        let frame = user_view_builder::frame(v.fold, v.columns.iter().map(|c| c.formula.as_str()));
        for c in &v.columns {
            let f = Flow::parse(&c.formula)?;
            if f.source.as_deref() == Some("sql") {
                sql_column::validate_on(
                    connection,
                    f.sql.as_deref().unwrap_or(""),
                    &Frame::UserView(&frame),
                    &[],
                    20,
                )?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_json_ignores_key_order_but_not_values() {
        assert!(same_json(r#"{"a":1,"b":[1,2]}"#, r#"{"b":[1,2],"a":1}"#));
        assert!(!same_json(r#"{"a":1.0}"#, r#"{"a":1}"#));
        assert!(!same_json(r#"{"a":[1,2]}"#, r#"{"a":[2,1]}"#));
        assert!(same_json(r#"{"a":"A"}"#, r#"{"a":"A"}"#));
        assert!(!same_json("{", "{"));
    }

    /// C# `겹친_키가_든_JSON_은_같은_JSON_이_아니다` — 같은 키가 두 번 든 글은 어디에 들었든 같지 않다.
    #[test]
    fn same_json_refuses_duplicate_keys() {
        assert!(same_json(r#"{"a":1,"b":[1,2]}"#, r#"{"b":[1,2],"a":1}"#));
        assert!(!same_json(r#"{"a":1,"a":1}"#, r#"{"a":1}"#));
        assert!(!same_json(r#"{"a":1,"a":1}"#, r#"{"a":1,"a":1}"#));
        assert!(!same_json(r#"{"a":[{"b":1,"b":2}]}"#, r#"{"a":[{"b":2}]}"#));
        assert!(!same_json(r#"{"a":1}"#, r#"{"a":1,"\u0061":1}"#));
        assert!(same_json(r#"{"a":1,"A":1}"#, r#"{"A":1,"a":1}"#));
    }

    #[test]
    fn presets_are_supported_packages() {
        assert_eq!(
            read_view_package(builtin_view_package())
                .unwrap()
                .views
                .len(),
            1
        );
        assert_eq!(
            read_view_package(example_view_package_v26())
                .unwrap()
                .views
                .len(),
            1
        );
    }

    #[test]
    fn conflict_message_picks_particle() {
        assert!(conflict_message("notice", "메모").contains("「메모」 와"));
        assert!(conflict_message("contract", "담당").contains("「담당」 과"));
        assert!(conflict_message("request", "칸3").contains("「칸3」 과"));
        assert!(conflict_message("request", "").contains("「」 와"));
    }

    #[test]
    fn copy_view_name_adds_suffix_within_limit() {
        let mut taken = HashSet::new();
        assert_eq!(copy_view_name("보기", &mut taken), "보기");
        assert_eq!(copy_view_name("보기", &mut taken), "보기 (복사본)");
        assert_eq!(copy_view_name("보기", &mut taken), "보기 (복사본 2)");
        let long = "가".repeat(40);
        assert_eq!(copy_view_name(&long, &mut taken), long);
        assert_eq!(
            copy_view_name(&long, &mut taken),
            format!("{} (복사본)", "가".repeat(34))
        );
        assert_eq!(
            copy_view_name(&long, &mut taken),
            format!("{} (복사본 2)", "가".repeat(32))
        );
    }
}

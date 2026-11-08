//! 수집 매핑(C# `Erp/Mapping.cs`) — 매핑 레코드와 그 읽기·검증·판, 원천 JSON 을 업무 표의 행으로 옮기기, 매핑 저장.
//!
//! SQL 식별자와 변환 종류는 프로그램이 허용한다. 매핑 파일은 실행 코드가 아니다 — 기본 매핑(`mapping.json`, C# 의 내장
//! 자원과 같은 파일)의 목적지 목록이 곧 수집 허용 목록이다.
//!
//! **판은 바이트로 정해진다.** `Mapping.Hash` 는 레코드를 `Mapping.Json`(Web + 들여쓰기 + 기본 인코더)으로 쓴 글의
//! SHA-256 이다. 레코드의 속성 차례·`null` 을 적는 자리·기본값이 그 글을 정하므로, 받은 JSON 은 그 꼴을 적은 표
//! ([`FIELD_MAP`] 등)로 다시 세우고, 레코드는 그 차례로 다시 쓴다([`MappingSet::to_value`]). 들여쓰기의 줄바꿈은
//! .NET 8 의 `Environment.NewLine` — Windows 에서 `\r\n` 이다(실측). 기본 매핑의 판은 시험이 C# 이 낸 값과 견준다.
//!
//! **레코드의 칸은 C# 처럼 비어 있을 수 있다.** 역직렬화는 빠진 칸을 `null` 로 채우고 거절은 [`validate`] 의 몫이라,
//! 그 사이의 값을 그대로 들고 다니려고 참조형 칸은 모두 `Option` 이다. 검증을 지난 매핑은 비지 않는다 — 걷는 쪽은
//! `*_iter`·글 접근자로 읽는다.

use std::sync::OnceLock;

use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use super::decimal::Decimal;
use super::element;
use crate::env::Env;
use crate::error::{Error, Result};
use crate::json::{self, Encoder, JsonException, JsonResult, read_bool, read_i32, read_string};
use crate::storage::database::{Database, begin};
use crate::text::{eq_ignore_case, is_blank, utf16_len};

// ── 레코드의 꼴 ─────────────────────────────────────────────

/// 레코드 자리의 꼴 — C# 레코드의 형과 같다.
enum Shape {
    Str,
    Int,
    Bool,
    /// `Dictionary<string, string>` — 키 차례 그대로.
    Dict,
    Array(&'static Shape),
    Record(&'static [Prop]),
}

/// 레코드 속성 하나 — 이름(camelCase), 꼴, JSON 에 없을 때의 값(생성자 기본값).
struct Prop(&'static str, &'static Shape, Missing);

#[derive(Clone, Copy)]
enum Missing {
    Null,
    Zero,
    False,
    Text(&'static str),
}

static STR: Shape = Shape::Str;
static INT: Shape = Shape::Int;
static BOOL: Shape = Shape::Bool;
static DICT: Shape = Shape::Dict;
static STRS: Shape = Shape::Array(&STR);

/// `FieldMap(Source, Target, Transform = "text", Codes = null, SecondarySource = null)`.
static FIELD_MAP: Shape = Shape::Record(&[
    Prop("source", &STR, Missing::Null),
    Prop("target", &STR, Missing::Null),
    Prop("transform", &STR, Missing::Text("text")),
    Prop("codes", &DICT, Missing::Null),
    Prop("secondarySource", &STR, Missing::Null),
]);
static FIELD_MAPS: Shape = Shape::Array(&FIELD_MAP);

/// `TableMap(Source, Target, Keys, Fields, Required = false, TotalSelector = null)`.
static TABLE_MAP: Shape = Shape::Record(&[
    Prop("source", &STR, Missing::Null),
    Prop("target", &STR, Missing::Null),
    Prop("keys", &STRS, Missing::Null),
    Prop("fields", &FIELD_MAPS, Missing::Null),
    Prop("required", &BOOL, Missing::False),
    Prop("totalSelector", &STR, Missing::Null),
]);
static TABLE_MAPS: Shape = Shape::Array(&TABLE_MAP);

/// `MappingScreen(Code, Name)`.
static SCREEN: Shape = Shape::Record(&[
    Prop("code", &STR, Missing::Null),
    Prop("name", &STR, Missing::Null),
]);

/// `MappingProfile(Id, EntityType, IdentitySource, BaseField, SeqField, Fields, Tables, Heading, Selectors, Screen)`.
static PROFILE: Shape = Shape::Record(&[
    Prop("id", &STR, Missing::Null),
    Prop("entityType", &STR, Missing::Null),
    Prop("identitySource", &STR, Missing::Null),
    Prop("baseField", &STR, Missing::Null),
    Prop("seqField", &STR, Missing::Null),
    Prop("fields", &FIELD_MAPS, Missing::Null),
    Prop("tables", &TABLE_MAPS, Missing::Null),
    Prop("heading", &STR, Missing::Null),
    Prop("selectors", &DICT, Missing::Null),
    Prop("screen", &SCREEN, Missing::Null),
]);
static PROFILES: Shape = Shape::Array(&PROFILE);

/// `MappingSet(Version, Profiles)`.
static MAPPING_SET: Shape = Shape::Record(&[
    Prop("version", &INT, Missing::Zero),
    Prop("profiles", &PROFILES, Missing::Null),
]);

/// `MappedRow(Table, SourceKey, Values, Origins = null)`.
static MAPPED_ROW: Shape = Shape::Record(&[
    Prop("table", &STR, Missing::Null),
    Prop("sourceKey", &STR, Missing::Null),
    Prop("values", &DICT, Missing::Null),
    Prop("origins", &DICT, Missing::Null),
]);
static MAPPED_ROWS: Shape = Shape::Array(&MAPPED_ROW);

/// `MappedDocument(Profile, EntityType, Base, Seq, Rows, Scope)`.
static MAPPED_DOCUMENT: Shape = Shape::Record(&[
    Prop("profile", &STR, Missing::Null),
    Prop("entityType", &STR, Missing::Null),
    Prop("base", &STR, Missing::Null),
    Prop("seq", &STR, Missing::Null),
    Prop("rows", &MAPPED_ROWS, Missing::Null),
    Prop("scope", &STR, Missing::Null),
]);

/// 역직렬화가 멎은 자리 — `JsonException.Path`(`$.profiles[0].fields`)와, 모르는 칸 때문인가.
struct ShapeError {
    path: String,
    unknown: bool,
}

/// System.Text.Json 의 경로 한 단. 특별한 글자가 든 이름은 `['이름']` 이다.
fn segment(path: &str, name: &str) -> String {
    const SPECIAL: &[char] = &[
        '.', ' ', '\'', '/', '"', '[', ']', '(', ')', '\t', '\n', '\r', '\u{c}', '\u{8}', '\\',
        '\u{85}', '\u{2028}', '\u{2029}',
    ];
    if name.contains(SPECIAL) {
        format!("{path}['{name}']")
    } else {
        format!("{path}.{name}")
    }
}

/// 받은 JSON 을 `Mapping.Json` 으로 역직렬화했다가 다시 쓴 꼴로 세운다 — 모르는 속성은 거절하고
/// (`UnmappedMemberHandling.Disallow`), 이름은 대소문자를 가리지 않으며(같은 이름이 둘이면 뒤의 것), 빠진 속성은
/// 생성자 기본값으로 채우고 속성은 레코드 차례로 놓는다. 멎으면 그 자리의 경로를 낸다 — 글의 차례로 처음 걸린 것이다.
fn normalize_at(
    value: &Value,
    shape: &Shape,
    path: &str,
) -> std::result::Result<Value, ShapeError> {
    let fail = || ShapeError {
        path: path.to_string(),
        unknown: false,
    };
    match shape {
        Shape::Str => Ok(read_string(value)
            .map_err(|_| fail())?
            .map_or(Value::Null, Value::String)),
        Shape::Int => {
            let n = read_i32(value).map_err(|_| fail())?.ok_or_else(fail)?;
            Ok(json::number(&n.to_string()))
        }
        Shape::Bool => Ok(Value::Bool(
            read_bool(value).map_err(|_| fail())?.ok_or_else(fail)?,
        )),
        Shape::Dict => match value {
            Value::Null => Ok(Value::Null),
            Value::Object(map) => {
                let mut out = Map::new();
                for (key, item) in map {
                    let text = read_string(item).map_err(|_| ShapeError {
                        path: segment(path, key),
                        unknown: false,
                    })?;
                    out.insert(key.clone(), text.map_or(Value::Null, Value::String));
                }
                Ok(Value::Object(out))
            }
            _ => Err(fail()),
        },
        Shape::Array(item) => match value {
            Value::Null => Ok(Value::Null),
            Value::Array(items) => items
                .iter()
                .enumerate()
                .map(|(i, v)| normalize_at(v, item, &format!("{path}[{i}]")))
                .collect::<std::result::Result<Vec<_>, _>>()
                .map(Value::Array),
            _ => Err(fail()),
        },
        Shape::Record(props) => {
            let map = match value {
                Value::Null => return Ok(Value::Null),
                Value::Object(map) => map,
                _ => return Err(fail()),
            };
            let mut slots: Vec<Option<Value>> = vec![None; props.len()];
            for (key, item) in map {
                match props.iter().position(|p| eq_ignore_case(p.0, key)) {
                    Some(i) => {
                        slots[i] = Some(normalize_at(item, props[i].1, &segment(path, key))?)
                    }
                    None => {
                        return Err(ShapeError {
                            path: segment(path, key),
                            unknown: true,
                        });
                    }
                }
            }
            let mut out = Map::new();
            for (prop, slot) in props.iter().zip(slots) {
                let value = match (slot, prop.2) {
                    (Some(v), _) => v,
                    (None, Missing::Null) => Value::Null,
                    (None, Missing::Zero) => json::number("0"),
                    (None, Missing::False) => Value::Bool(false),
                    (None, Missing::Text(t)) => Value::String(t.into()),
                };
                out.insert(prop.0.into(), value);
            }
            Ok(Value::Object(out))
        }
    }
}

fn normalize(value: &Value, shape: &Shape) -> JsonResult<Value> {
    normalize_at(value, shape, "$").map_err(|_| JsonException)
}

// ── 레코드 ─────────────────────────────────────────────

/// `Dictionary<string, string?>` — 넣은 차례 그대로.
pub type Pairs = Vec<(String, Option<String>)>;

fn text_value(text: &Option<String>) -> Value {
    text.clone().map_or(Value::Null, Value::String)
}

fn pairs_value(pairs: &[(String, Option<String>)]) -> Value {
    Value::Object(
        pairs
            .iter()
            .map(|(k, v)| (k.clone(), text_value(v)))
            .collect(),
    )
}

fn optional_pairs_value(pairs: &Option<Pairs>) -> Value {
    pairs.as_deref().map_or(Value::Null, pairs_value)
}

fn object<const N: usize>(entries: [(&str, Value); N]) -> Value {
    Value::Object(
        entries
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
    )
}

fn get_text(map: &Map<String, Value>, key: &str) -> Option<String> {
    map.get(key).and_then(Value::as_str).map(str::to_string)
}

fn get_pairs(map: &Map<String, Value>, key: &str) -> Option<Pairs> {
    map.get(key).and_then(Value::as_object).map(|m| {
        m.iter()
            .map(|(k, v)| (k.clone(), v.as_str().map(str::to_string)))
            .collect()
    })
}

fn get_list<T>(map: &Map<String, Value>, key: &str, item: impl Fn(&Value) -> T) -> Option<Vec<T>> {
    map.get(key)
        .and_then(Value::as_array)
        .map(|items| items.iter().map(item).collect())
}

fn list_value<T>(items: &Option<Vec<Option<T>>>, item: impl Fn(&T) -> Value) -> Value {
    match items {
        None => Value::Null,
        Some(items) => Value::Array(
            items
                .iter()
                .map(|x| x.as_ref().map_or(Value::Null, &item))
                .collect(),
        ),
    }
}

/// C# `FieldMap` — 매핑의 칸 하나.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldMap {
    pub source: Option<String>,
    pub target: Option<String>,
    pub transform: Option<String>,
    pub codes: Option<Pairs>,
    pub secondary_source: Option<String>,
}

impl FieldMap {
    pub fn source(&self) -> &str {
        self.source.as_deref().unwrap_or("")
    }

    pub fn target(&self) -> &str {
        self.target.as_deref().unwrap_or("")
    }

    pub fn to_value(&self) -> Value {
        object([
            ("source", text_value(&self.source)),
            ("target", text_value(&self.target)),
            ("transform", text_value(&self.transform)),
            ("codes", optional_pairs_value(&self.codes)),
            ("secondarySource", text_value(&self.secondary_source)),
        ])
    }

    fn from_normalized(value: &Value) -> Option<FieldMap> {
        let map = value.as_object()?;
        Some(FieldMap {
            source: get_text(map, "source"),
            target: get_text(map, "target"),
            transform: get_text(map, "transform"),
            codes: get_pairs(map, "codes"),
            secondary_source: get_text(map, "secondarySource"),
        })
    }
}

/// C# `TableMap` — 원천 표 하나를 업무 표 하나로. 키가 비어 있으면 줄 차례가 곧 행이다.
#[derive(Clone, Debug, PartialEq)]
pub struct TableMap {
    pub source: Option<String>,
    pub target: Option<String>,
    pub keys: Option<Vec<Option<String>>>,
    pub fields: Option<Vec<Option<FieldMap>>>,
    pub required: bool,
    pub total_selector: Option<String>,
}

impl TableMap {
    pub fn source(&self) -> &str {
        self.source.as_deref().unwrap_or("")
    }

    pub fn target(&self) -> &str {
        self.target.as_deref().unwrap_or("")
    }

    /// 칸들(검증을 지난 매핑에는 빈 자리가 없다).
    pub fn fields_iter(&self) -> impl Iterator<Item = &FieldMap> {
        self.fields.iter().flatten().flatten()
    }

    /// 키들(검증을 지난 매핑에는 빈 자리가 없다).
    pub fn keys_iter(&self) -> impl Iterator<Item = &str> {
        self.keys
            .iter()
            .flatten()
            .map(|k| k.as_deref().unwrap_or(""))
    }

    fn key_count(&self) -> usize {
        self.keys.as_ref().map_or(0, Vec::len)
    }

    pub fn to_value(&self) -> Value {
        object([
            ("source", text_value(&self.source)),
            ("target", text_value(&self.target)),
            (
                "keys",
                match &self.keys {
                    None => Value::Null,
                    Some(keys) => Value::Array(keys.iter().map(text_value).collect()),
                },
            ),
            ("fields", list_value(&self.fields, FieldMap::to_value)),
            ("required", Value::Bool(self.required)),
            ("totalSelector", text_value(&self.total_selector)),
        ])
    }

    fn from_normalized(value: &Value) -> Option<TableMap> {
        let map = value.as_object()?;
        Some(TableMap {
            source: get_text(map, "source"),
            target: get_text(map, "target"),
            keys: get_list(map, "keys", |v| v.as_str().map(str::to_string)),
            fields: get_list(map, "fields", FieldMap::from_normalized),
            required: map
                .get("required")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            total_selector: get_text(map, "totalSelector"),
        })
    }
}

/// C# `MappingScreen` — 수집하는 화면의 메뉴 번호(숫자 다섯)와 사람이 읽을 이름.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MappingScreen {
    pub code: Option<String>,
    pub name: Option<String>,
}

impl MappingScreen {
    pub fn code(&self) -> &str {
        self.code.as_deref().unwrap_or("")
    }

    pub fn name(&self) -> &str {
        self.name.as_deref().unwrap_or("")
    }

    pub fn to_value(&self) -> Value {
        object([
            ("code", text_value(&self.code)),
            ("name", text_value(&self.name)),
        ])
    }
}

/// C# `MappingProfile` — 화면 하나를 읽는 규칙. `screen` 이 있는 프로필만이 수집하는 화면이다(ADR-037).
#[derive(Clone, Debug, PartialEq)]
pub struct MappingProfile {
    pub id: Option<String>,
    pub entity_type: Option<String>,
    pub identity_source: Option<String>,
    pub base_field: Option<String>,
    pub seq_field: Option<String>,
    pub fields: Option<Vec<Option<FieldMap>>>,
    pub tables: Option<Vec<Option<TableMap>>>,
    pub heading: Option<String>,
    pub selectors: Option<Pairs>,
    pub screen: Option<MappingScreen>,
}

impl MappingProfile {
    pub fn id(&self) -> &str {
        self.id.as_deref().unwrap_or("")
    }

    pub fn entity_type(&self) -> &str {
        self.entity_type.as_deref().unwrap_or("")
    }

    pub fn identity_source(&self) -> &str {
        self.identity_source.as_deref().unwrap_or("")
    }

    pub fn base_field(&self) -> &str {
        self.base_field.as_deref().unwrap_or("")
    }

    pub fn seq_field(&self) -> &str {
        self.seq_field.as_deref().unwrap_or("")
    }

    /// 머리 칸들(검증을 지난 매핑에는 빈 자리가 없다).
    pub fn fields_iter(&self) -> impl Iterator<Item = &FieldMap> {
        self.fields.iter().flatten().flatten()
    }

    /// 딸린 표들(검증을 지난 매핑에는 빈 자리가 없다).
    pub fn tables_iter(&self) -> impl Iterator<Item = &TableMap> {
        self.tables.iter().flatten().flatten()
    }

    pub fn to_value(&self) -> Value {
        object([
            ("id", text_value(&self.id)),
            ("entityType", text_value(&self.entity_type)),
            ("identitySource", text_value(&self.identity_source)),
            ("baseField", text_value(&self.base_field)),
            ("seqField", text_value(&self.seq_field)),
            ("fields", list_value(&self.fields, FieldMap::to_value)),
            ("tables", list_value(&self.tables, TableMap::to_value)),
            ("heading", text_value(&self.heading)),
            ("selectors", optional_pairs_value(&self.selectors)),
            (
                "screen",
                self.screen
                    .as_ref()
                    .map_or(Value::Null, MappingScreen::to_value),
            ),
        ])
    }

    fn from_normalized(value: &Value) -> Option<MappingProfile> {
        let map = value.as_object()?;
        Some(MappingProfile {
            id: get_text(map, "id"),
            entity_type: get_text(map, "entityType"),
            identity_source: get_text(map, "identitySource"),
            base_field: get_text(map, "baseField"),
            seq_field: get_text(map, "seqField"),
            fields: get_list(map, "fields", FieldMap::from_normalized),
            tables: get_list(map, "tables", TableMap::from_normalized),
            heading: get_text(map, "heading"),
            selectors: get_pairs(map, "selectors"),
            screen: map
                .get("screen")
                .and_then(Value::as_object)
                .map(|s| MappingScreen {
                    code: get_text(s, "code"),
                    name: get_text(s, "name"),
                }),
        })
    }
}

/// C# `MappingSet(Version, Profiles)`.
#[derive(Clone, Debug, PartialEq)]
pub struct MappingSet {
    pub version: i32,
    pub profiles: Option<Vec<Option<MappingProfile>>>,
}

impl MappingSet {
    /// 프로필들(검증을 지난 매핑에는 빈 자리가 없다).
    pub fn profiles_iter(&self) -> impl Iterator<Item = &MappingProfile> {
        self.profiles.iter().flatten().flatten()
    }

    /// `Profiles.SingleOrDefault(p => p.Id == id)` — 검증을 지난 매핑의 id 는 겹치지 않는다.
    pub fn profile(&self, id: &str) -> Option<&MappingProfile> {
        self.profiles_iter().find(|p| p.id.as_deref() == Some(id))
    }

    /// `Mapping.Json` 으로 쓴 꼴(JSON 값). [`hash`] 와 저장(`erp_mapping.json`)이 이것을 쓴다.
    pub fn to_value(&self) -> Value {
        object([
            ("version", self.version.into()),
            (
                "profiles",
                list_value(&self.profiles, MappingProfile::to_value),
            ),
        ])
    }

    fn from_normalized(value: &Value) -> MappingSet {
        let map = value.as_object();
        MappingSet {
            version: map
                .and_then(|m| m.get("version"))
                .and_then(|v| v.to_string().parse().ok())
                .unwrap_or(0),
            profiles: map.and_then(|m| get_list(m, "profiles", MappingProfile::from_normalized)),
        }
    }
}

/// C# `MappedRow` — 업무 표의 행 하나. 머리는 `table` 이 종류 이름이고 `source_key` 가 빈 글이다.
#[derive(Clone, Debug, PartialEq)]
pub struct MappedRow {
    pub table: String,
    pub source_key: String,
    pub values: Pairs,
    pub origins: Option<Pairs>,
}

impl MappedRow {
    /// `Values.GetValueOrDefault(key)`.
    pub fn value(&self, key: &str) -> Option<&str> {
        self.values
            .iter()
            .find(|(k, _)| k == key)
            .and_then(|(_, v)| v.as_deref())
    }

    pub fn to_value(&self) -> Value {
        object([
            ("table", Value::String(self.table.clone())),
            ("sourceKey", Value::String(self.source_key.clone())),
            ("values", pairs_value(&self.values)),
            ("origins", optional_pairs_value(&self.origins)),
        ])
    }
}

/// `dictionary[key] = value` — 있으면 그 자리에서 바꾸고, 없으면 끝에 더한다.
pub(crate) fn set_pair(pairs: &mut Pairs, key: &str, value: Option<String>) {
    match pairs.iter_mut().find(|(k, _)| k == key) {
        Some(slot) => slot.1 = value,
        None => pairs.push((key.to_string(), value)),
    }
}

/// C# `MappedDocument` — 원천 문서 하나를 옮긴 것.
#[derive(Clone, Debug, PartialEq)]
pub struct MappedDocument {
    pub profile: String,
    pub entity_type: String,
    pub base: String,
    pub seq: String,
    pub rows: Vec<MappedRow>,
    pub scope: String,
}

impl MappedDocument {
    pub fn to_value(&self) -> Value {
        object([
            ("profile", Value::String(self.profile.clone())),
            ("entityType", Value::String(self.entity_type.clone())),
            ("base", Value::String(self.base.clone())),
            ("seq", Value::String(self.seq.clone())),
            (
                "rows",
                Value::Array(self.rows.iter().map(MappedRow::to_value).collect()),
            ),
            ("scope", Value::String(self.scope.clone())),
        ])
    }
}

/// `JsonSerializer.Deserialize<MappedDocument>(json, Mapping.Json)!.Rows` — 저장된 원천 문서(`erp_source`)의 행들.
pub(crate) fn stored_rows(json_text: &str) -> Result<Vec<MappedRow>> {
    let unreadable = || Error::invalid("저장된 원천 문서를 읽지 못했습니다.");
    let value = json::parse(json_text).map_err(|_| unreadable())?;
    let normalized = normalize(&value, &MAPPED_DOCUMENT).map_err(|_| unreadable())?;
    let rows = normalized
        .get("rows")
        .and_then(Value::as_array)
        .ok_or_else(unreadable)?;
    rows.iter()
        .map(|row| {
            let map = row.as_object().ok_or_else(unreadable)?;
            Ok(MappedRow {
                table: get_text(map, "table").ok_or_else(unreadable)?,
                source_key: get_text(map, "sourceKey").ok_or_else(unreadable)?,
                values: get_pairs(map, "values").unwrap_or_default(),
                origins: get_pairs(map, "origins"),
            })
        })
        .collect()
}

// ── 기본 매핑과 판 ─────────────────────────────────────────

/// 실려 나가는 파일 — 이웃한 `mapping.json` 을 읽어 넣는다(C# 의 내장 자원 `Pclm.Core.Erp.mapping.json` 이던 파일).
const DEFAULT_MAPPING: &str = include_str!("mapping.json");

fn defaults_ref() -> &'static MappingSet {
    static DEFAULTS: OnceLock<MappingSet> = OnceLock::new();
    DEFAULTS.get_or_init(|| MappingSet::from_normalized(&defaults_value()))
}

/// `Mapping.Defaults()` 를 `Mapping.Json` 으로 쓴 꼴(JSON 값).
pub(crate) fn defaults_value() -> Value {
    // 실려 나가는 파일이라 읽지 못하면 C# 도 시작부터 실패한다. 시험(`default_revision_matches_csharp`)이 지킨다.
    json::parse(DEFAULT_MAPPING)
        .and_then(|v| normalize(&v, &MAPPING_SET))
        .unwrap_or(Value::Null)
}

/// `Mapping.Defaults()` — 기본 매핑.
pub fn defaults() -> MappingSet {
    defaults_ref().clone()
}

/// `Mapping.Hash` — `Mapping.Json` 꼴 글의 SHA-256, 대문자 16진. 들여 쓴 글의 줄바꿈은 C# `Mapping.HashText` 처럼
/// 운영체제와 상관없이 `\r\n` 이다([`json::to_indented`]) — 파일에 적힌 판이 어디서 떠도 같아야 한다.
pub fn hash(normalized: &Value) -> String {
    let text = json::to_indented(normalized, Encoder::Default);
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect()
}

/// `Mapping.Hash(mapping)`.
pub fn revision(set: &MappingSet) -> String {
    hash(&set.to_value())
}

// ── 읽기 ─────────────────────────────────────────────

const UNREADABLE: &str = "매핑 JSON 형식이 올바르지 않습니다.";

/// `Mapping.Parse` — 사람이 지금 편집해 저장하는 매핑. 기본이 거둔 필드는 거절한다.
pub fn parse(json_text: &str) -> Result<MappingSet> {
    let value = normalize_text(json_text)?;
    validate(&value)?;
    Ok(value)
}

/// `Mapping.Read` — 작업자료에 저장한 매핑(`erp_mapping`). [`parse`] 에 더해 기본 매핑이 거둔 필드를 조용히 떨구고
/// (ADR-043), 기본에 새로 더해진 대상이 편집본에 없으면 기본의 것으로 채운다.
pub fn read(json_text: &str) -> Result<MappingSet> {
    let value = without_retired_fields(with_missing_fields(normalize_text(json_text)?));
    validate(&value)?;
    Ok(value)
}

/// `Mapping.Normalize` — 읽지 못하는 글은 .NET 의 영어 예외 대신 한국어 사유로 거절한다. 같은 키가 두 번 든 글도 읽을
/// 수 없는 글이다(뒤 값을 조용히 고르지 않는다).
fn normalize_text(json_text: &str) -> Result<MappingSet> {
    // 문법 검사를 겸한다 — 깨진 글·빈 글·너무 깊은 글은 여기서 거절된다.
    let value = json::parse(json_text).map_err(|_| Error::invalid(UNREADABLE))?;
    if json::has_duplicate_keys(json_text) {
        return Err(Error::invalid(UNREADABLE));
    }
    let normalized = normalize_at(&value, &MAPPING_SET, "$").map_err(reason)?;
    if normalized.is_null() {
        return Err(Error::invalid("매핑이 비어 있습니다."));
    }
    Ok(with_screens(with_new_profiles(
        MappingSet::from_normalized(&normalized),
    )))
}

/// `Mapping.Reason` — 문법은 맞는데 꼴이 틀린 글의 사유. 모르는 칸은 그 이름이 경로의 끝이다. 뿌리 자체가 틀리면
/// 경로가 `$` 뿐이라 형식 사유로 낸다.
fn reason(e: ShapeError) -> Error {
    let path = match e.path.strip_prefix("$.") {
        Some(p) if !p.is_empty() => p,
        _ => return Error::invalid(UNREADABLE),
    };
    if e.unknown {
        let name = path.rfind('.').map_or(path, |at| &path[at + 1..]);
        return Error::invalid(format!("매핑에 모르는 칸이 있습니다: {name}"));
    }
    Error::invalid(format!("매핑의 ‘{path}’ 값 형식이 올바르지 않습니다."))
}

/// `Mapping.WithMissingFields` — 기본 매핑에 대상이 더해지기 전에 저장한 편집본에 빠진 필드·표를 기본의 것 그대로
/// 채운다(ADR-043 결정 2). 이미 있는 대상은 건드리지 않는다. 표는 기본의 자리 차례대로 끼운다.
fn with_missing_fields(mut set: MappingSet) -> MappingSet {
    let Some(profiles) = set.profiles.as_mut() else {
        return set;
    };
    let defaults = defaults_ref();
    fn fill(fields: &mut Vec<Option<FieldMap>>, allowed: &[Option<FieldMap>]) {
        let missing: Vec<Option<FieldMap>> = allowed
            .iter()
            .flatten()
            .filter(|a| {
                !fields
                    .iter()
                    .any(|f| f.as_ref().and_then(|f| f.target.as_ref()) == a.target.as_ref())
            })
            .cloned()
            .map(Some)
            .collect();
        fields.extend(missing);
    }
    for p in profiles.iter_mut().flatten() {
        if p.fields.is_none() || p.tables.is_none() {
            continue;
        }
        let Some(original) = single(defaults.profiles_iter(), |d| d.id == p.id) else {
            continue;
        };
        let tables = p.tables.as_mut().unwrap_or_else(|| unreachable!());
        for t in tables.iter_mut().flatten() {
            let Some(fields) = t.fields.as_mut() else {
                continue;
            };
            if let Some(allowed) = single(original.tables_iter(), |x| x.target == t.target) {
                fill(fields, allowed.fields.as_deref().unwrap_or_default());
            }
        }
        for (i, table) in original.tables.iter().flatten().enumerate() {
            let target = table.as_ref().and_then(|t| t.target.as_ref());
            if !tables
                .iter()
                .any(|t| t.as_ref().and_then(|t| t.target.as_ref()) == target)
            {
                let at = i.min(tables.len());
                tables.insert(at, table.clone());
            }
        }
        if let Some(fields) = p.fields.as_mut() {
            fill(fields, original.fields.as_deref().unwrap_or_default());
        }
    }
    set
}

/// `Mapping.WithoutRetiredFields` — 기본의 같은 프로필(같은 표)에 대상이 없는 필드를 뺀다. 그 필드만 걸던 선택자도
/// 함께 떨군다. 모르는 프로필·표와 깨진 항목(null)은 그대로 둔다 — 거절은 [`validate`] 의 몫이다.
fn without_retired_fields(mut set: MappingSet) -> MappingSet {
    let Some(profiles) = set.profiles.as_mut() else {
        return set;
    };
    let defaults = defaults_ref();
    fn keep(
        fields: &mut Vec<Option<FieldMap>>,
        allowed: &[Option<FieldMap>],
        retired: &mut Vec<FieldMap>,
    ) {
        let gone = |f: &FieldMap| {
            f.target.is_some() && !allowed.iter().flatten().any(|a| a.target == f.target)
        };
        retired.extend(fields.iter().flatten().filter(|f| gone(f)).cloned());
        fields.retain(|f| !f.as_ref().is_some_and(gone));
    }
    for p in profiles.iter_mut().flatten() {
        if p.fields.is_none() || p.tables.is_none() {
            continue;
        }
        let Some(original) = single(defaults.profiles_iter(), |d| d.id == p.id) else {
            continue;
        };
        let mut retired = Vec::new();
        if let Some(fields) = p.fields.as_mut() {
            keep(
                fields,
                original.fields.as_deref().unwrap_or_default(),
                &mut retired,
            );
        }
        for t in p.tables.iter_mut().flatten().flatten() {
            let Some(fields) = t.fields.as_mut() else {
                continue;
            };
            if let Some(allowed) = single(original.tables_iter(), |x| x.target == t.target) {
                keep(
                    fields,
                    allowed.fields.as_deref().unwrap_or_default(),
                    &mut retired,
                );
            }
        }
        if retired.is_empty() {
            continue;
        }
        let mut used: Vec<Option<String>> = p
            .fields_iter()
            .flat_map(|f| [f.source.clone(), f.secondary_source.clone()])
            .collect();
        used.push(p.base_field.clone());
        used.push(p.seq_field.clone());
        let orphaned: Vec<String> = retired
            .iter()
            .flat_map(|f| [f.source.clone(), f.secondary_source.clone()])
            .flatten()
            .filter(|s| !used.contains(&Some(s.clone())))
            .collect();
        if let Some(selectors) = p.selectors.as_mut() {
            selectors.retain(|(k, _)| !orphaned.contains(k));
        }
    }
    set
}

/// `Mapping.WithNewProfiles` — 기본 매핑에 프로필이 더해지기 전에 저장한 매핑에 빠진 프로필을 기본의 것 그대로 채운다.
/// 자리는 기본 매핑의 순서를 따른다.
fn with_new_profiles(mut set: MappingSet) -> MappingSet {
    let Some(profiles) = set.profiles.as_mut() else {
        return set;
    };
    let defaults = defaults_ref();
    let ids: Vec<Option<String>> = profiles.iter().flatten().map(|p| p.id.clone()).collect();
    let all: Vec<&Option<MappingProfile>> = defaults.profiles.iter().flatten().collect();
    if all
        .iter()
        .all(|d| ids.contains(&d.as_ref().and_then(|d| d.id.clone())))
    {
        return set;
    }
    for (i, d) in all.into_iter().enumerate() {
        if !ids.contains(&d.as_ref().and_then(|d| d.id.clone())) {
            let at = i.min(profiles.len());
            profiles.insert(at, d.clone());
        }
    }
    set
}

/// `Mapping.WithScreens` — 화면을 적기 전에 저장한 매핑(`screen` 이 없다)에 기본 매핑의 화면을 프로필 id 로 채운다.
fn with_screens(mut set: MappingSet) -> MappingSet {
    let Some(profiles) = set.profiles.as_mut() else {
        return set;
    };
    if profiles.iter().flatten().all(|p| p.screen.is_some()) {
        return set;
    }
    let defaults = defaults_ref();
    for p in profiles.iter_mut().flatten() {
        if p.screen.is_none() {
            p.screen =
                single(defaults.profiles_iter(), |d| d.id == p.id).and_then(|d| d.screen.clone());
        }
    }
    set
}

/// `SingleOrDefault(predicate)` — 맞는 것이 둘 이상이면 C# 은 던지지만 기본 매핑의 id·대상은 겹치지 않는다.
fn single<'a, T>(
    items: impl Iterator<Item = &'a T>,
    predicate: impl Fn(&T) -> bool,
) -> Option<&'a T>
where
    T: 'a,
{
    items.into_iter().find(|x| predicate(x))
}

// ── 검증 ─────────────────────────────────────────────

const TRANSFORMS: [&str; 7] = [
    "text", "decimal", "integer", "date", "datetime", "boolean", "code",
];

/// `account(?!ing)|actno|prsnno|password|atflpath|sess|ipar` (대소문자 무시). 검사하는 글은 이미 ASCII 경로꼴이다.
fn sensitive(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    let account = lower
        .match_indices("account")
        .any(|(at, _)| !lower[at + 7..].starts_with("ing"));
    account
        || ["actno", "prsnno", "password", "atflpath", "sess", "ipar"]
            .iter()
            .any(|w| lower.contains(w))
}

/// `\A[a-zA-Z0-9_*]+(?:\.[a-zA-Z0-9_]+)*\z`.
fn path_shape(path: &str) -> bool {
    let mut parts = path.split('.');
    let first = parts.next().unwrap_or("");
    let word = |s: &str, star: bool| {
        !s.is_empty()
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || (star && b == b'*'))
    };
    word(first, true) && parts.all(|p| word(p, false))
}

fn check_path(path: Option<&str>) -> Result<()> {
    match path {
        Some(p) if !is_blank(p) && !sensitive(p) && utf16_len(p) <= 250 && path_shape(p) => Ok(()),
        _ => Err(Error::invalid(
            "경로는 필드 ID와 점, 표 접미사 와일드카드만 지원합니다.",
        )),
    }
}

/// `Mapping.Validate` — 기본 매핑의 목적지 목록이 수집 허용 목록이다. 임의 DB 열·민감 필드를 더할 수 없다.
pub fn validate(set: &MappingSet) -> Result<()> {
    let defaults = defaults_ref();
    let default_count = defaults.profiles.as_ref().map_or(0, Vec::len);
    let config = || Error::invalid("지원하는 매핑 판·프로필 구성을 확인하세요.");
    let Some(profiles) = set.profiles.as_ref() else {
        return Err(config());
    };
    if set.version != 1
        || profiles.iter().any(|p| match p {
            None => true,
            Some(p) => {
                p.fields.is_none()
                    || p.tables.is_none()
                    || p.base_field.is_none()
                    || p.seq_field.is_none()
                    || p.identity_source.is_none()
            }
        })
        || profiles.len() != default_count
    {
        return Err(config());
    }
    let mut ids: Vec<&Option<String>> = Vec::new();
    for p in profiles.iter().flatten() {
        if !ids.contains(&&p.id) {
            ids.push(&p.id);
        }
    }
    if ids.len() != profiles.len() {
        return Err(config());
    }
    for p in profiles.iter().flatten() {
        let original = single(defaults.profiles_iter(), |x| x.id == p.id);
        let tables = p.tables.as_deref().unwrap_or_default();
        let original = match original {
            Some(o) if o.entity_type == p.entity_type && tables.len() <= 12 => o,
            _ => return Err(Error::invalid("자료 종류나 프로필 ID는 바꿀 수 없습니다.")),
        };
        // 받는 화면과 그 화면의 고유키는 한 자리(기본 매핑)에만 있다(ADR-037).
        if p.screen != original.screen
            || p.identity_source != original.identity_source
            || p.base_field != original.base_field
            || p.seq_field != original.seq_field
        {
            return Err(Error::invalid("화면 번호와 고유키는 바꿀 수 없습니다."));
        }
        check_path(p.identity_source.as_deref())?;
        check_path(p.base_field.as_deref())?;
        check_path(p.seq_field.as_deref())?;
        let fields = p.fields.as_deref().unwrap_or_default();
        check_fields(fields, original.fields.as_deref().unwrap_or_default())?;
        if !p
            .fields_iter()
            .any(|f| f.target.as_deref() == Some("title"))
        {
            return Err(Error::invalid("문서 제목 매핑이 필요합니다."));
        }
        let mut targets: Vec<Option<&String>> = Vec::new();
        for t in tables {
            let target = t.as_ref().and_then(|t| t.target.as_ref());
            if !targets.contains(&target) {
                targets.push(target);
            }
        }
        if targets.len() != tables.len() {
            return Err(Error::invalid("동일 목적지 표를 중복 수집할 수 없습니다."));
        }
        for table in tables {
            let table = match table {
                Some(t) if t.keys.is_some() && t.fields.is_some() && t.source.is_some() => t,
                _ => return Err(Error::invalid("표 매핑을 확인하세요.")),
            };
            let allowed = single(original.tables_iter(), |t| t.target == table.target)
                .ok_or_else(|| Error::invalid("지원하지 않는 대상 표입니다."))?;
            check_path(table.source.as_deref())?;
            // 키를 비우는 것(줄 차례로 행을 가리기)은 기본 매핑이 그렇게 둔 표에서만 된다.
            if table.key_count() > 5 || (table.key_count() == 0 && allowed.key_count() != 0) {
                return Err(Error::invalid("원천 행 복합키가 필요합니다."));
            }
            for key in table.keys.iter().flatten() {
                check_path(key.as_deref())?;
            }
            check_fields(
                table.fields.as_deref().unwrap_or_default(),
                allowed.fields.as_deref().unwrap_or_default(),
            )?;
            if table
                .total_selector
                .as_deref()
                .is_some_and(|s| utf16_len(s) > 500)
            {
                return Err(Error::invalid("전체 건수 선택자가 너무 깁니다."));
            }
        }
        let item_table = format!("{}_item", p.entity_type());
        if p.id.as_deref() != Some("g2b-public-notice-header-v1")
            && !p
                .tables_iter()
                .any(|t| t.target.as_deref() == Some(item_table.as_str()) && t.required)
        {
            return Err(Error::invalid("품목 전체 범위 검증은 필수입니다."));
        }
        if let Some(selectors) = &p.selectors {
            let bad = selectors.iter().any(|(key, value)| {
                (!p.fields_iter().any(|f| {
                    f.source.as_deref() == Some(key.as_str())
                        || f.secondary_source.as_deref() == Some(key.as_str())
                }) && p.base_field.as_deref() != Some(key.as_str())
                    && p.seq_field.as_deref() != Some(key.as_str()))
                    || value.as_deref().is_none_or(|v| utf16_len(v) > 500)
            });
            if bad {
                return Err(Error::invalid("허용 필드의 선택자만 지정할 수 있습니다."));
            }
        }
    }
    Ok(())
}

fn check_fields(fields: &[Option<FieldMap>], allowed: &[Option<FieldMap>]) -> Result<()> {
    if fields.iter().any(|f| {
        f.as_ref()
            .is_none_or(|f| f.source.is_none() || f.target.is_none())
    }) {
        return Err(Error::invalid("필드 매핑을 확인하세요."));
    }
    let mut targets: Vec<&Option<String>> = Vec::new();
    for f in fields.iter().flatten() {
        if !targets.contains(&&f.target) {
            targets.push(&f.target);
        }
    }
    if targets.len() != fields.len() {
        return Err(Error::invalid(
            "한 대상 필드에 두 출처를 지정할 수 없습니다.",
        ));
    }
    for f in fields.iter().flatten() {
        check_path(f.source.as_deref())?;
        if let Some(secondary) = &f.secondary_source {
            check_path(Some(secondary))?;
        }
        if !allowed.iter().flatten().any(|a| a.target == f.target)
            || !f
                .transform
                .as_deref()
                .is_some_and(|t| TRANSFORMS.contains(&t))
        {
            return Err(Error::invalid(format!(
                "허용되지 않은 대상·변환입니다: {}",
                f.target()
            )));
        }
        if sensitive(f.source()) {
            return Err(Error::invalid("민감·내부 필드는 매핑할 수 없습니다."));
        }
    }
    Ok(())
}

// ── 원천 값 ─────────────────────────────────────────────

/// `node.Deserialize<FieldMap>(Mapping.Json)`. JSON `null` 이면 `None`. 판올림 보정이 당시 매핑의 칸을 읽는다.
pub(crate) fn field_map(node: &Value) -> JsonResult<Option<FieldMap>> {
    Ok(FieldMap::from_normalized(&normalize(node, &FIELD_MAP)?))
}

/// `Mapping.At` 이 던지는 오류(C# `InvalidOperationException`) — 여러 분류 표에서 문서 전체 값을 꺼내려 했다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AtError;

const AT_ERROR: &str = "문서 전체 금액은 단일 분류 표에서만 가져올 수 있습니다.";

impl From<AtError> for Error {
    fn from(_: AtError) -> Self {
        Error::invalid(AT_ERROR)
    }
}

/// `Mapping.At` — 점으로 이은 경로를 따라 내려간다. 표(배열)의 `0` 은 원소가 꼭 하나일 때만 읽는다.
/// 없는 자리는 `None`, JSON `null` 인 자리는 `Some(Null)` 이다(C# `TryGetProperty` 와 같다).
pub(crate) fn at<'a>(
    mut value: &'a Value,
    path: &str,
) -> std::result::Result<Option<&'a Value>, AtError> {
    for part in path.split('.') {
        match value {
            Value::Array(items) if part == "0" => {
                // 빈 표는 그 값이 없는 문서다 — 걷지 않은 값으로 둔다. 막는 것은 분류가 여럿일 때뿐이다.
                match items.as_slice() {
                    [] => return Ok(None),
                    [only] => value = only,
                    _ => return Err(AtError),
                }
            }
            Value::Object(map) => match map.get(part) {
                Some(child) => value = child,
                None => return Ok(None),
            },
            _ => return Ok(None),
        }
    }
    Ok(Some(value))
}

/// `Mapping.Text` — 글은 앞뒤 공백을 걷고, 그 밖의 값은 `JsonElement.ToString()` 꼴(수는 원문, 참거짓은
/// `True`/`False`, 객체·배열은 기본 인코더로 쓴 JSON)이다.
pub(crate) fn text(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::Null => None,
        Value::String(s) => Some(s.trim().to_string()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(true) => Some("True".into()),
        Value::Bool(false) => Some("False".into()),
        other => Some(json::to_compact(other, Encoder::Default)),
    }
}

/// `Mapping.ConvertValue` 의 오류 — `필드 형식 오류: 원천 → 대상 (변환)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConvertError(pub String);

impl From<ConvertError> for Error {
    fn from(e: ConvertError) -> Self {
        Error::InvalidOperation(e.0)
    }
}

/// `Mapping.ConvertValue` — 원천 글을 매핑의 변환으로 바꾼다. 빈 값(과 날짜의 `-`)은 `None`.
pub fn convert_value(value: Option<&str>, f: &FieldMap) -> Result<Option<String>> {
    Ok(convert(value, f)?)
}

pub(crate) fn convert(
    value: Option<&str>,
    f: &FieldMap,
) -> std::result::Result<Option<String>, ConvertError> {
    let transform = f.transform.as_deref();
    let Some(value) = value else { return Ok(None) };
    if is_blank(value) || (value == "-" && matches!(transform, Some("date" | "datetime"))) {
        return Ok(None);
    }
    let fail = || {
        ConvertError(format!(
            "필드 형식 오류: {} → {} ({})",
            f.source.as_deref().unwrap_or(""),
            f.target.as_deref().unwrap_or(""),
            transform.unwrap_or("")
        ))
    };
    let converted = match transform {
        Some("text") => Some(value.to_string()),
        Some("code") => Some(
            f.codes
                .as_ref()
                .and_then(|codes| codes.iter().find(|(k, _)| k == value))
                .and_then(|(_, v)| v.clone())
                .unwrap_or_else(|| format!("{value} (명칭 미확정)")),
        ),
        Some("decimal") => Some(parse_decimal(&value.replace(',', "")).ok_or_else(fail)?),
        Some("integer") => Some(parse_int(&value.replace(',', "")).ok_or_else(fail)?),
        Some("boolean") => Some(
            match value {
                "Y" | "True" | "true" | "1" | "허용" | "가능" | "예" => "1",
                "N" | "False" | "false" | "0" | "불허" | "불가" | "아니오" => "0",
                _ => return Err(fail()),
            }
            .to_string(),
        ),
        Some(t @ ("date" | "datetime")) => Some(parse_date(value, t == "date").ok_or_else(fail)?),
        _ => return Err(fail()),
    };
    Ok(converted)
}

/// `decimal.Parse(s, AllowLeadingSign | AllowDecimalPoint, Invariant).ToString(Invariant)`.
///
/// 소수부의 0 은 남는다(`1.50` → `1.50`), 정수부 앞의 0 은 걷힌다(`007` → `7`), 음의 0 은 부호가 없다(실측).
/// 소수 자리는 28 까지이고 넘는 자리는 짝수 쪽으로 반올림한다. 29 자리 최댓값을 넘으면 실패(OverflowException).
fn parse_decimal(s: &str) -> Option<String> {
    let (negative, body) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    let (int_part, frac_part) = body.split_once('.').unwrap_or((body, ""));
    if !int_part.bytes().all(|b| b.is_ascii_digit())
        || !frac_part.bytes().all(|b| b.is_ascii_digit())
        || int_part.len() + frac_part.len() == 0
    {
        return None;
    }
    let int_part = int_part.trim_start_matches('0');
    const MAX: u128 = 79_228_162_514_264_337_593_543_950_335;
    // 정수부만으로 넘치면 실패다.
    let whole: u128 = if int_part.is_empty() {
        0
    } else {
        int_part.parse().ok()?
    };
    if int_part.len() > 29 || whole > MAX {
        return None;
    }
    // 소수 자리를 담을 수 있는 만큼(28 자리, 96비트)으로 줄이며 반올림한다.
    let mut scale = frac_part.len().min(28);
    loop {
        let digits = format!("{int_part}{}", &frac_part[..scale]);
        let kept: u128 = if digits.is_empty() {
            0
        } else {
            digits.parse().ok()?
        };
        let rest = &frac_part[scale..];
        let round_up = match rest.as_bytes().first() {
            Some(b'5'..=b'9') if rest.as_bytes()[0] > b'5' => true,
            Some(b'5') => rest[1..].bytes().any(|b| b != b'0') || kept % 2 == 1,
            _ => false,
        };
        let mantissa = kept + u128::from(round_up);
        if mantissa <= MAX {
            let text = format!("{mantissa:0>width$}", width = scale + 1);
            let (i, f) = text.split_at(text.len() - scale);
            let zero = mantissa == 0;
            let mut out = String::new();
            if negative && !zero {
                out.push('-');
            }
            out.push_str(i);
            if scale > 0 {
                out.push('.');
                out.push_str(f);
            }
            return Some(out);
        }
        if scale == 0 {
            return None;
        }
        scale -= 1;
    }
}

/// `int.Parse(s, Invariant)` — `NumberStyles.Integer`: 앞뒤 공백(`\t\n\v\f\r` 와 빈칸)과 앞 부호를 받는다.
fn parse_int(s: &str) -> Option<String> {
    let ws = |c: char| matches!(c, '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ');
    let t = s.trim_matches(ws);
    let body = t.strip_prefix(['-', '+']).unwrap_or(t);
    if body.is_empty() || !body.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    t.parse::<i32>().ok().map(|n| n.to_string())
}

/// `DateTime.ParseExact(s, 형식들, Invariant, None)` 을 `yyyy-MM-dd`(날짜) 또는 `yyyy-MM-dd HH:mm:ss` 로.
///
/// 형식은 C# 의 목록과 같은 차례다. 두 자리 칸은 꼭 두 자리(`2026-1-5` 는 실패), `FFFFFFF` 는 0~7 자리(점 뒤가
/// 비어도 된다), 공백은 받지 않는다(실측).
fn parse_date(s: &str, date_only: bool) -> Option<String> {
    const FORMATS: [&str; 11] = [
        "yyyy-MM-ddTHH:mm:ss",
        "yyyy-MM-ddTHH:mm:ss.FFFFFFF",
        "yyyyMMdd",
        "yyyyMMddHHmmss",
        "yyyyMMddHHmm",
        "yyyy-MM-dd",
        "yyyy/MM/dd",
        "yyyy-MM-dd HH:mm:ss",
        "yyyy/MM/dd HH:mm:ss",
        "yyyy-MM-dd HH:mm",
        "yyyy/MM/dd HH:mm",
    ];
    let (y, mo, d, h, mi, se) = FORMATS.iter().find_map(|f| parse_exact(s, f))?;
    Some(if date_only {
        format!("{y:04}-{mo:02}-{d:02}")
    } else {
        format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{se:02}")
    })
}

fn parse_exact(s: &str, format: &str) -> Option<(u32, u32, u32, u32, u32, u32)> {
    let input = s.as_bytes();
    let mut pos = 0;
    let mut parts = [0u32, 1, 1, 0, 0, 0];
    let mut f = format;
    let digits = |pos: &mut usize, n: usize| -> Option<u32> {
        let slice = input.get(*pos..*pos + n)?;
        if !slice.iter().all(u8::is_ascii_digit) {
            return None;
        }
        *pos += n;
        std::str::from_utf8(slice).ok()?.parse().ok()
    };
    while !f.is_empty() {
        let (token, slot, width) = if let Some(rest) = f.strip_prefix("yyyy") {
            (rest, Some(0), 4)
        } else if let Some(rest) = f.strip_prefix("MM") {
            (rest, Some(1), 2)
        } else if let Some(rest) = f.strip_prefix("dd") {
            (rest, Some(2), 2)
        } else if let Some(rest) = f.strip_prefix("HH") {
            (rest, Some(3), 2)
        } else if let Some(rest) = f.strip_prefix("mm") {
            (rest, Some(4), 2)
        } else if let Some(rest) = f.strip_prefix("ss") {
            (rest, Some(5), 2)
        } else if let Some(rest) = f.strip_prefix("FFFFFFF") {
            // 0~7 자리. 값은 초 밑이라 결과 꼴에 나오지 않는다.
            let mut n = 0;
            while n < 7 && input.get(pos).is_some_and(u8::is_ascii_digit) {
                pos += 1;
                n += 1;
            }
            (rest, None, 0)
        } else {
            let c = f.as_bytes()[0];
            if input.get(pos) != Some(&c) {
                return None;
            }
            pos += 1;
            (&f[1..], None, 0)
        };
        if let Some(slot) = slot {
            parts[slot] = digits(&mut pos, width)?;
        }
        f = token;
    }
    if pos != input.len() {
        return None;
    }
    let [y, mo, d, h, mi, se] = parts;
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let days = match mo {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return None,
    };
    if y == 0 || d == 0 || d > days || h > 23 || mi > 59 || se > 59 {
        return None;
    }
    Some((y, mo, d, h, mi, se))
}

// ── 표 ─────────────────────────────────────────────

fn ambiguous(source: &str) -> Error {
    Error::invalid(format!("대표 표를 하나로 확인하지 못했습니다: {source}"))
}

/// `Mapping.Table` — 출처가 가리키는 표의 행. `tables` 가 없거나 맞는 표가 없으면 거절한다.
pub fn table<'a>(input: &'a Value, source: &str) -> Result<&'a [Value]> {
    if element::try_get(input, "tables")?.is_none() {
        return Err(Error::invalid("tables가 없습니다."));
    }
    Ok(resolve(input, source)?.ok_or_else(|| ambiguous(source))?.1)
}

/// `Mapping.Resolve` — 출처가 가리키는 표 하나(표 이름과 행). 맞는 표가 없으면 `None`(그 표를 걷지 않은 문서다).
///
/// 와일드카드(`*접미사`)가 여러 표에 맞으면 빈 표를 버리고, 남은 것이 내용까지 같으면 앞의 것 하나로 본다 —
/// 「미리보기」 창이 같은 첨부를 한 벌 더 싣는다. 모두 비었으면 앞의 빈 표다. 서로 다른 비지 않은 표가 둘 이상이면
/// 거절한다. 정확한 이름은 하나에만 맞는다.
fn resolve<'a>(input: &'a Value, source: &str) -> Result<Option<(&'a str, &'a [Value])>> {
    let Some(Value::Object(tables)) = element::try_get(input, "tables")? else {
        return Ok(None);
    };
    let wildcard = source.strip_prefix('*');
    let matches: Vec<(&String, &Value)> = tables
        .iter()
        .filter(|(name, _)| match wildcard {
            Some(suffix) => name.ends_with(suffix),
            None => name.as_str() == source,
        })
        .collect();
    if matches.is_empty() {
        return Ok(None);
    }
    if matches.iter().any(|(_, v)| !v.is_array()) || (wildcard.is_none() && matches.len() != 1) {
        return Err(ambiguous(source));
    }
    let rows = |v: &'a Value| v.as_array().map_or(&[][..], Vec::as_slice);
    let filled: Vec<&(&String, &Value)> = matches
        .iter()
        .filter(|(_, v)| !rows(v).is_empty())
        .collect();
    if filled.iter().skip(1).any(|m| !same(m.1, filled[0].1)) {
        return Err(ambiguous(source));
    }
    let chosen = filled.first().copied().unwrap_or(&matches[0]);
    Ok(Some((chosen.0.as_str(), rows(chosen.1))))
}

/// 구조로 같은가 — 객체는 키의 순서와 관계없이 같은 키에 같은 값, 배열은 같은 순서로 같은 원소, 나머지는 적힌 그대로.
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| same(v, w)))
        }
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same(p, q))
        }
        (Value::Object(_) | Value::Array(_), _) | (_, Value::Object(_) | Value::Array(_)) => false,
        (p, q) => element::kind(p) == element::kind(q) && raw_text(p) == raw_text(q),
    }
}

/// `GetRawText()` 의 견줄 꼴 — 수는 원문, 글은 다시 쓴 글. 원문 그대로가 아니지만 같은 글은 같게, 다른 글은 다르게 쓴다.
fn raw_text(value: &Value) -> String {
    json::to_compact(value, Encoder::Default)
}

/// 출처 하나의 칸들 — 두 출처를 잇는 칸은 둘 다.
fn sources(fields: &[&FieldMap]) -> Vec<String> {
    fields
        .iter()
        .flat_map(|f| {
            let mut out = vec![f.source().to_string()];
            if let Some(s) = &f.secondary_source {
                out.push(s.clone());
            }
            out
        })
        .collect()
}

/// `Mapping.Sanitize` — 원천 JSON 에서 매핑이 읽는 칸만 남긴다(수집 기록에 남길 사진). 고른 표 하나만 남긴다 — 걸러
/// 담은 자료를 다시 읽어도 같은 표가 골라진다.
pub fn sanitize(set: &MappingSet, profile: &str, input: &Value) -> Result<Value> {
    let p = set
        .profile(profile)
        .ok_or_else(|| Error::invalid("Sequence contains no matching element"))?;
    let fields: Vec<&FieldMap> = p.fields_iter().collect();
    let mut point_keys: Vec<String> = sources(&fields)
        .into_iter()
        .filter(|k| !k.starts_with("tables."))
        .collect();
    if p.identity_source() == "pointInfo" {
        point_keys.push(p.base_field().to_string());
        point_keys.push(p.seq_field().to_string());
    }
    let mut tables = Map::new();
    for t in p.tables_iter() {
        let Some((key, rows)) = resolve(input, t.source())? else {
            continue;
        };
        let mut paths = sources(&t.fields_iter().collect::<Vec<_>>());
        paths.extend(t.keys_iter().map(str::to_string));
        let picked = rows
            .iter()
            .map(|row| pick(row, &paths))
            .collect::<Result<Vec<_>>>()?;
        tables.insert(key.to_string(), Value::Array(picked));
    }
    let mut groups: Vec<(String, Vec<&FieldMap>)> = Vec::new();
    for f in fields.iter().filter(|f| f.source().starts_with("tables.")) {
        let key = f.source().split('.').nth(1).unwrap_or("").to_string();
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, list)) => list.push(f),
            None => groups.push((key, vec![f])),
        }
    }
    for (key, group) in groups {
        if let Some(values) = element::try_get(element::get(input, "tables")?, &key)? {
            let paths: Vec<String> = group
                .iter()
                .map(|f| f.source().split('.').skip(3).collect::<Vec<_>>().join("."))
                .collect();
            let picked = element::array(values)?
                .iter()
                .map(|r| pick(r, &paths))
                .collect::<Result<Vec<_>>>()?;
            tables.insert(key, Value::Array(picked));
        }
    }
    let point = pick(element::get(input, "pointInfo")?, &point_keys)?;
    let mut out = Map::new();
    out.insert("pointInfo".into(), point);
    out.insert("tables".into(), Value::Object(tables));
    Ok(Value::Object(out))
}

/// `Sanitize.Pick` — 경로마다 그 값을 같은 경로의 자리에 옮긴다. 경로는 처음 나온 차례로 한 번씩.
fn pick(row: &Value, paths: &[String]) -> Result<Value> {
    let mut result = Map::new();
    let mut seen: Vec<&str> = Vec::new();
    for path in paths {
        if seen.contains(&path.as_str()) {
            continue;
        }
        seen.push(path);
        let Some(value) = at(row, path)? else {
            continue;
        };
        let parts: Vec<&str> = path.split('.').collect();
        let mut target = &mut result;
        for part in &parts[..parts.len() - 1] {
            let slot = target
                .entry(part.to_string())
                .or_insert_with(|| Value::Object(Map::new()));
            if slot.is_null() {
                *slot = Value::Object(Map::new());
            }
            target = match slot {
                Value::Object(map) => map,
                other => {
                    return Err(Error::InvalidOperation(format!(
                        "The node must be of type 'JsonObject', but the node is of type '{}'.",
                        node_kind(other)
                    )));
                }
            };
        }
        target.insert(parts[parts.len() - 1].to_string(), value.clone());
    }
    Ok(Value::Object(result))
}

fn node_kind(value: &Value) -> &'static str {
    match value {
        Value::Array(_) => "JsonArray",
        Value::Object(_) => "JsonObject",
        _ => "JsonValue",
    }
}

// ── 옮기기 ─────────────────────────────────────────────

/// 입찰공고 상세의 꼴(공고 A)을 읽는 프로필 — 일정 행을 세우는 후처리가 둘에 함께 걸린다.
const NOTICE_A: [&str; 2] = ["g2b-notice-a-v1", "g2b-notice-a-service-v1"];

/// `Mapping.Map` — 원천 문서를 업무 표의 행들로. 문서를 가르는 것은 번호·차수뿐이다 — 제목·표가 없으면 그 값을
/// 수집하지 않은 것으로 두고 기존 값을 건드리지 않는다.
pub fn map(
    set: &MappingSet,
    profile_id: &str,
    input: &Value,
    scope: &str,
) -> Result<MappedDocument> {
    validate(set)?;
    let p = set
        .profile(profile_id)
        .ok_or_else(|| Error::invalid("매핑 프로필을 선택하세요."))?;
    let point = element::get(input, "pointInfo")?;
    let identity = if p.identity_source() == "pointInfo" {
        Some(point)
    } else {
        table(input, p.identity_source())?.first()
    };
    let read = |path: &str| -> Result<Option<String>> {
        Ok(match identity {
            Some(v) => text(at(v, path)?),
            None => None,
        })
    };
    let mut b = read(p.base_field())?;
    let mut s = read(p.seq_field())?;
    // 본번호의 머리는 R(실제) 또는 T(시험용 번호)다.
    if p.base_field() == p.seq_field() {
        let (base, seq) = split_numbered(b.as_deref().unwrap_or(""))
            .ok_or_else(|| Error::invalid("본번호·차수 형식을 확인하세요."))?;
        b = Some(base);
        s = Some(seq);
    }
    let (b, s) = match (b, s) {
        (Some(b), Some(s))
            if !b.is_empty()
                && b.bytes().all(|c| c.is_ascii_alphanumeric())
                && (1..=3).contains(&s.chars().count())
                && s.chars().all(dotnet_digit) =>
        {
            (b, s)
        }
        _ => {
            return Err(Error::invalid(
                "문서 식별자와 차수가 필요합니다. 선행 0을 유지하세요.",
            ));
        }
    };
    let seq_len = s.chars().count();
    if (p.entity_type() == "notice" && (!numbered(&b, "BK") || seq_len != 3))
        || (p.entity_type() == "contract" && (!numbered(&b, "TA") || seq_len != 2))
    {
        return Err(Error::invalid("공고·계약 식별자 형식이 다릅니다."));
    }
    let fields: Vec<&FieldMap> = p.fields_iter().collect();
    let mut rows = vec![MappedRow {
        table: p.entity_type().to_string(),
        source_key: String::new(),
        values: field_values(point, &fields, Some(input))?,
        origins: None,
    }];
    for t in p.tables_iter() {
        let Some((_, source)) = resolve(input, t.source())? else {
            continue;
        };
        if source.is_empty() {
            continue; // 빈 표는 덜 불러온 탭일 수 있다 — 있던 행을 지우지 않는다.
        }
        let table_fields: Vec<&FieldMap> = t.fields_iter().collect();
        let mut keys: Vec<String> = Vec::new();
        for (index, row) in source.iter().enumerate() {
            // 키가 없는 표는 줄 차례가 곧 행이다 — 같은 값의 줄도, 빈 칸의 줄도 저마다 선다.
            let parts: Vec<Option<String>> = if t.key_count() == 0 {
                vec![Some(format!("#{}", index + 1))]
            } else {
                t.keys_iter()
                    .map(|k| Ok(text(at(row, k)?)))
                    .collect::<Result<_>>()?
            };
            if parts.iter().any(|x| x.as_deref().is_none_or(str::is_empty)) {
                return Err(Error::invalid(format!(
                    "원천 행 키가 없습니다: {}",
                    t.source()
                )));
            }
            let key = json::to_compact(
                &Value::Array(
                    parts
                        .into_iter()
                        .map(|x| x.map_or(Value::Null, Value::String))
                        .collect(),
                ),
                Encoder::Default,
            );
            if keys.contains(&key) {
                return Err(Error::invalid(format!(
                    "원천 행 키가 중복됩니다: {}",
                    t.source()
                )));
            }
            keys.push(key.clone());
            if p.entity_type() == "request"
                && t.target() == "request_item"
                && (text(at(row, p.base_field())?).as_deref() != Some(b.as_str())
                    || text(at(row, p.seq_field())?).as_deref() != Some(s.as_str()))
            {
                return Err(Error::invalid(
                    "서로 다른 접수를 한 문서로 저장할 수 없습니다.",
                ));
            }
            if t.target() == "contract_item"
                && (text(at(row, "ctrtNo")?).as_deref() != Some(b.as_str())
                    || text(at(row, "ctrtChgOrd")?).as_deref() != Some(s.as_str()))
            {
                return Err(Error::invalid(
                    "서로 다른 계약이나 변경차수의 품목을 한 문서로 저장할 수 없습니다.",
                ));
            }
            rows.push(MappedRow {
                table: t.target().to_string(),
                source_key: key,
                values: field_values(row, &table_fields, None)?,
                origins: None,
            });
        }
    }
    // 품목을 걷지 않았으면 총액을 다시 셈하지 않는다 — 0 으로 덮게 된다.
    if p.entity_type() == "contract" && rows.iter().any(|r| r.table == "contract_item") {
        let amounts: Vec<Option<&str>> = rows
            .iter()
            .filter(|r| r.table == "contract_item")
            .map(|r| r.value("amount"))
            .collect();
        if amounts.iter().any(|a| a.is_none_or(str::is_empty)) {
            return Err(Error::invalid(
                "전체 품목 금액이 있어야 계약 총액을 계산할 수 있습니다.",
            ));
        }
        let mut sum = Decimal::ZERO;
        for amount in amounts.into_iter().flatten() {
            sum = sum.add(Decimal::parse(amount)?)?;
        }
        set_pair(
            &mut rows[0].values,
            "amount",
            Some(sum.truncate_tens().to_text()),
        );
    }
    if NOTICE_A.contains(&p.id()) {
        let values = rows[0].values.clone();
        let has = |k: &str| values.iter().any(|(key, _)| key == k);
        let get = |k: &str| {
            values
                .iter()
                .find(|(key, _)| key == k)
                .and_then(|(_, v)| v.clone())
        };
        for (name, start, end) in [
            ("입찰서제출", "bid_opens_at", "bid_closes_at"),
            ("개찰", "opening_at", ""),
            ("입찰참가자격등록", "", "registration_closes_at"),
        ] {
            let mut schedule: Pairs = vec![("name".into(), Some(name.into()))];
            if has(start) {
                schedule.push(("starts_at".into(), get(start)));
            }
            if has(end) {
                schedule.push(("ends_at".into(), get(end)));
            }
            if schedule.len() > 1 {
                rows.push(MappedRow {
                    table: "notice_schedule".into(),
                    source_key: format!("schedule:{name}"),
                    values: schedule,
                    origins: None,
                });
            }
        }
    }
    for row in &mut rows {
        row.origins = Some(
            row.values
                .iter()
                .map(|(k, _)| (k.clone(), Some(p.id().to_string())))
                .collect(),
        );
    }
    // 접수의 키는 접수번호·접수차수 그대로다(ADR-027). 접두어를 달지 않는다.
    Ok(MappedDocument {
        profile: p.id().to_string(),
        entity_type: p.entity_type().to_string(),
        base: b,
        seq: s,
        rows,
        scope: scope.to_string(),
    })
}

/// `Mapping.Fields` — 머리(또는 표의 줄) 하나의 칸들. `tables.` 로 시작하는 출처는 문서의 뿌리에서 읽는다.
fn field_values(source: &Value, fields: &[&FieldMap], root: Option<&Value>) -> Result<Pairs> {
    let mut result: Pairs = Vec::new();
    for f in fields {
        let from = match root {
            Some(root) if f.source().starts_with("tables.") => root,
            _ => source,
        };
        let value = at(from, f.source())?;
        if matches!(value, Some(Value::Object(_) | Value::Array(_))) {
            return Err(Error::invalid("원천 필드는 단일 값이어야 합니다."));
        }
        if value.is_none() {
            continue;
        }
        let mut text_value = text(value);
        if let Some(secondary) = &f.secondary_source {
            let suffix = text(at(source, secondary)?);
            if text_value.as_deref().is_some_and(|t| !t.is_empty())
                && suffix.as_deref().is_none_or(str::is_empty)
            {
                continue;
            }
            // C# 의 null + null 은 빈 글이다.
            text_value = Some(format!(
                "{}{}",
                text_value.unwrap_or_default(),
                suffix.unwrap_or_default()
            ));
        }
        set_pair(&mut result, f.target(), convert(text_value.as_deref(), f)?);
    }
    Ok(result)
}

/// `[RT]\d{2}<kind>\d{8}` — 공고(`BK`)·계약(`TA`) 본번호.
fn numbered(b: &str, kind: &str) -> bool {
    let chars: Vec<char> = b.chars().collect();
    chars.len() == 13
        && matches!(chars[0], 'R' | 'T')
        && chars[1..3].iter().all(|c| dotnet_digit(*c))
        && chars[3..5].iter().collect::<String>() == kind
        && chars[5..].iter().all(|c| dotnet_digit(*c))
}

/// `\A([RT]\d{2}[A-Z]{2}\d{8})\s*-\s*(\d{2,3})\z` — 번호와 차수가 한 칸에 든 화면.
fn split_numbered(text: &str) -> Option<(String, String)> {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() < 13 {
        return None;
    }
    let head = &chars[..13];
    if !(matches!(head[0], 'R' | 'T')
        && head[1..3].iter().all(|c| dotnet_digit(*c))
        && head[3..5].iter().all(char::is_ascii_uppercase)
        && head[5..].iter().all(|c| dotnet_digit(*c)))
    {
        return None;
    }
    let mut rest = &chars[13..];
    while rest.first().is_some_and(|c| c.is_whitespace()) {
        rest = &rest[1..];
    }
    rest = rest.strip_prefix(&['-'])?;
    while rest.first().is_some_and(|c| c.is_whitespace()) {
        rest = &rest[1..];
    }
    if !(2..=3).contains(&rest.len()) || !rest.iter().all(|c| dotnet_digit(*c)) {
        return None;
    }
    Some((head.iter().collect(), rest.iter().collect()))
}

/// .NET 정규식의 `\d` — 유니코드 Nd(십진 숫자). .NET 정규식은 UTF-16 단위로 보므로 BMP 의 것만 맞는다(서로게이트 짝은
/// 숫자가 아니다). 블록마다 첫 글자(열 자씩).
pub(crate) fn dotnet_digit(c: char) -> bool {
    const ZEROS: &[u32] = &[
        0x30, 0x660, 0x6F0, 0x7C0, 0x966, 0x9E6, 0xA66, 0xAE6, 0xB66, 0xBE6, 0xC66, 0xCE6, 0xD66,
        0xDE6, 0xE50, 0xED0, 0xF20, 0x1040, 0x1090, 0x17E0, 0x1810, 0x1946, 0x19D0, 0x1A80, 0x1A90,
        0x1B50, 0x1BB0, 0x1C40, 0x1C50, 0xA620, 0xA8D0, 0xA900, 0xA9D0, 0xA9F0, 0xAA50, 0xABF0,
        0xFF10,
    ];
    let cp = c as u32;
    ZEROS.iter().any(|&zero| (zero..zero + 10).contains(&cp))
}

// ── 매핑 저장 ─────────────────────────────────────────────

/// C# `MappingStore` — 작업자료의 매핑 판들(`erp_mapping`). 활성 판이 없으면 기본 매핑이다.
pub struct MappingStore<'a> {
    database: &'a Database,
    env: &'a dyn Env,
}

impl<'a> MappingStore<'a> {
    pub fn new(database: &'a Database, env: &'a dyn Env) -> Self {
        MappingStore { database, env }
    }

    /// `MappingStore.Active` — 활성 판. 없으면 기본 매핑.
    pub fn active(&self) -> Result<MappingSet> {
        let c = self.database.open_read_only()?;
        active_in(&c)
    }

    /// `MappingStore.List` — 다리가 받는 꼴(`{active, defaults, versions}`).
    pub fn list(&self) -> Result<Value> {
        let c = self.database.open_read_only()?;
        let active = active_in(&c)?;
        let mut statement = c.prepare(
            "SELECT revision, json, active, validated, created_at FROM erp_mapping ORDER BY created_at DESC",
        )?;
        let names: Vec<String> = statement.column_names().map(str::to_string).collect();
        let versions = statement
            .query_map([], |r| {
                let mut row = Map::new();
                for (i, name) in names.iter().enumerate() {
                    row.insert(name.clone(), crate::storage::store::json_of(r.get_ref(i)?));
                }
                Ok(Value::Object(row))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(object([
            ("active", active.to_value()),
            ("defaults", defaults_ref().to_value()),
            ("versions", Value::Array(versions)),
        ]))
    }

    /// `MappingStore.Save` — 읽어 검증한 매핑을 판으로 적는다(이미 있으면 그대로). 판을 낸다.
    pub fn save(&self, json_text: &str) -> Result<String> {
        let set = parse(json_text)?;
        let value = set.to_value();
        let revision = hash(&value);
        let c = self.database.open()?;
        c.execute(
            "INSERT INTO erp_mapping(revision,json,created_at) VALUES(?1,?2,?3) ON CONFLICT DO NOTHING",
            params![revision, json::to_indented(&value, Encoder::Default), self.env.now_utc()],
        )?;
        Ok(revision)
    }

    /// `MappingStore.Activate` — 표본으로 검증한 판만 활성으로.
    pub fn activate(&self, revision: &str) -> Result<()> {
        let c = self.database.open()?;
        let tx = begin(&c)?;
        let count: i64 = tx.query_row(
            "SELECT count(*) FROM erp_mapping WHERE revision=?1 AND validated=1",
            [revision],
            |r| r.get(0),
        )?;
        if count != 1 {
            return Err(Error::invalid(
                "샘플 입력으로 매핑을 검증한 뒤 활성화하세요.",
            ));
        }
        tx.execute("UPDATE erp_mapping SET active=0", [])?;
        tx.execute(
            "UPDATE erp_mapping SET active=1 WHERE revision=?1",
            [revision],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// `MappingStore.ValidateSample` — 그 프로필로 표본을 옮겨 보고, 기본과 다른 프로필이 모두 표본을 지났으면 판을
    /// 검증된 것으로 적는다.
    pub fn validate_sample(
        &self,
        json_text: &str,
        profile: &str,
        input: &Value,
    ) -> Result<MappedDocument> {
        let set = parse(json_text)?;
        let document = map(&set, profile, input, "file")?;
        let revision = self.save(json_text)?;
        let c = self.database.open()?;
        let tx = begin(&c)?;
        let stored: String = tx.query_row(
            "SELECT samples FROM erp_mapping WHERE revision=?1",
            [&revision],
            |r| r.get(0),
        )?;
        let parsed = json::parse(&stored).map_err(|_| Error::invalid(UNREADABLE))?;
        let mut samples: Vec<String> = Vec::new();
        for item in element::array(&parsed)? {
            let item = element::string(item)?.unwrap_or_default().to_string();
            if !samples.contains(&item) {
                samples.push(item);
            }
        }
        if !samples.iter().any(|s| s == profile) {
            samples.push(profile.to_string());
        }
        let required: Vec<&str> = set
            .profiles_iter()
            .filter(|p| {
                let original = defaults_ref().profile(p.id()).map(MappingProfile::to_value);
                Some(hash(&p.to_value())) != original.map(|o| hash(&o))
            })
            .map(MappingProfile::id)
            .collect();
        let validated = i64::from(required.iter().all(|r| samples.iter().any(|s| s == r)));
        let samples_json = json::to_compact(
            &Value::Array(samples.into_iter().map(Value::String).collect()),
            Encoder::Default,
        );
        tx.execute(
            "UPDATE erp_mapping SET validated=?1,samples=?2 WHERE revision=?3",
            params![validated, samples_json, revision],
        )?;
        tx.commit()?;
        Ok(document)
    }
}

/// 열린 연결에서 활성 판을 읽는다. 없으면 기본 매핑.
pub(crate) fn active_in(c: &Connection) -> Result<MappingSet> {
    let json_text: Option<String> = c
        .query_row("SELECT json FROM erp_mapping WHERE active = 1", [], |r| {
            r.get(0)
        })
        .optional()?;
    match json_text {
        None => Ok(defaults()),
        Some(text) => read(&text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// C# `Mapping.Hash(Mapping.Defaults())` 의 실측값(scratch 콘솔, 지금의 `Erp/mapping.json`).
    /// 매핑 파일을 고치면 이 값도 바뀐다 — 그때는 C# 에서 다시 떠서 고친다.
    #[test]
    fn default_revision_matches_csharp() {
        assert_eq!(
            hash(&defaults_value()),
            "BCBF796437854FE6E3EB9224297B14A8100CE17AD501FCA5B489790D48F912D8"
        );
        // 레코드로 읽었다가 다시 쓴 꼴도 같은 판이다.
        assert_eq!(
            revision(&defaults()),
            "BCBF796437854FE6E3EB9224297B14A8100CE17AD501FCA5B489790D48F912D8"
        );
        // 판을 뜨는 글에 홀로 선 `\n` 이 없다 — 줄바꿈은 모두 `\r\n` 이다(C# `매핑의_판은_어느_운영체제에서나_같다`).
        let text = json::to_indented(&defaults_value(), Encoder::Default);
        assert!(text.contains("\r\n"));
        assert!(
            text.match_indices('\n')
                .all(|(at, _)| at > 0 && text.as_bytes()[at - 1] == b'\r')
        );
    }

    /// 기대값은 `decimal.Parse(…).ToString()` 실측.
    #[test]
    fn decimals_match_dotnet() {
        let cases = [
            ("1.50", Some("1.50")),
            ("-0", Some("0")),
            ("-0.00", Some("0.00")),
            (".5", Some("0.5")),
            ("5.", Some("5")),
            ("+1", Some("1")),
            ("007", Some("7")),
            (
                "0.0000000000000000000000000001",
                Some("0.0000000000000000000000000001"),
            ),
            (
                "12345678901234567890123456789",
                Some("12345678901234567890123456789"),
            ),
            (
                "79228162514264337593543950335",
                Some("79228162514264337593543950335"),
            ),
            ("79228162514264337593543950336", None),
            (
                "0.00000000000000000000000000001",
                Some("0.0000000000000000000000000000"),
            ),
            (
                "1.000000000000000000000000000001",
                Some("1.0000000000000000000000000000"),
            ),
            (" 1", None),
            ("1e3", None),
        ];
        for (input, expected) in cases {
            assert_eq!(parse_decimal(input).as_deref(), expected, "{input}");
        }
    }

    #[test]
    fn integers_match_dotnet() {
        assert_eq!(parse_int(" 5 ").as_deref(), Some("5"));
        assert_eq!(parse_int("+5").as_deref(), Some("5"));
        assert_eq!(parse_int("-0").as_deref(), Some("0"));
        assert_eq!(parse_int("\t5\n").as_deref(), Some("5"));
        assert_eq!(parse_int("2147483648"), None);
        assert_eq!(parse_int("5.0"), None);
    }

    #[test]
    fn dates_match_dotnet() {
        let cases = [
            ("2026-1-5", None),
            ("2026-01-05", Some("2026-01-05 00:00:00")),
            ("2026-01-05T03:04:05.", Some("2026-01-05 03:04:05")),
            ("2026-01-05T03:04:05.1", Some("2026-01-05 03:04:05")),
            ("2026-01-05T03:04:05.12345678", None),
            ("20260105", Some("2026-01-05 00:00:00")),
            ("2026-02-30", None),
            ("2026-01-05 3:04", None),
            ("2026-01-05 03:04", Some("2026-01-05 03:04:00")),
            ("0001-01-01", Some("0001-01-01 00:00:00")),
            ("2026-01-05T24:00:00", None),
            (" 2026-01-05", None),
        ];
        for (input, expected) in cases {
            assert_eq!(parse_date(input, false).as_deref(), expected, "{input}");
        }
    }

    #[test]
    fn at_reads_single_row_tables_only() {
        let doc = json::parse(r#"{"a":{"b":[{"c":" x "}]},"t":[1,2],"e":[],"n":null}"#).unwrap();
        assert_eq!(text(at(&doc, "a.b.0.c").unwrap()).as_deref(), Some("x"));
        assert_eq!(at(&doc, "t.0"), Err(AtError));
        assert_eq!(at(&doc, "e.0"), Ok(None));
        assert_eq!(at(&doc, "n"), Ok(Some(&Value::Null)));
        assert_eq!(at(&doc, "zz"), Ok(None));
    }

    #[test]
    fn regex_ports_match_dotnet() {
        assert!(sensitive("giveActno"));
        assert!(sensitive("bankAccountNo"));
        assert!(!sensitive("accounting"));
        assert!(sensitive("accountingaccount"));
        assert!(path_shape("*_grdFile"));
        assert!(path_shape("tables.a.0.b"));
        assert!(!path_shape("a.*b"));
        assert!(!path_shape("a..b"));
        assert_eq!(
            split_numbered("R26TA00000001 - 00"),
            Some(("R26TA00000001".into(), "00".into()))
        );
        assert_eq!(split_numbered("R26TA00000001-0"), None);
        assert!(dotnet_digit('٣') && dotnet_digit('９') && !dotnet_digit('a'));
    }
}

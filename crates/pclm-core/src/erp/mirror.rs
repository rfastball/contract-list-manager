//! 지금 보는 화면의 투영(C# `Erp/Mirror.cs`, ADR-036). 확장이 읽어 보낸 스냅샷을 **창 안에서** 미리 보고, 사람이 읽을
//! 모양으로 편다. 쓰지 않는다 — 투영의 쓰기는 모두 되돌린다. 링크를 짓는 [`explicit_links::resolve`] 는 부르지 않는다.
//!
//! **칸은 계약면의 표기 그대로다** — 쓰기 잠금 안의 savepoint 에 그 화면을 적용하고 면 뷰에서 읽은 뒤 되돌린다. 표기를 아는
//! 것은 뷰뿐이라 화면이 따로 흉내 내면 두 벌이 된다.
//!
//! [`explicit_links::resolve`]: super::explicit_links::resolve

use rusqlite::{Connection, Transaction, TransactionBehavior, params};
use serde_json::Value;

use super::capture::{
    self, CaptureChange, CaptureInput, CapturePreview, Choices, ErpCapture, inspect_in,
};
use super::element;
use super::mapping::{self, MappingProfile, MappingSet, MappingStore};
use super::screens;
use crate::env::Env;
use crate::error::{Error, Result};
use crate::local_time::{LocalZone, parse_to_local};
use crate::storage::database::Database;
use crate::storage::store::{EntityRef, face_key, face_view, text_of};
use crate::storage::views::{self, KEY_COLUMNS};
use crate::text::is_blank;

const POINT: &str = "pclm_mirror";

fn object(pairs: Vec<(&str, Value)>) -> Value {
    Value::Object(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

fn text(value: &str) -> Value {
    Value::String(value.to_string())
}

fn option(value: &Option<String>) -> Value {
    value.clone().map_or(Value::Null, Value::String)
}

/// C# `MirrorField` — 칸 하나. `kind` 는 `id`(번호 열)·`same`·`changed`·`new`(견줄 차수가 없다)·`override`(사람이 고친 칸의
/// 아래 값이 이번 화면으로 바뀐다)·`blank`(있던 값을 화면이 비워 보냈다).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MirrorField {
    pub column: String,
    pub kind: String,
    pub value: String,
    pub old: Option<String>,
    pub raw: Option<String>,
    pub human: Option<String>,
    pub choice_id: Option<String>,
    pub sources: Vec<String>,
    pub from: String,
}

impl MirrorField {
    pub fn to_value(&self) -> Value {
        object(vec![
            ("column", text(&self.column)),
            ("kind", text(&self.kind)),
            ("value", text(&self.value)),
            ("old", option(&self.old)),
            ("raw", option(&self.raw)),
            ("human", option(&self.human)),
            ("choiceId", option(&self.choice_id)),
            (
                "sources",
                Value::Array(self.sources.iter().map(|s| text(s)).collect()),
            ),
            ("from", text(&self.from)),
        ])
    }
}

/// C# `ScreenRow` — 확장이 화면에서 읽어 보낸 이름표와 보이는 글 한 줄. 값은 화면에 보인 그대로다.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScreenRow {
    pub group: String,
    pub label: String,
    pub text: String,
    pub source: String,
}

impl ScreenRow {
    pub fn to_value(&self) -> Value {
        object(vec![
            ("group", text(&self.group)),
            ("label", text(&self.label)),
            ("text", text(&self.text)),
            ("source", text(&self.source)),
        ])
    }
}

/// C# `MirrorExport` — 「현재 화면 엑셀로 내보내기」 의 그 탭 마지막 결과.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MirrorExport {
    pub at: i64,
    pub ok: bool,
    pub message: String,
}

impl MirrorExport {
    pub fn to_value(&self) -> Value {
        object(vec![
            ("at", self.at.into()),
            ("ok", Value::Bool(self.ok)),
            ("message", text(&self.message)),
        ])
    }
}

/// C# `MirrorItemBefore` — 견준 차수의 같은 순번 품목.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MirrorItemBefore {
    pub quantity: String,
    pub unit: String,
    pub price: String,
    pub amount: String,
}

/// C# `MirrorItem`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MirrorItem {
    pub line: String,
    pub name: String,
    pub spec: String,
    pub quantity: String,
    pub unit: String,
    pub price: String,
    pub amount: String,
    pub before: Option<MirrorItemBefore>,
    pub added: bool,
    pub source: String,
}

impl MirrorItem {
    pub fn to_value(&self) -> Value {
        let before = self.before.as_ref().map_or(Value::Null, |b| {
            object(vec![
                ("quantity", text(&b.quantity)),
                ("unit", text(&b.unit)),
                ("price", text(&b.price)),
                ("amount", text(&b.amount)),
            ])
        });
        object(vec![
            ("line", text(&self.line)),
            ("name", text(&self.name)),
            ("spec", text(&self.spec)),
            ("quantity", text(&self.quantity)),
            ("unit", text(&self.unit)),
            ("price", text(&self.price)),
            ("amount", text(&self.amount)),
            ("before", before),
            ("added", Value::Bool(self.added)),
            ("source", text(&self.source)),
        ])
    }
}

/// C# `MirrorPlace` — 접수 → 공고 → 계약 의 한 칸. `state` 는 `here`·`linked`·`ref`·`missing`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MirrorPlace {
    pub kind: String,
    pub state: String,
    pub number: String,
    pub collected: bool,
    pub source: Option<String>,
}

impl MirrorPlace {
    fn new(kind: &str, state: &str, number: &str, collected: bool) -> Self {
        MirrorPlace {
            kind: kind.into(),
            state: state.into(),
            number: number.into(),
            collected,
            source: None,
        }
    }

    pub fn to_value(&self) -> Value {
        object(vec![
            ("kind", text(&self.kind)),
            ("state", text(&self.state)),
            ("number", text(&self.number)),
            ("collected", Value::Bool(self.collected)),
            ("source", option(&self.source)),
        ])
    }
}

/// C# `MirrorRound` — `state` 는 `stored`·`current`·`ghost`. `saved_on` 은 로컬 `MM/dd`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MirrorRound {
    pub seq: String,
    pub amount: String,
    pub saved_on: String,
    pub state: String,
}

impl MirrorRound {
    pub fn to_value(&self) -> Value {
        object(vec![
            ("seq", text(&self.seq)),
            ("amount", text(&self.amount)),
            ("savedOn", text(&self.saved_on)),
            ("state", text(&self.state)),
        ])
    }
}

/// C# `MirrorMissing` — 작업자료에 있는데 이번 화면에 없는 행(`__row`) 하나.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MirrorMissing {
    pub table: String,
    pub table_name: String,
    pub line: i64,
    pub label: String,
    pub amount: String,
    pub choice_id: String,
}

impl MirrorMissing {
    pub fn to_value(&self) -> Value {
        object(vec![
            ("table", text(&self.table)),
            ("tableName", text(&self.table_name)),
            ("line", self.line.into()),
            ("label", text(&self.label)),
            ("amount", text(&self.amount)),
            ("choiceId", text(&self.choice_id)),
        ])
    }
}

/// C# `MirrorScreen` — 수집하는 화면 하나(ADR-037).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MirrorScreen {
    pub code: String,
    pub entity_type: String,
    pub kind: String,
    pub name: String,
}

impl MirrorScreen {
    pub fn to_value(&self) -> Value {
        object(vec![
            ("code", text(&self.code)),
            ("entityType", text(&self.entity_type)),
            ("kind", text(&self.kind)),
            ("name", text(&self.name)),
        ])
    }
}

/// C# `MirrorShot` — 지금 보는 화면을 작업자료에 비춘 것.
#[derive(Clone, Debug, PartialEq)]
pub struct MirrorShot {
    pub entity_type: String,
    pub kind: String,
    pub number: String,
    pub base: String,
    pub seq: String,
    pub title: String,
    pub status: String,
    pub changes: i64,
    pub compare_seq: Option<String>,
    pub latest_seq: Option<String>,
    pub view: String,
    pub view_columns: i64,
    pub fields: Vec<MirrorField>,
    pub rest: i64,
    pub items: Vec<MirrorItem>,
    pub item_changes: i64,
    pub item_rows: i64,
    pub items_all_read: bool,
    pub place: Vec<MirrorPlace>,
    pub rounds: Vec<MirrorRound>,
    pub base_token: String,
    pub missing: Vec<MirrorMissing>,
    pub kept_cells: i64,
    pub kept_in: String,
    pub totals_with_defaults: Option<bool>,
    pub totals_without_missing: Option<bool>,
}

impl MirrorShot {
    /// 다리(camelCase)의 꼴.
    pub fn to_value(&self) -> Value {
        let flag = |b: Option<bool>| b.map_or(Value::Null, Value::Bool);
        object(vec![
            ("entityType", text(&self.entity_type)),
            ("kind", text(&self.kind)),
            ("number", text(&self.number)),
            ("base", text(&self.base)),
            ("seq", text(&self.seq)),
            ("title", text(&self.title)),
            ("status", text(&self.status)),
            ("changes", self.changes.into()),
            ("compareSeq", option(&self.compare_seq)),
            ("latestSeq", option(&self.latest_seq)),
            ("view", text(&self.view)),
            ("viewColumns", self.view_columns.into()),
            (
                "fields",
                Value::Array(self.fields.iter().map(MirrorField::to_value).collect()),
            ),
            ("rest", self.rest.into()),
            (
                "items",
                Value::Array(self.items.iter().map(MirrorItem::to_value).collect()),
            ),
            ("itemChanges", self.item_changes.into()),
            ("itemRows", self.item_rows.into()),
            ("itemsAllRead", Value::Bool(self.items_all_read)),
            (
                "place",
                Value::Array(self.place.iter().map(MirrorPlace::to_value).collect()),
            ),
            (
                "rounds",
                Value::Array(self.rounds.iter().map(MirrorRound::to_value).collect()),
            ),
            ("baseToken", text(&self.base_token)),
            (
                "missing",
                Value::Array(self.missing.iter().map(MirrorMissing::to_value).collect()),
            ),
            ("keptCells", self.kept_cells.into()),
            ("keptIn", text(&self.kept_in)),
            ("totalsWithDefaults", flag(self.totals_with_defaults)),
            ("totalsWithoutMissing", flag(self.totals_without_missing)),
        ])
    }
}

/// `Mirror.KindName`.
pub fn kind_name(entity_type: &str) -> &'static str {
    screens::kind_name(entity_type)
}

/// `Mirror.SupportedScreens` — 수집하는 화면들. 매핑을 읽지 못하면 기본 매핑으로 물러선다: 화면과 고유키는 기본의 것으로
/// 고정이라 같다.
pub fn supported_screens(database: &Database, env: &dyn Env) -> Vec<MirrorScreen> {
    let mapping = MappingStore::new(database, env)
        .active()
        .unwrap_or_else(|_| mapping::defaults());
    screens::supported(&mapping)
        .map(|p| {
            let screen = p.screen.as_ref();
            MirrorScreen {
                code: screen.map_or("", |s| s.code()).to_string(),
                entity_type: p.entity_type().to_string(),
                kind: kind_name(p.entity_type()).to_string(),
                name: screen.map_or("", |s| s.name()).to_string(),
            }
        })
        .collect()
}

fn title_column(entity_type: &str) -> &'static str {
    match entity_type {
        "request" => "요청명",
        "notice" => "공고명",
        _ => "계약건명",
    }
}

fn amount_column(entity_type: &str) -> &'static str {
    match entity_type {
        "request" => "품대",
        "notice" => "추정가격",
        _ => "계약금액",
    }
}

struct ItemView {
    view: &'static str,
    key: &'static str,
    name: &'static str,
    price: &'static str,
    amount: Option<&'static str>,
}

fn items_of(entity_type: &str) -> ItemView {
    match entity_type {
        "request" => ItemView {
            view: "v_접수품목",
            key: "접수번호",
            name: "세부품명",
            price: "단가",
            amount: Some("금액"),
        },
        "notice" => ItemView {
            view: "v_공고품목",
            key: "입찰공고번호",
            name: "세부품명",
            price: "추정단가",
            amount: None,
        },
        _ => ItemView {
            view: "v_계약품목",
            key: "계약번호",
            name: "품명",
            price: "단가",
            amount: Some("금액"),
        },
    }
}

/// `Mirror.Wipes` — 있던 값을 빈 값으로 지우는 변경. 늦게 그려진 탭을 빈 칸으로 읽은 것이 대부분이라 유지가 기본이다.
pub fn wipes(change: &CaptureChange) -> bool {
    change.conflict
        && change.override_column.is_none()
        && change.field != "__row"
        && change.after.as_deref().unwrap_or("").is_empty()
        && !change.before.as_deref().unwrap_or("").is_empty()
}

/// `Mirror.DefaultChoices` — 더하고 바꾸는 것은 기본, 지우는 것은 사람이 고를 때만. 덮개(`__override`)·있던 값이 비게
/// 되는 것([`wipes`])·화면에 없는 행(`__row`)은 그대로 두고, `picks` 에 있을 때만 적용한다. 충돌마다 하나를 빠짐없이 낸다.
pub fn default_choices(preview: &CapturePreview, picks: &[String]) -> Choices {
    let mut choices: Choices = Vec::new();
    for change in preview.changes.iter().filter(|ch| ch.conflict) {
        let choice = if change.field == "__override" || change.field == "__row" || wipes(change) {
            if picks.contains(&change.id) {
                "apply"
            } else {
                "keep"
            }
        } else {
            "apply"
        };
        match choices.iter_mut().find(|(k, _)| *k == change.id) {
            Some(slot) => slot.1 = choice.into(),
            None => choices.push((change.id.clone(), choice.into())),
        }
    }
    choices
}

/// `Mirror.AsNewRows` — 작업자료에 있는 줄과 짝이 없는 화면의 줄은 모두 새 줄로 둔다(`rowMatches` 를 0 으로). 투영과
/// 가져오기가 함께 부른다: baseToken 은 rowMatches 까지 셈하므로 한쪽만 고치면 토큰이 갈린다.
pub(crate) fn as_new_rows(
    mut inspect: impl FnMut(&CaptureInput) -> Result<CapturePreview>,
    input: &CaptureInput,
) -> Result<(CaptureInput, CapturePreview)> {
    let preview = inspect(input)?;
    if preview.unmatched.is_empty() {
        return Ok((input.clone(), preview));
    }
    let mut matches = input.row_matches.clone().unwrap_or_default();
    for row in &preview.unmatched {
        match matches.iter_mut().find(|(k, _)| *k == row.id) {
            Some(slot) => slot.1 = 0,
            None => matches.push((row.id.clone(), 0)),
        }
    }
    let paired = CaptureInput {
        row_matches: Some(matches),
        ..input.clone()
    };
    let preview = inspect(&paired)?;
    Ok((paired, preview))
}

/// `Mirror.Prepare` — 창의 가져오기가 쓸 입력과 그 미리보기([`as_new_rows`]).
pub fn prepare(
    capture: &ErpCapture<'_>,
    input: &CaptureInput,
) -> Result<(CaptureInput, CapturePreview)> {
    as_new_rows(|i| capture.inspect(i), input)
}

/// `Mirror.Project` — 스냅샷 하나를 투영한다. 쓰기 잠금을 짧게 쥐고(BEGIN IMMEDIATE) 그 안의 savepoint 에서만 쓴 뒤 모두
/// 되돌린다. `screen` 은 확장이 함께 보낸 화면의 이름표와 보이는 글이다. `zone` 은 「고친 날」 을 로컬로 읽는 시간대다.
pub fn project(
    capture: &ErpCapture<'_>,
    env: &dyn Env,
    zone: &dyn LocalZone,
    input: &CaptureInput,
    screen: &[ScreenRow],
) -> Result<MirrorShot> {
    let c = capture.open()?;
    let mapping = mapping::active_in(&c)?;
    let tx = Transaction::new_unchecked(&c, TransactionBehavior::Immediate)?;
    let (live, preview) = as_new_rows(|i| inspect_in(&tx, env, i), input)?;
    let shot = project_in(&tx, env, zone, &mapping, &live, preview, screen);
    tx.rollback()?;
    shot
}

/// `Mirror.ScreenRows` — 탭 보고 한 탭의 `screenRows`. 없거나 모양이 틀린 줄은 건너뛴다.
pub fn screen_rows(tab: &Value) -> Vec<ScreenRow> {
    let Some(Value::Array(list)) = tab.as_object().and_then(|t| t.get("screenRows")) else {
        return Vec::new();
    };
    list.iter()
        .filter_map(|row| {
            let field = |name: &str| element::text_property(row, name).map(str::to_string);
            Some(ScreenRow {
                group: field("group")?,
                label: field("label")?,
                text: field("text")?,
                source: field("source")?,
            })
        })
        .collect()
}

/// `Mirror.Export` — 탭 보고 한 탭의 `export`. 없거나 모양이 틀리면 `None`.
pub fn export(tab: &Value) -> Option<MirrorExport> {
    let e = tab.as_object()?.get("export")?.as_object()?;
    let at = match e.get("at")? {
        Value::Number(n) => {
            let raw = n.to_string();
            let body = raw.strip_prefix('-').unwrap_or(&raw);
            if body.is_empty() || !body.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            raw.parse::<i64>().ok()?
        }
        _ => return None,
    };
    let ok = e.get("ok")?.as_bool()?;
    let message = e
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    Some(MirrorExport { at, ok, message })
}

/// `Mirror.Points` — 화면 줄의 자리가 그 source 를 가리키는가. 표 자리(`table:x`)는 그 표의 모든 줄(`table:x:n`)을
/// 가리킨다.
pub fn points(source: &str, row: &str) -> bool {
    !source.is_empty()
        && (row == source
            || (source.starts_with("table:")
                && row.len() > source.len()
                && row.starts_with(source)
                && row.as_bytes()[source.len()] == b':'))
}

/// `Mirror.Input` — 탭 보고의 스냅샷을 수집 입력으로. 확장의 `inspect` 와 같은 모양이다.
pub fn input(snapshot: &Value) -> Result<CaptureInput> {
    CaptureInput::from_json(snapshot)?.ok_or_else(|| Error::invalid("화면 자료가 비었습니다."))
}

fn distinct(items: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for item in items {
        if !out.contains(&item) {
            out.push(item);
        }
    }
    out
}

/// `Mirror.SourcesOf` — 면 열 하나를 채우는 화면의 자리.
fn sources_of(
    profile: Option<&MappingProfile>,
    entity_type: &str,
    column: &str,
    references: &[(String, Vec<String>)],
) -> Vec<String> {
    let Some(profile) = profile else {
        return Vec::new();
    };
    if column == face_key(entity_type) || column.ends_with("본번호") || column == "차수" {
        return if profile.identity_source() == "pointInfo" {
            distinct(vec![
                profile.base_field().to_string(),
                profile.seq_field().to_string(),
            ])
        } else {
            Vec::new()
        };
    }
    let mut sources = Vec::new();
    let refs = references
        .iter()
        .find(|(c, _)| c == column)
        .map_or(&[][..], |(_, r)| r.as_slice());
    for reference in refs {
        let Some((table, target)) = reference.split_once('.') else {
            continue;
        };
        if table == entity_type {
            sources.extend(
                profile
                    .fields_iter()
                    .filter(|f| f.target() == target)
                    .map(|f| f.source().to_string()),
            );
        } else if let Some(t) = profile.tables_iter().find(|t| t.target() == table)
            && t.fields_iter().any(|f| f.target() == target)
        {
            sources.push(format!("table:{table}"));
        }
    }
    distinct(sources)
}

/// `Mirror.RefSource` — 그 종류를 참조하는 번호 칸의 자리(`ref_<종류>_base` 를 채우는 것).
fn ref_source(profile: Option<&MappingProfile>, kind: &str) -> Option<String> {
    let profile = profile?;
    let target = format!("ref_{kind}_base");
    if let Some(field) = profile.fields_iter().find(|f| f.target() == target) {
        return Some(field.source().to_string());
    }
    profile
        .tables_iter()
        .find(|t| t.fields_iter().any(|f| f.target() == target))
        .map(|t| format!("table:{}", t.target()))
}

/// 뷰의 줄 하나 — 열 차례를 지키고 값은 문자열로(계약면은 빈 값도 빈 문자열이다).
type ViewRow = Vec<(String, String)>;

fn get<'r>(row: &'r ViewRow, name: &str) -> Option<&'r str> {
    row.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
}

fn get_or<'r>(row: &'r ViewRow, name: &str) -> &'r str {
    get(row, name).unwrap_or("")
}

/// `Mirror.Rows`.
fn rows(c: &Connection, view: &str, key: &str, value: &str) -> Result<Vec<ViewRow>> {
    let mut statement = c.prepare(&format!("SELECT * FROM \"{view}\" WHERE \"{key}\"=?1"))?;
    let names: Vec<String> = statement.column_names().map(str::to_string).collect();
    let mut out = Vec::new();
    let mut found = statement.query([value])?;
    while let Some(row) = found.next()? {
        let mut values: ViewRow = Vec::new();
        for (i, name) in names.iter().enumerate() {
            let v = text_of(row.get_ref(i)?).unwrap_or_default();
            match values.iter_mut().find(|(k, _)| k == name) {
                Some(slot) => slot.1 = v,
                None => values.push((name.clone(), v)),
            }
        }
        out.push(values);
    }
    Ok(out)
}

fn row(c: &Connection, view: &str, key: &str, value: &str) -> Result<Option<ViewRow>> {
    Ok(rows(c, view, key, value)?.into_iter().next())
}

/// Dapper `QuerySingleOrDefault<string>` — 줄이 없으면 `None`, 둘 이상이면 거절한다.
fn single_or_default(c: &Connection, sql: &str, args: &[&str]) -> Result<Option<String>> {
    let mut statement = c.prepare(sql)?;
    let mut found = statement.query(rusqlite::params_from_iter(args.iter()))?;
    let Some(first) = found.next()? else {
        return Ok(None);
    };
    let value = text_of(first.get_ref(0)?);
    if found.next()?.is_some() {
        return Err(Error::invalid("Sequence contains more than one element"));
    }
    Ok(value)
}

fn texts(c: &Connection, sql: &str, args: &[&str]) -> Result<Vec<Option<String>>> {
    let mut statement = c.prepare(sql)?;
    let out = statement
        .query_map(rusqlite::params_from_iter(args.iter()), |r| {
            Ok(text_of(r.get_ref(0)?))
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(out)
}

/// `Mirror.DropLater` — 그 차수보다 늦은 차수를 걷는다. savepoint 안에서만 부른다.
fn drop_later(c: &Connection, entity_type: &str, base: &str, seq: &str) -> Result<()> {
    c.execute(
        &format!(
            "DELETE FROM {entity_type} WHERE {entity_type}_base=?1 AND CAST(seq AS INTEGER) > CAST(?2 AS INTEGER)"
        ),
        params![base, seq],
    )?;
    Ok(())
}

/// `int.Parse(seq, InvariantCulture)` — 차수를 수로.
fn seq_number(seq: &str) -> Result<i32> {
    let t = seq.trim_matches(|c: char| matches!(c, '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' '));
    let body = t.strip_prefix(['-', '+']).unwrap_or(t);
    if body.is_empty() || !body.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Error::invalid(format!(
            "The input string '{seq}' was not in a correct format."
        )));
    }
    t.parse()
        .map_err(|_| Error::invalid("Value was either too large or too small for an Int32."))
}

fn project_in(
    c: &Connection,
    env: &dyn Env,
    zone: &dyn LocalZone,
    mapping: &MappingSet,
    input: &CaptureInput,
    preview: CapturePreview,
    screen: &[ScreenRow],
) -> Result<MirrorShot> {
    let doc = &preview.document;
    let ty = doc.entity_type.as_str();
    let profile = mapping.profile(input.profile());
    let references = views::face_references(ty);
    let face = face_view(ty);
    let key = face_key(ty);
    let items = items_of(ty);
    let display = |seq: &str| EntityRef::new(ty, &doc.base, seq).display();

    // 쌓인 차수. 고치기 전에 읽는다.
    let stored: Vec<(String, Option<String>)> = c
        .prepare(&format!(
            "SELECT seq, updated_at FROM {ty} WHERE {ty}_base=?1 ORDER BY CAST(seq AS INTEGER)"
        ))?
        .query_map([&doc.base], |r| {
            Ok((
                text_of(r.get_ref(0)?).unwrap_or_default(),
                text_of(r.get_ref(1)?),
            ))
        })?
        .collect::<rusqlite::Result<_>>()?;
    let exists = stored.iter().any(|(s, _)| *s == doc.seq);
    let latest = stored.last().map(|(s, _)| s.clone());
    let compare = if exists {
        Some(doc.seq.clone())
    } else {
        latest.clone()
    };
    let mut rounds = Vec::new();
    for (seq, updated_at) in &stored {
        let amount = row(c, capture::revision_view(ty), key, &display(seq))?
            .and_then(|r| get(&r, amount_column(ty)).map(str::to_string))
            .unwrap_or_default();
        rounds.push(MirrorRound {
            seq: seq.clone(),
            amount,
            saved_on: parse_to_local(updated_at.as_deref(), zone)
                .map(|at| at.month_day())
                .unwrap_or_default(),
            state: if *seq == doc.seq { "current" } else { "stored" }.into(),
        });
    }
    let users: Vec<String> = texts(
        c,
        "SELECT field_name FROM user_column WHERE entity_type=?1",
        &[ty],
    )?
    .into_iter()
    .flatten()
    .collect();

    let mut before: Option<ViewRow> = None;
    let mut before_items: Vec<(String, ViewRow)> = Vec::new();
    let mut item_sources: Vec<(String, String)> = Vec::new();
    let mut with_defaults = None;
    let mut without_missing = None;
    c.execute_batch(&format!("SAVEPOINT {POINT}"))?;
    let applied = (|| -> Result<(ViewRow, Vec<ViewRow>, Vec<MirrorPlace>)> {
        // 견줄 차수를 계약면에서 읽는다. 면 뷰는 본번호마다 가장 늦은 차수만 내므로, 그보다 늦은 것을 잠시 걷는다.
        if let Some(compare) = &compare {
            drop_later(c, ty, &doc.base, compare)?;
            before = row(c, face, key, &display(compare))?;
            for r in rows(c, items.view, items.key, &display(compare))? {
                let line = get_or(&r, "순번").to_string();
                if !before_items.iter().any(|(k, _)| *k == line) {
                    before_items.push((line, r));
                }
            }
            c.execute_batch(&format!("ROLLBACK TO {POINT}"))?;
        }
        // 이 화면을 기본 규칙으로 적용하고 그 차수를 면에서 읽는다.
        drop_later(c, ty, &doc.base, &doc.seq)?;
        let defaults = default_choices(&preview, &[]);
        let now = env.now_utc();
        capture::write(
            c,
            input,
            doc,
            &preview.changes,
            |ch| {
                Some(if ch.conflict {
                    defaults
                        .iter()
                        .find(|(k, _)| *k == ch.id)
                        .map_or("keep", |(_, v)| v.as_str())
                } else {
                    "apply"
                })
            },
            &now,
        )?;
        if ty == "contract" {
            with_defaults = Some(capture::totals_match(c, doc)?);
        }
        let after = row(c, face, key, &display(&doc.seq))?.unwrap_or_default();
        let after_items = rows(c, items.view, items.key, &display(&doc.seq))?;
        // 품목 줄마다 화면의 몇째 줄인가 — 확장이 화면의 품목 줄을 같은 차례로 센다(table:<대상 표>:<n>).
        let item_table = format!("{ty}_item");
        let mut order: Vec<(&str, usize)> = Vec::new();
        for (i, r) in doc
            .rows
            .iter()
            .filter(|r| r.table == item_table)
            .enumerate()
        {
            if !order.iter().any(|(k, _)| *k == r.source_key) {
                order.push((&r.source_key, i + 1));
            }
        }
        let tracked: Vec<(String, i64)> = c
            .prepare(
                "SELECT source_key, line_no FROM erp_row WHERE entity_type=?1 AND entity_base=?2 AND entity_seq=?3 AND table_name=?4",
            )?
            .query_map(params![ty, doc.base, doc.seq, item_table], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?
            .collect::<rusqlite::Result<_>>()?;
        for (source_key, line) in tracked {
            if let Some((_, n)) = order.iter().find(|(k, _)| *k == source_key) {
                let line = line.to_string();
                let value = format!("table:{item_table}:{n}");
                match item_sources.iter_mut().find(|(k, _)| *k == line) {
                    Some(slot) => slot.1 = value,
                    None => item_sources.push((line, value)),
                }
            }
        }
        let place = place_of(c, ty, &doc.base, &doc.seq, &display(&doc.seq))?;
        // 화면에 없는 품목 행을 모두 지우면 맞는가 — 맞지 않는 계약에 「모두 지우기」 를 내밀지 가린다.
        if ty == "contract" {
            for removed in preview
                .changes
                .iter()
                .filter(|ch| ch.field == "__row" && ch.table == "contract_item")
            {
                capture::remove_row(c, doc, removed)?;
            }
            without_missing = Some(capture::totals_match(c, doc)?);
        }
        Ok((after, after_items, place))
    })();
    c.execute_batch(&format!("ROLLBACK TO {POINT}; RELEASE {POINT};"))?;
    let (after, after_items, place) = applied?;
    let columns = after.len() as i64;
    let place: Vec<MirrorPlace> = place
        .into_iter()
        .map(|p| {
            if p.state == "ref" || p.state == "linked" {
                let kind = match p.kind.as_str() {
                    "접수" => "request",
                    "공고" => "notice",
                    _ => "contract",
                };
                MirrorPlace {
                    source: ref_source(profile, kind),
                    ..p
                }
            } else {
                p
            }
        })
        .collect();

    // 칸. 화면의 이름표와 보이는 글은 그 칸을 채우는 자리에서 읽는다.
    let mut overrides: Vec<(&str, &CaptureChange)> = Vec::new();
    for ch in preview
        .changes
        .iter()
        .filter(|ch| ch.field == "__override" && ch.override_column.is_some())
    {
        let column = ch.override_column.as_deref().unwrap_or_default();
        if !overrides.iter().any(|(k, _)| *k == column) {
            overrides.push((column, ch));
        }
    }
    // 화면이 비워 보낸 머리 칸 — 그 본 표의 열을 재료로 쓰는 면 열에 단다.
    let mut wiped: Vec<(String, &CaptureChange)> = Vec::new();
    for ch in preview
        .changes
        .iter()
        .filter(|ch| ch.table == ty && wipes(ch))
    {
        let k = format!("{ty}.{}", ch.field);
        if !wiped.iter().any(|(w, _)| *w == k) {
            wiped.push((k, ch));
        }
    }
    let mut blanks: Vec<(&str, &CaptureChange)> = Vec::new();
    for (column, refs) in &references {
        if let Some(wipe) = refs
            .iter()
            .find_map(|r| wiped.iter().find(|(w, _)| w == r).map(|(_, ch)| *ch))
        {
            match blanks.iter_mut().find(|(k, _)| *k == column) {
                Some(slot) => slot.1 = wipe,
                None => blanks.push((column, wipe)),
            }
        }
    }
    let mut fields = Vec::new();
    let mut rest = 0;
    for (column, value) in &after {
        if users.contains(column) {
            continue; // 사람이 채우는 열은 화면에서 오지 않는다.
        }
        let sources = sources_of(profile, ty, column, &references);
        let from = from_of(&sources, screen);
        let field = |kind: &str, value: &str, old, raw, human, choice_id| MirrorField {
            column: column.clone(),
            kind: kind.into(),
            value: value.into(),
            old,
            raw,
            human,
            choice_id,
            sources: sources.clone(),
            from: from.clone(),
        };
        if KEY_COLUMNS.contains(&column.as_str()) {
            if !value.is_empty() {
                fields.push(field("id", value, None, None, None, None));
            }
            continue;
        }
        if let Some((_, o)) = overrides.iter().find(|(k, _)| k == column) {
            let after_value = o.after.clone().unwrap_or_default();
            fields.push(field(
                "override",
                &after_value,
                None,
                shown(&after_value, &sources, screen),
                Some(o.before.clone().unwrap_or_default()),
                Some(o.id.clone()),
            ));
            continue;
        }
        if let Some((_, blank)) = blanks.iter().find(|(k, _)| k == column) {
            fields.push(field(
                "blank",
                value,
                None,
                None,
                None,
                Some(blank.id.clone()),
            ));
            continue;
        }
        let old = before.as_ref().and_then(|b| get(b, column));
        if value.is_empty() && old.unwrap_or("").is_empty() {
            rest += 1;
            continue;
        }
        let kind = if before.is_none() {
            "new"
        } else if Some(value.as_str()) == old {
            "same"
        } else {
            "changed"
        };
        fields.push(field(
            kind,
            value,
            (kind == "changed").then(|| old.unwrap_or("").to_string()),
            shown(value, &sources, screen),
            None,
            None,
        ));
    }

    // 품목.
    let mut list = Vec::new();
    for r in &after_items {
        let line = get_or(r, "순번").to_string();
        let mut item = MirrorItem {
            line: line.clone(),
            name: get_or(r, items.name).into(),
            spec: get_or(r, "규격").into(),
            quantity: get_or(r, "수량").into(),
            unit: get_or(r, "단위").into(),
            price: get_or(r, items.price).into(),
            amount: items.amount.map_or("", |a| get_or(r, a)).into(),
            before: None,
            added: false,
            source: item_sources
                .iter()
                .find(|(k, _)| *k == line)
                .map_or(String::new(), |(_, v)| v.clone()),
        };
        if before.is_some() {
            match before_items.iter().find(|(k, _)| *k == line) {
                None => item.added = true,
                Some((_, earlier)) => {
                    let was = MirrorItemBefore {
                        quantity: get_or(earlier, "수량").into(),
                        unit: get_or(earlier, "단위").into(),
                        price: get_or(earlier, items.price).into(),
                        amount: items.amount.map_or("", |a| get_or(earlier, a)).into(),
                    };
                    if was.quantity != item.quantity
                        || was.unit != item.unit
                        || was.price != item.price
                        || was.amount != item.amount
                    {
                        item.before = Some(was);
                    }
                }
            }
        }
        list.push(item);
    }
    let item_changes = list
        .iter()
        .filter(|i| i.before.is_some() || i.added)
        .count() as i64;
    let item_rows = item_rows_of(mapping, input, ty);

    // 화면에 없는 행. 품목은 계약면의 표기로, 그 밖은 지울 줄의 원값에서 알아볼 글을 고른다.
    let mut missing = Vec::new();
    for r in preview.changes.iter().filter(|ch| ch.field == "__row") {
        let line = r.line.to_string();
        let shown_row = (r.table == format!("{ty}_item"))
            .then(|| before_items.iter().find(|(k, _)| *k == line))
            .flatten();
        missing.push(match shown_row {
            Some((_, shown_row)) => {
                let sum = items.amount.and_then(|a| get(shown_row, a)).unwrap_or("");
                MirrorMissing {
                    table: r.table.clone(),
                    table_name: table_name(&r.table).into(),
                    line: r.line,
                    label: get_or(shown_row, items.name).into(),
                    amount: if sum.is_empty() {
                        get_or(shown_row, items.price).into()
                    } else {
                        sum.into()
                    },
                    choice_id: r.id.clone(),
                }
            }
            None => MirrorMissing {
                table: r.table.clone(),
                table_name: table_name(&r.table).into(),
                line: r.line,
                label: label_of(r.before.as_deref()),
                amount: String::new(),
                choice_id: r.id.clone(),
            },
        });
    }
    let kept: Vec<&CaptureChange> = preview
        .changes
        .iter()
        .filter(|ch| ch.table != ty && wipes(ch))
        .collect();
    let kept_in = distinct(
        kept.iter()
            .map(|ch| table_name(&ch.table).to_string())
            .collect(),
    )
    .join("·");

    let changes = fields.iter().filter(|f| f.kind == "changed").count() as i64 + item_changes;
    let status = if stored.is_empty() {
        "새 자료"
    } else if !exists {
        "새 차수"
    } else if !overrides.is_empty() {
        "검토 대기"
    } else if changes > 0 {
        "바뀐 칸"
    } else {
        "저장됨"
    };
    if !exists {
        rounds.push(MirrorRound {
            seq: doc.seq.clone(),
            amount: get_or(&after, amount_column(ty)).into(),
            saved_on: String::new(),
            state: "ghost".into(),
        });
    }
    let mut keyed = Vec::new();
    for round in rounds {
        keyed.push((seq_number(&round.seq)?, round));
    }
    keyed.sort_by_key(|(n, _)| *n);
    let rounds = keyed.into_iter().map(|(_, r)| r).collect();
    let user_columns = after.iter().filter(|(k, _)| users.contains(k)).count() as i64;

    Ok(MirrorShot {
        entity_type: ty.into(),
        kind: kind_name(ty).into(),
        number: display(&doc.seq),
        base: doc.base.clone(),
        seq: doc.seq.clone(),
        title: get_or(&after, title_column(ty)).into(),
        status: status.into(),
        changes,
        compare_seq: compare,
        latest_seq: latest,
        view: face.into(),
        view_columns: columns - user_columns,
        fields,
        rest,
        items: list,
        item_changes,
        item_rows,
        items_all_read: item_rows > 0 && preview.item_count == item_rows,
        place,
        rounds,
        base_token: preview.base_token.clone(),
        missing,
        kept_cells: kept.len() as i64,
        kept_in,
        totals_with_defaults: with_defaults,
        totals_without_missing: without_missing,
    })
}

/// `Mirror.TableName` — 딸린 표의 사람이 읽을 이름.
fn table_name(table: &str) -> &str {
    if table.ends_with("_item") {
        "품목"
    } else if table == "notice_schedule" {
        "일정"
    } else if table == "notice_officer_contact" {
        "담당자"
    } else if table == "notice_restricted_industry" {
        "제한 업종"
    } else if table.ends_with("_attachment") {
        "첨부"
    } else if table == "erp_partner" {
        "계약상대자"
    } else {
        table
    }
}

/// `Mirror.LabelOf` — 지울 줄의 원값(JSON)에서 그 줄을 알아볼 글 하나. 없으면 빈 문자열.
fn label_of(json_text: Option<&str>) -> String {
    let Some(json_text) = json_text.filter(|t| !t.is_empty()) else {
        return String::new();
    };
    let Ok(Value::Object(root)) = crate::json::parse(json_text) else {
        return String::new();
    };
    let values: Vec<(&str, String)> = root
        .iter()
        .filter(|(name, _)| *name != "seq" && *name != "line_no" && !name.ends_with("_base"))
        .filter_map(|(name, v)| {
            let text = match v {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                _ => return None,
            };
            (!is_blank(&text)).then_some((name.as_str(), text))
        })
        .collect();
    for name in [
        "item_name",
        "name",
        "file_name",
        "officer",
        "document_type",
        "department",
        "demand_agency",
    ] {
        if let Some((_, found)) = values.iter().find(|(n, _)| *n == name) {
            return found.clone();
        }
    }
    values
        .into_iter()
        .next()
        .map(|(_, t)| t)
        .unwrap_or_default()
}

/// `Mirror.Place` — 접수 → 공고 → 계약 의 자리. **지금 저장된 연결로만** 읽는다.
fn place_of(
    c: &Connection,
    ty: &str,
    base: &str,
    seq: &str,
    display: &str,
) -> Result<Vec<MirrorPlace>> {
    let latest = |kind: &str, b: &str| -> Result<Option<String>> {
        Ok(single_or_default(
            c,
            &format!(
                "SELECT seq FROM {kind} WHERE {kind}_base=?1 ORDER BY CAST(seq AS INTEGER) DESC LIMIT 1"
            ),
            &[b],
        )?
        .map(|s| EntityRef::new(kind, b, &s).display()))
    };
    let has = |kind: &str, b: &str| -> Result<bool> {
        let n: i64 = c.query_row(
            &format!("SELECT count(*) FROM {kind}_series WHERE {kind}_base=?1"),
            [b],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    };
    let group = match ty {
        "notice" => single_or_default(
            c,
            "SELECT group_base FROM notice_series WHERE notice_base=?1",
            &[base],
        )?,
        "contract" => single_or_default(
            c,
            "SELECT notice_group FROM project_link WHERE contract_base=?1",
            &[base],
        )?,
        _ => single_or_default(
            c,
            "SELECT notice_group FROM request_link WHERE request_base=?1",
            &[base],
        )?,
    };
    let here = |kind: &str| Some(MirrorPlace::new(kind, "here", display, true));
    let mut request = if ty == "request" {
        here("접수")
    } else {
        None
    };
    let mut notice = if ty == "notice" { here("공고") } else { None };
    let mut contract = if ty == "contract" {
        here("계약")
    } else {
        None
    };
    if let Some(group) = &group {
        if notice.is_none() {
            let current = match single_or_default(
                c,
                "SELECT 현행공고 FROM v_공고 WHERE 공고건=?1 AND 현행공고<>'' LIMIT 1",
                &[group],
            )? {
                Some(current) => current,
                None => {
                    let mut shown = Vec::new();
                    for b in texts(
                        c,
                        "SELECT notice_base FROM notice_series WHERE group_base=?1 ORDER BY notice_base DESC",
                        &[group],
                    )? {
                        if let Some(l) = latest("notice", &b.unwrap_or_default())? {
                            shown.push(l);
                        }
                    }
                    shown.join(" · ")
                }
            };
            if !current.is_empty() {
                notice = Some(MirrorPlace::new("공고", "linked", &current, true));
            }
        }
        if request.is_none()
            && let Some(r) = single_or_default(
                c,
                "SELECT request_base FROM request_link WHERE notice_group=?1",
                &[group],
            )?
            && let Some(shown) = latest("request", &r)?
        {
            request = Some(MirrorPlace::new("접수", "linked", &shown, true));
        }
        if contract.is_none() {
            let mut linked = Vec::new();
            for b in texts(
                c,
                "SELECT contract_base FROM project_link WHERE notice_group=?1 ORDER BY contract_base",
                &[group],
            )? {
                if let Some(l) = latest("contract", &b.unwrap_or_default())? {
                    linked.push(l);
                }
            }
            if !linked.is_empty() {
                contract = Some(MirrorPlace::new(
                    "계약",
                    "linked",
                    &linked.join(" · "),
                    true,
                ));
            }
        }
    }
    // 원천 참조. 계약은 공고·접수 번호를, 공고는 품목마다 접수 번호를 싣는다.
    if ty == "contract" {
        let mut statement = c.prepare(
            "SELECT ref_notice_base, ref_request_base FROM contract WHERE contract_base=?1 AND seq=?2",
        )?;
        let mut found = statement.query(params![base, seq])?;
        let (ref_notice, ref_request) = match found.next()? {
            Some(r) => (text_of(r.get_ref(0)?), text_of(r.get_ref(1)?)),
            None => (None, None),
        };
        if found.next()?.is_some() {
            return Err(Error::invalid("Sequence contains more than one element"));
        }
        if notice.is_none()
            && let Some(n) = ref_notice.filter(|n| !n.is_empty())
        {
            notice = Some(MirrorPlace::new("공고", "ref", &n, has("notice", &n)?));
        }
        if request.is_none()
            && let Some(r) = ref_request.filter(|r| !r.is_empty())
        {
            request = Some(MirrorPlace::new("접수", "ref", &r, has("request", &r)?));
        }
    }
    if ty == "notice" && request.is_none() {
        let refs = texts(
            c,
            "SELECT DISTINCT ref_request_base FROM notice_item WHERE notice_base=?1 AND seq=?2 AND coalesce(ref_request_base,'')<>''",
            &[base, seq],
        )?;
        if refs.len() == 1 {
            let r = refs[0].clone().unwrap_or_default();
            request = Some(MirrorPlace::new("접수", "ref", &r, has("request", &r)?));
        }
    }
    Ok(vec![
        request.unwrap_or_else(|| MirrorPlace::new("접수", "missing", "", false)),
        notice.unwrap_or_else(|| MirrorPlace::new("공고", "missing", "", false)),
        contract.unwrap_or_else(|| MirrorPlace::new("계약", "missing", "", false)),
    ])
}

/// `Mirror.From` — 「화면 · 이름표」. 머리 칸이면 그 칸의 이름표, 표에서 오면 그 표가 선 구역의 제목.
fn from_of(sources: &[String], screen: &[ScreenRow]) -> String {
    for source in sources.iter().filter(|s| !s.starts_with("table:")) {
        if let Some(r) = screen
            .iter()
            .find(|r| r.source == *source && !r.label.is_empty())
        {
            return r.label.clone();
        }
    }
    for source in sources.iter().filter(|s| s.starts_with("table:")) {
        if let Some(r) = screen.iter().find(|r| points(source, &r.source)) {
            return r.group.clone();
        }
    }
    String::new()
}

/// `Mirror.Shown` — 화면 표기. 그 칸이 화면에 보인 글이 계약면의 표기와 다를 때만. 머리 칸 하나에서 오는 열만.
fn shown(value: &str, sources: &[String], screen: &[ScreenRow]) -> Option<String> {
    let header: Vec<&String> = sources
        .iter()
        .filter(|s| !s.starts_with("table:"))
        .collect();
    if header.len() != 1 || header.len() != sources.len() {
        return None;
    }
    let text = screen
        .iter()
        .find(|r| r.source == *header[0])
        .map(|r| r.text.as_str())?;
    (!text.is_empty() && text != value).then(|| text.to_string())
}

/// `Mirror.ItemRows` — 스냅샷의 품목 표 행 수. 표가 없거나 하나로 정해지지 않으면 0.
fn item_rows_of(mapping: &MappingSet, input: &CaptureInput, ty: &str) -> i64 {
    let target = format!("{ty}_item");
    let Some(table) = mapping
        .profile(input.profile())
        .and_then(|p| p.tables_iter().find(|t| t.target() == target))
    else {
        return 0;
    };
    match &input.data {
        Some(data) => mapping::table(data, table.source()).map_or(0, |rows| rows.len() as i64),
        None => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points_follow_table_rows() {
        assert!(points("table:contract_item", "table:contract_item:3"));
        assert!(points("pointInfo.title", "pointInfo.title"));
        assert!(!points("", ""));
        assert!(!points("table:contract_item", "table:contract_items:1"));
        assert!(!points("pointInfo.a", "pointInfo.a:1"));
    }

    #[test]
    fn label_prefers_known_names() {
        assert_eq!(
            label_of(Some(
                r#"{"contract_base":"X","line_no":1,"spec":"규격","item_name":"볼펜"}"#
            )),
            "볼펜"
        );
        assert_eq!(label_of(Some(r#"{"seq":"1","amount":1000}"#)), "1000");
        assert_eq!(label_of(Some("[1]")), "");
        assert_eq!(label_of(None), "");
    }
}

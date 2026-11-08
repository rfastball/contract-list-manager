//! C# `Store`(Store.cs·StoreColumns.cs·StoreUserViews.cs·StoreViewPackages.cs·BatchOperations.cs) — 들어온 자료 위에서
//! 사람이 하는 일(사람 열·덮개·지우기·맞춤 뷰·보기 구성)과 키 해석.
//!
//! 자료를 들이는 길은 이 모듈에 없다(ADR-028). `user_field`·`field_override` 는 들이는 길이 쓰지 않는다 — 사람이
//! 적은 값이 다시 수집해도 지워지지 않게 하는 구조적 보장이다(ADR-007·020).
//!
//! **화면이 받는 꼴 그대로 낸다.** C# 의 `object` 를 내는 연산(익명 형식·Dapper 의 동적 행)은 [`Value`] 로, 레코드는
//! 형식으로 내고 `to_json` 이 다리(`Bridge.Json` — camelCase, 사전 열쇠는 그대로)가 쓰는 꼴을 짓는다. 시계와 식별자는
//! 모두 [`Env`] 로 짓는다 — `DateTime.UtcNow.ToString("O")` 는 [`Env::now_utc`], `Guid.NewGuid().ToString("N")` 은
//! [`Env::new_id`] 다.
//!
//! 시험 픽스처(`UpsertNotice`·`UpsertContract`·`UpsertRequest`)는 옮기지 않는다 — 프로덕션 호출자가 없고, 이 크레이트의
//! 시험은 SQL 픽스처로 자료를 세운다.

mod batch;
mod columns;
mod user_views;
mod view_packages;

use rusqlite::types::ValueRef;
use rusqlite::{Connection, Params, Transaction, TransactionBehavior, params};
use serde_json::{Map, Value};

use crate::domain::value_parser;
use crate::env::Env;
use crate::erp::explicit_links;
use crate::error::{Error, Result};
use crate::json::{self, Encoder};
use crate::storage::database::{self, Access, Database, refresh};
use crate::storage::flow::{Flow, FlowFormula};
use crate::storage::flow_check::FlowIssue;
use crate::storage::flow_compiler::{self, SELF, Target};
use crate::storage::views::{self, UserColumnDefinition};
use crate::storage::{computed_column, sql_column, user_view_builder};
use crate::text::{dotnet_double_r, eq_ignore_case, ignore_case_key, utf16_len};

pub use batch::{BatchDeletionPlan, CellChange};
pub use columns::FreezePlan;
pub use user_views::{USER_VIEW_EDGES, UserViewInfo};
pub use view_packages::{
    ViewPackage, ViewPackageColumn, ViewPackagePlannedColumn, ViewPackagePlannedView,
    ViewPackagePreview, ViewPackageView, builtin_view_package, example_view_package_v26,
    read_view_package, stored_definition,
};
pub(crate) use view_packages::{seed_example_views, upgrade_example_views};

/// C# `EntityRef(EntityType, Base, Seq)` — 접수·공고·계약 하나를 가리키는 것. **본번호와 차수가 실제 키다.**
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EntityRef {
    pub entity_type: String,
    pub base: String,
    pub seq: String,
}

impl EntityRef {
    pub fn new(entity_type: &str, base: &str, seq: &str) -> Self {
        EntityRef {
            entity_type: entity_type.to_string(),
            base: base.to_string(),
            seq: seq.to_string(),
        }
    }

    /// `EntityRef.Display` — 사람에게 보여줄 때 쓰는 한 덩어리 표기. 계약만 끝 두 자리를 그대로 붙인다.
    pub fn display(&self) -> String {
        if self.entity_type == "contract" {
            format!("{}{}", self.base, self.seq)
        } else {
            format!("{}-{}", self.base, self.seq)
        }
    }

    /// 다리가 쓰는 꼴(camelCase).
    pub fn to_json(&self) -> Value {
        self.json_with(["entityType", "base", "seq", "display"])
    }

    /// 옵션 없는 `JsonSerializer.Serialize` 의 꼴(이름 그대로) — 일괄 삭제의 지문이 쓴다.
    fn to_pascal_json(&self) -> Value {
        self.json_with(["EntityType", "Base", "Seq", "Display"])
    }

    fn json_with(&self, names: [&str; 4]) -> Value {
        object([
            (names[0], Value::String(self.entity_type.clone())),
            (names[1], Value::String(self.base.clone())),
            (names[2], Value::String(self.seq.clone())),
            (names[3], Value::String(self.display())),
        ])
    }
}

/// C# `FieldOverride(Base, Seq, ColumnName, Original)` — 사람이 고쳐 둔 칸 하나. `original` 은 처음 고칠 때 그 자리에
/// 있던 값이다.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldOverride {
    pub base: String,
    pub seq: String,
    pub column_name: String,
    pub original: String,
}

impl FieldOverride {
    pub fn to_json(&self) -> Value {
        object([
            ("base", Value::String(self.base.clone())),
            ("seq", Value::String(self.seq.clone())),
            ("columnName", Value::String(self.column_name.clone())),
            ("original", Value::String(self.original.clone())),
        ])
    }
}

/// C# `DeletionPlan` — 지우면 무엇이 함께 사라지는가. **묻기 전에 세어 보여 준다.**
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeletionPlan {
    pub entity_type: String,
    pub display: String,
    pub seq: String,
    pub title: String,
    /// 이 본번호로 쌓인 차수 전부. 낮은 것부터.
    pub revisions: Vec<String>,
    /// 계열까지 걷히는가 — 걷히면 사람이 적은 값과 링크도 함께 간다.
    pub series_goes: bool,
    pub child_rows: i32,
    pub user_fields: i32,
    pub overrides: i32,
    pub linked: bool,
}

impl DeletionPlan {
    pub fn to_json(&self) -> Value {
        object([
            ("entityType", Value::String(self.entity_type.clone())),
            ("display", Value::String(self.display.clone())),
            ("seq", Value::String(self.seq.clone())),
            ("title", Value::String(self.title.clone())),
            ("revisions", strings_json(&self.revisions)),
            ("seriesGoes", Value::Bool(self.series_goes)),
            ("childRows", self.child_rows.into()),
            ("userFields", self.user_fields.into()),
            ("overrides", self.overrides.into()),
            ("linked", Value::Bool(self.linked)),
        ])
    }
}

/// `UserColumnDefinition` 의 다리 꼴 — 계산 속성 `Choices` 까지 싣는다.
pub fn user_column_json(column: &UserColumnDefinition) -> Value {
    object([
        ("entityType", Value::String(column.entity_type.clone())),
        ("fieldName", Value::String(column.field_name.clone())),
        ("kind", Value::String(column.kind.clone())),
        (
            "options",
            column.options.clone().map_or(Value::Null, Value::String),
        ),
        ("sortOrder", column.sort_order.into()),
        ("choices", strings_json(&column.choices())),
    ])
}

/// 다리(`Bridge.Json` — 기본 인코더)가 쓰는 JSON 글. 대조 도구가 결과를 적을 때 쓴다.
pub fn bridge_json(value: &Value) -> String {
    json::to_compact(value, Encoder::Default)
}

/// [`Error`] 의 C# `Exception.Message` — `SqliteException` 은 `SQLite Error {코드}: '{글}'.` 꼴이다.
pub fn error_message(error: &Error) -> String {
    sql_column::error_message(error)
}

/// C# `Store` — 파일 하나 위의 편집. 연산마다 연결을 새로 연다(C# 과 같다).
pub struct Store<'e> {
    database: Database,
    env: &'e dyn Env,
}

/// 표본 실행에서 읽어 보는 줄 수(`ComputedSample`).
const COMPUTED_SAMPLE: usize = 20;

impl<'e> Store<'e> {
    /// `new Store(database)` — 시계와 식별자는 `env` 가 낸다.
    pub fn new(database: Database, env: &'e dyn Env) -> Self {
        Store { database, env }
    }

    pub fn database(&self) -> &Database {
        &self.database
    }

    // ── 사람 열 ──────────────────────────────────────────────

    /// `Store.UserColumns` — 사람이 채우는 열의 정의. `entity_type` 이 없으면 전부.
    pub fn user_columns(&self, entity_type: Option<&str>) -> Result<Vec<UserColumnDefinition>> {
        let connection = self.database.open()?;
        read_user_columns(
            &connection,
            "SELECT entity_type, field_name, kind, options, sort_order\n\
             FROM user_column\n\
             WHERE (?1 IS NULL OR entity_type = ?1)\n\
             ORDER BY entity_type, sort_order;",
            params![entity_type],
        )
    }

    /// `Store.AddUserColumn` — 사람이 채우는 열을 새로 만든다. 만들고 나면 뷰를 다시 지어 표에 곧바로 선다.
    pub fn add_user_column(
        &self,
        entity_type: &str,
        field_name: &str,
        kind: &str,
        choices: &[String],
    ) -> Result<()> {
        let name = clean_name(field_name)?;
        check_kind(kind)?;
        {
            let connection = self.database.open()?;
            check_free(&connection, entity_type, &name, None)?;
            connection.execute(
                "INSERT INTO user_column (entity_type, field_name, kind, options, sort_order)\n\
                 VALUES (?1, ?2, ?3, ?4,\n\
                         COALESCE((SELECT MAX(sort_order) FROM user_column\n\
                                   WHERE entity_type = ?1), 0) + 10);",
                params![entity_type, name, kind, pack(kind, choices)],
            )?;
        }
        self.database.refresh_views()
    }

    /// `Store.SaveComputedColumn` — 가공열을 저장한다. **화면을 믿지 않고 다시 검사한다**(§11.2): JSON 모양 → 꼴
    /// 검사 → 컴파일 → 그 열이 붙는 모든 뷰를 한 트랜잭션에서 다시 짓고 읽어 본다. 하나라도 실패하면 되돌린다.
    pub fn save_computed_column(
        &self,
        entity_type: &str,
        field_name: &str,
        json: &str,
        previous_name: Option<&str>,
    ) -> Result<()> {
        let name = clean_name(field_name)?;
        let mut formula = computed_column::parse_for(entity_type, json)?;
        if formula.source.as_deref() == Some("sql") {
            // 읽기 권한·모양·표본은 쓰기 전에 읽기 전용 연결에서 본다. 순환은 뷰에 넣어 본 뒤에야 드러난다(아래).
            let reader = sql_column::open_checked(&self.database)?;
            sql_column::validate(
                &reader,
                entity_type,
                formula.sql.as_deref().unwrap_or(""),
                &[],
            )?;
        } else {
            flow_compiler::expression(entity_type, &formula, Target::Internal, None)?;
        }
        let connection = self.database.open()?;
        let transaction = database::begin(&connection)?;
        check_free(&transaction, entity_type, &name, previous_name)?;
        if let Some(previous) = previous_name {
            // 입력열을 주면 가공열로 바뀐다(§3.1) — 적힌 값은 지우지 않고 식에 가려 숨는다. 굳혀 둔 식이 있으면 그
            // 식별자를 이어 쓴다: 소비자에 저장된 필드 연결이 끊기지 않는다.
            let old = single_or_default(
                &transaction,
                "SELECT kind, options FROM user_column WHERE entity_type=?1 AND field_name=?2",
                params![entity_type, previous],
                |r| Ok((text_of(r.get_ref(0)?), text_of(r.get_ref(1)?))),
            )?
            .ok_or_else(|| Error::invalid("수정할 열이 없습니다."))?;
            let mut id = None;
            if let Some(stored) = old.1.as_deref()
                && stored.trim_start().starts_with('{')
            {
                match Flow::read_for_identity(stored) {
                    Ok(f) => id = f.id,
                    Err(Error::InvalidOperation(_)) => {}
                    Err(e) => return Err(e),
                }
            }
            formula.id = Some(match id {
                Some(id) if !id.is_empty() => id,
                _ => self.env.new_id(),
            });
        } else {
            formula.id = Some(self.env.new_id());
        }
        let options = Flow::serialize(&formula)?;
        match previous_name {
            None => transaction.execute(
                "INSERT INTO user_column(entity_type,field_name,kind,options,sort_order)\n\
                 VALUES(?1,?2,'computed',?3,\n\
                   coalesce((SELECT max(sort_order) FROM user_column WHERE entity_type=?1),0)+10)",
                params![entity_type, name, options],
            )?,
            Some(previous) => transaction.execute(
                "UPDATE user_column SET field_name=?2,kind='computed',options=?3\n\
                 WHERE entity_type=?1 AND field_name=?4",
                params![entity_type, name, options, previous],
            )?,
        };
        if let Some(previous) = previous_name
            && previous != name
        {
            let (table, _) = table_for(entity_type)?;
            transaction.execute(
                &format!("UPDATE {table} SET field_name = ?1 WHERE field_name = ?2;"),
                params![name, previous],
            )?;
            // 그 열을 읽던 맞춤 뷰 필드도 새 이름을 따라간다(docs/user-views.md §5).
            user_view_builder::rename_field(&transaction, entity_type, previous, &name)?;
        }
        let checked = (|| -> Result<()> {
            // 깨진 열은 뷰를 막지 않고 빈 글로 선다 — 저장하려는 이 열이 그렇게 되면 거절한다.
            let broken = refresh(&transaction)?;
            if let Some(why) = broken.columns.get(&(entity_type.to_string(), name.clone())) {
                return Err(Error::InvalidOperation(
                    match self.locate_computed_failure(entity_type, &formula)? {
                        Some(located) => located,
                        None if why.is_empty() => "가공식을 계산하지 못했습니다.".into(),
                        None => why.clone(),
                    },
                ));
            }
            for view in computed_views(entity_type)? {
                read_all(
                    &transaction,
                    &format!(
                        "SELECT \"{}\" FROM \"{view}\" LIMIT {COMPUTED_SAMPLE};",
                        name.replace('"', "\"\"")
                    ),
                )?;
            }
            Ok(())
        })();
        match checked {
            Ok(()) => {}
            Err(Error::Sqlite(error)) => {
                return Err(Error::InvalidOperation(
                    match self.locate_computed_failure(entity_type, &formula)? {
                        Some(located) => located,
                        None => format!(
                            "가공식을 계산하지 못했습니다: {}",
                            sql_column::dotnet_message(&error)
                        ),
                    },
                ));
            }
            Err(other) => return Err(other),
        }
        transaction.commit()?;
        Ok(())
    }

    /// `Store.LocateComputedFailure` — 실행에서 실패한 식이 몇 단계에서 깨지는지 찾는다. 앞부분을 하나씩 늘려 지금
    /// 저장된 면 뷰의 표본 위에서 돌려 본다. 쓰지 않는 읽기 연결이라 저장하던 트랜잭션과 섞이지 않는다.
    fn locate_computed_failure(
        &self,
        entity_type: &str,
        formula: &FlowFormula,
    ) -> Result<Option<String>> {
        let Some(flow) = &formula.flow else {
            return Ok(None);
        };
        let connection = self.database.open_read_only()?;
        let steps = flow.steps.as_ref().map_or(0, Vec::len);
        for k in 0..=steps {
            let part = flow_compiler::prefix(entity_type, flow, k, Target::Internal, None)?;
            let probe = if part.is_relation() {
                format!("(SELECT count(*) FROM ({}))", part.sql)
            } else {
                part.sql.clone()
            };
            let sql = format!(
                "SELECT {probe} FROM \"{}\" AS {SELF} LIMIT {COMPUTED_SAMPLE};",
                face_view(entity_type)
            );
            match read_all(&connection, &sql) {
                Ok(()) => {}
                Err(Error::Sqlite(error)) => {
                    return Ok(Some(
                        FlowIssue {
                            index: k,
                            message: format!(
                                "계산하지 못했습니다 — {}",
                                sql_column::dotnet_message(&error)
                            ),
                            r#final: false,
                        }
                        .describe(),
                    ));
                }
                Err(other) => return Err(other),
            }
        }
        Ok(None)
    }

    /// `Store.PreviewComputedColumn` — 표본(최신 문서 다섯)마다 이 식이 낼 값. v1·v2 아무것이나 받는다.
    pub fn preview_computed_column(&self, entity_type: &str, json: &str) -> Result<Value> {
        let expression = computed_column::expression(
            entity_type,
            &computed_column::parse_for(entity_type, json)?,
            None,
        )?;
        let separator = if entity_type == "contract" { "" } else { "-" };
        let connection = self.database.open_read_only()?;
        let sql = format!(
            "SELECT p.{e}_base AS base, p.seq AS revision,\n\
                    coalesce(p.title,'') AS title,\n\
                    coalesce((SELECT {expression} FROM \"{face}\" AS {SELF}\n\
                              WHERE {SELF}.\"{key}\" = p.{e}_base || '{separator}' || p.seq), '') AS value\n\
             FROM ({parents}) p ORDER BY p.{e}_base",
            e = entity_type,
            face = face_view(entity_type),
            key = face_key(entity_type),
            parents = latest_column_parents(entity_type)?,
        );
        Ok(Value::Array(rows_json(&connection, &sql, [])?))
    }

    /// `Store.ColumnContext` — 열 정의 작업공간의 실제 부모 표와 같은 차수 품목. 읽기 전용 사진 한 벌만 읽는다.
    pub fn column_context(&self, entity_type: &str) -> Result<Value> {
        let fields = computed_column::field_names(entity_type)?;
        let parents_sql = latest_column_parents(entity_type)?;
        let view = face_view(entity_type);
        let connection = self.database.open_read_only()?;
        let _transaction = begin_deferred(&connection)?;
        let columns = view_columns(&connection, view)?;
        let separator = if entity_type == "contract" { "" } else { "-" };
        let rows = rows_json(
            &connection,
            &format!(
                "SELECT v.* FROM \"{view}\" v JOIN ({parents_sql}) p\n\
                   ON v.\"{key}\"=p.{e}_base || '{separator}' || p.seq\n\
                 ORDER BY p.{e}_base",
                key = face_key(entity_type),
                e = entity_type,
            ),
            [],
        )?;
        let parents = query(
            &connection,
            &format!(
                "SELECT p.{e}_base, p.seq, coalesce(p.title,'') FROM ({parents_sql}) p ORDER BY p.{e}_base",
                e = entity_type
            ),
            [],
            |r| {
                Ok((
                    text_of(r.get_ref(0)?),
                    text_of(r.get_ref(1)?),
                    text_of(r.get_ref(2)?),
                ))
            },
        )?;
        let source_sql = computed_column::source_sql(entity_type, "@Base", "@Revision")?;
        let field_names = fields
            .iter()
            .map(|f| format!("\"{f}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let mut sources = Vec::new();
        for (base, revision, title) in parents {
            let args = rusqlite::named_params! {"@Base": base, "@Revision": revision};
            let rows = rows_json(
                &connection,
                &format!("SELECT {field_names} FROM ({source_sql}) ORDER BY __order LIMIT 100;"),
                args,
            )?;
            let total = scalar_i64(
                &connection,
                &format!("SELECT COUNT(*) FROM ({source_sql});"),
                args,
            )? as i32;
            sources.push(object([
                ("base", opt_string(base)),
                ("revision", opt_string(revision)),
                ("title", opt_string(title)),
                ("rows", Value::Array(rows)),
                ("totalRows", total.into()),
            ]));
        }
        Ok(object([
            ("columns", strings_json(&columns)),
            ("rows", Value::Array(rows)),
            ("fields", strings_json(&fields)),
            ("sources", Value::Array(sources)),
            ("available", available(&connection, entity_type)?),
        ]))
    }

    /// `Store.UpdateUserColumn` — 열 하나를 고친다. 이름을 바꾸면 **적어 둔 값도 함께 옮긴다** — 값은 열 이름으로
    /// 매달려 있어서, 정의만 고치면 적어 둔 것이 통째로 미아가 된다.
    pub fn update_user_column(
        &self,
        entity_type: &str,
        field_name: &str,
        new_name: &str,
        kind: &str,
        choices: &[String],
    ) -> Result<()> {
        let name = clean_name(new_name)?;
        check_kind(kind)?;
        {
            let connection = self.database.open()?;
            check_exists(&connection, entity_type, field_name)?;
            refuse_computed_write(&connection, entity_type, field_name)?;
            check_free(&connection, entity_type, &name, Some(field_name))?;

            let transaction = database::begin(&connection)?;
            // 굳혀 둔 식(값으로 굳히기가 비활성으로 남긴 가공식)은 고르기가 후보로 덮기 전까지 남긴다 — 다시 켤 수 있게.
            let kept = single_or_default(
                &transaction,
                "SELECT options FROM user_column WHERE entity_type = ?1 AND field_name = ?2",
                params![entity_type, field_name],
                |r| Ok(text_of(r.get_ref(0)?)),
            )?
            .flatten();
            let options = pack(kind, choices)
                .or_else(|| kept.filter(|k| kind != "choice" && k.trim_start().starts_with('{')));
            transaction.execute(
                "UPDATE user_column SET field_name = ?3, kind = ?4, options = ?5\n\
                 WHERE entity_type = ?1 AND field_name = ?2;",
                params![entity_type, field_name, name, kind, options],
            )?;
            if name != field_name {
                let (table, _) = table_for(entity_type)?;
                transaction.execute(
                    &format!("UPDATE {table} SET field_name = ?1 WHERE field_name = ?2;"),
                    params![name, field_name],
                )?;
                // 그 열을 읽던 맞춤 뷰 필드도 새 이름을 따라간다(docs/user-views.md §5).
                user_view_builder::rename_field(&transaction, entity_type, field_name, &name)?;
            }
            transaction.commit()?;
        }
        self.database.refresh_views()
    }

    /// `Store.RemoveUserColumn` — 열을 없앤다. **적어 둔 값은 남긴다** — 같은 이름으로 다시 만들면 그대로 되살아난다
    /// (ADR-007).
    pub fn remove_user_column(&self, entity_type: &str, field_name: &str) -> Result<()> {
        let connection = self.database.open()?;
        check_exists(&connection, entity_type, field_name)?;
        let transaction = database::begin(&connection)?;
        transaction.execute(
            "DELETE FROM user_column WHERE entity_type = ?1 AND field_name = ?2;",
            params![entity_type, field_name],
        )?;
        refresh(&transaction)?;
        transaction.commit()?;
        Ok(())
    }

    /// `Store.MoveUserColumn` — 열의 차례를 한 칸 옮긴다(`delta` 가 음수면 앞으로). 옮긴 뒤 **번호를 다시 매긴다** —
    /// 같은 `sort_order` 를 가진 열이 있으면 맞바꾸기만으로는 차례가 정해지지 않는다.
    pub fn move_user_column(&self, entity_type: &str, field_name: &str, delta: i32) -> Result<()> {
        {
            let connection = self.database.open()?;
            check_exists(&connection, entity_type, field_name)?;
            let mut order = strings(
                &connection,
                "SELECT field_name FROM user_column\n\
                 WHERE entity_type = ?1 ORDER BY sort_order, field_name;",
                [entity_type],
            )?;
            let at = order
                .iter()
                .position(|f| f == field_name)
                .map_or(-1, |i| i as i64);
            let to = (at + i64::from(delta)).clamp(0, order.len() as i64 - 1);
            if at == to {
                return Ok(());
            }
            let moved = order.remove(at as usize);
            order.insert(to as usize, moved);
            let transaction = database::begin(&connection)?;
            for (i, name) in order.iter().enumerate() {
                transaction.execute(
                    "UPDATE user_column SET sort_order = ?3\n\
                     WHERE entity_type = ?1 AND field_name = ?2;",
                    params![entity_type, name, (i as i64 + 1) * 10],
                )?;
            }
            transaction.commit()?;
        }
        self.database.refresh_views()
    }

    // ── 사람 값 ──────────────────────────────────────────────

    /// `Store.SetUserField` — 사람이 적은 값을 넣는다. 값은 **차수가 아니라 계열**에 붙는다(ADR-012). 빈 값이면 지운다.
    pub fn set_user_field(
        &self,
        entity: &EntityRef,
        field_name: &str,
        value: Option<&str>,
    ) -> Result<()> {
        let connection = self.database.open()?;
        set_user_field_on(&connection, entity, field_name, value, self.env)
    }

    // ── 파서가 읽은 값을 사람이 고치기 ───────────────────────────
    // 사람 값과 나란히 서지만 매다는 자리가 다르다 — 이쪽은 "이 문서의 이 칸" 에 대한 정정이라 차수에 붙는다.

    /// `Store.SetOverride` — 계약면 칸 하나를 사람이 고친다. 공고종류를 고치면 공고 건과 연결을 다시 짓는다.
    pub fn set_override(&self, entity: &EntityRef, column_name: &str, value: &str) -> Result<()> {
        let connection = self.database.open()?;
        let transaction = database::begin(&connection)?;
        set_override_on(&transaction, entity, column_name, value, None, self.env)?;
        if entity.entity_type == "notice" && column_name == "공고종류" {
            explicit_links::resolve(&transaction)?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// `Store.ClearOverride` — 고친 칸을 수집 원값으로 되돌린다(그 차수의 덮개를 지운다).
    pub fn clear_override(&self, entity: &EntityRef, column_name: &str) -> Result<()> {
        let connection = self.database.open()?;
        let transaction = database::begin(&connection)?;
        clear_override_on(&transaction, entity, column_name)?;
        if entity.entity_type == "notice" && column_name == "공고종류" {
            explicit_links::resolve(&transaction)?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// `Store.CheckCorrectable` — 이 칸을 덮개로 고칠 수 있는가. 키 열·가공열·사람 열·면 뷰에 없는 열은 거절한다.
    pub fn check_correctable(&self, entity: &EntityRef, column_name: &str) -> Result<()> {
        let connection = self.database.open()?;
        check_correctable_on(&connection, entity, column_name)
    }

    /// `Store.FaceValue` — 면 뷰가 지금 그 칸에 내는 값(덮개를 씌운 뒤의 것). 없으면 빈 글.
    pub fn face_value(&self, entity: &EntityRef, column_name: &str) -> Result<String> {
        let connection = self.database.open_read_only()?;
        face_value_on(&connection, entity, column_name)
    }

    /// `Store.Overrides` — 이 종류에 걸린 덮개 전부.
    pub fn overrides(&self, entity_type: &str) -> Result<Vec<FieldOverride>> {
        let connection = self.database.open()?;
        query(
            &connection,
            "SELECT base, seq, column_name, original\n\
             FROM field_override WHERE entity_type = ?1;",
            [entity_type],
            |r| {
                Ok(FieldOverride {
                    base: text_of(r.get_ref(0)?).unwrap_or_default(),
                    seq: text_of(r.get_ref(1)?).unwrap_or_default(),
                    column_name: text_of(r.get_ref(2)?).unwrap_or_default(),
                    original: text_of(r.get_ref(3)?).unwrap_or_default(),
                })
            },
        )
    }

    // ── 지우기 ───────────────────────────────────────────────────

    /// `Store.PlanDeletion` — 지우면 무엇이 함께 사라지는가를 센다. 지우기 전에 반드시 이것을 보인다.
    pub fn plan_deletion(&self, entity: &EntityRef, whole_series: bool) -> Result<DeletionPlan> {
        let connection = self.database.open()?;
        plan_deletion_on(&connection, entity, whole_series)
    }

    /// `Store.Delete` — 지운다. 되돌릴 수 없다 — 묘비를 두지 않는다.
    pub fn delete(&self, entity: &EntityRef, whole_series: bool) -> Result<()> {
        let connection = self.database.open()?;
        let transaction = database::begin(&connection)?;
        delete_on(&transaction, entity, whole_series, true)?;
        transaction.commit()?;
        Ok(())
    }

    /// `Store.Resolve` — 사람이 적은 번호 하나를 레코드로. 계약번호 → 공고번호 → 접수번호 → 아무 조달요구번호 차례다.
    pub fn resolve(&self, key: &str) -> Result<Option<EntityRef>> {
        let connection = self.database.open()?;
        resolve_on(&connection, key)
    }
}

// ── 연산 몸(연결 위) ─────────────────────────────────────────

fn set_user_field_on(
    connection: &Connection,
    entity: &EntityRef,
    field_name: &str,
    value: Option<&str>,
    env: &dyn Env,
) -> Result<()> {
    let (table, base_column) = table_for(&entity.entity_type)?;
    refuse_computed_write(connection, &entity.entity_type, field_name)?;
    match value {
        None | Some("") => {
            // 비우기는 줄을 지운다 — 없는 값을 비워도 오류가 아니다. 뷰는 줄이 없는 칸을 빈 문자열로 낸다.
            connection.execute(
                &format!("DELETE FROM {table} WHERE {base_column} = ?1 AND field_name = ?2;"),
                params![entity.base, field_name],
            )?;
        }
        Some(value) => {
            connection.execute(
                &format!(
                    "INSERT INTO {table} ({base_column}, field_name, value, updated_at)\n\
                     VALUES (?1, ?2, ?3, ?4)\n\
                     ON CONFLICT({base_column}, field_name) DO UPDATE SET\n\
                         value = excluded.value, updated_at = excluded.updated_at;"
                ),
                params![entity.base, field_name, value, env.now_utc()],
            )?;
        }
    }
    Ok(())
}

fn set_override_on(
    connection: &Connection,
    entity: &EntityRef,
    column_name: &str,
    value: &str,
    original_value: Option<&str>,
    env: &dyn Env,
) -> Result<()> {
    check_correctable_on(connection, entity, column_name)?;
    let original = match original_value {
        Some(o) => o.to_string(),
        None => face_value_on(connection, entity, column_name)?,
    };
    connection.execute(
        "INSERT INTO field_override\n\
             (entity_type, base, seq, column_name, value, original, updated_at)\n\
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)\n\
         ON CONFLICT(entity_type, base, seq, column_name) DO UPDATE SET\n\
             value = excluded.value, updated_at = excluded.updated_at;",
        params![
            entity.entity_type,
            entity.base,
            entity.seq,
            column_name,
            value,
            original,
            env.now_utc()
        ],
    )?;
    Ok(())
}

fn clear_override_on(connection: &Connection, entity: &EntityRef, column_name: &str) -> Result<()> {
    check_correctable_on(connection, entity, column_name)?;
    connection.execute(
        "DELETE FROM field_override\n\
         WHERE entity_type = ?1 AND base = ?2 AND seq = ?3\n\
           AND column_name = ?4;",
        params![entity.entity_type, entity.base, entity.seq, column_name],
    )?;
    Ok(())
}

fn check_correctable_on(
    connection: &Connection,
    entity: &EntityRef,
    column_name: &str,
) -> Result<()> {
    if views::KEY_COLUMNS.contains(&column_name) {
        return Err(Error::InvalidOperation(format!(
            "레코드를 가리키는 열이라 고칠 수 없습니다: {column_name}"
        )));
    }
    refuse_computed_write(connection, &entity.entity_type, column_name)?;
    if scalar_i64(
        connection,
        "SELECT COUNT(*) FROM user_column\n\
         WHERE entity_type = ?1 AND field_name = ?2;",
        params![entity.entity_type, column_name],
    )? > 0
    {
        return Err(Error::InvalidOperation(format!(
            "손으로 채우는 열입니다: {column_name}"
        )));
    }
    if !view_columns(connection, face_view(&entity.entity_type))?
        .iter()
        .any(|c| c == column_name)
    {
        return Err(Error::InvalidOperation(format!(
            "계약면에 없는 열입니다: {column_name}"
        )));
    }
    Ok(())
}

fn face_value_on(connection: &Connection, entity: &EntityRef, column_name: &str) -> Result<String> {
    let sql = format!(
        "SELECT \"{}\" FROM \"{}\" WHERE \"{}\" = $key;",
        column_name.replace('"', "\"\""),
        face_view(&entity.entity_type),
        face_key(&entity.entity_type)
    );
    let mut statement = connection.prepare(&sql)?;
    let mut rows = statement.query(rusqlite::named_params! {"$key": entity.display()})?;
    Ok(match rows.next()? {
        Some(row) => text_of(row.get_ref(0)?).unwrap_or_default(),
        None => String::new(),
    })
}

fn plan_deletion_on(
    connection: &Connection,
    entity: &EntityRef,
    whole_series: bool,
) -> Result<DeletionPlan> {
    let shape = shape(&entity.entity_type)?;
    let base_column = shape.base_column;
    let revisions = strings(
        connection,
        &format!(
            "SELECT seq FROM {} WHERE {base_column} = @Base ORDER BY CAST(seq AS INTEGER);",
            shape.head
        ),
        rusqlite::named_params! {"@Base": entity.base},
    )?;
    if revisions.is_empty() {
        return Err(Error::InvalidOperation(format!(
            "그런 접수·공고·계약이 없습니다: {}",
            entity.display()
        )));
    }
    // 마지막 차수를 지우면 계열은 남을 자리가 없다 — 사람 값과 링크가 미아가 되기 전에 함께 걷는다.
    let series_goes = whole_series || revisions.len() <= 1;
    let mut child_rows = 0i32;
    for table in shape.children {
        child_rows += if whole_series {
            scalar_i64(
                connection,
                &format!("SELECT COUNT(*) FROM {table} WHERE {base_column} = @Base;"),
                rusqlite::named_params! {"@Base": entity.base},
            )?
        } else {
            scalar_i64(
                connection,
                &format!(
                    "SELECT COUNT(*) FROM {table} WHERE {base_column} = @Base AND seq = @Seq;"
                ),
                rusqlite::named_params! {"@Base": entity.base, "@Seq": entity.seq},
            )?
        } as i32;
    }
    let title = single_or_default(
        connection,
        &format!(
            "SELECT title FROM {} WHERE {base_column} = @Base AND seq = @Seq;",
            shape.head
        ),
        rusqlite::named_params! {"@Base": entity.base, "@Seq": entity.seq},
        |r| Ok(text_of(r.get_ref(0)?)),
    )?
    .flatten()
    .unwrap_or_default();
    let user_fields = if series_goes {
        scalar_i64(
            connection,
            &format!(
                "SELECT COUNT(*) FROM {} WHERE {base_column} = @Base;",
                shape.user_fields
            ),
            rusqlite::named_params! {"@Base": entity.base},
        )? as i32
    } else {
        0
    };
    let overrides = if whole_series {
        scalar_i64(
            connection,
            "SELECT COUNT(*) FROM field_override\n\
             WHERE entity_type = @EntityType AND base = @Base;",
            rusqlite::named_params! {"@EntityType": entity.entity_type, "@Base": entity.base},
        )?
    } else {
        scalar_i64(
            connection,
            "SELECT COUNT(*) FROM field_override\n\
             WHERE entity_type = @EntityType AND base = @Base AND seq = @Seq;",
            rusqlite::named_params! {"@EntityType": entity.entity_type, "@Base": entity.base, "@Seq": entity.seq},
        )?
    } as i32;
    let linked = series_goes && link_count(connection, entity, base_column)? > 0;
    Ok(DeletionPlan {
        entity_type: entity.entity_type.clone(),
        display: entity.display(),
        seq: entity.seq.clone(),
        title,
        revisions,
        series_goes,
        child_rows,
        user_fields,
        overrides,
        linked,
    })
}

/// `Store.LinkCount` — 계열이 걷히면 함께 걷히는 연결 수. 공고는 건(`notice_group`)에 매달린 연결을 센다.
fn link_count(connection: &Connection, entity: &EntityRef, base_column: &str) -> Result<i64> {
    if entity.entity_type == "notice" {
        scalar_i64(
            connection,
            "SELECT\n\
                 (SELECT COUNT(*) FROM project_link l\n\
                  JOIN notice_series s ON s.group_base = l.notice_group\n\
                  WHERE s.notice_base = ?1)\n\
               + (SELECT COUNT(*) FROM request_link l\n\
                  JOIN notice_series s ON s.group_base = l.notice_group\n\
                  WHERE s.notice_base = ?1);",
            [&entity.base],
        )
    } else {
        let table = if entity.entity_type == "request" {
            "request_link"
        } else {
            "project_link"
        };
        scalar_i64(
            connection,
            &format!("SELECT COUNT(*) FROM {table} WHERE {base_column} = ?1;"),
            [&entity.base],
        )
    }
}

/// `Store.Delete(connection, transaction, entity, wholeSeries, rebuild)` — [`plan_deletion_on`] 이 센 것을 걷는다.
fn delete_on(
    connection: &Connection,
    entity: &EntityRef,
    whole_series: bool,
    rebuild: bool,
) -> Result<()> {
    let plan = plan_deletion_on(connection, entity, whole_series)?;
    let shape = shape(&entity.entity_type)?;
    let base_column = shape.base_column;
    if whole_series {
        connection.execute(
            "DELETE FROM field_override\nWHERE entity_type = ?1 AND base = ?2;",
            params![entity.entity_type, entity.base],
        )?;
        connection.execute(
            &format!("DELETE FROM {} WHERE {base_column} = ?1;", shape.head),
            [&entity.base],
        )?;
    } else {
        connection.execute(
            "DELETE FROM field_override\nWHERE entity_type = ?1 AND base = ?2 AND seq = ?3;",
            params![entity.entity_type, entity.base, entity.seq],
        )?;
        connection.execute(
            &format!(
                "DELETE FROM {} WHERE {base_column} = ?1 AND seq = ?2;",
                shape.head
            ),
            params![entity.base, entity.seq],
        )?;
    }
    // 계열이 걷혀야 사람 값·링크·거부가 캐스케이드로 따라간다.
    if plan.series_goes {
        connection.execute(
            &format!("DELETE FROM {} WHERE {base_column} = ?1;", shape.series),
            [&entity.base],
        )?;
    }
    // 아무 계약도 가리키지 않게 된 상대자. 남겨 두면 status 의 상대자 수가 실물과 어긋난다.
    if entity.entity_type == "contract" {
        connection.execute(
            "DELETE FROM counterparty\n\
             WHERE business_number NOT IN\n\
                 (SELECT counterparty_number FROM contract WHERE counterparty_number IS NOT NULL);",
            [],
        )?;
    }
    // 공고가 걷히면 건이 갈리거나 통째로 사라진다 — 링크는 건에 매달려 있어 여기서 다시 짓는다.
    if rebuild {
        explicit_links::resolve(connection)?;
    }
    Ok(())
}

fn resolve_on(connection: &Connection, key: &str) -> Result<Option<EntityRef>> {
    if let Some((base, seq)) = value_parser::contract_number(Some(key))
        && scalar_i64(
            connection,
            "SELECT COUNT(*) FROM contract WHERE contract_base = ?1 AND seq = ?2;",
            params![base, seq],
        )? > 0
    {
        return Ok(Some(EntityRef::new("contract", &base, &seq)));
    }
    if let Some((base, seq)) = value_parser::notice_number(Some(key))
        && scalar_i64(
            connection,
            "SELECT COUNT(*) FROM notice WHERE notice_base = ?1 AND seq = ?2;",
            params![base, seq],
        )? > 0
    {
        return Ok(Some(EntityRef::new("notice", &base, &seq)));
    }
    // 접수번호. 있는 행과 글자 그대로 맞춘다.
    let key = key.trim();
    let request = single_or_default(
        connection,
        "SELECT request_base, seq FROM request WHERE request_base || '-' || seq = ?1;",
        [key],
        |r| Ok((text_of(r.get_ref(0)?), text_of(r.get_ref(1)?))),
    )?;
    if let Some((Some(base), seq)) = request {
        return Ok(Some(EntityRef {
            entity_type: "request".into(),
            base,
            seq: seq.unwrap_or_default(),
        }));
    }
    // 아무 조달요구번호. 그 계열의 최신 차수를 준다 — 사람이 가리킨 것은 조달요구이지 특정 차수의 접수가 아니다.
    Ok(query(
        connection,
        "SELECT DISTINCT i.request_base, i.seq FROM request_item i\n\
         JOIN (SELECT request_base AS b, MAX(CAST(seq AS INTEGER)) AS s\n\
               FROM request GROUP BY request_base) latest\n\
           ON i.request_base = latest.b AND CAST(i.seq AS INTEGER) = latest.s\n\
         WHERE i.request_number IN (?1)\n\
         ORDER BY i.request_base;",
        [key],
        |r| {
            Ok(EntityRef {
                entity_type: "request".into(),
                base: text_of(r.get_ref(0)?).unwrap_or_default(),
                seq: text_of(r.get_ref(1)?).unwrap_or_default(),
            })
        },
    )?
    .into_iter()
    .next())
}

// ── 열 검사 ───────────────────────────────────────────────

/// `Store.RefuseComputedWrite` — 가공열의 값은 직접 고칠 수 없다.
fn refuse_computed_write(
    connection: &Connection,
    entity_type: &str,
    field_name: &str,
) -> Result<()> {
    if scalar_i64(
        connection,
        "SELECT count(*) FROM user_column WHERE entity_type=?1 AND field_name=?2 AND kind='computed'",
        params![entity_type, field_name],
    )? > 0
    {
        return Err(Error::invalid(
            "가공열의 값은 직접 고칠 수 없습니다. 열 관리에서 가공식을 수정하세요.",
        ));
    }
    Ok(())
}

/// `Store.Pack` — 후보는 `choice` 일 때만 담는다.
fn pack(kind: &str, choices: &[String]) -> Option<String> {
    if kind != "choice" {
        return None;
    }
    let mut clean: Vec<&str> = Vec::new();
    for c in choices.iter().map(|c| c.trim()) {
        if !c.is_empty() && !c.contains('|') && !clean.contains(&c) {
            clean.push(c);
        }
    }
    (!clean.is_empty()).then(|| clean.join("|"))
}

/// `Store.CheckKind`.
fn check_kind(kind: &str) -> Result<()> {
    if !matches!(kind, "text" | "choice" | "date" | "number") {
        return Err(Error::InvalidOperation(format!(
            "모르는 종류입니다: {kind}"
        )));
    }
    Ok(())
}

/// `Store.CleanName` — 열 이름을 다듬고 쓸 수 있는지 본다. **이름이 SQL 에 그대로 박히고 뷰는 열릴 때마다 다시
/// 지어지므로** 따옴표가 섞인 이름은 앱이 뜨지 못하게 만든다 — 다리로 직접 부를 수 있으니 여기서 막는다.
fn clean_name(field_name: &str) -> Result<String> {
    let name = field_name.trim();
    if name.is_empty() {
        return Err(Error::invalid("열 이름이 비었습니다."));
    }
    if utf16_len(name) > 20 {
        return Err(Error::invalid("열 이름은 20자까지입니다."));
    }
    if name
        .chars()
        .any(|ch| matches!(ch, '\'' | '"' | '\\') || ch.is_control())
    {
        return Err(Error::invalid(
            "열 이름에 따옴표나 역슬래시를 쓸 수 없습니다.",
        ));
    }
    Ok(name.to_string())
}

/// `Store.CheckExists`.
fn check_exists(connection: &Connection, entity_type: &str, field_name: &str) -> Result<()> {
    if scalar_i64(
        connection,
        "SELECT COUNT(*) FROM user_column\n\
         WHERE entity_type = ?1 AND field_name = ?2;",
        params![entity_type, field_name],
    )? == 0
    {
        return Err(Error::InvalidOperation(format!(
            "그런 열이 없습니다: {field_name}"
        )));
    }
    Ok(())
}

/// `Store.CheckFree` — 이 이름을 써도 되는가. 뷰가 이미 내는 이름과 부딪히면 SQLite 가 조용히 뒤엣것을 `이름:1` 로
/// 바꾸므로, 뷰가 실제로 내는 열 이름을 읽어 견준다. 계약은 `v_통합`·`v_통합차수` 를 함께 본다.
fn check_free(
    connection: &Connection,
    entity_type: &str,
    name: &str,
    allow: Option<&str>,
) -> Result<()> {
    let mine = strings(
        connection,
        "SELECT field_name FROM user_column WHERE entity_type = ?1;",
        [entity_type],
    )?;
    if !allow.is_some_and(|a| eq_ignore_case(name, a))
        && mine.iter().any(|m| eq_ignore_case(m, name))
    {
        return Err(Error::InvalidOperation(format!(
            "이미 있는 열입니다: {name}"
        )));
    }
    let views: &[&str] = match entity_type {
        "contract" => &["v_통합", "v_통합차수"],
        "request" => &["v_접수"],
        _ => &["v_공고"],
    };
    let key = ignore_case_key(name);
    for view in views {
        for column in view_columns(connection, view)? {
            if !mine.contains(&column) && ignore_case_key(&column) == key {
                return Err(Error::InvalidOperation(format!(
                    "표가 이미 쓰는 이름입니다: {name}"
                )));
            }
        }
    }
    Ok(())
}

/// `Store.ViewColumns` — 뷰가 실제로 내는 열 이름(`PRAGMA table_info`).
fn view_columns(connection: &Connection, view: &str) -> Result<Vec<String>> {
    query(
        connection,
        &format!("PRAGMA table_info(\"{view}\");"),
        [],
        |r| r.get::<_, String>(1),
    )
}

/// `Store.ComputedViews` — 가공열이 붙는 뷰: 면 뷰와 차수 뷰, 계약이면 통합 둘까지.
fn computed_views(entity_type: &str) -> Result<&'static [&'static str]> {
    Ok(match entity_type {
        "notice" => &["v_공고", "v_공고차수"],
        "contract" => &["v_계약", "v_계약차수", "v_통합", "v_통합차수"],
        "request" => &["v_접수"],
        _ => return Err(Error::invalid("가공열의 자료 종류가 올바르지 않습니다.")),
    })
}

/// `Store.LatestColumnParents` — 열마다 최신 차수의 문서 다섯.
fn latest_column_parents(entity_type: &str) -> Result<String> {
    computed_column::fields(entity_type)?; // 표 이름을 넣기 전에 종류를 검증한다.
    Ok(format!(
        "SELECT p.* FROM {e} p\n\
         WHERE CAST(p.seq AS INTEGER)=(SELECT max(CAST(x.seq AS INTEGER)) FROM {e} x WHERE x.{e}_base=p.{e}_base)\n\
         ORDER BY p.{e}_base LIMIT 5",
        e = entity_type
    ))
}

/// `Store.Available` — 가공식 편집기의 재료(원천 필드·연결 문서·읽을 수 있는 뷰의 열).
fn available(connection: &Connection, entity_type: &str) -> Result<Value> {
    let env = Flow::env_for(entity_type)?;
    let linked = env
        .linked
        .iter()
        .map(|(kind, l)| {
            object([
                ("kind", Value::String(kind.clone())),
                ("fields", strings_json(&l.fields)),
                ("items", strings_json(&l.items)),
                ("many", Value::Bool(l.many)),
            ])
        })
        .collect();
    Ok(object([
        ("view", Value::String(face_view(entity_type).into())),
        ("base", strings_json(&env.base)),
        ("items", strings_json(&env.items)),
        ("linked", Value::Array(linked)),
        ("schema", readable_schema(connection)?),
    ]))
}

/// `SqlColumn.Readable.ToDictionary(v => v, v => ViewColumns(connection, v))`.
fn readable_schema(connection: &Connection) -> Result<Value> {
    let mut schema = Map::new();
    for view in sql_column::READABLE {
        schema.insert(
            view.to_string(),
            strings_json(&view_columns(connection, view)?),
        );
    }
    Ok(Value::Object(schema))
}

/// `Store.Shape` 의 결과.
struct Shape {
    head: &'static str,
    base_column: &'static str,
    series: &'static str,
    user_fields: &'static str,
    children: &'static [&'static str],
}

/// .NET `ArgumentOutOfRangeException(nameof(entityType), entityType, "모르는 종류입니다.")` 의 글.
fn unknown_kind(entity_type: &str) -> Error {
    Error::Argument(format!(
        "모르는 종류입니다. (Parameter 'entityType')\r\nActual value was {entity_type}."
    ))
}

/// `Store.Shape` — 종류마다 머리 표·본번호 열·계열 표·사람 값 표·딸린 표.
fn shape(entity_type: &str) -> Result<Shape> {
    Ok(match entity_type {
        "contract" => Shape {
            head: "contract",
            base_column: "contract_base",
            series: "contract_series",
            user_fields: "contract_user_field",
            children: &["contract_item", "contract_attachment"],
        },
        "notice" => Shape {
            head: "notice",
            base_column: "notice_base",
            series: "notice_series",
            user_fields: "notice_user_field",
            children: &[
                "notice_item",
                "notice_schedule",
                "notice_officer_contact",
                "notice_relation",
                "notice_restricted_industry",
            ],
        },
        "request" => Shape {
            head: "request",
            base_column: "request_base",
            series: "request_series",
            user_fields: "request_user_field",
            children: &["request_item"],
        },
        _ => return Err(unknown_kind(entity_type)),
    })
}

/// `Store.TableFor` — 사람 값 표와 본번호 열.
fn table_for(entity_type: &str) -> Result<(&'static str, &'static str)> {
    Ok(match entity_type {
        "contract" => ("contract_user_field", "contract_base"),
        "notice" => ("notice_user_field", "notice_base"),
        "request" => ("request_user_field", "request_base"),
        _ => return Err(unknown_kind(entity_type)),
    })
}

/// `Store.FaceView` — 이 종류를 통째로 내는 계약면 뷰. 덮개를 씌울 수 있는 열은 이 뷰의 열이다.
pub fn face_view(entity_type: &str) -> &'static str {
    match entity_type {
        "notice" => "v_공고",
        "request" => "v_접수",
        _ => "v_계약",
    }
}

/// `Store.FaceKey` — 그 면 뷰의 줄 열쇠 열.
pub fn face_key(entity_type: &str) -> &'static str {
    match entity_type {
        "notice" => "입찰공고번호",
        "request" => "접수번호",
        _ => "계약번호",
    }
}

// ── Dapper 자리 ─────────────────────────────────────────────

/// `connection.BeginTransaction(deferred: true)` — `BEGIN DEFERRED`. 읽기 전용 연결에서 사진 한 벌을 잡는다.
fn begin_deferred(connection: &Connection) -> Result<Transaction<'_>> {
    Ok(Transaction::new_unchecked(
        connection,
        TransactionBehavior::Deferred,
    )?)
}

/// Dapper 가 `string` 으로 읽는 값 — `Convert.ToString` 처럼 수도 글로 바꾼다. NULL 은 `None`. 다리(`pclm-bridge`)의 표 칸도 이것이다.
pub fn text_of(value: ValueRef<'_>) -> Option<String> {
    match value {
        ValueRef::Null => None,
        ValueRef::Integer(i) => Some(i.to_string()),
        ValueRef::Real(d) => Some(dotnet_double_r(d)),
        ValueRef::Text(t) => Some(String::from_utf8_lossy(t).into_owned()),
        ValueRef::Blob(_) => Some("System.Byte[]".into()),
    }
}

/// Dapper 동적 행의 값 하나를 System.Text.Json 이 쓰는 꼴로 — `long`·`double` 은 수, 글은 글, BLOB 은 base64.
pub(crate) fn json_of(value: ValueRef<'_>) -> Value {
    match value {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(i) => i.into(),
        ValueRef::Real(d) if d.is_finite() => json::number(&json::format_double(d)),
        // .NET 은 유한하지 않은 double 을 쓰지 못한다(예외). SQLite 는 NaN 을 NULL 로 두므로 무한대만 남는다.
        ValueRef::Real(d) => Value::String(dotnet_double_r(d)),
        ValueRef::Text(t) => Value::String(String::from_utf8_lossy(t).into_owned()),
        ValueRef::Blob(b) => Value::String(base64(b)),
    }
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(TABLE[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// `connection.Query(sql)` 의 동적 행 — {열 이름: 값}. 같은 이름이 둘이면 뒤의 것이 이긴다.
fn rows_json<P: Params>(connection: &Connection, sql: &str, args: P) -> Result<Vec<Value>> {
    let mut statement = connection.prepare(sql)?;
    let names: Vec<String> = statement.column_names().map(str::to_string).collect();
    let mut rows = statement.query(args)?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        let mut map = Map::new();
        for (i, name) in names.iter().enumerate() {
            map.insert(name.clone(), json_of(row.get_ref(i)?));
        }
        out.push(Value::Object(map));
    }
    Ok(out)
}

/// 행을 모두 읽고 버린다(`connection.Query(...).ToList()`) — 실행 오류는 줄을 읽다가 난다.
fn read_all(connection: &Connection, sql: &str) -> Result<()> {
    let mut statement = connection.prepare(sql)?;
    let mut rows = statement.query([])?;
    while rows.next()?.is_some() {}
    Ok(())
}

fn query<T, P: Params>(
    connection: &Connection,
    sql: &str,
    args: P,
    map: impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
) -> Result<Vec<T>> {
    let mut statement = connection.prepare(sql)?;
    let rows = statement.query_map(args, map)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// `Query<string>` — NULL 은 빈 글로.
fn strings<P: Params>(connection: &Connection, sql: &str, args: P) -> Result<Vec<String>> {
    query(connection, sql, args, |r| {
        Ok(text_of(r.get_ref(0)?).unwrap_or_default())
    })
}

/// `ExecuteScalar<long>` — 줄이 없거나 NULL 이면 0.
fn scalar_i64<P: Params>(connection: &Connection, sql: &str, args: P) -> Result<i64> {
    let mut statement = connection.prepare(sql)?;
    let mut rows = statement.query(args)?;
    Ok(match rows.next()? {
        Some(row) => match row.get_ref(0)? {
            ValueRef::Integer(i) => i,
            ValueRef::Real(d) => d as i64,
            ValueRef::Text(t) => String::from_utf8_lossy(t).trim().parse().unwrap_or(0),
            _ => 0,
        },
        None => 0,
    })
}

/// Dapper `QuerySingleOrDefault` — 줄이 없으면 `None`, 둘 이상이면 .NET 의 그 말로 거절한다.
fn single_or_default<T, P: Params>(
    connection: &Connection,
    sql: &str,
    args: P,
    mut map: impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
) -> Result<Option<T>> {
    let mut statement = connection.prepare(sql)?;
    let mut rows = statement.query(args)?;
    let Some(first) = rows.next()? else {
        return Ok(None);
    };
    let value = map(first)?;
    if rows.next()?.is_some() {
        return Err(Error::invalid("Sequence contains more than one element"));
    }
    Ok(Some(value))
}

/// Dapper `QuerySingle` — 꼭 한 줄.
fn single<T, P: Params>(
    connection: &Connection,
    sql: &str,
    args: P,
    map: impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
) -> Result<T> {
    single_or_default(connection, sql, args, map)?
        .ok_or_else(|| Error::invalid("Sequence contains no elements"))
}

fn read_user_columns<P: Params>(
    connection: &Connection,
    sql: &str,
    args: P,
) -> Result<Vec<UserColumnDefinition>> {
    query(connection, sql, args, |r| {
        Ok(UserColumnDefinition {
            entity_type: r.get(0)?,
            field_name: r.get(1)?,
            kind: r.get(2)?,
            options: r.get(3)?,
            sort_order: r.get(4)?,
        })
    })
}

fn object<const N: usize>(entries: [(&str, Value); N]) -> Value {
    let mut map = Map::new();
    for (k, v) in entries {
        map.insert(k.to_string(), v);
    }
    Value::Object(map)
}

fn strings_json(items: &[String]) -> Value {
    Value::Array(items.iter().cloned().map(Value::String).collect())
}

fn opt_string(value: Option<String>) -> Value {
    value.map_or(Value::Null, Value::String)
}

/// `Access.Read` 손잡이에서 쓰기를 거절하는 말.
fn refuse_read(database: &Database, message: &str) -> Result<()> {
    if database.access() == Access::Read {
        return Err(Error::invalid(message));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_dotnet() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foob"), "Zm9vYg==");
    }

    #[test]
    fn pack_trims_dedupes_and_drops_bars() {
        let c = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(pack("text", &c(&["a"])), None);
        assert_eq!(
            pack("choice", &c(&[" a ", "a", "", "b|c", "d"])),
            Some("a|d".into())
        );
        assert_eq!(pack("choice", &c(&[" "])), None);
    }

    #[test]
    fn clean_name_counts_utf16() {
        assert!(clean_name(&"가".repeat(20)).is_ok());
        assert!(clean_name(&"가".repeat(21)).is_err());
        assert!(clean_name(" a\"b").is_err());
        assert_eq!(clean_name("  메모 ").unwrap(), "메모");
    }
}

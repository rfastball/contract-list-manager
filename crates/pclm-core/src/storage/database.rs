//! SQLite 연결과 스키마 판올림(C# `Storage/Database.cs`).
//!
//! **연결마다 같은 프래그마를 건다**([`configure`]). C# 이 쓰는 e_sqlite3(Microsoft.Data.Sqlite 10.0.11, SQLite
//! 3.53.3)는 `foreign_keys` 가 처음부터 켜져 있고 `recursive_triggers` 는 꺼져 있다(실측). rusqlite 의 bundled
//! SQLite 는 `foreign_keys` 가 꺼진 채 열리므로, 같은 파일에 같은 일을 하려면 둘 다 연결마다 명시해야 한다 —
//! 빠뜨리면 지우기가 딸린 행을 끌고 가지 않는 것이 조용히 갈린다.

use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{Connection, OpenFlags, OptionalExtension, Transaction, TransactionBehavior};

use crate::env::{Env, sqlite_now};
use crate::erp::explicit_links;
use crate::error::{Error, Result};
use crate::storage::pclm_file::{self, PclmRole};
use crate::storage::user_view_builder::{self, BrokenFields};
use crate::storage::views::{self, BrokenColumns, UserColumnDefinition};
use crate::storage::{computed_column, flow_compiler, schema, schema_repairs, sql_column, store};

/// C# `Access` — [`Database`] 로 여는 연결이 쓸 수 있는가.
///
/// 화면에서 단추를 숨기는 것만으로는 새 쓰기 경로 하나를 빠뜨리는 순간 뚫린다. 손잡이에 붙여 두면 그 손잡이로
/// 여는 모든 연결이 SQLite 수준에서 막힌다(ADR-031).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Write,
    Read,
}

/// C# `Database` — 파일 하나를 가리키는 손잡이.
///
/// WAL 모드로 연다 — 편집 화면이 쓰는 동안에도 소비 쪽이 막히지 않고 읽을 수 있어야 한다(ADR-006).
#[derive(Clone, Debug)]
pub struct Database {
    path: PathBuf,
    access: Access,
}

/// Microsoft.Data.Sqlite 는 `busy_timeout` 을 0 으로 두고 `SQLITE_BUSY` 를 스스로 되풀이한다 — 명령 제한
/// 시간(`CommandTimeout`, 기본 30초)까지. rusqlite 는 되풀이하지 않으므로 SQLite 의 기다림으로 같은 너비를 준다.
const BUSY_TIMEOUT: Duration = Duration::from_secs(30);

/// 연결마다 거는 프래그마(위 모듈 설명). e_sqlite3 의 실측 기본값을 그대로 적는다. 큰따옴표 글자를 끄고
/// `ceil`·`floor` 를 두는 것도 여기다([`crate::sqlite::match_csharp`]) — 같은 SQL 을 같게 받거나 거절해야 깨진
/// 가공열이 두 판에서 같게 깨진다.
fn configure(connection: &Connection) -> Result<()> {
    connection.busy_timeout(BUSY_TIMEOUT)?;
    connection.execute_batch("PRAGMA foreign_keys = ON; PRAGMA recursive_triggers = OFF;")?;
    crate::sqlite::match_csharp(connection)?;
    Ok(())
}

/// 경로를 SQLite 에 넘길 글로. C# 은 `string` 경로라 UTF-16 으로 나타낼 수 없는 경로가 애초에 없다.
fn path_text(path: &Path) -> Result<&str> {
    path.to_str().ok_or_else(|| {
        Error::Argument(format!(
            "경로를 글로 나타낼 수 없습니다: {}",
            path.display()
        ))
    })
}

/// 플래그 그대로 연다. Microsoft.Data.Sqlite 의 `Mode` 와 같은 플래그다 — `ReadOnly` = `READ_ONLY`,
/// `ReadWrite` = `READ_WRITE`, `ReadWriteCreate` = `READ_WRITE | CREATE`. `URI` 를 넣지 않아 `file:` 로
/// 시작하는 경로도 그 이름의 파일이다(C# 과 같다).
pub(crate) fn connect(path: &Path, flags: OpenFlags) -> Result<Connection> {
    let connection = Connection::open_with_flags(path, flags | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
    configure(&connection)?;
    Ok(connection)
}

/// C# `connection.BeginTransaction()` — Microsoft.Data.Sqlite 는 `BEGIN IMMEDIATE` 로 연다. 커밋하지 않고 놓으면
/// 되돌린다.
pub fn begin(connection: &Connection) -> Result<Transaction<'_>> {
    Ok(Transaction::new_unchecked(
        connection,
        TransactionBehavior::Immediate,
    )?)
}

impl Database {
    /// `new Database(path, access)` — 파일을 가리키기만 한다. **아무것도 만들지 않는다** — 폴더도, 파일도.
    ///
    /// 빈 자료가 서는 길은 [`pclm_file::create`] 하나다(ADR-030). C# 은 여기서 `Path.GetFullPath` 로 온전한
    /// 경로를 짓지만, 이 크레이트는 현재 폴더를 보지 않는다 — 온전한 경로는 부르는 쪽이 준다.
    pub fn new(path: impl Into<PathBuf>, access: Access) -> Self {
        Database {
            path: path.into(),
            access,
        }
    }

    /// `Database.Path`.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// `Database.Access` — 이 손잡이로 여는 연결이 쓸 수 있는가. [`Database::open`] 이 따른다.
    pub fn access(&self) -> Access {
        self.access
    }

    /// `Database.Open` — 연결. **없는 파일은 만들지 않고 실패한다**(SQLite 코드 14).
    ///
    /// [`Access::Read`] 면 읽기 전용으로 열고 `query_only` 까지 건다(ADR-031). WAL 로 바꾸는 것도 파일 머리를
    /// 고치는 쓰기라 하지 않는다. [`Access::Write`] 면 옮겨진 옛 파일(`retired`)을 거절한다.
    pub fn open(&self) -> Result<Connection> {
        if self.access == Access::Read {
            let reader = self.open_read_only()?;
            reader.execute_batch("PRAGMA query_only = ON;")?;
            return Ok(reader);
        }
        let connection = connect(&self.path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        self.refuse_retired(&connection)?;
        // journal_mode 는 결과 행을 내므로 execute_batch 로 돌린다(C# ExecuteNonQuery 와 같이 결과를 버린다).
        connection.execute_batch("PRAGMA journal_mode = WAL;")?;
        connection.execute_batch("PRAGMA foreign_keys = ON;")?;
        Ok(connection)
    }

    /// `Database.RefuseRetired` — 역할이 `retired` 면 거절한다. 이름표가 없는 파일(판올림 전의 v19, 막 지은 빈
    /// 파일)에서만 물음이 실패하고(SQLite 코드 1), 그때만 표가 정말 없는지 한 번 더 보고 넘어간다.
    fn refuse_retired(&self, connection: &Connection) -> Result<()> {
        let role = match connection
            .query_row("SELECT role FROM pclm_file WHERE singleton = 1;", [], |r| {
                r.get::<_, rusqlite::types::Value>(0)
            })
            .optional()
        {
            Ok(role) => role,
            // C# `SqliteErrorCode == 1` — 기본 결과 코드 SQLITE_ERROR(없는 표).
            Err(e) if primary_code(&e) == Some(1) && !has_pclm_file(connection)? => return Ok(()),
            Err(e) => return Err(e.into()),
        };
        // C# `ExecuteScalar() as string` — 글이 아니면 역할이 없는 것으로 본다.
        if let Some(rusqlite::types::Value::Text(role)) = role
            && role == PclmRole::RETIRED
        {
            return Err(Error::invalid(format!(
                "이 자료는 다른 자리로 옮겨졌습니다: {} — 지금 작업자료를 쓰려면 프로그램을 다시 실행하세요.",
                self.path.display()
            )));
        }
        Ok(())
    }

    /// `Database.CreateEmptyFile` — 빈 파일 하나를 짓는다. **`READ_WRITE | CREATE` 로 여는 곳은 여기 하나뿐이고**,
    /// 부르는 곳도 [`pclm_file::create`] 하나뿐이다. WAL 로 바꾸는 것까지 해 둔다 — 머리를 한 번 적어야 파일이
    /// 실제로 디스크에 선다.
    pub(crate) fn create_empty_file(path: &Path) -> Result<()> {
        let connection = connect(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )?;
        connection.execute_batch("PRAGMA journal_mode = WAL;")?;
        Ok(())
    }

    /// `Database.OpenReadOnly` — 읽기 전용 연결. 소비 쪽에 넘길 때 쓴다.
    pub fn open_read_only(&self) -> Result<Connection> {
        connect(&self.path, OpenFlags::SQLITE_OPEN_READ_ONLY)
    }

    /// `Database.Snapshot` — 자료 한 벌을 `target` 으로 뜬다. 뜬 자리를 낸다.
    ///
    /// `VACUUM INTO` 라 `-wal`·`-shm` 없이 **한 파일로 정합하게** 떨어진다. 원본은 읽기 전용 연결로 뜬다 — 원본에
    /// 쓰기 잠금을 잡을 까닭이 없다. 뜬 결과는 WAL 이 아닌 일반 저널 한 파일이다.
    ///
    /// `overwrite` 가 거짓이면 이미 있는 자리에 쏟지 않는다(`VACUUM INTO` 가 거절한다) — 작업자료 자리를 지키는
    /// 방벽이다(ADR-031). **C# 과 달리 폴더를 짓지 않는다** — 폴더는 부르는 쪽(셸)이 짓는다(#8).
    pub fn snapshot(&self, target: &Path, overwrite: bool) -> Result<PathBuf> {
        if !self.path.is_file() {
            return Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("뜰 자료가 없습니다: {}", self.path.display()),
            )));
        }
        // VACUUM INTO 는 이미 있는 파일에 쏟지 않는다. 덮어쓰기로 부른 것만 치운다.
        if overwrite {
            for suffix in ["", "-wal", "-shm"] {
                remove_if_exists(&with_suffix(target, suffix))?;
            }
        }
        let source = connect(&self.path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        // 경로에 따옴표가 섞일 수 있어 매개변수로 넘긴다.
        source.execute("VACUUM INTO ?1;", [path_text(target)?])?;
        Ok(target.to_path_buf())
    }

    /// `Database.Migrate` — 스키마를 지금 판([`schema::VERSION`])까지 올리고 계약면 뷰를 새로 짓는다.
    ///
    /// 빈 파일(`user_version` 0)은 [`schema::BASELINE`] 으로 v19 를 곧장 짓고 그 위로 [`schema::STEPS`] 를 밟는다.
    /// 기준선보다 옛 시험판(v1~v18)은 **건드리지 않고** 거절한다. [`Access::Read`] 면 아무것도 쓰지 않는다 — 올릴
    /// 것이 있으면 거절하고, 지금 판이면 뷰도 다시 짓지 않는다.
    ///
    /// C# 이 SQL 안에서 짓던 무작위 이름표와 시각은 `env` 에서 받는다([`schema::BASELINE_SEED_BOUND`]·
    /// [`schema::V20_SEED_BOUND`]). `sqlite_master` 에 남는 글은 그대로라 파일의 스키마는 같다.
    pub fn migrate(&self, env: &dyn Env) -> Result<()> {
        self.refuse_pre_baseline()?;

        if self.access == Access::Read {
            let version = user_version(&self.open_read_only()?)?;
            if version < schema::VERSION {
                return Err(Error::invalid(format!(
                    "읽기 전용으로 연 자료라 판을 올리지 않습니다(v{version} → v{}): {}",
                    schema::VERSION,
                    self.path.display()
                )));
            }
            return Ok(());
        }

        let connection = self.open()?;
        let mut current = user_version(&connection)?;

        if current < schema::VERSION {
            // 판올림이 표를 통째로 갈아 끼우기도 하는데, 뷰가 그 표를 가리키고 있으면 SQLite 가 "error in view" 로
            // 막는다. 뷰에는 데이터가 없으니 먼저 치우고 나중에 다시 짓는다.
            drop_views(&connection)?;

            // 외래키 검사는 트랜잭션 안에서 켜고 끌 수 없어 바깥에서 다룬다.
            connection.execute_batch("PRAGMA foreign_keys = OFF;")?;

            // 이미 있던 작업자료만 예시 보기를 받는다 — 갓 지은 파일(0 에서 온 것)은 pclm_file::create 가 역할을 보고 심는다.
            let seed_example = current > 0 && current < schema::EXAMPLE_VIEW_VERSION;
            let upgrade_example = current > 0 && current < schema::EXAMPLE_VIEW_UPGRADE_VERSION;

            let transaction = begin(&connection)?;
            if current == 0 {
                run_baseline(&transaction, env)?;
                current = schema::BASELINE_VERSION;
            }
            for v in current..schema::VERSION {
                if v == schema::BASELINE_VERSION {
                    run_v20(&transaction, env)?;
                } else {
                    transaction
                        .execute_batch(schema::STEPS[(v - schema::BASELINE_VERSION) as usize])?;
                }
                if v == 29 {
                    schema_repairs::upgrade30(&transaction, current)?;
                }
                if v == 31 {
                    schema_repairs::upgrade32(&transaction)?;
                }
            }

            // v26 은 그 판의 예시를 심고, v27 은 손대지 않은 그것을 지금 구성으로 갈아 끼운다 — v25 에서 오면 둘 다 밟는다.
            let work = (seed_example || upgrade_example) && pclm_file::is_work(&transaction)?;
            if work && seed_example {
                store::seed_example_views(
                    &transaction,
                    Some(store::example_view_package_v26()),
                    env,
                )?;
            }
            if work && upgrade_example {
                store::upgrade_example_views(&transaction)?;
            }

            // PRAGMA 는 매개변수를 받지 않아 값을 직접 넣는다. 상수라 주입 위험이 없다.
            transaction.execute_batch(&format!("PRAGMA user_version = {};", schema::VERSION))?;
            transaction.commit()?;

            connection.execute_batch("PRAGMA foreign_keys = ON;")?;

            // 건을 관련공고를 타고 실제로 이어 붙이는 것은 판올림이 아니라 여기다. 참조 검사보다 앞서야 옮겨진
            // 링크가 검사를 지난다.
            let transaction = begin(&connection)?;
            explicit_links::resolve(&transaction)?;
            transaction.commit()?;

            // 갈아 끼운 참조가 실제로 맞는지 확인한다. 어긋난 채로 넘어가면 나중에 조용히 곪는다.
            assert_foreign_keys_hold(&connection)?;
        }

        refresh(&connection)?;
        Ok(())
    }

    /// `Database.RefreshViews()` — 사람이 채우는 열이 바뀌면 뷰를 다시 짓는다.
    pub fn refresh_views(&self) -> Result<()> {
        refresh(&self.open()?)?;
        Ok(())
    }

    /// `Database.RefusePreBaseline` — 기준선보다 옛 시험판이면 거절한다. **읽기 전용 연결로 먼저 본다** —
    /// [`Database::open`] 은 WAL 로 바꾸며 파일 머리를 고치므로, 받지 않을 파일에는 그것조차 하지 않는다.
    fn refuse_pre_baseline(&self) -> Result<()> {
        if !self.path.is_file() {
            return Ok(());
        }
        let version = user_version(&self.open_read_only()?)?;
        if schema::is_pre_baseline(version) {
            return Err(Error::invalid(format!(
                "정식판 이전의 옛 시험판(v{version})으로 지은 자료라 이 프로그램이 올리지 못합니다. \
                 0.7.0 으로 한 번 열어 v{} 로 올린 뒤 다시 여세요: {}",
                schema::BASELINE_VERSION,
                self.path.display()
            )));
        }
        Ok(())
    }
}

/// [`ExplicitLinks.Resolve(Database)`](explicit_links::resolve) — 한 트랜잭션에서 연결을 다시 짓고 이은 수를 낸다.
pub fn resolve_links(database: &Database) -> Result<usize> {
    let connection = database.open()?;
    let transaction = begin(&connection)?;
    let count = explicit_links::resolve(&transaction)?
        .iter()
        .filter(|r| r.state == "연결됨")
        .count();
    transaction.commit()?;
    Ok(count)
}

/// 기준선을 돌린다. 끝의 무작위 이름표 문장만 떼어 `env` 의 식별자로 넣는다.
fn run_baseline(connection: &Connection, env: &dyn Env) -> Result<()> {
    let head = schema::BASELINE
        .strip_suffix(schema::BASELINE_SEED_SQL)
        .expect("기준선 끝은 이름표 문장이다(schema 시험이 지킨다)");
    connection.execute_batch(head)?;
    connection.execute(schema::BASELINE_SEED_BOUND, [env.new_id()])?;
    Ok(())
}

/// v19 → v20 을 돌린다. 가운데 이름표 문장만 떼어 매개변수로 넣는다. `COALESCE` 는 이미 있는 이름표가 있으면
/// `randomblob` 을 부르지 않으므로, 새 식별자도 없을 때만 뽑는다.
fn run_v20(connection: &Connection, env: &dyn Env) -> Result<()> {
    let (head, tail) = schema::V20
        .split_once(schema::V20_SEED_SQL)
        .expect("V20 에는 이름표 문장이 한 번 있다(schema 시험이 지킨다)");
    connection.execute_batch(head)?;
    let existing: Option<String> = connection
        .query_row(
            "SELECT dataset_id FROM erp_dataset WHERE singleton = 1",
            [],
            |r| r.get(0),
        )
        .optional()?
        .flatten();
    let fresh = existing.is_none().then(|| env.new_id());
    connection.execute(
        schema::V20_SEED_BOUND,
        rusqlite::params![fresh, sqlite_now(env)],
    )?;
    connection.execute_batch(tail)?;
    Ok(())
}

/// [`refresh`] 의 결과 — 깨진 가공열과 깨진 맞춤 뷰 필드(C# `Database.Refresh` 의 `(Columns, UserViews)`). 차례는 C# 의
/// 사전 차례 그대로다.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Refreshed {
    /// (종류, 열 이름) → 까닭.
    pub columns: BrokenColumns,
    /// (뷰 식별자, 필드 식별자) → 까닭.
    pub user_views: BrokenFields,
}

/// `Database.Refresh(connection, transaction)` — 뷰를 지웠다 다시 짓는다(docs/user-views.md §5). 맞춤 뷰(`u_%`)를 맨 먼저
/// 모두 치우고 계약면 뷰를 지은 뒤 맨 끝에 맞춤 뷰를 짓는다.
///
/// 연결이 트랜잭션 밖이면 제 트랜잭션(`BEGIN IMMEDIATE`)을 열어 끝에 커밋하고, 이미 트랜잭션 안이면(`Transaction` 을
/// 넘겼으면) 그 안에서 돈다 — C# 의 `transaction is null` 갈래다.
pub fn refresh(connection: &Connection) -> Result<Refreshed> {
    // 사람이 채우는 열은 표가 아니라 정의로 있으므로, 뷰를 지을 때마다 읽어 붙인다.
    let columns = read_user_columns(connection)?;

    let own = if connection.is_autocommit() {
        Some(begin(connection)?)
    } else {
        None
    };
    user_view_builder::drop_all(connection)?;
    let broken = isolate_computed(connection, &columns)?;
    let user_views = user_view_builder::build(connection, &columns)?;
    if let Some(transaction) = own {
        transaction.commit()?;
    }
    Ok(Refreshed {
        columns: broken,
        user_views,
    })
}

/// `Database.IsolateComputed` — 가공열 하나가 깨져도 뷰 전체가 서지 못하는 일이 없게 한다(column-management §11.5).
///
/// 1. 가공열을 모두 빈 글로 둔 뼈대를 짓는다.
/// 2. 그 위에서 열마다 따로 컴파일하고 면 뷰 위에서 돌려 본다(SQL 열은 authorizer 를 건 채 준비한다).
/// 3. 깨진 것만 빈 글로 두고 다시 짓는다. 그래도 뷰가 준비되지 않으면 SQL 열이 서로 도는 것이다(순환) — SQL 열을 모두
///    걷었다가 하나씩 되돌려, 되돌리면 깨지는 것을 깨진 것으로 둔다.
fn isolate_computed(
    connection: &Connection,
    columns: &[UserColumnDefinition],
) -> Result<BrokenColumns> {
    let mut broken = BrokenColumns::new();
    let computed: Vec<&UserColumnDefinition> =
        columns.iter().filter(|c| c.kind == "computed").collect();
    let key = |c: &UserColumnDefinition| (c.entity_type.clone(), c.field_name.clone());
    let build = |broken: &BrokenColumns| -> Result<()> {
        for definition in views::definitions(columns, broken)? {
            connection.execute_batch(&definition)?;
        }
        Ok(())
    };
    if computed.is_empty() {
        build(&broken)?;
        return Ok(broken);
    }

    // 1. 뼈대
    for c in &computed {
        broken.insert(key(c), String::new());
    }
    build(&broken)?;
    broken.clear();

    // 2. 하나씩
    for c in &computed {
        let options = c.options.as_deref().unwrap_or("");
        let tried = (|| -> Result<()> {
            let formula = computed_column::parse(options)?;
            computed_column::final_shape(&c.entity_type, options)?;
            if formula.source.as_deref() == Some("sql") {
                connection.execute_batch("PRAGMA query_only = ON;")?;
                let prepared = sql_column::prepare(
                    connection,
                    &c.entity_type,
                    formula.sql.as_deref().unwrap_or(""),
                );
                connection.execute_batch("PRAGMA query_only = OFF;")?;
                prepared?;
            }
            let expression = computed_column::expression(&c.entity_type, &formula, None)?;
            let sql = format!(
                "SELECT {expression} FROM \"{}\" AS {} LIMIT 20;",
                store::face_view(&c.entity_type),
                flow_compiler::SELF
            );
            let mut statement = connection.prepare(&sql)?;
            let mut rows = statement.query([])?;
            while rows.next()?.is_some() {}
            Ok(())
        })();
        match tried {
            Ok(()) => {}
            Err(error @ (Error::InvalidOperation(_) | Error::Sqlite(_))) => {
                broken.insert(key(c), sql_column::error_message(&error));
            }
            Err(error) => return Err(error),
        }
    }

    // 3. 깨진 것만 걷고 짓는다. SQL 열끼리의 순환은 준비해 봐야 드러난다.
    build(&broken)?;
    if prepare_views(connection)?.is_none() {
        return Ok(broken);
    }
    let mut sql = Vec::new();
    for c in &computed {
        if !broken.contains_key(&key(c))
            && computed_column::parse(c.options.as_deref().unwrap_or(""))?
                .source
                .as_deref()
                == Some("sql")
        {
            sql.push(*c);
        }
    }
    for c in &sql {
        broken.insert(key(c), String::new());
    }
    build(&broken)?;
    if let Some(base_error) = prepare_views(connection)? {
        return Err(Error::InvalidOperation(format!(
            "계약면 뷰를 짓지 못했습니다: {base_error}"
        )));
    }
    for c in &sql {
        broken.remove(&key(c));
        build(&broken)?;
        if let Some(error) = prepare_views(connection)? {
            broken.insert(key(c), error);
        }
    }
    build(&broken)?;
    Ok(broken)
}

/// `Database.PrepareViews` — 모든 뷰를 준비만 해 본다(이름 차례). 실패하면 첫 오류 글(C# 의 예외 글 꼴).
fn prepare_views(connection: &Connection) -> Result<Option<String>> {
    let names: Vec<String> = connection
        .prepare("SELECT name FROM sqlite_master WHERE type = 'view' ORDER BY name;")?
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    for view in names {
        let tried = (|| -> rusqlite::Result<()> {
            let mut statement = connection.prepare(&format!(
                "SELECT * FROM \"{}\" LIMIT 0;",
                view.replace('"', "\"\"")
            ))?;
            let mut rows = statement.query([])?;
            rows.next()?;
            Ok(())
        })();
        if let Err(error) = tried {
            return Ok(Some(sql_column::dotnet_message(&error)));
        }
    }
    Ok(None)
}

/// `Database.ReadUserColumns` — 사람이 채우는 열의 정의(종류, 차례 순).
fn read_user_columns(connection: &Connection) -> Result<Vec<UserColumnDefinition>> {
    Ok(connection
        .prepare(
            "SELECT entity_type, field_name, kind, options, sort_order FROM user_column ORDER BY entity_type, sort_order;",
        )?
        .query_map([], |r| {
            Ok(UserColumnDefinition {
                entity_type: r.get(0)?,
                field_name: r.get(1)?,
                kind: r.get(2)?,
                options: r.get(3)?,
                sort_order: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?)
}

/// `Database.DropViews` — 모든 뷰를 지운다. 앱은 따옴표 든 뷰 이름을 짓지 않지만 손으로 만들었거나 남의 도구가 지은
/// 파일에는 있을 수 있어, 안의 `"` 를 두 겹으로 한다 — 그러지 않으면 이 DROP 이 문법 오류로 판올림을 멈춘다.
fn drop_views(connection: &Connection) -> Result<()> {
    let names: Vec<String> = connection
        .prepare("SELECT name FROM sqlite_master WHERE type = 'view';")?
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    for name in names {
        let quoted = name.replace('"', "\"\"");
        connection.execute_batch(&format!("DROP VIEW IF EXISTS \"{quoted}\";"))?;
    }
    Ok(())
}

/// `Database.AssertForeignKeysHold` — 외래키가 실제로 성립하는지 본다.
fn assert_foreign_keys_hold(connection: &Connection) -> Result<()> {
    let broken: Vec<String> = connection
        .prepare("PRAGMA foreign_key_check;")?
        .query_map([], |r| {
            Ok(format!(
                "{} 행이 {} 를 가리키지 못합니다",
                literal(r.get(0)?),
                literal(r.get(2)?)
            ))
        })?
        .collect::<rusqlite::Result<_>>()?;
    if broken.is_empty() {
        return Ok(());
    }
    Err(Error::invalid(format!(
        "판올림 뒤 참조가 어긋났습니다: {}",
        broken.join(" / ")
    )))
}

/// `reader.GetValue(i)` 를 글에 끼운 꼴. 표 이름은 늘 글이다.
fn literal(value: rusqlite::types::Value) -> String {
    use rusqlite::types::Value;
    match value {
        Value::Text(s) => s,
        Value::Integer(i) => i.to_string(),
        Value::Real(f) => f.to_string(),
        Value::Null | Value::Blob(_) => String::new(),
    }
}

/// SQLite 기본 결과 코드(확장 코드의 아래 8비트). C# `SqliteException.SqliteErrorCode` 와 같은 수다.
pub(crate) fn primary_code(error: &rusqlite::Error) -> Option<i32> {
    match error {
        rusqlite::Error::SqliteFailure(failure, _) => Some(failure.extended_code & 0xff),
        _ => None,
    }
}

fn has_pclm_file(connection: &Connection) -> Result<bool> {
    let count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'pclm_file';",
        [],
        |r| r.get(0),
    )?;
    Ok(count > 0)
}

/// `Database.UserVersion`.
pub(crate) fn user_version(connection: &Connection) -> Result<i32> {
    Ok(connection.query_row("PRAGMA user_version;", [], |r| r.get(0))?)
}

/// 경로 뒤에 글을 붙인다(`-wal`·`-shm`). `Path::with_extension` 은 확장자를 갈아 끼우므로 쓰지 않는다.
pub(crate) fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut text = path.as_os_str().to_os_string();
    text.push(suffix);
    PathBuf::from(text)
}

/// C# `File.Delete` — 없는 파일이면 조용히 넘어간다.
pub(crate) fn remove_if_exists(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        other => Ok(other?),
    }
}

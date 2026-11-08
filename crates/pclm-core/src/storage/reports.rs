//! 축적 상태를 훑어보는 조회들(C# `Storage/Reports.cs`) — 요약(`Summary`), 차수 묶음(`Revisions`), 브리핑(`Dashboard`).
//! 쓰기는 하지 않는다.
//!
//! **브리핑의 「지금」은 부르는 쪽이 준다**(#8). C# 은 `asOf` 를 주지 않으면 `DateTimeOffset.UtcNow` 를 읽는다 — 이
//! 크레이트는 시계를 읽지 않으므로 [`Reports::dashboard`] 가 그 순간(`Env::now_utc` 꼴의 글)을 받는다. 셈은 KST(+09:00)
//! 고정이다 — 기계의 시간대를 보지 않는다(C# 과 같다).
//!
//! 값은 C# 처럼 읽는다 — 뷰 칸은 `Convert.ToString(값, InvariantCulture)`(NULL 은 빈 글), 금액은
//! `decimal.TryParse(NumberStyles.Number)`([`Decimal`]), 날짜는 `yyyy/MM/dd`·`yyyy-MM-dd`(뒤에 ` HH:mm:ss` 가 붙을 수
//! 있다)를 KST 로 읽는 `DateTimeOffset.TryParseExact` 다([`kst_date`]).

use rusqlite::types::{Value as SqlValue, ValueRef};
use rusqlite::{Connection, Transaction, TransactionBehavior};
use serde_json::{Map, Value};

use crate::decimal::Decimal;
use crate::error::{Error, Result};
use crate::json;
use crate::local_time::{FixedZone, LocalDateTime, days_in_month, parse_to_local};
use crate::storage::database::Database;
use crate::storage::store::text_of;
use crate::text::cmp_ordinal;

/// KST 의 UTC 와의 차이(초).
const KST: i64 = 9 * 3600;

/// `DateTimeOffset.MinValue` 의 UTC 순간(0001-01-01T00:00:00Z)을 1970 부터의 초로.
const MIN_INSTANT: i64 = -62_135_596_800;

// ── 형식 ─────────────────────────────────────────────────────

/// C# `StatusSummary` — 표마다의 행 수를 못 박은 요약.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusSummary {
    pub notice_rows: i32,
    pub notice_bases: i32,
    pub contract_rows: i32,
    pub contract_bases: i32,
    pub counterparties: i32,
    pub unlinked_contracts: i32,
    pub request_rows: i32,
    pub request_bases: i32,
    pub unlinked_requests: i32,
}

impl StatusSummary {
    /// 다리(`Bridge.Json` — camelCase)의 꼴.
    pub fn to_json(&self) -> Value {
        object(vec![
            ("noticeRows", self.notice_rows.into()),
            ("noticeBases", self.notice_bases.into()),
            ("contractRows", self.contract_rows.into()),
            ("contractBases", self.contract_bases.into()),
            ("counterparties", self.counterparties.into()),
            ("unlinkedContracts", self.unlinked_contracts.into()),
            ("requestRows", self.request_rows.into()),
            ("requestBases", self.request_bases.into()),
            ("unlinkedRequests", self.unlinked_requests.into()),
        ])
    }
}

/// C# `RevisionGroup(Base, Kind, Revisions, Latest, Title)` — 차수가 쌓인 건 하나. `revisions` 는 64비트(`COUNT(*)`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RevisionGroup {
    pub base: String,
    pub kind: String,
    pub revisions: i64,
    pub latest: String,
    pub title: Option<String>,
}

impl RevisionGroup {
    pub fn to_json(&self) -> Value {
        object(vec![
            ("base", Value::String(self.base.clone())),
            ("kind", Value::String(self.kind.clone())),
            ("revisions", self.revisions.into()),
            ("latest", Value::String(self.latest.clone())),
            ("title", opt(&self.title)),
        ])
    }
}

/// C# `DashboardContract`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DashboardContract {
    pub number: String,
    pub title: String,
    pub date: String,
    pub amount: Option<Decimal>,
    pub draft: bool,
    pub terminated: bool,
}

impl DashboardContract {
    pub fn to_json(&self) -> Value {
        object(vec![
            ("number", Value::String(self.number.clone())),
            ("title", Value::String(self.title.clone())),
            ("date", Value::String(self.date.clone())),
            ("amount", money(self.amount)),
            ("draft", Value::Bool(self.draft)),
            ("terminated", Value::Bool(self.terminated)),
        ])
    }
}

/// C# `DeliveryDeadline(Raw, Date, DaysRemaining)` — 적힌 글, 읽은 날(`yyyy-MM-dd`), 남은 날 수.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeliveryDeadline {
    pub raw: String,
    pub date: Option<String>,
    pub days_remaining: Option<i32>,
}

impl DeliveryDeadline {
    pub fn to_json(&self) -> Value {
        object(vec![
            ("raw", Value::String(self.raw.clone())),
            ("date", opt(&self.date)),
            (
                "daysRemaining",
                self.days_remaining.map_or(Value::Null, Value::from),
            ),
        ])
    }
}

/// C# `DeliveryItem`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeliveryItem {
    pub line_no: i32,
    pub name: String,
    pub specification: String,
    pub agency: String,
    pub quantity: String,
    pub unit: String,
    pub due: DeliveryDeadline,
}

impl DeliveryItem {
    pub fn to_json(&self) -> Value {
        object(vec![
            ("lineNo", self.line_no.into()),
            ("name", Value::String(self.name.clone())),
            ("specification", Value::String(self.specification.clone())),
            ("agency", Value::String(self.agency.clone())),
            ("quantity", Value::String(self.quantity.clone())),
            ("unit", Value::String(self.unit.clone())),
            ("due", self.due.to_json()),
        ])
    }
}

/// C# `DeliveryContract`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeliveryContract {
    pub number: String,
    pub title: String,
    pub contract_date: String,
    pub party: String,
    pub agency: String,
    pub terminated: bool,
    pub draft: bool,
    pub due: DeliveryDeadline,
    pub items: Vec<DeliveryItem>,
    pub issues: Vec<String>,
}

impl DeliveryContract {
    pub fn to_json(&self) -> Value {
        object(vec![
            ("number", Value::String(self.number.clone())),
            ("title", Value::String(self.title.clone())),
            ("contractDate", Value::String(self.contract_date.clone())),
            ("party", Value::String(self.party.clone())),
            ("agency", Value::String(self.agency.clone())),
            ("terminated", Value::Bool(self.terminated)),
            ("draft", Value::Bool(self.draft)),
            ("due", self.due.to_json()),
            (
                "items",
                Value::Array(self.items.iter().map(DeliveryItem::to_json).collect()),
            ),
            ("issues", strings(&self.issues)),
        ])
    }
}

/// C# `DashboardCase` — 조달 건 하나. `stage` 가 `None` 이면 확인할 거리다.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DashboardCase {
    pub key: String,
    pub title: String,
    pub stage: Option<String>,
    pub request_number: String,
    pub notice_number: String,
    pub notice_numbers: Vec<String>,
    pub notice_kind: String,
    pub request_date: String,
    pub posted_at: String,
    pub bid_starts_at: String,
    pub bid_closes_at: String,
    pub opening_at: String,
    pub agency: String,
    pub contracts: Vec<DashboardContract>,
    pub contract_amount: Option<Decimal>,
    pub issues: Vec<String>,
    pub draft_contracts: i32,
    pub notice_cancelled: bool,
    pub terminated_contracts: i32,
}

impl DashboardCase {
    pub fn to_json(&self) -> Value {
        object(vec![
            ("key", Value::String(self.key.clone())),
            ("title", Value::String(self.title.clone())),
            ("stage", opt(&self.stage)),
            ("requestNumber", Value::String(self.request_number.clone())),
            ("noticeNumber", Value::String(self.notice_number.clone())),
            ("noticeNumbers", strings(&self.notice_numbers)),
            ("noticeKind", Value::String(self.notice_kind.clone())),
            ("requestDate", Value::String(self.request_date.clone())),
            ("postedAt", Value::String(self.posted_at.clone())),
            ("bidStartsAt", Value::String(self.bid_starts_at.clone())),
            ("bidClosesAt", Value::String(self.bid_closes_at.clone())),
            ("openingAt", Value::String(self.opening_at.clone())),
            ("agency", Value::String(self.agency.clone())),
            (
                "contracts",
                Value::Array(
                    self.contracts
                        .iter()
                        .map(DashboardContract::to_json)
                        .collect(),
                ),
            ),
            ("contractAmount", money(self.contract_amount)),
            ("issues", strings(&self.issues)),
            ("draftContracts", self.draft_contracts.into()),
            ("noticeCancelled", Value::Bool(self.notice_cancelled)),
            ("terminatedContracts", self.terminated_contracts.into()),
        ])
    }
}

/// C# `DashboardStage(Key, Label, Count)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DashboardStage {
    pub key: String,
    pub label: String,
    pub count: i32,
}

/// C# `DashboardDocuments` — 최신 차수의 문서 수와 쌓인 차수의 행 수.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DashboardDocuments {
    pub requests: i32,
    pub notices: i32,
    pub contracts: i32,
    pub request_revisions: i32,
    pub notice_revisions: i32,
    pub contract_revisions: i32,
}

/// C# `DashboardReport` — 브리핑 한 벌. `as_of` 는 KST 의 `"O"` 표기(`…+09:00`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DashboardReport {
    pub as_of: String,
    pub total_cases: i32,
    pub stages: Vec<DashboardStage>,
    pub rows: Vec<DashboardCase>,
    pub documents: DashboardDocuments,
    pub contract_amount: Decimal,
    pub contracts_with_amount: i32,
    pub data_check_count: i32,
    pub deliveries: Vec<DeliveryContract>,
}

impl DashboardReport {
    pub fn to_json(&self) -> Value {
        let d = &self.documents;
        object(vec![
            ("asOf", Value::String(self.as_of.clone())),
            ("totalCases", self.total_cases.into()),
            (
                "stages",
                Value::Array(
                    self.stages
                        .iter()
                        .map(|s| {
                            object(vec![
                                ("key", Value::String(s.key.clone())),
                                ("label", Value::String(s.label.clone())),
                                ("count", s.count.into()),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "rows",
                Value::Array(self.rows.iter().map(DashboardCase::to_json).collect()),
            ),
            (
                "documents",
                object(vec![
                    ("requests", d.requests.into()),
                    ("notices", d.notices.into()),
                    ("contracts", d.contracts.into()),
                    ("requestRevisions", d.request_revisions.into()),
                    ("noticeRevisions", d.notice_revisions.into()),
                    ("contractRevisions", d.contract_revisions.into()),
                ]),
            ),
            ("contractAmount", money(Some(self.contract_amount))),
            ("contractsWithAmount", self.contracts_with_amount.into()),
            ("dataCheckCount", self.data_check_count.into()),
            (
                "deliveries",
                Value::Array(
                    self.deliveries
                        .iter()
                        .map(DeliveryContract::to_json)
                        .collect(),
                ),
            ),
        ])
    }
}

// ── 조회 ─────────────────────────────────────────────────────

/// C# `Reports(database)` — 축적 상태를 훑어보는 조회들. 쓰기는 하지 않는다.
pub struct Reports {
    database: Database,
}

impl Reports {
    pub fn new(database: Database) -> Self {
        Reports { database }
    }

    /// `Reports.Dashboard(asOf)` — 수집된 조달 건의 브리핑. `now_utc` 는 그 순간의 `"O"` 표기(`Env::now_utc` 의 꼴 —
    /// `Z` 나 `±HH:mm` 이 붙은 것). 모든 수는 한 읽기 사진에서 낸다.
    ///
    /// 문서 부재는 업무 미진행을, 계약 문서는 이행 완료를 뜻하지 않는다. Outline 과 같은 공고건·명시 링크를 쓰고 품목
    /// 목록은 펼치지 않는다.
    pub fn dashboard(&self, now_utc: &str) -> Result<DashboardReport> {
        let now = parse_to_local(Some(now_utc), &FixedZone(KST)).ok_or_else(|| {
            Error::Argument(format!("브리핑의 시각을 읽지 못했습니다: {now_utc}"))
        })?;
        let now = Now {
            instant: (now.unix_seconds() - KST, now.ticks),
            day: day_number(now),
            text: format!("{}.{:07}+09:00", now.second_text(), now.ticks),
        };
        let connection = self.database.open_read_only()?;
        let transaction = Transaction::new_unchecked(&connection, TransactionBehavior::Deferred)?;
        let report = dashboard_on(&transaction, &now)?;
        drop(transaction);
        Ok(report)
    }

    /// `Reports.Summary`.
    pub fn summary(&self) -> Result<StatusSummary> {
        let connection = self.database.open()?;
        let count = |sql: &str| -> Result<i32> { scalar_i32(&connection, sql) };
        Ok(StatusSummary {
            notice_rows: count("SELECT COUNT(*) FROM notice;")?,
            notice_bases: count("SELECT COUNT(DISTINCT notice_base) FROM notice;")?,
            contract_rows: count("SELECT COUNT(*) FROM contract;")?,
            contract_bases: count("SELECT COUNT(DISTINCT contract_base) FROM contract;")?,
            counterparties: count("SELECT COUNT(*) FROM counterparty;")?,
            unlinked_contracts: count(
                "SELECT COUNT(DISTINCT c.contract_base) FROM contract c\n\
                 WHERE NOT EXISTS (\n    \
                 SELECT 1 FROM project_link l WHERE l.contract_base = c.contract_base\n);",
            )?,
            request_rows: count("SELECT COUNT(*) FROM request;")?,
            request_bases: count("SELECT COUNT(DISTINCT request_base) FROM request;")?,
            unlinked_requests: count(
                "SELECT COUNT(DISTINCT r.request_base) FROM request r\n\
                 WHERE NOT EXISTS (\n    \
                 SELECT 1 FROM request_link l WHERE l.request_base = r.request_base\n);",
            )?,
        })
    }

    /// `Reports.Revisions` — 차수 묶음. 많이 쌓인 것이 앞이고, 같으면 본번호 차례(Ordinal). 제목은 **최신 차수의 것**이다
    /// — 사전순 최대를 고르면 「최신 001」 옆에 000 의 제목이 붙는다.
    pub fn revisions(&self) -> Result<Vec<RevisionGroup>> {
        let connection = self.database.open()?;
        let mut all = Vec::new();
        for (table, key, kind) in [
            ("request", "request_base", "접수"),
            ("notice", "notice_base", "공고"),
            ("contract", "contract_base", "계약"),
        ] {
            let sql = format!(
                "SELECT t.{key} AS Base, COUNT(*) AS Revisions,\n       \
                 (SELECT x.seq FROM {table} x WHERE x.{key} = t.{key}\n        \
                 ORDER BY CAST(x.seq AS INTEGER) DESC LIMIT 1) AS Latest,\n       \
                 (SELECT x.title FROM {table} x WHERE x.{key} = t.{key}\n        \
                 ORDER BY CAST(x.seq AS INTEGER) DESC LIMIT 1) AS Title\n\
                 FROM {table} t GROUP BY t.{key};"
            );
            let mut statement = connection.prepare(&sql)?;
            let mut rows = statement.query([])?;
            while let Some(row) = rows.next()? {
                all.push(RevisionGroup {
                    base: cast_string(row.get_ref(0)?)?.unwrap_or_default(),
                    kind: kind.to_string(),
                    revisions: crate::storage::status::scalar_i64(row.get_ref(1)?),
                    latest: cast_string(row.get_ref(2)?)?.unwrap_or_default(),
                    title: cast_string(row.get_ref(3)?)?,
                });
            }
        }
        all.sort_by(|a, b| {
            b.revisions
                .cmp(&a.revisions)
                .then_with(|| cmp_ordinal(&a.base, &b.base))
        });
        Ok(all)
    }
}

/// 브리핑의 「지금」 — UTC 순간, KST 의 날 번호, `"O"` 표기.
struct Now {
    instant: (i64, u32),
    day: i64,
    text: String,
}

/// Dapper 동적 행 하나 — 열 이름과 값.
pub(crate) struct Row(Vec<(String, SqlValue)>);

impl Row {
    /// C# `Reports.Text` — `Convert.ToString(값, InvariantCulture) ?? ""`. 없는 열도 빈 글이다.
    pub(crate) fn text(&self, key: &str) -> String {
        self.0
            .iter()
            .find(|(k, _)| k == key)
            .and_then(|(_, v)| text_of(ValueRef::from(v)))
            .unwrap_or_default()
    }

    pub(crate) fn value(&self, key: &str) -> ValueRef<'_> {
        self.0
            .iter()
            .find(|(k, _)| k == key)
            .map_or(ValueRef::Null, |(_, v)| ValueRef::from(v))
    }
}

fn text(row: Option<&Row>, key: &str) -> String {
    row.map(|r| r.text(key)).unwrap_or_default()
}

pub(crate) fn read(connection: &Connection, sql: &str) -> Result<Vec<Row>> {
    let mut statement = connection.prepare(sql)?;
    let names: Vec<String> = statement.column_names().map(String::from).collect();
    let mut rows = statement.query([])?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        let mut cells = Vec::with_capacity(names.len());
        for (i, name) in names.iter().enumerate() {
            cells.push((name.clone(), row.get::<_, SqlValue>(i)?));
        }
        out.push(Row(cells));
    }
    Ok(out)
}

/// `ToDictionary` — 같은 열쇠가 또 오면 .NET 처럼 거절한다.
pub(crate) fn insert_unique<V>(
    entries: &mut Vec<(String, V)>,
    key: String,
    value: V,
) -> Result<()> {
    if entries.iter().any(|(k, _)| *k == key) {
        return Err(Error::Argument(format!(
            "An item with the same key has already been added. Key: {key}"
        )));
    }
    entries.push((key, value));
    Ok(())
}

pub(crate) fn find<'a, V>(entries: &'a [(String, V)], key: &str) -> Option<&'a V> {
    entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

/// 이음 표 — (본번호, 공고건). C# `Query<(string, string)>` 은 NULL 을 그대로 두지만 두 열 모두 NOT NULL 이다.
pub(crate) fn links(connection: &Connection, sql: &str) -> Result<Vec<(String, String)>> {
    let mut statement = connection.prepare(sql)?;
    let mut rows = statement.query([])?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        let key = cast_string(row.get_ref(0)?)?.unwrap_or_default();
        let group = cast_string(row.get_ref(1)?)?.unwrap_or_default();
        insert_unique(&mut out, key, group)?;
    }
    Ok(out)
}

fn dashboard_on(connection: &Connection, now: &Now) -> Result<DashboardReport> {
    let mut requests: Vec<(String, Row)> = Vec::new();
    for row in read(
        connection,
        "SELECT 접수본번호, 접수번호, 요청명, 접수일자, 수요기관,\n    \
         (SELECT count(*) FROM request_item i WHERE i.request_base = 접수본번호\n     \
         AND i.request_base || '-' || i.seq = 접수번호 AND i.cancelled = 1) AS 취소품목수\n\
         FROM v_접수",
    )? {
        let key = row.text("접수본번호");
        insert_unique(&mut requests, key, row)?;
    }
    let notices = read(
        connection,
        "SELECT 공고본번호, 입찰공고번호, 공고명, 공고종류, 게시일시, 공고기관, 수요기관,\n       \
         입찰개시일시, 입찰마감일시, 개찰일시, 공고건, 현행공고 FROM v_공고",
    )?;
    let mut contracts: Vec<(String, Row)> = Vec::new();
    for row in read(
        connection,
        "SELECT v.계약본번호, v.계약번호, v.계약건명, v.계약일자, v.계약금액, v.수요기관, v.계약상대자, v.납품기한,\n       \
         c.terminated AS 해지\n\
         FROM v_계약 v JOIN contract c ON v.계약본번호 = c.contract_base AND v.계약번호 = c.contract_base || c.seq",
    )? {
        let key = row.text("계약본번호");
        insert_unique(&mut contracts, key, row)?;
    }
    let mut items_by_contract: Vec<(String, Vec<Row>)> = Vec::new();
    for row in read(
        connection,
        "SELECT 계약번호, 순번, 품명, 규격, 수요기관, 수량, 단위, 납품기한 FROM v_계약품목",
    )? {
        let key = row.text("계약번호");
        match items_by_contract.iter_mut().find(|(k, _)| *k == key) {
            Some((_, list)) => list.push(row),
            None => items_by_contract.push((key, vec![row])),
        }
    }
    // 기한은 계약과 모든 최신 품목을 따로 보존한다. 품목의 더 이른 날로 계약 기한을 덮지 않는다.
    let mut ordered_contracts: Vec<&Row> = contracts.iter().map(|(_, r)| r).collect();
    ordered_contracts.sort_by(|a, b| cmp_ordinal(&a.text("계약번호"), &b.text("계약번호")));
    let mut deliveries = Vec::new();
    for contract in ordered_contracts {
        let items = find(&items_by_contract, &contract.text("계약번호"))
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        deliveries.push(delivery(contract, items, now)?);
    }
    let request_links = links(
        connection,
        "SELECT request_base, notice_group FROM request_link",
    )?;
    let contract_links = links(
        connection,
        "SELECT contract_base, notice_group FROM project_link",
    )?;

    // 공고는 건마다 — 건 이름이 비면 제 본번호로.
    let mut notice_groups: Vec<(String, Vec<&Row>)> = Vec::new();
    for notice in &notices {
        let key = first(&[notice.text("공고건"), notice.text("공고본번호")]);
        match notice_groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, list)) => list.push(notice),
            None => notice_groups.push((key, vec![notice])),
        }
    }
    let mut requests_by_group: Vec<(String, &Row)> = Vec::new();
    for (base, group) in &request_links {
        if let Some(request) = find(&requests, base) {
            insert_unique(&mut requests_by_group, group.clone(), request)?;
        }
    }
    let mut contracts_by_group: Vec<(String, Vec<&Row>)> = Vec::new();
    for (base, group) in &contract_links {
        if let Some(contract) = find(&contracts, base) {
            match contracts_by_group.iter_mut().find(|(k, _)| k == group) {
                Some((_, list)) => list.push(contract),
                None => contracts_by_group.push((group.clone(), vec![contract])),
            }
        }
    }
    // 참조만 남은 공고건에도 실제 접수·계약 문서가 있으면 보인다. 빈 건 표 자체는 분모에 넣지 않는다.
    let mut groups: Vec<&str> = Vec::new();
    for key in notice_groups
        .iter()
        .map(|(k, _)| k)
        .chain(requests_by_group.iter().map(|(k, _)| k))
        .chain(contracts_by_group.iter().map(|(k, _)| k))
    {
        if !groups.contains(&key.as_str()) {
            groups.push(key);
        }
    }
    let mut rows = Vec::new();
    for group in groups {
        let captured: Vec<&Row> = find(&notice_groups, group).cloned().unwrap_or_default();
        // `SingleOrDefault` — 둘 이상이면 .NET 은 던진다.
        let mut current = captured
            .iter()
            .copied()
            .filter(|n| n.text("입찰공고번호") == n.text("현행공고"));
        let first_current = current.next();
        if current.next().is_some() {
            return Err(Error::invalid(
                "Sequence contains more than one matching element",
            ));
        }
        let request = find(&requests_by_group, group).copied();
        let linked: Vec<&Row> = find(&contracts_by_group, group)
            .cloned()
            .unwrap_or_default();
        rows.push(case(
            format!("group:{group}"),
            request,
            first_current,
            &captured,
            &linked,
            now,
        ));
    }
    for (key, request) in &requests {
        if find(&request_links, key).is_none() {
            rows.push(case(
                format!("request:{key}"),
                Some(request),
                None,
                &[],
                &[],
                now,
            ));
        }
    }
    for (key, contract) in &contracts {
        if find(&contract_links, key).is_none() {
            rows.push(case(
                format!("contract:{key}"),
                None,
                None,
                &[],
                &[contract],
                now,
            ));
        }
    }

    let stages = [
        ("requestOnly", "공고 대기"),
        ("open", "공고중"),
        ("awaitingContract", "계약 대기"),
        ("contracted", "계약 완료"),
    ];
    let amounts: Vec<Decimal> = contracts
        .iter()
        .filter_map(|(_, c)| Decimal::parse_number(&c.text("계약금액")))
        .collect();
    let count = |table: &str| scalar_i32(connection, &format!("SELECT count(*) FROM {table}"));
    let documents = DashboardDocuments {
        requests: requests.len() as i32,
        notices: notices.len() as i32,
        contracts: contracts.len() as i32,
        request_revisions: count("request")?,
        notice_revisions: count("notice")?,
        contract_revisions: count("contract")?,
    };
    let stages = stages
        .iter()
        .map(|(key, label)| DashboardStage {
            key: (*key).into(),
            label: (*label).into(),
            count: rows
                .iter()
                .filter(|r| r.stage.as_deref() == Some(*key))
                .count() as i32,
        })
        .collect();
    let data_check_count = rows.iter().filter(|r| r.stage.is_none()).count() as i32;
    let total_cases = rows.len() as i32;
    // 늦은 것이 앞(게시일시, 없으면 접수일자, 없으면 첫 계약일자 — Ordinal), 같으면 열쇠 차례. 안정 정렬이다.
    rows.sort_by(|a, b| {
        cmp_ordinal(&sorts_on(b), &sorts_on(a)).then_with(|| cmp_ordinal(&a.key, &b.key))
    });
    Ok(DashboardReport {
        as_of: now.text.clone(),
        total_cases,
        stages,
        rows,
        documents,
        contract_amount: sum(&amounts)?.unwrap_or(Decimal::ZERO),
        contracts_with_amount: amounts.len() as i32,
        data_check_count,
        deliveries,
    })
}

fn sorts_on(row: &DashboardCase) -> String {
    first(&[
        row.posted_at.clone(),
        row.request_date.clone(),
        row.contracts
            .first()
            .map(|c| c.date.clone())
            .unwrap_or_default(),
    ])
}

/// `Enumerable.Sum` — 비면 `None`(부르는 쪽이 0 이나 null 로). 넘치면 .NET 의 `OverflowException` 글.
fn sum(amounts: &[Decimal]) -> Result<Option<Decimal>> {
    let mut total: Option<Decimal> = None;
    for amount in amounts {
        total = Some(match total {
            None => Decimal::ZERO
                .checked_add(*amount)
                .ok_or_else(decimal_overflow)?,
            Some(t) => t.checked_add(*amount).ok_or_else(decimal_overflow)?,
        });
    }
    Ok(total)
}

fn decimal_overflow() -> Error {
    Error::invalid("Value was either too large or too small for a Decimal.")
}

fn case(
    key: String,
    request: Option<&Row>,
    notice: Option<&Row>,
    notices: &[&Row],
    contracts: &[&Row],
    now: &Now,
) -> DashboardCase {
    let mut issues = Vec::new();
    let mut ordered: Vec<&Row> = contracts.to_vec();
    ordered.sort_by(|a, b| cmp_ordinal(&a.text("계약번호"), &b.text("계약번호")));
    let documents: Vec<DashboardContract> = ordered
        .iter()
        .map(|c| DashboardContract {
            number: c.text("계약번호"),
            title: c.text("계약건명"),
            date: c.text("계약일자"),
            amount: Decimal::parse_number(&c.text("계약금액")),
            draft: c.text("계약일자").is_empty(),
            terminated: c.text("해지") == "1",
        })
        .collect();
    let known: Vec<Decimal> = documents.iter().filter_map(|c| c.amount).collect();
    for contract in ordered.iter().filter(|c| c.text("해지") == "1") {
        issues.push(format!("해지 표시 계약: {}", contract.text("계약번호")));
    }
    if documents.iter().any(|c| c.amount.is_none()) {
        issues.push("금액이 없거나 숫자로 읽히지 않는 계약 문서가 있습니다.".into());
    }
    match request {
        None => issues.push("접수 문서가 이 건에 연결되어 있지 않습니다.".into()),
        Some(r) if r.text("취소품목수") != "0" => issues.push(format!(
            "취소 표시 접수 품목 {}개가 있습니다.",
            r.text("취소품목수")
        )),
        Some(_) => {}
    }
    // 접수만 있고 공고가 없는 것은 공고 대기 그 자체라 확인할 거리가 아니다. 계약이 공고 없이 선 것은 확인할 거리다.
    if notices.is_empty() {
        if !contracts.is_empty() {
            issues.push("연결된 공고 문서가 없습니다.".into());
        }
    } else if notice.is_none() {
        issues.push("관련공고 관계로 현행 공고 하나를 정할 수 없습니다.".into());
    }
    if notices.len() > 1 {
        let numbers: Vec<String> = notices.iter().map(|n| n.text("입찰공고번호")).collect();
        issues.push(format!(
            "관련공고 {}개 계열을 한 건으로 셉니다: {}",
            notices.len(),
            numbers.join(", ")
        ));
    }
    let opening = kst_date(&text(notice, "개찰일시"));
    let posted = kst_date(&text(notice, "게시일시"));
    let is_cancelled = cancelled(notice);
    let drafts = documents
        .iter()
        .filter(|c| !c.number.is_empty() && c.draft)
        .count() as i32;
    // 실공고: 게시일시가 읽히고 이미 지났으며 취소공고가 아닌 공고. 현행을 정하지 못하면 수집된 공고 모두를 본다.
    let real = |n: &Row| {
        kst_date(&n.text("게시일시")).is_some_and(|at| at <= now.instant) && !cancelled(Some(n))
    };
    let considered: Vec<&Row> = match notice {
        Some(n) => vec![n],
        None => notices.to_vec(),
    };
    let stage: Option<&str>;
    if !documents.is_empty() {
        if documents
            .iter()
            .any(|c| c.number.is_empty() || (!c.date.is_empty() && kst_date(&c.date).is_none()))
        {
            stage = None;
            issues.push("계약번호·계약일자를 확인하세요.".into());
        } else {
            stage = Some(if drafts > 0 {
                "awaitingContract"
            } else {
                "contracted"
            });
        }
        if is_cancelled {
            issues.push("취소공고와 계약 문서가 함께 있습니다. 계약 상태를 확인하세요.".into());
        }
    } else if request.is_some() && !considered.iter().any(|n| real(n)) {
        // 접수 후 게시된 현행 공고가 없다 — 공고 문서 없음, 게시 전, 게시일시 미기재, 현행 취소를 모두 포함한다.
        stage = Some("requestOnly");
        if considered
            .iter()
            .any(|n| !n.text("게시일시").is_empty() && kst_date(&n.text("게시일시")).is_none())
        {
            issues.push("게시일시를 확인하세요.".into());
        }
        if !text(notice, "개찰일시").is_empty() && opening.is_none() {
            issues.push("개찰일시를 확인하세요.".into());
        }
        if is_cancelled {
            issues.push("현행 취소공고가 있습니다. 후속 재공고를 확인하세요.".into());
        }
    } else if notice.is_none() {
        stage = None;
    } else if is_cancelled {
        stage = None;
        issues.push("현행 취소공고가 있습니다.".into());
    } else if posted.is_none()
        || (!text(notice, "개찰일시").is_empty() && opening.is_none())
        || matches!((opening, posted), (Some(o), Some(p)) if o < p)
    {
        stage = None;
        issues.push("게시일시·개찰일시를 확인하세요.".into());
    } else if posted.is_some_and(|p| p > now.instant) {
        stage = None;
        issues.push("게시 예정 시각 전 공고이며 접수 문서가 연결되어 있지 않습니다.".into());
    } else if let Some(opening) = opening {
        stage = Some(if opening <= now.instant {
            "awaitingContract"
        } else {
            "open"
        });
    } else {
        stage = None;
        issues.push("개찰일시를 확인하세요.".into());
    }
    // 현행이 없으면 게시일시가 늦은 것(Ordinal, 같으면 먼저 온 것).
    let display = notice.or_else(|| {
        let mut latest: Option<&Row> = None;
        for n in notices {
            if latest.is_none_or(|l| cmp_ordinal(&n.text("게시일시"), &l.text("게시일시")).is_gt())
            {
                latest = Some(n);
            }
        }
        latest
    });
    let notice_number = text(notice, "입찰공고번호");
    let mut numbers: Vec<String> = notices.iter().map(|n| n.text("입찰공고번호")).collect();
    numbers.sort_by(|a, b| {
        u8::from(*a != notice_number)
            .cmp(&u8::from(*b != notice_number))
            .then_with(|| cmp_ordinal(a, b))
    });
    let title = first(&[
        text(display, "공고명"),
        text(request, "요청명"),
        documents
            .first()
            .map(|c| c.title.clone())
            .unwrap_or_default(),
        "제목 없음".into(),
    ]);
    let agency = first(&[
        text(request, "수요기관"),
        text(notice, "수요기관"),
        text(notice, "공고기관"),
        text(ordered.first().copied(), "수요기관"),
    ]);
    let contract_amount = if known.is_empty() {
        None
    } else {
        // 금액 합이 넘치는 일은 없다고 본다 — 넘치면 .NET 은 던지고 여기서는 그 건만 금액을 비운다.
        sum(&known).ok().flatten()
    };
    let terminated = documents.iter().filter(|c| c.terminated).count() as i32;
    DashboardCase {
        key,
        title,
        stage: stage.map(String::from),
        request_number: text(request, "접수번호"),
        notice_number,
        notice_numbers: numbers,
        notice_kind: text(notice, "공고종류"),
        request_date: text(request, "접수일자"),
        posted_at: text(notice, "게시일시"),
        bid_starts_at: text(notice, "입찰개시일시"),
        bid_closes_at: text(notice, "입찰마감일시"),
        opening_at: text(notice, "개찰일시"),
        agency,
        contracts: documents,
        contract_amount,
        issues,
        draft_contracts: drafts,
        notice_cancelled: is_cancelled,
        terminated_contracts: terminated,
    }
}

fn delivery(contract: &Row, source_items: &[Row], now: &Now) -> Result<DeliveryContract> {
    let due = deadline(&contract.text("납품기한"), now);
    let mut numbered = Vec::with_capacity(source_items.len());
    for item in source_items {
        numbered.push((to_int32(item.value("순번"))?, item));
    }
    numbered.sort_by_key(|(line, _)| *line);
    let items: Vec<DeliveryItem> = numbered
        .into_iter()
        .map(|(line_no, i)| DeliveryItem {
            line_no,
            name: i.text("품명"),
            specification: i.text("규격"),
            agency: i.text("수요기관"),
            quantity: i.text("수량"),
            unit: i.text("단위"),
            due: deadline(&i.text("납품기한"), now),
        })
        .collect();
    let mut issues = Vec::new();
    if due.date.is_none() {
        issues.push(if due.raw.is_empty() {
            "계약 전체 납품기한이 없습니다.".into()
        } else {
            "계약 전체 납품기한을 날짜로 읽을 수 없습니다.".into()
        });
    }
    for item in items
        .iter()
        .filter(|i| i.due.date.is_none() && !(i.due.raw.is_empty() && due.date.is_some()))
    {
        issues.push(format!(
            "품목 {}: {}",
            item.line_no,
            if item.due.raw.is_empty() {
                "납품기한이 없습니다."
            } else {
                "납품기한을 날짜로 읽을 수 없습니다."
            }
        ));
    }
    let date = contract.text("계약일자");
    if !date.is_empty() && kst_date(&date).is_none() {
        issues.push("계약일자를 읽을 수 없습니다.".into());
    }
    Ok(DeliveryContract {
        number: contract.text("계약번호"),
        title: contract.text("계약건명"),
        contract_date: date.clone(),
        party: contract.text("계약상대자"),
        agency: contract.text("수요기관"),
        terminated: contract.text("해지") == "1",
        draft: date.is_empty(),
        due,
        items,
        issues,
    })
}

fn deadline(raw: &str, now: &Now) -> DeliveryDeadline {
    match kst_local(raw) {
        None => DeliveryDeadline {
            raw: raw.to_string(),
            date: None,
            days_remaining: None,
        },
        Some(local) => DeliveryDeadline {
            raw: raw.to_string(),
            date: Some(local.day_text()),
            days_remaining: Some((day_number(local) - now.day) as i32),
        },
    }
}

/// 1970-01-01 부터의 날 수(그 날짜를 달력 그대로).
fn day_number(local: LocalDateTime) -> i64 {
    local.date().unix_seconds().div_euclid(86_400)
}

fn first(values: &[String]) -> String {
    values
        .iter()
        .find(|v| !v.is_empty())
        .cloned()
        .unwrap_or_default()
}

fn cancelled(notice: Option<&Row>) -> bool {
    matches!(
        text(notice, "공고종류").as_str(),
        "취소공고" | "실공고(취소공고)"
    )
}

/// C# `Reports.Date` — KST 로 읽은 순간(UTC 초, 100나노초). 못 읽으면 `None`.
fn kst_date(text: &str) -> Option<(i64, u32)> {
    kst_local(text).map(|local| (local.unix_seconds() - KST, 0))
}

/// `DateTimeOffset.TryParseExact(text + " +09:00", ["yyyy/MM/dd zzz", "yyyy/MM/dd HH:mm:ss zzz", "yyyy-MM-dd zzz",
/// "yyyy-MM-dd HH:mm:ss zzz"], InvariantCulture, None)` 의 달력 쪽. 자리는 ASCII 숫자 정확히 그 수만, 앞뒤 공백 없이.
/// UTC 로 `0001-01-01` 앞이 되는 순간(0001/01/01 09:00 전)은 .NET 이 범위 밖으로 거절한다(실측).
fn kst_local(text: &str) -> Option<LocalDateTime> {
    let bytes = text.as_bytes();
    let digits = |from: usize, len: usize| -> Option<u32> {
        let part = bytes.get(from..from + len)?;
        if !part.iter().all(u8::is_ascii_digit) {
            return None;
        }
        Some(part.iter().fold(0, |n, b| n * 10 + u32::from(b - b'0')))
    };
    if bytes.len() != 10 && bytes.len() != 19 {
        return None;
    }
    let separator = bytes[4];
    if !(separator == b'/' || separator == b'-') || bytes[7] != separator {
        return None;
    }
    let (year, month, day) = (digits(0, 4)?, digits(5, 2)?, digits(8, 2)?);
    let (mut hour, mut minute, mut second) = (0, 0, 0);
    if bytes.len() == 19 {
        if bytes[10] != b' ' || bytes[13] != b':' || bytes[16] != b':' {
            return None;
        }
        hour = digits(11, 2)?;
        minute = digits(14, 2)?;
        second = digits(17, 2)?;
    }
    if year == 0
        || !(1..=12).contains(&month)
        || day == 0
        || day > days_in_month(i64::from(year), month)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return None;
    }
    let local = LocalDateTime {
        year: i64::from(year),
        month,
        day,
        hour,
        minute,
        second,
        ticks: 0,
    };
    (local.unix_seconds() - KST >= MIN_INSTANT).then_some(local)
}

/// `Convert.ToInt32(object, InvariantCulture)` — 정수는 범위를 보고, 실수는 짝수 쪽으로 반올림하고, 글은
/// `int.Parse`(앞뒤 공백·앞 부호)로, NULL 은 0. 그 밖은 .NET 의 예외 글.
pub(crate) fn to_int32(value: ValueRef<'_>) -> Result<i32> {
    let overflow = || Error::invalid("Value was either too large or too small for an Int32.");
    match value {
        ValueRef::Null => Ok(0),
        ValueRef::Integer(i) => i32::try_from(i).map_err(|_| overflow()),
        ValueRef::Real(d) => {
            let r = d.round_ties_even();
            if r.is_nan() || r < f64::from(i32::MIN) || r > f64::from(i32::MAX) {
                Err(overflow())
            } else {
                Ok(r as i32)
            }
        }
        ValueRef::Text(t) => {
            let s = String::from_utf8_lossy(t);
            let trimmed = s.trim_matches(|c: char| matches!(c, '\u{9}'..='\u{d}' | ' '));
            let (negative, digits) = match trimmed.as_bytes().first() {
                Some(b'+') => (false, &trimmed[1..]),
                Some(b'-') => (true, &trimmed[1..]),
                _ => (false, trimmed),
            };
            if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
                return Err(Error::invalid(format!(
                    "The input string '{s}' was not in a correct format."
                )));
            }
            let significant = digits.trim_start_matches('0');
            let magnitude: i64 = if significant.is_empty() {
                0
            } else {
                significant.parse().map_err(|_| overflow())?
            };
            i32::try_from(if negative { -magnitude } else { magnitude }).map_err(|_| overflow())
        }
        ValueRef::Blob(_) => Err(Error::invalid(
            "Unable to cast object of type 'System.Byte[]' to type 'System.IConvertible'.",
        )),
    }
}

/// `connection.ExecuteScalar<int>` — `COUNT(*)` 를 32비트로.
fn scalar_i32(connection: &Connection, sql: &str) -> Result<i32> {
    let value = connection.query_row(sql, [], |r| {
        Ok(crate::storage::status::scalar_i64(r.get_ref(0)?))
    })?;
    i32::try_from(value)
        .map_err(|_| Error::invalid("Value was either too large or too small for an Int32."))
}

/// Dapper 의 `(string)row.X` — 글은 글, NULL 은 `None`. 다른 형이면 .NET 의 `InvalidCastException` 글.
pub(crate) fn cast_string(value: ValueRef<'_>) -> Result<Option<String>> {
    let type_name = match value {
        ValueRef::Null => return Ok(None),
        ValueRef::Text(t) => return Ok(Some(String::from_utf8_lossy(t).into_owned())),
        ValueRef::Integer(_) => "System.Int64",
        ValueRef::Real(_) => "System.Double",
        ValueRef::Blob(_) => "System.Byte[]",
    };
    Err(Error::invalid(format!(
        "Unable to cast object of type '{type_name}' to type 'System.String'."
    )))
}

fn object(entries: Vec<(&str, Value)>) -> Value {
    let mut map = Map::new();
    for (k, v) in entries {
        map.insert(k.to_string(), v);
    }
    Value::Object(map)
}

fn strings(items: &[String]) -> Value {
    Value::Array(items.iter().cloned().map(Value::String).collect())
}

fn opt(value: &Option<String>) -> Value {
    value.clone().map_or(Value::Null, Value::String)
}

/// `decimal?` 의 JSON — 수(자리 그대로) 또는 `null`.
fn money(amount: Option<Decimal>) -> Value {
    amount.map_or(Value::Null, |a| json::number(&a.to_text()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// .NET 8 실측(`TryParseExact(t + " +09:00", …)`).
    #[test]
    fn reads_dates_like_dotnet() {
        let ok = |t: &str| kst_local(t).map(|l| l.second_text());
        assert_eq!(ok("2026/09/20").as_deref(), Some("2026-09-20T00:00:00"));
        assert_eq!(ok("2026-09-20").as_deref(), Some("2026-09-20T00:00:00"));
        assert_eq!(
            ok("2026/09/20 08:00:00").as_deref(),
            Some("2026-09-20T08:00:00")
        );
        assert_eq!(
            ok("2026-09-20 08:00:00").as_deref(),
            Some("2026-09-20T08:00:00")
        );
        assert_eq!(ok("9999/12/31").as_deref(), Some("9999-12-31T00:00:00"));
        assert_eq!(ok("2024-02-29").as_deref(), Some("2024-02-29T00:00:00"));
        assert_eq!(
            ok("0001/01/01 09:00:00").as_deref(),
            Some("0001-01-01T09:00:00")
        );
        for bad in [
            "2026/9/20",
            "2026/09/20 8:00:00",
            "0001/01/01",
            "2026-02-29",
            "2026/09/20 24:00:00",
            " 2026/09/20",
            "2026/09/20 ",
            "20260/09/20",
            "2026/09/20T08:00:00",
            "2026/09/20 08:00",
            "2026/09/20  08:00:00",
            "２０２６/09/20",
            "2026/09/20 08:00:60",
            "0000/01/01",
            "2026/09-20",
            "",
        ] {
            assert_eq!(ok(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn converts_like_convert_to_int32() {
        use rusqlite::types::ValueRef as V;
        assert_eq!(to_int32(V::Text(b"3")).unwrap(), 3);
        assert_eq!(to_int32(V::Text(b" 3 ")).unwrap(), 3);
        assert_eq!(to_int32(V::Text(b"+3")).unwrap(), 3);
        assert_eq!(to_int32(V::Text(b"-3")).unwrap(), -3);
        assert_eq!(to_int32(V::Real(3.5)).unwrap(), 4);
        assert_eq!(to_int32(V::Real(2.5)).unwrap(), 2);
        assert_eq!(to_int32(V::Null).unwrap(), 0);
        assert_eq!(
            to_int32(V::Text(b"3.0")).unwrap_err().to_string(),
            "The input string '3.0' was not in a correct format."
        );
        assert_eq!(
            to_int32(V::Integer(3_000_000_000)).unwrap_err().to_string(),
            "Value was either too large or too small for an Int32."
        );
    }
}

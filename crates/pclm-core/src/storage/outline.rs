//! C# `Outline` — 축적된 것을 **생긴 모양 그대로** 낸다: 접수 → 공고(건) → 계약 → 명세.
//!
//! 값은 계약면 뷰에서 읽는다(금액 자릿점·날짜 표기를 여기서 다시 짜지 않는다). 건과 현행도 뷰의 `공고건`·`현행공고` 를
//! 따른다 — 「무엇이 현행인가」 의 규칙을 여기 한 벌 더 두지 않는다. 차수는 접어서 이름표로만 낸다.
//!
//! 차례는 C# 과 같다: 건은 `v_공고` 에 처음 나온 차례로 돌고, 사슬은 마지막에 `SortsOn` 내림차순·`Key` 오름차순
//! (Ordinal, 안정 정렬)으로 선다. `ToDictionary` 가 같은 열쇠에서 거절하는 자리는 .NET 의 글로 거절한다.

use serde_json::{Map, Value};

use crate::error::Result;
use crate::storage::database::Database;
use crate::storage::reports::{Row, cast_string, find, insert_unique, links, read, to_int32};
use crate::text::cmp_ordinal;

/// 명세 한 줄.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutlineItem {
    pub line_no: i32,
    pub name: String,
    pub specification: String,
    pub quantity: String,
    pub unit: String,
    pub unit_price: String,
    pub amount: String,
}

/// 계약 하나. `revisions` 는 이 계약번호로 쌓인 차수를 낮은 것부터.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutlineContract {
    pub key: String,
    pub number: String,
    pub title: String,
    pub contracted_on: String,
    pub amount: String,
    pub counterparty: String,
    pub demand_agency: String,
    pub revisions: Vec<String>,
    pub items: Vec<OutlineItem>,
}

/// 공고 하나. 계약을 품지 않는다 — 사슬이 품는다.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutlineNotice {
    pub key: String,
    pub number: String,
    pub title: String,
    pub posted_at: String,
    pub agency: String,
    pub estimated_price: String,
    pub revisions: Vec<String>,
}

/// 사슬의 가운데 칸 — 공고 하나가 아니라 **건** 하나. `current` 는 없을 수 있다(서로를 가리키는 건).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutlineNoticeGroup {
    pub group_base: String,
    pub current: Option<OutlineNotice>,
    /// 늦은 것부터.
    pub superseded: Vec<OutlineNotice>,
}

/// 접수 하나. `request_numbers` 는 이 접수서가 묶은 조달요구번호 전부.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutlineRequest {
    pub key: String,
    pub number: String,
    pub title: String,
    pub received_on: String,
    pub goods_amount: String,
    pub budget_amount: String,
    pub demand_agency: String,
    pub request_numbers: Vec<String>,
    pub revisions: Vec<String>,
}

/// 조달 건 하나 — 접수 → 공고 → 계약. 셋 중 어느 것이 없어도 선다.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutlineChain {
    pub key: String,
    pub request: Option<OutlineRequest>,
    pub notice: Option<OutlineNoticeGroup>,
    pub contracts: Vec<OutlineContract>,
}

impl OutlineChain {
    /// C# `SortsOn` — 현행의 게시일, 없으면 대체된 것 중 가장 이른 게시일, 없으면 접수일, 없으면 첫 계약의 계약일.
    pub fn sorts_on(&self) -> String {
        if let Some(posted) = self
            .notice
            .as_ref()
            .and_then(|n| n.current.as_ref())
            .map(|c| c.posted_at.as_str())
            .filter(|p| !p.is_empty())
        {
            return posted.to_string();
        }
        if let Some(superseded) = self.notice.as_ref().and_then(|n| {
            n.superseded
                .iter()
                .map(|s| s.posted_at.as_str())
                .filter(|p| !p.is_empty())
                .min_by(|a, b| cmp_ordinal(a, b))
        }) {
            return superseded.to_string();
        }
        if let Some(received) = self
            .request
            .as_ref()
            .map(|r| r.received_on.as_str())
            .filter(|r| !r.is_empty())
        {
            return received.to_string();
        }
        self.contracts
            .first()
            .map(|c| c.contracted_on.clone())
            .unwrap_or_default()
    }
}

/// 쌓인 것을 생긴 모양대로 낸 것.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutlineTree {
    pub chains: Vec<OutlineChain>,
}

impl OutlineTree {
    /// C# `Bridge.ReadOutline` 이 싣는 꼴 — 이름은 camelCase, 없는 접수·건·현행은 `null`.
    pub fn to_json(&self) -> Value {
        let chains = self
            .chains
            .iter()
            .map(|c| {
                object(vec![
                    ("key", c.key.clone().into()),
                    (
                        "request",
                        c.request.as_ref().map_or(Value::Null, request_json),
                    ),
                    ("notice", c.notice.as_ref().map_or(Value::Null, group_json)),
                    (
                        "contracts",
                        Value::Array(c.contracts.iter().map(contract_json).collect()),
                    ),
                ])
            })
            .collect();
        object(vec![("chains", Value::Array(chains))])
    }
}

fn strings(values: &[String]) -> Value {
    Value::Array(values.iter().cloned().map(Value::String).collect())
}

fn contract_json(c: &OutlineContract) -> Value {
    let items = c
        .items
        .iter()
        .map(|i| {
            object(vec![
                ("lineNo", i.line_no.into()),
                ("name", i.name.clone().into()),
                ("specification", i.specification.clone().into()),
                ("quantity", i.quantity.clone().into()),
                ("unit", i.unit.clone().into()),
                ("unitPrice", i.unit_price.clone().into()),
                ("amount", i.amount.clone().into()),
            ])
        })
        .collect();
    object(vec![
        ("key", c.key.clone().into()),
        ("number", c.number.clone().into()),
        ("title", c.title.clone().into()),
        ("contractedOn", c.contracted_on.clone().into()),
        ("amount", c.amount.clone().into()),
        ("counterparty", c.counterparty.clone().into()),
        ("demandAgency", c.demand_agency.clone().into()),
        ("revisions", strings(&c.revisions)),
        ("items", Value::Array(items)),
    ])
}

fn request_json(r: &OutlineRequest) -> Value {
    object(vec![
        ("key", r.key.clone().into()),
        ("number", r.number.clone().into()),
        ("title", r.title.clone().into()),
        ("receivedOn", r.received_on.clone().into()),
        ("goodsAmount", r.goods_amount.clone().into()),
        ("budgetAmount", r.budget_amount.clone().into()),
        ("demandAgency", r.demand_agency.clone().into()),
        ("requestNumbers", strings(&r.request_numbers)),
        ("revisions", strings(&r.revisions)),
    ])
}

fn notice_json(n: &OutlineNotice) -> Value {
    object(vec![
        ("key", n.key.clone().into()),
        ("number", n.number.clone().into()),
        ("title", n.title.clone().into()),
        ("postedAt", n.posted_at.clone().into()),
        ("agency", n.agency.clone().into()),
        ("estimatedPrice", n.estimated_price.clone().into()),
        ("revisions", strings(&n.revisions)),
    ])
}

fn group_json(g: &OutlineNoticeGroup) -> Value {
    object(vec![
        ("groupBase", g.group_base.clone().into()),
        (
            "current",
            g.current.as_ref().map_or(Value::Null, notice_json),
        ),
        (
            "superseded",
            Value::Array(g.superseded.iter().map(notice_json).collect()),
        ),
    ])
}

fn object(entries: Vec<(&str, Value)>) -> Value {
    let mut map = Map::new();
    for (k, v) in entries {
        map.insert(k.to_string(), v);
    }
    Value::Object(map)
}

/// C# `Outline`.
pub struct Outline {
    database: Database,
}

impl Outline {
    pub fn new(database: Database) -> Self {
        Outline { database }
    }

    /// `Outline.Build()`.
    pub fn build(&self) -> Result<OutlineTree> {
        let connection = self.database.open()?;
        let connection = &connection;

        let revisions = revisions(connection)?;
        let items = items(connection)?;
        let revisions_of =
            |key: &str| -> Vec<String> { find(&revisions, key).cloned().unwrap_or_default() };

        let contracts: Vec<OutlineContract> = read(
            connection,
            "SELECT 계약본번호, 계약번호, 계약건명, 계약일자, 계약금액, 계약상대자, 수요기관\nFROM v_계약;\n",
        )?
        .iter()
        .map(|row| {
            let key = row.text("계약본번호");
            let number = row.text("계약번호");
            OutlineContract {
                revisions: revisions_of(&format!("contract {key}")),
                items: find(&items, &number).cloned().unwrap_or_default(),
                title: row.text("계약건명"),
                contracted_on: row.text("계약일자"),
                amount: row.text("계약금액"),
                counterparty: row.text("계약상대자"),
                demand_agency: row.text("수요기관"),
                key,
                number,
            }
        })
        .collect();

        // (건, 현행, 공고) — 계열이 서지 않아 건 이름이 비면 제 본번호로 선다.
        let notice_rows: Vec<(String, String, OutlineNotice)> = read(
            connection,
            "SELECT 공고본번호, 입찰공고번호, 공고명, 게시일시, 공고기관, 추정가격, 공고건, 현행공고\nFROM v_공고;\n",
        )?
        .iter()
        .map(|row| {
            let key = row.text("공고본번호");
            let group = row.text("공고건");
            (
                if group.is_empty() { key.clone() } else { group },
                row.text("현행공고"),
                OutlineNotice {
                    revisions: revisions_of(&format!("notice {key}")),
                    number: row.text("입찰공고번호"),
                    title: row.text("공고명"),
                    posted_at: row.text("게시일시"),
                    agency: row.text("공고기관"),
                    estimated_price: row.text("추정가격"),
                    key,
                },
            )
        })
        .collect();

        // GroupBy — 처음 나온 차례.
        let mut grouped: Vec<(String, Vec<(String, OutlineNotice)>)> = Vec::new();
        for (group, current, notice) in notice_rows {
            match grouped.iter_mut().find(|(g, _)| *g == group) {
                Some((_, members)) => members.push((current, notice)),
                None => grouped.push((group, vec![(current, notice)])),
            }
        }
        let groups: Vec<OutlineNoticeGroup> = grouped
            .into_iter()
            .map(|(group_base, members)| {
                let current = members
                    .iter()
                    .find(|(current, n)| n.number == *current)
                    .map(|(_, n)| n.clone());
                let mut superseded: Vec<OutlineNotice> = members
                    .into_iter()
                    .map(|(_, n)| n)
                    .filter(|n| current.as_ref().is_none_or(|c| n.key != c.key))
                    .collect();
                // 늦은 것이 앞.
                superseded.sort_by(|a, b| {
                    cmp_ordinal(&b.posted_at, &a.posted_at)
                        .then_with(|| cmp_ordinal(&b.key, &a.key))
                });
                OutlineNoticeGroup {
                    group_base,
                    current,
                    superseded,
                }
            })
            .collect();

        let requests: Vec<OutlineRequest> = read(
            connection,
            "SELECT 접수본번호, 접수번호, 요청명, 접수일자, 품대, 예산금액, 수요기관, 조달요구번호\nFROM v_접수;\n",
        )?
        .iter()
        .map(|row| {
            let key = row.text("접수본번호");
            OutlineRequest {
                revisions: revisions_of(&format!("request {key}")),
                number: row.text("접수번호"),
                title: row.text("요청명"),
                received_on: row.text("접수일자"),
                goods_amount: row.text("품대"),
                budget_amount: row.text("예산금액"),
                demand_agency: row.text("수요기관"),
                // `Split(',', RemoveEmptyEntries | TrimEntries)`.
                request_numbers: row
                    .text("조달요구번호")
                    .split(',')
                    .map(dotnet_trim)
                    .filter(|s| !s.is_empty())
                    .map(String::from)
                    .collect(),
                key,
            }
        })
        .collect();

        let contract_to_notice = links(
            connection,
            "SELECT contract_base, notice_group FROM project_link;",
        )?;
        // 접수:건이 1:1 이라 건에서 접수로 되짚는다(C# 은 NoticeGroup 을 열쇠로 ToDictionary).
        let notice_to_request = links(
            connection,
            "SELECT notice_group, request_base FROM request_link;",
        )?;

        let mut by_notice: Vec<(String, Vec<OutlineContract>)> = Vec::new();
        for contract in &contracts {
            if let Some(group) = find(&contract_to_notice, &contract.key) {
                match by_notice.iter_mut().find(|(g, _)| g == group) {
                    Some((_, list)) => list.push(contract.clone()),
                    None => by_notice.push((group.clone(), vec![contract.clone()])),
                }
            }
        }
        for (_, list) in &mut by_notice {
            ordered(list);
        }

        let mut request_by_base: Vec<(String, OutlineRequest)> = Vec::new();
        for request in &requests {
            insert_unique(&mut request_by_base, request.key.clone(), request.clone())?;
        }
        let mut chains = Vec::new();
        let mut placed: Vec<String> = Vec::new();

        for group in groups {
            let mut request = None;
            if let Some(request_base) = find(&notice_to_request, &group.group_base)
                && let Some(found) = find(&request_by_base, request_base)
            {
                request = Some(found.clone());
                placed.push(request_base.clone());
            }
            let contracts = find(&by_notice, &group.group_base)
                .cloned()
                .unwrap_or_default();
            chains.push(OutlineChain {
                key: key(request.as_ref(), Some(&group), &[]),
                request,
                notice: Some(group),
                contracts,
            });
        }

        for request in requests.iter().filter(|r| !placed.contains(&r.key)) {
            chains.push(OutlineChain {
                key: key(Some(request), None, &[]),
                request: Some(request.clone()),
                notice: None,
                contracts: Vec::new(),
            });
        }

        let mut loose: Vec<OutlineContract> = contracts
            .iter()
            .filter(|c| find(&contract_to_notice, &c.key).is_none())
            .cloned()
            .collect();
        ordered(&mut loose);
        for contract in loose {
            chains.push(OutlineChain {
                key: key(None, None, std::slice::from_ref(&contract)),
                request: None,
                notice: None,
                contracts: vec![contract],
            });
        }

        let mut keyed: Vec<(String, OutlineChain)> =
            chains.into_iter().map(|c| (c.sorts_on(), c)).collect();
        keyed.sort_by(|(a, x), (b, y)| cmp_ordinal(b, a).then_with(|| cmp_ordinal(&x.key, &y.key)));
        Ok(OutlineTree {
            chains: keyed.into_iter().map(|(_, c)| c).collect(),
        })
    }
}

/// 사슬의 이름 — 있는 것의 본번호를 `/` 로 잇는다. 접수도 건도 없을 때만 계약이 이름이 된다.
fn key(
    request: Option<&OutlineRequest>,
    notice: Option<&OutlineNoticeGroup>,
    contracts: &[OutlineContract],
) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if let Some(r) = request {
        parts.push(&r.key);
    }
    if let Some(n) = notice {
        parts.push(&n.group_base);
    }
    if parts.is_empty() {
        parts.extend(contracts.iter().map(|c| c.key.as_str()));
    }
    parts.join("/")
}

/// 계약일 늦은 것부터, 같으면 계약번호 차례(Ordinal, 안정).
fn ordered(contracts: &mut [OutlineContract]) {
    contracts.sort_by(|a, b| {
        cmp_ordinal(&b.contracted_on, &a.contracted_on)
            .then_with(|| cmp_ordinal(&a.number, &b.number))
    });
}

/// `string.Trim()` — .NET 의 공백(유니코드 White_Space). Rust 의 `trim` 과 같은 집합이다.
fn dotnet_trim(s: &str) -> &str {
    s.trim()
}

/// 쌓인 차수 — `"{종류} {본번호}"` 마다 `CAST(seq AS INTEGER)` 차례로.
fn revisions(connection: &rusqlite::Connection) -> Result<Vec<(String, Vec<String>)>> {
    let mut all: Vec<(String, Vec<String>)> = Vec::new();
    for (table, column, kind) in [
        ("request", "request_base", "request"),
        ("notice", "notice_base", "notice"),
        ("contract", "contract_base", "contract"),
    ] {
        let mut statement = connection.prepare(&format!(
            "SELECT {column}, seq FROM {table} ORDER BY CAST(seq AS INTEGER);"
        ))?;
        let mut rows = statement.query([])?;
        let mut grouped: Vec<(String, Vec<String>)> = Vec::new();
        while let Some(row) = rows.next()? {
            let base = cast_string(row.get_ref(0)?)?.unwrap_or_default();
            let seq = cast_string(row.get_ref(1)?)?.unwrap_or_default();
            match grouped.iter_mut().find(|(b, _)| *b == base) {
                Some((_, list)) => list.push(seq),
                None => grouped.push((base, vec![seq])),
            }
        }
        for (base, list) in grouped {
            let key = format!("{kind} {base}");
            match all.iter_mut().find(|(k, _)| *k == key) {
                Some((_, existing)) => *existing = list,
                None => all.push((key, list)),
            }
        }
    }
    Ok(all)
}

/// 계약번호로 묶은 명세. 뷰가 이미 최신 차수만 낸다.
fn items(connection: &rusqlite::Connection) -> Result<Vec<(String, Vec<OutlineItem>)>> {
    let rows = read(
        connection,
        "SELECT 계약번호, 순번, 품명, 규격, 수량, 단위, 단가, 금액\nFROM v_계약품목 ORDER BY 계약번호, 순번;\n",
    )?;
    let mut grouped: Vec<(String, Vec<&Row>)> = Vec::new();
    for row in &rows {
        let number = row.text("계약번호");
        match grouped.iter_mut().find(|(n, _)| *n == number) {
            Some((_, list)) => list.push(row),
            None => grouped.push((number, vec![row])),
        }
    }
    let mut out = Vec::new();
    for (number, rows) in grouped {
        let mut list = Vec::with_capacity(rows.len());
        for row in rows {
            list.push(OutlineItem {
                line_no: to_int32(row.value("순번"))?,
                name: row.text("품명"),
                specification: row.text("규격"),
                quantity: row.text("수량"),
                unit: row.text("단위"),
                unit_price: row.text("단가"),
                amount: row.text("금액"),
            });
        }
        out.push((number, list));
    }
    Ok(out)
}

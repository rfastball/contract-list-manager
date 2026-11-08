//! 현황(C# `Storage/Status.cs`) — 쌓인 것을 세어 **자료로 된 지표 목록**대로 낸다.
//!
//! 무엇을 보일지가 아니라, 무엇을 보일지 나중에 갈아끼울 수 있게 하는 것이 목표다 — 지표를 더하거나 빼는 일이
//! [`metrics`] 배열만 고치는 일이어야 한다. 화면·다리·형식은 그대로다. 읽기만 한다.

use rusqlite::types::ValueRef;
use serde_json::{Map, Value};

use crate::error::Result;
use crate::storage::database::Database;

/// C# `Metric(묶음, 이름, Sql, 거르개)` — 현황 한 칸의 정의.
///
/// - `묶음`: 이 칸이 설 자리. 같은 이름이면 한 묶음이고, 묶음은 배열에 **처음 나온 차례**로 선다.
/// - `sql`: 수 하나를 내는 스칼라 조회(`COUNT(*)` 는 64비트라 `i64`).
/// - `거르개`: 눌렀을 때 계획 탭이 걸 조건. 꼴은 `"열이름=값"` 하나뿐이다. 비면 누를 수 없는 줄이다.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Metric {
    /// `묶음`.
    pub group: String,
    /// `이름`.
    pub name: String,
    pub sql: String,
    /// `거르개`.
    pub filter: Option<String>,
}

impl Metric {
    pub fn new(group: &str, name: &str, sql: &str, filter: Option<&str>) -> Self {
        Metric {
            group: group.into(),
            name: name.into(),
            sql: sql.into(),
            filter: filter.map(Into::into),
        }
    }
}

/// C# `Cell(이름, 수, 거르개)` — 센 결과 한 칸.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    /// `이름`.
    pub name: String,
    /// `수`.
    pub count: i64,
    /// `거르개`.
    pub filter: Option<String>,
}

/// C# `Group(이름, Cells)` — 같은 묶음의 칸들.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    /// `이름`.
    pub name: String,
    pub cells: Vec<Cell>,
}

/// C# `StatusReport(Groups)` — 현황 한 벌. 화면과 명령줄은 오는 대로 그린다.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusReport {
    pub groups: Vec<Group>,
}

impl StatusReport {
    /// 다리(`Bridge.Json` — camelCase)가 쓰는 꼴. 한글 이름은 camelCase 가 바꾸지 않는다.
    pub fn to_json(&self) -> Value {
        let groups = self
            .groups
            .iter()
            .map(|g| {
                let cells = g
                    .cells
                    .iter()
                    .map(|c| {
                        let mut cell = Map::new();
                        cell.insert("이름".into(), Value::String(c.name.clone()));
                        cell.insert("수".into(), c.count.into());
                        cell.insert(
                            "거르개".into(),
                            c.filter.clone().map_or(Value::Null, Value::String),
                        );
                        Value::Object(cell)
                    })
                    .collect();
                let mut group = Map::new();
                group.insert("이름".into(), Value::String(g.name.clone()));
                group.insert("cells".into(), Value::Array(cells));
                Value::Object(group)
            })
            .collect();
        let mut map = Map::new();
        map.insert("groups".into(), Value::Array(groups));
        Value::Object(map)
    }
}

/// `Status.Metrics` — 붙박이 지표. **이 배열이 전부다 — 갈아끼울 자리.** 지금 것은 가상이다.
pub fn metrics() -> Vec<Metric> {
    vec![
        // 쌓인 것 — 표를 그대로 센다. 계획만 차수가 없어 행이 곧 건이다.
        Metric::new("쌓인 것", "계획", "SELECT COUNT(*) FROM plan;", None),
        Metric::new(
            "쌓인 것",
            "접수",
            "SELECT COUNT(DISTINCT request_base) FROM request;",
            None,
        ),
        Metric::new(
            "쌓인 것",
            "공고",
            "SELECT COUNT(DISTINCT notice_base) FROM notice;",
            None,
        ),
        // 「공고」는 본번호를 세므로 취소 뒤 재채번된 건을 둘로 센다. 그 옆에 조달 건이 몇인지를 함께 낸다(스키마 V15).
        // notice_group 을 그대로 세지 않는다 — 문서 없는 조상만으로 이름이 선 건까지 세면 쌓이지 않은 것이 한 건으로
        // 잡힌다.
        Metric::new(
            "쌓인 것",
            "공고건",
            "SELECT COUNT(DISTINCT s.group_base) FROM notice_series s\nJOIN notice n ON n.notice_base = s.notice_base;",
            None,
        ),
        Metric::new(
            "쌓인 것",
            "계약",
            "SELECT COUNT(DISTINCT contract_base) FROM contract;",
            None,
        ),
        // 단계 — v_계획 이 낸 것을 그대로 센다. 세는 곳과 보는 곳이 같아야 「17건」을 눌러 열었을 때 17줄이 선다.
        Metric::new(
            "단계",
            "미착수",
            "SELECT COUNT(*) FROM v_계획 WHERE 단계 = '미착수';",
            Some("단계=미착수"),
        ),
        Metric::new(
            "단계",
            "접수",
            "SELECT COUNT(*) FROM v_계획 WHERE 단계 = '접수';",
            Some("단계=접수"),
        ),
        Metric::new(
            "단계",
            "공고",
            "SELECT COUNT(*) FROM v_계획 WHERE 단계 = '공고';",
            Some("단계=공고"),
        ),
        Metric::new(
            "단계",
            "계약",
            "SELECT COUNT(*) FROM v_계획 WHERE 단계 = '계약';",
            Some("단계=계약"),
        ),
    ]
}

/// C# `Status(database, metrics)` — 쌓인 것을 세어 현황으로 낸다. 읽기만 한다.
pub struct Status {
    database: Database,
    metrics: Vec<Metric>,
}

impl Status {
    /// `new Status(database)` — 붙박이 지표로.
    pub fn new(database: Database) -> Self {
        Status::with_metrics(database, metrics())
    }

    /// `new Status(database, metrics)` — 지표를 갈아끼운다(시험이 이음매를 본다).
    pub fn with_metrics(database: Database, metrics: Vec<Metric>) -> Self {
        Status { database, metrics }
    }

    /// `Status.Report` — 지표를 차례대로 세어 묶음으로 접는다. 묶음은 배열에 처음 나온 차례로 세운다.
    pub fn report(&self) -> Result<StatusReport> {
        let connection = self.database.open_read_only()?;
        let mut groups: Vec<Group> = Vec::new();
        for metric in &self.metrics {
            let count = connection.query_row(&metric.sql, [], |r| Ok(scalar_i64(r.get_ref(0)?)))?;
            let cell = Cell {
                name: metric.name.clone(),
                count,
                filter: metric.filter.clone(),
            };
            match groups.iter_mut().find(|g| g.name == metric.group) {
                Some(group) => group.cells.push(cell),
                None => groups.push(Group {
                    name: metric.group.clone(),
                    cells: vec![cell],
                }),
            }
        }
        Ok(StatusReport { groups })
    }
}

/// Dapper `ExecuteScalar<long>` — 정수는 그대로, NULL 은 0, 실수는 반올림(짝수 쪽), 글은 수로 읽는다(못 읽으면 0).
pub(crate) fn scalar_i64(value: ValueRef<'_>) -> i64 {
    match value {
        ValueRef::Integer(i) => i,
        ValueRef::Real(d) => d.round_ties_even() as i64,
        ValueRef::Text(t) => std::str::from_utf8(t)
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0),
        ValueRef::Null | ValueRef::Blob(_) => 0,
    }
}

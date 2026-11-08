//! 연간 조달계획 엑셀을 읽어 `plan` 에 넣는다(C# `Storage/PlanImport.cs`).
//!
//! **서식은 표본에 고정한다**(ADR-023 개정). 열은 [`COLUMNS`] 하나로 못박고 머리글 이름이 **정확히 같은** 열만 잡는다 —
//! 틀리게 잡힌 열은 빈 열보다 고치기 어렵다. **필수 둘(조달요구번호·담당자)을 못 찾으면 한 줄도 넣지 않는다.**
//!
//! 값은 **적힌 그대로** 담는다(ADR-016) — 지시수량만 `ValueParser.Integer` 를 타고 나머지는 글이다. 칸을 글로 바꾸는 길은
//! **엑셀이 보여 주는 모양이 아니라 담긴 값**이다(C# `Raw`): 수는 `0.###…`(지수로 도망가지 않게), 날짜 꼴의 수는 날짜(시각이
//! 있으면 시각까지), 글은 앞뒤 공백을 걷는다. 칸의 형은 ClosedXML 이 읽은 형을 따른다([`crate::reader`]).
//!
//! **던지지 않고 담아 낸다.** 머리글이 어긋난 것은 사람이 파일을 다시 보면 되는 일이라 [`PlanImportResult`] 로 낸다.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use pclm_core::domain::value_parser;
use pclm_core::storage::database::begin;
use pclm_core::storage::store::text_of;
use pclm_core::{Database, Env, Error, Result};
use serde_json::{Map, Value};

use crate::dotnet;
use crate::reader::{self, CellData, CellValue, ReadError, Worksheet};

/// 담을 자리 → 표본의 머리글. **차례가 곧 표본의 차례**이고, 뷰와 명령줄이 보여 주는 차례이기도 하다.
pub const COLUMNS: &[(&str, &str)] = &[
    ("request_number", "조달요구번호"),
    ("stock_number", "재고번호"),
    ("item_name", "품명"),
    ("currency", "화폐구분"),
    ("unit", "단위"),
    ("quantity", "지시수량"),
    ("requesting_unit", "요청부대부서명"),
    ("requesting_officer", "요청부대담당자"),
    ("requesting_phone", "요청부대사용자전화번호"),
    ("contract_department", "계약부서"),
    ("officer", "담당자"),
    ("contact", "연락처"),
];

/// 이것을 못 찾으면 한 줄도 넣지 않는다.
pub const REQUIRED: &[&str] = &["request_number", "officer"];

/// C# `PlanImportResult` — 계획 엑셀 한 권을 읽은 결과.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlanImportResult {
    /// 머리글 아래에서 읽은 줄 수. 통째로 빈 줄은 세지 않는다.
    pub rows: i32,
    pub created: i32,
    pub updated: i32,
    /// 조달요구번호가 빈 줄.
    pub skipped: i32,
    /// 없던 필수 **머리글 이름**. 하나라도 있으면 한 줄도 들어가지 않았다.
    pub missing_required: Vec<String>,
    /// 실제로 잡힌 머리글(표본 차례).
    pub found: Vec<String>,
    /// 입력값 오류. 행 번호와 필드 이름을 담는다.
    pub errors: Vec<String>,
}

impl PlanImportResult {
    /// 필수 머리글과 입력값이 올바른가. 거짓이면 DB 는 손대지 않은 채다.
    pub fn ok(&self) -> bool {
        self.missing_required.is_empty() && self.errors.is_empty()
    }

    /// 다리의 꼴(camelCase, 선언 차례) — `rows`·`created`·`updated`·`skipped`·`missingRequired`·`found`·`errors`·`ok`.
    pub fn to_json(&self) -> Value {
        let strings = |v: &[String]| Value::Array(v.iter().cloned().map(Value::String).collect());
        let mut out = Map::new();
        out.insert("rows".into(), Value::from(self.rows));
        out.insert("created".into(), Value::from(self.created));
        out.insert("updated".into(), Value::from(self.updated));
        out.insert("skipped".into(), Value::from(self.skipped));
        out.insert("missingRequired".into(), strings(&self.missing_required));
        out.insert("found".into(), strings(&self.found));
        out.insert("errors".into(), strings(&self.errors));
        out.insert("ok".into(), Value::Bool(self.ok()));
        Value::Object(out)
    }
}

/// `PlanImport.Headers` — 첫 줄의 머리글. 빈 칸도 자리를 지켜 그대로 낸다(차례가 곧 열 번호다).
pub fn headers(path: &Path) -> Result<Vec<String>> {
    let sheet = open(path)?;
    let header_row = sheet.as_ref().and_then(Worksheet::first_row_used);
    headers_of(sheet.as_ref(), header_row)
}

/// `PlanImport.Import` — 엑셀 한 권을 읽어 `request_number` 기준으로 업서트한다. 계획변경은 차수를 올리지 않고 덮어쓴다.
pub fn import(database: &Database, path: &Path, env: &dyn Env) -> Result<PlanImportResult> {
    let sheet = open(path)?;
    let header_row = sheet.as_ref().and_then(Worksheet::first_row_used);
    let headers = headers_of(sheet.as_ref(), header_row)?;

    let found = resolve(&headers);
    let missing: Vec<String> = REQUIRED
        .iter()
        .filter(|t| !found.contains_key(*t))
        .map(|t| header_of(t).to_owned())
        .collect();
    // 못 찾았으면 줄을 세지도 않는다.
    let read: Vec<String> = COLUMNS
        .iter()
        .filter(|(t, _)| found.contains_key(t))
        .map(|(_, h)| (*h).to_owned())
        .collect();
    let (Some(sheet), Some(header_row), true) = (sheet, header_row, missing.is_empty()) else {
        return Ok(PlanImportResult {
            missing_required: missing,
            found: read,
            ..Default::default()
        });
    };

    let last_row = sheet.last_row_used().unwrap_or(header_row);
    let now = env.now_utc();
    let source_name = file_name(path);

    let connection = database.open()?;
    let transaction = begin(&connection)?;

    // 이미 있던 번호를 미리 걷어 신규와 갱신을 가른다. 한 파일 안에 같은 번호가 두 번 나오면 두 번째는 갱신이다.
    let mut seen: HashSet<String> = HashSet::new();
    {
        let mut statement = transaction.prepare("SELECT request_number FROM plan;")?;
        let mut cursor = statement.query([])?;
        while let Some(row) = cursor.next()? {
            if let Some(number) = text_of(row.get_ref(0)?) {
                seen.insert(number);
            }
        }
    }

    let mut result = PlanImportResult {
        found: read,
        ..Default::default()
    };
    for r in header_row + 1..=last_row {
        let mut values: HashMap<&str, String> = HashMap::new();
        for (target, _) in COLUMNS {
            let value = match found.get(target) {
                Some(&column) => raw(sheet.cell(r, column))?,
                None => String::new(),
            };
            values.insert(target, value);
        }

        // 통째로 빈 줄은 자료가 아니다.
        if values.values().all(String::is_empty) {
            continue;
        }
        result.rows += 1;

        let number = values["request_number"].clone();
        if number.is_empty() {
            result.skipped += 1;
            continue;
        }

        let written = &values["quantity"];
        let quantity = value_parser::integer(Some(written));
        if !written.is_empty() && quantity.is_none() {
            // 트랜잭션을 완료하지 않아 앞서 읽은 정상 행의 변경도 모두 되돌린다.
            return Ok(PlanImportResult {
                rows: result.rows,
                created: 0,
                updated: 0,
                skipped: result.skipped,
                missing_required: Vec::new(),
                found: result.found,
                errors: vec![format!("{r}행 지시수량 「{written}」은 정수가 아닙니다.")],
            });
        }

        if seen.insert(number.clone()) {
            result.created += 1;
        } else {
            result.updated += 1;
        }

        let text = |key: &str| -> Option<String> {
            let v = &values[key];
            (!v.is_empty()).then(|| v.clone())
        };
        transaction.execute(
            UPSERT,
            rusqlite::params![
                number,
                text("stock_number"),
                text("item_name"),
                text("currency"),
                text("unit"),
                quantity,
                text("requesting_unit"),
                text("requesting_officer"),
                text("requesting_phone"),
                text("contract_department"),
                text("officer"),
                text("contact"),
                source_name,
                now,
            ],
        )?;
    }

    transaction.commit()?;
    Ok(result)
}

const UPSERT: &str = "INSERT INTO plan (
    request_number, stock_number, item_name, currency, unit, quantity,
    requesting_unit, requesting_officer, requesting_phone,
    contract_department, officer, contact, source_name, imported_at
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
ON CONFLICT(request_number) DO UPDATE SET
    stock_number        = excluded.stock_number,
    item_name           = excluded.item_name,
    currency            = excluded.currency,
    unit                = excluded.unit,
    quantity            = excluded.quantity,
    requesting_unit     = excluded.requesting_unit,
    requesting_officer  = excluded.requesting_officer,
    requesting_phone    = excluded.requesting_phone,
    contract_department = excluded.contract_department,
    officer             = excluded.officer,
    contact             = excluded.contact,
    source_name         = excluded.source_name,
    imported_at         = excluded.imported_at;";

fn open(path: &Path) -> Result<Option<Worksheet>> {
    reader::first_worksheet(path).map_err(|e| match e {
        ReadError::NotFound(m) | ReadError::Invalid(m) => Error::InvalidOperation(m),
    })
}

fn header_of(target: &str) -> &'static str {
    COLUMNS
        .iter()
        .find(|(t, _)| *t == target)
        .map_or("", |(_, h)| h)
}

/// 자리 이름 → 열 번호. 머리글 이름이 **정확히 같은** 열만 잡고, 같은 이름이 둘이면 **앞의 것**을 쓴다.
fn resolve(headers: &[String]) -> HashMap<&'static str, u32> {
    let mut by_name: HashMap<&str, u32> = HashMap::new();
    for (i, header) in headers.iter().enumerate() {
        if !header.is_empty() {
            by_name.entry(header.as_str()).or_insert(i as u32 + 1);
        }
    }
    COLUMNS
        .iter()
        .filter_map(|(target, header)| by_name.get(header).map(|&c| (*target, c)))
        .collect()
}

fn headers_of(sheet: Option<&Worksheet>, row: Option<u32>) -> Result<Vec<String>> {
    let (Some(sheet), Some(row)) = (sheet, row) else {
        return Ok(Vec::new());
    };
    let last = sheet.last_column_used(row).unwrap_or(0);
    (1..=last).map(|c| raw(sheet.cell(row, c))).collect()
}

/// C# `Raw` — 칸을 글로. 엑셀이 보여 주는 모양이 아니라 담긴 값을 집는다.
fn raw(cell: Option<&CellData>) -> Result<String> {
    let Some(cell) = cell else {
        return Ok(String::new());
    };
    Ok(match &cell.value {
        CellValue::Blank => String::new(),
        // 지수 표기로 도망가지 않게 자릿수를 넉넉히 편다. 소수부는 있는 만큼만 남는다.
        CellValue::Number(n) => dotnet::number_text(*n),
        // 시각이 붙어 있으면 버리지 않는다 — 사람이 적어 둔 것이라 지울 근거가 없다.
        CellValue::DateSerial(serial) => dotnet::serial_date(*serial)
            .map_err(Error::Argument)?
            .plan_text(),
        CellValue::Date(d) => d.plan_text(),
        CellValue::Text(t) => t.trim().to_owned(),
        CellValue::Boolean(b) => if *b { "TRUE" } else { "FALSE" }.to_owned(),
        CellValue::Error(e) => (*e).to_owned(),
        CellValue::TimeSpan(days) => dotnet::time_span_text(*days)
            .map_err(Error::Argument)?
            .trim()
            .to_owned(),
    })
}

/// `Path.GetFileName`.
fn file_name(path: &Path) -> String {
    let text = path.to_string_lossy();
    text.rsplit(['\\', '/']).next().unwrap_or("").to_string()
}

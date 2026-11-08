//! 가공열 — 식이 채우는 열(C# `Storage/ComputedColumn.cs`).
//!
//! 값을 저장하지 않고 순수 SQLite 식으로 계산하므로 앱 전용 함수나 캐시가 없어 외부 읽기 연결에서도 같은 값이 나오며
//! 재수집 즉시 반영된다. 식은 흐름식 v2([`Flow`])이고 SQL 로 옮기는 것은 [`flow_compiler`] 다. 품목은 부모의 정확한
//! 차수에 묶인다.

use crate::error::{Error, Result};
use crate::storage::flow::{Flow, FlowFormula};
use crate::storage::flow_check::FlowEnv;
use crate::storage::{flow_compiler, sql_column, views};

/// `ComputedColumn.Parse` — 저장된·화면이 보낸 가공식을 v2 로 읽는다(v1 은 옮긴다).
pub fn parse(json: &str) -> Result<FlowFormula> {
    Flow::parse(json)
}

/// `ComputedColumn.ParseFor` — 저장·미리보기가 받는 꼴. 종류를 먼저 보고, v1 의 「고르기」 단계가 이 종류의 품목
/// 필드를 겹치지 않게 하나 이상 고르는지 본 뒤 [`parse`] 한다. 검사는 본문 읽기와 같은 키를 본다 — 대소문자를 가리지
/// 않고, 같은 이름이 여럿이면 뒤의 것이다. 같은 키가 두 번 든 식은 맨 앞에서 형식 오류다.
pub fn parse_for(entity: &str, json: &str) -> Result<FlowFormula> {
    let known = fields(entity)?;
    let bad_shape = || Error::invalid("가공식 형식이 올바르지 않습니다.");
    let root = crate::json::parse(json).map_err(|_| bad_shape())?;
    if crate::json::has_duplicate_keys(json) {
        return Err(bad_shape());
    }
    if let serde_json::Value::Object(root) = &root
        && let Some(serde_json::Value::Number(version)) = property(root, "version")
        // C# `version.GetInt32() == 1` — 정수가 아닌 수는 1 이 아니다.
        && version.to_string().parse::<i32>().ok() == Some(1)
        && let Some(serde_json::Value::Array(steps)) = property(root, "steps")
    {
        for step in steps {
            let serde_json::Value::Object(step) = step else {
                continue;
            };
            if property(step, "op").and_then(serde_json::Value::as_str) != Some("pick") {
                continue;
            }
            let picked: Vec<&str> = match property(step, "fields") {
                Some(serde_json::Value::Array(f)) => {
                    f.iter().map(|x| x.as_str().unwrap_or("")).collect()
                }
                _ => Vec::new(),
            };
            let distinct: std::collections::HashSet<&&str> = picked.iter().collect();
            if picked.is_empty()
                || distinct.len() != picked.len()
                || picked.iter().any(|x| !known.iter().any(|(k, _)| k == x))
            {
                return Err(Error::invalid("선택한 필드가 없거나 중복되었습니다."));
            }
        }
    }
    parse(json)
}

/// 본문 읽기가 고르는 속성 — 대소문자를 가리지 않고, 여럿이면 뒤의 것(C# `ComputedColumn.Property`).
fn property<'a>(
    map: &'a serde_json::Map<String, serde_json::Value>,
    name: &str,
) -> Option<&'a serde_json::Value> {
    map.iter()
        .rev()
        .find(|(key, _)| crate::text::eq_ignore_case(key, name))
        .map(|(_, value)| value)
}

/// `ComputedColumn.Expression(entity, json)` — 뷰의 한 칸이 될 식. 바깥 행은 `현재`. 꼴 검사를 지나지 못하면 단계
/// 번호를 담아 거절한다.
pub fn expression_of(entity: &str, json: &str) -> Result<String> {
    expression(entity, &parse(json)?, None)
}

/// `ComputedColumn.Expression(entity, formula, env)` — 흐름식은 컴파일하고, SQL 열은 모양을 본 뒤 그 글을 감싼다
/// (§11.5). SQL 의 읽기 권한은 여기서 보지 않는다 — 저장과 뷰 짓기가 [`sql_column::prepare`] 로 따로 본다.
pub fn expression(entity: &str, formula: &FlowFormula, env: Option<&FlowEnv>) -> Result<String> {
    if formula.source.as_deref() != Some("sql") {
        return flow_compiler::expression(entity, formula, flow_compiler::Target::Internal, env);
    }
    sql_column::check_shape(formula.sql.as_deref())?;
    Ok(sql_column::expression(formula.sql.as_deref().unwrap_or("")))
}

/// `ComputedColumn.FinalShape(entity, json)` — 끝 꼴(`value`·`number`·`date`). SQL 열은 글이다.
pub fn final_shape(entity: &str, json: &str) -> Result<&'static str> {
    flow_compiler::final_shape(entity, &parse(json)?, None)
}

/// `ComputedColumn.Fields` — 품목 필드(화면 이름 → 표의 열). 차례가 곧 품목 원천의 열 차례다.
pub fn fields(entity: &str) -> Result<&'static [(&'static str, &'static str)]> {
    Ok(match entity {
        "notice" => &[
            ("세부품명", "item_name"),
            ("세부품명번호", "detail_item_number"),
            ("물품식별번호", "item_id_number"),
            ("규격", "specification"),
            ("수요기관", "demand_agency"),
            ("단위", "unit"),
            ("수량", "quantity"),
            ("추정단가", "unit_price"),
            ("납품장소", "delivery_place"),
            ("인도조건", "delivery_terms"),
        ],
        "contract" => &[
            ("품명", "item_name"),
            ("물품분류번호", "classification_number"),
            ("물품식별번호", "item_id_number"),
            ("규격", "specification"),
            ("수요기관", "demand_agency"),
            ("단위", "unit"),
            ("수량", "quantity"),
            ("단가", "unit_price"),
            ("금액", "amount"),
            ("인도조건", "delivery_terms"),
        ],
        "request" => &[
            ("세부품명", "item_name"),
            ("세부품명번호", "detail_item_number"),
            ("물품식별번호", "item_id_number"),
            ("규격", "specification"),
            ("단위", "unit"),
            ("수량", "quantity"),
            ("조달요구번호", "request_number"),
            ("단가", "unit_price"),
            ("금액", "amount"),
            ("납품장소", "delivery_place"),
            ("인도조건", "delivery_terms"),
        ],
        _ => {
            return Err(Error::invalid("가공열의 자료 종류가 올바르지 않습니다."));
        }
    })
}

/// `ComputedColumn.Fields(entity).Keys`.
pub fn field_names(entity: &str) -> Result<Vec<String>> {
    Ok(fields(entity)?.iter().map(|(k, _)| k.to_string()).collect())
}

/// `ComputedColumn.Identifier` — SQL 식별자.
fn identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

/// `ComputedColumn.SourceSql` — 품목 원천: 이 문서의 정확한 차수에 속한 품목 행. `__order` 는 순번이다. 금액은 계약면
/// 품목 뷰와 같은 자릿점이다.
pub(crate) fn source_sql(
    entity: &str,
    base_expression: &str,
    revision_expression: &str,
) -> Result<String> {
    let columns: Vec<String> = fields(entity)?
        .iter()
        .map(|(name, column)| {
            let value = if matches!(*column, "unit_price" | "amount") {
                views::money(&format!("i.{column}"))
            } else {
                format!("coalesce(CAST(i.{column} AS TEXT), '')")
            };
            format!("{value} AS {}", identifier(name))
        })
        .collect();
    Ok(format!(
        "SELECT i.line_no AS __order, {} FROM {entity}_item i WHERE i.{entity}_base = {base_expression} AND i.seq = {revision_expression}",
        columns.join(", ")
    ))
}

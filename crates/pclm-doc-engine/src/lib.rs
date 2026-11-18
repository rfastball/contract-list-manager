//! 문서 만들기 엔진(ADR-045) — 「서식 바이트 + 값 → 문서 바이트」. 나르미 컴팩트 0.2.0 의 엔진을 옮겨 왔다.
//!
//! 파일 시스템·시계에 닿지 않는다. 확장의 「문서 만들기」 사이드 패널은 `wasm32-unknown-unknown` 으로 지은 이 크레이트를
//! `abi` 의 C ABI 로 부르고(`scripts/build-engine.ps1` 이 `extension/engine.wasm` 으로 짓는다), 네이티브 시험은 같은
//! 함수를 곧바로 부른다. 값은 exe 에 실린 통합 뷰(「문서 서식」)의 한 줄이다 — 호스트 `document {op:"read"}` 의
//! 답을 그대로 건으로 받고, 서식의 칸 이름은 그 뷰의 열 이름이다([`values`]). 수집은 계약목록의 것이라 여기에 없다.
//!
//! - [`list_fields`]: 서식의 누름틀 이름
//! - [`fill`]: 이름이 맞는 누름틀은 값으로, 값이 없는 누름틀은 [`MISSING`] 으로 채우고, 다시 읽어 확인한다

use hwpx_core::native_admission::{
    apply_field_fill, read_field_value, try_apply_simple_field_fill_batch,
};
use hwpx_core::package::{HwpxPackage, PackageLimits};
use hwpx_core::structural_boundary::{BoundaryEvent, BoundaryPairRef, scan_structural_boundaries};
use serde_json::{Map, Value, json};

#[cfg(target_arch = "wasm32")]
mod abi;
mod josa;
pub mod naming;
pub mod template;
pub mod text;
pub mod values;
pub mod vocab;

/// 계약목록에 없는 값을 문서에 남기는 표시.
pub const MISSING: &str = "【확인 필요】";

pub(crate) fn open(bytes: &[u8]) -> Result<HwpxPackage, String> {
    HwpxPackage::from_bytes(bytes, PackageLimits::default())
        .map_err(|e| format!("서식을 읽지 못했습니다: {e:?}"))
}

/// 채울 칸: 이름이 있는 누름틀(`CLICK_HERE`)만. 하이퍼링크 같은 다른 필드와 이름 없는 누름틀은 건드리지 않는다.
fn input_field(event: &BoundaryEvent) -> Option<(String, BoundaryPairRef)> {
    match event {
        BoundaryEvent::FieldBegin {
            raw_name: Some(name),
            field_type,
            pair,
            ..
        } if field_type.as_deref() == Some("CLICK_HERE") && !name.trim().is_empty() => {
            Some((name.clone(), pair.clone()))
        }
        _ => None,
    }
}

pub(crate) fn field_names(package: &HwpxPackage) -> Vec<String> {
    scan_structural_boundaries(package)
        .entries
        .iter()
        .flat_map(|entry| entry.events.iter())
        .filter_map(input_field)
        .map(|(name, _)| name)
        .collect()
}

pub fn list_fields(template: &[u8]) -> Result<Vec<String>, String> {
    Ok(field_names(&open(template)?))
}

fn given<'a>(values: &'a Map<String, Value>, name: &str) -> Option<&'a str> {
    values
        .get(name)
        .and_then(Value::as_str)
        .filter(|v| !v.is_empty())
}

fn report_line(
    values: &Map<String, Value>,
    name: &str,
    filled: bool,
    blockers: Vec<String>,
) -> Value {
    let value = given(values, name);
    json!({ "field": name, "value": value.unwrap_or(MISSING), "missing": value.is_none(), "filled": filled, "blockers": blockers })
}

/// 빠른 길: 구역(본문 구역·머리말·꼬리말)마다 한 번에 채운다. 어느 구역이든 거절하면 `None` 을 돌려주고,
/// 그때는 [`fill_one_by_one`] 이 처음부터 다시 간다(이미 같은 값이 든 칸은 그대로 지나간다).
fn fill_batch(
    package: &mut HwpxPackage,
    values: &Map<String, Value>,
) -> Result<Option<Vec<Value>>, String> {
    let entries: Vec<String> = scan_structural_boundaries(package)
        .entries
        .iter()
        .map(|e| e.entry.clone())
        .collect();
    let mut report = Vec::new();
    for entry in &entries {
        // 채우면 그 구역의 해시가 바뀌어 앞의 scan 이 낡는다 — 구역마다 다시 훑는다.
        let scan = scan_structural_boundaries(package);
        let Some(found) = scan.entries.iter().find(|e| &e.entry == entry) else {
            continue;
        };
        let requests: Vec<_> = found
            .events
            .iter()
            .filter_map(input_field)
            .map(|(name, pair)| {
                (
                    pair,
                    given(values, &name).unwrap_or(MISSING).to_string(),
                    name,
                )
            })
            .collect();
        let batch: Vec<_> = requests
            .iter()
            .map(|(pair, value, _)| (pair.clone(), value.clone()))
            .collect();
        match try_apply_simple_field_fill_batch(package, &scan, entry, &batch) {
            Ok(Some(())) => report.extend(
                requests
                    .iter()
                    .map(|(_, _, name)| report_line(values, name, true, vec![])),
            ),
            Ok(None) => return Ok(None),
            Err(e) => return Err(format!("{entry}: {e:?}")),
        }
    }
    Ok(Some(report))
}

/// 느린 길: 누름틀마다 다시 훑어 하나씩 채운다(41칸에 2초 가까이 걸린다 — 빠른 길이 거절할 때만).
fn fill_one_by_one(
    package: &mut HwpxPackage,
    values: &Map<String, Value>,
) -> Result<Vec<Value>, String> {
    let total = field_names(package).len();
    let mut report = Vec::with_capacity(total);
    for i in 0..total {
        let scan = scan_structural_boundaries(package);
        let (name, pair) = scan
            .entries
            .iter()
            .flat_map(|entry| entry.events.iter())
            .filter_map(input_field)
            .nth(i)
            .ok_or("채우는 사이 누름틀 개수가 바뀌었습니다")?;
        let value = given(values, &name).unwrap_or(MISSING);
        let mutation =
            apply_field_fill(package, &scan, &pair, value).map_err(|e| format!("{name}: {e:?}"))?;
        let blockers = mutation.blockers.iter().map(|b| format!("{b:?}")).collect();
        report.push(report_line(values, &name, mutation.filled, blockers));
    }
    Ok(report)
}

/// 다시 읽어 실제로 들어갔는지 본다(조용히 틀리지 않기).
pub(crate) fn read_back(bytes: &[u8]) -> Result<Vec<Value>, String> {
    let package = open(bytes)?;
    let scan = scan_structural_boundaries(&package);
    let mut readback = Vec::new();
    for event in scan.entries.iter().flat_map(|entry| entry.events.iter()) {
        if let Some((name, pair)) = input_field(event) {
            let text = read_field_value(&package, &scan, &pair).map_err(|e| format!("{e:?}"))?;
            readback.push(json!({ "field": name, "text": text.text }));
        }
    }
    Ok(readback)
}

/// 돌려주는 것: (문서 바이트, 보고). 보고의 `fields` 는 누름틀마다 넣은 값·【확인 필요】 여부·막힌 까닭,
/// `readback` 은 만든 문서를 다시 읽은 글, `mismatch` 는 둘이 다른 칸 수다(0 이 아니면 저장하지 않는다).
pub fn fill(template: &[u8], values: &Map<String, Value>) -> Result<(Vec<u8>, Value), String> {
    let mut package = open(template)?;
    let (fields, path) = match fill_batch(&mut package, values)? {
        Some(report) => (report, "batch"),
        None => (fill_one_by_one(&mut package, values)?, "one-by-one"),
    };
    let bytes = package
        .to_bytes()
        .map_err(|e| format!("문서를 쓰지 못했습니다: {e:?}"))?;
    let readback = read_back(&bytes)?;
    let mismatch = fields
        .iter()
        .zip(&readback)
        .filter(|(f, r)| f["value"] != r["text"])
        .count()
        + fields.len().abs_diff(readback.len());
    Ok((
        bytes,
        json!({ "fields": fields, "readback": readback, "mismatch": mismatch, "path": path }),
    ))
}

//! 로컬 앱 전용 고급 도구(C# `Erp/ErpTools.cs`). Native Messaging 은 이 관리 API 를 노출하지 않는다.

use serde_json::{Map, Value};

use super::capture::{self, CaptureInput, Destination, ErpCapture};
use super::element;
use super::explicit_links;
use super::mapping::MappingStore;
use crate::env::Env;
use crate::error::{Error, Result};
use crate::json;
use crate::storage::database::{Database, begin};
use crate::storage::store::json_of;

fn json_error() -> Error {
    Error::invalid("고급 도구의 요청을 JSON 으로 읽지 못했습니다.")
}

fn required_text(p: &Value, name: &str) -> Result<String> {
    Ok(element::string(element::get(p, name)?)?
        .unwrap_or_default()
        .to_string())
}

/// `ErpTools.Invoke` — 답은 다리가 받는 꼴(camelCase)이다. `home` 을 넘기면 「검토한 자료 저장」 이 쓰기 잠금 안에서 쪽지가
/// 아직 이 파일을 가리키는지 다시 본다(ADR-032) — 「지금 보는 화면」 의 가져오기와 같은 확인이다.
///
/// 요청 글이 JSON 이 아니면 C# 은 `JsonException` 의 영어 글을 냈다 — 여기서는 한국어 한 줄로 낸다(같게 내지 않는다).
pub fn invoke(
    db: &Database,
    env: &dyn Env,
    operation: &str,
    json_text: &str,
    home: Option<&dyn Destination>,
) -> Result<Value> {
    let p = json::parse(json_text).map_err(|_| json_error())?;
    let mappings = MappingStore::new(db, env);
    let capture = || {
        let c = ErpCapture::new(db.path(), env);
        match home {
            Some(home) => c.with_destination(home),
            None => c,
        }
    };
    match operation {
        "list" => mappings.list(),
        "saveMapping" => Ok(Value::String(mappings.save(&required_text(&p, "json")?)?)),
        "activateMapping" => {
            mappings.activate(&required_text(&p, "revision")?)?;
            mappings.list()
        }
        "sample" => Ok(mappings
            .validate_sample(
                &required_text(&p, "json")?,
                &required_text(&p, "profile")?,
                element::get(&p, "data")?,
            )?
            .to_value()),
        "inspect" => {
            let input = CaptureInput::from_json(&p)?.ok_or_else(null_input)?;
            Ok(capture().inspect(&input)?.to_value())
        }
        "capture" => {
            let input =
                CaptureInput::from_json(element::get(&p, "snapshot")?)?.ok_or_else(null_input)?;
            let choices = capture::choices_from_json(element::get(&p, "choices")?)?;
            Ok(capture()
                .save(
                    &input,
                    Some(&required_text(&p, "baseToken")?),
                    Some(&required_text(&p, "captureId")?),
                    &choices,
                )?
                .to_value())
        }
        "references" => {
            let c = db.open()?;
            let tx = begin(&c)?;
            let result = explicit_links::resolve(&tx)?;
            tx.commit()?;
            Ok(Value::Array(
                result
                    .iter()
                    .map(|r| {
                        let mut o = Map::new();
                        o.insert("entityType".into(), Value::String(r.entity_type.clone()));
                        o.insert("base".into(), Value::String(r.base.clone()));
                        o.insert("target".into(), Value::String(r.target.clone()));
                        o.insert("state".into(), Value::String(r.state.clone()));
                        Value::Object(o)
                    })
                    .collect(),
            ))
        }
        "history" => {
            let c = db.open_read_only()?;
            let mut statement = c.prepare(
                "SELECT capture_id,profile,entity_base,entity_seq,captured_at,snapshot_json FROM erp_capture ORDER BY captured_at DESC LIMIT 100",
            )?;
            let names: Vec<String> = statement.column_names().map(str::to_string).collect();
            let rows = statement
                .query_map([], |r| {
                    let mut o = Map::new();
                    for (i, name) in names.iter().enumerate() {
                        o.insert(name.clone(), json_of(r.get_ref(i)?));
                    }
                    Ok(Value::Object(o))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(Value::Array(rows))
        }
        _ => Err(Error::invalid("지원하지 않는 고급 도구입니다.")),
    }
}

/// C# 은 `Deserialize<CaptureInput>(…)!` 가 null 이면 다음 줄에서 `NullReferenceException` 으로 멎었다.
fn null_input() -> Error {
    Error::invalid("Object reference not set to an instance of an object.")
}

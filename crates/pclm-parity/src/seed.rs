//! 지금 판의 비교용 자료(E2, #10) — C# `Seed.cs` 와 같은 길. 빈 작업자료에 `tests/flow-cases.json` 의 시험 문서를
//! `FlowCaseTests` 와 같은 꼴로 넣고(연결은 `ReferenceFixture` 처럼 명시 참조로만), 짚은 폴더가 있으면 그 안의 ERP JSON 을
//! 창의 「JSON 파일로 가져오기」 와 같은 저장 길(`mirror::prepare` → `mirror::default_choices` → `ErpCapture::save`)로
//! 들인다. 기본 선택을 받아들인다 — 지우는 것·사람 값을 걷는 것은 고르지 않는다.
//!
//! 시험 문서는 C# 의 `Store.UpsertNotice`·`UpsertContract`·`UpsertRequest`(시험 픽스처 전용, ADR-028)가 짓는 줄을 SQL 로
//! 바로 짓는다 — Rust Core 에는 그 픽스처를 옮기지 않았다. 묶는 꼴은 Microsoft.Data.Sqlite 의 것이다(날짜 `yyyy-MM-dd
//! 00:00:00`, 금액 `0.0###`, 수량 정수). 덤프 대조가 두 길이 같은 줄을 짓는지 잰다.
//!
//! 실제 ERP JSON 은 저장소 밖에서만 읽는다. 여기 옮겨 적지 않는다.

use std::path::{Path, PathBuf};

use pclm_core::Env;
use pclm_core::erp::capture::{CaptureInput, ErpCapture};
use pclm_core::erp::{explicit_links, mapping, mirror, screens};
use pclm_core::storage::pclm_file;
use rusqlite::{Connection, params};
use serde_json::{Map, Value};

use crate::system_env::SystemEnv;

/// 수집 ID — 파일 차례(1부터)로 짓는다. 두 계기가 같은 ID 를 써야 수집 기록이 견주어진다.
pub fn capture_id(index: usize) -> String {
    format!("00000000-0000-0000-0000-{index:012}")
}

pub fn run(path: &str, erp_directory: Option<&str>) -> Result<(), String> {
    let env = SystemEnv;
    let path = Path::new(path);
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let database = pclm_file::create(path, pclm_core::PclmRole::WORK, &env).map_err(error)?;
    let fixture_text = std::fs::read_to_string(flow_cases()?).map_err(|e| e.to_string())?;
    let fixture: Value = serde_json::from_str(&fixture_text).map_err(|e| e.to_string())?;
    let docs = fixture["docs"]
        .as_object()
        .ok_or("flow-cases.json 에 docs 가 없습니다.")?;
    let c = database.open().map_err(error)?;
    documents(&c, docs, &env)?;
    drop(c);

    if let Some(directory) = erp_directory {
        let mapping = mapping::MappingStore::new(&database, &env)
            .active()
            .map_err(error)?;
        let revision = mapping::revision(&mapping);
        let mut files: Vec<PathBuf> = std::fs::read_dir(directory)
            .map_err(|e| e.to_string())?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.is_file()
                    && p.extension()
                        .and_then(|e| e.to_str())
                        .is_some_and(|e| e.eq_ignore_ascii_case("json"))
            })
            .collect();
        // Directory.GetFiles(…).Order(StringComparer.Ordinal) — UTF-16 차례.
        files.sort_by(|a, b| {
            let a: Vec<u16> = a.to_string_lossy().encode_utf16().collect();
            let b: Vec<u16> = b.to_string_lossy().encode_utf16().collect();
            a.cmp(&b)
        });
        let capture = ErpCapture::new(database.path(), &env);
        let (mut stored, mut skipped) = (0, 0);
        for (index, file) in files.iter().enumerate() {
            let name = file
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            match import(&capture, &mapping, &revision, file, index + 1) {
                Ok(()) => stored += 1,
                Err(Skip(message)) => {
                    skipped += 1;
                    eprintln!("건너뜀 {name}: {message}");
                }
            }
        }
        println!(
            "erp_files {} stored {stored} skipped {skipped}",
            files.len()
        );
    }

    let c = database.open_read_only().map_err(error)?;
    for table in [
        "request",
        "request_item",
        "notice",
        "notice_item",
        "contract",
        "contract_item",
        "request_link",
        "project_link",
        "request_item_link",
        "user_view",
        "erp_capture",
    ] {
        let n: i64 = c
            .query_row(&format!("SELECT count(*) FROM {table};"), [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        println!("{table} {n}");
    }
    Ok(())
}

fn error(e: pclm_core::Error) -> String {
    pclm_core::storage::store::error_message(&e)
}

/// 건너뛸 까닭. C# 은 `InvalidOperationException`·`JsonException`·`FormatException` 만 건너뛰고 그 밖은 멎는다 —
/// Rust Core 는 앞의 셋을 모두 `Error::InvalidOperation` 으로 낸다. 그 밖의 오류는 계기를 멈춘다.
struct Skip(String);

fn import(
    capture: &ErpCapture<'_>,
    mapping: &mapping::MappingSet,
    revision: &str,
    file: &Path,
    index: usize,
) -> Result<(), Skip> {
    let bytes = std::fs::read(file).map_err(|e| fatal(&e.to_string()))?;
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
    let text =
        std::str::from_utf8(bytes).map_err(|_| Skip("JSON 으로 읽지 못했습니다(UTF-8).".into()))?;
    let data = pclm_core::erp::parse_json(text)
        .ok_or_else(|| Skip("JSON 으로 읽지 못했습니다.".into()))?;
    let code = data.get("pointInfo").and_then(screens::leaf);
    let Some(profile) = screens::profile_for(mapping, code.as_deref()) else {
        return Err(Skip(format!(
            "받는 화면이 아닙니다(메뉴 {}).",
            code.as_deref().unwrap_or("없음")
        )));
    };
    let saved = (|| {
        let (input, preview) =
            mirror::prepare(capture, &CaptureInput::new(profile.id(), revision, data))?;
        let choices = mirror::default_choices(&preview, &[]);
        capture.save(
            &input,
            Some(&preview.base_token),
            Some(&capture_id(index)),
            &choices,
        )
    })();
    match saved {
        Ok(_) => Ok(()),
        Err(pclm_core::Error::InvalidOperation(message)) => Err(Skip(message)),
        Err(e) => Err(fatal(&error(e))),
    }
}

fn fatal(message: &str) -> Skip {
    eprintln!("pclm-parity: {message}");
    std::process::exit(1);
}

// ── 시험 문서(FlowCaseTests.Seed) ─────────────────────────────

/// `T(o, key)` — 빈 글은 NULL.
fn t(o: &Value, key: &str) -> Option<String> {
    o.get(key)
        .and_then(Value::as_str)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
}

/// `D(o, key)` — `yyyy/MM/dd` 를 `DateTime` 으로 읽어 묶은 꼴.
fn d(o: &Value, key: &str) -> Option<String> {
    t(o, key).map(|v| format!("{} 00:00:00", v.replace('/', "-")))
}

/// `M(o, key)` — 쉼표를 걷고 `decimal` 로 읽어 묶은 꼴(`0.0###…`).
fn m(o: &Value, key: &str) -> Option<String> {
    t(o, key).map(|v| {
        let v = v.replace(',', "");
        match v.split_once('.') {
            None => format!("{v}.0"),
            Some((whole, fraction)) => {
                let fraction = fraction.trim_end_matches('0');
                let fraction = if fraction.is_empty() { "0" } else { fraction };
                format!("{whole}.{fraction}")
            }
        }
    })
}

/// `Q(o, key)` — `int`.
fn q(o: &Value, key: &str) -> Result<Option<i64>, String> {
    t(o, key)
        .map(|v| {
            v.parse()
                .map_err(|_| format!("수량이 수가 아닙니다: {key}"))
        })
        .transpose()
}

fn s(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

fn sql(e: rusqlite::Error) -> String {
    e.to_string()
}

fn resolve(c: &Connection) -> Result<(), String> {
    let tx = c.unchecked_transaction().map_err(sql)?;
    explicit_links::resolve(&tx).map_err(error)?;
    tx.commit().map_err(sql)
}

fn documents(c: &Connection, docs: &Map<String, Value>, env: &dyn Env) -> Result<(), String> {
    for doc in docs.values() {
        let f = &doc["fields"];
        let empty = Vec::new();
        let items = doc["items"].as_array().unwrap_or(&empty);
        let (b, seq) = (s(&doc["base"]), s(&doc["seq"]));
        let now = env.now_utc();
        let tx = c.unchecked_transaction().map_err(sql)?;
        match s(&doc["entity"]) {
            "notice" => {
                tx.execute(
                    "INSERT OR IGNORE INTO notice_group (group_base) VALUES (?1);",
                    [b],
                )
                .map_err(sql)?;
                tx.execute(
                    "INSERT OR IGNORE INTO notice_series (notice_base, group_base) VALUES (?1, ?1);",
                    [b],
                )
                .map_err(sql)?;
                tx.execute(
                    "INSERT INTO notice (notice_base, seq, title, posted_at, base_price, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6);",
                    params![b, seq, t(f, "공고명"), d(f, "게시일시"), m(f, "기초금액"), now],
                )
                .map_err(sql)?;
                for (i, x) in items.iter().enumerate() {
                    tx.execute(
                        "INSERT INTO notice_item (notice_base, seq, line_no, demand_agency, item_name, detail_item_number, item_id_number, specification, quantity, unit, unit_price, delivery_place, delivery_terms)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13);",
                        params![
                            b, seq, i as i64 + 1, t(x, "수요기관"), t(x, "세부품명"), t(x, "세부품명번호"),
                            t(x, "물품식별번호"), t(x, "규격"), q(x, "수량")?, t(x, "단위"), m(x, "추정단가"),
                            t(x, "납품장소"), t(x, "인도조건")
                        ],
                    )
                    .map_err(sql)?;
                }
            }
            "contract" => {
                tx.execute(
                    "INSERT OR IGNORE INTO contract_series (contract_base) VALUES (?1);",
                    [b],
                )
                .map_err(sql)?;
                tx.execute(
                    "INSERT INTO contract (contract_base, seq, title, contracted_on, amount, delivery_due, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7);",
                    params![b, seq, t(f, "계약건명"), d(f, "계약일자"), m(f, "계약금액"), d(f, "납품기한"), now],
                )
                .map_err(sql)?;
                for (i, x) in items.iter().enumerate() {
                    tx.execute(
                        "INSERT INTO contract_item (contract_base, seq, line_no, classification_number, item_id_number, item_name, specification, unit, delivery_terms, quantity, unit_price, amount, demand_agency)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13);",
                        params![
                            b, seq, i as i64 + 1, t(x, "물품분류번호"), t(x, "물품식별번호"), t(x, "품명"),
                            t(x, "규격"), t(x, "단위"), t(x, "인도조건"), q(x, "수량")?, m(x, "단가"),
                            m(x, "금액"), t(x, "수요기관")
                        ],
                    )
                    .map_err(sql)?;
                }
            }
            _ => {
                tx.execute(
                    "INSERT OR IGNORE INTO request_series (request_base) VALUES (?1);",
                    [b],
                )
                .map_err(sql)?;
                tx.execute(
                    "INSERT INTO request (request_base, seq, title, received_on, goods_amount, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6);",
                    params![b, seq, t(f, "요청명"), d(f, "접수일자"), m(f, "품대"), now],
                )
                .map_err(sql)?;
                for (i, x) in items.iter().enumerate() {
                    tx.execute(
                        "INSERT INTO request_item (request_base, seq, line_no, request_number, detail_item_number, item_id_number, item_name, specification, unit_price, quantity, amount, unit, delivery_terms, delivery_place)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14);",
                        params![
                            b, seq, i as i64 + 1, t(x, "조달요구번호"), t(x, "세부품명번호"), t(x, "물품식별번호"),
                            t(x, "세부품명"), t(x, "규격"), m(x, "단가"), q(x, "수량")?, m(x, "금액"),
                            t(x, "단위"), t(x, "인도조건"), t(x, "납품장소")
                        ],
                    )
                    .map_err(sql)?;
                }
            }
        }
        explicit_links::resolve(&tx).map_err(error)?;
        tx.commit().map_err(sql)?;
    }

    let reference = |id: &str| -> (String, String) {
        let doc = &docs[id];
        (s(&doc["base"]).to_string(), s(&doc["seq"]).to_string())
    };
    for (id, doc) in docs {
        let notice = doc["links"].get("notice").and_then(Value::as_str);
        match (s(&doc["entity"]), notice) {
            // `ReferenceFixture.Contract`
            ("contract", Some(notice)) => {
                let (contract, _) = reference(id);
                let (notice, _) = reference(notice);
                c.execute(
                    "UPDATE contract SET ref_notice_base=?1 WHERE contract_base=?2",
                    params![notice, contract],
                )
                .map_err(sql)?;
                resolve(c)?;
            }
            // `ReferenceFixture.Request`
            ("request", Some(notice)) => {
                let (request, seq) = reference(id);
                let (notice, _) = reference(notice);
                let tx = c.unchecked_transaction().map_err(sql)?;
                tx.execute(
                    "UPDATE request_item SET ref_request_base=request_base, ref_request_seq=seq,
                         ref_request_item=CAST(line_no AS TEXT) WHERE request_base=?1",
                    [&request],
                )
                .map_err(sql)?;
                tx.execute(
                    "UPDATE notice_item SET ref_request_base=?1, ref_request_seq=?2,
                         ref_request_item=CAST(line_no AS TEXT) WHERE notice_base=?3",
                    params![request, seq, notice],
                )
                .map_err(sql)?;
                tx.execute(
                    "INSERT INTO notice_item(notice_base,seq,line_no,ref_request_base)
                         SELECT notice_base,seq,1,?1 FROM notice n WHERE notice_base=?2
                         AND NOT EXISTS(SELECT 1 FROM notice_item i WHERE i.notice_base=n.notice_base AND i.seq=n.seq)",
                    params![request, notice],
                )
                .map_err(sql)?;
                explicit_links::resolve(&tx).map_err(error)?;
                tx.commit().map_err(sql)?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// `tests/flow-cases.json` — 실행 파일 자리에서 위로 올라가며 찾고, 없으면 지금 폴더에서 찾는다.
fn flow_cases() -> Result<PathBuf, String> {
    let mut starts = Vec::new();
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        starts.push(dir.to_path_buf());
    }
    if let Ok(dir) = std::env::current_dir() {
        starts.push(dir);
    }
    for start in starts {
        let mut directory = Some(start.as_path());
        while let Some(d) = directory {
            let candidate = d.join("tests").join("flow-cases.json");
            if candidate.is_file() {
                return Ok(candidate);
            }
            directory = d.parent();
        }
    }
    Err("tests/flow-cases.json 을 찾지 못했습니다.".into())
}

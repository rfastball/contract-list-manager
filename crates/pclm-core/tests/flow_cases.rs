//! 가공식 v2 — 공유 사례표(`tests/flow-cases.json`, docs/column-management.md §11.7). C# `FlowCaseTests` 의 이식.
//!
//! 화면의 vitest 와 C# xunit 이 같은 파일을 읽는다. 여기서는 Rust 꼴 검사와 SQL 컴파일러를 실제 SQLite 위에서 돌린다 —
//! 셋 중 하나가 갈리면 그쪽이 붉어진다. 값은 모두 지어낸 것이다.
//!
//! 문서는 C# 의 `Store.UpsertNotice`·`UpsertContract`·`UpsertRequest`(시험 픽스처 전용) 대신 행을 직접 넣는다 — 그
//! 편집 API 는 다음 조각이다. 넣는 꼴은 C# 이 Microsoft.Data.Sqlite 로 묶는 꼴 그대로다: `DateTime` 은
//! `yyyy-MM-dd HH:mm:ss`, `decimal` 은 `0.0###…`(소수 한 자리 이상), `int` 는 INTEGER.

mod common;

use common::{Temp, TestEnv};
use pclm_core::Database;
use pclm_core::storage::database::refresh;
use pclm_core::storage::flow::Flow;
use pclm_core::storage::flow_compiler::{self, SELF, Target};
use pclm_core::storage::{pclm_file, schema, store};
use rusqlite::types::Value as Sql;
use rusqlite::{Connection, params};
use serde_json::{Map, Value};

fn fixture() -> Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/flow-cases.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn cases(fixture: &Value) -> &Vec<Value> {
    fixture["cases"].as_array().unwrap()
}

fn docs(fixture: &Value) -> &Map<String, Value> {
    fixture["docs"].as_object().unwrap()
}

fn s(value: &Value) -> &str {
    value.as_str().unwrap()
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|v| s(v).to_string())
        .collect()
}

/// `Key(doc)` — 면 뷰의 줄 열쇠.
fn key(doc: &Value) -> String {
    let (b, q) = (s(&doc["base"]), s(&doc["seq"]));
    if s(&doc["entity"]) == "contract" {
        format!("{b}{q}")
    } else {
        format!("{b}-{q}")
    }
}

/// C# 의 `QuerySingle<string>` — 글은 그대로, 정수는 글로.
fn text(value: Sql) -> Option<String> {
    match value {
        Sql::Null => None,
        Sql::Text(t) => Some(t),
        Sql::Integer(i) => Some(i.to_string()),
        Sql::Real(r) => panic!("실수가 나왔습니다: {r}"),
        Sql::Blob(_) => panic!("BLOB 이 나왔습니다"),
    }
}

/// 시험 DB 하나.
struct Db {
    _temp: Temp,
    database: Database,
}

fn fresh() -> Db {
    let temp = Temp::new("flow-cases");
    let path = temp.path("flow.pclm");
    let database = pclm_file::create(&path, "work", &TestEnv::new()).unwrap();
    Db {
        _temp: temp,
        database,
    }
}

// ── 넣는 꼴(Microsoft.Data.Sqlite 의 묶음) ─────────────────────────

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
fn q(o: &Value, key: &str) -> Option<i64> {
    t(o, key).map(|v| v.parse().unwrap())
}

const NOW: &str = "2026-11-07T03:04:05.0000000Z";

/// `Seed()` — 문서를 넣고 사람이 확정한 연결을 명시 참조로 세운다(`ReferenceFixture`).
fn seed(db: &Db, fixture: &Value) {
    let c = db.database.open().unwrap();
    let docs = docs(fixture);
    for doc in docs.values() {
        let f = &doc["fields"];
        let items = doc["items"].as_array().unwrap();
        let (b, q0) = (s(&doc["base"]), s(&doc["seq"]));
        match s(&doc["entity"]) {
            "notice" => {
                c.execute(
                    "INSERT OR IGNORE INTO notice_group (group_base) VALUES (?1);",
                    [b],
                )
                .unwrap();
                c.execute(
                    "INSERT OR IGNORE INTO notice_series (notice_base, group_base) VALUES (?1, ?1);",
                    [b],
                )
                .unwrap();
                c.execute(
                    "INSERT INTO notice (notice_base, seq, title, posted_at, base_price, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6);",
                    params![b, q0, t(f, "공고명"), d(f, "게시일시"), m(f, "기초금액"), NOW],
                )
                .unwrap();
                for (i, x) in items.iter().enumerate() {
                    c.execute(
                        "INSERT INTO notice_item (notice_base, seq, line_no, demand_agency, item_name, detail_item_number, item_id_number, specification, quantity, unit, unit_price, delivery_place, delivery_terms)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13);",
                        params![
                            b, q0, i as i64 + 1, t(x, "수요기관"), t(x, "세부품명"), t(x, "세부품명번호"),
                            t(x, "물품식별번호"), t(x, "규격"), q(x, "수량"), t(x, "단위"), m(x, "추정단가"),
                            t(x, "납품장소"), t(x, "인도조건")
                        ],
                    )
                    .unwrap();
                }
            }
            "contract" => {
                c.execute(
                    "INSERT OR IGNORE INTO contract_series (contract_base) VALUES (?1);",
                    [b],
                )
                .unwrap();
                c.execute(
                    "INSERT INTO contract (contract_base, seq, title, contracted_on, amount, delivery_due, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7);",
                    params![b, q0, t(f, "계약건명"), d(f, "계약일자"), m(f, "계약금액"), d(f, "납품기한"), NOW],
                )
                .unwrap();
                for (i, x) in items.iter().enumerate() {
                    c.execute(
                        "INSERT INTO contract_item (contract_base, seq, line_no, classification_number, item_id_number, item_name, specification, unit, delivery_terms, quantity, unit_price, amount, demand_agency)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13);",
                        params![
                            b, q0, i as i64 + 1, t(x, "물품분류번호"), t(x, "물품식별번호"), t(x, "품명"),
                            t(x, "규격"), t(x, "단위"), t(x, "인도조건"), q(x, "수량"), m(x, "단가"),
                            m(x, "금액"), t(x, "수요기관")
                        ],
                    )
                    .unwrap();
                }
            }
            _ => {
                c.execute(
                    "INSERT OR IGNORE INTO request_series (request_base) VALUES (?1);",
                    [b],
                )
                .unwrap();
                c.execute(
                    "INSERT INTO request (request_base, seq, title, received_on, goods_amount, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6);",
                    params![b, q0, t(f, "요청명"), d(f, "접수일자"), m(f, "품대"), NOW],
                )
                .unwrap();
                for (i, x) in items.iter().enumerate() {
                    c.execute(
                        "INSERT INTO request_item (request_base, seq, line_no, request_number, detail_item_number, item_id_number, item_name, specification, unit_price, quantity, amount, unit, delivery_terms, delivery_place)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14);",
                        params![
                            b, q0, i as i64 + 1, t(x, "조달요구번호"), t(x, "세부품명번호"), t(x, "물품식별번호"),
                            t(x, "세부품명"), t(x, "규격"), m(x, "단가"), q(x, "수량"), m(x, "금액"),
                            t(x, "단위"), t(x, "인도조건"), t(x, "납품장소")
                        ],
                    )
                    .unwrap();
                }
            }
        }
        pclm_core::erp::explicit_links::resolve(&c).unwrap();
    }

    let reference = |id: &str| -> (String, String) {
        let doc = &docs[id];
        (s(&doc["base"]).to_string(), s(&doc["seq"]).to_string())
    };
    for (id, doc) in docs {
        let links = &doc["links"];
        let notice = links.get("notice").and_then(Value::as_str);
        match (s(&doc["entity"]), notice) {
            // `ReferenceFixture.Contract`
            ("contract", Some(notice)) => {
                let (contract, _) = reference(id);
                let (notice, _) = reference(notice);
                c.execute(
                    "UPDATE contract SET ref_notice_base=?1 WHERE contract_base=?2",
                    params![notice, contract],
                )
                .unwrap();
                pclm_core::erp::explicit_links::resolve(&c).unwrap();
            }
            // `ReferenceFixture.Request`
            ("request", Some(notice)) => {
                let (request, seq) = reference(id);
                let (notice, _) = reference(notice);
                c.execute_batch(&format!(
                    "UPDATE request_item SET ref_request_base=request_base, ref_request_seq=seq,
                         ref_request_item=CAST(line_no AS TEXT) WHERE request_base='{request}';
                     UPDATE notice_item SET ref_request_base='{request}', ref_request_seq='{seq}',
                         ref_request_item=CAST(line_no AS TEXT) WHERE notice_base='{notice}';
                     INSERT INTO notice_item(notice_base,seq,line_no,ref_request_base)
                         SELECT notice_base,seq,1,'{request}' FROM notice n WHERE notice_base='{notice}'
                         AND NOT EXISTS(SELECT 1 FROM notice_item i WHERE i.notice_base=n.notice_base AND i.seq=n.seq);"
                ))
                .unwrap();
                pclm_core::erp::explicit_links::resolve(&c).unwrap();
            }
            _ => {}
        }
    }
}

// ── 재료 ─────────────────────────────────────────────

/// `사례표의_재료는_C샵이_쓰는_재료와_같다`
#[test]
fn fixture_env_matches_the_core_env() {
    let fixture = fixture();
    for entity in ["contract", "notice", "request"] {
        let env = Flow::env_for(entity).unwrap();
        let expected = &fixture["env"][entity];
        assert_eq!(strings(&expected["base"]), env.base, "{entity}.base");
        assert_eq!(strings(&expected["items"]), env.items, "{entity}.items");
        let linked = expected["linked"].as_object().unwrap();
        let mut want: Vec<&String> = linked.keys().collect();
        want.sort();
        let mut got: Vec<&String> = env.linked.iter().map(|(k, _)| k).collect();
        got.sort();
        assert_eq!(want, got, "{entity}.linked");
        for (kind, link) in linked {
            let have = env.link(kind).unwrap();
            assert_eq!(
                strings(&link["fields"]),
                have.fields,
                "{entity}.{kind}.fields"
            );
            assert_eq!(strings(&link["items"]), have.items, "{entity}.{kind}.items");
            assert_eq!(
                link["many"].as_bool().unwrap(),
                have.many,
                "{entity}.{kind}.many"
            );
        }
    }
}

/// `시험_문서는_면_뷰에_적힌_대로_찍힌다`
#[test]
fn seeded_documents_print_as_written_in_the_face_views() {
    let fixture = fixture();
    let db = fresh();
    seed(&db, &fixture);
    let c = db.database.open_read_only().unwrap();
    for (id, doc) in docs(&fixture) {
        let entity = s(&doc["entity"]);
        for (field, value) in doc["fields"].as_object().unwrap() {
            let got: Sql = c
                .query_row(
                    &format!(
                        "SELECT \"{field}\" FROM \"{}\" WHERE \"{}\" = ?1",
                        store::face_view(entity),
                        store::face_key(entity)
                    ),
                    [key(doc)],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(Some(s(value).to_string()), text(got), "{id}.{field}");
        }
    }
}

// ── 사례 ─────────────────────────────────────────────

/// 스키마 22 의 SQL 이 v1 을 옮긴 결과(`MigrateWithSchemaStep`). 세 벌(TS·C#·SQL)이 같은 v2 를 내야 한다.
fn migrate_with_schema_step(v1: &str) -> String {
    let c = Connection::open_in_memory().unwrap();
    c.execute_batch("CREATE TABLE user_column (entity_type TEXT, field_name TEXT, kind TEXT, options TEXT, sort_order INTEGER);")
        .unwrap();
    c.execute(
        "INSERT INTO user_column VALUES ('notice', 'x', 'computed', ?1, 10);",
        [v1],
    )
    .unwrap();
    // v21 → v22 단계. STEPS[0] 이 v19 → v20 이다.
    c.execute_batch(schema::STEPS[(21 - schema::BASELINE_VERSION) as usize])
        .unwrap();
    c.query_row("SELECT options FROM user_column", [], |r| r.get(0))
        .unwrap()
}

/// `사례(name)` 의 `migrated` 중 「세 벌이 같은 v2 를 낸다」 의 SQL 벌(`MigrateWithSchemaStep`).
///
/// SQLite 3.54 는 스칼라 부질의를 지난 값의 JSON 하위형을 떨어뜨려, C# 의 v22 SQL 그대로면 `steps` 가 배열이 아니라 배열을
/// 적은 **글**로 담긴다(3.53 은 넘긴다). Rust 의 v22.sql 은 그 부질의를 `json(…)` 으로 감싸 같은 값을 낸다 — 이 시험이 그것을
/// 쥐고, 바이트로 같은지는 계기의 `migrate-v21` 이 C# 결과와 견준다.
#[test]
fn schema_22_migrates_v1_cases_like_flow() {
    let fixture = fixture();
    let mut seen = 0;
    for case in cases(&fixture) {
        let Some(migrated) = case.get("migrated") else {
            continue;
        };
        let name = s(&case["name"]);
        let by_sql: Value =
            serde_json::from_str(&migrate_with_schema_step(&case["formula"].to_string())).unwrap();
        assert_eq!(migrated, &by_sql, "{name}: 스키마 22");
        seen += 1;
    }
    assert!(seen > 0);
}

/// `사례(name)` — 사례마다 꼴·오류 자리·끝 값(안쪽 SQL 과 꺼낸 SQL 둘 다)을 본다.
#[test]
fn every_case() {
    let fixture = fixture();
    let db = fresh();
    seed(&db, &fixture);
    let c = db.database.open_read_only().unwrap();
    let docs = docs(&fixture);
    let mut seen = 0;
    for case in cases(&fixture) {
        let name = s(&case["name"]);
        let entity = s(&case["entity"]);
        let source = case["formula"].to_string();
        let formula = Flow::parse(&source).unwrap_or_else(|e| panic!("{name}: {e}"));

        if let Some(migrated) = case.get("migrated") {
            let serialized = Flow::serialize(&formula).unwrap();
            let ours: Value = serde_json::from_str(&serialized).unwrap();
            assert_eq!(migrated, &ours, "{name}: {serialized}");
            // 스키마 22 의 SQL 로 옮긴 결과는 아래 따로 본다 — 링크된 SQLite 판에서 갈린다.
        }

        let env = Flow::env_for(entity).unwrap();
        let checked = Flow::check(formula.flow.as_ref().unwrap(), &env);
        if let Some(shapes) = case.get("shapes") {
            let want: Vec<Option<&str>> = shapes
                .as_array()
                .unwrap()
                .iter()
                .map(Value::as_str)
                .collect();
            assert_eq!(want, checked.shapes, "{name}: 꼴");
        }
        if let Some(error) = case.get("error") {
            assert!(!checked.ok(), "{name}: 오류가 없습니다");
            let index = checked.errors[0].index;
            assert_eq!(
                error["index"].as_u64().unwrap() as usize,
                index,
                "{name}: 오류 자리"
            );
            let refused = flow_compiler::expression(entity, &formula, Target::Internal, None)
                .expect_err(name);
            let pclm_core::Error::InvalidOperation(message) = refused else {
                panic!("{name}: {refused:?}");
            };
            let prefix = if index == 0 {
                "시작 데이터: ".to_string()
            } else {
                format!("{index}단계: ")
            };
            assert!(message.starts_with(&prefix), "{name}: {message}");
            seen += 1;
            continue;
        }
        assert!(
            checked.ok(),
            "{name}: {}",
            checked
                .errors
                .iter()
                .map(|e| e.describe())
                .collect::<Vec<_>>()
                .join(" / ")
        );

        let expression =
            flow_compiler::expression(entity, &formula, Target::Internal, None).unwrap();
        let eject = flow_compiler::eject(entity, &formula, None).unwrap();
        let from = format!(
            "FROM \"{}\" AS {SELF} WHERE {SELF}.\"{}\" = ?1",
            store::face_view(entity),
            store::face_key(entity)
        );
        for (id, value) in case["final"].as_object().unwrap() {
            let doc = &docs[id];
            assert_eq!(entity, s(&doc["entity"]), "{name}: {id}");
            let inside: Sql = c
                .query_row(&format!("SELECT {expression} {from}"), [key(doc)], |r| {
                    r.get(0)
                })
                .unwrap_or_else(|e| panic!("{name}: {e}\n{expression}"));
            let inside = text(inside);
            assert_eq!(Some(s(value).to_string()), inside, "{name} {id}: 안쪽 SQL");
            // 「SQL로 꺼내기」 — 계약면 뷰만 읽는 두 번째 출력기. 최신 차수에서 같아야 한다.
            let outside: Sql = c
                .query_row(&format!("SELECT ({eject}) {from}"), [key(doc)], |r| {
                    r.get(0)
                })
                .unwrap_or_else(|e| panic!("{name}: {e}\n{eject}"));
            assert_eq!(inside, text(outside), "{name} {id}: 꺼낸 SQL");
        }
        seen += 1;
    }
    assert_eq!(cases(&fixture).len(), seen);
}

/// `사례를_가공열로_저장하면_뷰가_같은_값을_낸다` — 모든 사례를 실제 열로 저장하면 뷰가 같은 값을 낸다(감싼 뷰의 바깥 행
/// `현재` 를 거친다). C# `Store.SaveComputedColumn` 대신 정의를 직접 넣고 뷰를 다시 짓는다 — 저장의 재검사는 다음 조각이다.
#[test]
fn cases_saved_as_computed_columns_give_the_same_values_in_views() {
    let fixture = fixture();
    let db = fresh();
    seed(&db, &fixture);
    let docs = docs(&fixture);
    let mut saved = Vec::new();
    {
        let c = db.database.open().unwrap();
        let mut i = 0;
        for case in cases(&fixture).iter().filter(|c| c.get("final").is_some()) {
            i += 1;
            let entity = s(&case["entity"]);
            let column = format!("사례{i:02}");
            let mut formula = Flow::parse(&case["formula"].to_string()).unwrap();
            formula.id = Some(format!("{i:032x}"));
            c.execute(
                "INSERT INTO user_column(entity_type,field_name,kind,options,sort_order)
                 VALUES(?1,?2,'computed',?3,
                   coalesce((SELECT max(sort_order) FROM user_column WHERE entity_type=?1),0)+10)",
                params![entity, column, Flow::serialize(&formula).unwrap()],
            )
            .unwrap();
            saved.push((
                column,
                entity.to_string(),
                case["final"].as_object().unwrap(),
            ));
        }
        let refreshed = refresh(&c).unwrap();
        assert!(
            refreshed.columns.is_empty(),
            "깨진 열: {:?}",
            refreshed.columns
        );
    }

    let c = db.database.open_read_only().unwrap();
    for (column, entity, last) in &saved {
        for (id, value) in *last {
            let doc = &docs[id];
            let views: &[(&str, &str)] = match entity.as_str() {
                "notice" => &[("v_공고", "입찰공고번호"), ("v_공고차수", "입찰공고번호")],
                "contract" => &[
                    ("v_계약", "계약번호"),
                    ("v_계약차수", "계약번호"),
                    ("v_통합", "계약번호"),
                    ("v_통합차수", "계약번호"),
                ],
                _ => &[("v_접수", "접수번호")],
            };
            for (view, key_column) in views {
                let got: Sql = c
                    .query_row(
                        &format!("SELECT \"{column}\" FROM {view} WHERE {key_column} = ?1"),
                        [key(doc)],
                        |r| r.get(0),
                    )
                    .unwrap();
                assert_eq!(
                    Some(s(value).to_string()),
                    text(got),
                    "{view}.{column} {id}"
                );
            }
        }
    }

    // 결과타입은 끝 꼴에서 온다.
    let types: Vec<(String, String)> = c
        .prepare("SELECT 열이름, 결과타입 FROM v_가공열")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(saved.len(), types.len());
    for kind in ["number", "date", "text"] {
        assert!(types.iter().any(|(_, t)| t == kind), "{kind}");
    }
}

/// C# `계약_3000건_가공열_10개의_통합_조회_시간` — 계약 3,000건 × 가공열 10개(품목 나열·연결 필드 섞어)에서
/// `SELECT * FROM v_통합` 시간(§11.7). 숫자는 표준 오류로 낸다. 10초를 넘으면 붉어진다 — 그 아래는 기계마다 다른 숫자라
/// 막지 않는다.
#[test]
fn unified_query_time_with_3000_contracts_and_10_computed_columns() {
    use serde_json::json;
    const CONTRACTS: usize = 3000;
    let db = fresh();
    {
        let mut c = db.database.open().unwrap();
        let tx = c.transaction().unwrap();
        // 공고 하나에 계약 셋이 매달린다(분할 낙찰처럼). 건은 제 본번호다.
        for g in 0..CONTRACTS / 3 {
            let notice = format!("PERFN{g:05}");
            tx.execute(
                "INSERT INTO notice_group (group_base) VALUES (?1);",
                [&notice],
            )
            .unwrap();
            tx.execute(
                "INSERT INTO notice_series (notice_base, group_base) VALUES (?1, ?1);",
                [&notice],
            )
            .unwrap();
            tx.execute(
                "INSERT INTO notice (notice_base, seq, title, posted_at, base_price, updated_at) VALUES (?1, '000', ?2, '2026-07-01', '12345000', '2026-10-08');",
                params![notice, format!("성능 공고 {g}")],
            )
            .unwrap();
            for line in 1..=3 {
                tx.execute(
                    "INSERT INTO notice_item (notice_base, seq, line_no, item_name, detail_item_number, quantity) VALUES (?1, '000', ?2, ?3, ?4, 2)",
                    params![notice, line, format!("품목{line}"), format!("{line:03}")],
                )
                .unwrap();
            }
        }
        for k in 0..CONTRACTS {
            let contract = format!("PERFK{k:05}");
            tx.execute(
                "INSERT INTO contract_series (contract_base) VALUES (?1);",
                [&contract],
            )
            .unwrap();
            tx.execute(
                "INSERT INTO contract (contract_base, seq, title, contracted_on, amount, delivery_due, updated_at) VALUES (?1, '00', ?2, '2026-08-01', '1500000', '2026-09-20', '2026-10-08');",
                params![contract, format!("성능 계약 {k}")],
            )
            .unwrap();
            tx.execute(
                "INSERT INTO project_link (contract_base, notice_group) VALUES (?1, ?2);",
                params![contract, format!("PERFN{:05}", k / 3)],
            )
            .unwrap();
            for line in 1..=4 {
                tx.execute(
                    "INSERT INTO contract_item (contract_base, seq, line_no, item_name, classification_number, quantity, unit_price, amount) VALUES (?1, '00', ?2, ?3, ?4, 3, '1000', '3000')",
                    params![contract, line, format!("품목{}", line % 3), format!("{line:03}")],
                )
                .unwrap();
            }
        }
        tx.commit().unwrap();
    }

    let items = |steps: Value| {
        json!({ "version": 2, "id": "", "source": "flow", "flow": { "from": { "kind": "items" }, "steps": steps } })
            .to_string()
    };
    let linked = |from: Value, steps: Value| {
        json!({ "version": 2, "id": "", "source": "flow", "flow": { "from": from, "steps": steps } })
            .to_string()
    };
    let formulas = [
        items(
            json!([{ "op": "distinct", "fields": ["품명", "물품분류번호"] }, { "op": "format", "text": "{품명}({물품분류번호})" }, { "op": "join", "text": ", " }]),
        ),
        items(
            json!([{ "op": "get", "field": "품명" }, { "op": "distinct" }, { "op": "abbrev", "n": 1, "text": "{앞} 외 {나머지}종" }]),
        ),
        items(json!([{ "op": "sum", "field": "금액" }, { "op": "thousands" }])),
        items(json!([{ "op": "count" }])),
        items(
            json!([{ "op": "sort", "field": "단가", "dir": "desc" }, { "op": "first" }, { "op": "get", "field": "품명" }]),
        ),
        linked(
            json!({ "kind": "linked", "link": "notice", "field": "기초금액" }),
            json!([]),
        ),
        linked(
            json!({ "kind": "linked", "link": "notice" }),
            json!([{ "op": "format", "text": "{공고명}({입찰공고번호})" }]),
        ),
        linked(
            json!({ "kind": "linked", "link": "notice", "items": true }),
            json!([{ "op": "get", "field": "세부품명" }, { "op": "join", "text": "/" }]),
        ),
        linked(
            json!({ "kind": "linked", "link": "notice", "field": "게시일시" }),
            json!([{ "op": "toDate" }, { "op": "dateDiff", "field": "계약일자" }]),
        ),
        linked(
            json!({ "kind": "field", "field": "계약금액" }),
            json!([{ "op": "toNumber" }, { "op": "div", "value": 3 }, { "op": "round", "n": 2 }]),
        ),
    ];

    let time = || {
        let reader = db.database.open_read_only().unwrap();
        let started = std::time::Instant::now();
        let mut statement = reader.prepare("SELECT * FROM v_통합").unwrap();
        let width = statement.column_count();
        let rows: Vec<Vec<Option<String>>> = statement
            .query_map([], |r| {
                (0..width).map(|i| r.get::<_, Sql>(i).map(text)).collect()
            })
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        let names: Vec<String> = statement.column_names().map(String::from).collect();
        (started.elapsed(), rows, names)
    };
    let (before, _, _) = time();
    let env = TestEnv::new();
    let s = store::Store::new(db.database.clone(), &env);
    for (i, formula) in formulas.iter().enumerate() {
        s.save_computed_column("contract", &format!("성능{i}"), formula, None)
            .unwrap();
    }

    let (elapsed, rows, names) = time();
    eprintln!(
        "PERF v_통합 {}줄 × 가공열 {}개: {} ms (가공열 없이 {} ms)",
        rows.len(),
        formulas.len(),
        elapsed.as_millis(),
        before.as_millis()
    );
    assert!(rows.len() >= CONTRACTS);
    let at = |name: &str| names.iter().position(|n| n == name).unwrap();
    let sample = rows
        .iter()
        .find(|r| r[at("계약번호")].as_deref() == Some("PERFK0000000"))
        .unwrap();
    assert_eq!(sample[at("성능5")].as_deref(), Some("12,345,000"));
    assert_eq!(sample[at("성능8")].as_deref(), Some("-31"));
    assert!(
        elapsed < std::time::Duration::from_secs(10),
        "{} ms",
        elapsed.as_millis()
    );
}

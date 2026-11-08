//! C# `ConfirmedFieldTests` 의 Store·뷰·판올림 몫 — 확정되고도 담지 않던 필드(ADR-043, 스키마 28)가 면 뷰의 새 열에
//! 닿는가. 수집 흐름(`ErpCapture`·`MappingStore`·`Mapping`)을 거치는 시험은 그 부분이 옮겨진 뒤의 일이다. 값은 모두
//! 지어낸 것이다.

mod common;

use common::store_fixture::Fixture;
use common::{Temp, TestEnv};
use pclm_core::storage::schema;
use pclm_core::storage::store::EntityRef;
use pclm_core::{Access, Database};
use rusqlite::{Connection, named_params};
use serde_json::json;

/// `새_머리_열은_덮개로_고쳐진다` — 새 열도 사람이 고칠 수 있다. 덮개가 그 칸을 감싸지 않으면 그 칸만 조용히 고칠 수
/// 없게 된다(ADR-020).
///
/// C# 은 접수 화면을 수집해(`ErpCapture`) 자료를 세운다 — 여기서는 같은 머리 값을 `UpsertRequest` 픽스처로 넣는다.
/// 보는 것은 수집이 아니라 덮개가 `추정가격` 을 감싸는가다.
#[test]
fn new_head_column_is_corrected_by_override() {
    let f = Fixture::new("confirmed-override");
    f.request(
        "RC022",
        "000",
        &[("title", "시험 구매"), ("estimated_price", "1000")],
        &[&[("line_no", "1")]],
    );
    f.store()
        .set_override(
            &EntityRef::new("request", "RC022", "000"),
            "추정가격",
            "2,000",
        )
        .unwrap();
    assert_eq!(
        f.scalar("SELECT 추정가격 FROM v_접수 WHERE 접수번호 = 'RC022-000'")
            .as_deref(),
        Some("2,000")
    );
}

/// `Doc(profile, target, value)` — 수집 원천 문서 한 장.
fn doc(profile: &str, target: &str, value: &str) -> String {
    json!({
        "profile": profile,
        "rows": [{
            "table": "notice",
            "sourceKey": "",
            "values": {target: value},
            "origins": {target: profile},
        }],
    })
    .to_string()
}

fn finance_officers(c: &Connection) -> Vec<String> {
    let mut s = c
        .prepare("SELECT 재무관 FROM v_공고차수 ORDER BY 공고본번호")
        .unwrap();
    s.query_map([], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

/// `재무관_판올림은_실제_출처를_검증하고_새_수집값을_보존한다` (Theory 28·29) — 배포된 v29 를 거쳐도 실제 매핑·수집
/// 원값으로 증명한 재무관만 남기고 새 대상으로 수집한 값은 보존한다.
#[test]
fn finance_officer_migration_checks_real_sources_and_keeps_new_values() {
    for version in [28, 29] {
        let temp = Temp::new("confirmed-finance");
        let path = temp.path(&format!("v{version}.pclm"));
        {
            let c = common::raw(&path);
            c.execute_batch(schema::BASELINE).unwrap();
            for step in &schema::STEPS[..(28 - schema::BASELINE_VERSION) as usize] {
                c.execute_batch(step).unwrap();
            }
            const PROFILE: &str = "g2b-notice-a-v1";
            for (b, value, source) in [
                ("TESTN0001", "홍길동", "fncrNm"),
                ("TESTN0002", "시험 기관장", "grnteCrdrNm"),
                ("TESTN0003", "옛 값", "unknown"),
                ("TESTN0004", "수집과 다른 값", "fncrNm"),
            ] {
                c.execute("INSERT INTO notice_group VALUES(?1)", [b])
                    .unwrap();
                c.execute("INSERT INTO notice_series VALUES(?1,?1)", [b])
                    .unwrap();
                c.execute(
                    "INSERT INTO notice(notice_base,seq,creditor,updated_at) VALUES(?1,'000',?2,'2026-10-01')",
                    [b, value],
                )
                .unwrap();
                c.execute(
                    "INSERT INTO erp_source VALUES('notice',?1,'000',?2,'later-unrelated','2026-10-02')",
                    [b, &doc(PROFILE, "creditor", value)],
                )
                .unwrap();
                if source == "unknown" {
                    continue;
                }
                let mapping = json!({"profiles": [{"id": PROFILE, "fields": [
                    {"source": source, "target": "creditor", "transform": "text"}]}]});
                c.execute(
                    "INSERT INTO erp_mapping(revision,json,created_at) VALUES(?1,?2,'2026-10-01')",
                    [b, &mapping.to_string()],
                )
                .unwrap();
                let raw = if b == "TESTN0004" {
                    "다른 원값"
                } else {
                    value
                };
                let snapshot = json!({"profile": PROFILE, "mappingRevision": b,
                    "data": {"pointInfo": {source: raw}}});
                c.execute(
                    "INSERT INTO erp_capture VALUES(:b,'h','h',:profile,:b,'000',:snapshot,'{}','2026-10-01')",
                    named_params! {":b": b, ":profile": PROFILE, ":snapshot": snapshot.to_string()},
                )
                .unwrap();
            }
            if version == 29 {
                c.execute_batch(schema::STEPS[(29 - schema::BASELINE_VERSION - 1) as usize])
                    .unwrap();
                let (b, value) = ("TESTN0005", "새 재무관");
                c.execute("INSERT INTO notice_group VALUES(?1)", [b])
                    .unwrap();
                c.execute("INSERT INTO notice_series VALUES(?1,?1)", [b])
                    .unwrap();
                c.execute(
                    "INSERT INTO notice(notice_base,seq,finance_officer,updated_at) VALUES(?1,'000',?2,'2026-10-03')",
                    [b, value],
                )
                .unwrap();
                c.execute(
                    "INSERT INTO erp_source VALUES('notice',?1,'000',?2,'new-default','2026-10-03')",
                    [b, &doc("g2b-notice-a-v1", "finance_officer", value)],
                )
                .unwrap();
            }
            c.execute_batch(&format!("PRAGMA user_version = {version};"))
                .unwrap();
        }
        let env = TestEnv::new();
        let database = Database::new(&path, Access::Write);
        database.migrate(&env).unwrap();
        let read = database.open_read_only().unwrap();
        let expected: Vec<&str> = if version == 29 {
            vec!["홍길동", "", "", "", "새 재무관"]
        } else {
            vec!["홍길동", "", "", ""]
        };
        assert_eq!(finance_officers(&read), expected, "v{version}");
        database.migrate(&env).unwrap();
        assert_eq!(finance_officers(&read), expected, "v{version} 다시");
    }
}

fn erp_raw(c: &Connection) -> Vec<String> {
    let mut s = c
        .prepare(
            "SELECT 'erp_source|'||entity_base||'|'||document_json FROM erp_source
             UNION ALL SELECT 'erp_capture|'||capture_id||'|'||snapshot_json FROM erp_capture
             UNION ALL SELECT 'erp_mapping|'||revision||'|'||json FROM erp_mapping ORDER BY 1",
        )
        .unwrap();
    s.query_map([], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

/// `ERP_원자료에_겹친_키가_들면_판올림이_멈추고_원자료는_그대로다` (Theory) — 재무관(v30)·공고종류(v32) 판올림이 읽는 수집
/// 기록·영수증·매핑에 같은 키가 두 번 들면 C# 은 그 객체를 열며 `ArgumentException` 으로 판올림을 멈춘다. Rust 도 같은
/// 글로 멈추고 원자료는 그대로다 — 뒤 값으로 접어 보수하고, v32 에서는 수집 기록을 접어 다시 쓰고 있었다.
#[test]
fn duplicate_keys_in_erp_sources_stop_the_migration() {
    const HEAD: &str = r#"{"table":"notice","sourceKey":"","values":{"creditor":"홍길동","notice_kind":"실공고(일반공고)"},"origins":{"creditor":"g2b-notice-a-v1","notice_kind":"g2b-notice-a-v1"}}"#;
    const MAP: &str = r#"{"profiles":[{"id":"g2b-notice-a-v1","fields":[{"source":"fncrNm","target":"creditor","transform":"text"},{"source":"kind","target":"notice_kind","transform":"text"}]}]}"#;
    const RECEIPT: &str = r#"{"profile":"g2b-notice-a-v1","mappingRevision":"m-1","data":{"pointInfo":{"fncrNm":"홍길동","kind":"실공고"}}}"#;
    for (version, place, key) in [
        (28, "document", "creditor"),
        (28, "receipt", "mappingRevision"),
        (28, "mapping", "profiles"),
        (31, "document", "table"),
        (31, "receipt", "mappingRevision"),
        (31, "mapping", "profiles"),
    ] {
        let head = match (place, version) {
            ("document", 28) => HEAD.replace(
                r#""creditor":"홍길동","#,
                r#""creditor":"홍길동","creditor":"다른 값","#,
            ),
            ("document", _) => HEAD.replace(
                r#""table":"notice","#,
                r#""table":"notice","table":"notice","#,
            ),
            _ => HEAD.to_string(),
        };
        let document = format!(r#"{{"profile":"g2b-notice-a-v1","rows":[{head}]}}"#);
        let receipt = if place == "receipt" {
            RECEIPT.replace(
                r#""mappingRevision":"#,
                r#""mappingRevision":"m-1","mappingRevision":"#,
            )
        } else {
            RECEIPT.to_string()
        };
        let mapping = if place == "mapping" {
            MAP.replace(r#"{"profiles":"#, r#"{"profiles":[],"profiles":"#)
        } else {
            MAP.to_string()
        };

        let temp = Temp::new("confirmed-duplicate");
        let path = temp.path(&format!("v{version}-{place}.pclm"));
        {
            let c = common::raw(&path);
            c.execute_batch(schema::BASELINE).unwrap();
            for step in &schema::STEPS[..(version - schema::BASELINE_VERSION) as usize] {
                c.execute_batch(step).unwrap();
            }
            let officer = if version == 28 {
                "creditor"
            } else {
                "finance_officer"
            };
            c.execute_batch(
                "INSERT INTO notice_group VALUES('TESTN0001'); INSERT INTO notice_series VALUES('TESTN0001','TESTN0001');",
            )
            .unwrap();
            c.execute(
                &format!(
                    "INSERT INTO notice(notice_base,seq,{officer},notice_kind,updated_at) VALUES('TESTN0001','000','홍길동','실공고(일반공고)','2026-10-01')"
                ),
                [],
            )
            .unwrap();
            c.execute(
                "INSERT INTO erp_source VALUES('notice','TESTN0001','000',?1,'m-1','2026-10-02')",
                [&document],
            )
            .unwrap();
            c.execute(
                "INSERT INTO erp_mapping(revision,json,created_at) VALUES('m-1',?1,'2026-10-01')",
                [&mapping],
            )
            .unwrap();
            c.execute(
                "INSERT INTO erp_capture(capture_id,request_hash,content_hash,profile,entity_base,entity_seq,snapshot_json,result_json,captured_at)
                 VALUES('TESTC0001','h','h','g2b-notice-a-v1','TESTN0001','000',?1,'{}','2026-10-01')",
                [&receipt],
            )
            .unwrap();
            c.execute_batch(&format!("PRAGMA user_version = {version};"))
                .unwrap();
        }
        let before = erp_raw(&common::raw(&path));

        let error = Database::new(&path, Access::Write)
            .migrate(&TestEnv::new())
            .unwrap_err();
        assert!(
            matches!(&error, pclm_core::Error::Argument(m)
                if *m == format!("An item with the same key has already been added. Key: {key} (Parameter 'propertyName')")),
            "v{version} {place}: {error:?}"
        );
        let after = common::raw(&path);
        assert_eq!(erp_raw(&after), before, "v{version} {place}");
        let stayed: i32 = after
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(stayed, version, "v{version} {place}");
    }
}

//! C# `NoticeItemViewTests`(공고 품목이 `v_공고` 에 어떻게 비치는가)와 `FlowColumnTests`(가공열 v2 가 계약면에 서는
//! 자리) — `기본_열_목록이_면_뷰와_같다` 는 `views.rs` 가 이미 옮겼다.
//!
//! 자료는 `Store.UpsertNotice`·`UpsertContract` 를 옮긴 픽스처로 세운다. 값은 모두 지어낸 것이다.

mod common;

use std::fmt::Debug;

use common::TestEnv;
use common::store_fixture::Fixture;
use pclm_core::Error;
use pclm_core::erp::explicit_links;
use pclm_core::storage::flow::Flow;
use pclm_core::storage::flow_compiler::{self, Target};
use pclm_core::storage::store::{EntityRef, face_view};
use pclm_core::storage::{computed_column, schema, sql_column, views};
use pclm_core::{Access, Database};
use rusqlite::Connection;
use rusqlite::types::Value as Sql;
use serde_json::{Value, json};

// ── 거들기 ───────────────────────────────────────────

/// `Assert.Throws<InvalidOperationException>` — 글을 돌려준다.
fn invalid<T: Debug>(result: pclm_core::Result<T>) -> String {
    match result {
        Err(Error::InvalidOperation(message)) => message,
        other => panic!("InvalidOperation 이어야 한다: {other:?}"),
    }
}

/// C# `Convert.ToString` 꼴 — NULL 은 `None`.
fn text(value: Sql) -> Option<String> {
    match value {
        Sql::Null => None,
        Sql::Integer(i) => Some(i.to_string()),
        Sql::Real(f) => Some(f.to_string()),
        Sql::Text(t) => Some(t),
        Sql::Blob(_) => Some("<blob>".into()),
    }
}

/// `QuerySingle<string>` — 줄이 꼭 하나여야 한다.
fn single(c: &Connection, sql: &str) -> Option<String> {
    let mut statement = c.prepare(sql).unwrap();
    let rows: Vec<Option<String>> = statement
        .query_map([], |r| r.get::<_, Sql>(0))
        .unwrap()
        .map(|v| text(v.unwrap()))
        .collect();
    assert_eq!(rows.len(), 1, "{sql}");
    rows.into_iter().next().unwrap()
}

// ══ NoticeItemViewTests ════════════════════════════════
// 줄 하나가 공고 하나라는 약속을 지키면서 품목을 실으려면 하나로 정할 수 있을 때만 실어야 한다 — 첫 줄을 공고 전체의
// 값인 양 내면 틀린 값이 그대로 문서에 찍힌다.

/// C# `NoticeItemRecord` 의 시험 꼴(`Item(...)` 의 기본값 그대로). `None` 은 그 열을 넣지 않는다(NULL).
#[derive(Clone)]
struct Item {
    line_no: i32,
    name: &'static str,
    number: &'static str,
    quantity: Option<i32>,
    agency: Option<&'static str>,
    place: &'static str,
    days: Option<i32>,
    due: Option<&'static str>,
    terms: &'static str,
    unit: &'static str,
}

fn item(line_no: i32, name: &'static str, number: &'static str, quantity: Option<i32>) -> Item {
    Item {
        line_no,
        name,
        number,
        quantity,
        agency: Some("한별군수지원단"),
        place: "어딘가",
        days: Some(300),
        due: None,
        terms: "현장설치도",
        unit: "대",
    }
}

impl Item {
    /// `Store.UpsertNotice` 의 `notice_item` 열.
    fn columns(&self) -> Vec<(&'static str, String)> {
        let mut c = vec![
            ("line_no", self.line_no.to_string()),
            ("item_name", self.name.to_string()),
            ("detail_item_number", self.number.to_string()),
            ("unit", self.unit.to_string()),
            ("delivery_place", self.place.to_string()),
            ("delivery_terms", self.terms.to_string()),
        ];
        if let Some(a) = self.agency {
            c.push(("demand_agency", a.to_string()));
        }
        if let Some(q) = self.quantity {
            c.push(("quantity", q.to_string()));
        }
        if let Some(d) = self.days {
            c.push(("delivery_days", d.to_string()));
        }
        // Microsoft.Data.Sqlite 는 DateTime 을 `yyyy-MM-dd HH:mm:ss` 글로 묶는다.
        if let Some(d) = self.due {
            c.push(("delivery_due", d.to_string()));
        }
        c
    }
}

/// C# `NoticeContactRecord` 의 시험 꼴.
struct Contact {
    line_no: i32,
    agency: &'static str,
    officer: &'static str,
    phone: Option<&'static str>,
}

fn put(items: &[Item]) -> Fixture {
    put_with(items, &[])
}

fn put_with(items: &[Item], contacts: &[Contact]) -> Fixture {
    let fixture = Fixture::new("notice-items");
    let owned: Vec<Vec<(&str, String)>> = items.iter().map(Item::columns).collect();
    let pairs: Vec<Vec<(&str, &str)>> = owned
        .iter()
        .map(|i| i.iter().map(|(k, v)| (*k, v.as_str())).collect())
        .collect();
    let refs: Vec<&[(&str, &str)]> = pairs.iter().map(Vec::as_slice).collect();
    fixture.notice(
        "R26BK98001199",
        "000",
        &[("title", "지어낸 공고")],
        &refs,
        &[],
    );
    let c = fixture.conn();
    for contact in contacts {
        c.execute(
            "INSERT INTO notice_officer_contact (notice_base, seq, line_no, demand_agency, officer, phone)
             VALUES ('R26BK98001199', '000', ?1, ?2, ?3, ?4);",
            rusqlite::params![contact.line_no, contact.agency, contact.officer, contact.phone],
        )
        .unwrap();
    }
    fixture
}

type Row = Vec<(String, String)>;

fn rows(fixture: &Fixture, view: &str) -> Vec<Row> {
    let c = fixture.database().open_read_only().unwrap();
    let mut statement = c
        .prepare(&format!("SELECT * FROM \"{view}\" ORDER BY 1;"))
        .unwrap();
    let names: Vec<String> = statement
        .column_names()
        .map(|n: &str| n.to_string())
        .collect();
    statement
        .query_map([], |r| {
            Ok(names
                .iter()
                .enumerate()
                .map(|(i, n): (usize, &String)| {
                    (n.clone(), text(r.get(i).unwrap()).unwrap_or_default())
                })
                .collect())
        })
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

fn get<'a>(row: &'a Row, column: &str) -> &'a str {
    row.iter()
        .find(|(n, _)| n == column)
        .map(|(_, v)| v.as_str())
        .unwrap_or_else(|| panic!("열이 없다: {column}"))
}

fn notice_row(fixture: &Fixture) -> Row {
    rows(fixture, "v_공고").remove(0)
}

/// `세부품명이_하나면_수량을_합쳐_싣는다` — 같은 물건을 여러 수요기관에 나눠 넣느라 줄만 늘어난 공고. 수량은 합이다.
#[test]
fn single_detail_item_sums_quantity() {
    let f = put(&[
        Item {
            place: "한별기동지원단",
            ..item(1, "승강판", "2510190201", Some(1))
        },
        Item {
            place: "누림항공지원단",
            ..item(2, "승강판", "2510190201", Some(2))
        },
    ]);
    let row = notice_row(&f);
    assert_eq!(get(&row, "세부품명"), "승강판");
    assert_eq!(get(&row, "세부품명번호"), "2510190201");
    assert_eq!(get(&row, "수량"), "3");
    assert_eq!(get(&row, "단위"), "대");
}

/// `세부품명이_여럿이어도_줄마다_같은_칸은_싣는다` — 칸마다 본다. 갈리는 세부품명은 비우고, 단위가 같으니 수량은 합이다.
#[test]
fn several_detail_items_still_carry_uniform_cells() {
    let f = put(&[
        Item {
            due: Some("2026-09-26 00:00:00"),
            ..item(1, "승강판", "2510190201", Some(1))
        },
        Item {
            due: Some("2026-09-26 00:00:00"),
            ..item(2, "여과망", "1210190101", Some(5))
        },
    ]);
    let row = notice_row(&f);
    assert_eq!(get(&row, "수요기관"), "한별군수지원단");
    assert_eq!(get(&row, "단위"), "대");
    assert_eq!(get(&row, "납품기한"), "2026/09/26");
    assert_eq!(get(&row, "수량"), "6");
    assert_eq!(get(&row, "세부품명"), "");
    assert_eq!(get(&row, "세부품명번호"), "");

    // 비운 것은 공고 뷰뿐이다. 품목 뷰에는 두 줄이 그대로 있다.
    assert_eq!(rows(&f, "v_공고품목").len(), 2);
}

/// `단위가_갈리면_수량을_비운다` — 「대」 와 「식」 을 더한 수는 아무것도 말하지 않으면서 그럴듯하게 찍힌다.
#[test]
fn differing_units_blank_quantity() {
    let f = put(&[
        item(1, "승강판", "2510190201", Some(1)),
        Item {
            unit: "식",
            ..item(2, "여과망", "1210190101", Some(5))
        },
    ]);
    let row = notice_row(&f);
    assert_eq!(get(&row, "수량"), "");
    assert_eq!(get(&row, "단위"), "");
}

/// `인도조건은_세부품명이_여럿이어도_같으면_싣는다` (Theory).
#[test]
fn delivery_terms_carry_when_uniform_across_detail_items() {
    for (first, second, expected) in [
        ("현장설치도", "현장설치도", "현장설치도"),
        ("현장설치도", "납품장소 입고도", ""),
    ] {
        let f = put(&[
            Item {
                terms: first,
                ..item(1, "승강판", "2510190201", Some(1))
            },
            Item {
                terms: second,
                ..item(2, "여과망", "1210190101", Some(5))
            },
        ]);
        assert_eq!(
            get(&notice_row(&f), "인도조건"),
            expected,
            "{first} / {second}"
        );
    }
}

/// `세부품명이_하나여도_갈리는_칸은_비운다` — 수요기관이 줄마다 다르면 그 칸만 빈다.
#[test]
fn single_detail_item_blanks_differing_cells() {
    let f = put(&[
        item(1, "승강판", "2510190201", Some(1)),
        Item {
            agency: Some("누림군수지원단"),
            ..item(2, "승강판", "2510190201", Some(2))
        },
    ]);
    let row = notice_row(&f);
    assert_eq!(get(&row, "수요기관"), "");
    assert_eq!(get(&row, "세부품명"), "승강판");
    assert_eq!(get(&row, "수량"), "3");
}

/// `빈_줄이_섞이면_칸을_비운다` (Theory) — 빈 줄도 한 값이다. 옛 줄에는 NULL 대신 빈 문자열이 남아 있을 수 있어 둘 다 본다.
#[test]
fn blank_lines_blank_the_cell() {
    for (first, second, expected) in [
        (
            Some("한별군수지원단"),
            Some("한별군수지원단"),
            "한별군수지원단",
        ),
        (Some("한별군수지원단"), None, ""),
        (Some("한별군수지원단"), Some(""), ""),
        (None, Some(""), ""),
    ] {
        let f = put(&[
            Item {
                agency: first,
                ..item(1, "승강판", "2510190201", Some(1))
            },
            Item {
                agency: second,
                ..item(2, "승강판", "2510190201", Some(2))
            },
        ]);
        let row = notice_row(&f);
        assert_eq!(get(&row, "수요기관"), expected, "{first:?} / {second:?}");
        assert_eq!(get(&row, "세부품명"), "승강판");
    }
}

/// `수량이_빈_줄이_있으면_더하지_않는다` (Theory) — `SUM` 은 NULL 을 건너뛰어 모르는 수량을 0 으로 친 합을 냈다.
#[test]
fn blank_quantity_is_not_summed() {
    for (first, second, expected) in [
        (Some(5), Some(3), "8"),
        (Some(5), None, ""),
        (None, None, ""),
    ] {
        let f = put(&[
            item(1, "승강판", "2510190201", first),
            item(2, "여과망", "1210190101", second),
        ]);
        let row = notice_row(&f);
        assert_eq!(get(&row, "수량"), expected, "{first:?} / {second:?}");
        assert_eq!(get(&row, "단위"), "대");
    }
}

/// `담당자_칸도_빈_사람이_섞이면_비운다` (Theory) — 전화를 적은 사람과 비운 사람을 두고 앞사람의 전화를 낼 수 없다.
#[test]
fn officer_cells_blank_when_a_blank_person_mixes_in() {
    for (first, second, expected) in [
        (
            Some("010-0000-0000"),
            Some("010-0000-0000"),
            "010-0000-0000",
        ),
        (Some("010-0000-0000"), None, ""),
        (Some("010-0000-0000"), Some(""), ""),
    ] {
        let f = put_with(
            &[item(1, "승강판", "2510190201", Some(1))],
            &[
                Contact {
                    line_no: 1,
                    agency: "한별군수지원단",
                    officer: "홍길동",
                    phone: first,
                },
                Contact {
                    line_no: 2,
                    agency: "한별군수지원단",
                    officer: "홍길동",
                    phone: second,
                },
            ],
        );
        let row = notice_row(&f);
        assert_eq!(get(&row, "담당자전화"), expected, "{first:?} / {second:?}");
        assert_eq!(get(&row, "담당자"), "홍길동");
    }
}

/// `납품기한은_날짜가_먼저다` (Theory) — 날짜가 있는 공고는 일수를 `0` 으로 적어 두므로 날짜가 먼저다.
#[test]
fn delivery_due_date_comes_first() {
    for (days, due, expected) in [
        (90, None, "계약 후 90일 이내"),
        (0, Some("2026-09-26 00:00:00"), "2026/09/26"),
        (0, None, ""),
    ] {
        let f = put(&[Item {
            days: Some(days),
            due,
            ..item(1, "여과망", "1210190101", Some(3))
        }]);
        assert_eq!(
            get(&notice_row(&f), "납품기한"),
            expected,
            "{days} / {due:?}"
        );
        assert_eq!(
            get(&rows(&f, "v_공고품목")[0], "납품기한"),
            expected,
            "{days} / {due:?}"
        );
    }
}

// ══ FlowColumnTests ════════════════════════════════════
// 식 하나하나의 값은 `flow_cases.rs` 가 공유 사례표로 본다 — 여기서는 뷰를 감싸는 것, 차수, 정정, 저장 검증, 판올림을 본다.

/// `Flow(from, steps)` — 흐름식 v2 의 JSON.
fn flow(from: Value, steps: Vec<Value>) -> String {
    json!({"version": 2, "id": "", "source": "flow", "flow": {"from": from, "steps": steps}})
        .to_string()
}

fn items() -> Value {
    json!({"kind": "items"})
}

/// `Read(sql)` — 앱 함수를 등록하지 않은 독립 읽기 전용 연결이 소비자 경계다.
fn read(fixture: &Fixture, sql: &str) -> Option<String> {
    single(&common::raw_read(&fixture.path), sql)
}

fn columns(fixture: &Fixture, view: &str) -> Vec<String> {
    let c = fixture.database().open_read_only().unwrap();
    let mut s = c
        .prepare(&format!("SELECT name FROM pragma_table_info('{view}')"))
        .unwrap();
    s.query_map([], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

fn people(fixture: &Fixture, entity: &str) -> Vec<String> {
    fixture
        .store()
        .user_columns(Some(entity))
        .unwrap()
        .into_iter()
        .map(|c| c.field_name)
        .collect()
}

/// `감싼_뷰는_열_차례를_바꾸지_않는다` — 가공열이 있으면 뷰를 한 겹 감싸는데, 열 이름과 차례는 그대로다.
#[test]
fn wrapped_views_keep_column_order() {
    let f = Fixture::new("flowcol-wrap");
    let store = f.store();
    store
        .save_computed_column(
            "contract",
            "품목수",
            &flow(items(), vec![json!({"op": "count"})]),
            None,
        )
        .unwrap();
    store
        .add_user_column("contract", "비고", "text", &[])
        .unwrap();
    store
        .save_computed_column(
            "notice",
            "품목수",
            &flow(items(), vec![json!({"op": "count"})]),
            None,
        )
        .unwrap();
    store
        .save_computed_column(
            "request",
            "품목합계",
            &flow(items(), vec![json!({"op": "sum", "field": "금액"})]),
            None,
        )
        .unwrap();
    store.move_user_column("contract", "품목수", -1).unwrap(); // 진행상태(10)·품목수·메모 … 사이에 끼운다

    for entity in ["notice", "contract", "request"] {
        let mut expected: Vec<String> = views::base_columns(entity)
            .unwrap()
            .iter()
            .map(|c| c.to_string())
            .collect();
        expected.extend(people(&f, entity));
        assert_eq!(columns(&f, face_view(entity)), expected, "{entity}");
    }
    assert_eq!(columns(&f, "v_공고"), columns(&f, "v_공고차수"));
    assert_eq!(columns(&f, "v_계약"), columns(&f, "v_계약차수"));
    let mut unified = columns(&f, "v_통합");
    unified.extend(["공고건".to_string(), "현행공고".to_string()]);
    assert_eq!(unified, columns(&f, "v_통합차수"));
    let unified = columns(&f, "v_통합");
    for column in columns(&f, "v_계약") {
        assert!(unified.contains(&column), "{column}");
    }

    // 감싸도 값은 모두 문자열이다.
    let c = f.database().open_read_only().unwrap();
    for view in views::NAMES {
        let mut s = c
            .prepare(&format!(
                "SELECT name, type FROM pragma_table_info('{view}')"
            ))
            .unwrap();
        let pairs: Vec<(String, String)> = s
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        for (name, kind) in pairs {
            assert!(
                name == "순번" || kind == "TEXT" || kind.is_empty(),
                "{view}.{name}: {kind}"
            );
        }
    }
}

/// `차수_뷰의_옛_줄은_제_차수의_품목으로_계산한다` — 품목 뷰가 최신 차수만 내므로 내부 표를 읽는다(§11.1-1).
#[test]
fn round_view_old_rows_use_their_own_items() {
    let f = Fixture::new("flowcol-rounds");
    f.notice(
        "TESTOLD",
        "000",
        &[],
        &[
            &[("line_no", "1"), ("item_name", "책상")],
            &[("line_no", "2"), ("item_name", "의자")],
        ],
        &[],
    );
    f.notice(
        "TESTOLD",
        "001",
        &[],
        &[&[("line_no", "1"), ("item_name", "모니터")]],
        &[],
    );
    let names = flow(
        items(),
        vec![
            json!({"op": "get", "field": "세부품명"}),
            json!({"op": "join", "text": ", "}),
        ],
    );
    f.store()
        .save_computed_column("notice", "품명들", &names, None)
        .unwrap();

    assert_eq!(
        read(&f, "SELECT 품명들 FROM v_공고차수 WHERE 차수 = '000'").as_deref(),
        Some("책상, 의자")
    );
    assert_eq!(
        read(&f, "SELECT 품명들 FROM v_공고차수 WHERE 차수 = '001'").as_deref(),
        Some("모니터")
    );
    assert_eq!(
        read(&f, "SELECT 품명들 FROM v_공고").as_deref(),
        Some("모니터")
    );

    // 꺼낸 SQL 은 계약면 품목 뷰를 읽으므로 최신 차수 기준이다 — 옛 줄에서는 빈 값.
    let eject = flow_compiler::eject("notice", &Flow::parse(&names).unwrap(), None).unwrap();
    assert!(eject.contains("-- 1단계: 꺼내기"), "{eject}");
    assert!(eject.contains("AS 결과"), "{eject}");
    sql_column::check_shape(Some(&eject)).unwrap();
    // 주석과 표시용 별칭이 추가돼도 기존 SQL 저장 검증과 공개 뷰 경계를 통과한다.
    f.store()
        .save_computed_column(
            "notice",
            "꺼낸품명",
            &json!({"version": 2, "id": "", "source": "sql", "sql": eject}).to_string(),
            None,
        )
        .unwrap();
    assert_eq!(
        read(&f, "SELECT 꺼낸품명 FROM v_공고").as_deref(),
        Some("모니터")
    );
    assert_eq!(
        read(
            &f,
            &format!("SELECT ({eject}) FROM v_공고차수 AS 현재 WHERE 차수 = '001'")
        )
        .as_deref(),
        Some("모니터")
    );
    assert_eq!(
        read(
            &f,
            &format!("SELECT ({eject}) FROM v_공고차수 AS 현재 WHERE 차수 = '000'")
        )
        .as_deref(),
        Some("")
    );
}

/// `정정한_값을_읽는다` — 자기 행의 열은 정정(`field_override`)을 마친 값이다. 연결 문서의 필드도 그 개체의 덮개 식을
/// 다시 불러 읽으므로 정정이 비친다.
#[test]
fn reads_corrected_values() {
    let f = Fixture::new("flowcol-override");
    // C# `BasePrice = 1000m` — Microsoft.Data.Sqlite 는 decimal 을 글로 묶는다.
    f.notice(
        "TESTN",
        "000",
        &[("title", "시험 공고"), ("base_price", "1000")],
        &[],
        &[],
    );
    f.contract("TESTK", "00", &[("title", "시험 계약")], &[]);
    {
        // `ReferenceFixture.Contract`
        let mut c = f.conn();
        let tx = c.transaction().unwrap();
        tx.execute(
            "UPDATE contract SET ref_notice_base=?1 WHERE contract_base=?2",
            ["TESTN", "TESTK"],
        )
        .unwrap();
        explicit_links::resolve(&tx).unwrap();
        tx.commit().unwrap();
    }
    let store = f.store();
    store
        .save_computed_column(
            "contract",
            "건명",
            &flow(
                json!({"kind": "field", "field": "계약건명"}),
                vec![json!({"op": "replace", "text": "시험", "to": "본"})],
            ),
            None,
        )
        .unwrap();
    store
        .save_computed_column(
            "contract",
            "공고기초",
            &flow(
                json!({"kind": "linked", "link": "notice", "field": "기초금액"}),
                vec![],
            ),
            None,
        )
        .unwrap();
    assert_eq!(
        read(&f, "SELECT 건명 FROM v_계약").as_deref(),
        Some("본 계약")
    );
    assert_eq!(
        read(&f, "SELECT 공고기초 FROM v_통합").as_deref(),
        Some("1,000")
    );

    let contract = EntityRef::new("contract", "TESTK", "00");
    store
        .set_override(&contract, "계약건명", "시험 고친 계약")
        .unwrap();
    store
        .set_override(
            &EntityRef::new("notice", "TESTN", "000"),
            "기초금액",
            "2,000",
        )
        .unwrap();
    assert_eq!(
        read(&f, "SELECT 건명 FROM v_계약").as_deref(),
        Some("본 고친 계약")
    );
    assert_eq!(
        read(&f, "SELECT 공고기초 FROM v_계약").as_deref(),
        Some("2,000")
    );

    // 가공열 칸은 여전히 고칠 수 없다.
    invalid(store.set_override(&contract, "건명", "수동값"));
    invalid(store.set_user_field(&contract, "건명", Some("수동값")));
}

/// `저장은_다시_검사하고_단계_번호로_거절한다` — 저장은 화면을 믿지 않고 다시 검사한다(§11.2). 꼴 오류는 단계 번호를
/// 담고, 실패하면 아무것도 남지 않는다. v1 이 와도 받아 v2 로 담는다.
#[test]
fn save_rechecks_and_refuses_with_step_numbers() {
    let f = Fixture::new("flowcol-save");
    let store = f.store();
    let save = |json: &str| store.save_computed_column("contract", "실패열", json, None);

    let error = invalid(save(&flow(
        items(),
        vec![json!({"op": "count"}), json!({"op": "join", "text": ", "})],
    )));
    assert!(error.starts_with("2단계: "), "{error}");
    let error = invalid(save(&flow(
        json!({"kind": "linked", "link": "contract"}),
        vec![],
    )));
    assert!(error.starts_with("시작 데이터: "), "{error}");
    let error = invalid(save(&flow(items(), vec![json!({"op": "distinct"})])));
    assert!(error.starts_with("1단계: "), "{error}");
    invalid(save(
        r#"{"version":2,"id":"","source":"sql","sql":"SELECT count(*) FROM contract_item"}"#,
    ));
    invalid(save(
        r#"{"version":2,"id":"","source":"flow","flow":{"from":{"kind":"items"},"steps":[{"op":"filter","cond":[{"field":"품명","cmp":"~"}]}]}}"#,
    ));
    assert!(
        !store
            .user_columns(None)
            .unwrap()
            .iter()
            .any(|c| c.field_name == "실패열")
    );
    f.database().refresh_views().unwrap();

    const LEGACY: &str = r#"{"version":1,"id":"","steps":[{"op":"pick","fields":["품명"]},{"op":"distinct"},{"op":"count"}]}"#;
    // `ComputedColumn.ReadForEditing` 은 Rust 에 없다 — `migrationError` 표식이 없는 식에서 그것은
    // `Flow.Serialize(Flow.ReadForIdentity(json))` 이므로 그 둘로 같은 것을 본다.
    let editable_text = Flow::serialize(&Flow::read_for_identity(LEGACY).unwrap()).unwrap();
    let editable: Value = serde_json::from_str(&editable_text).unwrap();
    assert_eq!(editable["version"].as_i64(), Some(2));
    assert!(editable.get("sql").is_none());
    assert_eq!(
        Flow::serialize(&computed_column::parse(LEGACY).unwrap()).unwrap(),
        editable_text
    );
    store
        .save_computed_column("contract", "옛식", LEGACY, None)
        .unwrap();
    let options = store
        .user_columns(Some("contract"))
        .unwrap()
        .into_iter()
        .find(|c| c.field_name == "옛식")
        .unwrap()
        .options
        .unwrap();
    let stored: Value = serde_json::from_str(&options).unwrap();
    assert_eq!(stored["version"].as_i64(), Some(2));
    assert_eq!(stored["source"].as_str(), Some("flow"));
    assert!(!stored["id"].as_str().unwrap().is_empty());
    assert_eq!(
        stored["flow"]["steps"],
        json!([{"op": "distinct", "fields": ["품명"]}, {"op": "count"}])
    );
    assert_eq!(
        read(&f, "SELECT 결과타입 FROM v_가공열 WHERE 열이름 = '옛식'").as_deref(),
        Some("number")
    );
    let shown: Value = serde_json::from_str(
        &read(&f, "SELECT 가공식 FROM v_가공열 WHERE 열이름 = '옛식'").unwrap(),
    )
    .unwrap();
    assert_eq!(stored, shown);
}

/// `경우는_첫_갈래가_이기는_CASE_로_컴파일한다` — 갈래마다 값 조건이고 처음 맞는 것이 이긴다. 그외가 없으면 빈 글이다.
#[test]
fn cases_compile_to_first_match_case() {
    let field = || json!({"kind": "field", "field": "계약건명"});
    let formula = Flow::parse(&flow(
        field(),
        vec![json!({
            "op": "cases",
            "branches": [
                {"cond": [{"field": "", "cmp": "안비어있음"}], "then": "있음"},
                {"cond": [{"field": "계약금액", "cmp": ">", "value": 10}], "then": "큼"},
            ],
        })],
    ))
    .unwrap();
    let eject = flow_compiler::eject("contract", &formula, None).unwrap();
    assert!(eject.contains("-- 1단계: 경우"), "{eject}");
    assert!(
        eject.contains("CASE WHEN (v <> '') THEN '있음' WHEN ((CASE WHEN"),
        "{eject}"
    );
    assert!(eject.contains("현재.\"계약금액\""), "{eject}");
    assert!(eject.contains("THEN '큼' ELSE '' END"), "{eject}");

    let problem = |step: Value| -> Option<String> {
        let parsed = Flow::parse(&flow(field(), vec![step])).unwrap();
        Flow::check(
            parsed.flow.as_ref().unwrap(),
            &Flow::env_for("contract").unwrap(),
        )
        .errors
        .first()
        .map(|e| e.message.clone())
    };
    assert_eq!(
        problem(json!({"op": "cases", "branches": []})).as_deref(),
        Some("경우를 하나 이상 적으세요")
    );
    assert_eq!(
        problem(json!({"op": "cases", "branches": [
            {"cond": [{"field": "", "cmp": "비어있음"}], "then": ""},
            {"cond": [{"field": "없는열", "cmp": "비어있음"}], "then": ""},
        ]}))
        .as_deref(),
        Some("2번째 경우: 이 행에 없는 열: 없는열")
    );
    let many: Vec<Value> = (0..21)
        .map(|_| json!({"cond": [{"field": "", "cmp": "비어있음"}], "then": ""}))
        .collect();
    assert_eq!(
        problem(json!({"op": "cases", "branches": many})).as_deref(),
        Some("경우는 20개까지입니다")
    );
    invalid(Flow::parse(&flow(
        items(),
        vec![json!({"op": "cases", "branches": [{"then": "x"}]})],
    )));
}

/// `앞부분만_컴파일한다` — 단계 k 까지의 값을 SQL 로(보표가 쓴다). 뒤에 오류가 있어도 앞은 컴파일된다.
#[test]
fn compiles_prefixes_only() {
    let f = Fixture::new("flowcol-prefix");
    f.contract(
        "TESTK",
        "00",
        &[],
        &[
            &[("line_no", "1"), ("item_name", "책상")],
            &[("line_no", "2"), ("item_name", "책상")],
            &[("line_no", "3"), ("item_name", "의자")],
        ],
    );
    let parsed = Flow::parse(&flow(
        items(),
        vec![
            json!({"op": "get", "field": "품명"}),
            json!({"op": "distinct"}),
            json!({"op": "count"}),
            json!({"op": "join", "text": ","}),
        ],
    ))
    .unwrap();
    let definition = parsed.flow.unwrap();
    let at = |k: usize| flow_compiler::prefix("contract", &definition, k, Target::Internal, None);
    assert_eq!(at(0).unwrap().shape, "rows");
    assert_eq!(
        read(
            &f,
            &format!(
                "SELECT (SELECT count(*) FROM ({})) FROM v_계약 AS 현재",
                at(0).unwrap().sql
            )
        )
        .as_deref(),
        Some("3")
    );
    assert_eq!(at(2).unwrap().shape, "values");
    assert_eq!(
        read(
            &f,
            &format!(
                "SELECT (SELECT group_concat(__value, ',' ORDER BY __order) FROM ({})) FROM v_계약 AS 현재",
                at(2).unwrap().sql
            )
        )
        .as_deref(),
        Some("책상,의자")
    );
    assert_eq!(at(3).unwrap().shape, "number");
    assert_eq!(
        read(
            &f,
            &format!(
                "SELECT CAST({} AS TEXT) FROM v_계약 AS 현재",
                at(3).unwrap().sql
            )
        )
        .as_deref(),
        Some("2")
    );
    let error = invalid(at(4));
    assert!(error.starts_with("4단계: "), "{error}");
}

/// `판올림은_깨진_가공식을_건너뛰고_v1_가공식을_옮긴다` — 스키마 22 는 읽을 수 없는 가공식을 건너뛴다(그 열은 뒤에 깨진
/// 열로 선다). json_valid 없이 json_extract 를 불러 깨진 식 한 줄이 판올림을 멈췄다.
#[test]
fn migration_skips_broken_formulas_and_moves_v1() {
    let temp = common::Temp::new("flowcol-v21-broken");
    let path = temp.path("v21.db");
    {
        let old = common::raw(&path);
        let sql = format!(
            "{}\n{}\n{}\n{}",
            schema::BASELINE,
            schema::STEPS[0],
            schema::STEPS[1],
            r#"INSERT INTO user_column VALUES ('notice', '깨짐', 'computed', '{"version":1', 31);
INSERT INTO user_column VALUES ('notice', '글', 'computed', 'not json', 32);
INSERT INTO user_column VALUES ('notice', '물품요약', 'computed',
  '{"version":1,"id":"keep","steps":[{"op":"pick","fields":["세부품명","세부품명번호"],"text":null,"index":null},{"op":"distinct","fields":null,"text":null,"index":null},{"op":"format","fields":null,"text":"{세부품명}({세부품명번호})","index":null},{"op":"join","fields":null,"text":", ","index":null}]}', 33);
PRAGMA user_version=21;"#
        );
        old.execute_batch(&sql).unwrap();
    }
    let env: &'static TestEnv = Box::leak(Box::new(TestEnv::new()));
    Database::new(&path, Access::Write).migrate(env).unwrap();
    let f = Fixture { temp, path, env };
    let store = f.store();
    let options = |name: &str| {
        store
            .user_columns(Some("notice"))
            .unwrap()
            .into_iter()
            .find(|c| c.field_name == name)
            .unwrap()
            .options
            .unwrap()
    };
    assert_eq!(options("깨짐"), "{\"version\":1");
    assert_eq!(options("글"), "not json");
    assert!(options("물품요약").starts_with("{\"version\":2,\"id\":\"keep\""));
    let broken = store.broken_columns().unwrap();
    assert!(
        broken
            .get(&("notice".to_string(), "깨짐".to_string()))
            .is_some()
    );
    assert!(
        broken
            .get(&("notice".to_string(), "글".to_string()))
            .is_some()
    );
    assert!(
        broken
            .get(&("notice".to_string(), "물품요약".to_string()))
            .is_none()
    );
}

/// `판올림은_v1_가공식을_v2_로_옮긴다` — 스키마 22 가 저장된 v1 가공식을 v2 로 옮겨 쓴다. 다른 사람 열과 손으로 적은 값은
/// 그대로이고, 옮긴 식이 앞 판과 같은 값을 낸다.
#[test]
fn migration_moves_v1_formulas_to_v2() {
    let temp = common::Temp::new("flowcol-v21");
    let path = temp.path("v21.db");
    {
        let old = common::raw(&path);
        // C# 은 단계를 바로 잇는다 — 여기서는 줄을 끊는다(주석으로 끝나는 단계가 다음 문장을 먹지 않게).
        let sql = format!(
            "{}\n{}\n{}\n{}",
            schema::BASELINE,
            schema::STEPS[0],
            schema::STEPS[1],
            r#"INSERT INTO user_column VALUES ('notice', '물품요약', 'computed',
  '{"version":1,"id":"keep","steps":[{"op":"pick","fields":["세부품명","세부품명번호"],"text":null,"index":null},{"op":"distinct","fields":null,"text":null,"index":null},{"op":"format","fields":null,"text":"{세부품명}({세부품명번호})","index":null},{"op":"join","fields":null,"text":", ","index":null}]}', 30);
PRAGMA user_version=21;"#
        );
        old.execute_batch(&sql).unwrap();
    }
    let env: &'static TestEnv = Box::leak(Box::new(TestEnv::new()));
    Database::new(&path, Access::Write).migrate(env).unwrap();
    let f = Fixture { temp, path, env };
    let store = f.store();
    let column = store
        .user_columns(Some("notice"))
        .unwrap()
        .into_iter()
        .find(|c| c.field_name == "물품요약")
        .unwrap();
    assert_eq!(column.kind, "computed");
    assert_eq!(
        column.options.as_deref(),
        Some(
            r#"{"version":2,"id":"keep","source":"flow","flow":{"from":{"kind":"items"},"steps":[{"op":"distinct","fields":["세부품명","세부품명번호"]},{"op":"format","text":"{세부품명}({세부품명번호})"},{"op":"join","text":", "}]}}"#
        )
    );
    assert_eq!(people(&f, "notice"), ["검토여부", "메모", "물품요약"]);

    f.notice(
        "TESTN",
        "000",
        &[],
        &[
            &[
                ("line_no", "3"),
                ("item_name", "책상"),
                ("detail_item_number", "002"),
            ],
            &[
                ("line_no", "1"),
                ("item_name", "책상"),
                ("detail_item_number", "001"),
            ],
            &[
                ("line_no", "2"),
                ("item_name", "책상"),
                ("detail_item_number", "001"),
            ],
        ],
        &[],
    );
    let c = f.database().open_read_only().unwrap();
    assert_eq!(
        single(&c, "SELECT 물품요약 FROM v_공고").as_deref(),
        Some("책상(001), 책상(002)")
    );
    assert_eq!(
        single(&c, "SELECT 식별자 FROM v_가공열").as_deref(),
        Some("keep")
    );
}

//! 제출과 취합(C# `MergeTests`).
//!
//! 여기서 붙드는 것은 편의가 아니라 **조용한 유실**이다. 취합은 사람이 적은 것을 서로 밀어내는 일이라, 밀린 것이
//! 알림 없이 사라지면 두 사람 다 그 사실을 모른다. 그래서 기계는 고르기만 하고 **무엇을 밀어냈는지 반드시
//! 알린다**(ADR-016 의 결) — 그 규율을 이 시험이 붙든다.
//!
//! 값은 전부 지어낸 것이다. 실제 조달 자료는 시험에 오지 않는다.

mod common;

use std::path::{Path, PathBuf};

use common::{Temp, TestEnv, raw, suffixed};
use pclm_core::local_time::FixedZone;
use pclm_core::storage::merger::{self, MergeRejection, MergeReport};
use pclm_core::storage::{merge_report_text, pclm_file, schema};
use pclm_core::{Access, Database, Error, PclmKind, PclmRole};
use rusqlite::types::Value;

struct Fixture {
    temp: Temp,
    env: TestEnv,
}

impl Fixture {
    fn new() -> Self {
        let temp = Temp::new("merge");
        std::fs::create_dir_all(temp.path("임시")).unwrap();
        Fixture {
            temp,
            env: TestEnv::new(),
        }
    }

    fn at(&self, name: &str) -> PathBuf {
        self.temp.path(name)
    }

    fn out(&self) -> PathBuf {
        self.at("취합.pclm")
    }

    /// 제출본 하나를 짓는다. 남의 설정도 함께 담아 둔다 — 그것이 취합본으로 새지 않는지를 시험이 봐야 한다.
    fn submission(&self, submitter: &str, sql: &[&str]) -> PathBuf {
        let path = self.at(&format!("제출_{submitter}.pclm"));
        let database = pclm_file::create(&path, PclmRole::SUBMISSION, &self.env).unwrap();
        let connection = database.open().unwrap();
        connection
            .execute(
                "INSERT INTO app_setting (key, value) VALUES ('submit.name', ?1);",
                [submitter],
            )
            .unwrap();
        connection
            .execute_batch(
                r"INSERT INTO app_setting (key, value) VALUES ('plan.path', 'C:\남의폴더\조달계획.xlsx');",
            )
            .unwrap();
        for statement in sql {
            connection.execute_batch(statement).unwrap();
        }
        path
    }

    /// 작업자료 하나를 `작업` 폴더에 짓는다(크레이트는 폴더를 짓지 않는다).
    fn work(&self, name: &str) -> Database {
        std::fs::create_dir_all(self.at("작업")).unwrap();
        pclm_file::create(&self.at("작업").join(name), PclmRole::WORK, &self.env).unwrap()
    }

    fn merge(&self, sources: &[&Path], out: &Path) -> pclm_core::Result<MergeReport> {
        merger::merge(sources, out, &self.at("임시"), &self.env)
    }

    fn render(&self, report: &MergeReport) -> String {
        merge_report_text::render(report, &self.out(), &FixedZone(9 * 3_600))
    }
}

/// 공고 한 장. **계열에 건을 함께 넣는다** — `group_base` 가 NOT NULL 이라 빠뜨리면 `OR IGNORE` 가 그 위반을 삼켜
/// 계열이 서지 않는다.
fn notice(base: &str, title: &str, updated: &str) -> String {
    format!(
        "INSERT OR IGNORE INTO notice_group  (group_base) VALUES ('{base}');
INSERT OR IGNORE INTO notice_series (notice_base, group_base) VALUES ('{base}', '{base}');
INSERT INTO notice (notice_base, seq, title, updated_at) VALUES ('{base}', '0', '{title}', '{updated}');"
    )
}

fn contract(base: &str, title: &str, updated: &str) -> String {
    format!(
        "INSERT OR IGNORE INTO contract_series (contract_base) VALUES ('{base}');
INSERT INTO contract (contract_base, seq, title, updated_at) VALUES ('{base}', '0', '{title}', '{updated}');"
    )
}

fn request(base: &str, title: &str, updated: &str) -> String {
    format!(
        "INSERT OR IGNORE INTO request_series (request_base) VALUES ('{base}');
INSERT INTO request (request_base, seq, title, updated_at) VALUES ('{base}', '0', '{title}', '{updated}');"
    )
}

fn contract_item(base: &str, line: i32, name: &str) -> String {
    format!(
        "INSERT INTO contract_item (contract_base, seq, line_no, item_name, quantity)
VALUES ('{base}', '0', {line}, '{name}', {});",
        line * 10
    )
}

/// 줄마다 칸을 `|` 로 잇는다. NULL 은 빈 글이다.
fn read(db: &Path, sql: &str) -> Vec<String> {
    let connection = Database::new(db, Access::Read).open().unwrap();
    let mut statement = connection.prepare(sql).unwrap();
    let width = statement.column_count();
    statement
        .query_map([], |r| {
            Ok((0..width)
                .map(|i| match r.get::<_, Value>(i).unwrap() {
                    Value::Null => String::new(),
                    Value::Integer(n) => n.to_string(),
                    Value::Real(f) => f.to_string(),
                    Value::Text(s) => s,
                    Value::Blob(_) => "System.Byte[]".into(),
                })
                .collect::<Vec<_>>()
                .join("|"))
        })
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

fn count(db: &Path, sql: &str) -> i64 {
    read(db, sql)[0].parse().unwrap()
}

fn rejection(file: &str, reason: &str) -> MergeRejection {
    MergeRejection {
        file: file.into(),
        reason: reason.into(),
    }
}

#[test]
fn erp_원천키와_업체는_채택된_계약의_것을_취합한다() {
    let f = Fixture::new();
    let evidence = |key: &str| {
        format!(
            r#"INSERT INTO erp_source VALUES('contract','C-1','0','{{"rows":[]}}','{key}','2026-09-27');
INSERT INTO erp_row VALUES('contract','C-1','0','contract_item','{key}',1);
INSERT INTO erp_partner(contract_base,seq,line_no,name) VALUES('C-1','0',1,'{key}');"#
        )
    };
    let first = f.submission(
        "이전",
        &[
            &contract("C-1", "이전", "2026-09-01"),
            &contract_item("C-1", 1, "이전"),
            &evidence("old"),
        ],
    );
    let last = f.submission(
        "최신",
        &[
            &contract("C-1", "최신", "2026-09-27"),
            &contract_item("C-1", 1, "최신"),
            &evidence("new"),
        ],
    );
    f.merge(&[&first, &last], &f.out()).unwrap();
    assert_eq!(
        read(&f.out(), "SELECT mapping_revision FROM erp_source"),
        ["new"]
    );
    assert_eq!(read(&f.out(), "SELECT source_key FROM erp_row"), ["new"]);
    assert_eq!(read(&f.out(), "SELECT name FROM erp_partner"), ["new"]);
    assert_eq!(count(&f.out(), "SELECT count(*) FROM erp_capture"), 0);
}

// ── 겹친 것을 알린다 ────────────────────────────────────────────

/// **알림이 본체다.** 밀린 쪽이 어느 제출본에서 온 무엇이었는지 나오지 않으면, 그 사람은 자기가 적은 것이 사라진
/// 줄도 모른다.
#[test]
fn 겹친_키를_알리고_늦은_것을_싣는다() {
    let f = Fixture::new();
    let hong = f.submission(
        "홍길동",
        &[&contract(
            "C-1",
            "먼저 적은 건명",
            "2026-08-22T09:00:00.0000000Z",
        )],
    );
    let kim = f.submission(
        "김철수",
        &[&contract(
            "C-1",
            "나중에 적은 건명",
            "2026-08-28T09:00:00.0000000Z",
        )],
    );

    let report = f.merge(&[&hong, &kim], &f.out()).unwrap();

    assert_eq!(
        read(&f.out(), "SELECT title FROM contract;"),
        ["나중에 적은 건명"]
    );
    let [conflict] = report.conflicts.as_slice() else {
        panic!("겹친 것이 하나여야 한다: {:?}", report.conflicts);
    };
    assert_eq!(conflict.kind, "계약");
    assert_eq!(conflict.key, "C-1·0");
    assert_eq!(conflict.kept.submitter, "김철수");
    let [pushed] = conflict.pushed.as_slice() else {
        panic!("밀린 것이 하나여야 한다");
    };
    assert_eq!(pushed.submitter, "홍길동");
    assert!(pushed.time.contains("08-22"));
}

/// 시각이 같으면 먼저 온 제출본을 싣는다 — 그것도 알린다.
#[test]
fn 시각이_같으면_먼저_온_제출본을_싣고_그것도_알린다() {
    let f = Fixture::new();
    let same = "2026-08-25T09:00:00.0000000Z";
    let hong = f.submission("홍길동", &[&contract("C-1", "홍길동의 것", same)]);
    let kim = f.submission("김철수", &[&contract("C-1", "김철수의 것", same)]);

    let report = f.merge(&[&hong, &kim], &f.out()).unwrap();

    assert_eq!(
        read(&f.out(), "SELECT title FROM contract;"),
        ["홍길동의 것"]
    );
    assert_eq!(report.conflicts.len(), 1);
    assert_eq!(report.conflicts[0].kept.submitter, "홍길동");
}

/// **같은 값을 들고 있는 것은 겹침이 아니라 일치다.** 시각만 갈리는 것까지 한 줄씩 내면 진짜 갈린 둘이 수백 줄에
/// 묻힌다.
#[test]
fn 값이_같으면_겹친_것으로_내지_않는다() {
    let f = Fixture::new();
    let hong = f.submission(
        "홍길동",
        &[&notice(
            "N-1",
            "같은 공고문",
            "2026-08-22T09:00:00.0000000Z",
        )],
    );
    let kim = f.submission(
        "김철수",
        &[&notice(
            "N-1",
            "같은 공고문",
            "2026-08-28T09:00:00.0000000Z",
        )],
    );

    let report = f.merge(&[&hong, &kim], &f.out()).unwrap();

    assert!(report.conflicts.is_empty());
    assert_eq!(report.notices, 1);

    // 값이 하나라도 갈리면 그때는 반드시 낸다.
    let park = f.submission(
        "박영수",
        &[&notice(
            "N-1",
            "다르게 읽힌 공고문",
            "2026-08-30T09:00:00.0000000Z",
        )],
    );
    assert_eq!(
        f.merge(&[&hong, &kim, &park], &f.out())
            .unwrap()
            .conflicts
            .len(),
        1
    );
}

/// 딸린 줄은 **부모를 따라 통째로** 간다. 반씩 섞으면 **어느 문서에도 없던 표**가 선다.
#[test]
fn 딸린_줄은_이긴_제출본의_것만_실린다() {
    let f = Fixture::new();
    let hong = f.submission(
        "홍길동",
        &[
            &contract("C-1", "옛 차수", "2026-08-22T09:00:00.0000000Z"),
            &contract_item("C-1", 1, "홍길동이 읽은 품목"),
            &contract_item("C-1", 2, "홍길동만 가진 둘째 줄"),
        ],
    );
    let kim = f.submission(
        "김철수",
        &[
            &contract("C-1", "새 차수", "2026-08-28T09:00:00.0000000Z"),
            &contract_item("C-1", 1, "김철수가 읽은 품목"),
        ],
    );

    f.merge(&[&hong, &kim], &f.out()).unwrap();

    // 이긴 쪽의 줄만 남는다. 진 쪽의 둘째 줄이 따라 들어오면 없던 표가 된다.
    assert_eq!(
        read(
            &f.out(),
            "SELECT item_name FROM contract_item ORDER BY line_no;"
        ),
        ["김철수가 읽은 품목"]
    );
}

#[test]
fn 취합한_명시참조로_관계를_재생성한다() {
    let f = Fixture::new();
    let first = f.submission(
        "담당자A",
        &[
            &request("R-1", "접수", "2026-08-20"),
            &notice("N-1", "공고", "2026-08-20"),
            "INSERT INTO notice_item(notice_base,seq,line_no,ref_request_base) VALUES('N-1','0',1,'R-1');",
        ],
    );
    let second = f.submission(
        "담당자B",
        &[
            &contract("C-1", "계약", "2026-08-21"),
            "UPDATE contract SET ref_notice_base='N-1' WHERE contract_base='C-1';",
        ],
    );
    f.merge(&[&first, &second], &f.out()).unwrap();
    assert_eq!(
        read(
            &f.out(),
            "SELECT request_base, notice_group FROM request_link;"
        ),
        ["R-1|N-1"]
    );
    assert_eq!(
        read(
            &f.out(),
            "SELECT contract_base, notice_group FROM project_link;"
        ),
        ["C-1|N-1"]
    );
}

// ── 판 ──────────────────────────────────────────────────────────

/// 기준선보다 옛 시험판 제출본은 올릴 단계가 이 프로그램에 없다. **거절하되 멈추지는 않는다.**
#[test]
fn 옛_시험판_제출본은_거절하고_나머지는_합친다() {
    let f = Fixture::new();
    let old = f.at("제출_옛판.pclm");
    raw(&old)
        .execute_batch(
            "CREATE TABLE notice (notice_base TEXT, seq TEXT); PRAGMA user_version = 11;",
        )
        .unwrap();
    let hong = f.submission(
        "홍길동",
        &[&notice(
            "N-1",
            "지금 판에서 온 공고",
            "2026-08-20T09:00:00.0000000Z",
        )],
    );

    let report = f.merge(&[&old, &hong], &f.out()).unwrap();

    assert_eq!(
        report.rejected,
        [rejection("제출_옛판.pclm", merger::TOO_OLD)]
    );
    assert_eq!(report.submissions, 1);
    assert_eq!(
        read(&f.out(), "SELECT title FROM notice;"),
        ["지금 판에서 온 공고"]
    );
    assert!(f.render(&report).contains("0.7.0"));
}

/// 판이 새로운 것은 내릴 길이 없다. **거절하되 멈추지는 않는다.**
#[test]
fn 새_판_제출본은_거절하고_나머지는_합친다() {
    let f = Fixture::new();
    let newer = f.submission(
        "앞서간이",
        &[&notice(
            "N-앞",
            "새 판에서 온 공고",
            "2026-08-20T09:00:00.0000000Z",
        )],
    );
    Database::new(&newer, Access::Write)
        .open()
        .unwrap()
        .execute_batch(&format!("PRAGMA user_version = {};", schema::VERSION + 7))
        .unwrap();
    let hong = f.submission(
        "홍길동",
        &[&notice(
            "N-1",
            "지금 판에서 온 공고",
            "2026-08-20T09:00:00.0000000Z",
        )],
    );

    let report = f.merge(&[&newer, &hong], &f.out()).unwrap();

    assert_eq!(
        report.rejected,
        [rejection("제출_앞서간이.pclm", merger::TOO_NEW)]
    );
    assert_eq!(report.submissions, 1);
    assert_eq!(
        read(&f.out(), "SELECT title FROM notice;"),
        ["지금 판에서 온 공고"]
    );
}

// ── 매번 처음부터 짓는다 ────────────────────────────────────────

/// **묘비를 두지 않으므로** 취합본을 고쳐 쓸 수 없다. 고쳐 쓰면 누가 자기 DB 에서 지운 건이 다음 취합본에 영영 남는다.
#[test]
fn 지운_것은_다음_취합본에_없다() {
    let f = Fixture::new();
    let first = f.submission(
        "홍길동",
        &[
            &notice("N-1", "남을 공고", "2026-08-20T09:00:00.0000000Z"),
            &notice("N-2", "지울 공고", "2026-08-20T09:00:00.0000000Z"),
        ],
    );

    assert_eq!(f.merge(&[&first], &f.out()).unwrap().notices, 2);

    // 사람이 자기 DB 에서 N-2 를 지우고 다시 제출했다.
    Database::new(&first, Access::Write)
        .open()
        .unwrap()
        .execute_batch(
            "DELETE FROM notice WHERE notice_base = 'N-2'; DELETE FROM notice_series WHERE notice_base = 'N-2';",
        )
        .unwrap();

    let report = f.merge(&[&first], &f.out()).unwrap();

    assert_eq!(report.notices, 1);
    assert_eq!(read(&f.out(), "SELECT title FROM notice;"), ["남을 공고"]);
}

// ── 사람이 세운 것 ──────────────────────────────────────────────

/// 사용자 열은 **합집합**이다 — 이름이 같으면 하나로 다르면 둘 다 세운다. 값은 계열 키에 매달려 그대로 따라온다.
#[test]
fn 사용자_열은_합집합이고_값이_따라온다() {
    let f = Fixture::new();
    let hong = f.submission(
        "홍길동",
        &[
            &contract("C-1", "홍길동의 계약", "2026-08-20T09:00:00.0000000Z"),
            "INSERT INTO user_column (entity_type, field_name, kind, options, sort_order)
VALUES ('contract', '홍길동칸', 'text', NULL, 50);
INSERT INTO contract_user_field (contract_base, field_name, value, updated_at)
VALUES ('C-1', '홍길동칸', '홍길동이 적은 값', '2026-08-22T09:00:00.0000000Z');",
        ],
    );
    let kim = f.submission(
        "김철수",
        &[
            &notice("N-1", "김철수의 공고", "2026-08-20T09:00:00.0000000Z"),
            "INSERT INTO user_column (entity_type, field_name, kind, options, sort_order)
VALUES ('notice', '김철수칸', 'choice', '가|나|다', 60);
INSERT INTO notice_user_field (notice_base, field_name, value, updated_at)
VALUES ('N-1', '김철수칸', '나', '2026-08-22T09:00:00.0000000Z');",
        ],
    );

    f.merge(&[&hong, &kim], &f.out()).unwrap();

    let columns = read(&f.out(), "SELECT entity_type, field_name FROM user_column;");
    assert!(columns.contains(&"contract|홍길동칸".to_string()));
    assert!(columns.contains(&"notice|김철수칸".to_string()));

    // 뷰는 열 정의를 읽어 짓는 것이라, 다시 짓지 않으면 그 칸이 계약면에 서지 않는다.
    assert_eq!(
        read(&f.out(), "SELECT 홍길동칸 FROM v_계약;"),
        ["홍길동이 적은 값"]
    );
    assert_eq!(read(&f.out(), "SELECT 김철수칸 FROM v_공고;"), ["나"]);
}

/// 이름이 같은데 뜻이 다르면 먼저 온 것을 두고 **알린다** — 누가 옳은지 기계가 가리지 않는다.
#[test]
fn 같은_이름의_열이_뜻이_다르면_알린다() {
    let f = Fixture::new();
    let hong = f.submission(
        "홍길동",
        &[
            "INSERT INTO user_column (entity_type, field_name, kind, options, sort_order)
VALUES ('contract', '진행단계', 'choice', '준비|계약', 50);",
        ],
    );
    let kim = f.submission(
        "김철수",
        &[
            "INSERT INTO user_column (entity_type, field_name, kind, options, sort_order)
VALUES ('contract', '진행단계', 'choice', '준비|투찰|낙찰|계약', 50);",
        ],
    );

    let report = f.merge(&[&hong, &kim], &f.out()).unwrap();

    assert_eq!(
        read(
            &f.out(),
            "SELECT options FROM user_column WHERE field_name = '진행단계';"
        ),
        ["준비|계약"]
    );
    assert_eq!(report.conflicts.len(), 1);
    assert_eq!(report.conflicts[0].kind, "사용자 열");
    assert_eq!(report.conflicts[0].key, "contract·진행단계");
}

// ── 새지 않는다 ─────────────────────────────────────────────────

/// 취합기가 아는 표가 **스키마의 표 전부**인가. 표를 하나 세우고 여기를 고치지 않으면 그 표는 취합본에서 **말없이
/// 빠진다**. 두고 오는 것은 [`merger::UNTOUCHED`] 에 적힌 것만이다.
#[test]
fn 취합기가_아는_표가_스키마의_표_전부다() {
    let f = Fixture::new();
    let database = pclm_file::create(&f.at("스키마.db"), PclmRole::WORK, &f.env).unwrap();
    let connection = database.open_read_only().unwrap();
    let actual: Vec<String> = connection
        .prepare(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%';",
        )
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();

    let known: Vec<&str> = merger::tables()
        .into_iter()
        .chain(merger::UNTOUCHED)
        .collect();

    let missing: Vec<&String> = actual
        .iter()
        .filter(|t| !known.contains(&t.as_str()))
        .collect();
    assert!(missing.is_empty(), "새 표를 빠뜨렸다: {missing:?}");
    let ghosts: Vec<&&str> = known
        .iter()
        .filter(|t| !actual.iter().any(|a| a == **t))
        .collect();
    assert!(ghosts.is_empty(), "없는 표를 나르려 한다: {ghosts:?}");
}

/// 제출한 사람의 설정은 **넘어오지 않는다.**
#[test]
fn 남의_설정은_취합본으로_넘어오지_않는다() {
    let f = Fixture::new();
    let hong = f.submission(
        "홍길동",
        &[&notice("N-1", "공고", "2026-08-20T09:00:00.0000000Z")],
    );

    f.merge(&[&hong], &f.out()).unwrap();

    assert_eq!(count(&f.out(), "SELECT COUNT(*) FROM app_setting;"), 0);
}

/// 취합본은 **한 파일**이다. `-wal` 이 옆에 남으면 받는 쪽이 본체만 가져가 마지막 것이 빠진 자료를 연다.
#[test]
fn 취합본은_한_파일로_떨어진다() {
    let f = Fixture::new();
    let hong = f.submission(
        "홍길동",
        &[&notice("N-1", "공고", "2026-08-20T09:00:00.0000000Z")],
    );

    f.merge(&[&hong], &f.out()).unwrap();

    assert!(f.out().exists());
    assert!(!suffixed(&f.out(), "-wal").exists());
    assert!(!suffixed(&f.out(), "-shm").exists());
    // 작업 폴더의 사본도 남지 않는다.
    assert_eq!(std::fs::read_dir(f.at("임시")).unwrap().count(), 0);
}

/// 조용히 깨진 취합본을 내놓지 않는다.
#[test]
fn 취합_뒤_외래키가_성하다() {
    let f = Fixture::new();
    let hong = f.submission(
        "홍길동",
        &[
            &request("R-1", "접수", "2026-08-20T09:00:00.0000000Z"),
            &notice("N-1", "공고", "2026-08-20T09:00:00.0000000Z"),
            &contract("C-1", "계약", "2026-08-20T09:00:00.0000000Z"),
            &contract_item("C-1", 1, "품목"),
            "INSERT INTO notice_item(notice_base,seq,line_no,ref_request_base) VALUES('N-1','0',1,'R-1');
UPDATE contract SET ref_notice_base='N-1' WHERE contract_base='C-1';
INSERT INTO contract_user_field (contract_base, field_name, value, updated_at)
VALUES ('C-1', '메모', '지어낸 메모', '2026-08-22T09:00:00.0000000Z');",
        ],
    );
    let kim = f.submission(
        "김철수",
        &[&notice("N-2", "다른 공고", "2026-08-20T09:00:00.0000000Z")],
    );

    let report = f.merge(&[&hong, &kim], &f.out()).unwrap();

    assert!(read(&f.out(), "PRAGMA foreign_key_check;").is_empty());
    assert_eq!(report.submissions, 2);
}

/// 제출자 이름을 정하지 않았으면 파일 이름으로 부른다.
#[test]
fn 이름이_비면_파일_이름으로_부른다() {
    let f = Fixture::new();
    let unnamed = f.at("제출_20260729.pclm");
    pclm_file::create(&unnamed, PclmRole::WORK, &f.env)
        .unwrap()
        .open()
        .unwrap()
        .execute_batch(&contract(
            "C-1",
            "이름 없는 이의 계약",
            "2026-08-28T09:00:00.0000000Z",
        ))
        .unwrap();
    let hong = f.submission(
        "홍길동",
        &[&contract(
            "C-1",
            "홍길동의 계약",
            "2026-08-20T09:00:00.0000000Z",
        )],
    );

    let report = f.merge(&[&unnamed, &hong], &f.out()).unwrap();

    assert_eq!(report.conflicts.len(), 1);
    assert_eq!(report.conflicts[0].kept.submitter, "제출_20260729");
}

// ── 걷기와 보고 ─────────────────────────────────────────────────

/// 지난 취합본은 제출본으로 세지 않는다. 가르는 것은 **이름이 아니라 역할**이다 — 이름을 바꾼 취합본도 빠지고,
/// `취합_` 으로 시작하는 제출본도 들어간다.
#[test]
fn 취합본은_이름이_아니라_역할로_거절한다() {
    let f = Fixture::new();
    let hong = f.submission(
        "홍길동",
        &[&contract(
            "C-1",
            "홍길동의 것",
            "2026-08-22T09:00:00.0000000Z",
        )],
    );
    f.submission(
        "김철수",
        &[&contract(
            "C-2",
            "김철수의 것",
            "2026-08-23T09:00:00.0000000Z",
        )],
    );

    // 지난 취합본이 같은 폴더에 있다 — 누가 이름을 바꿔 두었다.
    f.merge(&[&hong], &f.at("지난것.pclm")).unwrap();

    // 이름이 취합_ 으로 시작하는 제출본. 예전에는 말없이 빠졌다.
    let work = f.work("내것.pclm");
    work.open()
        .unwrap()
        .execute_batch(&contract(
            "C-3",
            "이름이 헷갈리는 이의 것",
            "2026-08-24T09:00:00.0000000Z",
        ))
        .unwrap();
    pclm_file::snapshot(
        &work,
        &f.at("취합_이몽룡.pclm"),
        PclmRole::SUBMISSION,
        false,
        &f.env,
    )
    .unwrap();

    let found = merger::find_submissions(&f.temp.root);
    let names: Vec<String> = found
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        names,
        [
            "제출_김철수.pclm",
            "제출_홍길동.pclm",
            "지난것.pclm",
            "취합_이몽룡.pclm"
        ]
    );

    let report = merger::merge(&found, &f.out(), &f.at("임시"), &f.env).unwrap();

    assert_eq!(report.rejected, [rejection("지난것.pclm", "취합본")]);
    assert_eq!(report.submissions, 3);
    assert_eq!(
        read(&f.out(), "SELECT contract_base FROM contract ORDER BY 1;"),
        ["C-1", "C-2", "C-3"]
    );
    assert!(f.render(&report).contains("지난것.pclm — 취합본"));
}

/// 받아들인 제출본이 없으면 짓지 않는다. 그대로 지으면 같은 날짜의 취합본을 빈 것으로 덮고 성공처럼 알린다 — 대상
/// 파일에 닿기 전에 까닭을 들어 멈춘다.
#[test]
fn 받아들인_제출본이_없으면_멈추고_있던_취합본을_건드리지_않는다() {
    let f = Fixture::new();
    let hong = f.submission(
        "홍길동",
        &[&contract(
            "C-1",
            "홍길동의 것",
            "2026-08-22T09:00:00.0000000Z",
        )],
    );
    f.merge(&[&hong], &f.out()).unwrap();
    let work = f.work("내것.pclm");
    let backup = pclm_file::snapshot(
        &work,
        &f.at("백업_내것.pclm"),
        PclmRole::BACKUP,
        false,
        &f.env,
    )
    .unwrap();
    std::fs::remove_file(&hong).unwrap();

    let before = std::fs::read(f.out()).unwrap();

    let refused = merger::merge(
        &merger::find_submissions(&f.temp.root),
        &f.out(),
        &f.at("임시"),
        &f.env,
    )
    .unwrap_err();
    let Error::InvalidOperation(message) = refused else {
        panic!("InvalidOperation 이어야 한다: {refused:?}");
    };
    assert!(
        message.starts_with("합칠 수 있는 제출본이 없습니다"),
        "{message}"
    );
    assert!(message.contains("백업_내것.pclm(백업)"), "{message}");
    assert_eq!(std::fs::read(f.out()).unwrap(), before);
    assert_eq!(
        read(&f.out(), "SELECT contract_base FROM contract;"),
        ["C-1"]
    );
    assert!(backup.exists());
}

/// 남의 파일은 읽기만 한다 — 판을 올리는 것도 사본에서다. PCLM 이 아닌 파일은 까닭을 달아 거절하고 나머지는 합친다.
#[test]
fn 입력_파일은_한_바이트도_바뀌지_않고_pclm_아닌_것은_까닭과_함께_거절한다() {
    let f = Fixture::new();
    let work = f.work("내것.pclm");
    work.open()
        .unwrap()
        .execute_batch(&contract(
            "C-1",
            "지어낸 계약",
            "2026-08-24T09:00:00.0000000Z",
        ))
        .unwrap();
    let submitted = pclm_file::snapshot(
        &work,
        &f.at("제출_홍길동.pclm"),
        PclmRole::SUBMISSION,
        false,
        &f.env,
    )
    .unwrap();

    let odd = f.at("제출_엉뚱.pclm");
    raw(&odd)
        .execute_batch("CREATE TABLE notice (notice_base TEXT, seq TEXT);")
        .unwrap();

    let before = (
        std::fs::read(&submitted).unwrap(),
        std::fs::read(&odd).unwrap(),
    );

    let report = f.merge(&[&submitted, &odd], &f.out()).unwrap();

    assert_eq!(
        report.rejected,
        [rejection("제출_엉뚱.pclm", merger::NOT_PCLM)]
    );
    assert_eq!(report.submissions, 1);
    assert!(
        f.render(&report)
            .contains("제출_엉뚱.pclm — PCLM 파일이 아님")
    );

    assert_eq!(std::fs::read(&submitted).unwrap(), before.0);
    assert_eq!(std::fs::read(&odd).unwrap(), before.1);
    for file in [&submitted, &odd] {
        assert!(!suffixed(file, "-wal").exists());
        assert!(!suffixed(file, "-shm").exists());
    }

    // 취합본은 자기 역할과 자기 신원을 갖는다.
    let info = pclm_file::inspect(&f.out());
    assert_eq!(
        (info.kind, info.role.as_deref()),
        (PclmKind::Ok, Some(PclmRole::MERGED))
    );
    assert_ne!(pclm_file::inspect(&submitted).dataset_id, info.dataset_id);
}

/// 보고 **글**이 알림의 본체다. 밀린 쪽이 누구였는지 여기 서지 않으면 볼 데가 없다.
#[test]
fn 보고_글에_겹친_것이_한_줄씩_선다() {
    let f = Fixture::new();
    let hong = f.submission(
        "홍길동",
        &[&notice(
            "N-1",
            "먼저 읽은 공고문",
            "2026-08-22T09:00:00.0000000Z",
        )],
    );
    let kim = f.submission(
        "김철수",
        &[&notice(
            "N-1",
            "다르게 읽은 공고문",
            "2026-08-28T09:00:00.0000000Z",
        )],
    );

    let text = f.render(&f.merge(&[&hong, &kim], &f.out()).unwrap());

    assert!(text.contains("겹친 것 1건"));
    assert!(text.contains("김철수"));
    assert!(text.contains("홍길동"));
    assert!(text.contains("◀ 실림"));
    assert!(text.contains(&f.out().display().to_string()));
}

/// 보고 글의 꼴 — 줄과 칸 맞춤, 월/일(서울 시각), 할 일을 푼 줄. C# 의 `MergeReportText.Render` 와 바이트까지 같은지는
/// 대조 계기(`merge-*.txt`)가 본다.
#[test]
fn 보고_글의_꼴() {
    let f = Fixture::new();
    let hong = f.submission(
        "홍길동",
        &[&notice(
            "N-1",
            "먼저 읽은 공고문",
            "2026-08-22T15:30:00.0000000Z",
        )],
    );
    let kim = f.submission(
        "김철수",
        &[&notice(
            "N-1",
            "다르게 읽은 공고문",
            "2026-08-28T09:00:00.0000000Z",
        )],
    );
    let newer = f.submission("앞서간이", &[]);
    Database::new(&newer, Access::Write)
        .open()
        .unwrap()
        .execute_batch(&format!("PRAGMA user_version = {};", schema::VERSION + 1))
        .unwrap();

    let text = f.render(&f.merge(&[&hong, &kim, &newer], &f.out()).unwrap());

    let nl = merge_report_text::NEWLINE;
    let expected = [
        "취합 결과",
        "  제출본 2개 · 조달요구 0 · 접수 0 · 공고 1 · 계약 0",
        "",
        "  받지 않은 파일 1개",
        "    제출_앞서간이.pclm — 새 판",
        "    새 판으로 지은 자료는 내릴 길이 없습니다. 이 프로그램을 새로 받아 다시 합치세요.",
        "",
        "  겹친 것 1건 — 늦은 것을 실었습니다",
        &format!("    {:<16} 김철수(08/28) ◀ 실림   홍길동(08/23)", "N-1·0"),
        "",
        &format!("  파일  {}", f.out().display()),
    ]
    .iter()
    .map(|l| format!("{l}{nl}"))
    .collect::<String>();
    assert_eq!(text, expected);
}

// ── 제출 ────────────────────────────────────────────────────────

/// 제출본은 `VACUUM INTO` 로 뜬 **한 파일**이다 — `-wal`·`-shm` 을 딸려 보내지 않는다.
#[test]
fn 제출본을_다시_열면_같은_것이_들어_있다() {
    let f = Fixture::new();
    let database = pclm_file::create(&f.at("원본.db"), PclmRole::WORK, &f.env).unwrap();
    database
        .open()
        .unwrap()
        .execute_batch(&format!(
            "{}\n{}",
            contract("C-1", "지어낸 계약", "2026-08-20T09:00:00.0000000Z"),
            contract_item("C-1", 1, "지어낸 품목")
        ))
        .unwrap();

    let saved = database
        .snapshot(&f.at("제출_홍길동_20260729.pclm"), false)
        .unwrap();

    assert!(saved.exists());
    assert!(!suffixed(&saved, "-wal").exists());
    assert!(!suffixed(&saved, "-shm").exists());
    assert_eq!(
        read(&saved, "SELECT contract_base, title FROM contract;"),
        ["C-1|지어낸 계약"]
    );
    assert_eq!(
        read(&saved, "SELECT item_name FROM contract_item;"),
        ["지어낸 품목"]
    );
    assert_eq!(
        read(&saved, "PRAGMA user_version;")[0],
        schema::VERSION.to_string()
    );
}

/// 덮어쓰기를 부르지 않으면 이미 있는 파일을 지우지 않는다 — 자리 옮기기의 방벽이다.
#[test]
fn 덮어쓰기를_부르지_않으면_있는_파일을_지키다() {
    let f = Fixture::new();
    let database = pclm_file::create(&f.at("원본.db"), PclmRole::WORK, &f.env).unwrap();
    let existing = f.at("이미있음.pclm");
    std::fs::write(&existing, "남의 것").unwrap();

    assert!(database.snapshot(&existing, false).is_err());
    assert_eq!(std::fs::read_to_string(&existing).unwrap(), "남의 것");

    // 사람이 자리를 손수 짚어 뜨는 제출본만 덮는다.
    database.snapshot(&existing, true).unwrap();
    assert_ne!(std::fs::read(&existing).unwrap(), "남의 것".as_bytes());
}

/// C# `PclmFileTests.취합본은_작업자료_자리에_짓지_않는다` — 결과물로 작업자료를 덮지 않는다(ADR-031). 거절은 제출본을 읽기
/// 전에 오고, 작업자료는 한 바이트도 바뀌지 않는다.
#[test]
fn 취합본은_작업자료_자리에_짓지_않는다() {
    let f = Fixture::new();
    let work = f.work("계약자료.pclm");
    work.open()
        .unwrap()
        .execute_batch(&contract(
            "C-1",
            "지어낸 계약",
            "2026-09-01T00:00:00.0000000Z",
        ))
        .unwrap();
    let submission = pclm_file::snapshot(
        &work,
        &f.at("제출_홍길동.pclm"),
        PclmRole::SUBMISSION,
        false,
        &f.env,
    )
    .unwrap();
    let before = std::fs::read(work.path()).unwrap();

    let error = f.merge(&[&submission], work.path()).unwrap_err();
    assert!(matches!(error, Error::InvalidOperation(_)), "{error}");
    assert_eq!(before, std::fs::read(work.path()).unwrap());
}

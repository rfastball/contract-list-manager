//! 명령줄의 동작 — 못박은 시계와 격리한 홈(`--home`)으로 [`pclm_cli::run`] 을 부르고, 표준 출력·오류의 바이트와 종료 코드를
//! 본다. C# 과의 바이트 대조는 `scripts/parity.ps1` 의 `cli-*` 사례가 하고, 여기서는 갈래마다의 약속을 붙든다. 값은 모두
//! 지어낸 것이다.

use std::path::{Path, PathBuf};

use pclm_cli::{CRASH_EXIT, Context, run};
use pclm_core::Env;
use pclm_core::local_time::FixedZone;
use pclm_core::storage::pclm_file;
use pclm_home::Home;

/// 2026-09-10 09:00 (KST) — 「오늘」 은 20260910 이다.
struct Pinned;

impl Env for Pinned {
    fn now_utc(&self) -> String {
        "2026-09-10T00:00:00.0000000Z".into()
    }
    fn new_id(&self) -> String {
        pclm_home::win::new_guid_n()
    }
}

const KST: FixedZone = FixedZone(9 * 3600);

struct Temp(PathBuf);

impl Temp {
    fn new() -> Temp {
        let path = std::env::temp_dir().join(format!("pclm-cli-{}", pclm_home::win::new_guid_n()));
        std::fs::create_dir_all(&path).unwrap();
        Temp(path)
    }
    fn join(&self, part: &str) -> PathBuf {
        self.0.join(part)
    }
    fn home(&self) -> String {
        self.join("home").display().to_string()
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Ran {
    out: String,
    err: String,
    code: i32,
}

fn never_default() -> std::io::Result<Home> {
    panic!("시험은 업무 홈을 열지 않는다 — --home 을 준다")
}

fn pclm(args: &[&str]) -> Ran {
    let context = Context {
        env: &Pinned,
        zone: &KST,
        default_home: &never_default,
    };
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = run(&args, &context, &mut out, &mut err);
    Ran {
        out: String::from_utf8(out).unwrap(),
        err: String::from_utf8(err).unwrap(),
        code,
    }
}

/// 쪽지가 가리키는 작업자료 하나 — 계약 하나(차수 둘)와 공고 하나.
fn bound(temp: &Temp) -> PathBuf {
    let workfile = temp.join("data").join("work.pclm");
    std::fs::create_dir_all(workfile.parent().unwrap()).unwrap();
    pclm_file::create(&workfile, "work", &Pinned).unwrap();
    let c = rusqlite::Connection::open(&workfile).unwrap();
    c.execute_batch(
        "INSERT INTO contract_series (contract_base) VALUES ('TESTC0001');
         INSERT INTO contract (contract_base, seq, title, updated_at) VALUES ('TESTC0001', '00', '지어낸 책상 계약', '2026-09-01T00:00:00.0000000Z');
         INSERT INTO contract (contract_base, seq, title, updated_at) VALUES ('TESTC0001', '01', '지어낸 책상 계약 (변경)', '2026-09-02T00:00:00.0000000Z');",
    )
    .unwrap();
    drop(c);
    let id = pclm_file::inspect(&workfile).dataset_id.unwrap();
    Home::new(temp.home())
        .unwrap()
        .write_config(&workfile, &id, None)
        .unwrap();
    workfile
}

fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map(|r| {
            r.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

#[test]
fn 인자_거절은_홈을_열기_전에_2_로_끝난다() {
    let ran = pclm(&["status", "--db", "시험자료"]);
    assert_eq!(ran.code, 2);
    assert_eq!(ran.out, "");
    assert_eq!(
        ran.err,
        "--db 는 없어졌습니다. 시험 자료는 --home <폴더> 로 따로 여세요.\r\n"
    );
    let ran = pclm(&["fix", "키", "열", "--긴급"]);
    assert_eq!(ran.code, 2);
    assert!(
        ran.err.starts_with("모르는 옵션입니다: --긴급."),
        "{}",
        ran.err
    );
}

#[test]
fn file_은_읽는_명령에만_받고_거절_글에는_소문자_명령이_든다() {
    let temp = Temp::new();
    let ran = pclm(&["PLAN", "--file", "x", "--home", &temp.home()]);
    assert_eq!(ran.code, 2);
    assert_eq!(
        ran.err,
        "--file 은 열람 전용입니다 — status · 현황 · export 에만 씁니다. plan 는 작업자료에서 하세요.\r\n"
    );
    // 거절은 홈에 아무것도 짓지 않는다.
    assert!(!temp.join("home").exists());
}

#[test]
fn 쪽지가_없는_파일을_가리키면_까닭만_말하고_아무것도_짓지_않는다() {
    let temp = Temp::new();
    let missing = temp.join("지워진 자리").join("계약자료.pclm");
    Home::new(temp.home())
        .unwrap()
        .write_config(&missing, "ab", None)
        .unwrap();
    let ran = pclm(&["status", "--home", &temp.home()]);
    assert_eq!(ran.code, 2);
    assert_eq!(ran.out, "");
    // 글에 경로가 들어 있으면 경로 줄을 더 쓰지 않는다.
    assert_eq!(
        ran.err,
        format!(
            "작업자료 파일이 없습니다. 지워졌거나 옮겨졌거나 이름이 바뀌었을 수 있습니다: {}\r\n",
            missing.display()
        )
    );
    assert_eq!(names(&temp.join("home")), ["config.json"]);
}

#[test]
fn 깨진_쪽지는_글_뒤에_쪽지_자리를_한_줄_더_쓴다() {
    let temp = Temp::new();
    std::fs::create_dir_all(temp.join("home")).unwrap();
    std::fs::write(temp.join("home").join("config.json"), "{\"version\":2}").unwrap();
    let ran = pclm(&["현황", "--home", &temp.home()]);
    assert_eq!(ran.code, 2);
    let lines: Vec<&str> = ran.err.split("\r\n").collect();
    assert_eq!(lines.len(), 3, "{}", ran.err);
    assert!(lines[0].starts_with("작업자료를 가리키는 설정 파일을 읽을 수 없습니다("));
    assert_eq!(
        lines[1],
        temp.join("home").join("config.json").display().to_string()
    );
    assert_eq!(lines[2], "");
}

#[test]
fn 처음_실행은_작업자료를_짓고_줄_끝은_cs_처럼_섞인다() {
    let temp = Temp::new();
    let ran = pclm(&["status", "--home", &temp.home()]);
    assert_eq!(ran.code, 0, "{}", ran.err);
    let workfile = temp.join("home").join("계약자료.pclm");
    // "DB  …\n" 을 WriteLine 하면 LF 뒤에 CR LF 가 붙는다.
    assert!(
        ran.out.starts_with(&format!(
            "DB  {}\n\r\n  접수      0행 /    0건\r\n",
            workfile.display()
        )),
        "{}",
        ran.out
    );
    assert!(ran.out.ends_with("  미연결 접수 0건 · 미연결 계약 0건\r\n"));
    assert_eq!(names(&temp.join("home")), ["config.json", "계약자료.pclm"]);
}

#[test]
fn 모르는_명령은_도움말이다() {
    let temp = Temp::new();
    bound(&temp);
    let ran = pclm(&["없는명령", "--home", &temp.home()]);
    assert_eq!(ran.code, 0);
    assert!(ran.out.starts_with("pclm plan [엑셀]  "));
    assert!(
        ran.out
            .ends_with("쌓이는 자리는 pclm status 가 첫 줄에 적는다.\r\n")
    );
    // 도움말 안의 줄은 LF 다(C# 원시 문자열 — 소스의 줄 끝).
    assert_eq!(ran.out.matches("\r\n").count(), 1);
    assert_eq!(ran.out.matches('\n').count(), 20);
}

#[test]
fn 차수가_쌓인_건을_적는다() {
    let temp = Temp::new();
    bound(&temp);
    let ran = pclm(&["STATUS", "--home", &temp.home()]);
    // 읽기 전용 연결이 마지막에 닫혀도 작업자료 곁에 -wal · -shm 이 남지 않는다(C# 과 같은 자리).
    assert_eq!(names(&temp.join("data")), ["work.pclm"]);
    assert_eq!(ran.code, 0, "{}", ran.err);
    assert!(
        ran.out.ends_with(
            "\r\n\n  차수가 쌓인 건\r\n    계약 TESTC0001 · 2차 (최신 01) 지어낸 책상 계약 (변경)\r\n"
        ),
        "{}",
        ran.out
    );
}

#[test]
fn 사람_값은_넣고_지우며_모르는_열은_쓸_수_있는_열과_함께_거절한다() {
    let temp = Temp::new();
    bound(&temp);
    let home = temp.home();
    let ran = pclm(&["set", "--home", &home]);
    assert!(
        ran.out
            .starts_with("사람이 채우는 열\n\r\n  [계약]\r\n    진행상태         text    \r\n"),
        "{}",
        ran.out
    );
    let ran = pclm(&["set", "TESTC000100", "없는열", "--home", &home]);
    assert_eq!(ran.code, 2);
    assert_eq!(
        ran.err,
        "모르는 열입니다: 없는열\r\n쓸 수 있는 열: 진행상태, 메모\r\n"
    );
    let ran = pclm(&[
        "set",
        "TESTC000100",
        "메모",
        "지어낸",
        "메모",
        "--home",
        &home,
    ]);
    assert_eq!(ran.out, "넣음  TESTC000100.메모 = 지어낸 메모\r\n");
    let ran = pclm(&["set", "TESTC000100", "메모", "--home", &home]);
    assert_eq!(ran.out, "지움  TESTC000100.메모\r\n");
    let ran = pclm(&["set", "없는키", "메모", "--home", &home]);
    assert_eq!(ran.err, "그런 공고·계약이 없습니다: 없는키\r\n");
    assert_eq!(
        pclm(&["set", "TESTC000100", "--home", &home]).err,
        "pclm set <키> <열> [값]\r\n"
    );
}

#[test]
fn 고치기는_빈_값을_보이고_값이_없으면_되돌린다() {
    let temp = Temp::new();
    bound(&temp);
    let home = temp.home();
    let ran = pclm(&["fix", "TESTC000101", "계약건명", "", "--home", &home]);
    assert_eq!(ran.code, 0, "{}", ran.err);
    assert_eq!(
        ran.out,
        "고침  TESTC000101.계약건명\r\n  지어낸 책상 계약 (변경)  →  (빔)\r\n"
    );
    let ran = pclm(&["fix", "TESTC000101", "계약건명", "--home", &home]);
    assert_eq!(
        ran.out,
        "되돌림  TESTC000101.계약건명\r\n  (빔)  →  지어낸 책상 계약 (변경)\r\n"
    );
    // 면 뷰는 계열마다 최신 차수만 낸다 — 옛 차수의 칸은 빈 값으로 보인다.
    let ran = pclm(&["fix", "TESTC000100", "계약건명", "옛 것", "--home", &home]);
    assert_eq!(
        ran.out,
        "고침  TESTC000100.계약건명\r\n  (빔)  →  옛 것\r\n"
    );
    let ran = pclm(&["fix", "TESTC000100", "메모", "x", "--home", &home]);
    assert_eq!(ran.code, 2);
    assert!(
        ran.err
            .ends_with("\r\n쓸 수 있는 열: v_계약 의 열 (키 열과 사람 열은 뺀다)\r\n"),
        "{}",
        ran.err
    );
}

#[test]
fn 지우기는_예_없이는_세어_보이기만_한다() {
    let temp = Temp::new();
    let workfile = bound(&temp);
    let home = temp.home();
    let ran = pclm(&["delete", "TESTC000101", "--home", &home]);
    assert_eq!(ran.code, 0, "{}", ran.err);
    assert_eq!(
        ran.out,
        "계약  TESTC000101  지어낸 책상 계약 (변경)\n\r\n  쌓인 차수   00 · 01\r\n  지울 것     01 차수 하나\r\n\
         \x20 딸린 줄     0\r\n  손으로 고친 칸 0\r\n\n지우려면 --예 를 붙이세요. 되돌릴 수 없습니다.\r\n"
    );
    let rows = |w: &Path| -> i64 {
        rusqlite::Connection::open(w)
            .unwrap()
            .query_row("SELECT COUNT(*) FROM contract;", [], |r| r.get(0))
            .unwrap()
    };
    assert_eq!(rows(&workfile), 2);
    let ran = pclm(&["delete", "TESTC000101", "--예", "--home", &home]);
    assert!(
        ran.out.ends_with("\r\n\n지움  TESTC000101\r\n"),
        "{}",
        ran.out
    );
    assert_eq!(rows(&workfile), 1);
    assert_eq!(
        pclm(&["delete", "--home", &home]).err,
        "pclm delete <키> [--계열] [--예]\r\n"
    );
}

#[test]
fn 제출본_이름은_못박은_오늘과_정해_둔_이름으로_짓는다() {
    let temp = Temp::new();
    let workfile = bound(&temp);
    let home = temp.home();
    let out = temp.join("out");
    std::fs::create_dir_all(&out).unwrap();
    let target = out.display().to_string();

    let ran = pclm(&["submit", &target, "--home", &home]);
    assert_eq!(ran.code, 0, "{}", ran.err);
    let saved = out.join("제출_20260910.pclm");
    assert!(
        ran.out
            .starts_with(&format!("제출본  {}\r\n        ", saved.display()))
    );
    assert!(
        ran.out
            .contains(&format!("MB · DB {}\r\n", workfile.display()))
    );
    assert!(
        ran.out
            .contains("\n  ※ 이름이 비어 있어 파일 이름에 넣지 못했습니다.\n")
    );
    assert_eq!(
        pclm_file::inspect(&saved).role.as_deref(),
        Some(pclm_file::PclmRole::SUBMISSION)
    );

    let ran = pclm(&["submit", "--이름", "홍길동", &target, "--home", &home]);
    assert!(!ran.out.contains("※"), "{}", ran.out);
    assert!(out.join("제출_홍길동_20260910.pclm").is_file());
    // 작업자료 자리는 덮지 않는다 — C# 은 잡지 않는 예외로 끝난다.
    let ran = pclm(&["submit", &workfile.display().to_string(), "--home", &home]);
    assert_eq!(ran.code, CRASH_EXIT);
    assert!(
        ran.err
            .starts_with("Unhandled exception. 그 자리에는 작업자료이(가) 있어 덮지 않습니다")
    );
}

#[test]
fn 합칠_것이_없으면_취합본을_짓지_않고_2_로_끝난다() {
    let temp = Temp::new();
    bound(&temp);
    let home = temp.home();
    let ran = pclm(&["merge", "--home", &home]);
    assert_eq!(ran.code, 2);
    assert_eq!(
        ran.err,
        "합칠 제출본을 찾지 못했습니다.\r\n  pclm merge <폴더|파일…> [--out <경로>]\r\n\n제출본은 pclm submit 이 뜬 .pclm 파일입니다.\r\n"
    );
    let missing = temp.join("없음").display().to_string();
    let out = temp.join("합.pclm").display().to_string();
    let ran = pclm(&["merge", &missing, "--out", &out, "--home", &home]);
    assert_eq!(ran.code, 2);
    assert_eq!(
        ran.err,
        format!(
            "합칠 제출본을 찾지 못했습니다.\r\n  {missing} — 그런 폴더도 파일도 없습니다\r\n\n제출본은 pclm submit 이 뜬 .pclm 파일입니다.\r\n"
        )
    );
    assert!(!temp.join("합.pclm").exists());
}

#[test]
fn 열람은_사본을_읽고_다_읽으면_걷는다() {
    let temp = Temp::new();
    let workfile = bound(&temp);
    let other = temp.join("data").join("제출.pclm");
    pclm_file::snapshot(
        &pclm_core::Database::new(&workfile, pclm_core::Access::Read),
        &other,
        pclm_file::PclmRole::SUBMISSION,
        false,
        &Pinned,
    )
    .unwrap();
    let ran = pclm(&[
        "현황",
        "--file",
        &other.display().to_string(),
        "--home",
        &temp.home(),
    ]);
    assert_eq!(ran.code, 0, "{}", ran.err);
    assert!(
        ran.out.starts_with(&format!(
            "열람  {} (제출본, v{}) — 읽기 전용\n\r\nDB  {}\r\n\r\n",
            other.display(),
            pclm_core::storage::schema::VERSION,
            other.display()
        )),
        "{}",
        ran.out
    );
    // 사본은 홈의 view 에 섰다가 걷혔다.
    assert!(names(&temp.join("home").join("view")).is_empty());

    let ran = pclm(&[
        "status",
        "--file",
        &temp.join("없음.pclm").display().to_string(),
        "--home",
        &temp.home(),
    ]);
    assert_eq!(ran.code, 2);
    assert!(ran.err.starts_with("파일이 없습니다: "), "{}", ran.err);
}

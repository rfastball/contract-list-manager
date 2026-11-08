//! 동등성 계기의 Rust 쪽(#9). 명령과 덤프 꼴은 `crates/pclm-parity/README.md` 가 정본이다.
//!
//! `info`·`dump` 와, `pclm-core` 에 선 일(`create`·`migrate`·`refresh`·`snapshot`·`run`·`seed`·`submit`·`merge`)을 부른다. `synth` 는
//! "아직 없음" 으로 2 를 내고 멎는다 — C# 계기만 짓는 입력이다. `lock` 은 `pclm-home` 의 창 잠금을 잡아 본다(#10 — 확장 호스트
//! 견주기의 Rust 쪽은 계기가 아니라 `pclm-erp-host` 다). `config-write`·`resolve`·`switch` 는 `pclm-home` 의 쪽지 쓰기·
//! 시작·작업자료 바꾸기다(#8 ④). `bridge` 는 `pclm-bridge` 의 다리에 대본의 요청을 넘긴다(#8 ⑤ · ⑥). `xlsx-dump` 는 엑셀 한 권을
//! 덤프한다(#8 ⑥ — `pclm-excel` 을 쓰지 않는다).

mod bridge;
mod dump;
mod home;
mod script;
mod seed;
mod system_env;
mod xlsx_dump;

use std::io::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;

use rusqlite::Connection;

const USAGE: &str = "쓰는 법: pclm-parity <명령>
  info
  dump <file> [--out <path>] [--mask|--mask-hex <object>.<column>]... [--skip-views]
  create <file> <role>
  migrate <file>
  refresh <file>
  snapshot <src> <dst>
  synth <file> <version>        (아직 없음)
  seed <file> [--erp <dir>]
  run <file> <script.json>      (결과는 <file>.out.json)
  submit <src> <dst>            (pclm_file::snapshot, 역할 submission)
  merge <out> <src|folder>...   (merger::merge, 보고 글은 <out>.txt)
  lock <home> [--hold]          (창 잠금을 잡아 본다 — free/busy/held, C# 과 같은 커널 객체인지 드라이버가 견준다)
  config-write <home> <workfile> <datasetId> [retired]
  resolve <home> [--legacy <config>]
  switch <move|use|from-snapshot|new|backup|check> <home> <current> <args>...
  bridge <home> <file> <script.json> [--view] [--files <dir>]   (다리 대본 — 답은 <file>.bridge.txt, 고르기는 <file>.prompts.txt)
  xlsx-dump <file.xlsx> <out.txt>   (엑셀 한 권의 덤프 — 시트 · 고정 · 거르개 · 열 · 칸)";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("info") => info(),
        Some("dump") => dump_command(&args[1..]),
        Some("create") => create(&args[1..]),
        Some("migrate") => migrate(&args[1..]),
        Some("refresh") => refresh(&args[1..]),
        Some("snapshot") => snapshot(&args[1..]),
        Some("run") => match &args[1..] {
            [file, script] => script::run(file, script),
            _ => Err("run <file> <script.json>".into()),
        },
        Some("submit") => submit(&args[1..]),
        Some("merge") if args.len() >= 3 => match merge(&args[1], &args[2..]) {
            Ok(code) => return code,
            Err(message) => Err(message),
        },
        Some("seed") => match &args[1..] {
            [file] => seed::run(file, None),
            [file, flag, dir] if flag == "--erp" => seed::run(file, Some(dir)),
            _ => Err("seed <file> [--erp <dir>]".into()),
        },
        Some("lock") => match &args[1..] {
            [home] => lock(home, false),
            [home, flag] if flag == "--hold" => lock(home, true),
            _ => Err("lock <home> [--hold]".into()),
        },
        Some("config-write") => home::config_write(&args[1..]),
        Some("resolve") => home::resolve(&args[1..]),
        Some("switch") => home::switch(&args[1..]),
        Some("bridge") => bridge::run(&args[1..]),
        Some("xlsx-dump") => xlsx_dump::run(&args[1..]),
        Some(command @ "synth") => {
            eprintln!("pclm-parity: {command} — 아직 없음");
            return ExitCode::from(2);
        }
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(64);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("pclm-parity: {message}");
            ExitCode::FAILURE
        }
    }
}

/// `Home.TryLock` 을 잡아 본다. `hold` 면 잡은 채 stdin 이 닫힐 때까지 머문다.
fn lock(directory: &str, hold: bool) -> Result<(), String> {
    let home = pclm_home::Home::new(directory).map_err(|e| e.to_string())?;
    let held = home
        .try_lock(std::time::Duration::ZERO)
        .map_err(|e| e.to_string())?;
    let word = match (&held, hold) {
        (None, _) => "busy",
        (Some(_), true) => "held",
        (Some(_), false) => "free",
    };
    println!("{word}");
    std::io::stdout().flush().map_err(|e| e.to_string())?;
    if hold && held.is_some() {
        std::io::copy(&mut std::io::stdin(), &mut std::io::sink()).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// `PclmFile.Create` 와 같은 일. 폴더는 Core 가 짓지 않으므로(#8) 계기가 짓는다 — C# 은 Create 가 지었다.
fn create(args: &[String]) -> Result<(), String> {
    let [file, role] = args else {
        return Err("create <file> <role>".into());
    };
    let path = std::path::Path::new(file);
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    pclm_core::storage::pclm_file::create(path, role, &system_env::SystemEnv)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// `new Database(file).Migrate()` 와 같은 일.
fn migrate(args: &[String]) -> Result<(), String> {
    let [file] = args else {
        return Err("migrate <file>".into());
    };
    pclm_core::Database::new(file, pclm_core::Access::Write)
        .migrate(&system_env::SystemEnv)
        .map_err(|e| e.to_string())
}

/// `new Database(file).RefreshViews()` 와 같은 일 — 뷰만 지웠다 다시 짓는다.
fn refresh(args: &[String]) -> Result<(), String> {
    let [file] = args else {
        return Err("refresh <file>".into());
    };
    pclm_core::Database::new(file, pclm_core::Access::Write)
        .refresh_views()
        .map_err(|e| e.to_string())
}

/// `new Database(src, Access.Read).Snapshot(dst)` 와 같은 일.
fn snapshot(args: &[String]) -> Result<(), String> {
    let [source, target] = args else {
        return Err("snapshot <src> <dst>".into());
    };
    pclm_core::Database::new(source, pclm_core::Access::Read)
        .snapshot(std::path::Path::new(target), false)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// `pclm submit` 의 뜨기 — 설정·이름 짓기는 빼고 `PclmFile.Snapshot(…, Submission, overwrite: true)` 만.
fn submit(args: &[String]) -> Result<(), String> {
    let [source, target] = args else {
        return Err("submit <src> <dst>".into());
    };
    pclm_core::storage::pclm_file::snapshot(
        &pclm_core::Database::new(source, pclm_core::Access::Read),
        std::path::Path::new(target),
        pclm_core::PclmRole::SUBMISSION,
        true,
        &system_env::SystemEnv,
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

/// `pclm merge` 의 취합 — C# 계기의 `merge` 와 같은 일. 폴더는 `find_submissions` 로 펴고, 그 밖의 인자는 없는
/// 파일이라도 그대로 넘긴다. 보고 글(또는 `InvalidOperation` 의 글과 줄바꿈 — 그때는 2 로 끝낸다)을 `<out>.txt` 에
/// 적는다. 작업 폴더는 C# 처럼 임시 폴더 아래 `pclm-merge-<8자>` 를 짓고 끝나면 지운다 — Core 는 폴더를 짓지 않는다.
fn merge(output: &str, sources: &[String]) -> Result<ExitCode, String> {
    use pclm_core::Env as _;
    use pclm_core::storage::{merge_report_text, merger};

    let mut files: Vec<PathBuf> = Vec::new();
    for source in sources {
        let path = PathBuf::from(source);
        if path.is_dir() {
            files.extend(merger::find_submissions(&path));
        } else {
            files.push(path);
        }
    }

    let env = system_env::SystemEnv;
    let work = std::env::temp_dir().join(format!("pclm-merge-{}", &env.new_id()[..8]));
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let out = std::path::Path::new(output);
    let result = merger::merge(&files, out, &work, &env);
    let _ = std::fs::remove_dir_all(&work);

    let report_path = format!("{output}.txt");
    match result {
        Ok(report) => {
            std::fs::write(&report_path, merge_report_text::render(&report, out, &env))
                .map_err(|e| e.to_string())?;
            Ok(ExitCode::SUCCESS)
        }
        Err(pclm_core::Error::InvalidOperation(message)) => {
            std::fs::write(
                &report_path,
                format!("{message}{}", merge_report_text::NEWLINE),
            )
            .map_err(|e| e.to_string())?;
            Ok(ExitCode::from(2))
        }
        Err(e) => Err(e.to_string()),
    }
}

/// 링크된 SQLite 의 판·컴파일 선택과, 아무 PRAGMA 없이 연 연결의 기본값. C# 쪽과 같은 줄을 낸다 — 둘이 갈리는 것이
/// 곧 Core 이식이 손으로 맞춰야 할 것이다(외래키 기본값 따위).
fn info() -> Result<(), String> {
    let connection = Connection::open_in_memory().map_err(|e| e.to_string())?;
    let mut out = String::new();
    let version: String = connection
        .query_row("SELECT sqlite_version();", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    out.push_str(&format!("sqlite_version {version}\n"));
    let mut statement = connection
        .prepare("PRAGMA compile_options;")
        .map_err(|e| e.to_string())?;
    let options = statement
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    for option in options {
        out.push_str(&format!(
            "compile_option {}\n",
            option.map_err(|e| e.to_string())?
        ));
    }
    for pragma in ["foreign_keys", "recursive_triggers"] {
        let value: i64 = connection
            .query_row(&format!("PRAGMA {pragma};"), [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        out.push_str(&format!("pragma {pragma} {value}\n"));
    }
    std::io::stdout()
        .write_all(out.as_bytes())
        .map_err(|e| e.to_string())
}

fn dump_command(args: &[String]) -> Result<(), String> {
    let mut file: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut masks = Vec::new();
    let mut skip_views = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--out" => {
                output = Some(PathBuf::from(
                    iter.next().ok_or("--out 뒤에 경로가 없습니다.")?,
                ))
            }
            flag @ ("--mask" | "--mask-hex") => {
                // 첫 마침표에서 가른다 — C# 쪽과 같다.
                let spec = iter
                    .next()
                    .ok_or(format!("{flag} 뒤에 <object>.<column> 이 없습니다."))?;
                match spec.split_once('.') {
                    Some((object, column)) if !object.is_empty() && !column.is_empty() => {
                        masks.push(dump::Mask {
                            object: object.to_owned(),
                            column: column.to_owned(),
                            hex: flag == "--mask-hex",
                        });
                    }
                    _ => return Err(format!("{flag} 는 <object>.<column> 꼴입니다: {spec}")),
                }
            }
            "--skip-views" => skip_views = true,
            other if file.is_none() && !other.starts_with("--") => {
                file = Some(PathBuf::from(other))
            }
            other => return Err(format!("모르는 인자입니다: {other}")),
        }
    }
    let file = file.ok_or("덤프할 파일이 없습니다.")?;
    if !file.is_file() {
        return Err(format!("파일이 없습니다: {}", file.display()));
    }

    let bytes = dump::run(&file, &masks, skip_views).map_err(|e| e.to_string())?;
    match output {
        None => std::io::stdout()
            .lock()
            .write_all(&bytes)
            .map_err(|e| e.to_string()),
        Some(path) => {
            if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::write(&path, bytes).map_err(|e| e.to_string())
        }
    }
}

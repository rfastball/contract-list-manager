//! 홈의 동등성(#8 ④) — C# `HomeParity.cs` 와 같은 명령, 같은 글. 꼴은 `crates/pclm-parity/README.md` 「홈」 이 정본이다.
//!
//! `pclm-home` 의 쪽지 쓰기·시작·작업자료 바꾸기를 그대로 부르고 결과를 글로 낸다. 오류는 `error` 줄 하나로 적고 0 으로
//! 끝낸다 — 거절의 글까지 견준다.

use std::io::Write as _;
use std::path::Path;

use pclm_core::{Access, Database, Error};
use pclm_home::{Home, HomeState};

use crate::dump::escape;

/// `config-write <home> <workfile> <datasetId> [retired]`.
pub fn config_write(args: &[String]) -> Result<(), String> {
    let (home, workfile, dataset_id, retired) = match args {
        [home, workfile, id] => (home, workfile, id, None),
        [home, workfile, id, retired] => (home, workfile, id, Some(retired)),
        _ => return Err("config-write <home> <workfile> <datasetId> [retired]".into()),
    };
    Home::new(home)
        .and_then(|home| {
            home.write_config(
                Path::new(workfile),
                dataset_id,
                retired.map(|r| Path::new(r.as_str())),
            )
        })
        .map_err(|e| e.to_string())
}

/// `resolve <home> [--legacy <config>]`.
pub fn resolve(args: &[String]) -> Result<(), String> {
    let home = match args {
        [home] => Home::new(home),
        [home, flag, legacy] if flag == "--legacy" => {
            Home::with_legacy_roaming_config(home, legacy)
        }
        _ => return Err("resolve <home> [--legacy <config>]".into()),
    }
    .map_err(|e| e.to_string())?;
    let mut lines = Vec::new();
    match home.resolve() {
        Ok(HomeState::Ready { database, notes }) => {
            lines.push("ready".to_string());
            lines.push(format!(
                "database\t{}",
                escape(&database.path().to_string_lossy())
            ));
            lines.extend(notes.iter().map(|n| format!("note\t{}", escape(n))));
        }
        Ok(HomeState::Problem {
            kind,
            path,
            message,
        }) => {
            lines.push(format!("problem\t{kind:?}"));
            lines.push(format!("path\t{}", escape(&path.to_string_lossy())));
            lines.push(format!("message\t{}", escape(&message)));
        }
        Err(e) => lines.push(error(&e)),
    }
    write(&lines)
}

/// `switch <op> <home> <current> <args>...`.
pub fn switch(args: &[String]) -> Result<(), String> {
    let [op, home, current, rest @ ..] = args else {
        return Err(
            "switch <move|use|from-snapshot|new|backup|check> <home> <current> <args>...".into(),
        );
    };
    let home = Home::new(home).map_err(|e| e.to_string())?;
    // C# 의 new Database(current) 는 경로를 온전히 편다.
    let current = Database::new(
        std::path::absolute(current).map_err(|e| e.to_string())?,
        Access::Write,
    );
    let p = |i: usize| Path::new(rest[i].as_str());
    let result = match (op.as_str(), rest.len()) {
        ("move", 1) => home.move_to(&current, p(0)),
        ("use", 1) => home.use_in_place(&current, p(0)),
        ("from-snapshot", 2) => home.create_from_snapshot(&current, p(0), p(1)),
        ("new", 1) => home.create_new_while_running(&current, p(0)),
        ("backup", 1) => pclm_home::backup_to(&current, p(0)),
        ("check", 1) => pclm_home::check_new_target(&current, p(0)),
        _ => return Err(format!("모르는 동작: {op}")),
    };
    let line = match result {
        Ok(path) => format!("ok\t{}", escape(&path.to_string_lossy())),
        Err(e) => error(&e),
    };
    write(&[line])
}

/// C# `HomeParity.Class` 와 같은 갈래 이름.
fn error(e: &Error) -> String {
    let class = match e {
        Error::PclmFile(_) => "pclm",
        Error::InvalidOperation(_) => "invalid",
        Error::Io(_) => "io",
        Error::Argument(_) => "argument",
        Error::Sqlite(_) => "sqlite",
    };
    format!("error\t{class}\t{}", escape(&e.to_string()))
}

fn write(lines: &[String]) -> Result<(), String> {
    let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
    let mut stdout = std::io::stdout().lock();
    stdout
        .write_all(text.as_bytes())
        .and_then(|()| stdout.flush())
        .map_err(|e| e.to_string())
}

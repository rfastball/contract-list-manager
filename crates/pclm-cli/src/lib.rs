//! 명령줄 `pclm`(C# `src/Pclm.Cli/Program.cs`) — 같은 인자에 **같은 바이트**의 표준 출력·표준 오류와 같은 종료 코드를 낸다.
//!
//! 창과 같은 홈의 쪽지를 본다(ADR-030). `--home` 은 개발·시험 격리의 문이고, `--file` 은 다른 자료를 읽기 전용 사본으로
//! 열어 본다(ADR-031 — `status`·`현황`·`export` 에만). 쪽지가 깨졌거나 가리키는 파일이 없으면 까닭을 말하고 2 로 그친다 —
//! 빈 자료를 세우거나 다른 자료를 대신 열지 않는다. 명령줄은 홈 잠금을 잡지 않는다.
//!
//! **글의 꼴.** C# 은 `Console.WriteLine` 으로 쓴다 — 줄 끝은 `Environment.NewLine`(Windows 는 CR LF)이고, 글 안의 `\n` 은
//! 그대로 LF 다. 그래서 한 줄 안에 LF 와 CR LF 가 섞인다. 여기도 [`NEWLINE`] 과 글 안의 `\n` 을 그대로 둔다.
//!
//! **시계.** 「오늘」(내보내기·제출본·취합본의 기본 파일 이름) 은 [`Context`] 의 [`Env::now_utc`] 를 [`LocalZone`] 으로
//! 읽는다 — C# 은 `DateTime.Now` 다. 시험은 고정한 시계를 넣는다.
//!
//! **C# 이 잡지 않는 예외.** C# 명령줄이 잡지 않는 예외(내보낼 자리가 잠겼다 따위)는 .NET 이 「Unhandled exception.」 과
//! 스택을 표준 오류에 적고 `0xE0434352` 로 끝낸다. 여기서는 그 첫 줄의 꼴로 글만 적고 같은 종료 코드로 끝낸다([`CRASH_EXIT`])
//! — 스택은 옮기지 않는다.

pub mod arguments;
pub mod text;

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use pclm_core::local_time::{LocalDateTime, LocalZone, parse_to_local};
use pclm_core::storage::merge_report_text::{self, NEWLINE};
use pclm_core::storage::merger;
use pclm_core::storage::pclm_file::{self, PclmRole, ViewedFile};
use pclm_core::storage::reports::Reports;
use pclm_core::storage::settings::SettingsStore;
use pclm_core::storage::status::Status;
use pclm_core::storage::store::{self, Store};
use pclm_core::storage::views::{self, UserColumnDefinition};
use pclm_core::{Access, Database, Env, Error};
use pclm_home::{Home, HomeState, dotnet};

use crate::arguments::CliArguments;
use crate::text::{cmp_ignore_case, format_0_hash, pad_left, pad_right, to_lower_invariant};

/// .NET 의 잡지 않은 예외의 종료 코드(`0xE0434352`, CLR 예외).
pub const CRASH_EXIT: i32 = 0xE043_4352_u32 as i32;

/// `--file` 을 받는 명령 — 읽기만 하는 것.
pub const VIEW_COMMANDS: &[&str] = &["status", "현황", "export"];

/// 명령줄이 밖에서 받는 것 — 시계와 시간대, 그리고 `--home` 이 없을 때의 홈.
pub struct Context<'a> {
    pub env: &'a dyn Env,
    pub zone: &'a dyn LocalZone,
    /// C# `Home.Default`.
    pub default_home: &'a dyn Fn() -> io::Result<Home>,
}

/// 명령 하나가 멈추는 길. `Exit` 은 C# 의 `Environment.Exit(code)`, `Crash` 는 C# 이 잡지 않는 예외다.
enum Stop {
    Exit(i32),
    Crash(String),
}

type Step = Result<(), Stop>;

fn crash(error: impl std::fmt::Display) -> Stop {
    Stop::Crash(error.to_string())
}

/// 출력 둘. 쓰기 실패는 버린다 — 닫힌 파이프에서 C# 은 조용히 넘어간다.
struct Console<'w> {
    out: &'w mut dyn Write,
    err: &'w mut dyn Write,
}

impl Console<'_> {
    /// `Console.WriteLine(text)`.
    fn line(&mut self, text: &str) {
        let _ = write!(self.out, "{text}{NEWLINE}");
        let _ = self.out.flush();
    }

    /// `Console.Write(text)`.
    fn write(&mut self, text: &str) {
        let _ = self.out.write_all(text.as_bytes());
        let _ = self.out.flush();
    }

    /// `Console.Error.WriteLine(text)`.
    fn error(&mut self, text: &str) {
        let _ = write!(self.err, "{text}{NEWLINE}");
        let _ = self.err.flush();
    }
}

/// `pclm <args>` — 종료 코드를 낸다. `args[0]` 이 명령이다(프로그램 이름은 빼고 준다).
pub fn run(args: &[String], context: &Context, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let mut console = Console { out, err };
    match start(args, context, &mut console) {
        Ok(()) => 0,
        Err(Stop::Exit(code)) => code,
        Err(Stop::Crash(message)) => {
            console.error(&format!("Unhandled exception. {message}"));
            CRASH_EXIT
        }
    }
}

fn start(args: &[String], context: &Context, console: &mut Console) -> Step {
    let parsed = match CliArguments::parse(args) {
        Ok(parsed) => parsed,
        Err(message) => {
            console.error(&message);
            return Err(Stop::Exit(2));
        }
    };
    let command = args
        .first()
        .map(|a| to_lower_invariant(a))
        .unwrap_or_else(|| "help".into());
    // 창과 같은 홈의 쪽지를 본다. --home 은 개발·시험 격리의 문이다. --db 는 걷었다(인자 풀이가 거절한다).
    let home = match parsed.option("--home") {
        None => (context.default_home)(),
        Some(directory) => Home::new(directory),
    }
    .map_err(crash)?;

    // --file 은 다른 자료를 열어 본다. 읽는 명령에만 받는다: 쓰는 명령이 받으면 고친 것이 사본과 함께 사라진다.
    let file = parsed.option("--file");
    if file.is_some() && !VIEW_COMMANDS.contains(&command.as_str()) {
        console.error(&format!(
            "--file 은 열람 전용입니다 — {} 에만 씁니다. {command} 는 작업자료에서 하세요.",
            VIEW_COMMANDS.join(" · ")
        ));
        return Err(Stop::Exit(2));
    }

    let (database, viewed) = match file {
        Some(file) => {
            // 지난번에 걷지 못한 사본을 걷는다. 창과 같은 자리·같은 규칙이다.
            home.prune_views();
            let viewed = match open_view(&home, file, context.env) {
                Ok(viewed) => viewed,
                Err(Error::PclmFile(error)) => {
                    console.error(&error.message);
                    return Err(Stop::Exit(2));
                }
                Err(other) => return Err(crash(other)),
            };
            console.line(&format!(
                "열람  {} ({}, v{}) — 읽기 전용\n",
                viewed.source_path().display(),
                PclmRole::name(Some(viewed.role())),
                viewed.version()
            ));
            (viewed.database().clone(), Some(viewed))
        }
        // 쪽지가 깨졌거나 가리키는 파일이 없거나 작업자료가 아니면 까닭을 말하고 그친다.
        None => match home.resolve().map_err(crash)? {
            HomeState::Problem { path, message, .. } => {
                console.error(&message);
                let path = path.to_string_lossy();
                if !message.contains(path.as_ref()) {
                    console.error(&path);
                }
                return Err(Stop::Exit(2));
            }
            HomeState::Ready { database, notes } => {
                for note in notes {
                    console.line(&note);
                    console.line("");
                }
                (database, None)
            }
        },
    };

    let mut cli = Cli {
        parsed,
        database,
        reading: viewed.as_ref().map(|v| v.source_path().to_path_buf()),
        context,
        console,
    };
    let result = match command.as_str() {
        "plan" => cli.plan(),
        "status" => cli.status(),
        "현황" => cli.현황(),
        "export" => cli.export(),
        "set" => cli.set(),
        "fix" => cli.fix(),
        "delete" => cli.remove(),
        "submit" => cli.submit(),
        "merge" => cli.merge(),
        _ => {
            cli.help();
            Ok(())
        }
    };
    // C# 의 쓰기 손잡이는 연결 풀을 놓으며 쓰기 연결로 닫아 -wal · -shm 을 걷는다. 풀이 없는 Rust 에서는 명령이 마지막에
    // 닫은 것이 읽기 전용 연결(status · export …)이면 빈 곁파일이 작업자료 옆에 남는다 — 쓰기 연결 하나를 열어 마지막에 닫는다.
    if cli.database.access() == Access::Write {
        drop(cli.database.open());
    }
    match result {
        // 열어 본 사본은 다 읽었으면 지운다. 못 지운 것은 다음 열람이 걷는다.
        Ok(()) => drop(viewed),
        // C# 의 Environment.Exit 와 잡지 않은 예외는 사본을 걷지 않는다 — 다음 열람이 걷는다.
        Err(stop) => {
            std::mem::forget(viewed);
            return Err(stop);
        }
    }
    Ok(())
}

/// 창과 같은 자리·같은 규칙으로 연다([`Home::open_view`]).
fn open_view(home: &Home, file: &str, env: &dyn Env) -> pclm_core::Result<ViewedFile> {
    home.open_view(Path::new(file), env)
}

const HELP: &[&str] = &[
    "pclm plan [엑셀]                    연간 조달계획을 읽어 넣는다 (조달요구번호 기준 업서트)",
    "                                    인자가 없으면 지난번에 고른 엑셀을 다시 읽는다",
    "pclm status                         쌓인 상태를 본다",
    "pclm 현황                            계획을 분모로 본 진행 (지금 지표는 가상이다)",
    "pclm export [경로]                  계약면을 엑셀로 내보낸다 (그 시점의 사진)",
    "pclm submit [경로]                  내 자료 한 벌을 제출본 파일로 뜬다 (.pclm)",
    "  --이름 <이름>                     제출본 파일 이름에 쓸 내 이름을 정해 둔다",
    "pclm merge <폴더|파일…>              제출본을 모아 취합본 하나로 짓는다 (매번 처음부터)",
    "  --out <경로>                      취합본을 놓을 자리 (기본은 지금 폴더)",
    "pclm set [키 열 값]                 손으로 채우는 값 (인자 없으면 열 목록)",
    "pclm fix <키> <열> [값]             수집한 값을 고친다 (값 없으면 원래대로)",
    "pclm delete <키>                    레코드를 지운다 (무엇이 사라지는지 세어 보인다)",
    "  --계열                            그 본번호의 전 차수를 지운다",
    "  --예                              실제로 지운다 (없으면 세어 보이기만 한다)",
    "  --home <폴더>                     다른 홈의 작업자료를 연다 (기본은 창과 같은 홈)",
    "  --file <파일>                     다른 자료를 열어 본다 — status · 현황 · export 만, 읽기 전용",
    "",
    "<키> 는 계약번호·공고번호·접수번호다. 접수는 아무 요청번호로도 찾는다.",
    "-- 뒤의 값은 옵션으로 해석하지 않는다. 예: pclm fix <키> 공고명 -- --긴급",
    "쌓이는 자리는 pclm status 가 첫 줄에 적는다.",
];

struct Cli<'a, 'c, 'w> {
    parsed: CliArguments,
    database: Database,
    /// 열람 중이면 원본의 자리. 홈 안의 임시 사본은 사람이 찾아갈 자리가 아니다.
    reading: Option<PathBuf>,
    context: &'a Context<'a>,
    console: &'c mut Console<'w>,
}

impl<'a> Cli<'a, '_, '_> {
    fn positional(&self, index: usize) -> Option<&str> {
        self.parsed.positionals.get(index).map(String::as_str)
    }

    fn refuse(&mut self, message: &str) -> Step {
        self.console.error(message);
        Err(Stop::Exit(2))
    }

    /// C# `DateTime.Now`.
    fn today(&self) -> Result<LocalDateTime, Stop> {
        let now = self.context.env.now_utc();
        parse_to_local(Some(&now), self.context.zone)
            .ok_or_else(|| crash(format!("지금을 읽지 못했습니다: {now}")))
    }

    fn store(&self) -> Store<'a> {
        Store::new(self.database.clone(), self.context.env)
    }

    /// 읽고 있는 자료. 열람 중이면 원본이다.
    fn reading_place(&self) -> String {
        match &self.reading {
            Some(source) => source.display().to_string(),
            None => self.database.path().display().to_string(),
        }
    }

    /// 연간 조달계획 엑셀을 읽어 넣는다. 인자가 없으면 설정에 적어 둔 엑셀을 다시 읽고, 주면 넣은 뒤 그 자리를 적어 둔다.
    fn plan(&mut self) -> Step {
        let database = self.database.clone();
        let settings = SettingsStore::new(&database);
        let target = match self.positional(0) {
            Some(target) => target.to_string(),
            None => {
                let target = settings.read().map_err(crash)?.plan_path;
                if target.is_empty() {
                    return self.refuse("읽을 엑셀을 지정하거나 설정에서 골라 두세요.");
                }
                self.console.line(&format!("골라 둔 엑셀  {target}\n"));
                target
            }
        };

        if !Path::new(&target).is_file() {
            return self.refuse(&format!("그런 파일이 없습니다: {target}"));
        }

        let result =
            pclm_excel::import(&database, Path::new(&target), self.context.env).map_err(crash)?;

        if !result.ok() {
            for error in &result.errors {
                self.console.error(error);
            }
            // 무엇이 없는지와 무엇이 있었는지를 함께 적는다.
            if !result.missing_required.is_empty() {
                self.console
                    .error(&format!("없는 열: {}", result.missing_required.join(", ")));
            }
            if !result.found.is_empty() {
                self.console
                    .error(&format!("읽은 열: {}", result.found.join(", ")));
            }
            return self.refuse("\n한 줄도 넣지 않았습니다. 엑셀의 오류를 확인하세요.");
        }

        // 넣고 나서 적어 둔다 — 읽지도 못한 파일을 다음번의 기본으로 남기지 않는다.
        settings.save_plan_path(&target).map_err(crash)?;

        self.console
            .line(&format!("읽은 열: {}", result.found.join(", ")));
        self.console.line(&format!(
            "\n{}줄 · 신규 {} · 갱신 {} · 건너뜀 {} · DB {}",
            result.rows,
            result.created,
            result.updated,
            result.skipped,
            database.path().display()
        ));
        if result.skipped > 0 {
            self.console.line(&format!(
                "  ※ 건너뛴 {}줄은 조달요구번호가 비어 있습니다.",
                result.skipped
            ));
        }
        Ok(())
    }

    fn status(&mut self) -> Step {
        let reports = Reports::new(self.database.clone());
        let s = reports.summary().map_err(crash)?;
        let n = |v: i32| pad_left(&v.to_string(), 4);

        self.console
            .line(&format!("DB  {}\n", self.reading_place()));
        self.console.line(&format!(
            "  접수   {}행 / {}건",
            n(s.request_rows),
            n(s.request_bases)
        ));
        self.console.line(&format!(
            "  공고   {}행 / {}건",
            n(s.notice_rows),
            n(s.notice_bases)
        ));
        self.console.line(&format!(
            "  계약   {}행 / {}건",
            n(s.contract_rows),
            n(s.contract_bases)
        ));
        self.console
            .line(&format!("  상대자 {}", n(s.counterparties)));
        self.console.line(&format!(
            "  미연결 접수 {}건 · 미연결 계약 {}건",
            s.unlinked_requests, s.unlinked_contracts
        ));

        let revisions: Vec<_> = reports
            .revisions()
            .map_err(crash)?
            .into_iter()
            .filter(|r| r.revisions > 1)
            .collect();
        if !revisions.is_empty() {
            self.console.line("\n  차수가 쌓인 건");
            for r in revisions {
                self.console.line(&format!(
                    "    {} {} · {}차 (최신 {}) {}",
                    r.kind,
                    r.base,
                    r.revisions,
                    r.latest,
                    r.title.unwrap_or_default()
                ));
            }
        }
        Ok(())
    }

    /// 현황을 오는 대로 적는다 — 지표는 `status::metrics()` 하나에 있고, 여기는 그것을 알지 않는다.
    fn 현황(&mut self) -> Step {
        let report = Status::new(self.database.clone()).report().map_err(crash)?;

        self.console.line(&format!("DB  {}", self.reading_place()));
        self.console.line("");

        for group in &report.groups {
            self.console.line(&format!("  {}", group.name));
            for cell in &group.cells {
                // 거르개가 있으면 창의 계획 탭에서 눌러 볼 수 있는 줄이다. 알아보게만 적는다.
                let mark = cell
                    .filter
                    .as_ref()
                    .map(|f| format!("   ▸ {f}"))
                    .unwrap_or_default();
                self.console.line(&format!(
                    "    {} {}{mark}",
                    pad_right(&cell.name, 10),
                    pad_left(&cell.count.to_string(), 6)
                ));
            }
            self.console.line("");
        }
        Ok(())
    }

    fn export(&mut self) -> Step {
        // 자리를 짚지 않으면 지금 폴더에 떨어뜨린다.
        let target = match self.positional(0) {
            Some(target) => target.to_string(),
            None => pclm_excel::default_file_name(self.today()?),
        };
        let saved = pclm_excel::export(&self.database, Path::new(&target)).map_err(crash)?;
        self.console.line(&format!("내보냄  {}", saved.display()));
        let sheets: Vec<&str> = views::EXPORTED
            .iter()
            .map(|v| views::sheet_name(v))
            .collect();
        self.console
            .line(&format!("시트    {}", sheets.join(" · ")));
        Ok(())
    }

    /// 내 자료 한 벌을 제출본 파일로 뜬다. `--이름` 은 파일 이름에만 쓴다(ADR-023).
    fn submit(&mut self) -> Step {
        let database = self.database.clone();
        let settings = SettingsStore::new(&database);
        let mut current = settings.read().map_err(crash)?;

        // 이름을 주면 설정에 남긴다 — 다음부터 말이 없다.
        if let Some(given) = self.parsed.option("--이름") {
            current = settings.save(Some(given), None).map_err(crash)?;
        }

        let name = current.submitter_name;
        let stamp = pclm_excel::exporter::compact_date(self.today()?);
        let file_name = if name.is_empty() {
            format!("제출_{stamp}.pclm")
        } else {
            format!("제출_{name}_{stamp}.pclm")
        };

        let path = match self.positional(0) {
            None => file_name,
            Some(target) if Path::new(target).is_dir() => path_combine(target, &file_name),
            Some(target) => target.to_string(),
        };

        // 제출본은 자기 역할과 새 신원을 달고 나간다 — 창의 「제출본 만들기」 와 같은 길이다.
        let saved = snapshot_submission(&database, &path, self.context.env).map_err(crash)?;
        let length = std::fs::metadata(&saved).map_err(crash)?.len();

        self.console.line(&format!("제출본  {}", saved.display()));
        self.console.line(&format!(
            "        {}MB · DB {}",
            format_0_hash(length as f64 / 1024.0 / 1024.0),
            database.path().display()
        ));

        if name.is_empty() {
            self.console.line(
                "\n  ※ 이름이 비어 있어 파일 이름에 넣지 못했습니다.\n     \
                 pclm submit --이름 <이름> 으로 정합니다 — 제출본 파일 이름에만 쓰는 값입니다.",
            );
        }

        self.console.line(
            "\n  알맹이는 SQLite 이지만 확장자는 .pclm 입니다 — 메일 게이트웨이가 .db 를 막는 일이 있어서입니다.\n  \
             그것마저 막히면 압축해서 보내세요.",
        );
        Ok(())
    }

    /// 제출본을 모아 취합본 하나로 짓는다. 기계는 고르기만 하고 판정하지 않으므로 알림이 본체다.
    fn merge(&mut self) -> Step {
        let stamp = pclm_excel::exporter::compact_date(self.today()?);
        let default_name = format!("취합_{stamp}.pclm");
        let mut merged = self
            .parsed
            .option("--out")
            .map(String::from)
            .unwrap_or_else(|| default_name.clone());
        if Path::new(&merged).is_dir() {
            merged = path_combine(&merged, &default_name);
        }
        let merged = dotnet::full_path(&merged).map_err(crash)?;
        let merged_text = merged.to_string_lossy().into_owned();

        let mut found: Vec<PathBuf> = Vec::new();
        let mut missing: Vec<String> = Vec::new();
        for target in &self.parsed.positionals {
            let path = Path::new(target);
            if path.is_dir() {
                let picked = merger::find_submissions(path);
                if picked.is_empty() {
                    missing.push(format!("{target} — 이 폴더에 .pclm 이 없습니다"));
                }
                found.extend(picked);
            } else if path.is_file() {
                found.push(path.to_path_buf());
            } else {
                missing.push(format!("{target} — 그런 폴더도 파일도 없습니다"));
            }
        }

        // 지난 취합본이 그 폴더에 있어도 제출본으로 세지 않는다.
        let mut files: Vec<String> = Vec::new();
        for f in found {
            let full = dotnet::full_path(&f).map_err(crash)?;
            let full = full.to_string_lossy().into_owned();
            if dotnet::eq_ignore_case(&full, &merged_text)
                || files.iter().any(|seen| dotnet::eq_ignore_case(seen, &full))
            {
                continue;
            }
            files.push(full);
        }
        files.sort_by(|a, b| cmp_ignore_case(a, b));

        if files.is_empty() {
            // 조용히 빈 것을 만들지 않는다 — 받는 사람은 그것을 취합 결과로 안다.
            self.console.error("합칠 제출본을 찾지 못했습니다.");
            for m in &missing {
                self.console.error(&format!("  {m}"));
            }
            if self.parsed.positionals.is_empty() {
                self.console
                    .error("  pclm merge <폴더|파일…> [--out <경로>]");
            }
            return self.refuse("\n제출본은 pclm submit 이 뜬 .pclm 파일입니다.");
        }

        for m in &missing {
            self.console.line(&format!("※ {m}"));
        }

        // C# Merger 처럼 임시 폴더 아래 pclm-merge-<8자> 에서 짓고 지운다 — Core 는 폴더를 짓지 않는다.
        let env = self.context.env;
        let work = std::env::temp_dir().join(format!("pclm-merge-{}", &env.new_id()[..8]));
        std::fs::create_dir_all(&work).map_err(crash)?;
        let result = merger::merge(&files, &merged, &work, env);
        let _ = std::fs::remove_dir_all(&work);

        match result {
            // 글은 Core 가 짓는다 — 창이 취합본 옆에 남기는 것과 한 글자도 다르면 안 된다.
            Ok(report) => {
                self.console.write(&merge_report_text::render(
                    &report,
                    &merged,
                    self.context.zone,
                ));
                Ok(())
            }
            // 받아들인 제출본이 없으면 Merger 가 결과물을 쓰기 전에 멈춘다 — 까닭을 그대로 보인다.
            Err(Error::InvalidOperation(message)) => self.refuse(&message),
            Err(other) => Err(crash(other)),
        }
    }

    fn set(&mut self) -> Step {
        let store = self.store();
        let rest = self.parsed.positionals.clone();

        if rest.is_empty() {
            self.console.line("사람이 채우는 열\n");
            let columns = store.user_columns(None).map_err(crash)?;
            // GroupBy — 처음 나온 차례로 묶는다.
            let mut groups: Vec<(String, Vec<UserColumnDefinition>)> = Vec::new();
            for c in columns {
                match groups.iter_mut().find(|(t, _)| t == &c.entity_type) {
                    Some((_, list)) => list.push(c),
                    None => groups.push((c.entity_type.clone(), vec![c])),
                }
            }
            for (entity_type, group) in groups {
                self.console
                    .line(&format!("  [{}]", entity_label(&entity_type)?));
                for c in group {
                    self.console.line(&format!(
                        "    {} {} {}",
                        pad_right(&c.field_name, 12),
                        pad_right(&c.kind, 7),
                        c.choices().join(" / ")
                    ));
                }
            }
            self.console
                .line("\n  넣기: pclm set <키> <열> <값>   (값을 비우면 지운다)");
            return Ok(());
        }

        if rest.len() < 2 {
            return self.refuse("pclm set <키> <열> [값]");
        }

        let key = &rest[0];
        let Some(entity) = store.resolve(key).map_err(crash)? else {
            return self.refuse(&format!("그런 공고·계약이 없습니다: {key}"));
        };

        let field = &rest[1];
        let definitions = store
            .user_columns(Some(&entity.entity_type))
            .map_err(crash)?;
        let known: Vec<&str> = definitions.iter().map(|c| c.field_name.as_str()).collect();
        if !known.contains(&field.as_str()) {
            self.console.error(&format!("모르는 열입니다: {field}"));
            return self.refuse(&format!("쓸 수 있는 열: {}", known.join(", ")));
        }

        let value = (rest.len() >= 3).then(|| rest[2..].join(" "));

        // 후보에 없는 값도 넣게 두되 조용히 넘기지는 않는다 — 오타인지 새 값인지는 사람이 안다.
        let choices = definitions
            .iter()
            .find(|c| &c.field_name == field)
            .map(UserColumnDefinition::choices)
            .unwrap_or_default();
        if let Some(value) = &value
            && !choices.is_empty()
            && !choices.contains(value)
        {
            self.console.line(&format!(
                "  ※ 후보에 없는 값입니다. 후보: {}",
                choices.join(" / ")
            ));
        }

        store
            .set_user_field(&entity, field, value.as_deref())
            .map_err(crash)?;
        let display = entity.display();
        self.console.line(&match value {
            None => format!("지움  {display}.{field}"),
            Some(value) => format!("넣음  {display}.{field} = {value}"),
        });
        Ok(())
    }

    /// 수집한 값을 손으로 고친다. 값을 아예 주지 않으면 되돌린다 — 칸을 비우려면 빈 값을 준다.
    fn fix(&mut self) -> Step {
        let store = self.store();
        let rest = self.parsed.positionals.clone();

        if rest.len() < 2 {
            return self.refuse("pclm fix <키> <열> [값]     (값이 없으면 원래대로 되돌린다)");
        }

        let key = &rest[0];
        let Some(entity) = store.resolve(key).map_err(crash)? else {
            return self.refuse(&format!("그런 공고·계약이 없습니다: {key}"));
        };

        let column = &rest[1];
        match store.check_correctable(&entity, column) {
            Ok(()) => {}
            Err(Error::InvalidOperation(message)) => {
                self.console.error(&message);
                return self.refuse(&format!(
                    "쓸 수 있는 열: {} 의 열 (키 열과 사람 열은 뺀다)",
                    store::face_view(&entity.entity_type)
                ));
            }
            Err(other) => return Err(crash(other)),
        }

        let was = store.face_value(&entity, column).map_err(crash)?;
        let display = entity.display();

        if rest.len() < 3 {
            store.clear_override(&entity, column).map_err(crash)?;
            let now = store.face_value(&entity, column).map_err(crash)?;
            self.console.line(&format!("되돌림  {display}.{column}"));
            self.console
                .line(&format!("  {}  →  {}", shown(&was), shown(&now)));
            return Ok(());
        }

        let value = rest[2..].join(" ");
        store.set_override(&entity, column, &value).map_err(crash)?;

        self.console.line(&format!("고침  {display}.{column}"));
        self.console
            .line(&format!("  {}  →  {}", shown(&was), shown(&value)));
        Ok(())
    }

    /// 레코드를 지운다. 세어 보이고, `--예` 없이는 지우지 않는다 — 되돌릴 수 없다.
    fn remove(&mut self) -> Step {
        let store = self.store();

        let Some(key) = self.positional(0).map(String::from) else {
            return self.refuse("pclm delete <키> [--계열] [--예]");
        };

        let Some(entity) = store.resolve(&key).map_err(crash)? else {
            return self.refuse(&format!("그런 공고·계약이 없습니다: {key}"));
        };

        let whole = self.parsed.flag("--계열");
        let plan = store.plan_deletion(&entity, whole).map_err(crash)?;

        self.console.line(&format!(
            "{}  {}  {}\n",
            entity_label(&plan.entity_type)?,
            plan.display,
            plan.title
        ));
        self.console
            .line(&format!("  쌓인 차수   {}", plan.revisions.join(" · ")));
        let what = if whole {
            "전 차수".to_string()
        } else {
            format!("{} 차수 하나", entity.seq)
        };
        self.console.line(&format!("  지울 것     {what}"));
        self.console
            .line(&format!("  딸린 줄     {}", plan.child_rows));
        self.console
            .line(&format!("  손으로 고친 칸 {}", plan.overrides));

        if plan.series_goes {
            self.console.line(&format!(
                "  사람이 적은 값 {}   ← 계열이 통째로 걷힌다",
                plan.user_fields
            ));
            if plan.linked {
                self.console.line("  확정한 링크도 함께 풀린다");
            }
        }

        if !self.parsed.flag("--예") {
            self.console
                .line("\n지우려면 --예 를 붙이세요. 되돌릴 수 없습니다.");
            return Ok(());
        }

        store.delete(&entity, whole).map_err(crash)?;
        self.console.line(&format!("\n지움  {}", plan.display));
        Ok(())
    }

    fn help(&mut self) {
        self.console.line(&HELP.join("\n"));
    }
}

/// `PclmFile.Snapshot(database, path, Submission, overwrite: true)` — C# 은 덮을 수 있는 자리인지 먼저 보고, 그다음
/// 온전한 경로의 폴더를 지어 뜬다. 뜬 온전한 경로를 낸다.
fn snapshot_submission(
    database: &Database,
    path: &str,
    env: &dyn Env,
) -> pclm_core::Result<PathBuf> {
    let full = dotnet::full_path(path)?;
    pclm_file::require_overwritable(&full)?;
    if let Some(parent) = full.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    pclm_file::snapshot(database, &full, PclmRole::SUBMISSION, true, env)
}

/// `Path.Combine(first, second)` — 둘째가 뿌리부터면 둘째, 아니면 사이에 구분자가 없을 때만 `\` 를 넣는다.
fn path_combine(first: &str, second: &str) -> String {
    if first.is_empty() || Path::new(second).has_root() {
        return second.to_string();
    }
    let separator = |c: char| c == '\\' || c == '/';
    if first.ends_with(separator) || second.starts_with(separator) {
        format!("{first}{second}")
    } else {
        format!("{first}\\{second}")
    }
}

/// 빈 값은 눈에 보여야 한다. 아무것도 찍히지 않으면 지워진 것인지 알 수 없다.
fn shown(value: &str) -> &str {
    if value.is_empty() { "(빔)" } else { value }
}

fn entity_label(entity_type: &str) -> Result<&'static str, Stop> {
    match entity_type {
        "request" => Ok("접수"),
        "notice" => Ok("공고"),
        "contract" => Ok("계약"),
        _ => Err(crash(format!("모르는 자료 종류입니다: {entity_type}"))),
    }
}

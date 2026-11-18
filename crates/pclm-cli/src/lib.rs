//! 명령줄 `pclm` — 같은 인자에는 같은 표준 출력·표준 오류와 종료 코드를 낸다.
//!
//! 창과 같은 홈의 쪽지를 본다(ADR-030). `--home` 은 개발·시험 격리의 문이고, `--file` 은 다른 자료를 읽기 전용 사본으로
//! 열어 본다(ADR-031 — `status`·`export` 에만). 쪽지가 깨졌거나 가리키는 파일이 없으면 까닭을 말하고 2 로 그친다 —
//! 빈 자료를 세우거나 다른 자료를 대신 열지 않는다. 명령줄은 홈 잠금을 잡지 않는다.
//!
//! **글의 꼴.** 줄 끝은 [`NEWLINE`](Windows 는 CR LF)이고, 글 안의 `\n` 은
//! 그대로 LF 다. 그래서 한 줄 안에 LF 와 CR LF 가 섞인다. 여기도 [`NEWLINE`] 과 글 안의 `\n` 을 그대로 둔다.
//!
//! **시계.** 「오늘」(내보내기의 기본 파일 이름) 은 [`Context`] 의 [`Env::now_utc`] 를 [`LocalZone`] 으로
//! 읽는다. 시험은 고정한 시계를 넣는다.
//!
//! **명령이 다루지 않는 오류.** 명령이 따로 다루지 않는 오류(내보낼 자리가 잠겼다 따위)는 표준 오류에 「오류: 」 뒤에 그 글을
//! 적고 [`CRASH_EXIT`] 로 끝낸다.

pub mod arguments;
pub mod text;

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use pclm_core::local_time::{LocalDateTime, LocalZone, parse_to_local};
use pclm_core::storage::pclm_file::{PclmRole, ViewedFile};
use pclm_core::storage::reports::Reports;
use pclm_core::storage::store::{self, Store};
use pclm_core::storage::views::{self, UserColumnDefinition};
use pclm_core::{Access, Database, Env, Error};
use pclm_home::{Home, HomeState};

use crate::arguments::CliArguments;
use crate::text::{pad_left, pad_right, to_lower_invariant};

/// 줄바꿈 — Windows 에서는 CR LF, 그 밖에서는 LF.
pub const NEWLINE: &str = if cfg!(windows) { "\r\n" } else { "\n" };

/// 명령이 다루지 않는 오류의 종료 코드.
pub const CRASH_EXIT: i32 = 1;

/// `--file` 을 받는 명령 — 읽기만 하는 것.
pub const VIEW_COMMANDS: &[&str] = &["status", "export"];

/// 명령줄이 밖에서 받는 것 — 시계와 시간대, 그리고 `--home` 이 없을 때의 홈.
pub struct Context<'a> {
    pub env: &'a dyn Env,
    pub zone: &'a dyn LocalZone,
    /// 기본 홈(`--home` 이 없을 때).
    pub default_home: &'a dyn Fn() -> io::Result<Home>,
}

/// 명령 하나가 멈추는 길. `Exit` 은 종료 코드를 정해 끝내고, `Crash` 는 명령이 다루지 않은 오류다.
enum Stop {
    Exit(i32),
    Crash(String),
}

type Step = Result<(), Stop>;

fn crash(error: impl std::fmt::Display) -> Stop {
    Stop::Crash(error.to_string())
}

/// 출력 둘. 쓰기 실패는 버린다 — 닫힌 파이프에서는 조용히 넘어간다.
struct Console<'w> {
    out: &'w mut dyn Write,
    err: &'w mut dyn Write,
}

impl Console<'_> {
    /// 한 줄을 적는다(줄 끝 포함).
    fn line(&mut self, text: &str) {
        let _ = write!(self.out, "{text}{NEWLINE}");
        let _ = self.out.flush();
    }

    /// 오류 한 줄을 오류 출력에 쓴다.
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
            console.error(&format!("오류: {message}"));
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
        "status" => cli.status(),
        "export" => cli.export(),
        "set" => cli.set(),
        "fix" => cli.fix(),
        "delete" => cli.remove(),
        _ => {
            cli.help();
            Ok(())
        }
    };
    // 연결 풀이 없어 명령이 마지막에 닫은 것이 읽기 전용 연결(status · export …)이면
    // 빈 곁파일이 작업자료 옆에 남는다 — 쓰기 연결 하나를 열어 마지막에 닫는다.
    if cli.database.access() == Access::Write {
        drop(cli.database.open());
    }
    match result {
        // 열어 본 사본은 다 읽었으면 지운다. 못 지운 것은 다음 열람이 걷는다.
        Ok(()) => drop(viewed),
        // 끝내거나 다루지 않은 오류로 멈출 때는 사본을 걷지 않는다 — 다음 열람이 걷는다.
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
    "pclm status                         쌓인 상태를 본다",
    "pclm export [경로]                  계약면을 엑셀로 내보낸다 (그 시점의 사진)",
    "pclm set [키 열 값]                 손으로 채우는 값 (인자 없으면 열 목록)",
    "pclm fix <키> <열> [값]             수집한 값을 고친다 (값 없으면 원래대로)",
    "pclm delete <키>                    레코드를 지운다 (무엇이 사라지는지 세어 보인다)",
    "  --계열                            그 본번호의 전 차수를 지운다",
    "  --예                              실제로 지운다 (없으면 세어 보이기만 한다)",
    "  --home <폴더>                     다른 홈의 작업자료를 연다 (기본은 창과 같은 홈)",
    "  --file <파일>                     다른 자료를 열어 본다 — status · export 만, 읽기 전용",
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

    /// 지금.
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

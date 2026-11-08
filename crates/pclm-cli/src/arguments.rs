//! 인자 풀이(C# `src/Pclm.Cli/CliArguments.cs`).
//!
//! 첫 인자는 명령이라 풀지 않는다 — 둘째부터 옵션·깃발·자리 인자로 가른다. `--` 뒤는 모두 자리 인자다. 모르는 옵션을
//! 자리 인자로 흘려보내지 않는다: `--긴급` 같은 값을 옵션으로 잘못 적은 것인지 사람이 알 수 없게 된다. 거절은 C# 의
//! `ArgumentException.Message` 와 같은 글이다.

/// 값을 받는 옵션.
const OPTIONS: &[&str] = &["--home", "--file", "--out", "--이름"];

/// 값 없이 서는 깃발.
const FLAGS: &[&str] = &["--계열", "--예"];

/// C# `CliArguments` — 풀린 인자.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CliArguments {
    /// 옵션은 처음 것이 이긴다(C# `TryAdd`).
    options: Vec<(String, String)>,
    flags: Vec<String>,
    pub positionals: Vec<String>,
}

impl CliArguments {
    /// `Option(name)` — 그 옵션의 값. 없으면 `None`.
    pub fn option(&self, name: &str) -> Option<&str> {
        self.options
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    /// `Flag(name)` — 그 깃발이 섰는가.
    pub fn flag(&self, name: &str) -> bool {
        self.flags.iter().any(|f| f == name)
    }

    /// `CliArguments.Parse(args)` — `args[0]` 은 명령이라 건너뛴다. 거절이면 그 글.
    pub fn parse(args: &[String]) -> Result<CliArguments, String> {
        let mut parsed = CliArguments::default();
        let mut i = 1;
        while i < args.len() {
            let arg = &args[i];
            if arg == "--" {
                parsed.positionals.extend(args[i + 1..].iter().cloned());
                break;
            }
            if !arg.starts_with("--") {
                parsed.positionals.push(arg.clone());
                i += 1;
                continue;
            }
            if FLAGS.contains(&arg.as_str()) {
                if !parsed.flag(arg) {
                    parsed.flags.push(arg.clone());
                }
                i += 1;
                continue;
            }
            if arg == "--db" {
                return Err(
                    "--db 는 없어졌습니다. 시험 자료는 --home <폴더> 로 따로 여세요.".into(),
                );
            }
            if !OPTIONS.contains(&arg.as_str()) {
                return Err(format!(
                    "모르는 옵션입니다: {arg}. -- 뒤에는 입력값을 그대로 적을 수 있습니다."
                ));
            }
            if i + 1 >= args.len() || args[i + 1].starts_with("--") {
                return Err(format!("{arg} 뒤에 값을 적으세요."));
            }
            i += 1;
            if parsed.option(arg).is_none() {
                parsed.options.push((arg.clone(), args[i].clone()));
            }
            i += 1;
        }
        Ok(parsed)
    }
}

/// `tests/Pclm.Core.Tests/CliArgumentsTests.cs` 를 사례마다 옮긴 것.
#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn 모르는_옵션을_값누락으로_넘기지_않는다() {
        for command in ["fix", "set"] {
            let error = CliArguments::parse(&args(&[command, "문서번호", "열", "--긴급"]))
                .expect_err("거절해야 한다");
            assert!(error.contains("--긴급"), "{error}");
        }
    }

    #[test]
    fn 종료표식_뒤에는_옵션도_입력값이다() {
        for command in ["fix", "set"] {
            let parsed = CliArguments::parse(&args(&[
                command,
                "--home",
                "시험홈",
                "문서번호",
                "열",
                "--",
                "--긴급",
                "--예",
            ]))
            .unwrap();
            assert_eq!(parsed.option("--home"), Some("시험홈"));
            assert_eq!(
                parsed.positionals,
                args(&["문서번호", "열", "--긴급", "--예"])
            );
            assert!(!parsed.flag("--예"));
        }
    }

    #[test]
    fn 옵션값이_없거나_다음옵션이면_거절한다() {
        for option in ["--home", "--file", "--out", "--이름"] {
            assert!(CliArguments::parse(&args(&["merge", option])).is_err());
            assert!(CliArguments::parse(&args(&["merge", option, "--예"])).is_err());
        }
    }

    #[test]
    fn 알려진_옵션과_깃발은_자리인자에서_분리한다() {
        let parsed = CliArguments::parse(&args(&[
            "merge",
            "제출본1",
            "--home",
            "시험홈",
            "--file",
            "열람본",
            "--out",
            "취합본",
            "--이름",
            "담당자",
            "--계열",
            "--예",
            "제출본2",
        ]))
        .unwrap();
        assert_eq!(parsed.positionals, args(&["제출본1", "제출본2"]));
        assert_eq!(parsed.option("--home"), Some("시험홈"));
        assert_eq!(parsed.option("--file"), Some("열람본"));
        assert_eq!(parsed.option("--out"), Some("취합본"));
        assert_eq!(parsed.option("--이름"), Some("담당자"));
        assert!(parsed.flag("--계열"));
        assert!(parsed.flag("--예"));
        assert_eq!(parsed.option("--없는옵션"), None);
    }

    #[test]
    fn 없어진_db옵션은_실제홈을_열기전에_거절한다() {
        let error = CliArguments::parse(&args(&["status", "--db", "시험자료"])).unwrap_err();
        assert!(error.contains("--db 는 없어졌습니다"), "{error}");
    }

    // ── C# 시험에 없던 자리(같은 코드의 갈래) ─────────────────────

    #[test]
    fn 거절_글은_cs_와_같다() {
        assert_eq!(
            CliArguments::parse(&args(&["status", "--db", "x"])).unwrap_err(),
            "--db 는 없어졌습니다. 시험 자료는 --home <폴더> 로 따로 여세요."
        );
        assert_eq!(
            CliArguments::parse(&args(&["fix", "--긴급"])).unwrap_err(),
            "모르는 옵션입니다: --긴급. -- 뒤에는 입력값을 그대로 적을 수 있습니다."
        );
        assert_eq!(
            CliArguments::parse(&args(&["merge", "--out"])).unwrap_err(),
            "--out 뒤에 값을 적으세요."
        );
    }

    #[test]
    fn 깃발이_db_보다_먼저고_db_가_모르는_옵션보다_먼저다() {
        // 차례: -- · 자리 인자 · 깃발 · --db · 모르는 옵션 · 값 누락.
        let error = CliArguments::parse(&args(&["status", "--예", "--db", "--긴급"])).unwrap_err();
        assert!(error.starts_with("--db"), "{error}");
        let error = CliArguments::parse(&args(&["status", "--긴급", "--db"])).unwrap_err();
        assert!(error.starts_with("모르는 옵션"), "{error}");
    }

    #[test]
    fn 같은_옵션은_처음_것이_이긴다() {
        let parsed = CliArguments::parse(&args(&["status", "--home", "a", "--home", "b"])).unwrap();
        assert_eq!(parsed.option("--home"), Some("a"));
        assert!(parsed.positionals.is_empty());
    }

    #[test]
    fn 첫_인자는_명령이라_풀지_않는다() {
        let parsed = CliArguments::parse(&args(&["--home", "x", "status"])).unwrap();
        assert_eq!(parsed.option("--home"), None);
        assert_eq!(parsed.positionals, args(&["x", "status"]));
        assert_eq!(CliArguments::parse(&[]).unwrap(), CliArguments::default());
    }

    #[test]
    fn 하이픈_하나는_자리_인자다() {
        let parsed = CliArguments::parse(&args(&["fix", "-x", "열", "--", "--"])).unwrap();
        assert_eq!(parsed.positionals, args(&["-x", "열", "--"]));
    }
}

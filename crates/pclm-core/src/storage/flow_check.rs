//! 가공식 v2 의 꼴 검사(C# `Storage/Flow.cs` 의 `Check` 와 그 재료 — `FlowEnv`·`FlowLinked`·`FlowIssue`·
//! `FlowChecked`·`Verbs`·`EnvFor`·`EnvForView`·`ToNumber`). 자료 모형과 읽기·쓰기는 [`super::flow`] 에 있다.
//!
//! **화면의 `frontend/src/flow/` 와 한 벌이다.** 동사 이름·인자 자리·꼴 표가 갈리면 화면에서 되던 식이 저장에서
//! 막히거나, 막혀야 할 식이 저장된다. 세 쪽(vitest·xunit·이 크레이트의 `flow_cases` 시험)이
//! `tests/flow-cases.json` 하나를 함께 돌려 갈림을 잡는다.

use crate::error::{Error, Result};
use crate::storage::flow::{Flow, FlowBranch, FlowClause, FlowDefinition, FlowSource, FlowStep};
use crate::storage::{computed_column, views};
use crate::text::utf16_len;

/// C# `FlowLinked(Fields, Items, Many)` — 이을 수 있는 연결 문서 하나. `many` 면 원천이 행목록이다(§11.1-2).
#[derive(Clone, Debug, PartialEq)]
pub struct FlowLinked {
    pub fields: Vec<String>,
    pub items: Vec<String>,
    pub many: bool,
}

/// C# `FlowEnv(Base, Items, Linked, View)` — 꼴 검사가 보는 재료. `view` 면 맞춤 뷰의 재료다(docs/user-views.md §3):
/// 「이 행」 이 없고 접수·공고·계약이 모두 한 건이다. `linked` 는 C# 사전의 넣은 차례 그대로다.
#[derive(Clone, Debug, PartialEq)]
pub struct FlowEnv {
    pub base: Vec<String>,
    pub items: Vec<String>,
    pub linked: Vec<(String, FlowLinked)>,
    pub view: bool,
}

impl FlowEnv {
    /// `env.Linked.TryGetValue(kind, …)`.
    pub fn link(&self, kind: &str) -> Option<&FlowLinked> {
        self.linked.iter().find(|(k, _)| k == kind).map(|(_, l)| l)
    }
}

/// C# `FlowIssue(Index, Message, Final)` — 꼴 검사의 오류. `index` 0 이 원천, i+1 이 `steps[i]` 다.
#[derive(Clone, Debug, PartialEq)]
pub struct FlowIssue {
    pub index: usize,
    pub message: String,
    pub r#final: bool,
}

impl FlowIssue {
    /// `FlowIssue.Describe` — 저장 거절에 싣는 말(「3단계: …」).
    pub fn describe(&self) -> String {
        let place = if self.index == 0 {
            "시작 데이터".to_string()
        } else {
            format!("{}단계", self.index)
        };
        format!("{place}: {}", self.message)
    }
}

/// C# `FlowChecked(Shapes, Fields, Errors, Final)`.
#[derive(Clone, Debug, PartialEq)]
pub struct FlowChecked {
    pub shapes: Vec<Option<&'static str>>,
    pub fields: Vec<Vec<String>>,
    pub errors: Vec<FlowIssue>,
    pub r#final: Option<&'static str>,
}

impl FlowChecked {
    /// `FlowChecked.Ok`.
    pub fn ok(&self) -> bool {
        self.errors.is_empty()
    }
}

/// C# `Flow.Verb(Op, Word, Io)` — 동사 하나. `io` 는 들어감 → 나옴(화면 `verbs.ts` 와 같은 표).
#[derive(Clone, Copy, Debug)]
pub struct Verb {
    pub op: &'static str,
    pub word: &'static str,
    pub io: &'static [(&'static str, &'static str)],
}

const fn verb(
    op: &'static str,
    word: &'static str,
    io: &'static [(&'static str, &'static str)],
) -> Verb {
    Verb { op, word, io }
}

/// `Flow.Verbs`.
pub const VERBS: &[Verb] = &[
    verb("filter", "거르기", &[("rows", "rows")]),
    verb("sort", "정렬", &[("rows", "rows"), ("values", "values")]),
    verb(
        "distinct",
        "고유",
        &[("rows", "rows"), ("values", "values")],
    ),
    verb("take", "앞에서", &[("rows", "rows"), ("values", "values")]),
    verb("nth", "번째", &[("rows", "row"), ("values", "value")]),
    verb("first", "처음", &[("rows", "row"), ("values", "value")]),
    verb("last", "마지막", &[("rows", "row"), ("values", "value")]),
    verb("format", "서식", &[("rows", "values"), ("row", "value")]),
    verb("get", "꺼내기", &[("rows", "values"), ("row", "value")]),
    verb("join", "잇기", &[("values", "value")]),
    verb("abbrev", "줄여쓰기", &[("values", "value")]),
    verb("count", "개수", &[("rows", "number"), ("values", "number")]),
    verb("sum", "합계", &[("rows", "number"), ("values", "number")]),
    verb("min", "최소", &[("rows", "number"), ("values", "number")]),
    verb("max", "최대", &[("rows", "number"), ("values", "number")]),
    verb("avg", "평균", &[("rows", "number"), ("values", "number")]),
    verb("trim", "다듬기", &[("value", "value")]),
    verb("replace", "바꾸기", &[("value", "value")]),
    verb("before", "앞부분", &[("value", "value")]),
    verb("after", "뒷부분", &[("value", "value")]),
    verb("substr", "자르기", &[("value", "value")]),
    verb("padStart", "앞채우기", &[("value", "value")]),
    verb("ifEmpty", "비면", &[("value", "value")]),
    verb("map", "대응", &[("value", "value")]),
    verb("if", "만약", &[("row", "value"), ("value", "value")]),
    verb("cases", "경우", &[("value", "value")]),
    verb("toNumber", "숫자로", &[("value", "number")]),
    verb("add", "더하기", &[("number", "number")]),
    verb("sub", "빼기", &[("number", "number")]),
    verb("mul", "곱하기", &[("number", "number")]),
    verb("div", "나누기", &[("number", "number")]),
    verb("round", "반올림", &[("number", "number")]),
    verb("floor", "버림", &[("number", "number")]),
    verb("thousands", "천단위", &[("number", "value")]),
    verb("plain", "그대로", &[("number", "value")]),
    verb("toDate", "날짜로", &[("value", "date")]),
    verb("dateAdd", "날짜더하기", &[("date", "date")]),
    verb("dateDiff", "날짜차이", &[("date", "number")]),
    verb("year", "연도", &[("date", "number")]),
    verb("month", "월", &[("date", "number")]),
    verb("quarter", "분기", &[("date", "number")]),
    verb("dateFormat", "날짜서식", &[("date", "value")]),
];

/// `Flow.ByOp`.
pub fn verb_of(op: &str) -> Option<&'static Verb> {
    VERBS.iter().find(|v| v.op == op)
}

/// `Flow.Shapes` — 꼴의 사람 말. 꼴은 이 여섯뿐이다(C# 사전에 없는 열쇠는 `KeyNotFoundException`).
pub fn shape_word(shape: &str) -> &'static str {
    match shape {
        "rows" => "행목록",
        "values" => "값목록",
        "row" => "행",
        "value" => "값",
        "number" => "수",
        "date" => "날짜",
        _ => unreachable!("모르는 꼴: {shape}"),
    }
}

/// `Flow.LinkWords` — 가공열에서 연결 문서를 부르는 말.
pub fn link_words(kind: &str) -> Option<&'static str> {
    match kind {
        "notice" => Some("연결공고"),
        "request" => Some("연결접수"),
        "contract" => Some("연결계약"),
        _ => None,
    }
}

/// `Flow.DocWords` — 맞춤 뷰의 문서 이름(글 꼴의 `공고.공고명` 이자 뷰 SQL 의 바깥 행 별칭).
pub fn doc_words(kind: &str) -> Option<&'static str> {
    match kind {
        "request" => Some("접수"),
        "notice" => Some("공고"),
        "contract" => Some("계약"),
        _ => None,
    }
}

/// `Flow.DocWords` 의 열쇠 차례(사전에 넣은 차례).
const DOC_KINDS: [&str; 3] = ["request", "notice", "contract"];

/// `Flow.MaxBranches` — `경우` 의 갈래 수 상한(화면 `check.ts` 의 `maxBranches`).
pub const MAX_BRANCHES: usize = 20;

/// `Flow.Ends` — 저장할 수 있는 끝 꼴.
const ENDS: [&str; 3] = ["value", "number", "date"];

/// `Flow.LinkWord` — 이 재료에서 연결 문서를 부르는 말. 가공열은 `연결공고`, 맞춤 뷰는 `공고`. 모르면 그대로.
pub fn link_word(env: &FlowEnv, link: Option<&str>) -> String {
    let link = link.unwrap_or("");
    let word = if env.view {
        doc_words(link)
    } else {
        link_words(link)
    };
    word.unwrap_or(link).to_string()
}

/// `Flow.Qualified` — 맞춤 뷰에서 단계가 읽는 행의 열(`공고.공고명`)을 (종류, 열)로 가른다. 첫 점에서 가른다.
pub fn qualified(field: &str) -> Option<(&'static str, &str)> {
    let dot = field.find('.')?;
    if dot == 0 {
        return None;
    }
    let word = &field[..dot];
    let kind = DOC_KINDS.iter().find(|k| doc_words(k) == Some(word))?;
    Some((kind, &field[dot + 1..]))
}

/// `Flow.Clauses` — 단계의 조건 마디 모두(`경우` 의 갈래 포함).
pub fn clauses(step: &FlowStep) -> impl Iterator<Item = &FlowClause> {
    let own = step.cond.iter().flatten().flatten();
    let branches = step
        .branches
        .iter()
        .flatten()
        .flatten()
        .flat_map(|b| b.cond.iter().flatten().flatten());
    own.chain(branches)
}

impl Flow {
    /// `Flow.EnvFor` — 이 자료 종류의 재료. 기본 열은 사람 열을 뺀 면 뷰의 열, 품목은 `ComputedColumn.Fields`,
    /// 연결은 §11.1-2 의 표 — 공고·접수에서 연결계약은 여러 건이다.
    pub fn env_for(entity: &str) -> Result<FlowEnv> {
        fn link(kind: &str, many: bool) -> Result<FlowLinked> {
            Ok(FlowLinked {
                fields: owned(views::base_columns(kind)?),
                items: computed_column::field_names(kind)?,
                many,
            })
        }
        let linked = match entity {
            "contract" => vec![
                ("notice".to_string(), link("notice", false)?),
                ("request".to_string(), link("request", false)?),
            ],
            "notice" => vec![
                ("request".to_string(), link("request", false)?),
                ("contract".to_string(), link("contract", true)?),
            ],
            "request" => vec![
                ("notice".to_string(), link("notice", false)?),
                ("contract".to_string(), link("contract", true)?),
            ],
            _ => {
                return Err(Error::invalid("가공열의 자료 종류가 올바르지 않습니다."));
            }
        };
        Ok(FlowEnv {
            base: owned(views::base_columns(entity)?),
            items: computed_column::field_names(entity)?,
            linked,
            view: false,
        })
    }

    /// `Flow.EnvForView` — 맞춤 뷰의 재료. 문서마다 그 면 뷰의 열을 모두 읽는다: 기본 열 끝에 사람 열·가공열
    /// `own`(종류 → 이름, 차례대로)을 더한다. 맞춤 뷰는 면 뷰 위층이라 그 열을 읽어도 순환이 없다.
    pub fn env_for_view(own: &[(String, Vec<String>)]) -> FlowEnv {
        let link = |kind: &str| {
            let columns = views::base_columns(kind).expect("세 종류는 기본 열이 있다");
            let mine = own
                .iter()
                .find(|(k, _)| k == kind)
                .map(|(_, m)| m.as_slice())
                .unwrap_or(&[]);
            let mut fields = owned(columns);
            fields.extend(
                mine.iter()
                    .filter(|n| !columns.contains(&n.as_str()))
                    .cloned(),
            );
            FlowLinked {
                fields,
                items: computed_column::field_names(kind).expect("세 종류는 품목이 있다"),
                many: false,
            }
        };
        FlowEnv {
            base: Vec::new(),
            items: Vec::new(),
            linked: vec![
                ("request".to_string(), link("request")),
                ("notice".to_string(), link("notice")),
                ("contract".to_string(), link("contract")),
            ],
            view: true,
        }
    }

    /// `Flow.SourceShape` — 원천의 꼴과 필드. 오류면 글.
    pub fn source_shape(
        from: &FlowSource,
        env: &FlowEnv,
    ) -> std::result::Result<(&'static str, Vec<String>), String> {
        let field = from.field.as_deref();
        match from.kind.as_deref() {
            Some("items") if env.view => Err(
                "맞춤 뷰에는 「이 행」 이 없습니다. ‘공고.품목’처럼 문서를 붙여 읽으세요".into(),
            ),
            Some("field") if env.view => Err(format!(
                "맞춤 뷰에는 「이 행」 이 없습니다. ‘공고.{}’처럼 문서를 붙여 읽으세요",
                field.unwrap_or("")
            )),
            Some("items") => Ok(("rows", env.items.clone())),
            Some("text") => Ok(("value", Vec::new())),
            Some("number") => Ok(("number", Vec::new())),
            Some("field") => {
                if env.base.iter().any(|b| b == field.unwrap_or("")) {
                    Ok(("value", Vec::new()))
                } else {
                    Err(format!("이 행에 없는 열: {}", field.unwrap_or("")))
                }
            }
            Some("linked") => {
                let word = link_word(env, from.link.as_deref());
                let Some(link) = from.link.as_deref().and_then(|l| env.link(l)) else {
                    return Err(format!(
                        "{} 이 자료 종류에서 쓸 수 없습니다",
                        eun_neun(&word)
                    ));
                };
                if from.items == Some(true) {
                    if !link.many {
                        return Ok(("rows", link.items.clone()));
                    }
                    return Err(format!(
                        "{} 여러 건이라 품목을 바로 읽을 수 없습니다. ‘{word} |> 처음’처럼 한 건을 먼저 고르세요",
                        eun_neun(&word)
                    ));
                }
                let Some(field) = field else {
                    return Ok((if link.many { "rows" } else { "row" }, link.fields.clone()));
                };
                if link.many {
                    return Err(format!(
                        "{} 여러 건입니다. ‘{word} |> 처음 |> 꺼내기 {field}’처럼 한 건을 골라 읽으세요",
                        eun_neun(&word)
                    ));
                }
                if link.fields.iter().any(|f| f == field) {
                    Ok(("value", Vec::new()))
                } else {
                    Err(format!("{word}에 없는 필드: {field}"))
                }
            }
            other => Err(format!("알 수 없는 시작 데이터: {}", other.unwrap_or(""))),
        }
    }

    /// `Flow.Placeholders` — 서식의 `{필드}` 를 차례대로. 짝이 안 맞으면 오류 글.
    pub fn placeholders(text: &str) -> std::result::Result<Vec<String>, String> {
        let chars: Vec<char> = text.chars().collect();
        let mut output = Vec::new();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '}' {
                return Err("서식의 ‘}’ 짝이 없습니다".into());
            }
            if chars[i] != '{' {
                i += 1;
                continue;
            }
            let after = |target: char| {
                chars[i + 1..]
                    .iter()
                    .position(|&c| c == target)
                    .map(|p| p + i + 1)
            };
            let end = match (after('}'), after('{')) {
                (Some(end), Some(open)) if open < end => None,
                (end, _) => end,
            };
            let Some(end) = end else {
                return Err("서식의 ‘{’ 가 닫히지 않았습니다".into());
            };
            if end == i + 1 {
                return Err("서식에 빈 {} 가 있습니다".into());
            }
            output.push(chars[i + 1..end].iter().collect());
            i = end + 1;
        }
        Ok(output)
    }

    /// `Flow.Check` — 꼴 검사(§4.2·§4.4). 오류는 그것을 만든 단계에 붙고, 첫 오류 뒤의 단계는 꼴을 모른다(`None`).
    /// 화면의 `checkFlow` 와 같은 결과를 낸다. 단계·원천은 [`Flow::parse`] 를 지난 것이라 비어 있지 않다.
    pub fn check(flow: &FlowDefinition, env: &FlowEnv) -> FlowChecked {
        let steps: Vec<&FlowStep> = flow.steps.iter().flatten().flatten().collect();
        let n = steps.len() + 1;
        let mut shapes: Vec<Option<&'static str>> = vec![None; n];
        let mut fields: Vec<Vec<String>> = vec![Vec::new(); n];
        let mut errors = Vec::new();
        let empty = FlowSource::default();
        let (shape, source_fields) =
            match Flow::source_shape(flow.from.as_ref().unwrap_or(&empty), env) {
                Ok(s) => s,
                Err(message) => {
                    errors.push(issue(0, message));
                    return FlowChecked {
                        shapes,
                        fields,
                        errors,
                        r#final: None,
                    };
                }
            };
        shapes[0] = Some(shape);
        fields[0] = source_fields;
        for (i, step) in steps.iter().enumerate() {
            let input = shapes[i].expect("앞 단계는 꼴을 안다");
            let op = step.op.as_deref().unwrap_or("");
            let Some(verb) = verb_of(op) else {
                errors.push(issue(i + 1, format!("알 수 없는 연산: {op}")));
                break;
            };
            let Some(&(_, output)) = verb.io.iter().find(|(io_in, _)| *io_in == input) else {
                let mut accepts: Vec<&str> = Vec::new();
                for (io_in, _) in verb.io {
                    let word = shape_word(io_in);
                    if !accepts.contains(&word) {
                        accepts.push(word);
                    }
                }
                let word = verb_word(step);
                let suffix = eun_neun(&word)[word.len()..].to_string();
                errors.push(issue(
                    i + 1,
                    format!(
                        "‘{word}’{suffix} {}에 쓸 수 없습니다. {}에 쓰는 동사입니다",
                        shape_word(input),
                        accepts.join("·")
                    ),
                ));
                break;
            };
            if let Some(problem) = step_problem(step, input, &fields[i], env) {
                errors.push(issue(i + 1, problem));
                break;
            }
            shapes[i + 1] = Some(output);
            fields[i + 1] = if matches!(output, "rows" | "row") {
                fields[i].clone()
            } else {
                Vec::new()
            };
        }
        let last = if errors.is_empty() {
            shapes[n - 1]
        } else {
            None
        };
        if let Some(last) = last
            && !ENDS.contains(&last)
        {
            errors.push(FlowIssue {
                index: n - 1,
                message: format!(
                    "마지막 결과가 {}입니다. 값·수·날짜 중 하나로 끝나야 저장할 수 있습니다",
                    shape_word(last)
                ),
                r#final: true,
            });
        }
        let r#final = if errors.is_empty() { last } else { None };
        FlowChecked {
            shapes,
            fields,
            errors,
            r#final,
        }
    }

    /// `Flow.ToNumber` — 글을 수로. 쉼표와 앞뒤 공백을 걷고 `^[-+]?([0-9]+\.?[0-9]*|\.[0-9]+)$` 꼴만 읽는다(지수
    /// 표기는 받지 않는다). 화면의 `toNumber`, SQL 의 숫자 판정(`FlowCompiler.Number`)과 같은 규칙이다.
    pub fn to_number(text: &str) -> Option<f64> {
        let t = text.replace(',', "");
        let t = t.trim();
        let b = t.as_bytes();
        let body = &b[usize::from(matches!(b.first(), Some(b'+' | b'-')))..];
        let digit = |c: &u8| c.is_ascii_digit();
        let ok = if body.first() == Some(&b'.') {
            body.len() > 1 && body[1..].iter().all(digit)
        } else {
            let whole = body.iter().take_while(|c| digit(c)).count();
            whole > 0
                && (whole == body.len()
                    || (body[whole] == b'.' && body[whole + 1..].iter().all(digit)))
        };
        if !ok {
            return None;
        }
        t.parse::<f64>().ok()
    }
}

fn issue(index: usize, message: String) -> FlowIssue {
    FlowIssue {
        index,
        message,
        r#final: false,
    }
}

fn owned(names: &[&str]) -> Vec<String> {
    names.iter().map(|s| s.to_string()).collect()
}

/// `Flow.은는` — 받침에 맞춰 「은/는」을 붙인다(화면 `flow/josa.ts` 와 같은 규칙).
fn eun_neun(word: &str) -> String {
    let last = word
        .trim_end()
        .trim_end_matches(['’', '\'', '」', ')', ']']);
    let c = last.chars().last().unwrap_or(' ');
    let batchim = if ('가'..='힣').contains(&c) {
        !(c as u32 - '가' as u32).is_multiple_of(28)
    } else {
        matches!(c, '0' | '1' | '3' | '6' | '7' | '8')
    };
    format!("{word}{}", if batchim { "은" } else { "는" })
}

/// `Flow.VerbWord`.
fn verb_word(step: &FlowStep) -> String {
    let op = step.op.as_deref().unwrap_or("");
    if op == "nth" {
        return format!("{}번째", step.n.unwrap_or(1));
    }
    verb_of(op).map_or_else(|| op.to_string(), |v| v.word.to_string())
}

/// `Flow.CondProblem` — 조건 검사. 행을 견주면 마디의 필드는 그 행의 필드다. 값 하나를 견주면 빈 필드가 지금
/// 값이고, 적힌 필드는 바깥의 열이라 `outer` 를 탄다.
fn cond_problem(
    cond: Option<&Vec<Option<FlowClause>>>,
    shape: &str,
    fields: &[String],
    outer: &dyn Fn(&str) -> Option<String>,
) -> Option<String> {
    let Some(cond) = cond.filter(|c| !c.is_empty()) else {
        return Some("조건이 비었습니다".into());
    };
    for c in cond.iter().flatten() {
        let field = c.field.as_deref().unwrap_or("");
        if shape == "value" {
            if !field.is_empty()
                && let Some(problem) = outer(field)
            {
                return Some(problem);
            }
            continue;
        }
        if field.is_empty() {
            return Some("견줄 필드를 고르세요".into());
        }
        if !fields.iter().any(|f| f == field) {
            return Some(format!("없는 필드: {field}"));
        }
    }
    None
}

/// `Flow.StepProblem`.
fn step_problem(step: &FlowStep, input: &str, fields: &[String], env: &FlowEnv) -> Option<String> {
    let rowish = matches!(input, "rows" | "row");
    let need_field = |f: Option<&str>| -> Option<String> {
        match f {
            None | Some("") => Some("필드를 고르세요".into()),
            Some(f) if !fields.iter().any(|x| x == f) => Some(format!("없는 필드: {f}")),
            _ => None,
        }
    };
    let base_field = |f: &str| -> Option<String> {
        if env.view {
            doc_field(f, env)
        } else if env.base.iter().any(|b| b == f) {
            None
        } else {
            Some(format!("이 행에 없는 열: {f}"))
        }
    };
    let field = step.field.as_deref();
    match step.op.as_deref().unwrap_or("") {
        "filter" => cond_problem(step.cond.as_ref(), input, fields, &base_field),
        "if" => cond_problem(
            step.cond.as_ref(),
            if input == "row" { "row" } else { "value" },
            fields,
            &base_field,
        ),
        "sort" => match field {
            None => None,
            Some(_) if rowish => need_field(field),
            Some(_) => Some("값목록은 필드 없이 정렬합니다".into()),
        },
        "distinct" => {
            let chosen = step.fields.as_ref().filter(|f| !f.is_empty())?;
            if !rowish {
                return Some("값목록은 기준 필드 없이 고유를 셉니다".into());
            }
            chosen.iter().find_map(|f| need_field(f.as_deref()))
        }
        "take" | "nth" => (step.n.unwrap_or(0) < 1).then(|| "1 이상의 수를 적으세요".into()),
        "abbrev" => {
            if step.n.unwrap_or(0) < 1 {
                return Some("보일 개수는 1 이상입니다".into());
            }
            match Flow::placeholders(step.text.as_deref().unwrap_or("")) {
                Err(error) => Some(error),
                Ok(ph) => ph
                    .iter()
                    .find(|p| !matches!(p.as_str(), "앞" | "나머지"))
                    .map(|bad| format!("줄여쓰기에는 {{앞}}·{{나머지}}만 씁니다: {{{bad}}}")),
            }
        }
        "format" => match Flow::placeholders(step.text.as_deref().unwrap_or("")) {
            Err(error) => Some(error),
            Ok(ph) => ph
                .iter()
                .find(|p| !fields.contains(p))
                .map(|bad| format!("서식에 없는 필드: {{{bad}}}")),
        },
        "get" => need_field(field),
        "sum" | "min" | "max" | "avg" => {
            if input == "rows" {
                need_field(field)
            } else if field.is_some() {
                Some("값목록에는 필드를 적지 않습니다".into())
            } else {
                None
            }
        }
        "substr" => (step.n.unwrap_or(0) < 1 || step.length.unwrap_or(-1) < 0)
            .then(|| "시작은 1 이상, 길이는 0 이상입니다".into()),
        "padStart" => (step.length.unwrap_or(0) < 1
            || step.text.as_deref().map_or(0, utf16_len) < 1)
            .then(|| "길이와 채울 글자를 적으세요".into()),
        "ifEmpty" => field.and_then(base_field),
        "map" => step
            .pairs
            .as_ref()
            .is_none_or(Vec::is_empty)
            .then(|| "대응할 값을 하나 이상 적으세요".into()),
        "cases" => {
            let branches: Vec<&FlowBranch> = step.branches.iter().flatten().flatten().collect();
            if branches.is_empty() {
                return Some("경우를 하나 이상 적으세요".into());
            }
            if branches.len() > MAX_BRANCHES {
                return Some(format!("경우는 {MAX_BRANCHES}개까지입니다"));
            }
            branches.iter().enumerate().find_map(|(i, b)| {
                cond_problem(b.cond.as_ref(), "value", fields, &base_field)
                    .map(|problem| format!("{}번째 경우: {problem}", i + 1))
            })
        }
        "add" | "sub" | "mul" | "div" => {
            if let Some(f) = field {
                return base_field(f);
            }
            (step.op.as_deref() == Some("div") && step.value == Some(0.0))
                .then(|| "0 으로 나눌 수 없습니다".into())
        }
        "dateDiff" => match field {
            Some(f) if !f.is_empty() => base_field(f),
            _ => Some("뺄 날짜 열을 고르세요".into()),
        },
        "dateFormat" => step
            .text
            .as_deref()
            .is_none_or(str::is_empty)
            .then(|| "서식을 적으세요".into()),
        _ => None,
    }
}

/// `Flow.DocField` — 맞춤 뷰에서 단계가 읽는 행의 열(`공고.공고명`)이 그 문서에 있는가. 없으면 오류 글.
fn doc_field(field: &str, env: &FlowEnv) -> Option<String> {
    let Some((kind, column)) = qualified(field) else {
        return Some(format!(
            "문서를 붙여 적으세요 — ‘공고.{field}’처럼 읽습니다"
        ));
    };
    let word = doc_words(kind).unwrap_or(kind);
    match env.link(kind) {
        Some(link) if link.fields.iter().any(|f| f == column) => None,
        _ => Some(format!("{word}에 없는 필드: {column}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_number_reads_only_plain_decimals() {
        assert_eq!(Flow::to_number(" 1,234.5 "), Some(1234.5));
        assert_eq!(Flow::to_number("+.5"), Some(0.5));
        assert_eq!(Flow::to_number("-3."), Some(-3.0));
        assert_eq!(Flow::to_number("1e5"), None);
        assert_eq!(Flow::to_number("."), None);
        assert_eq!(Flow::to_number(""), None);
        assert_eq!(Flow::to_number("abc"), None);
    }

    #[test]
    fn josa_follows_batchim() {
        assert_eq!(eun_neun("연결공고"), "연결공고는");
        assert_eq!(eun_neun("공고"), "공고는");
        assert_eq!(eun_neun("합계"), "합계는");
        assert_eq!(eun_neun("반올림"), "반올림은");
        assert_eq!(eun_neun("3번째"), "3번째는");
    }

    #[test]
    fn placeholders_pair_braces() {
        assert_eq!(
            Flow::placeholders("{a}-{b}").unwrap(),
            vec!["a".to_string(), "b".to_string()]
        );
        assert!(Flow::placeholders("{a").is_err());
        assert!(Flow::placeholders("a}").is_err());
        assert!(Flow::placeholders("{}").is_err());
        assert!(Flow::placeholders("{a{b}").is_err());
    }
}

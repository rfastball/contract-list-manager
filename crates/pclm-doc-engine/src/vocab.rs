//! 서식의 칸 이름이 가리킬 수 있는 것(ADR-047) — 통합 뷰의 열과 패널의 내 정보.
//!
//! 통합 뷰는 exe 에 실린 보기 구성(`pclm-core` 의 `presets/builtin-views.json`)의 [`VIEW`] 다. 호스트 `document {op:"read"}`
//! 가 작업자료의 「문서 서식」 정의(사람이 고쳤으면 고친 것, ADR-052 — 없으면 이 파일의 것)를 셈해 열 이름 → 글의 한 줄로
//! 싣고, 엔진은 이 파일에서 열 이름만 읽는다(`include_str`) — 어휘는 exe 의 정의다. 문서에 찍힐 꼴
//! (금액의 「원」 · 공문서 날짜 · 한글 금액 · 오늘)도 뷰가 짓는다 — 엔진은 값을 바꾸지 않는다(ADR-051). 열의 다른 이름과
//! 내 정보는 `vocabulary.json` 에 있고 패널은 `vocabulary` 요청으로 같은 것을 받는다.

use std::sync::OnceLock;

use serde_json::Value;

pub const SOURCE: &str = include_str!("../vocabulary.json");

/// 통합 뷰가 실린 보기 구성 — 호스트가 읽을 때마다 셈하는 그 파일.
pub const VIEWS: &str = include_str!("../../pclm-core/src/storage/presets/builtin-views.json");

/// 보기 구성 안의 통합 뷰 이름.
pub const VIEW: &str = "문서 서식";

#[derive(Clone, Debug)]
pub struct Me {
    pub name: String,
    pub key: String,
    pub aka: Vec<String>,
}

#[derive(Debug)]
pub struct Vocabulary {
    pub missing: String,
    /// 통합 뷰의 열 이름, 차례대로.
    pub columns: Vec<String>,
    /// 통합 뷰의 열 → 그 열의 다른 이름들(옛 서식의 칸 이름).
    pub aka: Vec<(String, Vec<String>)>,
    pub me: Vec<Me>,
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect()
}

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or_default()
}

fn parse(source: &str, views: &str) -> Vocabulary {
    let root: Value = serde_json::from_str(source).expect("vocabulary.json");
    let views: Value = serde_json::from_str(views).expect("builtin-views.json");
    let columns = views["views"]
        .as_array()
        .expect("views")
        .iter()
        .find(|v| v["name"] == VIEW)
        .unwrap_or_else(|| panic!("보기 구성에 「{VIEW}」 가 없습니다"))["definition"]["columns"]
        .as_array()
        .expect("columns")
        .iter()
        .map(|c| text(c, "name").to_owned())
        .collect();
    let aka = root["aka"]
        .as_object()
        .expect("aka")
        .iter()
        .map(|(name, words)| (name.clone(), strings(words)))
        .collect();
    let me = root["me"]
        .as_array()
        .expect("me")
        .iter()
        .map(|m| Me {
            name: text(m, "name").to_owned(),
            key: text(m, "key").to_owned(),
            aka: strings(&m["aka"]),
        })
        .collect();
    Vocabulary {
        missing: text(&root, "missing").to_owned(),
        columns,
        aka,
        me,
    }
}

pub fn vocabulary() -> &'static Vocabulary {
    static V: OnceLock<Vocabulary> = OnceLock::new();
    V.get_or_init(|| parse(SOURCE, VIEWS))
}

/// 이름을 견줄 때는 빈칸·밑줄·가운뎃점·붙임표를 보지 않는다(`계약 금액` = `계약금액`).
pub fn key(name: &str) -> String {
    name.chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, '_' | '·' | '-'))
        .collect()
}

impl Vocabulary {
    /// (이름, 다른 이름들) 모두 — 통합 뷰의 열이 먼저, 그다음 내 정보.
    pub fn all(&self) -> impl Iterator<Item = (&str, &[String])> {
        self.columns
            .iter()
            .map(|c| (c.as_str(), self.aka_of(c)))
            .chain(self.me.iter().map(|m| (m.name.as_str(), m.aka.as_slice())))
    }

    /// 통합 뷰의 열의 다른 이름들.
    fn aka_of(&self, column: &str) -> &[String] {
        self.aka
            .iter()
            .find(|(name, _)| name == column)
            .map_or(&[][..], |(_, words)| words.as_slice())
    }

    /// 서식의 칸 이름 → 가리키는 이름. 어디에도 없으면 `None`(그 칸은 【확인 필요】로 남는다).
    pub fn canonical(&self, field: &str) -> Option<&str> {
        let wanted = key(field);
        if wanted.is_empty() {
            return None;
        }
        self.all()
            .find(|(name, aka)| key(name) == wanted || aka.iter().any(|a| key(a) == wanted))
            .map(|(name, _)| name)
    }

    pub fn group_of(&self, name: &str) -> &str {
        if self.columns.iter().any(|c| c == name) {
            return "통합 뷰";
        }
        if self.me.iter().any(|m| m.name == name) {
            return "내 정보";
        }
        ""
    }

    /// 사람이 읽는 이름 표(Markdown) — `docs/document-vocabulary.md` 의 표 부분. 손으로 고치지 않고
    /// `cargo run -p pclm-doc-engine --example vocabulary_doc` 으로 다시 짓는다(`tests/vocabulary.rs` 가 어긋남을 잡는다).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn markdown(&self) -> String {
        use std::fmt::Write as _;
        let mut out = String::from(
            "<!-- vocab:start — cargo run -p pclm-doc-engine --example vocabulary_doc 이 짓는다 -->\n",
        );
        out.push_str(
            "\n### 통합 뷰의 열 — 호스트가 읽을 때마다 셈해 싣는다\n\n| 이름 | 다른 이름 |\n|---|---|\n",
        );
        for c in &self.columns {
            let _ = writeln!(out, "| {c} | {} |", self.aka_of(c).join(", "));
        }
        out.push_str("\n### 내 정보 — 패널에서 한 번 적는다\n\n| 이름 | 다른 이름 |\n|---|---|\n");
        for m in &self.me {
            let _ = writeln!(out, "| {} | {} |", m.name, m.aka.join(", "));
        }
        out.push_str("\n<!-- vocab:end -->");
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 이름과_다른_이름은_한_곳만_가리킨다() {
        let v = vocabulary();
        let mut seen = std::collections::HashMap::new();
        for (name, aka) in v.all() {
            for word in std::iter::once(name).chain(aka.iter().map(String::as_str)) {
                if let Some(before) = seen.insert(key(word), name) {
                    panic!("「{word}」이 {before} 와 {name} 둘에 걸린다");
                }
            }
        }
    }

    #[test]
    fn 엔진이_기대하는_열은_통합_뷰에_있다() {
        // 파일 이름(naming)은 건명을 읽고, 다른 이름은 통합 뷰의 열에 단다 — 열 이름이 바뀌면 여기서 붉어진다.
        let v = vocabulary();
        assert!(v.columns.iter().any(|c| c == "건명"));
        assert!(!v.aka.is_empty());
        for (name, _) in &v.aka {
            assert!(v.columns.contains(name), "aka: {name}");
        }
    }

    #[test]
    fn 서식의_이름이_통합_뷰의_열을_먼저_찾는다() {
        let v = vocabulary();
        assert_eq!(v.canonical("계약 금액"), Some("계약금액"));
        assert_eq!(v.canonical("담당자 전화번호"), Some("담당자 전화번호"));
        assert_eq!(v.canonical("담당자전화번호"), Some("담당자 전화번호"));
        // 칸 이름이 통합 뷰의 열과 같으면 그 열이다 — 담당자 · 담당부서는 내 정보가 아니라 뷰의 열이다.
        assert_eq!(v.canonical("담당자"), Some("담당자"));
        assert_eq!(v.canonical("담당부서"), Some("담당부서"));
        // 공고서의 「① 계약담당자」 줄은 내 정보다(「② 수요기관 연락처」 줄의 담당자 · 담당부서와 다른 사람).
        assert_eq!(v.canonical("계약담당자"), Some("내_이름"));
        assert_eq!(v.canonical("계약담당 전화번호"), Some("내_연락처"));
        assert_eq!(v.canonical("계약담당 팩스번호"), Some("내_팩스"));
        assert_eq!(v.canonical("한글금액"), Some("계약금액_한글"));
        assert_eq!(v.canonical("작성일"), Some("오늘_년월일"));
        assert_eq!(v.canonical("오늘"), Some("오늘_년월일"));
        assert_eq!(v.canonical("오늘날짜"), Some("오늘_년월"));
        assert_eq!(v.group_of("계약금액_한글"), "통합 뷰");
        assert_eq!(v.canonical("전화번호"), Some("내_연락처"));
        assert_eq!(v.canonical("팩스"), Some("내_팩스"));
        assert_eq!(v.canonical("사업명"), None, "다른 이름은 걷었다");
        assert_eq!(v.canonical(""), None);
    }

    #[test]
    fn 빈_값_표시는_엔진의_표시와_같다() {
        assert_eq!(vocabulary().missing, crate::MISSING);
    }
}

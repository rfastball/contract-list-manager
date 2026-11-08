//! 엑셀 한 권의 덤프(#8 ⑥) — `xlsx-dump <file.xlsx> <out.txt>`. 꼴은 `crates/pclm-parity/README.md` 「엑셀」 이 정본이고, C# 쪽은
//! `tests/Pclm.Parity/XlsxDump.cs` 다.
//!
//! **`pclm-excel` 의 읽개를 쓰지 않는다.** 견줄 것을 지은 쪽의 코드로 읽으면 같은 잘못이 양쪽을 덮는다 — zip 을 열어
//! 부분(part)의 XML 을 그대로 읽는다. 글은 XML 이 정한 대로만 푼다(줄끝 정규화 · 문자 참조 · 기본 개체). 공유 문자열의
//! `_xHHHH_` 는 풀지 않는다.

use std::collections::HashMap;
use std::io::Read as _;

use quick_xml::events::{BytesStart, Event};

/// XML 요소 하나 — 이름(접두사를 뗀 것) · 속성 · 자식 · 글.
#[derive(Debug, Default)]
struct Node {
    name: String,
    attrs: Vec<(String, String)>,
    children: Vec<Node>,
    text: String,
}

impl Node {
    fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name == name)
    }

    fn all<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> {
        self.children.iter().filter(move |c| c.name == name)
    }

    /// 아래의 그 이름 전부(문서 차례).
    fn descendants<'a>(&'a self, name: &str, out: &mut Vec<&'a Node>) {
        for c in &self.children {
            if c.name == name {
                out.push(c);
            }
            c.descendants(name, out);
        }
    }

    /// XElement.Value — 아래 글 전부를 이은 것.
    fn value(&self) -> String {
        let mut out = self.text.clone();
        for c in &self.children {
            out.push_str(&c.value());
        }
        out
    }
}

pub fn run(args: &[String]) -> Result<(), String> {
    let [file, output] = args else {
        return Err("xlsx-dump <file.xlsx> <out.txt>".into());
    };
    let text = dump(file)?;
    std::fs::write(output, text).map_err(|e| e.to_string())
}

fn dump(file: &str) -> Result<String, String> {
    let handle = std::fs::File::open(file).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(handle).map_err(|e| e.to_string())?;
    let mut read = |path: &str| -> Result<Node, String> {
        let mut entry = zip
            .by_name(path)
            .map_err(|_| format!("부분이 없습니다: {path}"))?;
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        parse(&bytes)
    };

    let mut lines = vec!["pclm-parity-xlsx 1".to_string()];
    let workbook_path = target("", &read("_rels/.rels")?, "/officeDocument")
        .ok_or("관계가 없습니다: /officeDocument")?;
    let workbook = read(&workbook_path)?;
    let rels = read(&rels_of(&workbook_path))?;
    let base = dir(&workbook_path);
    let styles = match target(&base, &rels, "/styles") {
        Some(p) => Some(read(&p)?),
        None => None,
    };
    let shared: Vec<String> = match target(&base, &rels, "/sharedStrings") {
        Some(p) => read(&p)?.all("si").map(item_text).collect(),
        None => Vec::new(),
    };

    let sheets = workbook.child("sheets").ok_or("sheets 가 없습니다")?;
    for (i, sheet) in sheets.all("sheet").enumerate() {
        lines.push(format!(
            "sheet\t{}\t{}",
            i + 1,
            escape(sheet.attr("name").unwrap_or("-"))
        ));
        let id = sheet.attr("r:id").or_else(|| {
            sheet
                .attrs
                .iter()
                .find(|(k, _)| k.ends_with(":id"))
                .map(|(_, v)| v.as_str())
        });
        let path = rels
            .all("Relationship")
            .find(|r| r.attr("Id") == id)
            .and_then(|r| r.attr("Target"))
            .map(|t| resolve(&base, t))
            .ok_or("시트의 관계가 없습니다")?;
        let body = read(&path)?;

        let mut panes = Vec::new();
        body.descendants("pane", &mut panes);
        for pane in panes {
            lines.push(format!(
                "pane\t{}",
                attrs(
                    pane,
                    &["xSplit", "ySplit", "topLeftCell", "activePane", "state"]
                )
            ));
        }
        for filter in body.all("autoFilter") {
            lines.push(format!("autofilter\t{}", attrs(filter, &["ref"])));
        }
        for cols in body.all("cols") {
            for col in cols.all("col") {
                lines.push(format!(
                    "col\t{}",
                    attrs(col, &["min", "max", "width", "customWidth", "style"])
                ));
            }
        }

        let mut cells: Vec<&Node> = body
            .all("sheetData")
            .flat_map(|d| d.all("row"))
            .flat_map(|r| r.all("c"))
            .collect();
        cells.sort_by_key(|c| position(c.attr("r").unwrap_or("")));
        for cell in cells {
            let kind = cell.attr("t").unwrap_or("n");
            let style: usize = cell.attr("s").unwrap_or("0").parse().unwrap_or(0);
            let value = match kind {
                "s" => match cell.child("v") {
                    Some(v) => {
                        let index: usize =
                            v.value().trim().parse().map_err(|_| "공유 문자열 번호")?;
                        shared.get(index).cloned().ok_or("공유 문자열이 없습니다")?
                    }
                    None => "-".into(),
                },
                "inlineStr" => cell.child("is").map_or("-".into(), item_text),
                _ => cell.child("v").map_or("-".into(), Node::value),
            };
            let formula = cell
                .child("f")
                .map_or("-".into(), |f| format!("={}", escape(&f.value())));
            lines.push(format!(
                "cell\t{}\t{}\t{}\t{}\t{}",
                attrs(cell, &["r"]),
                kind,
                style_of(styles.as_ref(), style)?,
                formula,
                escape(&value)
            ));
        }
    }

    let mut out = String::new();
    for line in lines {
        out.push_str(&line);
        out.push('\n');
    }
    Ok(out)
}

/// 꼴 다섯 — 수 꼴(사용자 꼴이면 그 글, 아니면 `#번호`), 굵게(1/0), 채움(`무늬[:fgColor]`), 줄바꿈(1/0), 따옴표 머리(1/0).
fn style_of(styles: Option<&Node>, index: usize) -> Result<String, String> {
    let Some(root) = styles else {
        return Ok("#0\t0\tnone\t0\t0".into());
    };
    let xf = root
        .child("cellXfs")
        .and_then(|x| x.all("xf").nth(index))
        .ok_or("꼴이 없습니다")?;
    let num_fmt_id = xf.attr("numFmtId").unwrap_or("0");
    let code = root.child("numFmts").and_then(|n| {
        n.all("numFmt")
            .find(|f| f.attr("numFmtId") == Some(num_fmt_id))
            .and_then(|f| f.attr("formatCode"))
    });
    let font = nth(root, xf, "fonts", "font", "fontId")?;
    let bold = font
        .child("b")
        .is_some_and(|b| matches!(b.attr("val"), None | Some("1") | Some("true")));
    let fill = nth(root, xf, "fills", "fill", "fillId")?.child("patternFill");
    let pattern = fill.and_then(|f| f.attr("patternType")).unwrap_or("none");
    let rgb = fill
        .and_then(|f| f.child("fgColor"))
        .and_then(|c| c.attr("rgb"));
    let on = |v: Option<&str>| matches!(v, Some("1") | Some("true"));
    let wrap = on(xf.child("alignment").and_then(|a| a.attr("wrapText")));
    let quote = on(xf.attr("quotePrefix"));
    Ok(format!(
        "{}\t{}\t{}\t{}\t{}",
        code.map_or_else(|| format!("#{num_fmt_id}"), escape),
        if bold { "1" } else { "0" },
        match rgb {
            Some(rgb) => format!("{pattern}:{rgb}"),
            None => pattern.to_string(),
        },
        u8::from(wrap),
        u8::from(quote)
    ))
}

/// `xf` 의 `key` 번째 `list/item`.
fn nth<'a>(
    root: &'a Node,
    xf: &Node,
    list: &str,
    item: &'a str,
    key: &str,
) -> Result<&'a Node, String> {
    let i: usize = xf.attr(key).unwrap_or("0").parse().unwrap_or(0);
    root.child(list)
        .and_then(|l| l.all(item).nth(i))
        .ok_or_else(|| format!("{list} 의 {i} 번이 없습니다"))
}

/// 공유 문자열 하나 · 인라인 글 — `<t>` 하나거나 런(`<r><t>`)을 이은 것. 발음 표기(rPh)는 뺀다.
fn item_text(item: &Node) -> String {
    match item.child("t") {
        Some(t) => t.value(),
        None => item
            .all("r")
            .map(|r| r.child("t").map(Node::value).unwrap_or_default())
            .collect(),
    }
}

fn target(base: &str, rels: &Node, type_suffix: &str) -> Option<String> {
    rels.all("Relationship")
        .find(|r| r.attr("Type").unwrap_or("").ends_with(type_suffix))
        .and_then(|r| r.attr("Target"))
        .map(|t| resolve(base, t))
}

/// 관계의 대상 — `/` 로 시작하면 꾸러미의 뿌리부터, 아니면 그 부분의 폴더부터.
fn resolve(base: &str, target: &str) -> String {
    match target.strip_prefix('/') {
        Some(rest) => rest.to_string(),
        None => format!("{base}{target}"),
    }
}

fn dir(path: &str) -> String {
    path.rfind('/')
        .map_or(String::new(), |i| path[..=i].to_string())
}

fn rels_of(path: &str) -> String {
    let name = path.rfind('/').map_or(path, |i| &path[i + 1..]);
    format!("{}_rels/{name}.rels", dir(path))
}

fn attrs(node: &Node, names: &[&str]) -> String {
    names
        .iter()
        .map(|n| node.attr(n).map_or("-".into(), escape))
        .collect::<Vec<_>>()
        .join("\t")
}

/// 칸 차례 — 줄, 그다음 열.
fn position(reference: &str) -> (u32, u32) {
    let letters = reference
        .bytes()
        .take_while(u8::is_ascii_alphabetic)
        .count();
    let column = reference[..letters].bytes().fold(0u32, |n, b| {
        n * 26 + u32::from(b.to_ascii_uppercase() - b'A' + 1)
    });
    (reference[letters..].parse().unwrap_or(0), column)
}

/// `\` · 탭 · 줄바꿈 · 그 밖의 제어 문자를 글자로 — 한 칸이 한 줄에 선다(UTF-16 단위로 보는 C# 과 같은 꼴).
fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 || c == '\u{7f}' => {
                out.push_str(&format!("\\u{:04X}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

// ── XML ─────────────────────────────────────────────────────────

/// 문서 하나를 나무로. 글은 XML 1.0 이 정한 대로 — 날 글의 줄끝(`\r\n` · `\r`)은 `\n` 으로, 그 뒤 참조를 푼다.
/// 문자 참조(`&#13;`)로 적은 `\r` 은 그대로 남는다(XmlReader 와 같다). 공백만 있는 글도 지킨다(PreserveWhitespace).
fn parse(bytes: &[u8]) -> Result<Node, String> {
    let mut reader = quick_xml::Reader::from_reader(bytes);
    let config = reader.config_mut();
    config.trim_text(false);
    config.expand_empty_elements = false;
    let mut stack: Vec<Node> = vec![Node::default()];
    let mut buf = Vec::new();
    loop {
        let event = reader
            .read_event_into(&mut buf)
            .map_err(|e| e.to_string())?;
        match event {
            Event::Start(e) => stack.push(start(&e)?),
            Event::Empty(e) => {
                let node = start(&e)?;
                stack.last_mut().ok_or("깨진 XML")?.children.push(node);
            }
            Event::End(_) => {
                let node = stack.pop().ok_or("깨진 XML")?;
                stack.last_mut().ok_or("깨진 XML")?.children.push(node);
            }
            Event::Text(t) => {
                let raw =
                    String::from_utf8(t.into_inner().into_owned()).map_err(|e| e.to_string())?;
                let text = unescape(&normalize_line_ends(&raw))?;
                push_text(&mut stack, &text)?;
            }
            Event::GeneralRef(r) => {
                let name =
                    String::from_utf8(r.into_inner().into_owned()).map_err(|e| e.to_string())?;
                let text = unescape(&format!("&{name};"))?;
                push_text(&mut stack, &text)?;
            }
            Event::CData(c) => {
                let raw =
                    String::from_utf8(c.into_inner().into_owned()).map_err(|e| e.to_string())?;
                push_text(&mut stack, &normalize_line_ends(&raw))?;
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    let mut root = stack.pop().ok_or("깨진 XML")?;
    if !stack.is_empty() {
        return Err("닫히지 않은 요소".into());
    }
    root.children.pop().ok_or_else(|| "뿌리가 없습니다".into())
}

/// 글은 마지막 자식 뒤가 아니라 그 요소의 글로 모은다 — 여기서 읽는 요소(`t` · `v` · `f`)는 글만 품는다.
fn push_text(stack: &mut [Node], text: &str) -> Result<(), String> {
    stack.last_mut().ok_or("깨진 XML")?.text.push_str(text);
    Ok(())
}

fn start(e: &BytesStart<'_>) -> Result<Node, String> {
    let name = String::from_utf8(e.local_name().as_ref().to_vec()).map_err(|x| x.to_string())?;
    let mut attrs = Vec::new();
    for a in e.attributes() {
        let a = a.map_err(|x| x.to_string())?;
        let key = String::from_utf8(a.key.as_ref().to_vec()).map_err(|x| x.to_string())?;
        // 속성 값 정규화(XML 1.0 3.3.3): 날 공백 문자는 빈칸으로, 그 뒤 참조를 푼다.
        let raw = String::from_utf8(a.value.into_owned()).map_err(|x| x.to_string())?;
        let spaced: String = normalize_line_ends(&raw)
            .chars()
            .map(|c| if matches!(c, '\t' | '\n') { ' ' } else { c })
            .collect();
        // 이름은 접두사를 뗀다 — r:id 만은 그대로 둔다(같은 이름의 id 와 가르려고).
        let key = if key.contains(':') && !key.starts_with("xmlns") && !key.ends_with(":id") {
            key.rsplit(':').next().unwrap_or(&key).to_string()
        } else if key.ends_with(":id") {
            "r:id".to_string()
        } else {
            key
        };
        attrs.push((key, unescape(&spaced)?));
    }
    Ok(Node {
        name,
        attrs,
        children: Vec::new(),
        text: String::new(),
    })
}

fn normalize_line_ends(raw: &str) -> String {
    raw.replace("\r\n", "\n").replace('\r', "\n")
}

/// 기본 개체 다섯과 문자 참조.
fn unescape(text: &str) -> Result<String, String> {
    if !text.contains('&') {
        return Ok(text.to_string());
    }
    let named: HashMap<&str, char> = [
        ("lt", '<'),
        ("gt", '>'),
        ("amp", '&'),
        ("quot", '"'),
        ("apos", '\''),
    ]
    .into_iter()
    .collect();
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let end = after.find(';').ok_or("닫히지 않은 참조")?;
        let name = &after[..end];
        let ch = if let Some(hex) = name.strip_prefix("#x") {
            u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
        } else if let Some(dec) = name.strip_prefix('#') {
            dec.parse().ok().and_then(char::from_u32)
        } else {
            named.get(name).copied()
        };
        out.push(ch.ok_or_else(|| format!("모르는 참조: {name}"))?);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

//! 계획 엑셀 읽기 — ClosedXML 0.105.1 이 **첫 워크시트**를 읽어 칸마다 두는 값과 같은 값(`XLWorkbook_Load`).
//!
//! 엑셀 읽기 라이브러리(calamine 따위)를 쓰지 않는 까닭: C# 이 칸을 글로 바꾸는 길(`PlanImport.Raw`)은 ClosedXML 이 그 칸을
//! **무슨 형으로 읽었는지**에 달렸다. ClosedXML 은 수 칸의 형을 꼴 번호와 꼴 글(`numFmt`)로 가르는데(14·15·16·22 는 날짜,
//! 18~21·45~47 은 시간, 사용자 꼴은 첫 `y`·`d`·`h`·`s`·`m` 으로), 라이브러리들의 날짜 판정은 그와 다르고 꼴 번호를 내주지
//! 않는다. 공유 문자열의 `_xHHHH_` 풀기와 줄바꿈 고치기(`FixNewLines`)도 ClosedXML 의 것이 따로 있다.
//!
//! 옮기지 않은 것: 값이 저장되지 않은 수식(ClosedXML 은 그 자리에서 계산한다 — 여기서는 빈 칸이다), 1904 날짜 체계의 시간,
//! 셀 메타데이터. 수식은 저장된 값을 그대로 쓴다(ClosedXML 도 그렇다).

use std::collections::{BTreeMap, HashMap};
use std::io::{Read, Seek};
use std::path::Path;

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::dotnet::{self, DateTime};

/// 칸 하나의 값 — ClosedXML `XLCellValue` 의 형.
#[derive(Clone, Debug, PartialEq)]
pub enum CellValue {
    Blank,
    Number(f64),
    Text(String),
    Boolean(bool),
    Error(&'static str),
    /// 날짜 꼴의 수 — 꺼낼 때 `FromOADate` 로 바꾼다(그때 실패할 수 있다).
    DateSerial(f64),
    /// 이미 날짜로 읽은 것(`t="d"` · 1904 체계).
    Date(DateTime),
    /// 시간 꼴의 수.
    TimeSpan(f64),
}

/// 칸 하나.
#[derive(Clone, Debug, PartialEq)]
pub struct CellData {
    pub value: CellValue,
    pub formula: bool,
    pub comment: bool,
}

impl CellData {
    /// `IsEmpty(XLCellsUsedOptions.AllContents)` 의 반대 — 빈 글이 아닌 값·수식·메모가 있는가.
    pub fn used(&self) -> bool {
        let empty = match &self.value {
            CellValue::Blank => true,
            CellValue::Text(t) => t.is_empty(),
            _ => false,
        };
        !empty || self.formula || self.comment
    }
}

/// 첫 워크시트. 칸은 (줄, 열) 차례 — 1부터.
#[derive(Debug, Default)]
pub struct Worksheet {
    pub cells: BTreeMap<(u32, u32), CellData>,
}

impl Worksheet {
    pub fn cell(&self, row: u32, column: u32) -> Option<&CellData> {
        self.cells.get(&(row, column))
    }

    /// `FirstRowUsed()` 의 줄 번호.
    pub fn first_row_used(&self) -> Option<u32> {
        self.cells
            .iter()
            .find(|(_, c)| c.used())
            .map(|((r, _), _)| *r)
    }

    /// `LastRowUsed()` 의 줄 번호.
    pub fn last_row_used(&self) -> Option<u32> {
        self.cells
            .iter()
            .rev()
            .find(|(_, c)| c.used())
            .map(|((r, _), _)| *r)
    }

    /// 그 줄의 `LastCellUsed()` 열 번호.
    pub fn last_column_used(&self, row: u32) -> Option<u32> {
        self.cells
            .range((row, 0)..=(row, u32::MAX))
            .rev()
            .find(|(_, c)| c.used())
            .map(|((_, c), _)| *c)
    }
}

/// 엑셀 파일을 열지 못한 까닭. C# 은 ClosedXML·System.IO 의 예외 글을 낸다 — 없는 파일만 같은 글이고, 그 밖의 깨진 파일은
/// 글이 다르다(README 「견주지 않는 것」).
#[derive(Debug)]
pub enum ReadError {
    /// `FileNotFoundException` — `Could not find file '…'.`
    NotFound(String),
    /// 그 밖의 읽기 실패.
    Invalid(String),
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadError::NotFound(m) | ReadError::Invalid(m) => f.write_str(m),
        }
    }
}

/// `new XLWorkbook(path).Worksheets.FirstOrDefault()` — 워크시트가 없으면 `None`.
pub fn first_worksheet(path: &Path) -> Result<Option<Worksheet>, ReadError> {
    let full = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(ReadError::NotFound(format!(
                "Could not find file '{}'.",
                full.display()
            )));
        }
        Err(e) => return Err(ReadError::Invalid(e.to_string())),
    };
    let mut package = Package::open(file)?;
    package.first_worksheet()
}

fn invalid(message: impl Into<String>) -> ReadError {
    ReadError::Invalid(message.into())
}

struct Package<R: Read + Seek> {
    zip: zip::ZipArchive<R>,
    names: Vec<String>,
}

impl<R: Read + Seek> Package<R> {
    fn open(reader: R) -> Result<Self, ReadError> {
        let zip = zip::ZipArchive::new(reader)
            .map_err(|e| invalid(format!("엑셀 파일(xlsx)로 읽지 못했습니다: {e}")))?;
        let names = zip
            .file_names()
            .filter_map(|n| n.ok().map(|n| n.into_owned()))
            .collect();
        Ok(Package { zip, names })
    }

    fn part(&mut self, name: &str) -> Result<Option<Vec<u8>>, ReadError> {
        let found = self
            .names
            .iter()
            .find(|n| n.as_str() == name)
            .or_else(|| self.names.iter().find(|n| n.eq_ignore_ascii_case(name)))
            .cloned();
        let Some(found) = found else {
            return Ok(None);
        };
        let mut entry = self
            .zip
            .by_name(&found)
            .map_err(|e| invalid(e.to_string()))?;
        let mut bytes = Vec::new();
        entry
            .read_to_end(&mut bytes)
            .map_err(|e| invalid(e.to_string()))?;
        Ok(Some(bytes))
    }

    /// 부분 하나의 관계(`Id → (Type, 대상 부분 이름)`).
    fn relationships(&mut self, part: &str) -> Result<Vec<(String, String, String)>, ReadError> {
        let (dir, file) = match part.rfind('/') {
            Some(i) => (&part[..i], &part[i + 1..]),
            None => ("", part),
        };
        let rels = if dir.is_empty() {
            format!("_rels/{file}.rels")
        } else {
            format!("{dir}/_rels/{file}.rels")
        };
        let Some(bytes) = self.part(&rels)? else {
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        let mut reader = Reader::from_reader(bytes.as_slice());
        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf).map_err(xml)? {
                Event::Start(e) | Event::Empty(e) if e.local_name().as_ref() == b"Relationship" => {
                    let id = attr(&e, b"Id").unwrap_or_default();
                    let kind = attr(&e, b"Type").unwrap_or_default();
                    let target = attr(&e, b"Target").unwrap_or_default();
                    let mode = attr(&e, b"TargetMode").unwrap_or_default();
                    if mode != "External" {
                        out.push((id, kind, resolve(dir, &target)));
                    }
                }
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }
        Ok(out)
    }

    fn first_worksheet(&mut self) -> Result<Option<Worksheet>, ReadError> {
        let root = self.relationships("")?;
        let workbook = root
            .iter()
            .find(|(_, kind, _)| kind.ends_with("/officeDocument"))
            .map(|(_, _, t)| t.clone())
            .ok_or_else(|| invalid("엑셀 파일(xlsx)로 읽지 못했습니다: 통합 문서가 없습니다."))?;
        let bytes = self
            .part(&workbook)?
            .ok_or_else(|| invalid("엑셀 파일(xlsx)로 읽지 못했습니다: 통합 문서가 없습니다."))?;
        let rels = self.relationships(&workbook)?;

        // 시트 차례와 1904 체계.
        let mut sheet_ids = Vec::new();
        let mut date1904 = false;
        let mut reader = Reader::from_reader(bytes.as_slice());
        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf).map_err(xml)? {
                Event::Start(e) | Event::Empty(e) => match e.local_name().as_ref() {
                    b"sheet" => {
                        if let Some(id) = attr_ns(&e, b"id") {
                            sheet_ids.push(id);
                        }
                    }
                    b"workbookPr" => {
                        date1904 = attr(&e, b"date1904").is_some_and(|v| v == "1" || v == "true");
                    }
                    _ => {}
                },
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }

        let target = |kind: &str| {
            rels.iter()
                .find(|(_, k, _)| k.ends_with(kind))
                .map(|(_, _, t)| t.clone())
        };
        let shared = match target("/sharedStrings") {
            Some(part) => match self.part(&part)? {
                Some(bytes) => shared_strings(&bytes)?,
                None => Vec::new(),
            },
            None => Vec::new(),
        };
        let styles = match target("/styles") {
            Some(part) => match self.part(&part)? {
                Some(bytes) => Some(Styles::parse(&bytes)?),
                None => None,
            },
            None => None,
        };

        // Worksheets 는 워크시트만 센다(차트 시트는 들지 않는다).
        let Some(sheet_part) = sheet_ids.iter().find_map(|id| {
            rels.iter()
                .find(|(rid, kind, _)| rid == id && kind.ends_with("/worksheet"))
                .map(|(_, _, t)| t.clone())
        }) else {
            return Ok(None);
        };
        let sheet_bytes = self
            .part(&sheet_part)?
            .ok_or_else(|| invalid(format!("시트가 없습니다: {sheet_part}")))?;
        let mut comments = Vec::new();
        for (_, kind, part) in self.relationships(&sheet_part)? {
            if kind.ends_with("/comments")
                && let Some(bytes) = self.part(&part)?
            {
                comments.extend(comment_refs(&bytes)?);
            }
        }
        let context = Context {
            shared: &shared,
            styles: styles.as_ref(),
            date1904,
        };
        let mut sheet = read_sheet(&sheet_bytes, &context)?;
        for (row, column) in comments {
            sheet
                .cells
                .entry((row, column))
                .or_insert(CellData {
                    value: CellValue::Blank,
                    formula: false,
                    comment: false,
                })
                .comment = true;
        }
        Ok(Some(sheet))
    }
}

fn xml(e: quick_xml::Error) -> ReadError {
    invalid(format!("엑셀 파일(xlsx)의 XML 을 읽지 못했습니다: {e}"))
}

/// 관계의 대상 — `/` 로 시작하면 꾸러미 뿌리에서, 아니면 그 부분의 폴더에서.
fn resolve(dir: &str, target: &str) -> String {
    let mut parts: Vec<&str> = if target.starts_with('/') {
        Vec::new()
    } else {
        dir.split('/').filter(|p| !p.is_empty()).collect()
    };
    for piece in target.split('/') {
        match piece {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            p => parts.push(p),
        }
    }
    parts.join("/")
}

/// 이름공간 없는 속성 하나(로컬 이름으로 찾는다).
fn attr(e: &BytesStart<'_>, name: &[u8]) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        (a.key.local_name().as_ref() == name && a.key.prefix().is_none())
            .then(|| a.unescape_value().ok().map(|v| v.into_owned()))
            .flatten()
    })
}

/// 이름공간이 붙은 속성(`r:id`).
fn attr_ns(e: &BytesStart<'_>, name: &[u8]) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        (a.key.local_name().as_ref() == name && a.key.prefix().is_some())
            .then(|| a.unescape_value().ok().map(|v| v.into_owned()))
            .flatten()
    })
}

/// 요소 하나의 글 내용 전부(아래 요소 안의 것까지 — `InnerText`). `start` 를 읽은 직후에 부른다.
fn inner_text(reader: &mut Reader<&[u8]>, buf: &mut Vec<u8>) -> Result<String, ReadError> {
    let mut out = String::new();
    let mut depth = 1usize;
    loop {
        match reader.read_event_into(buf).map_err(xml)? {
            Event::Start(_) => depth += 1,
            Event::End(_) => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            Event::Text(t) => out.push_str(&t.xml10_content().map_err(|e| invalid(e.to_string()))?),
            Event::CData(t) => {
                let raw = t.decode().map_err(|e| invalid(e.to_string()))?;
                out.push_str(&raw.replace("\r\n", "\n").replace('\r', "\n"));
            }
            Event::GeneralRef(r) => {
                if let Some(c) = r.resolve_char_ref().map_err(|e| invalid(e.to_string()))? {
                    out.push(c);
                } else {
                    let name = r.decode().map_err(|e| invalid(e.to_string()))?;
                    out.push_str(match name.as_ref() {
                        "lt" => "<",
                        "gt" => ">",
                        "amp" => "&",
                        "quot" => "\"",
                        "apos" => "'",
                        other => return Err(invalid(format!("모르는 엔터티: {other}"))),
                    });
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(out)
}

/// 문자열 항목(`<si>`·`<is>`) 하나 — `<r>` 들과 바로 아래 `<t>`. 발음 표기(`<rPh>`)는 값에 들지 않는다.
struct Rst {
    plain: Option<String>,
    runs: Vec<String>,
}

fn read_rst(reader: &mut Reader<&[u8]>, buf: &mut Vec<u8>) -> Result<Rst, ReadError> {
    let mut rst = Rst {
        plain: None,
        runs: Vec::new(),
    };
    loop {
        match reader.read_event_into(buf).map_err(xml)? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"t" => {
                    let text = inner_text(reader, &mut Vec::new())?;
                    if rst.plain.is_none() {
                        rst.plain = Some(text);
                    }
                }
                b"r" => rst.runs.push(run_text(reader)?),
                _ => skip(reader)?,
            },
            Event::Empty(e) => match e.local_name().as_ref() {
                b"t" => {
                    if rst.plain.is_none() {
                        rst.plain = Some(String::new());
                    }
                }
                b"r" => rst.runs.push(String::new()),
                _ => {}
            },
            Event::End(_) | Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(rst)
}

/// `<r>` 하나의 `<t>` 글.
fn run_text(reader: &mut Reader<&[u8]>) -> Result<String, ReadError> {
    let mut text = String::new();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf).map_err(xml)? {
            Event::Start(e) if e.local_name().as_ref() == b"t" => {
                text = inner_text(reader, &mut Vec::new())?;
            }
            Event::Start(_) => skip(reader)?,
            Event::End(_) | Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(text)
}

/// 방금 연 요소를 끝까지 건너뛴다.
fn skip(reader: &mut Reader<&[u8]>) -> Result<(), ReadError> {
    let mut depth = 1usize;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf).map_err(xml)? {
            Event::Start(_) => depth += 1,
            Event::End(_) => {
                depth -= 1;
                if depth == 0 {
                    return Ok(());
                }
            }
            Event::Eof => return Ok(()),
            _ => {}
        }
        buf.clear();
    }
}

impl Rst {
    /// ClosedXML `SetCellText` — 조각이 있으면 조각마다 `FixNewLines` 해 잇고, 없으면 `<t>` 를 `DecodeString` 한다.
    fn cell_text(&self) -> String {
        if !self.runs.is_empty() {
            self.runs.iter().map(|r| fix_new_lines(r)).collect()
        } else {
            decode_string(self.plain.as_deref())
        }
    }
}

fn shared_strings(bytes: &[u8]) -> Result<Vec<String>, ReadError> {
    let mut out = Vec::new();
    let mut reader = Reader::from_reader(bytes);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf).map_err(xml)? {
            Event::Start(e) if e.local_name().as_ref() == b"si" => {
                out.push(read_rst(&mut reader, &mut Vec::new())?.cell_text());
            }
            Event::Empty(e) if e.local_name().as_ref() == b"si" => out.push(String::new()),
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(out)
}

/// 메모가 달린 칸들.
fn comment_refs(bytes: &[u8]) -> Result<Vec<(u32, u32)>, ReadError> {
    let mut out = Vec::new();
    let mut reader = Reader::from_reader(bytes);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf).map_err(xml)? {
            Event::Start(e) | Event::Empty(e) if e.local_name().as_ref() == b"comment" => {
                if let Some(at) = attr(&e, b"ref").as_deref().and_then(cell_ref) {
                    out.push(at);
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(out)
}

/// `A1` → (1, 1).
pub fn cell_ref(text: &str) -> Option<(u32, u32)> {
    let letters = text.bytes().take_while(u8::is_ascii_alphabetic).count();
    if letters == 0 || letters > 3 {
        return None;
    }
    let mut column = 0u32;
    for b in text[..letters].bytes() {
        column = column * 26 + u32::from(b.to_ascii_uppercase() - b'A' + 1);
    }
    let row: u32 = text[letters..].parse().ok()?;
    (row >= 1 && column >= 1).then_some((row, column))
}

/// 꼴 — `cellXfs` 의 꼴 번호와 `numFmts` 의 꼴 글.
struct Styles {
    xfs: Vec<Option<u32>>,
    formats: HashMap<u32, String>,
}

impl Styles {
    fn parse(bytes: &[u8]) -> Result<Styles, ReadError> {
        let mut xfs = Vec::new();
        let mut formats = HashMap::new();
        let mut reader = Reader::from_reader(bytes);
        let mut buf = Vec::new();
        let mut in_cell_xfs = false;
        loop {
            match reader.read_event_into(&mut buf).map_err(xml)? {
                Event::Start(e) | Event::Empty(e) if e.local_name().as_ref() == b"numFmt" => {
                    if let (Some(id), Some(code)) = (
                        attr(&e, b"numFmtId").and_then(|v| v.parse().ok()),
                        attr(&e, b"formatCode"),
                    ) {
                        // FirstOrDefault — 같은 번호가 둘이면 앞의 것.
                        formats.entry(id).or_insert(code);
                    }
                }
                Event::Start(e) if e.local_name().as_ref() == b"cellXfs" => in_cell_xfs = true,
                Event::End(e) if e.local_name().as_ref() == b"cellXfs" => in_cell_xfs = false,
                Event::Start(e) | Event::Empty(e)
                    if in_cell_xfs && e.local_name().as_ref() == b"xf" =>
                {
                    xfs.push(attr(&e, b"numFmtId").and_then(|v| v.parse().ok()));
                }
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }
        Ok(Styles { xfs, formats })
    }

    /// 꼴 번호 `s` 의 (번호, 꼴 글). 사용자 꼴 글이 있으면 번호는 -1 이다(`XLNumberFormatKey.ForFormat`).
    fn number_format(&self, style: usize) -> Result<(i64, String), ReadError> {
        let Some(xf) = self.xfs.get(style) else {
            return Err(invalid(
                "Specified argument was out of the range of valid values. (Parameter 'index')",
            ));
        };
        let Some(id) = xf else {
            return Ok((0, String::new()));
        };
        match self.formats.get(id) {
            Some(code) if !code.is_empty() => Ok((-1, code.clone())),
            _ => Ok((i64::from(*id), String::new())),
        }
    }
}

struct Context<'a> {
    shared: &'a [String],
    styles: Option<&'a Styles>,
    date1904: bool,
}

/// 형 셋 — 수 칸이 무엇으로 읽히는가(`GetNumberDataType`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum NumberKind {
    Number,
    DateTime,
    TimeSpan,
}

fn number_kind(id: i64, format: &str) -> NumberKind {
    if matches!(id, 18..=21 | 45..=47) {
        return NumberKind::TimeSpan;
    }
    if matches!(id, 14 | 15 | 16 | 22) {
        return NumberKind::DateTime;
    }
    if !format.chars().all(char::is_whitespace) {
        return kind_from_format(format).unwrap_or(NumberKind::Number);
    }
    NumberKind::Number
}

/// `GetDataTypeFromFormat`. 따옴표가 닫히지 않은 꼴에서 C# 은 처음으로 돌아가 끝없이 돈다 — 여기서는 수로 본다.
fn kind_from_format(format: &str) -> Option<NumberKind> {
    let f: Vec<char> = format.chars().map(|c| c.to_ascii_lowercase()).collect();
    let length = f.len();
    let mut i = 0usize;
    while i < length {
        let c = f[i];
        if c == '"' {
            match f[i + 1..].iter().position(|&x| x == '"') {
                Some(p) => i = i + 1 + p,
                None => return None,
            }
        } else if c == '[' {
            match f[i + 1..].iter().position(|&x| x == ']') {
                Some(p) => i = i + 1 + p,
                None => return None,
            }
        } else if c == '0' || c == '#' || c == '?' {
            return Some(NumberKind::Number);
        } else if c == 'y' || c == 'd' {
            return Some(NumberKind::DateTime);
        } else if c == 'h' || c == 's' {
            return Some(NumberKind::TimeSpan);
        } else if c == 'm' {
            for &n in &f[i + 1..] {
                if n == 'm' {
                    continue;
                } else if n == 's' {
                    return Some(NumberKind::TimeSpan);
                } else if n.is_ascii_lowercase() || n.is_ascii_digit() {
                    return Some(NumberKind::DateTime);
                }
            }
            return Some(NumberKind::DateTime);
        }
        i += 1;
    }
    None
}

fn read_sheet(bytes: &[u8], context: &Context<'_>) -> Result<Worksheet, ReadError> {
    let mut sheet = Worksheet::default();
    let mut reader = Reader::from_reader(bytes);
    let mut buf = Vec::new();
    let mut last_row = 0u32;
    let mut last_column = 0u32;
    let mut row = 0u32;
    loop {
        match reader.read_event_into(&mut buf).map_err(xml)? {
            Event::Start(e) | Event::Empty(e) if e.local_name().as_ref() == b"row" => {
                row = match attr(&e, b"r").and_then(|r| r.parse().ok()) {
                    Some(r) => r,
                    None => last_row + 1,
                };
                last_row = row;
                last_column = 0;
            }
            Event::Start(e) if e.local_name().as_ref() == b"c" => {
                let (at, cell) = read_cell(&e, false, &mut reader, context, row, last_column)?;
                last_column = at.1;
                sheet.cells.insert(at, cell);
            }
            Event::Empty(e) if e.local_name().as_ref() == b"c" => {
                let (at, cell) = read_cell(&e, true, &mut reader, context, row, last_column)?;
                last_column = at.1;
                sheet.cells.insert(at, cell);
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(sheet)
}

fn read_cell(
    start: &BytesStart<'_>,
    empty: bool,
    reader: &mut Reader<&[u8]>,
    context: &Context<'_>,
    row: u32,
    last_column: u32,
) -> Result<((u32, u32), CellData), ReadError> {
    let style: usize = attr(start, b"s").and_then(|s| s.parse().ok()).unwrap_or(0);
    let at = attr(start, b"r")
        .as_deref()
        .and_then(cell_ref)
        .unwrap_or((row, last_column + 1));
    let kind = attr(start, b"t");
    match kind.as_deref() {
        None | Some("b" | "n" | "e" | "s" | "str" | "inlineStr" | "d") => {}
        Some(_) => return Err(invalid("Unknown cell type.")),
    }
    let number_format = match context.styles {
        Some(styles) => styles.number_format(style)?,
        None => (0, String::new()),
    };

    let mut formula = false;
    let mut value_text: Option<String> = None;
    let mut inline: Option<Rst> = None;
    if !empty {
        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf).map_err(xml)? {
                Event::Start(e) => match e.local_name().as_ref() {
                    b"f" => {
                        formula = true;
                        skip(reader)?;
                    }
                    b"v" => value_text = Some(inner_text(reader, &mut Vec::new())?),
                    b"is" => inline = Some(read_rst(reader, &mut Vec::new())?),
                    _ => skip(reader)?,
                },
                Event::Empty(e) => match e.local_name().as_ref() {
                    b"f" => formula = true,
                    b"v" => value_text = Some(String::new()),
                    b"is" => {
                        inline = Some(Rst {
                            plain: None,
                            runs: Vec::new(),
                        })
                    }
                    _ => {}
                },
                Event::End(_) | Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }
    }

    let mut value = CellValue::Blank;
    match kind.as_deref() {
        None | Some("n") => {
            if let Some(number) = value_text.as_deref().and_then(parse_double) {
                value = match number_kind(number_format.0, &number_format.1) {
                    NumberKind::DateTime => CellValue::DateSerial(number),
                    NumberKind::TimeSpan => CellValue::TimeSpan(number),
                    NumberKind::Number => CellValue::Number(number),
                };
            }
        }
        Some("s") => {
            value = CellValue::Text(match value_text.as_deref() {
                Some(v) => v
                    .trim()
                    .parse::<usize>()
                    .ok()
                    .and_then(|i| context.shared.get(i))
                    .cloned()
                    .unwrap_or_default(),
                None => String::new(),
            });
        }
        Some("str") => value = CellValue::Text(value_text.unwrap_or_default()),
        Some("b") => {
            if let Some(v) = value_text {
                value = CellValue::Boolean(v == "1" || v.eq_ignore_ascii_case("TRUE"));
            }
        }
        Some("e") => {
            if let Some(error) = value_text.as_deref().and_then(error_value) {
                value = CellValue::Error(error);
            }
        }
        Some("d") => {
            if let Some(v) = value_text {
                value = CellValue::Date(parse_date_cell(&v)?);
            }
        }
        Some("inlineStr") => {
            if let Some(rst) = inline {
                value = CellValue::Text(match rst.plain {
                    // inlineString.Text.Text.FixNewLines() — 풀지(DecodeString) 않는다.
                    Some(t) => fix_new_lines(&t),
                    None => rst.cell_text(),
                });
            }
        }
        _ => {}
    }

    if context.date1904 {
        let date = match &value {
            CellValue::DateSerial(serial) => Some(dotnet::serial_date(*serial).map_err(invalid)?),
            CellValue::Date(d) => Some(*d),
            _ => None,
        };
        if let Some(d) = date {
            value = CellValue::Date(DateTime::from_millis(d.millis() + 1462 * 86_400_000));
        }
    }

    Ok((
        at,
        CellData {
            value,
            formula,
            comment: false,
        },
    ))
}

/// `double.TryParse(…, Float | AllowThousands, Invariant)` 의 흔한 꼴.
fn parse_double(text: &str) -> Option<f64> {
    let t = text.trim();
    match t {
        "Infinity" => return Some(f64::INFINITY),
        "-Infinity" => return Some(f64::NEG_INFINITY),
        "NaN" => return Some(f64::NAN),
        _ => {}
    }
    if t.is_empty()
        || !t
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'+' | b'-' | b'.' | b'e' | b'E'))
    {
        return None;
    }
    t.parse().ok()
}

/// `XLErrorParser.TryParseError`.
fn error_value(text: &str) -> Option<&'static str> {
    [
        "#NULL!", "#DIV/0!", "#VALUE!", "#REF!", "#NAME?", "#NUM!", "#N/A",
    ]
    .into_iter()
    .find(|e| *e == text)
}

/// `t="d"` 칸 — `DateTime.ParseExact(v, ["yyyy-MM-ddTHH:mm:ss.fff", "yyyy-MM-ddTHH:mm", "yyyy-MM-dd"], …, AllowLeadingWhite |
/// AllowTrailingWhite)`. 읽지 못하면 C# 은 통합 문서를 열지 못한다.
fn parse_date_cell(text: &str) -> Result<DateTime, ReadError> {
    let t = text.trim_matches(|c: char| c.is_whitespace());
    let digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    let num = |s: &str| s.parse::<u32>().ok();
    let fail = || {
        invalid(format!(
            "String '{text}' was not recognized as a valid DateTime."
        ))
    };
    let b = t.as_bytes();
    let parse = || -> Option<DateTime> {
        if b.len() < 10 || b[4] != b'-' || b[7] != b'-' {
            return None;
        }
        let (y, m, d) = (&t[0..4], &t[5..7], &t[8..10]);
        if !digits(y) || !digits(m) || !digits(d) {
            return None;
        }
        let mut dt = DateTime {
            year: i64::from(num(y)?),
            month: num(m)?,
            day: num(d)?,
            hour: 0,
            minute: 0,
            second: 0,
            millisecond: 0,
        };
        let rest = &t[10..];
        if !rest.is_empty() {
            let r = rest.as_bytes();
            let time = |s: &str| digits(s).then(|| num(s)).flatten();
            match r.len() {
                6 if r[0] == b'T' && r[3] == b':' => {
                    dt.hour = time(&rest[1..3])?;
                    dt.minute = time(&rest[4..6])?;
                }
                13 if r[0] == b'T' && r[3] == b':' && r[6] == b':' && r[9] == b'.' => {
                    dt.hour = time(&rest[1..3])?;
                    dt.minute = time(&rest[4..6])?;
                    dt.second = time(&rest[7..9])?;
                    dt.millisecond = time(&rest[10..13])?;
                }
                _ => return None,
            }
        }
        let valid = (1..=12).contains(&dt.month)
            && dt.day >= 1
            && dt.day <= days_in_month(dt.year, dt.month)
            && dt.year >= 1
            && dt.hour < 24
            && dt.minute < 60
            && dt.second < 60;
        valid.then_some(dt)
    };
    parse().ok_or_else(fail)
}

fn days_in_month(year: i64, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        _ => 28,
    }
}

/// ClosedXML `FixNewLines` — 앞에 `\r` 가 없는 `\n` 과 `\r\n` 을 `Environment.NewLine`(`\r\n`)으로.
pub fn fix_new_lines(value: &str) -> String {
    if !value.contains('\n') {
        return value.to_owned();
    }
    let mut out = String::with_capacity(value.len() + 8);
    let mut previous = '\0';
    for c in value.chars() {
        if c == '\n' && previous != '\r' {
            out.push_str("\r\n");
        } else {
            out.push(c);
        }
        previous = c;
    }
    out
}

/// ClosedXML `XmlEncoder.DecodeString` — `_X` 로 시작하는 넷자리 꼴을 막은 뒤 `XmlConvert.DecodeName`(`_xHHHH_`·
/// `_xHHHHHHHH_`, x 는 대소문자 모두).
pub fn decode_string(value: Option<&str>) -> String {
    let Some(value) = value.filter(|v| !v.is_empty()) else {
        return String::new();
    };
    let chars: Vec<char> = value.chars().collect();
    let hex = |range: std::ops::Range<usize>| {
        range
            .clone()
            .all(|k| chars.get(k).is_some_and(char::is_ascii_hexdigit))
    };
    // 1단계: `_(X[\dA-Fa-f]{4})_` → `_x005F_$1_`.
    let mut guarded: Vec<char> = Vec::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '_'
            && chars.get(i + 1) == Some(&'X')
            && hex(i + 2..i + 6)
            && chars.get(i + 6) == Some(&'_')
        {
            guarded.extend("_x005F_".chars());
            guarded.extend(&chars[i + 1..i + 6]);
            guarded.push('_');
            i += 7;
            continue;
        }
        guarded.push(chars[i]);
        i += 1;
    }
    // 2단계: DecodeName.
    let chars = guarded;
    let hex = |range: std::ops::Range<usize>| {
        range
            .clone()
            .all(|k| chars.get(k).is_some_and(char::is_ascii_hexdigit))
    };
    let mut units: Vec<u16> = Vec::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '_' && matches!(chars.get(i + 1), Some('x' | 'X')) {
            if hex(i + 2..i + 6) && chars.get(i + 6) == Some(&'_') {
                let code: String = chars[i + 2..i + 6].iter().collect();
                units.push(u16::from_str_radix(&code, 16).unwrap_or(0));
                i += 7;
                continue;
            }
            if hex(i + 2..i + 10) && chars.get(i + 10) == Some(&'_') {
                let code: String = chars[i + 2..i + 10].iter().collect();
                let u = u32::from_str_radix(&code, 16).unwrap_or(u32::MAX);
                if u < 0x1_0000 {
                    units.push(u as u16);
                } else if let Some(c) = char::from_u32(u) {
                    let mut pair = [0u16; 2];
                    units.extend_from_slice(c.encode_utf16(&mut pair));
                } else {
                    // UCS-4 밖 — 바꾸지 않고 그대로 둔다(건너뛴 자리도 그대로 남는다).
                    for c in &chars[i..i + 11] {
                        let mut pair = [0u16; 2];
                        units.extend_from_slice(c.encode_utf16(&mut pair));
                    }
                }
                i += 11;
                continue;
            }
        }
        let mut pair = [0u16; 2];
        units.extend_from_slice(chars[i].encode_utf16(&mut pair));
        i += 1;
    }
    // .NET 글은 짝 잃은 대리 글자를 품을 수 있지만 SQLite 에 UTF-8 로 넣을 때 U+FFFD 가 된다.
    String::from_utf16_lossy(&units)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_kinds_follow_closedxml() {
        assert_eq!(number_kind(14, ""), NumberKind::DateTime);
        assert_eq!(number_kind(17, ""), NumberKind::Number);
        assert_eq!(number_kind(20, ""), NumberKind::TimeSpan);
        assert_eq!(number_kind(-1, "yyyy/mm/dd"), NumberKind::DateTime);
        assert_eq!(number_kind(-1, "[$-412]yyyy\"년\""), NumberKind::DateTime);
        assert_eq!(number_kind(-1, "mm:ss"), NumberKind::TimeSpan);
        assert_eq!(number_kind(-1, "#,##0"), NumberKind::Number);
        assert_eq!(number_kind(-1, "@"), NumberKind::Number);
        assert_eq!(number_kind(-1, "\"a"), NumberKind::Number);
    }

    #[test]
    fn strings_decode_like_closedxml() {
        assert_eq!(decode_string(Some("_x005F_x0041_")), "_x0041_");
        assert_eq!(decode_string(Some("_x0041_")), "A");
        assert_eq!(decode_string(Some("_X0041_")), "_X0041_");
        assert_eq!(decode_string(Some("a_x000D_\nb")), "a\r\nb");
        assert_eq!(fix_new_lines("a\nb\r\nc\rd"), "a\r\nb\r\nc\rd");
    }

    #[test]
    fn references() {
        assert_eq!(cell_ref("A1"), Some((1, 1)));
        assert_eq!(cell_ref("AB12"), Some((12, 28)));
        assert_eq!(cell_ref("1"), None);
    }

    #[test]
    fn date_cells() {
        assert_eq!(
            parse_date_cell("2026-03-05").unwrap().plan_text(),
            "2026-03-05"
        );
        assert_eq!(
            parse_date_cell(" 2026-03-05T10:30 ").unwrap().plan_text(),
            "2026-03-05 10:30:00"
        );
        assert!(parse_date_cell("2026-3-5").is_err());
    }
}

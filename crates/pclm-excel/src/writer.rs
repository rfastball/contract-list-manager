//! xlsx 쓰기 — ClosedXML 0.105.1 이 [`crate::exporter`] 의 두 길에서 짓는 것과 **같은 칸·같은 꼴**의 통합 문서.
//!
//! 라이브러리(rust_xlsxwriter 따위)를 쓰지 않는 까닭: ClosedXML 은 빈 글(`""`)을 **빈 공유 문자열 칸**으로 적는데, 쓰기
//! 라이브러리들은 그것을 꼴만 있는 빈 칸으로 바꾼다 — 엑셀에서 `COUNTA` 가 달라진다. 열 너비도 ClosedXML 이 적는 값
//! (`픽셀 → 글자 폭 + 0.710625`)을 그대로 적으려면 직접 쓰는 편이 짧다. 부분(part)의 내용은 ClosedXML 이 적는 것을
//! 본떴다(쓰인 꼴만 적는 styles.xml — 기본 · 머리글(굵게, `#EEF1F4` 채움) · 글(`@`) · 줄바꿈 · 따옴표 머리, 이웃한 같은 너비를
//! 접은 `<col>`, 첫 줄 고정, 자동 거르개, 공유 문자열). 테마(`theme1.xml`)·문서 속성(`docProps`)은 적지 않는다 — 칸에
//! 닿지 않고, ClosedXML 의 것에는 지을 때마다 바뀌는 이름과 시각이 박힌다.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::{Seek, Write};

use zip::write::SimpleFileOptions;

/// 칸의 꼴 — ClosedXML 이 칸마다 쥐는 꼴 값 가운데 이 판이 쓰는 넷. 같은 꼴은 `cellXfs` 의 한 자리를 나눠 쓴다.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Style {
    /// 머리글 — 굵게, `#EEF1F4` 로 채움.
    pub header: bool,
    /// 글로 못박은 칸(`@`).
    pub text: bool,
    /// 줄바꿈(`wrapText`). ClosedXML 은 `\r\n` 이 든 글을 넣을 때 켠다 — 홀로 선 `\n` 에는 켜지 않는다.
    pub wrap: bool,
    /// 따옴표 머리(`quotePrefix`). ClosedXML 은 `'` 로 시작하는 글을 넣을 때 켜고 그 `'` 하나를 떼어 담는다.
    pub quote: bool,
}

impl Style {
    /// 꼴 없음(기본).
    pub const DEFAULT: Style = Style {
        header: false,
        text: false,
        wrap: false,
        quote: false,
    };
    /// 머리글.
    pub const HEADER: Style = Style {
        header: true,
        ..Style::DEFAULT
    };
    /// 글(`@`).
    pub const TEXT: Style = Style {
        text: true,
        ..Style::DEFAULT
    };

    fn num_fmt_id(self) -> u32 {
        if self.text { 164 } else { 0 }
    }

    fn font_id(self) -> u32 {
        u32::from(self.header)
    }

    fn fill_id(self) -> u32 {
        if self.header { 2 } else { 0 }
    }
}

/// 칸의 값.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Text(String),
    /// 수. 내보내기는 쓰지 않는다 — 시험이 수가 든 계획 엑셀을 지을 때만 쓴다.
    Number(f64),
}

/// 칸 하나.
#[derive(Clone, Debug)]
pub struct Cell {
    pub value: Value,
    pub style: Style,
}

/// 시트 하나.
#[derive(Clone, Debug, Default)]
pub struct Sheet {
    pub name: String,
    /// 줄마다의 칸(1번 줄부터, A 열부터). 줄마다 길이가 달라도 된다.
    pub rows: Vec<Vec<Cell>>,
    /// 열마다 적을 `width`(1번 열부터). 비면 `<cols>` 를 쓰지 않는다.
    pub widths: Vec<f64>,
    /// 첫 줄을 고정하는가.
    pub freeze_first_row: bool,
    /// 자동 거르개의 범위 — (마지막 줄, 마지막 열), 1부터. A1 에서 시작한다.
    pub autofilter: Option<(usize, usize)>,
}

const MAIN: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

/// 통합 문서 한 권을 쓴다.
pub fn write(target: impl Write + Seek, sheets: &[Sheet]) -> std::io::Result<()> {
    let mut zip = zip::ZipWriter::new(target);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut strings = SharedStrings::default();
    let mut parts: Vec<(String, String)> = Vec::new();
    let styles = used_styles(sheets);
    for (i, sheet) in sheets.iter().enumerate() {
        parts.push((
            format!("xl/worksheets/sheet{}.xml", i + 1),
            worksheet(sheet, &styles, &mut strings),
        ));
    }

    let mut put = |name: &str, body: &str| -> std::io::Result<()> {
        zip.start_file(name, options)
            .map_err(std::io::Error::other)?;
        zip.write_all(body.as_bytes())
    };
    put("[Content_Types].xml", &content_types(sheets.len()))?;
    put(
        "_rels/.rels",
        &format!(
            r#"<?xml version="1.0" encoding="utf-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Type="{REL}/officeDocument" Target="/xl/workbook.xml" Id="rId1" /></Relationships>"#
        ),
    )?;
    put("xl/workbook.xml", &workbook(sheets))?;
    put("xl/_rels/workbook.xml.rels", &workbook_rels(sheets.len()))?;
    put("xl/styles.xml", &styles_xml(&styles))?;
    put("xl/sharedStrings.xml", &strings.xml())?;
    for (name, body) in &parts {
        put(name, body)?;
    }
    zip.finish().map_err(std::io::Error::other)?;
    Ok(())
}

fn content_types(sheets: usize) -> String {
    let mut out = String::from(
        r#"<?xml version="1.0" encoding="utf-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml" /><Default Extension="xml" ContentType="application/xml" /><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml" /><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml" /><Override PartName="/xl/sharedStrings.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml" />"#,
    );
    for i in 1..=sheets {
        let _ = write!(
            out,
            r#"<Override PartName="/xl/worksheets/sheet{i}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml" />"#
        );
    }
    out.push_str("</Types>");
    out
}

fn workbook(sheets: &[Sheet]) -> String {
    let mut out = format!(
        r#"<?xml version="1.0" encoding="utf-8"?><workbook xmlns="{MAIN}" xmlns:r="{REL}"><workbookPr codeName="ThisWorkbook" /><bookViews><workbookView firstSheet="0" activeTab="0" /></bookViews><sheets>"#
    );
    for (i, sheet) in sheets.iter().enumerate() {
        let _ = write!(
            out,
            r#"<sheet name="{}" sheetId="{}" r:id="rId{}" />"#,
            attribute(&sheet.name),
            i + 1,
            i + 3
        );
    }
    out.push_str("</sheets>");
    let filters: Vec<String> = sheets
        .iter()
        .enumerate()
        .filter_map(|(i, sheet)| {
            let (row, col) = sheet.autofilter?;
            // ClosedXML 은 이름을 꼭 따옴표로 싸야 할 때만 싼다. 엑셀은 늘 싼 것도 같게 읽으므로 늘 싼다.
            let quoted = format!("'{}'", sheet.name.replace('\'', "''"));
            Some(format!(
                r#"<definedName name="_xlnm._FilterDatabase" localSheetId="{i}" hidden="1">{}!$A$1:${}${row}</definedName>"#,
                text(&quoted),
                column_name(col)
            ))
        })
        .collect();
    if !filters.is_empty() {
        out.push_str("<definedNames>");
        for f in filters {
            out.push_str(&f);
        }
        out.push_str("</definedNames>");
    }
    out.push_str(r#"<calcPr calcId="125725" /></workbook>"#);
    out
}

fn workbook_rels(sheets: usize) -> String {
    let mut out = format!(
        r#"<?xml version="1.0" encoding="utf-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Type="{REL}/sharedStrings" Target="/xl/sharedStrings.xml" Id="rId1" /><Relationship Type="{REL}/styles" Target="/xl/styles.xml" Id="rId2" />"#
    );
    for i in 1..=sheets {
        let _ = write!(
            out,
            r#"<Relationship Type="{REL}/worksheet" Target="/xl/worksheets/sheet{i}.xml" Id="rId{}" />"#,
            i + 2
        );
    }
    out.push_str("</Relationships>");
    out
}

/// 쓰인 꼴의 목록 — `cellXfs` 의 차례. ClosedXML `WorkbookStylesPartWriter` 처럼 기본 꼴을 맨 앞에 두고, 시트마다 칸을
/// 줄 차례로 돌며 처음 보는 꼴을 잇는다(열·줄에 매단 꼴은 이 판에 없다). 번호의 차례는 칸의 꼴을 바꾸지 않는다.
fn used_styles(sheets: &[Sheet]) -> Vec<Style> {
    let mut list = vec![Style::DEFAULT];
    for cell in sheets.iter().flat_map(|s| s.rows.iter().flatten()) {
        if !list.contains(&cell.style) {
            list.push(cell.style);
        }
    }
    list
}

/// ClosedXML 이 이 꼴들에 적는 styles.xml — 수 꼴 0(빈 글)은 늘 적고 `@` 는 쓰일 때만 164 로, 굵은 글꼴과 채움도
/// 쓰일 때만 더한다. `cellStyleXfs` 는 정렬(줄바꿈)을 빼고 같은 꼴을 하나로 접는다. 채운 꼴의 `applyFill` 이 `0` 인
/// 것까지 ClosedXML 이 적는 그대로다.
fn styles_xml(styles: &[Style]) -> String {
    let text = styles.iter().any(|s| s.text);
    let header = styles.iter().any(|s| s.header);
    let mut out = String::from(
        r#"<?xml version="1.0" encoding="utf-8"?><styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">"#,
    );
    let _ = write!(
        out,
        r#"<numFmts count="{}"><numFmt numFmtId="0" formatCode="" />"#,
        1 + usize::from(text)
    );
    if text {
        out.push_str(r#"<numFmt numFmtId="164" formatCode="@" />"#);
    }
    out.push_str("</numFmts>");
    const FONT: &str = r#"<vertAlign val="baseline" /><sz val="11" /><color rgb="FF000000" /><name val="Calibri" /><family val="2" /></font>"#;
    let _ = write!(
        out,
        r#"<fonts count="{}"><font>{FONT}"#,
        1 + usize::from(header)
    );
    if header {
        let _ = write!(out, "<font><b />{FONT}");
    }
    let _ = write!(
        out,
        r#"</fonts><fills count="{}"><fill><patternFill patternType="none" /></fill><fill><patternFill patternType="gray125" /></fill>"#,
        2 + usize::from(header)
    );
    if header {
        out.push_str(
            r#"<fill><patternFill patternType="solid"><fgColor rgb="FFEEF1F4" /></patternFill></fill>"#,
        );
    }
    out.push_str(r#"</fills><borders count="1"><border diagonalUp="0" diagonalDown="0"><left style="none"><color rgb="FF000000" /></left><right style="none"><color rgb="FF000000" /></right><top style="none"><color rgb="FF000000" /></top><bottom style="none"><color rgb="FF000000" /></bottom><diagonal style="none"><color rgb="FF000000" /></diagonal></border></borders>"#);

    let mut bases: Vec<Style> = Vec::new();
    for style in styles {
        let base = Style {
            wrap: false,
            ..*style
        };
        if !bases.contains(&base) {
            bases.push(base);
        }
    }
    let _ = write!(out, r#"<cellStyleXfs count="{}">"#, bases.len());
    for style in &bases {
        xf(&mut out, *style, false);
    }
    let _ = write!(out, r#"</cellStyleXfs><cellXfs count="{}">"#, styles.len());
    for style in styles {
        xf(&mut out, *style, true);
    }
    out.push_str(r#"</cellXfs><cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0" /></cellStyles></styleSheet>"#);
    out
}

/// `xf` 하나. `cell` 이면 `cellXfs` 의 것이라 `xfId` 와 정렬을 단다.
fn xf(out: &mut String, style: Style, cell: bool) {
    let _ = write!(
        out,
        r#"<xf numFmtId="{}" fontId="{}" fillId="{}" borderId="0""#,
        style.num_fmt_id(),
        style.font_id(),
        style.fill_id()
    );
    if cell {
        out.push_str(r#" xfId="0""#);
    }
    if style.quote {
        out.push_str(r#" quotePrefix="1""#);
    }
    let _ = write!(
        out,
        r#" applyNumberFormat="1" applyFill="{}" applyBorder="0" applyAlignment="1" applyProtection="1">"#,
        u8::from(!style.header)
    );
    if cell {
        let _ = write!(
            out,
            r#"<alignment horizontal="general" vertical="bottom" textRotation="0" wrapText="{}" indent="0" relativeIndent="0" justifyLastLine="0" shrinkToFit="0" readingOrder="0" />"#,
            u8::from(style.wrap)
        );
    }
    out.push_str(r#"<protection locked="1" hidden="0" /></xf>"#);
}

fn worksheet(sheet: &Sheet, styles: &[Style], strings: &mut SharedStrings) -> String {
    let last_row = sheet.rows.len().max(1);
    let last_col = sheet.rows.iter().map(Vec::len).max().unwrap_or(0).max(1);
    let mut out = format!(
        r#"<?xml version="1.0" encoding="utf-8" standalone="yes"?><worksheet xmlns="{MAIN}" xmlns:r="{REL}"><sheetPr><outlinePr summaryBelow="1" summaryRight="1" /></sheetPr><dimension ref="A1:{}{last_row}" /><sheetViews><sheetView workbookViewId="0">"#,
        column_name(last_col)
    );
    if sheet.freeze_first_row {
        out.push_str(r#"<pane xSplit="0" ySplit="1" topLeftCell="A2" activePane="bottomLeft" state="frozenSplit" />"#);
    }
    out.push_str(r#"</sheetView></sheetViews><sheetFormatPr defaultRowHeight="15" />"#);
    if !sheet.widths.is_empty() {
        out.push_str("<cols>");
        // ClosedXML `CollapseColumns` — 이웃한 열의 너비가 같으면 `<col>` 하나로 접는다.
        let mut min = 1;
        for (i, width) in sheet.widths.iter().enumerate() {
            if sheet.widths.get(i + 1) == Some(width) {
                continue;
            }
            let _ = write!(
                out,
                r#"<col min="{min}" max="{}" width="{width}" style="0" customWidth="1" />"#,
                i + 1
            );
            min = i + 2;
        }
        out.push_str("</cols>");
    }
    out.push_str("<sheetData>");
    for (r, row) in sheet.rows.iter().enumerate() {
        if row.is_empty() {
            continue;
        }
        let _ = write!(out, r#"<row r="{}" spans="1:{}">"#, r + 1, row.len());
        for (c, cell) in row.iter().enumerate() {
            let reference = format!("{}{}", column_name(c + 1), r + 1);
            let style = styles.iter().position(|s| *s == cell.style).unwrap_or(0);
            match &cell.value {
                Value::Text(t) => {
                    let id = strings.id(t);
                    let _ = write!(
                        out,
                        r#"<c r="{reference}" s="{style}" t="s"><v>{id}</v></c>"#
                    );
                }
                Value::Number(n) => {
                    let _ = write!(out, r#"<c r="{reference}" s="{style}"><v>{n:?}</v></c>"#);
                }
            }
        }
        out.push_str("</row>");
    }
    out.push_str("</sheetData>");
    if let Some((row, col)) = sheet.autofilter {
        let _ = write!(out, r#"<autoFilter ref="A1:{}{row}" />"#, column_name(col));
    }
    out.push_str(r#"<printOptions horizontalCentered="0" verticalCentered="0" headings="0" gridLines="0" /><pageMargins left="0.75" right="0.75" top="0.75" bottom="0.5" header="0.5" footer="0.75" /><pageSetup paperSize="1" scale="100" pageOrder="downThenOver" orientation="default" blackAndWhite="0" draft="0" cellComments="none" errors="displayed" /><headerFooter /><tableParts count="0" /></worksheet>"#);
    out
}

/// 공유 문자열 — 같은 글은 한 번만 적는다.
#[derive(Default)]
struct SharedStrings {
    ids: HashMap<String, usize>,
    items: Vec<String>,
}

impl SharedStrings {
    fn id(&mut self, value: &str) -> usize {
        if let Some(&id) = self.ids.get(value) {
            return id;
        }
        let id = self.items.len();
        self.items.push(value.to_owned());
        self.ids.insert(value.to_owned(), id);
        id
    }

    fn xml(&self) -> String {
        let mut out = format!(r#"<?xml version="1.0" encoding="utf-8"?><sst xmlns="{MAIN}">"#);
        for item in &self.items {
            // ClosedXML: Trim() 한 것이 같지 않으면 xml:space="preserve".
            if item.trim() != item {
                out.push_str(r#"<si><t xml:space="preserve">"#);
            } else {
                out.push_str("<si><t>");
            }
            out.push_str(&text(&encode_string(item)));
            out.push_str("</t></si>");
        }
        out.push_str("</sst>");
        out
    }
}

/// ClosedXML `XmlEncoder.EncodeString` — `_xHHHH_` 꼴의 글을 `_x005F_xHHHH_` 로 막고, XML 에 쓸 수 없는 글자(제어 글자
/// 따위)를 `_xHHHH_` 로 바꾼다.
pub fn encode_string(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    let mut guarded = String::with_capacity(value.len());
    let mut i = 0;
    while i < chars.len() {
        // 정규식 `_(x[\dA-Fa-f]{4})_` — 겹치지 않게 왼쪽부터.
        if chars[i] == '_'
            && chars.get(i + 1) == Some(&'x')
            && (2..6).all(|k| chars.get(i + k).is_some_and(char::is_ascii_hexdigit))
            && chars.get(i + 6) == Some(&'_')
        {
            guarded.push_str("_x005F_");
            guarded.extend(&chars[i + 1..i + 6]);
            guarded.push('_');
            i += 7;
            continue;
        }
        guarded.push(chars[i]);
        i += 1;
    }
    let mut out = String::with_capacity(guarded.len());
    for c in guarded.chars() {
        if is_xml_char(c) {
            out.push(c);
        } else {
            // XmlConvert.EncodeName(한 글자) — `_xHHHH_`(대문자 16진).
            let _ = write!(out, "_x{:04X}_", c as u32);
        }
    }
    out
}

/// `XmlConvert.IsXmlChar` — 0x9 · 0xA · 0xD · 0x20~0xD7FF · 0xE000~0xFFFD(보조 평면 글자는 짝을 이룬 대리쌍으로 받는다).
fn is_xml_char(c: char) -> bool {
    matches!(c as u32, 0x9 | 0xA | 0xD | 0x20..=0xD7FF | 0xE000..=0xFFFD | 0x10000..)
}

/// 요소 안의 글 — `&`·`<`·`>`.
fn text(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
    out
}

/// 속성 안의 글.
fn attribute(value: &str) -> String {
    text(value).replace('"', "&quot;")
}

/// 열 번호(1부터) → `A`·`Z`·`AA`.
pub fn column_name(mut n: usize) -> String {
    let mut out = Vec::new();
    while n > 0 {
        let rem = (n - 1) % 26;
        out.push(b'A' + rem as u8);
        n = (n - 1) / 26;
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_like_closedxml() {
        // ClosedXML 0.105.1 이 시험 엑셀에 적은 것.
        assert_eq!(
            encode_string("_x0041_ <&> \u{1} 끝 "),
            "_x005F_x0041_ <&> _x0001_ 끝 "
        );
        assert_eq!(encode_string("_X0041_"), "_X0041_");
    }

    #[test]
    fn column_names() {
        assert_eq!(column_name(1), "A");
        assert_eq!(column_name(26), "Z");
        assert_eq!(column_name(27), "AA");
        assert_eq!(column_name(16384), "XFD");
    }
}

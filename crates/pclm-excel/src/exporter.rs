//! 계약면 뷰를 엑셀 한 권으로 내보낸다(C# `Storage/Exporter.cs`).
//!
//! 이 파일은 **그 시점의 사진**이지 원본이 아니다. 언제든 다시 뽑을 수 있고, 여기에 손으로 적은 것은 DB 로 돌아오지
//! 않는다. 시트를 뷰마다 하나씩 두는 까닭은 한 줄이 뜻하는 것이 시트마다 다르기 때문이다.
//!
//! 값은 이미 표시용 문자열이라 모든 칸을 글(`@`)로 못박는다 — 엑셀이 숫자로 재해석해 자릿점을 지우거나 긴 번호를 지수로
//! 바꾸지 않게. 머리글은 굵게·`#EEF1F4` 로 채우고, 첫 줄을 고정하고, 쓰인 범위에 자동 거르개를 걸고, 열 너비를
//! ClosedXML `AdjustToContents(1, 40, 60)` 처럼 잰다([`crate::measure`]).

use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use pclm_core::local_time::LocalDateTime;
use pclm_core::storage::store::text_of;
use pclm_core::storage::views;
use pclm_core::{Database, Error, Result};
use rusqlite::types::ValueRef;

use crate::font;
use crate::measure::{self, MeasuredCell};
use crate::writer::{self, Cell, Sheet, Style, Value};

/// C# `SheetSnapshot(string[] Columns, string[][] Rows)` — 화면에서 확정한 열·행 차례 그대로의 한 장. JSON 의 null 은
/// `None` 이다(검사가 거절한다).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SheetSnapshot {
    pub columns: Option<Vec<Option<String>>>,
    pub rows: Option<Vec<Option<Vec<Option<String>>>>>,
}

/// `Exporter.DefaultFileName(on)` — 날짜를 박아 사진임을 드러낸다. `on` 은 부르는 쪽의 로컬 날짜(C# 은 `DateTime.Now`).
pub fn default_file_name(on: LocalDateTime) -> String {
    format!("관리대장_{}.xlsx", compact_date(on))
}

/// `yyyyMMdd`.
pub fn compact_date(on: LocalDateTime) -> String {
    format!("{:04}{:02}{:02}", on.year, on.month, on.day)
}

/// `Exporter.Export(path)` — [`views::EXPORTED`] 의 뷰마다 시트 하나. 실제로 쓴 전체 경로를 낸다.
///
/// `Names` 가 아니라 `Exported` 를 돈다. 차수를 편 뷰까지 함께 내면 같은 건이 여러 장에 되풀이된다.
pub fn export(database: &Database, path: &Path) -> Result<PathBuf> {
    let full = std::path::absolute(path)?;
    if let Some(directory) = full.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(directory)?;
    }

    let connection = database.open_read_only()?;
    let mut sheets = Vec::new();
    for view in views::EXPORTED {
        let mut statement = connection.prepare(&format!("SELECT * FROM \"{view}\";"))?;
        let columns: Vec<String> = statement.column_names().map(String::from).collect();
        let mut rows = Vec::new();
        let mut cursor = statement.query([])?;
        while let Some(row) = cursor.next()? {
            let mut cells = Vec::with_capacity(columns.len());
            for i in 0..columns.len() {
                cells.push(display(row.get_ref(i)?));
            }
            rows.push(cells);
        }
        // 시트 이름에는 뷰 판을 남기지 않는다 — 사람이 고르는 이름이라 짧을수록 낫다.
        sheets.push(build(views::sheet_name(view), &columns, &rows));
    }
    save(&full, &sheets)?;
    Ok(full)
}

/// `Exporter.ExportSheet(path, title, snapshot)` — 화면에서 확정한 열/행 차례 그대로 한 장. 전체 자료 내보내기와 별개의
/// 사진이다. 없는 폴더는 짓는다 — C# 은 짓지 않지만 ClosedXML 의 `SaveAs` 가 짓는다.
pub fn export_sheet(path: &Path, title: &str, snapshot: &SheetSnapshot) -> Result<PathBuf> {
    validate_snapshot(title, snapshot)?;
    let columns: Vec<String> = snapshot
        .columns
        .iter()
        .flatten()
        .map(|c| c.clone().unwrap_or_default())
        .collect();
    let rows: Vec<Vec<String>> = snapshot
        .rows
        .iter()
        .flatten()
        .map(|r| {
            r.iter()
                .flatten()
                .map(|v| v.clone().unwrap_or_default())
                .collect()
        })
        .collect();
    let full = std::path::absolute(path)?;
    save(&full, &[build(title, &columns, &rows)])?;
    Ok(full)
}

/// `Exporter.ValidateSnapshot` — 엑셀이 받지 않는 시트 이름과 크기·값을 C# 과 같은 글로 거절한다. 길이는 UTF-16 단위다.
pub fn validate_snapshot(title: &str, snapshot: &SheetSnapshot) -> Result<()> {
    let utf16 = |s: &str| s.encode_utf16().count();
    // char.IsControl 은 UTF-16 단위 하나씩 본다 — Cc(U+0000~001F, U+007F~009F).
    let control = |c: char| matches!(c as u32, 0..=0x1F | 0x7F..=0x9F);
    if title.chars().all(char::is_whitespace)
        || utf16(title) > 31
        || title.contains(['[', ']', ':', '*', '?', '/', '\\'])
        || title.starts_with('\'')
        || title.ends_with('\'')
        || title.chars().any(control)
    {
        return Err(Error::InvalidOperation(
            "엑셀 시트 이름이 올바르지 않습니다.".into(),
        ));
    }
    let bad = || {
        Error::InvalidOperation(
            "내보낼 표의 크기나 셀 값이 올바르지 않습니다. 최대 2,000,000셀을 내보낼 수 있습니다."
                .into(),
        )
    };
    let (Some(columns), Some(rows)) = (&snapshot.columns, &snapshot.rows) else {
        return Err(bad());
    };
    if columns.is_empty()
        || columns.len() > 16384
        || rows.len() > 1_048_575
        || (columns.len() as u64) * (rows.len() as u64) > 2_000_000
    {
        return Err(bad());
    }
    if columns.iter().any(|c| {
        c.as_deref()
            .is_none_or(|c| c.is_empty() || utf16(c) > 32767)
    }) {
        return Err(bad());
    }
    // Distinct().Count() — null 도 값 하나로 센다(위에서 이미 거절했다).
    let mut seen = std::collections::HashSet::new();
    if !columns.iter().all(|c| seen.insert(c)) {
        return Err(bad());
    }
    if rows.iter().any(|r| {
        r.as_ref().is_none_or(|r| {
            r.len() != columns.len()
                || r.iter()
                    .any(|v| v.as_deref().is_none_or(|v| utf16(v) > 32767))
        })
    }) {
        return Err(bad());
    }
    Ok(())
}

/// `reader.IsDBNull(c) ? "" : reader.GetValue(c).ToString()` — 수는 현재 문화권 글(ko-KR). Invariant 와 갈리는 것은
/// 무한대 표기뿐이라 그것만 따로 적는다(다리의 `sheet` 와 같다).
fn display(value: ValueRef<'_>) -> String {
    match value {
        ValueRef::Real(d) if d == f64::INFINITY => "∞".into(),
        ValueRef::Real(d) if d == f64::NEG_INFINITY => "-∞".into(),
        other => text_of(other).unwrap_or_default(),
    }
}

/// ClosedXML 이 칸에 글을 넣을 때(`SetValue` · `Value =`) — `GetStyleForValue` 가 `'` 로 시작하는 글에 따옴표 머리를,
/// `Environment.NewLine`(Windows 의 `\r\n`)이 든 글에 줄바꿈을 켜고, 꼴을 정한 뒤 `'` 하나를 떼어 담는다. 두 길 모두
/// 값을 먼저 넣고 꼴(굵게·채움, `@`)을 나중에 더하므로 둘이 겹친다. 뗀 값으로 너비를 재고 쓰인 범위를 정한다.
fn cell(value: &str, style: Style) -> Cell {
    Cell {
        value: Value::Text(value.strip_prefix('\'').unwrap_or(value).to_owned()),
        style: Style {
            quote: value.starts_with('\''),
            wrap: value.contains("\r\n"),
            ..style
        },
    }
}

/// 머리글 한 줄과 몸 줄들로 시트 하나 — 두 길이 같은 꼴을 쓴다.
fn build(name: &str, columns: &[String], rows: &[Vec<String>]) -> Sheet {
    let header: Vec<Cell> = columns.iter().map(|c| cell(c, Style::HEADER)).collect();
    let mut all = vec![header];
    for row in rows {
        all.push(row.iter().map(|v| cell(v, Style::TEXT)).collect());
    }

    // RangeUsed — 빈 글이 아닌 칸이 있는 마지막 줄·열.
    let mut last_row = 0;
    let mut last_col = 0;
    for (r, row) in all.iter().enumerate() {
        for (c, cell) in row.iter().enumerate() {
            if matches!(&cell.value, Value::Text(t) if !t.is_empty()) {
                last_row = last_row.max(r + 1);
                last_col = last_col.max(c + 1);
            }
        }
    }
    let autofilter = (last_row > 0).then_some((last_row, last_col));

    let widths = match font::workbook_font() {
        Some(font) => {
            let mdw = measure::mdw(font);
            (0..columns.len())
                .map(|c| {
                    let cells = all.iter().enumerate().filter_map(|(r, row)| {
                        let Value::Text(text) = &row.get(c)?.value else {
                            return None;
                        };
                        Some(MeasuredCell {
                            text,
                            autofilter_row: autofilter.is_some() && r == 0,
                        })
                    });
                    measure::stored_width(measure::column_px(font, cells), mdw)
                })
                .collect()
        }
        // 잴 글꼴이 없으면(Windows 가 아닌 기계) 가장 좁은 너비로 둔다.
        None => vec![measure::stored_width(285, 7); columns.len()],
    };

    Sheet {
        name: name.to_owned(),
        rows: all,
        widths,
        freeze_first_row: true,
        autofilter,
    }
}

/// ClosedXML `SaveAs` — 없는 폴더를 지은 뒤 쓴다.
fn save(path: &Path, sheets: &[Sheet]) -> Result<()> {
    if let Some(directory) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(directory)?;
    }
    let file = File::create(path)?;
    let mut out = BufWriter::new(file);
    writer::write(&mut out, sheets)?;
    use std::io::Write as _;
    out.flush()?;
    Ok(())
}

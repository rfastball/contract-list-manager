//! 열 너비 — ClosedXML 0.105.1 의 `Columns().AdjustToContents(1, 40, 60)` 을 그대로 옮긴다(`XLColumn.AdjustToContents`,
//! `CalculateMinColumnWidth`, `XLCell.GetGlyphBoxes`, `DefaultGraphicEngine.GetGlyphBox`).
//!
//! 칸의 글을 글자 묶음(grapheme)으로 가르고, 묶음마다 글자들의 전진폭을 더해 픽셀로 바꿔 **반올림**한다(Calibri 11pt,
//! 96 DPI). 줄바꿈에서 줄을 끊고 가장 긴 줄이 그 칸의 글 너비다. 양옆 여백(글 너비의 3% + 숫자 폭의 1/4, 올림)과
//! 칸 사이 선 1px 을 더하고, 자동 거르개가 선 줄(머리글)에는 단추 16px 를 더한다. 열의 너비는 칸들 가운데 가장 큰 것을
//! 40~60 글자 폭 사이로 자른 것이다.
//!
//! **C# 의 버릇도 옮긴다.** `\r\n` 은 묶음 하나인데 ClosedXML 은 줄바꿈 길이(2)만큼 묶음을 건너뛰어, 그 뒤의 묶음
//! 하나를 재지 않는다.

use unicode_segmentation::UnicodeSegmentation;

use crate::font::Metrics;

/// 통합 문서 글꼴의 크기(pt). 새 통합 문서는 Calibri 11.
const FONT_SIZE: f64 = 11.0;
/// DPI — ClosedXML 의 기본.
const DPI: f64 = 96.0;
/// 더 좁히지 않는 너비(글자 폭 수).
pub const MIN_WIDTH: f64 = 40.0;
/// 더 넓히지 않는 너비(글자 폭 수).
pub const MAX_WIDTH: f64 = 60.0;
/// ClosedXML 이 너비를 적을 때 더하는 값(`XLConstants.ColumnWidthOffset`).
const COLUMN_WIDTH_OFFSET: f64 = 0.710625;

/// 잴 칸 하나.
pub struct MeasuredCell<'a> {
    pub text: &'a str,
    /// 자동 거르개가 선 줄의 칸인가(머리글).
    pub autofilter_row: bool,
}

/// 글꼴 단위 → 픽셀(반올림 전). `PointsToPixels(advanceFu * size / unitsPerEm, dpi)`.
fn units_to_px(units: i64, units_per_em: u16) -> f64 {
    (units as f64 * FONT_SIZE / f64::from(units_per_em)) / 72.0 * DPI
}

fn round_away(x: f64) -> f64 {
    x.round()
}

/// 숫자 폭(픽셀, 반올림 전) — `GetMaxDigitWidth`.
fn max_digit_width(font: &Metrics) -> f64 {
    let per_em = f64::from(font.max_digit_advance()) / f64::from(font.units_per_em());
    per_em * FONT_SIZE / 72.0 * DPI
}

/// 글 하나의 너비(픽셀) — 가장 긴 줄. `GetGlyphBoxes` 와 `GetContentWidth(0, …)`.
fn content_width(font: &Metrics, text: &str) -> f64 {
    let graphemes: Vec<&str> = text.graphemes(true).collect();
    let mut max = 0f64;
    let mut line = 0f64;
    let mut i = 0usize;
    while i < graphemes.len() {
        // TrySliceNewLine 은 남은 글 전체를 본다 — 묶음이 아니라 글자 둘.
        let rest_starts_crlf = graphemes[i].starts_with("\r\n")
            || (graphemes[i] == "\r" && graphemes.get(i + 1).is_some_and(|g| g.starts_with('\n')));
        let newline =
            rest_starts_crlf || graphemes[i].starts_with('\n') || graphemes[i].starts_with('\r');
        if newline {
            let length = if rest_starts_crlf { 2 } else { 1 };
            i += length - 1;
            // 잇따른 줄바꿈 사이의 ZWJ 상자는 너비가 0 이라 줄 너비에 보태지 않는다.
            line = 0.0;
        } else {
            let units: i64 = graphemes[i]
                .chars()
                .map(|c| i64::from(font.advance(c as u32)))
                .sum();
            let px = round_away(units_to_px(units, font.units_per_em())) as f32;
            line += f64::from(px);
            max = max.max(line);
        }
        i += 1;
    }
    max
}

/// 열 하나의 너비(픽셀) — 칸마다 잰 것 가운데 가장 큰 것을 [`MIN_WIDTH`]~[`MAX_WIDTH`] 로 자른다.
pub fn column_px<'a>(font: &Metrics, cells: impl IntoIterator<Item = MeasuredCell<'a>>) -> i32 {
    let mdw_raw = max_digit_width(font);
    let scaled_mdw = round_away(mdw_raw);
    let mut column = 0i32;
    for cell in cells {
        // CellsUsed — 빈 글의 칸은 재지 않는다.
        if cell.text.is_empty() {
            continue;
        }
        let text_px = content_width(font, cell.text).ceil() as i32;
        let one_side = (f64::from(text_px) * 0.03 + scaled_mdw / 4.0).ceil() as i32;
        let mut cell_px = text_px + 2 * one_side + 1;
        if cell.autofilter_row {
            cell_px += round_away(16.0 * DPI / 96.0) as i32;
        }
        column = column.max(cell_px);
    }
    // Math.Round 는 짝수 쪽으로 반올림한다(이 자리만 AwayFromZero 가 아니다).
    let mdw = mdw_raw.round_ties_even() as i32;
    let min = no_c_to_pixels(MIN_WIDTH, mdw).ceil();
    if f64::from(column) < min {
        column = min as i32;
    }
    let max = no_c_to_pixels(MAX_WIDTH, mdw).ceil();
    if f64::from(column) > max {
        column = max as i32;
    }
    column
}

/// 그 열의 숫자 폭(정수 픽셀).
pub fn mdw(font: &Metrics) -> i32 {
    max_digit_width(font).round_ties_even() as i32
}

/// `XLHelper.NoCToPixels`.
fn no_c_to_pixels(noc: f64, mdw: i32) -> f64 {
    let pp = 2 * (f64::from(mdw) / 4.0).ceil() as i32 + 1;
    if noc < 1.0 {
        noc * f64::from(mdw + pp)
    } else {
        noc * f64::from(mdw) + f64::from(pp)
    }
}

/// `XLHelper.PixelToNoC`.
fn pixel_to_no_c(px: i32, mdw: i32) -> f64 {
    let pp = 2 * (f64::from(mdw) / 4.0).ceil() as i32 + 1;
    if px >= mdw + pp {
        f64::from(px - pp) / f64::from(mdw)
    } else {
        f64::from(px) / f64::from(mdw + pp)
    }
}

/// 파일에 적을 `width` — ClosedXML 이 적는 그대로(`Math.Round(noc + 0.710625, 6)`).
pub fn stored_width(px: i32, mdw: i32) -> f64 {
    let noc = pixel_to_no_c(px, mdw);
    let noc = if noc <= 0.0 { 9.140625 } else { noc };
    let value = (noc + COLUMN_WIDTH_OFFSET).clamp(0.0, 255.0);
    (value * 1e6).round_ties_even() / 1e6
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widths_follow_closedxml_pixels() {
        assert_eq!(no_c_to_pixels(40.0, 7), 285.0);
        assert_eq!(no_c_to_pixels(60.0, 7), 425.0);
        assert_eq!(stored_width(285, 7), 40.710625);
        // ClosedXML 0.105.1 이 343px 열에 적은 값(시험 엑셀에서 잰 것).
        assert_eq!(stored_width(343, 7), 48.996339);
    }
}

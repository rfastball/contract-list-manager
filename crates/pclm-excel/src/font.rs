//! 열 너비를 잴 글꼴 — ClosedXML 0.105.1 `DefaultGraphicEngine` 이 SixLabors.Fonts 로 읽는 것과 같은 수.
//!
//! ClosedXML 은 통합 문서의 글꼴 이름(새 통합 문서는 `Calibri`)으로 **이 기계의 글꼴**을 찾고, 없으면
//! `Microsoft Sans Serif`, 그것도 없으면 제가 품은 CarlitoBare 를 쓴다. 굵기·기울기를 따로 고르지 않는다 —
//! `FontFamily.CreateFont(size)` 는 늘 보통체다(굵은 머리글도 보통체의 너비로 잰다). 글자 하나의 너비는 cmap 이
//! 가리키는 글리프의 hmtx 전진폭이고, cmap 에 없는 글자는 0번(.notdef)의 너비다 — 한글이 없는 Calibri 에서 한글은
//! 모두 .notdef 의 너비로 잰다.
//!
//! 글꼴은 Windows 의 두 자리(`%SystemRoot%\Fonts`, `%LOCALAPPDATA%\Microsoft\Windows\Fonts`)에서 이름표(name 1)로
//! 찾는다. 프로세스에 한 번만 읽는다.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// 한 글꼴의 너비 표.
#[derive(Debug)]
pub struct Metrics {
    units_per_em: u16,
    advances: Vec<u16>,
    /// 글자 → 글리프 번호. 차례대로 찾고 처음 0 이 아닌 것을 쓴다.
    tables: Vec<CmapTable>,
}

#[derive(Debug)]
enum CmapTable {
    /// format 4 — (끝, 시작, delta, range offset 자리) 와 글리프 배열.
    Segments {
        segments: Vec<(u16, u16, i16, u16, usize)>,
        glyphs: Vec<u16>,
    },
    /// format 12 — (시작, 끝, 시작 글리프).
    Groups(Vec<(u32, u32, u32)>),
    /// format 0 · 6 — 시작 글자부터 이어진 배열.
    Trimmed { first: u32, glyphs: Vec<u16> },
}

impl Metrics {
    /// `units_per_em`.
    pub fn units_per_em(&self) -> u16 {
        self.units_per_em
    }

    /// 글자 하나의 전진폭(글꼴 단위). cmap 에 없으면 0번 글리프의 것.
    pub fn advance(&self, code_point: u32) -> u16 {
        let glyph = self.glyph(code_point);
        self.glyph_advance(glyph)
    }

    fn glyph_advance(&self, glyph: u16) -> u16 {
        let index = usize::from(glyph);
        match self.advances.get(index) {
            Some(&a) => a,
            None => self.advances.last().copied().unwrap_or(0),
        }
    }

    fn glyph(&self, code_point: u32) -> u16 {
        for table in &self.tables {
            let glyph = table.glyph(code_point);
            if glyph != 0 {
                return glyph;
            }
        }
        0
    }

    /// 0~9 가운데 가장 넓은 것(글꼴 단위) — ClosedXML `CalculateMaxDigitWidth`.
    pub fn max_digit_advance(&self) -> u16 {
        ('0'..='9')
            .map(|c| self.advance(c as u32))
            .max()
            .unwrap_or(0)
    }

    /// 글꼴 파일 하나(바이트)에서 읽는다. TTC 면 `index` 번째.
    pub fn parse(data: &[u8], index: usize) -> Option<Metrics> {
        let font = Font::at(data, index)?;
        let head = font.table(b"head")?;
        let units_per_em = be16(head, 18)?;
        let hhea = font.table(b"hhea")?;
        let count = usize::from(be16(hhea, 34)?);
        let hmtx = font.table(b"hmtx")?;
        let advances = (0..count)
            .map(|g| be16(hmtx, g * 4))
            .collect::<Option<Vec<u16>>>()?;
        let tables = cmap_tables(font.table(b"cmap")?);
        Some(Metrics {
            units_per_em,
            advances,
            tables,
        })
    }
}

impl CmapTable {
    fn glyph(&self, c: u32) -> u16 {
        match self {
            CmapTable::Segments { segments, glyphs } => {
                if c > 0xFFFF {
                    return 0;
                }
                let c = c as u16;
                for &(end, start, delta, range_offset, at) in segments {
                    if c > end {
                        continue;
                    }
                    if c < start {
                        return 0;
                    }
                    if range_offset == 0 {
                        return (c as i32 + delta as i32) as u16;
                    }
                    // idRangeOffset 는 그 자리에서 글리프 배열 안을 가리킨다 — 배열의 몇째인지로 바꿔 둔 것이 `at`.
                    let index = at + usize::from(c - start);
                    return match glyphs.get(index) {
                        Some(0) | None => 0,
                        Some(&g) => (g as i32 + delta as i32) as u16,
                    };
                }
                0
            }
            CmapTable::Groups(groups) => {
                for &(start, end, glyph) in groups {
                    if (start..=end).contains(&c) {
                        return (glyph + (c - start)) as u16;
                    }
                }
                0
            }
            CmapTable::Trimmed { first, glyphs } => c
                .checked_sub(*first)
                .and_then(|i| glyphs.get(i as usize))
                .copied()
                .unwrap_or(0),
        }
    }
}

struct Font<'a> {
    data: &'a [u8],
    tables: Vec<([u8; 4], usize, usize)>,
}

impl<'a> Font<'a> {
    fn at(data: &'a [u8], index: usize) -> Option<Font<'a>> {
        let start = if data.get(0..4)? == b"ttcf" {
            let count = be32(data, 8)? as usize;
            if index >= count {
                return None;
            }
            be32(data, 12 + index * 4)? as usize
        } else {
            0
        };
        let count = usize::from(be16(data, start + 4)?);
        let mut tables = Vec::with_capacity(count);
        for i in 0..count {
            let record = start + 12 + i * 16;
            let tag: [u8; 4] = data.get(record..record + 4)?.try_into().ok()?;
            let offset = be32(data, record + 8)? as usize;
            let length = be32(data, record + 12)? as usize;
            tables.push((tag, offset, length));
        }
        Some(Font { data, tables })
    }

    fn table(&self, tag: &[u8; 4]) -> Option<&'a [u8]> {
        let &(_, offset, length) = self.tables.iter().find(|(t, _, _)| t == tag)?;
        self.data.get(offset..offset.checked_add(length)?)
    }
}

fn cmap_tables(cmap: &[u8]) -> Vec<CmapTable> {
    let mut out = Vec::new();
    let Some(count) = be16(cmap, 2) else {
        return out;
    };
    for i in 0..usize::from(count) {
        let record = 4 + i * 8;
        let (Some(platform), Some(offset)) = (be16(cmap, record), be32(cmap, record + 4)) else {
            continue;
        };
        // 유니코드(0)와 Windows(3)만 — 매킨토시 표(1)는 유니코드 글자 번호가 아니다.
        if platform != 0 && platform != 3 {
            continue;
        }
        if let Some(table) = cmap_subtable(cmap, offset as usize) {
            out.push(table);
        }
    }
    out
}

fn cmap_subtable(cmap: &[u8], at: usize) -> Option<CmapTable> {
    match be16(cmap, at)? {
        0 => {
            let glyphs = (0..256)
                .map(|i| cmap.get(at + 6 + i).map(|&g| u16::from(g)))
                .collect::<Option<Vec<u16>>>()?;
            Some(CmapTable::Trimmed { first: 0, glyphs })
        }
        4 => {
            let segs = usize::from(be16(cmap, at + 6)? / 2);
            let ends = at + 14;
            let starts = ends + segs * 2 + 2;
            let deltas = starts + segs * 2;
            let ranges = deltas + segs * 2;
            let glyph_array = ranges + segs * 2;
            let length = usize::from(be16(cmap, at + 2)?);
            let end_of_table = (at + length).min(cmap.len());
            let glyphs = (glyph_array..end_of_table.max(glyph_array))
                .step_by(2)
                .map(|p| be16(cmap, p).unwrap_or(0))
                .collect::<Vec<u16>>();
            let mut segments = Vec::with_capacity(segs);
            for s in 0..segs {
                let end = be16(cmap, ends + s * 2)?;
                let start = be16(cmap, starts + s * 2)?;
                let delta = be16(cmap, deltas + s * 2)? as i16;
                let range_offset = be16(cmap, ranges + s * 2)?;
                // 글리프 = *(&idRangeOffset[s] + idRangeOffset[s]/2 + (c - start)) — 배열 첫 칸부터의 차례로 바꾼다.
                let here = ranges + s * 2;
                let target = here + usize::from(range_offset);
                let at = target.saturating_sub(glyph_array) / 2;
                segments.push((end, start, delta, range_offset, at));
            }
            Some(CmapTable::Segments { segments, glyphs })
        }
        6 => {
            let first = u32::from(be16(cmap, at + 6)?);
            let count = usize::from(be16(cmap, at + 8)?);
            let glyphs = (0..count)
                .map(|i| be16(cmap, at + 10 + i * 2))
                .collect::<Option<Vec<u16>>>()?;
            Some(CmapTable::Trimmed { first, glyphs })
        }
        12 => {
            let count = be32(cmap, at + 12)? as usize;
            let groups = (0..count)
                .map(|i| {
                    let g = at + 16 + i * 12;
                    Some((be32(cmap, g)?, be32(cmap, g + 4)?, be32(cmap, g + 8)?))
                })
                .collect::<Option<Vec<_>>>()?;
            Some(CmapTable::Groups(groups))
        }
        _ => None,
    }
}

fn be16(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

fn be32(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

/// 이 기계에서 ClosedXML 이 고를 글꼴 — `Calibri`, 없으면 `Microsoft Sans Serif`. 둘 다 없으면 `None` 이다(ClosedXML 은
/// 제가 품은 CarlitoBare 를 쓴다 — Windows 가 아닌 기계에서만 생기는 일이라 옮기지 않았다).
pub fn workbook_font() -> Option<&'static Metrics> {
    static FONT: OnceLock<Option<Metrics>> = OnceLock::new();
    FONT.get_or_init(|| {
        let files = font_files();
        find(&files, "Calibri").or_else(|| find(&files, "Microsoft Sans Serif"))
    })
    .as_ref()
}

/// SixLabors.Fonts 의 Windows 시스템 글꼴 자리.
fn font_files() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(root) = std::env::var_os("SystemRoot").or_else(|| std::env::var_os("WINDIR")) {
        dirs.push(PathBuf::from(root).join("Fonts"));
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        dirs.push(
            PathBuf::from(local)
                .join("Microsoft")
                .join("Windows")
                .join("Fonts"),
        );
    }
    let mut files = Vec::new();
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut found: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.extension().and_then(|e| e.to_str()).is_some_and(|e| {
                    matches!(e.to_ascii_lowercase().as_str(), "ttf" | "otf" | "ttc")
                })
            })
            .collect();
        found.sort();
        files.extend(found);
    }
    files
}

/// 이름표가 `family` 인 보통체(굵지도 기울지도 않은 것) 하나.
fn find(files: &[PathBuf], family: &str) -> Option<Metrics> {
    for path in files {
        for index in 0..faces(path) {
            let Some((name, regular)) = describe(path, index) else {
                continue;
            };
            if regular && name.eq_ignore_ascii_case(family) {
                let data = std::fs::read(path).ok()?;
                if let Some(metrics) = Metrics::parse(&data, index) {
                    return Some(metrics);
                }
            }
        }
    }
    None
}

/// TTC 면 그 안의 글꼴 수, 아니면 1.
fn faces(path: &Path) -> usize {
    let mut head = [0u8; 12];
    let Ok(mut file) = File::open(path) else {
        return 0;
    };
    if file.read_exact(&mut head).is_err() {
        return 0;
    }
    if &head[0..4] == b"ttcf" {
        be32(&head, 8).map_or(0, |n| n as usize)
    } else {
        1
    }
}

/// 글꼴 하나의 이름(name 1)과 보통체인지. 파일 전체를 읽지 않고 필요한 표만 읽는다.
fn describe(path: &Path, index: usize) -> Option<(String, bool)> {
    let mut file = File::open(path).ok()?;
    let read = |file: &mut File, at: u64, len: usize| -> Option<Vec<u8>> {
        file.seek(SeekFrom::Start(at)).ok()?;
        let mut buf = vec![0u8; len];
        file.read_exact(&mut buf).ok()?;
        Some(buf)
    };
    let head = read(&mut file, 0, 16)?;
    let start = if &head[0..4] == b"ttcf" {
        let offset = read(&mut file, 12 + index as u64 * 4, 4)?;
        be32(&offset, 0)? as u64
    } else {
        0
    };
    let directory = read(&mut file, start, 12)?;
    let count = usize::from(be16(&directory, 4)?);
    let records = read(&mut file, start + 12, count * 16)?;
    let table = |tag: &[u8; 4]| -> Option<(u64, usize)> {
        (0..count).find_map(|i| {
            let r = i * 16;
            (records.get(r..r + 4)? == tag).then(|| {
                Some((
                    be32(&records, r + 8)? as u64,
                    be32(&records, r + 12)? as usize,
                ))
            })?
        })
    };
    let (name_at, name_len) = table(b"name")?;
    let name = read(&mut file, name_at, name_len)?;
    let family = family_name(&name)?;
    let regular = match table(b"OS/2") {
        Some((at, len)) if len >= 64 => {
            let os2 = read(&mut file, at, 64)?;
            let selection = be16(&os2, 62)?;
            selection & 0x21 == 0
        }
        _ => {
            let (at, _) = table(b"head")?;
            let head = read(&mut file, at, 54)?;
            be16(&head, 44)? & 0x3 == 0
        }
    };
    Some((family, regular))
}

/// name 표의 1번(글꼴 가족) — Windows 영어(미국)를 먼저, 없으면 Windows 아무것, 없으면 매킨토시 로마자.
fn family_name(name: &[u8]) -> Option<String> {
    let count = usize::from(be16(name, 2)?);
    let storage = usize::from(be16(name, 4)?);
    let mut best: Option<(u8, String)> = None;
    for i in 0..count {
        let r = 6 + i * 12;
        let platform = be16(name, r)?;
        let language = be16(name, r + 4)?;
        let id = be16(name, r + 6)?;
        if id != 1 {
            continue;
        }
        let len = usize::from(be16(name, r + 8)?);
        let offset = usize::from(be16(name, r + 10)?);
        let bytes = name.get(storage + offset..storage + offset + len)?;
        let (rank, text) = match platform {
            3 => {
                let units: Vec<u16> = bytes
                    .chunks_exact(2)
                    .map(|c| u16::from_be_bytes([c[0], c[1]]))
                    .collect();
                let rank = if language == 0x409 { 0 } else { 1 };
                (rank, String::from_utf16_lossy(&units))
            }
            1 => (2, bytes.iter().map(|&b| b as char).collect()),
            _ => continue,
        };
        if best.as_ref().is_none_or(|(r, _)| rank < *r) {
            best = Some((rank, text));
        }
    }
    best.map(|(_, t)| t)
}

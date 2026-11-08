//! 확장이 붙고 끊긴 기록(C# `Erp/ExtensionLog.cs`, ADR-035·036). 홈의 `extension-log.txt` 에 한 줄씩 — 탭으로 가른 다섯
//! 칸(시각 · 사건 · 브라우저 · 판 · 앞의 판 또는 오류 코드).
//!
//! Edge·Chrome 의 호스트(그리고 C# 호스트)가 함께 쓸 수 있다. 쓰는 쪽은 파일을 홀로 열고(읽기만 나눈다) 막히면 25ms 씩
//! 40번까지 다시 열어, 줄이 섞이지 않는다. 이레가 지난 줄과 500줄을 넘는 앞쪽을 버린다.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use pclm_core::local_time::LocalDateTime;
use pclm_home::dotnet;

use crate::status::accepts;

pub const CONNECTED: &str = "connected";
pub const DISCONNECTED: &str = "disconnected";
/// 붙을 때 그 브라우저의 판이 바로 앞 기록과 다르다.
pub const UPDATED: &str = "updated";
/// 확장의 수집 흐름이 실패했다. 다섯째 칸이 그 코드다.
pub const ERROR: &str = "error";
/// 결과를 확인하지 못한 저장을 같은 요청으로 다시 보내 풀었다.
pub const RECOVERED: &str = "recovered";
/// 확장이 상시 포트로 보내는 사건의 메서드.
pub const EVENT_METHOD: &str = "event";

pub const MAX_LINES: usize = 500;
pub const KEEP_DAYS: i64 = 7;

/// 한 줄. `previous` 는 판 바뀜이면 그 앞의 판, 오류면 그 코드, 아니면 빈 글.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub at: String,
    pub event: String,
    pub browser: String,
    pub version: String,
    pub previous: String,
}

/// 오류 하나. `recovered` 는 그 뒤 같은 브라우저가 다음 오류 전에 다시 보내 풀었는가.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    pub at: String,
    pub browser: String,
    pub code: String,
    pub recovered: bool,
}

pub fn log_path(directory: &Path) -> PathBuf {
    directory.join("extension-log.txt")
}

/// 붙음. 그 브라우저의 바로 앞 기록과 판이 다르면 「판 바뀜」 을 먼저 남긴다.
pub fn record_connected(
    directory: &Path,
    browser: &str,
    version: &str,
    now: LocalDateTime,
) -> io::Result<()> {
    if !accepts(browser, version) {
        return Ok(());
    }
    write(directory, now, |entries| {
        let at = now.second_text();
        let previous = entries
            .iter()
            .rev()
            .find(|e| e.browser == browser)
            .map(|e| e.version.clone());
        let mut added = Vec::new();
        if let Some(previous) = previous.filter(|p| p != version) {
            added.push(entry(&at, UPDATED, browser, version, &previous));
        }
        added.push(entry(&at, CONNECTED, browser, version, ""));
        added
    })
}

/// 끊김. 포트가 닫혔거나 호스트가 제 실행 파일이 비켜진 것을 보고 스스로 떠났다.
pub fn record_disconnected(
    directory: &Path,
    browser: &str,
    version: &str,
    now: LocalDateTime,
) -> io::Result<()> {
    if !accepts(browser, version) {
        return Ok(());
    }
    write(directory, now, |_| {
        vec![entry(
            &now.second_text(),
            DISCONNECTED,
            browser,
            version,
            "",
        )]
    })
}

/// 수집 흐름의 사건 — 오류(`code` 가 그 까닭) 또는 다시 보내 풀림. 모양이 틀리면 적지 않고 `false`.
pub fn record_event(
    directory: &Path,
    browser: &str,
    version: &str,
    kind: &str,
    code: Option<&str>,
    now: LocalDateTime,
) -> io::Result<bool> {
    if !accepts(browser, version) {
        return Ok(false);
    }
    if kind == ERROR && !code.is_some_and(code_shaped) {
        return Ok(false);
    }
    if kind != ERROR && kind != RECOVERED {
        return Ok(false);
    }
    let previous = if kind == ERROR {
        code.unwrap_or_default()
    } else {
        ""
    };
    write(directory, now, |_| {
        vec![entry(&now.second_text(), kind, browser, version, previous)]
    })?;
    Ok(true)
}

/// `\A[a-z][a-z_]{0,31}\z`.
fn code_shaped(code: &str) -> bool {
    let b = code.as_bytes();
    !b.is_empty()
        && b.len() <= 32
        && b[0].is_ascii_lowercase()
        && b.iter().all(|&c| c.is_ascii_lowercase() || c == b'_')
}

/// 오늘의 오류(일어난 차례대로)와 오늘 전의 마지막 오류.
pub fn problems(directory: &Path, now: LocalDateTime) -> (Vec<Problem>, Option<Problem>) {
    let entries = read(directory, now.add_days(-KEEP_DAYS));
    let mut problems = Vec::new();
    for (i, e) in entries.iter().enumerate() {
        if e.event != ERROR {
            continue;
        }
        let mut recovered = false;
        for later in &entries[i + 1..] {
            if later.browser != e.browser {
                continue;
            }
            if later.event == ERROR {
                break;
            }
            if later.event == RECOVERED {
                recovered = true;
                break;
            }
        }
        problems.push(Problem {
            at: e.at.clone(),
            browser: e.browser.clone(),
            code: e.previous.clone(),
            recovered,
        });
    }
    let today = now.date().second_text();
    let earlier = problems
        .iter()
        .rev()
        .find(|p| ordinal(&p.at) < ordinal(&today))
        .cloned();
    (
        problems
            .into_iter()
            .filter(|p| ordinal(&p.at) >= ordinal(&today))
            .collect(),
        earlier,
    )
}

fn ordinal(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// `since` 부터의 기록, 일어난 차례대로. 없거나 못 읽으면 빈 목록.
pub fn read(directory: &Path, since: LocalDateTime) -> Vec<Entry> {
    let from = since.second_text();
    let text = (|| -> io::Result<String> {
        let mut file = retry(|| {
            OpenOptions::new()
                .read(true)
                .share_mode(SHARE_READ | SHARE_WRITE | SHARE_DELETE)
                .open(log_path(directory))
        })?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        // StreamReader(stream, UTF8) — 머리표를 보고 푼다.
        Ok(dotnet::decode_text(&bytes))
    })();
    let Ok(text) = text else { return Vec::new() };
    parse(&text)
        .into_iter()
        .map(|(_, e)| e)
        .filter(|e| ordinal(&e.at) >= ordinal(&from))
        .collect()
}

const SHARE_READ: u32 = 1;
const SHARE_WRITE: u32 = 2;
const SHARE_DELETE: u32 = 4;

fn entry(at: &str, event: &str, browser: &str, version: &str, previous: &str) -> Entry {
    Entry {
        at: at.into(),
        event: event.into(),
        browser: browser.into(),
        version: version.into(),
        previous: previous.into(),
    }
}

fn line(e: &Entry) -> String {
    [e.at.as_str(), &e.event, &e.browser, &e.version, &e.previous].join("\t")
}

/// 반쯤 쓴 줄·모르는 사건은 건너뛴다 — 다음에 쓸 때 줄이면서 걷힌다.
fn parse(text: &str) -> Vec<(String, Entry)> {
    let mut entries = Vec::new();
    for raw in text.split('\n') {
        let cells: Vec<&str> = raw.trim_end_matches('\r').split('\t').collect();
        if cells.len() != 5
            || pclm_core::erp::wire::utf16_len(cells[0]) != 19
            || !stamp_shaped(cells[0])
            || ![CONNECTED, DISCONNECTED, UPDATED, ERROR, RECOVERED].contains(&cells[1])
            || !accepts(cells[2], cells[3])
        {
            continue;
        }
        entries.push((
            cells.join("\t"),
            entry(cells[0], cells[1], cells[2], cells[3], cells[4]),
        ));
    }
    entries
}

/// `DateTime.TryParseExact(s, "yyyy-MM-ddTHH:mm:ss", Invariant, None)` — 날짜가 실제로 있어야 한다.
fn stamp_shaped(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 19
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
    {
        return false;
    }
    let number = |from: usize, len: usize| -> Option<u32> {
        let part = &b[from..from + len];
        part.iter()
            .all(u8::is_ascii_digit)
            .then(|| part.iter().fold(0u32, |n, d| n * 10 + u32::from(d - b'0')))
    };
    let (Some(year), Some(month), Some(day), Some(hour), Some(minute), Some(second)) = (
        number(0, 4),
        number(5, 2),
        number(8, 2),
        number(11, 2),
        number(14, 2),
        number(17, 2),
    ) else {
        return false;
    };
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    year >= 1 && (1..=days).contains(&day) && hour < 24 && minute < 60 && second < 60
}

fn write(
    directory: &Path,
    now: LocalDateTime,
    add: impl FnOnce(&[Entry]) -> Vec<Entry>,
) -> io::Result<()> {
    std::fs::create_dir_all(directory)?;
    let mut file = retry(|| {
        OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .share_mode(SHARE_READ)
            .open(log_path(directory))
    })?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    // 쓰는 쪽은 머리표를 보지 않는다(detectEncodingFromByteOrderMarks: false).
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let lines = text
        .split('\n')
        .filter(|l| !l.trim_end_matches('\r').is_empty())
        .count();
    let parsed = parse(&text);
    let entries: Vec<Entry> = parsed.iter().map(|(_, e)| e.clone()).collect();
    let added = add(&entries);

    let cutoff = ordinal(&now.add_days(-KEEP_DAYS).second_text());
    let mut kept: Vec<String> = parsed
        .into_iter()
        .filter(|(_, e)| ordinal(&e.at) >= cutoff)
        .map(|(l, _)| l)
        .chain(added.iter().map(line))
        .collect();
    if kept.len() > MAX_LINES {
        kept.drain(..kept.len() - MAX_LINES);
    }

    if kept.len() == lines + added.len() {
        // 버릴 것이 없다 — 끝에 덧붙이기만 한다.
        let length = file.seek(SeekFrom::End(0))?;
        let mut tail = String::new();
        if length > 0 && !text.ends_with('\n') {
            tail.push('\n');
        }
        for e in &added {
            tail.push_str(&line(e));
            tail.push('\n');
        }
        file.write_all(tail.as_bytes())?;
    } else {
        let body: String = kept.iter().map(|l| format!("{l}\n")).collect();
        file.set_len(0)?;
        file.seek(SeekFrom::Start(0))?;
        file.write_all(body.as_bytes())?;
    }
    file.flush()
}

/// 다른 호스트가 쥐고 있으면 잠깐 기다려 다시 연다. 오래 막히면 던진다 — 부르는 쪽이 삼킨다. 없는 파일·폴더와 권한
/// 오류는 기다리지 않는다(C# 의 `FileNotFoundException`·`DirectoryNotFoundException`·`UnauthorizedAccessException`).
fn retry(open: impl Fn() -> io::Result<File>) -> io::Result<File> {
    let mut attempt = 0;
    loop {
        match open() {
            Ok(file) => return Ok(file),
            Err(e) if attempt < 40 && !matches!(e.raw_os_error(), Some(2 | 3 | 5)) => {
                attempt += 1;
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(e) => return Err(e),
        }
    }
}

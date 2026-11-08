//! 정규 덤프 1판(`crates/pclm-parity/README.md` 가 정본이다). C# 쪽(`tests/Pclm.Parity/Dump.cs`)과
//! **바이트까지** 같아야 한다 — 두 도구는 재는 자다. 갈리면 자를 고치지, 가리개(`--mask`)로 덮지 않는다.
//!
//! 줄의 차례는 `String` 의 차례, 곧 UTF-8 바이트 차례다. C# 쪽이 UTF-16 단위가 아니라 바이트로 견주는 까닭이 이것과
//! 맞추려는 것이다.

use std::fmt::Write as _;
use std::path::Path;

use rusqlite::types::ValueRef;
use rusqlite::{Connection, OpenFlags};

/// `--mask <object>.<column>`. `object` 가 `*` 이면 모든 표·뷰의 그 열이다.
pub struct Mask {
    pub object: String,
    pub column: String,
    /// `--mask-hex` — 칸 전체가 아니라 글 안의 32자 소문자 16진 덩어리만 `m` 으로([`mask_hex`]).
    pub hex: bool,
}

/// `--mask-hex` 가 가리는 것 — 앞뒤가 16진 글자가 아닌, 꼭 32자인 소문자 16진 덩어리(`Guid.ToString("N")`)를 `m` 으로.
/// C# 쪽 정규식 `(?<![0-9a-f])[0-9a-f]{32}(?![0-9a-f])` 와 같다.
pub fn mask_hex(text: &str) -> String {
    let is_hex = |c: char| matches!(c, '0'..='9' | 'a'..='f');
    let mut out = String::with_capacity(text.len());
    let mut run = String::new();
    let flush = |run: &mut String, out: &mut String| {
        if run.len() == 32 {
            out.push('m');
        } else {
            out.push_str(run);
        }
        run.clear();
    };
    for c in text.chars() {
        if is_hex(c) {
            run.push(c);
        } else {
            flush(&mut run, &mut out);
            out.push(c);
        }
    }
    flush(&mut run, &mut out);
    out
}

pub fn run(path: &Path, masks: &[Mask], skip_views: bool) -> rusqlite::Result<Vec<u8>> {
    // 읽기 전용으로 열고 query_only 까지 건다 — C# 쪽의 Mode=ReadOnly + PRAGMA query_only 와 같다.
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    connection.execute_batch("PRAGMA query_only = ON;")?;

    let user_version: i64 = connection.query_row("PRAGMA user_version;", [], |r| r.get(0))?;
    let application_id: i64 = connection.query_row("PRAGMA application_id;", [], |r| r.get(0))?;
    let mut output = vec![
        "pclm-parity-dump 1".to_owned(),
        format!("user_version {user_version}"),
        format!("application_id {application_id}"),
    ];

    let mut objects: Vec<(String, String, String, Option<String>)> = Vec::new();
    {
        let mut statement =
            connection.prepare("SELECT type, name, tbl_name, sql FROM sqlite_master;")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let name: String = row.get(1)?;
            let kind: String = row.get(0)?;
            // --skip-views 는 뷰를 통째로 뺀다 — 정의도 행도(C# 쪽과 같다).
            if name.starts_with("sqlite_stat") || (skip_views && kind == "view") {
                continue;
            }
            objects.push((kind, name, row.get(2)?, row.get(3)?));
        }
    }

    let mut lines: Vec<String> = objects
        .iter()
        .map(|(kind, name, table, sql)| {
            let sql = sql.as_deref().map_or_else(|| "N".to_owned(), escape);
            format!("object\t{kind}\t{}\t{}\t{sql}", escape(name), escape(table))
        })
        .collect();
    lines.sort();
    output.extend(lines);

    for table in by_name(&objects, "table") {
        let (columns, mut rows) = select(&connection, table, masks)?;
        output.push(format!("table\t{}\t{}", escape(table), rows.len()));
        output.push(columns_line(&columns));
        rows.sort();
        output.extend(rows);
    }

    if !skip_views {
        for view in by_name(&objects, "view") {
            match select(&connection, view, masks) {
                Ok((columns, mut rows)) => {
                    output.push(format!(
                        "view\t{}\t{}\t{}",
                        escape(view),
                        rows.len(),
                        order_hash(&rows)
                    ));
                    output.push(columns_line(&columns));
                    rows.sort();
                    output.extend(rows);
                }
                Err(error) => {
                    output.push(format!(
                        "view\t{}\terror\t{}",
                        escape(view),
                        escape(&native_message(&error))
                    ));
                }
            }
        }
    }

    let mut bytes = Vec::new();
    for line in output {
        bytes.extend_from_slice(line.as_bytes());
        bytes.push(b'\n');
    }
    Ok(bytes)
}

/// 한 표·뷰의 열 이름과 행 줄(가린 뒤, SQLite 가 돌려준 차례). 도중에 실패하면 읽은 것을 버리고 오류를 낸다 —
/// C# 쪽도 같다.
fn select(
    connection: &Connection,
    name: &str,
    masks: &[Mask],
) -> rusqlite::Result<(Vec<String>, Vec<String>)> {
    let mut statement =
        connection.prepare(&format!("SELECT * FROM \"{}\";", name.replace('"', "\"\"")))?;
    let columns: Vec<String> = statement.column_names().map(str::to_owned).collect();
    let hits =
        |m: &Mask, column: &String| (m.object == "*" || m.object == name) && &m.column == column;
    // 칸 전체를 가리는 것이 이긴다. 아니면 16진만, 아니면 그대로.
    let masked: Vec<Option<bool>> = columns
        .iter()
        .map(|column| {
            if masks.iter().any(|m| !m.hex && hits(m, column)) {
                Some(false)
            } else if masks.iter().any(|m| m.hex && hits(m, column)) {
                Some(true)
            } else {
                None
            }
        })
        .collect();

    let mut lines = Vec::new();
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let mut line = String::from("row");
        for (i, mask) in masked.iter().enumerate() {
            line.push('\t');
            cell(row.get_ref(i)?, *mask, &mut line);
        }
        lines.push(line);
    }
    Ok((columns, lines))
}

/// 칸 하나. 저장 꼴은 값이 실제로 담긴 꼴(`sqlite3_column_type`)이다 — `ValueRef` 가 그것이다.
fn cell(value: ValueRef<'_>, masked: Option<bool>, line: &mut String) {
    match value {
        ValueRef::Null => line.push('N'),
        _ if masked == Some(false) => line.push('m'),
        ValueRef::Text(bytes) if masked == Some(true) => {
            line.push('t');
            line.push_str(&escape(&mask_hex(&String::from_utf8_lossy(bytes))));
        }
        ValueRef::Integer(n) => {
            let _ = write!(line, "i{n}");
        }
        ValueRef::Real(f) => {
            let _ = write!(line, "r{:016x}", f.to_bits());
        }
        // C# 쪽은 SQLite 가 준 UTF-8 을 .NET 문자열로 풀며 깨진 바이트를 U+FFFD 로 바꾼다. 같은 규칙(가장 긴 잘못된
        // 부분열 하나에 하나)으로 바꾼다.
        ValueRef::Text(bytes) => {
            line.push('t');
            line.push_str(&escape(&String::from_utf8_lossy(bytes)));
        }
        ValueRef::Blob(bytes) => {
            line.push('b');
            for b in bytes {
                let _ = write!(line, "{b:02x}");
            }
        }
    }
}

/// SQLite 가 낸 말 그대로(`sqlite3_errmsg`). C# 쪽은 Microsoft.Data.Sqlite 의 껍질을 벗겨 같은 말을 얻는다.
fn native_message(error: &rusqlite::Error) -> String {
    match error {
        rusqlite::Error::SqliteFailure(_, Some(message)) => message.clone(),
        other => other.to_string(),
    }
}

/// FNV-1a 64비트 — 줄마다 UTF-8 바이트와 LF 를, SQLite 가 돌려준 차례대로. 16자리 소문자.
fn order_hash(rows: &[String]) -> String {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0100_0000_01b3;
    let mut hash = OFFSET;
    for row in rows {
        for b in row.bytes().chain(std::iter::once(b'\n')) {
            hash = (hash ^ u64::from(b)).wrapping_mul(PRIME);
        }
    }
    format!("{hash:016x}")
}

/// 글 칸·SQL·이름의 이스케이프 — 역빗금·탭·LF·CR 넷만. 나머지는 그대로다.
pub fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '\t' => escaped.push_str("\\t"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            other => escaped.push(other),
        }
    }
    escaped
}

fn columns_line(columns: &[String]) -> String {
    let mut line = String::from("columns");
    for column in columns {
        line.push('\t');
        line.push_str(&escape(column));
    }
    line
}

fn by_name<'a>(
    objects: &'a [(String, String, String, Option<String>)],
    kind: &str,
) -> Vec<&'a str> {
    let mut names: Vec<&str> = objects
        .iter()
        .filter(|(k, ..)| k == kind)
        .map(|(_, name, ..)| name.as_str())
        .collect();
    names.sort_unstable();
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_touches_only_four_characters() {
        assert_eq!(escape("a\\b\tc\nd\re 𝄞"), "a\\\\b\\tc\\nd\\re 𝄞");
    }

    #[test]
    fn order_hash_of_nothing_is_the_offset() {
        assert_eq!(order_hash(&[]), "cbf29ce484222325");
    }

    #[test]
    fn mask_hex_takes_only_whole_32_runs() {
        let id = "0123456789abcdef0123456789abcdef";
        assert_eq!(mask_hex(&format!("{{\"id\":\"{id}\"}}")), "{\"id\":\"m\"}");
        assert_eq!(mask_hex(&format!("c-{id}")), "c-m");
        assert_eq!(mask_hex(&format!("{id}0")), format!("{id}0"));
        assert_eq!(mask_hex(&id.to_uppercase()), id.to_uppercase());
        assert_eq!(mask_hex(&format!("{id}-{id}")), "m-m");
    }

    #[test]
    fn cells_follow_the_stored_class_not_the_declared_type() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE t (n INTEGER, r REAL, b TEXT);
                 INSERT INTO t VALUES ('글자', 0.1, X'00ff');",
            )
            .unwrap();
        let (_, rows) = select(&connection, "t", &[]).unwrap();
        assert_eq!(rows, ["row\tt글자\tr3fb999999999999a\tb00ff"]);
    }
}

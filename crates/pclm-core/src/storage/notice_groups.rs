//! 수집된 관련공고·접수 참조로 공고건을 재구성한다(C# `Storage/NoticeGroups.cs`).

use std::collections::HashMap;

use rusqlite::{Connection, OptionalExtension, params};

use crate::domain::value_parser::split_notice_ref;
use crate::error::Result;
use crate::text::cmp_ordinal;

/// `NoticeGroups.Kind` — 종류와 상태를 따로 수집한다. 이미 완성된 다른 화면의 공고종류는 그대로 표시한다.
pub(crate) fn kind(kind: &str, status: &str) -> String {
    format!(
        "CASE WHEN NULLIF({kind}, '') IS NOT NULL AND {status} IN ('일반공고','변경공고','취소공고','재공고')\n  \
         AND {kind} NOT IN ('일반공고','변경공고','취소공고','재공고')\n  \
         AND {kind} NOT LIKE '%(일반공고)' AND {kind} NOT LIKE '%(변경공고)'\n  \
         AND {kind} NOT LIKE '%(취소공고)' AND {kind} NOT LIKE '%(재공고)'\n\
         THEN {kind} || '(' || {status} || ')' ELSE COALESCE(NULLIF({kind}, ''), {status}) END"
    )
}

/// `NoticeGroups.현행공고` — 명시 대체 관계와 차수로 남긴 후보가 하나이거나, 여러 후보 중 재공고가 하나면 현행이다.
/// (`group_base`, `notice_base`, `seq`) 를 내는 부질의.
pub(crate) fn current_notice() -> String {
    format!(
        "SELECT group_base, notice_base, seq FROM (
    SELECT 유효.*, COUNT(*) OVER (PARTITION BY group_base) AS 후보수,
           SUM(재공고) OVER (PARTITION BY group_base) AS 재공고수
    FROM (
        SELECT 계열.group_base, 현행.notice_base, 현행.seq,
               CASE WHEN COALESCE(
                   (SELECT o.value FROM field_override o
                    WHERE o.entity_type = 'notice' AND o.base = 현행.notice_base
                      AND o.seq = 현행.seq AND o.column_name = '공고종류'),
                   {})
                   IN ('재공고', '실공고(재공고)') THEN 1 ELSE 0 END AS 재공고
        FROM notice 현행
        JOIN notice_series 계열 ON 계열.notice_base = 현행.notice_base
        WHERE CAST(현행.seq AS INTEGER) =
              (SELECT MAX(CAST(x.seq AS INTEGER)) FROM notice x WHERE x.notice_base = 현행.notice_base)
          AND NOT EXISTS (
              SELECT 1 FROM notice_relation r
              WHERE r.related_base = 현행.notice_base AND r.related_seq = 현행.seq
                AND r.notice_base <> 현행.notice_base)
    ) 유효
) 후보
WHERE 후보수 = 1 OR (후보수 > 1 AND 재공고수 = 1 AND 재공고 = 1)",
        kind("현행.notice_kind", "현행.source_status")
    )
}

/// `NoticeGroups.GroupOf` — 이 본번호가 속한 건. 계열이 없으면 `None`.
pub fn group_of(connection: &Connection, notice_base: &str) -> Result<Option<String>> {
    Ok(connection
        .query_row(
            "SELECT group_base FROM notice_series WHERE notice_base = ?1;",
            [notice_base],
            |r| r.get(0),
        )
        .optional()?
        .flatten())
}

/// `NoticeGroups.Rebuild` — 링크 셋을 비우고 관련공고를 해소한 뒤 건을 다시 짓는다. 부르는 쪽의 트랜잭션 안에서 돈다.
pub(crate) fn rebuild(connection: &Connection) -> Result<()> {
    connection.execute_batch(
        "DELETE FROM request_item_link; DELETE FROM request_link; DELETE FROM project_link;",
    )?;
    resolve_relations(connection)?;
    let roots = components(connection)?;
    let mut inserted = std::collections::HashSet::new();
    for (_, root) in &roots {
        if inserted.insert(root.clone()) {
            connection.execute(
                "INSERT OR IGNORE INTO notice_group(group_base) VALUES(?1)",
                [root],
            )?;
        }
    }
    for (number, root) in &roots {
        connection.execute(
            "UPDATE notice_series SET group_base=?1 WHERE notice_base=?2",
            params![root, number],
        )?;
    }
    connection.execute(
        "DELETE FROM notice_group WHERE group_base NOT IN (SELECT group_base FROM notice_series)",
        [],
    )?;
    Ok(())
}

/// `NoticeGroups.해소` — 적힌 관련공고를 본번호와 차수로 갈라 옆 칸에 둔다. 이미 같은 값이면 건드리지 않는다.
fn resolve_relations(connection: &Connection) -> Result<()> {
    type Row = (String, String, i64, String, Option<String>, Option<String>);
    let rows: Vec<Row> = connection
        .prepare(
            "SELECT notice_base, seq, line_no, related, related_base, related_seq FROM notice_relation;",
        )?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?
        .collect::<rusqlite::Result<_>>()?;
    for (base, seq, line, related, old_base, old_seq) in rows {
        let split = split_notice_ref(Some(&related));
        let (new_base, new_seq) = match &split {
            Some((b, s)) => (Some(b.as_str()), Some(s.as_str())),
            None => (None, None),
        };
        if new_base == old_base.as_deref() && new_seq == old_seq.as_deref() {
            continue;
        }
        connection.execute(
            "UPDATE notice_relation SET related_base = ?1, related_seq = ?2\n\
             WHERE notice_base = ?3 AND seq = ?4 AND line_no = ?5;",
            params![new_base, new_seq, base, seq, line],
        )?;
    }
    Ok(())
}

/// 넣은 차례를 지키는 사전 — C# `Dictionary` 는 지우지 않는 한 넣은 차례로 돈다. 건을 넣는 차례가 행의
/// rowid 차례가 되므로 그대로 따른다.
#[derive(Default)]
struct Ordered {
    keys: Vec<String>,
    parent: HashMap<String, String>,
}

impl Ordered {
    fn plant(&mut self, number: &str) {
        if !self.parent.contains_key(number) {
            self.keys.push(number.to_string());
            self.parent.insert(number.to_string(), number.to_string());
        }
    }

    fn find(&mut self, number: &str) -> String {
        let mut root = number.to_string();
        while self.parent[&root] != root {
            root = self.parent[&root].clone();
        }
        // 길을 접어 둔다.
        let mut step = number.to_string();
        while step != root {
            let next = self.parent[&step].clone();
            self.parent.insert(step, root.clone());
            step = next;
        }
        root
    }
}

/// `NoticeGroups.성분` — 관련공고 간선을 풀어 본번호 → 그 건의 이름(성분의 Ordinal 최소 본번호).
fn components(connection: &Connection) -> Result<Vec<(String, String)>> {
    let mut sets = Ordered::default();
    let strings = |sql: &str| -> Result<Vec<String>> {
        Ok(connection
            .prepare(sql)?
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?)
    };
    for number in strings("SELECT notice_base FROM notice_series;")? {
        sets.plant(&number);
    }
    for number in strings(
        "SELECT DISTINCT related_base FROM notice_relation WHERE related_base IS NOT NULL;",
    )? {
        sets.plant(&number);
    }
    let edges: Vec<(String, String)> = connection
        .prepare(
            "SELECT notice_base, related_base FROM notice_relation WHERE related_base IS NOT NULL
UNION
SELECT a.notice_base, b.notice_base FROM notice_item a JOIN notice_item b
  ON a.ref_request_base=b.ref_request_base AND a.notice_base<b.notice_base
WHERE coalesce(a.ref_request_base,'')<>''
  AND CAST(a.seq AS INTEGER)=(SELECT max(CAST(n.seq AS INTEGER)) FROM notice n WHERE n.notice_base=a.notice_base)
  AND CAST(b.seq AS INTEGER)=(SELECT max(CAST(n.seq AS INTEGER)) FROM notice n WHERE n.notice_base=b.notice_base)
  AND NOT EXISTS(SELECT 1 FROM notice_item x
    WHERE x.notice_base IN(a.notice_base,b.notice_base)
      AND CAST(x.seq AS INTEGER)=(SELECT max(CAST(n.seq AS INTEGER)) FROM notice n WHERE n.notice_base=x.notice_base)
      AND coalesce(x.ref_request_base,'')<>'' AND x.ref_request_base<>a.ref_request_base)",
        )?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    for (left, right) in edges {
        sets.plant(&left);
        sets.plant(&right);
        let a = sets.find(&left);
        let b = sets.find(&right);
        if cmp_ordinal(&a, &b).is_le() {
            sets.parent.insert(b, a);
        } else {
            sets.parent.insert(a, b);
        }
    }
    let keys = sets.keys.clone();
    Ok(keys
        .into_iter()
        .map(|number| {
            let root = sets.find(&number);
            (number, root)
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Env;
    use crate::storage::pclm_file;

    struct Fixed;
    impl Env for Fixed {
        fn now_utc(&self) -> String {
            "2026-11-07T03:04:05.1234567Z".into()
        }
        fn new_id(&self) -> String {
            "0".repeat(32)
        }
    }

    /// 시험 픽스처의 `UpsertNotice` 몫 — 건·계열·차수 행·관련공고를 넣고 연결(건)을 다시 짓는다. 값은 지어낸 것이다.
    fn put(c: &Connection, base: &str, seq: &str, related: &[&str]) {
        let kind = if seq == "001" {
            "취소공고"
        } else {
            "등록공고"
        };
        c.execute(
            "INSERT OR IGNORE INTO notice_group (group_base) VALUES (?1);",
            [base],
        )
        .unwrap();
        c.execute(
            "INSERT OR IGNORE INTO notice_series (notice_base, group_base) VALUES (?1, ?1);",
            [base],
        )
        .unwrap();
        c.execute(
            "INSERT INTO notice (notice_base, seq, title, notice_kind, posted_at, updated_at)
             VALUES (?1, ?2, '지어낸 공고', ?3, '2026-07-18 00:00:00', '2026-01-02T03:04:05.0000000Z');",
            params![base, seq, kind],
        )
        .unwrap();
        for (i, r) in related.iter().enumerate() {
            let split = split_notice_ref(Some(r));
            c.execute(
                "INSERT INTO notice_relation (notice_base, seq, line_no, related, related_base, related_seq)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6);",
                params![
                    base,
                    seq,
                    (i + 1) as i64,
                    r,
                    split.as_ref().map(|s| s.0.clone()),
                    split.as_ref().map(|s| s.1.clone())
                ],
            )
            .unwrap();
        }
        crate::erp::explicit_links::resolve(c).unwrap();
    }

    fn read(c: &Connection, sql: &str) -> Vec<String> {
        let mut s = c.prepare(sql).unwrap();
        s.query_map([], |r| {
            Ok(format!(
                "{}|{}",
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?
            ))
        })
        .unwrap()
        .map(|r| r.unwrap())
        .collect()
    }

    /// C# `NoticeGroupTests.뷰의_현행공고가_정본과_같다` — 뷰가 고르는 현행공고와 정본 부질의(`현행공고`)가 같다. 규칙이 두 벌로
    /// 갈리면 화면과 뷰가 다른 공고를 현행으로 세운다. 실측의 세 모양(재채번·앞가리킴·문서 없는 조상)을 함께 넣는다.
    #[test]
    fn view_current_notice_is_the_canonical_rule() {
        let directory = std::env::temp_dir().join(format!(
            "pclm-core-notice-groups-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("건.pclm");
        let database = pclm_file::create(&path, "work", &Fixed).unwrap();
        {
            let c = database.open().unwrap();
            // 조립대 — 당초 → 취소 → 재채번된 재공고.
            put(&c, "R26BK98000154", "000", &[]);
            put(&c, "R26BK98000154", "001", &["R26BK98000154-000"]);
            put(
                &c,
                "R26BK98000282",
                "000",
                &["R26BK98000154-001", "R26BK98000154-000"],
            );
            // 절단기 — 재출력본이 자기 뒤차수를 가리킨다.
            put(&c, "R26BK98000530", "000", &["R26BK98000530-001"]);
            put(&c, "R26BK98000530", "001", &["R26BK98000530-000"]);
            // 가스성분분석기 — 가리키는 조상이 DB 에 없다.
            put(&c, "R26BK98000491", "000", &["R26BK98000319-000"]);

            let expected = [
                "R26BK98000154|R26BK98000282-000",
                "R26BK98000319|R26BK98000491-000",
                "R26BK98000530|R26BK98000530-001",
            ];
            assert_eq!(
                read(
                    &c,
                    &format!(
                        "SELECT group_base, notice_base || '-' || seq FROM ({}) ORDER BY group_base;",
                        current_notice()
                    )
                ),
                expected
            );
            assert_eq!(
                read(
                    &c,
                    "SELECT DISTINCT 공고건, 현행공고 FROM v_공고 ORDER BY 공고건;"
                ),
                expected
            );
        }
        drop(database);
        let _ = std::fs::remove_dir_all(&directory);
    }
}

//! 연결은 수집된 명시 참조의 파생값이다(C# `Erp/ExplicitLinks.cs`, ADR-040). 재수집·삭제·취합·판올림마다 같은
//! 입력에서 다시 짓는다.

use std::collections::HashMap;

use rusqlite::{Connection, params};

use crate::error::Result;
use crate::storage::notice_groups;

/// C# `ReferenceState(EntityType, Base, Target, State)` — 참조 하나의 해소 결과.
/// `state` 는 `연결됨`·`참조 충돌`·`미수집`·`참조 없음` 이다.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceState {
    pub entity_type: String,
    pub base: String,
    pub target: String,
    pub state: String,
}

const LINKED: &str = "연결됨";
const CONFLICT: &str = "참조 충돌";
const MISSING: &str = "미수집";
const NONE: &str = "참조 없음";

/// `ExplicitLinks.Resolve(connection, transaction)` — 공고건을 다시 짓고 접수·계약·품목 연결을 명시 참조에서
/// 다시 세운다. 부르는 쪽의 트랜잭션 안에서 돈다.
pub fn resolve(c: &Connection) -> Result<Vec<ReferenceState>> {
    notice_groups::rebuild(c)?;
    let mut result = Vec::new();
    let references: Vec<(String, String)> = c
        .prepare(
            "SELECT DISTINCT s.group_base, i.ref_request_base
FROM notice_item i JOIN notice_series s USING(notice_base)
WHERE coalesce(i.ref_request_base,'')<>''
  AND CAST(i.seq AS INTEGER)=(SELECT max(CAST(n.seq AS INTEGER)) FROM notice n WHERE n.notice_base=i.notice_base)",
        )?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    for (group, request) in &references {
        let conflict = references.iter().filter(|r| &r.0 == group).count() != 1
            || references.iter().filter(|r| &r.1 == request).count() != 1;
        let collected: i64 = c.query_row(
            "SELECT count(*) FROM request_series WHERE request_base=?1",
            [request],
            |r| r.get(0),
        )?;
        let state = if conflict {
            CONFLICT
        } else if collected == 0 {
            MISSING
        } else {
            LINKED
        };
        if state == LINKED {
            c.execute(
                "INSERT INTO request_link(request_base,notice_group) VALUES(?1,?2)",
                params![request, group],
            )?;
        }
        result.push(ReferenceState {
            entity_type: "request".into(),
            base: request.clone(),
            target: group.clone(),
            state: state.into(),
        });
    }

    let contracts: Vec<(String, String, String)> = c
        .prepare(
            "SELECT contract_base, coalesce(ref_notice_base,''), coalesce(ref_request_base,'') FROM contract k
WHERE CAST(seq AS INTEGER)=(SELECT max(CAST(x.seq AS INTEGER)) FROM contract x WHERE x.contract_base=k.contract_base)",
        )?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;
    for (base, notice, request) in contracts {
        let groups: Vec<String> = if !notice.is_empty() {
            c.prepare(
                "SELECT group_base FROM notice_series WHERE notice_base=?1
UNION
SELECT s.group_base FROM notice_relation r JOIN notice_series s USING(notice_base) WHERE r.related_base=?1",
            )?
            .query_map([&notice], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?
        } else {
            let mut seen = Vec::new();
            for (g, _) in references.iter().filter(|r| r.1 == request) {
                if !seen.contains(g) {
                    seen.push(g.clone());
                }
            }
            seen
        };
        let inconsistent = !request.is_empty()
            && groups.len() == 1
            && references
                .iter()
                .any(|r| r.0 == groups[0] && r.1 != request);
        let state = if groups.len() > 1 || inconsistent {
            CONFLICT
        } else if groups.len() == 1 {
            LINKED
        } else if notice.is_empty() && request.is_empty() {
            NONE
        } else {
            MISSING
        };
        if state == LINKED {
            c.execute(
                "INSERT INTO project_link(contract_base,notice_group) VALUES(?1,?2)",
                params![base, groups[0]],
            )?;
        }
        let target = if notice.is_empty() { request } else { notice };
        result.push(ReferenceState {
            entity_type: "contract".into(),
            base,
            target,
            state: state.into(),
        });
    }

    // 품목은 정확한 접수번호·차수·품목순번이 일치하고 대상이 유일할 때만 연결한다.
    let items: Vec<(String, String, i64, String, String, i64)> = c
        .prepare(&format!(
            "SELECT r.request_base,r.seq,r.line_no,n.notice_base,n.seq,n.line_no
FROM request_item r JOIN notice_item n ON r.ref_request_base=n.ref_request_base
  AND r.ref_request_seq=n.ref_request_seq AND r.ref_request_item=n.ref_request_item
JOIN ({}) current ON current.notice_base=n.notice_base AND current.seq=n.seq
JOIN request_link l ON l.request_base=r.request_base AND l.notice_group=current.group_base
WHERE r.ref_request_base<>'' AND r.ref_request_seq<>'' AND r.ref_request_item<>''",
            notice_groups::current_notice()
        ))?
        .query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        })?
        .collect::<rusqlite::Result<_>>()?;
    // GroupBy 는 처음 나온 차례를 지킨다. 대상이 하나인 품목만 잇는다.
    let mut order: Vec<(String, String, i64)> = Vec::new();
    let mut groups: HashMap<(String, String, i64), Vec<usize>> = HashMap::new();
    for (i, row) in items.iter().enumerate() {
        let key = (row.0.clone(), row.1.clone(), row.2);
        let entry = groups.entry(key.clone()).or_default();
        if entry.is_empty() {
            order.push(key);
        }
        entry.push(i);
    }
    for key in order {
        let [only] = groups[&key].as_slice() else {
            continue;
        };
        let (base, seq, line, notice, notice_seq, notice_line) = &items[*only];
        c.execute(
            "INSERT INTO request_item_link(request_base,request_seq,line_no,notice_base,notice_seq,notice_line_no) VALUES(?1,?2,?3,?4,?5,?6)",
            params![base, seq, line, notice, notice_seq, notice_line],
        )?;
    }
    Ok(result)
}

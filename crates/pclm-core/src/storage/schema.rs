//! 스키마 정본(C# `Storage/Schema.cs`).
//!
//! **SQL 글은 C# 원문과 한 바이트도 다르지 않다.** SQLite 는 `sqlite_master` 에 정의 원문을 담아 두므로, Rust 가
//! 지은 파일과 C# 이 지은 파일은 그 글자까지 같아야 같은 판이다(`contract/schema.txt`). `schema/*.sql` 은
//! C# 원시 문자열 리터럴(`"""`)을 컴파일러가 하는 그대로 펼친 것이다 — 닫는 `"""` 의 들여쓰기만큼 줄머리를 걷고,
//! 여는 줄 뒤·닫는 줄 앞의 줄바꿈은 넣지 않았다. 줄바꿈은 LF(저장소의 `eol=lf`)이고 C# 쪽도 LF 원본에서
//! 컴파일된다. V24 의 `{계약잇기}` 는 C# 보간과 같게 그 자리에 펼쳐 넣었다.
//!
//! 판올림은 앞으로만 간다. 빈 파일은 [`BASELINE`] 하나로 v19 를 곧장 짓고, 그 뒤는 [`STEPS`] 를 한 단계씩 밟는다.

/// `Schema.Version` — 스키마 판.
pub const VERSION: i32 = 32;

/// `Schema.BaselineVersion` — [`BASELINE`] 이 짓는 판. 이보다 옛 판은 받지 않는다.
pub const BASELINE_VERSION: i32 = 19;

/// `Schema.ExampleViewVersion` — 이 판을 **지나는** 작업자료에 서식용 예시 보기를 한 번 심는다.
pub const EXAMPLE_VIEW_VERSION: i32 = 26;

/// `Schema.ExampleViewUpgradeVersion` — 이 판을 지나는 작업자료에서 손대지 않은 v26 예시 보기를 지금 구성으로 갈아 끼운다.
pub const EXAMPLE_VIEW_UPGRADE_VERSION: i32 = 27;

/// `Schema.Baseline` — 빈 파일에 v19 를 곧장 짓는다. 원문 그대로다.
///
/// 끝의 이름표 줄(`erp_dataset` 에 `lower(hex(randomblob(16)))`)은 SQL 이 무작위 값을 짓는 자리라 판올림은 이 글을
/// 그대로 돌리지 않는다 — [`BASELINE_SEED_SQL`] 앞까지를 돌리고 이름표는 [`crate::Env::new_id`] 를 매개변수로
/// 넣는다(`sqlite_master` 에는 남지 않는 문장이라 파일의 스키마는 같다).
pub const BASELINE: &str = include_str!("schema/baseline.sql");

/// [`BASELINE`] 의 끝 — 이름표를 무작위로 뽑는 문장과 그 설명. 판올림은 이 앞까지만 돌린다.
pub(crate) const BASELINE_SEED_SQL: &str = "-- 이 DB 의 이름표. 지을 때 한 번 뽑는다.\nINSERT INTO erp_dataset (singleton, dataset_id) VALUES (1, lower(hex(randomblob(16))));";

/// [`BASELINE_SEED_SQL`] 을 매개변수로 바꾼 것.
pub(crate) const BASELINE_SEED_BOUND: &str =
    "INSERT INTO erp_dataset (singleton, dataset_id) VALUES (1, ?1);";

/// v19 → v20. 파일이 스스로 PCLM 임과 역할을 밝힌다(ADR-031). 원문 그대로다.
///
/// 가운데 `INSERT INTO pclm_file` 은 `randomblob`·`strftime('now')` 를 쓰므로 판올림은 [`V20_SEED_SQL`] 을 떼어
/// [`V20_SEED_BOUND`] 로 돌린다. `created_at` 은 C# 과 같은 꼴(밀리초 + `0000`)로 넣는다(`env::sqlite_now`).
pub const V20: &str = include_str!("schema/v20.sql");

/// [`V20`] 가운데 무작위·시각을 SQL 이 짓는 문장(설명 줄 포함).
pub(crate) const V20_SEED_SQL: &str = "-- 이름표가 없던 파일은 없지만(기준선이 심는다), 비어 있어도 판올림이 멎지 않게 새로 뽑는다.\nINSERT INTO pclm_file (singleton, dataset_id, role, created_at)\nVALUES (1,\n        COALESCE((SELECT dataset_id FROM erp_dataset WHERE singleton = 1), lower(hex(randomblob(16)))),\n        'work',\n        strftime('%Y-%m-%dT%H:%M:%f0000Z', 'now'));";

/// [`V20_SEED_SQL`] 을 매개변수로 바꾼 것. `?1` 은 새로 뽑을 때만 쓰는 이름표, `?2` 는 지은 때.
pub(crate) const V20_SEED_BOUND: &str = "INSERT INTO pclm_file (singleton, dataset_id, role, created_at)\nVALUES (1,\n        COALESCE((SELECT dataset_id FROM erp_dataset WHERE singleton = 1), ?1),\n        'work',\n        ?2);";

const V21: &str = include_str!("schema/v21.sql");
const V22: &str = include_str!("schema/v22.sql");
const V23: &str = include_str!("schema/v23.sql");
const V24: &str = include_str!("schema/v24.sql");
const V25: &str = include_str!("schema/v25.sql");
/// v25 → v26. 표는 그대로다. 서식용 예시 보기는 판올림([`crate::Database::migrate`])이 심는다.
const V26: &str = "-- 표는 그대로다. 서식용 예시 보기는 Database.Migrate 가 심는다.";
/// v26 → v27. 표는 그대로다. 손대지 않은 예시 보기는 판올림이 갈아 끼운다.
const V27: &str =
    "-- 표는 그대로다. 손대지 않은 서식용 예시 보기는 Database.Migrate 가 갈아 끼운다.";
const V28: &str = include_str!("schema/v28.sql");
const V29: &str = include_str!("schema/v29.sql");
/// v29 → v30. 출처와 사용자 열 충돌은 `SchemaRepairs.Upgrade30` 이 같은 트랜잭션에서 고친다.
const V30: &str = "-- 출처와 사용자 열 충돌을 고친다.";
/// v30 → v31. 유일한 재공고를 현행으로 삼아 품목 연결을 판올림 끝의 명시 참조 재계산이 다시 짓는다.
const V31: &str = "-- 현행 공고와 품목 연결을 새 선정 규칙으로 다시 계산한다.";
/// v31 → v32. 공고종류의 옛 상태 합성을 `SchemaRepairs.Upgrade32` 가 걷는다.
const V32: &str = "-- 공고종류와 상태의 부분 수집을 일관되게 표시한다.";

/// `Schema.Steps` — 기준선 뒤의 단계. `STEPS[0]` 이 v19 → v20 이다. 끝에만 덧붙인다.
pub const STEPS: [&str; 13] = [
    V20, V21, V22, V23, V24, V25, V26, V27, V28, V29, V30, V31, V32,
];

/// `Schema.IsPreBaseline` — 정식판 이전의 시험판(v1~v18)으로 지은 판인가. `0` 은 빈 파일이라 들지 않는다.
pub fn is_pre_baseline(version: i32) -> bool {
    version > 0 && version < BASELINE_VERSION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_baseline_plus_steps() {
        assert_eq!(VERSION, BASELINE_VERSION + STEPS.len() as i32);
    }

    /// 떼어 내 돌리는 자리가 원문과 어긋나면(원문을 고쳤는데 여기를 빠뜨리면) 무작위 문장이 그대로 돌거나 아예
    /// 빠진다 — 둘 다 조용하다. 원문 안에 꼭 한 번, 기준선은 맨 끝에 있어야 한다.
    #[test]
    fn bound_statements_are_cut_from_the_verbatim_text() {
        assert!(BASELINE.ends_with(BASELINE_SEED_SQL));
        assert_eq!(BASELINE.matches("randomblob").count(), 1);
        assert_eq!(V20.matches(V20_SEED_SQL).count(), 1);
        assert_eq!(V20.matches("randomblob").count(), 1);
        assert_eq!(V20.matches("'now'").count(), 1);
        for step in &STEPS[1..] {
            assert!(!step.contains("randomblob") && !step.contains("'now'"));
        }
    }
}

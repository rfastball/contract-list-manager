//! 저장(C# `Pclm.Core.Storage`) — 스키마·판올림·파일 판별·예시 보기·가공식 자료형·제출본 취합·DB 안의 설정.

pub mod computed_column;
pub mod database;
pub mod flow;
pub mod flow_check;
pub mod flow_compiler;
pub mod merge_report_text;
pub mod merger;
pub mod notice_groups;
pub mod outline;
pub mod pclm_file;
pub mod reports;
pub mod schema;
pub(crate) mod schema_repairs;
pub mod settings;
pub mod sql_column;
pub mod status;
pub mod store;
pub mod user_view_builder;
pub mod views;

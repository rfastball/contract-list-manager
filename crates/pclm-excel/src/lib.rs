//! 엑셀 — 계약면 뷰 내보내기(#8 ⑥).
//!
//! xlsx 를 쓰는 의존(zip)은 이 크레이트에만 둔다 — `pclm-core` 는 문서나르미가 함께 링크한다.
//!
//! 기준은 옛 판이 쓰던 ClosedXML 0.105.1 이 내던 통합 문서다. 내보내기는 그것과 같은 칸·꼴·고정·거르개·열 너비를
//! 낸다. 엑셀 라이브러리를 쓰지 않은 까닭은 [`writer`] 에 적었다.

pub mod exporter;
pub mod font;
pub mod measure;
pub mod writer;

pub use exporter::{SheetSnapshot, default_file_name, export, export_sheet, validate_snapshot};

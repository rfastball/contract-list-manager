//! 엑셀 — 계약면 뷰 내보내기(C# `Storage/Exporter.cs`)와 연간 조달계획 엑셀 가져오기(C# `Storage/PlanImport.cs`)(#8 ⑥).
//!
//! xlsx 를 읽고 쓰는 의존(zip·XML)은 이 크레이트에만 둔다 — `pclm-core` 는 문서나르미가 함께 링크한다.
//!
//! 기준은 C# 이 쓰는 ClosedXML 0.105.1 이다. 내보내기는 ClosedXML 이 짓는 것과 같은 칸·꼴·고정·거르개·열 너비를,
//! 가져오기는 ClosedXML 이 칸마다 읽는 것과 같은 형과 값을 낸다. 엑셀 라이브러리를 쓰지 않은 까닭은 [`writer`] 와
//! [`reader`] 에 적었다.

pub mod dotnet;
pub mod exporter;
pub mod font;
pub mod measure;
pub mod plan_import;
pub mod reader;
pub mod writer;

pub use exporter::{SheetSnapshot, default_file_name, export, export_sheet, validate_snapshot};
pub use plan_import::{PlanImportResult, headers, import};

//! `docs/document-vocabulary.md` 의 표 부분(`<!-- vocab:start … -->` ~ `<!-- vocab:end -->`)을 통합 뷰의 열과 `vocabulary.json` 에서 다시 짓는다.
//!
//! ```text
//! cargo run -p pclm-doc-engine --example vocabulary_doc
//! ```
//!
//! 표 밖의 글은 건드리지 않는다. 어긋남은 `tests/vocabulary.rs` 가 잡는다.

use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/document-vocabulary.md");
    let doc = match std::fs::read_to_string(&path) {
        Ok(text) => text.replace("\r\n", "\n"),
        Err(e) => {
            eprintln!("{}: {e}", path.display());
            return ExitCode::FAILURE;
        }
    };
    let (Some(start), Some(end)) = (doc.find("<!-- vocab:start"), doc.find("<!-- vocab:end -->"))
    else {
        eprintln!(
            "{}: vocab:start · vocab:end 표시가 없습니다.",
            path.display()
        );
        return ExitCode::FAILURE;
    };
    let end = end + "<!-- vocab:end -->".len();
    let next = format!(
        "{}{}{}",
        &doc[..start],
        pclm_doc_engine::vocab::vocabulary().markdown(),
        &doc[end..]
    );
    if let Err(e) = std::fs::write(&path, next) {
        eprintln!("{}: {e}", path.display());
        return ExitCode::FAILURE;
    }
    println!("{}", path.display());
    ExitCode::SUCCESS
}

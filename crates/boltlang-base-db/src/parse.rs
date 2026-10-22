use boltlang_parser::Diagnostic;
use crate::rowan::GreenNode;

use crate::{Db, File, source_text};

/// The syntax tree of a file and the errors found while parsing it.
#[salsa::tracked]
pub struct Parse<'db> {
    #[tracked]
    #[returns(ref)]
    pub tree: GreenNode,
    #[tracked]
    #[returns(ref)]
    pub errors: Vec<Diagnostic>,
}

/// Parse a file. The errors are returned instead of reported, so that the
/// crates on top can turn them into their own diagnostics.
#[salsa::tracked]
pub fn parse_file(db: &dyn Db, file: File) -> Parse<'_> {
    let text = source_text(db, file);
    let (tree, errors) = boltlang_parser::parse_file(&text);
    Parse::new(db, tree, errors)
}

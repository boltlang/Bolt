
mod util;
mod error;
mod diagnostic;
mod db;
mod text;
mod import;
mod syntax;
mod parser;
mod ast;
mod tc;

use std::collections::HashMap;
use salsa::Accumulator;
use crate::tc::{Constraints, InferContext};

/// Re-export of the Salsa library that boltlang uses
pub use salsa;

/// Re-export of the Rowan library that boltlang uses
pub use rowan;

pub type OwnedUri = String;

pub type BorrowedUri = str;

pub use {
    tc::{Type, CheckResult},
    error::{Error, Result},
    syntax::{SyntaxKind, SyntaxNode, SyntaxToken, SyntaxElement},
    db::{RootDatabase, File},
    parser::lexer::LineColumn,
    parser::parse_file,
    text::{LineIndex, index_lines},
    diagnostic::{DbDiagnostic, Diagnostic, Severity},
    ast::*,
};

#[salsa::tracked]
pub fn check_file(db: &dyn salsa::Database, file: File) -> CheckResult {
    let node = parse_file(db, file);
    let source_file = SourceFile::wrap(SyntaxNode::new_root(node.node(db).clone()));
    let mapping = HashMap::new();
    let mut infer = InferContext::new();
    let (constraints, mut diagnostics) = infer.infer_source_file(&source_file, file);
    diagnostics.extend(infer.solve(&constraints));
    for diagnostic in diagnostics {
        DbDiagnostic::new(infer.solver.unifier.normalize_diagnostic(diagnostic)).accumulate(db);
    }
    CheckResult {
        mapping,
    }
}

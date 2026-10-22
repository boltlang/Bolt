


mod diagnostic;

mod import;
mod tc;

mod emit;

use std::collections::HashMap;
use salsa::Accumulator;

use crate::tc::InferContext;

pub use boltlang_base_db::{
    salsa, rowan, OwnedUri, BorrowedUri, FxDashMap, FxDashSet,
    Db, Error, Result, File, FilePath, Files, Parse,
    System, SystemPath, SystemPathBuf, WritableSystem, OsSystem, InMemorySystem,
    LineIndex, index_lines, source_text,
    FileRevision, FileType, MemoryFs, Metadata, Path, PathBuf,
};

pub use {
    diagnostic::{DbDiagnostic, Diagnostic, Severity},
    boltlang_parser::{LineColumn},
    boltlang_syntax::{SyntaxKind, SyntaxNode, SyntaxToken, SyntaxElement, ast::*},
    tc::{Type, CheckResult, Constraints},
    emit::{Formatter, Emit},
};

#[cfg(test)]
pub use boltlang_base_db::TestSystem;

/// Parse a file and report its parse errors as diagnostics.
#[salsa::tracked]
pub fn parse_file(db: &dyn Db, file: File) -> Parse<'_> {
    let parse = boltlang_base_db::parse_file(db, file);
    for d in parse.errors(db) {
        DbDiagnostic::new(Diagnostic::Parse(d.clone().into()).with_file(file)).accumulate(db);
    }
    *parse
}

#[salsa::tracked]
pub fn check_file(db: &dyn Db, file: File) -> CheckResult {
    let raw_node = parse_file(db, file);
    let node = SyntaxNode::new_root(raw_node.tree(db).clone());
    let source_file = SourceFile::wrap(node);
    let mapping = HashMap::new();
    let mut infer = InferContext::new();
    let res = infer.infer_source_file(&source_file);
    [ res.diagnostics, infer.solve(&res.constraints) ]
        .into_iter()
        .flatten()
        .map(|d| infer.solver.unifier.normalize_diagnostic(d))
        .map(|d| d.with_file(file))
        .for_each(|d| {
            DbDiagnostic::new(d).accumulate(db);
        });
    CheckResult {
        mapping,
    }
}


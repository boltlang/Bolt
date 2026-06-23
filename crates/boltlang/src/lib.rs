
mod util;
mod error;

mod vfs;

mod system;
mod files;
mod diagnostic;
mod db;

mod text;
mod import;
mod syntax;
mod parser;
mod ast;
mod tc;

mod emit;

use std::{collections::HashMap, hash::BuildHasherDefault};
use rustc_hash::FxHasher;
use salsa::Accumulator;
use crate::tc::InferContext;

/// Re-export of the Salsa library that boltlang uses
pub use salsa;

/// Re-export of the Rowan library that boltlang uses
pub use rowan;

pub type OwnedUri = String;

pub type BorrowedUri = str;

pub type FxDashMap<K, V> = dashmap::DashMap<K, V, BuildHasherDefault<FxHasher>>;
pub type FxDashSet<K> = dashmap::DashSet<K, BuildHasherDefault<FxHasher>>;

pub use {
    ast::*,
    db::Db,
    diagnostic::{DbDiagnostic, Diagnostic, Severity},
    error::{Error, Result},
    files::{File, FilePath, Files},
    parser::lexer::LineColumn,
    parser::parse_file,
    syntax::{SyntaxKind, SyntaxNode, SyntaxToken, SyntaxElement, DbNode},
    system::{System, SystemPath, SystemPathBuf, WritableSystem, OsSystem, InMemorySystem},
    tc::{Type, CheckResult, Constraints},
    text::{LineIndex, index_lines},
    vfs::{FileRevision, FileType, MemoryFs, Metadata, Path, PathBuf},
    emit::{Formatter, Emit},
};

#[cfg(test)]
pub use crate::system::TestSystem;

#[salsa::tracked]
pub fn check_file(db: &dyn Db, file: File) -> CheckResult {
    let node = parse_file(db, file);
    let source_file = SourceFile::wrap(SyntaxNode::new_root(node.node(db).clone()));
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


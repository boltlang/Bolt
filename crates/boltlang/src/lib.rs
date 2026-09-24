
mod error;

mod vfs;

mod system;
mod files;
mod diagnostic;
mod db;

mod text;
mod import;
mod tc;

mod emit;

use std::{collections::HashMap, hash::BuildHasherDefault};
use rustc_hash::FxHasher;
use salsa::Accumulator;
use rowan::GreenNode;

use crate::{tc::InferContext, text::source_text};

/// Re-export of the Salsa library that boltlang uses
pub use salsa;

/// Re-export of the Rowan library that boltlang uses
pub use boltlang_syntax::rowan;

pub type OwnedUri = String;

pub type BorrowedUri = str;

pub type FxDashMap<K, V> = dashmap::DashMap<K, V, BuildHasherDefault<FxHasher>>;
pub type FxDashSet<K> = dashmap::DashSet<K, BuildHasherDefault<FxHasher>>;

pub use {
    db::Db,
    diagnostic::{DbDiagnostic, Diagnostic, Severity},
    error::{Error, Result},
    files::{File, FilePath, Files},
    boltlang_parser::{LineColumn},
    boltlang_syntax::{SyntaxKind, SyntaxNode, SyntaxToken, SyntaxElement, ast::*},
    system::{System, SystemPath, SystemPathBuf, WritableSystem, OsSystem, InMemorySystem},
    tc::{Type, CheckResult, Constraints},
    text::{LineIndex, index_lines},
    vfs::{FileRevision, FileType, MemoryFs, Metadata, Path, PathBuf},
    emit::{Formatter, Emit},
};

#[cfg(test)]
pub use crate::system::TestSystem;

#[salsa::tracked]
pub struct DbNode<'db> {
    #[tracked]
    #[returns(ref)]
    pub node: GreenNode,
}

#[salsa::tracked]
pub fn parse_file(db: &dyn Db, file: File) -> DbNode<'_> {
    let text = source_text(db, file);
    let (node, diagnostics) = boltlang_parser::parse_file(&text);
    for d in diagnostics {
        DbDiagnostic::new(Diagnostic::SyntaxDiagnostic(d.into()).with_file(file)).accumulate(db);
    }
    DbNode::new(db, node)
}

#[salsa::tracked]
pub fn check_file(db: &dyn Db, file: File) -> CheckResult {
    let raw_node = parse_file(db, file);
    let node = SyntaxNode::new_root(raw_node.node(db).clone());
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


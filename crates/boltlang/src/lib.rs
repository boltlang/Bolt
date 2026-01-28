
mod util;
mod error;
mod diagnostic;
mod db;
mod text;
mod import;
mod syntax;
mod parser;
mod ast;

/// Re-export of the Salsa library that boltlang uses
pub use salsa;

/// Re-export of the Rowan library that boltlang uses
pub use rowan;

pub type OwnedUri = String;

pub type BorrowedUri = str;

pub use {
    error::{Error, Result},
    syntax::{SyntaxKind, SyntaxNode, SyntaxToken, SyntaxElement},
    db::{RootDatabase, File},
    parser::lexer::LineColumn,
    parser::parse_file,
    text::{LineIndex, index_lines},
    diagnostic::{Diagnostic, Severity},
    ast::*,
};

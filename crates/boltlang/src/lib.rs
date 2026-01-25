
mod util;
mod error;
mod diagnostic;
mod db;
mod text;
mod import;
mod syntax;
mod parser;

/// Re-export of the Salsa library that boltlang uses
pub use salsa;

/// Re-export of the Rowan library that boltlang uses
pub use rowan;

pub use {
    syntax::{SyntaxKind, SyntaxNode, SyntaxToken, SyntaxElement},
    db::{RootDatabase, File},
    parser::parse_file,
    text::{line_column_of_offset, start_offset_of_line, end_offset_of_line, index_lines},
    diagnostic::Diagnostic,
};

pub mod token_set;
pub mod lexer;
pub mod event;
pub mod parser;
pub mod grammar;

pub(crate) use event::{process_events, intersperse_trivia};
pub(crate) use parser::Parser;
pub(crate) use grammar::*;

use salsa::Accumulator;
use crate::{Db, DbDiagnostic, DbNode, File, text::source_text};

#[salsa::tracked]
pub fn parse_file(db: &dyn Db, file: File) -> DbNode<'_> {
    let text = source_text(db, file);
    let lexed = lexer::tokenize(&text);
    let inp = lexed.to_input();
    let mut p = Parser::new(&inp);
    parse_source_file(&mut p);
    let interspersed = intersperse_trivia(
        p.finish().into_iter(),
        &lexed
    );
    let (node, diagnostics) = process_events(
        interspersed.into_iter(),
        &lexed,
        &text
    );
    for diagnostic in diagnostics {
        DbDiagnostic::new(diagnostic.with_file(file)).accumulate(db);
    }
    DbNode::new(db, node)
}


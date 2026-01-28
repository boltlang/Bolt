pub mod token_set;
pub mod lexer;
pub mod event;
pub mod parser;
pub mod grammar;

pub(crate) use event::{process_events, intersperse_trivia};
pub(crate) use parser::Parser;
pub(crate) use grammar::*;

use salsa::Accumulator;
use crate::db::{File, ParsedFile};

#[salsa::tracked]
pub fn parse_file(db: &dyn salsa::Database, file: File) -> ParsedFile<'_> {
    let text = file.contents(db);
    let lexed = lexer::tokenize(text);
    let inp = lexed.to_input();
    let mut p = Parser::new(&inp);
    parse_source_file(&mut p);
    let interspersed = intersperse_trivia(
        p.finish().into_iter(),
        &lexed
    );
    let (node, diagnostics) = process_events(
        interspersed.into_iter(),
        file,
        &lexed,
        &text
    );
    for diagnostic in diagnostics {
        diagnostic.accumulate(db);
    }
    ParsedFile::new(db, node)
}


mod token_set;
mod lexer;
mod event;
mod parser;
pub mod grammar;
mod diagnostic;

use boltlang_syntax::SyntaxKind::EOF;
pub use boltlang_syntax::rowan;

use rowan::GreenNode;

pub(crate) use event::{process_events, intersperse_trivia};

pub use diagnostic::Diagnostic;

pub use {
    lexer::LineColumn,
    parser::Parser,
};

use crate::lexer::tokenize;

pub fn parse_file(text: &str) -> (GreenNode, Vec<Diagnostic>) {
    let lexed = lexer::tokenize(text);
    let inp = lexed.to_input();
    let mut p = Parser::new(&inp);
    grammar::parse_source_file(&mut p);
    let interspersed = intersperse_trivia(
        p.finish().into_iter(),
        &lexed
    );
    process_events(
        interspersed.into_iter(),
        &lexed,
        &text
    )
}

pub fn parse<R, F: Fn(&mut Parser) -> R>(text: &str, rule: F) -> (GreenNode, Vec<Diagnostic>) {
    let lexed = tokenize(text);
    let inp = lexed.to_input();
    let mut p = Parser::new(&inp);
    rule(&mut p);
    if !p.at(EOF) {
        p.error("parser not at end of file");
    }
    let interspersed = intersperse_trivia(
        p.finish().into_iter(),
        &lexed
    );
    process_events(
        interspersed.into_iter(),
        &lexed,
        &text
    )
}

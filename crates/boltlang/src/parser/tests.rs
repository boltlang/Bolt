use crate::parser::{Parser, lexer::tokenize, parser::CompletedMarker};
use crate::SyntaxKind::EOF;

type P = fn (&mut Parser) -> Option<CompletedMarker>;

pub fn assert_parse_succeed(text: &str, func: P) {
    let res = tokenize(text);
    let inp = res.to_input();
    let mut p = Parser::new(&inp);
    match func(&mut p) {
        None => panic!("input failed to parse"),
        Some(_) => {},
    }
    if !p.at(EOF) {
        panic!("parser not at EOF");
    }
}

pub fn assert_parse_fail(text: &str, func: P) {
    let res = tokenize(text);
    let inp = res.to_input();
    let mut p = Parser::new(&inp);
    match func(&mut p) {
        None => {},
        Some(_) => panic!("input failed to parse"),
    }
    if !p.at(EOF) {
        panic!("parser not at EOF");
    }
}

use boltlang_syntax::SyntaxKind::*;
use crate::{
    grammar::{parse_pattern, type_},
    parser::{CompletedMarker, Parser},
};

pub fn parse_param(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    parse_pattern(p);
    if p.eat(EQUALS) {
        type_(p);
    }
    m.complete(p, PARAM)
}

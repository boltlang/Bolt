use boltlang_syntax::SyntaxKind::*;
use crate::{
    grammar::{parse_pattern, parse_type_expression},
    parser::{CompletedMarker, Parser},
};

pub fn parse_param(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    parse_pattern(p);
    if p.eat(EQUALS) {
        parse_type_expression(p);
    }
    m.complete(p, PARAM)
}

use boltlang_syntax::{SyntaxKind::*, T};
use crate::{
    grammar::{parse_expr, parse_pattern, type_},
    parser::{CompletedMarker, Parser},
};

/// A function parameter: a pattern with an optional type and default value,
/// like `x`, `x: i32` or `x: i32 = 1`.
pub fn parse_param(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    parse_pattern(p);
    if p.eat(T![:]) {
        type_(p);
    }
    if p.eat(T![=]) {
        parse_expr(p);
    }
    m.complete(p, PARAM)
}

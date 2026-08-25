
use crate::{
    parser::parser::{Parser, CompletedMarker},
    syntax::SyntaxKind::*,
    parser::grammar::PATH_NAME_REF_KINDS
};

pub fn parse_named_type_expression(p: &mut Parser) -> Option<CompletedMarker> {
    if p.at_ts(PATH_NAME_REF_KINDS) {
        let m = p.start();
        p.bump_any();
        Some(m.complete(p, NAMED_TYPE_EXPR))
    } else {
        p.error_and_bump("expected an identifier");
        None
    }
}

pub fn parse_prim_type_expression(p: &mut Parser) -> Option<CompletedMarker> {
    match p.current() {
        IDENT => parse_named_type_expression(p),
        _ => {
            p.error_and_bump("expected type expression");
            None
        }
    }
}

pub fn parse_type_expression(p: &mut Parser) -> Option<CompletedMarker> {
    let m = p.start();
    let m_2 = parse_prim_type_expression(p);
    let mut has_params = false;
    loop {
        if !p.eat(R_ARROW) {
            break;
        }
        has_params = true;
        parse_prim_type_expression(p);
    }
    if has_params {
        Some(m.complete(p, ARROW_TYPE_EXPR))
    } else {
        m.abandon(p);
        m_2
    }
}

/// Parse a type **without** type bounds.
///
/// This is e.g. used in [[parse_cast_expr]], where a cast to a type with type bounds would be
/// illegal.
pub fn type_no_bounds(p: &mut Parser) -> Option<CompletedMarker> {
    // TODO change me as soon as type bounds are added
    parse_type_expression(p)
}

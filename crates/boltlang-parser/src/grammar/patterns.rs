use boltlang_syntax::SyntaxKind::*;
use crate::{Parser, grammar::PATH_NAME_REF_KINDS, parser::CompletedMarker};

pub fn parse_named_pattern(p: &mut Parser) -> Option<CompletedMarker> {
    if p.at_ts(PATH_NAME_REF_KINDS) {
        let m = p.start();
        p.bump_any();
        Some(m.complete(p, NAMED_PATT))
    } else {
        p.error_and_bump("expected identifier");
        None
    }
}

pub fn parse_parenthesized_pattern(_p: &mut Parser) -> CompletedMarker {
    todo!()
}

pub fn parse_pattern(p: &mut Parser) -> Option<CompletedMarker> {
    match p.current() {
        IDENT => parse_named_pattern(p),
        L_PAREN => Some(parse_parenthesized_pattern(p)),
        _ => {
            p.error_and_bump("expected pattern");
            None
        }
    }
}



use crate::{
    parser::{
        Parser,
        grammar::PATH_NAME_REF_KINDS,
        parse_block,
        parser::CompletedMarker
    },
    syntax::SyntaxKind::*
};

pub fn parse_reference_expression(p: &mut Parser) -> Option<CompletedMarker> {
    if p.at_ts(PATH_NAME_REF_KINDS) {
        let m = p.start();
        p.bump_any();
        Some(m.complete(p, PATH_EXPR))
    } else {
        p.error_and_bump("expected identifier");
        None
    }
}

pub fn parse_block_expression(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    parse_block(p);
    m.complete(p, BLOCK_EXPR)
}

pub fn parse_literal_expression(p: &mut Parser) -> Option<CompletedMarker> {
    match p.current() {
        STRING | BIN_INT | OCT_INT | DEC_INT | HEX_INT => {
            let m = p.start();
            p.bump_any();
            Some(m.complete(p, LIT_EXPR))
        }
        _ => {
            p.error_and_bump("expected literal");
            None
        }
    }
}

pub fn parse_parenthesized_expression(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.expect(L_PAREN);
    let mut saw_comma = false;
    if p.eat(COMMA) {
        p.error("expected expression");
        saw_comma = true;
    }
    while !p.at(EOF) && !p.at(R_PAREN) {
        if parse_expression(p).is_none() {
            break;
        }
        if !p.at(R_PAREN) {
            saw_comma = true;
            p.expect(COMMA);
        }
    }
    m.complete(p, if saw_comma { TUPLE_EXPR } else { NEST_EXPR })
}

pub fn parse_return_expression(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.expect(RETURN_KEYWORD);
    parse_expression(p);
    m.complete(p, RET_EXPR)
}

pub fn parse_prim_expression(p: &mut Parser) -> Option<CompletedMarker> {
    match p.current() {
        STRING | BIN_INT | OCT_INT | DEC_INT | HEX_INT => parse_literal_expression(p),
        IDENT => parse_reference_expression(p),
        L_BRACE => Some(parse_block_expression(p)),
        L_PAREN => Some(parse_parenthesized_expression(p)),
        RETURN_KEYWORD => Some(parse_return_expression(p)),
        _ => {
            p.error_and_bump("expected expression");
            return None;
        }
    }
}

pub fn parse_call_expression(p: &mut Parser) -> Option<CompletedMarker> {
    let m = p.start();
    let m_2 = parse_prim_expression(p);
    if !p.eat(L_PAREN) {
        m.abandon(p);
        return m_2
    }
    if !p.eat(R_PAREN) {
        while !p.at(EOF) && !p.at(SEMI) && !p.at(R_BRACE) && !p.at(R_BRACKET) {
            parse_expression(p);
            if p.at(R_PAREN) {
                break;
            } else if p.eat(COMMA) {
                continue;
            }
            p.error("expected ')' or ','");
        }
    }
    p.expect(R_PAREN);
    Some(m.complete(p, CALL_EXPR))
}

pub fn parse_expression(p: &mut Parser) -> Option<CompletedMarker> {
    parse_call_expression(p)
}


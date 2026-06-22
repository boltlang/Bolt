
use crate::{parser::{parser::{CompletedMarker, Parser}, token_set::TokenSet}, syntax::SyntaxKind::{self, *}};

const PATH_NAME_REF_KINDS: TokenSet = TokenSet::new(&[IDENTIFIER]);

fn peek_after_modifiers(p: &mut Parser) -> SyntaxKind {
    let mut i = 0;
    loop {
        let k0 = p.nth(i);
        match k0 {
            PUB_KEYWORD => { i += 1; },
            _ => return k0,
        }
    }
}

pub fn parse_reference_expression(p: &mut Parser) -> Option<CompletedMarker> {
    if p.at_ts(PATH_NAME_REF_KINDS) {
        let m = p.start();
        p.bump_any();
        Some(m.complete(p, REF_EXPR))
    } else {
        p.error_and_bump("expected identifier");
        None
    }
}

pub fn parse_block(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.expect(L_BRACE);
    while !p.at(END_OF_FILE) && !p.eat(R_BRACE) {
        parse_body_element(p);
    }
    m.complete(p, BLOCK)
}

pub fn parse_block_expression(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    parse_block(p);
    m.complete(p, BLOCK_EXPR)
}

pub fn parse_literal_expression(p: &mut Parser) -> Option<CompletedMarker> {
    match p.current() {
        BIN_INT | OCT_INT | DEC_INT | HEX_INT => {
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
    while !p.at(END_OF_FILE) && !p.at(R_PAREN) {
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
        BIN_INT | OCT_INT | DEC_INT | HEX_INT => parse_literal_expression(p),
        IDENTIFIER => parse_reference_expression(p),
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
        while !p.at(END_OF_FILE) && !p.at(SEMI) && !p.at(R_BRACE) && !p.at(R_BRACKET) {
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
    let m = p.start();
    let m_2 = match p.current() {
        IDENTIFIER => parse_named_pattern(p),
        L_PAREN => Some(parse_parenthesized_pattern(p)),
        _ => {
            p.error_and_bump("expected pattern");
            None
        }
    };

    // Attempt to parse typed pattern
    if p.eat(COLON) {
        parse_type_expression(p);
        Some(m.complete(p, TYPED_PATT))
    } else {
        m.abandon(p);
        m_2
    }
}

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
        IDENTIFIER => parse_named_type_expression(p),
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

pub fn parse_type_ascription(p: &mut Parser) {
    p.bump(COLON);
    parse_type_expression(p);
}

pub fn parse_param(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    parse_pattern(p);
    if p.eat(EQUALS) {
        parse_type_expression(p);
    }
    m.complete(p, PARAM)
}

pub fn parse_named_function_declaration(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.eat(PUB_KEYWORD);
    p.expect(FN_KEYWORD);
    match p.current() {
        IDENTIFIER => p.bump_any(),
        L_PAREN => {
            p.bump_any();
            p.expect(OPERATOR);
            p.expect(R_PAREN);
        },
        _ => {
            p.error("expected an identifier or an operator wrapped between '(' and ')'");
            // TODO maybe bump
        }
    }
    p.expect(L_PAREN);
    while !p.at(SEMI) && !p.at(END_OF_FILE) && !p.eat(R_PAREN) {
        parse_param(p);
    }
    if p.eat(R_ARROW) {
        parse_type_expression(p);
    }
    if p.eat(EQUALS) {
        parse_expression(p);
        check_semi(p);
    } else if p.at(L_BRACE) {
        parse_block(p);
    } else {
        check_semi(p);
    }
    m.complete(p, FUNC_DECL)
}

pub fn parse_variable_declaration(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.eat(PUB_KEYWORD);
    p.expect(LET_KEYWORD);
    p.eat(MUT_KEYWORD);
    parse_pattern(p);
    if p.current() == COLON {
        parse_type_ascription(p);
    }
    if p.current() == EQUALS {
        p.expect(EQUALS);
        parse_expression(p);
    }
    check_semi(p);
    m.complete(p, VAR_DECL)
}

fn check_semi(p: &mut Parser) {
    if !p.at(R_BRACE) {
        p.expect(SEMI);
    }
}

pub fn parse_expression_statement(p: &mut Parser) -> Option<CompletedMarker> {
    let m = parse_expression(p);
    check_semi(p);
    m
}

pub fn parse_body_element(p: &mut Parser) -> Option<CompletedMarker> {
    let kind = peek_after_modifiers(p);
    match kind {
        LET_KEYWORD => Some(parse_variable_declaration(p)),
        FN_KEYWORD => Some(parse_named_function_declaration(p)),
        _ => parse_expression_statement(p),
    }
}

pub fn parse_source_element(p: &mut Parser) -> Option<CompletedMarker> {
    let kind = peek_after_modifiers(p);
    match kind {
        LET_KEYWORD => Some(parse_variable_declaration(p)),
        FN_KEYWORD => Some(parse_named_function_declaration(p)),
        _ => parse_expression_statement(p),
    }
}

pub fn parse_source_file(p: &mut Parser) {
    let m = p.start();
    while !p.at(END_OF_FILE) {
        parse_source_element(p);
    }
    m.complete(p, SOURCE_FILE);
}

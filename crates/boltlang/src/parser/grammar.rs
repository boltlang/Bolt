mod expr;
mod pat;
mod ty;
mod item;

use crate::{
    parser::{
        grammar::{expr::parse_expression, pat::parse_pattern, ty::parse_type_expression},
        parser::{CompletedMarker, Parser},
        token_set::TokenSet
    },
    syntax::SyntaxKind::{self, *}
};

const PATH_NAME_REF_KINDS: TokenSet = TokenSet::new(&[IDENT]);

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
        IDENT => p.bump_any(),
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
    while !p.at(SEMI) && !p.at(EOF) && !p.eat(R_PAREN) {
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
    if p.eat(EQUALS) {
        parse_expression(p);
    }
    check_semi(p);
    m.complete(p, LET_STMT)
}

fn check_semi(p: &mut Parser) {
    if !p.at(R_BRACE) && !p.eat(SEMI) {
        p.error("expected ';' or '}'");
        // TODO handle newlines
        while !p.at(R_BRACE) && !p.eat(SEMI) {
            p.bump_any();
        }
    }
}

pub fn parse_expression_statement(p: &mut Parser) -> Option<CompletedMarker> {
    let m = parse_expression(p);
    check_semi(p);
    m
}

pub fn parse_block(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.expect(L_BRACE);
    while !p.at(EOF) && !p.eat(R_BRACE) {
        parse_body_element(p);
    }
    m.complete(p, BLOCK)
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
    while !p.at(EOF) {
        parse_source_element(p);
    }
    m.complete(p, SOURCE_FILE);
}

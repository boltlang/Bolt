use boltlang_syntax::SyntaxKind::{self, *};
use crate::{
    Parser,
    grammar::{PATH_NAME_REF_KINDS, delimited, parse_block, parse_pattern, parse_type_expression},
    parser::{CompletedMarker, Marker},
    token_set::TokenSet
};

pub(super) const PATH_FIRST: TokenSet = TokenSet::new(&[
    IDENT,
    SELF_KEYWORD,
    SUPER_KEYWORD,
    PACKAGE_KEYWORD,
    COLON,
    LT
]);

pub(crate) const LITERAL_FIRST: TokenSet = TokenSet::new(&[
    TRUE_KEYWORD,
    FALSE_KEYWORD,
    INT_NUMBER,
    FLOAT_NUMBER,
    BYTE,
    CHAR,
    STRING,
    BYTE_STRING,
]);

pub(super) const ATOM_EXPR_FIRST: TokenSet =
    LITERAL_FIRST.union(PATH_FIRST).union(TokenSet::new(&[
        L_PAREN,
        L_BRACE,
        L_BRACKET,
        VBAR,
        BREAK_KEYWORD,
        CONST_KEYWORD,
        CONTINUE_KEYWORD,
        FOR_KEYWORD,
        IF_KEYWORD,
        LET_KEYWORD,
        FN_KEYWORD,
        LOOP_KEYWORD,
        MATCH_KEYWORD,
        RETURN_KEYWORD,
        STATIC_KEYWORD,
        WHILE_KEYWORD,
        YIELD_KEYWORD
    ]));

const LHS_FIRST: TokenSet =
    ATOM_EXPR_FIRST.union(TokenSet::new(&[EXCL, DOT, MINUS, UNDERSCORE, HASHTAG]));

const EXPR_FIRST: TokenSet = LHS_FIRST;

pub(in crate::grammar) const EXPR_RECOVERY_SET: TokenSet =
    TokenSet::new(&[R_BRACE, R_PAREN, R_BRACKET, COMMA]);

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

pub fn parse_block_expr(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    parse_block(p);
    m.complete(p, BLOCK_EXPR)
}

pub fn parse_literal_expr(p: &mut Parser) -> Option<CompletedMarker> {
    match p.current() {
        STRING | INT_NUMBER | FLOAT_NUMBER | TRUE_KEYWORD | FALSE_KEYWORD => {
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

pub fn parse_nest_expr(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.expect(L_PAREN);
    let mut saw_comma = false;
    if p.eat(COMMA) {
        p.error("expected expression");
        saw_comma = true;
    }
    while !p.at(EOF) && !p.at(R_PAREN) {
        if parse_expr(p).is_none() {
            break;
        }
        if !p.at(R_PAREN) {
            saw_comma = true;
            p.expect(COMMA);
        }
    }
    p.expect(R_PAREN);
    m.complete(p, if saw_comma { TUPLE_EXPR } else { NEST_EXPR })
}

pub fn parse_return_expr(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.expect(RETURN_KEYWORD);
    parse_expr(p);
    m.complete(p, RET_EXPR)
}

/// Parse an anonymous function: `fn (x y) = expr` or `fn (x y) { ... }`,
/// optionally with a return type: `fn (x) -> Int = x`.
///
/// Unlike in a named function, the parameters are bare patterns, which is
/// what [`FunExpr::params`](boltlang_syntax::ast::FunExpr::params) expects.
pub fn parse_fun_expr(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.bump(FN_KEYWORD);
    p.expect(L_PAREN);
    while !p.at(SEMI) && !p.at(EOF) && !p.eat(R_PAREN) {
        parse_pattern(p);
    }
    if p.eat(R_ARROW) {
        parse_type_expression(p);
    }
    if p.eat(EQUALS) {
        parse_expr(p);
    } else if p.at(L_BRACE) {
        parse_block_expr(p);
    } else {
        p.error("expected '=' or '{' to start the function body");
    }
    m.complete(p, FUN_EXPR)
}

pub fn parse_atom_expr(p: &mut Parser) -> Option<CompletedMarker> {
    match p.current() {
        STRING | INT_NUMBER | FLOAT_NUMBER | TRUE_KEYWORD | FALSE_KEYWORD => parse_literal_expr(p),
        IDENT => parse_reference_expression(p),
        L_BRACE => Some(parse_block_expr(p)),
        L_PAREN => Some(parse_nest_expr(p)),
        RETURN_KEYWORD => Some(parse_return_expr(p)),
        FN_KEYWORD => Some(parse_fun_expr(p)),
        _ => {
            p.error_and_bump("expected expression");
            return None;
        }
    }
}

/// Whether the current token is an operator that can start a prefix expression.
fn at_prefix_op(p: &Parser) -> bool {
    matches!(p.op_text(), Some("-" | "+" | "!"))
}

fn parse_lhs(p: &mut Parser) -> Option<CompletedMarker> {
    let m;
    let kind = match p.current() {
        _ if at_prefix_op(p) => {
            m = p.start();
            p.bump_any();
            PREFIX_EXPR
        },
        _ => {
            let lhs = parse_atom_expr(p)?;
            let cm = parse_postfix_expr(p, lhs, true);
            return Some(cm);
        }
    };
    // parse the interior of the unary expression
    parse_expr_bp(p, None, 255);
    let cm =  m.complete(p, kind);
    Some(cm)
}

enum Associativity {
    Left,
    Right,
}

fn parse_cast_expr(p: &mut Parser<'_>, lhs: CompletedMarker) -> CompletedMarker {
    assert!(p.at(AS_KEYWORD));
    let m = lhs.precede(p);
    p.bump(AS_KEYWORD);
    // Use type_no_bounds(), because cast expressions are not
    // allowed to have bounds.
    super::ty::type_no_bounds(p);
    m.complete(p, CAST_EXPR)
}

fn parse_index_expr(p: &mut Parser<'_>, lhs: CompletedMarker) -> CompletedMarker {
    assert!(p.at(L_BRACKET));
    let m = lhs.precede(p);
    p.bump(L_BRACKET);
    parse_expr(p);
    p.expect(R_BRACKET);
    m.complete(p, INDEX_EXPR)
}

fn parse_try_expr(p: &mut Parser<'_>, lhs: CompletedMarker) -> CompletedMarker {
    assert!(p.at(QUEST));
    let m = lhs.precede(p);
    p.bump(QUEST);
    m.complete(p, TRY_EXPR)
}

fn parse_arg_list(p: &mut Parser<'_>) {
    assert!(p.at(L_PAREN));
    let m = p.start();
    delimited(
        p,
        L_PAREN,
        R_PAREN,
        COMMA,
        || "expected expression".into(),
        EXPR_FIRST,
        |p| parse_expr(p).is_some(),
    );
    m.complete(p, ARG_LIST);
}

fn parse_call_expr(p: &mut Parser<'_>, lhs: CompletedMarker) -> CompletedMarker {
    assert!(p.at(L_PAREN));
    let m = lhs.precede(p);
    parse_arg_list(p);
    m.complete(p, CALL_EXPR)
}

fn parse_postfix_expr(
    p: &mut Parser,
    mut lhs: CompletedMarker,
    mut allow_calls: bool,
) -> CompletedMarker {
    loop {
        lhs = match p.current() {
            L_PAREN if allow_calls => parse_call_expr(p, lhs),
            L_BRACKET if allow_calls => parse_index_expr(p, lhs),
            DOT => match parse_postfix_dot_expr(p, lhs) {
                Ok(it) => it,
                Err(it) => {
                    lhs = it;
                    break;
                }
            },
            QUEST => parse_try_expr(p, lhs),
            _ => break,
        };
        allow_calls = true;
    }
    lhs
}

fn parse_field_expr(p: &mut Parser, lhs: CompletedMarker) -> Result<CompletedMarker, CompletedMarker> {
    todo!()
}

fn parse_method_call_expr(p: &mut Parser, lhs: CompletedMarker) -> CompletedMarker {
    todo!()
}

fn parse_postfix_dot_expr(p: &mut Parser, lhs: CompletedMarker) -> Result<CompletedMarker, CompletedMarker> {

    assert!(p.at(DOT));

    if PATH_NAME_REF_KINDS.contains(p.nth(1))
        && (p.nth(2) == L_PAREN || p.nth_at(2, COLONCOLON))
        || p.nth(1) == L_PAREN
    {
        return Ok(parse_method_call_expr(p, lhs));
    }

    if p.at(DOTDOTEQUALS) || p.at(DOTDOT) {
        return Err(lhs);
    }

    parse_field_expr(p, lhs)
}

pub fn parse_expr_bp(p: &mut Parser, m: Option<Marker>, min_bp: u8) -> Option<CompletedMarker> {

    let m = m.unwrap_or_else(|| {
        let m = p.start();
        // attributes::outer_attrs(p);
        m
    });

    if !p.at_ts(EXPR_FIRST) && !at_prefix_op(p) {
        p.err_recover("expected expression", EXPR_RECOVERY_SET);
        m.abandon(p);
        return None;
    }
    let mut lhs = match parse_lhs(p) {
        Some(lhs) => {
            let lhs = lhs.extend_to(p, m);
            lhs
        },
        None => {
            m.abandon(p);
            return None;
        }
    };

    loop {
        let (op_bp, op, associativity) = current_op(p);
        if op_bp < min_bp {
            break;
        }
        if p.at(AS_KEYWORD) {
            lhs = parse_cast_expr(p, lhs);
            continue;
        }
        let m = lhs.precede(p);
        p.bump(op);
        let op_bp = match associativity {
            Associativity::Left => op_bp + 1,
            Associativity::Right => op_bp,
        };
        parse_expr_bp(p, None, op_bp);
        lhs = m.complete(p, BIN_EXPR);
    }

    Some(lhs)
}

pub fn parse_expr(p: &mut Parser) -> Option<CompletedMarker> {
    parse_expr_bp(p, None, 1)
}

fn current_op(p: &Parser<'_>) -> (u8, SyntaxKind, Associativity) {
    use Associativity::*;
    const NOT_AN_OP: (u8, SyntaxKind, Associativity) = (0, AT, Left);
    match p.current() {
        _ if p.op_text().is_some() => {
            let name = p.op_text().unwrap();
            let (bp, assoc) = match name {
                "||"  => (3, Left),
                "|="  => (1, Right),
                "|"   => (6, Left),
                ">>=" => (1, Right),
                ">>"  => (9, Left),
                ">="  => (5, Left),
                ">"   => (5, Left),
                "=="  => (5, Left),
                "="   => (1, Right),
                "<="  => (5, Left),
                "<<=" => (1, Right),
                "<<"  => (9, Left),
                "<"   => (5, Left),
                "+="  => (1, Right),
                "+"   => (10, Left),
                "^="  => (1, Right),
                "^"   => (7, Left),
                "%="  => (1, Right),
                "%"   => (11, Left),
                "&&"  => (4, Left),
                "&"   => (8, Left),
                "/="  => (1, Right),
                "/"   => (11, Left),
                "*="  => (1, Right),
                "*"   => (11, Left),
                "..=" => (2, Left),
                ".."  => (2, Left),
                "!="  => (5, Left),
                "-="  => (1, Right),
                "-"   => (10, Left),
                _ => return NOT_AN_OP,
            };
            (bp, p.current(), assoc)
        }
        AS_KEYWORD => (12, AS_KEYWORD,  Left),
        _ => NOT_AN_OP
    }
}

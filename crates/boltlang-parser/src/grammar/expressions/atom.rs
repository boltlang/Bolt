//! Expressions that don't start with an operator: literals, paths, blocks,
//! parenthesized expressions, and expressions that start with a keyword.

use boltlang_syntax::{SyntaxKind::*, T};

use super::{BlockLike, EXPR_FIRST, Restrictions, expr, expr_block_contents, expr_bp, expr_no_struct};
use crate::{
    grammar::{expr_path, is_path_start, opt_ret_type, param_list, parse_pattern},
    parser::{CompletedMarker, Parser},
    token_set::TokenSet,
};

pub(crate) const LITERAL_FIRST: TokenSet = TokenSet::new(&[
    T![true],
    T![false],
    INT_NUMBER,
    FLOAT_NUMBER,
    BYTE,
    CHAR,
    STRING,
    BYTE_STRING,
    C_STRING,
]);

pub(crate) const ATOM_EXPR_FIRST: TokenSet = LITERAL_FIRST.union(TokenSet::new(&[
    IDENT,
    T![self],
    METAVARIABLE,
    T!['('],
    T!['{'],
    T![return],
    T![fn],
    T![if],
    T![let],
    T![while],
]));

/// Where expression parsing gives up and leaves the token to an enclosing
/// rule.
pub(crate) const EXPR_RECOVERY_SET: TokenSet =
    TokenSet::new(&[T![')'], T![']'], T!['}'], T![,], T![;], T![let]]);

/// A literal like `1`, `"s"` or `true`.
pub fn parse_literal_expr(p: &mut Parser) -> Option<CompletedMarker> {
    if !p.at_ts(LITERAL_FIRST) {
        return None;
    }
    let m = p.start();
    p.bump_any();
    Some(m.complete(p, LIT_EXPR))
}

pub(super) fn atom_expr(p: &mut Parser, r: Restrictions) -> Option<(CompletedMarker, BlockLike)> {
    if let Some(m) = parse_literal_expr(p) {
        return Some((m, BlockLike::NotBlock));
    }
    if is_path_start(p) {
        return Some((path_expr(p), BlockLike::NotBlock));
    }
    let done = match p.current() {
        T!['('] => paren_or_tuple_expr(p),
        T!['{'] => return Some((parse_block_expr(p), BlockLike::Block)),
        T![return] => return_expr(p),
        T![fn] => fun_expr(p),
        T![if] => return Some((if_expr(p), BlockLike::Block)),
        T![while] => return Some((while_expr(p), BlockLike::Block)),
        T![let] => let_expr(p, r),
        _ => {
            p.err_recover("expected an expression", EXPR_RECOVERY_SET);
            return None;
        }
    };
    Some((done, BlockLike::NotBlock))
}

/// `x`, `self` or `$x`
fn path_expr(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    expr_path(p);
    m.complete(p, PATH_EXPR)
}

/// `(a)`, `(a,)`, `(a, b)` or `()`
fn paren_or_tuple_expr(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.bump(T!['(']);
    let mut saw_comma = false;
    let mut saw_expr = false;
    while !p.at(EOF) && !p.at(T![')']) {
        saw_expr = true;
        if !p.at_ts(EXPR_FIRST) {
            p.error("expected an expression");
            break;
        }
        expr(p);
        if p.eat(T![,]) {
            saw_comma = true;
        } else if p.at_ts(EXPR_FIRST) {
            // `(a b)`: assume a comma is missing.
            p.error("expected ','");
            saw_comma = true;
        } else {
            break;
        }
    }
    p.expect(T![')']);
    m.complete(p, if saw_expr && !saw_comma { NEST_EXPR } else { TUPLE_EXPR })
}

/// `{ statements; tail }`
pub fn parse_block_expr(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    block(p);
    m.complete(p, BLOCK_EXPR)
}

fn block(p: &mut Parser) {
    let m = p.start();
    if !p.expect(T!['{']) {
        m.abandon(p);
        return;
    }
    expr_block_contents(p);
    p.expect(T!['}']);
    m.complete(p, BLOCK);
}

/// `return` or `return x`
fn return_expr(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.bump(T![return]);
    if p.at_ts(EXPR_FIRST) {
        expr(p);
    }
    m.complete(p, RET_EXPR)
}

/// An anonymous function: `fn (x, y) = expr` or `fn (x, y) { ... }`,
/// optionally with a return type: `fn (x) -> Int = x`.
fn fun_expr(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.bump(T![fn]);
    param_list(p);
    opt_ret_type(p);
    if p.eat(T![=]) {
        expr(p);
    } else if p.at(T!['{']) {
        parse_block_expr(p);
    } else {
        p.error("expected '=' or '{' to start the function body");
    }
    m.complete(p, FUN_EXPR)
}

/// `if cond { } else if cond { } else { }`
fn if_expr(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.bump(T![if]);
    expr_no_struct(p);
    parse_block_expr(p);
    if p.eat(T![else]) {
        if p.at(T![if]) {
            if_expr(p);
        } else {
            parse_block_expr(p);
        }
    }
    m.complete(p, IF_EXPR)
}

/// `while cond { }`
fn while_expr(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.bump(T![while]);
    expr_no_struct(p);
    parse_block_expr(p);
    m.complete(p, WHILE_EXPR)
}

/// `let pattern = value` in a condition, like in `if let Some(x) = y`. The
/// value binds tighter than `&&`, so `if let a = b && c` checks `c`
/// separately, as in the grammar's `let_condition`.
fn let_expr(p: &mut Parser, r: Restrictions) -> CompletedMarker {
    let m = p.start();
    p.bump(T![let]);
    parse_pattern(p);
    p.expect(T![=]);
    expr_bp(p, None, Restrictions { prefer_stmt: false, ..r }, 5);
    m.complete(p, LET_EXPR)
}

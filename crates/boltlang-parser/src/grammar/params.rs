//! Parameter lists of functions, as in the grammar's `parameters`.

use boltlang_syntax::{SyntaxKind::*, T};
use crate::{
    grammar::{delimited, parse_expr, parse_pattern, type_},
    parser::{CompletedMarker, Parser},
    token_set::TokenSet,
};

const PARAM_FIRST: TokenSet = TokenSet::new(&[IDENT, T![_], T![mut], T![self], T!['('], T![...], T![#]]);

/// `(a, b: T = 1, ...)`
pub(crate) fn param_list(p: &mut Parser) {
    if !p.at(T!['(']) {
        p.error("expected '('");
        return;
    }
    let m = p.start();
    delimited(
        p,
        T!['('],
        T![')'],
        T![,],
        || "expected a parameter".into(),
        PARAM_FIRST,
        |p| {
            if p.at_ts(PARAM_FIRST) {
                param(p);
                true
            } else {
                false
            }
        },
    );
    m.complete(p, PARAM_LIST);
}

/// One parameter: `self`, `mut self`, `...`, `args: ...`, or a pattern with
/// an optional type and default value, like `x`, `x: T` or `x: T = 1`.
fn param(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    if p.at(T![...]) {
        p.bump(T![...]);
        return m.complete(p, VARIADIC_PARAM);
    }
    let self_at = if p.at(T![mut]) { 1 } else { 0 };
    if p.nth_at(self_at, T![self]) && !p.nth_at(self_at + 1, T![:]) {
        p.eat(T![mut]);
        p.bump(T![self]);
        return m.complete(p, SELF_PARAM);
    }
    p.eat(T![mut]);
    if p.at(T![self]) {
        let name = p.start();
        p.bump(T![self]);
        name.complete(p, NAME);
    } else {
        parse_pattern(p);
    }
    if p.eat(T![:]) {
        if p.eat(T![...]) {
            return m.complete(p, VARIADIC_PARAM);
        }
        type_(p);
    }
    if p.eat(T![=]) {
        parse_expr(p);
    }
    m.complete(p, PARAM)
}

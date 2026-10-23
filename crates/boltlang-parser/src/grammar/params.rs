//! Parameter lists of functions, as in the grammar's `parameters`.

use boltlang_syntax::{SyntaxKind::*, T};
use crate::{
    grammar::{PATTERN_FIRST, delimited, outer_attrs, parse_expr, pattern_single, type_},
    parser::{CompletedMarker, Parser},
    token_set::TokenSet,
};

const PARAM_FIRST: TokenSet = PATTERN_FIRST.union(TokenSet::new(&[T![...], T![#]]));

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

/// One parameter: `...`, `args: ...`, or a pattern with an optional type
/// and default value, like `x`, `x: T` or `x: T = 1`.
fn param(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    outer_attrs(p);
    if p.at(T![...]) {
        p.bump(T![...]);
        return m.complete(p, VARIADIC_PARAM);
    }
    p.eat(T![mut]);
    pattern_single(p);
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

//! Parameter lists of functions, as in the grammar's `parameters`.

use crate::{
    grammar::{
        PATTERN_FIRST, ParamOrder, at_keyword_marker, at_positional_marker, at_spread, delimited,
        outer_attrs, parse_expr, pattern_single, spread, type_,
    },
    parser::{CompletedMarker, Parser},
    token_set::TokenSet,
};
use boltlang_syntax::{SyntaxKind::*, T};

const PARAM_FIRST: TokenSet = PATTERN_FIRST.union(TokenSet::new(&[T![...], T![#], T![*]]));

/// `(a, b: T = 1, ...)`, and `(a, /, b)`, `(a, *, sep: T)`, `(a, *args:
/// *Ts)` or `(a, **opts: **Ks)`. Before a `/`, the parameters can only be
/// passed by position, after a `*` or `*args` only by name, and `**opts`
/// comes last.
pub(crate) fn param_list(p: &mut Parser) {
    if !p.at(T!['(']) {
        p.error("expected '('");
        return;
    }
    let m = p.start();
    let mut order = ParamOrder::default();
    delimited(p, T!['('], T![')'], "expected a parameter", PARAM_FIRST, |p| {
        if !p.at_ts(PARAM_FIRST) && !p.at_op("**") && !at_positional_marker(p) {
            return false;
        }
        order.check(p);
        param(p);
        true
    });
    m.complete(p, PARAM_LIST);
}

/// One parameter: `...`, `args: ...`, a pattern with an optional type
/// and default value, like `x`, `x: T` or `x: T = 1`, a rest like
/// `*args: *Ts` or `**opts: **Ks`, or a `/` or `*` alone.
pub(crate) fn param(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    outer_attrs(p);
    // `/` alone, after positional-only parameters.
    if at_positional_marker(p) {
        p.bump_any();
        return m.complete(p, POSITIONAL_MARKER);
    }
    if p.at(T![...]) {
        p.bump(T![...]);
        return m.complete(p, VARIADIC_PARAM);
    }
    // `*` alone, before keyword-only parameters.
    if at_keyword_marker(p) {
        p.bump(T![*]);
        return m.complete(p, KEYWORD_MARKER);
    }
    if p.at(T![*]) || p.at_op("**") {
        p.bump_any();
        pattern_single(p);
        if p.expect(T![:]) {
            let rest = p.start();
            if at_spread(p) {
                spread(p);
                type_(p);
            } else {
                p.error("expected a pack, like `*Ts` or `**Ks`");
            }
            rest.complete(p, TUPLE_TYPE_REST);
        }
        return m.complete(p, REST_PARAM);
    }
    // `mut x` is a `MUT_PATT`, like in any other pattern.
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

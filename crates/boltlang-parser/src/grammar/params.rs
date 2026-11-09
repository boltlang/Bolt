//! Parameter lists of functions, as in the grammar's `parameters`.

use crate::{
    grammar::{
        PATTERN_FIRST, at_spread, delimited, outer_attrs, parse_expr, pattern_single, spread, type_,
    },
    parser::{CompletedMarker, Parser},
    token_set::TokenSet,
};
use boltlang_syntax::{SyntaxKind::*, T};

const PARAM_FIRST: TokenSet = PATTERN_FIRST.union(TokenSet::new(&[T![...], T![#], T![*]]));

/// Where a parameter list is, for the rules on what comes after what.
#[derive(Default)]
struct ParamOrder {
    /// After a `*` or `*args`: the parameters are keyword-only.
    keyword: bool,
    /// After `**opts`, which comes last.
    done: bool,
}

/// `(a, b: T = 1, ...)`, and `(a, *, sep: T)`, `(a, *args: *Ts)` or
/// `(a, **opts: **Ks)`. After a `*` or `*args`, the parameters can only be
/// passed by name, and `**opts` comes last.
pub(crate) fn param_list(p: &mut Parser) {
    if !p.at(T!['(']) {
        p.error("expected '('");
        return;
    }
    let m = p.start();
    let mut order = ParamOrder::default();
    delimited(
        p,
        T!['('],
        T![')'],
        T![,],
        || "expected a parameter".into(),
        PARAM_FIRST,
        |p| {
            if !p.at_ts(PARAM_FIRST) && !p.at_op("**") {
                return false;
            }
            if order.done {
                p.error("`**` takes the other keyword arguments, so it comes last");
            }
            let star = p.at(T![*]) || p.at_op("**");
            if star && p.at(T![*]) && order.keyword {
                p.error("there's already a `*` or `*args` before this");
            }
            match star {
                true if p.at(T![*]) => order.keyword = true,
                true => order.done = true,
                false => {}
            }
            param(p);
            true
        },
    );
    m.complete(p, PARAM_LIST);
}

/// One parameter: `...`, `args: ...`, a pattern with an optional type
/// and default value, like `x`, `x: T` or `x: T = 1`, a rest like
/// `*args: *Ts` or `**opts: **Ks`, or a `*` alone.
pub(crate) fn param(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    outer_attrs(p);
    if p.at(T![...]) {
        p.bump(T![...]);
        return m.complete(p, VARIADIC_PARAM);
    }
    // `*` alone, before keyword-only parameters.
    if p.at(T![*]) && (p.nth_at(1, T![,]) || p.nth_at(1, T![')'])) {
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

//! Types, as in the grammar's `_type`.

use boltlang_syntax::{SyntaxKind::*, T};

use crate::{
    grammar::{is_path_start, parse_expr, type_path},
    parser::{CompletedMarker, Parser},
    token_set::TokenSet,
};

pub(crate) const TYPE_FIRST: TokenSet = TokenSet::new(&[
    IDENT,
    T![self],
    METAVARIABLE,
    T!['('],
    T!['['],
    T![!],
]);

pub(crate) const TYPE_RECOVERY_SET: TokenSet =
    TokenSet::new(&[T![')'], T![']'], T![,], T![=], T![;], T!['{'], T!['}']]);

/// A type, including bounded types like `A + B`.
pub fn type_(p: &mut Parser) {
    type_with_bounds_cond(p, true);
}

/// A type without bounds. Used where a `+` would be ambiguous, e.g. in
/// `x as A + b`, which adds `b` to the result of the cast.
pub fn type_no_bounds(p: &mut Parser) {
    type_with_bounds_cond(p, false);
}

fn type_with_bounds_cond(p: &mut Parser, allow_bounds: bool) {
    let Some(mut lhs) = atom_type(p) else {
        return;
    };
    // `bounded_type` is left-associative: `A + B + C` is `(A + B) + C`.
    while allow_bounds && p.at(T![+]) {
        let m = lhs.precede(p);
        p.bump(T![+]);
        atom_type(p);
        lhs = m.complete(p, BOUNDED_TYPE);
    }
}

fn atom_type(p: &mut Parser) -> Option<CompletedMarker> {
    let m = match p.current() {
        T!['('] => tuple_type(p),
        T!['['] => array_type(p),
        T![!] => never_type(p),
        _ if is_path_start(p) => path_type(p),
        _ => {
            p.err_recover("expected a type", TYPE_RECOVERY_SET);
            return None;
        }
    };
    Some(m)
}

/// `Vec[T]`, `Shape.Circle`, `M[N].O`
fn path_type(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    type_path(p);
    m.complete(p, PATH_TYPE)
}

/// `()`, `(A,)` or `(A, B)`. Like the grammar, `(A)` is a tuple type too.
fn tuple_type(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.bump(T!['(']);
    while !p.at(T![')']) && !p.at(EOF) {
        if !p.at_ts(TYPE_FIRST) {
            p.err_recover("expected a type", TYPE_RECOVERY_SET);
            break;
        }
        type_(p);
        if !p.at(T![')']) {
            p.expect(T![,]);
        }
    }
    p.expect(T![')']);
    m.complete(p, TUPLE_TYPE)
}

/// `[T]` or `[T; N]`
fn array_type(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.bump(T!['[']);
    type_(p);
    if p.eat(T![;]) {
        parse_expr(p);
    }
    p.expect(T![']']);
    m.complete(p, ARRAY_TYPE)
}

/// `!`
fn never_type(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.bump(T![!]);
    m.complete(p, NEVER_TYPE)
}

/// `: A + B`, as in the grammar's `trait_bounds`.
pub(crate) fn trait_bounds(p: &mut Parser) {
    let m = p.start();
    p.bump(T![:]);
    type_no_bounds(p);
    while p.eat(T![+]) {
        type_no_bounds(p);
    }
    m.complete(p, TRAIT_BOUNDS);
}

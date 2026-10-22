//! Types, as in the grammar's `_type`.

use boltlang_syntax::{SyntaxKind::*, T};

use crate::{
    grammar::{delimited, outer_attrs, generic_param_list, is_path_start, parse_expr, pattern_single, type_path},
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
    T![fn],
    T![async],
    T![const],
    T![extern],
    T![impl],
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
        T![fn] | T![async] | T![const] | T![extern] => fn_type(p),
        T![impl] => impl_trait_type(p),
        _ if is_path_start(p) => path_or_fn_trait_type(p),
        _ => {
            p.err_recover("expected a type", TYPE_RECOVERY_SET);
            return None;
        }
    };
    Some(m)
}

/// `Vec[T]`, `Shape.Circle`, `M[N].O`, or a function type written with a
/// trait, like `FnOnce(i32) -> i32`.
fn path_or_fn_trait_type(p: &mut Parser) -> CompletedMarker {
    let path = type_path(p).expect("called at the start of a path");
    path_type_after_path(p, path)
}

/// Turn a parsed path into a PATH_TYPE, or into an FN_TYPE if `(` follows,
/// as in `FnOnce(i32) -> i32`.
pub(crate) fn path_type_after_path(p: &mut Parser, path: CompletedMarker) -> CompletedMarker {
    let m = path.precede(p);
    if p.at(T!['(']) {
        fn_type_param_list(p);
        opt_ret_type(p);
        return m.complete(p, FN_TYPE);
    }
    m.complete(p, PATH_TYPE)
}

/// `impl Trait`, or `impl for[T] Trait` to introduce generic parameters.
/// Like the grammar's `abstract_type`, the trait can have bounds:
/// `impl A + B` is one impl type of `A + B`.
fn impl_trait_type(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.bump(T![impl]);
    if p.eat(T![for]) {
        if p.at(T!['[']) {
            generic_param_list(p);
        } else {
            p.error("expected generic parameters like `[T]`");
        }
    }
    type_(p);
    m.complete(p, IMPL_TRAIT_TYPE)
}

/// `fn(A) -> B`, with optional modifiers: `async`, `const`, `extern "C"`.
fn fn_type(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    function_modifiers(p);
    p.expect(T![fn]);
    fn_type_param_list(p);
    opt_ret_type(p);
    m.complete(p, FN_TYPE)
}

/// `async`, `const` and `extern "C"`, in any order, as in the grammar's
/// `function_modifiers`.
pub(crate) fn function_modifiers(p: &mut Parser) {
    loop {
        match p.current() {
            T![async] | T![const] => p.bump_any(),
            T![extern] => abi(p),
            _ => break,
        }
    }
}

/// `extern` or `extern "C"`
pub(crate) fn abi(p: &mut Parser) {
    let m = p.start();
    p.bump(T![extern]);
    p.eat(STRING);
    m.complete(p, ABI);
}

/// `-> T`, if there is one.
pub(crate) fn opt_ret_type(p: &mut Parser) {
    if p.at(T![->]) {
        let m = p.start();
        p.bump(T![->]);
        type_no_bounds(p);
        m.complete(p, RET_TYPE);
    }
}

const FN_TYPE_PARAM_FIRST: TokenSet = TYPE_FIRST.union(TokenSet::new(&[T![_], T![mut], T![...], T![#]]));

/// The parameters of a function type. Unlike in a function declaration, a
/// bare name is a type here: `fn(i32)` takes an `i32`. A parameter can still
/// be named, as in `fn(x: i32)`.
fn fn_type_param_list(p: &mut Parser) {
    let m = p.start();
    delimited(
        p,
        T!['('],
        T![')'],
        T![,],
        || "expected a parameter".into(),
        FN_TYPE_PARAM_FIRST,
        |p| {
            if !p.at_ts(FN_TYPE_PARAM_FIRST) {
                return false;
            }
            fn_type_param(p);
            true
        },
    );
    m.complete(p, PARAM_LIST);
}

fn fn_type_param(p: &mut Parser) {
    let m = p.start();
    outer_attrs(p);
    if p.eat(T![...]) {
        m.complete(p, VARIADIC_PARAM);
        return;
    }
    let after_mut = if p.at(T![mut]) { 1 } else { 0 };
    if p.nth_at(after_mut, T![self]) && !p.nth_at(after_mut + 1, T![:]) {
        p.eat(T![mut]);
        p.bump(T![self]);
        m.complete(p, SELF_PARAM);
        return;
    }
    // `x: T` or `mut x: T`
    let named = matches!(p.nth(after_mut), IDENT | T![_] | T![self]) && p.nth_at(after_mut + 1, T![:]);
    if named {
        p.eat(T![mut]);
        if p.at(T![self]) {
            let name = p.start();
            p.bump(T![self]);
            name.complete(p, NAME);
        } else {
            pattern_single(p);
        }
        p.bump(T![:]);
        if p.eat(T![...]) {
            m.complete(p, VARIADIC_PARAM);
            return;
        }
        type_(p);
    } else if !p.eat(T![_]) {
        type_(p);
    }
    m.complete(p, PARAM);
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

//! Types, as in the grammar's `_type`.

use boltlang_syntax::{SyntaxKind::*, T};

use crate::{
    grammar::{
        TupleOrder, TupleSide, at_macro_call, at_named_field, at_spread, delimited,
        generic_param_list, is_path_start, macro_call, name, outer_attrs, parse_expr,
        pattern_single, spread, type_path,
    },
    parser::{CompletedMarker, Parser},
    token_set::TokenSet,
};

pub(crate) const TYPE_FIRST: TokenSet = TokenSet::new(&[
    IDENT,
    METAVARIABLE,
    T!['('],
    T!['['],
    T![mut],
    T![imm],
    LABEL_IDENT,
    T![!],
    T![fn],
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
        T![mut] | T![imm] if p.nth_at(1, T!['[']) => array_type(p),
        T![mut] | T![imm] | LABEL_IDENT => mut_type(p),
        T![!] => never_type(p),
        T![fn] | T![const] | T![extern] => fn_type(p),
        T![impl] => impl_trait_type(p),
        _ if at_macro_call(p) => {
            let m = p.start();
            macro_call(p);
            m.complete(p, MACRO_TYPE)
        }
        _ if is_path_start(p) => path_type(p),
        _ => {
            p.err_recover("expected a type", TYPE_RECOVERY_SET);
            return None;
        }
    };
    Some(m)
}

/// `Vec[T]`, `Shape.Circle` or `M[N].O`
fn path_type(p: &mut Parser) -> CompletedMarker {
    let path = type_path(p).expect("called at the start of a path");
    path_type_after_path(p, path)
}

/// Turn a parsed path into a PATH_TYPE. Function types are only written
/// `fn(A) -> B`: a `(` after the path, as in Rust's `FnOnce(i32) -> i32`, is
/// an error, and all of it becomes an ERROR node, so that the path isn't
/// looked up as a type.
pub(crate) fn path_type_after_path(p: &mut Parser, path: CompletedMarker) -> CompletedMarker {
    let m = path.precede(p);
    if p.at(T!['(']) {
        p.error("function types are written `fn(A) -> B`");
        fn_type_param_list(p);
        opt_ret_type(p);
        return m.complete(p, ERROR);
    }
    m.complete(p, PATH_TYPE)
}

/// `impl Trait`, or `impl for[T] Trait` to introduce generic parameters.
/// Like in the grammar's `abstract_type`, `impl` binds tighter than `+`:
/// `impl A + B` is the impl type `impl A` with the bound `B`.
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
    type_no_bounds(p);
    m.complete(p, IMPL_TRAIT_TYPE)
}

/// `fn(A) -> B`, with an optional `const`, and an `extern "C"` that's
/// reported.
fn fn_type(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    function_modifiers(p);
    p.expect(T![fn]);
    fn_type_param_list(p);
    opt_ret_type(p);
    opt_effect_row(p);
    m.complete(p, FN_TYPE)
}

/// `const`, and `extern "C"`, which is reported, in any order, as in the
/// grammar's `function_modifiers`.
pub(crate) fn function_modifiers(p: &mut Parser) {
    loop {
        match p.current() {
            T![const] => p.bump_any(),
            T![extern] => abi(p),
            _ => break,
        }
    }
}

/// `extern` or `extern "C"`, which Bolt doesn't have: `extern` is
/// reserved, and a function the host provides has `#[link(..)]`.
pub(crate) fn abi(p: &mut Parser) {
    let m = p.start();
    p.error(
        "`extern` is reserved: a function the host provides is written \
         `#[link(wasm, module = \"env\")] fn log(x: i32);`",
    );
    p.bump(T![extern]);
    p.eat(STRING);
    m.complete(p, ERROR);
}

/// `does A + B`, the effects of a function, if it has any. Each effect is
/// a type, like `Log` or `State[i32]`, or an effect variable like `E`. It
/// comes after the result type, or after the parameters if there's none.
pub(crate) fn opt_effect_row(p: &mut Parser) {
    if !p.at(T![does]) {
        return;
    }
    let m = p.start();
    p.bump(T![does]);
    type_no_bounds(p);
    while p.eat(T![+]) {
        type_no_bounds(p);
    }
    m.complete(p, EFFECT_ROW);
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

const FN_TYPE_PARAM_FIRST: TokenSet =
    TYPE_FIRST.union(TokenSet::new(&[T![_], T![mut], T![...], T![#]]));

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
    // `x: T` or `mut x: T`
    let after_mut = if p.at(T![mut]) { 1 } else { 0 };
    let named = matches!(p.nth(after_mut), IDENT | T![_]) && p.nth_at(after_mut + 1, T![:]);
    if named {
        pattern_single(p);
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
/// Named fields and rests come after the positional types, as in
/// `(A, x: B)`, `(A, *Ts)`, `(x: A, **Ks)` or `(...A)`.
fn tuple_type(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.bump(T!['(']);
    let mut order = TupleOrder::default();
    while !p.at(T![')']) && !p.at(EOF) {
        if at_named_field(p) {
            order.next(p, TupleSide::Named);
            let field = p.start();
            name(p);
            p.bump(T![:]);
            type_(p);
            field.complete(p, TUPLE_TYPE_FIELD);
        } else if at_spread(p) {
            let rest = p.start();
            let side = spread(p);
            order.next(p, side);
            type_(p);
            rest.complete(p, TUPLE_TYPE_REST);
        } else if p.at_ts(TYPE_FIRST) {
            order.next(p, TupleSide::Positional);
            type_(p);
        } else {
            p.err_recover("expected a type", TYPE_RECOVERY_SET);
            break;
        }
        if !p.at(T![')']) {
            p.expect(T![,]);
        }
    }
    p.expect(T![')']);
    m.complete(p, TUPLE_TYPE)
}

/// `[T]` or `[T; N]`, maybe after `mut` or `imm`: `mut [T]` can be
/// changed, and `imm [T]` never changes.
fn array_type(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    if !p.eat(T![mut]) {
        p.eat(T![imm]);
    }
    p.bump(T!['[']);
    type_(p);
    if p.eat(T![;]) {
        parse_expr(p);
    }
    p.expect(T![']']);
    m.complete(p, ARRAY_TYPE)
}

/// `mut T` or `imm T` for a type other than an array, like `mut Point`, or
/// a named mutability before any type, like `'m Point` or `'m [i32]`.
/// Whether the type can have a mutability is checked later, once it's known
/// what the type is.
fn mut_type(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.bump_any();
    if p.at(T![mut]) || p.at(T![imm]) || p.at(LABEL_IDENT) {
        p.error("a type can have only one `mut` or `imm`");
    }
    // Without a type, it's a mutability on its own, which can only be a
    // bound; the type checker reports it anywhere else.
    if p.at_ts(TYPE_FIRST) {
        type_no_bounds(p);
    }
    m.complete(p, MUT_TYPE)
}

/// `!`
fn never_type(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.bump(T![!]);
    m.complete(p, NEVER_TYPE)
}

/// `: A + B`, as in the grammar's `trait_bounds`. A bound can be a
/// mutability, like the `imm` of `T: imm`: nothing in `T` is more `mut` than
/// `imm` (#243).
pub(crate) fn trait_bounds(p: &mut Parser) {
    let m = p.start();
    p.bump(T![:]);
    bound(p);
    while p.eat(T![+]) {
        bound(p);
    }
    m.complete(p, TRAIT_BOUNDS);
}

fn bound(p: &mut Parser) {
    if at_mutability(p) {
        mutability(p);
    } else {
        type_no_bounds(p);
    }
}

/// Whether a mutability on its own is next, like `imm` or `'m`, rather than
/// a type that starts with one, like `imm Point`.
pub(crate) fn at_mutability(p: &Parser) -> bool {
    (p.at(T![mut]) || p.at(T![imm]) || p.at(LABEL_IDENT)) && !p.nth_at_ts(1, TYPE_FIRST)
}

/// A mutability on its own, as a bound or what's bounded: `imm`, `mut` or
/// `'m`. It's a `MUT_TYPE` without a type.
pub(crate) fn mutability(p: &mut Parser) {
    let m = p.start();
    p.bump_any();
    m.complete(p, MUT_TYPE);
}

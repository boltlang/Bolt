//! Generic parameters like the `[T: Ord, const N: usize]` in
//! `fn sort[T: Ord, const N: usize]`, as in the grammar's `type_parameters`.

use boltlang_syntax::{SyntaxKind::*, T};

use crate::{
    grammar::{LITERAL_FIRST, outer_attrs, delimited, name, parse_block_expr, parse_literal_expr, trait_bounds, type_},
    parser::Parser,
    token_set::TokenSet,
};

const GENERIC_PARAM_FIRST: TokenSet = TokenSet::new(&[IDENT, METAVARIABLE, LABEL_IDENT, T![const], T![#]]);

/// `[T, U: Bound = Default, const N: usize = 3, 'v: imm]`
pub(crate) fn generic_param_list(p: &mut Parser) {
    let m = p.start();
    delimited(
        p,
        T!['['],
        T![']'],
        T![,],
        || "expected a generic parameter".into(),
        GENERIC_PARAM_FIRST,
        generic_param,
    );
    m.complete(p, GENERIC_PARAM_LIST);
}

fn generic_param(p: &mut Parser) -> bool {
    // Attributes go before the parameter, outside its node, like siblings in
    // the grammar's `type_parameters`.
    outer_attrs(p);
    match p.current() {
        METAVARIABLE => {
            let m = p.start();
            p.bump(METAVARIABLE);
            m.complete(p, TYPE_PARAM);
        }
        IDENT => type_param(p),
        LABEL_IDENT => mutability_param(p),
        T![const] => const_param(p),
        _ => return false,
    }
    true
}

/// `T`, `T: A + B`, `T = Default`
fn type_param(p: &mut Parser) {
    let m = p.start();
    name(p);
    if p.at(T![:]) {
        trait_bounds(p);
    }
    if p.eat(T![=]) {
        type_(p);
    }
    m.complete(p, TYPE_PARAM);
}

/// `'v` or `'v: imm`: a named mutability that a struct takes, like the `'v`
/// of `struct VecIter[T, 'v] { data: 'v Vec[T] }` (#243).
fn mutability_param(p: &mut Parser) {
    let m = p.start();
    p.bump(LABEL_IDENT);
    if p.at(T![:]) {
        trait_bounds(p);
    }
    m.complete(p, MUTABILITY_PARAM);
}

/// `const N: usize` or `const N: usize = 3`. Like in the grammar, the
/// default is a block, a name or a (negative) literal.
fn const_param(p: &mut Parser) {
    let m = p.start();
    p.bump(T![const]);
    name(p);
    p.expect(T![:]);
    type_(p);
    if p.eat(T![=]) {
        match p.current() {
            T!['{'] => {
                parse_block_expr(p);
            }
            T![-] if p.nth_at_ts(1, LITERAL_FIRST) => {
                let neg = p.start();
                p.bump(T![-]);
                parse_literal_expr(p);
                neg.complete(p, PREFIX_EXPR);
            }
            IDENT => {
                let path = p.start();
                crate::grammar::expr_path(p);
                path.complete(p, PATH_EXPR);
            }
            k if LITERAL_FIRST.contains(k) => {
                parse_literal_expr(p);
            }
            _ => p.error("expected a block, a name or a literal"),
        }
    }
    m.complete(p, CONST_PARAM);
}

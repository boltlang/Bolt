//! Generic arguments like the `[K, V]` in `HashMap[K, V]`, as in the
//! grammar's `type_arguments`.

use boltlang_syntax::{SyntaxKind::*, T};

use crate::{
    grammar::{
        LITERAL_FIRST, TYPE_FIRST, delimited, name_ref, parse_block_expr, parse_literal_expr,
        PathMode, path_for_qualifier, path_type_after_path, trait_bounds, type_, type_no_bounds,
    },
    parser::Parser,
    token_set::TokenSet,
};

const GENERIC_ARG_FIRST: TokenSet = TYPE_FIRST.union(LITERAL_FIRST).union(TokenSet::new(&[T!['{'], T![-]]));

/// `[A, B]`
pub(crate) fn generic_arg_list(p: &mut Parser) {
    let m = p.start();
    delimited(
        p,
        T!['['],
        T![']'],
        T![,],
        || "expected a generic argument".into(),
        GENERIC_ARG_FIRST,
        generic_arg,
    );
    m.complete(p, GENERIC_ARG_LIST);
}

/// One generic argument, optionally followed by bounds: a type, a binding
/// like `Item = T`, or a literal or block.
fn generic_arg(p: &mut Parser) -> bool {
    let m = p.start();
    let kind = match p.current() {
        T!['{'] => {
            parse_block_expr(p);
            CONST_ARG
        }
        T![-] if p.nth_at_ts(1, LITERAL_FIRST) => {
            let lit = p.start();
            p.bump(T![-]);
            parse_literal_expr(p);
            lit.complete(p, PREFIX_EXPR);
            CONST_ARG
        }
        k if LITERAL_FIRST.contains(k) => {
            parse_literal_expr(p);
            CONST_ARG
        }
        // `Item = T` or `Item[A] = T`. Otherwise it's an ordinary path type
        // that starts the same way, so turn what was parsed into one.
        IDENT => {
            let segment = p.start();
            name_ref(p);
            if p.at(T!['[']) {
                generic_arg_list(p);
            }
            if p.eat(T![=]) {
                segment.abandon(p);
                type_(p);
                ASSOC_TYPE_ARG
            } else {
                let segment = segment.complete(p, PATH_SEGMENT);
                let mut path = segment.precede(p).complete(p, PATH);
                while p.at(T![.]) {
                    path = path_for_qualifier(p, path, PathMode::Type);
                }
                let mut ty = path_type_after_path(p, path);
                while p.at(T![+]) {
                    let bounded = ty.precede(p);
                    p.bump(T![+]);
                    type_no_bounds(p);
                    ty = bounded.complete(p, BOUNDED_TYPE);
                }
                TYPE_ARG
            }
        }
        _ if p.at_ts(TYPE_FIRST) => {
            type_(p);
            TYPE_ARG
        }
        _ => {
            m.abandon(p);
            return false;
        }
    };
    if p.at(T![:]) {
        trait_bounds(p);
    }
    m.complete(p, kind);
    true
}

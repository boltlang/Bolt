//! Attributes like `#[derive(Debug)]` and `#![allow(unused)]`, as in the
//! grammar's `attribute_item` and `inner_attribute_item`, and the token
//! trees they and macros take as arguments.

use boltlang_syntax::{SyntaxKind::*, T};

use crate::{
    grammar::{dotted_path, expr},
    parser::Parser,
};

/// Whether an inner attribute like `#![allow(unused)]` starts here.
pub(crate) fn at_inner_attr(p: &Parser) -> bool {
    p.at(T![#]) && p.nth_at(1, T![!]) && p.nth_at(2, T!['['])
}

/// Whether an outer attribute like `#[derive(Debug)]` starts here.
pub(crate) fn at_outer_attr(p: &Parser) -> bool {
    p.at(T![#]) && p.nth_at(1, T!['['])
}

/// Any number of `#[...]`.
pub(crate) fn outer_attrs(p: &mut Parser) -> bool {
    let mut any = false;
    while at_outer_attr(p) {
        attr(p, false);
        any = true;
    }
    any
}

/// `#[meta]` or `#![meta]`
pub(crate) fn attr(p: &mut Parser, inner: bool) {
    let m = p.start();
    p.bump(T![#]);
    if inner {
        p.bump(T![!]);
    }
    p.bump(T!['[']);
    meta(p);
    p.expect(T![']']);
    m.complete(p, ATTR);
}

/// `path`, `path = value` or `path(tokens)`, as in the grammar's
/// `attribute`.
fn meta(p: &mut Parser) {
    let m = p.start();
    if dotted_path(p).is_none() {
        p.error("expected the name of an attribute");
    }
    match p.current() {
        T![=] => {
            p.bump(T![=]);
            expr(p);
        }
        T!['('] | T!['['] | T!['{'] => token_tree(p),
        _ => {}
    }
    m.complete(p, META);
}

/// Any tokens between balanced `()`, `[]` or `{}`, as in the grammar's
/// `delim_token_tree`.
pub(crate) fn token_tree(p: &mut Parser) {
    let m = p.start();
    let close = match p.current() {
        T!['('] => T![')'],
        T!['['] => T![']'],
        T!['{'] => T!['}'],
        _ => unreachable!("token_tree called without an opening delimiter"),
    };
    p.bump_any();
    while !p.at(EOF) && !p.at(close) {
        match p.current() {
            T!['('] | T!['['] | T!['{'] => token_tree(p),
            // A closing delimiter that doesn't match ends the tree; leave it
            // to the enclosing tree.
            T![')'] | T![']'] | T!['}'] => break,
            _ => p.bump_any(),
        }
    }
    p.expect(close);
    m.complete(p, TOKEN_TREE);
}

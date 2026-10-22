//! Patterns, as in the grammar's `_pattern`.

use boltlang_syntax::{SyntaxKind::*, T};

use crate::{
    grammar::{LITERAL_FIRST, PATH_SEGMENT_FIRST, delimited, dotted_path, name, name_ref, parse_block_expr},
    parser::{CompletedMarker, Parser},
    token_set::TokenSet,
};

pub(crate) const PATTERN_FIRST: TokenSet = LITERAL_FIRST.union(PATH_SEGMENT_FIRST).union(TokenSet::new(&[
    T![-],
    T!['('],
    T!['['],
    T![_],
    T![mut],
    T![..],
    T![..=],
    T![|],
    T![const],
]));

const PATTERN_RECOVERY_SET: TokenSet =
    TokenSet::new(&[T![')'], T![']'], T!['}'], T![,], T![=], T![=>], T![:], T![;], T![if], T![in]]);

/// A pattern, including or-patterns like `A | B` with an optional leading
/// `|`.
pub fn parse_pattern(p: &mut Parser) -> Option<CompletedMarker> {
    let m = p.start();
    let leading_bar = p.eat(T![|]);
    let first = pattern_single(p);
    if !leading_bar && !p.at(T![|]) {
        m.abandon(p);
        return first;
    }
    while p.eat(T![|]) {
        pattern_single(p);
    }
    Some(m.complete(p, OR_PATT))
}

/// A pattern without a top-level `|`, as in parameters.
pub(crate) fn pattern_single(p: &mut Parser) -> Option<CompletedMarker> {
    if p.at(T![mut]) {
        let m = p.start();
        p.bump(T![mut]);
        pattern_single(p);
        return Some(m.complete(p, MUT_PATT));
    }
    if p.at(T![..=]) || (p.at(T![..]) && at_range_end(p, 1)) {
        let m = p.start();
        p.bump_any();
        range_end(p);
        return Some(m.complete(p, RANGE_PATT));
    }
    let lhs = atom_pattern(p)?;
    if matches!(lhs.kind(), LIT_PATT | PATH_PATT) && (p.at(T![..=]) || p.at(T![..])) {
        let m = lhs.precede(p);
        let inclusive = p.at(T![..=]);
        p.bump_any();
        if inclusive || at_range_end(p, 0) {
            range_end(p);
        }
        return Some(m.complete(p, RANGE_PATT));
    }
    Some(lhs)
}

/// Whether the token `n` ahead can end a range pattern: a literal or path.
fn at_range_end(p: &Parser, n: usize) -> bool {
    p.nth_at_ts(n, LITERAL_FIRST.union(PATH_SEGMENT_FIRST)) || (p.nth_at(n, T![-]) && p.nth_at_ts(n + 1, LITERAL_FIRST))
}

fn range_end(p: &mut Parser) {
    if p.at_ts(PATH_SEGMENT_FIRST) {
        path_pattern(p);
    } else if p.at_ts(LITERAL_FIRST) || p.at(T![-]) {
        literal_pattern(p);
    } else {
        p.err_recover("expected a literal or a path", PATTERN_RECOVERY_SET);
    }
}

fn atom_pattern(p: &mut Parser) -> Option<CompletedMarker> {
    let m = match p.current() {
        T![_] => {
            let m = p.start();
            p.bump(T![_]);
            m.complete(p, WILDCARD_PATT)
        }
        T![-] if p.nth_at_ts(1, LITERAL_FIRST) => literal_pattern(p),
        k if LITERAL_FIRST.contains(k) => literal_pattern(p),
        T!['('] => tuple_pattern(p),
        T!['['] => slice_pattern(p),
        T![..] => {
            let m = p.start();
            p.bump(T![..]);
            m.complete(p, REST_PATT)
        }
        T![const] if p.nth_at(1, T!['{']) => {
            let m = p.start();
            p.bump(T![const]);
            parse_block_expr(p);
            m.complete(p, CONST_BLOCK_PATT)
        }
        IDENT if p.nth_at(1, T![@]) => {
            let m = p.start();
            name(p);
            p.bump(T![@]);
            pattern_single(p);
            m.complete(p, CAPTURED_PATT)
        }
        // A plain name is a binding, unless something makes it a path:
        // `Color.Red`, `Some(x)`, `Point { .. }` or the start of a range.
        IDENT if !matches!(p.nth(1), T![.] | T!['('] | T!['{'] | T![..] | T![..=]) => {
            let m = p.start();
            name(p);
            m.complete(p, NAMED_PATT)
        }
        _ if p.at_ts(PATH_SEGMENT_FIRST) => path_or_struct_pattern(p),
        _ => {
            p.err_recover("expected a pattern", PATTERN_RECOVERY_SET);
            return None;
        }
    };
    Some(m)
}

/// `1`, `-1`, `"s"`
fn literal_pattern(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.eat(T![-]);
    if p.at_ts(LITERAL_FIRST) {
        p.bump_any();
    } else {
        p.error("expected a literal");
    }
    m.complete(p, LIT_PATT)
}

fn path_pattern(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    dotted_path(p);
    m.complete(p, PATH_PATT)
}

/// `Color.Red`, `Some(x)` or `Point { x, .. }`
fn path_or_struct_pattern(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    dotted_path(p);
    match p.current() {
        T!['('] => {
            pattern_list(p, T!['('], T![')']);
            m.complete(p, TUPLE_STRUCT_PATT)
        }
        T!['{'] => {
            field_pattern_list(p);
            m.complete(p, STRUCT_PATT)
        }
        _ => m.complete(p, PATH_PATT),
    }
}

/// `(a, b)`. Like in the grammar, `(a)` is a tuple pattern too.
fn tuple_pattern(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    pattern_list(p, T!['('], T![')']);
    m.complete(p, TUPLE_PATT)
}

/// `[a, .., b]`
fn slice_pattern(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    pattern_list(p, T!['['], T![']']);
    m.complete(p, SLICE_PATT)
}

fn pattern_list(p: &mut Parser, bra: boltlang_syntax::SyntaxKind, ket: boltlang_syntax::SyntaxKind) {
    delimited(p, bra, ket, T![,], || "expected a pattern".into(), PATTERN_FIRST, |p| {
        if !p.at_ts(PATTERN_FIRST) {
            return false;
        }
        parse_pattern(p);
        true
    });
}

/// `{ x, mut y, z: pattern, .. }`
fn field_pattern_list(p: &mut Parser) {
    delimited(
        p,
        T!['{'],
        T!['}'],
        T![,],
        || "expected a field".into(),
        TokenSet::new(&[IDENT, T![mut], T![..]]),
        |p| {
            match p.current() {
                T![..] => {
                    let m = p.start();
                    p.bump(T![..]);
                    m.complete(p, REST_PATT);
                }
                IDENT | T![mut] => field_pattern(p),
                _ => return false,
            }
            true
        },
    );
}

/// `x`, `mut x` or `x: pattern`
fn field_pattern(p: &mut Parser) {
    let m = p.start();
    p.eat(T![mut]);
    if p.nth_at(1, T![:]) {
        name_ref(p);
        p.bump(T![:]);
        parse_pattern(p);
    } else {
        name(p);
    }
    m.complete(p, FIELD_PATT);
}

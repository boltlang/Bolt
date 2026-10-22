//! Macro invocations like `println!("{}", x)` and definitions like
//! `macro_rules! name { ... }`, as in the grammar's `macro_invocation` and
//! `macro_definition`.

use boltlang_syntax::{SyntaxKind::*, T};

use crate::{
    grammar::{name, name_ref, token_tree},
    parser::{CompletedMarker, Marker, Parser},
};

/// Whether a macro invocation starts here: a name, `!` and an opening
/// delimiter.
pub(crate) fn at_macro_call(p: &Parser) -> bool {
    p.at(IDENT) && p.nth_at(1, T![!]) && matches!(p.nth(2), T!['('] | T!['['] | T!['{'])
}

/// `name!(tokens)`, `name![tokens]` or `name!{tokens}`. Returns whether the
/// tokens are between braces, which makes it end like a block.
pub(crate) fn macro_call(p: &mut Parser) -> (CompletedMarker, bool) {
    let m = p.start();
    name_ref(p);
    p.bump(T![!]);
    let braces = p.at(T!['{']);
    token_tree(p);
    (m.complete(p, MACRO_CALL), braces)
}

/// Whether a macro definition starts here.
pub(crate) fn at_macro_rules(p: &Parser) -> bool {
    p.at_contextual_kw(T![macro_rules]) && p.nth_at(1, T![!]) && p.nth_at(2, IDENT)
}

/// `macro_rules! name { rules }`, `macro_rules! name ( rules );` or
/// `macro_rules! name [ rules ];`, where rules are separated by `;`.
pub(crate) fn macro_rules(p: &mut Parser, m: Marker) {
    p.bump_remap(T![macro_rules]);
    p.bump(T![!]);
    name(p);
    let close = match p.current() {
        T!['('] => T![')'],
        T!['['] => T![']'],
        T!['{'] => T!['}'],
        _ => {
            p.error("expected '{', '(' or '['");
            m.complete(p, MACRO_RULES);
            return;
        }
    };
    p.bump_any();
    while !p.at(EOF) && !p.at(close) {
        if !matches!(p.current(), T!['('] | T!['['] | T!['{']) {
            p.error_and_bump("expected a macro rule");
            continue;
        }
        macro_rule(p);
        if !p.eat(T![;]) {
            break;
        }
    }
    p.expect(close);
    if close != T!['}'] {
        p.expect(T![;]);
    }
    m.complete(p, MACRO_RULES);
}

/// `(pattern) => { body }`
fn macro_rule(p: &mut Parser) {
    let m = p.start();
    macro_token_tree(p, true);
    p.expect(T![=>]);
    if matches!(p.current(), T!['('] | T!['['] | T!['{']) {
        macro_token_tree(p, false);
    } else {
        p.error("expected '{', '(' or '['");
    }
    m.complete(p, MACRO_RULE);
}

/// A token tree in a macro rule. Repetitions like `$($x),*` become
/// TOKEN_REPETITION nodes, and in a pattern, bindings like `$e:expr` become
/// TOKEN_BINDING nodes.
fn macro_token_tree(p: &mut Parser, is_pattern: bool) {
    let m = p.start();
    let close = match p.current() {
        T!['('] => T![')'],
        T!['['] => T![']'],
        _ => T!['}'],
    };
    p.bump_any();
    while !p.at(EOF) && !p.at(close) {
        match p.current() {
            T!['('] | T!['['] | T!['{'] => macro_token_tree(p, is_pattern),
            T![')'] | T![']'] | T!['}'] => break,
            T![$] if p.nth_at(1, T!['(']) => token_repetition(p, is_pattern),
            METAVARIABLE if is_pattern && p.nth_at(1, T![:]) && p.nth_at(2, IDENT) => {
                let binding = p.start();
                p.bump(METAVARIABLE);
                p.bump(T![:]);
                p.bump(IDENT);
                binding.complete(p, TOKEN_BINDING);
            }
            _ => p.bump_any(),
        }
    }
    p.expect(close);
    m.complete(p, TOKEN_TREE);
}

/// `$( tokens ) sep op`, where the separator is optional and the operator is
/// `*`, `+` or `?`.
fn token_repetition(p: &mut Parser, is_pattern: bool) {
    let m = p.start();
    p.bump(T![$]);
    macro_token_tree(p, is_pattern);
    if !matches!(p.current(), T![*] | T![+] | T![?]) && !matches!(p.current(), T!['('] | T!['['] | T!['{'] | T![')'] | T![']'] | T!['}'] | EOF) {
        // The separator, like `,` or `;`.
        p.bump_any();
    }
    if matches!(p.current(), T![*] | T![+] | T![?]) {
        p.bump_any();
    } else {
        p.error("expected '*', '+' or '?'");
    }
    m.complete(p, TOKEN_REPETITION);
}

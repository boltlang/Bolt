//! The grammar of Bolt, laid out like rust-analyzer's parser: one module
//! per area of the grammar, with shared helpers here.

mod attributes;
mod expressions;
mod generic_args;
mod generic_params;
mod items;
mod macros;
mod params;
mod paths;
mod patterns;
mod types;

pub(crate) use attributes::*;
pub use expressions::*;
pub(crate) use generic_args::*;
pub(crate) use generic_params::*;
pub(crate) use items::*;
pub(crate) use macros::*;
pub(crate) use params::*;
pub(crate) use paths::*;
pub(crate) use patterns::*;
pub use types::*;

// Entry points for single rules, which `crate::parse` can start from.
pub use items::parse_item;
pub use patterns::parse_pattern;

use crate::{parser::Parser, token_set::TokenSet};
use boltlang_syntax::{
    SyntaxKind::{self, *},
    T,
};

/// Which part of a tuple an element goes in: its positional fields, its
/// named fields, or both, as `...e` does.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum TupleSide {
    Positional,
    Named,
    Both,
}

/// Whether a spread or a rest starts here: `*`, `**` or `...`.
pub(crate) fn at_spread(p: &Parser) -> bool {
    p.at(T![*]) || p.at_op("**") || p.at(T![...])
}

/// Consume `*`, `**` or `...`, and give the part of a tuple it's in.
pub(crate) fn spread(p: &mut Parser) -> TupleSide {
    let side = if p.at(T![*]) {
        TupleSide::Positional
    } else if p.at(T![...]) {
        TupleSide::Both
    } else {
        TupleSide::Named
    };
    p.bump_any();
    side
}

/// Whether a named field like `x: ..` starts here.
pub(crate) fn at_named_field(p: &Parser) -> bool {
    p.at(IDENT) && p.nth_at(1, T![:])
}

/// The rule that a tuple's positional fields come before its named ones.
#[derive(Default)]
pub(crate) struct TupleOrder {
    named: bool,
}

impl TupleOrder {
    /// Note an element of `side`, which starts here, reporting it if it's
    /// positional after a named one.
    pub(crate) fn next(&mut self, p: &mut Parser, side: TupleSide) {
        match side {
            TupleSide::Named => self.named = true,
            TupleSide::Positional if self.named => {
                p.error("a positional field can't come after a named one");
            }
            _ => {}
        }
    }
}

/// The name of something being declared, like the `f` in `fn f()`.
pub(crate) fn name(p: &mut Parser) {
    if p.at(IDENT) {
        let m = p.start();
        p.bump(IDENT);
        m.complete(p, NAME);
    } else {
        p.err_recover("expected a name", TokenSet::EMPTY);
    }
}

/// A reference to a name: an identifier or a metavariable.
pub(crate) fn name_ref(p: &mut Parser) {
    if p.at_ts(TokenSet::new(&[IDENT, METAVARIABLE])) {
        let m = p.start();
        p.bump_any();
        m.complete(p, NAME_REF);
    } else {
        p.error_and_bump("expected a name");
    }
}

/// The `parser` passed this is required to at least consume one token if it returns `true`.
/// If the `parser` returns false, parsing will stop.
fn delimited(
    p: &mut Parser<'_>,
    bra: SyntaxKind,
    ket: SyntaxKind,
    delim: SyntaxKind,
    unexpected_delim_message: impl Fn() -> String,
    first_set: TokenSet,
    mut parser: impl FnMut(&mut Parser<'_>) -> bool,
) {
    p.bump(bra);
    while !p.at(ket) && !p.at(EOF) {
        if p.at(delim) {
            // Recover if an argument is missing and only got a delimiter,
            // e.g. `(a, , b)`.

            // Wrap the erroneous delimiter in an error node so that fixup logic gets rid of it.
            // FIXME: Ideally this should be handled in fixup in a structured way, but our list
            // nodes currently have no concept of a missing node between two delimiters.
            // So doing it this way is easier.
            let m = p.start();
            p.error(unexpected_delim_message());
            p.bump(delim);
            m.complete(p, ERROR);
            continue;
        }
        if !parser(p) {
            break;
        }
        if !p.eat(delim) {
            if p.at_ts(first_set) {
                p.error(format!("expected {}", delim.pretty()));
            } else {
                break;
            }
        }
    }
    p.expect(ket);
}

pub fn parse_source_file(p: &mut Parser) {
    let m = p.start();
    p.eat(SHEBANG);
    while !p.at(EOF) {
        // Nothing else consumes a `}` outside a block, so skip it here to
        // make progress.
        if p.at(R_BRACE) {
            p.error_and_bump("unexpected '}'");
            continue;
        }
        stmt(p, Semicolon::Required);
    }
    m.complete(p, SOURCE_FILE);
}

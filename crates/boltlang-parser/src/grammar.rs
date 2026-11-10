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

/// Which kind of tuple an element is for: one with positional fields, or
/// one with named fields. A rest pattern, `..`, is for either.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum TupleSide {
    Positional,
    Named,
    Either,
}

/// Whether a spread or a rest starts here: `*` or `**`, or `...`, which is
/// read to report it.
pub(crate) fn at_spread(p: &Parser) -> bool {
    p.at(T![*]) || p.at_op("**") || p.at(T![...])
}

/// The kind of tuple the spread here is for. `...`, an error, counts as
/// either.
pub(crate) fn spread_side(p: &Parser) -> TupleSide {
    if p.at(T![*]) {
        TupleSide::Positional
    } else if p.at(T![...]) {
        TupleSide::Either
    } else {
        TupleSide::Named
    }
}

/// Consume `*` or `**`, or `...`, which is an error.
pub(crate) fn spread(p: &mut Parser) {
    if p.at(T![...]) {
        p.error("`...` isn't a spread: write `*` for positional fields or `**` for named ones");
    }
    p.bump_any();
}

/// Whether a `*` alone starts here, before keyword-only parameters.
pub(crate) fn at_keyword_marker(p: &Parser) -> bool {
    p.at(T![*]) && (p.nth_at(1, T![,]) || p.nth_at(1, T![')']))
}

/// Whether a `/` alone, after positional-only parameters, is here.
pub(crate) fn at_positional_marker(p: &Parser) -> bool {
    p.at_op("/") && (p.nth_at(1, T![,]) || p.nth_at(1, T![')']))
}

/// Where a parameter list is, for the rules on what comes after what: the
/// parameters before a `/` are positional-only, after a `*` or a positional
/// rest they're keyword-only, and a named rest comes last.
#[derive(Default)]
pub(crate) struct ParamOrder {
    /// After a `*` or `*args`: the parameters are keyword-only.
    pub(crate) keyword: bool,
    /// After `**opts`, which comes last.
    done: bool,
    /// How many parameters came before, and whether a `/` did.
    count: usize,
    slash: bool,
}

impl ParamOrder {
    /// Report a parameter here that's out of order, and move past it.
    pub(crate) fn check(&mut self, p: &mut Parser) {
        if self.done {
            p.error("`**` takes the other keyword arguments, so it comes last");
        }
        let first = self.count == 0;
        self.count += 1;
        if at_positional_marker(p) {
            if self.slash {
                p.error("there's already a `/` before this");
            } else if self.keyword {
                p.error(
                    "`/` comes after the positional-only parameters, so before a `*` or `*args`",
                );
            } else if first {
                p.error("`/` comes after the positional-only parameters, so it can't be first");
            }
            self.slash = true;
        } else if p.at(T![*]) {
            if self.keyword {
                p.error("there's already a `*` or `*args` before this");
            }
            self.keyword = true;
        } else if p.at_op("**") {
            self.done = true;
        }
    }
}

/// Whether a named field like `x: ..` starts here.
pub(crate) fn at_named_field(p: &Parser) -> bool {
    p.at(IDENT) && p.nth_at(1, T![:])
}

/// The rule that a tuple has positional fields or named ones, not both:
/// `(1, 2)` and `(x: 1, y: 2)`, but not `(1, x: 2)`.
#[derive(Default)]
pub(crate) struct TupleKind {
    side: Option<TupleSide>,
}

impl TupleKind {
    /// Note an element for `side`, which starts here, reporting it if the
    /// tuple is of the other kind.
    pub(crate) fn next(&mut self, p: &mut Parser, side: TupleSide) {
        match (self.side, side) {
            (_, TupleSide::Either) => {}
            (None, side) => self.side = Some(side),
            (Some(had), side) if had != side => {
                p.error("a tuple has positional fields or named fields, not both");
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

/// A list like `(a, b, c)`: `bra`, elements separated by commas, and `ket`.
/// `parser` parses one element. It returns `false` if no element starts
/// here, which ends the list, and must consume a token if it returns `true`.
/// `missing_message` is the error for a missing element, like in
/// `(a, , b)`, and `first_set` holds the tokens that start an element.
fn delimited(
    p: &mut Parser<'_>,
    bra: SyntaxKind,
    ket: SyntaxKind,
    missing_message: &str,
    first_set: TokenSet,
    mut parser: impl FnMut(&mut Parser<'_>) -> bool,
) {
    p.bump(bra);
    while !p.at(ket) && !p.at(EOF) {
        if p.at(T![,]) {
            // An element is missing, as in `(a, , b)`. The comma goes in an
            // error node.
            let m = p.start();
            p.error(missing_message);
            p.bump(T![,]);
            m.complete(p, ERROR);
            continue;
        }
        if !parser(p) {
            break;
        }
        if !p.eat(T![,]) {
            if p.at_ts(first_set) {
                p.error("expected ','");
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
        stmt(p);
    }
    m.complete(p, SOURCE_FILE);
}

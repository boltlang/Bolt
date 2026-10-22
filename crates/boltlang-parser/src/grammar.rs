//! The grammar of Bolt, laid out like rust-analyzer's parser: one module
//! per area of the grammar, with shared helpers here.

mod expressions;
mod generic_args;
mod items;
mod params;
mod paths;
mod patterns;
mod types;

pub use expressions::*;
pub(crate) use generic_args::*;
pub use items::*;
pub use params::*;
pub(crate) use paths::*;
pub use patterns::*;
pub use types::*;

use boltlang_syntax::{SyntaxKind::{self, *}, T};
use crate::{
        parser::Parser,
        token_set::TokenSet
};

const PATH_NAME_REF_KINDS: TokenSet = TokenSet::new(&[IDENT]);

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

/// A reference to a name: an identifier, `self` or a metavariable.
pub(crate) fn name_ref(p: &mut Parser) {
    if p.at_ts(TokenSet::new(&[IDENT, T![self], METAVARIABLE])) {
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
                p.error(format!("expected {delim:?}"));
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
        parse_source_element(p);
    }
    m.complete(p, SOURCE_FILE);
}

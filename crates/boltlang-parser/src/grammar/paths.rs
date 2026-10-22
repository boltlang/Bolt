//! Paths like `Shape.Circle`, as in the grammar's `scoped_identifier` and
//! `scoped_type_identifier`.
//!
//! A path is a `PATH` node holding an optional qualifier `PATH` and one
//! `PATH_SEGMENT`, so `a.b.c` is `PATH(PATH(PATH(a) . b) . c)`, like in
//! rust-analyzer.

use boltlang_syntax::{SyntaxKind::*, T};

use crate::{
    grammar::name_ref,
    parser::{CompletedMarker, Parser},
    token_set::TokenSet,
};

/// The tokens that can start a path segment.
pub(crate) const PATH_SEGMENT_FIRST: TokenSet = TokenSet::new(&[IDENT, T![self], METAVARIABLE]);

pub(crate) fn is_path_start(p: &Parser) -> bool {
    p.at_ts(PATH_SEGMENT_FIRST)
}

/// A path in an expression. Expressions only have single-segment paths,
/// because `a.b` is a field access there.
pub(crate) fn expr_path(p: &mut Parser) -> Option<CompletedMarker> {
    let m = p.start();
    if path_segment(p).is_none() {
        m.abandon(p);
        return None;
    }
    Some(m.complete(p, PATH))
}

/// A dotted path like `Shape.Circle`, as used in patterns and attributes.
pub(crate) fn dotted_path(p: &mut Parser) -> Option<CompletedMarker> {
    let mut qualifier = expr_path(p)?;
    while p.at(T![.]) && p.nth_at_ts(1, PATH_SEGMENT_FIRST) {
        qualifier = path_for_qualifier(p, qualifier);
    }
    Some(qualifier)
}

/// Continue the path `qualifier` with `.` and one more segment.
pub(crate) fn path_for_qualifier(p: &mut Parser, qualifier: CompletedMarker) -> CompletedMarker {
    let m = qualifier.precede(p);
    p.bump(T![.]);
    path_segment(p);
    m.complete(p, PATH)
}

fn path_segment(p: &mut Parser) -> Option<CompletedMarker> {
    if !is_path_start(p) {
        p.error("expected a name, `self` or a metavariable");
        return None;
    }
    let m = p.start();
    name_ref(p);
    Some(m.complete(p, PATH_SEGMENT))
}

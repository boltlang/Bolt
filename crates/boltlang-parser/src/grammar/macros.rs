//! Macro invocations like `println!("{}", x)`, as in the grammar's
//! `macro_invocation`.

use boltlang_syntax::{SyntaxKind::*, T};

use crate::{
    grammar::{name_ref, token_tree},
    parser::{CompletedMarker, Parser},
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

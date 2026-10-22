use boltlang_syntax::{SyntaxKind::*, T};
use crate::{
    grammar::{generic_param_list, opt_ret_type, parse_block_expr, parse_expr, param_list},
    parser::{CompletedMarker, Parser, is_operator_kind},
};

/// The number of modifier tokens (like `pub`) at the current position.
pub(crate) fn count_modifiers(p: &Parser) -> usize {
    let mut i = 0;
    while p.nth(i) == PUB_KEYWORD {
        i += 1;
    }
    i
}

/// Whether the `fn` keyword at token `n` starts a named function or operator
/// declaration, rather than an anonymous function like `fn (x) = x`.
pub(crate) fn is_named_fn(p: &Parser, n: usize) -> bool {
    match p.nth(n + 1) {
        // `fn (+)(a b) = ...` declares an operator
        L_PAREN => is_operator_kind(p.nth(n + 2)) && p.nth(n + 3) == R_PAREN,
        // Anything else is a name, or a missing one that
        // parse_named_function_declaration reports.
        _ => true,
    }
}

pub fn parse_named_function_declaration(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.eat(PUB_KEYWORD);
    p.expect(FN_KEYWORD);
    match p.current() {
        IDENT => p.bump_any(),
        L_PAREN => {
            p.bump_any();
            if is_operator_kind(p.current()) {
                p.bump_any();
            } else {
                p.error("expected an operator");
            }
            p.expect(R_PAREN);
        },
        _ => {
            p.error("expected an identifier or an operator wrapped between '(' and ')'");
            // TODO maybe bump
        }
    }
    if p.at(T!['[']) {
        generic_param_list(p);
    }
    param_list(p);
    opt_ret_type(p);
    if p.eat(EQUALS) {
        parse_expr(p);
        p.expect(T![;]);
    } else if p.at(L_BRACE) {
        parse_block_expr(p);
    } else {
        p.expect(T![;]);
    }
    m.complete(p, FUNC_DECL)
}

//! Expressions that don't start with an operator: literals, paths, blocks,
//! parenthesized expressions, and expressions that start with a keyword.

use boltlang_syntax::{SyntaxKind::*, T};

use super::{BlockLike, at_expr_start, Restrictions, expr, expr_block_contents, expr_bp, expr_no_struct, expr_stmt};
use crate::{
    grammar::{at_macro_call, macro_call, attr, at_inner_attr, at_outer_attr, dotted_path, outer_attrs, expr_path, is_path_start, opt_ret_type, param_list, parse_pattern, pattern_single, type_},
    parser::{CompletedMarker, Marker, Parser},
    token_set::TokenSet,
};

pub(crate) const LITERAL_FIRST: TokenSet = TokenSet::new(&[
    INT_NUMBER,
    FLOAT_NUMBER,
    BYTE,
    CHAR,
    STRING,
    BYTE_STRING,
    C_STRING,
]);

pub(crate) const ATOM_EXPR_FIRST: TokenSet = LITERAL_FIRST.union(TokenSet::new(&[
    IDENT,
    METAVARIABLE,
    T!['('],
    T!['{'],
    T!['['],
    T![mut],
    T![|],
    T![return],
    T![fn],
    T![if],
    T![let],
    T![while],
    T![loop],
    T![for],
    T![match],
    T![break],
    T![continue],
    T![yield],
    T![async],
    T![const],
    LABEL_IDENT,
]));

/// Where expression parsing gives up and leaves the token to an enclosing
/// rule.
pub(crate) const EXPR_RECOVERY_SET: TokenSet =
    TokenSet::new(&[T![')'], T![']'], T!['}'], T![,], T![;], T![let]]);

/// A literal like `1` or `"s"`. `True` and `False` are names, not literals.
pub fn parse_literal_expr(p: &mut Parser) -> Option<CompletedMarker> {
    if !p.at_ts(LITERAL_FIRST) {
        return None;
    }
    let m = p.start();
    p.bump_any();
    Some(m.complete(p, LIT_EXPR))
}

pub(super) fn atom_expr(p: &mut Parser, r: Restrictions) -> Option<(CompletedMarker, BlockLike)> {
    if let Some(m) = parse_literal_expr(p) {
        return Some((m, BlockLike::NotBlock));
    }
    if at_macro_call(p) {
        let m = p.start();
        let (_, braces) = macro_call(p);
        let blocklike = if braces { BlockLike::Block } else { BlockLike::NotBlock };
        return Some((m.complete(p, MACRO_EXPR), blocklike));
    }
    if is_path_start(p) {
        if !r.forbid_structs && at_struct_literal(p) {
            return Some((record_expr(p), BlockLike::NotBlock));
        }
        return Some((path_expr(p), BlockLike::NotBlock));
    }
    // `'label: loop { }` and other labelled loops and blocks
    if p.at(LABEL_IDENT) && p.nth_at(1, T![:]) {
        let m = p.start();
        label(p);
        p.bump(T![:]);
        let kind = match p.current() {
            T![loop] => loop_expr(p, m),
            T![while] => while_expr(p, m),
            T![for] => for_expr(p, m),
            T!['{'] => {
                block(p);
                m.complete(p, BLOCK_EXPR)
            }
            _ => {
                p.error("expected a loop or block after a label");
                m.complete(p, ERROR)
            }
        };
        return Some((kind, BlockLike::Block));
    }
    let done = match p.current() {
        T!['('] => paren_or_tuple_expr(p),
        T!['['] => array_expr(p),
        T![mut] => mut_expr(p, r),
        T![|] => closure_expr(p, r),
        _ if p.at_op("||") => closure_expr(p, r),
        T![async] if p.nth_at(1, T![|]) || p.nth_at_op(1, "||") => closure_expr(p, r),
        T!['{'] => return Some((parse_block_expr(p), BlockLike::Block)),
        T![async] | T![const] if p.nth_at(1, T!['{']) => {
            let m = p.start();
            p.bump_any();
            block(p);
            return Some((m.complete(p, BLOCK_EXPR), BlockLike::Block));
        }
        T![loop] => {
            let m = p.start();
            return Some((loop_expr(p, m), BlockLike::Block));
        }
        T![for] => {
            let m = p.start();
            return Some((for_expr(p, m), BlockLike::Block));
        }
        T![match] => return Some((match_expr(p), BlockLike::Block)),
        T![break] => break_expr(p, r),
        T![continue] => continue_expr(p),
        T![yield] => {
            let m = p.start();
            p.bump(T![yield]);
            opt_value(p, r);
            m.complete(p, YIELD_EXPR)
        }
        T![return] => return_expr(p, r),
        T![fn] => fun_expr(p, r),
        T![if] => return Some((if_expr(p), BlockLike::Block)),
        T![while] => {
            let m = p.start();
            return Some((while_expr(p, m), BlockLike::Block));
        }
        T![let] => let_expr(p, r),
        _ => {
            p.err_recover("expected an expression", EXPR_RECOVERY_SET);
            return None;
        }
    };
    Some((done, BlockLike::NotBlock))
}

/// `x` or `$x`
fn path_expr(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    expr_path(p);
    m.complete(p, PATH_EXPR)
}

/// Whether a struct literal starts here: a name or dotted path like
/// `Shape.Circle`, followed by `{`. Elsewhere in an expression, `a.b` is a
/// field access.
fn at_struct_literal(p: &Parser) -> bool {
    if !p.at(IDENT) {
        return false;
    }
    let mut n = 0;
    while p.nth_at(n + 1, T![.]) && p.nth_at(n + 2, IDENT) {
        n += 2;
    }
    p.nth_at(n + 1, T!['{'])
}

/// `Point { x, y: 1, ..base }`
fn record_expr(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    dotted_path(p);
    record_expr_field_list(p);
    m.complete(p, RECORD_EXPR)
}

fn record_expr_field_list(p: &mut Parser) {
    let m = p.start();
    p.bump(T!['{']);
    while !p.at(EOF) && !p.at(T!['}']) {
        match p.current() {
            // `..base` must come last.
            T![..] => {
                p.bump(T![..]);
                expr(p);
                if p.at(T![,]) {
                    p.error("expected '}' after the base of a struct literal");
                    p.bump(T![,]);
                }
                continue;
            }
            IDENT | INT_NUMBER | T![#] => {
                let field = p.start();
                outer_attrs(p);
                // `#[a]` with no field after it, like in `Point { #[a] }`.
                if !p.at(IDENT) && !p.at(INT_NUMBER) {
                    p.error("expected a field");
                    field.complete(p, RECORD_EXPR_FIELD);
                    if !p.at(T![,]) {
                        break;
                    }
                    p.bump(T![,]);
                    continue;
                }
                let name = p.start();
                p.bump_any();
                name.complete(p, NAME_REF);
                if p.eat(T![:]) {
                    expr(p);
                }
                field.complete(p, RECORD_EXPR_FIELD);
            }
            _ => {
                p.err_recover("expected a field", TokenSet::new(&[T!['}'], T![,]]));
                if !p.at(T![,]) {
                    break;
                }
            }
        }
        if !p.at(T!['}']) {
            p.expect(T![,]);
        }
    }
    p.expect(T!['}']);
    m.complete(p, RECORD_EXPR_FIELD_LIST);
}

/// `(a)`, `(a,)`, `(a, b)` or `()`
fn paren_or_tuple_expr(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.bump(T!['(']);
    let mut saw_comma = false;
    let mut saw_expr = false;
    while !p.at(EOF) && !p.at(T![')']) {
        saw_expr = true;
        if !at_expr_start(p) {
            p.error("expected an expression");
            break;
        }
        expr(p);
        if p.eat(T![,]) {
            saw_comma = true;
        } else if at_expr_start(p) {
            // `(a b)`: assume a comma is missing.
            p.error("expected ','");
            saw_comma = true;
        } else {
            break;
        }
    }
    p.expect(T![')']);
    m.complete(p, if saw_expr && !saw_comma { NEST_EXPR } else { TUPLE_EXPR })
}

/// Something that can be changed: `mut [1, 2]`, a struct literal like
/// `mut Point { x: 1, y: 2 }`, or a call of a tuple struct's constructor,
/// like `mut Wrapper(1)`. The `mut` is part of the array, struct literal or
/// call.
fn mut_expr(p: &mut Parser, r: Restrictions) -> CompletedMarker {
    if p.nth_at(1, T!['[']) {
        return array_expr(p);
    }
    let m = p.start();
    p.bump(T![mut]);
    if is_path_start(p) {
        if !r.forbid_structs && at_struct_literal(p) {
            dotted_path(p);
            record_expr_field_list(p);
            return m.complete(p, RECORD_EXPR);
        }
        // The constructor, which can be in a module or an enum, like
        // `m.Wrapper` or `Shape.Circle`: a field access, as elsewhere.
        let mut callee = path_expr(p);
        while p.at(T![.]) && p.nth_at(1, IDENT) {
            let field = callee.precede(p);
            p.bump(T![.]);
            let name = p.start();
            p.bump(IDENT);
            name.complete(p, NAME_REF);
            callee = field.complete(p, FIELD_EXPR);
        }
        if p.at(T!['(']) {
            super::arg_list(p);
            return m.complete(p, CALL_EXPR);
        }
    }
    p.error("expected an array or a struct after `mut`, like `mut [1]` or `mut Point { x: 1 }`");
    m.complete(p, ERROR)
}

/// `[a, b, c]`, `[]` or `[x; n]`, or one of those after `mut`, like
/// `mut [1, 2]`, for an array that can be changed.
fn array_expr(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.eat(T![mut]);
    p.bump(T!['[']);
    let mut first = true;
    while !p.at(EOF) && !p.at(T![']']) {
        if !at_expr_start(p) {
            p.err_recover("expected an expression", TokenSet::new(&[T![']'], T![,]]));
            break;
        }
        expr(p);
        if first && p.eat(T![;]) {
            expr(p);
            break;
        }
        first = false;
        if p.eat(T![,]) {
            continue;
        }
        if at_expr_start(p) {
            p.error("expected ','");
        } else {
            break;
        }
    }
    p.expect(T![']']);
    m.complete(p, ARRAY_EXPR)
}

/// `{ statements; tail }`
pub fn parse_block_expr(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    block(p);
    m.complete(p, BLOCK_EXPR)
}

fn block(p: &mut Parser) {
    let m = p.start();
    if !p.expect(T!['{']) {
        m.abandon(p);
        return;
    }
    expr_block_contents(p);
    p.expect(T!['}']);
    m.complete(p, BLOCK);
}

/// `return` or `return x`
fn return_expr(p: &mut Parser, r: Restrictions) -> CompletedMarker {
    let m = p.start();
    p.bump(T![return]);
    opt_value(p, r);
    m.complete(p, RET_EXPR)
}

/// An anonymous function: `fn (x, y) = x + y`, optionally with a return
/// type: `fn (x) -> Int = x`. Like in the grammar's `function_expression`,
/// the body extends as far as possible and, like a closure body, takes the
/// restrictions of its context.
fn fun_expr(p: &mut Parser, r: Restrictions) -> CompletedMarker {
    let m = p.start();
    p.bump(T![fn]);
    param_list(p);
    opt_ret_type(p);
    if p.eat(T![=]) {
        expr_bp(p, None, Restrictions { prefer_stmt: false, ..r }, 1);
    } else {
        p.error("expected '=' before the body of the function");
        if p.at(T!['{']) {
            parse_block_expr(p);
        }
    }
    m.complete(p, FUN_EXPR)
}

/// `if cond { } else if cond { } else { }`
fn if_expr(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.bump(T![if]);
    expr_no_struct(p);
    parse_block_expr(p);
    if p.eat(T![else]) {
        if p.at(T![if]) {
            if_expr(p);
        } else {
            parse_block_expr(p);
        }
    }
    m.complete(p, IF_EXPR)
}

/// `while cond { }`. `m` was started before an optional label.
fn while_expr(p: &mut Parser, m: Marker) -> CompletedMarker {
    p.bump(T![while]);
    expr_no_struct(p);
    parse_block_expr(p);
    m.complete(p, WHILE_EXPR)
}

/// `let pattern = value` in a condition, like in `if let Some(x) = y`. The
/// value binds tighter than `&&`, so `if let a = b && c` checks `c`
/// separately, as in the grammar's `let_condition`.
fn let_expr(p: &mut Parser, r: Restrictions) -> CompletedMarker {
    let m = p.start();
    p.bump(T![let]);
    parse_pattern(p);
    p.expect(T![=]);
    expr_bp(p, None, Restrictions { prefer_stmt: false, ..r }, 5);
    m.complete(p, LET_EXPR)
}

/// `loop { }`. `m` was started before an optional label.
fn loop_expr(p: &mut Parser, m: Marker) -> CompletedMarker {
    p.bump(T![loop]);
    parse_block_expr(p);
    m.complete(p, LOOP_EXPR)
}

/// `for pattern in iterable { }`. `m` was started before an optional label.
fn for_expr(p: &mut Parser, m: Marker) -> CompletedMarker {
    p.bump(T![for]);
    parse_pattern(p);
    p.expect(T![in]);
    expr_no_struct(p);
    parse_block_expr(p);
    m.complete(p, FOR_EXPR)
}

/// `'outer`
fn label(p: &mut Parser) {
    let m = p.start();
    p.bump(LABEL_IDENT);
    m.complete(p, LABEL);
}

/// `break`, `break 'outer`, `break value` or `break 'outer value`
fn break_expr(p: &mut Parser, r: Restrictions) -> CompletedMarker {
    let m = p.start();
    p.bump(T![break]);
    if p.at(LABEL_IDENT) {
        label(p);
    }
    opt_value(p, r);
    m.complete(p, BREAK_EXPR)
}

/// `continue` or `continue 'outer`
fn continue_expr(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.bump(T![continue]);
    if p.at(LABEL_IDENT) {
        label(p);
    }
    m.complete(p, CONTINUE_EXPR)
}

/// The optional value of `break`, `return` or `yield`. In a condition, a
/// `{` starts the body, not the value: `if x { break } {`.
fn opt_value(p: &mut Parser, r: Restrictions) {
    if at_expr_start(p) && !(r.forbid_structs && p.at(T!['{'])) {
        expr(p);
    }
}

/// `match value { pattern => value, ... }`
fn match_expr(p: &mut Parser) -> CompletedMarker {
    let m = p.start();
    p.bump(T![match]);
    expr_no_struct(p);
    if p.at(T!['{']) {
        match_arm_list(p);
    } else {
        p.error("expected '{'");
    }
    m.complete(p, MATCH_EXPR)
}

const MATCH_ARM_SKIP: TokenSet = TokenSet::new(&[T![')'], T![']'], T![;]]);

fn match_arm_list(p: &mut Parser) {
    let m = p.start();
    p.bump(T!['{']);
    while !p.at(EOF) && !p.at(T!['}']) {
        if p.at(T!['{']) {
            // Recover from a stray block without eating the rest of the arms.
            let e = p.start();
            p.error("expected a match arm");
            parse_block_expr(p);
            e.complete(p, ERROR);
            continue;
        }
        // Patterns and values leave closing tokens like `)` for an enclosing
        // list to recover on, so an arm would consume nothing and the loop
        // would never end. Skip them instead.
        if p.at_ts(MATCH_ARM_SKIP) {
            p.error_and_bump("expected a match arm");
            continue;
        }
        let start = p.pos();
        match_arm(p);
        if p.pos() == start {
            p.error_and_bump("expected a match arm");
        }
    }
    p.expect(T!['}']);
    m.complete(p, MATCH_ARM_LIST);
}

/// `pattern if guard => value,`. The comma is optional after the last arm
/// and after a value that ends in a block.
fn match_arm(p: &mut Parser) {
    let m = p.start();
    // The grammar allows both outer and inner attributes here.
    while at_outer_attr(p) || at_inner_attr(p) {
        attr(p, at_inner_attr(p));
    }
    if p.at(T!['}']) {
        // Only attributes, like `#![cfg(...)]` at the start of the arms.
        m.abandon(p);
        return;
    }
    parse_pattern(p);
    if p.at(T![if]) {
        let guard = p.start();
        p.bump(T![if]);
        expr(p);
        guard.complete(p, MATCH_GUARD);
    }
    p.expect(T![=>]);
    let blocklike = match expr_stmt(p, None) {
        Some((_, blocklike)) => blocklike,
        None => BlockLike::NotBlock,
    };
    if !p.eat(T![,]) && !blocklike.is_block() && !p.at(T!['}']) {
        p.error("expected ','");
    }
    m.complete(p, MATCH_ARM);
}

/// `|a, b| a + b`, `|x: i32| -> i32 { x }`, `async |x| ...` or `|| x`.
/// The body takes the restrictions of the context: in `if let f = || a {`,
/// the `{` starts the body of the `if`.
fn closure_expr(p: &mut Parser, r: Restrictions) -> CompletedMarker {
    let m = p.start();
    p.eat(T![async]);
    let params = p.start();
    if p.at_op("||") {
        // The lexer joined the two `|` of an empty parameter list.
        p.split_bump(T![|], 1);
        p.bump(T![|]);
    } else {
        p.bump(T![|]);
        while !p.at(EOF) && !p.at(T![|]) {
            closure_param(p);
            if !p.eat(T![,]) {
                break;
            }
        }
        p.expect(T![|]);
    }
    params.complete(p, PARAM_LIST);
    if p.at(T![->]) {
        // With a return type, the body must be a block.
        opt_ret_type(p);
        if p.at(T!['{']) {
            parse_block_expr(p);
        } else {
            p.error("expected a block after the return type");
        }
    } else if !p.eat(T![_]) {
        expr_bp(p, None, Restrictions { prefer_stmt: false, ..r }, 1);
    }
    m.complete(p, CLOSURE_EXPR)
}

/// `x`, `(a, b)` or `x: T`
fn closure_param(p: &mut Parser) {
    let m = p.start();
    pattern_single(p);
    if p.eat(T![:]) {
        type_(p);
    }
    m.complete(p, PARAM);
}

#[cfg(test)]
mod tests {
    use boltlang_syntax::{SyntaxKind::*, SyntaxNode};

    use crate::parse_file;

    #[test]
    fn attributes_without_a_field_stop_at_the_brace() {
        let (green, diags) = parse_file("let p = Point { #[a] };\nlet y = 1;\n");
        let messages: Vec<_> = diags.iter().map(ToString::to_string).collect();
        assert_eq!(messages, ["expected a field"]);
        let lets = SyntaxNode::new_root(green).descendants().filter(|n| n.kind() == LET_STMT).count();
        assert_eq!(lets, 2);
    }
}

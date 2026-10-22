//! Statements and expressions, as in the grammar's `_statement` and
//! `_expression`. Ported from rust-analyzer: a Pratt parser for operators
//! ([`expr_bp`]) on top of atoms ([`atom`]).

mod atom;

pub(crate) use atom::*;

use boltlang_syntax::{SyntaxKind::{self, *}, T};

use crate::{
    grammar::{count_modifiers, is_named_fn, parse_named_function_declaration, parse_pattern, type_, type_no_bounds},
    parser::{CompletedMarker, Marker, Parser},
    token_set::TokenSet,
};

/// Whether an expression ends in a block, like `if` and `match`. As a
/// statement, such an expression doesn't need a `;`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum BlockLike {
    Block,
    NotBlock,
}

impl BlockLike {
    pub(crate) fn is_block(self) -> bool {
        self == BlockLike::Block
    }
}

/// Constraints on the expression being parsed.
#[derive(Clone, Copy)]
pub(crate) struct Restrictions {
    /// `{` can't start a struct literal, as in `if x {`.
    forbid_structs: bool,
    /// An expression ending in a block ends the statement, so `{ 1 } - 1`
    /// is a block followed by `-1`.
    prefer_stmt: bool,
}

const EXPR_FIRST: TokenSet = LHS_FIRST;

const LHS_FIRST: TokenSet = ATOM_EXPR_FIRST.union(TokenSet::new(&[T![-], T![!], T![..], T![..=]]));

/// Whether an expression can start here. Besides [`EXPR_FIRST`], `||` can
/// start a closure without parameters; the lexer joins it into an operator.
pub(crate) fn at_expr_start(p: &Parser) -> bool {
    p.at_ts(EXPR_FIRST) || p.at_op("||")
}

/// An expression.
pub fn parse_expr(p: &mut Parser) -> Option<CompletedMarker> {
    expr(p)
}

pub(crate) fn expr(p: &mut Parser) -> Option<CompletedMarker> {
    let r = Restrictions { forbid_structs: false, prefer_stmt: false };
    expr_bp(p, None, r, 1).map(|(m, _)| m)
}

/// An expression where `{` can't start a struct literal, like the condition
/// of an `if`.
pub(crate) fn expr_no_struct(p: &mut Parser) -> Option<CompletedMarker> {
    let r = Restrictions { forbid_structs: true, prefer_stmt: false };
    expr_bp(p, None, r, 1).map(|(m, _)| m)
}

fn expr_stmt(p: &mut Parser, m: Option<Marker>) -> Option<(CompletedMarker, BlockLike)> {
    let r = Restrictions { forbid_structs: false, prefer_stmt: true };
    expr_bp(p, m, r, 1)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Semicolon {
    Required,
    Optional,
}

/// A statement: `;`, a `let`, an item like a function, or an expression.
pub(crate) fn stmt(p: &mut Parser, semicolon: Semicolon) {
    // An empty statement.
    if p.eat(T![;]) {
        return;
    }
    if p.at(T![let]) {
        let_stmt(p, semicolon);
        return;
    }
    let n = count_modifiers(p);
    if p.nth_at(n, T![fn]) && is_named_fn(p, n) {
        parse_named_function_declaration(p);
        return;
    }
    if !at_expr_start(p) {
        p.error_and_bump("expected an expression, an item or a let statement");
        return;
    }
    let m = p.start();
    if let Some((cm, blocklike)) = expr_stmt(p, Some(m)) {
        // An expression at the end of a block is its value, not a statement.
        if p.at(T!['}']) {
            return;
        }
        let m = cm.precede(p);
        match semicolon {
            Semicolon::Required if blocklike.is_block() => {
                p.eat(T![;]);
            }
            Semicolon::Required => {
                p.expect(T![;]);
            }
            Semicolon::Optional => {
                p.eat(T![;]);
            }
        }
        m.complete(p, EXPR_STMT);
    }
}

/// `let pattern: Type = value else { ... };`, with the type, value and
/// `else` block optional.
pub(crate) fn let_stmt(p: &mut Parser, semicolon: Semicolon) {
    let m = p.start();
    p.bump(T![let]);
    p.eat(T![mut]);
    parse_pattern(p);
    if p.eat(T![:]) {
        type_(p);
    }
    if p.eat(T![=]) {
        expr(p);
    }
    if p.eat(T![else]) {
        parse_block_expr(p);
    }
    match semicolon {
        Semicolon::Required => {
            p.expect(T![;]);
        }
        Semicolon::Optional => {
            p.eat(T![;]);
        }
    }
    m.complete(p, LET_STMT);
}

/// The statements and tail expression of a block, up to its `}`.
pub(crate) fn expr_block_contents(p: &mut Parser) {
    while !p.at(EOF) && !p.at(T!['}']) {
        stmt(p, Semicolon::Required);
    }
}

#[derive(Clone, Copy)]
enum Associativity {
    Left,
    Right,
}

/// Binding powers of the binary operators, following the grammar's `PREC`.
/// See <https://matklad.github.io/2020/04/13/simple-but-powerful-pratt-parsing.html>.
///
/// The grammar only knows a fixed set of operators. Other operators, like
/// `<$>`, are left-associative and bind tighter than all built-in binary
/// operators except `as`, like undeclared operators in Haskell.
#[rustfmt::skip]
fn current_op(p: &Parser) -> (u8, SyntaxKind, Associativity) {
    use Associativity::*;
    const NOT_AN_OP: (u8, SyntaxKind, Associativity) = (0, T![@], Left);
    match p.current() {
        T![as] => return (13, T![as], Left),
        T![=] | ASSIGNMENT => return (1, p.current(), Right),
        // These have a fixed role elsewhere in the grammar.
        T![->] | T![=>] | T![!] | T![...] | T![.] => return NOT_AN_OP,
        _ => {}
    }
    let Some(op) = p.op_text() else {
        return NOT_AN_OP;
    };
    let (bp, assoc) = match op {
        ".." | "..="                   => (2,  Left),
        "||"                           => (3,  Left),
        "&&"                           => (4,  Left),
        "==" | "!=" | "<" | "<=" | ">" | ">=" => (5,  Left),
        "|"                            => (6,  Left),
        "^"                            => (7,  Left),
        "&"                            => (8,  Left),
        "<<" | ">>"                    => (9,  Left),
        "+" | "-"                      => (10, Left),
        "*" | "/" | "%"                => (11, Left),
        _                              => (12, Left),
    };
    (bp, p.current(), assoc)
}

/// Parse an expression whose operators bind at least as tightly as `bp`.
fn expr_bp(p: &mut Parser, m: Option<Marker>, r: Restrictions, bp: u8) -> Option<(CompletedMarker, BlockLike)> {
    let m = m.unwrap_or_else(|| p.start());
    if !at_expr_start(p) {
        p.err_recover("expected an expression", EXPR_RECOVERY_SET);
        m.abandon(p);
        return None;
    }
    let mut lhs = match lhs(p, r) {
        Some((lhs, blocklike)) => {
            let lhs = lhs.extend_to(p, m);
            if r.prefer_stmt && blocklike.is_block() {
                return Some((lhs, BlockLike::Block));
            }
            lhs
        }
        None => {
            m.abandon(p);
            return None;
        }
    };

    loop {
        let (op_bp, op, associativity) = current_op(p);
        if op_bp < bp {
            break;
        }
        if p.at(T![as]) {
            lhs = cast_expr(p, lhs);
            continue;
        }
        let is_range = p.at(T![..]) || p.at(T![..=]);
        let m = lhs.precede(p);
        p.bump(op);
        if is_range {
            // `a..` has no end.
            if !at_range_end(p, r) {
                lhs = m.complete(p, RANGE_EXPR);
                break;
            }
            expr_bp(p, None, Restrictions { prefer_stmt: false, ..r }, op_bp + 1);
            lhs = m.complete(p, RANGE_EXPR);
            continue;
        }
        let op_bp = match associativity {
            Associativity::Left => op_bp + 1,
            Associativity::Right => op_bp,
        };
        expr_bp(p, None, Restrictions { prefer_stmt: false, ..r }, op_bp);
        lhs = m.complete(p, BIN_EXPR);
    }
    Some((lhs, BlockLike::NotBlock))
}

/// Whether an expression follows that can end a range. In a condition, `{`
/// starts the body: `for i in 0.. {`.
fn at_range_end(p: &Parser, r: Restrictions) -> bool {
    at_expr_start(p) && !(r.forbid_structs && p.at(T!['{']))
}

/// A prefix operator applied to an expression, or an atom followed by
/// postfix operators like calls.
fn lhs(p: &mut Parser, r: Restrictions) -> Option<(CompletedMarker, BlockLike)> {
    // `..b`, `..=b` or `..`
    if p.at(T![..]) || p.at(T![..=]) {
        let m = p.start();
        p.bump_any();
        if at_range_end(p, r) {
            expr_bp(p, None, Restrictions { prefer_stmt: false, ..r }, 3);
        }
        return Some((m.complete(p, RANGE_EXPR), BlockLike::NotBlock));
    }
    if matches!(p.current(), T![-] | T![!]) {
        let m = p.start();
        p.bump_any();
        // Prefix operators bind tighter than casts and binary operators, but
        // not as tight as postfix operators: `-f(x)` is `-(f(x))`.
        expr_bp(p, None, r, 255);
        return Some((m.complete(p, PREFIX_EXPR), BlockLike::NotBlock));
    }
    let (lhs, blocklike) = atom_expr(p, r)?;
    let (lhs, blocklike) = postfix_expr(p, lhs, blocklike, !(r.prefer_stmt && blocklike.is_block()));
    Some((lhs, blocklike))
}

fn postfix_expr(
    p: &mut Parser,
    mut lhs: CompletedMarker,
    mut blocklike: BlockLike,
    // As a statement, `{ } (x)` is a block followed by a tuple, not a call.
    mut allow_calls: bool,
) -> (CompletedMarker, BlockLike) {
    loop {
        lhs = match p.current() {
            T!['('] if allow_calls => call_expr(p, lhs),
            T!['['] if allow_calls => index_expr(p, lhs),
            T![?] => try_expr(p, lhs),
            T![.] => dot_expr(p, lhs),
            _ => break,
        };
        allow_calls = true;
        blocklike = BlockLike::NotBlock;
    }
    (lhs, blocklike)
}

/// `f(a, b)`
fn call_expr(p: &mut Parser, lhs: CompletedMarker) -> CompletedMarker {
    let m = lhs.precede(p);
    arg_list(p);
    m.complete(p, CALL_EXPR)
}

fn arg_list(p: &mut Parser) {
    let m = p.start();
    crate::grammar::delimited(
        p,
        T!['('],
        T![')'],
        T![,],
        || "expected an expression".into(),
        EXPR_FIRST,
        |p| expr(p).is_some(),
    );
    m.complete(p, ARG_LIST);
}

/// `xs[i]`
fn index_expr(p: &mut Parser, lhs: CompletedMarker) -> CompletedMarker {
    let m = lhs.precede(p);
    p.bump(T!['[']);
    expr(p);
    p.expect(T![']']);
    m.complete(p, INDEX_EXPR)
}

/// `x.await`, `x.field` or `x.0`. A method call like `x.f()` is a call of
/// the field access `x.f`, as in the grammar.
fn dot_expr(p: &mut Parser, lhs: CompletedMarker) -> CompletedMarker {
    let m = lhs.precede(p);
    p.bump(T![.]);
    match p.current() {
        T![await] => {
            p.bump(T![await]);
            return m.complete(p, AWAIT_EXPR);
        }
        IDENT | INT_NUMBER => {
            let name = p.start();
            p.bump_any();
            name.complete(p, NAME_REF);
        }
        // `t.0.1` is lexed with the float `0.1`. Split it into `0`, `.` and
        // `1`, like rust-analyzer does.
        FLOAT_NUMBER if is_tuple_index_float(p.nth_text(0)) => {
            let text = p.nth_text(0).to_string();
            let dot = text.find('.').unwrap();
            let name = p.start();
            p.split_bump_as(INT_NUMBER, dot, T![.]);
            name.complete(p, NAME_REF);
            let lhs = m.complete(p, FIELD_EXPR);
            if dot + 1 == text.len() {
                // `t.0.` followed by something else: leave the `.` to the
                // caller.
                return lhs;
            }
            let m = lhs.precede(p);
            p.split_bump_as(T![.], 1, INT_NUMBER);
            let name = p.start();
            p.bump(INT_NUMBER);
            name.complete(p, NAME_REF);
            return m.complete(p, FIELD_EXPR);
        }
        _ => p.error("expected a field name or a number"),
    }
    m.complete(p, FIELD_EXPR)
}

/// Whether a float is two tuple indices, like `0.1`, rather than something
/// like `1e5` or `1.5f32`.
fn is_tuple_index_float(text: &str) -> bool {
    text.chars().all(|ch| ch.is_ascii_digit() || ch == '.') && text.matches('.').count() == 1
}

/// `x?`
fn try_expr(p: &mut Parser, lhs: CompletedMarker) -> CompletedMarker {
    let m = lhs.precede(p);
    p.bump(T![?]);
    m.complete(p, TRY_EXPR)
}

/// `x as T`. The type has no bounds: `x as A + b` adds `b` to the cast.
fn cast_expr(p: &mut Parser, lhs: CompletedMarker) -> CompletedMarker {
    let m = lhs.precede(p);
    p.bump(T![as]);
    type_no_bounds(p);
    m.complete(p, CAST_EXPR)
}

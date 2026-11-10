//! Items, as in the grammar's `_declaration_statement`: functions, structs,
//! enums, traits, impls, imports and so on. Laid out like rust-analyzer's
//! `items` module.

use boltlang_syntax::{
    SyntaxKind::{self, *},
    T,
};

use crate::{
    grammar::{
        Semicolon, TYPE_FIRST, abi, at_inner_attr, at_macro_call, at_macro_rules, at_mutability,
        attr, delimited, dotted_path, expr, function_modifiers, generic_param_list, is_path_start,
        let_stmt, macro_call, macro_rules, mutability, name, name_ref, opt_effect_row,
        opt_generic_param_list, opt_ret_type, outer_attrs, param, param_list, parse_block_expr,
        trait_bounds, type_, type_no_bounds,
    },
    parser::{Marker, Parser},
    token_set::TokenSet,
};

/// Parse a single item, like a function or a struct, with the attributes
/// before it.
pub fn parse_item(p: &mut Parser) {
    let m = p.start();
    outer_attrs(p);
    if let Err(m) = opt_item(p, m) {
        m.abandon(p);
        p.error("expected an item");
    }
}

/// Parse an item if one starts here, completing `m` with it. `m` was
/// started before the item, so that attributes can be part of it.
pub(crate) fn opt_item(p: &mut Parser, m: Marker) -> Result<(), Marker> {
    if at_macro_rules(p) {
        macro_rules(p, m);
        return Ok(());
    }
    let has_visibility = opt_visibility(p);
    match p.current() {
        _ if at_fn(p) => fn_item(p, m),
        T![struct] => struct_item(p, m),
        // `imm struct String([u8]);`: a struct whose values are all `imm`.
        T![imm] if p.nth_at(1, T![struct]) => {
            p.bump(T![imm]);
            struct_item(p, m);
        }
        T![enum] => enum_item(p, m),
        T![const] if p.nth_at(1, IDENT) => const_item(p, m),
        T![type] => type_alias(p, m),
        T![class] => class_item(p, m),
        T![effect] => effect_item(p, m),
        T![impl] => impl_item(p, m),
        T![extern] => extern_block(p, m),
        T![import] => import_item(p, m),
        _ if has_visibility => {
            p.error("expected an item after the visibility");
            m.complete(p, ERROR);
        }
        _ => return Err(m),
    }
    Ok(())
}

/// Whether a named function starts here, possibly after modifiers like
/// `const`, or an `extern "C"` that's reported. `fn (x) = x` is an
/// anonymous function instead.
fn at_fn(p: &Parser) -> bool {
    let mut n = 0;
    loop {
        match p.nth(n) {
            T![const] => n += 1,
            T![extern] => {
                n += 1;
                if p.nth_at(n, STRING) {
                    n += 1;
                }
            }
            T![fn] => return is_named_fn(p, n),
            _ => return false,
        }
    }
}

/// Whether the `fn` keyword at token `n` starts a named function or operator
/// definition, rather than an anonymous function like `fn (x) = x`.
fn is_named_fn(p: &Parser, n: usize) -> bool {
    match p.nth(n + 1) {
        // `fn (a: T) + (b: T)` defines an operator.
        T!['('] => closing_paren(p, n + 1).is_some_and(|close| is_infix_operator(p.nth(close + 1))),
        // Anything else is a name, an operator definition, or a missing name
        // that fn_item reports.
        _ => true,
    }
}

/// Whether an operator definition starts at the current token, just after
/// `fn`: `fn[T] ...`, `fn a + b`, `fn (a: T) + (b: T)` or `fn -x`.
fn at_operator_def(p: &Parser) -> bool {
    match p.current() {
        T!['['] => true,
        IDENT => is_infix_operator(p.nth(1)),
        T!['('] => closing_paren(p, 0).is_some_and(|close| is_infix_operator(p.nth(close + 1))),
        kind => is_prefix_operator(kind),
    }
}

/// The operators that `fn a + b` can define: the built-in binary operators
/// and others like `<>`, but not those with another role, like `->` or `.`.
fn is_infix_operator(kind: SyntaxKind) -> bool {
    matches!(kind, OPERATOR | VBAR | LT | GT | STAR | PLUS | MINUS)
}

/// The operators that `fn -x` can define.
fn is_prefix_operator(kind: SyntaxKind) -> bool {
    matches!(kind, MINUS | EXCL)
}

/// The position of the `)` that closes the `(` at token `open`, if it's
/// before the end of the statement.
fn closing_paren(p: &Parser, open: usize) -> Option<usize> {
    let mut depth = 0;
    for n in open.. {
        match p.nth(n) {
            T!['('] => depth += 1,
            T![')'] => {
                depth -= 1;
                if depth == 0 {
                    return Some(n);
                }
            }
            T![;] | T!['{'] | T!['}'] | EOF => return None,
            _ => {}
        }
    }
    None
}

/// `pub`, `pub(super)` or `pub(package)`
pub(crate) fn opt_visibility(p: &mut Parser) -> bool {
    if !p.at(T![pub]) {
        return false;
    }
    let m = p.start();
    p.bump(T![pub]);
    if p.at(T!['(']) && matches!(p.nth(1), T![super] | T![package] | IDENT) && p.nth_at(2, T![')'])
    {
        // Another name, like the `self` of Rust's `pub(self)`, is parsed
        // like these, so that only it is reported.
        if p.nth_at(1, IDENT) {
            p.error("a visibility takes `super` or `package`, like `pub(package)`");
        }
        p.bump(T!['(']);
        p.bump_any();
        p.bump(T![')']);
    }
    m.complete(p, VISIBILITY);
    true
}

/// A function, following the grammar's `function_item` and
/// `function_signature_item`:
///
/// - `fn f[T](x: T) -> T where T: Ord { ... }`
/// - `fn f(x: T) -> T;` without a body
/// - `fn f(x: T) -> T = x;` with an expression body
/// - `fn Vec.new() -> Vec[T]` in the namespace of a type
/// - `fn a + b = add(a, b);` or `fn -x = neg(x);` for an operator, see
///   [`operator_def`]
fn fn_item(p: &mut Parser, m: Marker) {
    function_modifiers(p);
    p.expect(T![fn]);
    if at_operator_def(p) {
        operator_def(p);
    } else {
        // `fn Vec.new()`: a function in the namespace of the type `Vec`.
        if p.at(IDENT) && p.nth_at(1, T![.]) && p.nth_at(2, IDENT) {
            let owner = p.start();
            p.bump(IDENT);
            owner.complete(p, NAME_REF);
            p.bump(T![.]);
        }
        match p.current() {
            IDENT | METAVARIABLE => {
                let name = p.start();
                p.bump_any();
                name.complete(p, NAME);
            }
            _ => p.error("expected a name or an operator definition"),
        }
        opt_generic_param_list(p);
        param_list(p);
    }
    opt_ret_type(p);
    opt_effect_row(p);
    opt_where_clause(p);
    if p.eat(T![=]) {
        expr(p);
        p.expect(T![;]);
    } else if p.at(T!['{']) {
        parse_block_expr(p);
    } else {
        p.expect(T![;]);
    }
    m.complete(p, FUNC_DECL);
}

/// The head of an operator definition, after `fn`, following the grammar's
/// `operator_definition`: `fn a + b`, `fn[T] (a: T) + (b: T)`, `fn -x` or
/// `fn !(f: Flag)`. The operands are the parameters, so they go in a
/// PARAM_LIST, with the OPERATOR_NAME between or before them.
fn operator_def(p: &mut Parser) {
    opt_generic_param_list(p);
    let params = p.start();
    if is_prefix_operator(p.current()) {
        operator_name(p);
        operand(p);
    } else {
        operand(p);
        if is_infix_operator(p.current()) {
            operator_name(p);
        } else {
            p.error("expected an operator");
        }
        operand(p);
    }
    params.complete(p, PARAM_LIST);
}

fn operator_name(p: &mut Parser) {
    let m = p.start();
    p.bump_any();
    m.complete(p, OPERATOR_NAME);
}

/// A name, or a parameter in parentheses: `a` or `(a: T)`.
fn operand(p: &mut Parser) {
    match p.current() {
        IDENT => {
            let param = p.start();
            let pat = p.start();
            name(p);
            pat.complete(p, NAMED_PATT);
            param.complete(p, PARAM);
        }
        T!['('] => {
            p.bump(T!['(']);
            param(p);
            p.expect(T![')']);
        }
        _ => p.error("expected an operand: a name, or a parameter in parentheses"),
    }
}

/// `where T: Ord, Vec[T]: Hash, Convert[A, B]`, following the grammar's
/// `where_clause`.
pub(crate) fn opt_where_clause(p: &mut Parser) {
    if !p.at(T![where]) {
        return;
    }
    let m = p.start();
    p.bump(T![where]);
    while p.at_ts(TYPE_FIRST) {
        // `T: Show + Eq`, or a class constraint on its own: `Convert[A, B]`.
        // What's bounded can be a mutability, as in `'v: 'm`.
        let pred = p.start();
        if at_mutability(p) {
            mutability(p);
        } else {
            type_no_bounds(p);
        }
        if p.at(T![:]) {
            trait_bounds(p);
        }
        pred.complete(p, WHERE_PRED);
        if !p.eat(T![,]) {
            break;
        }
    }
    m.complete(p, WHERE_CLAUSE);
}

/// `struct Point[T] where T: Num { x: T, y: T }`, `struct Pair(A, B);` or
/// `struct Unit;`, after an `imm` if there's one. Like in the grammar, a
/// `where` clause goes before named fields but after tuple fields.
fn struct_item(p: &mut Parser, m: Marker) {
    p.bump(T![struct]);
    name(p);
    opt_generic_param_list(p);
    match p.current() {
        T![;] => p.bump(T![;]),
        T!['('] => {
            tuple_field_list(p);
            opt_where_clause(p);
            p.expect(T![;]);
        }
        _ => {
            opt_where_clause(p);
            if p.at(T!['{']) {
                record_field_list(p);
            } else {
                p.error("expected '{', '(' or ';'");
            }
        }
    }
    m.complete(p, STRUCT);
}

/// `enum Option[T] { None, Some(T) }`
fn enum_item(p: &mut Parser, m: Marker) {
    p.bump(T![enum]);
    name(p);
    opt_generic_param_list(p);
    opt_where_clause(p);
    if p.at(T!['{']) {
        variant_list(p);
    } else {
        p.error("expected '{'");
    }
    m.complete(p, ENUM);
}

/// The tokens that start a variant or a named field: its name, or the
/// visibility or attributes before it.
const NAMED_MEMBER_FIRST: TokenSet = TokenSet::new(&[IDENT, T![pub], T![#]]);

const TUPLE_FIELD_FIRST: TokenSet = TYPE_FIRST.union(TokenSet::new(&[T![pub], T![#]]));

fn variant_list(p: &mut Parser) {
    let m = p.start();
    delimited(
        p,
        T!['{'],
        T!['}'],
        T![,],
        || "expected a variant".into(),
        NAMED_MEMBER_FIRST,
        |p| {
            if !p.at_ts(NAMED_MEMBER_FIRST) {
                return false;
            }
            variant(p);
            true
        },
    );
    m.complete(p, VARIANT_LIST);
}

/// `None`, `Some(T)`, `Point { x: i32 }` or `A = 1`
fn variant(p: &mut Parser) {
    let m = p.start();
    outer_attrs(p);
    opt_visibility(p);
    name(p);
    match p.current() {
        T!['{'] => record_field_list(p),
        T!['('] => tuple_field_list(p),
        _ => {}
    }
    if p.eat(T![=]) {
        expr(p);
    }
    m.complete(p, VARIANT);
}

/// `{ x: i32, pub y: i32 }`
pub(crate) fn record_field_list(p: &mut Parser) {
    let m = p.start();
    delimited(
        p,
        T!['{'],
        T!['}'],
        T![,],
        || "expected a field".into(),
        NAMED_MEMBER_FIRST,
        |p| {
            if !p.at_ts(NAMED_MEMBER_FIRST) {
                return false;
            }
            let field = p.start();
            outer_attrs(p);
            opt_visibility(p);
            name(p);
            p.expect(T![:]);
            type_(p);
            field.complete(p, RECORD_FIELD);
            true
        },
    );
    m.complete(p, RECORD_FIELD_LIST);
}

/// `(i32, pub String)`
pub(crate) fn tuple_field_list(p: &mut Parser) {
    let m = p.start();
    delimited(
        p,
        T!['('],
        T![')'],
        T![,],
        || "expected a type".into(),
        TUPLE_FIELD_FIRST,
        |p| {
            if !p.at_ts(TUPLE_FIELD_FIRST) {
                return false;
            }
            let field = p.start();
            outer_attrs(p);
            opt_visibility(p);
            type_(p);
            field.complete(p, TUPLE_FIELD);
            true
        },
    );
    m.complete(p, TUPLE_FIELD_LIST);
}

/// `const NAME: Type = value;`, with the value optional.
fn const_item(p: &mut Parser, m: Marker) {
    p.bump(T![const]);
    name(p);
    p.expect(T![:]);
    type_(p);
    if p.eat(T![=]) {
        expr(p);
    }
    p.expect(T![;]);
    m.complete(p, CONST);
}

/// `type A[T] where T: Ord = B[T];`, following the grammar's `type_item`,
/// or `type Item[T]: Bound where T: Ord;` in a trait, following its
/// `associated_type`. A `where` clause can also come after the type.
fn type_alias(p: &mut Parser, m: Marker) {
    p.bump(T![type]);
    name(p);
    opt_generic_param_list(p);
    if p.at(T![:]) {
        trait_bounds(p);
    }
    opt_where_clause(p);
    if p.eat(T![=]) {
        type_(p);
        opt_where_clause(p);
    }
    p.expect(T![;]);
    m.complete(p, TYPE_ALIAS);
}

/// `class Show[T] where T: Eq { items }`, following the grammar's
/// `class_item`. The class parameters are required, and superclasses go in
/// the `where` clause.
fn class_item(p: &mut Parser, m: Marker) {
    p.bump(T![class]);
    name(p);
    if p.at(T!['[']) {
        generic_param_list(p);
    } else {
        p.error("expected the parameters of the class, like `[T]`");
    }
    opt_where_clause(p);
    if p.at(T!['{']) {
        item_list(p);
    } else {
        p.error("expected '{'");
    }
    m.complete(p, CLASS);
}

/// `effect State[S] { fn get() -> S; fn set(value: S); }`: an effect and
/// its operations. Unlike a class, an effect needs no parameters.
fn effect_item(p: &mut Parser, m: Marker) {
    p.bump(T![effect]);
    name(p);
    opt_generic_param_list(p);
    if p.at(T!['{']) {
        item_list(p);
    } else {
        p.error("expected '{'");
    }
    m.complete(p, EFFECT);
}

/// A class instance: `impl Show[i32] { }` or
/// `impl[T] Show[Vec[T]] where T: Show { }`, or without a body:
/// `impl Marker[i32];`. Functions for a type are defined at the top level
/// instead, and called like `x.f()`.
fn impl_item(p: &mut Parser, m: Marker) {
    p.bump(T![impl]);
    // Like in the grammar, `[` right after `impl` starts generic parameters.
    opt_generic_param_list(p);
    // Whether the path names a class is up to name resolution. Another kind
    // of type is still parsed, so that the items after it are too.
    if is_path_start(p) {
        type_no_bounds(p);
    } else {
        let e = p.start();
        p.error("expected a class, like `Show[i32]`");
        type_no_bounds(p);
        e.complete(p, ERROR);
    }
    opt_where_clause(p);
    if p.at(T!['{']) {
        item_list(p);
    } else {
        p.expect(T![;]);
    }
    m.complete(p, IMPL);
}

/// `{ items }`, as in the grammar's `declaration_list`: items, let
/// statements and empty statements.
pub(crate) fn item_list(p: &mut Parser) {
    let m = p.start();
    p.bump(T!['{']);
    while !p.at(EOF) && !p.at(T!['}']) {
        if p.eat(T![;]) {
            continue;
        }
        if at_inner_attr(p) {
            attr(p, true);
            continue;
        }
        let item = p.start();
        let has_attrs = outer_attrs(p);
        if p.at(T![let]) {
            let_stmt(p, item, Semicolon::Required);
            continue;
        }
        if has_attrs && p.at(T!['}']) {
            item.abandon(p);
            continue;
        }
        if at_macro_call(p) {
            let (call, braces) = macro_call(p);
            call.extend_to(p, item);
            if !braces {
                p.expect(T![;]);
            } else {
                p.eat(T![;]);
            }
            continue;
        }
        if let Err(item) = opt_item(p, item) {
            item.abandon(p);
            if p.at(T!['{']) {
                // Skip a stray block as a whole.
                let e = p.start();
                p.error("expected an item");
                parse_block_expr(p);
                e.complete(p, ERROR);
            } else {
                p.error_and_bump("expected an item");
            }
        }
    }
    p.expect(T!['}']);
    m.complete(p, ITEM_LIST);
}

/// `extern "C" { items }` or `extern "C";`, which Bolt doesn't have:
/// `extern` is reserved, and a function the host provides has
/// `#[link(..)]`. It's parsed to report it, and its items are left out.
fn extern_block(p: &mut Parser, m: Marker) {
    abi(p);
    if p.at(T!['{']) {
        item_list(p);
    } else {
        p.eat(T![;]);
    }
    m.complete(p, ERROR);
}

/// `import "./path";`, `import "./path" (a, b as c);`,
/// `import "./path" as name;`, `import "./path" *;` or `import Option.*;`,
/// following the grammar's `import_declaration`.
fn import_item(p: &mut Parser, m: Marker) {
    p.bump(T![import]);
    if is_path_start(p) {
        // `import Option.*;` brings the variants of an enum into scope.
        dotted_path(p);
        if p.at_op(".*") {
            // The lexer joined `.` and `*`.
            p.split_bump(T![.], 1);
            p.bump(T![*]);
        } else if p.at(T![.]) && p.nth_at(1, T![*]) {
            p.bump(T![.]);
            p.bump(T![*]);
        } else {
            p.error("expected '.*' after the path, to import the variants of an enum");
        }
        p.expect(T![;]);
        m.complete(p, IMPORT);
        return;
    }
    if !p.eat(STRING) {
        p.error("expected a string with the path to import");
    }
    match p.current() {
        T!['('] => import_list(p),
        T![as] => {
            p.bump(T![as]);
            name(p);
        }
        T![*] => p.bump(T![*]),
        _ => {}
    }
    p.expect(T![;]);
    m.complete(p, IMPORT);
}

/// `(a, b as c, ==)`. An operator imports every function of that name,
/// prefix and infix: `-` brings in both `fn -a` and `fn a - b`.
fn import_list(p: &mut Parser) {
    let m = p.start();
    delimited(
        p,
        T!['('],
        T![')'],
        T![,],
        || "expected a name or an operator".into(),
        TokenSet::new(&[IDENT, OPERATOR, VBAR, LT, GT, STAR, PLUS, MINUS, EXCL]),
        |p| {
            if is_infix_operator(p.current()) || is_prefix_operator(p.current()) {
                let name = p.start();
                p.bump_any();
                name.complete(p, NAME_REF);
                return true;
            }
            if !p.at(IDENT) {
                return false;
            }
            if p.nth_at(1, T![as]) {
                let alias = p.start();
                name_ref(p);
                p.bump(T![as]);
                name(p);
                alias.complete(p, IMPORT_ALIAS);
            } else {
                name_ref(p);
            }
            true
        },
    );
    m.complete(p, IMPORT_LIST);
}

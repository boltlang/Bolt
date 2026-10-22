//! Items, as in the grammar's `_declaration_statement`: functions, structs,
//! enums, traits, impls, imports and so on. Laid out like rust-analyzer's
//! `items` module.

use boltlang_syntax::{SyntaxKind::*, T};

use crate::{
    grammar::{
        abi, at_macro_call, at_macro_rules, macro_rules, macro_call, attr, at_inner_attr, delimited, outer_attrs, name_ref, expr, function_modifiers, let_stmt, Semicolon, generic_param_list, name, type_, opt_ret_type, param_list, parse_block_expr,
        trait_bounds, type_no_bounds, TYPE_FIRST,
    },
    parser::{Marker, Parser, is_operator_kind},
    token_set::TokenSet,
};

/// Parse an item if one starts here, completing `m` with it. `m` was
/// started before the item, so that attributes can be part of it.
pub(crate) fn opt_item(p: &mut Parser, m: Marker) -> Result<(), Marker> {
    if at_macro_rules(p) {
        macro_rules(p, m);
        return Ok(());
    }
    let has_visibility = opt_visibility(p);
    if at_fn(p) {
        fn_item(p, m);
        return Ok(());
    }
    match p.current() {
        T![struct] => {
            struct_item(p, m);
            return Ok(());
        }
        T![enum] => {
            enum_item(p, m);
            return Ok(());
        }
        T![const] if p.nth_at(1, IDENT) => {
            const_item(p, m);
            return Ok(());
        }
        T![type] => {
            type_alias(p, m);
            return Ok(());
        }
        T![class] => {
            class_item(p, m);
            return Ok(());
        }
        T![impl] => {
            impl_item(p, m);
            return Ok(());
        }
        T![extern] => {
            extern_block(p, m);
            return Ok(());
        }
        T![import] => {
            import_item(p, m);
            return Ok(());
        }
        _ => {}
    }
    if has_visibility {
        p.error("expected an item after the visibility");
        m.complete(p, ERROR);
        return Ok(());
    }
    Err(m)
}

/// Whether a named function starts here, possibly after modifiers like
/// `async` or `extern "C"`. `fn (x) = x` is an anonymous function instead.
fn at_fn(p: &Parser) -> bool {
    let mut n = 0;
    loop {
        match p.nth(n) {
            T![async] | T![const] => n += 1,
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
/// declaration, rather than an anonymous function like `fn (x) = x`.
fn is_named_fn(p: &Parser, n: usize) -> bool {
    match p.nth(n + 1) {
        // `fn (+)(a, b) = ...` declares an operator
        T!['('] => is_operator_kind(p.nth(n + 2)) && p.nth_at(n + 3, T![')']),
        // Anything else is a name, or a missing one that fn_item reports.
        _ => true,
    }
}

/// `pub`, `pub(self)`, `pub(super)` or `pub(package)`
pub(crate) fn opt_visibility(p: &mut Parser) -> bool {
    if !p.at(T![pub]) {
        return false;
    }
    let m = p.start();
    p.bump(T![pub]);
    if p.at(T!['(']) && matches!(p.nth(1), T![self] | T![super] | T![package]) && p.nth_at(2, T![')']) {
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
/// - `fn (+)(a, b) = add(a, b);` for an operator
fn fn_item(p: &mut Parser, m: Marker) {
    function_modifiers(p);
    p.expect(T![fn]);
    match p.current() {
        IDENT | METAVARIABLE => {
            let name = p.start();
            p.bump_any();
            name.complete(p, NAME);
        }
        T!['('] => {
            let name = p.start();
            p.bump(T!['(']);
            if is_operator_kind(p.current()) {
                p.bump_any();
            } else {
                p.error("expected an operator");
            }
            p.expect(T![')']);
            name.complete(p, OPERATOR_NAME);
        }
        _ => p.error("expected a name or an operator wrapped between '(' and ')'"),
    }
    if p.at(T!['[']) {
        generic_param_list(p);
    }
    param_list(p);
    opt_ret_type(p);
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
        let pred = p.start();
        type_no_bounds(p);
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
/// `struct Unit;`. Like in the grammar, a `where` clause goes before named
/// fields but after tuple fields.
fn struct_item(p: &mut Parser, m: Marker) {
    p.bump(T![struct]);
    name(p);
    if p.at(T!['[']) {
        generic_param_list(p);
    }
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
    if p.at(T!['[']) {
        generic_param_list(p);
    }
    opt_where_clause(p);
    if p.at(T!['{']) {
        variant_list(p);
    } else {
        p.error("expected '{'");
    }
    m.complete(p, ENUM);
}

fn variant_list(p: &mut Parser) {
    let m = p.start();
    delimited(
        p,
        T!['{'],
        T!['}'],
        T![,],
        || "expected a variant".into(),
        TokenSet::new(&[IDENT, T![pub], T![#]]),
        |p| {
            if !p.at(IDENT) && !p.at(T![pub]) && !p.at(T![#]) {
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
        TokenSet::new(&[IDENT, T![pub], T![#]]),
        |p| {
            if !p.at(IDENT) && !p.at(T![pub]) && !p.at(T![#]) {
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
        TYPE_FIRST.union(TokenSet::new(&[T![pub], T![#]])),
        |p| {
            if !p.at_ts(TYPE_FIRST) && !p.at(T![pub]) && !p.at(T![#]) {
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
    if p.at(T!['[']) {
        generic_param_list(p);
    }
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

/// `impl[T] Type { }`, `impl Trait for Type { }`, `impl !Send for T {}` or
/// `impl T for U;`
fn impl_item(p: &mut Parser, m: Marker) {
    p.bump(T![impl]);
    // Like in the grammar, `[` right after `impl` starts generic parameters.
    if p.at(T!['[']) {
        generic_param_list(p);
    }
    let negative = p.eat(T![!]);
    type_no_bounds(p);
    if p.eat(T![for]) {
        type_no_bounds(p);
    } else if negative {
        p.error("expected 'for' after a negative trait");
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

/// `extern "C" { items }` or `extern "C";`, following the grammar's
/// `foreign_mod_item`.
fn extern_block(p: &mut Parser, m: Marker) {
    abi(p);
    if p.at(T!['{']) {
        item_list(p);
    } else {
        p.expect(T![;]);
    }
    m.complete(p, EXTERN_BLOCK);
}

/// `import "./path";`, `import "./path" (a, b as c);`,
/// `import "./path" as name;` or `import "./path" *;`, following the
/// grammar's `import_declaration`.
fn import_item(p: &mut Parser, m: Marker) {
    p.bump(T![import]);
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

/// `(a, b as c)`
fn import_list(p: &mut Parser) {
    let m = p.start();
    delimited(
        p,
        T!['('],
        T![')'],
        T![,],
        || "expected a name".into(),
        TokenSet::new(&[IDENT]),
        |p| {
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

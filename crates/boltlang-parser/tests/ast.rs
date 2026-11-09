//! Checks that the typed AST in `boltlang_syntax::ast` can read the trees
//! this parser produces.

use boltlang_parser::parse_file;
use boltlang_syntax::{
    SyntaxNode,
    ast::{EffectRow, Expr, GenericArg, Node, Pattern, SourceElement, SourceFile, Type},
};

/// Parse `let x = <expr>;` and return the initializer.
fn let_expr(source: &str) -> Expr {
    let (green, diags) = parse_file(source);
    assert!(diags.is_empty(), "{source:?} has diagnostics: {diags:?}");
    let file = SourceFile::cast(SyntaxNode::new_root(green)).unwrap();
    match file.elements().next() {
        Some(SourceElement::VarDecl(decl)) => decl.expr().expect("missing initializer"),
        _ => panic!("{source:?} doesn't start with a let"),
    }
}

/// The source text of an expression.
fn text(expr: Option<Expr>) -> String {
    expr.expect("missing expression").syntax().text().to_string()
}

#[test]
fn bin_expr() {
    let Expr::Bin(bin) = let_expr("let x = a + b * c;") else { panic!() };
    assert_eq!(text(bin.lhs()), "a");
    assert_eq!(bin.operator().unwrap().text(), "+");
    assert_eq!(text(bin.rhs()), "b * c");
    assert!(matches!(bin.rhs(), Some(Expr::Bin(_))));
}

#[test]
fn prefix_expr() {
    let Expr::Prefix(prefix) = let_expr("let x = -a;") else { panic!() };
    assert_eq!(prefix.operator().unwrap().text(), "-");
    assert_eq!(text(prefix.expr()), "a");
}

#[test]
fn nest_expr() {
    let Expr::Nest(nest) = let_expr("let x = (a + b);") else { panic!() };
    assert!(matches!(nest.expr(), Some(Expr::Bin(_))));
}

#[test]
fn tuple_expr() {
    let Expr::Tuple(tuple) = let_expr("let x = (a, b + c);") else { panic!() };
    let elements: Vec<_> = tuple.elements().map(|e| e.syntax().text().to_string()).collect();
    assert_eq!(elements, ["a", "b + c"]);
}

#[test]
fn index_expr() {
    let Expr::Index(index) = let_expr("let x = xs[i + 1];") else { panic!() };
    assert_eq!(text(index.base()), "xs");
    assert_eq!(text(index.index()), "i + 1");
}

#[test]
fn ret_expr() {
    let Expr::Ret(ret) = let_expr("let x = return a + b;") else { panic!() };
    assert!(matches!(ret.expr(), Some(Expr::Bin(_))));
}

#[test]
fn fun_expr_body_can_be_any_expression() {
    let Expr::Fun(fun) = let_expr("let f = fn (x) = x + 1;") else { panic!() };
    assert!(matches!(fun.body(), Some(Expr::Bin(_))));
}

#[test]
fn func_decl_block_body() {
    let (green, diags) = parse_file("fn f(x) { x + 1 }");
    assert!(diags.is_empty(), "{diags:?}");
    let file = SourceFile::cast(SyntaxNode::new_root(green)).unwrap();
    let Some(SourceElement::FuncDecl(decl)) = file.elements().next() else { panic!() };
    assert!(matches!(decl.body(), Some(Expr::Block(_))));
}

#[test]
fn call_expr_args() {
    let Expr::Call(call) = let_expr("let x = f(a, b + c);") else { panic!() };
    assert_eq!(text(call.operator()), "f");
    let args: Vec<_> = call.args().map(|e| e.syntax().text().to_string()).collect();
    assert_eq!(args, ["a", "b + c"]);
}

#[test]
fn path_expr() {
    let Expr::Path(path_expr) = let_expr("let x = foo;") else { panic!() };
    let path = path_expr.path().unwrap();
    assert!(path.qualifier().is_none());
    let segments: Vec<_> = path.segments().iter().map(|s| s.name_ref().unwrap().text()).collect();
    assert_eq!(segments, ["foo"]);
}

#[test]
fn path_type_with_generic_args() {
    let (green, diags) = parse_file("let m: HashMap[K, Vec[V]] = x;");
    assert!(diags.is_empty(), "{diags:?}");
    let file = SourceFile::cast(SyntaxNode::new_root(green)).unwrap();
    let Some(SourceElement::VarDecl(decl)) = file.elements().next() else { panic!() };
    assert!(matches!(decl.pattern(), Some(Pattern::Named(_))));
    let Some(Type::Path(ty)) = decl.type_expr() else { panic!() };
    let segment = ty.path().unwrap().segment().unwrap();
    assert_eq!(segment.name_ref().unwrap().text(), "HashMap");
    let args: Vec<_> = segment.generic_arg_list().unwrap().args().collect();
    assert_eq!(args.len(), 2);
    let GenericArg::Type(arg) = &args[1] else { panic!() };
    assert!(matches!(arg.ty(), Some(Type::Path(_))));
}

#[test]
fn func_decl_params() {
    let (green, diags) = parse_file("fn f(a, b: i32 = 1, rest: ...) = a;");
    assert!(diags.is_empty(), "{diags:?}");
    let file = SourceFile::cast(SyntaxNode::new_root(green)).unwrap();
    let Some(SourceElement::FuncDecl(decl)) = file.elements().next() else { panic!() };
    let list = decl.param_list().unwrap();
    assert!(list.variadic_param().is_some());
    let params = decl.params();
    assert_eq!(params.len(), 2);
    assert!(params[0].type_expr().is_none() && params[0].default().is_none());
    assert!(matches!(params[1].type_expr(), Some(Type::Path(_))));
    assert!(matches!(params[1].default(), Some(Expr::Lit(_))));
}

#[test]
fn struct_pattern() {
    let (green, diags) = parse_file("let Shape.Circle { radius, center: (x, _), .. } = s;");
    assert!(diags.is_empty(), "{diags:?}");
    let file = SourceFile::cast(SyntaxNode::new_root(green)).unwrap();
    let Some(SourceElement::VarDecl(decl)) = file.elements().next() else { panic!() };
    let Some(Pattern::Struct(pattern)) = decl.pattern() else { panic!() };
    let path: Vec<_> =
        pattern.path().unwrap().segments().iter().map(|s| s.name_ref().unwrap().text()).collect();
    assert_eq!(path, ["Shape", "Circle"]);
    let fields: Vec<_> = pattern.fields().collect();
    assert_eq!(fields[0].name().unwrap().text(), "radius");
    assert_eq!(fields[1].name_ref().unwrap().text(), "center");
    assert!(matches!(fields[1].pattern(), Some(Pattern::Tuple(_))));
    assert!(pattern.rest().is_some());
}

#[test]
fn block_statements_and_tail() {
    let (green, diags) = parse_file("fn f() { let Some(x) = o else { return; }; g(x); x + 1 }");
    assert!(diags.is_empty(), "{diags:?}");
    let file = SourceFile::cast(SyntaxNode::new_root(green)).unwrap();
    let Some(SourceElement::FuncDecl(decl)) = file.elements().next() else { panic!() };
    let Some(Expr::Block(body)) = decl.body() else { panic!() };
    let block = body.block().unwrap();
    let elements: Vec<_> = block.elements().collect();
    assert_eq!(elements.len(), 2);
    let SourceElement::VarDecl(let_stmt) = &elements[0] else { panic!() };
    assert!(let_stmt.else_block().is_some());
    let SourceElement::ExprStmt(stmt) = &elements[1] else { panic!() };
    assert!(matches!(stmt.expr(), Some(Expr::Call(_))));
    assert!(matches!(block.tail_expr(), Some(Expr::Bin(_))));
}

#[test]
fn doc_comments() {
    use boltlang_syntax::ast::{CommentKind, HasDocs};
    let source = "//! The file\n\n// plain\n/// First\n/** Second */\nfn f() {}\n//// not a doc\nfn g() {}\n";
    let (green, diags) = parse_file(source);
    assert!(diags.is_empty(), "{diags:?}");
    let file = SourceFile::cast(SyntaxNode::new_root(green)).unwrap();
    let inner: Vec<_> =
        file.inner_doc_comments().iter().map(|c| c.doc_text().unwrap().to_string()).collect();
    assert_eq!(inner, [" The file"]);
    let items: Vec<_> = file.elements().collect();
    let SourceElement::FuncDecl(f) = &items[0] else { panic!() };
    let docs: Vec<_> =
        f.doc_comments().iter().map(|c| (c.kind(), c.doc_text().unwrap().to_string())).collect();
    assert_eq!(
        docs,
        [
            (CommentKind::OuterDoc, " First".to_string()),
            (CommentKind::OuterDoc, " Second ".to_string())
        ]
    );
    let SourceElement::FuncDecl(g) = &items[1] else { panic!() };
    assert!(g.doc_comments().is_empty());
}

/// Parse a file without diagnostics, and give its first statement.
fn first_element(source: &str) -> SourceElement {
    let (green, diags) = parse_file(source);
    assert!(diags.is_empty(), "{source:?} has diagnostics: {diags:?}");
    let file = SourceFile::cast(SyntaxNode::new_root(green)).unwrap();
    file.elements().next().expect("no statement")
}

/// The source text of each effect after `does`.
fn effects(row: Option<EffectRow>) -> Vec<String> {
    row.expect("no `does`").effects().map(|t| t.syntax().text().to_string()).collect()
}

#[test]
fn effect_decl() {
    let SourceElement::Effect(effect) =
        first_element("effect State[S] { fn get() -> S; fn set(value: S); }")
    else {
        panic!()
    };
    assert_eq!(effect.name().unwrap().syntax().text().to_string(), "State");
    assert_eq!(effect.generic_param_list().unwrap().params().count(), 1);
    assert_eq!(effect.item_list().unwrap().items().count(), 2);
}

#[test]
fn func_decl_effects() {
    let SourceElement::FuncDecl(f) = first_element("fn f() -> () does Async + Log { }") else {
        panic!()
    };
    assert!(f.ret_type().is_some());
    assert_eq!(effects(f.effect_row()), ["Async", "Log"]);

    // Without a result type, `does` comes right after the parameters.
    let SourceElement::FuncDecl(g) = first_element("fn g(x) does State[i32] = x;") else {
        panic!()
    };
    assert!(g.ret_type().is_none());
    assert_eq!(effects(g.effect_row()), ["State[i32]"]);
    assert!(matches!(g.body(), Some(Expr::Path(_))));
}

#[test]
fn fn_type_effects_belong_to_the_nearest_fn() {
    let SourceElement::VarDecl(decl) = first_element("let k: fn(A) -> fn(B) -> C does Log = f;")
    else {
        panic!()
    };
    let Some(Type::Fn(outer)) = decl.type_expr() else { panic!() };
    assert!(outer.effect_row().is_none());
    let Some(Type::Fn(inner)) = outer.ret_type().unwrap().ty() else { panic!() };
    assert_eq!(effects(inner.effect_row()), ["Log"]);
}

#[test]
fn closure_effects() {
    let Expr::Closure(closure) = let_expr("let a = |x| does Log { log(x) };") else { panic!() };
    assert!(closure.ret_type().is_none());
    assert_eq!(effects(closure.effect_row()), ["Log"]);
    assert!(matches!(closure.body(), Some(Expr::Block(_))));
}

#[test]
fn effect_arms() {
    let Expr::Match(m) = let_expr("let x = match f() { v => v, effect yield(n), k => n };") else {
        panic!()
    };
    let arms = m.arms();
    assert!(arms[0].pattern().is_some() && arms[0].effect_pattern().is_none());
    assert!(arms[1].pattern().is_none());
    let effect = arms[1].effect_pattern().unwrap();
    assert_eq!(effect.operation().unwrap().syntax().text().to_string(), "yield(n)");
    assert_eq!(effect.continuation().unwrap().syntax().text().to_string(), "k");
    assert_eq!(text(arms[1].expr()), "n");
}

#[test]
fn effect_arms_with_several_operations() {
    let Expr::Match(m) =
        let_expr("let x = match f() { v => v, effect info(m) | State.warn(m), k => m };")
    else {
        panic!()
    };
    let effect = m.arms()[1].effect_pattern().unwrap();
    let ops: Vec<String> = effect.operations().map(|p| p.syntax().text().to_string()).collect();
    assert_eq!(ops, ["info(m)", "State.warn(m)"]);
    assert_eq!(effect.continuation().unwrap().syntax().text().to_string(), "k");
}

#[test]
fn tuple_expr_with_spreads() {
    use boltlang_syntax::ast::{Spread, TupleExprItem};
    let Expr::Tuple(tuple) = let_expr("let x = (1, *xs);") else { panic!() };
    let items: Vec<_> = tuple.items().collect();
    assert_eq!(items.len(), 2);
    assert!(matches!(&items[0], TupleExprItem::Positional(_)));
    let TupleExprItem::Spread(s) = &items[1] else { panic!() };
    assert_eq!(s.spread(), Some(Spread::Positional));
    assert_eq!(text(s.expr()), "xs");
    // Only the positional elements.
    assert_eq!(tuple.elements().count(), 1);
}

#[test]
fn tuple_expr_with_named_fields_and_spreads() {
    use boltlang_syntax::ast::{Spread, TupleExprItem};
    let Expr::Tuple(tuple) = let_expr("let x = (y: 2, **opts);") else { panic!() };
    let items: Vec<_> = tuple.items().collect();
    assert_eq!(items.len(), 2);
    let TupleExprItem::Named(f) = &items[0] else { panic!() };
    assert_eq!(f.name_ref().unwrap().text(), "y");
    assert_eq!(text(f.expr()), "2");
    let TupleExprItem::Spread(s) = &items[1] else { panic!() };
    assert_eq!(s.spread(), Some(Spread::Named));
}

#[test]
fn a_named_field_alone_is_a_tuple() {
    assert!(matches!(let_expr("let x = (y: 1);"), Expr::Tuple(_)));
    assert!(matches!(let_expr("let x = (*xs);"), Expr::Tuple(_)));
    assert!(matches!(let_expr("let x = (y);"), Expr::Nest(_)));
}

#[test]
fn tuple_types_with_rests() {
    use boltlang_syntax::ast::{Spread, TupleTypeItem};
    let (green, diags) = parse_file("type A = (i32, *Ts); type B = (x: i32, **Ks);");
    assert!(diags.is_empty(), "{diags:?}");
    let root = SyntaxNode::new_root(green);
    let tuples: Vec<_> =
        root.descendants().filter_map(boltlang_syntax::ast::TupleType::cast).collect();
    let a: Vec<_> = tuples[0].items().collect();
    assert!(matches!(&a[0], TupleTypeItem::Positional(_)));
    let TupleTypeItem::Rest(r) = &a[1] else { panic!() };
    assert_eq!(r.spread(), Some(Spread::Positional));
    let b: Vec<_> = tuples[1].items().collect();
    let TupleTypeItem::Named(f) = &b[0] else { panic!() };
    assert_eq!(f.name().unwrap().text(), "x");
    let TupleTypeItem::Rest(r) = &b[1] else { panic!() };
    assert_eq!(r.spread(), Some(Spread::Named));
}

#[test]
fn pack_params() {
    use boltlang_syntax::ast::{GenericParam, PackParam, Spread};
    let (green, diags) = parse_file("fn f[T, *Ts, **Ks]() = 1;");
    assert!(diags.is_empty(), "{diags:?}");
    let root = SyntaxNode::new_root(green);
    let packs: Vec<_> = root.descendants().filter_map(PackParam::cast).collect();
    let spreads: Vec<_> = packs.iter().map(|p| p.spread()).collect();
    assert_eq!(spreads, [Some(Spread::Positional), Some(Spread::Named)]);
    let names: Vec<_> = packs.iter().map(|p| p.name().unwrap().text()).collect();
    assert_eq!(names, ["Ts", "Ks"]);
    let list = root.descendants().find_map(boltlang_syntax::ast::GenericParamList::cast).unwrap();
    assert_eq!(list.params().filter(|p| matches!(p, GenericParam::Pack(_))).count(), 2);
}

#[test]
fn tuple_pattern_with_named_fields() {
    let (green, diags) = parse_file("let (x: b, ..) = t;");
    assert!(diags.is_empty(), "{diags:?}");
    let root = SyntaxNode::new_root(green);
    let tuple = root.descendants().find_map(boltlang_syntax::ast::TuplePattern::cast).unwrap();
    let items: Vec<_> = tuple.items().collect();
    assert_eq!(items.len(), 2);
    let Err(f) = &items[0] else { panic!() };
    assert_eq!(f.name_ref().unwrap().text(), "x");
    assert!(matches!(&items[1], Ok(Pattern::Rest(_))));
}

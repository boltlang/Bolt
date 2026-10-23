//! Checks that the typed AST in `boltlang_syntax::ast` can read the trees
//! this parser produces.

use boltlang_parser::parse_file;
use boltlang_syntax::{
    SyntaxNode,
    ast::{Expr, GenericArg, Node, Pattern, SourceElement, SourceFile, Type},
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
    let path: Vec<_> = pattern.path().unwrap().segments().iter().map(|s| s.name_ref().unwrap().text()).collect();
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
    let inner: Vec<_> = file.inner_doc_comments().iter().map(|c| c.doc_text().unwrap().to_string()).collect();
    assert_eq!(inner, [" The file"]);
    let items: Vec<_> = file.elements().collect();
    let SourceElement::FuncDecl(f) = &items[0] else { panic!() };
    let docs: Vec<_> = f.doc_comments().iter().map(|c| (c.kind(), c.doc_text().unwrap().to_string())).collect();
    assert_eq!(docs, [(CommentKind::OuterDoc, " First".to_string()), (CommentKind::OuterDoc, " Second ".to_string())]);
    let SourceElement::FuncDecl(g) = &items[1] else { panic!() };
    assert!(g.doc_comments().is_empty());
}

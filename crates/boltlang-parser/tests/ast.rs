//! Checks that the typed AST in `boltlang_syntax::ast` can read the trees
//! this parser produces.

use boltlang_parser::parse_file;
use boltlang_syntax::{
    SyntaxNode,
    ast::{Expr, Node, SourceElement, SourceFile},
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

use boltlang_base_db::TestDb;
use boltlang_hir_def::{
    Name,
    arena::Idx,
    builtin_type::{BuiltinInt, BuiltinType},
    hir::{Literal, LogicOp},
};
use boltlang_hir_ty::{
    Ty,
    builtin::{Builtin, PrimOp},
};
use boltlang_pattern::decision::Ctor;

use crate::{
    Body, BodyOwner, ExprId, ExprKind, ItemRef, Local, LocalDecl, Pattern, Place, Shim, Stmt,
    Visitor, VisitorMut, walk_expr, walk_expr_mut,
};

fn i32() -> Ty<'static> {
    Ty::Builtin(BuiltinType::Int(BuiltinInt::I32))
}

fn bool() -> Ty<'static> {
    Ty::Builtin(BuiltinType::Bool)
}

fn int(body: &mut Body<'static>, n: u128) -> ExprId {
    body.alloc(ExprKind::Literal(Literal::Int(n, None)), i32(), None)
}

fn binding(body: &mut Body<'static>, index: usize, name: &str, ty: Ty<'static>) -> Local {
    body.bindings
        .insert(Idx::new(index), LocalDecl { name: Some(Name::new(name)), ty, mutable: false });
    Local::Binding(Idx::new(index))
}

fn local(body: &mut Body<'static>, l: Local) -> ExprId {
    let ty = body.local(l).unwrap().ty.clone();
    body.alloc(ExprKind::Local(l), ty, None)
}

fn add(body: &mut Body<'static>, a: ExprId, b: ExprId) -> ExprId {
    let add = ItemRef::Builtin(Builtin::Prim(PrimOp::Add, BuiltinType::Int(BuiltinInt::I32)));
    let callee = body.alloc(ExprKind::Item(add), Ty::unit(), None);
    body.alloc(ExprKind::Call { callee, args: Box::new([a, b]) }, i32(), None)
}

/// A body of a shim, which needs no file: `(n: i32) -> i32`, which counts
/// `n` down in a loop, and returns `imm` of a tuple.
fn sample() -> Body<'static> {
    let mut body = Body::new(BodyOwner::Shim(Shim::ImmDeep(i32())));
    let n = binding(&mut body, 0, "n", i32());
    body.params.push(n);
    let t = body.new_temp(None, i32());

    let one = int(&mut body, 1);
    let n1 = local(&mut body, n);
    let sum = add(&mut body, n1, one);
    let let_t = Stmt::Let { pat: Pattern::Local(Some(t)), init: Some(sum), else_: None };

    let label = body.new_label(None);
    let n2 = local(&mut body, n);
    let zero = int(&mut body, 0);
    let stop = body.alloc(ExprKind::Break { label, value: None }, Ty::unit(), None);
    let cond = body.alloc(ExprKind::Logic { op: LogicOp::Or, lhs: n2, rhs: zero }, bool(), None);
    let check = body.alloc(ExprKind::If { cond, then: stop, else_: None }, Ty::unit(), None);
    let one = int(&mut body, 1);
    let step = body.alloc(
        ExprKind::Assign { place: Place::Local(n), op: None, value: one },
        Ty::unit(),
        None,
    );
    let inner = body.alloc(
        ExprKind::Block {
            stmts: Box::new([Stmt::Expr(check), Stmt::Expr(step)]),
            tail: None,
            label: None,
        },
        Ty::unit(),
        None,
    );
    let lp = body.alloc(ExprKind::Loop { body: inner, label }, Ty::unit(), None);

    let tl = local(&mut body, t);
    let arr = body.alloc(ExprKind::Array { elems: Box::new([tl]) }, Ty::unit(), None);
    let todo = body.alloc(ExprKind::Unsupported("`e?`"), i32(), None);
    let tuple = body.alloc(
        ExprKind::Construct { ctor: Ctor::Tuple(2), fields: Box::new([arr, todo]) },
        Ty::unit(),
        None,
    );
    let tail = body.alloc(ExprKind::Imm { expr: tuple }, Ty::unit(), None);
    body.root = body.alloc(
        ExprKind::Block { stmts: Box::new([let_t, Stmt::Expr(lp)]), tail: Some(tail), label: None },
        Ty::unit(),
        None,
    );
    body
}

#[test]
fn display() {
    let db = TestDb::new();
    assert_eq!(
        sample().display(&db),
        r#"shim imm_deep[i32](n: i32) -> () = {
  let _t0 = builtin.i32_add(n, 1);
  'l0: loop {
    if n || 0 break 'l0;
    n = 1;
  };
  imm ([_t0], unsupported("`e?`"))
}
"#
    );
}

#[test]
fn display_with_types() {
    let db = TestDb::new();
    let mut body = Body::new(BodyOwner::Shim(Shim::ImmDeep(i32())));
    let one = int(&mut body, 1);
    let two = int(&mut body, 2);
    body.root = add(&mut body, one, two);
    assert_eq!(
        body.display_with_types(&db),
        "shim imm_deep[i32]() -> i32 = ((builtin.i32_add : ())((1 : i32), (2 : i32)) : i32)\n"
    );
}

#[test]
fn locals_that_share_a_name_get_their_number() {
    let db = TestDb::new();
    let mut body = Body::new(BodyOwner::Shim(Shim::ImmDeep(i32())));
    let a = binding(&mut body, 0, "x", i32());
    let b = binding(&mut body, 1, "x", i32());
    let c = binding(&mut body, 2, "y", i32());
    let (a, b, c) = (local(&mut body, a), local(&mut body, b), local(&mut body, c));
    body.root = body.alloc(
        ExprKind::Construct { ctor: Ctor::Tuple(3), fields: Box::new([a, b, c]) },
        Ty::unit(),
        None,
    );
    assert_eq!(body.display(&db), "shim imm_deep[i32]() -> () = (x#b0, x#b1, y)\n");
}

#[test]
fn a_visitor_sees_each_node_once_in_order() {
    struct Literals(Vec<u128>);
    impl<'db> Visitor<'db> for Literals {
        fn visit_expr(&mut self, body: &Body<'db>, e: ExprId) {
            if let ExprKind::Literal(Literal::Int(n, _)) = body[e].kind {
                self.0.push(n);
            }
            walk_expr(self, body, e);
        }
    }
    let body = sample();
    let mut v = Literals(Vec::new());
    v.visit_expr(&body, body.root);
    assert_eq!(v.0, [1, 0, 1]);
}

#[test]
fn a_visitor_can_rewrite_in_place() {
    // Replace each `imm e` by `e`, and each literal by twice its value.
    struct Rewrite;
    impl<'db> VisitorMut<'db> for Rewrite {
        fn visit_expr(&mut self, body: &mut Body<'db>, e: ExprId) {
            walk_expr_mut(self, body, e);
            match body[e].kind.clone() {
                ExprKind::Imm { expr } => {
                    let inner = body[expr].kind.clone();
                    body.replace(e, inner);
                }
                ExprKind::Literal(Literal::Int(n, s)) => {
                    body.replace(e, ExprKind::Literal(Literal::Int(n * 2, s)))
                }
                _ => {}
            }
        }
    }
    let db = TestDb::new();
    let mut body = sample();
    let root = body.root;
    Rewrite.visit_expr(&mut body, root);
    let text = body.display(&db);
    assert!(text.contains("builtin.i32_add(n, 2)"), "{text}");
    assert!(text.contains("n = 2;"), "{text}");
    assert!(text.contains("  ([_t0], unsupported"), "{text}");
    assert!(!text.contains("imm ("), "{text}");
}

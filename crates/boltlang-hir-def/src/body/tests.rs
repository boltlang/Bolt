use super::*;
use crate::{
    hir::{Literal, Statement},
    ids::{EnumId, ItemContainerId},
    item_tree::{ModItem, item_tree},
    test_db::TestDb,
};

fn function<'db>(db: &'db TestDb, file: File, index: usize) -> DefWithBodyId<'db> {
    let tree = item_tree(db, file);
    let ModItem::Function(idx) = tree.top_level[index] else { panic!() };
    DefWithBodyId::new(db, DefWithBody::Function(FunctionId::new(db, ItemContainerId::File(file), idx)))
}

#[test]
fn function_bodies_and_params() {
    let db = TestDb::new();
    let file = db.file("/main.bolt", "fn f(mut self, a, (b, c): (i32, i32)) { a + b }\nfn g(x) = x;\nfn h();\n");
    let f = body(&db, function(&db, file, 0));
    assert_eq!(f.params.len(), 2);
    assert!(f.store[f.self_param.unwrap()].mutable);
    assert!(matches!(f.store[f.body_expr], Expr::Block { .. }));
    let g = body(&db, function(&db, file, 1));
    assert!(matches!(g.store[g.body_expr], Expr::Path(_)));
    let h = body(&db, function(&db, file, 2));
    assert!(matches!(h.store[h.body_expr], Expr::Missing));
}

#[test]
fn const_variant_and_file_bodies() {
    let db = TestDb::new();
    let file = db.file("/main.bolt", "const N: i32 = 42;\nenum E { A, B = 7 }\nlet x = 1;\nprint(x);\n");
    let tree = item_tree(&db, file);
    let ModItem::Const(idx) = tree.top_level[0] else { panic!() };
    let def = DefWithBodyId::new(&db, DefWithBody::Const(crate::ids::ConstId::new(&db, ItemContainerId::File(file), idx)));
    let n = body(&db, def);
    assert_eq!(n.store[n.body_expr], Expr::Literal(Literal::Int(42, None)));

    let ModItem::Enum(idx) = tree.top_level[1] else { panic!() };
    let variant = EnumVariantId { parent: EnumId::new(&db, file, idx), index: 1 };
    let b = body(&db, DefWithBodyId::new(&db, DefWithBody::Variant(variant)));
    assert_eq!(b.store[b.body_expr], Expr::Literal(Literal::Int(7, None)));

    let top = body(&db, DefWithBodyId::new(&db, DefWithBody::File(file)));
    let Expr::Block { statements, .. } = &top.store[top.body_expr] else { panic!() };
    assert!(matches!(statements[..], [Statement::Item, Statement::Item, Statement::Let { .. }, Statement::Expr { .. }]));
}

/// A query that depends on the body of the first function in a file, to
/// check when it reruns. It takes the file because IDs borrow the database,
/// which the test needs to edit.
#[salsa::tracked]
fn expr_count(db: &dyn Db, file: File) -> usize {
    let tree = item_tree(db, file);
    let ModItem::Function(idx) = tree.top_level[0] else { panic!() };
    let def = DefWithBodyId::new(db, DefWithBody::Function(FunctionId::new(db, ItemContainerId::File(file), idx)));
    body(db, def).store.exprs.len()
}

#[test]
fn moving_a_body_does_not_rerun_what_depends_on_it() {
    let mut db = TestDb::new();
    let file = db.file("/main.bolt", "fn f() { 1 + 2 }\n");
    assert_eq!(*expr_count(&db, file), 4);

    // Whitespace moves the body but doesn't change it.
    let executed = db.log_executed(|db| {
        db.edit(file, "\n\nfn f() {   1 + 2 }\n");
        assert_eq!(*expr_count(db, file), 4);
    });
    assert!(executed.iter().any(|q| q.contains("body_with_source_map")), "{executed:?}");
    assert!(!executed.iter().any(|q| q.contains("expr_count")), "{executed:?}");

    let executed = db.log_executed(|db| {
        db.edit(file, "fn f() { 1 + 2 + 3 }\n");
        assert_eq!(*expr_count(db, file), 6);
    });
    assert!(executed.iter().any(|q| q.contains("expr_count")), "{executed:?}");
}

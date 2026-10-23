use boltlang_base_db::TestDb;
use boltlang_hir_def::{
    body::DefWithBody,
    builtin_type::{BuiltinInt, BuiltinType},
    hir::ExprId,
    ids::{ClassId, EnumId},
    item_tree::ModItem,
};

use super::*;
use crate::ty::AdtId;

const SOURCE: &str = "\
class Eq[T] {}
class Ord[T] where Eq[T] {}
class Show[T] {}
enum List[T] { Nil }
impl Show[i32] {}
impl[T] Show[List[T]] where Show[T] {}
";

fn i32() -> Ty<'static> {
    Ty::Builtin(BuiltinType::Int(BuiltinInt::I32))
}

struct Setup<'db> {
    eq: ClassId<'db>,
    ord: ClassId<'db>,
    show: ClassId<'db>,
    list: EnumId<'db>,
    origin: Origin<'db>,
}

fn setup(db: &TestDb) -> (Setup<'_>, File) {
    let file = db.file("/main.bolt", SOURCE);
    let tree = item_tree(db, file);
    let class = |i: usize| {
        let ModItem::Class(idx) = tree.top_level[i] else { panic!() };
        ClassId::new(db, file, idx)
    };
    let ModItem::Enum(idx) = tree.top_level[3] else { panic!() };
    let body = DefWithBodyId::new(db, DefWithBody::File(file));
    let origin = Origin::new(body, ExprOrPat::Expr(ExprId::new(0)));
    (Setup { eq: class(0), ord: class(1), show: class(2), list: EnumId::new(db, file, idx), origin }, file)
}

fn class<'db>(class: ClassId<'db>, ty: Ty<'db>) -> ClassConstraint<'db> {
    ClassConstraint { class, args: Box::new([ty]) }
}

#[test]
fn instances_from_impls() {
    let db = TestDb::new();
    let (s, file) = setup(&db);
    let list = |t| Ty::Adt(AdtId::Enum(s.list), Box::new([t]));
    let mut solver = Solver::new(&db, file);
    solver.add(Constraint::Class { constraint: class(s.show, list(list(i32()))), origin: s.origin });
    solver.add(Constraint::Class { constraint: class(s.show, list(Ty::bool())), origin: s.origin });
    solver.finish();
    // `Show[List[bool]]` needs `Show[bool]`, which has no impl.
    assert_eq!(solver.errors, [TypeError::NoInstance { origin: s.origin, constraint: class(s.show, Ty::bool()) }]);
}

#[test]
fn waits_for_variables() {
    let db = TestDb::new();
    let (s, file) = setup(&db);
    let mut solver = Solver::new(&db, file);
    let var = solver.new_var(VarKind::General);
    let int = solver.new_var(VarKind::Int);
    let unknown = solver.new_var(VarKind::General);
    solver.add(Constraint::Class { constraint: class(s.show, var.clone()), origin: s.origin });
    solver.add(Constraint::Class { constraint: class(s.show, int), origin: s.origin });
    solver.add(Constraint::Class { constraint: class(s.show, unknown.clone()), origin: s.origin });
    assert!(!solver.solve_classes());
    solver.equal(&var, &i32(), s.origin);
    assert!(solver.solve_classes());
    // The integer variable defaults to `i32`; the other stays unknown.
    solver.finish();
    assert!(matches!(&solver.errors[..], [TypeError::Ambiguous { .. }]));
}

#[test]
fn assumptions_and_superclasses() {
    let db = TestDb::new();
    let (s, file) = setup(&db);
    let def = GenericDefId::Class(s.show);
    let t = Ty::Param(GenericParamId { def, index: 0 });
    let mut solver = Solver::new(&db, file);
    solver.assume(class(s.ord, t.clone()));
    solver.add(Constraint::Class { constraint: class(s.eq, t.clone()), origin: s.origin });
    solver.add(Constraint::Class { constraint: class(s.show, t), origin: s.origin });
    solver.finish();
    assert!(matches!(&solver.errors[..], [TypeError::NoInstance { constraint, .. }] if constraint.class == s.show));
}

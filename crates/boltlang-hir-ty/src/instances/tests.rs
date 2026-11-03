use boltlang_base_db::TestDb;
use boltlang_hir_def::{
    Name,
    body::{DefWithBody, DefWithBodyId, body},
    builtin_type::{BuiltinInt, BuiltinType},
    hir::{Expr, ExprId, Path},
    item_tree::{ModItem, item_tree},
    resolver::{Resolver, ValueNs, ValueResolution},
};

use super::*;
use crate::{check::function_scheme, infer_body};

const SOURCE: &str = "\
class Describe[T] { fn describe(x: T) -> i32; }
impl Describe[i32] { fn describe(x) = 1; }
impl Describe[i64] { fn describe(x) = 2; }
fn shout(x) = describe(x);
shout(1);
shout(2i64);
";

/// The uses of the name `name` in the body `def`, in order.
fn uses(db: &dyn Db, def: DefWithBodyId<'_>, name: &str) -> Vec<ExprId> {
    body(db, def)
        .exprs
        .iter()
        .filter(|(_, e)| matches!(e, Expr::Path(p) if p.segments.len() == 1 && p.segments[0].name.as_str() == name))
        .map(|(id, _)| id)
        .collect()
}

/// The function that `name` names at `expr`.
fn function<'db>(
    db: &'db dyn Db,
    def: DefWithBodyId<'db>,
    expr: ExprId,
    name: &str,
) -> FunctionId<'db> {
    match Resolver::for_expr(db, def, expr)
        .resolve_path_in_value_ns(db, &Path::from_name(Name::new(name)))
    {
        Some(ValueResolution::Value(ValueNs::Function(f))) => f,
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_use_in_generic_code_has_the_impl_of_the_types_it_s_used_with() {
    let db = TestDb::new();
    let file = db.file("/main.bolt", SOURCE);
    let ModItem::Function(idx) = item_tree(&db, file).top_level[3] else { panic!() };
    let shout = FunctionId::new(&db, ItemContainerId::File(file), idx);
    let shout_body = DefWithBodyId::new(&db, DefWithBody::Function(shout));
    let top = DefWithBodyId::new(&db, DefWithBody::File(file));
    let int = |b| Ty::Builtin(BuiltinType::Int(b));

    // The calls give `shout` the type of their argument.
    let calls: Vec<Vec<Ty<'_>>> = uses(&db, top, "shout")
        .into_iter()
        .map(|e| infer_body(&db, top).type_args(e).unwrap().to_vec())
        .collect();
    assert_eq!(calls, [vec![int(BuiltinInt::I32)], vec![int(BuiltinInt::I64)]]);

    // In `shout`, `describe` is used with `shout`'s own type, which the
    // types of a call replace to choose the impl.
    let [use_] = uses(&db, shout_body, "describe")[..] else { panic!() };
    let describe = function(&db, shout_body, use_, "describe");
    let ItemContainerId::Class(class) = describe.container(&db) else { panic!() };
    let inner = infer_body(&db, shout_body).type_args(use_).unwrap();
    let shout_params = &function_scheme(&db, shout).unwrap().params;
    let describe_params = &function_scheme(&db, describe).unwrap().params;
    let chosen: Vec<Ty<'_>> = calls
        .iter()
        .map(|call| {
            let args: Vec<_> =
                inner.iter().map(|t| subst_scheme_args(shout_params, call, t)).collect();
            let imp =
                select_impl(&db, &[file], class, &class_args(&db, class, describe_params, &args))
                    .unwrap();
            impl_header(&db, imp).class.as_ref().unwrap().args[0].clone()
        })
        .collect();
    assert_eq!(chosen, [int(BuiltinInt::I32), int(BuiltinInt::I64)]);
}

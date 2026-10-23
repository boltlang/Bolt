use super::*;
use crate::{
    hir::{Expr, Literal, TypeRef},
    ids::ItemContainerId,
    item_tree::{AssocItem, ModItem, item_tree},
    test_db::TestDb,
};

fn top<'db>(db: &'db TestDb, file: File, index: usize) -> ModItem {
    item_tree(db, file).top_level[index]
}

/// The name of a type that is a single-segment path.
fn type_name(store: &ExpressionStore, ty: TypeRefId) -> String {
    match &store[ty] {
        TypeRef::Path(path) => path.segments.iter().map(|s| s.name.to_string()).collect::<Vec<_>>().join("."),
        other => format!("{other:?}"),
    }
}

#[test]
fn function_signatures() {
    let db = TestDb::new();
    let file = db.file(
        "/main.bolt",
        "pub async fn f[T: Show, const N: usize = 3](x: T, y: i32 = N, ...) -> [T; N] where T: Eq, Convert[T, i32] {}\n",
    );
    let ModItem::Function(idx) = top(&db, file, 0) else { panic!() };
    let sig = function_signature(&db, FunctionId::new(&db, ItemContainerId::File(file), idx));
    let s = &sig.store;
    assert_eq!(sig.visibility, RawVisibility::Public);
    assert!(sig.is_async && !sig.is_const);
    assert!(sig.is_variadic);
    assert_eq!(sig.params.len(), 2);
    assert_eq!(type_name(s, sig.params[0].type_ref.unwrap()), "T");
    assert!(sig.params[0].default.is_none());
    assert!(matches!(s[sig.params[1].default.unwrap()], Expr::Path(_)));
    assert!(matches!(s[sig.ret_type.unwrap()], TypeRef::Array { len: Some(_), .. }));

    let g = &sig.generic_params;
    assert_eq!(g.type_params.len(), 1);
    assert_eq!(g.type_params[0].name.as_str(), "T");
    assert_eq!(type_name(s, g.type_params[0].bounds[0]), "Show");
    assert_eq!(g.const_params[0].name.as_str(), "N");
    assert_eq!(s[g.const_params[0].default.unwrap()], Expr::Literal(Literal::Int(3, None)));
    assert_eq!(g.where_predicates.len(), 2);
    assert_eq!(type_name(s, g.where_predicates[0].target), "T");
    assert_eq!(type_name(s, g.where_predicates[0].bounds[0]), "Eq");
    // A bare class constraint has no bounds.
    assert!(g.where_predicates[1].bounds.is_empty());
}

#[test]
fn struct_and_enum_signatures() {
    let db = TestDb::new();
    let file = db.file(
        "/main.bolt",
        "struct P[T] { pub x: T, y: i32 }\nstruct Pair(i32, pub String);\nstruct Unit;\nenum Option[T] { None, Some(T), Named { v: T } = 3 }\n",
    );
    let ModItem::Struct(idx) = top(&db, file, 0) else { panic!() };
    let p = struct_signature(&db, StructId::new(&db, file, idx));
    let FieldList::Record(fields) = &p.fields else { panic!("{:?}", p.fields) };
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].visibility, RawVisibility::Public);
    assert_eq!(type_name(&p.store, fields[0].type_ref), "T");
    assert_eq!(fields[1].visibility, RawVisibility::Private);

    let ModItem::Struct(idx) = top(&db, file, 1) else { panic!() };
    let pair = struct_signature(&db, StructId::new(&db, file, idx));
    let FieldList::Tuple(fields) = &pair.fields else { panic!() };
    assert_eq!(fields[1].name.as_str(), "1");
    assert_eq!(type_name(&pair.store, fields[1].type_ref), "String");

    let ModItem::Struct(idx) = top(&db, file, 2) else { panic!() };
    assert_eq!(struct_signature(&db, StructId::new(&db, file, idx)).fields, FieldList::Unit);

    let ModItem::Enum(idx) = top(&db, file, 3) else { panic!() };
    let e = enum_signature(&db, EnumId::new(&db, file, idx));
    assert_eq!(e.generic_params.type_params.len(), 1);
    let names: Vec<_> = e.variants.iter().map(|v| v.name.as_str()).collect();
    assert_eq!(names, ["None", "Some", "Named"]);
    assert_eq!(e.variants[0].fields, FieldList::Unit);
    assert!(matches!(&e.variants[1].fields, FieldList::Tuple(f) if f.len() == 1));
    assert!(matches!(&e.variants[2].fields, FieldList::Record(f) if f.len() == 1));
}

#[test]
fn class_impl_const_and_type_alias_signatures() {
    let db = TestDb::new();
    let file = db.file(
        "/main.bolt",
        "class Show[T] where Eq[T] { type Out: Show; fn show(x: T) -> String; }\nimpl[T] Show[Vec[T]] where Show[T] {}\nconst N: usize = 1;\ntype Map[K] = HashMap[K, i32];\n",
    );
    let ModItem::Class(idx) = top(&db, file, 0) else { panic!() };
    let class = ClassId::new(&db, file, idx);
    let sig = class_signature(&db, class);
    assert_eq!(sig.name.as_ref().unwrap().as_str(), "Show");
    assert_eq!(sig.generic_params.type_params[0].name.as_str(), "T");
    assert_eq!(sig.generic_params.where_predicates.len(), 1);

    let AssocItem::TypeAlias(idx) = class.item(&db).items[0] else { panic!() };
    let out = type_alias_signature(&db, TypeAliasId::new(&db, ItemContainerId::Class(class), idx));
    assert_eq!(type_name(&out.store, out.bounds[0]), "Show");
    assert!(out.type_ref.is_none());

    let AssocItem::Function(idx) = class.item(&db).items[1] else { panic!() };
    let show = function_signature(&db, FunctionId::new(&db, ItemContainerId::Class(class), idx));
    assert_eq!(show.params.len(), 1);
    assert_eq!(type_name(&show.store, show.ret_type.unwrap()), "String");

    let ModItem::Impl(idx) = top(&db, file, 1) else { panic!() };
    let imp = impl_signature(&db, ImplId::new(&db, file, idx));
    let TypeRef::Path(path) = &imp.store[imp.class] else { panic!() };
    assert_eq!(path.segments[0].name.as_str(), "Show");
    assert!(path.segments[0].generic_args.is_some());
    assert_eq!(imp.generic_params.type_params.len(), 1);

    let ModItem::Const(idx) = top(&db, file, 2) else { panic!() };
    let n = const_signature(&db, ConstId::new(&db, ItemContainerId::File(file), idx));
    assert_eq!(type_name(&n.store, n.type_ref), "usize");

    let ModItem::TypeAlias(idx) = top(&db, file, 3) else { panic!() };
    let map = type_alias_signature(&db, TypeAliasId::new(&db, ItemContainerId::File(file), idx));
    assert_eq!(map.generic_params.type_params[0].name.as_str(), "K");
    assert!(matches!(&map.store[map.type_ref.unwrap()], TypeRef::Path(p) if p.segments[0].name.as_str() == "HashMap"));
}

/// Depends on the signature of the first function in a file.
#[salsa::tracked]
fn param_count(db: &dyn Db, file: File) -> usize {
    let ModItem::Function(idx) = item_tree(db, file).top_level[0] else { panic!() };
    function_signature(db, FunctionId::new(db, ItemContainerId::File(file), idx)).params.len()
}

#[test]
fn editing_a_body_does_not_rerun_what_depends_on_the_signature() {
    let mut db = TestDb::new();
    let file = db.file("/main.bolt", "fn f(x: i32) { x }\n");
    assert_eq!(*param_count(&db, file), 1);
    let executed = db.log_executed(|db| {
        db.edit(file, "\nfn f(x:   i32) { x + 1 }\n");
        assert_eq!(*param_count(db, file), 1);
    });
    assert!(!executed.iter().any(|q| q.contains("param_count")), "{executed:?}");
}

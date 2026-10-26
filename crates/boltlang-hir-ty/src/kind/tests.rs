use boltlang_base_db::TestDb;
use boltlang_hir_def::{
    ids::{ClassId, EnumId, FunctionId, ItemContainerId, StructId},
    item_tree::{ModItem, item_tree},
    resolver::GenericDefId,
};

use super::{Kind, generic_param_kinds};

fn star() -> Kind {
    Kind::Type
}

fn arrow(a: Kind, b: Kind) -> Kind {
    Kind::Arrow(Box::new(a), Box::new(b))
}

/// The kinds of the parameters of each top-level item of `code`, by name.
fn kinds(code: &str) -> Vec<(String, Vec<Kind>)> {
    let db = TestDb::new();
    let file = db.file("/main.bolt", code);
    let tree = item_tree(&db, file);
    let mut out = Vec::new();
    for item in &tree.top_level {
        let (name, def) = match *item {
            ModItem::Struct(idx) => {
                let id = StructId::new(&db, file, idx);
                (id.item(&db).name.clone(), GenericDefId::Struct(id))
            }
            ModItem::Enum(idx) => {
                let id = EnumId::new(&db, file, idx);
                (id.item(&db).name.clone(), GenericDefId::Enum(id))
            }
            ModItem::Class(idx) => {
                let id = ClassId::new(&db, file, idx);
                (id.item(&db).name.clone(), GenericDefId::Class(id))
            }
            ModItem::Function(idx) => {
                let id = FunctionId::new(&db, ItemContainerId::File(file), idx);
                let name = match &id.item(&db).name {
                    Some(boltlang_hir_def::item_tree::FunctionName::Name(n)) => Some(n.clone()),
                    _ => None,
                };
                (name, GenericDefId::Function(id))
            }
            _ => continue,
        };
        out.push((name.unwrap().to_string(), generic_param_kinds(&db, def).into_vec()));
    }
    out
}

#[test]
fn parameters_are_types_by_default() {
    assert_eq!(kinds("struct Pair[A, B] { a: A, b: B }"), vec![("Pair".into(), vec![star(), star()])]);
    assert_eq!(kinds("fn id[T](x: T) -> T = x;"), vec![("id".into(), vec![star()])]);
    // Unused parameters too.
    assert_eq!(kinds("enum Phantom[T] { P }"), vec![("Phantom".into(), vec![star()])]);
}

#[test]
fn applied_parameters_are_constructors() {
    let code = "class Functor[F] { fn fmap[A, B](f: fn (A) -> B, x: F[A]) -> F[B]; }";
    assert_eq!(kinds(code), vec![("Functor".into(), vec![arrow(star(), star())])]);

    let code = "struct Wrap[F, A] { value: F[A] }";
    assert_eq!(kinds(code), vec![("Wrap".into(), vec![arrow(star(), star()), star()])]);

    let code = "struct Fix[F] { unfix: F[Fix[F]] }";
    assert_eq!(kinds(code), vec![("Fix".into(), vec![arrow(star(), star())])]);
}

#[test]
fn kinds_come_from_other_definitions() {
    // `F` is passed to `Functor` and to `Wrap`, which take constructors.
    let code = "
        class Functor[F] { fn fmap[A, B](f: fn (A) -> B, x: F[A]) -> F[B]; }
        struct Wrap[F, A] { value: F[A] }
        fn f[F](x: Wrap[F, i32]) -> i32 = 1;
        fn g[G](x: i32) -> i32 where Functor[G] = x;
        struct Apply[T] { inner: T[i32, Bool] }
    ";
    let k = kinds(code);
    assert_eq!(k[2], ("f".into(), vec![arrow(star(), star())]));
    assert_eq!(k[3], ("g".into(), vec![arrow(star(), star())]));
    assert_eq!(k[4], ("Apply".into(), vec![arrow(star(), arrow(star(), star()))]));
}

#[test]
fn higher_order_kinds() {
    // `T` takes a constructor: `(Type -> Type) -> Type`.
    let code = "
        struct Box[A] { a: A }
        struct HasBox[T] { t: T[Box] }
    ";
    assert_eq!(kinds(code)[1], ("HasBox".into(), vec![arrow(arrow(star(), star()), star())]));
}

#[test]
fn display() {
    assert_eq!(arrow(star(), star()).to_string(), "Type -> Type");
    assert_eq!(arrow(arrow(star(), star()), star()).to_string(), "(Type -> Type) -> Type");
    assert_eq!(arrow(star(), arrow(star(), star())).to_string(), "Type -> Type -> Type");
}

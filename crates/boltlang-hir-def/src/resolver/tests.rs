use super::*;
use crate::{
    hir::{Expr, TypeRef},
    item_tree::{AssocItem, ModItem, item_tree},
    test_db::TestDb,
};

/// The definition a resolution names, as its kind and name.
fn describe_type(db: &dyn Db, ty: TypeNs<'_>) -> String {
    match ty {
        TypeNs::Struct(id) => format!("struct {}", id.item(db).name.as_ref().unwrap()),
        TypeNs::Enum(id) => format!("enum {}", id.item(db).name.as_ref().unwrap()),
        TypeNs::Variant(id) => format!("variant {}", id.name(db)),
        TypeNs::Class(id) => format!("class {}", id.item(db).name.as_ref().unwrap()),
        TypeNs::TypeAlias(id) => format!("type {}", id.item(db).name.as_ref().unwrap()),
        TypeNs::GenericParam { def, index } => {
            format!("param {}", def.generic_params(db).type_params[index as usize].name)
        }
        TypeNs::Builtin(ty) => format!("builtin {ty}"),
    }
}

fn describe_value(db: &dyn Db, value: ValueResolution<'_>) -> String {
    match value {
        ValueResolution::Value(ValueNs::Local(_)) => "local".into(),
        ValueResolution::Value(ValueNs::Function(id)) => match id.item(db).name.as_ref().unwrap() {
            crate::item_tree::FunctionName::Name(n) | crate::item_tree::FunctionName::Operator(n) => format!("fn {n}"),
        },
        ValueResolution::Value(ValueNs::Overloaded(set)) => format!("{} overloads", set.functions(db).len()),
        ValueResolution::Value(ValueNs::Const(id)) => format!("const {}", id.item(db).name.as_ref().unwrap()),
        ValueResolution::Value(ValueNs::Variant(id)) => format!("variant {}", id.name(db)),
        ValueResolution::Value(ValueNs::Builtin(value)) => format!("builtin {value}"),
        ValueResolution::Value(ValueNs::ConstParam { def, index }) => {
            format!("const param {}", def.generic_params(db).const_params[index as usize].name)
        }
        ValueResolution::Type(TypeResolution { ty, unresolved }) => match unresolved {
            None => describe_type(db, ty),
            Some(i) => format!("{} + {i}", describe_type(db, ty)),
        },
    }
}

fn path(text: &str) -> Path {
    Path {
        segments: text
            .split('.')
            .map(|s| PathSegment { name: Name::new(s), generic_args: None })
            .collect(),
    }
}

const LIB: &str = "pub struct Point { x: i32 }\nstruct Hidden;\npub fn area() {}\npub enum Shape { Circle, Square }\n";

const MAIN: &str = "\
import \"./lib\" as lib;
import \"./lib\" (Shape);
enum Option[T] { None, Some(T) }
struct i32;
const N: u8 = 1;
class Show[T] { fn show[U](x: T, y: U); }
fn f[T, const M: usize](a: T) { let b = a; b }
";

fn setup() -> (TestDb, File) {
    let db = TestDb::new();
    db.file("/lib.bolt", LIB);
    let main = db.file("/main.bolt", MAIN);
    (db, main)
}

#[test]
fn types_at_the_top_level() {
    let (db, main) = setup();
    let r = Resolver::for_file(main);
    let ty = |p: &str| r.resolve_path_in_type_ns(&db, &path(p)).map(|t| match t.unresolved {
        None => describe_type(&db, t.ty),
        Some(i) => format!("{} + {i}", describe_type(&db, t.ty)),
    });
    assert_eq!(ty("Option").as_deref(), Some("enum Option"));
    assert_eq!(ty("Option.Some").as_deref(), Some("variant Some"));
    assert_eq!(ty("Option.new").as_deref(), Some("enum Option + 1"));
    assert_eq!(ty("lib.Point").as_deref(), Some("struct Point"));
    assert_eq!(ty("lib.Shape.Circle").as_deref(), Some("variant Circle"));
    assert_eq!(ty("Shape").as_deref(), Some("enum Shape"));
    // Private items of another file aren't visible.
    assert_eq!(ty("lib.Hidden"), None);
    // A definition shadows a builtin type.
    assert_eq!(ty("i32").as_deref(), Some("struct i32"));
    assert_eq!(ty("u8").as_deref(), Some("builtin u8"));
    assert_eq!(ty("u8.MAX").as_deref(), Some("builtin u8 + 1"));
    assert_eq!(ty("lib.u8"), None);
    assert_eq!(ty("T"), None);
    assert_eq!(ty("Missing"), None);
}

#[test]
fn values_at_the_top_level() {
    let (db, main) = setup();
    let r = Resolver::for_file(main);
    let value = |p: &str| r.resolve_path_in_value_ns(&db, &path(p)).map(|v| describe_value(&db, v));
    assert_eq!(value("N").as_deref(), Some("const N"));
    assert_eq!(value("lib.area").as_deref(), Some("fn area"));
    assert_eq!(value("Option.None").as_deref(), Some("variant None"));
    assert_eq!(value("lib.Shape.Square").as_deref(), Some("variant Square"));
    assert_eq!(value("Option.new").as_deref(), Some("enum Option + 1"));
    assert_eq!(value("Option").as_deref(), Some("enum Option"));
    assert_eq!(value("lib.Point").as_deref(), Some("struct Point"));
    assert_eq!(value("f").as_deref(), Some("fn f"));
    assert_eq!(value("a"), None);
}

#[test]
fn generic_params_and_locals() {
    let (db, main) = setup();
    let tree = item_tree(&db, main);

    // In a method of a class, the class's and the method's parameters.
    let ModItem::Class(idx) = tree.top_level[5] else { panic!() };
    let class = ClassId::new(&db, main, idx);
    let AssocItem::Function(idx) = class.item(&db).items[0] else { panic!() };
    let show = FunctionId::new(&db, ItemContainerId::Class(class), idx);
    let r = Resolver::for_generic_def(&db, GenericDefId::Function(show));
    let ty = |p: &str| r.resolve_path_in_type_ns(&db, &path(p)).map(|t| describe_type(&db, t.ty));
    assert_eq!(ty("T").as_deref(), Some("param T"));
    assert_eq!(ty("U").as_deref(), Some("param U"));

    // In a body, the parameters and the locals visible at each expression.
    let ModItem::Function(idx) = tree.top_level[6] else { panic!() };
    let f = FunctionId::new(&db, ItemContainerId::File(main), idx);
    let def = DefWithBodyId::new(&db, DefWithBody::Function(f));
    let body = crate::body::body(&db, def);
    let Expr::Block { tail: Some(tail), .. } = body[body.body_expr] else { panic!() };
    let r = Resolver::for_expr(&db, def, tail);
    let value = |p: &str| r.resolve_path_in_value_ns(&db, &path(p)).map(|v| describe_value(&db, v));
    assert_eq!(value("a").as_deref(), Some("local"));
    assert_eq!(value("b").as_deref(), Some("local"));
    assert_eq!(value("M").as_deref(), Some("const param M"));
    assert_eq!(r.resolve_path_in_type_ns(&db, &path("T")).map(|t| describe_type(&db, t.ty)).as_deref(), Some("param T"));
    // `b` isn't in scope at the start of the body.
    let r = Resolver::for_body(&db, def);
    assert!(r.resolve_local_binding(&Name::new("b")).is_none());
    assert!(r.resolve_local_binding(&Name::new("a")).is_some());

    // The type of `a`, resolved from the signature.
    let sig = function_signature(&db, f);
    let TypeRef::Path(p) = &sig.store[sig.params[0].type_ref.unwrap()] else { panic!() };
    let r = Resolver::for_generic_def(&db, GenericDefId::Function(f));
    assert_eq!(
        r.resolve_path_in_type_ns(&db, p).map(|t| t.ty),
        Some(TypeNs::GenericParam { def: GenericDefId::Function(f), index: 0 })
    );
}

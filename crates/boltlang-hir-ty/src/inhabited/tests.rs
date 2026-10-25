use boltlang_base_db::TestDb;
use boltlang_hir_def::{
    ids::{EnumId, StructId},
    item_tree::{ModItem, item_tree},
    resolver::GenericDefId,
};

use super::has_values;

/// Whether each struct and enum of `code` has values, by name.
fn values(code: &str) -> Vec<(String, bool)> {
    let db = TestDb::new();
    let file = db.file("/main.bolt", code);
    item_tree(&db, file)
        .top_level
        .iter()
        .filter_map(|item| {
            let (name, def) = match *item {
                ModItem::Struct(idx) => {
                    let id = StructId::new(&db, file, idx);
                    (id.item(&db).name.clone(), GenericDefId::Struct(id))
                }
                ModItem::Enum(idx) => {
                    let id = EnumId::new(&db, file, idx);
                    (id.item(&db).name.clone(), GenericDefId::Enum(id))
                }
                _ => return None,
            };
            Some((name.unwrap().to_string(), has_values(&db, def)))
        })
        .collect()
}

fn named(pairs: &[(&str, bool)]) -> Vec<(String, bool)> {
    pairs.iter().map(|(n, b)| (n.to_string(), *b)).collect()
}

#[test]
fn recursion_needs_a_way_out() {
    assert_eq!(values("enum List[T] { Cons(T, List[T]), Nil }"), named(&[("List", true)]));
    assert_eq!(values("enum Node[T] { Ref(Node[T]) }"), named(&[("Node", false)]));
    assert_eq!(values("struct S { s: S }"), named(&[("S", false)]));
    assert_eq!(values("struct P(P, i32);"), named(&[("P", false)]));
    // An array can be empty, so it's a way out.
    assert_eq!(values("struct Tree { children: [Tree] }"), named(&[("Tree", true)]));
}

#[test]
fn types_that_refer_to_each_other() {
    let code = "struct A { b: B }\nstruct B { a: A }\nstruct C { d: D }\nenum D { Leaf, Node(C) }";
    assert_eq!(values(code), named(&[("A", false), ("B", false), ("C", true), ("D", true)]));
}

#[test]
fn types_without_values_spread() {
    let code = "enum Never {}\nstruct S { n: Never }\nstruct T { x: (i32, S) }\nstruct U { f: fn (S) -> i32 }";
    assert_eq!(values(code), named(&[("Never", false), ("S", false), ("T", false), ("U", true)]));
}

#[test]
fn type_parameters_have_values() {
    assert_eq!(values("struct Wrap[T] { t: T }\nstruct Unit;"), named(&[("Wrap", true), ("Unit", true)]));
}

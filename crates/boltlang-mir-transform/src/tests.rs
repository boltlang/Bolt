use boltlang_base_db::TestDb;
use boltlang_hir_def::{
    ids::EnumVariantId,
    item_tree::{ModItem, item_tree},
};
use boltlang_mir::Shim;
use boltlang_pattern::decision::Ctor;

use crate::shim_body;

/// A constructor takes its fields and makes an `imm` value: a field of the
/// struct's own mutability is `imm` too, and a named one stays a parameter.
#[test]
fn a_constructor_shim_makes_its_value() {
    let mut db = TestDb::new();
    db.add_std();
    let file =
        db.file("/main.bolt", "enum Pair[A] { Two(A, i32) }\nstruct Pair2[T, 'v]('v [T], [T]);\n");
    let tree = item_tree(&db, file);
    let mut out = Vec::new();
    for item in &tree.top_level {
        let ctor = match *item {
            ModItem::Enum(idx) => {
                let e = boltlang_hir_def::ids::EnumId::new(&db, file, idx);
                Ctor::Variant(EnumVariantId { parent: e, index: 0 })
            }
            ModItem::Struct(idx) => {
                Ctor::Struct(boltlang_hir_def::ids::StructId::new(&db, file, idx))
            }
            _ => continue,
        };
        out.push(shim_body(&db, &Shim::Ctor(ctor)).display_with_types(&db));
    }
    assert_eq!(
        out.concat(),
        "shim Pair.Two(_t0: A, _t1: i32) -> Pair[A] = (Pair.Two((_t0 : A), (_t1 : i32)) : Pair[A])\n\
         shim Pair2(_t0: 'v [T], _t1: imm [T]) -> imm Pair2[T, 'v] = (Pair2((_t0 : 'v [T]), (_t1 : imm [T])) : imm Pair2[T, 'v])\n"
    );
}

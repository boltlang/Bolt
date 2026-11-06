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
        out.push(shim_body(&db, &Shim::Ctor(ctor), file).display_with_types(&db));
    }
    assert_eq!(
        out.concat(),
        "shim Pair.Two(_t0: A, _t1: i32) -> Pair[A] = (Pair.Two((_t0 : A), (_t1 : i32)) : Pair[A])\n\
         shim Pair2(_t0: 'v [T], _t1: imm [T]) -> imm Pair2[T, 'v] = (Pair2((_t0 : 'v [T]), (_t1 : imm [T])) : imm Pair2[T, 'v])\n"
    );
}

/// The deep copy of a struct and an enum makes them again with `imm` of
/// each field, and that of an array with a loop. An `imm` type needs none.
#[test]
fn the_deep_copy_of_imm_e_copies_part_by_part() {
    use boltlang_hir_def::builtin_type::{BuiltinInt, BuiltinType};
    use boltlang_hir_ty::{AdtId, Ty, ty::Mutability};

    let mut db = TestDb::new();
    db.add_test_std();
    let file = db.file("/main.bolt", "struct Cell { items: [i32] }\nenum Two { A([i32]), B }\n");
    let tree = item_tree(&db, file);
    let (mut cell, mut two) = (None, None);
    for item in &tree.top_level {
        match *item {
            ModItem::Struct(idx) => {
                cell = Some(boltlang_hir_def::ids::StructId::new(&db, file, idx))
            }
            ModItem::Enum(idx) => two = Some(boltlang_hir_def::ids::EnumId::new(&db, file, idx)),
            _ => {}
        }
    }
    let int = Ty::Builtin(BuiltinType::Int(BuiltinInt::I32));
    let m = |m| Ty::Mutability(m);
    let mut_ints = Ty::array(m(Mutability::Mut), int.clone());
    let tys = [
        Ty::adt(AdtId::Struct(cell.unwrap()), [], [], m(Mutability::Mut)),
        Ty::adt(AdtId::Enum(two.unwrap()), [], [], m(Mutability::Imm)),
        mut_ints.clone(),
        Ty::array(m(Mutability::Imm), int),
    ];
    let out: Vec<String> =
        tys.iter().map(|t| shim_body(&db, &Shim::ImmDeep(t.clone()), file).display(&db)).collect();
    assert_eq!(
        out.concat(),
        "shim imm_deep[mut Cell](x: mut Cell) -> imm Cell = Cell(imm x.Cell.0)\n\
         shim imm_deep[Two](x: Two) -> Two = x\n\
         shim imm_deep[mut [i32]](x: mut [i32]) -> imm [i32] = {\n  \
           let n = builtin.array_len(x);\n  \
           let ys = builtin.array_alloc(n);\n  \
           let mut i = 0;\n  \
           'l0: while builtin.i32_lt(i, n) {\n    \
             builtin.array_set(ys, i, imm builtin.array_get(x, i));\n    \
             i = builtin.i32_add(i, 1);\n  \
           };\n  \
           builtin.array_freeze(ys)\n\
         }\n\
         shim imm_deep[imm [i32]](x: imm [i32]) -> imm [i32] = x\n"
    );
}

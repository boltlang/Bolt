//! What a struct holds, for deep `imm` (#256): the "at most" atoms that its
//! fields imply of its arguments, like `'v ⊑ m` and `T ⊑ 'v` for
//! `struct VecIter[T, 'v] { data: 'v [T], index: i32 }`, with `m` its own
//! mutability. A struct type is well-formed exactly when its arguments meet
//! them, as `StructDecl.atMost_ty_iff` in `lean/BoltMut/Structs.lean`
//! proves. It's how Rust infers `T: 'a` from a struct's fields.
//!
//! Its type arguments aren't enough to say: `struct Handler[T] { f: fn(T) }`
//! doesn't hold a `T`, and `struct Frozen[T] { items: imm [T] }` holds one
//! that must be `imm`, whatever the struct's mutability.
//!
//! A struct whose fields have another struct uses that one's atoms, with
//! its arguments in place. For structs that refer to each other, that's a
//! least fixed point: they start out implying nothing, and Salsa iterates
//! until that stops changing, as for [`crate::inhabited`]. It ends, since
//! the atoms are about the structs' parameters, of which there are few.

use boltlang_base_db::Db;
use boltlang_hir_def::{ids::StructId, resolver::GenericDefId};

use crate::{
    lower::{struct_args, struct_field_types, struct_mutability, subst_params},
    solve::{AtMostConstraint, Held},
    ty::{AdtId, Mutability, Ty},
};

/// What the fields of the struct `s` imply of its parameters: the atoms of
/// "its fields are at most its own mutability", each a mutability or a type
/// that isn't known any further, like a type parameter, at most a
/// mutability.
#[salsa::tracked(returns(ref), cycle_initial = struct_atoms_initial)]
pub fn struct_atoms<'db>(db: &'db dyn Db, s: StructId<'db>) -> Box<[AtMostConstraint<'db>]> {
    let own = struct_mutability(db, s);
    let mut out = Vec::new();
    for (_, ty) in struct_field_types(db, s).fields.iter() {
        split(db, ty, &own, &mut out);
    }
    out.into()
}

/// Structs that refer to themselves start out implying nothing.
fn struct_atoms_initial<'db>(_db: &'db dyn Db, _id: salsa::Id, _s: StructId<'db>) -> Box<[AtMostConstraint<'db>]> {
    Box::new([])
}

/// The atoms of the struct type `Adt(s, args, mutabilities, mutability)`,
/// with its arguments in place: what it holds, and how `mut` each part may
/// be. A type in one may be split further.
pub fn struct_atoms_of<'db>(
    db: &'db dyn Db,
    s: StructId<'db>,
    args: &[Ty<'db>],
    mutabilities: &[Ty<'db>],
    mutability: &Ty<'db>,
) -> Vec<AtMostConstraint<'db>> {
    let subst = struct_args(args, mutabilities, mutability);
    let def = GenericDefId::Struct(s);
    struct_atoms(db, s).iter().map(|a| a.map(|t| subst_params(t, def, &subst))).collect()
}

/// Split "`ty` is at most `bound`" into atoms, as `split` does in
/// `lean/BoltMut/Solver.lean`: an array, struct or function is at most
/// `bound`, and what an array or struct holds at most its own mutability;
/// an enum or tuple is a value, which holds its parts; a type parameter, an
/// associated type or an unknown stays as it is. Atoms that hold by the
/// order alone, like `imm ⊑ m`, are left out.
pub fn split<'db>(db: &'db dyn Db, ty: &Ty<'db>, bound: &Ty<'db>, out: &mut Vec<AtMostConstraint<'db>>) {
    let push = |out: &mut Vec<AtMostConstraint<'db>>, held: Held<'db>, bound: &Ty<'db>| {
        let trivial = match &held {
            Held::Mutability(m) => {
                m == bound
                    || matches!(m, Ty::Mutability(Mutability::Imm) | Ty::Error | Ty::Never)
                    || matches!(bound, Ty::Mutability(Mutability::Mut))
            }
            Held::Type(_) => matches!(bound, Ty::Mutability(Mutability::Mut)),
        };
        let atom = AtMostConstraint { held, bound: bound.clone() };
        if !trivial && !out.contains(&atom) {
            out.push(atom);
        }
    };
    match ty {
        Ty::Array(m, elem) => {
            push(out, Held::Mutability((**m).clone()), bound);
            split(db, elem, m, out);
        }
        Ty::Adt(AdtId::Struct(s), args, mutabilities, m) => {
            push(out, Held::Mutability((**m).clone()), bound);
            for atom in struct_atoms_of(db, *s, args, mutabilities, m) {
                match atom.held {
                    Held::Type(t) => split(db, &t, &atom.bound, out),
                    held => push(out, held, &atom.bound),
                }
            }
        }
        Ty::Adt(AdtId::Enum(_), args, ..) | Ty::Tuple(args) => {
            for arg in args.iter() {
                split(db, arg, bound, out);
            }
        }
        Ty::Fn(_, _, m) => push(out, Held::Mutability((**m).clone()), bound),
        Ty::Builtin(_)
        | Ty::Mutability(_)
        | Ty::Unwritten
        | Ty::Number(_)
        | Ty::Never
        | Ty::Error => {}
        Ty::Param(_) | Ty::Var(_) | Ty::Bound(_) | Ty::App(..) | Ty::Assoc(..) => push(out, Held::Type(ty.clone()), bound),
    }
}

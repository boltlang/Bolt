//! Whether a struct or enum has values. A type has values when one of its
//! variants can be built from values that already exist, so
//! `enum List[T] { Cons(T, List[T]), Nil }` has values, starting with `Nil`,
//! but `enum Node[T] { Ref(Node[T]) }` has none: building a `Node` needs a
//! `Node` first. Bolt evaluates strictly, so there's no way around that, as
//! there is with Haskell's laziness.
//!
//! This is a least fixed point: types start out without values, and get
//! them once one of their variants has fields of types with values. Salsa
//! iterates the query until that stops changing, which also covers types
//! that refer to each other, in the same file or not.
//!
//! Type parameters count as having values, so `Wrap[T]` has values even
//! though `Wrap[!]` doesn't: the check is about definitions, not uses.

use boltlang_base_db::Db;
use boltlang_hir_def::{Name, resolver::GenericDefId};

use crate::{
    lower::{enum_field_types, struct_field_types},
    ty::{AdtId, Ty},
};

/// Whether the struct or enum `def` has values. Other definitions do.
#[salsa::tracked(returns(copy), cycle_initial = has_values_initial)]
pub fn has_values<'db>(db: &'db dyn Db, def: GenericDefId<'db>) -> bool {
    match def {
        GenericDefId::Struct(id) => fields_have_values(db, &struct_field_types(db, id).fields),
        GenericDefId::Enum(id) => enum_field_types(db, id).iter().any(|v| fields_have_values(db, &v.fields)),
        _ => true,
    }
}

/// Types that refer to themselves start out without values.
fn has_values_initial<'db>(_db: &'db dyn Db, _id: salsa::Id, _def: GenericDefId<'db>) -> bool {
    false
}

fn fields_have_values<'db>(db: &'db dyn Db, fields: &[(Name, Ty<'db>)]) -> bool {
    fields.iter().all(|(_, ty)| ty_has_values(db, ty))
}

/// Whether there are values of `ty`. Arrays always have one, the empty one,
/// and so do functions; `!` has none.
pub fn ty_has_values<'db>(db: &'db dyn Db, ty: &Ty<'db>) -> bool {
    match ty {
        Ty::Never => false,
        Ty::Tuple(tys) => tys.iter().all(|t| ty_has_values(db, t)),
        Ty::Adt(AdtId::Struct(id), ..) => has_values(db, GenericDefId::Struct(*id)),
        Ty::Adt(AdtId::Enum(id), ..) => has_values(db, GenericDefId::Enum(*id)),
        _ => true,
    }
}

/// The first field without values among `fields`: why a struct or variant
/// can't be built.
pub fn field_without_values<'a, 'db>(db: &'db dyn Db, fields: &'a [(Name, Ty<'db>)]) -> Option<&'a (Name, Ty<'db>)> {
    fields.iter().find(|(_, ty)| !ty_has_values(db, ty))
}

#[cfg(test)]
mod tests;

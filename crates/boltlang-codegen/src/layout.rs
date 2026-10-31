//! How the values of each type are kept in Wasm.
//!
//! - `()` and `!` have no value.
//! - The scalar types are one Wasm value: see [`Scalar`].
//! - An `imm struct` with one field, like `imm struct Char(u32)`, is the
//!   value of its field, so it costs nothing.
//! - Other structs and tuples are the `i32` address of their fields in
//!   linear memory, which [`Callee::Alloc`](crate::Callee::Alloc) gives.
//!   Each field takes the size of its Wasm value, 4 or 8 bytes, at an
//!   offset aligned to that size, in the order they're declared. A field
//!   without a value takes no room.
//!
//! Structs are shared, so a change to a field shows through every name for
//! the struct. Tuples are values: the code generator never changes one in
//! place, but makes a changed copy, so sharing them is safe.

use boltlang_base_db::Db;
use boltlang_hir_def::{Name, resolver::GenericDefId};
use boltlang_hir_ty::{
    AdtId, Ty,
    lang::string_struct,
    lower::{VariantDef, field_types, struct_args, subst_params},
};

use crate::{ir::ValTy, lower::Scalar};

/// How the values of a type are kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Repr<'db> {
    /// `()` and `!`, which have no value.
    Unit,
    Scalar(Scalar),
    /// An `imm struct` with one field, of this type: the field's value.
    Newtype(Ty<'db>),
    /// A struct or a tuple: the address of its fields.
    Boxed,
}

/// The fields of a struct or tuple in memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Layout<'db> {
    pub slots: Vec<Slot<'db>>,
    /// The bytes it takes, a multiple of 8.
    pub size: u32,
}

/// A field of a struct or tuple: `0`, `1`, ... for those of tuples.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Slot<'db> {
    pub name: Name,
    pub ty: Ty<'db>,
    pub offset: u32,
    /// `None` for a field without a value, like one of type `()`.
    pub val_ty: Option<ValTy>,
}

impl<'db> Layout<'db> {
    pub fn slot(&self, name: &Name) -> Option<&Slot<'db>> {
        self.slots.iter().find(|s| s.name == *name)
    }
}

/// How values of `ty` are kept, or `None` for a type the code generator
/// doesn't support.
pub(crate) fn repr<'db>(db: &'db dyn Db, ty: &Ty<'db>) -> Option<Repr<'db>> {
    match ty {
        Ty::Never => Some(Repr::Unit),
        Ty::Tuple(tys) if tys.is_empty() => Some(Repr::Unit),
        Ty::Tuple(_) => Some(Repr::Boxed),
        Ty::Adt(AdtId::Struct(id), ..) => {
            // Compiled code doesn't have strings yet: a literal is the
            // address of its bytes, which isn't the prelude's `String`.
            if string_struct(db, id.file(db)) == Some(*id) {
                return None;
            }
            let fields = fields(db, ty)?;
            match &fields[..] {
                [(_, field)] if id.item(db).is_imm => Some(Repr::Newtype(field.clone())),
                _ => Some(Repr::Boxed),
            }
        }
        _ => Scalar::of(db, ty).map(Repr::Scalar),
    }
}

/// The Wasm type of values of `ty`: `Some(None)` for a type without values,
/// and `None` for one the code generator doesn't support.
pub(crate) fn val_ty<'db>(db: &'db dyn Db, ty: &Ty<'db>) -> Option<Option<ValTy>> {
    let mut ty = ty.clone();
    // An `imm struct A(A)` has no values, but mustn't hang.
    for _ in 0..64 {
        match repr(db, &ty)? {
            Repr::Unit => return Some(None),
            Repr::Scalar(scalar) => return Some(Some(scalar.val_ty())),
            Repr::Boxed => return Some(Some(ValTy::I32)),
            Repr::Newtype(field) => ty = field,
        }
    }
    None
}

/// The names and types of the fields of a struct or tuple, with its type
/// arguments in them.
pub(crate) fn fields<'db>(db: &'db dyn Db, ty: &Ty<'db>) -> Option<Vec<(Name, Ty<'db>)>> {
    match ty {
        Ty::Tuple(tys) => Some(tys.iter().enumerate().map(|(i, t)| (Name::new(i.to_string()), t.clone())).collect()),
        Ty::Adt(AdtId::Struct(id), args, mutabilities, own) => {
            let args = struct_args(args, mutabilities, own);
            let def = GenericDefId::Struct(*id);
            let fields = &field_types(db, VariantDef::Struct(*id)).fields;
            Some(fields.iter().map(|(name, t)| (name.clone(), subst_params(t, def, &args))).collect())
        }
        _ => None,
    }
}

/// Where the fields of a struct or tuple of type `ty` are, or `None` when
/// it isn't one, or a field has a type the code generator doesn't support.
pub(crate) fn layout<'db>(db: &'db dyn Db, ty: &Ty<'db>) -> Option<Layout<'db>> {
    let mut slots = Vec::new();
    let mut end: u32 = 0;
    for (name, ty) in fields(db, ty)? {
        let val_ty = val_ty(db, &ty)?;
        let size = val_ty.map_or(0, |t| t.size());
        let offset = if size == 0 { end } else { end.next_multiple_of(size) };
        end = offset + size;
        slots.push(Slot { name, ty, offset, val_ty });
    }
    Some(Layout { slots, size: end.next_multiple_of(8) })
}

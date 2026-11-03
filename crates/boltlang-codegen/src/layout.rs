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
//! - An array is the `i32` address of its length, a `u32`, followed by its
//!   elements, from 4 bytes on, or 8 when they're 8 bytes each. Each element
//!   takes the bytes of its type: 1 for `u8`, `i8` and `Bool`, 2 for `u16`
//!   and `i16`, and the size of its Wasm value for the others. So a `[u8]`
//!   is laid out like a string literal: its length, then its bytes.
//!
//! - An enum without fields is the index of its variant, an `i32`. One
//!   with fields is the `i32` address of the index of its variant, an
//!   `i32`, followed by the variant's fields, laid out like a struct's from
//!   4 bytes on. A variant without fields takes just the index.
//!
//! Structs and arrays are shared, so a change to a field or an element
//! shows through every name for it. Tuples are values: the code generator never changes one in
//! place, but makes a changed copy, so sharing them is safe.

use boltlang_base_db::Db;
use boltlang_hir_def::{Name, ids::EnumVariantId, resolver::GenericDefId};
use boltlang_hir_ty::{
    AdtId, Ty,
    lower::{VariantDef, field_types, struct_args, subst_params},
};

use crate::{
    ir::{Narrow, ValTy},
    lower::Scalar,
};

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
    /// An array of elements of this type: the address of its length.
    Array(Ty<'db>),
    /// An enum with fields: the address of its variant's index and fields.
    /// The fields' types aren't checked here, since an enum can hold
    /// itself, like a list.
    Enum,
}

/// How the elements of an array are kept: see the module's docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Element {
    /// Their Wasm value, `None` for a type without values, like `()`.
    pub val_ty: Option<ValTy>,
    /// For an `i32` kept in fewer bytes, how.
    pub narrow: Option<Narrow>,
    /// The bytes each takes.
    pub size: u32,
    /// Where the first one is, after the length.
    pub start: u32,
}

/// How elements of type `ty` are kept in an array, or `None` for a type the
/// code generator doesn't support.
pub(crate) fn element<'db>(db: &'db dyn Db, ty: &Ty<'db>) -> Option<Element> {
    let val_ty = val_ty(db, ty)?;
    let narrow = match Scalar::through(db, ty) {
        Some(Scalar::Bool) => Some(Narrow::U8),
        Some(Scalar::Int { bits: 8, signed }) => Some(if signed { Narrow::S8 } else { Narrow::U8 }),
        Some(Scalar::Int { bits: 16, signed }) => {
            Some(if signed { Narrow::S16 } else { Narrow::U16 })
        }
        _ => None,
    };
    let size = match (narrow, val_ty) {
        (Some(narrow), _) => narrow.size(),
        (None, Some(val_ty)) => val_ty.size(),
        (None, None) => 0,
    };
    Some(Element { val_ty, narrow, size, start: if size == 8 { 8 } else { 4 } })
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
        // The prelude's `String`, an `imm struct` of its `[u8]`, is that
        // array, laid out like a literal: its length, then its bytes.
        Ty::Adt(AdtId::Struct(id), ..) => {
            let fields = fields(db, ty)?;
            match &fields[..] {
                [(_, field)] if id.item(db).is_imm => Some(Repr::Newtype(field.clone())),
                _ => Some(Repr::Boxed),
            }
        }
        Ty::Array(_, elem) => {
            element(db, elem)?;
            Some(Repr::Array((**elem).clone()))
        }
        Ty::Adt(AdtId::Enum(_), ..) => Some(Scalar::of(db, ty).map_or(Repr::Enum, Repr::Scalar)),
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
            Repr::Boxed | Repr::Array(_) | Repr::Enum => return Some(Some(ValTy::I32)),
            Repr::Newtype(field) => ty = field,
        }
    }
    None
}

/// The names and types of the fields of a struct or tuple, with its type
/// arguments in them.
pub(crate) fn fields<'db>(db: &'db dyn Db, ty: &Ty<'db>) -> Option<Vec<(Name, Ty<'db>)>> {
    match ty {
        Ty::Tuple(tys) => Some(
            tys.iter().enumerate().map(|(i, t)| (Name::new(i.to_string()), t.clone())).collect(),
        ),
        Ty::Adt(AdtId::Struct(id), args, mutabilities, own) => {
            let args = struct_args(args, mutabilities, own);
            let def = GenericDefId::Struct(*id);
            let fields = &field_types(db, VariantDef::Struct(*id)).fields;
            Some(
                fields
                    .iter()
                    .map(|(name, t)| (name.clone(), subst_params(t, def, &args)))
                    .collect(),
            )
        }
        _ => None,
    }
}

/// Where the fields of a struct or tuple of type `ty` are, or `None` when
/// it isn't one, or a field has a type the code generator doesn't support.
pub(crate) fn layout<'db>(db: &'db dyn Db, ty: &Ty<'db>) -> Option<Layout<'db>> {
    layout_from(db, fields(db, ty)?, 0)
}

/// The names and types of the fields of the variant `v` of the enum of
/// type `ty`, with its type arguments in them.
pub(crate) fn variant_fields<'db>(
    db: &'db dyn Db,
    ty: &Ty<'db>,
    v: EnumVariantId<'db>,
) -> Option<Vec<(Name, Ty<'db>)>> {
    let Ty::Adt(AdtId::Enum(id), args, ..) = ty else { return None };
    if *id != v.parent {
        return None;
    }
    let def = GenericDefId::Enum(*id);
    let fields = &field_types(db, VariantDef::Variant(v)).fields;
    Some(fields.iter().map(|(name, t)| (name.clone(), subst_params(t, def, args))).collect())
}

/// Where the fields of the variant `v` of an enum with fields of type `ty`
/// are, after its index.
pub(crate) fn variant_layout<'db>(
    db: &'db dyn Db,
    ty: &Ty<'db>,
    v: EnumVariantId<'db>,
) -> Option<Layout<'db>> {
    layout_from(db, variant_fields(db, ty, v)?, 4)
}

/// The layout of `fields`, from the byte `start` on.
fn layout_from<'db>(
    db: &'db dyn Db,
    fields: Vec<(Name, Ty<'db>)>,
    start: u32,
) -> Option<Layout<'db>> {
    let mut slots = Vec::new();
    let mut end: u32 = start;
    for (name, ty) in fields {
        let val_ty = val_ty(db, &ty)?;
        let size = val_ty.map_or(0, |t| t.size());
        let offset = if size == 0 { end } else { end.next_multiple_of(size) };
        end = offset + size;
        slots.push(Slot { name, ty, offset, val_ty });
    }
    Some(Layout { slots, size: end.next_multiple_of(8) })
}

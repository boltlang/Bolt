//! The classes of the prelude that syntax uses: `v[i]` calls `index` of
//! `Index`, `v[i] = x` calls `set_index` of `IndexMut`, and `for x in c`
//! calls `iter` of `Iterable` and then `next` of `Iterator`, for arrays too:
//! the prelude's impls for arrays use the builtins `array_get` and
//! `array_set`. Without a prelude that defines the classes, the syntax is an
//! error.
//!
//! The types of literals are the prelude's too: `"hi"` is a `String` and
//! `'a'` a `Char`. And the evaluator shows a value of a type with an impl
//! of `Debug` with it.

use boltlang_base_db::{Db, File};
use boltlang_hir_def::{
    Name,
    hir::Mutability,
    ids::{ClassId, EnumId, FunctionId, ItemContainerId, ModuleDefId, StructId, TypeAliasId},
    item_tree::AssocItem,
    nameres::{Namespace, file_scope, prelude_lib},
};

use crate::{AdtId, Ty, lower::assoc_type};

/// The struct called `name` in the prelude, as `file` sees it. Files of
/// the prelude's own package find it too.
fn prelude_struct<'db>(db: &'db dyn Db, file: File, name: &str) -> Option<StructId<'db>> {
    let lib = prelude_lib(db, file)?;
    match file_scope(db, lib).scope.get(Namespace::Types, &Name::new(name))?.def {
        ModuleDefId::Struct(id) => Some(id),
        _ => None,
    }
}

fn prelude_enum<'db>(db: &'db dyn Db, file: File, name: &str) -> Option<EnumId<'db>> {
    let lib = prelude_lib(db, file)?;
    match file_scope(db, lib).scope.get(Namespace::Types, &Name::new(name))?.def {
        ModuleDefId::Enum(id) => Some(id),
        _ => None,
    }
}

/// The enum `Option` of the prelude, as `file` sees it, which `e?` takes.
pub fn option_enum<'db>(db: &'db dyn Db, file: File) -> Option<EnumId<'db>> {
    prelude_enum(db, file, "Option")
}

/// The enum `Result` of the prelude, as `file` sees it, which `e?` takes.
pub fn result_enum<'db>(db: &'db dyn Db, file: File) -> Option<EnumId<'db>> {
    prelude_enum(db, file, "Result")
}

/// The struct `id` as a type, if it's an `imm struct` without type
/// parameters, like `String` and `Char`.
fn imm_struct_ty<'db>(id: StructId<'db>) -> Ty<'db> {
    Ty::adt(AdtId::Struct(id), [], [], Ty::Mutability(Mutability::Imm))
}

/// The type of strings: the struct `String` of the prelude, as `file` sees
/// it, like `"hi"` has. Files of the prelude's own package find it too.
pub fn string_struct<'db>(db: &'db dyn Db, file: File) -> Option<StructId<'db>> {
    prelude_struct(db, file, "String")
}

/// The type of continuations: the struct `Cont[B, U, R]` of the prelude, as
/// `file` sees it, which the `k` of an effect arm like `effect get(), k`
/// has. `B` is what the operation gives, `U` what the handler's `match`
/// gives, and `R` the effects left after it.
pub fn cont_struct<'db>(db: &'db dyn Db, file: File) -> Option<StructId<'db>> {
    prelude_struct(db, file, "Cont")
}

/// The struct `Range[T]` of the prelude, as `file` sees it, which `a..b`
/// makes.
pub fn range_struct<'db>(db: &'db dyn Db, file: File) -> Option<StructId<'db>> {
    prelude_struct(db, file, "Range")
}

/// The struct `RangeInclusive[T]` of the prelude, as `file` sees it, which
/// `a..=b` makes.
pub fn range_inclusive_struct<'db>(db: &'db dyn Db, file: File) -> Option<StructId<'db>> {
    prelude_struct(db, file, "RangeInclusive")
}

/// [`string_struct`] as a type: an `imm String`.
pub fn string_ty<'db>(db: &'db dyn Db, file: File) -> Option<Ty<'db>> {
    string_struct(db, file).map(imm_struct_ty)
}

/// The type of characters: the struct `Char` of the prelude, as `file` sees
/// it, like `'a'` has. It's an `imm struct` with the code point as its one
/// field, a `u32`.
pub fn char_struct<'db>(db: &'db dyn Db, file: File) -> Option<StructId<'db>> {
    prelude_struct(db, file, "Char")
}

/// [`char_struct`] as a type: an `imm Char`.
pub fn char_ty<'db>(db: &'db dyn Db, file: File) -> Option<Ty<'db>> {
    char_struct(db, file).map(imm_struct_ty)
}

/// The function `repr` of the prelude, as `file` sees it: how a
/// value of a type with an impl of `Debug` looks, as a `String`.
pub fn repr_fn<'db>(db: &'db dyn Db, file: File) -> Option<FunctionId<'db>> {
    let lib = prelude_lib(db, file)?;
    match file_scope(db, lib).scope.get(Namespace::Values, &Name::new("repr"))?.def {
        ModuleDefId::Function(id) => Some(id),
        _ => None,
    }
}

/// The function `panic` of the prelude, as `file` sees it, which compiled
/// code calls when no arm of a `match` matches.
pub fn panic_fn<'db>(db: &'db dyn Db, file: File) -> Option<FunctionId<'db>> {
    let lib = prelude_lib(db, file)?;
    match file_scope(db, lib).scope.get(Namespace::Values, &Name::new("panic"))?.def {
        ModuleDefId::Function(id) => Some(id),
        _ => None,
    }
}

/// A class of the prelude that syntax uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LangClass {
    /// `class Index[C] { type Item; fn index(c: C, i: i32) -> Item; }`
    Index,
    /// `class IndexMut[C] where Index[C] { fn set_index(c: C, i: i32, x: C.Item); }`
    IndexMut,
    /// `class Iterable[C] { type Iter: Iterator; fn iter(c: C) -> Iter; }`
    Iterable,
    /// `class Iterator[I] { type Item; fn next(it: I) -> Option[Item]; }`
    Iterator,
    /// `class Debug[T] { fn debug(x: T, out: mut StringBuilder); }`, which
    /// the evaluator shows values with.
    Debug,
    /// `class ToImmDeep[T] { fn to_imm_deep(x: T) -> imm T; }`, which the
    /// deep copy of `imm e` calls for a type that has an impl of it.
    ToImmDeep,
}

impl LangClass {
    pub fn name(self) -> &'static str {
        match self {
            LangClass::Index => "Index",
            LangClass::IndexMut => "IndexMut",
            LangClass::Iterable => "Iterable",
            LangClass::Iterator => "Iterator",
            LangClass::Debug => "Debug",
            LangClass::ToImmDeep => "ToImmDeep",
        }
    }

    /// The method that the syntax calls.
    pub fn method_name(self) -> &'static str {
        match self {
            LangClass::Index => "index",
            LangClass::IndexMut => "set_index",
            LangClass::Iterable => "iter",
            LangClass::Iterator => "next",
            LangClass::Debug => "debug",
            LangClass::ToImmDeep => "to_imm_deep",
        }
    }

    /// The associated type that the syntax gives, if any: the element of
    /// `v[i]`, the iterator of a `for`, and the items it goes over.
    pub fn assoc_name(self) -> Option<&'static str> {
        match self {
            LangClass::Index | LangClass::Iterator => Some("Item"),
            LangClass::Iterable => Some("Iter"),
            LangClass::IndexMut | LangClass::Debug | LangClass::ToImmDeep => None,
        }
    }

    /// The class, as the prelude of `file` defines it. Files of the
    /// prelude's own package, which don't get the prelude, find it too, in
    /// their own copy of the package.
    pub fn class(self, db: &dyn Db, file: File) -> Option<ClassId<'_>> {
        let lib = prelude_lib(db, file)?;
        match file_scope(db, lib).scope.get(Namespace::Types, &Name::new(self.name()))?.def {
            ModuleDefId::Class(class) => Some(class),
            _ => None,
        }
    }

    /// The method of the class that the syntax calls.
    pub fn method<'db>(self, db: &'db dyn Db, class: ClassId<'db>) -> Option<FunctionId<'db>> {
        let name = Name::new(self.method_name());
        class.item(db).items.iter().find_map(|item| match *item {
            AssocItem::Function(idx) => {
                let f = FunctionId::new(db, ItemContainerId::Class(class), idx);
                (f.item(db).name.as_ref().map(|n| n.name()) == Some(&name)).then_some(f)
            }
            _ => None,
        })
    }

    /// The associated type of the class that the syntax gives.
    pub fn assoc<'db>(self, db: &'db dyn Db, class: ClassId<'db>) -> Option<TypeAliasId<'db>> {
        assoc_type(db, class, &Name::new(self.assoc_name()?))
    }
}

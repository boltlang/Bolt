//! The classes of the prelude that syntax uses: `v[i]` calls `index` of
//! `Index`, `v[i] = x` calls `set_index` of `IndexMut`, and `for x in c`
//! calls `iter` of `Iterable` and then `next` of `Iterator`, for arrays too:
//! the prelude's impls for arrays use the builtins `array_get` and
//! `array_set`. Without a prelude that defines the classes, the syntax is an
//! error.

use boltlang_base_db::{Db, File};
use boltlang_hir_def::{
    Name,
    ids::{ClassId, FunctionId, ItemContainerId, ModuleDefId, TypeAliasId},
    item_tree::AssocItem,
    nameres::{Namespace, file_scope, prelude_lib},
};

use crate::lower::assoc_type;

/// A class of the prelude that syntax uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LangClass {
    /// `class Index[C] { type Item; fn index(c: C, i: i32) -> Item; }`
    Index,
    /// `class IndexMut[C] where Index[C] { fn set_index(c: C, i: i32, x: C.Item); }`
    IndexMut,
    /// `class Iterable[C] { type Iter; fn iter(c: C) -> Iter; }`
    Iterable,
    /// `class Iterator[I] { type Item; fn next(it: I) -> Option[Item]; }`
    Iterator,
}

impl LangClass {
    pub fn name(self) -> &'static str {
        match self {
            LangClass::Index => "Index",
            LangClass::IndexMut => "IndexMut",
            LangClass::Iterable => "Iterable",
            LangClass::Iterator => "Iterator",
        }
    }

    /// The method that the syntax calls.
    pub fn method_name(self) -> &'static str {
        match self {
            LangClass::Index => "index",
            LangClass::IndexMut => "set_index",
            LangClass::Iterable => "iter",
            LangClass::Iterator => "next",
        }
    }

    /// The associated type that the syntax gives, if any: the element of
    /// `v[i]`, the iterator of a `for`, and the items it goes over.
    pub fn assoc_name(self) -> Option<&'static str> {
        match self {
            LangClass::Index | LangClass::Iterator => Some("Item"),
            LangClass::Iterable => Some("Iter"),
            LangClass::IndexMut => None,
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

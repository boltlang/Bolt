//! IDs for the items of a program.
//!
//! Like in rust-analyzer, an ID is an interned location: where the item is
//! (its file, or its class, impl or extern block) and its index in the
//! [`ItemTree`] of that file. Items directly in a file only need their file;
//! functions, consts and type aliases can also be in a container.

use boltlang_base_db::{Db, File};

use crate::{
    Name,
    item_tree::{self, Idx, item_tree as tree_of},
};

/// Where a function, const or type alias is defined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum ItemContainerId<'db> {
    File(File),
    Class(ClassId<'db>),
    Impl(ImplId<'db>),
    ExternBlock(ExternBlockId<'db>),
}

impl<'db> ItemContainerId<'db> {
    pub fn file(self, db: &'db dyn Db) -> File {
        match self {
            ItemContainerId::File(file) => file,
            ItemContainerId::Class(id) => id.file(db),
            ItemContainerId::Impl(id) => id.file(db),
            ItemContainerId::ExternBlock(id) => id.file(db),
        }
    }
}

/// Defines an interned ID for an item that is always directly in a file.
macro_rules! file_item_id {
    ($(#[$meta:meta])* $name:ident, $item:ident, $field:ident) => {
        $(#[$meta])*
        #[salsa::interned(debug)]
        pub struct $name<'db> {
            #[returns(copy)]
            pub file: File,
            #[returns(copy)]
            pub idx: Idx<item_tree::$item>,
        }

        impl<'db> $name<'db> {
            /// The item in the item tree of its file.
            pub fn item(self, db: &'db dyn Db) -> &'db item_tree::$item {
                &tree_of(db, self.file(db)).$field[self.idx(db).index()]
            }
        }
    };
}

/// Defines an interned ID for an item that can be in a container.
macro_rules! assoc_item_id {
    ($(#[$meta:meta])* $name:ident, $item:ident, $field:ident) => {
        $(#[$meta])*
        #[salsa::interned(debug)]
        pub struct $name<'db> {
            #[returns(copy)]
            pub container: ItemContainerId<'db>,
            #[returns(copy)]
            pub idx: Idx<item_tree::$item>,
        }

        impl<'db> $name<'db> {
            pub fn file(self, db: &'db dyn Db) -> File {
                self.container(db).file(db)
            }

            /// The item in the item tree of its file.
            pub fn item(self, db: &'db dyn Db) -> &'db item_tree::$item {
                &tree_of(db, self.file(db)).$field[self.idx(db).index()]
            }
        }
    };
}

file_item_id!(StructId, Struct, structs);
file_item_id!(EnumId, Enum, enums);
file_item_id!(ClassId, Class, classes);
file_item_id!(ImplId, Impl, impls);
file_item_id!(ExternBlockId, ExternBlock, extern_blocks);
file_item_id!(MacroRulesId, MacroRules, macro_rules);
assoc_item_id!(FunctionId, Function, functions);
assoc_item_id!(ConstId, Const, consts);
assoc_item_id!(TypeAliasId, TypeAlias, type_aliases);

/// A variant of an enum, like `Color.Red`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub struct EnumVariantId<'db> {
    pub parent: EnumId<'db>,
    pub index: u32,
}

impl<'db> EnumVariantId<'db> {
    pub fn name(self, db: &'db dyn Db) -> &'db Name {
        &self.parent.item(db).variants[self.index as usize]
    }
}

impl<'db> EnumId<'db> {
    /// The variant called `name`, if there is one.
    pub fn variant(self, db: &'db dyn Db, name: &Name) -> Option<EnumVariantId<'db>> {
        let index = self.item(db).variants.iter().position(|v| v == name)?;
        Some(EnumVariantId { parent: self, index: index as u32 })
    }
}

/// Anything a name can refer to in a file's scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum ModuleDefId<'db> {
    /// A whole file, as brought in by `import "./path" as name;`
    File(File),
    Function(FunctionId<'db>),
    Struct(StructId<'db>),
    Enum(EnumId<'db>),
    Variant(EnumVariantId<'db>),
    Class(ClassId<'db>),
    Const(ConstId<'db>),
    TypeAlias(TypeAliasId<'db>),
    MacroRules(MacroRulesId<'db>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{item_tree::{AssocItem, ModItem, item_tree}, test_db::TestDb};

    #[test]
    fn ids_lead_back_to_their_items() {
        let db = TestDb::new();
        let file = db.file("/main.bolt", "enum Color { Red, Green }\nclass Show[T] { fn show(x: T); }\n");
        let tree = item_tree(&db, file);

        let ModItem::Enum(idx) = tree.top_level[0] else { panic!() };
        let color = EnumId::new(&db, file, idx);
        assert_eq!(color.item(&db).name, Some(Name::new("Color")));
        let green = color.variant(&db, &Name::new("Green")).unwrap();
        assert_eq!(green.name(&db), &Name::new("Green"));
        // Interning the same location gives the same ID.
        assert_eq!(EnumId::new(&db, file, idx), color);

        let ModItem::Class(idx) = tree.top_level[1] else { panic!() };
        let class = ClassId::new(&db, file, idx);
        let AssocItem::Function(idx) = class.item(&db).items[0] else { panic!() };
        let show = FunctionId::new(&db, ItemContainerId::Class(class), idx);
        assert_eq!(show.file(&db), file);
        assert!(matches!(&show.item(&db).name, Some(crate::item_tree::FunctionName::Name(n)) if n.as_str() == "show"));
    }
}

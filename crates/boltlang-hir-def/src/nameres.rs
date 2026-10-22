//! Name resolution: what the names at the top level of a file refer to.
//!
//! This replaces rust-analyzer's `DefMap`, which is much simpler for Bolt:
//! every file is a module, and imports name the file they import from.
//!
//! - [`local_scope`] has the items defined in a file.
//! - [`file_scope`] adds what the file's imports bring in.
//!
//! Imports only see the items defined in the file they import from, not
//! what that file imports itself, so `pub import` doesn't re-export yet. That
//! also means files can import each other without cycles.

use std::collections::BTreeMap;

use boltlang_base_db::{Db, File, SystemPath};

use crate::{
    Name,
    ids::{
        ClassId, ConstId, EnumId, ExternBlockId, FunctionId, ImplId, ItemContainerId, MacroRulesId, ModuleDefId,
        StructId, TypeAliasId,
    },
    item_tree::{AssocItem, AstId, FunctionName, ImportKind, ModItem, RawVisibility, item_tree},
};

/// The namespaces a name can be in. A name can refer to a type, a value and
/// a macro at the same time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, salsa::SalsaValue)]
pub enum Namespace {
    Types,
    Values,
    Macros,
}

/// What a name in a scope refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, salsa::SalsaValue)]
pub struct ScopeEntry<'db> {
    pub def: ModuleDefId<'db>,
    /// The visibility of the definition, which decides whether other files
    /// can import it.
    pub visibility: RawVisibility,
    /// The import that brought the name in, if it's not defined here.
    pub import: Option<AstId>,
}

/// The names at the top level of a file.
#[derive(Debug, Default, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct ItemScope<'db> {
    types: BTreeMap<Name, ScopeEntry<'db>>,
    values: BTreeMap<Name, ScopeEntry<'db>>,
    macros: BTreeMap<Name, ScopeEntry<'db>>,
    impls: Vec<ImplId<'db>>,
}

impl<'db> ItemScope<'db> {
    pub fn get(&self, ns: Namespace, name: &Name) -> Option<&ScopeEntry<'db>> {
        self.namespace(ns).get(name)
    }

    /// All names in a namespace, in alphabetical order.
    pub fn entries(&self, ns: Namespace) -> impl Iterator<Item = (&Name, &ScopeEntry<'db>)> {
        self.namespace(ns).iter()
    }

    /// The impls in the file. They don't have a name.
    pub fn impls(&self) -> &[ImplId<'db>] {
        &self.impls
    }

    fn namespace(&self, ns: Namespace) -> &BTreeMap<Name, ScopeEntry<'db>> {
        match ns {
            Namespace::Types => &self.types,
            Namespace::Values => &self.values,
            Namespace::Macros => &self.macros,
        }
    }

    fn namespace_mut(&mut self, ns: Namespace) -> &mut BTreeMap<Name, ScopeEntry<'db>> {
        match ns {
            Namespace::Types => &mut self.types,
            Namespace::Values => &mut self.values,
            Namespace::Macros => &mut self.macros,
        }
    }

    /// Add a name, unless it's already there. Returns whether it was added.
    fn insert(&mut self, ns: Namespace, name: Name, entry: ScopeEntry<'db>) -> bool {
        use std::collections::btree_map::Entry;
        match self.namespace_mut(ns).entry(name) {
            Entry::Vacant(vacant) => {
                vacant.insert(entry);
                true
            }
            Entry::Occupied(_) => false,
        }
    }
}

/// A problem found while resolving the names of a file.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub enum NameResDiagnostic {
    /// Two definitions or imports of the same name in the same namespace.
    Duplicate { name: Name, ast_id: AstId },
    /// The file to import from doesn't exist, or the path isn't relative.
    UnresolvedImportPath { import: AstId, path: String },
    /// The file to import from has no item with this name.
    UnresolvedImportName { import: AstId, name: Name },
    /// The item to import exists but isn't public.
    PrivateImport { import: AstId, name: Name },
}

/// The names in a file and the problems found resolving them.
#[derive(Debug, Default, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct FileScope<'db> {
    pub scope: ItemScope<'db>,
    pub diagnostics: Vec<NameResDiagnostic>,
}

/// The items defined in `file`, without imports.
#[salsa::tracked(returns(ref))]
pub fn local_scope<'db>(db: &'db dyn Db, file: File) -> FileScope<'db> {
    let tree = item_tree(db, file);
    let mut out = FileScope::default();
    let container = ItemContainerId::File(file);
    let define = |out: &mut FileScope<'db>, ns, name: &Option<Name>, def, visibility, ast_id| {
        let Some(name) = name else { return };
        if !out.scope.insert(ns, name.clone(), ScopeEntry { def, visibility, import: None }) {
            out.diagnostics.push(NameResDiagnostic::Duplicate { name: name.clone(), ast_id });
        }
    };
    for item in &tree.top_level {
        match *item {
            ModItem::Function(idx) => {
                let f = &tree.functions[idx.index()];
                let def = ModuleDefId::Function(FunctionId::new(db, container, idx));
                define(&mut out, Namespace::Values, &function_name(&f.name), def, f.visibility, f.ast_id);
            }
            ModItem::Struct(idx) => {
                let s = &tree.structs[idx.index()];
                let def = ModuleDefId::Struct(StructId::new(db, file, idx));
                define(&mut out, Namespace::Types, &s.name, def, s.visibility, s.ast_id);
            }
            ModItem::Enum(idx) => {
                let e = &tree.enums[idx.index()];
                let def = ModuleDefId::Enum(EnumId::new(db, file, idx));
                define(&mut out, Namespace::Types, &e.name, def, e.visibility, e.ast_id);
            }
            ModItem::Class(idx) => {
                let c = &tree.classes[idx.index()];
                let def = ModuleDefId::Class(ClassId::new(db, file, idx));
                define(&mut out, Namespace::Types, &c.name, def, c.visibility, c.ast_id);
            }
            ModItem::Const(idx) => {
                let c = &tree.consts[idx.index()];
                let def = ModuleDefId::Const(ConstId::new(db, container, idx));
                define(&mut out, Namespace::Values, &c.name, def, c.visibility, c.ast_id);
            }
            ModItem::TypeAlias(idx) => {
                let t = &tree.type_aliases[idx.index()];
                let def = ModuleDefId::TypeAlias(TypeAliasId::new(db, container, idx));
                define(&mut out, Namespace::Types, &t.name, def, t.visibility, t.ast_id);
            }
            ModItem::MacroRules(idx) => {
                let m = &tree.macro_rules[idx.index()];
                let def = ModuleDefId::MacroRules(MacroRulesId::new(db, file, idx));
                // Macros don't have a visibility; treat them as public.
                define(&mut out, Namespace::Macros, &m.name, def, RawVisibility::Public, m.ast_id);
            }
            ModItem::Impl(idx) => out.scope.impls.push(ImplId::new(db, file, idx)),
            // The items of an extern block are part of the file.
            ModItem::ExternBlock(idx) => {
                let block = ExternBlockId::new(db, file, idx);
                let container = ItemContainerId::ExternBlock(block);
                for item in &tree.extern_blocks[idx.index()].items {
                    match *item {
                        AssocItem::Function(idx) => {
                            let f = &tree.functions[idx.index()];
                            let def = ModuleDefId::Function(FunctionId::new(db, container, idx));
                            define(&mut out, Namespace::Values, &function_name(&f.name), def, f.visibility, f.ast_id);
                        }
                        AssocItem::Const(idx) => {
                            let c = &tree.consts[idx.index()];
                            let def = ModuleDefId::Const(ConstId::new(db, container, idx));
                            define(&mut out, Namespace::Values, &c.name, def, c.visibility, c.ast_id);
                        }
                        AssocItem::TypeAlias(idx) => {
                            let t = &tree.type_aliases[idx.index()];
                            let def = ModuleDefId::TypeAlias(TypeAliasId::new(db, container, idx));
                            define(&mut out, Namespace::Types, &t.name, def, t.visibility, t.ast_id);
                        }
                        AssocItem::MacroCall(_) => {}
                    }
                }
            }
            ModItem::Import(_) => {}
        }
    }
    out
}

/// The name a function is defined under: its name, or for an operator like
/// `fn (+)`, the operator.
fn function_name(name: &Option<FunctionName>) -> Option<Name> {
    match name {
        Some(FunctionName::Name(name) | FunctionName::Operator(name)) => Some(name.clone()),
        None => None,
    }
}

/// The names in `file`: its own items and what its imports bring in.
#[salsa::tracked(returns(ref))]
pub fn file_scope<'db>(db: &'db dyn Db, file: File) -> FileScope<'db> {
    let tree = item_tree(db, file);
    let mut out = local_scope(db, file).clone();
    for import in &tree.imports {
        let Some(source) = &import.source else { continue };
        let Some(target) = resolve_import_path(db, file, source) else {
            out.diagnostics.push(NameResDiagnostic::UnresolvedImportPath { import: import.ast_id, path: source.clone() });
            continue;
        };
        let exports = &local_scope(db, target).scope;
        let imported = |entry: &ScopeEntry<'db>| ScopeEntry { import: Some(import.ast_id), ..*entry };
        match &import.kind {
            ImportKind::Plain => {}
            ImportKind::Alias(alias) => {
                let entry = ScopeEntry { def: ModuleDefId::File(target), visibility: import.visibility, import: Some(import.ast_id) };
                if !out.scope.insert(Namespace::Types, alias.clone(), entry) {
                    out.diagnostics.push(NameResDiagnostic::Duplicate { name: alias.clone(), ast_id: import.ast_id });
                }
            }
            ImportKind::List(names) => {
                for (name, alias) in names {
                    let local_name = alias.as_ref().unwrap_or(name);
                    let mut found = false;
                    let mut private = false;
                    for ns in [Namespace::Types, Namespace::Values, Namespace::Macros] {
                        let Some(entry) = exports.get(ns, name) else { continue };
                        if entry.visibility == RawVisibility::Private {
                            private = true;
                            continue;
                        }
                        found = true;
                        if !out.scope.insert(ns, local_name.clone(), imported(entry)) {
                            out.diagnostics
                                .push(NameResDiagnostic::Duplicate { name: local_name.clone(), ast_id: import.ast_id });
                        }
                    }
                    if !found {
                        out.diagnostics.push(if private {
                            NameResDiagnostic::PrivateImport { import: import.ast_id, name: name.clone() }
                        } else {
                            NameResDiagnostic::UnresolvedImportName { import: import.ast_id, name: name.clone() }
                        });
                    }
                }
            }
            // A glob import brings in all public names, but doesn't replace
            // names that are already in scope.
            ImportKind::Glob => {
                for ns in [Namespace::Types, Namespace::Values, Namespace::Macros] {
                    for (name, entry) in exports.entries(ns) {
                        if entry.visibility != RawVisibility::Private {
                            out.scope.insert(ns, name.clone(), imported(entry));
                        }
                    }
                }
            }
        }
    }
    out
}

/// The file that the path in `import "path"` refers to, relative to the
/// directory of `importer`. A missing extension means `.bolt`.
///
/// Only relative paths (`./a`, `../b`) are supported for now; paths like
/// `std/io` need a package system.
pub fn resolve_import_path(db: &dyn Db, importer: File, source: &str) -> Option<File> {
    if !(source.starts_with("./") || source.starts_with("../")) {
        return None;
    }
    let importer_path = importer.path(db).as_system_path_buf()?;
    let dir = importer_path.as_std_path().parent()?;
    let mut path = dir.join(source);
    if path.extension().is_none() {
        path.set_extension("bolt");
    }
    let path = SystemPath::absolute(SystemPath::new(&path), SystemPath::new("/"));
    let file = db.files().resolve(db, &path);
    file.exists(db).then_some(file)
}

#[cfg(test)]
mod tests;

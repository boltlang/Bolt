//! Name resolution: what the names at the top level of a file refer to.
//!
//! This replaces rust-analyzer's `DefMap`, which is much simpler for Bolt:
//! every file is a module, and imports name the file they import from.
//!
//! - [`local_scope`] has the items defined in a file.
//! - [`file_scope`] adds what the file's imports bring in. It's also what
//!   other files can import: an imported name has the narrower of the
//!   visibilities of the import and of the item, so `import` keeps it to
//!   the file and `pub import` re-exports it.
//!
//! Files can import each other, so the scopes of files can depend on each
//! other. They're computed by fixed-point iteration, starting from the
//! items of each file: imports only add names, so this ends.

use std::collections::BTreeMap;

use boltlang_base_db::{Db, File, Settings, SystemPath, SystemPathBuf};

use crate::manifest::{Manifest, PreludeChoice, dependency_dir, is_package_name, manifest, package_manifest, package_root};

use crate::{
    Name,
    ids::{
        ClassId, ConstId, EnumId, EnumVariantId, ExternBlockId, FunctionId, ImplId, ItemContainerId, MacroRulesId, ModuleDefId,
        StructId, TypeAliasId,
    },
    item_tree::{AssocItem, AstId, ImportKind, ItemTree, ModItem, RawVisibility, item_tree},
};

/// The namespaces a name can be in. A name can refer to a type, a value and
/// a macro at the same time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, salsa::SalsaValue)]
pub enum Namespace {
    Types,
    Values,
    Macros,
}

impl Namespace {
    pub const ALL: [Namespace; 3] = [Namespace::Types, Namespace::Values, Namespace::Macros];
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
    /// Whether the name comes from the prelude.
    pub prelude: bool,
}

impl<'db> ScopeEntry<'db> {
    /// A name that `import` brings in.
    fn imported(def: ModuleDefId<'db>, visibility: RawVisibility, import: AstId) -> Self {
        ScopeEntry { def, visibility, import: Some(import), prelude: false }
    }
}

/// The names at the top level of a file.
#[derive(Debug, Default, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct ItemScope<'db> {
    types: BTreeMap<Name, ScopeEntry<'db>>,
    values: BTreeMap<Name, ScopeEntry<'db>>,
    macros: BTreeMap<Name, ScopeEntry<'db>>,
    /// The functions after the first with a name in `values`: together, the
    /// overloads of the name (#275).
    overloads: BTreeMap<Name, Vec<ScopeEntry<'db>>>,
    impls: Vec<ImplId<'db>>,
}

impl<'db> ItemScope<'db> {
    /// What `name` refers to, or the first of its overloads.
    pub fn get(&self, ns: Namespace, name: &Name) -> Option<&ScopeEntry<'db>> {
        self.namespace(ns).get(name)
    }

    /// Everything `name` refers to: one entry, or every overload of a
    /// function.
    pub fn get_all(&self, ns: Namespace, name: &Name) -> impl Iterator<Item = &ScopeEntry<'db>> {
        let more = match ns {
            Namespace::Values => self.overloads.get(name).map(Vec::as_slice).unwrap_or_default(),
            _ => &[],
        };
        self.namespace(ns).get(name).into_iter().chain(more)
    }

    /// All names in a namespace, in alphabetical order, with a name once
    /// for each of its overloads.
    pub fn entries(&self, ns: Namespace) -> impl Iterator<Item = (&Name, &ScopeEntry<'db>)> {
        self.namespace(ns).keys().flat_map(move |name| self.get_all(ns, name).map(move |entry| (name, entry)))
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

    /// Add the names of `other` that aren't here yet.
    fn extend(&mut self, other: &ItemScope<'db>) {
        for ns in Namespace::ALL {
            for (name, entry) in other.entries(ns) {
                self.insert(ns, name.clone(), *entry);
            }
        }
    }

    /// Add a name, unless it's already there. A function whose name other
    /// functions have is added as one more overload of the name, unless
    /// it's one of them already. Returns whether it was added.
    fn insert(&mut self, ns: Namespace, name: Name, entry: ScopeEntry<'db>) -> bool {
        use std::collections::btree_map::Entry;
        let is_function = |e: &ScopeEntry<'db>| matches!(e.def, ModuleDefId::Function(_));
        let overloads = ns == Namespace::Values
            && is_function(&entry)
            && self.get_all(ns, &name).all(|e| is_function(e) && e.def != entry.def);
        match self.namespace_mut(ns).entry(name.clone()) {
            Entry::Vacant(vacant) => {
                vacant.insert(entry);
                true
            }
            Entry::Occupied(_) if overloads => {
                self.overloads.entry(name).or_default().push(entry);
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
    /// The path of an import doesn't lead to a file.
    UnresolvedImportPath { import: AstId, path: String, error: ImportPathError },
    /// The file to import from has no item with this name.
    UnresolvedImportName { import: AstId, name: Name },
    /// The item to import exists but isn't visible here: it's private, or
    /// `pub(package)` in another package when `package_only` is set.
    PrivateImport { import: AstId, name: Name, package_only: bool },
    /// The enum of `import Option.*;` can't be found. `path` is as far as
    /// the path got, like `Option.Some` for `import Option.Some.*;`.
    UnresolvedImportEnum { import: AstId, path: String },
    /// The path of `import Point.*;` names something other than an enum.
    NotAnEnum { import: AstId, path: String },
    /// The prelude of [`prelude_package`] can't be found, and it was asked
    /// for.
    UnresolvedPrelude { package: String, error: ImportPathError },
    /// The `Vec` of `fn Vec.new()` isn't a struct or enum of the same file.
    UnresolvedOwner { function: AstId, name: Name },
    /// `fn Vec.new()` somewhere other than the top level of a file, like in
    /// a class.
    OwnerNotTopLevel { function: AstId },
}

/// The names in a file and the problems found resolving them.
#[derive(Debug, Default, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct FileScope<'db> {
    pub scope: ItemScope<'db>,
    /// The functions in the namespaces of the file's types, like `new` in
    /// `fn Vec.new()`, by the name of the type. They're only found by a
    /// path that starts with the type: see [`type_member`].
    pub members: BTreeMap<Name, BTreeMap<Name, ScopeEntry<'db>>>,
    pub diagnostics: Vec<NameResDiagnostic>,
}

impl<'db> FileScope<'db> {
    /// Add a name, or report that it's already there. `ast_id` is the item
    /// or import that adds it.
    fn define(&mut self, ns: Namespace, name: &Name, entry: ScopeEntry<'db>, ast_id: AstId) {
        if !self.scope.insert(ns, name.clone(), entry) {
            self.diagnostics.push(NameResDiagnostic::Duplicate { name: name.clone(), ast_id });
        }
    }

    /// Add an item that's defined in the file.
    fn define_item(&mut self, ns: Namespace, name: &Option<Name>, def: ModuleDefId<'db>, visibility: RawVisibility, ast_id: AstId) {
        if let Some(name) = name {
            self.define(ns, name, ScopeEntry { def, visibility, import: None, prelude: false }, ast_id);
        }
    }

    /// Add a function, const or type alias of `container`. `visibility` is
    /// the one it gets, if it's not its own.
    fn define_assoc(
        &mut self,
        db: &'db dyn Db,
        tree: &ItemTree,
        container: ItemContainerId<'db>,
        item: AssocItem,
        visibility: Option<RawVisibility>,
    ) {
        match item {
            AssocItem::Function(idx) => {
                let f = &tree.functions[idx.index()];
                let def = ModuleDefId::Function(FunctionId::new(db, container, idx));
                let name = f.name.as_ref().map(|name| name.name().clone());
                self.define_item(Namespace::Values, &name, def, visibility.unwrap_or(f.visibility), f.ast_id);
            }
            AssocItem::Const(idx) => {
                let c = &tree.consts[idx.index()];
                let def = ModuleDefId::Const(ConstId::new(db, container, idx));
                self.define_item(Namespace::Values, &c.name, def, visibility.unwrap_or(c.visibility), c.ast_id);
            }
            AssocItem::TypeAlias(idx) => {
                let t = &tree.type_aliases[idx.index()];
                let def = ModuleDefId::TypeAlias(TypeAliasId::new(db, container, idx));
                self.define_item(Namespace::Types, &t.name, def, visibility.unwrap_or(t.visibility), t.ast_id);
            }
            AssocItem::MacroCall(_) => {}
        }
    }
}

/// The items defined in `file`, without imports.
#[salsa::tracked(returns(ref))]
pub fn local_scope<'db>(db: &'db dyn Db, file: File) -> FileScope<'db> {
    let tree = item_tree(db, file);
    let mut out = FileScope::default();
    let container = ItemContainerId::File(file);
    for item in &tree.top_level {
        match *item {
            // Those of a type are added below, once the types are known.
            // Tests aren't called by name: see `test_functions`.
            ModItem::Function(idx) if tree.functions[idx.index()].owner.is_some() || tree.functions[idx.index()].is_test => {}
            ModItem::Function(idx) => out.define_assoc(db, tree, container, AssocItem::Function(idx), None),
            ModItem::Const(idx) => out.define_assoc(db, tree, container, AssocItem::Const(idx), None),
            ModItem::TypeAlias(idx) => out.define_assoc(db, tree, container, AssocItem::TypeAlias(idx), None),
            ModItem::Struct(idx) => {
                let s = &tree.structs[idx.index()];
                let def = ModuleDefId::Struct(StructId::new(db, file, idx));
                out.define_item(Namespace::Types, &s.name, def, s.visibility, s.ast_id);
            }
            ModItem::Enum(idx) => {
                let e = &tree.enums[idx.index()];
                let def = ModuleDefId::Enum(EnumId::new(db, file, idx));
                out.define_item(Namespace::Types, &e.name, def, e.visibility, e.ast_id);
            }
            ModItem::Class(idx) => {
                let c = &tree.classes[idx.index()];
                let class = ClassId::new(db, file, idx);
                out.define_item(Namespace::Types, &c.name, ModuleDefId::Class(class), c.visibility, c.ast_id);
                // Class methods are called like functions, as in Haskell, and
                // class constants are used like constants, so they're in the
                // file's scope, with the class's visibility. Associated types
                // aren't.
                for &item in &c.items {
                    if !matches!(item, AssocItem::TypeAlias(_)) {
                        out.define_assoc(db, tree, ItemContainerId::Class(class), item, Some(c.visibility));
                    }
                }
            }
            ModItem::MacroRules(idx) => {
                let m = &tree.macro_rules[idx.index()];
                let def = ModuleDefId::MacroRules(MacroRulesId::new(db, file, idx));
                // Macros don't have a visibility; treat them as public.
                out.define_item(Namespace::Macros, &m.name, def, RawVisibility::Public, m.ast_id);
            }
            ModItem::Impl(idx) => out.scope.impls.push(ImplId::new(db, file, idx)),
            // The items of an extern block are part of the file.
            ModItem::ExternBlock(idx) => {
                let block = ItemContainerId::ExternBlock(ExternBlockId::new(db, file, idx));
                for &item in &tree.extern_blocks[idx.index()].items {
                    out.define_assoc(db, tree, block, item, None);
                }
            }
            ModItem::Import(_) => {}
        }
    }
    define_members(db, file, tree, &mut out);
    out
}

/// Add the functions of `file` that are in the namespace of one of its
/// types, like `fn Vec.new()`. The type must be a struct or enum of the
/// same file, so that only the file that defines a type adds to it.
fn define_members<'db>(db: &'db dyn Db, file: File, tree: &ItemTree, out: &mut FileScope<'db>) {
    let container = ItemContainerId::File(file);
    for item in &tree.top_level {
        let ModItem::Function(idx) = *item else { continue };
        let f = &tree.functions[idx.index()];
        if f.is_test {
            continue;
        }
        let (Some(owner), Some(name)) = (&f.owner, &f.name) else { continue };
        let variants = match out.scope.get(Namespace::Types, owner).map(|entry| entry.def) {
            Some(ModuleDefId::Struct(_)) => &[][..],
            Some(ModuleDefId::Enum(id)) => &id.item(db).variants[..],
            _ => {
                out.diagnostics.push(NameResDiagnostic::UnresolvedOwner { function: f.ast_id, name: owner.clone() });
                continue;
            }
        };
        let name = name.name();
        let def = ModuleDefId::Function(FunctionId::new(db, container, idx));
        let entry = ScopeEntry { def, visibility: f.visibility, import: None, prelude: false };
        // A variant is in the same namespace: `Option.Some`. The first
        // definition of a name stays, as in the file's scope.
        let members = out.members.entry(owner.clone()).or_default();
        let added = !variants.contains(name) && !members.contains_key(name);
        if added {
            members.insert(name.clone(), entry);
        } else {
            out.diagnostics.push(NameResDiagnostic::Duplicate { name: name.clone(), ast_id: f.ast_id });
        }
    }
    // Only functions at the top level can be in a type's namespace.
    let top_level: Vec<AstId> = tree
        .top_level
        .iter()
        .filter_map(|item| match *item {
            ModItem::Function(idx) => Some(tree.functions[idx.index()].ast_id),
            _ => None,
        })
        .collect();
    for f in &tree.functions {
        if f.owner.is_some() && !top_level.contains(&f.ast_id) {
            out.diagnostics.push(NameResDiagnostic::OwnerNotTopLevel { function: f.ast_id });
        }
    }
}

/// The tests of `file`: its functions with `#[test]` before them, in the
/// order they're written. They aren't in the file's scope, so a test
/// called `push` isn't one more `push`.
#[salsa::tracked(returns(ref))]
pub fn test_functions<'db>(db: &'db dyn Db, file: File) -> Vec<FunctionId<'db>> {
    let tree = item_tree(db, file);
    tree.top_level
        .iter()
        .filter_map(|item| match *item {
            ModItem::Function(idx) if tree.functions[idx.index()].is_test => {
                Some(FunctionId::new(db, ItemContainerId::File(file), idx))
            }
            _ => None,
        })
        .collect()
}

/// The function `name` in the namespace of the type `owner`, a struct or
/// enum defined in `file`, as for `Vec.new`, if it's visible from `from`.
pub fn type_member<'db>(db: &'db dyn Db, file: File, owner: &Name, name: &Name, from: File) -> Option<ModuleDefId<'db>> {
    let entry = type_member_entry(db, file, owner, name)?;
    is_visible_from(db, entry.visibility, file, from).then_some(entry.def)
}

/// The function `name` in the namespace of the type `owner` of `file`,
/// whether it's visible or not.
pub fn type_member_entry<'db>(db: &'db dyn Db, file: File, owner: &Name, name: &Name) -> Option<ScopeEntry<'db>> {
    local_scope(db, file).members.get(owner)?.get(name).copied()
}

/// The names in `file`: its own items and what its imports bring in.
#[salsa::tracked(returns(ref), cycle_initial = file_scope_initial)]
pub fn file_scope<'db>(db: &'db dyn Db, file: File) -> FileScope<'db> {
    let tree = item_tree(db, file);
    let mut out = local_scope(db, file).clone();
    // Glob imports come after the others, so that they never replace or
    // clash with a name that one of them brings in, wherever it is.
    let mut globs = Vec::new();
    for import in &tree.imports {
        let Some(source) = &import.source else { continue };
        let target = match resolve_import_path(db, file, source) {
            Ok(target) => target,
            Err(error) => {
                let path = source.clone();
                out.diagnostics.push(NameResDiagnostic::UnresolvedImportPath { import: import.ast_id, path, error });
                continue;
            }
        };
        let exports = &file_scope(db, target).scope;
        let imported = |entry: &ScopeEntry<'db>| {
            ScopeEntry::imported(entry.def, narrower(import.visibility, entry.visibility), import.ast_id)
        };
        match &import.kind {
            ImportKind::Plain => {}
            ImportKind::Alias(alias) => {
                let entry = ScopeEntry::imported(ModuleDefId::File(target), import.visibility, import.ast_id);
                out.define(Namespace::Types, alias, entry, import.ast_id);
            }
            ImportKind::List(names) => {
                for (name, alias) in names {
                    let local_name = alias.as_ref().unwrap_or(name);
                    let mut found = false;
                    let mut hidden = None;
                    // Every overload of a function, too.
                    for ns in Namespace::ALL {
                        for entry in exports.get_all(ns, name) {
                            if !is_visible_from(db, entry.visibility, target, file) {
                                hidden = Some(entry.visibility);
                                continue;
                            }
                            found = true;
                            out.define(ns, local_name, imported(entry), import.ast_id);
                        }
                    }
                    if !found {
                        out.diagnostics.push(match hidden {
                            Some(visibility) => NameResDiagnostic::PrivateImport {
                                import: import.ast_id,
                                name: name.clone(),
                                package_only: visibility == RawVisibility::Package,
                            },
                            None => NameResDiagnostic::UnresolvedImportName { import: import.ast_id, name: name.clone() },
                        });
                    }
                }
            }
            ImportKind::Glob => globs.push((import, target)),
            // Handled below.
            ImportKind::Variants(_) => {}
        }
    }
    // A glob import brings in all the names visible here, but doesn't
    // replace names that are already in scope.
    for (import, target) in globs {
        for ns in Namespace::ALL {
            for (name, entry) in file_scope(db, target).scope.entries(ns) {
                if is_visible_from(db, entry.visibility, target, file) {
                    let entry =
                        ScopeEntry::imported(entry.def, narrower(import.visibility, entry.visibility), import.ast_id);
                    out.scope.insert(ns, name.clone(), entry);
                }
            }
        }
    }
    // The prelude's names come last, so that everything else replaces them.
    // They aren't re-exported.
    let mut prelude = ItemScope::default();
    match prelude_file(db, file).clone() {
        Ok(Some(source)) => {
            for ns in Namespace::ALL {
                for (name, entry) in file_scope(db, source).scope.entries(ns) {
                    if is_visible_from(db, entry.visibility, source, file) {
                        let entry = ScopeEntry { visibility: RawVisibility::Private, import: None, prelude: true, ..*entry };
                        prelude.insert(ns, name.clone(), entry);
                    }
                }
            }
        }
        Ok(None) => {}
        Err(error) => {
            if let Some((package, true)) = prelude_package(db, file) {
                out.diagnostics.push(NameResDiagnostic::UnresolvedPrelude { package, error });
            }
        }
    }
    // `import Option.*;` comes after the other imports, so that the enum can
    // be one of the names they bring in, wherever the import is, or one of
    // the prelude.
    let mut enums = out.scope.clone();
    enums.extend(&prelude);
    for import in &tree.imports {
        let ImportKind::Variants(path) = &import.kind else { continue };
        let (id, visibility) = match import_enum(db, file, import.ast_id, &enums, path) {
            Ok((id, visibility)) => (id, narrower(import.visibility, visibility)),
            Err(error) => {
                out.diagnostics.push(error);
                continue;
            }
        };
        for (index, name) in id.item(db).variants.iter().enumerate() {
            let variant = EnumVariantId { parent: id, index: index as u32 };
            let entry = ScopeEntry::imported(ModuleDefId::Variant(variant), visibility, import.ast_id);
            // Like a glob import, it doesn't replace names already in scope.
            // A variant is a value, like `None`, and can be the type of a
            // record literal, like `Circle { r: 1 }`.
            for ns in [Namespace::Values, Namespace::Types] {
                out.scope.insert(ns, name.clone(), entry);
            }
        }
    }
    out.scope.extend(&prelude);
    out
}

/// The package whose prelude `file` gets, and whether that's asked for: by
/// its package's `bolt.toml`, or else [`Settings::prelude`]. `None` for no
/// prelude, with `prelude = false` or no prelude in the settings, as with
/// `bolt check --no-prelude`. The files of `std` get none.
pub fn prelude_package(db: &dyn Db, file: File) -> Option<(String, bool)> {
    let default = Settings::get(db).prelude(db).clone()?;
    let required = *Settings::get(db).prelude_required(db);
    let Some(m) = package_manifest(db, file) else { return Some((default, required)) };
    match manifest(db, m) {
        Ok(Manifest { prelude: PreludeChoice::Package(name), .. }) => Some((name.clone(), true)),
        Ok(Manifest { prelude: PreludeChoice::None, .. }) => None,
        Ok(Manifest { prelude: PreludeChoice::Default, .. }) | Err(_) => Some((default, required)),
    }
}

/// The entry file of the prelude that `file` gets, that of
/// [`prelude_package`]: see [`prelude_entry`]. `Ok(None)` when there's no
/// prelude, and for the files of the prelude's own package, which would
/// otherwise import themselves.
#[salsa::tracked]
pub fn prelude_file(db: &dyn Db, file: File) -> Result<Option<File>, ImportPathError> {
    let Some((package, _)) = prelude_package(db, file) else { return Ok(None) };
    let prelude = prelude_entry(db, file, &package)?;
    let own = package_of(db, prelude);
    Ok((own.is_none() || package_of(db, file) != own).then_some(prelude))
}

/// The files whose items `file` can use: its prelude, the files it imports,
/// and those they use in turn, each once, without `file` itself. They're in
/// the order they're found, nearest first, so the prelude comes first.
#[salsa::tracked(returns(ref))]
pub fn files_used_by(db: &dyn Db, file: File) -> Vec<File> {
    let mut seen = vec![file];
    let mut queue = std::collections::VecDeque::from([file]);
    while let Some(next) = queue.pop_front() {
        let tree = item_tree(db, next);
        let prelude = prelude_file(db, next).as_ref().ok().copied().flatten();
        let imports = tree.top_level.iter().filter_map(|item| match *item {
            ModItem::Import(idx) => tree.imports[idx.index()].source.as_ref(),
            _ => None,
        });
        let imported = imports.filter_map(|source| resolve_import_path(db, next, source).ok());
        for found in prelude.into_iter().chain(imported) {
            if !seen.contains(&found) {
                seen.push(found);
                queue.push_back(found);
            }
        }
    }
    seen.remove(0);
    seen
}

/// The entry file of the prelude's package, as seen from `file`, even when
/// `file` is in that package, unlike [`prelude_file`]: where the classes
/// that syntax uses are, like `Index`.
pub fn prelude_lib(db: &dyn Db, file: File) -> Option<File> {
    let (package, _) = prelude_package(db, file).or_else(|| {
        // The files of `std` have no prelude, but still `std`'s classes.
        let std = Settings::get(db).std_dir(db).as_ref()?;
        (package_of(db, file).as_ref() == Some(std)).then(|| (STD.to_string(), false))
    })?;
    prelude_entry(db, file, &package).ok()
}

/// The entry file of the prelude of `package`, as seen from `file`: its
/// `__prelude__.bolt`, for a prelude of one file, or its
/// `__prelude__/lib.bolt`, for one of several, looked up like an import.
/// Having both is an error, rather than a choice between them.
fn prelude_entry(db: &dyn Db, file: File, package: &str) -> Result<File, ImportPathError> {
    let single = resolve_import_path(db, file, &format!("{package}/__prelude__"));
    let several = resolve_import_path(db, file, &format!("{package}/__prelude__/lib"));
    match (single, several) {
        (Ok(_), Ok(_)) => Err(ImportPathError::TwoPreludes { package: package.to_string() }),
        (Ok(prelude), Err(_)) | (Err(_), Ok(prelude)) => Ok(prelude),
        (Err(_), Err(error)) => Err(error),
    }
}

/// Where the fixed-point iteration of [`file_scope`] starts, for files that
/// import each other.
fn file_scope_initial<'db>(db: &'db dyn Db, _id: salsa::Id, file: File) -> FileScope<'db> {
    local_scope(db, file).clone()
}

/// The more restrictive of two visibilities.
fn narrower(a: RawVisibility, b: RawVisibility) -> RawVisibility {
    let rank = |v| match v {
        RawVisibility::Private => 0,
        RawVisibility::Package => 1,
        RawVisibility::Public | RawVisibility::Super => 2,
    };
    if rank(a) <= rank(b) { a } else { b }
}

/// The enum of `import Option.*;`, the import `import` in `file` whose
/// names so far are `scope`, and its visibility. The path can go through
/// files imported with `as`, like `m.Shape`, and then sees what those files
/// export, as other imports do.
fn import_enum<'db>(
    db: &'db dyn Db,
    file: File,
    import: AstId,
    scope: &ItemScope<'db>,
    path: &[Name],
) -> Result<(EnumId<'db>, RawVisibility), NameResDiagnostic> {
    let joined = |count: usize| path[..count].iter().map(Name::as_str).collect::<Vec<_>>().join(".");
    let (mut scope, mut owner) = (scope, file);
    for (i, name) in path.iter().enumerate() {
        let last = i + 1 == path.len();
        let Some(entry) = scope.get(Namespace::Types, name) else {
            return Err(NameResDiagnostic::UnresolvedImportEnum { import, path: joined(i + 1) });
        };
        if owner != file && !is_visible_from(db, entry.visibility, owner, file) {
            let package_only = entry.visibility == RawVisibility::Package;
            return Err(NameResDiagnostic::PrivateImport { import, name: name.clone(), package_only });
        }
        match entry.def {
            ModuleDefId::File(next) if !last => {
                owner = next;
                scope = &file_scope(db, next).scope;
            }
            ModuleDefId::Enum(id) if last => return Ok((id, entry.visibility)),
            _ if last => return Err(NameResDiagnostic::NotAnEnum { import, path: joined(i + 1) }),
            // Only files can be in the middle of the path.
            _ => return Err(NameResDiagnostic::UnresolvedImportEnum { import, path: joined(i + 2) }),
        }
    }
    // An empty path, after a syntax error.
    Err(NameResDiagnostic::UnresolvedImportEnum { import, path: String::new() })
}

/// Why the path of an import doesn't lead to a file.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub enum ImportPathError {
    /// A relative path to a file that doesn't exist.
    FileNotFound,
    /// A package, but the importer is in no package: there's no `bolt.toml`
    /// in its directory or one of its parents, so it can only use `std`.
    NoManifest { package: String },
    /// A package that isn't a dependency in the importer's `bolt.toml`.
    NotADependency { package: String },
    /// The importer's `bolt.toml` has an error, which is reported on its own.
    BadManifest,
    /// The directory of a dependency, `path` as written in `bolt.toml`, has
    /// no `bolt.toml`, or doesn't exist.
    DependencyNotFound { package: String, path: String },
    /// `std`, which the compiler doesn't have.
    NoStd,
    /// The package exists, but the file in it doesn't. `path` is relative to
    /// the package, like `io.bolt`.
    NotInPackage { package: String, path: String },
    /// Neither a relative path nor one in a package: an absolute path, a
    /// package name that isn't one, or a path that leaves its package.
    Invalid,
    /// The package has both a `__prelude__.bolt` and a
    /// `__prelude__/lib.bolt`, so it isn't clear which is its prelude.
    TwoPreludes { package: String },
}

/// The file that the path in `import "path"` refers to. A missing extension
/// means `.bolt`.
///
/// - `./a` and `../b` are relative to the directory of `importer`.
/// - `shapes/circle` is the file `circle.bolt` of the package `shapes`, and
///   `shapes` alone is the package's `lib.bolt`. The package is a dependency
///   in the `bolt.toml` of the importer's package ([`crate::manifest`]), or
///   `std`, which comes with the compiler ([`Settings::std_dir`]).
///
/// Every file is reached by its real path, with symbolic links resolved, so
/// that it has one [`File`] however it's reached
/// ([`Files::resolve`](boltlang_base_db::Files::resolve)).
/// Salsa doesn't track symbolic links, so changing where one points takes
/// effect when the server starts again.
pub fn resolve_import_path(db: &dyn Db, importer: File, source: &str) -> Result<File, ImportPathError> {
    let dir = importer
        .path(db)
        .as_system_path_buf()
        .and_then(|p| p.as_std_path().parent())
        .ok_or(ImportPathError::FileNotFound)?;
    if source.starts_with("./") || source.starts_with("../") {
        let path = SystemPath::absolute(SystemPath::new(&with_extension(dir.join(source))), SystemPath::new("/"));
        let file = db.files().resolve(db, &path);
        return if file.exists(db) { Ok(file) } else { Err(ImportPathError::FileNotFound) };
    }
    let (package, path) = source.split_once('/').unwrap_or((source, "lib"));
    let segments_ok = path.split('/').all(|s| !s.is_empty() && s != "." && s != "..");
    if !is_package_name(package) || !segments_ok {
        return Err(ImportPathError::Invalid);
    }
    let root = package_dir(db, importer, package)?;
    let path = with_extension(std::path::PathBuf::from(path));
    let file = db.files().resolve(db, SystemPath::new(&root.as_std_path().join(&path)));
    if file.exists(db) {
        Ok(file)
    } else {
        Err(ImportPathError::NotInPackage { package: package.to_string(), path: path.to_string_lossy().into_owned() })
    }
}

/// The directory of the package that `importer` calls `package`: `std`, or a
/// dependency of its package.
fn package_dir(db: &dyn Db, importer: File, package: &str) -> Result<SystemPathBuf, ImportPathError> {
    if package == STD {
        return Settings::get(db).std_dir(db).clone().ok_or(ImportPathError::NoStd);
    }
    let Some(manifest_file) = package_manifest(db, importer) else {
        return Err(ImportPathError::NoManifest { package: package.to_string() });
    };
    let Ok(manifest) = manifest(db, manifest_file) else { return Err(ImportPathError::BadManifest) };
    let Some(written) = manifest.dependency(package) else {
        return Err(ImportPathError::NotADependency { package: package.to_string() });
    };
    dependency_dir(db, manifest_file, written)
        .ok_or_else(|| ImportPathError::DependencyNotFound { package: package.to_string(), path: written.to_string() })
}

/// The path of the file an import names: `.bolt` is added, unless it's
/// there already, so that `./parser.v2` is `./parser.v2.bolt`.
fn with_extension(path: std::path::PathBuf) -> std::path::PathBuf {
    if path.extension().is_some_and(|e| e == "bolt") {
        return path;
    }
    let mut path = path.into_os_string();
    path.push(".bolt");
    path.into()
}


#[cfg(test)]
mod tests;

/// The name of the package that comes with the compiler.
pub const STD: &str = "std";

/// The package `file` is in, as its directory: `std`'s, for the files of
/// the `std` that comes with the compiler, by their path or their real one,
/// or else that of the nearest `bolt.toml` ([`package_manifest`]). `None`
/// for a file in no package.
pub fn package_of(db: &dyn Db, file: File) -> Option<SystemPathBuf> {
    let path = file.path(db).as_system_path_buf()?;
    if let Some(root) = Settings::get(db).std_dir(db) {
        let real = || db.system().canonicalize_path(path).ok();
        if path.as_std_path().starts_with(root.as_std_path())
            || real().is_some_and(|real| real.as_std_path().starts_with(root.as_std_path()))
        {
            return Some(root.clone());
        }
    }
    // By its real path, so that it's the same package through a link.
    let root = package_manifest(db, file).and_then(|m| package_root(db, m))?;
    Some(db.system().canonicalize_path(&root).unwrap_or(root))
}

/// Whether an item defined in `owner` with `visibility` can be used in
/// `from`: a private item only in its own file, and a `pub(package)` one
/// only in the files of the same package.
pub fn is_visible_from(db: &dyn Db, visibility: RawVisibility, owner: File, from: File) -> bool {
    match visibility {
        RawVisibility::Private => owner == from,
        RawVisibility::Package => owner == from || package_of(db, owner) == package_of(db, from),
        RawVisibility::Public | RawVisibility::Super => true,
    }
}

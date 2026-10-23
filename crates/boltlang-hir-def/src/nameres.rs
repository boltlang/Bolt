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

use boltlang_base_db::{Db, File, SystemPath, SystemPathBuf};

use crate::{
    Name,
    ids::{
        ClassId, ConstId, EnumId, EnumVariantId, ExternBlockId, FunctionId, ImplId, ItemContainerId, MacroRulesId, ModuleDefId,
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
                let class = ClassId::new(db, file, idx);
                define(&mut out, Namespace::Types, &c.name, ModuleDefId::Class(class), c.visibility, c.ast_id);
                // Class methods are called like functions, as in Haskell, so
                // they're in the file's scope, with the class's visibility.
                for item in &c.items {
                    if let AssocItem::Function(idx) = *item {
                        let f = &tree.functions[idx.index()];
                        let def = ModuleDefId::Function(FunctionId::new(db, ItemContainerId::Class(class), idx));
                        define(&mut out, Namespace::Values, &function_name(&f.name), def, c.visibility, f.ast_id);
                    }
                }
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
/// `fn a + b`, the operator.
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
        let target = match resolve_import_path(db, file, source) {
            Ok(target) => target,
            Err(error) => {
                let path = source.clone();
                out.diagnostics.push(NameResDiagnostic::UnresolvedImportPath { import: import.ast_id, path, error });
                continue;
            }
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
                    let mut hidden = None;
                    for ns in [Namespace::Types, Namespace::Values, Namespace::Macros] {
                        let Some(entry) = exports.get(ns, name) else { continue };
                        if !is_visible_from(db, entry.visibility, target, file) {
                            hidden = Some(entry.visibility);
                            continue;
                        }
                        found = true;
                        if !out.scope.insert(ns, local_name.clone(), imported(entry)) {
                            out.diagnostics
                                .push(NameResDiagnostic::Duplicate { name: local_name.clone(), ast_id: import.ast_id });
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
            // A glob import brings in all the names visible here, but doesn't
            // replace names that are already in scope.
            ImportKind::Glob => {
                for ns in [Namespace::Types, Namespace::Values, Namespace::Macros] {
                    for (name, entry) in exports.entries(ns) {
                        if is_visible_from(db, entry.visibility, target, file) {
                            out.scope.insert(ns, name.clone(), imported(entry));
                        }
                    }
                }
            }
            // Handled below.
            ImportKind::Variants(_) => {}
        }
    }
    // `import Option.*;` comes after the other imports, so that the enum can
    // be one of the names they bring in, wherever the import is.
    for import in &tree.imports {
        let ImportKind::Variants(path) = &import.kind else { continue };
        let (id, visibility) = match import_enum(db, file, import.ast_id, &out.scope, path) {
            Ok(found) => found,
            Err(error) => {
                out.diagnostics.push(error);
                continue;
            }
        };
        for (index, name) in id.item(db).variants.iter().enumerate() {
            let variant = EnumVariantId { parent: id, index: index as u32 };
            let entry = ScopeEntry { def: ModuleDefId::Variant(variant), visibility, import: Some(import.ast_id) };
            // Like a glob import, it doesn't replace names already in scope.
            // A variant is a value, like `None`, and can be the type of a
            // record literal, like `Circle { r: 1 }`.
            for ns in [Namespace::Values, Namespace::Types] {
                out.scope.insert(ns, name.clone(), entry);
            }
        }
    }
    out
}

/// The enum of `import Option.*;`, the import `import` in `file` whose
/// names so far are `scope`, and its visibility. The path can go through
/// files imported with `as`, like `m.Shape`, and then sees the items defined
/// in them, as other imports do.
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
                scope = &local_scope(db, next).scope;
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
    /// No `.bolt/packages/<package>` in the directory of the importer or in
    /// one of its parents.
    PackageNotFound { package: String },
    /// The package exists, but the file in it doesn't. `path` is relative to
    /// the package, like `io.bolt`.
    NotInPackage { package: String, path: String },
    /// Neither a relative path nor one in a package: an absolute path, a
    /// package name that isn't one, or a path that leaves its package.
    Invalid,
}

/// The file that the path in `import "path"` refers to. A missing extension
/// means `.bolt`.
///
/// - `./a` and `../b` are relative to the directory of `importer`.
/// - `std/io` is the file `io.bolt` of the package `std`, and `std` alone
///   is the package's `lib.bolt`. Packages are the directories in
///   `.bolt/packages`, and the nearest one wins: the one in the directory of
///   `importer`, or else in its parent, and so on.
///
/// Symbolic links are resolved for the files of the project, so that a file
/// has one [`File`] however it's reached. Paths through `.bolt/packages` are
/// kept, since they say which package a file is in (see [`package_of`]):
/// a linked package stays a package. Salsa doesn't track symbolic links, so
/// changing one takes effect when the file is synced.
pub fn resolve_import_path(db: &dyn Db, importer: File, source: &str) -> Result<File, ImportPathError> {
    let dir = importer
        .path(db)
        .as_system_path_buf()
        .and_then(|p| p.as_std_path().parent())
        .ok_or(ImportPathError::FileNotFound)?;
    if source.starts_with("./") || source.starts_with("../") {
        let path = SystemPath::absolute(SystemPath::new(&with_extension(dir.join(source))), SystemPath::new("/"));
        let file = db.files().resolve(db, &path);
        return if file.exists(db) { Ok(canonical(db, file)) } else { Err(ImportPathError::FileNotFound) };
    }
    let (package, path) = source.split_once('/').unwrap_or((source, "lib"));
    let segments_ok = path.split('/').all(|s| !s.is_empty() && s != "." && s != "..");
    if !is_package_name(package) || !segments_ok {
        return Err(ImportPathError::Invalid);
    }
    let path = with_extension(std::path::PathBuf::from(path));
    for dir in dir.ancestors() {
        let root = dir.join(".bolt").join("packages").join(package);
        if !db.files().resolve(db, SystemPath::new(&root)).is_directory(db) {
            continue;
        }
        let file = db.files().resolve(db, SystemPath::new(&root.join(&path)));
        if file.exists(db) {
            return Ok(canonical(db, file));
        }
        let path = path.to_string_lossy().into_owned();
        return Err(ImportPathError::NotInPackage { package: package.to_string(), path });
    }
    Err(ImportPathError::PackageNotFound { package: package.to_string() })
}

fn with_extension(mut path: std::path::PathBuf) -> std::path::PathBuf {
    if path.extension().is_none() {
        path.set_extension("bolt");
    }
    path
}

/// Like an identifier, with `-` allowed after the first character.
fn is_package_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// The file with the symbolic links in its path resolved, unless it's in a
/// package.
fn canonical(db: &dyn Db, file: File) -> File {
    let Some(path) = file.path(db).as_system_path_buf() else { return file };
    if package_dir(path.as_std_path()).is_some() {
        return file;
    }
    match db.system().canonicalize_path(path) {
        Ok(real) if real != *path => db.files().resolve(db, &real),
        _ => file,
    }
}

#[cfg(test)]
mod tests;

/// The package `file` is in: the directory `.bolt/packages/<name>` in its
/// path, the innermost one if there are several. `None` for the files of
/// the project itself.
pub fn package_of(db: &dyn Db, file: File) -> Option<SystemPathBuf> {
    let path = file.path(db).as_system_path_buf()?;
    package_dir(path.as_std_path()).map(|dir| SystemPathBuf::from(dir.to_path_buf()))
}

fn package_dir(path: &std::path::Path) -> Option<&std::path::Path> {
    path.ancestors().skip(1).find(|dir| {
        let Some(packages) = dir.parent() else { return false };
        packages.file_name() == Some("packages".as_ref())
            && packages.parent().and_then(|bolt| bolt.file_name()) == Some(".bolt".as_ref())
    })
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

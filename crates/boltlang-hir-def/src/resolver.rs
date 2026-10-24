//! Resolving paths, like rust-analyzer's `Resolver`: a stack of scopes,
//! from the file's names, through the generic parameters of the items
//! around a position, to the local variables of a body.
//!
//! The primitive types, like `i32`, are found when nothing in scope has
//! their name.
//!
//! In an expression, `Color.Red` and `module.f` are parsed as field
//! accesses, since that's what `point.x` is. The type checker resolves them:
//! when the base of `Field { expr: Path(p), name }` isn't a value, it
//! resolves `p.name` as a path instead.

use boltlang_base_db::{Db, File};

use crate::{
    Name,
    builtin_type::BuiltinType,
    body::{DefWithBody, DefWithBodyId},
    hir::{BindingId, ExprId, Path, PathSegment, TypeRef},
    item_tree::{AssocItem, FunctionName, item_tree},
    ids::{
        ClassId, ConstId, EnumId, EnumVariantId, FunctionId, ImplId, ItemContainerId, ModuleDefId, StructId,
        TypeAliasId,
    },
    nameres::{Namespace, file_scope, is_visible_from},
    scope::{ExprScopes, ScopeId, expr_scopes},
    signatures::{
        GenericParams, class_signature, enum_signature, function_signature, impl_signature, struct_signature,
        type_alias_signature,
    },
};

/// An item that can have generic parameters. It can be the key of a query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue, salsa::Supertype)]
pub enum GenericDefId<'db> {
    Function(FunctionId<'db>),
    Struct(StructId<'db>),
    Enum(EnumId<'db>),
    Class(ClassId<'db>),
    Impl(ImplId<'db>),
    TypeAlias(TypeAliasId<'db>),
}

impl<'db> GenericDefId<'db> {
    pub fn generic_params(self, db: &'db dyn Db) -> &'db GenericParams {
        match self {
            GenericDefId::Function(id) => &function_signature(db, id).generic_params,
            GenericDefId::Struct(id) => &struct_signature(db, id).generic_params,
            GenericDefId::Enum(id) => &enum_signature(db, id).generic_params,
            GenericDefId::Class(id) => &class_signature(db, id).generic_params,
            GenericDefId::Impl(id) => &impl_signature(db, id).generic_params,
            GenericDefId::TypeAlias(id) => &type_alias_signature(db, id).generic_params,
        }
    }
}

/// What a path in a type refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TypeNs<'db> {
    Struct(StructId<'db>),
    Enum(EnumId<'db>),
    /// A variant, as in the struct literal `Shape.Circle { radius: 1 }`.
    Variant(EnumVariantId<'db>),
    Class(ClassId<'db>),
    TypeAlias(TypeAliasId<'db>),
    /// The `index`th type parameter of `def`.
    GenericParam { def: GenericDefId<'db>, index: u32 },
    Builtin(BuiltinType),
}

/// What a path in an expression refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValueNs<'db> {
    Local(BindingId),
    Function(FunctionId<'db>),
    Const(ConstId<'db>),
    /// A variant used as a value or constructor, like `None` or `Some`.
    Variant(EnumVariantId<'db>),
    /// The `index`th const parameter of `def`.
    ConstParam { def: GenericDefId<'db>, index: u32 },
}

/// A resolved type path. `unresolved` is the index of the first segment
/// that wasn't resolved, if any, as in `T.Item` or `Vec.new`: those name
/// associated items, which the type checker finds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeResolution<'db> {
    pub ty: TypeNs<'db>,
    pub unresolved: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueResolution<'db> {
    Value(ValueNs<'db>),
    /// The path names a type, or an associated item of a type. Calling a
    /// tuple struct like `Pair(1, 2)` resolves to the struct this way.
    Type(TypeResolution<'db>),
}

#[derive(Debug, Clone)]
enum Scope<'db> {
    Generics(GenericDefId<'db>, &'db GenericParams),
    Expr { scopes: &'db ExprScopes, scope: ScopeId },
}

#[derive(Debug, Clone)]
pub struct Resolver<'db> {
    file: File,
    /// Innermost last.
    scopes: Vec<Scope<'db>>,
}

impl<'db> Resolver<'db> {
    /// A resolver for the top level of a file.
    pub fn for_file(file: File) -> Self {
        Resolver { file, scopes: Vec::new() }
    }

    /// A resolver for the signature of an item: the generic parameters of
    /// the item and of the class or impl it's in are in scope.
    pub fn for_generic_def(db: &'db dyn Db, def: GenericDefId<'db>) -> Self {
        let container = match def {
            GenericDefId::Function(id) => Some(id.container(db)),
            GenericDefId::TypeAlias(id) => Some(id.container(db)),
            _ => None,
        };
        let mut resolver = match container {
            Some(container) => Self::for_container(db, container),
            None => Self::for_file(file_of(db, def)),
        };
        resolver.push_generics(db, def);
        resolver
    }

    /// A resolver for the items of a class or impl, or of a file.
    pub fn for_container(db: &'db dyn Db, container: ItemContainerId<'db>) -> Self {
        let mut resolver = Self::for_file(container.file(db));
        match container {
            ItemContainerId::Class(id) => resolver.push_generics(db, GenericDefId::Class(id)),
            ItemContainerId::Impl(id) => resolver.push_generics(db, GenericDefId::Impl(id)),
            ItemContainerId::File(_) | ItemContainerId::ExternBlock(_) => {}
        }
        resolver
    }

    /// A resolver for the start of a body, with its parameters in scope.
    pub fn for_body(db: &'db dyn Db, def: DefWithBodyId<'db>) -> Self {
        let mut resolver = match def.def(db) {
            DefWithBody::Function(id) => Self::for_generic_def(db, GenericDefId::Function(id)),
            DefWithBody::Const(id) => Self::for_container(db, id.container(db)),
            DefWithBody::Variant(id) => Self::for_generic_def(db, GenericDefId::Enum(id.parent)),
            DefWithBody::File(file) => Self::for_file(file),
        };
        let scopes = expr_scopes(db, def);
        resolver.scopes.push(Scope::Expr { scopes, scope: scopes.root() });
        resolver
    }

    /// A resolver for an expression in a body, with the local variables
    /// visible there in scope.
    pub fn for_expr(db: &'db dyn Db, def: DefWithBodyId<'db>, expr: ExprId) -> Self {
        let mut resolver = Self::for_body(db, def);
        if let Some(Scope::Expr { scopes, scope }) = resolver.scopes.last_mut()
            && let Some(inner) = scopes.scope_for(expr)
        {
            *scope = inner;
        }
        resolver
    }

    pub fn file(&self) -> File {
        self.file
    }

    fn push_generics(&mut self, db: &'db dyn Db, def: GenericDefId<'db>) {
        self.scopes.push(Scope::Generics(def, def.generic_params(db)));
    }

    /// The type parameter or local value a single name refers to.
    fn resolve_local(&self, ns: Namespace, name: &Name) -> Option<Local<'db>> {
        for scope in self.scopes.iter().rev() {
            match (scope, ns) {
                (Scope::Expr { scopes, scope }, Namespace::Values) => {
                    if let Some(binding) = scopes.resolve_name(*scope, name) {
                        return Some(Local::Value(ValueNs::Local(binding)));
                    }
                }
                (Scope::Generics(def, params), Namespace::Types) => {
                    if let Some(index) = params.type_params.iter().position(|p| &p.name == name) {
                        return Some(Local::Type(TypeNs::GenericParam { def: *def, index: index as u32 }));
                    }
                }
                (Scope::Generics(def, params), Namespace::Values) => {
                    if let Some(index) = params.const_params.iter().position(|p| &p.name == name) {
                        return Some(Local::Value(ValueNs::ConstParam { def: *def, index: index as u32 }));
                    }
                }
                _ => {}
            }
        }
        None
    }

    pub fn resolve_path_in_type_ns(&self, db: &'db dyn Db, path: &Path) -> Option<TypeResolution<'db>> {
        let segments = &path.segments;
        let first = segments.first()?;
        if let Some(Local::Type(ty)) = self.resolve_local(Namespace::Types, &first.name) {
            return Some(TypeResolution { ty, unresolved: rest(segments, 1) });
        }
        let (file, i) = self.walk_files(db, segments);
        let Some(def) = lookup(db, file, self.file, Namespace::Types, &segments.get(i)?.name) else {
            // `i32`, or `u8.MAX`, unless it's in another file, like `m.i32`.
            let builtin = if i == 0 { BuiltinType::by_name(first.name.as_str()) } else { None };
            return builtin.map(|ty| TypeResolution { ty: TypeNs::Builtin(ty), unresolved: rest(segments, 1) });
        };
        let ty = match def {
            ModuleDefId::Struct(id) => TypeNs::Struct(id),
            ModuleDefId::Enum(id) => {
                if let Some(variant) = segments.get(i + 1).and_then(|s| id.variant(db, &s.name)) {
                    return Some(TypeResolution { ty: TypeNs::Variant(variant), unresolved: rest(segments, i + 2) });
                }
                TypeNs::Enum(id)
            }
            ModuleDefId::Variant(id) => TypeNs::Variant(id),
            ModuleDefId::Class(id) => TypeNs::Class(id),
            ModuleDefId::TypeAlias(id) => TypeNs::TypeAlias(id),
            ModuleDefId::File(_)
            | ModuleDefId::Function(_)
            | ModuleDefId::Const(_)
            | ModuleDefId::MacroRules(_) => return None,
        };
        Some(TypeResolution { ty, unresolved: rest(segments, i + 1) })
    }

    pub fn resolve_path_in_value_ns(&self, db: &'db dyn Db, path: &Path) -> Option<ValueResolution<'db>> {
        let segments = &path.segments;
        let first = segments.first()?;
        if segments.len() == 1
            && let Some(Local::Value(value)) = self.resolve_local(Namespace::Values, &first.name)
        {
            return Some(ValueResolution::Value(value));
        }
        // In an impl, the methods and constants of its class, so that a
        // method can call itself or the others without importing them.
        if segments.len() == 1
            && let Some(value) = self.resolve_impl_class_item(db, &first.name)
        {
            return Some(ValueResolution::Value(value));
        }
        // A value directly in this file, or in an imported one: `f`, `m.f`.
        let (file, i) = self.walk_files(db, segments);
        if i + 1 == segments.len() {
            let value = lookup(db, file, self.file, Namespace::Values, &segments[i].name).and_then(|def| match def {
                ModuleDefId::Function(id) => Some(ValueNs::Function(id)),
                ModuleDefId::Const(id) => Some(ValueNs::Const(id)),
                ModuleDefId::Variant(id) => Some(ValueNs::Variant(id)),
                _ => None,
            });
            if let Some(value) = value {
                return Some(ValueResolution::Value(value));
            }
        }
        // Otherwise a variant, like `Option.None`, or something of a type.
        let resolution = self.resolve_path_in_type_ns(db, path)?;
        Some(match resolution {
            TypeResolution { ty: TypeNs::Variant(id), unresolved: None } => ValueResolution::Value(ValueNs::Variant(id)),
            resolution => ValueResolution::Type(resolution),
        })
    }

    /// The method or constant `name` of the class of the innermost impl
    /// around this position, like `concat` in `impl Semigroup[List[T]]`.
    fn resolve_impl_class_item(&self, db: &'db dyn Db, name: &Name) -> Option<ValueNs<'db>> {
        let imp = self.scopes.iter().rev().find_map(|scope| match scope {
            Scope::Generics(GenericDefId::Impl(id), _) => Some(*id),
            _ => None,
        })?;
        let class = impl_class(db, imp)?;
        let container = ItemContainerId::Class(class);
        let tree = item_tree(db, class.file(db));
        class.item(db).items.iter().find_map(|item| match *item {
            AssocItem::Function(idx) => {
                let f = &tree.functions[idx.index()];
                matches!(&f.name, Some(FunctionName::Name(n)) if n == name)
                    .then(|| ValueNs::Function(FunctionId::new(db, container, idx)))
            }
            AssocItem::Const(idx) => (tree.consts[idx.index()].name.as_ref() == Some(name))
                .then(|| ValueNs::Const(ConstId::new(db, container, idx))),
            AssocItem::TypeAlias(_) | AssocItem::MacroCall(_) => None,
        })
    }

    /// The local variable a name refers to at this position.
    pub fn resolve_local_binding(&self, name: &Name) -> Option<BindingId> {
        match self.resolve_local(Namespace::Values, name)? {
            Local::Value(ValueNs::Local(binding)) => Some(binding),
            _ => None,
        }
    }

    /// Follow the leading segments of a path that name imported files, like
    /// `m` in `m.f`. Returns the file the rest of the path is in, and the
    /// index of the first segment after them.
    /// The last segment is never followed, since it's what's being looked
    /// up.
    fn walk_files(&self, db: &'db dyn Db, segments: &[PathSegment]) -> (File, usize) {
        let (mut file, mut i) = (self.file, 0);
        // A type parameter shadows a file with the same name.
        if self.resolve_local(Namespace::Types, &segments[0].name).is_some() {
            return (file, i);
        }
        while i + 1 < segments.len() {
            match lookup(db, file, self.file, Namespace::Types, &segments[i].name) {
                Some(ModuleDefId::File(next)) => {
                    file = next;
                    i += 1;
                }
                _ => break,
            }
        }
        (file, i)
    }
}

enum Local<'db> {
    Type(TypeNs<'db>),
    Value(ValueNs<'db>),
}

/// Look up a name in a file, for a path in `from`. From another file, only
/// the names visible there can be used: see [`is_visible_from`]. An
/// imported name has the visibility of its import, so `m.f` only sees the
/// names `m` defines or re-exports.
fn lookup<'db>(db: &'db dyn Db, file: File, from: File, ns: Namespace, name: &Name) -> Option<ModuleDefId<'db>> {
    let entry = file_scope(db, file).scope.get(ns, name)?;
    is_visible_from(db, entry.visibility, file, from).then_some(entry.def)
}

/// The class an impl is an instance of, like `Show` for `impl Show[Dog]`.
pub fn impl_class<'db>(db: &'db dyn Db, id: ImplId<'db>) -> Option<ClassId<'db>> {
    let sig = impl_signature(db, id);
    let TypeRef::Path(path) = &sig.store[sig.class] else { return None };
    match Resolver::for_generic_def(db, GenericDefId::Impl(id)).resolve_path_in_type_ns(db, path)? {
        TypeResolution { ty: TypeNs::Class(class), unresolved: None } => Some(class),
        _ => None,
    }
}

fn rest(segments: &[PathSegment], index: usize) -> Option<usize> {
    (index < segments.len()).then_some(index)
}

fn file_of<'db>(db: &'db dyn Db, def: GenericDefId<'db>) -> File {
    match def {
        GenericDefId::Function(id) => id.file(db),
        GenericDefId::Struct(id) => id.file(db),
        GenericDefId::Enum(id) => id.file(db),
        GenericDefId::Class(id) => id.file(db),
        GenericDefId::Impl(id) => id.file(db),
        GenericDefId::TypeAlias(id) => id.file(db),
    }
}

#[cfg(test)]
mod tests;

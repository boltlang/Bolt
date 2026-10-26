//! Kinds: the types of types. `i32` and `Option[i32]` are of kind `Type`,
//! `Option` is of kind `Type -> Type`, and the `F` of `class Functor[F]` too,
//! since the class applies it to arguments, as in `F[A]`.
//!
//! The kinds of type parameters aren't written: they're inferred from how
//! the parameters are used, as in Haskell 98. Parameters used in the same
//! definitions are inferred together, in a *group*: a class and its items,
//! an impl and its items, or a single struct, enum, type alias or function.
//! Kinds that nothing decides are `Type`, so there's no kind polymorphism.
//!
//! Inference doesn't report errors. When the uses of a parameter disagree,
//! the first one wins, and [lowering](crate::lower) reports the others,
//! since it checks the kind of every type it lowers.

use std::fmt;

use boltlang_base_db::Db;
use boltlang_hir_def::{
    expr_store::ExpressionStore,
    hir::{GenericArg, TypeRef, TypeRefId},
    ids::{ConstId, FunctionId, ItemContainerId, TypeAliasId},
    item_tree::AssocItem,
    resolver::{GenericDefId, Resolver, TypeNs},
    signatures::{
        FieldList, GenericParams, class_signature, const_signature, enum_signature, function_signature,
        impl_signature, struct_signature, type_alias_signature,
    },
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum Kind {
    /// The kind of types that values have, like `i32` or `Option[i32]`.
    Type,
    /// A type constructor, which gives a type of the second kind when it's
    /// applied to a type of the first: `Option` is `Type -> Type`.
    Arrow(Box<Kind>, Box<Kind>),
}

impl Kind {
    /// `k1 -> ... -> kn -> result`
    pub fn arrows(params: impl DoubleEndedIterator<Item = Kind>, result: Kind) -> Kind {
        params.rev().fold(result, |acc, k| Kind::Arrow(Box::new(k), Box::new(acc)))
    }

    /// The kinds of the arguments it takes, and the kind it then gives.
    pub fn split(&self) -> (Vec<&Kind>, &Kind) {
        let mut params = Vec::new();
        let mut kind = self;
        while let Kind::Arrow(param, result) = kind {
            params.push(&**param);
            kind = result;
        }
        (params, kind)
    }

    /// The kind after applying `count` arguments, if it takes that many.
    pub fn apply(&self, count: usize) -> Option<&Kind> {
        let mut kind = self;
        for _ in 0..count {
            let Kind::Arrow(_, result) = kind else { return None };
            kind = result;
        }
        Some(kind)
    }

    /// Whether all the arguments it takes are types, like `Type -> Type`,
    /// as opposed to `(Type -> Type) -> Type`.
    pub fn is_first_order(&self) -> bool {
        self.split().0.iter().all(|k| **k == Kind::Type)
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Kind::Type => f.write_str("Type"),
            Kind::Arrow(param, result) if matches!(**param, Kind::Arrow(..)) => write!(f, "({param}) -> {result}"),
            Kind::Arrow(param, result) => write!(f, "{param} -> {result}"),
        }
    }
}

/// The kinds of the type parameters of `def`.
pub fn generic_param_kinds<'db>(db: &'db dyn Db, def: GenericDefId<'db>) -> Box<[Kind]> {
    let count = def.generic_params(db).type_params.len();
    let kinds = group_kinds(db, group_root(db, def)).iter().find(|(d, _)| *d == def).map(|(_, k)| k.clone());
    kinds.filter(|k| k.len() == count).unwrap_or_else(|| vec![Kind::Type; count].into())
}

/// The kind of a struct, enum, class or type alias: that of a constructor
/// taking its type parameters, like `Type -> Type` for `Option`.
pub fn def_kind<'db>(db: &'db dyn Db, def: GenericDefId<'db>) -> Kind {
    Kind::arrows(generic_param_kinds(db, def).into_vec().into_iter(), Kind::Type)
}

/// The definition whose group `def` is inferred in: its class or impl for
/// an item in one, itself otherwise.
fn group_root<'db>(db: &'db dyn Db, def: GenericDefId<'db>) -> GenericDefId<'db> {
    let container = match def {
        GenericDefId::Function(id) => id.container(db),
        GenericDefId::TypeAlias(id) => id.container(db),
        _ => return def,
    };
    match container {
        ItemContainerId::Class(id) => GenericDefId::Class(id),
        ItemContainerId::Impl(id) => GenericDefId::Impl(id),
        ItemContainerId::File(_) | ItemContainerId::ExternBlock(_) => def,
    }
}

/// The definitions of a group, which have type parameters.
fn group_members<'db>(db: &'db dyn Db, root: GenericDefId<'db>) -> Vec<GenericDefId<'db>> {
    let mut members = vec![root];
    let (container, items) = match root {
        GenericDefId::Class(id) => (ItemContainerId::Class(id), &id.item(db).items),
        GenericDefId::Impl(id) => (ItemContainerId::Impl(id), &id.item(db).items),
        _ => return members,
    };
    for item in items {
        match *item {
            AssocItem::Function(idx) => members.push(GenericDefId::Function(FunctionId::new(db, container, idx))),
            AssocItem::TypeAlias(idx) => members.push(GenericDefId::TypeAlias(TypeAliasId::new(db, container, idx))),
            AssocItem::Const(_) | AssocItem::MacroCall(_) => {}
        }
    }
    members
}

/// The kinds of the type parameters of each definition of the group of
/// `root`.
#[salsa::tracked(returns(ref), cycle_result = group_kinds_cycle)]
fn group_kinds<'db>(db: &'db dyn Db, root: GenericDefId<'db>) -> Vec<(GenericDefId<'db>, Box<[Kind]>)> {
    let mut inference = KindInference { db, vars: Vec::new(), members: Vec::new() };
    for def in group_members(db, root) {
        let count = def.generic_params(db).type_params.len();
        let vars = (0..count).map(|_| inference.new_var()).collect();
        inference.members.push((def, vars));
    }
    for def in group_members(db, root) {
        inference.walk_def(def);
    }
    if let GenericDefId::Class(id) = root {
        inference.walk_consts(ItemContainerId::Class(id), &id.item(db).items);
    }
    if let GenericDefId::Impl(id) = root {
        inference.walk_consts(ItemContainerId::Impl(id), &id.item(db).items);
    }
    let members = std::mem::take(&mut inference.members);
    members.into_iter().map(|(def, vars)| (def, vars.iter().map(|k| inference.finish(k)).collect())).collect()
}

/// When definitions of different groups refer to each other, like two
/// structs with fields of each other's type, their parameters are `Type`.
fn group_kinds_cycle<'db>(
    db: &'db dyn Db,
    _id: salsa::Id,
    root: GenericDefId<'db>,
) -> Vec<(GenericDefId<'db>, Box<[Kind]>)> {
    group_members(db, root)
        .into_iter()
        .map(|def| (def, vec![Kind::Type; def.generic_params(db).type_params.len()].into()))
        .collect()
}

/// A kind during inference.
#[derive(Debug, Clone, PartialEq, Eq)]
enum K {
    Type,
    Arrow(Box<K>, Box<K>),
    Var(u32),
}

impl K {
    /// `k1 -> ... -> kn -> result`, like [`Kind::arrows`].
    fn arrows(params: Vec<K>, result: K) -> K {
        params.into_iter().rev().fold(result, |acc, k| K::Arrow(Box::new(k), Box::new(acc)))
    }

    fn from_kind(kind: &Kind) -> K {
        match kind {
            Kind::Type => K::Type,
            Kind::Arrow(a, b) => K::Arrow(Box::new(K::from_kind(a)), Box::new(K::from_kind(b))),
        }
    }
}

struct KindInference<'db> {
    db: &'db dyn Db,
    /// What each kind variable is bound to.
    vars: Vec<Option<K>>,
    /// The definitions of the group, with the kinds of their parameters.
    members: Vec<(GenericDefId<'db>, Vec<K>)>,
}

impl<'db> KindInference<'db> {
    fn new_var(&mut self) -> K {
        self.vars.push(None);
        K::Var(self.vars.len() as u32 - 1)
    }

    fn shallow(&self, k: &K) -> K {
        let mut k = k.clone();
        while let K::Var(v) = k {
            match &self.vars[v as usize] {
                Some(bound) => k = bound.clone(),
                None => break,
            }
        }
        k
    }

    fn occurs(&self, v: u32, k: &K) -> bool {
        match self.shallow(k) {
            K::Var(w) => v == w,
            K::Arrow(a, b) => self.occurs(v, &a) || self.occurs(v, &b),
            K::Type => false,
        }
    }

    /// Make two kinds equal if they can be. A mismatch is left for lowering
    /// to report.
    fn unify(&mut self, a: &K, b: &K) {
        match (self.shallow(a), self.shallow(b)) {
            (K::Var(x), K::Var(y)) if x == y => {}
            (K::Var(v), k) | (k, K::Var(v)) => {
                if !self.occurs(v, &k) {
                    self.vars[v as usize] = Some(k);
                }
            }
            (K::Arrow(a1, b1), K::Arrow(a2, b2)) => {
                self.unify(&a1, &a2);
                self.unify(&b1, &b2);
            }
            _ => {}
        }
    }

    /// The final kind, with `Type` for what nothing decided.
    fn finish(&self, k: &K) -> Kind {
        match self.shallow(k) {
            K::Type | K::Var(_) => Kind::Type,
            K::Arrow(a, b) => Kind::Arrow(Box::new(self.finish(&a)), Box::new(self.finish(&b))),
        }
    }

    fn member_params(&self, def: GenericDefId<'db>) -> Option<&Vec<K>> {
        self.members.iter().find(|(d, _)| *d == def).map(|(_, vars)| vars)
    }

    fn param_kind(&self, def: GenericDefId<'db>, index: u32) -> K {
        match self.member_params(def) {
            Some(vars) => vars.get(index as usize).cloned().unwrap_or(K::Type),
            None => generic_param_kinds(self.db, def).get(index as usize).map_or(K::Type, K::from_kind),
        }
    }

    /// The kind of a constructor taking the parameters of `def`.
    fn def_kind(&self, def: GenericDefId<'db>) -> K {
        let params: Vec<K> = match self.member_params(def) {
            Some(vars) => vars.clone(),
            None => generic_param_kinds(self.db, def).iter().map(K::from_kind).collect(),
        };
        K::arrows(params, K::Type)
    }

    fn walk_def(&mut self, def: GenericDefId<'db>) {
        let db = self.db;
        let resolver = Resolver::for_generic_def(db, def);
        match def {
            GenericDefId::Function(id) => {
                let sig = function_signature(db, id);
                let types = sig.params.iter().filter_map(|p| p.type_ref).chain(sig.ret_type);
                for type_ref in types {
                    self.expect_type(&resolver, &sig.store, type_ref);
                }
                self.walk_generics(def, &resolver, &sig.store, &sig.generic_params);
            }
            GenericDefId::Struct(id) => {
                let sig = struct_signature(db, id);
                self.walk_fields(&resolver, &sig.store, &sig.fields);
                self.walk_generics(def, &resolver, &sig.store, &sig.generic_params);
            }
            GenericDefId::Enum(id) => {
                let sig = enum_signature(db, id);
                for variant in sig.variants.iter() {
                    self.walk_fields(&resolver, &sig.store, &variant.fields);
                }
                self.walk_generics(def, &resolver, &sig.store, &sig.generic_params);
            }
            GenericDefId::Class(id) => {
                let sig = class_signature(db, id);
                self.walk_generics(def, &resolver, &sig.store, &sig.generic_params);
            }
            GenericDefId::Impl(id) => {
                let sig = impl_signature(db, id);
                self.class_constraint(&resolver, &sig.store, sig.class, None);
                self.walk_generics(def, &resolver, &sig.store, &sig.generic_params);
            }
            GenericDefId::TypeAlias(id) => {
                let sig = type_alias_signature(db, id);
                if let Some(type_ref) = sig.type_ref {
                    self.expect_type(&resolver, &sig.store, type_ref);
                }
                self.walk_generics(def, &resolver, &sig.store, &sig.generic_params);
            }
        }
    }

    /// The types of the consts of a class or impl.
    fn walk_consts(&mut self, container: ItemContainerId<'db>, items: &[AssocItem]) {
        let resolver = Resolver::for_container(self.db, container);
        for item in items {
            if let AssocItem::Const(idx) = *item {
                let sig = const_signature(self.db, ConstId::new(self.db, container, idx));
                self.expect_type(&resolver, &sig.store, sig.type_ref);
            }
        }
    }

    fn walk_fields(&mut self, resolver: &Resolver<'db>, store: &ExpressionStore, fields: &FieldList) {
        let fields = match fields {
            FieldList::Record(fields) | FieldList::Tuple(fields) => &fields[..],
            FieldList::Unit => &[],
        };
        for field in fields {
            self.expect_type(resolver, store, field.type_ref);
        }
    }

    /// The bounds of the parameters, like `[T: Show]`, and `where` clauses.
    fn walk_generics(
        &mut self,
        def: GenericDefId<'db>,
        resolver: &Resolver<'db>,
        store: &ExpressionStore,
        params: &GenericParams,
    ) {
        for (index, param) in params.type_params.iter().enumerate() {
            let target = self.param_kind(def, index as u32);
            for &bound in param.bounds.iter() {
                self.class_constraint(resolver, store, bound, Some(target.clone()));
            }
        }
        for pred in params.where_predicates.iter() {
            if pred.bounds.is_empty() {
                self.class_constraint(resolver, store, pred.target, None);
            } else {
                let target = self.kind_of(resolver, store, pred.target);
                for &bound in pred.bounds.iter() {
                    self.class_constraint(resolver, store, bound, Some(target.clone()));
                }
            }
        }
    }

    /// A type that values have, like that of a parameter or field.
    fn expect_type(&mut self, resolver: &Resolver<'db>, store: &ExpressionStore, id: TypeRefId) {
        let kind = self.kind_of(resolver, store, id);
        self.unify(&kind, &K::Type);
    }

    fn kind_of(&mut self, resolver: &Resolver<'db>, store: &ExpressionStore, id: TypeRefId) -> K {
        match &store[id] {
            TypeRef::Path(path) => {
                let head = match resolver.resolve_path_in_type_ns(self.db, path) {
                    Some(r) if r.unresolved.is_none() => match r.ty {
                        TypeNs::GenericParam { def, index } => self.param_kind(def, index),
                        TypeNs::Struct(s) => self.def_kind(GenericDefId::Struct(s)),
                        TypeNs::Enum(e) => self.def_kind(GenericDefId::Enum(e)),
                        TypeNs::TypeAlias(a) => self.def_kind(GenericDefId::TypeAlias(a)),
                        TypeNs::Builtin(_) => K::Type,
                        TypeNs::Class(_) | TypeNs::Variant(_) => self.new_var(),
                    },
                    // An associated type like `T.Item`, or an unresolved
                    // path, which lowering reports.
                    _ => self.new_var(),
                };
                let mut arg_kinds = Vec::new();
                for arg in path.last_generic_args() {
                    if let GenericArg::Type { type_ref, .. } = arg {
                        arg_kinds.push(self.kind_of(resolver, store, *type_ref));
                    }
                }
                if arg_kinds.is_empty() {
                    return head;
                }
                let result = self.new_var();
                self.unify(&head, &K::arrows(arg_kinds, result.clone()));
                result
            }
            TypeRef::Tuple(tys) => {
                for &ty in tys.iter() {
                    self.expect_type(resolver, store, ty);
                }
                K::Type
            }
            TypeRef::Array { element, .. } => {
                self.expect_type(resolver, store, *element);
                K::Type
            }
            TypeRef::Fn { params, ret_type, .. } => {
                for &(_, ty) in params.iter() {
                    self.expect_type(resolver, store, ty);
                }
                if let Some(ret) = ret_type {
                    self.expect_type(resolver, store, *ret);
                }
                K::Type
            }
            TypeRef::Never => K::Type,
            TypeRef::Error | TypeRef::ImplTrait { .. } | TypeRef::Bounded(_) | TypeRef::MacroCall { .. } => {
                self.new_var()
            }
        }
    }

    /// A class constraint like `Functor[F]`, whose arguments have the kinds
    /// of the class's parameters. `target` is the kind of the type before
    /// the `:` of a bound, which is the first argument.
    fn class_constraint(&mut self, resolver: &Resolver<'db>, store: &ExpressionStore, id: TypeRefId, target: Option<K>) {
        let TypeRef::Path(path) = &store[id] else { return };
        let Some(r) = resolver.resolve_path_in_type_ns(self.db, path) else { return };
        let (TypeNs::Class(class), None) = (r.ty, r.unresolved) else { return };
        let def = GenericDefId::Class(class);
        let count = def.generic_params(self.db).type_params.len();
        let params: Vec<K> = (0..count as u32).map(|i| self.param_kind(def, i)).collect();
        let mut params = params.into_iter();
        if let Some(target) = target
            && let Some(param) = params.next()
        {
            self.unify(&target, &param);
        }
        for (arg, param) in path.last_generic_args().iter().zip(params) {
            if let GenericArg::Type { type_ref, .. } = arg {
                let kind = self.kind_of(resolver, store, *type_ref);
                self.unify(&kind, &param);
            }
        }
    }
}

#[cfg(test)]
mod tests;

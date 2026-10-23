//! Turning type references, as written, into [`Ty`]s, and the signatures
//! of items into the types the checker uses.

use boltlang_base_db::Db;
use boltlang_hir_def::{
    Name,
    expr_store::ExpressionStore,
    hir::{GenericArg, Path, TypeRef, TypeRefId},
    ids::{ClassId, ConstId, EnumId, EnumVariantId, FunctionId, ImplId, StructId, TypeAliasId},
    resolver::{GenericDefId, Resolver, TypeNs},
    signatures::{
        self, FieldList, GenericParams, class_signature, const_signature, enum_signature, function_signature,
        impl_signature, struct_signature, type_alias_signature,
    },
};

use crate::ty::{AdtId, GenericParamId, Ty};

/// A class applied to types, like `Show[T]` or `Convert[A, B]`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub struct ClassConstraint<'db> {
    pub class: ClassId<'db>,
    pub args: Box<[Ty<'db>]>,
}

impl<'db> ClassConstraint<'db> {
    pub fn map(&self, mut f: impl FnMut(&Ty<'db>) -> Ty<'db>) -> Self {
        ClassConstraint { class: self.class, args: self.args.iter().map(&mut f).collect() }
    }
}

/// A problem with a type reference, in the store of the signature it's in.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub enum LowerDiagnostic {
    UnresolvedType(TypeRefId),
    /// A path that names something other than a type, like a class.
    NotAType(TypeRefId),
    /// A bound or `where` predicate that isn't a class.
    NotAClass(TypeRefId),
    WrongArgCount { type_ref: TypeRefId, expected: usize, found: usize },
    /// A type alias that refers to itself.
    RecursiveAlias(TypeRefId),
    /// Something the checker doesn't support yet, like `impl Trait` or
    /// associated types.
    Unsupported(TypeRefId),
}

pub struct TyLoweringContext<'a, 'db> {
    db: &'db dyn Db,
    resolver: &'a Resolver<'db>,
    store: &'a ExpressionStore,
    pub diagnostics: Vec<LowerDiagnostic>,
}

impl<'a, 'db> TyLoweringContext<'a, 'db> {
    pub fn new(db: &'db dyn Db, resolver: &'a Resolver<'db>, store: &'a ExpressionStore) -> Self {
        TyLoweringContext { db, resolver, store, diagnostics: Vec::new() }
    }

    pub fn lower_ty(&mut self, id: TypeRefId) -> Ty<'db> {
        match &self.store[id] {
            TypeRef::Error => Ty::Error,
            TypeRef::Path(path) => self.lower_path_ty(id, path),
            TypeRef::Tuple(tys) => Ty::Tuple(tys.iter().map(|t| self.lower_ty(*t)).collect()),
            TypeRef::Array { element, .. } => Ty::Array(Box::new(self.lower_ty(*element))),
            TypeRef::Never => Ty::Never,
            // `FnOnce(A) -> B` is a function type too, for now.
            TypeRef::Fn { params, ret_type, .. } => {
                let params = params.iter().map(|(_, t)| self.lower_ty(*t)).collect();
                let ret = ret_type.map_or_else(Ty::unit, |t| self.lower_ty(t));
                Ty::Fn(params, Box::new(ret))
            }
            TypeRef::ImplTrait { .. } | TypeRef::Bounded(_) | TypeRef::MacroCall { .. } => {
                self.diagnostics.push(LowerDiagnostic::Unsupported(id));
                Ty::Error
            }
        }
    }

    pub fn lower_opt_ty(&mut self, id: Option<TypeRefId>) -> Option<Ty<'db>> {
        id.map(|id| self.lower_ty(id))
    }

    fn lower_path_ty(&mut self, id: TypeRefId, path: &Path) -> Ty<'db> {
        let Some(resolution) = self.resolver.resolve_path_in_type_ns(self.db, path) else {
            self.diagnostics.push(LowerDiagnostic::UnresolvedType(id));
            return Ty::Error;
        };
        if resolution.unresolved.is_some() {
            // `T.Item` and other associated types.
            self.diagnostics.push(LowerDiagnostic::Unsupported(id));
            return Ty::Error;
        }
        let args = path.segments.last().and_then(|s| s.generic_args.as_deref());
        match resolution.ty {
            TypeNs::Struct(s) => {
                let count = struct_signature(self.db, s).generic_params.type_params.len();
                Ty::Adt(AdtId::Struct(s), self.lower_args(id, args, count))
            }
            TypeNs::Enum(e) => {
                let count = enum_signature(self.db, e).generic_params.type_params.len();
                Ty::Adt(AdtId::Enum(e), self.lower_args(id, args, count))
            }
            TypeNs::Builtin(ty) => {
                self.lower_args(id, args, 0);
                Ty::Builtin(ty)
            }
            TypeNs::GenericParam { def, index } => {
                self.lower_args(id, args, 0);
                Ty::Param(GenericParamId { def, index })
            }
            TypeNs::TypeAlias(alias) => {
                let count = type_alias_signature(self.db, alias).generic_params.type_params.len();
                let args = self.lower_args(id, args, count);
                match &type_alias_ty(self.db, alias).ty {
                    Some(ty) if *ty == Ty::Error && type_alias_ty(self.db, alias).recursive => {
                        self.diagnostics.push(LowerDiagnostic::RecursiveAlias(id));
                        Ty::Error
                    }
                    Some(ty) => subst_params(ty, GenericDefId::TypeAlias(alias), &args),
                    // An associated type without a definition.
                    None => {
                        self.diagnostics.push(LowerDiagnostic::Unsupported(id));
                        Ty::Error
                    }
                }
            }
            TypeNs::Class(_) | TypeNs::Variant(_) => {
                self.diagnostics.push(LowerDiagnostic::NotAType(id));
                Ty::Error
            }
        }
    }

    /// The type arguments of a path, padded with errors or cut to `count`.
    fn lower_args(&mut self, id: TypeRefId, args: Option<&[GenericArg]>, count: usize) -> Box<[Ty<'db>]> {
        let mut tys: Vec<Ty<'db>> = Vec::new();
        for arg in args.unwrap_or_default() {
            match arg {
                GenericArg::Type { type_ref, .. } => tys.push(self.lower_ty(*type_ref)),
                GenericArg::AssocType { .. } | GenericArg::Const(_) => {
                    self.diagnostics.push(LowerDiagnostic::Unsupported(id));
                }
            }
        }
        if tys.len() != count && !(tys.is_empty() && args.is_none() && count == 0) {
            self.diagnostics.push(LowerDiagnostic::WrongArgCount { type_ref: id, expected: count, found: tys.len() });
        }
        tys.resize(count, Ty::Error);
        tys.into()
    }

    /// Lower a class constraint like `Convert[A, B]`. With `target`, it's a
    /// bound like the `Show` of `T: Show`, and the target is the first
    /// argument.
    pub fn lower_class_constraint(&mut self, id: TypeRefId, target: Option<Ty<'db>>) -> Option<ClassConstraint<'db>> {
        let path = match &self.store[id] {
            TypeRef::Path(path) => path,
            // Already a syntax error.
            TypeRef::Error => return None,
            _ => {
                self.diagnostics.push(LowerDiagnostic::NotAClass(id));
                return None;
            }
        };
        let class = match self.resolver.resolve_path_in_type_ns(self.db, path) {
            Some(r) if r.unresolved.is_none() && matches!(r.ty, TypeNs::Class(_)) => {
                let TypeNs::Class(class) = r.ty else { unreachable!() };
                class
            }
            Some(_) => {
                self.diagnostics.push(LowerDiagnostic::NotAClass(id));
                return None;
            }
            None => {
                self.diagnostics.push(LowerDiagnostic::UnresolvedType(id));
                return None;
            }
        };
        let count = class_signature(self.db, class).generic_params.type_params.len();
        let given = path.segments.last().and_then(|s| s.generic_args.as_deref());
        let mut args: Vec<Ty<'db>> = target.into_iter().collect();
        let explicit = self.lower_args(id, given, count.saturating_sub(args.len()));
        args.extend(explicit);
        Some(ClassConstraint { class, args: args.into() })
    }

    /// The class constraints of generic parameters: the bounds in
    /// `[T: Show]` and the `where` predicates.
    pub fn lower_predicates(&mut self, def: GenericDefId<'db>, params: &GenericParams) -> Vec<ClassConstraint<'db>> {
        let mut out = Vec::new();
        for (index, param) in params.type_params.iter().enumerate() {
            let target = Ty::Param(GenericParamId { def, index: index as u32 });
            for &bound in param.bounds.iter() {
                out.extend(self.lower_class_constraint(bound, Some(target.clone())));
            }
        }
        for pred in params.where_predicates.iter() {
            if pred.bounds.is_empty() {
                // A bare constraint like `Convert[A, B]`.
                out.extend(self.lower_class_constraint(pred.target, None));
            } else {
                let target = self.lower_ty(pred.target);
                for &bound in pred.bounds.iter() {
                    out.extend(self.lower_class_constraint(bound, Some(target.clone())));
                }
            }
        }
        out
    }
}

/// Replace the type parameters of `def` with `args`.
pub fn subst_params<'db>(ty: &Ty<'db>, def: GenericDefId<'db>, args: &[Ty<'db>]) -> Ty<'db> {
    ty.fold(&mut |t| match t {
        Ty::Param(p) if p.def == def => Some(args.get(p.index as usize).cloned().unwrap_or(Ty::Error)),
        _ => None,
    })
}

/// The type parameters of `def` as types, as seen from inside it.
pub fn identity_args<'db>(db: &'db dyn Db, def: GenericDefId<'db>) -> Box<[Ty<'db>]> {
    (0..def.generic_params(db).type_params.len())
        .map(|index| Ty::Param(GenericParamId { def, index: index as u32 }))
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct TypeAliasTy<'db> {
    /// The aliased type, or `None` for an associated type without one.
    pub ty: Option<Ty<'db>>,
    /// Whether the alias refers to itself, in which case `ty` is an error.
    pub recursive: bool,
    pub diagnostics: Vec<LowerDiagnostic>,
}

/// The type an alias stands for, in terms of its own type parameters.
#[salsa::tracked(returns(ref), cycle_result = type_alias_cycle)]
pub fn type_alias_ty<'db>(db: &'db dyn Db, alias: TypeAliasId<'db>) -> TypeAliasTy<'db> {
    let sig = type_alias_signature(db, alias);
    let resolver = Resolver::for_generic_def(db, GenericDefId::TypeAlias(alias));
    let mut ctx = TyLoweringContext::new(db, &resolver, &sig.store);
    let ty = ctx.lower_opt_ty(sig.type_ref);
    TypeAliasTy { ty, recursive: false, diagnostics: ctx.diagnostics }
}

fn type_alias_cycle<'db>(_db: &'db dyn Db, _id: salsa::Id, _alias: TypeAliasId<'db>) -> TypeAliasTy<'db> {
    TypeAliasTy { ty: Some(Ty::Error), recursive: true, diagnostics: Vec::new() }
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct Predicates<'db> {
    pub predicates: Box<[ClassConstraint<'db>]>,
    pub diagnostics: Vec<LowerDiagnostic>,
}

/// The class constraints an item declares on its own generic parameters.
/// For a class, these are its superclasses.
#[salsa::tracked(returns(ref))]
pub fn generic_predicates<'db>(db: &'db dyn Db, def: GenericDefId<'db>) -> Predicates<'db> {
    let resolver = Resolver::for_generic_def(db, def);
    let (store, params) = match def {
        GenericDefId::Function(id) => {
            let sig = function_signature(db, id);
            (&sig.store, &sig.generic_params)
        }
        GenericDefId::Struct(id) => {
            let sig = struct_signature(db, id);
            (&sig.store, &sig.generic_params)
        }
        GenericDefId::Enum(id) => {
            let sig = enum_signature(db, id);
            (&sig.store, &sig.generic_params)
        }
        GenericDefId::Class(id) => {
            let sig = class_signature(db, id);
            (&sig.store, &sig.generic_params)
        }
        GenericDefId::Impl(id) => {
            let sig = impl_signature(db, id);
            (&sig.store, &sig.generic_params)
        }
        GenericDefId::TypeAlias(id) => {
            let sig = type_alias_signature(db, id);
            (&sig.store, &sig.generic_params)
        }
    };
    let mut ctx = TyLoweringContext::new(db, &resolver, store);
    let predicates = ctx.lower_predicates(def, params).into();
    Predicates { predicates, diagnostics: ctx.diagnostics }
}

/// The declared types of a function.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct FnSig<'db> {
    /// The type of each parameter; `None` where no type is written.
    pub params: Box<[Option<Ty<'db>>]>,
    /// The return type, if written. A function without a body and without
    /// a return type, like a class method, returns `()`.
    pub ret: Option<Ty<'db>>,
    pub diagnostics: Vec<LowerDiagnostic>,
}

impl FnSig<'_> {
    /// Whether every type is written, so that callers can use the
    /// signature without looking at the body.
    pub fn is_complete(&self) -> bool {
        self.ret.is_some() && self.params.iter().all(Option::is_some)
    }
}

#[salsa::tracked(returns(ref))]
pub fn fn_sig<'db>(db: &'db dyn Db, id: FunctionId<'db>) -> FnSig<'db> {
    let sig = function_signature(db, id);
    let resolver = Resolver::for_generic_def(db, GenericDefId::Function(id));
    let mut ctx = TyLoweringContext::new(db, &resolver, &sig.store);
    let params = sig.params.iter().map(|p| ctx.lower_opt_ty(p.type_ref)).collect();
    let ret = match sig.ret_type {
        Some(ret) => Some(ctx.lower_ty(ret)),
        None if !id.item(db).has_body => Some(Ty::unit()),
        None => None,
    };
    FnSig { params, ret, diagnostics: ctx.diagnostics }
}

/// A struct or a variant: something with fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum VariantDef<'db> {
    Struct(StructId<'db>),
    Variant(EnumVariantId<'db>),
}

impl<'db> VariantDef<'db> {
    /// The ADT and its generics, which the field types are in terms of.
    pub fn adt(self) -> (AdtId<'db>, GenericDefId<'db>) {
        match self {
            VariantDef::Struct(id) => (AdtId::Struct(id), GenericDefId::Struct(id)),
            VariantDef::Variant(id) => (AdtId::Enum(id.parent), GenericDefId::Enum(id.parent)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, salsa::SalsaValue)]
pub enum FieldsKind {
    Record,
    Tuple,
    Unit,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct FieldTypes<'db> {
    pub kind: FieldsKind,
    pub fields: Box<[(Name, Ty<'db>)]>,
    pub diagnostics: Vec<LowerDiagnostic>,
}

impl<'db> FieldTypes<'db> {
    pub fn field(&self, name: &Name) -> Option<(usize, &Ty<'db>)> {
        self.fields.iter().enumerate().find(|(_, (n, _))| n == name).map(|(i, (_, ty))| (i, ty))
    }
}

/// The types of the fields of a struct or variant, in terms of the type
/// parameters of the struct or enum.
pub fn field_types<'db>(db: &'db dyn Db, def: VariantDef<'db>) -> &'db FieldTypes<'db> {
    match def {
        VariantDef::Struct(id) => struct_field_types(db, id),
        VariantDef::Variant(id) => &enum_field_types(db, id.parent)[id.index as usize],
    }
}

#[salsa::tracked(returns(ref))]
pub fn struct_field_types<'db>(db: &'db dyn Db, id: StructId<'db>) -> FieldTypes<'db> {
    let sig = struct_signature(db, id);
    let resolver = Resolver::for_generic_def(db, GenericDefId::Struct(id));
    lower_fields(&mut TyLoweringContext::new(db, &resolver, &sig.store), &sig.fields)
}

/// The field types of each variant of an enum.
#[salsa::tracked(returns(ref))]
pub fn enum_field_types<'db>(db: &'db dyn Db, id: EnumId<'db>) -> Vec<FieldTypes<'db>> {
    let sig = enum_signature(db, id);
    let resolver = Resolver::for_generic_def(db, GenericDefId::Enum(id));
    let mut ctx = TyLoweringContext::new(db, &resolver, &sig.store);
    sig.variants.iter().map(|v| lower_fields(&mut ctx, &v.fields)).collect()
}

fn lower_fields<'db>(ctx: &mut TyLoweringContext<'_, 'db>, fields: &FieldList) -> FieldTypes<'db> {
    let (kind, list): (_, &[signatures::FieldData]) = match fields {
        FieldList::Record(fields) => (FieldsKind::Record, fields),
        FieldList::Tuple(fields) => (FieldsKind::Tuple, fields),
        FieldList::Unit => (FieldsKind::Unit, &[]),
    };
    let fields = list.iter().map(|f| (f.name.clone(), ctx.lower_ty(f.type_ref))).collect();
    FieldTypes { kind, fields, diagnostics: std::mem::take(&mut ctx.diagnostics) }
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct ImplHeader<'db> {
    /// The instance, like `Show[Dog]`. `None` if it's not a class.
    pub class: Option<ClassConstraint<'db>>,
    pub diagnostics: Vec<LowerDiagnostic>,
}

/// The instance an impl is for, in terms of its own type parameters.
#[salsa::tracked(returns(ref))]
pub fn impl_header<'db>(db: &'db dyn Db, id: ImplId<'db>) -> ImplHeader<'db> {
    let sig = impl_signature(db, id);
    let resolver = Resolver::for_generic_def(db, GenericDefId::Impl(id));
    let mut ctx = TyLoweringContext::new(db, &resolver, &sig.store);
    let class = ctx.lower_class_constraint(sig.class, None);
    ImplHeader { class, diagnostics: ctx.diagnostics }
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct ConstTy<'db> {
    pub ty: Ty<'db>,
    pub diagnostics: Vec<LowerDiagnostic>,
}

#[salsa::tracked(returns(ref))]
pub fn const_ty<'db>(db: &'db dyn Db, id: ConstId<'db>) -> ConstTy<'db> {
    let sig = const_signature(db, id);
    let resolver = Resolver::for_container(db, id.container(db));
    let mut ctx = TyLoweringContext::new(db, &resolver, &sig.store);
    let ty = ctx.lower_ty(sig.type_ref);
    ConstTy { ty, diagnostics: ctx.diagnostics }
}

#[cfg(test)]
mod tests;

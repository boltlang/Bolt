//! Turning type references, as written, into [`Ty`]s, and the signatures
//! of items into the types the checker uses.

use boltlang_base_db::Db;
use boltlang_hir_def::{
    Name,
    expr_store::ExpressionStore,
    hir::{GenericArg, MutabilityRef, Path, TypeRef, TypeRefId},
    ids::{ClassId, ConstId, EnumId, EnumVariantId, FunctionId, ImplId, ItemContainerId, StructId, TypeAliasId},
    item_tree::AssocItem,
    resolver::{GenericDefId, Resolver, TypeNs, TypeResolution},
    signatures::{
        self, FieldList, GenericParams, class_signature, const_signature, enum_signature, function_signature,
        impl_signature, struct_signature, type_alias_signature,
    },
};

use crate::{
    kind::{Kind, def_kind, generic_param_kinds},
    ty::{AdtId, GenericParamId, Mutability, Ty},
};

pub(crate) fn imm<'db>() -> Ty<'db> {
    Ty::Mutability(Mutability::Imm)
}

/// A function that gives the hidden mutability parameters of `def`, for the
/// array types without `mut` or `imm` in its signature, numbered after its
/// declared type parameters. `count` is how many it gave.
struct HiddenParams<'db> {
    def: GenericDefId<'db>,
    first: u32,
    count: u32,
}

impl<'db> HiddenParams<'db> {
    fn new(db: &'db dyn Db, def: GenericDefId<'db>) -> Self {
        let first = def.generic_params(db).type_params.len() + named_mutabilities(db, def).len();
        HiddenParams { def, first: first as u32, count: 0 }
    }

    fn fill(&mut self, ty: &Ty<'db>) -> Ty<'db> {
        ty.fill_unwritten(&mut || {
            self.count += 1;
            Ty::Param(GenericParamId { def: self.def, index: self.first + self.count - 1 })
        })
    }
}

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
    /// A type of the wrong kind, like `i32` in `Functor[i32]`, where a type
    /// constructor like `Option` is expected.
    KindMismatch { type_ref: TypeRefId, expected: Kind, found: Kind },
    /// A type alias that refers to itself.
    RecursiveAlias(TypeRefId),
    /// Something the checker doesn't support yet, like `impl Trait`.
    Unsupported(TypeRefId),
    /// A path like `C.Item` where no class of `C` has the associated type.
    NoAssocType { type_ref: TypeRefId },
    /// A path like `C.Item` where several classes of `C` have it.
    AmbiguousAssocType { type_ref: TypeRefId },
    /// `mut` or `imm` before a type that isn't an array or a struct, like
    /// `mut i32`, or before one that already has the other.
    NoMutability(TypeRefId),
    /// `mut` in the type of a field. A field has the mutability of its
    /// struct, so that what an `imm` struct holds can't be changed either.
    MutField(TypeRefId),
    /// A named mutability, like the `'m` of `'m Point`, that isn't one of
    /// the function's or impl's: a name can only be used in the signature
    /// and the body of the function it's written in.
    UnknownMutability(TypeRefId),
}

pub struct TyLoweringContext<'a, 'db> {
    db: &'db dyn Db,
    resolver: &'a Resolver<'db>,
    store: &'a ExpressionStore,
    /// The function or impl whose named mutabilities can be used.
    mutabilities: Option<GenericDefId<'db>>,
    /// The item whose predicates are being lowered, if any, and those
    /// lowered so far. An associated type of its parameters, like `C.Iter`
    /// in `where Iterable[C], Iterator[C.Iter]`, is found through the ones
    /// before it.
    predicates_of: Option<GenericDefId<'db>>,
    lowered: Vec<ClassConstraint<'db>>,
    pub diagnostics: Vec<LowerDiagnostic>,
}

impl<'a, 'db> TyLoweringContext<'a, 'db> {
    pub fn new(db: &'db dyn Db, resolver: &'a Resolver<'db>, store: &'a ExpressionStore) -> Self {
        TyLoweringContext {
            db,
            resolver,
            store,
            mutabilities: None,
            predicates_of: None,
            lowered: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    /// Let the types use the named mutabilities of `def`, a function or an
    /// impl, like the `'m` of `fn next(n: 'm Node) -> 'm Node`.
    pub fn with_mutabilities(mut self, def: GenericDefId<'db>) -> Self {
        self.mutabilities = Some(def);
        self
    }

    /// The parameter that a named mutability like `'m` stands for.
    fn named_mutability(&self, name: &Name) -> Option<Ty<'db>> {
        let def = self.mutabilities?;
        let index = named_mutabilities(self.db, def).iter().position(|n| n == name)?;
        let index = def.generic_params(self.db).type_params.len() + index;
        Some(Ty::Param(GenericParamId { def, index: index as u32 }))
    }

    /// Lower a type that values have, of kind `Type`.
    pub fn lower_ty(&mut self, id: TypeRefId) -> Ty<'db> {
        self.lower_ty_expect(id, &Kind::Type)
    }

    pub fn lower_opt_ty(&mut self, id: Option<TypeRefId>) -> Option<Ty<'db>> {
        id.map(|id| self.lower_ty(id))
    }

    /// Lower a type that must be of kind `expected`, like the argument of
    /// `Functor[F]`, which is `Type -> Type`.
    fn lower_ty_expect(&mut self, id: TypeRefId, expected: &Kind) -> Ty<'db> {
        match self.lower_ty_kinded(id) {
            (ty, Some(kind)) if kind != *expected => {
                self.kind_mismatch(id, expected, &kind);
                // A struct or enum missing arguments, like `Pair[i32]`, is
                // still that type, with errors for what's missing, so that
                // the rest of it is known.
                match ty {
                    Ty::Adt(adt, args, m) if *expected == Kind::Type && kind.is_first_order() => {
                        let missing = kind.split().0.len();
                        let args = args.into_vec().into_iter().chain(std::iter::repeat_n(Ty::Error, missing));
                        Ty::Adt(adt, args.collect(), m)
                    }
                    _ => Ty::Error,
                }
            }
            (ty, _) => ty,
        }
    }

    /// Report that the type `id` is of kind `found` where `expected` is
    /// expected. A type constructor missing arguments where a type is
    /// expected, like the `Option` of `let x: Option`, is reported as a
    /// wrong number of type arguments, which is easier to understand.
    fn kind_mismatch(&mut self, id: TypeRefId, expected: &Kind, found: &Kind) {
        if *expected == Kind::Type && found.is_first_order() {
            let given = match &self.store[id] {
                TypeRef::Path(path) => path.last_generic_args().len(),
                _ => 0,
            };
            let missing = found.split().0.len();
            self.diagnostics.push(LowerDiagnostic::WrongArgCount { type_ref: id, expected: given + missing, found: given });
        } else {
            self.diagnostics.push(LowerDiagnostic::KindMismatch {
                type_ref: id,
                expected: expected.clone(),
                found: found.clone(),
            });
        }
    }

    /// Lower a type of any kind, and give its kind. The kind is `None` for a
    /// type that couldn't be lowered, which was already reported.
    fn lower_ty_kinded(&mut self, id: TypeRefId) -> (Ty<'db>, Option<Kind>) {
        let ty = match &self.store[id] {
            TypeRef::Error => return (Ty::Error, None),
            TypeRef::Path(path) => return self.lower_path_ty(id, path),
            TypeRef::Tuple(tys) => Ty::Tuple(tys.iter().map(|t| self.lower_ty(*t)).collect()),
            TypeRef::Array { element, mutability, .. } => {
                let mutability = mutability.map_or(Ty::Unwritten, Ty::Mutability);
                Ty::array(mutability, self.lower_ty(*element))
            }
            TypeRef::Mut { mutability, inner } => {
                let (ty, kind) = self.lower_ty_kinded(*inner);
                let written = match mutability {
                    MutabilityRef::Written(mutability) => Ty::Mutability(*mutability),
                    MutabilityRef::Named(name) => match self.named_mutability(name) {
                        Some(param) => param,
                        None => {
                            self.diagnostics.push(LowerDiagnostic::UnknownMutability(id));
                            return (ty, kind);
                        }
                    },
                };
                return (self.with_mutability(id, ty, written), kind);
            }
            TypeRef::Never => Ty::Never,
            TypeRef::Fn { params, ret_type, .. } => {
                let params = params.iter().map(|(_, t)| self.lower_ty(*t)).collect();
                let ret = ret_type.map_or_else(Ty::unit, |t| self.lower_ty(t));
                Ty::Fn(params, Box::new(ret), Box::new(Ty::Unwritten))
            }
            TypeRef::ImplTrait { .. } | TypeRef::Bounded(_) | TypeRef::MacroCall { .. } => {
                self.diagnostics.push(LowerDiagnostic::Unsupported(id));
                return (Ty::Error, None);
            }
        };
        (ty, Some(Kind::Type))
    }

    /// `ty` with the mutability written before it, as in `mut Point`: only
    /// arrays, structs and functions have one.
    fn with_mutability(&mut self, id: TypeRefId, ty: Ty<'db>, written: Ty<'db>) -> Ty<'db> {
        let fits = |old: &Ty<'db>| matches!(old, Ty::Unwritten) || *old == written;
        match ty {
            Ty::Adt(adt @ AdtId::Struct(_), args, old) if fits(&old) => Ty::Adt(adt, args, Box::new(written)),
            Ty::Array(old, elem) if fits(&old) => Ty::Array(Box::new(written), elem),
            Ty::Fn(params, ret, old) if fits(&old) => Ty::Fn(params, ret, Box::new(written)),
            Ty::Error => Ty::Error,
            ty => {
                self.diagnostics.push(LowerDiagnostic::NoMutability(id));
                ty
            }
        }
    }

    fn lower_path_ty(&mut self, id: TypeRefId, path: &Path) -> (Ty<'db>, Option<Kind>) {
        let args = path.last_generic_args();
        let Some(resolution) = self.resolver.resolve_path_in_type_ns(self.db, path) else {
            self.diagnostics.push(LowerDiagnostic::UnresolvedType(id));
            // Still report the problems in the arguments, like the `T` of
            // `List[T]` when `T` isn't declared either.
            self.lower_args_unchecked(id, args);
            return (Ty::Error, None);
        };
        if let Some(unresolved) = resolution.unresolved {
            return (self.lower_assoc_path(id, path, resolution.ty, unresolved), Some(Kind::Type));
        }
        let (head, kind) = match resolution.ty {
            // Like an array, a struct written without `mut` or `imm` has its
            // mutability decided by where it's written.
            TypeNs::Struct(s) => {
                (Ty::adt(AdtId::Struct(s), [], Ty::Unwritten), def_kind(self.db, GenericDefId::Struct(s)))
            }
            TypeNs::Enum(e) => (Ty::adt(AdtId::Enum(e), [], imm()), def_kind(self.db, GenericDefId::Enum(e))),
            TypeNs::Builtin(ty) => (Ty::Builtin(ty), Kind::Type),
            TypeNs::GenericParam { def, index } => {
                let kind = generic_param_kinds(self.db, def).get(index as usize).cloned().unwrap_or(Kind::Type);
                (Ty::Param(GenericParamId { def, index }), kind)
            }
            TypeNs::TypeAlias(alias) => return self.lower_alias(id, alias, args),
            TypeNs::Class(_) | TypeNs::Variant(_) => {
                self.diagnostics.push(LowerDiagnostic::NotAType(id));
                return (Ty::Error, None);
            }
        };
        // A constructor can be given fewer arguments than it takes, like
        // `Either[String]`, which is then a constructor itself.
        let (param_kinds, _) = kind.split();
        let type_args = self.type_args(id, args);
        if type_args.len() > param_kinds.len() {
            self.diagnostics.push(LowerDiagnostic::WrongArgCount {
                type_ref: id,
                expected: param_kinds.len(),
                found: type_args.len(),
            });
            for &arg in &type_args {
                self.lower_ty_kinded(arg);
            }
            return (Ty::Error, None);
        }
        let param_kinds: Vec<Kind> = param_kinds.into_iter().cloned().collect();
        let tys: Vec<Ty<'db>> = type_args.iter().zip(&param_kinds).map(|(&t, k)| self.lower_ty_expect(t, k)).collect();
        let result = kind.apply(tys.len()).cloned();
        (Ty::app(head, tys), result)
    }

    /// A type alias, which must be given all its arguments.
    fn lower_alias(&mut self, id: TypeRefId, alias: TypeAliasId<'db>, args: &[GenericArg]) -> (Ty<'db>, Option<Kind>) {
        let kinds = generic_param_kinds(self.db, GenericDefId::TypeAlias(alias));
        let args = self.lower_exact_args(id, args, &kinds);
        let ty = match &type_alias_ty(self.db, alias).ty {
            Some(ty) if *ty == Ty::Error && type_alias_ty(self.db, alias).recursive => {
                self.diagnostics.push(LowerDiagnostic::RecursiveAlias(id));
                Ty::Error
            }
            Some(ty) => subst_params(ty, GenericDefId::TypeAlias(alias), &args),
            // An associated type of a class, named from inside the class:
            // `Item` in `class Index[C]` is `C.Item`.
            None => match alias.container(self.db) {
                ItemContainerId::Class(class) => Ty::Assoc(alias, identity_args(self.db, GenericDefId::Class(class))),
                _ => {
                    self.diagnostics.push(LowerDiagnostic::Unsupported(id));
                    Ty::Error
                }
            },
        };
        (ty, Some(Kind::Type))
    }

    /// A path like `C.Item`: an associated type of a type parameter, from a
    /// class that the parameter has an instance of, through a bound or a
    /// `where` clause of the item that declares it, or a superclass of one.
    /// Inside a class, its own parameter has an instance of the class.
    /// Other types and longer paths aren't supported yet.
    fn lower_assoc_path(&mut self, id: TypeRefId, path: &Path, head: TypeNs<'db>, unresolved: usize) -> Ty<'db> {
        let db = self.db;
        let segments = &path.segments[unresolved..];
        let TypeNs::GenericParam { def, index } = head else {
            self.diagnostics.push(LowerDiagnostic::Unsupported(id));
            return Ty::Error;
        };
        if segments.iter().any(|s| s.generic_args.is_some()) {
            self.diagnostics.push(LowerDiagnostic::Unsupported(id));
            return Ty::Error;
        }
        let constraints = if self.predicates_of == Some(def) {
            with_superclasses(db, self.lowered.clone())
        } else {
            constraints_on(db, def)
        };
        // Each name is an associated type of the type before it, like the
        // `Item` of `C.Iter` in `C.Iter.Item`, from a class that type has an
        // instance of: those of `C`, or the bounds of the associated types of
        // their classes, like `Iterator[C.Iter]` for `type Iter: Iterator;`.
        let mut target = Ty::Param(GenericParamId { def, index });
        for segment in segments {
            let mut found: Vec<(TypeAliasId<'db>, Box<[Ty<'db>]>)> = Vec::new();
            for constraint in &constraints {
                if constraint.args.first() != Some(&target) {
                    continue;
                }
                if let Some(alias) = assoc_type(db, constraint.class, &segment.name)
                    && !found.iter().any(|(a, args)| *a == alias && *args == constraint.args)
                {
                    found.push((alias, constraint.args.clone()));
                }
            }
            target = match &found[..] {
                [(alias, args)] => Ty::Assoc(*alias, args.clone()),
                [] => {
                    self.diagnostics.push(LowerDiagnostic::NoAssocType { type_ref: id });
                    return Ty::Error;
                }
                _ => {
                    self.diagnostics.push(LowerDiagnostic::AmbiguousAssocType { type_ref: id });
                    return Ty::Error;
                }
            };
        }
        target
    }

    /// The arguments of something that must be given exactly one for each
    /// of `kinds`: those missing are errors, and extra ones are only lowered
    /// for their diagnostics.
    fn lower_exact_args(&mut self, id: TypeRefId, args: &[GenericArg], kinds: &[Kind]) -> Vec<Ty<'db>> {
        let type_args = self.type_args(id, args);
        if type_args.len() != kinds.len() {
            self.diagnostics.push(LowerDiagnostic::WrongArgCount {
                type_ref: id,
                expected: kinds.len(),
                found: type_args.len(),
            });
        }
        let tys = kinds
            .iter()
            .enumerate()
            .map(|(i, kind)| match type_args.get(i) {
                Some(&arg) => self.lower_ty_expect(arg, kind),
                None => Ty::Error,
            })
            .collect();
        for &extra in type_args.iter().skip(kinds.len()) {
            self.lower_ty_kinded(extra);
        }
        tys
    }

    /// The type arguments of a path. Other arguments aren't supported yet.
    fn type_args(&mut self, id: TypeRefId, args: &[GenericArg]) -> Vec<TypeRefId> {
        let mut out = Vec::new();
        for arg in args {
            match arg {
                GenericArg::Type { type_ref, .. } => out.push(*type_ref),
                GenericArg::AssocType { .. } | GenericArg::Const(_) => {
                    self.diagnostics.push(LowerDiagnostic::Unsupported(id));
                }
            }
        }
        out
    }

    /// The type arguments of a path, however many there are, for their
    /// diagnostics.
    fn lower_args_unchecked(&mut self, id: TypeRefId, args: &[GenericArg]) {
        for arg in self.type_args(id, args) {
            self.lower_ty_kinded(arg);
        }
    }

    /// Lower a class constraint like `Convert[A, B]`. With `target`, it's a
    /// bound like the `Show` of `T: Show`, and the target is the first
    /// argument, given with its kind.
    pub fn lower_class_constraint(
        &mut self,
        id: TypeRefId,
        target: Option<(Ty<'db>, Option<Kind>)>,
    ) -> Option<ClassConstraint<'db>> {
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
            Some(TypeResolution { ty: TypeNs::Class(class), unresolved: None }) => class,
            Some(_) => {
                self.diagnostics.push(LowerDiagnostic::NotAClass(id));
                return None;
            }
            None => {
                self.diagnostics.push(LowerDiagnostic::UnresolvedType(id));
                return None;
            }
        };
        let kinds = generic_param_kinds(self.db, GenericDefId::Class(class));
        let mut args = Vec::new();
        if let Some((ty, kind)) = target {
            match (kind, kinds.first()) {
                (Some(kind), Some(expected)) if kind != *expected => {
                    self.diagnostics.push(LowerDiagnostic::KindMismatch {
                        type_ref: id,
                        expected: expected.clone(),
                        found: kind,
                    });
                    args.push(Ty::Error);
                }
                _ => args.push(ty),
            }
        }
        let explicit = &kinds[args.len().min(kinds.len())..];
        args.extend(self.lower_exact_args(id, path.last_generic_args(), explicit));
        Some(ClassConstraint { class, args: args.into() })
    }

    /// The class constraints of generic parameters: the bounds in
    /// `[T: Show]` and the `where` predicates, each with the type reference
    /// of the class it's written with.
    pub fn lower_predicates(
        &mut self,
        def: GenericDefId<'db>,
        params: &GenericParams,
    ) -> Vec<(ClassConstraint<'db>, TypeRefId)> {
        let mut out = Vec::new();
        let kinds = generic_param_kinds(self.db, def);
        for (index, param) in params.type_params.iter().enumerate() {
            let target = Ty::Param(GenericParamId { def, index: index as u32 });
            let kind = kinds.get(index).cloned();
            for &bound in param.bounds.iter() {
                let constraint = self.lower_class_constraint(bound, Some((target.clone(), kind.clone())));
                out.extend(constraint.map(|c| (c, bound)));
                self.lowered = out.iter().map(|(c, _)| c.clone()).collect();
            }
        }
        for pred in params.where_predicates.iter() {
            if pred.bounds.is_empty() {
                // A bare constraint like `Convert[A, B]`.
                out.extend(self.lower_class_constraint(pred.target, None).map(|c| (c, pred.target)));
            } else {
                let target = self.lower_ty_kinded(pred.target);
                for &bound in pred.bounds.iter() {
                    out.extend(self.lower_class_constraint(bound, Some(target.clone())).map(|c| (c, bound)));
                }
            }
            self.lowered = out.iter().map(|(c, _)| c.clone()).collect();
        }
        out
    }
}

/// The mutabilities that a function or an impl names, like the `'m` of
/// `fn next(n: 'm Node) -> 'm Node`, in the order of the signature. They're
/// parameters of the function or impl, numbered after its type parameters
/// and before its hidden ones. Other items can't name any.
#[salsa::tracked(returns(ref))]
pub fn named_mutabilities<'db>(db: &'db dyn Db, def: GenericDefId<'db>) -> Box<[Name]> {
    let store = match def {
        GenericDefId::Function(id) => &function_signature(db, id).store,
        GenericDefId::Impl(id) => &impl_signature(db, id).store,
        _ => return Box::new([]),
    };
    let mut names = Vec::new();
    for (_, type_ref) in store.types.iter() {
        if let TypeRef::Mut { mutability: MutabilityRef::Named(name), .. } = type_ref
            && !names.contains(name)
        {
            names.push(name.clone());
        }
    }
    names.into()
}

/// The class constraints that hold for the parameters of `def`: its bounds
/// and `where` clauses, a class's own instance, and their superclasses.
pub fn constraints_on<'db>(db: &'db dyn Db, def: GenericDefId<'db>) -> Vec<ClassConstraint<'db>> {
    let mut constraints: Vec<ClassConstraint<'db>> = generic_predicates(db, def).predicates.to_vec();
    if let GenericDefId::Class(class) = def {
        constraints.push(ClassConstraint { class, args: identity_args(db, def) });
    }
    with_superclasses(db, constraints)
}

/// `constraints` and the superclasses they imply.
pub fn with_superclasses<'db>(db: &'db dyn Db, constraints: Vec<ClassConstraint<'db>>) -> Vec<ClassConstraint<'db>> {
    let mut stack = constraints;
    let mut out = Vec::new();
    while let Some(c) = stack.pop() {
        if out.contains(&c) {
            continue;
        }
        stack.extend(implied_by(db, &c));
        out.push(c);
    }
    out
}

/// What a class constraint implies directly: its superclasses, like the
/// `Eq[X]` of `Ord[X]` for `class Ord[T] where Eq[T]`, and the bounds of the
/// class's associated types, like the `Iterator[X.Iter]` of `Iterable[X]`
/// for `type Iter: Iterator;`.
pub fn implied_by<'db>(db: &'db dyn Db, c: &ClassConstraint<'db>) -> Vec<ClassConstraint<'db>> {
    let class_def = GenericDefId::Class(c.class);
    let superclasses = generic_predicates(db, class_def).predicates.iter();
    let mut out: Vec<ClassConstraint<'db>> =
        superclasses.map(|sup| sup.map(|t| subst_params(t, class_def, &c.args))).collect();
    for alias in class_assoc_types(db, c.class) {
        for bound in assoc_type_bounds(db, alias).predicates.iter() {
            out.push(bound.map(|t| subst_params(t, class_def, &c.args)));
        }
    }
    out
}

/// The associated types that `class` declares.
pub fn class_assoc_types<'db>(db: &'db dyn Db, class: ClassId<'db>) -> Vec<TypeAliasId<'db>> {
    class
        .item(db)
        .items
        .iter()
        .filter_map(|item| match *item {
            AssocItem::TypeAlias(idx) => Some(TypeAliasId::new(db, ItemContainerId::Class(class), idx)),
            _ => None,
        })
        .collect()
}

/// The bounds of an associated type of a class, like the `Iterator` of
/// `type Iter: Iterator;` in `class Iterable[C]`, as constraints on it in
/// terms of the class's parameters: `Iterator[C.Iter]`. Empty for an alias
/// that isn't in a class.
#[salsa::tracked(returns(ref))]
pub fn assoc_type_bounds<'db>(db: &'db dyn Db, alias: TypeAliasId<'db>) -> Predicates<'db> {
    let ItemContainerId::Class(class) = alias.container(db) else {
        return Predicates { predicates: Box::new([]), sources: Box::new([]), diagnostics: Vec::new() };
    };
    let sig = type_alias_signature(db, alias);
    let resolver = Resolver::for_generic_def(db, GenericDefId::TypeAlias(alias));
    let mut ctx = TyLoweringContext::new(db, &resolver, &sig.store);
    let target = Ty::Assoc(alias, identity_args(db, GenericDefId::Class(class)));
    let mut predicates = Vec::new();
    let mut sources = Vec::new();
    for &bound in sig.bounds.iter() {
        if let Some(constraint) = ctx.lower_class_constraint(bound, Some((target.clone(), Some(Kind::Type)))) {
            predicates.push(constraint);
            sources.push(bound);
        }
    }
    Predicates { predicates: predicates.into(), sources: sources.into(), diagnostics: ctx.diagnostics }
}

/// The associated type called `name` that `class` declares, if any.
pub fn assoc_type<'db>(db: &'db dyn Db, class: ClassId<'db>, name: &Name) -> Option<TypeAliasId<'db>> {
    class.item(db).items.iter().find_map(|item| match *item {
        AssocItem::TypeAlias(idx) => {
            let alias = TypeAliasId::new(db, ItemContainerId::Class(class), idx);
            (alias.item(db).name.as_ref() == Some(name)).then_some(alias)
        }
        _ => None,
    })
}

/// The type that `imp` gives the associated type `alias` of its class, in
/// terms of the impl's parameters, if it defines it. An array or struct in
/// it without `mut` or `imm` is `imm`, as in the return type of a function
/// without a body.
pub fn impl_assoc_ty<'db>(db: &'db dyn Db, imp: ImplId<'db>, alias: TypeAliasId<'db>) -> Option<Ty<'db>> {
    let name = alias.item(db).name.as_ref()?;
    imp.item(db).items.iter().find_map(|item| match *item {
        AssocItem::TypeAlias(idx) => {
            let own = TypeAliasId::new(db, ItemContainerId::Impl(imp), idx);
            if own.item(db).name.as_ref() != Some(name) {
                return None;
            }
            type_alias_ty(db, own).ty.as_ref().map(|t| t.fill_unwritten(&mut imm))
        }
        _ => None,
    })
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
    /// Where each predicate is written: the class of a bound or a `where`
    /// clause, in the store of the item's signature.
    pub sources: Box<[TypeRefId]>,
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
    let mut ctx = TyLoweringContext::new(db, &resolver, store).with_mutabilities(def);
    ctx.predicates_of = Some(def);
    // An array without `mut` or `imm` in a predicate, like the `[T]` of
    // `where Show[[T]]`, is `imm`: it can't be the same hidden parameter as
    // one in the signature.
    let (predicates, sources): (Vec<_>, Vec<_>) = ctx
        .lower_predicates(def, params)
        .into_iter()
        .map(|(p, at)| (p.map(|t| t.fill_unwritten(&mut imm)), at))
        .unzip();
    Predicates { predicates: predicates.into(), sources: sources.into(), diagnostics: ctx.diagnostics }
}

/// The declared types of a function.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct FnSig<'db> {
    /// The type of each parameter; `None` where no type is written.
    pub params: Box<[Option<Ty<'db>>]>,
    /// The return type, if written. A function without a body and without
    /// a return type, like a class method, returns `()`. An array or struct
    /// in it without `mut` or `imm` is [`Ty::Unwritten`]: the body decides
    /// its mutability, so the signature isn't complete. Without a body,
    /// it's `imm`.
    pub ret: Option<Ty<'db>>,
    /// How many hidden mutability parameters the function has: one for each
    /// array or struct type without `mut` or `imm` in its parameters, so
    /// that it takes values of either mutability.
    pub hidden: u32,
    pub diagnostics: Vec<LowerDiagnostic>,
}

impl FnSig<'_> {
    /// Whether every type is written, so that callers can use the
    /// signature without looking at the body. A return type with an array or
    /// struct whose mutability isn't written needs the body.
    pub fn is_complete(&self) -> bool {
        self.ret.as_ref().is_some_and(|t| !t.has_unwritten()) && self.params.iter().all(Option::is_some)
    }
}

#[salsa::tracked(returns(ref))]
pub fn fn_sig<'db>(db: &'db dyn Db, id: FunctionId<'db>) -> FnSig<'db> {
    let sig = function_signature(db, id);
    let resolver = Resolver::for_generic_def(db, GenericDefId::Function(id));
    let mut ctx = TyLoweringContext::new(db, &resolver, &sig.store).with_mutabilities(GenericDefId::Function(id));
    let mut hidden = HiddenParams::new(db, GenericDefId::Function(id));
    let params = sig.params.iter().map(|p| ctx.lower_opt_ty(p.type_ref).map(|t| hidden.fill(&t))).collect();
    let ret = match sig.ret_type {
        Some(ret) => Some(ctx.lower_ty(ret)),
        None if !id.item(db).has_body => Some(Ty::unit()),
        None => None,
    };
    FnSig { params, ret, hidden: hidden.count, diagnostics: ctx.diagnostics }
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

/// The types of a struct's fields. An array or struct in them without `mut`
/// or `imm` has the struct's own mutability, which is a hidden parameter of
/// the struct after its type parameters: see [`struct_mutability`]. So what
/// an `imm` struct holds can't be changed either.
#[salsa::tracked(returns(ref))]
pub fn struct_field_types<'db>(db: &'db dyn Db, id: StructId<'db>) -> FieldTypes<'db> {
    let sig = struct_signature(db, id);
    let resolver = Resolver::for_generic_def(db, GenericDefId::Struct(id));
    let own = struct_mutability(db, id);
    lower_fields(&mut TyLoweringContext::new(db, &resolver, &sig.store), &sig.fields, &own)
}

/// The mutability of a struct as seen from its fields: its hidden
/// parameter, which [`struct_args`] gives the value of.
pub fn struct_mutability<'db>(db: &'db dyn Db, id: StructId<'db>) -> Ty<'db> {
    let def = GenericDefId::Struct(id);
    Ty::Param(GenericParamId { def, index: def.generic_params(db).type_params.len() as u32 })
}

/// The arguments to substitute in the field types of a struct of type
/// `Adt(id, args, mutability)`: its type arguments, then its mutability.
pub fn struct_args<'db>(args: &[Ty<'db>], mutability: &Ty<'db>) -> Vec<Ty<'db>> {
    args.iter().chain(std::iter::once(mutability)).cloned().collect()
}

/// The field types of each variant of an enum. An enum has no mutability,
/// since its values can't be changed in place, so an array or struct in
/// them without `mut` or `imm` is `imm`.
#[salsa::tracked(returns(ref))]
pub fn enum_field_types<'db>(db: &'db dyn Db, id: EnumId<'db>) -> Vec<FieldTypes<'db>> {
    let sig = enum_signature(db, id);
    let resolver = Resolver::for_generic_def(db, GenericDefId::Enum(id));
    let mut ctx = TyLoweringContext::new(db, &resolver, &sig.store);
    sig.variants.iter().map(|v| lower_fields(&mut ctx, &v.fields, &imm())).collect()
}

/// Lower fields, with `own` for the arrays and structs in them without `mut`
/// or `imm`. A field can't say `mut`: whether what it holds can be changed
/// is up to the struct.
fn lower_fields<'db>(ctx: &mut TyLoweringContext<'_, 'db>, fields: &FieldList, own: &Ty<'db>) -> FieldTypes<'db> {
    let (kind, list): (_, &[signatures::FieldData]) = match fields {
        FieldList::Record(fields) => (FieldsKind::Record, fields),
        FieldList::Tuple(fields) => (FieldsKind::Tuple, fields),
        FieldList::Unit => (FieldsKind::Unit, &[]),
    };
    let fields = list
        .iter()
        .map(|f| {
            let ty = ctx.lower_ty(f.type_ref);
            if ty.any(&mut |t| *t == Ty::Mutability(Mutability::Mut)) {
                ctx.diagnostics.push(LowerDiagnostic::MutField(f.type_ref));
            }
            let ty = ty.fold(&mut |t| (*t == Ty::Mutability(Mutability::Mut)).then(|| own.clone()));
            (f.name.clone(), ty.fill_unwritten(&mut || own.clone()))
        })
        .collect();
    FieldTypes { kind, fields, diagnostics: std::mem::take(&mut ctx.diagnostics) }
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct ImplHeader<'db> {
    /// The instance, like `Show[Dog]`. `None` if it's not a class.
    pub class: Option<ClassConstraint<'db>>,
    /// How many hidden mutability parameters the impl has, one for each
    /// array type without `mut` or `imm` in its instance, so that
    /// `impl Show[[i32]]` is for arrays of either mutability.
    pub hidden: u32,
    pub diagnostics: Vec<LowerDiagnostic>,
}

/// The instance an impl is for, in terms of its own type parameters.
#[salsa::tracked(returns(ref))]
pub fn impl_header<'db>(db: &'db dyn Db, id: ImplId<'db>) -> ImplHeader<'db> {
    let sig = impl_signature(db, id);
    let resolver = Resolver::for_generic_def(db, GenericDefId::Impl(id));
    let mut ctx = TyLoweringContext::new(db, &resolver, &sig.store).with_mutabilities(GenericDefId::Impl(id));
    let mut hidden = HiddenParams::new(db, GenericDefId::Impl(id));
    let class = ctx.lower_class_constraint(sig.class, None).map(|c| c.map(|t| hidden.fill(t)));
    ImplHeader { class, hidden: hidden.count, diagnostics: ctx.diagnostics }
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
    // A const has one value, so an array without `mut` or `imm` is `imm`.
    let ty = ctx.lower_ty(sig.type_ref).fill_unwritten(&mut imm);
    ConstTy { ty, diagnostics: ctx.diagnostics }
}

#[cfg(test)]
mod tests;

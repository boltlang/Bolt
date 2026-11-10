//! Turning type references, as written, into [`Ty`]s, and the signatures
//! of items into the types the checker uses.

use boltlang_base_db::{Db, File};
use boltlang_hir_def::{
    Name,
    expr_store::ExpressionStore,
    hir::{GenericArg, MutabilityRef, ParamKind, Path, Spread, TupleItem, TypeRef, TypeRefId},
    ids::{
        ClassId, ConstId, EffectId, EnumId, EnumVariantId, FunctionId, ImplId, ItemContainerId,
        StructId, TypeAliasId,
    },
    item_tree::AssocItem,
    nameres::is_visible_from,
    resolver::{GenericDefId, Hidden, Resolver, TypeNs, TypeResolution},
    signatures::{
        self, FieldList, GenericParams, PackKind, class_signature, const_signature,
        effect_signature, enum_signature, function_signature, impl_signature, struct_signature,
        type_alias_signature,
    },
};

use crate::{
    kind::{Kind, def_kind, generic_param_kinds},
    solve::{AssocConstraint, AtMostConstraint, Held},
    ty::{AdtId, EffectTy, GenericParamId, Mutability, PackHalf, Params, Ty},
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
        HiddenParams { def, first: first_hidden(db, def), count: 0 }
    }

    fn fill(&mut self, ty: &Ty<'db>) -> Ty<'db> {
        ty.fill_unwritten(&mut || {
            self.count += 1;
            Ty::Param(GenericParamId { def: self.def, index: self.first + self.count - 1 })
        })
    }
}

/// The index of the first hidden mutability parameter of `def`: after its
/// declared type parameters and its named mutabilities.
pub fn first_hidden<'db>(db: &'db dyn Db, def: GenericDefId<'db>) -> u32 {
    (def.generic_params(db).type_params.len() + named_mutabilities(db, def).len()) as u32
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
    /// A path whose last name is in another file, but not visible here.
    NotVisible {
        type_ref: TypeRefId,
        hidden: Hidden,
    },
    /// A path that names something other than a type, like a class.
    NotAType(TypeRefId),
    /// A bound or `where` predicate that isn't a class.
    NotAClass(TypeRefId),
    WrongArgCount {
        type_ref: TypeRefId,
        expected: usize,
        found: usize,
    },
    /// A type of the wrong kind, like `i32` in `Functor[i32]`, where a type
    /// constructor like `Option` is expected.
    KindMismatch {
        type_ref: TypeRefId,
        expected: Kind,
        found: Kind,
    },
    /// A type alias that refers to itself.
    RecursiveAlias(TypeRefId),
    /// Something the checker doesn't support yet, like `impl Trait`.
    Unsupported(TypeRefId),
    /// A rest of a tuple type that isn't a pack of its kind, like the
    /// `*i32` of `(*i32)` or the `**Ts` of `(**Ts)` for a positional `*Ts`.
    NotAPack(TypeRefId),
    /// A field given twice in a tuple type, like `x` in `(x: i32, x: i64)`.
    DuplicateField {
        type_ref: TypeRefId,
        name: Name,
    },
    /// An effect in a row twice, with different arguments, like the second
    /// `State` of `does State[i32] + State[String]`.
    EffectTwice {
        type_ref: TypeRefId,
        effect: Name,
    },
    /// A path like `C.Item` where no class of `C` has the associated type.
    NoAssocType {
        type_ref: TypeRefId,
    },
    /// A path like `C.Item` where several classes of `C` have it.
    AmbiguousAssocType {
        type_ref: TypeRefId,
    },
    /// `Iterator[Itme = T]`, where the class has no associated type called
    /// `name`. `type_ref` is the class constraint.
    NotAnAssocTypeOf {
        type_ref: TypeRefId,
        class: Name,
        name: Name,
    },
    /// What an associated type is, like the `Item = T` of `Iterator[Item =
    /// T]`, somewhere other than a bound or `where` clause of a function or
    /// an impl, like an impl's instance. `type_ref` is the class constraint.
    EqualityNotAllowed(TypeRefId),
    /// `mut` or `imm` before a type that isn't an array or a struct, like
    /// `mut i32`, or before one that already has the other.
    NoMutability(TypeRefId),
    /// `mut` in the type of a field. A field has the mutability of its
    /// struct, so that what an `imm` struct holds can't be changed either.
    MutField(TypeRefId),
    /// A mutability on its own where a type is expected, like `let x: imm`:
    /// it can only be a bound, like the `imm` of `T: imm`.
    MutabilityAlone(TypeRefId),
    /// A named mutability, like the `'m` of `'m Point`, that isn't one of
    /// the function's or impl's, or one that its struct declares: a name can
    /// only be used in the signature and the body of the function it's
    /// written in, or in the fields of a struct that declares it.
    UnknownMutability(TypeRefId),
    /// More mutability arguments than the struct declares, like the `imm`
    /// of `Point[imm]` for a `Point` that declares none.
    WrongMutabilityCount {
        type_ref: TypeRefId,
        expected: usize,
        found: usize,
    },
    /// A mutability parameter, like the `'v` of `enum E['v]`, of an item
    /// that isn't a struct, function or impl.
    MutabilityParamNotAllowed(TypeRefId),
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
    /// The "at most" bounds lowered by [`TyLoweringContext::lower_predicates`],
    /// like `T: imm`, with where each is written.
    at_most: Vec<(AtMostConstraint<'db>, TypeRefId)>,
    /// What associated types are, like the `Item = T` of `I: Iterator[Item
    /// = T]`, with where the class is written, lowered with predicates.
    equalities: Vec<(AssocConstraint<'db>, TypeRefId)>,
    /// Whether what an associated type is can be said, like the `Item =
    /// Item` of `type Iter: Iterator[Item = Item];`, besides in the
    /// predicates of functions and impls.
    allow_equalities: bool,
    /// The associated type whose bounds are being lowered, if any.
    bounds_of: Option<TypeAliasId<'db>>,
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
            at_most: Vec::new(),
            equalities: Vec::new(),
            allow_equalities: false,
            bounds_of: None,
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

    /// A mutability on its own, like the `imm` of `T: imm`, or `None` if
    /// `id` isn't one. A name that isn't the item's is reported.
    fn lower_mutability(&mut self, id: TypeRefId) -> Option<Option<Ty<'db>>> {
        let TypeRef::Mutability(mutability) = &self.store[id] else { return None };
        Some(match mutability {
            MutabilityRef::Written(m) => Some(Ty::Mutability(*m)),
            MutabilityRef::Named(name) => {
                let found = self.named_mutability(name);
                if found.is_none() {
                    self.diagnostics.push(LowerDiagnostic::UnknownMutability(id));
                }
                found
            }
        })
    }

    /// A bound `bound` of `target`: an "at most" bound if `bound` is a
    /// mutability, and a class otherwise.
    fn lower_bound(&mut self, held: Held<'db>, bound: TypeRefId) -> Option<ClassConstraint<'db>> {
        if let Some(mutability) = self.lower_mutability(bound) {
            if let Some(mutability) = mutability {
                self.at_most.push((AtMostConstraint { held, bound: mutability }, bound));
            }
            return None;
        }
        match held {
            Held::Type(ty) => {
                let kind = None;
                self.lower_class_constraint(bound, Some((ty, kind)))
            }
            // A mutability has no classes.
            Held::Mutability(_) => {
                self.diagnostics.push(LowerDiagnostic::NotAClass(bound));
                None
            }
        }
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
                    Ty::Adt(adt, args, ms, m)
                        if *expected == Kind::Type && kind.is_first_order() =>
                    {
                        let missing = kind.split().0.len();
                        let args = args
                            .into_vec()
                            .into_iter()
                            .chain(std::iter::repeat_n(Ty::Error, missing));
                        Ty::Adt(adt, args.collect(), ms, m)
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
            self.diagnostics.push(LowerDiagnostic::WrongArgCount {
                type_ref: id,
                expected: given + missing,
                found: given,
            });
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
            // Like an array, a tuple written without `mut` or `imm` has its
            // mutability decided by where it's written.
            TypeRef::Tuple(tys) => {
                Ty::tuple(tys.iter().map(|t| self.lower_ty(*t)).collect(), Ty::Unwritten)
            }
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
                // `imm T` of a type that has no mutability of its own, like a
                // type parameter, is a copy of it that can't be changed.
                let deep = matches!(ty, Ty::Param(_) | Ty::Assoc(..) | Ty::App(..) | Ty::Imm(_));
                if deep && kind == Some(Kind::Type) && written == Ty::Mutability(Mutability::Imm) {
                    return (Ty::imm_of_unknown(ty), kind);
                }
                return (self.with_mutability(id, ty, written), kind);
            }
            TypeRef::Never => Ty::Never,
            TypeRef::Mutability(_) => {
                self.diagnostics.push(LowerDiagnostic::MutabilityAlone(id));
                return (Ty::Error, None);
            }
            TypeRef::Fn { params, ret_type, effects, .. } => {
                let params = self.lower_fn_params(params);
                let ret = ret_type.map_or_else(Ty::unit, |t| self.lower_ty(t));
                // A function type without `does` has no effects.
                let effects = effects.as_ref().map_or_else(Ty::nothing, |ts| self.lower_row(ts));
                Ty::Fn(Box::new(params), Box::new(ret), Box::new(Ty::Unwritten), Box::new(effects))
            }
            // Like a tuple, its mutability is decided by where it's written.
            TypeRef::NamedTuple(items) => {
                let mut positional = Vec::new();
                let mut named: Vec<(Name, Ty<'db>)> = Vec::new();
                for item in items.iter() {
                    match item {
                        TupleItem::Positional(t) => positional.push(self.lower_ty(*t)),
                        TupleItem::Named(name, t) => {
                            let ty = self.lower_ty(*t);
                            if named.iter().any(|(n, _)| n == name) {
                                self.diagnostics.push(LowerDiagnostic::DuplicateField {
                                    type_ref: id,
                                    name: name.clone(),
                                });
                            } else {
                                named.push((name.clone(), ty));
                            }
                        }
                        TupleItem::Spread(spread, t) => {
                            positional.extend(self.lower_rest(*spread, *t))
                        }
                    }
                }
                Ty::record(positional.into(), named, Ty::Unwritten)
            }
            TypeRef::ImplTrait { .. } | TypeRef::Bounded(_) | TypeRef::MacroCall { .. } => {
                self.diagnostics.push(LowerDiagnostic::Unsupported(id));
                return (Ty::Error, None);
            }
        };
        (ty, Some(Kind::Type))
    }

    /// `ty` with the mutability written before it, as in `mut Point`: only
    /// arrays, structs, tuples and functions have one.
    fn with_mutability(&mut self, id: TypeRefId, ty: Ty<'db>, written: Ty<'db>) -> Ty<'db> {
        let fits = |old: &Ty<'db>| matches!(old, Ty::Unwritten) || *old == written;
        match ty {
            Ty::Adt(adt @ AdtId::Struct(_), args, ms, old) if fits(&old) => {
                Ty::Adt(adt, args, ms, Box::new(written))
            }
            Ty::Array(old, elem) if fits(&old) => Ty::Array(Box::new(written), elem),
            Ty::Tuple(tys, old) if !tys.is_empty() && fits(&old) => Ty::tuple(tys, written),
            Ty::Record(tys, named, old) if fits(&old) => Ty::Record(tys, named, Box::new(written)),
            Ty::Fn(params, ret, old, effects) if fits(&old) => {
                Ty::Fn(params, ret, Box::new(written), effects)
            }
            Ty::Error => Ty::Error,
            ty => {
                self.diagnostics.push(LowerDiagnostic::NoMutability(id));
                ty
            }
        }
    }

    /// The type `path` doesn't resolve: say so, or that what it names isn't
    /// visible.
    fn unresolved_type(&mut self, id: TypeRefId, path: &Path) {
        self.diagnostics.push(match self.resolver.hidden(self.db, path) {
            Some(hidden) => LowerDiagnostic::NotVisible { type_ref: id, hidden },
            None => LowerDiagnostic::UnresolvedType(id),
        });
    }

    fn lower_path_ty(&mut self, id: TypeRefId, path: &Path) -> (Ty<'db>, Option<Kind>) {
        let args = path.last_generic_args();
        let Some(resolution) = self.resolver.resolve_path_in_type_ns(self.db, path) else {
            self.unresolved_type(id, path);
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
            // An `imm struct` is always `imm`, like an enum.
            TypeNs::Struct(s) => {
                let mutabilities = self.mutability_args(id, s, args);
                let own = if s.item(self.db).is_imm { imm() } else { Ty::Unwritten };
                let head = Ty::adt(AdtId::Struct(s), [], mutabilities, own);
                (head, def_kind(self.db, GenericDefId::Struct(s)))
            }
            TypeNs::Enum(e) => {
                (Ty::adt(AdtId::Enum(e), [], [], imm()), def_kind(self.db, GenericDefId::Enum(e)))
            }
            TypeNs::Builtin(ty) => (Ty::Builtin(ty), Kind::Type),
            TypeNs::Nothing => (Ty::nothing(), Kind::Effect),
            TypeNs::GenericParam { def, index } => {
                let kind = generic_param_kinds(self.db, def)
                    .get(index as usize)
                    .cloned()
                    .unwrap_or(Kind::Type);
                (Ty::Param(GenericParamId { def, index }), kind)
            }
            TypeNs::TypeAlias(alias) => return self.lower_alias(id, alias, args),
            TypeNs::Effect(e) => return self.lower_effect(id, e, args),
            TypeNs::Class(_) | TypeNs::Variant(_) => {
                self.diagnostics.push(LowerDiagnostic::NotAType(id));
                return (Ty::Error, None);
            }
        };
        // A constructor can be given fewer arguments than it takes, like
        // `Either[String]`, which is then a constructor itself.
        let (param_kinds, _) = kind.split();
        let mut type_args = self.type_args(id, args);
        // The mutability arguments of a struct, like the `imm` of
        // `VecIter[i32, imm]`, are already part of its head.
        if let TypeNs::Struct(_) = resolution.ty {
            type_args.retain(|&t| !matches!(self.store[t], TypeRef::Mutability(_)));
        }
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
        let tys: Vec<Ty<'db>> =
            type_args.iter().zip(&param_kinds).map(|(&t, k)| self.lower_ty_expect(t, k)).collect();
        let result = kind.apply(tys.len()).cloned();
        (Ty::app(head, tys), result)
    }

    /// An effect with its arguments, like `State[i32]`: a row of one effect,
    /// of kind `Effect`. Without all its arguments, like `State`, it's of a
    /// kind like `Type -> Effect`, and no row.
    fn lower_effect(
        &mut self,
        id: TypeRefId,
        effect: EffectId<'db>,
        args: &[GenericArg],
    ) -> (Ty<'db>, Option<Kind>) {
        let kind = def_kind(self.db, GenericDefId::Effect(effect));
        let param_kinds: Vec<Kind> = kind.split().0.into_iter().cloned().collect();
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
        let tys: Vec<Ty<'db>> =
            type_args.iter().zip(&param_kinds).map(|(&t, k)| self.lower_ty_expect(t, k)).collect();
        if tys.len() < param_kinds.len() {
            return (Ty::Error, kind.apply(tys.len()).cloned());
        }
        (Ty::row([EffectTy { effect, args: tys.into() }]), Some(Kind::Effect))
    }

    /// The row of the effects after `does`, like `Log + State[i32]`. An
    /// effect in it twice must have the same arguments, and is then in the
    /// row once.
    pub fn lower_row(&mut self, terms: &[TypeRefId]) -> Ty<'db> {
        let mut known: Vec<EffectTy<'db>> = Vec::new();
        let mut unknown = Vec::new();
        for &term in terms {
            match self.lower_ty_expect(term, &Kind::Effect) {
                Ty::Row(effects, rest) => {
                    unknown.extend(rest.into_vec());
                    for e in effects.into_vec() {
                        match known.iter().find(|k| k.effect == e.effect) {
                            Some(k) if k.args != e.args => {
                                let effect = e.effect.item(self.db).name.clone();
                                if let Some(effect) = effect {
                                    self.diagnostics.push(LowerDiagnostic::EffectTwice {
                                        type_ref: term,
                                        effect,
                                    });
                                }
                            }
                            Some(_) => {}
                            None => known.push(e),
                        }
                    }
                }
                Ty::Error => {}
                // A row parameter, like the `E` of `Log + E`.
                row => unknown.push(row),
            }
        }
        Ty::row_of(known, unknown)
    }

    /// The mutability arguments of the struct `s`, like the `imm` of
    /// `VecIter[i32, imm]`: the arguments that are a mutability on their own,
    /// one for each that it declares. Those not given are
    /// [`Ty::Unwritten`], decided by where the type is written.
    fn mutability_args(
        &mut self,
        id: TypeRefId,
        s: StructId<'db>,
        args: &[GenericArg],
    ) -> Vec<Ty<'db>> {
        let expected = named_mutabilities(self.db, GenericDefId::Struct(s)).len();
        let written: Vec<TypeRefId> = self
            .type_args(id, args)
            .into_iter()
            .filter(|&t| matches!(self.store[t], TypeRef::Mutability(_)))
            .collect();
        if written.len() > expected {
            self.diagnostics.push(LowerDiagnostic::WrongMutabilityCount {
                type_ref: id,
                expected,
                found: written.len(),
            });
        }
        let mut out: Vec<Ty<'db>> = written
            .iter()
            .take(expected)
            .map(|&t| self.lower_mutability(t).flatten().unwrap_or(Ty::Error))
            .collect();
        out.resize(expected, Ty::Unwritten);
        out
    }

    /// The parameters of a function type, like `fn(i32, *Ts, sep: String,
    /// **Ks)`. A rest is written as the tuple `(*Ts)` or `(**Ks)`, whose pack
    /// it is.
    fn lower_fn_params(&mut self, params: &[(Option<Name>, ParamKind, TypeRefId)]) -> Params<'db> {
        let tys: Vec<_> =
            params.iter().map(|(name, kind, t)| (*kind, name.clone(), self.lower_ty(*t))).collect();
        params_from(tys)
    }

    /// A rest of a tuple type, like the `*Ts` of `(T, *Ts)`: the spread of
    /// the pack it names, which must be of its kind: `*` for a positional
    /// pack and `**` for a named one.
    fn lower_rest(&mut self, spread: Spread, id: TypeRefId) -> Option<Ty<'db>> {
        let (ty, _) = self.lower_ty_kinded(id);
        let pack = match &ty {
            Ty::Param(p) => p.def.generic_params(self.db).type_params[p.index as usize].pack,
            Ty::Error => return None,
            _ => None,
        };
        let half = match (spread, pack) {
            (Spread::Positional, Some(PackKind::Positional)) => PackHalf::Positional,
            (Spread::Named, Some(PackKind::Named)) => PackHalf::Named,
            _ => {
                self.diagnostics.push(LowerDiagnostic::NotAPack(id));
                return None;
            }
        };
        Some(Ty::Spread(half, Box::new(ty)))
    }

    /// A type alias, which must be given all its arguments.
    fn lower_alias(
        &mut self,
        id: TypeRefId,
        alias: TypeAliasId<'db>,
        args: &[GenericArg],
    ) -> (Ty<'db>, Option<Kind>) {
        let kinds = generic_param_kinds(self.db, GenericDefId::TypeAlias(alias));
        let args = self.lower_exact_args(id, args, &kinds);
        // An associated type of a class, named from inside the class: `Item`
        // in `class Index[C]` is `C.Item`, whichever type the impl gives it,
        // even with a default like `type Item = i32;`, which is only what an
        // impl that doesn't say gets.
        if let ItemContainerId::Class(class) = alias.container(self.db) {
            return (
                Ty::Assoc(alias, identity_args(self.db, GenericDefId::Class(class))),
                Some(Kind::Type),
            );
        }
        // A type of the host, which is a type of its own.
        let item = alias.item(self.db);
        if item.link.is_some() && !item.has_value {
            return (Ty::Host(alias), Some(Kind::Type));
        }
        let ty = match &type_alias_ty(self.db, alias).ty {
            Some(ty) if *ty == Ty::Error && type_alias_ty(self.db, alias).recursive => {
                self.diagnostics.push(LowerDiagnostic::RecursiveAlias(id));
                Ty::Error
            }
            Some(ty) => subst_params(ty, GenericDefId::TypeAlias(alias), &args),
            // An alias without a type, which is reported where it's declared.
            None => Ty::Error,
        };
        (ty, Some(Kind::Type))
    }

    /// A path like `C.Item`: an associated type of a type parameter, from a
    /// class that the parameter has an instance of, through a bound or a
    /// `where` clause of the item that declares it, or a superclass of one.
    /// Inside a class, its own parameter has an instance of the class.
    /// Other types and longer paths aren't supported yet.
    fn lower_assoc_path(
        &mut self,
        id: TypeRefId,
        path: &Path,
        head: TypeNs<'db>,
        unresolved: usize,
    ) -> Ty<'db> {
        let db = self.db;
        let segments = &path.segments[unresolved..];
        if segments.iter().any(|s| s.generic_args.is_some()) {
            self.diagnostics.push(LowerDiagnostic::Unsupported(id));
            return Ty::Error;
        }
        let (mut target, constraints) = match head {
            TypeNs::GenericParam { def, index } => {
                let constraints = if self.predicates_of == Some(def) {
                    with_superclasses(db, self.lowered.clone())
                } else {
                    constraints_on(db, def)
                };
                (Ty::Param(GenericParamId { def, index }), constraints)
            }
            // `Iter.Item` inside `class Iterable[C]`, like in the default
            // `type Item = Iter.Item;`: `C.Iter.Item`, from the classes that
            // the bounds of `Iter` give it. Not in those bounds themselves,
            // which would need themselves.
            TypeNs::TypeAlias(alias) if self.bounds_of != Some(alias) => {
                match alias.container(db) {
                    ItemContainerId::Class(class) => {
                        let target =
                            Ty::Assoc(alias, identity_args(db, GenericDefId::Class(class)));
                        (
                            target,
                            with_superclasses(db, assoc_type_bounds(db, alias).predicates.to_vec()),
                        )
                    }
                    _ => {
                        self.diagnostics.push(LowerDiagnostic::Unsupported(id));
                        return Ty::Error;
                    }
                }
            }
            _ => {
                self.diagnostics.push(LowerDiagnostic::Unsupported(id));
                return Ty::Error;
            }
        };
        // Each name is an associated type of the type before it, like the
        // `Item` of `C.Iter` in `C.Iter.Item`, from a class that type has an
        // instance of: those of `C`, or the bounds of the associated types of
        // their classes, like `Iterator[C.Iter]` for `type Iter: Iterator;`.
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
    fn lower_exact_args(
        &mut self,
        id: TypeRefId,
        args: &[GenericArg],
        kinds: &[Kind],
    ) -> Vec<Ty<'db>> {
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
                self.unresolved_type(id, path);
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
        // `Iterator[Item = T]` says what an associated type is.
        let (bindings, plain): (Vec<GenericArg>, Vec<GenericArg>) = path
            .last_generic_args()
            .iter()
            .cloned()
            .partition(|a| matches!(a, GenericArg::AssocType { .. }));
        args.extend(self.lower_exact_args(id, &plain, explicit));
        let constraint = ClassConstraint { class, args: args.into() };
        for binding in &bindings {
            self.lower_equality(id, &constraint, binding);
        }
        Some(constraint)
    }

    /// Lower `Item = T` in the class constraint `constraint`, written at
    /// `id`, into what the associated type `Item` is for its arguments.
    fn lower_equality(
        &mut self,
        id: TypeRefId,
        constraint: &ClassConstraint<'db>,
        binding: &GenericArg,
    ) {
        let GenericArg::AssocType { name, generic_args, type_ref } = binding else { return };
        let ty = self.lower_ty(*type_ref);
        if generic_args.is_some() {
            self.diagnostics.push(LowerDiagnostic::Unsupported(id));
            return;
        }
        if !self.allow_equalities
            && !matches!(
                self.predicates_of,
                Some(GenericDefId::Function(_) | GenericDefId::Impl(_))
            )
        {
            self.diagnostics.push(LowerDiagnostic::EqualityNotAllowed(id));
            return;
        }
        let alias = class_assoc_types(self.db, constraint.class)
            .into_iter()
            .find(|alias| alias.item(self.db).name.as_ref() == Some(name));
        let Some(alias) = alias else {
            let class =
                constraint.class.item(self.db).name.clone().unwrap_or_else(|| Name::new("?"));
            let name = name.clone();
            self.diagnostics.push(LowerDiagnostic::NotAnAssocTypeOf { type_ref: id, class, name });
            return;
        };
        self.equalities.push((AssocConstraint { alias, args: constraint.args.clone(), ty }, id));
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
                let constraint = match self.lower_mutability(bound) {
                    Some(Some(mutability)) => {
                        let constraint = AtMostConstraint {
                            held: Held::Type(target.clone()),
                            bound: mutability,
                        };
                        self.at_most.push((constraint, bound));
                        None
                    }
                    Some(None) => None,
                    None => {
                        self.lower_class_constraint(bound, Some((target.clone(), kind.clone())))
                    }
                };
                out.extend(constraint.map(|c| (c, bound)));
                self.lowered = out.iter().map(|(c, _)| c.clone()).collect();
            }
        }
        // `'v: imm` in `struct VecIter[T, 'v: imm]`.
        let named = named_mutabilities(self.db, def);
        for param in params.mutability_params.iter() {
            let Some(index) = named.iter().position(|n| *n == param.name) else {
                self.diagnostics.push(LowerDiagnostic::MutabilityParamNotAllowed(param.type_ref));
                continue;
            };
            let index = (params.type_params.len() + index) as u32;
            let target = Ty::Param(GenericParamId { def, index });
            for &bound in param.bounds.iter() {
                out.extend(
                    self.lower_bound(Held::Mutability(target.clone()), bound).map(|c| (c, bound)),
                );
            }
        }
        for pred in params.where_predicates.iter() {
            if pred.bounds.is_empty() {
                // A bare constraint like `Convert[A, B]`.
                out.extend(
                    self.lower_class_constraint(pred.target, None).map(|c| (c, pred.target)),
                );
            } else if let Some(target) = self.lower_mutability(pred.target) {
                // `'v: 'm`: a mutability is at most another.
                let Some(target) = target else { continue };
                for &bound in pred.bounds.iter() {
                    out.extend(
                        self.lower_bound(Held::Mutability(target.clone()), bound)
                            .map(|c| (c, bound)),
                    );
                }
            } else {
                let target = self.lower_ty_kinded(pred.target);
                for &bound in pred.bounds.iter() {
                    let constraint = match self.lower_mutability(bound) {
                        Some(_) => self.lower_bound(Held::Type(target.0.clone()), bound),
                        None => self.lower_class_constraint(bound, Some(target.clone())),
                    };
                    out.extend(constraint.map(|c| (c, bound)));
                }
            }
            self.lowered = out.iter().map(|(c, _)| c.clone()).collect();
        }
        out
    }
}

/// The mutabilities that a function, an impl or a struct names, like the
/// `'m` of `fn next(n: 'm Node) -> 'm Node`, or the `'v` of `struct
/// VecIter[T, 'v]`. They're parameters of the item, numbered after its type
/// parameters and before its hidden ones, or a struct's own mutability.
///
/// A struct has those it declares, in order, since a type gives them in
/// that order, like `VecIter[i32, imm]`. A function or an impl has those it
/// declares, then those its signature uses, in the order they're written.
/// Other items can't name any.
#[salsa::tracked(returns(ref))]
pub fn named_mutabilities<'db>(db: &'db dyn Db, def: GenericDefId<'db>) -> Box<[Name]> {
    let declared = || def.generic_params(db).mutability_params.iter().map(|p| p.name.clone());
    let store = match def {
        GenericDefId::Function(id) => &function_signature(db, id).store,
        GenericDefId::Impl(id) => &impl_signature(db, id).store,
        GenericDefId::Struct(_) => {
            let mut names: Vec<Name> = Vec::new();
            for name in declared() {
                if !names.contains(&name) {
                    names.push(name);
                }
            }
            return names.into();
        }
        _ => return Box::new([]),
    };
    let mut names: Vec<Name> = Vec::new();
    for name in declared() {
        if !names.contains(&name) {
            names.push(name);
        }
    }
    for (_, type_ref) in store.types.iter() {
        if let TypeRef::Mut { mutability: MutabilityRef::Named(name), .. }
        | TypeRef::Mutability(MutabilityRef::Named(name)) = type_ref
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
    let mut constraints: Vec<ClassConstraint<'db>> =
        generic_predicates(db, def).predicates.to_vec();
    if let GenericDefId::Class(class) = def {
        constraints.push(ClassConstraint { class, args: identity_args(db, def) });
    }
    with_superclasses(db, constraints)
}

/// `constraints` and the superclasses they imply.
pub fn with_superclasses<'db>(
    db: &'db dyn Db,
    constraints: Vec<ClassConstraint<'db>>,
) -> Vec<ClassConstraint<'db>> {
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

/// What associated types are wherever `c` holds, from the bounds of its
/// class's associated types: with `type Iter: Iterator[Item = Item];` in
/// `class Iterable[C]`, `Iterable[X]` gives `X.Iter.Item = X.Item`.
pub fn implied_equalities<'db>(
    db: &'db dyn Db,
    c: &ClassConstraint<'db>,
) -> Vec<AssocConstraint<'db>> {
    let class_def = GenericDefId::Class(c.class);
    let subst = |t: &Ty<'db>| subst_params(t, class_def, &c.args);
    class_assoc_types(db, c.class)
        .into_iter()
        .flat_map(|alias| assoc_type_bounds(db, alias).equalities.iter())
        .map(|(e, _)| AssocConstraint {
            alias: e.alias,
            args: e.args.iter().map(subst).collect(),
            ty: subst(&e.ty),
        })
        .collect()
}

/// The associated types that `class` declares.
pub fn class_assoc_types<'db>(db: &'db dyn Db, class: ClassId<'db>) -> Vec<TypeAliasId<'db>> {
    class
        .item(db)
        .items
        .iter()
        .filter_map(|item| match *item {
            AssocItem::TypeAlias(idx) => {
                Some(TypeAliasId::new(db, ItemContainerId::Class(class), idx))
            }
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
        return Predicates {
            predicates: Box::new([]),
            equalities: Box::new([]),
            at_most: Box::new([]),
            sources: Box::new([]),
            diagnostics: Vec::new(),
        };
    };
    let sig = type_alias_signature(db, alias);
    let resolver = Resolver::for_generic_def(db, GenericDefId::TypeAlias(alias));
    let mut ctx = TyLoweringContext::new(db, &resolver, &sig.store);
    // `type Iter: Iterator[Item = Item];`: what `C.Iter.Item` is, for every
    // impl of the class.
    ctx.allow_equalities = true;
    ctx.bounds_of = Some(alias);
    let target = Ty::Assoc(alias, identity_args(db, GenericDefId::Class(class)));
    let mut predicates = Vec::new();
    let mut sources = Vec::new();
    for &bound in sig.bounds.iter() {
        if let Some(constraint) =
            ctx.lower_class_constraint(bound, Some((target.clone(), Some(Kind::Type))))
        {
            predicates.push(constraint);
            sources.push(bound);
        }
    }
    let equalities = std::mem::take(&mut ctx.equalities);
    Predicates {
        predicates: predicates.into(),
        equalities: equalities.into(),
        at_most: Box::new([]),
        sources: sources.into(),
        diagnostics: ctx.diagnostics,
    }
}

/// The associated type called `name` that `class` declares, if any.
pub fn assoc_type<'db>(
    db: &'db dyn Db,
    class: ClassId<'db>,
    name: &Name,
) -> Option<TypeAliasId<'db>> {
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
/// without a body. It's a query, since solving any projection through the
/// impl needs it.
#[salsa::tracked(returns(clone))]
pub fn impl_assoc_ty<'db>(
    db: &'db dyn Db,
    imp: ImplId<'db>,
    alias: TypeAliasId<'db>,
) -> Option<Ty<'db>> {
    impl_assoc_ty_seen(db, imp, alias, &mut Vec::new())
}

/// [`impl_assoc_ty`], with the class's default when the impl doesn't give
/// one, like the `i32` of `type Item = i32;`. A default can use the class's
/// other associated types, like `type Item = Iter.Item;`, which are the
/// impl's. `seen` are the defaults being found, so that defaults that refer
/// to each other are the error type.
fn impl_assoc_ty_seen<'db>(
    db: &'db dyn Db,
    imp: ImplId<'db>,
    alias: TypeAliasId<'db>,
    seen: &mut Vec<TypeAliasId<'db>>,
) -> Option<Ty<'db>> {
    let name = alias.item(db).name.as_ref()?;
    let own = imp.item(db).items.iter().find_map(|item| match *item {
        AssocItem::TypeAlias(idx) => {
            let own = TypeAliasId::new(db, ItemContainerId::Impl(imp), idx);
            (own.item(db).name.as_ref() == Some(name)).then_some(own)
        }
        _ => None,
    });
    if let Some(own) = own {
        return type_alias_ty(db, own).ty.as_ref().map(|t| t.fill_unwritten(&mut imm));
    }
    let ItemContainerId::Class(class) = alias.container(db) else { return None };
    let default = type_alias_ty(db, alias).ty.clone()?;
    let target = impl_header(db, imp).class.clone()?;
    if target.class != class {
        return None;
    }
    if seen.contains(&alias) {
        return Some(Ty::Error);
    }
    seen.push(alias);
    let ty =
        subst_params(&default, GenericDefId::Class(class), &target.args).fold(&mut |t| match t {
            Ty::Assoc(a, args)
                if a.container(db) == ItemContainerId::Class(class) && *args == target.args =>
            {
                Some(impl_assoc_ty_seen(db, imp, *a, seen).unwrap_or(Ty::Error))
            }
            _ => None,
        });
    seen.pop();
    Some(ty.fill_unwritten(&mut imm))
}

/// Whether the default of the associated type `alias` of a class leads back
/// to `alias` through the class's defaults, like those of `type A = B; type
/// B = A;` (#298). An impl that gives neither gets the error type for both,
/// in [`impl_assoc_ty`]; one that gives one of them breaks the cycle.
pub fn default_refers_to_itself<'db>(db: &'db dyn Db, alias: TypeAliasId<'db>) -> bool {
    let ItemContainerId::Class(class) = alias.container(db) else { return false };
    let own = identity_args(db, GenericDefId::Class(class));
    let mut todo = vec![alias];
    let mut seen = Vec::new();
    while let Some(next) = todo.pop() {
        if seen.contains(&next) {
            continue;
        }
        seen.push(next);
        let Some(default) = &type_alias_ty(db, next).ty else { continue };
        let found = default.any(&mut |t| match t {
            Ty::Assoc(a, args)
                if a.container(db) == ItemContainerId::Class(class) && *args == own =>
            {
                todo.push(*a);
                *a == alias
            }
            _ => false,
        });
        if found {
            return true;
        }
    }
    false
}

/// Replace the type parameters of `def` with `args`.
pub fn subst_params<'db>(ty: &Ty<'db>, def: GenericDefId<'db>, args: &[Ty<'db>]) -> Ty<'db> {
    ty.fold(&mut |t| match t {
        Ty::Param(p) if p.def == def => {
            Some(args.get(p.index as usize).cloned().unwrap_or(Ty::Error))
        }
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
    // An associated type in an impl can use the impl's named mutabilities,
    // like the `'v` of `impl[T, 'v] Iterable['v [T]] { type Iter = mut
    // ArrayIter[T, 'v]; }`: it's the mutability the impl is chosen for.
    if let ItemContainerId::Impl(imp) = alias.container(db) {
        ctx = ctx.with_mutabilities(GenericDefId::Impl(imp));
    }
    let ty = ctx.lower_opt_ty(sig.type_ref);
    TypeAliasTy { ty, recursive: false, diagnostics: ctx.diagnostics }
}

fn type_alias_cycle<'db>(
    _db: &'db dyn Db,
    _id: salsa::Id,
    _alias: TypeAliasId<'db>,
) -> TypeAliasTy<'db> {
    TypeAliasTy { ty: Some(Ty::Error), recursive: true, diagnostics: Vec::new() }
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct Predicates<'db> {
    pub predicates: Box<[ClassConstraint<'db>]>,
    /// What the item assumes its associated types are, like the `Item = T`
    /// of `I: Iterator[Item = T]`, with where the class is written.
    pub equalities: Box<[(AssocConstraint<'db>, TypeRefId)]>,
    /// What the item assumes is at most a mutability (#243), like `T: imm`,
    /// with where each is written.
    pub at_most: Box<[(AtMostConstraint<'db>, TypeRefId)]>,
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
        GenericDefId::Effect(id) => {
            let sig = effect_signature(db, id);
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
    let at_most = std::mem::take(&mut ctx.at_most)
        .into_iter()
        .map(|(c, at)| (c.map(|t| t.fill_unwritten(&mut imm)), at));
    let equalities = std::mem::take(&mut ctx.equalities).into_iter().map(|(c, at)| {
        let args = c.args.iter().map(|t| t.fill_unwritten(&mut imm)).collect();
        (AssocConstraint { alias: c.alias, args, ty: c.ty.fill_unwritten(&mut imm) }, at)
    });
    Predicates {
        predicates: predicates.into(),
        equalities: equalities.collect(),
        at_most: at_most.collect(),
        sources: sources.into(),
        diagnostics: ctx.diagnostics,
    }
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
    /// The effects: the row after `does`. Without `does`, a function whose
    /// return type is written, or that has no body, has none. `None` when
    /// they're inferred from the body. An operation of an effect does that
    /// effect, like `State[S]` for `get`.
    pub effects: Option<Ty<'db>>,
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
        self.ret.as_ref().is_some_and(|t| !t.has_unwritten())
            && self.params.iter().all(Option::is_some)
    }
}

/// The parameters of the function type of `f`, from the types of its
/// declared parameters, in order, as [`FnSig`] has them: the type of
/// `*args` or `**opts` is the tuple `(*Ts)` or `(**Ks)`, whose pack is the
/// rest.
pub fn fn_params<'db>(db: &'db dyn Db, f: FunctionId<'db>, tys: Vec<Ty<'db>>) -> Params<'db> {
    let decl = &function_signature(db, f).params;
    params_from(decl.iter().zip(tys).map(|(p, t)| (p.kind, p.name.clone(), t)))
}

/// Which parameters of `f` have defaults, by where they are in its
/// function type: for each positional parameter, the index of its declared
/// parameter if it has a default, and the keyword-only ones with defaults,
/// by name, with the indices of their declared parameters.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParamDefaults {
    pub positional: Vec<Option<u32>>,
    pub keyword: Vec<(Name, u32)>,
}

impl ParamDefaults {
    pub fn of(db: &dyn Db, f: FunctionId<'_>) -> Self {
        let mut out = ParamDefaults::default();
        for (i, p) in function_signature(db, f).params.iter().enumerate() {
            let default = p.default.map(|_| i as u32);
            match p.kind {
                ParamKind::Positional => out.positional.push(default),
                ParamKind::Keyword => {
                    if let (Some(name), Some(i)) = (&p.name, default) {
                        out.keyword.push((name.clone(), i));
                    }
                }
                ParamKind::Rest | ParamKind::KeywordRest => {}
            }
        }
        out
    }

    pub fn is_empty(&self) -> bool {
        self.keyword.is_empty() && self.positional.iter().all(Option::is_none)
    }

    pub fn has_positional(&self, i: usize) -> bool {
        self.positional.get(i).is_some_and(Option::is_some)
    }
}

/// The parameters of a function type from its declared parameters, in
/// order: how each is passed, its name, and its type,
/// which for `*args` or `**opts` is the tuple `(*Ts)` or `(**Ks)` whose
/// pack is the rest.
fn params_from<'db>(
    params: impl IntoIterator<Item = (ParamKind, Option<Name>, Ty<'db>)>,
) -> Params<'db> {
    let mut positional = Vec::new();
    let mut names = Vec::new();
    let mut rest = None;
    let mut keyword = Vec::new();
    let mut keyword_rest = None;
    // The pack of `(*Ts)` or `(**Ks)`.
    let pack = |t: &Ty<'db>| {
        let params = Params::from_tuple(t);
        params.rest.or(params.keyword_rest).map(|p| *p)
    };
    for (kind, name, t) in params {
        match kind {
            ParamKind::Positional => {
                positional.push(t);
                names.push(Ty::ParamName(name));
            }
            ParamKind::Keyword => keyword.push((name.unwrap_or_else(|| Name::new("_")), t)),
            ParamKind::Rest => rest = pack(&t),
            ParamKind::KeywordRest => keyword_rest = pack(&t),
        }
    }
    Params::new(positional, names, rest, keyword, keyword_rest)
}

/// The types of the declared parameters of `f`, in order, from the
/// parameters of its function type: the inverse of [`fn_params`]. `*args`
/// gets the tuple `(*Ts)` of its pack, and `**opts` the tuple `(**Ks)`.
pub fn declared_param_tys<'db>(
    db: &'db dyn Db,
    f: FunctionId<'db>,
    params: &Params<'db>,
) -> Vec<Ty<'db>> {
    let decl = &function_signature(db, f).params;
    let imm = || Ty::Mutability(Mutability::Imm);
    let mut positional = params.positional.iter();
    decl.iter()
        .map(|p| match p.kind {
            ParamKind::Positional => positional.next().cloned().unwrap_or(Ty::Error),
            ParamKind::Keyword => {
                let name = p.name.as_ref();
                let found = params.keyword.iter().find(|(n, _)| Some(n) == name);
                found.map_or(Ty::Error, |(_, t)| t.clone())
            }
            ParamKind::Rest => {
                // What the rest gets, with the positional parameters that
                // its pack became, if it's solved.
                let mut tys: Vec<Ty<'db>> = positional.by_ref().cloned().collect();
                tys.extend(params.rest.iter().map(|r| Ty::Spread(PackHalf::Positional, r.clone())));
                Ty::tuple(tys.into(), imm())
            }
            ParamKind::KeywordRest => {
                let tys = params.keyword_rest.iter();
                let tys = tys.map(|r| Ty::Spread(PackHalf::Named, r.clone())).collect();
                Ty::record(tys, Vec::new(), imm())
            }
        })
        .collect()
}

#[salsa::tracked(returns(ref))]
pub fn fn_sig<'db>(db: &'db dyn Db, id: FunctionId<'db>) -> FnSig<'db> {
    let sig = function_signature(db, id);
    let resolver = Resolver::for_generic_def(db, GenericDefId::Function(id));
    let mut ctx = TyLoweringContext::new(db, &resolver, &sig.store)
        .with_mutabilities(GenericDefId::Function(id));
    let mut hidden = HiddenParams::new(db, GenericDefId::Function(id));
    let params =
        sig.params.iter().map(|p| ctx.lower_opt_ty(p.type_ref).map(|t| hidden.fill(&t))).collect();
    let ret = match sig.ret_type {
        Some(ret) => Some(ctx.lower_ty(ret)),
        None if !id.item(db).has_body => Some(Ty::unit()),
        None => None,
    };
    let effects = match (&sig.effects, id.container(db)) {
        // An operation does its effect, with the effect's parameters.
        (_, ItemContainerId::Effect(effect)) => {
            let count = GenericDefId::Effect(effect).generic_params(db).type_params.len();
            let args = (0..count as u32)
                .map(|index| Ty::Param(GenericParamId { def: GenericDefId::Effect(effect), index }))
                .collect();
            Some(Ty::row([EffectTy { effect, args }]))
        }
        (Some(terms), _) => Some(ctx.lower_row(terms)),
        (None, _) if ret.is_some() || !id.item(db).has_body => Some(Ty::nothing()),
        (None, _) => None,
    };
    FnSig { params, ret, effects, hidden: hidden.count, diagnostics: ctx.diagnostics }
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

/// Whether the field `index` of `def` can be used in `from`: a struct's
/// field only in its own file, unless it's `pub` or `pub(package)`, like an
/// item. A variant's fields are always visible, as in Rust.
pub fn field_visible<'db>(db: &'db dyn Db, def: VariantDef<'db>, index: usize, from: File) -> bool {
    let VariantDef::Struct(s) = def else { return true };
    let fields = match &struct_signature(db, s).fields {
        FieldList::Record(fields) | FieldList::Tuple(fields) => fields,
        FieldList::Unit => return true,
    };
    fields.get(index).is_none_or(|f| is_visible_from(db, f.visibility, s.file(db), from))
}

/// The first field of `def` that can't be used in `from`, if there is one.
pub fn private_field<'db>(db: &'db dyn Db, def: VariantDef<'db>, from: File) -> Option<Name> {
    let fields = &field_types(db, def).fields;
    (0..fields.len()).find(|&i| !field_visible(db, def, i, from)).map(|i| fields[i].0.clone())
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
    // The fields of an `imm struct` are `imm`, like those of an enum.
    let own = if id.item(db).is_imm { imm() } else { struct_mutability(db, id) };
    // A field can have a mutability the struct declares, like the `'v` of
    // `data: 'v Vec[T]` in `struct VecIter[T, 'v]`.
    let mut ctx = TyLoweringContext::new(db, &resolver, &sig.store)
        .with_mutabilities(GenericDefId::Struct(id));
    lower_fields(&mut ctx, &sig.fields, &own)
}

/// The mutability of a struct as seen from its fields: its hidden
/// parameter, after its type parameters and the mutabilities it declares,
/// which [`struct_args`] gives the value of.
pub fn struct_mutability<'db>(db: &'db dyn Db, id: StructId<'db>) -> Ty<'db> {
    let def = GenericDefId::Struct(id);
    let index = def.generic_params(db).type_params.len() + named_mutabilities(db, def).len();
    Ty::Param(GenericParamId { def, index: index as u32 })
}

/// The arguments to substitute in the field types of a struct of type
/// `Adt(id, args, mutabilities, mutability)`: its type arguments, its
/// mutability arguments, then its mutability.
pub fn struct_args<'db>(
    args: &[Ty<'db>],
    mutabilities: &[Ty<'db>],
    mutability: &Ty<'db>,
) -> Vec<Ty<'db>> {
    args.iter().chain(mutabilities).chain(std::iter::once(mutability)).cloned().collect()
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
fn lower_fields<'db>(
    ctx: &mut TyLoweringContext<'_, 'db>,
    fields: &FieldList,
    own: &Ty<'db>,
) -> FieldTypes<'db> {
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
            (f.name.clone(), fill_field(&ty, own, own))
        })
        .collect();
    FieldTypes { kind, fields, diagnostics: std::mem::take(&mut ctx.diagnostics) }
}

/// Replace the unwritten mutabilities of a field's type with `fill`: the
/// struct's own, `own`, except inside an `imm` array, struct or tuple, where
/// everything is `imm` anyway, so that `imm [Point]` is `imm [imm Point]`
/// (#271). The parameters and result of a function type aren't held by it,
/// so they get `own`.
fn fill_field<'db>(ty: &Ty<'db>, fill: &Ty<'db>, own: &Ty<'db>) -> Ty<'db> {
    let inner =
        |m: &Ty<'db>| if *m == Ty::Mutability(Mutability::Imm) { m.clone() } else { fill.clone() };
    ty.fold(&mut |t| match t {
        Ty::Unwritten => Some(fill.clone()),
        Ty::Array(m, elem) => {
            let m = fill_field(m, fill, own);
            let elem = fill_field(elem, &inner(&m), own);
            Some(Ty::array(m, elem))
        }
        Ty::Adt(id, args, ms, m) => {
            let m = fill_field(m, fill, own);
            // An enum is always `imm`, without saying so, and can hold an
            // `Option[mut Point]`.
            let held = if matches!(id, AdtId::Struct(_)) { inner(&m) } else { fill.clone() };
            let args = args.iter().map(|a| fill_field(a, &held, own)).collect();
            let ms = ms.iter().map(|a| fill_field(a, &held, own)).collect();
            Some(Ty::Adt(*id, args, ms, Box::new(m)))
        }
        Ty::Tuple(tys, m) => {
            let m = fill_field(m, fill, own);
            let tys = tys.iter().map(|t| fill_field(t, &inner(&m), own)).collect();
            Some(Ty::tuple(tys, m))
        }
        Ty::Record(tys, named, m) => {
            let m = fill_field(m, fill, own);
            let tys = tys.iter().map(|t| fill_field(t, &inner(&m), own)).collect();
            let named =
                named.iter().map(|(n, t)| (n.clone(), fill_field(t, &inner(&m), own))).collect();
            Some(Ty::Record(tys, named, Box::new(m)))
        }
        Ty::Fn(params, ret, m, effects) => {
            let params = params.map(|p| fill_field(p, own, own));
            Some(Ty::Fn(
                Box::new(params),
                Box::new(fill_field(ret, own, own)),
                Box::new(fill_field(m, fill, own)),
                effects.clone(),
            ))
        }
        _ => None,
    })
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
    let mut ctx =
        TyLoweringContext::new(db, &resolver, &sig.store).with_mutabilities(GenericDefId::Impl(id));
    let mut hidden = HiddenParams::new(db, GenericDefId::Impl(id));
    let class = ctx.lower_class_constraint(sig.class, None).map(|c| c.map(|t| hidden.fill(t)));
    ImplHeader { class, hidden: hidden.count, diagnostics: ctx.diagnostics }
}

/// The parameters of an impl that its instance doesn't use, like the `U` of
/// `impl[T, U] Show[Box[T]]` or the `'v` of `impl[T, 'v] Iterable[Vec[T]]`,
/// by their index: the type parameters, then the named mutabilities.
/// Choosing the impl for a type decides only what's in its instance, so
/// these can never be decided (#251). Hidden mutabilities are always in it.
/// An instance with an error says nothing, since what's missing may be in
/// the part that couldn't be lowered.
pub fn unused_impl_params<'db>(db: &'db dyn Db, imp: ImplId<'db>) -> Vec<u32> {
    let def = GenericDefId::Impl(imp);
    let Some(class) = &impl_header(db, imp).class else { return Vec::new() };
    if class.args.iter().any(|t| t.any(&mut |t| *t == Ty::Error)) {
        return Vec::new();
    }
    let count = def.generic_params(db).type_params.len() + named_mutabilities(db, def).len();
    let mut used = vec![false; count];
    for arg in class.args.iter() {
        arg.any(&mut |t| {
            if let Ty::Param(p) = t
                && p.def == def
                && let Some(slot) = used.get_mut(p.index as usize)
            {
                *slot = true;
            }
            false
        });
    }
    (0..count as u32).filter(|&i| !used[i as usize]).collect()
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

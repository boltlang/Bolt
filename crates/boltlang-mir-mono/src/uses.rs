//! What the uses of functions and constants in an instance are.
//!
//! A use names a function the way the source does: a class method, an
//! overloaded name, a builtin. [`Cx::function_use`] says what it is in the
//! instance: an instance of a function with a body, a builtin, a function
//! of the host, or a method of a dictionary in erased code.
//!
//! ## Dictionaries
//!
//! Erased code doesn't know which impl a class method of an erased type
//! uses, so it's given a [`Dict`] for each such class constraint of its
//! scheme (see [`Instance::dict_params`]). A dictionary is a record with,
//! for each method and constant of the class, in the order the class has
//! them, the instance [`dict_entries`] gives, and then a dictionary for
//! each superclass, in the order of [`superclasses`].
//!
//! An entry's instance has dictionary parameters of its own: those for the
//! impl's constraints come first, and are the [`Dict::Impl`]'s `subs`,
//! which the record holds. The entry's own constraints come after, and a
//! call through the dictionary gives them ([`Use::Method`]). A class's
//! default method, which an impl doesn't override, has the class itself as
//! its first constraint: that's the dictionary itself.

use boltlang_hir_def::{
    body::{DefWithBody, DefWithBodyId},
    hir::{ExprId, Mutability},
    ids::{ClassId, ConstId, FunctionId, ImplId, ItemContainerId},
    item_tree::AssocItem,
    resolver::GenericDefId,
};
use boltlang_hir_ty::{
    GenericParamId, InferenceResult, Ty,
    builtin::Builtin,
    instances::{impl_const, impl_method},
    lower::{ClassConstraint, generic_predicates, impl_header},
    scheme::generic_param_ids,
    solve::visible_impls,
};

pub use boltlang_mir::ClassItem;
use boltlang_mir::ItemRef;

use crate::{
    instance::{Generics, GenericsError, Instance, Kind, MonoConstraint, generics},
    ty::{MonoMut, MonoTy, Slot, Subst, TyError},
};

/// What a use of a function or constant is in an instance.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum Use<'db> {
    /// A function or constant with a body. `mutabilities` are what the use
    /// gives the instance's mutability parameters, and `dicts` its
    /// dictionary parameters.
    Instance { instance: Instance<'db>, mutabilities: Box<[MonoMut]>, dicts: Box<[Dict<'db>]> },
    /// A function the compiler provides, like `i32_add` or `array_len`.
    Builtin(Builtin),
    /// A function of the host, in an `extern` block, without a body.
    Extern(FunctionId<'db>),
    /// A method or constant of a class, through a dictionary, in erased
    /// code. `item` is the class's method or constant, and `dicts` are the
    /// dictionaries for its own constraints.
    Method { dict: Dict<'db>, item: ClassItem<'db>, dicts: Box<[Dict<'db>]> },
}

/// A dictionary for a class constraint: see the module's documentation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum Dict<'db> {
    /// The instance's `i`th dictionary parameter.
    Param(u32),
    /// The `index`th superclass dictionary that `of` holds.
    Super { of: Box<Dict<'db>>, index: u32 },
    /// The impl `imp` for its type parameters `args`, with dictionaries for
    /// those of its constraints an erased type is in.
    Impl { imp: ImplId<'db>, args: Box<[MonoTy<'db>]>, subs: Box<[Dict<'db>]> },
}

impl<'db> Dict<'db> {
    /// The dictionary as text: `#0` for a parameter, `#0.super1` for a
    /// superclass of one, and `impl Show[(A, B)] at [i32, Bool] with (#0)`
    /// for an impl at its type parameters, with the dictionaries it's given.
    pub fn display(&self, db: &'db dyn boltlang_base_db::Db) -> String {
        match self {
            Dict::Param(i) => format!("#{i}"),
            Dict::Super { of, index } => format!("{}.super{index}", of.display(db)),
            Dict::Impl { imp, args, subs } => {
                let mut s = match &impl_header(db, *imp).class {
                    Some(c) => {
                        let args: Vec<String> =
                            c.args.iter().map(|t| t.display(db).to_string()).collect();
                        format!(
                            "impl {}[{}]",
                            crate::instance::class_name(db, c.class),
                            args.join(", ")
                        )
                    }
                    None => "impl".to_string(),
                };
                if !args.is_empty() {
                    let args: Vec<String> =
                        args.iter().map(|t| t.display(db).to_string()).collect();
                    s.push_str(&format!(" at [{}]", args.join(", ")));
                }
                if !subs.is_empty() {
                    let subs: Vec<String> = subs.iter().map(|d| d.display(db)).collect();
                    s.push_str(&format!(" with ({})", subs.join(", ")));
                }
                s
            }
        }
    }
}

/// Why a use can't be compiled.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub enum UseError {
    /// A type the checker didn't decide, after a type error.
    UnknownType,
    /// A type whose inference depends on itself across files.
    Cycle,
    /// No impl, or no dictionary, gives the class constraint, written out.
    NoImpl(String),
}

impl From<TyError> for UseError {
    fn from(_: TyError) -> Self {
        UseError::UnknownType
    }
}

impl From<GenericsError> for UseError {
    fn from(_: GenericsError) -> Self {
        UseError::Cycle
    }
}

/// An instance being looked at: its types, and which functions are erased.
pub struct Cx<'a, 'db> {
    pub db: &'db dyn boltlang_base_db::Db,
    pub instance: Instance<'db>,
    pub infer: &'db InferenceResult<'db>,
    generics: &'db Generics<'db>,
    slots: Vec<Slot<'db>>,
    dict_params: Vec<MonoConstraint<'db>>,
    erased: &'a [FunctionId<'db>],
}

impl<'a, 'db> Cx<'a, 'db> {
    pub fn new(
        db: &'db dyn boltlang_base_db::Db,
        instance: Instance<'db>,
        erased: &'a [FunctionId<'db>],
    ) -> Result<Self, UseError> {
        let generics = generics(db, instance.body(db)).as_ref().map_err(|e| UseError::from(*e))?;
        let slots = instance.slots(db, generics);
        let infer = boltlang_hir_ty::infer_body(db, instance.body(db));
        let dict_params = instance.dict_params(db);
        Ok(Cx { db, instance, infer, generics, slots, dict_params, erased })
    }

    fn subst(&self) -> Subst<'_, 'db> {
        Subst {
            db: self.db,
            root: self.instance.root(self.db),
            params: &self.generics.params,
            slots: &self.slots,
        }
    }

    /// A type of the body, in the instance.
    pub fn ty(&self, ty: &Ty<'db>) -> Result<MonoTy<'db>, UseError> {
        Ok(self.subst().ty(ty)?)
    }

    /// The type of the instance: that of its function, constant or file.
    pub fn instance_ty(&self) -> Result<MonoTy<'db>, UseError> {
        self.ty(&self.generics.ty)
    }

    pub fn expr_ty(&self, expr: ExprId) -> Result<MonoTy<'db>, UseError> {
        self.ty(self.infer.expr_ty(expr))
    }

    /// What a use in MIR is: `item`, used at the type `use_ty`, which is
    /// what its `Item` node or `Callee` has, in the instance.
    pub fn item_use(
        &self,
        item: &ItemRef<'db>,
        use_ty: &MonoTy<'db>,
    ) -> Result<Use<'db>, UseError> {
        let db = self.db;
        match item {
            ItemRef::Builtin(b) => Ok(Use::Builtin(*b)),
            ItemRef::Extern(f) => Ok(Use::Extern(*f)),
            ItemRef::Fn(f) => self.instance_use(DefWithBody::Function(*f), use_ty, &[]),
            ItemRef::Const(c) => self.instance_use(DefWithBody::Const(*c), use_ty, &[]),
            ItemRef::Method { item, class_args, imp } => {
                let container = match item {
                    ClassItem::Function(f) => f.container(db),
                    ClassItem::Const(c) => c.container(db),
                };
                let ItemContainerId::Class(class) = container else {
                    return Err(UseError::UnknownType);
                };
                let args = match (class_args, item) {
                    (Some(given), _) => self.mono_args(given.iter())?,
                    (None, ClassItem::Function(f)) => self.class_args_of_use(class, use_ty, *f)?,
                    (None, ClassItem::Const(_)) => return Err(UseError::UnknownType),
                };
                self.class_item_use(*imp, class, &args, *item, use_ty)
            }
        }
    }

    /// The use of a method or constant of a class, for a class method whose
    /// class arguments, like the `[i32]` of `Show[i32]`, are `args`, which
    /// syntax uses too, like `Index` for `xs[i]`, with the impl that the
    /// checker chose, if it did.
    fn class_item_use(
        &self,
        chosen: Option<ImplId<'db>>,
        class: ClassId<'db>,
        args: &[MonoTy<'db>],
        item: ClassItem<'db>,
        use_ty: &MonoTy<'db>,
    ) -> Result<Use<'db>, UseError> {
        let db = self.db;
        let constraint = MonoConstraint { class, args: args.into() };
        let imp = if args.iter().any(MonoTy::has_erased) {
            None
        } else {
            match chosen {
                Some(imp) => Some(imp),
                None => Some(
                    self.select_impl(&constraint)
                        .ok_or_else(|| UseError::NoImpl(constraint.display(db)))?,
                ),
            }
        };
        let Some(imp) = imp else {
            // Erased: through a dictionary.
            let dict = self.dict(&constraint)?;
            let dicts = match item {
                ClassItem::Function(f) => self.own_dicts(f, &constraint, use_ty)?,
                ClassItem::Const(_) => Box::default(),
            };
            return Ok(Use::Method { dict, item, dicts });
        };
        let header = self.impl_args(imp, &constraint)?;
        match item {
            ClassItem::Function(f) => {
                let target = impl_method(db, imp, f);
                let given = if target == f { self.class_slots(class, args) } else { header };
                self.instance_use(DefWithBody::Function(target), use_ty, &given)
            }
            ClassItem::Const(c) => {
                let target = impl_const(db, imp, c);
                let given = if target == c { self.class_slots(class, args) } else { header };
                self.instance_use(DefWithBody::Const(target), use_ty, &given)
            }
        }
    }

    /// The dictionaries for a class method's constraints other than its
    /// class, which a call through a dictionary gives.
    fn own_dicts(
        &self,
        f: FunctionId<'db>,
        class: &MonoConstraint<'db>,
        use_ty: &MonoTy<'db>,
    ) -> Result<Box<[Dict<'db>]>, UseError> {
        let callee = generics(self.db, DefWithBodyId::new(self.db, DefWithBody::Function(f)))
            .as_ref()
            .map_err(|e| UseError::from(*e))?;
        let mut found = vec![None; callee.kinds.len()];
        let class_params = generic_param_ids(self.db, [GenericDefId::Class(class.class)]);
        for (p, t) in class_params.iter().zip(class.args.iter()) {
            if let Some(i) = callee.params.iter().position(|q| q == p) {
                found[i] = Some(Slot::Ty(t.clone()));
            }
        }
        bind(&callee.params, &callee.ty, use_ty, &mut found);
        let actual = fill(callee, found);
        let erased = erase(&actual, true);
        let mut out = Vec::new();
        for c in callee.predicates.iter().skip(1) {
            self.push_dict(callee, c, &actual, &erased, &mut out)?;
        }
        Ok(out.into())
    }

    /// The slots that the class arguments `args` give a class method's
    /// scheme: its class's parameters come first.
    fn class_slots(&self, class: ClassId<'db>, args: &[MonoTy<'db>]) -> Vec<Option<Slot<'db>>> {
        let params = generic_param_ids(self.db, [GenericDefId::Class(class)]);
        params
            .iter()
            .enumerate()
            .map(|(i, &p)| {
                if boltlang_hir_ty::ty::is_hidden_param(self.db, p) {
                    None
                } else {
                    args.get(i).cloned().map(Slot::Ty)
                }
            })
            .collect()
    }

    /// The arguments of `class` for a use of its method `f` at the type
    /// `use_ty`.
    fn class_args_of_use(
        &self,
        class: ClassId<'db>,
        use_ty: &MonoTy<'db>,
        f: FunctionId<'db>,
    ) -> Result<Vec<MonoTy<'db>>, UseError> {
        let callee = generics(self.db, DefWithBodyId::new(self.db, DefWithBody::Function(f)))
            .as_ref()
            .map_err(|e| UseError::from(*e))?;
        let mut found = vec![None; callee.kinds.len()];
        bind(&callee.params, &callee.ty, use_ty, &mut found);
        let class_params = generic_param_ids(self.db, [GenericDefId::Class(class)]);
        class_params
            .iter()
            .filter(|&&p| !boltlang_hir_ty::ty::is_hidden_param(self.db, p))
            .map(|p| {
                let i = callee.params.iter().position(|q| q == p);
                match i.and_then(|i| found[i].clone()) {
                    Some(Slot::Ty(t)) => Ok(t),
                    _ => Err(UseError::UnknownType),
                }
            })
            .collect()
    }

    fn mono_args<'t>(
        &self,
        tys: impl Iterator<Item = &'t Ty<'db>>,
    ) -> Result<Vec<MonoTy<'db>>, UseError>
    where
        'db: 't,
    {
        tys.map(|t| self.ty(t)).collect()
    }

    /// The use of a body with the type `use_ty`, with what's already known
    /// of its slots in `given`, like an impl's parameters.
    fn instance_use(
        &self,
        def: DefWithBody<'db>,
        use_ty: &MonoTy<'db>,
        given: &[Option<Slot<'db>>],
    ) -> Result<Use<'db>, UseError> {
        let db = self.db;
        let body = DefWithBodyId::new(db, def);
        let callee = generics(db, body).as_ref().map_err(|e| UseError::from(*e))?;
        let mut found: Vec<Option<Slot<'db>>> = vec![None; callee.kinds.len()];
        for (slot, g) in found.iter_mut().zip(given) {
            *slot = g.clone();
        }
        bind(&callee.params, &callee.ty, use_ty, &mut found);
        let actual = fill(callee, found);
        let is_erased = matches!(def, DefWithBody::Function(f) if self.erased.contains(&f));
        let erased = erase(&actual, is_erased);
        let mut dicts = Vec::new();
        for c in callee.predicates.iter() {
            self.push_dict(callee, c, &actual, &erased, &mut dicts)?;
        }
        let args: Box<[MonoTy<'db>]> = erased
            .iter()
            .filter_map(|s| match s {
                Slot::Ty(t) => Some(t.clone()),
                Slot::Mut(_) | Slot::Row => None,
            })
            .collect();
        let mutabilities = actual
            .iter()
            .filter_map(|s| match s {
                Slot::Mut(m) => Some(*m),
                Slot::Ty(_) | Slot::Row => None,
            })
            .collect();
        let instance = Instance::new(db, body, canonical(&args), self.instance.root(db));
        Ok(Use::Instance { instance, mutabilities, dicts: dicts.into() })
    }

    /// Add the dictionary for the constraint `c` of a callee to `out`, if
    /// the callee's instance, with `erased`, has an erased type in it. It's
    /// the dictionary for what the use gives, `actual`.
    fn push_dict(
        &self,
        callee: &Generics<'db>,
        c: &ClassConstraint<'db>,
        actual: &[Slot<'db>],
        erased: &[Slot<'db>],
        out: &mut Vec<Dict<'db>>,
    ) -> Result<(), UseError> {
        let root = self.instance.root(self.db);
        let at = |slots: &[Slot<'db>]| {
            MonoConstraint::of(&Subst { db: self.db, root, params: &callee.params, slots }, c)
        };
        if at(erased)?.args.iter().any(MonoTy::has_erased) {
            out.push(self.dict(&at(actual)?)?);
        }
        Ok(())
    }

    /// The dictionary for `c` in this instance: one it was given, one a
    /// dictionary it was given holds for a superclass, or an impl's.
    pub fn dict(&self, c: &MonoConstraint<'db>) -> Result<Dict<'db>, UseError> {
        let db = self.db;
        for (i, given) in self.dict_params.iter().enumerate() {
            if let Some(found) = find_super(db, Dict::Param(i as u32), given, c, 0) {
                return Ok(found);
            }
        }
        let imp = self.match_impl(c).ok_or_else(|| UseError::NoImpl(c.display(db)))?;
        let found = self.impl_args(imp, c)?;
        let def = GenericDefId::Impl(imp);
        let params = generic_param_ids(db, [def]);
        let slots = fill_slots(db, &params, found);
        let erased_slots: Vec<Slot<'db>> = slots.clone();
        let mut subs = Vec::new();
        let generics = Generics {
            params: params.clone(),
            kinds: Box::new([]),
            predicates: Box::new([]),
            ty: Ty::unit(),
        };
        for p in generic_predicates(db, def).predicates.iter() {
            self.push_dict(&generics, p, &slots, &erased_slots, &mut subs)?;
        }
        let args = slots
            .into_iter()
            .filter_map(|s| match s {
                Slot::Ty(t) => Some(t),
                Slot::Mut(_) | Slot::Row => None,
            })
            .collect();
        Ok(Dict::Impl { imp, args, subs: subs.into() })
    }

    /// The impl's parameters that the constraint `c` gives, by matching its
    /// instance, like `T = i32` for `impl[T] Show[[T]]` and `Show[[i32]]`.
    fn impl_args(
        &self,
        imp: ImplId<'db>,
        c: &MonoConstraint<'db>,
    ) -> Result<Vec<Option<Slot<'db>>>, UseError> {
        let params = generic_param_ids(self.db, [GenericDefId::Impl(imp)]);
        let mut found = vec![None; params.len()];
        if let Some(target) = &impl_header(self.db, imp).class {
            for (pattern, ty) in target.args.iter().zip(c.args.iter()) {
                bind(&params, pattern, ty, &mut found);
            }
        }
        Ok(found)
    }

    /// The impl for a constraint without erased types. A mutability
    /// parameter in it chooses the impl that both mutabilities choose.
    fn select_impl(&self, c: &MonoConstraint<'db>) -> Option<ImplId<'db>> {
        let root = [self.instance.root(self.db)];
        let pick = |params: Mutability| {
            let c = c.to_constraint(params)?;
            boltlang_hir_ty::instances::select_impl(self.db, &root, c.class, &c.args)
        };
        let imp = pick(Mutability::Imm)?;
        if c.args.iter().any(MonoTy::has_mut_param) && pick(Mutability::Mut) != Some(imp) {
            return None;
        }
        Some(imp)
    }

    /// The impl whose instance fits `c`, which can have erased types: those
    /// fit only a parameter of the impl, since nothing else is known of them.
    fn match_impl(&self, c: &MonoConstraint<'db>) -> Option<ImplId<'db>> {
        if !c.args.iter().any(MonoTy::has_erased) {
            return self.select_impl(c);
        }
        let db = self.db;
        let mut found = None;
        for &imp in visible_impls(db, self.instance.root(db)).iter() {
            let Some(target) = &impl_header(db, imp).class else { continue };
            if target.class != c.class {
                continue;
            }
            let params = generic_param_ids(db, [GenericDefId::Impl(imp)]);
            if target.args.iter().zip(c.args.iter()).all(|(p, t)| fits(&params, p, t)) {
                if found.is_some() {
                    return None;
                }
                found = Some(imp);
            }
        }
        found
    }
}

/// The dictionary for `want` that `dict`, for `have`, gives: itself, or one
/// of the superclass dictionaries it holds, searched breadth-first.
fn find_super<'db>(
    db: &'db dyn boltlang_base_db::Db,
    dict: Dict<'db>,
    have: &MonoConstraint<'db>,
    want: &MonoConstraint<'db>,
    depth: u32,
) -> Option<Dict<'db>> {
    if have == want {
        return Some(dict);
    }
    // A class hierarchy is finite, but a bad one shouldn't hang this.
    if depth > 16 {
        return None;
    }
    for (index, sup) in superclasses(db, have).into_iter().enumerate() {
        let of = Dict::Super { of: Box::new(dict.clone()), index: index as u32 };
        if let Some(found) = find_super(db, of, &sup, want, depth + 1) {
            return Some(found);
        }
    }
    None
}

/// The superclasses of `c`, like `Eq[T]` for `Ord[T]`, in the order a
/// dictionary holds theirs.
pub fn superclasses<'db>(
    db: &'db dyn boltlang_base_db::Db,
    c: &MonoConstraint<'db>,
) -> Vec<MonoConstraint<'db>> {
    let def = GenericDefId::Class(c.class);
    let params = generic_param_ids(db, [def]);
    let slots: Vec<Slot<'db>> = c.args.iter().cloned().map(Slot::Ty).collect();
    // The root doesn't matter: a superclass is written in the class's
    // parameters, without associated types to normalize.
    let root = c.class.file(db);
    let subst = Subst { db, root, params: &params, slots: &slots };
    generic_predicates(db, def)
        .predicates
        .iter()
        .filter_map(|p| MonoConstraint::of(&subst, p).ok())
        .collect()
}

/// The entries of the dictionary for `imp` at `args`, its type
/// parameters: for each method and constant of the class, in order, the
/// instance that gives it.
pub fn dict_entries<'db>(
    db: &'db dyn boltlang_base_db::Db,
    imp: ImplId<'db>,
    args: &[MonoTy<'db>],
    root: boltlang_base_db::File,
) -> Result<Vec<(ClassItem<'db>, Instance<'db>)>, UseError> {
    let Some(target) = &impl_header(db, imp).class else { return Ok(Vec::new()) };
    let class = target.class;
    let impl_params = generic_param_ids(db, [GenericDefId::Impl(imp)]);
    let impl_slots = fill_slots(db, &impl_params, args.iter().cloned().map(|t| Some(Slot::Ty(t))));
    let subst = Subst { db, root, params: &impl_params, slots: &impl_slots };
    let class_args: Vec<MonoTy<'db>> =
        target.args.iter().map(|t| subst.ty(t)).collect::<Result<_, _>>()?;
    let mut out = Vec::new();
    for item in class.item(db).items.iter() {
        let (item, def) = match *item {
            AssocItem::Function(idx) => {
                let f = FunctionId::new(db, ItemContainerId::Class(class), idx);
                let target = impl_method(db, imp, f);
                (ClassItem::Function(f), DefWithBody::Function(target))
            }
            AssocItem::Const(idx) => {
                let c = ConstId::new(db, ItemContainerId::Class(class), idx);
                (ClassItem::Const(c), DefWithBody::Const(impl_const(db, imp, c)))
            }
            _ => continue,
        };
        let body = DefWithBodyId::new(db, def);
        let callee = generics(db, body).as_ref().map_err(|e| UseError::from(*e))?;
        // The impl's own items have its parameters first; a class default
        // has the class's. Whatever else they're generic over is erased.
        let in_impl = match def {
            DefWithBody::Function(f) => matches!(f.container(db), ItemContainerId::Impl(_)),
            DefWithBody::Const(c) => matches!(c.container(db), ItemContainerId::Impl(_)),
            _ => false,
        };
        let (params, given): (Box<[GenericParamId<'db>]>, Vec<MonoTy<'db>>) = if in_impl {
            (impl_params.clone(), args.to_vec())
        } else {
            (generic_param_ids(db, [GenericDefId::Class(class)]), class_args.clone())
        };
        let mut given = given.into_iter();
        let mut instance_args = Vec::new();
        let mut fresh = 0u32;
        for (i, kind) in callee.kinds.iter().enumerate() {
            if *kind != Kind::Ty {
                continue;
            }
            let own = callee.params.get(i).is_some_and(|p| params.contains(p));
            let next = given.next().filter(|_| own);
            instance_args.push(next.unwrap_or_else(|| {
                // Numbered after the impl's, which `canonical` renumbers.
                fresh += 1;
                MonoTy::Erased(u32::MAX - fresh)
            }));
        }
        out.push((item, Instance::new(db, body, canonical(&instance_args), root)));
    }
    Ok(out)
}

/// Whether `ty`, which can have erased types, fits the impl's `pattern`.
fn fits<'db>(params: &[GenericParamId<'db>], pattern: &Ty<'db>, ty: &MonoTy<'db>) -> bool {
    let all = |ps: &[Ty<'db>], ts: &[MonoTy<'db>]| {
        ps.len() == ts.len() && ps.iter().zip(ts).all(|(p, t)| fits(params, p, t))
    };
    match (pattern, ty) {
        (Ty::Param(p), _) if params.contains(p) => true,
        (_, MonoTy::Erased(_) | MonoTy::ErasedAssoc(..)) => false,
        (Ty::Builtin(a), MonoTy::Builtin(b)) => a == b,
        (Ty::Adt(a, xs, ..), MonoTy::Adt(b, ys, ..)) => a == b && all(xs, ys),
        (Ty::Tuple(xs), MonoTy::Tuple(ys)) => all(xs, ys),
        (Ty::Array(_, x), MonoTy::Array(_, y)) => fits(params, x, y),
        (Ty::Fn(xs, x, ..), MonoTy::Fn(ys, y, _)) => all(xs, ys) && fits(params, x, y),
        (Ty::Never, MonoTy::Never) => true,
        _ => false,
    }
}

/// Match a scheme's type, `pattern`, with a use's, setting the slots of
/// `params` and of the bound variables after them that it decides. What's
/// set already stays.
pub(crate) fn bind<'db>(
    params: &[GenericParamId<'db>],
    pattern: &Ty<'db>,
    ty: &MonoTy<'db>,
    found: &mut [Option<Slot<'db>>],
) {
    let index = |t: &Ty<'db>| match t {
        Ty::Param(p) => params.iter().position(|q| q == p),
        Ty::Bound(i) => Some(params.len() + *i as usize),
        _ => None,
    };
    let bind_mut = |p: &Ty<'db>, m: MonoMut, found: &mut [Option<Slot<'db>>]| {
        if let Some(slot) = index(p).and_then(|i| found.get_mut(i)) {
            slot.get_or_insert(Slot::Mut(m));
        }
    };
    let all = |ps: &[Ty<'db>], ts: &[MonoTy<'db>], found: &mut [Option<Slot<'db>>]| {
        for (p, t) in ps.iter().zip(ts) {
            bind(params, p, t, found);
        }
    };
    if let Some(i) = index(pattern) {
        if let Some(slot) = found.get_mut(i) {
            slot.get_or_insert_with(|| Slot::Ty(ty.clone()));
        }
        return;
    }
    match (pattern, ty) {
        (Ty::Adt(a, xs, ms, own), MonoTy::Adt(b, ys, ns, n)) if a == b => {
            all(xs, ys, found);
            for (m, n) in ms.iter().zip(ns.iter()) {
                bind_mut(m, *n, found);
            }
            bind_mut(own, *n, found);
        }
        // A partly applied type constructor, like `F[A]` for `Option[i32]`.
        (Ty::App(head, xs), MonoTy::Adt(b, ys, ns, n)) if ys.len() >= xs.len() => {
            let split = ys.len() - xs.len();
            let head_ty = MonoTy::Adt(*b, ys[..split].into(), ns.clone(), *n);
            bind(params, head, &head_ty, found);
            all(xs, &ys[split..], found);
        }
        (Ty::Tuple(xs), MonoTy::Tuple(ys)) => all(xs, ys, found),
        (Ty::Array(m, x), MonoTy::Array(n, y)) => {
            bind_mut(m, *n, found);
            bind(params, x, y, found);
        }
        (Ty::Fn(xs, x, m, _), MonoTy::Fn(ys, y, n)) => {
            all(xs, ys, found);
            bind(params, x, y, found);
            bind_mut(m, *n, found);
        }
        _ => {}
    }
}

/// The slots of a scheme with what `found` decided, and the rest as `()`
/// for a type, which can't change what the code does, and `imm` for a
/// mutability.
fn fill<'db>(generics: &Generics<'db>, found: Vec<Option<Slot<'db>>>) -> Vec<Slot<'db>> {
    generics
        .kinds
        .iter()
        .zip(found.into_iter().chain(std::iter::repeat(None)))
        .map(|(kind, slot)| match (kind, slot) {
            (Kind::Ty, Some(s @ Slot::Ty(_))) => s,
            (Kind::Mut, Some(s @ Slot::Mut(_))) => s,
            (Kind::Ty, _) => Slot::Ty(MonoTy::unit()),
            (Kind::Mut, _) => Slot::Mut(MonoMut::Imm),
            (Kind::Row, _) => Slot::Row,
        })
        .collect()
}

/// [`fill`] for the parameters of an item, whose kinds come from them.
fn fill_slots<'db>(
    db: &'db dyn boltlang_base_db::Db,
    params: &[GenericParamId<'db>],
    found: impl IntoIterator<Item = Option<Slot<'db>>>,
) -> Vec<Slot<'db>> {
    let kinds: Box<[Kind]> = params.iter().map(|&p| crate::instance::param_kind(db, p)).collect();
    let generics =
        Generics { params: params.into(), kinds, predicates: Box::new([]), ty: Ty::unit() };
    let mut found: Vec<Option<Slot<'db>>> = found.into_iter().collect();
    // Types given for an item's types only, in order: spread them over its
    // type parameters.
    if found.len() < params.len() {
        let mut types = found.into_iter();
        found = generics
            .kinds
            .iter()
            .map(|k| match k {
                Kind::Ty => types.next().flatten(),
                Kind::Mut | Kind::Row => None,
            })
            .collect();
    }
    fill(&generics, found)
}

/// The slots of a callee's instance: `actual`, with every type erased if
/// the callee is.
fn erase<'db>(actual: &[Slot<'db>], erased: bool) -> Vec<Slot<'db>> {
    let mut next = 0;
    actual
        .iter()
        .map(|s| match s {
            Slot::Ty(_) if erased => {
                next += 1;
                Slot::Ty(MonoTy::Erased(next - 1))
            }
            s => s.clone(),
        })
        .collect()
}

/// An instance's types with their erased types numbered in the order they
/// appear, so that the same instance isn't made twice under other numbers.
pub(crate) fn canonical<'db>(args: &[MonoTy<'db>]) -> Box<[MonoTy<'db>]> {
    let (mut map, mut next) = (Vec::new(), 0);
    args.iter().map(|t| t.renumber(&mut map, &mut next)).collect()
}

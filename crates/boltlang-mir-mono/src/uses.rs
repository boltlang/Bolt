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
    Name,
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
    ty::TupleParts,
};

pub use boltlang_mir::ClassItem;
use boltlang_mir::{ItemRef, Shim};

use crate::{
    instance::{
        Generics, GenericsError, Instance, InstanceDef, Kind, MonoConstraint, generics,
        instance_generics,
    },
    ty::{MonoMut, MonoRow, MonoTy, Slot, Subst, TyError, slot_index},
};

/// What a use of a function or constant is in an instance.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum Use<'db> {
    /// A function or constant with a body. `mutabilities` are what the use
    /// gives the instance's mutability parameters, `rows` its row
    /// parameters, and `dicts` its dictionary parameters.
    Instance {
        instance: Instance<'db>,
        mutabilities: Box<[MonoMut]>,
        rows: Box<[MonoRow<'db>]>,
        dicts: Box<[Dict<'db>]>,
    },
    /// A function the compiler provides, like `i32_add` or `array_len`.
    Builtin(Builtin),
    /// A function of the host, with `#[link]`, without a body.
    Extern(FunctionId<'db>),
    /// An operation of an effect, which the nearest handler of its effect
    /// runs: it has no instance. `effect_args` are the arguments of its
    /// effect, like the `[i32]` of `State[i32]`.
    Op { op: FunctionId<'db>, effect_args: Box<[MonoTy<'db>]> },
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
    /// A type that can't be compiled yet.
    Unsupported(&'static str),
}

impl From<TyError> for UseError {
    fn from(e: TyError) -> Self {
        match e {
            TyError::Unknown => UseError::UnknownType,
            TyError::Unsupported(what) => UseError::Unsupported(what),
        }
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
    /// What the checker found in the instance's body, unless it's a shim.
    pub infer: Option<&'db InferenceResult<'db>>,
    generics: Generics<'db>,
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
        let generics = instance_generics(db, instance.def(db)).map_err(UseError::from)?;
        let slots = instance.slots(db, &generics);
        let infer = instance.source_body(db).map(|body| boltlang_hir_ty::infer_body(db, body));
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
        let infer = self.infer.ok_or(UseError::UnknownType)?;
        self.ty(infer.expr_ty(expr))
    }

    /// What `imm e` is in the instance, for an `e` of type `from`: a call of
    /// the deep copy of its type, a [`Shim::ImmDeep`], or `None` when `imm`
    /// doesn't change the type, so that the value is shared as it is.
    ///
    /// The shim's type is the type it copies, with the mutability
    /// parameters of the instance in it as its own bound variables, so that
    /// every instance that copies the same type shares it.
    pub fn imm_use(&self, from: &Ty<'db>) -> Result<Option<Use<'db>>, UseError> {
        let from = self.ty(from)?;
        let to = self.ty(&Ty::Imm(Box::new(self.subst().back(&from)?)))?;
        if to == from {
            return Ok(None);
        }
        let order = std::cell::RefCell::new(Vec::<u32>::new());
        let key = from
            .to_ty_with(
                &|i| {
                    let mut order = order.borrow_mut();
                    let at = order.iter().position(|&j| j == i).unwrap_or_else(|| {
                        order.push(i);
                        order.len() - 1
                    });
                    Ty::Bound(at as u32)
                },
                None,
            )
            .ok_or(UseError::UnknownType)?;
        let root = self.instance.root(self.db);
        boltlang_mir_transform::check_imm_deep(self.db, root, &key).map_err(UseError::NoImpl)?;
        let use_ty = MonoTy::Fn(
            Box::new([from]),
            Box::new([]),
            Box::new(to),
            MonoMut::Imm,
            MonoRow::nothing(),
        );
        self.instance_use(InstanceDef::Shim(Shim::ImmDeep(key)), &use_ty, &[]).map(Some)
    }

    /// The use of the operation `op` as a function, not called, at the type
    /// `use_ty`: an instance of its [`Shim::Op`].
    pub fn op_fn_use(
        &self,
        op: FunctionId<'db>,
        use_ty: &MonoTy<'db>,
    ) -> Result<Use<'db>, UseError> {
        self.instance_use(InstanceDef::Shim(Shim::Op(op)), use_ty, &[])
    }

    /// The use of the operation `op` at the type `use_ty`, which gives the
    /// arguments of its effect. Its own type parameters, like the `T` of
    /// `fn raise[T](e: E) -> T`, are [`MonoTy::Any`] in its declaration.
    fn op_use(&self, op: FunctionId<'db>, use_ty: &MonoTy<'db>) -> Result<Use<'db>, UseError> {
        let db = self.db;
        let ItemContainerId::Effect(effect) = op.container(db) else {
            return Err(UseError::UnknownType);
        };
        let callee = fn_generics(db, op)?;
        let mut found = vec![None; callee.kinds.len()];
        bind(&callee.params, &callee.ty, use_ty, &mut found);
        let params = generic_param_ids(db, [GenericDefId::Effect(effect)]);
        let effect_args = params
            .iter()
            .map(|p| {
                let i = callee.params.iter().position(|q| q == p);
                match i.and_then(|i| found[i].clone()) {
                    Some(Slot::Ty(t)) => Ok(t),
                    _ => Err(UseError::UnknownType),
                }
            })
            .collect::<Result<_, _>>()?;
        Ok(Use::Op { op, effect_args })
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
            ItemRef::Op(f) => self.op_use(*f, use_ty),
            ItemRef::Fn(f) => self.body_use(DefWithBody::Function(*f), use_ty, &[]),
            // Its instance is generic over what the function is, at the type
            // the call uses the function at.
            ItemRef::Default { function, param, of } => {
                let body = DefWithBodyId::new(db, DefWithBody::Function(*function));
                let of = self.ty(of)?;
                self.instance_use_as(
                    InstanceDef::Default(body, *param),
                    &InstanceDef::Body(body),
                    &of,
                    &[],
                )
            }
            ItemRef::Const(c) => self.body_use(DefWithBody::Const(*c), use_ty, &[]),
            ItemRef::Ctor(c) => self.instance_use(InstanceDef::Shim(Shim::Ctor(*c)), use_ty, &[]),
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
                self.body_use(DefWithBody::Function(target), use_ty, &given)
            }
            ClassItem::Const(c) => {
                let target = impl_const(db, imp, c);
                let given = if target == c { self.class_slots(class, args) } else { header };
                self.body_use(DefWithBody::Const(target), use_ty, &given)
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
        let callee = fn_generics(self.db, f)?;
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
        let callee = fn_generics(self.db, f)?;
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

    /// The use of a body in the source with the type `use_ty`.
    fn body_use(
        &self,
        def: DefWithBody<'db>,
        use_ty: &MonoTy<'db>,
        given: &[Option<Slot<'db>>],
    ) -> Result<Use<'db>, UseError> {
        let body = DefWithBodyId::new(self.db, def);
        self.instance_use(InstanceDef::Body(body), use_ty, given)
    }

    /// The use of a body or shim with the type `use_ty`, with what's
    /// already known of its slots in `given`, like an impl's parameters.
    fn instance_use(
        &self,
        def: InstanceDef<'db>,
        use_ty: &MonoTy<'db>,
        given: &[Option<Slot<'db>>],
    ) -> Result<Use<'db>, UseError> {
        let bound = def.clone();
        self.instance_use_as(def, &bound, use_ty, given)
    }

    /// Like [`Cx::instance_use`], with the slots found from `use_ty` as the
    /// type of `bound`, which has the same generics as `def`, like a
    /// function for its default.
    fn instance_use_as(
        &self,
        def: InstanceDef<'db>,
        bound: &InstanceDef<'db>,
        use_ty: &MonoTy<'db>,
        given: &[Option<Slot<'db>>],
    ) -> Result<Use<'db>, UseError> {
        let db = self.db;
        let callee = &instance_generics(db, &def).map_err(UseError::from)?;
        let binding = &instance_generics(db, bound).map_err(UseError::from)?;
        let mut found: Vec<Option<Slot<'db>>> = vec![None; callee.kinds.len()];
        for (slot, g) in found.iter_mut().zip(given) {
            *slot = g.clone();
        }
        bind(&binding.params, &binding.ty, use_ty, &mut found);
        let actual = fill(callee, found);
        let is_erased = match &def {
            InstanceDef::Body(body) => {
                matches!(body.def(db), DefWithBody::Function(f) if self.erased.contains(&f))
            }
            InstanceDef::Default(..) | InstanceDef::Shim(_) => false,
        };
        let erased = erase(&actual, is_erased);
        let mut dicts = Vec::new();
        for c in callee.predicates.iter() {
            self.push_dict(callee, c, &actual, &erased, &mut dicts)?;
        }
        let mut mutabilities: Vec<MonoMut> = actual
            .iter()
            .filter_map(|s| match s {
                Slot::Mut(m) => Some(*m),
                Slot::Ty(_) | Slot::Row(_) => None,
            })
            .collect();
        let mut rows: Vec<MonoRow<'db>> = actual
            .iter()
            .filter_map(|s| match s {
                Slot::Row(r) => Some(r.clone()),
                Slot::Ty(_) | Slot::Mut(_) => None,
            })
            .collect();
        // The mutability parameters of this instance in the type arguments,
        // like the `'m` of `T = 'm [i32]`, become parameters of the callee's
        // instance, after its own, which the use gives (#371).
        // Each row in them, like that of `T = fn() does Log`, becomes a row
        // parameter of the callee's instance, which the use gives, so that
        // instances aren't specialized on rows.
        let own = mutabilities.len() as u32;
        let own_rows = rows.len() as u32;
        let mut free: Vec<u32> = Vec::new();
        let mut free_rows: Vec<MonoRow<'db>> = Vec::new();
        let args: Box<[MonoTy<'db>]> = erased
            .iter()
            .filter_map(|s| match s {
                Slot::Ty(t) => Some(
                    t.map_muts(&mut |m| match m {
                        MonoMut::Param(i) => MonoMut::Param(own + index_in(&mut free, i)),
                        m => m,
                    })
                    .abstract_rows(own_rows, &mut free_rows),
                ),
                Slot::Mut(_) | Slot::Row(_) => None,
            })
            .collect();
        mutabilities.extend(free.iter().map(|&i| MonoMut::Param(i)));
        rows.extend(free_rows);
        let instance = Instance::new(db, def, canonical(&args), self.instance.root(db));
        Ok(Use::Instance {
            instance,
            mutabilities: mutabilities.into(),
            rows: rows.into(),
            dicts: dicts.into(),
        })
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
                Slot::Mut(_) | Slot::Row(_) => None,
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

/// What the function `f` is generic over.
fn fn_generics<'db>(
    db: &'db dyn boltlang_base_db::Db,
    f: FunctionId<'db>,
) -> Result<&'db Generics<'db>, UseError> {
    let body = DefWithBodyId::new(db, DefWithBody::Function(f));
    generics(db, body).as_ref().map_err(|e| UseError::from(*e))
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
        out.push((item, Instance::of_body(db, body, canonical(&instance_args), root)));
    }
    Ok(out)
}

/// The parameters of a function type laid out flat, with the names of the
/// keyword ones at the end, as a tuple, to match [`Params::as_tuple`].
fn params_tuple<'db>(params: &[MonoTy<'db>], names: &[Name]) -> MonoTy<'db> {
    match names.is_empty() {
        true => MonoTy::Tuple(params.into(), MonoMut::Imm),
        false => MonoTy::Record(params.into(), names.into(), MonoMut::Imm),
    }
}

/// Whether the named fields of a checker's tuple type have `names`, in
/// that order.
fn same_names(named: &[(Name, Ty<'_>)], names: &[Name]) -> bool {
    named.len() == names.len() && named.iter().zip(names).all(|((a, _), b)| a == b)
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
        (Ty::Tuple(xs, _), MonoTy::Tuple(ys, _)) => all(xs, ys),
        (Ty::Record(xs, xn, _), MonoTy::Record(ys, names, _)) => {
            let fields: Vec<Ty<'db>> =
                xs.iter().cloned().chain(xn.iter().map(|(_, t)| t.clone())).collect();
            same_names(xn, names) && all(&fields, ys)
        }
        (Ty::Array(_, x), MonoTy::Array(_, y)) => fits(params, x, y),
        (Ty::Fn(xs, x, ..), MonoTy::Fn(ys, names, y, ..)) => {
            let given = match xs.is_positional() && names.is_empty() {
                true => all(&xs.positional, ys),
                false => fits(params, &xs.as_tuple(), &params_tuple(ys, names)),
            };
            given && fits(params, x, y)
        }
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
    let bind_mut = |p: &Ty<'db>, m: MonoMut, found: &mut [Option<Slot<'db>>]| {
        if let Some(slot) = slot_index(params, p).and_then(|i| found.get_mut(i)) {
            slot.get_or_insert(Slot::Mut(m));
        }
    };
    let all = |ps: &[Ty<'db>], ts: &[MonoTy<'db>], found: &mut [Option<Slot<'db>>]| {
        for (p, t) in ps.iter().zip(ts) {
            bind(params, p, t, found);
        }
    };
    // A row where a type goes, like the `R` of `Cont[B, U, R]`.
    if let MonoTy::Row(row) = ty {
        bind_row(params, pattern, row, found);
        return;
    }
    if let Some(i) = slot_index(params, pattern) {
        if let Some(slot) = found.get_mut(i) {
            slot.get_or_insert_with(|| Slot::Ty(ty.clone()));
        }
        return;
    }
    if pattern.has_spread()
        && let Some(parts) = pattern.tuple_parts()
    {
        bind_tuple(params, &parts, ty, found);
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
        (Ty::Tuple(xs, m), MonoTy::Tuple(ys, n)) => {
            all(xs, ys, found);
            bind_mut(m, *n, found);
        }
        (Ty::Record(xs, xn, m), MonoTy::Record(ys, names, n))
            if same_names(xn, names) && xs.len() + xn.len() == ys.len() =>
        {
            let fields: Vec<Ty<'db>> =
                xs.iter().cloned().chain(xn.iter().map(|(_, t)| t.clone())).collect();
            all(&fields, ys, found);
            bind_mut(m, *n, found);
        }
        (Ty::Array(m, x), MonoTy::Array(n, y)) => {
            bind_mut(m, *n, found);
            bind(params, x, y, found);
        }
        (Ty::Fn(xs, x, m, e), MonoTy::Fn(ys, names, y, n, r)) => {
            // With packs or keyword parameters, as tuples.
            match xs.is_positional() && names.is_empty() {
                true => all(&xs.positional, ys, found),
                false => bind(params, &xs.as_tuple(), &params_tuple(ys, names), found),
            }
            bind(params, x, y, found);
            bind_mut(m, *n, found);
            bind_row(params, e, r, found);
        }
        _ => {}
    }
}

/// Match a tuple type with packs spread in it, like `(T, *Ts, x: i32,
/// **R)`, with a use's tuple: the positional fields from both ends, then
/// the positional pack, if there's one, gets the ones in between, as a
/// tuple, and the named pack, if there's one, the named fields the type
/// doesn't name.
fn bind_tuple<'db>(
    params: &[GenericParamId<'db>],
    parts: &TupleParts<'db>,
    ty: &MonoTy<'db>,
    found: &mut [Option<Slot<'db>>],
) {
    let (elems, names, own): (&[MonoTy<'db>], &[Name], MonoMut) = match ty {
        MonoTy::Tuple(elems, own) => (elems, &[], *own),
        MonoTy::Record(elems, names, own) => (elems, names, *own),
        _ => return,
    };
    let set = |pack: &Ty<'db>, value: MonoTy<'db>, found: &mut [Option<Slot<'db>>]| {
        if let Some(slot) = slot_index(params, pack).and_then(|i| found.get_mut(i)) {
            slot.get_or_insert(Slot::Ty(value));
        }
    };
    let split = elems.len() - names.len();
    let (mut tys, named) = (&elems[..split], &elems[split..]);
    let mut pats = &parts.positional[..];
    while let (Some(p), Some(t)) = (pats.first(), tys.first()) {
        if p.is_spread() {
            break;
        }
        bind(params, p, t, found);
        (pats, tys) = (&pats[1..], &tys[1..]);
    }
    while let (Some(p), Some(t)) = (pats.last(), tys.last()) {
        if p.is_spread() {
            break;
        }
        bind(params, p, t, found);
        (pats, tys) = (&pats[..pats.len() - 1], &tys[..tys.len() - 1]);
    }
    if let [Ty::Spread(_, pack)] = pats {
        set(pack, MonoTy::Tuple(tys.into(), MonoMut::Imm), found);
    }
    let mut rest = Vec::new();
    for (name, t) in names.iter().zip(named) {
        match parts.named.iter().find(|(n, _)| n == name) {
            Some((_, p)) => bind(params, p, t, found),
            None => rest.push((name.clone(), t.clone())),
        }
    }
    if let [pack] = &parts.rests[..] {
        let value = if rest.is_empty() {
            MonoTy::unit()
        } else {
            let names = rest.iter().map(|(n, _)| n.clone()).collect();
            MonoTy::Record(rest.into_iter().map(|(_, t)| t).collect(), names, MonoMut::Imm)
        };
        set(pack, value, found);
    }
    if let Some(slot) = slot_index(params, &parts.mutability).and_then(|i| found.get_mut(i)) {
        slot.get_or_insert(Slot::Mut(own));
    }
}

/// Match the row `pattern` of a scheme with a use's `row`: each row it's
/// made of, like the `E` of `Log + E`, is what `row` has besides the
/// effects `pattern` names. With several, like `E + F`, each gets all of
/// it, which is more than it needs, and still fits. A row that's matched
/// more than once gets all of what it's matched with.
fn bind_row<'db>(
    params: &[GenericParamId<'db>],
    pattern: &Ty<'db>,
    row: &MonoRow<'db>,
    found: &mut [Option<Slot<'db>>],
) {
    let Some((effects, rest)) = pattern.row_parts() else { return };
    let mut left = row.clone();
    for e in effects {
        if let Some(given) = row.effects.iter().find(|g| g.effect == e.effect) {
            for (p, t) in e.args.iter().zip(given.args.iter()) {
                bind(params, p, t, found);
            }
        }
        left = left.without(e.effect);
    }
    for part in rest {
        let Some(slot) = slot_index(params, &part).and_then(|i| found.get_mut(i)) else {
            continue;
        };
        *slot = Some(match slot.take() {
            Some(Slot::Row(had)) => Slot::Row(had.union(&left)),
            _ => Slot::Row(left.clone()),
        });
    }
}

/// Where `i` is in `list`, put at its end if it isn't there yet.
fn index_in(list: &mut Vec<u32>, i: u32) -> u32 {
    match list.iter().position(|&j| j == i) {
        Some(at) => at as u32,
        None => {
            list.push(i);
            list.len() as u32 - 1
        }
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
            (Kind::Row, Some(s @ Slot::Row(_))) => s,
            (Kind::Ty, _) => Slot::Ty(MonoTy::unit()),
            (Kind::Mut, _) => Slot::Mut(MonoMut::Imm),
            // A row that nothing gives has no effects.
            (Kind::Row, _) => Slot::Row(MonoRow::nothing()),
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

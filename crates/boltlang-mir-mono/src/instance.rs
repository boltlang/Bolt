//! Instances: a body at concrete types.

use std::fmt;

use boltlang_base_db::{Db, File};
use boltlang_hir_def::{
    Name,
    body::{DefWithBody, DefWithBodyId},
    hir::Mutability,
    ids::{ConstId, FunctionId, ItemContainerId},
    resolver::GenericDefId,
};
use boltlang_hir_ty::{
    GenericParamId, Ty, function_scheme,
    lower::{ClassConstraint, const_ty, identity_args},
    scheme::generic_param_ids,
    solve::{AtMostConstraint, Held},
    ty::is_hidden_param,
};

use boltlang_mir::{Shim, display::container_prefix};
use boltlang_mir_transform::ctor_sig;
use boltlang_pattern::decision::Ctor;

use crate::ty::{MonoMut, MonoRow, MonoTy, Slot, Subst, TyError};

/// A body compiled for concrete types: a function, a constant, the
/// statements at the top level of a file, or a shim that the compiler
/// makes, like a constructor used as a function.
///
/// `args` has a type for each parameter of the body's scheme that's a type,
/// in order: its declared ones, then those it was generalized over. Where a
/// type is [`MonoTy::Erased`], the instance is erased there. The mutability
/// parameters aren't in it, since instances aren't specialized on them:
/// the `i`th one is [`MonoMut::Param`]`(i)` in the instance's types. Except
/// those in `ro`, which a use gives `ro` (#320): Conduit doesn't take that
/// as an argument, so they're [`MonoMut::ReadOnly`] in the instance's types,
/// and the use gives them `imm`, which nothing in the instance uses.
#[salsa::interned(debug)]
pub struct Instance<'db> {
    #[returns(ref)]
    pub def: InstanceDef<'db>,
    #[returns(ref)]
    pub args: Box<[MonoTy<'db>]>,
    /// The entry file of the program, whose visible impls are those that a
    /// class method in the instance can use (#182).
    #[returns(copy)]
    pub root: File,
    /// The mutability parameters that are `ro` in this instance, in order.
    #[returns(ref)]
    pub ro: Box<[u32]>,
}

/// What an instance is of: a body in the source, the default of a
/// parameter of a function, or a shim.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum InstanceDef<'db> {
    Body(DefWithBodyId<'db>),
    /// The default of the `param`th declared parameter of the function
    /// whose body this is, as a function of the parameters before it. It's
    /// generic over what the function is.
    Default(DefWithBodyId<'db>, u32),
    Shim(Shim<'db>),
}

impl<'db> Instance<'db> {
    /// The instance of a body in the source.
    pub fn of_body(
        db: &'db dyn Db,
        body: DefWithBodyId<'db>,
        args: Box<[MonoTy<'db>]>,
        root: File,
    ) -> Self {
        Instance::new(db, InstanceDef::Body(body), args, root, Box::default())
    }

    /// The instance for the top-level statements of `file`.
    pub fn entry(db: &'db dyn Db, file: File) -> Self {
        Instance::of_body(db, DefWithBodyId::new(db, DefWithBody::File(file)), Box::default(), file)
    }

    /// The body in the source the instance is of, unless it's a shim or a
    /// default.
    pub fn body(self, db: &'db dyn Db) -> Option<DefWithBodyId<'db>> {
        match self.def(db) {
            InstanceDef::Body(body) => Some(*body),
            InstanceDef::Default(..) | InstanceDef::Shim(_) => None,
        }
    }

    /// The body in the source whose inference the instance's code has: its
    /// own, or that of the function a default is of.
    pub fn source_body(self, db: &'db dyn Db) -> Option<DefWithBodyId<'db>> {
        match self.def(db) {
            InstanceDef::Body(body) | InstanceDef::Default(body, _) => Some(*body),
            InstanceDef::Shim(_) => None,
        }
    }

    /// The function the instance is of, if it is one.
    pub fn function(self, db: &'db dyn Db) -> Option<FunctionId<'db>> {
        match self.body(db)?.def(db) {
            DefWithBody::Function(f) => Some(f),
            _ => None,
        }
    }

    /// The file the instance's code is in: its body's, or for a shim, the
    /// file of what it's made for.
    pub fn file(self, db: &'db dyn Db) -> Option<File> {
        match self.def(db) {
            InstanceDef::Body(body) | InstanceDef::Default(body, _) => Some(body.def(db).file(db)),
            InstanceDef::Shim(Shim::Ctor(Ctor::Struct(s))) => Some(s.file(db)),
            InstanceDef::Shim(Shim::Ctor(Ctor::Variant(v))) => Some(v.parent.file(db)),
            InstanceDef::Shim(Shim::Op(f)) => match f.container(db) {
                ItemContainerId::Effect(e) => Some(e.file(db)),
                _ => None,
            },
            InstanceDef::Shim(_) => None,
        }
    }

    /// The slots of the instance's scheme: its types, a mutability
    /// parameter for each mutability, and a row parameter for each row.
    pub(crate) fn slots(self, db: &'db dyn Db, generics: &Generics<'db>) -> Vec<Slot<'db>> {
        let mut types = self.args(db).iter();
        let mut muts = 0;
        let mut rows = 0;
        generics
            .kinds
            .iter()
            .map(|kind| match kind {
                Kind::Ty => Slot::Ty(types.next().cloned().unwrap_or_else(MonoTy::unit)),
                Kind::Mut => {
                    muts += 1;
                    match self.ro(db).contains(&(muts - 1)) {
                        true => Slot::Mut(MonoMut::ReadOnly),
                        false => Slot::Mut(MonoMut::Param(muts - 1)),
                    }
                }
                Kind::Row => {
                    rows += 1;
                    Slot::Row(MonoRow::param(rows - 1))
                }
            })
            .collect()
    }

    /// The instance's dictionary parameters: the class constraints of its
    /// scheme that an erased type is in, which its callers give dictionaries
    /// for, in order.
    pub fn dict_params(self, db: &'db dyn Db) -> Vec<MonoConstraint<'db>> {
        let Ok(generics) = instance_generics(db, self.def(db)) else { return Vec::new() };
        let slots = self.slots(db, &generics);
        let subst = Subst { db, root: self.root(db), params: &generics.params, slots: &slots };
        generics
            .predicates
            .iter()
            .filter_map(|c| MonoConstraint::of(&subst, c).ok())
            .filter(|c| c.args.iter().any(MonoTy::has_erased))
            .collect()
    }

    /// How many mutability parameters the instance has: those of its
    /// scheme, then those its type arguments have, which its uses give
    /// (#371).
    pub fn mut_params(self, db: &'db dyn Db) -> usize {
        let own = instance_generics(db, self.def(db))
            .map_or(0, |g| g.kinds.iter().filter(|k| **k == Kind::Mut).count());
        let mut most = own;
        for t in self.args(db).iter() {
            t.map_muts(&mut |m| {
                if let MonoMut::Param(i) = m {
                    most = most.max(i as usize + 1);
                }
                m
            });
        }
        most
    }

    /// What the instance assumes about its mutability parameters: each
    /// mutability in what its scheme says is at most one, with its type
    /// arguments, like `mut ⊑ 'n` for `mk[mut [i32]]` of
    /// `fn mk['n, T: 'n]`. Those that always hold are left out.
    pub fn mut_assumes(self, db: &'db dyn Db) -> Vec<(MonoMut, MonoMut)> {
        let Ok(generics) = instance_generics(db, self.def(db)) else { return Vec::new() };
        let slots = self.slots(db, &generics);
        let subst = Subst { db, root: self.root(db), params: &generics.params, slots: &slots };
        let mut out = Vec::new();
        for c in generics.bounds.iter() {
            let Ok(bound) = subst.mutability(&c.bound) else { continue };
            match &c.held {
                Held::Mutability(m) => out.extend(subst.mutability(m).ok().map(|m| (m, bound))),
                // What an object holds is at most its own mutability, so
                // only the outermost objects' count.
                Held::Type(t) => {
                    if let Ok(t) = subst.ty(t) {
                        out.extend(t.outer_mutability().map(|m| (m, bound)));
                    }
                }
            }
        }
        out.retain(|&(m, bound)| m != bound && m != MonoMut::Imm && bound != MonoMut::Mut);
        out.dedup();
        out
    }

    /// How many row parameters the instance has: those of its scheme, then
    /// those its type arguments have, which its uses give, as for
    /// mutabilities.
    pub fn row_params(self, db: &'db dyn Db) -> usize {
        let own = instance_generics(db, self.def(db))
            .map_or(0, |g| g.kinds.iter().filter(|k| **k == Kind::Row).count());
        let mut found = Vec::new();
        for t in self.args(db).iter() {
            MonoRow::params_in(t, &mut found);
        }
        found.iter().map(|&i| i as usize + 1).fold(own, usize::max)
    }

    pub fn display(self, db: &'db dyn Db) -> impl fmt::Display + 'db {
        DisplayInstance { db, instance: self }
    }
}

struct DisplayInstance<'db> {
    db: &'db dyn Db,
    instance: Instance<'db>,
}

impl fmt::Display for DisplayInstance<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let db = self.db;
        let name = match self.instance.def(db) {
            InstanceDef::Body(body) => match body.def(db) {
                DefWithBody::File(_) => "<file>".to_string(),
                DefWithBody::Function(id) => fn_name(db, id),
                DefWithBody::Const(id) => {
                    item_name(db, id.container(db), id.item(db).name.as_ref())
                }
                DefWithBody::Variant(_) => "<discriminant>".to_string(),
            },
            // `f[i32]::default1`
            InstanceDef::Default(body, param) => {
                let DefWithBody::Function(id) = body.def(db) else { return write!(f, "?") };
                write!(f, "{}", fn_name(db, id))?;
                self.args(f)?;
                return write!(f, "::default{param}");
            }
            InstanceDef::Shim(Shim::Ctor(ctor)) => match ctor {
                Ctor::Struct(s) => name_or(s.item(db).name.as_ref()).to_string(),
                Ctor::Variant(v) => {
                    format!("{}.{}", name_or(v.parent.item(db).name.as_ref()), v.name(db))
                }
                Ctor::Tuple(_) | Ctor::TupleEnd | Ctor::TupleNamed(_) => "?".to_string(),
            },
            InstanceDef::Shim(Shim::ImmDeep(t)) => {
                return write!(f, "imm_deep[{}]", t.display(db));
            }
            InstanceDef::Shim(Shim::Op(op)) => fn_name(db, *op),
        };
        write!(f, "{name}")?;
        self.args(f)
    }
}

/// A function's name, after its class, impl or effect, like `Show.show`.
fn fn_name(db: &dyn Db, f: FunctionId<'_>) -> String {
    item_name(db, f.container(db), f.item(db).display_name().as_ref())
}

/// An item's name, after its container's prefix.
fn item_name(db: &dyn Db, container: ItemContainerId<'_>, name: Option<&Name>) -> String {
    format!("{}{}", container_prefix(db, container), name_or(name))
}

fn name_or(name: Option<&Name>) -> &str {
    name.map_or("?", |n| n.as_str())
}

impl DisplayInstance<'_> {
    fn args(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let db = self.db;
        let args = self.instance.args(db);
        if !args.is_empty() {
            write!(f, "[")?;
            for (i, t) in args.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{}", t.display(db))?;
            }
            write!(f, "]")?;
        }
        Ok(())
    }
}

pub(crate) fn class_name(db: &dyn Db, class: boltlang_hir_def::ids::ClassId<'_>) -> String {
    class.item(db).name.as_ref().map_or_else(|| "?".to_string(), |n| n.as_str().to_string())
}

/// A class constraint at closed types, like `Show[i32]` or `Show[any]`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub struct MonoConstraint<'db> {
    pub class: boltlang_hir_def::ids::ClassId<'db>,
    pub args: Box<[MonoTy<'db>]>,
}

impl<'db> MonoConstraint<'db> {
    pub(crate) fn of(subst: &Subst<'_, 'db>, c: &ClassConstraint<'db>) -> Result<Self, TyError> {
        Ok(MonoConstraint {
            class: c.class,
            args: c.args.iter().map(|t| subst.ty(t)).collect::<Result<_, _>>()?,
        })
    }

    /// The constraint as the checker writes it, with each mutability
    /// parameter as `params`, or `None` with an erased type.
    pub(crate) fn to_constraint(&self, params: Mutability) -> Option<ClassConstraint<'db>> {
        Some(ClassConstraint {
            class: self.class,
            args: self.args.iter().map(|t| t.to_ty(params)).collect::<Option<_>>()?,
        })
    }

    pub fn display(&self, db: &'db dyn Db) -> String {
        let args: Vec<String> = self.args.iter().map(|t| t.display(db).to_string()).collect();
        format!("{}[{}]", class_name(db, self.class), args.join(", "))
    }
}

/// Whether a parameter of a scheme is a type, a mutability, or a row of
/// effects. An instance is only made for each type: mutabilities and rows
/// are its parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, salsa::SalsaValue)]
pub enum Kind {
    Ty,
    Mut,
    Row,
}

/// The kind of the parameter `p` of a scheme.
pub(crate) fn param_kind<'db>(db: &'db dyn Db, p: GenericParamId<'db>) -> Kind {
    if is_hidden_param(db, p) {
        return Kind::Mut;
    }
    match boltlang_hir_ty::kind::generic_param_kinds(db, p.def).get(p.index as usize) {
        Some(boltlang_hir_ty::kind::Kind::Effect) => Kind::Row,
        _ => Kind::Ty,
    }
}

/// What a body is generic over: the parameters of its scheme, as for a
/// function's [`boltlang_hir_ty::Scheme`], with their kinds, its class
/// constraints, and its type, which a use is matched with.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct Generics<'db> {
    pub params: Box<[GenericParamId<'db>]>,
    /// The kind of each parameter, then of each bound variable.
    pub kinds: Box<[Kind]>,
    pub predicates: Box<[ClassConstraint<'db>]>,
    /// What its scheme says must be at most a mutability, like the `T: 'n`
    /// of `fn mk['n, T: 'n]`.
    pub bounds: Box<[AtMostConstraint<'db>]>,
    pub ty: Ty<'db>,
}

impl<'db> Generics<'db> {
    fn none(ty: Ty<'db>) -> Self {
        Generics {
            params: Box::new([]),
            kinds: Box::new([]),
            predicates: Box::new([]),
            bounds: Box::new([]),
            ty,
        }
    }
}

/// Why a body can't be compiled at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, salsa::SalsaValue)]
pub enum GenericsError {
    /// Its type couldn't be inferred: see
    /// [`boltlang_hir_ty::InferenceCycle`].
    Cycle,
}

/// What an instance's body or shim is generic over.
pub fn instance_generics<'db>(
    db: &'db dyn Db,
    def: &InstanceDef<'db>,
) -> Result<Generics<'db>, GenericsError> {
    match def {
        InstanceDef::Body(body) => generics(db, *body).clone(),
        // The function's, with the default's type: a function of the
        // parameters before it.
        InstanceDef::Default(body, param) => {
            let mut g = generics(db, *body).clone()?;
            let (DefWithBody::Function(f), Ty::Fn(params, _, _, effects)) = (body.def(db), &g.ty)
            else {
                return Ok(g);
            };
            let tys = boltlang_hir_ty::lower::declared_param_tys(db, f, params);
            let i = (*param as usize).min(tys.len());
            let ret = tys.get(i).cloned().unwrap_or(Ty::Error);
            g.ty = Ty::func_does(tys[..i].to_vec(), ret, (**effects).clone());
            Ok(g)
        }
        InstanceDef::Shim(Shim::Ctor(ctor)) => {
            let Some(sig) = ctor_sig(db, *ctor) else { return Ok(Generics::none(Ty::unit())) };
            let kinds = (0..sig.params.len())
                .map(|i| if i < sig.type_params { Kind::Ty } else { Kind::Mut })
                .collect();
            let predicates = match sig.params.first() {
                Some(p) => boltlang_hir_ty::lower::generic_predicates(db, p.def).predicates.clone(),
                None => Box::default(),
            };
            Ok(Generics { params: sig.params, kinds, predicates, bounds: Box::new([]), ty: sig.ty })
        }
        // The operation's.
        InstanceDef::Shim(Shim::Op(op)) => {
            let body = DefWithBodyId::new(db, DefWithBody::Function(*op));
            generics(db, body).clone()
        }
        // `fn(T) -> imm T` for the type `T` it copies, whose bound variables
        // are the mutabilities of the code that copies.
        InstanceDef::Shim(Shim::ImmDeep(ty)) => {
            let mut bound = 0;
            ty.any(&mut |t| {
                if let Ty::Bound(i) = t {
                    bound = bound.max(i + 1);
                }
                false
            });
            let ty = Ty::func([ty.clone()], Ty::Imm(Box::new(ty.clone())));
            let kinds = (0..bound).map(|_| Kind::Mut).collect();
            Ok(Generics {
                params: Box::new([]),
                kinds,
                predicates: Box::new([]),
                bounds: Box::new([]),
                ty,
            })
        }
    }
}

/// What `body` is generic over.
#[salsa::tracked(returns(ref))]
pub fn generics<'db>(
    db: &'db dyn Db,
    body: DefWithBodyId<'db>,
) -> Result<Generics<'db>, GenericsError> {
    Ok(match body.def(db) {
        DefWithBody::Function(f) => {
            let scheme = function_scheme(db, f).map_err(|_| GenericsError::Cycle)?;
            let mut kinds: Vec<Kind> = scheme.params.iter().map(|&p| param_kind(db, p)).collect();
            let bound_muts = bound_mutabilities(&scheme.ty, scheme.bound);
            kinds.extend(bound_muts.iter().enumerate().map(|(i, &m)| {
                if scheme.effect_vars.contains(&(i as u32)) {
                    Kind::Row
                } else if m {
                    Kind::Mut
                } else {
                    Kind::Ty
                }
            }));
            Generics {
                params: scheme.params.clone(),
                kinds: kinds.into(),
                predicates: scheme.predicates.clone(),
                bounds: scheme.bounds.clone(),
                ty: scheme.ty.clone(),
            }
        }
        DefWithBody::Const(c) => const_generics(db, c),
        DefWithBody::File(_) | DefWithBody::Variant(_) => Generics::none(Ty::unit()),
    })
}

/// A constant of a class or impl is generic in its parameters, like the
/// `empty` of `impl[T] Monoid[List[T]]`.
fn const_generics<'db>(db: &'db dyn Db, c: ConstId<'db>) -> Generics<'db> {
    let ty = const_ty(db, c).ty.clone();
    let def = match c.container(db) {
        ItemContainerId::Class(class) => GenericDefId::Class(class),
        ItemContainerId::Impl(imp) => GenericDefId::Impl(imp),
        _ => return Generics::none(ty),
    };
    let params = generic_param_ids(db, [def]);
    let kinds = params.iter().map(|&p| param_kind(db, p));
    let predicates: Box<[ClassConstraint<'db>]> = match def {
        GenericDefId::Class(class) => {
            Box::new([ClassConstraint { class, args: identity_args(db, def) }])
        }
        _ => boltlang_hir_ty::lower::generic_predicates(db, def).predicates.clone(),
    };
    Generics { kinds: kinds.collect(), params, predicates, bounds: Box::new([]), ty }
}

/// Which of the `bound` variables of a scheme's type are mutabilities: those
/// it has where a mutability goes.
fn bound_mutabilities(ty: &Ty<'_>, bound: u32) -> Vec<bool> {
    fn walk(ty: &Ty<'_>, out: &mut [bool]) {
        let mark = |m: &Ty<'_>, out: &mut [bool]| {
            if let Ty::Bound(i) = m
                && let Some(slot) = out.get_mut(*i as usize)
            {
                *slot = true;
            }
        };
        match ty {
            Ty::Adt(_, args, ms, own) => {
                ms.iter().for_each(|m| mark(m, out));
                mark(own, out);
                args.iter().for_each(|t| walk(t, out));
            }
            Ty::Array(own, elem) => {
                mark(own, out);
                walk(elem, out);
            }
            Ty::Tuple(tys, own) => {
                mark(own, out);
                tys.iter().for_each(|t| walk(t, out));
            }
            Ty::Record(tys, named, own) => {
                mark(own, out);
                tys.iter().for_each(|t| walk(t, out));
                named.iter().for_each(|(_, t)| walk(t, out));
            }
            Ty::Fn(params, ret, own, _) => {
                mark(own, out);
                params.tys().for_each(|t| walk(t, out));
                walk(ret, out);
            }
            _ => ty.walk_children(|t| walk(t, out)),
        }
    }
    let mut out = vec![false; bound as usize];
    walk(ty, &mut out);
    out
}

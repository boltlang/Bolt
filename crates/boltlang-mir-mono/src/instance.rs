//! Instances: a body at concrete types.

use std::fmt;

use boltlang_base_db::{Db, File};
use boltlang_hir_def::{
    body::{DefWithBody, DefWithBodyId},
    hir::Mutability,
    ids::{ConstId, FunctionId, ItemContainerId},
    resolver::GenericDefId,
};
use boltlang_hir_ty::{
    GenericParamId, Ty, function_scheme,
    lower::{ClassConstraint, const_ty, identity_args, impl_header},
    scheme::generic_param_ids,
    ty::is_hidden_param,
};

use boltlang_mir::Shim;
use boltlang_mir_transform::ctor_sig;
use boltlang_pattern::decision::Ctor;

use crate::ty::{MonoMut, MonoTy, Slot, Subst, TyError};

/// A body compiled for concrete types: a function, a constant, the
/// statements at the top level of a file, or a shim that the compiler
/// makes, like a constructor used as a function.
///
/// `args` has a type for each parameter of the body's scheme that's a type,
/// in order: its declared ones, then those it was generalized over. Where a
/// type is [`MonoTy::Erased`], the instance is erased there. The mutability
/// parameters aren't in it, since instances aren't specialized on them:
/// the `i`th one is [`MonoMut::Param`]`(i)` in the instance's types.
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
}

/// What an instance is of: a body in the source, or a shim.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum InstanceDef<'db> {
    Body(DefWithBodyId<'db>),
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
        Instance::new(db, InstanceDef::Body(body), args, root)
    }

    /// The instance for the top-level statements of `file`.
    pub fn entry(db: &'db dyn Db, file: File) -> Self {
        Instance::of_body(db, DefWithBodyId::new(db, DefWithBody::File(file)), Box::default(), file)
    }

    /// The body in the source the instance is of, unless it's a shim.
    pub fn body(self, db: &'db dyn Db) -> Option<DefWithBodyId<'db>> {
        match self.def(db) {
            InstanceDef::Body(body) => Some(*body),
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
            InstanceDef::Body(body) => Some(body.def(db).file(db)),
            InstanceDef::Shim(Shim::Ctor(Ctor::Struct(s))) => Some(s.file(db)),
            InstanceDef::Shim(Shim::Ctor(Ctor::Variant(v))) => Some(v.parent.file(db)),
            InstanceDef::Shim(Shim::Op(f)) => match f.container(db) {
                ItemContainerId::Effect(e) => Some(e.file(db)),
                _ => None,
            },
            InstanceDef::Shim(_) => None,
        }
    }

    /// The slots of the instance's scheme: its types, and a mutability
    /// parameter for each mutability.
    pub(crate) fn slots(self, db: &'db dyn Db, generics: &Generics<'db>) -> Vec<Slot<'db>> {
        let mut types = self.args(db).iter();
        let mut muts = 0;
        generics
            .kinds
            .iter()
            .map(|kind| match kind {
                Kind::Ty => Slot::Ty(types.next().cloned().unwrap_or_else(MonoTy::unit)),
                Kind::Mut => {
                    muts += 1;
                    Slot::Mut(MonoMut::Param(muts - 1))
                }
                Kind::Row => Slot::Row,
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
        let def = match self.instance.def(db) {
            InstanceDef::Body(body) => body.def(db),
            InstanceDef::Shim(Shim::Ctor(ctor)) => {
                let name = match ctor {
                    Ctor::Struct(s) => s.item(db).name.clone(),
                    Ctor::Variant(v) => {
                        let parent = v.parent.item(db).name.as_ref().map_or("?", |n| n.as_str());
                        Some(boltlang_hir_def::Name::new(format!("{parent}.{}", v.name(db))))
                    }
                    Ctor::Tuple(_) => None,
                };
                write!(f, "{}", name.as_ref().map_or("?", |n| n.as_str()))?;
                return self.args(f);
            }
            InstanceDef::Shim(Shim::ImmDeep(t)) => {
                return write!(f, "imm_deep[{}]", t.display(db));
            }
            InstanceDef::Shim(Shim::Op(op)) => {
                write!(f, "{}", container_prefix(db, op.container(db)))?;
                let name = op.item(db).display_name();
                write!(f, "{}", name.as_ref().map_or("?", |n| n.as_str()))?;
                return self.args(f);
            }
        };
        match def {
            DefWithBody::File(_) => write!(f, "<file>")?,
            DefWithBody::Function(id) => {
                write!(f, "{}", container_prefix(db, id.container(db)))?;
                let name = id.item(db).display_name();
                write!(f, "{}", name.as_ref().map_or("?", |n| n.as_str()))?;
            }
            DefWithBody::Const(id) => {
                write!(f, "{}", container_prefix(db, id.container(db)))?;
                let name = id.item(db).name.clone();
                write!(f, "{}", name.as_ref().map_or("?", |n| n.as_str()))?;
            }
            DefWithBody::Variant(_) => write!(f, "<discriminant>")?,
        }
        self.args(f)
    }
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

/// `Show[i32].` for an item of an impl, `Show.` for one of a class, and
/// `State.` for an operation of an effect.
fn container_prefix(db: &dyn Db, container: ItemContainerId<'_>) -> String {
    match container {
        ItemContainerId::Impl(imp) => match &impl_header(db, imp).class {
            Some(c) => {
                let args: Vec<String> = c.args.iter().map(|t| t.display(db).to_string()).collect();
                format!("{}[{}].", class_name(db, c.class), args.join(", "))
            }
            None => "impl.".to_string(),
        },
        ItemContainerId::Class(class) => format!("{}.", class_name(db, class)),
        ItemContainerId::Effect(effect) => match &effect.item(db).name {
            Some(name) => format!("{name}."),
            None => String::new(),
        },
        ItemContainerId::File(_) | ItemContainerId::ExternBlock(_) => String::new(),
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
/// effects. An instance is only made for each type: it doesn't keep rows,
/// which only the checker needs.
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
    pub ty: Ty<'db>,
}

impl<'db> Generics<'db> {
    fn none(ty: Ty<'db>) -> Self {
        Generics { params: Box::new([]), kinds: Box::new([]), predicates: Box::new([]), ty }
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
        InstanceDef::Shim(Shim::Ctor(ctor)) => {
            let Some(sig) = ctor_sig(db, *ctor) else { return Ok(Generics::none(Ty::unit())) };
            let kinds = (0..sig.params.len())
                .map(|i| if i < sig.type_params { Kind::Ty } else { Kind::Mut })
                .collect();
            let predicates = match sig.params.first() {
                Some(p) => boltlang_hir_ty::lower::generic_predicates(db, p.def).predicates.clone(),
                None => Box::default(),
            };
            Ok(Generics { params: sig.params, kinds, predicates, ty: sig.ty })
        }
        // `fn(T) -> imm T` for the type `T` it copies, whose bound variables
        // are the mutabilities of the code that copies.
        InstanceDef::Shim(Shim::Op(op)) => {
            let body = DefWithBodyId::new(db, DefWithBody::Function(*op));
            generics(db, body).clone()
        }
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
            Ok(Generics { params: Box::new([]), kinds, predicates: Box::new([]), ty })
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
    Generics { kinds: kinds.collect(), params, predicates, ty }
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
            Ty::Fn(params, ret, own, _) => {
                mark(own, out);
                params.iter().for_each(|t| walk(t, out));
                walk(ret, out);
            }
            _ => ty.walk_children(|t| walk(t, out)),
        }
    }
    let mut out = vec![false; bound as usize];
    walk(ty, &mut out);
    out
}

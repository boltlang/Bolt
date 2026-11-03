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

use crate::ty::{MonoMut, MonoTy, Slot, Subst, TyError};

/// A body compiled for concrete types: a function, a constant, or the
/// statements at the top level of a file.
///
/// `args` has a type for each parameter of the body's scheme that's a type,
/// in order: its declared ones, then those it was generalized over. Where a
/// type is [`MonoTy::Erased`], the instance is erased there. The mutability
/// parameters aren't in it, since instances aren't specialized on them:
/// the `i`th one is [`MonoMut::Param`]`(i)` in the instance's types.
#[salsa::interned(debug)]
pub struct Instance<'db> {
    #[returns(copy)]
    pub body: DefWithBodyId<'db>,
    #[returns(ref)]
    pub args: Box<[MonoTy<'db>]>,
    /// The entry file of the program, whose visible impls are those that a
    /// class method in the instance can use (#182).
    #[returns(copy)]
    pub root: File,
}

impl<'db> Instance<'db> {
    /// The instance for the top-level statements of `file`.
    pub fn entry(db: &'db dyn Db, file: File) -> Self {
        Instance::new(db, DefWithBodyId::new(db, DefWithBody::File(file)), Box::default(), file)
    }

    /// The function the instance is of, if it is one.
    pub fn function(self, db: &'db dyn Db) -> Option<FunctionId<'db>> {
        match self.body(db).def(db) {
            DefWithBody::Function(f) => Some(f),
            _ => None,
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
            })
            .collect()
    }

    /// The instance's dictionary parameters: the class constraints of its
    /// scheme that an erased type is in, which its callers give dictionaries
    /// for, in order.
    pub fn dict_params(self, db: &'db dyn Db) -> Vec<MonoConstraint<'db>> {
        let Ok(generics) = generics(db, self.body(db)) else { return Vec::new() };
        let slots = self.slots(db, generics);
        let subst = Subst { db, root: self.root(db), params: &generics.params, slots: &slots };
        generics
            .predicates
            .iter()
            .filter_map(|c| MonoConstraint::of(&subst, c).ok())
            .filter(|c| c.args.iter().any(MonoTy::has_erased))
            .collect()
    }

    /// How many mutability parameters the instance has.
    pub fn mut_params(self, db: &'db dyn Db) -> usize {
        generics(db, self.body(db))
            .as_ref()
            .map_or(0, |g| g.kinds.iter().filter(|k| **k == Kind::Mut).count())
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
        let def = self.instance.body(db).def(db);
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

/// `Show[i32].` for an item of an impl, `Show.` for one of a class.
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

/// Whether a parameter of a scheme is a type or a mutability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, salsa::SalsaValue)]
pub enum Kind {
    Ty,
    Mut,
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

/// What `body` is generic over.
#[salsa::tracked(returns(ref))]
pub fn generics<'db>(
    db: &'db dyn Db,
    body: DefWithBodyId<'db>,
) -> Result<Generics<'db>, GenericsError> {
    Ok(match body.def(db) {
        DefWithBody::Function(f) => {
            let scheme = function_scheme(db, f).map_err(|_| GenericsError::Cycle)?;
            let mut kinds: Vec<Kind> = scheme
                .params
                .iter()
                .map(|&p| if is_hidden_param(db, p) { Kind::Mut } else { Kind::Ty })
                .collect();
            let bound_muts = bound_mutabilities(&scheme.ty, scheme.bound);
            kinds.extend(bound_muts.iter().map(|&m| if m { Kind::Mut } else { Kind::Ty }));
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
    let kinds = params.iter().map(|&p| if is_hidden_param(db, p) { Kind::Mut } else { Kind::Ty });
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
            Ty::Fn(params, ret, own) => {
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

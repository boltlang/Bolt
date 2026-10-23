//! Type schemes: the types of functions, which can be generic, with the
//! class constraints that must hold where they're used.

use std::collections::HashMap;

use boltlang_base_db::Db;
use boltlang_hir_def::{
    ids::{FunctionId, ItemContainerId},
    item_tree::AssocItem,
    resolver::GenericDefId,
};

use crate::{
    lower::{ClassConstraint, fn_sig, generic_predicates, identity_args, impl_header, subst_params},
    solve::{Constraint, Origin, Solver},
    ty::{GenericParamId, Ty, VarKind, bound_name},
};

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct Scheme<'db> {
    /// The declared type parameters the scheme is generic over.
    pub params: Box<[GenericParamId<'db>]>,
    /// The number of inferred type variables it's generic over, which are
    /// [`Ty::Bound`] in `ty`.
    pub bound: u32,
    pub predicates: Box<[ClassConstraint<'db>]>,
    pub ty: Ty<'db>,
}

impl<'db> Scheme<'db> {
    pub fn mono(ty: Ty<'db>) -> Self {
        Scheme { params: Box::new([]), bound: 0, predicates: Box::new([]), ty }
    }

    /// The type with fresh variables for the parameters, adding the
    /// predicates as constraints.
    pub fn instantiate(&self, solver: &mut Solver<'db>, origin: Origin<'db>) -> Ty<'db> {
        let params: HashMap<_, _> = self.params.iter().map(|p| (*p, solver.new_var(VarKind::General))).collect();
        let bound: Vec<_> = (0..self.bound).map(|_| solver.new_var(VarKind::General)).collect();
        let mut subst = |t: &Ty<'db>| {
            t.fold(&mut |t| match t {
                Ty::Param(p) => params.get(p).cloned(),
                Ty::Bound(i) => Some(bound[*i as usize].clone()),
                _ => None,
            })
        };
        for pred in self.predicates.iter() {
            solver.add(Constraint::Class { constraint: pred.map(&mut subst), origin });
        }
        subst(&self.ty)
    }

    /// Show the scheme like `forall a. Show[a] => fn(a) -> a`.
    pub fn display(&self, db: &'db dyn Db) -> String {
        let mut out = String::new();
        let mut names: Vec<String> = self.params.iter().map(|p| Ty::Param(*p).display(db).to_string()).collect();
        names.extend((0..self.bound).map(bound_name));
        if !names.is_empty() {
            out.push_str(&format!("forall {}. ", names.join(" ")));
        }
        for pred in self.predicates.iter() {
            let args: Vec<_> = pred.args.iter().map(|t| t.display(db).to_string()).collect();
            let name = pred.class.item(db).name.as_ref().map_or("?", |n| n.as_str());
            out.push_str(&format!("{name}[{}] => ", args.join(", ")));
        }
        out.push_str(&self.ty.display(db).to_string());
        out
    }
}

/// The items whose type parameters are in scope in a function: its class
/// or impl, then the function itself.
pub fn generic_chain<'db>(db: &'db dyn Db, f: FunctionId<'db>) -> Vec<GenericDefId<'db>> {
    let mut chain = Vec::new();
    match f.container(db) {
        ItemContainerId::Class(c) => chain.push(GenericDefId::Class(c)),
        ItemContainerId::Impl(i) => chain.push(GenericDefId::Impl(i)),
        ItemContainerId::File(_) | ItemContainerId::ExternBlock(_) => {}
    }
    chain.push(GenericDefId::Function(f));
    chain
}

/// The class constraints that hold inside a function: its own, those of
/// its impl, and for a class method, the class itself.
pub fn assumptions<'db>(db: &'db dyn Db, f: FunctionId<'db>) -> Vec<ClassConstraint<'db>> {
    let mut out = Vec::new();
    if let ItemContainerId::Class(c) = f.container(db) {
        out.push(ClassConstraint { class: c, args: identity_args(db, GenericDefId::Class(c)) });
    }
    for def in generic_chain(db, f) {
        if !matches!(def, GenericDefId::Class(_)) {
            out.extend(generic_predicates(db, def).predicates.iter().cloned());
        }
    }
    out
}

/// The parameter and return types a method in a class impl must have,
/// from the class's declaration of the method: for `fn show(x: T) -> String`
/// in `impl Show[i32]`, `fn(i32) -> String`.
pub fn class_impl_method_sig<'db>(db: &'db dyn Db, f: FunctionId<'db>) -> Option<(Box<[Ty<'db>]>, Ty<'db>)> {
    let ItemContainerId::Impl(imp) = f.container(db) else { return None };
    let target = impl_header(db, imp).class.as_ref()?;
    let name = f.item(db).name.as_ref()?;
    let method = target.class.item(db).items.iter().find_map(|item| match *item {
        AssocItem::Function(idx) => {
            let m = FunctionId::new(db, ItemContainerId::Class(target.class), idx);
            (m.item(db).name.as_ref() == Some(name)).then_some(m)
        }
        _ => None,
    })?;
    let sig = fn_sig(db, method);
    let class_def = GenericDefId::Class(target.class);
    let subst = |t: &Ty<'db>| subst_params(t, class_def, &target.args);
    let params = sig.params.iter().map(|t| t.as_ref().map_or(Ty::Error, subst)).collect();
    let ret = sig.ret.as_ref().map_or(Ty::Error, subst);
    Some((params, ret))
}

/// Whether a function's type comes from its declaration alone, so that its
/// body can be checked on its own: its signature is complete, it has no
/// body, or it's in a class or an impl, whose class declares its type.
pub fn is_declared(db: &dyn Db, f: FunctionId<'_>) -> bool {
    fn_sig(db, f).is_complete()
        || !f.item(db).has_body
        || matches!(f.container(db), ItemContainerId::Class(_) | ItemContainerId::Impl(_))
}

/// The type of a function whose type comes from its declaration. Missing
/// types are errors.
#[salsa::tracked(returns(ref))]
pub fn declared_scheme<'db>(db: &'db dyn Db, f: FunctionId<'db>) -> Scheme<'db> {
    let sig = fn_sig(db, f);
    let (params, ret) = match class_impl_method_sig(db, f) {
        Some((class_params, class_ret)) => {
            let params = sig
                .params
                .iter()
                .zip(class_params.iter().chain(std::iter::repeat(&Ty::Error)))
                .map(|(own, class)| own.clone().unwrap_or_else(|| class.clone()))
                .collect();
            (params, sig.ret.clone().unwrap_or(class_ret))
        }
        None => (sig.params.iter().map(|t| t.clone().unwrap_or(Ty::Error)).collect(), sig.ret.clone().unwrap_or(Ty::Error)),
    };
    let mut type_params = Vec::new();
    for def in generic_chain(db, f) {
        let count = def.generic_params(db).type_params.len();
        type_params.extend((0..count).map(|index| GenericParamId { def, index: index as u32 }));
    }
    Scheme {
        params: type_params.into(),
        bound: 0,
        predicates: assumptions(db, f).into(),
        ty: Ty::Fn(params, Box::new(ret)),
    }
}

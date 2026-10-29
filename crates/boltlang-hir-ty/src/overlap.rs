//! Impls that overlap: two impls of a class whose instances some types fit
//! both, like `impl[T] Size[[T]]` and `impl Size[[i32]]` (#276). A
//! constraint those types meet can't choose between them, so the second is
//! an error where it's defined. The same goes for functions with the same
//! name that a call could fit both of (#275).
//!
//! Two instances overlap if they unify once each impl's parameters, its
//! hidden mutabilities too, are fresh variables. Their `where` clauses
//! aren't looked at, as in Haskell, so that an impl added in another file
//! can't make two impls overlap.

use std::collections::HashMap;

use boltlang_base_db::Db;
use boltlang_hir_def::{
    ids::{FunctionId, ImplId, ItemContainerId, ModuleDefId},
    nameres::{Namespace, file_scope, files_used_by, local_scope},
    resolver::GenericDefId,
};

use crate::{
    check::function_scheme,
    lower::{ClassConstraint, impl_header},
    ty::{GenericParamId, Ty, VarKind},
    unify::Unifier,
};

/// The impls that `imp` overlaps and that come before it: earlier in its
/// file, or in a file its file uses, like the prelude. So each pair is
/// reported once, at the impl that comes last.
#[salsa::tracked(returns(ref))]
pub fn overlapped_impls<'db>(db: &'db dyn Db, imp: ImplId<'db>) -> Vec<ImplId<'db>> {
    let file = imp.file(db);
    let earlier = local_scope(db, file).scope.impls().iter().take_while(|&&other| other != imp).copied();
    let used = files_used_by(db, file).iter().flat_map(|&f| local_scope(db, f).scope.impls().iter().copied());
    earlier.chain(used).filter(|&other| impls_overlap(db, imp, other)).collect()
}

/// Whether some types fit the instances of both `a` and `b`.
pub fn impls_overlap<'db>(db: &'db dyn Db, a: ImplId<'db>, b: ImplId<'db>) -> bool {
    let (Some(x), Some(y)) = (&impl_header(db, a).class, &impl_header(db, b).class) else { return false };
    if a == b || x.class != y.class || x.args.len() != y.args.len() {
        return false;
    }
    // An instance with an error is reported on its own, and would fit anything.
    let has_error = |c: &ClassConstraint<'db>| c.args.iter().any(|t| t.any(&mut |t| *t == Ty::Error));
    if has_error(x) || has_error(y) {
        return false;
    }
    let mut unifier = Unifier::new();
    let x = fresh(&mut unifier, x, GenericDefId::Impl(a));
    let y = fresh(&mut unifier, y, GenericDefId::Impl(b));
    x.args.iter().zip(y.args.iter()).all(|(s, t)| unifier.unify(s, t).is_ok())
}

/// `instance` with each parameter of `def` replaced by a new variable.
fn fresh<'db>(unifier: &mut Unifier<'db>, instance: &ClassConstraint<'db>, def: GenericDefId<'db>) -> ClassConstraint<'db> {
    let mut vars: HashMap<GenericParamId<'db>, Ty<'db>> = HashMap::new();
    instance.map(|t| {
        t.fold(&mut |t| match t {
            Ty::Param(p) if p.def == def => Some(vars.entry(*p).or_insert_with(|| unifier.new_var(VarKind::General)).clone()),
            _ => None,
        })
    })
}

/// The functions with the name of `f` that `f` overlaps and that come
/// before it: those of the file defined before it, and those that its
/// imports and the prelude bring in. So each pair in a file is reported
/// once, at the function that comes last (#275).
#[salsa::tracked(returns(ref))]
pub fn overlapped_functions<'db>(db: &'db dyn Db, f: FunctionId<'db>) -> Vec<FunctionId<'db>> {
    let file = f.file(db);
    // Functions in a namespace of a type, like `Vec.new`, aren't overloaded.
    if f.item(db).owner.is_some() || matches!(f.container(db), ItemContainerId::Impl(_)) {
        return Vec::new();
    }
    let Some(name) = f.item(db).name.as_ref().map(|n| n.name().clone()) else { return Vec::new() };
    let entries: Vec<_> = file_scope(db, file).scope.get_all(Namespace::Values, &name).collect();
    let Some(own) = entries.iter().position(|e| e.def == ModuleDefId::Function(f)) else { return Vec::new() };
    entries
        .iter()
        .enumerate()
        .filter(|(i, e)| *i < own || e.import.is_some() || e.prelude)
        .filter_map(|(_, e)| match e.def {
            ModuleDefId::Function(g) if g != f && functions_overlap(db, f, g) => Some(g),
            _ => None,
        })
        .collect()
}

/// Whether a call could fit both `a` and `b`: they take as many arguments,
/// and their parameter types unify once each one's type parameters are new
/// variables. What they give isn't looked at, since only the arguments
/// choose a function.
pub fn functions_overlap<'db>(db: &'db dyn Db, a: FunctionId<'db>, b: FunctionId<'db>) -> bool {
    let (Ok(x), Ok(y)) = (function_scheme(db, a), function_scheme(db, b)) else { return false };
    let mut unifier = Unifier::new();
    let x = x.probe_with(&mut || unifier.new_var(VarKind::General));
    let y = y.probe_with(&mut || unifier.new_var(VarKind::General));
    let (Ty::Fn(xs, _, _), Ty::Fn(ys, _, _)) = (&x, &y) else { return false };
    // A type with an error is reported on its own, and would fit anything.
    if xs.len() != ys.len() || xs.iter().chain(ys.iter()).any(|t| t.any(&mut |t| *t == Ty::Error)) {
        return false;
    }
    xs.iter().zip(ys.iter()).all(|(s, t)| unifier.unify(s, t).is_ok())
}

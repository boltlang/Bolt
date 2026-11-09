//! Which impl a use of a class method or constant uses, once the types it's
//! used with are known. The type checker records the type arguments of each
//! use ([`crate::InferenceResult::type_args`]); in a generic function they
//! can be its own type parameters, which the code generator and the
//! evaluator replace with the types they run it with, and then choose the
//! impl here (#182, #236).

use std::collections::HashMap;

use boltlang_base_db::{Db, File};
use boltlang_hir_def::{
    ids::{ClassId, ConstId, FunctionId, ImplId, ItemContainerId},
    item_tree::AssocItem,
    resolver::GenericDefId,
};

use crate::{
    Scheme,
    lower::{identity_args, impl_header},
    solve::{Match, match_all, normalize_assoc, visible_impls},
    ty::{GenericParamId, Ty},
};

/// `ty` with `args`, the type arguments of a scheme with `params`, for its
/// parameters and then its bound variables. What `args` leaves out stays.
pub fn subst_scheme_args<'db>(
    params: &[GenericParamId<'db>],
    args: &[Ty<'db>],
    ty: &Ty<'db>,
) -> Ty<'db> {
    ty.fold(&mut |t| match t {
        Ty::Param(p) => params.iter().position(|q| q == p).and_then(|i| args.get(i).cloned()),
        Ty::Bound(i) => args.get(params.len() + *i as usize).cloned(),
        _ => None,
    })
}

/// The arguments of `class` for a use of one of its methods or constants
/// whose scheme has `params` and that gives them `type_args`, like `[i32]`
/// for the `show` of `show(1)`.
pub fn class_args<'db>(
    db: &'db dyn Db,
    class: ClassId<'db>,
    params: &[GenericParamId<'db>],
    type_args: &[Ty<'db>],
) -> Box<[Ty<'db>]> {
    identity_args(db, GenericDefId::Class(class))
        .iter()
        .map(|t| subst_scheme_args(params, type_args, t))
        .collect()
}

/// The types that a use of type `ty` gives the parameters and then the bound
/// variables of `scheme`, found by matching `ty` with the scheme's type,
/// like `[i32]` for `id` at `fn(i32) -> i32`. `None` for those that `ty`
/// doesn't decide.
pub fn bind_scheme_args<'db>(scheme: &Scheme<'db>, ty: &Ty<'db>) -> Vec<Option<Ty<'db>>> {
    let mut found = vec![None; scheme.params.len() + scheme.bound as usize];
    bind(&scheme.ty, ty, scheme, &mut found);
    found
}

fn bind<'db>(pattern: &Ty<'db>, ty: &Ty<'db>, scheme: &Scheme<'db>, found: &mut [Option<Ty<'db>>]) {
    let all = |found: &mut [Option<Ty<'db>>], xs: &[Ty<'db>], ys: &[Ty<'db>]| {
        for (x, y) in xs.iter().zip(ys) {
            bind(x, y, scheme, found);
        }
    };
    match (pattern, ty) {
        (Ty::Param(p), _) => {
            if let Some(i) = scheme.params.iter().position(|q| q == p) {
                found[i].get_or_insert_with(|| ty.clone());
            }
        }
        (Ty::Bound(i), _) => {
            if let Some(slot) = found.get_mut(scheme.params.len() + *i as usize) {
                slot.get_or_insert_with(|| ty.clone());
            }
        }
        (Ty::Adt(a, xs, ms, m), Ty::Adt(b, ys, ns, n)) if a == b => {
            all(found, xs, ys);
            all(found, ms, ns);
            bind(m, n, scheme, found);
        }
        (Ty::Tuple(xs, m), Ty::Tuple(ys, n)) => {
            all(found, xs, ys);
            bind(m, n, scheme, found);
        }
        // Only the same names say which field is which.
        (Ty::Record(xs, xn, m), Ty::Record(ys, yn, n))
            if xs.len() == ys.len()
                && xn.len() == yn.len()
                && xn.iter().zip(yn.iter()).all(|((a, _), (b, _))| a == b) =>
        {
            all(found, xs, ys);
            for ((_, x), (_, y)) in xn.iter().zip(yn.iter()) {
                bind(x, y, scheme, found);
            }
            bind(m, n, scheme, found);
        }
        (Ty::Array(m, x), Ty::Array(n, y)) => {
            bind(m, n, scheme, found);
            bind(x, y, scheme, found);
        }
        (Ty::Fn(xs, x, m, e), Ty::Fn(ys, y, n, f)) => {
            match xs.is_positional() && ys.is_positional() {
                true => all(found, &xs.positional, &ys.positional),
                false => bind(&xs.as_tuple(), &ys.as_tuple(), scheme, found),
            }
            bind(x, y, scheme, found);
            bind(m, n, scheme, found);
            bind(e, f, scheme, found);
        }
        _ => {}
    }
}

/// Whether a type is known well enough to choose an impl by.
pub fn is_concrete(ty: &Ty<'_>) -> bool {
    !ty.any(&mut |t| matches!(t, Ty::Var(_) | Ty::Param(_) | Ty::Bound(_) | Ty::Error))
}

/// The impl of `class` for `args`, among the impls visible from `files`:
/// the program's files, which see every impl that the type checker could
/// have chosen. `None` if no impl fits them, more than one does, or they
/// aren't known well enough.
pub fn select_impl<'db>(
    db: &'db dyn Db,
    files: &[File],
    class: ClassId<'db>,
    args: &[Ty<'db>],
) -> Option<ImplId<'db>> {
    let file = *files.first()?;
    let args: Vec<Ty<'db>> = args.iter().map(|t| normalize_assoc(db, file, t)).collect();
    if !args.iter().all(is_concrete) {
        return None;
    }
    let mut impls: Vec<ImplId<'db>> = Vec::new();
    for &file in files {
        impls.extend(
            visible_impls(db, file).iter().filter(|i| !impls.contains(i)).collect::<Vec<_>>(),
        );
    }
    let mut found = None;
    for imp in impls {
        let Some(target) = &impl_header(db, imp).class else { continue };
        if target.class != class {
            continue;
        }
        if let Match::Yes = match_all(&target.args, &args, &mut HashMap::new()) {
            if found.is_some() {
                return None;
            }
            found = Some(imp);
        }
    }
    found
}

/// The function of `imp` for the class method `method`, or the method
/// itself if the impl leaves it to the class's default.
pub fn impl_method<'db>(
    db: &'db dyn Db,
    imp: ImplId<'db>,
    method: FunctionId<'db>,
) -> FunctionId<'db> {
    let name = &method.item(db).name;
    imp.item(db)
        .items
        .iter()
        .find_map(|item| match *item {
            AssocItem::Function(idx) => {
                let f = FunctionId::new(db, ItemContainerId::Impl(imp), idx);
                (f.item(db).name == *name).then_some(f)
            }
            _ => None,
        })
        .unwrap_or(method)
}

/// The const of `imp` for the class constant `c`, or `c` itself if the impl
/// leaves it to the class's default.
pub fn impl_const<'db>(db: &'db dyn Db, imp: ImplId<'db>, c: ConstId<'db>) -> ConstId<'db> {
    let name = &c.item(db).name;
    imp.item(db)
        .items
        .iter()
        .find_map(|item| match *item {
            AssocItem::Const(idx) => {
                let own = ConstId::new(db, ItemContainerId::Impl(imp), idx);
                (own.item(db).name == *name).then_some(own)
            }
            _ => None,
        })
        .unwrap_or(c)
}

#[cfg(test)]
mod tests;

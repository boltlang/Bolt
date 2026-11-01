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
    lower::{identity_args, impl_header},
    solve::{Match, match_all, normalize_assoc, visible_impls},
    ty::{GenericParamId, Ty},
};

/// `ty` with `args`, the type arguments of a scheme with `params`, for its
/// parameters and then its bound variables. What `args` leaves out stays.
pub fn subst_scheme_args<'db>(params: &[GenericParamId<'db>], args: &[Ty<'db>], ty: &Ty<'db>) -> Ty<'db> {
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
    identity_args(db, GenericDefId::Class(class)).iter().map(|t| subst_scheme_args(params, type_args, t)).collect()
}

/// Whether a type is known well enough to choose an impl by.
pub fn is_concrete(ty: &Ty<'_>) -> bool {
    !ty.any(&mut |t| matches!(t, Ty::Var(_) | Ty::Param(_) | Ty::Bound(_) | Ty::Error))
}

/// The impl of `class` for `args`, among the impls visible from `files`:
/// the program's files, which see every impl that the type checker could
/// have chosen. `None` if no impl fits them, more than one does, or they
/// aren't known well enough.
pub fn select_impl<'db>(db: &'db dyn Db, files: &[File], class: ClassId<'db>, args: &[Ty<'db>]) -> Option<ImplId<'db>> {
    let file = *files.first()?;
    let args: Vec<Ty<'db>> = args.iter().map(|t| normalize_assoc(db, file, t)).collect();
    if !args.iter().all(is_concrete) {
        return None;
    }
    let mut impls: Vec<ImplId<'db>> = Vec::new();
    for &file in files {
        impls.extend(visible_impls(db, file).iter().filter(|i| !impls.contains(i)).collect::<Vec<_>>());
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
pub fn impl_method<'db>(db: &'db dyn Db, imp: ImplId<'db>, method: FunctionId<'db>) -> FunctionId<'db> {
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

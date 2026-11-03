//! Finding the impl that a call of a class method uses when no types say,
//! by the values of its arguments. A use in a body has its impl from the
//! types the body is run with (`Evaluator::impl_at`, #236); this is for a
//! call from outside, like `debug_string(x)` for `Evaluator::show`, which
//! has none. Numbers of different types can't be told apart this way.

use boltlang_base_db::{Db, File};
use boltlang_hir_def::{
    builtin_type::BuiltinType,
    ids::{ClassId, FunctionId, ImplId, ItemContainerId},
    resolver::GenericDefId,
};
use boltlang_hir_ty::{
    AdtId, GenericParamId, Ty,
    lower::{fn_sig, impl_header},
    solve::visible_impls,
};

use crate::value::{Ctor, Value};

/// The class that `f` is a method of, if it's one.
pub(crate) fn method_class<'db>(db: &'db dyn Db, f: FunctionId<'db>) -> Option<ClassId<'db>> {
    match f.container(db) {
        ItemContainerId::Class(class) => Some(class),
        _ => None,
    }
}

/// The impl of the class of `method` that a call with `args` uses, chosen
/// by the arguments whose declared type is one of the class's parameters,
/// like `c` in `fn get(c: C, i: i32)`. `None` if no impl, or more than one,
/// fits them.
pub(crate) fn impl_for_args<'db>(
    db: &'db dyn Db,
    files: &[File],
    method: FunctionId<'db>,
    args: &[Value<'db>],
) -> Option<ImplId<'db>> {
    let class = method_class(db, method)?;
    let class_def = GenericDefId::Class(class);
    // The arguments that decide, with the class parameter each one is.
    let deciding: Vec<(usize, usize)> = fn_sig(db, method)
        .params
        .iter()
        .enumerate()
        .filter_map(|(i, ty)| match ty {
            Some(Ty::Param(GenericParamId { def, index })) if *def == class_def => {
                Some((i, *index as usize))
            }
            _ => None,
        })
        .collect();
    if deciding.is_empty() {
        return None;
    }
    // The impls visible from the class, and from the files being run, which
    // may have impls of their own.
    let mut impls: Vec<ImplId<'db>> = visible_impls(db, method.file(db)).clone();
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
        let fits =
            deciding.iter().all(|&(arg, param)| match (args.get(arg), target.args.get(param)) {
                (Some(value), Some(ty)) => fits(value, ty),
                _ => false,
            });
        if fits {
            if found.is_some() {
                return None;
            }
            found = Some(imp);
        }
    }
    found
}

/// Whether `value` can be of the type `ty` of an impl's header, as far as
/// a value tells. Numbers of different types can't be told apart.
fn fits<'db>(value: &Value<'db>, ty: &Ty<'db>) -> bool {
    match (ty, value) {
        (Ty::Param(_) | Ty::Error, _) => true,
        (Ty::Adt(AdtId::Struct(s), ..), Value::Adt { ctor: Ctor::Struct(t), .. }) => s == t,
        (Ty::Adt(AdtId::Enum(e), ..), Value::Adt { ctor: Ctor::Variant(v), .. }) => v.parent == *e,
        (Ty::Array(..), Value::Array(_)) => true,
        (Ty::Tuple(tys), Value::Tuple(values)) => {
            tys.len() == values.len() && tys.iter().zip(values).all(|(t, v)| fits(v, t))
        }
        (Ty::Fn(..), Value::Function(..) | Value::Closure(_) | Value::Constructor(_)) => true,
        (Ty::Builtin(ty), value) => matches!(
            (ty, value),
            (BuiltinType::Bool, Value::Bool(_))
                | (BuiltinType::Int(_) | BuiltinType::Uint(_), Value::Int(_))
                | (BuiltinType::Float(_), Value::Float(_))
        ),
        _ => false,
    }
}

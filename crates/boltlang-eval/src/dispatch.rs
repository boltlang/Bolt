//! Finding the impl that a use of a class method or constant uses. The
//! type checker records it where the types decide it, like for `show(1)`.
//! In a generic function, like `fn f(x) = show(x)`, they don't, and the
//! impl is chosen at run time, by the values of the arguments whose type
//! is a parameter of the class.

use boltlang_base_db::{Db, File};
use boltlang_hir_def::{
    builtin_type::BuiltinType,
    ids::{ClassId, ConstId, FunctionId, ImplId, ItemContainerId},
    item_tree::AssocItem,
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

/// The function of `imp` for the class method `method`, or the method
/// itself if the impl leaves it to the class's default.
pub(crate) fn impl_method<'db>(db: &'db dyn Db, imp: ImplId<'db>, method: FunctionId<'db>) -> FunctionId<'db> {
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
pub(crate) fn impl_const<'db>(db: &'db dyn Db, imp: ImplId<'db>, c: ConstId<'db>) -> ConstId<'db> {
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
            Some(Ty::Param(GenericParamId { def, index })) if *def == class_def => Some((i, *index as usize)),
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
        impls.extend(visible_impls(db, file).iter().filter(|i| !impls.contains(i)).collect::<Vec<_>>());
    }
    let mut found = None;
    for imp in impls {
        let Some(target) = &impl_header(db, imp).class else { continue };
        if target.class != class {
            continue;
        }
        let fits = deciding.iter().all(|&(arg, param)| match (args.get(arg), target.args.get(param)) {
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
        (Ty::Fn(..), Value::Function(_) | Value::Closure(_) | Value::Constructor(_)) => true,
        (Ty::Builtin(ty), value) => matches!(
            (ty, value),
            (BuiltinType::Bool, Value::Bool(_))
                | (BuiltinType::Char, Value::Char(_))
                | (BuiltinType::String, Value::Str(_))
                | (BuiltinType::Int(_) | BuiltinType::Uint(_), Value::Int(_))
                | (BuiltinType::Float(_), Value::Float(_))
        ),
        _ => false,
    }
}

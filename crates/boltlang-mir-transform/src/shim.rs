//! Shims: bodies that the compiler makes for what has none in the source,
//! as rustc's are (`rustc_mir_transform::shim`).

use boltlang_base_db::{Db, File};
use boltlang_hir_def::{Name, resolver::GenericDefId};
use boltlang_hir_ty::{
    GenericParamId, Ty,
    instances::select_impl,
    lang::LangClass,
    lower::{ClassConstraint, VariantDef, field_types, fn_sig, named_mutabilities},
    solve::{imm_of, missing_instances},
    ty::Mutability,
};
use boltlang_mir::{Body, BodyOwner, ClassItem, ExprId, ExprKind, ItemRef, Local, Shim};
use boltlang_pattern::decision::Ctor;

/// What a constructor is generic over, as a function: the type parameters
/// of its struct or enum, then a struct's named mutabilities, and its type,
/// `fn(fields) -> T`. The value it makes is `imm`, as a struct literal
/// without `mut` is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CtorSig<'db> {
    pub params: Box<[GenericParamId<'db>]>,
    /// How many of `params` are types: the others are mutabilities.
    pub type_params: usize,
    pub ty: Ty<'db>,
}

/// The signature of `ctor`, which must be a struct's or a variant's.
pub fn ctor_sig<'db>(db: &'db dyn Db, ctor: Ctor<'db>) -> Option<CtorSig<'db>> {
    let def = match ctor {
        Ctor::Struct(s) => VariantDef::Struct(s),
        Ctor::Variant(v) => VariantDef::Variant(v),
        Ctor::Tuple(_) | Ctor::TupleEnd | Ctor::TupleNamed(_) => return None,
    };
    let (adt, generic) = def.adt();
    let type_params = generic.generic_params(db).type_params.len();
    let named = match generic {
        GenericDefId::Struct(_) => named_mutabilities(db, generic).len(),
        _ => 0,
    };
    let param = |index: usize| GenericParamId { def: generic, index: index as u32 };
    let params: Box<[GenericParamId<'db>]> = (0..type_params + named).map(param).collect();
    let imm = Ty::Mutability(Mutability::Imm);
    // A struct's own mutability is the parameter after its named ones.
    let own = Ty::Param(param(type_params + named));
    let fields: Vec<Ty<'db>> = field_types(db, def)
        .fields
        .iter()
        .map(|(_, t)| t.fold(&mut |t| (*t == own).then(|| imm.clone())))
        .collect();
    let args: Vec<Ty<'db>> = (0..type_params).map(|i| Ty::Param(param(i))).collect();
    let mutabilities: Vec<Ty<'db>> =
        (type_params..type_params + named).map(|i| Ty::Param(param(i))).collect();
    let made = Ty::adt(adt, args, mutabilities, imm);
    Some(CtorSig { params, type_params, ty: Ty::func(fields, made) })
}

/// The MIR of a shim, in the program whose entry file is `root`, whose
/// impls are those it can call.
pub fn shim_body<'db>(db: &'db dyn Db, shim: &Shim<'db>, root: File) -> Body<'db> {
    let mut body = Body::new(BodyOwner::Shim(shim.clone()));
    match shim {
        Shim::Ctor(ctor) => {
            let Some(CtorSig { ty: Ty::Fn(fields, made, ..), .. }) = ctor_sig(db, *ctor) else {
                body.replace(body.root, ExprKind::Unsupported("this constructor"));
                return body;
            };
            let mut values = Vec::new();
            for ty in fields.positional.iter() {
                let local = body.new_temp(None, ty.clone());
                body.params.push(local);
                values.push(body.alloc(ExprKind::Local(local), ty.clone(), None));
            }
            let kind = ExprKind::Construct { ctor: *ctor, fields: values.into() };
            body.root = body.alloc(kind, *made, None);
        }
        // `fn(args) = perform op(args)`, in the parameters of the operation's
        // effect.
        Shim::Op(op) => {
            let sig = fn_sig(db, *op);
            let fill = |t: &Ty<'db>| t.fill_unwritten(&mut || Ty::Mutability(Mutability::Imm));
            let params: Vec<Ty<'db>> =
                sig.params.iter().map(|p| p.as_ref().map_or(Ty::Error, fill)).collect();
            let ret = sig.ret.as_ref().map_or_else(Ty::unit, fill);
            let mut args = Vec::new();
            for ty in &params {
                let local = body.new_temp(None, ty.clone());
                body.params.push(local);
                args.push(body.local_expr(local));
            }
            body.root = body.call(ItemRef::Op(*op), args, ret);
        }
        Shim::ImmDeep(ty) => {
            let x = body.new_temp(Some(Name::new("x")), ty.clone());
            body.params.push(x);
            body.root = imm_glue(db, root, &mut body, x, ty);
        }
    }
    body
}

/// The deep copy of `imm e` for a value of type `ty`, the local `x`: a
/// call of the type's `to_imm_deep`, or the value itself when `imm`
/// doesn't change the type, since it has nothing to copy.
///
/// The type has no type parameters, since the glue is made for each type it
/// copies, as rustc's drop glue is. Its mutabilities can be those of the
/// code that copies it, which are the shim's bound variables.
fn imm_glue<'db>(
    db: &'db dyn Db,
    root: File,
    body: &mut Body<'db>,
    x: Local,
    ty: &Ty<'db>,
) -> ExprId {
    let to = imm_of(db, root, ty);
    let value = body.local_expr(x);
    if to == *ty {
        return value;
    }
    match to_imm_deep(db, root, ty) {
        Ok(item) => body.call(item, vec![value], to),
        // Monomorphization reports it, before it asks for the glue.
        Err(_) => {
            body.replace(value, ExprKind::Unsupported("a copy without `ToImmDeep`"));
            value
        }
    }
}

/// Whether the deep copy of `ty` can be made: an error with the instance
/// that's missing when no impl of `ToImmDeep` is for `ty`, or one is but
/// its bounds don't hold, like `ToImmDeep[Point]` for a `Vec[Point]`.
pub fn check_imm_deep<'db>(db: &'db dyn Db, root: File, ty: &Ty<'db>) -> Result<(), String> {
    to_imm_deep(db, root, ty).map(|_| ())
}

/// The `to_imm_deep` of the impl of `ToImmDeep` for `ty`, or an error with
/// the instance that's missing. A mutability of the code that copies is
/// unknown here, so the impl must be the same whether it's `imm` or `mut`,
/// and hold for both.
fn to_imm_deep<'db>(db: &'db dyn Db, root: File, ty: &Ty<'db>) -> Result<ItemRef<'db>, String> {
    let at =
        |m: Mutability| ty.fold(&mut |t| matches!(t, Ty::Bound(_)).then_some(Ty::Mutability(m)));
    let missing = || format!("ToImmDeep[{}]", at(Mutability::Mut).display(db));
    let class = LangClass::ToImmDeep.class(db, root).ok_or_else(missing)?;
    let method = LangClass::ToImmDeep.method(db, class).ok_or_else(missing)?;
    let pick = |m: Mutability| select_impl(db, &[root], class, &[at(m)]);
    let imp = pick(Mutability::Imm).ok_or_else(missing)?;
    if pick(Mutability::Mut) != Some(imp) {
        return Err(missing());
    }
    for m in [Mutability::Imm, Mutability::Mut] {
        let constraint = ClassConstraint { class, args: Box::new([at(m)]) };
        if let Some(missing) = missing_instances(db, root, constraint).first() {
            let args: Vec<String> =
                missing.args.iter().map(|t| t.display(db).to_string()).collect();
            let name = missing.class.item(db).name.as_ref().map_or("?", |n| n.as_str());
            return Err(format!("{name}[{}]", args.join(", ")));
        }
    }
    Ok(ItemRef::Method {
        item: ClassItem::Function(method),
        class_args: Some(Box::new([ty.clone()])),
        imp: Some(imp),
    })
}

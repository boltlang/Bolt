//! Shims: bodies that the compiler makes for what has none in the source,
//! as rustc's are (`rustc_mir_transform::shim`).

use boltlang_base_db::{Db, File};
use boltlang_hir_def::{
    Name,
    builtin_type::{BuiltinInt, BuiltinType},
    hir::Literal,
    ids::EnumVariantId,
    resolver::GenericDefId,
};
use boltlang_hir_ty::{
    AdtId, GenericParamId, Ty,
    builtin::{Builtin, PrimOp},
    instances::select_impl,
    lang::LangClass,
    lower::{ClassConstraint, VariantDef, field_types, named_mutabilities},
    solve::{imm_of, missing_instances},
    ty::Mutability,
};
use boltlang_mir::{
    Arm, Body, BodyOwner, ClassItem, ExprId, ExprKind, ItemRef, Local, Place, Shim, Stmt,
};
use boltlang_pattern::decision::{Ctor, Decision};

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
        Ctor::Tuple(_) => return None,
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
            for ty in fields.iter() {
                let local = body.new_temp(None, ty.clone());
                body.params.push(local);
                values.push(body.alloc(ExprKind::Local(local), ty.clone(), None));
            }
            let kind = ExprKind::Construct { ctor: *ctor, fields: values.into() };
            body.root = body.alloc(kind, *made, None);
        }
        Shim::ImmDeep(ty) => {
            let x = body.new_temp(Some(Name::new("x")), ty.clone());
            body.params.push(x);
            body.root = imm_glue(db, root, &mut body, x, ty);
        }
    }
    body
}

/// The deep copy of `imm e` for a value of type `ty`, the local `x`: the
/// type's impl of `ToImmDeep`, if it has one, and otherwise a copy of each
/// of its parts, with `imm` of each, so that its own glue copies it. What's
/// already `imm` all the way down is given back as it is.
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
        Ok(Some(item)) => return body.call(item, vec![value], to),
        Ok(None) => {}
        // Monomorphization reports it, before it asks for the glue.
        Err(_) => {
            body.replace(value, ExprKind::Unsupported("a copy without `ToImmDeep`"));
            return value;
        }
    }
    // `imm` of a part, read from `x` with `read`.
    let copy = |body: &mut Body<'db>, ty: Ty<'db>, read: ExprKind<'db>| {
        let part = body.alloc(read, ty.clone(), None);
        body.alloc(ExprKind::Imm { expr: part }, imm_of(db, root, &ty), None)
    };
    match ty {
        Ty::Tuple(tys) => {
            let ctor = Ctor::Tuple(tys.len() as u32);
            let fields = tys
                .iter()
                .enumerate()
                .map(|(index, t)| {
                    let base = body.local_expr(x);
                    copy(body, t.clone(), ExprKind::Field { base, ctor, index: index as u32 })
                })
                .collect();
            body.alloc(ExprKind::Construct { ctor, fields }, to, None)
        }
        Ty::Adt(AdtId::Struct(s), args, mutabilities, own) => {
            let def = VariantDef::Struct(*s);
            let mut slots: Vec<Ty<'db>> = args.to_vec();
            slots.extend(mutabilities.iter().cloned());
            slots.push((**own).clone());
            let ctor = Ctor::Struct(*s);
            let fields = fields_of(db, def, &slots)
                .into_iter()
                .enumerate()
                .map(|(index, t)| {
                    let base = body.local_expr(x);
                    copy(body, t, ExprKind::Field { base, ctor, index: index as u32 })
                })
                .collect();
            body.alloc(ExprKind::Construct { ctor, fields }, to, None)
        }
        // A `match` on the variant, which makes it again with copies of
        // its fields.
        Ty::Adt(AdtId::Enum(e), args, ..) => {
            let decision = Box::new(Decision::variants(db, *e));
            let mut arms = Vec::new();
            for index in 0..decision.arms {
                let v = EnumVariantId { parent: *e, index };
                let ctor = Ctor::Variant(v);
                let fields = fields_of(db, VariantDef::Variant(v), args)
                    .into_iter()
                    .enumerate()
                    .map(|(index, t)| {
                        let base = body.local_expr(x);
                        copy(body, t, ExprKind::Field { base, ctor, index: index as u32 })
                    })
                    .collect();
                let made = body.alloc(ExprKind::Construct { ctor, fields }, to.clone(), None);
                arms.push(Arm { guard: None, body: made });
            }
            let scrutinee = body.local_expr(x);
            body.alloc(ExprKind::Match { scrutinee, decision, arms: arms.into() }, to, None)
        }
        Ty::Array(_, elem) => array_glue(db, root, body, x, elem, to),
        // A closure can't be copied, and must already be `imm`.
        _ => value,
    }
}

/// The deep copy of an array: a new one, with `imm` of each element.
///
/// ```text
/// let n = array_len(x);
/// let ys = array_alloc(n);
/// let mut i = 0;
/// while i < n { array_set(ys, i, imm array_get(x, i)); i = i + 1; }
/// array_freeze(ys)
/// ```
fn array_glue<'db>(
    db: &'db dyn Db,
    root: File,
    body: &mut Body<'db>,
    x: Local,
    elem: &Ty<'db>,
    to: Ty<'db>,
) -> ExprId {
    let int = BuiltinType::Int(BuiltinInt::I32);
    let i32_ty = Ty::Builtin(int);
    let elem_to = imm_of(db, root, elem);
    let ys_ty = Ty::array(Ty::Mutability(Mutability::Mut), elem_to.clone());
    let xs = body.local_expr(x);
    let len = body.call(ItemRef::Builtin(Builtin::ArrayLen), vec![xs], i32_ty.clone());
    let (n, let_n) = body.let_temp("n", len, false);
    let n_value = body.local_expr(n);
    let alloc = body.call(ItemRef::Builtin(Builtin::ArrayAlloc), vec![n_value], ys_ty);
    let (ys, let_ys) = body.let_temp("ys", alloc, false);
    let zero = body.alloc(ExprKind::Literal(Literal::Int(0, None)), i32_ty.clone(), None);
    let (i, let_i) = body.let_temp("i", zero, true);
    let (i_value, n_value) = (body.local_expr(i), body.local_expr(n));
    let lt = ItemRef::Builtin(Builtin::Prim(PrimOp::Lt, int));
    let cond = body.call(lt, vec![i_value, n_value], Ty::bool());
    let (xs, i_value) = (body.local_expr(x), body.local_expr(i));
    let get = body.call(ItemRef::Builtin(Builtin::ArrayGet), vec![xs, i_value], elem.clone());
    let copied = body.alloc(ExprKind::Imm { expr: get }, elem_to, None);
    let (ys_value, i_value) = (body.local_expr(ys), body.local_expr(i));
    let set = ItemRef::Builtin(Builtin::ArraySet);
    let set = body.call(set, vec![ys_value, i_value, copied], Ty::unit());
    let (i_value, one) = (
        body.local_expr(i),
        body.alloc(ExprKind::Literal(Literal::Int(1, None)), i32_ty.clone(), None),
    );
    let add = ItemRef::Builtin(Builtin::Prim(PrimOp::Add, int));
    let next = body.call(add, vec![i_value, one], i32_ty);
    let step = ExprKind::Assign { place: Place::Local(i), op: None, value: next };
    let step = body.alloc(step, Ty::unit(), None);
    let inner = body.block(vec![Stmt::Expr(set), Stmt::Expr(step)], None);
    let label = body.new_label(None);
    let lp = body.alloc(ExprKind::While { cond, body: inner, label }, Ty::unit(), None);
    let ys_value = body.local_expr(ys);
    let frozen = body.call(ItemRef::Builtin(Builtin::ArrayFreeze), vec![ys_value], to);
    body.block(vec![let_n, let_ys, let_i, Stmt::Expr(lp)], Some(frozen))
}

/// The types of the fields of `def`, with `slots` for its parameters: a
/// struct's type parameters, named mutabilities and own mutability, or an
/// enum's type parameters.
fn fields_of<'db>(db: &'db dyn Db, def: VariantDef<'db>, slots: &[Ty<'db>]) -> Vec<Ty<'db>> {
    let (_, generic) = def.adt();
    field_types(db, def)
        .fields
        .iter()
        .map(|(_, t)| {
            t.fold(&mut |t| match t {
                Ty::Param(p) if p.def == generic => slots.get(p.index as usize).cloned(),
                _ => None,
            })
        })
        .collect()
}

/// Whether the deep copy of `ty` can be made: an error with the instance
/// that's missing when an impl of `ToImmDeep` is for `ty` but its bounds
/// don't hold, like `ToImmDeep[Point]` for a `Vec[Point]`. A type that no
/// impl is for is copied part by part instead.
pub fn check_imm_deep<'db>(db: &'db dyn Db, root: File, ty: &Ty<'db>) -> Result<(), String> {
    to_imm_deep(db, root, ty).map(|_| ())
}

/// The `to_imm_deep` of the impl of `ToImmDeep` for `ty`, if there is one,
/// and an error with the instance that's missing if its bounds don't hold.
/// A mutability of the code that copies is unknown here, so the impl must
/// be the same whether it's `imm` or `mut`, and hold for both.
fn to_imm_deep<'db>(
    db: &'db dyn Db,
    root: File,
    ty: &Ty<'db>,
) -> Result<Option<ItemRef<'db>>, String> {
    let Some(class) = LangClass::ToImmDeep.class(db, root) else { return Ok(None) };
    let Some(method) = LangClass::ToImmDeep.method(db, class) else { return Ok(None) };
    let at =
        |m: Mutability| ty.fold(&mut |t| matches!(t, Ty::Bound(_)).then_some(Ty::Mutability(m)));
    let pick = |m: Mutability| select_impl(db, &[root], class, &[at(m)]);
    let Some(imp) = pick(Mutability::Imm) else { return Ok(None) };
    if pick(Mutability::Mut) != Some(imp) {
        return Ok(None);
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
    Ok(Some(ItemRef::Method {
        item: ClassItem::Function(method),
        class_args: Some(Box::new([ty.clone()])),
        imp: Some(imp),
    }))
}

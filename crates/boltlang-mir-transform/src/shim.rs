//! Shims: bodies that the compiler makes for what has none in the source,
//! as rustc's are (`rustc_mir_transform::shim`).

use boltlang_base_db::Db;
use boltlang_hir_def::resolver::GenericDefId;
use boltlang_hir_ty::{
    GenericParamId, Ty,
    lower::{VariantDef, field_types, named_mutabilities},
    ty::Mutability,
};
use boltlang_mir::{Body, BodyOwner, ExprKind, Shim};
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

/// The MIR of a shim.
pub fn shim_body<'db>(db: &'db dyn Db, shim: &Shim<'db>) -> Body<'db> {
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
        Shim::ImmDeep(_) => {
            body.replace(body.root, ExprKind::Unsupported("the deep copy of `imm e`"));
        }
    }
    body
}

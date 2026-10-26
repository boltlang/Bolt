//! Type schemes: the types of functions, which can be generic, with the
//! class and field constraints that must hold where they're used.

use std::collections::HashMap;

use boltlang_base_db::Db;
use boltlang_hir_def::{
    ids::{FunctionId, ItemContainerId},
    item_tree::AssocItem,
    resolver::GenericDefId,
};

use crate::{
    lower::{
        ClassConstraint, fn_sig, generic_predicates, identity_args, imm, impl_header, named_mutabilities, subst_params,
    },
    solve::{Constraint, FieldConstraint, Origin, Solver},
    ty::{GenericParamId, Ty, TyNames, VarKind, is_hidden_param},
};

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct Scheme<'db> {
    /// The declared type parameters the scheme is generic over.
    pub params: Box<[GenericParamId<'db>]>,
    /// The number of inferred type variables it's generic over, which are
    /// [`Ty::Bound`] in `ty`.
    pub bound: u32,
    pub predicates: Box<[ClassConstraint<'db>]>,
    /// The fields the types must have, for a function that uses fields of
    /// values whose types it doesn't decide.
    pub fields: Box<[FieldConstraint<'db>]>,
    pub ty: Ty<'db>,
}

impl<'db> Scheme<'db> {
    pub fn mono(ty: Ty<'db>) -> Self {
        Scheme { params: Box::new([]), bound: 0, predicates: Box::new([]), fields: Box::new([]), ty }
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
        for field in self.fields.iter() {
            solver.add(Constraint::Field { constraint: field.map(&mut subst), origin });
        }
        subst(&self.ty)
    }

    /// Show the scheme like `forall a. Show[a] => fn(a) -> a`, with fields
    /// like `a.x: i32 => `, or `mut a.x: i32 => ` for one that's assigned. Mutabilities aren't shown: an array whose
    /// mutability is generic is `[a]`.
    pub fn display(&self, db: &'db dyn Db) -> String {
        let mut tys = vec![&self.ty];
        for pred in self.predicates.iter() {
            tys.extend(pred.args.iter());
        }
        for field in self.fields.iter() {
            tys.extend([&field.ty, &field.field]);
        }
        let names = TyNames::new(db, &tys);
        let show = |t: &Ty<'db>| t.display_with(db, &names).to_string();
        let mut forall: Vec<String> =
            self.params.iter().filter(|p| !is_hidden_param(db, **p)).map(|p| show(&Ty::Param(*p))).collect();
        forall.extend(names.bound_names().map(str::to_string));
        let mut out = String::new();
        if !forall.is_empty() {
            out.push_str(&format!("forall {}. ", forall.join(" ")));
        }
        for pred in self.predicates.iter() {
            let args: Vec<_> = pred.args.iter().map(show).collect();
            let name = pred.class.item(db).name.as_ref().map_or("?", |n| n.as_str());
            out.push_str(&format!("{name}[{}] => ", args.join(", ")));
        }
        for field in self.fields.iter() {
            let prefix = if field.assign { "mut " } else { "" };
            out.push_str(&format!("{prefix}{}.{}: {} => ", show(&field.ty), field.name, show(&field.field)));
        }
        out.push_str(&show(&self.ty));
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

/// The type parameters of `defs`, in order, each followed by its named
/// mutabilities (see [`named_mutabilities`]) and its hidden ones (see
/// [`FnSig::hidden`](crate::lower::FnSig::hidden)).
pub fn generic_param_ids<'db>(
    db: &'db dyn Db,
    defs: impl IntoIterator<Item = GenericDefId<'db>>,
) -> Box<[GenericParamId<'db>]> {
    defs.into_iter()
        .flat_map(|def| {
            let hidden = match def {
                GenericDefId::Function(f) => fn_sig(db, f).hidden,
                GenericDefId::Impl(i) => impl_header(db, i).hidden,
                _ => 0,
            };
            let named = named_mutabilities(db, def).len() as u32;
            let count = def.generic_params(db).type_params.len() as u32 + named + hidden;
            (0..count).map(move |index| GenericParamId { def, index })
        })
        .collect()
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
    let method = class_method_of(db, f)?;
    let sig = fn_sig(db, method);
    let class_def = GenericDefId::Class(target.class);
    // The method's own type parameters, like the `A` and `B` of
    // `fn fmap[A, B]`, are those of the impl's method, in order, when it
    // has as many. Otherwise they stay the class's, which the impl's
    // method can't know anything about either.
    let method_def = GenericDefId::Function(method);
    let own_def = GenericDefId::Function(f);
    let own_params = same_generic_count(db, method, f).then(|| identity_args(db, own_def));
    let subst = |t: &Ty<'db>| {
        let t = subst_params(t, class_def, &target.args);
        match &own_params {
            Some(params) => subst_params(&t, method_def, params),
            None => t,
        }
    };
    let params = sig.params.iter().map(|t| t.as_ref().map_or(Ty::Error, subst)).collect();
    let ret = sig.ret.as_ref().map_or(Ty::Error, |t| subst(&t.fill_unwritten(&mut imm)));
    Some((params, ret))
}

/// The declaration in the class of a method of a class impl, like the
/// `show` of `class Show[T]` for the `show` of `impl Show[i32]`.
fn class_method_of<'db>(db: &'db dyn Db, f: FunctionId<'db>) -> Option<FunctionId<'db>> {
    let ItemContainerId::Impl(imp) = f.container(db) else { return None };
    let class = impl_header(db, imp).class.as_ref()?.class;
    let name = f.item(db).name.as_ref()?;
    class.item(db).items.iter().find_map(|item| match *item {
        AssocItem::Function(idx) => {
            let m = FunctionId::new(db, ItemContainerId::Class(class), idx);
            (m.item(db).name.as_ref() == Some(name)).then_some(m)
        }
        _ => None,
    })
}

fn same_generic_count<'db>(db: &'db dyn Db, a: FunctionId<'db>, b: FunctionId<'db>) -> bool {
    let count = |f| GenericDefId::Function(f).generic_params(db).type_params.len();
    count(a) == count(b)
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
            (params, sig.ret.as_ref().map_or(class_ret, |t| t.fill_unwritten(&mut imm)))
        }
        // Without a body to decide them, mutabilities of the return type
        // that aren't written are `imm`.
        None => {
            let params = sig.params.iter().map(|t| t.clone().unwrap_or(Ty::Error)).collect();
            (params, sig.ret.as_ref().map_or(Ty::Error, |t| t.fill_unwritten(&mut imm)))
        }
    };
    let mut defs = generic_chain(db, f);
    // An impl method without type parameters of its own, like the `fmap` of
    // `impl Functor[Option] { fn fmap(f, x) = ... }`, has those of the
    // class's method.
    if let Some(method) = class_method_of(db, f)
        && !same_generic_count(db, method, f)
        && GenericDefId::Function(f).generic_params(db).type_params.is_empty()
    {
        defs.push(GenericDefId::Function(method));
    }
    Scheme {
        params: generic_param_ids(db, defs),
        bound: 0,
        predicates: assumptions(db, f).into(),
        fields: Box::new([]),
        ty: Ty::Fn(params, Box::new(ret)),
    }
}

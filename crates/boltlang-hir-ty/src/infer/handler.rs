//! Checking the effect arms of a `match`, which make it a handler: see
//! `site/docs/internals/effects.mdx`.
//!
//! The scrutinee runs with effects of its own, `s`. Each arm names an
//! operation, whose signature gives the types of the arm's patterns, with
//! the arguments of its effect shared by all the arms of the `match`. The
//! continuation of an arm for `fn op(a: A) -> B` is a `Cont[B, U, ρ]`, where
//! `U` is the type of the `match` and `ρ` the effects of the code around the
//! `match`, which the arms run with. Handlers are deep: resuming runs the
//! rest of the scrutinee, still handled, so the arms can run again, and `ρ`
//! has their effects as well as those of `s` without the effects that have
//! arms, which must be in it too. That's also the row Conduit gives a
//! continuation. Only what the arms and `s` do would be more precise
//! (#370).

use boltlang_hir_def::{
    Name,
    hir::{EffectArm, ExprId, Pat, PatId, Path},
    ids::{EffectId, FunctionId, ItemContainerId},
    resolver::{GenericDefId, ValueNs, ValueResolution, effect_operations},
};

use super::{InferenceContext, InferenceDiagnostic};
use crate::{
    kind::{Kind, def_kind},
    lang::cont_struct,
    lower::{fn_sig, subst_params},
    rows::RowConstraint,
    solve::{Constraint, ExprOrPat, Origin},
    ty::{AdtId, EffectTy, Mutability, Ty, VarKind},
};

impl<'db> InferenceContext<'_, 'db> {
    /// Check the effect arms of the `match` `expr`, whose scrutinee did
    /// `inner`, and whose value is of type `ty`. The arms run outside the
    /// handler, with the effects of the code around it.
    pub(super) fn check_effect_arms(
        &mut self,
        expr: ExprId,
        arms: &[EffectArm],
        inner: Ty<'db>,
        ty: &Ty<'db>,
    ) {
        let db = self.db;
        let left = self.solver.new_var(VarKind::Effect);
        let file = self.file();
        let cont = cont_struct(db, file);
        if cont.is_none() {
            self.result
                .diagnostics
                .push(InferenceDiagnostic::NoLangType { at: ExprOrPat::Expr(expr), ty: "Cont" });
        }
        // Each effect with arms, and the first operation of it in the arms.
        let mut handled: Vec<(EffectTy<'db>, PatId)> = Vec::new();
        // The operations with arms.
        let mut covered: Vec<FunctionId<'db>> = Vec::new();
        for arm in arms {
            for &op in arm.ops.iter() {
                let body = self.body;
                let Pat::TupleStruct { path, args, rest } = &body[op] else {
                    self.check_pat(arm.cont, &Ty::Error);
                    continue;
                };
                let Some((f, effect)) = self.operation(expr, op, path) else {
                    for &arg in args.iter() {
                        self.check_pat(arg, &Ty::Error);
                    }
                    self.check_pat(arm.cont, &Ty::Error);
                    continue;
                };
                self.result.operations.insert(op, f);
                covered.push(f);
                // The arms of one effect share its arguments, like the `S`
                // of `State[S]` for `get` and `set`.
                let effect_args = match handled.iter().find(|(h, _)| h.effect == effect) {
                    Some((h, _)) => h.args.to_vec(),
                    None => {
                        let args = self.effect_args(effect);
                        handled.push((EffectTy { effect, args: args.clone().into() }, op));
                        args
                    }
                };
                // The operation's own type parameters, like the `T` of
                // `fn raise[T](e: E) -> T`, stay what they are: each arm
                // has to work for any `T`.
                let sig = fn_sig(db, f);
                let subst = |t: &Ty<'db>| {
                    subst_params(t, GenericDefId::Effect(effect), &effect_args)
                        .fill_unwritten(&mut || Ty::Mutability(Mutability::Imm))
                };
                let params: Vec<Ty<'db>> =
                    sig.params.iter().map(|p| p.as_ref().map_or(Ty::Error, subst)).collect();
                let ret = sig.ret.as_ref().map_or(Ty::unit(), subst);
                self.result.pat_types.insert(op, Ty::func(params.clone(), ret.clone()));
                self.check_pat_count(op, *rest, params.len(), args.len());
                self.check_pat_list(args, *rest, &params);
                // With several operations, the continuation is checked for
                // each, as the alternatives of an or-pattern: a named one
                // needs them all to give the same type.
                let cont_ty = match cont {
                    Some(c) => Ty::adt(
                        AdtId::Struct(c),
                        [ret, ty.clone(), self.effects.clone()],
                        [],
                        Ty::Mutability(Mutability::Imm),
                    ),
                    None => Ty::Error,
                };
                self.check_pat(arm.cont, &cont_ty);
            }
            if let Some(guard) = arm.guard {
                self.check_expr(guard, &Ty::bool());
            }
            self.check_expr(arm.expr, ty);
        }
        // A match that handles an effect handles all of it, so it needs
        // arms for all of its operations.
        for (effect, op) in &handled {
            let missing: Vec<Name> = effect_operations(db, effect.effect)
                .into_iter()
                .filter(|f| !covered.contains(f))
                .filter_map(|f| f.item(db).name.as_ref().map(|n| n.name().clone()))
                .collect();
            if !missing.is_empty() {
                let name = effect.effect.item(db).name.clone();
                self.result.diagnostics.push(InferenceDiagnostic::MissingOperations {
                    pat: *op,
                    effect: name,
                    missing,
                });
            }
        }
        // One effect is taken out at a time, so that an effect whose
        // arguments in the scrutinee aren't those the arms use is reported
        // at its first arm.
        let mut row = inner;
        for (i, (effect, op)) in handled.iter().enumerate() {
            let rest = if i + 1 == handled.len() {
                left.clone()
            } else {
                self.solver.new_var(VarKind::Effect)
            };
            let constraint = RowConstraint::Without {
                row,
                handled: Box::new([effect.clone()]),
                rest: rest.clone(),
            };
            let origin = Origin::new(self.def, ExprOrPat::Pat(*op));
            self.solver.add(Constraint::Row { constraint, origin });
            row = rest;
        }
        // Without an operation, nothing is handled.
        if handled.is_empty() {
            self.solver.equal(&row, &left, self.origin(expr));
        }
        let contains = RowConstraint::contains(left, self.effects.clone());
        self.solver.add(Constraint::Row { constraint: contains, origin: self.origin(expr) });
    }

    /// The operation that `path` names in the effect arm `op` of the
    /// `match` `expr`, with its effect.
    fn operation(
        &mut self,
        expr: ExprId,
        op: PatId,
        path: &Path,
    ) -> Option<(FunctionId<'db>, EffectId<'db>)> {
        let db = self.db;
        let of_effect = |f: FunctionId<'db>| match f.container(db) {
            ItemContainerId::Effect(e) => Some((f, e)),
            _ => None,
        };
        let resolver = self.resolver(expr);
        let found: Vec<(FunctionId<'db>, EffectId<'db>)> =
            match resolver.resolve_path_in_value_ns(db, path) {
                Some(ValueResolution::Value(ValueNs::Function(f))) => {
                    of_effect(f).into_iter().collect()
                }
                Some(ValueResolution::Value(ValueNs::Overloaded(set))) => {
                    set.functions(db).iter().filter_map(|&f| of_effect(f)).collect()
                }
                Some(_) => Vec::new(),
                None => {
                    self.unresolved_path(ExprOrPat::Pat(op), &resolver, path);
                    return None;
                }
            };
        match found[..] {
            [one] => Some(one),
            [] => {
                self.result.diagnostics.push(InferenceDiagnostic::NotAnOperation { pat: op });
                None
            }
            _ => {
                self.result.diagnostics.push(InferenceDiagnostic::AmbiguousOperation { pat: op });
                None
            }
        }
    }

    /// New variables for the arguments of `effect`, of the kinds of its
    /// parameters.
    fn effect_args(&mut self, effect: EffectId<'db>) -> Vec<Ty<'db>> {
        let kind = def_kind(self.db, GenericDefId::Effect(effect));
        let (params, _) = kind.split();
        params
            .into_iter()
            .map(|k| match k {
                Kind::Effect => self.solver.new_var(VarKind::Effect),
                _ => self.new_var(),
            })
            .collect()
    }
}

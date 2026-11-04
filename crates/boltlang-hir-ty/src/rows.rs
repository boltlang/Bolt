//! Rows of effects, after Morris and McKinna's Rose ("Abstracting
//! Extensible Data Types", POPL 2019): a row is a [`Ty::Row`] whose effects
//! are all known, or a variable or parameter, and how rows relate is a
//! predicate that the solver defers until the rows are known well enough.
//!
//! The row theory is that of consistent union: combining two rows gives all
//! their effects, and an effect in both must have the same arguments there.
//! So `Log ⊙ Log ~ Log`, but `State[i32]` and `State[String]` can't be in
//! one row. That's what "no effect twice" means (see
//! `site/docs/internals/effects.mdx`).
//!
//! The one predicate so far is containment, `sub ≲ sup`: every effect of
//! `sub` is in `sup`, with the same arguments. A call adds the effects of
//! what it calls to those of the code it's in, as `callee ≲ body`.

use crate::{
    solve::{Constraint, Origin, Solver, TypeError},
    ty::{EffectTy, Ty, TyVar, VarKind},
};

/// That every effect of `sub` is in `sup`, with the same arguments: `sub ≲
/// sup`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub struct RowConstraint<'db> {
    pub sub: Ty<'db>,
    pub sup: Ty<'db>,
}

impl<'db> RowConstraint<'db> {
    pub fn map(&self, mut f: impl FnMut(&Ty<'db>) -> Ty<'db>) -> Self {
        RowConstraint { sub: f(&self.sub), sup: f(&self.sup) }
    }
}

impl<'db> Solver<'db> {
    /// Solve `sub ≲ sup` if the rows are known well enough. Returns `false`
    /// if it has to wait.
    pub(crate) fn solve_row(&mut self, c: &RowConstraint<'db>, origin: Origin<'db>) -> bool {
        let sub = self.unifier.resolve(&c.sub);
        let sup = self.unifier.resolve(&c.sup);
        match (&sub, &sup) {
            // An error was already reported.
            (Ty::Error, _) | (_, Ty::Error) => true,
            // No effects fit anywhere, and a row fits in itself.
            (Ty::Row(effects), _) if effects.is_empty() => true,
            (a, b) if a == b => true,
            (Ty::Row(effects), Ty::Row(allowed)) => {
                let mut missing = Vec::new();
                for e in effects.iter() {
                    match allowed.iter().find(|a| a.effect == e.effect) {
                        Some(a) => {
                            for (x, y) in e.args.iter().zip(a.args.iter()) {
                                self.add(Constraint::Equal {
                                    expected: y.clone(),
                                    actual: x.clone(),
                                    origin,
                                });
                            }
                        }
                        None => missing.push(e.clone()),
                    }
                }
                if !missing.is_empty() {
                    self.errors.push(TypeError::Effects {
                        origin,
                        found: Ty::row(missing),
                        allowed: sup.clone(),
                    });
                }
                true
            }
            // Either side still unknown: wait until it's known, or until the
            // row is settled.
            (Ty::Var(_), _) | (_, Ty::Var(_)) => false,
            // A row parameter, or one of an inferred function, stands for
            // any row, so nothing but itself is known to fit in it, and it
            // isn't known to fit anywhere else.
            _ => {
                let found = match &sub {
                    Ty::Row(effects) => {
                        Ty::row(effects.iter().filter(|e| !row_has(&sup, e)).cloned())
                    }
                    _ => sub.clone(),
                };
                self.errors.push(TypeError::Effects { origin, found, allowed: sup.clone() });
                true
            }
        }
    }

    /// Give each row variable in `candidates` that `keep` doesn't have its
    /// least solution: the union of the rows that must fit in it, or no
    /// effects if there are none. A variable whose rows are still partly
    /// unknown waits for them, unless it's one of `keep`, which stays a
    /// variable, with its constraints. When a variable must hold exactly
    /// one other, unknown one, it becomes that one.
    pub(crate) fn settle_rows(&mut self, candidates: &[TyVar], keep: &[TyVar]) {
        loop {
            self.solve();
            let mut progress = false;
            let unbound: Vec<TyVar> = candidates
                .iter()
                .copied()
                .filter(|v| !keep.contains(v))
                .filter(
                    |v| matches!(self.unifier.shallow_resolve(&Ty::Var(*v)), Ty::Var(w) if w == *v),
                )
                .collect();
            for var in unbound {
                // Whatever settling the others did may have decided it.
                if !matches!(self.unifier.shallow_resolve(&Ty::Var(var)), Ty::Var(w) if w == var) {
                    continue;
                }
                // What must fit in it, each once: a function called twice
                // adds its effects twice.
                let mut below: Vec<(Ty<'db>, Origin<'db>)> = Vec::new();
                for (c, origin) in &self.pending_rows {
                    if self.unifier.resolve(&c.sup) != Ty::Var(var) {
                        continue;
                    }
                    let sub = self.unifier.resolve(&c.sub);
                    if !below.iter().any(|(t, _)| *t == sub) {
                        below.push((sub, *origin));
                    }
                }
                let known = below.iter().all(|(t, _)| matches!(t, Ty::Row(_) | Ty::Error));
                let solution = if known {
                    // The union of what must fit, with the arguments of an
                    // effect in several of them made the same.
                    let mut union = Ty::nothing();
                    for (row, origin) in &below {
                        let (Ty::Row(a), Ty::Row(b)) = (&union, row) else { continue };
                        let mut same = Vec::new();
                        union = Ty::union_rows(a, b, |x, y| same.push((x.clone(), y.clone())));
                        for (x, y) in same {
                            self.add_same_args(&x, &y, *origin);
                        }
                    }
                    union
                } else {
                    match &below[..] {
                        [(other @ (Ty::Var(_) | Ty::Param(_) | Ty::Bound(_)), _)] => other.clone(),
                        // Partly known, or several unknown ones: it waits.
                        _ => continue,
                    }
                };
                let origin = below.first().map(|(_, o)| *o);
                match origin {
                    Some(origin) => self.equal(&Ty::Var(var), &solution, origin),
                    None => {
                        let _ = self.unifier.unify(&Ty::Var(var), &solution);
                    }
                }
                progress = true;
            }
            if !progress {
                return;
            }
        }
    }

    fn add_same_args(&mut self, x: &EffectTy<'db>, y: &EffectTy<'db>, origin: Origin<'db>) {
        for (a, b) in x.args.iter().zip(y.args.iter()) {
            self.add(Constraint::Equal { expected: a.clone(), actual: b.clone(), origin });
        }
    }

    /// Give the rows that nothing decided, and that nothing must fit in, no
    /// effects, unless they're in `keep`. A row that's still unknown, like
    /// that of a closure that calls nothing, would keep class constraints
    /// about its function type from being solved.
    pub(crate) fn default_free_rows(&mut self, keep: &[TyVar]) {
        let bounded: Vec<TyVar> = self
            .pending_rows
            .iter()
            .filter_map(|(c, _)| match self.unifier.resolve(&c.sup) {
                Ty::Var(v) => Some(v),
                _ => None,
            })
            .collect();
        for var in self.unifier.unbound_rows() {
            if !keep.contains(&var) && !bounded.contains(&var) {
                let _ = self.unifier.unify(&Ty::Var(var), &Ty::nothing());
            }
        }
        self.solve();
    }

    /// The containment constraints left, resolved.
    pub fn take_pending_rows(&mut self) -> Vec<(RowConstraint<'db>, Origin<'db>)> {
        std::mem::take(&mut self.pending_rows)
            .into_iter()
            .map(|(c, origin)| (c.map(|t| self.unifier.resolve(t)), origin))
            .collect()
    }

    /// Report the containment constraints that are left once nothing more
    /// can be known, like that of a call in top-level code of a function
    /// whose effects are a variable nothing decided. Its variables have no
    /// effects then.
    pub(crate) fn finish_rows(&mut self) {
        let vars: Vec<TyVar> = self
            .pending_rows
            .iter()
            .flat_map(|(c, _)| {
                let mut vars = Vec::new();
                crate::solve::vars_in(&self.unifier.resolve(&c.sub), &mut vars);
                crate::solve::vars_in(&self.unifier.resolve(&c.sup), &mut vars);
                vars
            })
            .filter(|v| self.unifier.var_kind(*v) == Some(VarKind::Effect))
            .collect();
        self.settle_rows(&vars, &[]);
        for (c, origin) in std::mem::take(&mut self.pending_rows) {
            // Settling made the unknown rows known, or left them for the
            // defaults, which give no effects.
            let unifier = &self.unifier;
            let c = c.map(|t| {
                unifier.resolve(t).fold(&mut |t| match t {
                    Ty::Var(v) if unifier.var_kind(*v) == Some(VarKind::Effect) => {
                        Some(Ty::nothing())
                    }
                    _ => None,
                })
            });
            self.solve_row(&c, origin);
        }
    }
}

/// Whether the row `row` has the effect `e`, with any arguments.
fn row_has(row: &Ty<'_>, e: &EffectTy<'_>) -> bool {
    matches!(row, Ty::Row(effects) if effects.iter().any(|x| x.effect == e.effect))
}

/// The row variables of `ty` that are only in positive positions: the
/// effects of a function type, but not those of a parameter's, nor a row
/// inside a struct or an array. Such a row can be given its least solution
/// without making the type less general, since a smaller row fits wherever
/// a bigger one does through `≲`.
pub(crate) fn positive_rows(ty: &Ty<'_>, is_row: impl Fn(TyVar) -> bool) -> Vec<TyVar> {
    // The polarity of each place a variable is found: 1 for positive, -1
    // for negative, and 0 for both.
    fn walk(ty: &Ty<'_>, polarity: i8, out: &mut Vec<(TyVar, i8)>) {
        match ty {
            Ty::Var(v) => out.push((*v, polarity)),
            Ty::Fn(params, ret, _, effects) => {
                for p in params.iter() {
                    walk(p, -polarity, out);
                }
                walk(ret, polarity, out);
                walk(effects, polarity, out);
            }
            Ty::Tuple(tys) => tys.iter().for_each(|t| walk(t, polarity, out)),
            _ => ty.walk_children(|t| walk(t, 0, out)),
        }
    }
    let mut found = Vec::new();
    walk(ty, 1, &mut found);
    let mut out: Vec<TyVar> = Vec::new();
    for &(v, _) in &found {
        if is_row(v) && !out.contains(&v) && found.iter().all(|(w, p)| *w != v || *p == 1) {
            out.push(v);
        }
    }
    out
}

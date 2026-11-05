//! Rows of effects, after Morris and McKinna's Rose ("Abstracting
//! Extensible Data Types", POPL 2019). A row in a type is a [`Ty::Row`]: the
//! effects that are known, and rows that stand for any row, like the `E` of
//! `Log + E`, which are parameters or the variables of a scheme. A row made
//! of rows still being inferred is a variable instead, and how rows relate
//! is a predicate that the solver defers until the rows are known well
//! enough.
//!
//! The row theory is that of consistent union: combining two rows gives all
//! their effects, and an effect in both must have the same arguments there.
//! So `Log ⊙ Log ~ Log`, but `State[i32]` and `State[String]` can't be in
//! one row. That's what "no effect twice" means (see
//! `site/docs/internals/effects.mdx`).
//!
//! There are two predicates:
//!
//! - Containment, `sub ≲ sup`: every effect of `sub` is in `sup`, with the
//!   same arguments. A call adds the effects of what it calls to those of
//!   the code it's in, as `callee ≲ body`.
//! - Combination, `ρ₁ ⊙ … ⊙ ρₙ ~ goal`: `goal` has the effects of all the
//!   rows. It's what a row like `Log + E` is when `E` stands for a row
//!   that's still being inferred.

use crate::{
    solve::{Constraint, Origin, Solver, TypeError, vars_in},
    ty::{EffectTy, Ty, TyVar, VarKind},
};

/// A predicate on rows.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum RowConstraint<'db> {
    /// Every effect of `sub` is in `sup`, with the same arguments: `sub ≲
    /// sup`.
    Contains { sub: Ty<'db>, sup: Ty<'db> },
    /// `goal` has the effects of all of `parts`, and no others.
    Combine { parts: Box<[Ty<'db>]>, goal: Ty<'db> },
    /// `rest` is `row` without the effects of `handled`, which have the same
    /// arguments in `row`: what's left of the effects of a handler's
    /// scrutinee once its `match` handles its own.
    Without { row: Ty<'db>, handled: Box<[EffectTy<'db>]>, rest: Ty<'db> },
}

impl<'db> RowConstraint<'db> {
    pub fn contains(sub: Ty<'db>, sup: Ty<'db>) -> Self {
        RowConstraint::Contains { sub, sup }
    }

    pub fn map(&self, mut f: impl FnMut(&Ty<'db>) -> Ty<'db>) -> Self {
        match self {
            RowConstraint::Contains { sub, sup } => {
                RowConstraint::Contains { sub: f(sub), sup: f(sup) }
            }
            RowConstraint::Combine { parts, goal } => {
                RowConstraint::Combine { parts: parts.iter().map(&mut f).collect(), goal: f(goal) }
            }
            RowConstraint::Without { row, handled, rest } => RowConstraint::Without {
                row: f(row),
                handled: handled
                    .iter()
                    .map(|e| EffectTy {
                        effect: e.effect,
                        args: e.args.iter().map(&mut f).collect(),
                    })
                    .collect(),
                rest: f(rest),
            },
        }
    }

    /// The row that the constraint decides, which isn't free to be given its
    /// least solution: the goal of a combination, or what's left after a
    /// handler.
    fn decided(&self) -> Option<&Ty<'db>> {
        match self {
            RowConstraint::Contains { .. } => None,
            RowConstraint::Combine { goal, .. } => Some(goal),
            RowConstraint::Without { rest, .. } => Some(rest),
        }
    }

    /// The rows the constraint is about.
    pub fn rows(&self) -> Vec<&Ty<'db>> {
        match self {
            RowConstraint::Contains { sub, sup } => vec![sub, sup],
            RowConstraint::Combine { parts, goal } => parts.iter().chain([goal]).collect(),
            RowConstraint::Without { row, rest, .. } => vec![row, rest],
        }
    }
}

impl<'db> Solver<'db> {
    /// Solve a row constraint if its rows are known well enough. Returns
    /// `false` if it has to wait.
    pub(crate) fn solve_row(&mut self, c: &RowConstraint<'db>, origin: Origin<'db>) -> bool {
        match c {
            RowConstraint::Contains { sub, sup } => self.solve_contains(sub, sup, origin),
            RowConstraint::Combine { parts, goal } => self.solve_combine(parts, goal, origin),
            RowConstraint::Without { row, handled, rest } => {
                self.solve_without(row, handled, rest, origin)
            }
        }
    }

    fn solve_without(
        &mut self,
        row: &Ty<'db>,
        handled: &[EffectTy<'db>],
        rest: &Ty<'db>,
        origin: Origin<'db>,
    ) -> bool {
        let row = self.unifier.resolve(row);
        if row == Ty::Error {
            self.equal(rest, &Ty::Error, origin);
            return true;
        }
        let Some((effects, parts)) = row.row_parts() else { return false };
        let mut left = Vec::new();
        for e in effects {
            match handled.iter().find(|h| h.effect == e.effect) {
                // What the scrutinee does is what the arms are expected to
                // handle.
                Some(h) => self.same_args(h, e, origin),
                None => left.push(e.clone()),
            }
        }
        self.equal(rest, &Ty::row_of(left, parts), origin);
        true
    }

    fn solve_contains(&mut self, sub: &Ty<'db>, sup: &Ty<'db>, origin: Origin<'db>) -> bool {
        let sub = self.unifier.resolve(sub);
        let sup = self.unifier.resolve(sup);
        // No effects fit anywhere, and a row fits in itself.
        if sub == Ty::Error || sup == Ty::Error || sub == sup || sub == Ty::nothing() {
            return true;
        }
        // A row that the other is made of fits in it, whatever it is.
        if let Ty::Row(_, rest) = &sup
            && rest.contains(&sub)
        {
            return true;
        }
        let (Some((effects, parts)), Some((allowed, allowed_parts))) =
            (sub.row_parts(), sup.row_parts())
        else {
            // A variable on either side: wait until it's known, or until the
            // row is settled.
            return false;
        };
        let mut missing_effects = Vec::new();
        for e in effects {
            match allowed.iter().find(|a| a.effect == e.effect) {
                Some(a) => self.same_args(e, a, origin),
                None => missing_effects.push(e.clone()),
            }
        }
        // A row that stands for any row fits only in a row made of it: a
        // parameter like `E` isn't known to be in any other row.
        let missing_parts: Vec<Ty<'db>> =
            parts.into_iter().filter(|p| !allowed_parts.contains(p)).collect();
        if !missing_effects.is_empty() || !missing_parts.is_empty() {
            self.errors.push(TypeError::Effects {
                origin,
                found: Ty::row_of(missing_effects, missing_parts),
                allowed: sup.clone(),
            });
        }
        true
    }

    fn solve_combine(&mut self, parts: &[Ty<'db>], goal: &Ty<'db>, origin: Origin<'db>) -> bool {
        let mut parts: Vec<Ty<'db>> = parts.iter().map(|p| self.unifier.resolve(p)).collect();
        let unknown: Vec<usize> = (0..parts.len())
            .filter(|&i| parts[i].row_parts().is_none() && parts[i] != Ty::Error)
            .collect();
        if let [i] = unknown[..] {
            // With the goal known and one part not, that part is the least
            // row that gives the goal: what the goal has that the other parts
            // don't. So `Log ⊙ e ~ Log + Async` gives `e` just `Async`. Then
            // the combination is checked as if all its parts were known.
            let goal = self.unifier.resolve(goal);
            let Some((goal_effects, goal_rest)) = goal.row_parts() else { return false };
            let known: Vec<(Ty<'db>, Origin<'db>)> = parts
                .iter()
                .enumerate()
                .filter(|&(j, _)| j != i)
                .map(|(_, p)| (p.clone(), origin))
                .collect();
            let known = self.union(&known);
            let (known_effects, known_rest) = known.row_parts().unwrap_or((&[], Vec::new()));
            let effects: Vec<EffectTy<'db>> = goal_effects
                .iter()
                .filter(|e| !known_effects.iter().any(|k| k.effect == e.effect))
                .cloned()
                .collect();
            let rest: Vec<Ty<'db>> =
                goal_rest.into_iter().filter(|r| !known_rest.contains(r)).collect();
            let solution = Ty::row_of(effects, rest);
            self.equal(&parts[i], &solution, origin);
            parts[i] = solution;
        } else if !unknown.is_empty() {
            return false;
        }
        let parts: Vec<(Ty<'db>, Origin<'db>)> = parts.into_iter().map(|p| (p, origin)).collect();
        let union = self.union(&parts);
        self.equal(goal, &union, origin);
        true
    }

    /// The union of rows that aren't variables, with the arguments of an
    /// effect in several of them made the same, at the row's origin.
    fn union(&mut self, rows: &[(Ty<'db>, Origin<'db>)]) -> Ty<'db> {
        let mut effects: Vec<EffectTy<'db>> = Vec::new();
        let mut rest: Vec<Ty<'db>> = Vec::new();
        for (row, origin) in rows {
            let origin = *origin;
            let Some((es, rs)) = row.row_parts() else { continue };
            let mut same = Vec::new();
            effects = Ty::union_rows(&effects, es, |x, y| same.push((x.clone(), y.clone())));
            // The arguments found before are what's expected of this row's.
            for (x, y) in same {
                self.same_args(&y, &x, origin);
            }
            rest.extend(rs);
        }
        Ty::row_of(effects, rest)
    }

    fn same_args(&mut self, x: &EffectTy<'db>, y: &EffectTy<'db>, origin: Origin<'db>) {
        for (a, b) in x.args.iter().zip(y.args.iter()) {
            self.add(Constraint::Equal { expected: b.clone(), actual: a.clone(), origin });
        }
    }

    /// `ty` with each row that's made of variables, like the `Log + ?e` that
    /// instantiating `Log + E` gives, replaced by a new row variable that a
    /// combination constraint makes those rows.
    pub(crate) fn open_rows(&mut self, ty: &Ty<'db>, origin: Origin<'db>) -> Ty<'db> {
        let mut combines = Vec::new();
        let opened = ty.fold(&mut |t| match t {
            Ty::Row(effects, rest) if rest.iter().any(|r| matches!(r, Ty::Var(_))) => {
                let goal = self.unifier.new_var(VarKind::Effect);
                let mut parts = vec![Ty::row(effects.iter().cloned())];
                parts.extend(rest.iter().cloned());
                combines.push(RowConstraint::Combine { parts: parts.into(), goal: goal.clone() });
                Some(goal)
            }
            _ => None,
        });
        for constraint in combines {
            self.add(Constraint::Row { constraint, origin });
        }
        opened
    }

    /// Give rows that stay part of a type, in `keep`, the shape that a
    /// constraint that can't be solved asks of them, as Rose and Koka have
    /// it. For a combination `Log ⊙ e ~ g` with `g` a parameter's row, `g`
    /// becomes `Log + e`. For a handler of `Log` whose scrutinee's row is a
    /// parameter's row `r`, with `e` left, `r` becomes `Log + e`. The
    /// variables in the new rows are kept too. Without it, `e` would get no
    /// effects, and the type would only take functions that do just `Log`.
    /// Returns whether it changed anything.
    pub(crate) fn open_kept_rows(&mut self, keep: &mut Vec<TyVar>) -> bool {
        let mut changed = false;
        loop {
            self.solve();
            let mut opened = None;
            for (c, _) in &self.pending_rows {
                // The row that stays in the type, and what it's made of.
                let (row, parts): (&Ty<'db>, Vec<Ty<'db>>) = match c {
                    RowConstraint::Combine { parts, goal } => (goal, parts.to_vec()),
                    RowConstraint::Without { row, handled, rest } => {
                        (row, vec![Ty::row(handled.iter().cloned()), rest.clone()])
                    }
                    RowConstraint::Contains { .. } => continue,
                };
                let Ty::Var(g) = self.unifier.resolve(row) else { continue };
                if !keep.contains(&g) {
                    continue;
                }
                let mut effects: Vec<EffectTy<'db>> = Vec::new();
                let mut rest: Vec<Ty<'db>> = Vec::new();
                for part in parts.iter().map(|p| self.unifier.resolve(p)) {
                    match part.row_parts() {
                        Some((es, rs)) => {
                            effects = Ty::union_rows(&effects, es, |_, _| {});
                            rest.extend(rs);
                        }
                        None => rest.push(part),
                    }
                }
                let vars: Vec<TyVar> = rest
                    .iter()
                    .filter_map(|r| if let Ty::Var(v) = r { Some(*v) } else { None })
                    .collect();
                // A row that's one of its own parts says nothing new.
                if vars.contains(&g) {
                    continue;
                }
                opened = Some((g, Ty::Row(effects.into(), rest.into()), vars));
                break;
            }
            let Some((g, row, vars)) = opened else { return changed };
            if self.unifier.unify(&Ty::Var(g), &row).is_err() {
                return changed;
            }
            keep.extend(vars);
            changed = true;
        }
    }

    /// Give each row variable in `candidates` that `keep` doesn't have its
    /// least solution: the union of the rows that must fit in it, or no
    /// effects if there are none. The rows that must fit can be made of
    /// rows in `keep`, which stay as they are; a variable whose rows are
    /// still unknown otherwise waits for them.
    pub(crate) fn settle_rows(&mut self, candidates: &[TyVar], keep: &[TyVar]) {
        loop {
            self.solve();
            let mut progress = false;
            for &var in candidates {
                if keep.contains(&var) || !self.is_unbound(var) {
                    continue;
                }
                // A row that a combination or a handler decides isn't free
                // to choose.
                let combined = self.pending_rows.iter().any(|(c, _)| {
                    c.decided().is_some_and(|row| self.unifier.resolve(row) == Ty::Var(var))
                });
                if combined {
                    continue;
                }
                // What must fit in it, each once: a function called twice
                // adds its effects twice.
                let mut below: Vec<(Ty<'db>, Origin<'db>)> = Vec::new();
                for (c, origin) in &self.pending_rows {
                    let RowConstraint::Contains { sub, sup } = c else { continue };
                    if self.unifier.resolve(sup) != Ty::Var(var) {
                        continue;
                    }
                    let sub = self.unifier.resolve(sub);
                    if !below.iter().any(|(t, _)| *t == sub) {
                        below.push((sub, *origin));
                    }
                }
                let settled = |t: &Ty<'db>| match t {
                    Ty::Var(v) => keep.contains(v),
                    _ => true,
                };
                if !below.iter().all(|(t, _)| settled(t)) {
                    continue;
                }
                let rows: Vec<(Ty<'db>, Origin<'db>)> = below
                    .iter()
                    .map(|(t, origin)| match t {
                        // A variable that stays one is a part of the row, as
                        // a parameter is.
                        Ty::Var(_) => (Ty::Row(Box::new([]), Box::new([t.clone()])), *origin),
                        t => (t.clone(), *origin),
                    })
                    .collect();
                match below.first().map(|(_, o)| *o) {
                    Some(origin) => {
                        let solution = self.union(&rows);
                        self.equal(&Ty::Var(var), &solution, origin);
                    }
                    None => {
                        let _ = self.unifier.unify(&Ty::Var(var), &Ty::nothing());
                    }
                }
                progress = true;
            }
            // Rows that only wait for each other, like those of functions
            // that call each other, have the same least solution, so they
            // can be made one.
            if !progress {
                progress = self.merge_waiting_rows(candidates, keep);
            }
            if !progress {
                return;
            }
        }
    }

    /// Make a row variable of `candidates` one with the rows that must fit
    /// in it, if those are all variables of `candidates` that aren't in
    /// `keep`. Returns whether it did.
    fn merge_waiting_rows(&mut self, candidates: &[TyVar], keep: &[TyVar]) -> bool {
        let free = |v: &TyVar| candidates.contains(v) && !keep.contains(v);
        for &var in candidates {
            if !free(&var) || !self.is_unbound(var) {
                continue;
            }
            let mut below = Vec::new();
            for (c, origin) in &self.pending_rows {
                if let RowConstraint::Contains { sub, sup } = c
                    && self.unifier.resolve(sup) == Ty::Var(var)
                {
                    below.push((self.unifier.resolve(sub), *origin));
                }
            }
            let waiting = !below.is_empty()
                && below.iter().all(|(t, _)| matches!(t, Ty::Var(v) if free(v) && *v != var));
            if waiting {
                for (t, origin) in below {
                    self.equal(&Ty::Var(var), &t, origin);
                }
                return true;
            }
        }
        false
    }

    fn is_unbound(&self, var: TyVar) -> bool {
        matches!(self.unifier.shallow_resolve(&Ty::Var(var)), Ty::Var(w) if w == var)
    }

    /// Give the rows that nothing decided, and that nothing must fit in, no
    /// effects, unless they're in `keep`. A row that's still unknown, like
    /// that of a closure that calls nothing, would keep class constraints
    /// about its function type from being solved.
    pub(crate) fn default_free_rows(&mut self, keep: &[TyVar]) {
        let decided: Vec<TyVar> = self
            .pending_rows
            .iter()
            .filter_map(|(c, _)| {
                let row = match c {
                    RowConstraint::Contains { sup, .. } => sup,
                    c => c.decided()?,
                };
                match self.unifier.resolve(row) {
                    Ty::Var(v) => Some(v),
                    _ => None,
                }
            })
            .collect();
        for var in self.unifier.unbound_rows() {
            if !keep.contains(&var) && !decided.contains(&var) {
                let _ = self.unifier.unify(&Ty::Var(var), &Ty::nothing());
            }
        }
        self.solve();
    }

    /// The row constraints left, resolved.
    pub fn take_pending_rows(&mut self) -> Vec<(RowConstraint<'db>, Origin<'db>)> {
        std::mem::take(&mut self.pending_rows)
            .into_iter()
            .map(|(c, origin)| (c.map(|t| self.unifier.resolve(t)), origin))
            .collect()
    }

    /// The row variables of the row constraints left.
    pub(crate) fn pending_row_vars(&self) -> Vec<TyVar> {
        let mut vars = Vec::new();
        for (c, _) in &self.pending_rows {
            for row in c.rows() {
                vars_in(&self.unifier.resolve(row), &mut vars);
            }
        }
        vars.retain(|v| self.unifier.var_kind(*v) == Some(VarKind::Effect));
        vars
    }

    /// Report the row constraints that are left once nothing more can be
    /// known, like that of a call in top-level code of a function whose
    /// effects are a variable nothing decided. Its variables have no effects
    /// then.
    pub(crate) fn finish_rows(&mut self) {
        let vars = self.pending_row_vars();
        self.settle_rows(&vars, &[]);
        self.default_free_rows(&[]);
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

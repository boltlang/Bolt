//! The unifier: makes two types equal by binding inference variables. It
//! knows nothing about where types come from; the
//! [`Solver`](crate::solve::Solver) turns its errors into diagnostics.
//!
//! The variables are a union-find table: a variable is either unbound, or
//! bound to a type, which may be another variable.

use boltlang_hir_def::{
    Name,
    builtin_type::{BuiltinFloat, BuiltinInt, BuiltinType},
};

use crate::{
    ty::{Mutability, NumberKind, PackHalf, TupleParts, Ty, TyVar, VarKind},
    variance::{Variance, Variances, arg_variance},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnifyError<'db> {
    /// The innermost types that differ.
    Mismatch(Ty<'db>, Ty<'db>),
    /// Binding the variable would make an infinite type, like `?0 = [?0]`.
    Occurs(TyVar, Ty<'db>),
}

#[derive(Debug, Clone)]
enum VarState<'db> {
    Unbound(VarKind),
    Bound(Ty<'db>),
}

#[derive(Debug, Clone)]
struct Var<'db> {
    state: VarState<'db>,
    /// Whether the variable met the error type, so that it becomes the error
    /// type if nothing else decides it: see [`Unifier::error_unknowns`].
    met_error: bool,
    /// Whether it's the variable of a number literal that got its default
    /// type, so that type errors can say it's a number: see
    /// [`Unifier::resolve_literals`].
    defaulted: Option<NumberKind>,
    /// Whether it must be at most `ro`, so that a value it receives that
    /// holds what may change is seen as `ro`: see [`Unifier::see_as_ro`].
    seen_as_ro: bool,
    /// Whether it's a variable of a call that's being checked, which takes
    /// the least type of what flows into it, `lowers`, once the arguments
    /// are checked, rather than the first: see [`Unifier::make_flexible`].
    flexible: bool,
    lowers: Vec<Ty<'db>>,
}

/// A unification that gave a flexible variable a lower bound, to be done
/// again once the variable is settled, with what it's settled to. That
/// checks what the least type can't by itself, like the contents of a
/// `mut` array whose mutability was a variable too.
#[derive(Debug, Clone)]
pub(crate) struct Replay<'db> {
    pub a: Ty<'db>,
    pub b: Ty<'db>,
    pub flipped: bool,
    /// Where it was unified, as the solver numbers it: see [`Unifier::site`].
    pub site: u32,
}

/// That a function whose name slot is `hi` flows into the variable `lo`,
/// like `f` into `k` in `let k = f`: `lo` can only be called by the names
/// that all its sources have.
#[derive(Debug, Clone)]
struct NameEdge<'db> {
    lo: Ty<'db>,
    hi: Ty<'db>,
}

/// The names a parameter's slot can be called by (#411): any, while
/// nothing decided it, one, or none.
#[derive(Debug, Clone, PartialEq, Eq)]
enum NameSet {
    Any,
    One(Name),
    Empty,
}

impl NameSet {
    fn of(name: &Option<Name>) -> Self {
        match name {
            Some(name) => NameSet::One(name.clone()),
            None => NameSet::Empty,
        }
    }

    fn meet(&self, other: &NameSet) -> NameSet {
        match (self, other) {
            (NameSet::Any, s) | (s, NameSet::Any) => s.clone(),
            (NameSet::One(a), NameSet::One(b)) if a == b => NameSet::One(a.clone()),
            _ => NameSet::Empty,
        }
    }

    fn contains(&self, name: &Name) -> bool {
        match self {
            NameSet::Any => true,
            NameSet::One(n) => n == name,
            NameSet::Empty => false,
        }
    }
}

/// What a call by name finds in a slot: a name, a slot that it can claim,
/// or no name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotNow {
    Named(Name),
    Open,
    Unnamed,
}

#[derive(Debug, Clone, Default)]
pub struct Unifier<'db> {
    vars: Vec<Var<'db>>,
    /// How the arguments of structs and enums can change (#440).
    variances: Variances<'db>,
    /// Inside the parameters of a function type, an odd number of times:
    /// functions flow the other way around there.
    flipped: bool,
    names: Vec<NameEdge<'db>>,
    /// The names that calls passed name variables by.
    claims: std::collections::HashMap<TyVar, Name>,
    /// Each function type that was given for another, as the one that
    /// received it and the one given, for the warning about swapped names
    /// (#433). The solver takes them with where they were unified.
    pub(crate) given_fns: Vec<(Ty<'db>, Ty<'db>)>,
    /// How deep unifications are nested, and whether the outermost one
    /// gave a flexible variable a lower bound.
    depth: u32,
    recorded: bool,
    /// Where what's unified now is, as the solver numbers it, for
    /// [`Replay`]s.
    pub(crate) site: u32,
    pub(crate) replays: Vec<Replay<'db>>,
    /// The flexible variables that are the mutabilities of the objects
    /// whose contents are being unified, innermost last, and what each
    /// lower bound recorded inside one was: see [`Unifier::force_linked`].
    under: Vec<TyVar>,
    pub(crate) links: Vec<(TyVar, TyVar, Ty<'db>)>,
    /// Whether a flexible variable that gives a value is bound to what
    /// receives it, rather than settled: see [`Unifier::unify_binding`].
    binding: bool,
}

impl<'db> Unifier<'db> {
    /// A unifier where the arguments of every struct and enum are the same
    /// both ways, as for unit tests and for whether impls overlap.
    pub fn new() -> Self {
        Self::default()
    }

    /// A unifier where the arguments of structs and enums change as their
    /// variance says.
    pub fn with_variances(variances: Variances<'db>) -> Self {
        Unifier { variances, ..Self::default() }
    }

    pub fn new_var(&mut self, kind: VarKind) -> Ty<'db> {
        let var = TyVar(self.vars.len() as u32);
        self.vars.push(Var {
            state: VarState::Unbound(kind),
            met_error: false,
            defaulted: None,
            seen_as_ro: false,
            flexible: false,
            lowers: Vec::new(),
        });
        Ty::Var(var)
    }

    /// The kind of a variable, if it's still unbound.
    pub fn var_kind(&self, var: TyVar) -> Option<VarKind> {
        match self.vars[var.0 as usize].state {
            VarState::Unbound(kind) => Some(kind),
            VarState::Bound(_) => None,
        }
    }

    /// Follow bound variables until reaching a type that isn't one. The
    /// head of an application is resolved too, so that `?0[i32]` with `?0`
    /// bound to `Option` is `Option[i32]`.
    pub fn shallow_resolve(&self, ty: &Ty<'db>) -> Ty<'db> {
        let mut ty = ty.clone();
        loop {
            match ty {
                Ty::Var(var) => match &self.vars[var.0 as usize].state {
                    VarState::Bound(bound) => ty = bound.clone(),
                    VarState::Unbound(_) => return ty,
                },
                Ty::App(head, args) => {
                    let head = self.shallow_resolve(&head);
                    return Ty::app(head, args.into_vec());
                }
                // A `ro T` whose `T` is known is the view of it (#320).
                Ty::Ro(_) => return self.view_known_ro(ty),
                _ => return ty,
            }
        }
    }

    /// Replace every bound variable in a type by what it's bound to.
    pub fn resolve(&self, ty: &Ty<'db>) -> Ty<'db> {
        ty.fold(&mut |t| match t {
            Ty::Var(_) => match self.shallow_resolve(t) {
                Ty::Var(var) => Some(Ty::Var(var)),
                resolved => Some(self.resolve(&resolved)),
            },
            // A number holds nothing, so it's its own view.
            Ty::Ro(inner) => match self.shallow_resolve(inner) {
                Ty::Var(v) if matches!(self.var_kind(v), Some(VarKind::Int | VarKind::Float)) => {
                    Some(Ty::Var(v))
                }
                _ => None,
            },
            _ => None,
        })
        .normalize_ro(self.variances)
    }

    /// A `ro T` whose `T` is known as the view of it, so that unifying
    /// compares what it is.
    fn view_known_ro(&self, ty: Ty<'db>) -> Ty<'db> {
        let Ty::Ro(inner) = &ty else { return ty };
        match self.shallow_resolve(inner) {
            // A number holds nothing, so it's its own view.
            Ty::Var(v) if matches!(self.var_kind(v), Some(VarKind::Int | VarKind::Float)) => {
                Ty::Var(v)
            }
            Ty::Var(_) | Ty::Param(_) | Ty::Bound(_) | Ty::App(..) | Ty::Assoc(..) | Ty::Imm(_) => {
                ty
            }
            _ => self.resolve(&ty),
        }
    }

    /// Give unbound integer and float variables their default types.
    pub fn default_numeric_vars(&mut self) {
        for var in &mut self.vars {
            let (ty, kind) = match var.state {
                VarState::Unbound(VarKind::Int) => {
                    (BuiltinType::Int(BuiltinInt::I32), NumberKind::Int)
                }
                VarState::Unbound(VarKind::Float) => {
                    (BuiltinType::Float(BuiltinFloat::F64), NumberKind::Float)
                }
                _ => continue,
            };
            var.state = VarState::Bound(Ty::Builtin(ty));
            var.defaulted = Some(kind);
        }
    }

    /// Make the mutabilities that nothing decided `Imm`: an array that
    /// nothing changes can't be changed.
    pub fn default_mutability_vars(&mut self) {
        for var in &mut self.vars {
            if matches!(var.state, VarState::Unbound(VarKind::Mutability)) {
                var.state = VarState::Bound(Ty::Mutability(Mutability::Imm));
            }
        }
    }

    /// The row variables that nothing decided yet.
    pub fn unbound_rows(&self) -> Vec<TyVar> {
        (0..self.vars.len() as u32)
            .map(TyVar)
            .filter(|v| matches!(self.vars[v.0 as usize].state, VarState::Unbound(VarKind::Effect)))
            .collect()
    }

    /// Make the rows that nothing decided empty: a function that nothing
    /// says does something does nothing.
    pub fn default_effect_vars(&mut self) {
        for var in &mut self.vars {
            if matches!(var.state, VarState::Unbound(VarKind::Effect)) {
                var.state = VarState::Bound(Ty::nothing());
            }
        }
    }

    /// Like [`Unifier::resolve`], but a number literal whose type only comes
    /// from defaulting is [`Ty::Number`], for type errors: the `1` of
    /// `let a: f64 = 1;` is `{integer}`, not `i32`.
    pub fn resolve_literals(&self, ty: &Ty<'db>) -> Ty<'db> {
        ty.fold(&mut |t| {
            let &Ty::Var(mut var) = t else { return None };
            loop {
                let info = &self.vars[var.0 as usize];
                if let Some(kind) = info.defaulted {
                    return Some(Ty::Number(kind));
                }
                match &info.state {
                    VarState::Unbound(_) => return Some(Ty::Var(var)),
                    VarState::Bound(Ty::Var(next)) => var = *next,
                    VarState::Bound(ty) => return Some(self.resolve_literals(ty)),
                }
            }
        })
    }

    /// Make the variables that met the error type and are still unknown the
    /// error type, once inference is done. Otherwise a type that's only
    /// unknown because of an error would become a type parameter, like the
    /// `a` of `forall a. fn() -> a` for `fn h() = 1..2`. A variable isn't
    /// bound when it meets the error, so that what comes after can still
    /// decide it: the second arm of `match s { Nope(x) => x, _ => 0.0 }`
    /// makes it a float.
    pub fn error_unknowns(&mut self) {
        for var in &mut self.vars {
            if var.met_error && matches!(var.state, VarState::Unbound(_)) {
                var.state = VarState::Bound(Ty::Error);
            }
        }
    }

    /// Remember that `ty` met an error if it's a variable, like the parts of
    /// a pattern that can't match, so that [`Unifier::error_unknowns`] makes
    /// it the error type if nothing else decides it.
    pub fn met_error(&mut self, ty: &Ty<'db>) {
        if let Ty::Var(var) = self.shallow_resolve(ty) {
            self.vars[var.0 as usize].met_error = true;
        }
    }

    /// Whether a variable in `ty` met the error type, so that what's unknown
    /// about it is only unknown because of an error already reported.
    pub fn has_error_var(&self, ty: &Ty<'db>) -> bool {
        self.resolve(ty).any(&mut |t| matches!(t, Ty::Var(v) if self.vars[v.0 as usize].met_error))
    }

    /// Make `ty` the error type if it's a variable, after reporting an error
    /// about it, so that it doesn't cause more. [`Unifier::unify`] doesn't,
    /// since the error type fits anywhere.
    pub fn set_error(&mut self, ty: &Ty<'db>) {
        if let Ty::Var(var) = self.shallow_resolve(ty) {
            let _ = self.bind(var, &Ty::Error);
        }
    }

    /// Like [`Unifier::unify`], but the names of parameters flow from `a`
    /// into `b`, like from the value a pattern matches into its parts.
    pub fn unify_into(&mut self, a: &Ty<'db>, b: &Ty<'db>) -> Result<(), UnifyError<'db>> {
        self.reversed(|u| u.unify(a, b))
    }

    /// Let the variable `var`, of a type that must be at most `ro`, like the
    /// `T` of `fn pass[T: ro](x: T)`, receive what it's given seen as `ro`
    /// when that holds what may change: `pass(mut [1])` makes `T` a
    /// `ro [i32]`, which a `mut [i32]` fits, and `pass([1])` an `imm [i32]`.
    /// What's still unknown is taken as it is, and checked by the bound.
    pub(crate) fn see_as_ro(&mut self, var: TyVar) {
        self.vars[var.0 as usize].seen_as_ro = true;
    }

    /// `ty` seen as `ro`, if it holds what may change, and `ty` otherwise.
    fn view_if_it_may_change(&self, ty: &Ty<'db>) -> Ty<'db> {
        let resolved = self.resolve(ty);
        if may_change(self, &resolved) {
            resolved.ro_view(self.variances, false).normalize_ro(self.variances)
        } else {
            ty.clone()
        }
    }

    /// Let the variable `var`, of the scheme of a function being called,
    /// take the least type of what flows into it, once the call's arguments
    /// are checked ([`Unifier::settle`]), rather than the first. So
    /// `both([1], mut [2])` for `fn both[T](a: T, b: T) -> T` makes `T` a
    /// `ro [i32]`, as `Ty.lub` does in `lean/BoltMut/Least.lean`.
    pub(crate) fn make_flexible(&mut self, var: TyVar) {
        if self.var_kind(var) == Some(VarKind::General) {
            self.vars[var.0 as usize].flexible = true;
        }
    }

    fn is_flexible(&self, var: TyVar) -> bool {
        let var = &self.vars[var.0 as usize];
        var.flexible && matches!(var.state, VarState::Unbound(_))
    }

    /// Give a flexible variable the least type of what flowed into it, and
    /// at most `ro` if it must be ([`Unifier::see_as_ro`]). What the least
    /// type can't be found for, like functions that take different things,
    /// is the first that flowed in, as without flexible variables. Either
    /// way, the unifications that gave it its lower bounds are done again
    /// ([`Replay`]), which reports what doesn't fit.
    pub(crate) fn settle(&mut self, var: TyVar) {
        if !self.is_flexible(var) {
            return;
        }
        let data = &mut self.vars[var.0 as usize];
        data.flexible = false;
        let lowers = std::mem::take(&mut data.lowers);
        let ro = data.seen_as_ro;
        let Some(first) = lowers.first() else { return };
        let mut least = self.lub(first, first, ro);
        for lower in &lowers[1..] {
            least = least.and_then(|t| self.lub(&t, lower, ro));
        }
        // The least type is at most `ro` already, and isn't seen as `ro`
        // again when it's bound.
        if least.is_ok() {
            self.vars[var.0 as usize].seen_as_ro = false;
        }
        let ty = least.unwrap_or_else(|()| first.clone());
        let _ = self.unify_flipped(&Ty::Var(var), &ty, false);
    }

    /// Settle the flexible variables in `ty`.
    pub(crate) fn settle_in(&mut self, ty: &Ty<'db>) {
        let mut found = Vec::new();
        self.resolve(ty).any(&mut |t| {
            if let Ty::Var(v) = t
                && self.is_flexible(*v)
            {
                found.push(*v);
            }
            false
        });
        for v in found {
            self.settle(v);
        }
    }

    /// The least type that both `a` and `b` fit, and with `ro` the least
    /// such type that's at most `ro`, as `Ty.lub` in
    /// `lean/BoltMut/Least.lean`: the same type, or one with objects seen as
    /// `ro` where they differ or may change. What isn't known yet is made
    /// the same on both sides.
    fn lub(&mut self, a: &Ty<'db>, b: &Ty<'db>, ro: bool) -> Result<Ty<'db>, ()> {
        let (a, b) = (self.shallow_resolve(a), self.shallow_resolve(b));
        for t in [&a, &b] {
            if let Ty::Var(v) = t
                && self.is_flexible(*v)
            {
                self.settle(*v);
                return self.lub(&a, &b, ro);
            }
        }
        let viewed = |u: &Self, t: Ty<'db>| match ro && may_change(u, &u.resolve(&t)) {
            true => Ty::Ro(Box::new(t)).normalize_ro(u.variances),
            false => t,
        };
        match (&a, &b) {
            (Ty::Error, _) | (_, Ty::Error) => Ok(Ty::Error),
            (Ty::Never, t) | (t, Ty::Never) => {
                let t = t.clone();
                self.lub(&t, &t, ro)
            }
            (Ty::Var(_), _) | (_, Ty::Var(_)) => {
                self.unify_flipped(&a, &b, false).map_err(|_| ())?;
                Ok(viewed(self, self.shallow_resolve(&a)))
            }
            (Ty::Array(m, x), Ty::Array(n, y)) => {
                let pairs = [((**x).clone(), (**y).clone())];
                self.lub_object(m, n, &pairs, ro, |m, cs| {
                    Ty::Array(Box::new(m), Box::new(cs[0].clone()))
                })
            }
            (Ty::Tuple(xs, m), Ty::Tuple(ys, n))
                if xs.len() == ys.len() && !a.has_spread() && !b.has_spread() =>
            {
                let pairs: Vec<_> = xs.iter().cloned().zip(ys.iter().cloned()).collect();
                self.lub_object(m, n, &pairs, ro, |m, cs| Ty::Tuple(cs.into(), Box::new(m)))
            }
            (Ty::Record(xs, xn, m), Ty::Record(ys, yn, n))
                if xs.len() == ys.len()
                    && xn.len() == yn.len()
                    && xn.iter().zip(yn.iter()).all(|((a, _), (b, _))| a == b)
                    && !a.has_spread()
                    && !b.has_spread() =>
            {
                let names: Vec<_> = xn.iter().map(|(name, _)| name.clone()).collect();
                let named =
                    xn.iter().map(|(_, t)| t.clone()).zip(yn.iter().map(|(_, t)| t.clone()));
                let pairs: Vec<_> =
                    xs.iter().cloned().zip(ys.iter().cloned()).chain(named).collect();
                let positional = xs.len();
                self.lub_object(m, n, &pairs, ro, |m, cs| {
                    let named = names.iter().cloned().zip(cs[positional..].iter().cloned());
                    Ty::Record(cs[..positional].into(), named.collect(), Box::new(m))
                })
            }
            // A struct or enum with the same arguments, whose own mutability
            // may differ; seen as `ro`, its arguments are too.
            (Ty::Adt(x, xs, ms, m), Ty::Adt(y, ys, ns, n))
                if x == y && self.same_all(xs, ys) && self.same_all(ms, ns) =>
            {
                let (id, xs, ms) = (*x, xs.clone(), ms.clone());
                let adt = self.lub_object(m, n, &[], ro, |m, _| {
                    Ty::Adt(id, xs.clone(), ms.clone(), Box::new(m))
                })?;
                Ok(match &adt {
                    Ty::Adt(.., m) if **m == Ty::Mutability(Mutability::ReadOnly) => {
                        adt.ro_view(self.variances, false).normalize_ro(self.variances)
                    }
                    _ => adt,
                })
            }
            // Two mutabilities, as of a variable for a hidden mutability
            // parameter: the same one, or `ro`, which every one fits.
            (Ty::Mutability(_), Ty::Mutability(_) | Ty::Param(_))
            | (Ty::Param(_), Ty::Mutability(_)) => {
                let same = a == b && !(ro && a == Ty::Mutability(Mutability::Mut));
                Ok(if same { a.clone() } else { Ty::Mutability(Mutability::ReadOnly) })
            }
            (Ty::Param(x), Ty::Ro(y)) | (Ty::Ro(y), Ty::Param(x)) if **y == Ty::Param(*x) => {
                Ok(Ty::Ro(y.clone()))
            }
            _ if self.resolve(&a) == self.resolve(&b) => Ok(viewed(self, a.clone())),
            _ => Err(()),
        }
    }

    /// Whether `xs` and `ys` are the same types, as far as they're known.
    fn same_all(&self, xs: &[Ty<'db>], ys: &[Ty<'db>]) -> bool {
        xs.len() == ys.len() && xs.iter().zip(ys).all(|(x, y)| self.resolve(x) == self.resolve(y))
    }

    /// The least type of two objects, of mutabilities `m` and `n`, with what
    /// they hold, `pairs`: an `imm` one when both are and what they hold
    /// has a least type, the same object when they're the same, and a `ro`
    /// one otherwise. `make` makes the object of a mutability and contents.
    fn lub_object(
        &mut self,
        m: &Ty<'db>,
        n: &Ty<'db>,
        pairs: &[(Ty<'db>, Ty<'db>)],
        ro: bool,
        make: impl Fn(Ty<'db>, Vec<Ty<'db>>) -> Ty<'db>,
    ) -> Result<Ty<'db>, ()> {
        let (mut m, mut n) = (self.shallow_resolve(m), self.shallow_resolve(n));
        // A mutability that isn't known yet is the other.
        if matches!(m, Ty::Var(_)) || matches!(n, Ty::Var(_)) {
            self.unify_flipped(&m, &n, false).map_err(|_| ())?;
            (m, n) = (self.shallow_resolve(&m), self.shallow_resolve(&n));
        }
        let imm = Ty::Mutability(Mutability::Imm);
        if m == imm && n == imm {
            let held: Result<Vec<_>, ()> =
                pairs.iter().map(|(x, y)| self.lub(x, y, false)).collect();
            if let Ok(held) = held
                && (!ro || !held.iter().any(|t| may_change(self, &self.resolve(t))))
            {
                return Ok(make(imm, held));
            }
        } else if m == n {
            // The same object when what it holds is the same on both sides,
            // once what isn't known in it is.
            let held: Result<Vec<_>, ()> =
                pairs.iter().map(|(x, y)| self.lub(x, y, false)).collect();
            if let Ok(held) = held {
                let same = pairs.iter().zip(&held).all(|((x, y), h)| {
                    let h = self.resolve(h);
                    self.resolve(x) == h && self.resolve(y) == h
                });
                let at_most_ro =
                    matches!(m, Ty::Mutability(Mutability::Imm | Mutability::ReadOnly))
                        && !held.iter().any(|t| may_change(self, &self.resolve(t)));
                if same && (!ro || at_most_ro) {
                    return Ok(make(m, held));
                }
            }
        }
        let held: Result<Vec<_>, ()> = pairs.iter().map(|(x, y)| self.lub(x, y, true)).collect();
        Ok(make(Ty::Mutability(Mutability::ReadOnly), held?))
    }

    /// `f` the other way round: with what receives a value and what gives
    /// it swapped, as in the parameters of a function type.
    fn reversed<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        self.flipped = !self.flipped;
        let out = f(self);
        self.flipped = !self.flipped;
        out
    }

    /// Unify `a`, which receives a value, with `b`, where a row that's
    /// smaller fits, as the effects of a function type: one that does less
    /// fits where one that does more is expected, and an effect in both has
    /// the same arguments. When either row is still a variable, they're the
    /// same row, so that inferring one gives the other. Other types are
    /// unified as they are.
    fn unify_contained(&mut self, a: &Ty<'db>, b: &Ty<'db>) -> Result<(), UnifyError<'db>> {
        let (sub, sup) = if self.flipped { (a, b) } else { (b, a) };
        // Only rows, which a struct's other arguments aren't, and which a
        // type parameter could otherwise be taken for.
        let is_row = |u: &Self, t: &Ty<'db>| matches!(u.shallow_resolve(t), Ty::Row(..));
        if !is_row(self, sub) && !is_row(self, sup) {
            return self.unify(a, b);
        }
        let (sub, sup) = (self.resolve(sub), self.resolve(sup));
        let Some(fit) = sub.row_fit(&sup) else { return self.unify(a, b) };
        if !fit.fits() {
            return Err(UnifyError::Mismatch(a.clone(), b.clone()));
        }
        for (given, expected) in &fit.same {
            let (x, y) = if self.flipped { (*given, expected) } else { (expected, *given) };
            self.unify_all(&x.args, &y.args)?;
        }
        Ok(())
    }

    /// Unify `a`, which receives a value, with `b`, as `variance` says it
    /// can change: as it is, the other way round, or both ways when it
    /// can't change at all. A row that can change takes a smaller one, as
    /// the effects of a function do, like the `E` of
    /// `struct Lazy[E] { run: fn() does E }`.
    fn unify_by(
        &mut self,
        variance: Variance,
        a: &Ty<'db>,
        b: &Ty<'db>,
    ) -> Result<(), UnifyError<'db>> {
        match variance {
            Variance::Unused | Variance::Co => self.unify_contained(a, b),
            Variance::Contra => self.reversed(|u| u.unify_contained(a, b)),
            Variance::Inv => {
                self.unify(a, b)?;
                self.reversed(|u| u.unify(a, b))
            }
        }
    }

    /// Unify `a` and `b` so that the names of parameters flow both ways:
    /// they share their name slots, like a place and what it denotes.
    pub fn unify_shared(&mut self, a: &Ty<'db>, b: &Ty<'db>) -> Result<(), UnifyError<'db>> {
        self.unify(a, b)?;
        self.unify_into(a, b)
    }

    /// What's in a `mut` array, struct or tuple, which `xs` and `ys` are
    /// the contents of, can be written through either side, so their names
    /// flow both ways (#436). In one that isn't `mut`, they only flow the
    /// way the value does.
    fn share_if_mut(
        &mut self,
        mutability: &Ty<'db>,
        xs: &[Ty<'db>],
        ys: &[Ty<'db>],
    ) -> Result<(), UnifyError<'db>> {
        if self.shallow_resolve(mutability) != Ty::Mutability(Mutability::Mut) {
            return Ok(());
        }
        self.reversed(|u| u.unify_all(xs, ys))
    }

    pub fn unify(&mut self, a: &Ty<'db>, b: &Ty<'db>) -> Result<(), UnifyError<'db>> {
        self.depth += 1;
        let top = self.depth == 1;
        if top {
            self.recorded = false;
        }
        let unified = self.unify_inner(a, b);
        self.depth -= 1;
        if top && self.recorded && unified.is_ok() {
            let replay =
                Replay { a: a.clone(), b: b.clone(), flipped: self.flipped, site: self.site };
            self.replays.push(replay);
        }
        unified
    }

    /// Do `f`, which unifies the contents of two objects of mutabilities `m`
    /// and `n`, with the lower bounds it records linked to the mutability of
    /// the one that receives a value, if that's a flexible variable.
    fn under<T>(&mut self, m: &Ty<'db>, n: &Ty<'db>, f: impl FnOnce(&mut Self) -> T) -> T {
        let receives = self.shallow_resolve(if self.flipped { n } else { m });
        let pushed = match receives {
            Ty::Var(v) if self.is_flexible(v) => {
                self.under.push(v);
                true
            }
            _ => false,
        };
        let out = f(self);
        if pushed {
            self.under.pop();
        }
        out
    }

    /// Before the variables `vars` of a call are settled, make the
    /// mutability of an object `ro` where what it holds is widened, as
    /// [`Unifier::lub`] does for a single type, and what a `ro` one holds
    /// at most `ro`, until that changes nothing. `links` are what was
    /// recorded in the contents of objects whose mutability is a flexible
    /// variable. So for `fn f[T](a: [T], b: [T])`, given a `mut [mut [i32]]`
    /// and a `mut [[i32]]`, `T` is `ro [i32]` and the array `ro` too, as
    /// `LinkedProblem.principal` in `lean/BoltMut/Linked.lean` shows is the
    /// least solution.
    pub(crate) fn force_linked(&mut self, links: &[(TyVar, TyVar, Ty<'db>)]) {
        let ro = Ty::Mutability(Mutability::ReadOnly);
        let mut forced: Vec<TyVar> = Vec::new();
        loop {
            let mut changed = false;
            for (outer, inner, lower) in links {
                if !forced.contains(outer) && self.widened(*inner, lower, &forced) {
                    forced.push(*outer);
                    changed = true;
                }
                if forced.contains(outer) && self.is_flexible(*inner) {
                    // What a `ro` object holds is at most `ro`.
                    let data = &self.vars[inner.0 as usize];
                    let is_mutability = data
                        .lowers
                        .iter()
                        .any(|l| matches!(self.shallow_resolve(l), Ty::Mutability(_)));
                    if is_mutability {
                        let holds_mut = data
                            .lowers
                            .iter()
                            .any(|l| self.shallow_resolve(l) == Ty::Mutability(Mutability::Mut));
                        if holds_mut && !forced.contains(inner) {
                            forced.push(*inner);
                            changed = true;
                        }
                    } else if !data.seen_as_ro {
                        self.vars[inner.0 as usize].seen_as_ro = true;
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        for v in forced {
            if self.is_flexible(v) {
                self.vars[v.0 as usize].lowers.push(ro.clone());
            }
        }
    }

    /// Whether the flexible variable `var` will be settled to something
    /// other than `lower`, one of its lower bounds: when they aren't all
    /// the same, or it's at most `ro` and `lower` may change, or it's
    /// `forced` to be `ro`.
    fn widened(&self, var: TyVar, lower: &Ty<'db>, forced: &[TyVar]) -> bool {
        if !self.is_flexible(var) {
            return self.resolve(&Ty::Var(var)) != self.resolve(lower);
        }
        if forced.contains(&var) {
            return true;
        }
        let data = &self.vars[var.0 as usize];
        let resolved = self.resolve(lower);
        let same = data.lowers.iter().all(|l| self.resolve(l) == resolved);
        if same && !(data.seen_as_ro && may_change(self, &resolved)) {
            return false;
        }
        // What isn't known yet, like the elements of `[1]` and `[2]`, is made
        // the same when it's settled: see what it'd be.
        let mut u = self.clone();
        u.settle(var);
        u.resolve(&Ty::Var(var)) != u.resolve(lower)
    }

    /// Bind those of the flexible variables `vars` that are mutabilities in
    /// `actual`, like the `'m` of `'m [i32]`, to what `expected` makes them,
    /// rather than settle them from what flowed into them: the unifications
    /// that gave them lower bounds check that when they're done again, and
    /// report an argument that doesn't fit. Nothing else is decided:
    /// `actual` is unified with `expected` where it's checked, which reports
    /// the call when a type doesn't fit.
    pub(crate) fn decide_by(&mut self, vars: &[TyVar], expected: &Ty<'db>, actual: &Ty<'db>) {
        let actual = self.resolve(actual);
        let mut mutabilities = Vec::new();
        mutability_vars(&actual, &mut mutabilities);
        let open: Vec<TyVar> = vars
            .iter()
            .copied()
            .filter(|&v| self.is_flexible(v) && mutabilities.contains(&v))
            .collect();
        if open.is_empty() {
            return;
        }
        let mut u = self.clone();
        u.binding = true;
        if u.unify_flipped(expected, &actual, false).is_err() {
            return;
        }
        for v in open {
            let ty = u.resolve(&Ty::Var(v));
            // A variable the copy made, like a parameter's name, isn't here.
            let known = self.vars.len() as u32;
            let new = ty.any(&mut |t| matches!(t, Ty::Var(w) if w.0 >= known));
            if !u.is_flexible(v) && !new {
                self.vars[v.0 as usize].flexible = false;
                let _ = self.bind(v, &ty);
            }
        }
    }

    /// Unify `a` and `b` as [`Unifier::unify`] does, with what receives a
    /// value on the side `flipped` says.
    pub(crate) fn unify_flipped(
        &mut self,
        a: &Ty<'db>,
        b: &Ty<'db>,
        flipped: bool,
    ) -> Result<(), UnifyError<'db>> {
        let saved = std::mem::replace(&mut self.flipped, flipped);
        let unified = self.unify(a, b);
        self.flipped = saved;
        unified
    }

    fn unify_inner(&mut self, a: &Ty<'db>, b: &Ty<'db>) -> Result<(), UnifyError<'db>> {
        let a = self.shallow_resolve(a);
        let b = self.shallow_resolve(b);
        // A flexible variable that receives a value takes it as a lower
        // bound; one that gives a value is settled first.
        if a != b && !matches!(a, Ty::Error | Ty::Never) && !matches!(b, Ty::Error | Ty::Never) {
            let (receives, gives) = if self.flipped { (&b, &a) } else { (&a, &b) };
            if let Ty::Var(v) = receives
                && self.is_flexible(*v)
            {
                self.vars[v.0 as usize].lowers.push(gives.clone());
                if let Some(&outer) = self.under.last() {
                    self.links.push((outer, *v, gives.clone()));
                }
                self.recorded = true;
                return Ok(());
            }
            if let Ty::Var(v) = gives
                && self.is_flexible(*v)
                && !self.binding
            {
                self.settle(*v);
                return self.unify_inner(&a, &b);
            }
        }
        match (&a, &b) {
            (Ty::Var(var), Ty::Error) | (Ty::Error, Ty::Var(var)) => {
                self.vars[var.0 as usize].met_error = true;
                Ok(())
            }
            // `!` and errors fit anywhere, without deciding a variable.
            (Ty::Never | Ty::Error, _) | (_, Ty::Never | Ty::Error) => Ok(()),
            // A place written `ro` takes a value of any mutability, which is
            // seen as `ro` (#320). Only the place that receives the value
            // does: inside the parameters of a function type, that's the
            // other side, and inside a `mut` value, where things are shared
            // both ways (`share_if_mut`), the value is a place too. A
            // variable is decided as usual.
            (Ty::Mutability(_) | Ty::Param(_), Ty::Mutability(_) | Ty::Param(_))
                if *(if self.flipped { &b } else { &a })
                    == Ty::Mutability(Mutability::ReadOnly) =>
            {
                Ok(())
            }
            (Ty::Var(x), Ty::Var(y)) if x == y => Ok(()),
            (Ty::Var(x), Ty::Var(y)) => {
                let (Some(kx), Some(ky)) = (self.var_kind(*x), self.var_kind(*y)) else {
                    unreachable!()
                };
                let kind = match (kx, ky) {
                    (VarKind::General, k) | (k, VarKind::General) => k,
                    (k1, k2) if k1 == k2 => k1,
                    _ => return Err(UnifyError::Mismatch(a.clone(), b.clone())),
                };
                self.vars[y.0 as usize].state = VarState::Unbound(kind);
                self.vars[x.0 as usize].state = VarState::Bound(b.clone());
                if let Some(name) = self.claims.remove(x) {
                    self.claims.entry(*y).or_insert(name);
                }
                if self.vars[x.0 as usize].met_error {
                    self.vars[y.0 as usize].met_error = true;
                }
                Ok(())
            }
            // A variable on the side that's expected receives the names of
            // the other; on the other side, it gets open names.
            (Ty::Var(var), ty) => self.bind_freshened(*var, ty, true),
            (ty, Ty::Var(var)) => self.bind_freshened(*var, ty, false),
            // A tuple with a pack spread in it: its fields are matched with
            // the other's, and the packs take what's left.
            (x, y)
                if (x.has_spread() || y.has_spread())
                    && matches!(x, Ty::Tuple(..) | Ty::Record(..)) =>
            {
                match (self.resolve(x).tuple_parts(), self.resolve(y).tuple_parts()) {
                    (Some(xp), Some(yp)) => self.unify_tuples(xp, yp, &a, &b),
                    _ => Err(UnifyError::Mismatch(a.clone(), b.clone())),
                }
            }
            (Ty::Spread(h1, x), Ty::Spread(h2, y)) if h1 == h2 => self.unify(x, y),
            // The own mutability first: whether the arguments can change
            // depends on it. They change as the struct's variance says when
            // what receives the value isn't `mut`, like the `T` of
            // `struct Box[T] { item: T }`, seen as `ro` in a `ro Box`, and
            // the `T` of `struct Handler[T] { f: fn(T) }`, the other way
            // round. In a `mut` one, or one whose mutability isn't known,
            // they're the same both ways, as in a `mut` array (#440).
            (Ty::Adt(x, xs, ms, m), Ty::Adt(y, ys, ns, n))
                if x == y && xs.len() == ys.len() && ms.len() == ns.len() =>
            {
                self.unify(m, n)?;
                let own = self.shallow_resolve(if self.flipped { n } else { m });
                self.under(m, n, |u| {
                    let args = xs.iter().zip(ys.iter()).chain(ms.iter().zip(ns.iter()));
                    for (i, (a, b)) in args.enumerate() {
                        u.unify_by(arg_variance(u.variances, *x, &own, i), a, b)?;
                    }
                    Ok(())
                })
            }
            // `F[y1..ym] = G[x1..xn]`: type constructors are injective, so
            // the last arguments are equal, and so are the rest of each
            // side. With more arguments on one side, its head takes the
            // first ones: `?m[a] = Either[String, i32]` makes `?m` the
            // constructor `Either[String]` and `a` the type `i32`.
            (Ty::App(f, ys), Ty::Adt(id, xs, ms, m)) | (Ty::Adt(id, xs, ms, m), Ty::App(f, ys))
                if xs.len() >= ys.len() =>
            {
                let split = xs.len() - ys.len();
                self.unify(f, &Ty::Adt(*id, xs[..split].into(), ms.clone(), m.clone()))?;
                self.unify_all(&xs[split..], ys)
            }
            (Ty::App(f, xs), Ty::App(g, ys)) => {
                if xs.len() >= ys.len() {
                    let split = xs.len() - ys.len();
                    self.unify(&Ty::app((**f).clone(), xs[..split].to_vec()), g)?;
                    self.unify_all(&xs[split..], ys)
                } else {
                    let split = ys.len() - xs.len();
                    self.unify(f, &Ty::app((**g).clone(), ys[..split].to_vec()))?;
                    self.unify_all(xs, &ys[split..])
                }
            }
            // The mutability first, so that it's known even when an element
            // doesn't fit.
            (Ty::Tuple(xs, m), Ty::Tuple(ys, n)) if xs.len() == ys.len() => {
                self.unify(m, n)?;
                self.under(m, n, |u| u.unify_all(xs, ys))?;
                self.share_if_mut(m, xs, ys)
            }
            // The same names, which are in the same order in both.
            (Ty::Record(xs, xn, m), Ty::Record(ys, yn, n))
                if xs.len() == ys.len()
                    && xn.len() == yn.len()
                    && xn.iter().zip(yn.iter()).all(|((a, _), (b, _))| a == b) =>
            {
                self.unify(m, n)?;
                self.under(m, n, |u| {
                    u.unify_all(xs, ys)?;
                    for ((_, x), (_, y)) in xn.iter().zip(yn.iter()) {
                        u.unify(x, y)?;
                    }
                    Ok(())
                })?;
                let (xt, yt): (Vec<_>, Vec<_>) =
                    xn.iter().zip(yn.iter()).map(|((_, x), (_, y))| (x.clone(), y.clone())).unzip();
                self.share_if_mut(m, xs, ys)?;
                self.share_if_mut(m, &xt, &yt)
            }
            (Ty::Array(m, x), Ty::Array(n, y)) => {
                self.under(m, n, |u| u.unify(x, y))?;
                self.unify(m, n)?;
                self.share_if_mut(m, std::slice::from_ref(x), std::slice::from_ref(y))
            }
            (Ty::Mutability(x), Ty::Mutability(y)) if x == y => Ok(()),
            // That of the parameters and the arguments of a call, as tuples.
            (Ty::Unwritten, Ty::Unwritten) => Ok(()),
            // Positional parameters one by one. With packs or keyword
            // parameters, the parameters unify as tuples do: by position, by
            // name, and with the packs taking what's left. The names of the
            // positional ones that both have flow from the actual function
            // into the expected one, which a written name doesn't receive.
            (Ty::Fn(xs, x, m, e), Ty::Fn(ys, y, n, f))
                if !(xs.is_positional() && ys.is_positional())
                    || xs.positional.len() == ys.positional.len() =>
            {
                if !xs.names.is_empty() && !ys.names.is_empty() {
                    let given =
                        if self.flipped { (b.clone(), a.clone()) } else { (a.clone(), b.clone()) };
                    self.given_fns.push(given);
                }
                for (p, q) in xs.names.iter().zip(ys.names.iter()) {
                    let (lo, hi) = if self.flipped { (q, p) } else { (p, q) };
                    self.relate_names(lo, hi)
                        .map_err(|_| UnifyError::Mismatch(a.clone(), b.clone()))?;
                }
                self.reversed(|u| {
                    if xs.is_positional() && ys.is_positional() {
                        u.unify_all(&xs.positional, &ys.positional)
                    } else {
                        u.unify(&xs.as_tuple(), &ys.as_tuple())
                            .map_err(|_| UnifyError::Mismatch(a.clone(), b.clone()))
                    }
                })?;
                self.unify(x, y)?;
                self.unify(m, n)?;
                self.unify_contained(e, f)
            }
            // Rows are equal when they have the same effects, with the same
            // arguments, and are made of the same other rows. Rows are in
            // one order, so they can be compared one by one.
            (Ty::Row(xs, xr), Ty::Row(ys, yr))
                if xs.len() == ys.len()
                    && xs.iter().zip(ys).all(|(x, y)| x.effect == y.effect)
                    && xr == yr =>
            {
                xs.iter().zip(ys).try_for_each(|(x, y)| self.unify_all(&x.args, &y.args))
            }
            (Ty::Builtin(x), Ty::Builtin(y)) if x == y => Ok(()),
            (Ty::Host(x), Ty::Host(y)) if x == y => Ok(()),
            (Ty::Param(x), Ty::Param(y)) if x == y => Ok(()),
            (Ty::Bound(x), Ty::Bound(y)) if x == y => Ok(()),
            // `C.Item` only equals itself.
            (Ty::Assoc(x, xs), Ty::Assoc(y, ys)) if x == y && xs.len() == ys.len() => {
                self.unify_all(xs, ys)
            }
            // So does `imm T`, which the checker only keeps for types it can't
            // take apart, like type parameters.
            (Ty::Imm(x), Ty::Imm(y)) => self.unify(x, y),
            // `ro T` too, and a `T` fits where a `ro T` is received (#320).
            (Ty::Ro(x), Ty::Ro(y)) => self.unify(x, y),
            (Ty::Ro(x), _) if !self.flipped => self.unify(x, &b),
            (_, Ty::Ro(y)) if self.flipped => self.unify(&a, y),
            _ => Err(UnifyError::Mismatch(a.clone(), b.clone())),
        }
    }

    /// Unify two tuples with packs spread in them. Their positional fields
    /// are matched from the start and from the end, until a spread; what's
    /// left on one side is what a single pack variable on the other stands
    /// for. Their named fields are matched by name: a named pack variable
    /// on one side takes the fields only the other has, and when both have
    /// one, each takes the other's fields and a new variable for the rest.
    /// What can be matched in more than one way, like `(*A, *B)` with
    /// `(i32, Bool)`, is a mismatch.
    fn unify_tuples(
        &mut self,
        x: TupleParts<'db>,
        y: TupleParts<'db>,
        a: &Ty<'db>,
        b: &Ty<'db>,
    ) -> Result<(), UnifyError<'db>> {
        let mismatch = || UnifyError::Mismatch(a.clone(), b.clone());
        // The parameters and the arguments of a call aren't values, so they
        // have no mutability, which `()` has.
        if x.mutability != Ty::Unwritten && y.mutability != Ty::Unwritten {
            self.unify(&x.mutability, &y.mutability)?;
        }
        // Positional fields.
        let is_spread = |t: &Ty<'db>| matches!(t, Ty::Spread(..));
        let (mut xs, mut ys) = (&x.positional[..], &y.positional[..]);
        while let (Some(p), Some(q)) = (xs.first(), ys.first()) {
            if is_spread(p) || is_spread(q) {
                break;
            }
            self.unify(p, q)?;
            (xs, ys) = (&xs[1..], &ys[1..]);
        }
        while let (Some(p), Some(q)) = (xs.last(), ys.last()) {
            if is_spread(p) || is_spread(q) {
                break;
            }
            self.unify(p, q)?;
            (xs, ys) = (&xs[..xs.len() - 1], &ys[..ys.len() - 1]);
        }
        match (xs, ys) {
            ([], []) => {}
            ([Ty::Spread(_, p)], [Ty::Spread(_, q)]) => self.unify(p, q)?,
            ([Ty::Spread(_, p)], rest) | (rest, [Ty::Spread(_, p)])
                if matches!(self.shallow_resolve(p), Ty::Var(_)) =>
            {
                let pack = Ty::tuple(rest.into(), Ty::Mutability(Mutability::Imm));
                self.unify(p, &pack)?;
            }
            // The same layout, like `(*As, *Bs)` on both sides.
            (xs, ys)
                if xs.len() == ys.len()
                    && xs.iter().zip(ys).all(|(p, q)| is_spread(p) == is_spread(q)) =>
            {
                self.unify_all(xs, ys)?;
            }
            _ => return Err(mismatch()),
        }
        // Named fields, each name once.
        let twice = |named: &[(Name, Ty<'db>)]| named.windows(2).any(|w| w[0].0 == w[1].0);
        if twice(&x.named) || twice(&y.named) {
            return Err(mismatch());
        }
        let mut only_x = Vec::new();
        for (name, t) in &x.named {
            match y.named.iter().find(|(n, _)| n == name) {
                Some((_, u)) => self.unify(t, u)?,
                None => only_x.push((name.clone(), t.clone())),
            }
        }
        let only_y: Vec<_> =
            y.named.iter().filter(|(n, _)| !x.named.iter().any(|(m, _)| m == n)).cloned().collect();
        let fields = |fields: Vec<(Name, Ty<'db>)>, rests: &[Ty<'db>]| {
            let rests = rests.iter().map(|r| Ty::Spread(PackHalf::Named, Box::new(r.clone())));
            Ty::record(rests.collect(), fields, Ty::Mutability(Mutability::Imm))
        };
        let is_var = |s: &Self, t: &Ty<'db>| matches!(s.shallow_resolve(t), Ty::Var(_));
        match (&x.rests[..], &y.rests[..]) {
            ([], []) if only_x.is_empty() && only_y.is_empty() => Ok(()),
            ([r], []) if is_var(self, r) && only_x.is_empty() => {
                let pack = fields(only_y, &[]);
                self.unify(r, &pack)
            }
            ([], [r]) if is_var(self, r) && only_y.is_empty() => {
                let pack = fields(only_x, &[]);
                self.unify(r, &pack)
            }
            ([r], [s]) if self.shallow_resolve(r) == self.shallow_resolve(s) => {
                if only_x.is_empty() && only_y.is_empty() { Ok(()) } else { Err(mismatch()) }
            }
            ([r], [s]) if is_var(self, r) && is_var(self, s) => {
                let rest = self.new_var(VarKind::NamedPack);
                let to_r = fields(only_y, std::slice::from_ref(&rest));
                let to_s = fields(only_x, std::slice::from_ref(&rest));
                self.unify(r, &to_r)?;
                self.unify(s, &to_s)
            }
            ([r], [s]) if is_var(self, r) && only_x.is_empty() => {
                let pack = fields(only_y, std::slice::from_ref(s));
                self.unify(r, &pack)
            }
            ([r], [s]) if is_var(self, s) && only_y.is_empty() => {
                let pack = fields(only_x, std::slice::from_ref(r));
                self.unify(s, &pack)
            }
            _ => Err(mismatch()),
        }
    }

    /// That a function with the name slot `hi` flows into `lo`. A written
    /// name renames what it's given, so only a variable records it. A
    /// source that comes after a call by name must have that name.
    fn relate_names(&mut self, lo: &Ty<'db>, hi: &Ty<'db>) -> Result<(), ()> {
        let (lo, hi) = (self.shallow_resolve(lo), self.shallow_resolve(hi));
        let Ty::Var(v) = lo else { return Ok(()) };
        if lo == hi {
            return Ok(());
        }
        self.names.push(NameEdge { lo, hi: hi.clone() });
        if let Some(name) = self.claims.get(&v).cloned() {
            if !self.name_set(&hi, &mut Vec::new()).contains(&name) {
                return Err(());
            }
            self.claim(&hi, &name);
        }
        Ok(())
    }

    /// The names the slot `ty` can be called by: those all its sources
    /// have, or the one a call claimed.
    fn name_set(&self, ty: &Ty<'db>, seen: &mut Vec<TyVar>) -> NameSet {
        match self.shallow_resolve(ty) {
            Ty::ParamName(name) => NameSet::of(&name),
            Ty::Var(v) => {
                if let Some(name) = self.claims.get(&v) {
                    return NameSet::One(name.clone());
                }
                if seen.contains(&v) {
                    return NameSet::Any;
                }
                seen.push(v);
                let mut set = NameSet::Any;
                for edge in &self.names {
                    if self.shallow_resolve(&edge.lo) == Ty::Var(v) {
                        set = set.meet(&self.name_set(&edge.hi, seen));
                    }
                }
                set
            }
            _ => NameSet::Any,
        }
    }

    /// Claim `name` for the slot `ty` and for the open slots that flowed
    /// into it, so that what's given there can be called by it.
    fn claim(&mut self, ty: &Ty<'db>, name: &Name) {
        let Ty::Var(v) = self.shallow_resolve(ty) else { return };
        if self.claims.contains_key(&v) {
            return;
        }
        self.claims.insert(v, name.clone());
        let sources: Vec<Ty<'db>> = self
            .names
            .iter()
            .filter(|e| self.shallow_resolve(&e.lo) == Ty::Var(v))
            .map(|e| e.hi.clone())
            .collect();
        for source in sources {
            self.claim(&source, name);
        }
    }

    /// Bind `var` to `ty`, with new variables for the names of the
    /// parameters in it. When `var` is on the side that's expected, the
    /// names of `ty` flow into them, so a variable that two functions meet
    /// in has the names they share, like that of `if c { f } else { g }`.
    /// Otherwise they're open, for calls by name to claim.
    fn bind_freshened(
        &mut self,
        var: TyVar,
        ty: &Ty<'db>,
        expected: bool,
    ) -> Result<(), UnifyError<'db>> {
        let viewed;
        let ty = if self.vars[var.0 as usize].seen_as_ro && expected != self.flipped {
            viewed = self.view_if_it_may_change(ty);
            &viewed
        } else {
            ty
        };
        let has_names = |t: &Ty<'db>| matches!(t, Ty::Fn(p, ..) if !p.names.is_empty());
        if self.var_kind(var) != Some(VarKind::General) || !ty.any(&mut |t| has_names(t)) {
            return self.bind(var, ty);
        }
        let low = expected != self.flipped;
        let original = self.resolve(ty);
        let fresh = self.freshen(&original, low);
        self.bind(var, &fresh)?;
        // The copy and the original are one value, so what's in a `mut`
        // array, struct or tuple of it shares its names (#436): unifying
        // them again lets `share_if_mut` relate those the other way too.
        let has_mut = |t: &Ty<'db>| match t {
            Ty::Array(m, _) | Ty::Tuple(_, m) | Ty::Record(_, _, m) | Ty::Adt(_, _, _, m) => {
                **m == Ty::Mutability(Mutability::Mut)
            }
            _ => false,
        };
        if original.any(&mut |t| has_mut(t)) {
            match expected {
                true => self.unify(&fresh, &original)?,
                false => self.unify(&original, &fresh)?,
            }
        }
        Ok(())
    }

    /// `ty` with a new variable for each name of a parameter, which that
    /// name flows into when `low`, and that flows into the name otherwise.
    /// Inside the parameters of a function type, the other way around.
    fn freshen(&mut self, ty: &Ty<'db>, low: bool) -> Ty<'db> {
        ty.fold(&mut |t| {
            let Ty::Fn(params, ret, m, e) = t else { return None };
            let names = params
                .names
                .iter()
                .map(|name| {
                    let fresh = self.new_var(VarKind::Name);
                    let (lo, hi) = if low { (&fresh, name) } else { (name, &fresh) };
                    let _ = self.relate_names(lo, hi);
                    fresh
                })
                .collect();
            let params = params.map(|p| self.freshen(p, !low)).with_names(names);
            let fresh = Ty::Fn(
                Box::new(params),
                Box::new(self.freshen(ret, low)),
                m.clone(),
                Box::new(self.freshen(e, low)),
            );
            let given = if low { (fresh.clone(), t.clone()) } else { (t.clone(), fresh.clone()) };
            self.given_fns.push(given);
            Some(fresh)
        })
    }

    /// The names that `names` would have if they were settled now.
    pub fn names_now(&self, names: &[Ty<'db>]) -> Vec<Option<Name>> {
        self.slots_now(names)
            .into_iter()
            .map(|s| match s {
                SlotNow::Named(name) => Some(name),
                _ => None,
            })
            .collect()
    }

    /// What a call by name finds in each of `names` now.
    pub fn slots_now(&self, names: &[Ty<'db>]) -> Vec<SlotNow> {
        names
            .iter()
            .map(|n| match self.name_set(n, &mut Vec::new()) {
                NameSet::One(name) => SlotNow::Named(name),
                NameSet::Any => SlotNow::Open,
                NameSet::Empty => SlotNow::Unnamed,
            })
            .collect()
    }

    /// That the name `ty` is `name`, as a call by that name needs.
    pub fn require_name(&mut self, ty: &Ty<'db>, name: &Name) {
        self.claim(ty, name);
    }

    /// Give each name variable its name: the one a call claimed, or the
    /// one all its sources have, and otherwise none. A source that doesn't
    /// fit a claim is reported where it's unified.
    pub fn settle_names(&mut self) {
        let mut values = Vec::new();
        for i in 0..self.vars.len() {
            if matches!(self.vars[i].state, VarState::Unbound(VarKind::Name)) {
                let var = TyVar(i as u32);
                let name = match self.claims.get(&var) {
                    Some(name) => Some(name.clone()),
                    None => match self.name_set(&Ty::Var(var), &mut Vec::new()) {
                        NameSet::One(name) => Some(name),
                        _ => None,
                    },
                };
                values.push((var, name));
            }
        }
        for (var, name) in values {
            self.vars[var.0 as usize].state = VarState::Bound(Ty::ParamName(name));
        }
        self.names.clear();
        self.claims.clear();
    }

    fn unify_all(&mut self, xs: &[Ty<'db>], ys: &[Ty<'db>]) -> Result<(), UnifyError<'db>> {
        xs.iter().zip(ys).try_for_each(|(x, y)| self.unify(x, y))
    }

    fn bind(&mut self, var: TyVar, ty: &Ty<'db>) -> Result<(), UnifyError<'db>> {
        let kind = self.var_kind(var).unwrap();
        // A number is its own `ro` view, so a literal fits a `ro T` as a `T`.
        if let (VarKind::Int | VarKind::Float, Ty::Ro(inner)) = (kind, ty) {
            return self.unify(&Ty::Var(var), inner);
        }
        let fits = match kind {
            VarKind::General => true,
            VarKind::Int => matches!(ty, Ty::Builtin(BuiltinType::Int(_) | BuiltinType::Uint(_))),
            VarKind::Float => matches!(ty, Ty::Builtin(BuiltinType::Float(_))),
            // A parameter or bound variable in a mutability's place is a
            // mutability too: the checker only puts them there.
            VarKind::Mutability => matches!(ty, Ty::Mutability(_) | Ty::Param(_) | Ty::Bound(_)),
            // The same for a row, which can also be the error type.
            VarKind::Effect => matches!(ty, Ty::Row(..) | Ty::Param(_) | Ty::Bound(_) | Ty::Error),
            // A pack is the tuple of what it stands for.
            VarKind::PositionalPack | VarKind::NamedPack => matches!(
                ty,
                Ty::Tuple(..) | Ty::Record(..) | Ty::Param(_) | Ty::Bound(_) | Ty::Error
            ),
            // Names are related, never unified.
            VarKind::Name => false,
        };
        if !fits {
            return Err(UnifyError::Mismatch(Ty::Var(var), ty.clone()));
        }
        let ty = self.resolve(ty);
        if ty.any(&mut |t| *t == Ty::Var(var)) {
            return Err(UnifyError::Occurs(var, ty));
        }
        self.vars[var.0 as usize].state = VarState::Bound(ty);
        Ok(())
    }
}

/// The variables in `ty` where a mutability goes, like the `?m` of
/// `?m [i32]`.
fn mutability_vars(ty: &Ty<'_>, out: &mut Vec<TyVar>) {
    let mut push = |m: &Ty<'_>| {
        if let Ty::Var(v) = m {
            out.push(*v);
        }
    };
    match ty {
        Ty::Array(m, _) | Ty::Tuple(_, m) | Ty::Record(_, _, m) | Ty::Fn(_, _, m, _) => push(m),
        Ty::Adt(_, _, ms, m) => {
            push(m);
            ms.iter().for_each(&mut push);
        }
        _ => {}
    }
    ty.walk_children(|t| mutability_vars(t, out));
}

/// Whether something in a value of type `ty` may change: something `mut`,
/// or a type parameter or what isn't known yet, which may be `mut`, unless
/// it's a number.
fn may_change(u: &Unifier<'_>, ty: &Ty<'_>) -> bool {
    match ty {
        // A closure never changes once it's made, and `ro T` and `imm T`
        // aren't changed through.
        Ty::Fn(..) | Ty::Ro(_) | Ty::Imm(_) => false,
        Ty::Mutability(Mutability::Mut) => true,
        Ty::Var(v) => !matches!(u.var_kind(*v), Some(VarKind::Int | VarKind::Float)),
        Ty::Param(_) | Ty::Bound(_) | Ty::App(..) | Ty::Assoc(..) => true,
        ty => {
            let mut found = false;
            ty.walk_children(|t| found = found || may_change(u, t));
            found
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ty::Params;

    fn i32() -> Ty<'static> {
        Ty::Builtin(BuiltinType::Int(BuiltinInt::I32))
    }

    fn fun(params: Vec<Ty<'static>>, ret: Ty<'static>) -> Ty<'static> {
        Ty::func(params, ret)
    }

    #[test]
    fn binds_variables() {
        let mut u = Unifier::new();
        let a = u.new_var(VarKind::General);
        let b = u.new_var(VarKind::General);
        u.unify(&fun(vec![a.clone()], b.clone()), &fun(vec![i32()], a.clone())).unwrap();
        assert_eq!(u.resolve(&b), i32());
    }

    #[test]
    fn reports_the_innermost_mismatch() {
        let mut u = Unifier::new();
        let err = u.unify(
            &Ty::tuple(Box::new([i32(), Ty::bool()]), Ty::Mutability(Mutability::Imm)),
            &Ty::tuple(Box::new([i32(), Ty::unit()]), Ty::Mutability(Mutability::Imm)),
        );
        assert_eq!(err, Err(UnifyError::Mismatch(Ty::bool(), Ty::unit())));
        // Different arities differ as a whole.
        let err = u.unify(&fun(vec![], i32()), &fun(vec![i32()], i32()));
        assert!(matches!(err, Err(UnifyError::Mismatch(Ty::Fn(..), Ty::Fn(..)))));
    }

    /// `fn(x: i32) -> i32`, with `None` for no name.
    fn named(name: Option<&str>) -> Ty<'static> {
        let params =
            Params::positional([i32()]).with_names(vec![Ty::ParamName(name.map(Name::new))]);
        Ty::func_with(params, i32(), Ty::nothing())
    }

    /// The names of the parameters of `ty`, a function type.
    fn names_of<'db>(u: &Unifier<'db>, ty: &Ty<'db>) -> Box<[Ty<'db>]> {
        let Ty::Fn(params, ..) = u.resolve(ty) else { unreachable!() };
        params.names
    }

    #[test]
    fn names_never_make_unification_fail() {
        let mut u = Unifier::new();
        assert!(u.unify(&named(None), &named(Some("x"))).is_ok());
        assert!(u.unify(&named(Some("x")), &named(None)).is_ok());
        assert!(u.unify(&named(Some("x")), &named(Some("y"))).is_ok());
        let takes = |t| Ty::func(vec![t], i32());
        assert!(u.unify(&takes(named(None)), &takes(named(Some("x")))).is_ok());
    }

    #[test]
    fn a_variable_has_the_names_that_meet_in_it() {
        let mut u = Unifier::new();
        let a = u.new_var(VarKind::General);
        u.unify(&a, &named(Some("x"))).unwrap();
        let b = u.new_var(VarKind::General);
        u.unify(&b, &named(Some("x"))).unwrap();
        u.unify(&b, &named(Some("y"))).unwrap();
        u.settle_names();
        assert_eq!(u.resolve(&a), named(Some("x")));
        assert_eq!(u.resolve(&b), named(None));
    }

    #[test]
    fn a_written_name_renames_what_it_is_given() {
        let mut u = Unifier::new();
        let a = u.new_var(VarKind::General);
        u.unify(&a, &named(Some("x"))).unwrap();
        // Like `apply(a)` for `apply(k: fn(item: i32) -> i32)`.
        u.unify(&named(Some("item")), &a).unwrap();
        u.settle_names();
        assert_eq!(u.resolve(&a), named(Some("x")));
    }

    #[test]
    fn a_call_claims_an_open_slot_and_what_flowed_into_it() {
        let mut u = Unifier::new();
        // `f` is given where a written type is expected, so its slot is open.
        let f = u.new_var(VarKind::General);
        u.unify(&named(Some("item")), &f).unwrap();
        assert_eq!(u.slots_now(&names_of(&u, &f)), vec![SlotNow::Open]);
        // `let k = f; k(x: 1)`.
        let k = u.new_var(VarKind::General);
        u.unify(&k, &f).unwrap();
        u.require_name(&names_of(&u, &k)[0], &Name::new("x"));
        assert_eq!(u.names_now(&names_of(&u, &f)), vec![Some(Name::new("x"))]);
        u.settle_names();
        assert_eq!(u.resolve(&f), named(Some("x")));
        assert_eq!(u.resolve(&k), named(Some("x")));
    }

    #[test]
    fn a_function_given_after_a_call_needs_its_name() {
        let mut u = Unifier::new();
        let a = u.new_var(VarKind::General);
        u.unify(&a, &named(Some("x"))).unwrap();
        u.require_name(&names_of(&u, &a)[0], &Name::new("x"));
        assert!(u.unify(&a, &named(Some("y"))).is_err());
        assert!(u.unify(&a, &named(Some("x"))).is_ok());
    }

    #[test]
    fn occurs_check() {
        let mut u = Unifier::new();
        let a = u.new_var(VarKind::General);
        let err = u.unify(&a, &Ty::array(Ty::Mutability(Mutability::Imm), a.clone()));
        assert!(matches!(err, Err(UnifyError::Occurs(..))));
    }

    #[test]
    fn numeric_variables() {
        let mut u = Unifier::new();
        let int = u.new_var(VarKind::Int);
        let general = u.new_var(VarKind::General);
        // A general variable joined with an int variable becomes one.
        u.unify(&general, &int).unwrap();
        assert!(u.unify(&general, &Ty::bool()).is_err());
        let float = u.new_var(VarKind::Float);
        assert!(u.unify(&int, &float).is_err());
        let other = u.new_var(VarKind::Int);
        u.default_numeric_vars();
        assert_eq!(u.resolve(&general), i32());
        assert_eq!(u.resolve(&other), i32());
        assert_eq!(u.resolve(&float), Ty::Builtin(BuiltinType::Float(BuiltinFloat::F64)));
    }

    #[test]
    fn never_and_error_fit_anywhere() {
        let mut u = Unifier::new();
        let a = u.new_var(VarKind::General);
        u.unify(&a, &Ty::Never).unwrap();
        u.unify(&a, &i32()).unwrap();
        u.unify(&Ty::Error, &Ty::bool()).unwrap();
        assert_eq!(u.resolve(&a), i32());
    }

    /// Unify types with the enums of a small file, `Either[E, A]` and
    /// `Option[T]`.
    fn with_enums(f: impl for<'db> FnOnce(&mut Unifier<'db>, Ty<'db>, Ty<'db>)) {
        use boltlang_hir_def::{
            ids::EnumId,
            item_tree::{ModItem, item_tree},
        };
        let db = boltlang_base_db::TestDb::new();
        let file =
            db.file("/main.bolt", "enum Either[E, A] { L(E), R(A) }\nenum Option[T] { N, S(T) }");
        let enums: Vec<_> = item_tree(&db, file)
            .top_level
            .iter()
            .filter_map(|item| match *item {
                ModItem::Enum(idx) => Some(Ty::adt(
                    crate::ty::AdtId::Enum(EnumId::new(&db, file, idx)),
                    [],
                    [],
                    Ty::Error,
                )),
                _ => None,
            })
            .collect();
        f(&mut Unifier::new(), enums[0].clone(), enums[1].clone());
    }

    #[test]
    fn applications_split_from_the_right() {
        with_enums(|u, either, option| {
            // `?m[?a] = Either[String, i32]` gives `?m = Either[String]`, `?a = i32`.
            let m = u.new_var(VarKind::General);
            let a = u.new_var(VarKind::General);
            let str_ty = Ty::Builtin(BuiltinType::Bool);
            let target = Ty::app(either.clone(), [str_ty.clone(), i32()]);
            u.unify(&Ty::app(m.clone(), [a.clone()]), &target).unwrap();
            assert_eq!(u.resolve(&m), Ty::app(either.clone(), [str_ty.clone()]));
            assert_eq!(u.resolve(&a), i32());
            // Applying the solved head gives the type back, in its normal form.
            assert_eq!(u.resolve(&Ty::app(m.clone(), [a.clone()])), target);

            // A constructor of the wrong kind doesn't fit: `Option[i32]` has
            // one argument, `?f[?x, ?y]` two.
            let f = u.new_var(VarKind::General);
            let x = u.new_var(VarKind::General);
            let y = u.new_var(VarKind::General);
            let err = u.unify(&Ty::app(f, [x, y]), &Ty::app(option.clone(), [i32()]));
            assert!(matches!(err, Err(UnifyError::Mismatch(..))));

            // Two applications of variables.
            let g = u.new_var(VarKind::General);
            let h = u.new_var(VarKind::General);
            let b = u.new_var(VarKind::General);
            u.unify(&Ty::app(g.clone(), [i32()]), &Ty::app(h.clone(), [b.clone()])).unwrap();
            assert_eq!(u.resolve(&b), i32());
            u.unify(&h, &option).unwrap();
            assert_eq!(u.resolve(&Ty::app(g, [i32()])), Ty::app(option, [i32()]));
        });
    }
}

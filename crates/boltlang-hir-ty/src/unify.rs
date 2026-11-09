//! The unifier: makes two types equal by binding inference variables. It
//! knows nothing about where types come from; the
//! [`Solver`](crate::solve::Solver) turns its errors into diagnostics.
//!
//! The variables are a union-find table: a variable is either unbound, or
//! bound to a type, which may be another variable.

use boltlang_hir_def::builtin_type::{BuiltinFloat, BuiltinInt, BuiltinType};

use boltlang_hir_def::Name;

use crate::ty::{Mutability, NumberKind, PackHalf, TupleParts, Ty, TyVar, VarKind};

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
}

#[derive(Debug, Default, Clone)]
pub struct Unifier<'db> {
    vars: Vec<Var<'db>>,
}

impl<'db> Unifier<'db> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn new_var(&mut self, kind: VarKind) -> Ty<'db> {
        let var = TyVar(self.vars.len() as u32);
        self.vars.push(Var { state: VarState::Unbound(kind), met_error: false, defaulted: None });
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
            _ => None,
        })
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

    pub fn unify(&mut self, a: &Ty<'db>, b: &Ty<'db>) -> Result<(), UnifyError<'db>> {
        let a = self.shallow_resolve(a);
        let b = self.shallow_resolve(b);
        match (&a, &b) {
            (Ty::Var(var), Ty::Error) | (Ty::Error, Ty::Var(var)) => {
                self.vars[var.0 as usize].met_error = true;
                Ok(())
            }
            // `!` and errors fit anywhere, without deciding a variable.
            (Ty::Never | Ty::Error, _) | (_, Ty::Never | Ty::Error) => Ok(()),
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
                if self.vars[x.0 as usize].met_error {
                    self.vars[y.0 as usize].met_error = true;
                }
                Ok(())
            }
            (Ty::Var(var), ty) | (ty, Ty::Var(var)) => self.bind(*var, ty),
            // A tuple with a pack spread in it: its fields are matched with
            // the other's, and the packs take what's left.
            (x, y) if (x.has_spread() || y.has_spread()) && x.tuple_parts().is_some() => {
                match (self.resolve(x).tuple_parts(), self.resolve(y).tuple_parts()) {
                    (Some(xp), Some(yp)) => self.unify_tuples(xp, yp, &a, &b),
                    _ => Err(UnifyError::Mismatch(a.clone(), b.clone())),
                }
            }
            (Ty::Spread(h1, x), Ty::Spread(h2, y)) if h1 == h2 => self.unify(x, y),
            (Ty::Adt(x, xs, ms, m), Ty::Adt(y, ys, ns, n)) if x == y && xs.len() == ys.len() => {
                self.unify_all(xs, ys)?;
                self.unify_all(ms, ns)?;
                self.unify(m, n)
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
                self.unify_all(xs, ys)
            }
            // The same names, which are in the same order in both.
            (Ty::Record(xs, xn, m), Ty::Record(ys, yn, n))
                if xs.len() == ys.len()
                    && xn.len() == yn.len()
                    && xn.iter().zip(yn.iter()).all(|((a, _), (b, _))| a == b) =>
            {
                self.unify(m, n)?;
                self.unify_all(xs, ys)?;
                for ((_, x), (_, y)) in xn.iter().zip(yn.iter()) {
                    self.unify(x, y)?;
                }
                Ok(())
            }
            (Ty::Array(m, x), Ty::Array(n, y)) => {
                self.unify(x, y)?;
                self.unify(m, n)
            }
            (Ty::Mutability(x), Ty::Mutability(y)) if x == y => Ok(()),
            // That of the parameters and the arguments of a call, as tuples.
            (Ty::Unwritten, Ty::Unwritten) => Ok(()),
            (Ty::Fn(xs, x, m, e), Ty::Fn(ys, y, n, f))
                if xs.is_positional()
                    && ys.is_positional()
                    && xs.positional.len() == ys.positional.len() =>
            {
                self.unify_all(&xs.positional, &ys.positional)?;
                self.unify(x, y)?;
                self.unify(m, n)?;
                self.unify(e, f)
            }
            // With packs or keyword parameters, the parameters unify as
            // tuples do: by position, by name, and with the packs taking
            // what's left.
            (Ty::Fn(xs, x, m, e), Ty::Fn(ys, y, n, f))
                if !xs.is_positional() || !ys.is_positional() =>
            {
                self.unify(&xs.as_tuple(), &ys.as_tuple())
                    .map_err(|_| UnifyError::Mismatch(a.clone(), b.clone()))?;
                self.unify(x, y)?;
                self.unify(m, n)?;
                self.unify(e, f)
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
        let fields = |solver: &Self, fields: Vec<(Name, Ty<'db>)>, rests: &[Ty<'db>]| {
            let _ = solver;
            let rests = rests.iter().map(|r| Ty::Spread(PackHalf::Named, Box::new(r.clone())));
            Ty::record(rests.collect(), fields, Ty::Mutability(Mutability::Imm))
        };
        let is_var = |s: &Self, t: &Ty<'db>| matches!(s.shallow_resolve(t), Ty::Var(_));
        match (&x.rests[..], &y.rests[..]) {
            ([], []) if only_x.is_empty() && only_y.is_empty() => Ok(()),
            ([r], []) if is_var(self, r) && only_x.is_empty() => {
                let pack = fields(self, only_y, &[]);
                self.unify(r, &pack)
            }
            ([], [r]) if is_var(self, r) && only_y.is_empty() => {
                let pack = fields(self, only_x, &[]);
                self.unify(r, &pack)
            }
            ([r], [s]) if self.shallow_resolve(r) == self.shallow_resolve(s) => {
                if only_x.is_empty() && only_y.is_empty() { Ok(()) } else { Err(mismatch()) }
            }
            ([r], [s]) if is_var(self, r) && is_var(self, s) => {
                let rest = self.new_var(VarKind::NamedPack);
                let to_r = fields(self, only_y, std::slice::from_ref(&rest));
                let to_s = fields(self, only_x, std::slice::from_ref(&rest));
                self.unify(r, &to_r)?;
                self.unify(s, &to_s)
            }
            ([r], [s]) if is_var(self, r) && only_x.is_empty() => {
                let pack = fields(self, only_y, std::slice::from_ref(s));
                self.unify(r, &pack)
            }
            ([r], [s]) if is_var(self, s) && only_y.is_empty() => {
                let pack = fields(self, only_x, std::slice::from_ref(r));
                self.unify(s, &pack)
            }
            _ => Err(mismatch()),
        }
    }

    fn unify_all(&mut self, xs: &[Ty<'db>], ys: &[Ty<'db>]) -> Result<(), UnifyError<'db>> {
        xs.iter().zip(ys).try_for_each(|(x, y)| self.unify(x, y))
    }

    fn bind(&mut self, var: TyVar, ty: &Ty<'db>) -> Result<(), UnifyError<'db>> {
        let kind = self.var_kind(var).unwrap();
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

#[cfg(test)]
mod tests {
    use super::*;

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

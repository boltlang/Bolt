//! The unifier: makes two types equal by binding inference variables. It
//! knows nothing about where types come from; the
//! [`Solver`](crate::solve::Solver) turns its errors into diagnostics.
//!
//! The variables are a union-find table: a variable is either unbound, or
//! bound to a type, which may be another variable.

use boltlang_hir_def::builtin_type::BuiltinType;

use crate::ty::{Ty, TyVar, VarKind};

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

#[derive(Debug, Default, Clone)]
pub struct Unifier<'db> {
    vars: Vec<VarState<'db>>,
    /// Whether each variable met the error type, so that it becomes the
    /// error type if nothing else decides it: see [`Unifier::error_unknowns`].
    met_error: Vec<bool>,
}

impl<'db> Unifier<'db> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn new_var(&mut self, kind: VarKind) -> Ty<'db> {
        let var = TyVar(self.vars.len() as u32);
        self.vars.push(VarState::Unbound(kind));
        self.met_error.push(false);
        Ty::Var(var)
    }

    /// The kind of a variable, if it's still unbound.
    pub fn var_kind(&self, var: TyVar) -> Option<VarKind> {
        match self.vars[var.0 as usize] {
            VarState::Unbound(kind) => Some(kind),
            VarState::Bound(_) => None,
        }
    }

    /// Follow bound variables until reaching a type that isn't one.
    pub fn shallow_resolve(&self, ty: &Ty<'db>) -> Ty<'db> {
        let mut ty = ty.clone();
        while let Ty::Var(var) = ty {
            match &self.vars[var.0 as usize] {
                VarState::Bound(bound) => ty = bound.clone(),
                VarState::Unbound(_) => break,
            }
        }
        ty
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
        for state in &mut self.vars {
            match state {
                VarState::Unbound(VarKind::Int) => *state = VarState::Bound(Ty::Builtin(BuiltinType::Int(
                    boltlang_hir_def::builtin_type::BuiltinInt::I32,
                ))),
                VarState::Unbound(VarKind::Float) => *state = VarState::Bound(Ty::Builtin(BuiltinType::Float(
                    boltlang_hir_def::builtin_type::BuiltinFloat::F64,
                ))),
                _ => {}
            }
        }
    }

    /// Make the variables that met the error type and are still unknown the
    /// error type, once inference is done. Otherwise a type that's only
    /// unknown because of an error would become a type parameter, like the
    /// `a` of `forall a. fn() -> a` for `fn h() = 1..2`. A variable isn't
    /// bound when it meets the error, so that what comes after can still
    /// decide it: the second arm of `match s { Nope(x) => x, _ => 0.0 }`
    /// makes it a float.
    pub fn error_unknowns(&mut self) {
        for (state, met_error) in self.vars.iter_mut().zip(&self.met_error) {
            if *met_error && matches!(state, VarState::Unbound(_)) {
                *state = VarState::Bound(Ty::Error);
            }
        }
    }

    /// Remember that `ty` met an error if it's a variable, like the parts of
    /// a pattern that can't match, so that [`Unifier::error_unknowns`] makes
    /// it the error type if nothing else decides it.
    pub fn met_error(&mut self, ty: &Ty<'db>) {
        if let Ty::Var(var) = self.shallow_resolve(ty) {
            self.met_error[var.0 as usize] = true;
        }
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
                self.met_error[var.0 as usize] = true;
                Ok(())
            }
            // `!` and errors fit anywhere, without deciding a variable.
            (Ty::Never | Ty::Error, _) | (_, Ty::Never | Ty::Error) => Ok(()),
            (Ty::Var(x), Ty::Var(y)) if x == y => Ok(()),
            (Ty::Var(x), Ty::Var(y)) => {
                let (Some(kx), Some(ky)) = (self.var_kind(*x), self.var_kind(*y)) else { unreachable!() };
                let kind = match (kx, ky) {
                    (VarKind::General, k) | (k, VarKind::General) => k,
                    (k1, k2) if k1 == k2 => k1,
                    _ => return Err(UnifyError::Mismatch(a.clone(), b.clone())),
                };
                self.vars[y.0 as usize] = VarState::Unbound(kind);
                self.vars[x.0 as usize] = VarState::Bound(b.clone());
                if self.met_error[x.0 as usize] {
                    self.met_error[y.0 as usize] = true;
                }
                Ok(())
            }
            (Ty::Var(var), ty) | (ty, Ty::Var(var)) => self.bind(*var, ty),
            (Ty::Adt(x, xs), Ty::Adt(y, ys)) if x == y && xs.len() == ys.len() => self.unify_all(xs, ys),
            (Ty::Tuple(xs), Ty::Tuple(ys)) if xs.len() == ys.len() => self.unify_all(xs, ys),
            (Ty::Array(x), Ty::Array(y)) => self.unify(x, y),
            (Ty::Fn(xs, x), Ty::Fn(ys, y)) if xs.len() == ys.len() => {
                self.unify_all(xs, ys)?;
                self.unify(x, y)
            }
            (Ty::Builtin(x), Ty::Builtin(y)) if x == y => Ok(()),
            (Ty::Param(x), Ty::Param(y)) if x == y => Ok(()),
            (Ty::Bound(x), Ty::Bound(y)) if x == y => Ok(()),
            _ => Err(UnifyError::Mismatch(a.clone(), b.clone())),
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
        };
        if !fits {
            return Err(UnifyError::Mismatch(Ty::Var(var), ty.clone()));
        }
        let ty = self.resolve(ty);
        if ty.any(&mut |t| *t == Ty::Var(var)) {
            return Err(UnifyError::Occurs(var, ty));
        }
        self.vars[var.0 as usize] = VarState::Bound(ty);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use boltlang_hir_def::builtin_type::{BuiltinInt, BuiltinType};

    use super::*;

    fn i32() -> Ty<'static> {
        Ty::Builtin(BuiltinType::Int(BuiltinInt::I32))
    }

    fn fun(params: Vec<Ty<'static>>, ret: Ty<'static>) -> Ty<'static> {
        Ty::Fn(params.into(), Box::new(ret))
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
        let err = u.unify(&Ty::Tuple(Box::new([i32(), Ty::bool()])), &Ty::Tuple(Box::new([i32(), Ty::unit()])));
        assert_eq!(err, Err(UnifyError::Mismatch(Ty::bool(), Ty::unit())));
        // Different arities differ as a whole.
        let err = u.unify(&fun(vec![], i32()), &fun(vec![i32()], i32()));
        assert!(matches!(err, Err(UnifyError::Mismatch(Ty::Fn(..), Ty::Fn(..)))));
    }

    #[test]
    fn occurs_check() {
        let mut u = Unifier::new();
        let a = u.new_var(VarKind::General);
        let err = u.unify(&a, &Ty::Array(Box::new(a.clone())));
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
        assert_eq!(u.resolve(&float), Ty::Builtin(BuiltinType::Float(boltlang_hir_def::builtin_type::BuiltinFloat::F64)));
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
}

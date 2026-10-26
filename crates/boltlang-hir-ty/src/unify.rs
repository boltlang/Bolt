//! The unifier: makes two types equal by binding inference variables. It
//! knows nothing about where types come from; the
//! [`Solver`](crate::solve::Solver) turns its errors into diagnostics.
//!
//! The variables are a union-find table: a variable is either unbound, or
//! bound to a type, which may be another variable.

use boltlang_hir_def::builtin_type::{BuiltinFloat, BuiltinInt, BuiltinType};

use crate::ty::{NumberKind, Ty, TyVar, VarKind};

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
                VarState::Unbound(VarKind::Int) => (BuiltinType::Int(BuiltinInt::I32), NumberKind::Int),
                VarState::Unbound(VarKind::Float) => (BuiltinType::Float(BuiltinFloat::F64), NumberKind::Float),
                _ => continue,
            };
            var.state = VarState::Bound(Ty::Builtin(ty));
            var.defaulted = Some(kind);
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
                let (Some(kx), Some(ky)) = (self.var_kind(*x), self.var_kind(*y)) else { unreachable!() };
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
            (Ty::Adt(x, xs), Ty::Adt(y, ys)) if x == y && xs.len() == ys.len() => self.unify_all(xs, ys),
            // `F[y1..ym] = G[x1..xn]`: type constructors are injective, so
            // the last arguments are equal, and so are the rest of each
            // side. With more arguments on one side, its head takes the
            // first ones: `?m[a] = Either[String, i32]` makes `?m` the
            // constructor `Either[String]` and `a` the type `i32`.
            (Ty::App(f, ys), Ty::Adt(id, xs)) | (Ty::Adt(id, xs), Ty::App(f, ys)) if xs.len() >= ys.len() => {
                let split = xs.len() - ys.len();
                self.unify(f, &Ty::Adt(*id, xs[..split].into()))?;
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
        use boltlang_hir_def::{ids::EnumId, item_tree::{ModItem, item_tree}};
        let db = boltlang_base_db::TestDb::new();
        let file = db.file("/main.bolt", "enum Either[E, A] { L(E), R(A) }\nenum Option[T] { N, S(T) }");
        let enums: Vec<_> = item_tree(&db, file)
            .top_level
            .iter()
            .filter_map(|item| match *item {
                ModItem::Enum(idx) => Some(Ty::Adt(crate::ty::AdtId::Enum(EnumId::new(&db, file, idx)), Box::new([]))),
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
            let str_ty = Ty::Builtin(BuiltinType::String);
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

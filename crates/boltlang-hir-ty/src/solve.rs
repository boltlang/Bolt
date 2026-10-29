//! The solver: takes the constraints inference generates and solves them,
//! using the [`Unifier`] for equalities. Class constraints wait until their
//! types are known well enough to pick an impl.

use std::collections::HashMap;

use boltlang_base_db::{Db, File};
use boltlang_hir_def::{
    Name,
    body::DefWithBodyId,
    hir::{ExprId, PatId, TypeRefId},
    ids::{ClassId, FunctionId, ImplId, ItemContainerId, TypeAliasId},
    nameres::{file_scope, files_used_by},
    resolver::GenericDefId,
};

use crate::{
    holds::struct_atoms_of,
    lower::{
        ClassConstraint, VariantDef, field_types, generic_predicates, impl_assoc_ty, impl_header, struct_args,
        implied_by, subst_params, with_superclasses,
    },
    ty::{AdtId, GenericParamId, Mutability, Ty, TyVar, VarKind},
    unify::{Unifier, UnifyError},
};

/// Where a constraint comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub struct Origin<'db> {
    pub body: DefWithBodyId<'db>,
    /// What must have the expected type. Errors are reported here.
    pub node: ExprOrPat,
    /// Why the expected type is expected, so that errors can point at it.
    pub reason: Reason<'db>,
}

impl<'db> Origin<'db> {
    pub fn new(body: DefWithBodyId<'db>, node: ExprOrPat) -> Self {
        Origin { body, node, reason: Reason::None }
    }
}

/// Where a constraint in the type of a function comes from: the use in its
/// body that needs it, like the `p.x` of `fn get_x(p) = p.x`, or the bound or
/// `where` clause that declares it, like the `Show` of `[T: Show]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum ConstraintSource<'db> {
    Body { body: DefWithBodyId<'db>, node: ExprOrPat },
    Declared { def: GenericDefId<'db>, type_ref: TypeRefId },
}

/// Why a type is expected of an expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum Reason<'db> {
    /// Nothing worth pointing at, like the `Bool` of a condition.
    None,
    /// A type annotation in the body, like that of `let x: i32`.
    Annotation(TypeRefId),
    /// The declared return type of the function the body is of.
    ReturnType,
    /// The declared type of the const the body is of.
    ConstType,
    /// The `index`th parameter of the function being called, counting the
    /// receiver of a method call like `x.f(y)`.
    Argument { function: FunctionId<'db>, index: u32 },
    /// The `index`th field of a struct or variant.
    Field { variant: VariantDef<'db>, index: u32 },
    /// The type of another expression, like the other branch of an `if`.
    SameAs(ExprId),
    /// A constraint of the type of the function used, where the function
    /// needs it.
    RequiredBy(ConstraintSource<'db>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum ExprOrPat {
    Expr(ExprId),
    Pat(PatId),
}

/// That `ty` has a field `name` of type `field`, like `x` in `p.x`. It's
/// solved once `ty` is known, and a function whose argument's type isn't
/// known keeps it as part of its type, like `forall a. a.x: i32 => fn(a)
/// -> i32`, so that it works on any struct with that field. With `assign`,
/// the field is assigned to, like in `p.x = 1`, so a struct must be `mut`:
/// `forall a. mut a.x: i32 => fn(a) -> ()`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub struct FieldConstraint<'db> {
    pub ty: Ty<'db>,
    pub name: Name,
    pub field: Ty<'db>,
    pub assign: bool,
}

impl<'db> FieldConstraint<'db> {
    pub fn map(&self, mut f: impl FnMut(&Ty<'db>) -> Ty<'db>) -> Self {
        FieldConstraint { ty: f(&self.ty), name: self.name.clone(), field: f(&self.field), assign: self.assign }
    }
}

/// That `ty` is the associated type `alias` of its class for `args`, like
/// `?0 = Vec[i32].Item`. It's solved once an impl for `args` is found, which
/// gives the type, or once `args` are type parameters that a `where` clause
/// gives an instance for, which makes `ty` the associated type itself.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub struct AssocConstraint<'db> {
    pub alias: TypeAliasId<'db>,
    pub args: Box<[Ty<'db>]>,
    pub ty: Ty<'db>,
}

impl<'db> AssocConstraint<'db> {
    /// The class that declares the associated type.
    pub fn class(&self, db: &'db dyn Db) -> ClassConstraint<'db> {
        let ItemContainerId::Class(class) = self.alias.container(db) else {
            unreachable!("an associated type without a definition is in a class")
        };
        ClassConstraint { class, args: self.args.clone() }
    }
}

/// What an "at most" constraint bounds: a mutability, or everything that a
/// value of a type holds.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum Held<'db> {
    Mutability(Ty<'db>),
    Type(Ty<'db>),
}

impl<'db> Held<'db> {
    pub fn map(&self, mut f: impl FnMut(&Ty<'db>) -> Ty<'db>) -> Self {
        match self {
            Held::Mutability(m) => Held::Mutability(f(m)),
            Held::Type(t) => Held::Type(f(t)),
        }
    }

    pub fn ty(&self) -> &Ty<'db> {
        match self {
            Held::Mutability(t) | Held::Type(t) => t,
        }
    }
}

/// That `held` is at most `bound`, with `imm ⊑ mut` (#243): a mutability
/// below another, or nothing in a type more `mut` than the bound. An object
/// holds only what's at most its mutability, so this is how `imm` is deep:
/// `[x]` needs `x`'s type to be at most `imm`. Inside it, what an array or
/// struct holds is at most its own mutability.
///
/// Unknowns are decided where only one choice works: a mutability below
/// `imm` is `imm`, and one above `mut` is `mut`. What's left about an
/// inferred function's parameters becomes part of its type, like
/// `forall a. a ⊑ imm => fn(a) -> imm [a]`, and its callers must meet it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub struct AtMostConstraint<'db> {
    pub held: Held<'db>,
    pub bound: Ty<'db>,
}

impl<'db> AtMostConstraint<'db> {
    pub fn map(&self, mut f: impl FnMut(&Ty<'db>) -> Ty<'db>) -> Self {
        AtMostConstraint { held: self.held.map(&mut f), bound: f(&self.bound) }
    }
}

/// Where an "at most" constraint comes from, which decides how breaking it
/// is reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AtMostSource<'db> {
    /// Making an array or struct that must hold what this says: reported
    /// by `check_deep_immutability` for something `mut`, which shows the
    /// whole type, and here for a parameter.
    Object(AtMostConstraint<'db>),
    /// What a closure of this type captured: reported as the closure
    /// having to be `mut`.
    Closure(Ty<'db>),
    /// What a function's type needs of what it's called with, or a
    /// constraint about a parameter that a complete signature can't state.
    Needed(AtMostConstraint<'db>),
}

/// A use of a name that several functions have, like `push` (#275): `ty`
/// is the type of the use, which must be the type of one of `candidates`.
/// The solver crosses out the candidates whose type no longer fits as the
/// types become known, and never guesses: see [`Solver::narrow_overloads`].
#[derive(Debug, Clone)]
pub struct OverloadConstraint<'db> {
    /// The candidates that still fit.
    pub candidates: Vec<FunctionId<'db>>,
    /// The type of each candidate, with new variables for its parameters, to
    /// try against `ty` without deciding anything.
    pub probes: Vec<Ty<'db>>,
    pub ty: Ty<'db>,
    /// All the functions with the name, and their probes, for errors.
    pub all: Vec<FunctionId<'db>>,
    pub all_probes: Vec<Ty<'db>>,
    /// The call, when the use is the method of a method call like `x.f(y)`.
    pub method_call: Option<ExprId>,
    /// Where the arguments are, when the use is called: then `ty` is a
    /// function type with a parameter for each, and the chosen function is
    /// checked against them one by one, so that a mismatch is reported at
    /// the argument.
    pub args: Vec<Origin<'db>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Constraint<'db> {
    /// The type found must be the type expected.
    Equal { expected: Ty<'db>, actual: Ty<'db>, origin: Origin<'db> },
    /// The types must have an instance of the class.
    Class { constraint: ClassConstraint<'db>, origin: Origin<'db> },
    /// The type must have the field.
    Field { constraint: FieldConstraint<'db>, origin: Origin<'db> },
    /// The type is an associated type of a class.
    Assoc { constraint: AssocConstraint<'db>, origin: Origin<'db> },
    /// Something is at most a mutability.
    AtMost { constraint: AtMostConstraint<'db>, source: AtMostSource<'db>, origin: Origin<'db> },
}

/// A type error. Its types are resolved as far as inference got.
#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub enum TypeError<'db> {
    Mismatch { origin: Origin<'db>, expected: Ty<'db>, actual: Ty<'db> },
    InfiniteType { origin: Origin<'db>, ty: Ty<'db> },
    /// No impl or `where` clause provides the instance.
    NoInstance { origin: Origin<'db>, constraint: ClassConstraint<'db> },
    /// The types weren't known well enough to choose an instance.
    Ambiguous { origin: Origin<'db>, constraint: ClassConstraint<'db> },
    /// Something isn't at most a mutability that it must be, like a `mut`
    /// array given to a function that needs `a ⊑ imm`, or a value of a
    /// type parameter put in an `imm` array, which an assumption could
    /// allow but a complete signature can't state yet.
    NotAtMost { origin: Origin<'db>, constraint: AtMostConstraint<'db> },
    /// The type has no field with this name.
    NoSuchField { origin: Origin<'db>, ty: Ty<'db>, name: Name },
    /// The type of a value whose field is used was never found.
    TypeNeeded { origin: Origin<'db> },
    /// An element of an array, or a field of a struct, of type `ty` is
    /// assigned to, but `ty` isn't `mut`.
    CannotChange { origin: Origin<'db>, ty: Ty<'db> },
    /// None of the functions with a name fits its use, of type `ty` (#275).
    NoOverload { origin: Origin<'db>, ty: Ty<'db>, candidates: Vec<FunctionId<'db>> },
    /// More than one of the functions with a name still fits its use when
    /// the body is done.
    AmbiguousOverload { origin: Origin<'db>, ty: Ty<'db>, candidates: Vec<FunctionId<'db>> },
}

impl<'db> TypeError<'db> {
    pub fn origin(&self) -> &Origin<'db> {
        match self {
            TypeError::Mismatch { origin, .. }
            | TypeError::InfiniteType { origin, .. }
            | TypeError::NoInstance { origin, .. }
            | TypeError::Ambiguous { origin, .. }
            | TypeError::NoSuchField { origin, .. }
            | TypeError::CannotChange { origin, .. }
            | TypeError::NotAtMost { origin, .. }
            | TypeError::NoOverload { origin, .. }
            | TypeError::AmbiguousOverload { origin, .. }
            | TypeError::TypeNeeded { origin } => origin,
        }
    }

    /// Replace the types in the error with `f` of them.
    pub fn map_types(&mut self, f: &mut impl FnMut(&Ty<'db>) -> Ty<'db>) {
        match self {
            TypeError::Mismatch { expected, actual, .. } => {
                *expected = f(expected);
                *actual = f(actual);
            }
            TypeError::InfiniteType { ty, .. }
            | TypeError::NoSuchField { ty, .. }
            | TypeError::CannotChange { ty, .. }
            | TypeError::NoOverload { ty, .. }
            | TypeError::AmbiguousOverload { ty, .. } => *ty = f(ty),
            TypeError::NoInstance { constraint, .. } | TypeError::Ambiguous { constraint, .. } => {
                *constraint = constraint.map(f)
            }
            TypeError::NotAtMost { constraint, .. } => *constraint = constraint.map(f),
            TypeError::TypeNeeded { .. } => {}
        }
    }
}

pub struct Solver<'db> {
    db: &'db dyn Db,
    pub unifier: Unifier<'db>,
    /// The class constraints known to hold, like those of `where` clauses,
    /// with their superclasses.
    assumptions: Vec<ClassConstraint<'db>>,
    /// The "at most" constraints known to hold, like `T: imm`.
    at_most_assumptions: Vec<AtMostConstraint<'db>>,
    impls: &'db [ImplId<'db>],
    /// The file of the code being solved, whose impls decide associated
    /// types.
    file: File,
    pending: Vec<(ClassConstraint<'db>, Origin<'db>)>,
    pending_fields: Vec<(FieldConstraint<'db>, Origin<'db>)>,
    pending_assoc: Vec<(AssocConstraint<'db>, Origin<'db>)>,
    pending_at_most: Vec<(AtMostConstraint<'db>, AtMostSource<'db>, Origin<'db>)>,
    pending_overloads: Vec<(OverloadConstraint<'db>, Origin<'db>)>,
    /// The function chosen for each use of a name with several, at the use,
    /// with the call if it's a method call.
    pub overload_choices: Vec<(Origin<'db>, Option<ExprId>, FunctionId<'db>)>,
    /// The impl chosen for each class constraint of a use of a class method
    /// or constant, like the `Show[i32]` of `show(1)`, at the use. Only the
    /// first per use and class: the constraints that it leads to, like the
    /// `Show[i32]` of an impl `Show[Option[T]] where Show[T]`, come after.
    pub chosen: Vec<(Origin<'db>, ClassId<'db>, ImplId<'db>)>,
    pub errors: Vec<TypeError<'db>>,
}

impl<'db> Solver<'db> {
    /// A solver for code in `file`, which can use the impls visible there.
    pub fn new(db: &'db dyn Db, file: File) -> Self {
        Solver {
            db,
            unifier: Unifier::new(),
            assumptions: Vec::new(),
            at_most_assumptions: Vec::new(),
            impls: visible_impls(db, file),
            file,
            pending: Vec::new(),
            pending_fields: Vec::new(),
            pending_assoc: Vec::new(),
            pending_at_most: Vec::new(),
            pending_overloads: Vec::new(),
            overload_choices: Vec::new(),
            chosen: Vec::new(),
            errors: Vec::new(),
        }
    }

    pub fn new_var(&mut self, kind: VarKind) -> Ty<'db> {
        self.unifier.new_var(kind)
    }

    /// Assume a class constraint holds, and with it its superclasses.
    pub fn assume(&mut self, constraint: ClassConstraint<'db>) {
        let mut stack = vec![constraint];
        while let Some(c) = stack.pop() {
            if self.assumptions.contains(&c) {
                continue;
            }
            // `class Ord[T] where Eq[T]`: `Ord[X]` gives `Eq[X]`; and
            // `type Iter: Iterator;` in `class Iterable[C]`: `Iterable[X]`
            // gives `Iterator[X.Iter]`.
            stack.extend(implied_by(self.db, &c));
            self.assumptions.push(c);
        }
    }

    /// Assume that `constraint` holds, as a function's `T: imm` does in its
    /// body.
    pub fn assume_at_most(&mut self, constraint: AtMostConstraint<'db>) {
        self.at_most_assumptions.push(constraint);
    }

    /// Whether the mutability `m` is at most `bound` by the order and the
    /// assumptions, following them through chains like `'v: 'm` and then
    /// `'m: imm`. An assumption that starts at `mut`, like `mut: 'v`, can be
    /// followed from anywhere, since everything is at most `mut`. Each
    /// mutability is visited once, so a chain can be as long as there are
    /// assumptions, and cycles like `'a: 'b, 'b: 'a` end. `le_iff_leSearch`
    /// in `lean/BoltMut/Solver.lean` proves that this finds every `m ⊑ bound`.
    fn mutability_le(&self, m: &Ty<'db>, bound: &Ty<'db>) -> bool {
        let resolve = |t: &Ty<'db>| self.unifier.shallow_resolve(t);
        let bound = resolve(bound);
        let below = |x: &Ty<'db>| {
            *x == bound || matches!(x, Ty::Mutability(Mutability::Imm)) || matches!(bound, Ty::Mutability(Mutability::Mut))
        };
        // What `m` is at most, through the assumptions so far.
        let mut seen = vec![resolve(m), Ty::Mutability(Mutability::Mut)];
        let mut next = 0;
        while let Some(x) = seen.get(next).cloned() {
            next += 1;
            if below(&x) {
                return true;
            }
            for a in &self.at_most_assumptions {
                if let Held::Mutability(h) = &a.held
                    && resolve(h) == x
                {
                    let up = resolve(&a.bound);
                    if !seen.contains(&up) {
                        seen.push(up);
                    }
                }
            }
        }
        false
    }

    /// Whether the function assumes an "at most" about a mutability, like
    /// `'v: imm`, that `pred` holds for.
    fn assumes_mutability(&self, mut pred: impl FnMut(&Ty<'db>, &Ty<'db>) -> bool) -> bool {
        self.at_most_assumptions.iter().any(|a| match &a.held {
            Held::Mutability(h) => pred(&self.unifier.shallow_resolve(h), &self.unifier.shallow_resolve(&a.bound)),
            Held::Type(_) => false,
        })
    }

    /// Whether the type parameter or associated type `t` is at most `bound`
    /// by an assumption.
    fn type_assumed(&self, t: &Ty<'db>, bound: &Ty<'db>) -> bool {
        self.at_most_assumptions
            .iter()
            .any(|a| matches!(&a.held, Held::Type(h) if h == t) && self.mutability_le(&a.bound, bound))
    }

    pub fn add(&mut self, constraint: Constraint<'db>) {
        match constraint {
            Constraint::Equal { expected, actual, origin } => {
                if let Err(err) = self.unifier.unify(&expected, &actual) {
                    let error = match err {
                        UnifyError::Mismatch(..) => TypeError::Mismatch {
                            origin,
                            expected: self.unifier.resolve(&expected),
                            actual: self.unifier.resolve(&actual),
                        },
                        UnifyError::Occurs(_, ty) => TypeError::InfiniteType { origin, ty },
                    };
                    self.errors.push(error);
                }
            }
            Constraint::Class { constraint, origin } => self.pending.push((constraint, origin)),
            // Solved right away when possible, so that the field's type is
            // known for what follows.
            Constraint::Field { constraint, origin } => {
                if !self.solve_field(&constraint, origin) {
                    self.pending_fields.push((constraint, origin));
                }
            }
            // Solved right away when possible too.
            Constraint::Assoc { constraint, origin } => {
                if !self.solve_assoc(&constraint, origin) {
                    self.pending_assoc.push((constraint, origin));
                }
            }
            // Solved right away when possible, so that what it decides is
            // known for what follows.
            Constraint::AtMost { constraint, source, origin } => {
                if !self.solve_at_most(&constraint, &source, origin) {
                    self.pending_at_most.push((constraint, source, origin));
                }
            }
        }
    }

    /// `held ⊑ bound`, as far as it can be decided now. A type is split into
    /// the mutabilities it holds: an array or struct is at most the bound,
    /// and what it holds at most its own mutability. Returns whether the
    /// constraint is solved, possibly by adding simpler ones.
    fn solve_at_most(&mut self, c: &AtMostConstraint<'db>, source: &AtMostSource<'db>, origin: Origin<'db>) -> bool {
        let bound = self.unifier.shallow_resolve(&c.bound);
        if matches!(bound, Ty::Mutability(Mutability::Mut) | Ty::Error | Ty::Never) {
            return true;
        }
        let sub = |solver: &mut Self, held: Held<'db>, bound: &Ty<'db>| {
            let constraint = AtMostConstraint { held, bound: bound.clone() };
            solver.add(Constraint::AtMost { constraint, source: source.clone(), origin });
        };
        match &c.held {
            Held::Type(t) => match self.unifier.shallow_resolve(t) {
                Ty::Array(m, elem) => {
                    sub(self, Held::Mutability(*m.clone()), &bound);
                    sub(self, Held::Type(*elem), &m);
                    true
                }
                // What its fields hold, like `'v ⊑ m` and `T ⊑ 'v` for
                // `VecIter[T, 'v]` (`crate::holds`).
                Ty::Adt(AdtId::Struct(s), args, ms, m) => {
                    sub(self, Held::Mutability(*m.clone()), &bound);
                    for atom in struct_atoms_of(self.db, s, &args, &ms, &m) {
                        sub(self, atom.held, &atom.bound);
                    }
                    true
                }
                // An associated type is split once an impl decides it, like
                // `A.Iter` in a struct's field for an `A` that's known.
                t @ Ty::Assoc(..) => {
                    let t = self.unifier.resolve(&t);
                    let decided = normalize_assoc(self.db, self.file, &t);
                    if decided != t {
                        sub(self, Held::Type(decided), &bound);
                        return true;
                    }
                    !has_vars(&t) && self.type_assumed(&t, &bound)
                }
                // An enum or a tuple is a value, which holds its parts.
                Ty::Adt(AdtId::Enum(_), args, _, _) | Ty::Tuple(args) | Ty::App(_, args) => {
                    for arg in args {
                        sub(self, Held::Type(arg), &bound);
                    }
                    true
                }
                Ty::Fn(_, _, m) => {
                    sub(self, Held::Mutability(*m), &bound);
                    true
                }
                // Decided once the type is known, or by an assumption about
                // a type parameter.
                Ty::Var(_) => false,
                t @ (Ty::Param(_) | Ty::Bound(_)) => self.type_assumed(&t, &bound),
                _ => true,
            },
            Held::Mutability(m) => match (self.unifier.shallow_resolve(m), &bound) {
                (Ty::Mutability(Mutability::Imm) | Ty::Error | Ty::Never, _) => true,
                (m, bound) if m == *bound => true,
                // Only `mut` is above `mut`, unless the function assumes a
                // mutability is, like the `'v` of `where mut: 'v`: then the
                // bound could be that one, and is decided later
                // (`Le.eq_mutable` in `lean/BoltMut/Solver.lean`).
                (Ty::Mutability(Mutability::Mut), Ty::Var(_)) => {
                    if self.assumes_mutability(|h, _| matches!(h, Ty::Mutability(Mutability::Mut) | Ty::Var(_))) {
                        return false;
                    }
                    self.equal(&bound, &Ty::Mutability(Mutability::Mut), origin);
                    true
                }
                // Only `imm` is at most `imm`, unless the function assumes a
                // mutability is, like the `'v` of `where 'v: imm`: then `m`
                // could be that one, and is decided later (`Le.eq_immutable`).
                (m @ Ty::Var(_), Ty::Mutability(Mutability::Imm)) => {
                    if self.assumes_mutability(|_, b| matches!(b, Ty::Mutability(Mutability::Imm) | Ty::Var(_))) {
                        return false;
                    }
                    self.equal(&m, &Ty::Mutability(Mutability::Imm), origin);
                    true
                }
                // Something `mut` below a bound that isn't, unless an
                // assumption like `mut: 'v` says it is.
                (Ty::Mutability(Mutability::Mut), bound) if !self.mutability_le(&Ty::Mutability(Mutability::Mut), bound) => {
                    self.at_most_broken(source, origin);
                    true
                }
                (m, bound) => self.mutability_le(&m, bound),
            },
        }
    }

    /// Report that something `mut` is below a bound that isn't.
    fn at_most_broken(&mut self, source: &AtMostSource<'db>, origin: Origin<'db>) {
        match source {
            AtMostSource::Object(_) => {}
            AtMostSource::Closure(closure) => {
                let Ty::Fn(params, ret, _) = closure else { unreachable!("a closure has a function type") };
                let mutable = Ty::Fn(params.clone(), ret.clone(), Box::new(Ty::Mutability(Mutability::Mut)));
                let error = TypeError::Mismatch {
                    origin,
                    expected: self.unifier.resolve(closure),
                    actual: self.unifier.resolve(&mutable),
                };
                if !self.errors.contains(&error) {
                    self.errors.push(error);
                }
            }
            AtMostSource::Needed(needed) => {
                let error = TypeError::NotAtMost { origin, constraint: needed.map(|t| self.unifier.resolve(t)) };
                if !self.errors.contains(&error) {
                    self.errors.push(error);
                }
            }
        }
    }

    /// Settle the "at most" constraints that are left, keeping those whose
    /// unknowns are all in `keep`: they'll become part of a function's
    /// type. Another unknown can be chosen freely, since nothing else
    /// depends on it, so it's chosen to meet them: `mut` above, and `imm`
    /// below. What's left with no unknowns is about parameters, which only
    /// an assumption could meet; it's reported unless `keep_params`.
    pub fn settle_at_most(&mut self, keep: &[TyVar], keep_params: bool) {
        for (constraint, source, origin) in std::mem::take(&mut self.pending_at_most) {
            let resolved = constraint.map(|t| self.unifier.resolve(t));
            let free = |t: &Ty<'db>| {
                let mut vars = Vec::new();
                vars_in(t, &mut vars);
                vars.into_iter().filter(|v| !keep.contains(v)).count()
            };
            let held_free = free(resolved.held.ty());
            let bound_free = free(&resolved.bound);
            if held_free == 0 && bound_free == 0 {
                let has_vars = has_vars(resolved.held.ty()) || has_vars(&resolved.bound);
                if has_vars || keep_params {
                    self.pending_at_most.push((constraint, source, origin));
                } else {
                    let needed = match &source {
                        AtMostSource::Needed(needed) | AtMostSource::Object(needed) => needed.clone(),
                        AtMostSource::Closure(_) => resolved.clone(),
                    };
                    self.at_most_broken(&AtMostSource::Needed(needed), origin);
                }
                continue;
            }
            // The least choice first: what's below is `imm`. Only if that's
            // not free is what's above `mut`.
            match &resolved.held {
                Held::Mutability(m @ Ty::Var(v)) if !keep.contains(v) => {
                    self.equal(m, &Ty::Mutability(Mutability::Imm), origin);
                }
                // A type still unknown becomes the error type, which fits.
                Held::Type(_) if held_free > 0 => {}
                _ => {
                    if let Ty::Var(v) = &resolved.bound
                        && !keep.contains(v)
                    {
                        self.equal(&resolved.bound, &Ty::Mutability(Mutability::Mut), origin);
                    }
                }
            }
        }
        self.solve();
    }

    /// The "at most" constraints left, resolved.
    pub fn take_pending_at_most(&mut self) -> Vec<(AtMostConstraint<'db>, Origin<'db>)> {
        std::mem::take(&mut self.pending_at_most)
            .into_iter()
            .map(|(c, _, origin)| (c.map(|t| self.unifier.resolve(t)), origin))
            .collect()
    }

    /// Solve the associated type constraints that can be solved now.
    pub fn solve_pending_assoc(&mut self) {
        loop {
            let before = self.pending_assoc.len();
            for (constraint, origin) in std::mem::take(&mut self.pending_assoc) {
                if !self.solve_assoc(&constraint, origin) {
                    self.pending_assoc.push((constraint, origin));
                }
            }
            if self.pending_assoc.len() == before {
                return;
            }
        }
    }

    /// `ty` with each associated type in it, like `Vec[?0].Item`, replaced
    /// by a new variable that a [`Constraint::Assoc`] decides.
    pub fn flatten_assoc(&mut self, ty: &Ty<'db>, origin: Origin<'db>) -> Ty<'db> {
        if !ty.any(&mut |t| matches!(t, Ty::Assoc(..))) {
            return ty.clone();
        }
        ty.fold(&mut |t| {
            let Ty::Assoc(alias, args) = t else { return None };
            let args = args.iter().map(|a| self.flatten_assoc(a, origin)).collect();
            let var = self.new_var(VarKind::General);
            let constraint = AssocConstraint { alias: *alias, args, ty: var.clone() };
            self.add(Constraint::Assoc { constraint, origin });
            Some(var)
        })
    }

    /// Solve an associated type constraint if its arguments are known well
    /// enough. Returns `false` if it has to wait.
    fn solve_assoc(&mut self, constraint: &AssocConstraint<'db>, origin: Origin<'db>) -> bool {
        let goal = constraint.class(self.db).map(|t| self.unifier.resolve(t));
        if goal.args.iter().any(|t| t.any(&mut |t| *t == Ty::Error)) {
            self.unifier.set_error(&constraint.ty);
            return true;
        }
        let mut found = None;
        for &imp in self.impls {
            let Some(target) = &impl_header(self.db, imp).class else { continue };
            if target.class != goal.class {
                continue;
            }
            let mut subst = HashMap::new();
            match match_all(&target.args, &goal.args, &mut subst) {
                Match::Yes if found.is_none() => found = Some((imp, subst)),
                // Overlapping impls, or one that may match: wait until the
                // types decide.
                Match::Yes | Match::Maybe => return false,
                Match::No => {}
            }
        }
        let ty = match found {
            Some((imp, subst)) => match impl_assoc_ty(self.db, imp, constraint.alias) {
                Some(ty) => {
                    let ty = ty.fold(&mut |t| match t {
                        Ty::Param(p) if p.def == GenericDefId::Impl(imp) => {
                            Some(subst.get(p).cloned().unwrap_or(Ty::Error))
                        }
                        _ => None,
                    });
                    // What the impl gives can have associated types of its
                    // own, like the `A.Item` of `type Item = (A.Item, B.Item)`.
                    self.flatten_assoc(&ty, origin)
                }
                // The impl doesn't define it, which is reported with the impl.
                None => Ty::Error,
            },
            None if goal.args.iter().any(has_vars) => return false,
            // `C.Item` for a parameter `C` with `where Index[C]`.
            None if self.assumptions.contains(&goal) => Ty::Assoc(constraint.alias, goal.args.clone()),
            None => {
                let error = TypeError::NoInstance { origin, constraint: goal };
                if !self.errors.contains(&error) {
                    self.errors.push(error);
                }
                Ty::Error
            }
        };
        if ty == Ty::Error {
            self.unifier.set_error(&constraint.ty);
        } else {
            // The variable stands for the use, whose context decides what
            // it should be: `let n: i32 = get(...)` expects an `i32`.
            self.equal(&constraint.ty, &ty, origin);
        }
        true
    }

    /// Give the associated types that couldn't be decided, and whose
    /// variable nothing decided either, the type they stand for, like
    /// `?0.Item`, so that a function whose argument's type isn't known keeps
    /// them in its type: `forall a. Index[a] => fn(a) -> a.Item`. Those whose
    /// variable something decided, like the `Bool` of `xs[0] = True`, stay,
    /// to become part of the function's type: `a.Item = Bool`.
    pub fn keep_pending_assoc(&mut self) {
        for (constraint, origin) in std::mem::take(&mut self.pending_assoc) {
            let args = constraint.args.iter().map(|t| self.unifier.resolve(t)).collect();
            if let Ty::Var(_) = self.unifier.shallow_resolve(&constraint.ty) {
                let _ = self.unifier.unify(&Ty::Assoc(constraint.alias, args), &constraint.ty);
            } else {
                self.pending_assoc.push((constraint, origin));
            }
        }
    }

    /// The associated type constraints that couldn't be solved, resolved.
    pub fn take_pending_assoc(&mut self) -> Vec<(AssocConstraint<'db>, Origin<'db>)> {
        std::mem::take(&mut self.pending_assoc)
            .into_iter()
            .map(|(c, origin)| {
                let args = c.args.iter().map(|t| self.unifier.resolve(t)).collect();
                (AssocConstraint { alias: c.alias, args, ty: self.unifier.resolve(&c.ty) }, origin)
            })
            .collect()
    }

    /// Solve a field constraint if the type is known well enough. Returns
    /// `false` if it has to wait.
    fn solve_field(&mut self, constraint: &FieldConstraint<'db>, origin: Origin<'db>) -> bool {
        let ty = self.unifier.shallow_resolve(&constraint.ty);
        let field = match &ty {
            // `F[A]` could be any struct until `F` is known.
            Ty::Var(_) => return false,
            Ty::App(head, _) if matches!(**head, Ty::Var(_)) => return false,
            // An error was already reported, or the value is never made.
            Ty::Error | Ty::Never => {
                self.unifier.set_error(&constraint.field);
                return true;
            }
            Ty::Adt(AdtId::Struct(s), args, mutabilities, mutability) => {
                if constraint.assign {
                    self.require_mut(&ty, mutability, origin);
                }
                let fields = field_types(self.db, VariantDef::Struct(*s));
                let args = struct_args(args, mutabilities, mutability);
                fields.field(&constraint.name).map(|(_, t)| subst_params(t, GenericDefId::Struct(*s), &args))
            }
            Ty::Tuple(tys) => constraint.name.as_str().parse::<usize>().ok().and_then(|i| tys.get(i).cloned()),
            _ => None,
        };
        match field {
            Some(declared) => self.equal(&declared, &constraint.field, origin),
            None => {
                let ty = self.unifier.resolve(&ty);
                self.errors.push(TypeError::NoSuchField { origin, ty, name: constraint.name.clone() });
                self.unifier.set_error(&constraint.field);
            }
        }
        true
    }

    /// Report that `ty`, an array or struct with the mutability
    /// `mutability`, is changed at `origin` but isn't `mut`. A mutability
    /// that isn't known yet becomes `Mut`.
    pub fn require_mut(&mut self, ty: &Ty<'db>, mutability: &Ty<'db>, origin: Origin<'db>) {
        match self.unifier.shallow_resolve(mutability) {
            Ty::Mutability(Mutability::Mut) | Ty::Error | Ty::Never => {}
            Ty::Var(_) => self.equal(&Ty::Mutability(Mutability::Mut), mutability, origin),
            _ => self.errors.push(TypeError::CannotChange { origin, ty: self.unifier.resolve(ty) }),
        }
    }

    /// Mark the field constraint of `origin` as being about a field that's
    /// assigned to, if it's waiting for its type.
    pub fn assign_field(&mut self, origin: Origin<'db>) {
        if let Some((constraint, _)) = self.pending_fields.iter_mut().find(|(_, o)| *o == origin) {
            constraint.assign = true;
        }
    }

    pub fn equal(&mut self, expected: &Ty<'db>, actual: &Ty<'db>, origin: Origin<'db>) {
        self.add(Constraint::Equal { expected: expected.clone(), actual: actual.clone(), origin });
    }

    /// Wait for the types to decide which function a use of a name with
    /// several is.
    pub fn add_overload(&mut self, constraint: OverloadConstraint<'db>, origin: Origin<'db>) {
        self.pending_overloads.push((constraint, origin));
    }

    pub fn has_pending_overloads(&self) -> bool {
        !self.pending_overloads.is_empty()
    }

    /// Cross out the candidates of the uses of names with several functions
    /// that no longer fit. A candidate fits while its type can still be
    /// unified with that of the use, which is tried on a copy of the
    /// unifier, so that nothing is decided. Returns the uses with one
    /// candidate left, with it, for the caller to instantiate; those with
    /// none left are errors, and the others wait.
    pub fn narrow_overloads(&mut self) -> Vec<(OverloadConstraint<'db>, Origin<'db>, FunctionId<'db>)> {
        let mut ready = Vec::new();
        for (mut c, origin) in std::mem::take(&mut self.pending_overloads) {
            let fits: Vec<bool> = c.probes.iter().map(|probe| self.fits(probe, &c.ty)).collect();
            let mut keep = fits.iter();
            c.candidates.retain(|_| *keep.next().unwrap());
            let mut keep = fits.iter();
            c.probes.retain(|_| *keep.next().unwrap());
            match c.candidates.as_slice() {
                [] => match self.best_overload(&c) {
                    // Checked as if it had been chosen, for the errors of the
                    // arguments that don't fit it.
                    Some(f) => ready.push((c, origin, f)),
                    None => {
                        let ty = self.unifier.resolve(&c.ty);
                        self.errors.push(TypeError::NoOverload { origin, ty, candidates: c.all.clone() });
                    }
                },
                [f] => {
                    let f = *f;
                    ready.push((c, origin, f));
                }
                _ => self.pending_overloads.push((c, origin)),
            }
        }
        ready
    }

    /// When no function with a name fits a use, the one that comes closest,
    /// to report the errors as if it had been chosen: of those with as many
    /// parameters as the use's function type has, the only one that fits
    /// the most of them, then that fits the most if mutabilities don't
    /// count, so that an `imm [i32]` is closer to a `mut [i32]` than to a
    /// `Vec`. `None` when there's no such one.
    fn best_overload(&self, c: &OverloadConstraint<'db>) -> Option<FunctionId<'db>> {
        let Ty::Fn(params, _, _) = self.unifier.shallow_resolve(&c.ty) else { return None };
        // The error type fits anything.
        let loose = |t: &Ty<'db>| t.fold(&mut |t| matches!(t, Ty::Mutability(_)).then_some(Ty::Error));
        let scores: Vec<(FunctionId<'db>, (usize, usize))> = c
            .all
            .iter()
            .zip(&c.all_probes)
            .filter_map(|(&f, probe)| match probe {
                Ty::Fn(own, _, _) if own.len() == params.len() => {
                    let pairs = || own.iter().zip(params.iter());
                    let strict = pairs().filter(|(a, b)| self.fits(a, b)).count();
                    let loose = pairs().filter(|(a, b)| self.fits(&loose(a), &loose(&self.unifier.resolve(b)))).count();
                    Some((f, (strict, loose)))
                }
                _ => None,
            })
            .collect();
        let best = scores.iter().map(|(_, score)| *score).max()?;
        match scores.iter().filter(|(_, score)| *score == best).collect::<Vec<_>>().as_slice() {
            [(f, _)] => Some(*f),
            _ => None,
        }
    }

    /// Remember where the arguments of a call to a use of a name with
    /// several functions are; `ty` is the type of the use.
    pub fn attach_overload_args(&mut self, ty: &Ty<'db>, args: Vec<Origin<'db>>) {
        if let Some((c, _)) = self.pending_overloads.iter_mut().find(|(c, _)| c.ty == *ty) {
            c.args = args;
        }
    }

    /// Whether `a` can still be unified with `b`, without deciding anything.
    fn fits(&self, a: &Ty<'db>, b: &Ty<'db>) -> bool {
        self.unifier.clone().unify(a, b).is_ok()
    }

    /// Report the uses of names with several functions that the types never
    /// decided. They aren't generalized: generic code uses classes.
    pub fn report_pending_overloads(&mut self) {
        for (c, origin) in std::mem::take(&mut self.pending_overloads) {
            // Unknown because of an error, which is reported already.
            if self.unifier.has_error_var(&c.ty) {
                continue;
            }
            let ty = self.unifier.resolve(&c.ty);
            self.errors.push(TypeError::AmbiguousOverload { origin, ty, candidates: c.candidates });
        }
    }

    /// Solve the class and field constraints that can be solved now.
    /// Returns whether any were.
    pub fn solve(&mut self) -> bool {
        let mut progress_ever = false;
        loop {
            let mut progress = false;
            for (constraint, origin) in std::mem::take(&mut self.pending_fields) {
                if self.solve_field(&constraint, origin) {
                    progress = true;
                } else {
                    self.pending_fields.push((constraint, origin));
                }
            }
            for (constraint, origin) in std::mem::take(&mut self.pending_assoc) {
                if self.solve_assoc(&constraint, origin) {
                    progress = true;
                } else {
                    self.pending_assoc.push((constraint, origin));
                }
            }
            for (constraint, source, origin) in std::mem::take(&mut self.pending_at_most) {
                if self.solve_at_most(&constraint, &source, origin) {
                    progress = true;
                } else {
                    self.pending_at_most.push((constraint, source, origin));
                }
            }
            for (constraint, origin) in std::mem::take(&mut self.pending) {
                let constraint = constraint.map(|t| self.unifier.resolve(t));
                match self.try_solve(&constraint) {
                    Step::Solved(subgoals, at_most, imp) => {
                        progress = true;
                        let same_use = |(o, c, _): &(Origin<'db>, ClassId<'db>, ImplId<'db>)| {
                            o.body == origin.body && o.node == origin.node && *c == constraint.class
                        };
                        if let Some(imp) = imp
                            && !self.chosen.iter().any(same_use)
                        {
                            self.chosen.push((origin, constraint.class, imp));
                        }
                        self.pending.extend(subgoals.into_iter().map(|c| (c, origin)));
                        // What the impl assumes of its arguments, like the
                        // `T: imm` of `impl[T: imm] ToImm[Vec[T]]`.
                        for constraint in at_most {
                            let source = AtMostSource::Needed(constraint.clone());
                            self.add(Constraint::AtMost { constraint, source, origin });
                        }
                    }
                    Step::NoInstance => {
                        // An associated type of the same class may have
                        // reported it already.
                        let error = TypeError::NoInstance { origin, constraint };
                        if !self.errors.contains(&error) {
                            self.errors.push(error);
                        }
                    }
                    Step::Wait => self.pending.push((constraint, origin)),
                }
            }
            if !progress {
                return progress_ever;
            }
            progress_ever = true;
        }
    }

    /// The class constraints that couldn't be solved yet, resolved.
    pub fn take_pending(&mut self) -> Vec<(ClassConstraint<'db>, Origin<'db>)> {
        std::mem::take(&mut self.pending)
            .into_iter()
            .map(|(c, origin)| (c.map(|t| self.unifier.resolve(t)), origin))
            .collect()
    }

    /// The field constraints that couldn't be solved yet, resolved.
    pub fn take_pending_fields(&mut self) -> Vec<(FieldConstraint<'db>, Origin<'db>)> {
        std::mem::take(&mut self.pending_fields)
            .into_iter()
            .map(|(c, origin)| (c.map(|t| self.unifier.resolve(t)), origin))
            .collect()
    }

    /// Solve what can be, give numeric variables their default types, and
    /// solve what that decides.
    pub fn solve_with_defaults(&mut self) {
        self.solve();
        self.unifier.default_numeric_vars();
        self.solve();
    }

    /// Finish solving: default numeric variables and mutabilities, solve
    /// what can be, and report the rest as ambiguous.
    pub fn finish(&mut self) {
        self.solve_with_defaults();
        self.report_pending_overloads();
        // A body checked on its own can't add to its function's type.
        self.settle_at_most(&[], false);
        self.keep_pending_assoc();
        for (constraint, origin) in self.take_pending() {
            // Unknown because of an error, which is reported already.
            if !constraint.args.iter().any(|t| self.unifier.has_error_var(t)) {
                self.errors.push(TypeError::Ambiguous { origin, constraint });
            }
        }
        for (_, origin) in self.take_pending_fields() {
            self.errors.push(TypeError::TypeNeeded { origin });
        }
        let errors = std::mem::take(&mut self.errors);
        self.errors = errors.into_iter().map(|e| resolve_error(&self.unifier, e)).collect();
        prune_implied_errors(self.db, &mut self.errors);
        // After resolving the errors, so that an array whose mutability
        // nothing decided is shown as `[T]` in them, not as `imm [T]`.
        self.unifier.default_mutability_vars();
        // After resolving the errors, which would otherwise be left out for
        // having the error type in them.
        self.unifier.error_unknowns();
    }

    fn try_solve(&self, goal: &ClassConstraint<'db>) -> Step<'db> {
        // An error type somewhere means an error was already reported.
        if goal.args.iter().any(|t| t.any(&mut |t| *t == Ty::Error)) {
            return Step::Solved(Vec::new(), Vec::new(), None);
        }
        if self.assumptions.contains(goal) {
            return Step::Solved(Vec::new(), Vec::new(), None);
        }
        let has_vars = goal.args.iter().any(has_vars);
        let mut found = None;
        let mut maybe = has_vars && self.assumptions.iter().any(|a| a.class == goal.class);
        for &imp in self.impls {
            let Some(target) = &impl_header(self.db, imp).class else { continue };
            if target.class != goal.class {
                continue;
            }
            let mut subst = HashMap::new();
            match match_all(&target.args, &goal.args, &mut subst) {
                Match::Yes => {
                    if found.is_some() {
                        // Overlapping impls; wait until something decides.
                        return Step::Wait;
                    }
                    found = Some((imp, subst));
                }
                Match::Maybe => maybe = true,
                Match::No => {}
            }
        }
        match found {
            Some(_) if maybe => Step::Wait,
            Some((imp, subst)) => {
                let def = GenericDefId::Impl(imp);
                let instantiate = |t: &Ty<'db>| {
                    t.fold(&mut |t| match t {
                        Ty::Param(p) if p.def == def => Some(subst.get(p).cloned().unwrap_or(Ty::Error)),
                        _ => None,
                    })
                };
                let predicates = generic_predicates(self.db, def);
                let subgoals = predicates.predicates.iter().map(|p| p.map(instantiate)).collect();
                let at_most = predicates.at_most.iter().map(|(c, _)| c.map(instantiate)).collect();
                Step::Solved(subgoals, at_most, Some(imp))
            }
            None if maybe || has_vars => Step::Wait,
            None => Step::NoInstance,
        }
    }
}

enum Step<'db> {
    /// Solved, if these hold too, with the impl that was used, if any.
    Solved(Vec<ClassConstraint<'db>>, Vec<AtMostConstraint<'db>>, Option<ImplId<'db>>),
    NoInstance,
    /// The types aren't known well enough yet.
    Wait,
}

#[derive(Clone, Copy)]
enum Match {
    Yes,
    No,
    /// It depends on variables that aren't decided yet.
    Maybe,
}

impl Match {
    /// Whether this and `other` both match. `other` isn't tried when this
    /// doesn't match.
    fn and(self, other: impl FnOnce() -> Match) -> Match {
        match self {
            Match::No => Match::No,
            _ => match (self, other()) {
                (_, Match::No) => Match::No,
                (Match::Yes, Match::Yes) => Match::Yes,
                _ => Match::Maybe,
            },
        }
    }

    /// `Maybe`, unless it's certainly no match.
    fn at_most_maybe(self) -> Match {
        match self {
            Match::No => Match::No,
            _ => Match::Maybe,
        }
    }
}

/// Match the types of an impl, where its parameters stand for any type,
/// against the types of a goal, without deciding any variables.
fn match_all<'db>(
    patterns: &[Ty<'db>],
    tys: &[Ty<'db>],
    subst: &mut HashMap<GenericParamId<'db>, Ty<'db>>,
) -> Match {
    if patterns.len() != tys.len() {
        return Match::No;
    }
    patterns.iter().zip(tys).fold(Match::Yes, |result, (p, t)| result.and(|| match_ty(p, t, subst)))
}

fn match_ty<'db>(pattern: &Ty<'db>, ty: &Ty<'db>, subst: &mut HashMap<GenericParamId<'db>, Ty<'db>>) -> Match {
    match (pattern, ty) {
        (Ty::Param(p), _) if matches!(p.def, GenericDefId::Impl(_)) => match subst.get(p) {
            Some(bound) if bound == ty => Match::Yes,
            Some(bound) if has_vars(bound) || matches!(ty, Ty::Var(_)) => Match::Maybe,
            Some(_) => Match::No,
            None => {
                subst.insert(*p, ty.clone());
                Match::Yes
            }
        },
        (_, Ty::Var(_)) => Match::Maybe,
        (_, Ty::App(head, _)) if matches!(**head, Ty::Var(_)) => match pattern {
            // `F[A]` against `?0[i32]`: decided by what `?0` becomes, unless
            // the arguments can't match anyway.
            Ty::App(..) | Ty::Adt(..) => match_app(pattern, ty, subst).at_most_maybe(),
            _ => Match::No,
        },
        (Ty::Adt(a, xs, ms, m), Ty::Adt(b, ys, ns, n)) if a == b && xs.len() == ys.len() => {
            match_all(xs, ys, subst).and(|| match_all(ms, ns, subst)).and(|| match_ty(m, n, subst))
        }
        (Ty::App(..), Ty::Adt(..) | Ty::App(..)) | (Ty::Adt(..), Ty::App(..)) => match_app(pattern, ty, subst),
        (Ty::Tuple(xs), Ty::Tuple(ys)) => match_all(xs, ys, subst),
        (Ty::Array(m, x), Ty::Array(n, y)) => match_ty(x, y, subst).and(|| match_ty(m, n, subst)),
        (Ty::Fn(xs, x, m), Ty::Fn(ys, y, n)) => {
            match_all(xs, ys, subst).and(|| match_ty(x, y, subst)).and(|| match_ty(m, n, subst))
        }
        (x, y) if x == y => Match::Yes,
        _ => Match::No,
    }
}

/// Match two applications by their last arguments, and their heads with
/// the arguments before them, as in unification: the pattern `F[A]` matches
/// `Either[String, i32]` with `F` the constructor `Either[String]`.
fn match_app<'db>(pattern: &Ty<'db>, ty: &Ty<'db>, subst: &mut HashMap<GenericParamId<'db>, Ty<'db>>) -> Match {
    let split = |t: &Ty<'db>| -> (Ty<'db>, Vec<Ty<'db>>) {
        match t {
            Ty::Adt(id, args, ms, m) => (Ty::Adt(*id, Box::new([]), ms.clone(), m.clone()), args.to_vec()),
            Ty::App(head, args) => ((**head).clone(), args.to_vec()),
            t => (t.clone(), Vec::new()),
        }
    };
    let (p_head, p_args) = split(pattern);
    let (t_head, t_args) = split(ty);
    let n = p_args.len().min(t_args.len());
    let p_rest = Ty::app(p_head, p_args[..p_args.len() - n].to_vec());
    let t_rest = Ty::app(t_head, t_args[..t_args.len() - n].to_vec());
    match_all(&p_args[p_args.len() - n..], &t_args[t_args.len() - n..], subst).and(|| match_ty(&p_rest, &t_rest, subst))
}

/// The impls code in `file` can use: those of the file, of its prelude,
/// and of the files they import, directly or not.
#[salsa::tracked(returns(ref))]
pub fn visible_impls<'db>(db: &'db dyn Db, file: File) -> Vec<ImplId<'db>> {
    std::iter::once(file)
        .chain(files_used_by(db, file).iter().copied())
        .flat_map(|f| file_scope(db, f).scope.impls().iter().copied())
        .collect()
}

/// `ty` with the associated types in it that an impl visible in `file`
/// decides replaced by what the impl gives, like `Vec[i32].Item` by `i32`.
/// Those whose arguments aren't known well enough stay.
pub fn normalize_assoc<'db>(db: &'db dyn Db, file: File, ty: &Ty<'db>) -> Ty<'db> {
    if !ty.any(&mut |t| matches!(t, Ty::Assoc(..))) {
        return ty.clone();
    }
    ty.fold(&mut |t| {
        let Ty::Assoc(alias, args) = t else { return None };
        let args: Box<[Ty<'db>]> = args.iter().map(|a| normalize_assoc(db, file, a)).collect();
        let ItemContainerId::Class(class) = alias.container(db) else { return None };
        let mut found = None;
        for &imp in visible_impls(db, file) {
            let Some(target) = &impl_header(db, imp).class else { continue };
            if target.class != class {
                continue;
            }
            let mut subst = HashMap::new();
            match match_all(&target.args, &args, &mut subst) {
                Match::Yes if found.is_none() => found = Some((imp, subst)),
                Match::Yes | Match::Maybe => return Some(Ty::Assoc(*alias, args)),
                Match::No => {}
            }
        }
        let Some((imp, subst)) = found else { return Some(Ty::Assoc(*alias, args)) };
        let ty = impl_assoc_ty(db, imp, *alias)?;
        let ty = ty.fold(&mut |t| match t {
            Ty::Param(p) if p.def == GenericDefId::Impl(imp) => Some(subst.get(p).cloned().unwrap_or(Ty::Error)),
            _ => None,
        });
        // It can have associated types of its own.
        Some(normalize_assoc(db, file, &ty))
    })
}

/// Leave out the missing instances that another one at the same place
/// implies, like `Index[i32]` next to `IndexMut[i32]` for `n[0] = 1`.
pub fn prune_implied_errors<'db>(db: &'db dyn Db, errors: &mut Vec<TypeError<'db>>) {
    let missing: Vec<(Origin<'db>, ClassConstraint<'db>)> = errors
        .iter()
        .filter_map(|e| match e {
            TypeError::NoInstance { origin, constraint } => Some((*origin, constraint.clone())),
            _ => None,
        })
        .collect();
    let mut kept: Vec<(Origin<'db>, ClassConstraint<'db>)> = Vec::new();
    errors.retain(|e| {
        let TypeError::NoInstance { origin, constraint } = e else { return true };
        let same_place = |o: &Origin<'db>| o.body == origin.body && o.node == origin.node;
        let implied = missing.iter().any(|(o, other)| {
            same_place(o) && other != constraint && with_superclasses(db, vec![other.clone()]).contains(constraint)
        });
        // The same one twice, like from a class constraint and from an
        // associated type of the class: the one that says why is kept.
        let better = missing.iter().any(|(o, other)| {
            same_place(o) && other == constraint && o.reason != Reason::None && origin.reason == Reason::None
        });
        let again = kept.iter().any(|(o, other)| same_place(o) && other == constraint);
        if implied || better || again {
            return false;
        }
        kept.push((*origin, constraint.clone()));
        true
    });
}

/// Whether a type has an inference variable in it.
pub fn has_vars(ty: &Ty<'_>) -> bool {
    ty.any(&mut |t| matches!(t, Ty::Var(_)))
}

/// The inference variables in a type, in order of appearance.
pub fn vars_in(ty: &Ty<'_>, out: &mut Vec<TyVar>) {
    ty.any(&mut |t| {
        if let Ty::Var(v) = t
            && !out.contains(v)
        {
            out.push(*v);
        }
        false
    });
}

#[cfg(test)]
mod tests;

/// The types of an error as far as `unifier` knows them. A mismatch shows a
/// number literal whose type nothing decided as `{integer}` or `{float}`,
/// rather than the default it was given: that's not what was found. Other
/// errors, like a missing instance, are about the default.
pub fn resolve_error<'db>(unifier: &Unifier<'db>, mut error: TypeError<'db>) -> TypeError<'db> {
    match error {
        TypeError::Mismatch { .. } => error.map_types(&mut |t| unifier.resolve_literals(t)),
        _ => error.map_types(&mut |t| unifier.resolve(t)),
    }
    error
}

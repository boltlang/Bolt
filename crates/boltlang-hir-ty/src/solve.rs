//! The solver: takes the constraints inference generates and solves them,
//! using the [`Unifier`] for equalities. Class constraints wait until their
//! types are known well enough to pick an impl.

use std::collections::HashMap;

use boltlang_base_db::{Db, File};
use boltlang_hir_def::{
    Name,
    body::DefWithBodyId,
    hir::{ExprId, PatId, TypeRefId},
    ids::FunctionId,
    item_tree::{ModItem, item_tree},
    nameres::{file_scope, resolve_import_path},
    resolver::GenericDefId,
};

use crate::{
    lower::{ClassConstraint, VariantDef, field_types, generic_predicates, impl_header, subst_params},
    ty::{AdtId, GenericParamId, Ty, TyVar, VarKind},
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

/// Why a type is expected of an expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum Reason<'db> {
    /// Nothing worth pointing at, like the `bool` of a condition.
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub enum ExprOrPat {
    Expr(ExprId),
    Pat(PatId),
}

/// That `ty` has a field `name` of type `field`, like `x` in `p.x`. It's
/// solved once `ty` is known, and a function whose argument's type isn't
/// known keeps it as part of its type, like `forall a. a.x: i32 => fn(a)
/// -> i32`, so that it works on any struct with that field.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub struct FieldConstraint<'db> {
    pub ty: Ty<'db>,
    pub name: Name,
    pub field: Ty<'db>,
}

impl<'db> FieldConstraint<'db> {
    pub fn map(&self, mut f: impl FnMut(&Ty<'db>) -> Ty<'db>) -> Self {
        FieldConstraint { ty: f(&self.ty), name: self.name.clone(), field: f(&self.field) }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Constraint<'db> {
    /// The type found must be the type expected.
    Equal { expected: Ty<'db>, actual: Ty<'db>, origin: Origin<'db> },
    /// The types must have an instance of the class.
    Class { constraint: ClassConstraint<'db>, origin: Origin<'db> },
    /// The type must have the field.
    Field { constraint: FieldConstraint<'db>, origin: Origin<'db> },
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
    /// The type has no field with this name.
    NoSuchField { origin: Origin<'db>, ty: Ty<'db>, name: Name },
    /// The type of a value whose field is used was never found.
    TypeNeeded { origin: Origin<'db> },
}

pub struct Solver<'db> {
    db: &'db dyn Db,
    pub unifier: Unifier<'db>,
    /// The class constraints known to hold, like those of `where` clauses,
    /// with their superclasses.
    assumptions: Vec<ClassConstraint<'db>>,
    impls: &'db [boltlang_hir_def::ids::ImplId<'db>],
    pending: Vec<(ClassConstraint<'db>, Origin<'db>)>,
    pending_fields: Vec<(FieldConstraint<'db>, Origin<'db>)>,
    pub errors: Vec<TypeError<'db>>,
}

impl<'db> Solver<'db> {
    /// A solver for code in `file`, which can use the impls visible there.
    pub fn new(db: &'db dyn Db, file: File) -> Self {
        Solver {
            db,
            unifier: Unifier::new(),
            assumptions: Vec::new(),
            impls: visible_impls(db, file),
            pending: Vec::new(),
            pending_fields: Vec::new(),
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
            // `class Ord[T] where Eq[T]`: `Ord[X]` gives `Eq[X]`.
            let class_def = GenericDefId::Class(c.class);
            for sup in generic_predicates(self.db, class_def).predicates.iter() {
                stack.push(sup.map(|t| crate::lower::subst_params(t, class_def, &c.args)));
            }
            self.assumptions.push(c);
        }
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
        }
    }

    /// Solve a field constraint if the type is known well enough. Returns
    /// `false` if it has to wait.
    fn solve_field(&mut self, constraint: &FieldConstraint<'db>, origin: Origin<'db>) -> bool {
        let ty = self.unifier.shallow_resolve(&constraint.ty);
        let field = match &ty {
            Ty::Var(_) => return false,
            // An error was already reported, or the value is never made.
            Ty::Error | Ty::Never => {
                self.unifier.set_error(&constraint.field);
                return true;
            }
            Ty::Adt(AdtId::Struct(s), args) => {
                let fields = field_types(self.db, VariantDef::Struct(*s));
                fields.field(&constraint.name).map(|(_, t)| subst_params(t, GenericDefId::Struct(*s), args))
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

    pub fn equal(&mut self, expected: &Ty<'db>, actual: &Ty<'db>, origin: Origin<'db>) {
        self.add(Constraint::Equal { expected: expected.clone(), actual: actual.clone(), origin });
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
            for (constraint, origin) in std::mem::take(&mut self.pending) {
                let constraint = constraint.map(|t| self.unifier.resolve(t));
                match self.try_solve(&constraint) {
                    Step::Solved(subgoals) => {
                        progress = true;
                        self.pending.extend(subgoals.into_iter().map(|c| (c, origin)));
                    }
                    Step::NoInstance => self.errors.push(TypeError::NoInstance { origin, constraint }),
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

    /// Finish solving: default numeric variables, solve what can be, and
    /// report the rest as ambiguous.
    pub fn finish(&mut self) {
        self.solve();
        self.unifier.default_numeric_vars();
        self.solve();
        for (constraint, origin) in self.take_pending() {
            self.errors.push(TypeError::Ambiguous { origin, constraint });
        }
        for (_, origin) in self.take_pending_fields() {
            self.errors.push(TypeError::TypeNeeded { origin });
        }
        let errors = std::mem::take(&mut self.errors);
        self.errors = errors.into_iter().map(|e| self.resolve_error(e)).collect();
    }

    fn resolve_error(&self, error: TypeError<'db>) -> TypeError<'db> {
        let r = |t: &Ty<'db>| self.unifier.resolve(t);
        match error {
            TypeError::Mismatch { origin, expected, actual } => {
                TypeError::Mismatch { origin, expected: r(&expected), actual: r(&actual) }
            }
            TypeError::InfiniteType { origin, ty } => TypeError::InfiniteType { origin, ty: r(&ty) },
            TypeError::NoInstance { origin, constraint } => {
                TypeError::NoInstance { origin, constraint: constraint.map(r) }
            }
            TypeError::Ambiguous { origin, constraint } => TypeError::Ambiguous { origin, constraint: constraint.map(r) },
            TypeError::NoSuchField { origin, ty, name } => TypeError::NoSuchField { origin, ty: r(&ty), name },
            TypeError::TypeNeeded { origin } => TypeError::TypeNeeded { origin },
        }
    }

    fn try_solve(&self, goal: &ClassConstraint<'db>) -> Step<'db> {
        // An error type somewhere means an error was already reported.
        if goal.args.iter().any(|t| t.any(&mut |t| *t == Ty::Error)) {
            return Step::Solved(Vec::new());
        }
        if self.assumptions.contains(goal) {
            return Step::Solved(Vec::new());
        }
        let has_vars = goal.args.iter().any(|t| t.any(&mut |t| matches!(t, Ty::Var(_))));
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
                let subgoals = generic_predicates(self.db, def)
                    .predicates
                    .iter()
                    .map(|p| {
                        p.map(|t| {
                            t.fold(&mut |t| match t {
                                Ty::Param(p) if p.def == def => Some(subst.get(p).cloned().unwrap_or(Ty::Error)),
                                _ => None,
                            })
                        })
                    })
                    .collect();
                Step::Solved(subgoals)
            }
            None if maybe || has_vars => Step::Wait,
            None => Step::NoInstance,
        }
    }
}

enum Step<'db> {
    /// Solved, if these hold too.
    Solved(Vec<ClassConstraint<'db>>),
    NoInstance,
    /// The types aren't known well enough yet.
    Wait,
}

enum Match {
    Yes,
    No,
    /// It depends on variables that aren't decided yet.
    Maybe,
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
    let mut result = Match::Yes;
    for (p, t) in patterns.iter().zip(tys) {
        match match_ty(p, t, subst) {
            Match::No => return Match::No,
            Match::Maybe => result = Match::Maybe,
            Match::Yes => {}
        }
    }
    result
}

fn match_ty<'db>(pattern: &Ty<'db>, ty: &Ty<'db>, subst: &mut HashMap<GenericParamId<'db>, Ty<'db>>) -> Match {
    match (pattern, ty) {
        (Ty::Param(p), _) if matches!(p.def, GenericDefId::Impl(_)) => match subst.get(p) {
            Some(bound) if bound == ty => Match::Yes,
            Some(bound) if bound.any(&mut |t| matches!(t, Ty::Var(_))) || matches!(ty, Ty::Var(_)) => Match::Maybe,
            Some(_) => Match::No,
            None => {
                subst.insert(*p, ty.clone());
                Match::Yes
            }
        },
        (_, Ty::Var(_)) => Match::Maybe,
        (Ty::Adt(a, xs), Ty::Adt(b, ys)) if a == b => match_all(xs, ys, subst),
        (Ty::Tuple(xs), Ty::Tuple(ys)) => match_all(xs, ys, subst),
        (Ty::Array(x), Ty::Array(y)) => match_ty(x, y, subst),
        (Ty::Fn(xs, x), Ty::Fn(ys, y)) => match match_all(xs, ys, subst) {
            Match::No => Match::No,
            Match::Maybe => match match_ty(x, y, subst) {
                Match::No => Match::No,
                _ => Match::Maybe,
            },
            Match::Yes => match_ty(x, y, subst),
        },
        (x, y) if x == y => Match::Yes,
        _ => Match::No,
    }
}

/// The impls code in `file` can use: those of the file and of the files it
/// imports, directly or not.
#[salsa::tracked(returns(ref))]
pub fn visible_impls<'db>(db: &'db dyn Db, file: File) -> Vec<boltlang_hir_def::ids::ImplId<'db>> {
    let mut seen = vec![file];
    let mut stack = vec![file];
    let mut impls = Vec::new();
    while let Some(file) = stack.pop() {
        impls.extend(file_scope(db, file).scope.impls().iter().copied());
        let tree = item_tree(db, file);
        for item in &tree.top_level {
            let ModItem::Import(idx) = *item else { continue };
            let Some(source) = &tree.imports[idx.index()].source else { continue };
            if let Ok(imported) = resolve_import_path(db, file, source)
                && !seen.contains(&imported)
            {
                seen.push(imported);
                stack.push(imported);
            }
        }
    }
    impls
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

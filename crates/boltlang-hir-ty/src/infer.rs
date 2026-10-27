//! Inference of the types in a body: walks its expressions and patterns,
//! checking them against an expected type where one is known, and gives
//! the constraints it finds to the [`Solver`].

mod pat;

use std::collections::HashMap;

use boltlang_base_db::Db;
use boltlang_hir_def::{
    Name,
    arena::ArenaMap,
    body::{Body, DefWithBody, DefWithBodyId},
    builtin_type::{BuiltinInt, BuiltinType, BuiltinUint},
    hir::{
        ArithOp, BinaryOp, Binding, BindingId, Expr, ExprId, LabelId, Literal, Pat, PatId, Path, PathSegment,
        PrefixOp, RecordField, Statement, TypeRefId,
    },
    ids::{ClassId, ConstId, FunctionId, ImplId, ItemContainerId},
    signatures::function_signature,
    resolver::{GenericDefId, Resolver, TypeNs, TypeResolution, ValueNs, ValueResolution},
    scope::expr_scopes,
};

use crate::{
    lang::LangClass,
    lower::{
        ClassConstraint, FieldsKind, LowerDiagnostic, TyLoweringContext, VariantDef, const_ty, field_types,
        identity_args, struct_args, subst_params,
    },
    check::{InferenceCycle, function_scheme},
    scheme::{Scheme, generic_param_ids},
    solve::{Constraint, ExprOrPat, FieldConstraint, Origin, Reason, Solver, TypeError},
    ty::{AdtId, Mutability, Ty, VarKind},
};

/// The types found in a body, and what went wrong.
#[derive(Debug, Default, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct InferenceResult<'db> {
    pub expr_types: ArenaMap<Expr, Ty<'db>>,
    pub pat_types: ArenaMap<Pat, Ty<'db>>,
    pub binding_types: ArenaMap<Binding, Ty<'db>>,
    /// The function a call like `x.f(y)` calls, when it's `f(x, y)`.
    pub method_calls: ArenaMap<Expr, FunctionId<'db>>,
    /// The impl that a use of a class method or constant uses, by the
    /// expression that names it and the class, when the types decide it.
    /// In a generic function, like `fn f(x) = show(x)`, they don't.
    pub instances: Vec<(ExprId, ClassId<'db>, ImplId<'db>)>,
    pub diagnostics: Vec<InferenceDiagnostic<'db>>,
}

impl<'db> InferenceResult<'db> {
    pub fn expr_ty(&self, expr: ExprId) -> &Ty<'db> {
        self.expr_types.get(expr).unwrap_or(&Ty::Error)
    }

    pub fn pat_ty(&self, pat: PatId) -> &Ty<'db> {
        self.pat_types.get(pat).unwrap_or(&Ty::Error)
    }

    pub fn binding_ty(&self, binding: BindingId) -> &Ty<'db> {
        self.binding_types.get(binding).unwrap_or(&Ty::Error)
    }

    /// The impl of `class` that the expression `expr` uses, if the types
    /// decide it.
    pub fn instance(&self, expr: ExprId, class: ClassId<'db>) -> Option<ImplId<'db>> {
        self.instances.iter().find(|(e, c, _)| *e == expr && *c == class).map(|(_, _, imp)| *imp)
    }

    /// Keep the impls that `solver` chose for uses in the body `def`.
    pub(crate) fn add_instances(&mut self, def: DefWithBodyId<'db>, solver: &Solver<'db>) {
        for (origin, class, imp) in &solver.chosen {
            if origin.body == def
                && let ExprOrPat::Expr(expr) = origin.node
            {
                self.instances.push((expr, *class, *imp));
            }
        }
    }

    /// Report the expressions and patterns whose type has a `mut` array or
    /// struct inside an `imm` one, like `[mut [1]]`: `imm` is deep, so what
    /// an `imm` value holds can't be changed either. Each such type is
    /// reported once, where it first appears, once the types are resolved.
    pub(crate) fn check_deep_immutability(&mut self) {
        let mut seen = Vec::new();
        let exprs = self.expr_types.iter().map(|(e, t)| (ExprOrPat::Expr(e), t));
        let pats = self.pat_types.iter().map(|(p, t)| (ExprOrPat::Pat(p), t));
        for (at, ty) in exprs.chain(pats) {
            if let Some((outer, inner)) = mut_inside_imm(ty)
                && !seen.contains(&outer)
            {
                seen.push(outer.clone());
                self.diagnostics.push(InferenceDiagnostic::MutInsideImm { at, ty: outer, inner });
            }
        }
    }

    /// Replace the types with `f` of them, once inference is done.
    pub(crate) fn map_types(&mut self, f: &mut impl FnMut(&Ty<'db>) -> Ty<'db>) {
        for ty in self.expr_types.values_mut().chain(self.pat_types.values_mut()).chain(self.binding_types.values_mut()) {
            *ty = f(ty);
        }
        for diagnostic in &mut self.diagnostics {
            diagnostic.map_types(f);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub enum InferenceDiagnostic<'db> {
    Type(TypeError<'db>),
    /// A type reference in the body, like that of a `let`.
    Lower(LowerDiagnostic),
    UnresolvedName { expr: ExprOrPat },
    NotAFunction { expr: ExprId, ty: Ty<'db> },
    WrongArgCount { expr: ExprOrPat, expected: usize, found: usize },
    NoSuchField { expr: ExprOrPat, name: Name },
    MissingFields { expr: ExprId, names: Vec<Name> },
    /// A label or loop to break out of that doesn't exist.
    BreakOutside { expr: ExprId },
    /// An assignment to a variable, or to a field of one, that isn't `mut`.
    /// `expr` is the left side.
    AssignToImmutable { expr: ExprId, binding: BindingId },
    /// `mut` before something other than an array or a struct, like the
    /// call of a function or an enum's variant: `mut f(x)`.
    NoMutability { expr: ExprId },
    /// A type `ty` with a `mut` array or struct, `inner`, inside an `imm`
    /// one, which can't hold it: `imm` is deep.
    MutInsideImm { at: ExprOrPat, ty: Ty<'db>, inner: Ty<'db> },
    /// An assignment whose left side isn't a variable, a field or an
    /// element, like `1 = 2` or `f() = 3`. `expr` is the left side.
    NotAPlace { expr: ExprId },
    /// Something the checker doesn't support yet, like `?` or ranges.
    Unsupported { expr: ExprId },
    /// Syntax that needs a class of the prelude, like `xs[i]` needs
    /// `Index`, without a prelude that defines it.
    NoLangClass { expr: ExprId, class: &'static str },
    /// Functions without complete signatures in files that import each
    /// other call each other; one of them needs a signature.
    CyclicInference,
}

impl<'db> InferenceDiagnostic<'db> {
    pub(crate) fn map_types(&mut self, f: &mut impl FnMut(&Ty<'db>) -> Ty<'db>) {
        match self {
            InferenceDiagnostic::Type(error) => error.map_types(f),
            InferenceDiagnostic::NotAFunction { ty, .. } => *ty = f(ty),
            InferenceDiagnostic::MutInsideImm { ty, inner, .. } => {
                *ty = f(ty);
                *inner = f(inner);
            }
            _ => {}
        }
    }
}

/// The types of the functions being inferred together, which aren't
/// available from queries yet.
#[derive(Default)]
pub(crate) struct FnEnv<'db> {
    /// The functions of the group being inferred, not generalized yet.
    pub mono: HashMap<FunctionId<'db>, Ty<'db>>,
    /// Functions of the same file inferred before this group.
    pub done: HashMap<FunctionId<'db>, Scheme<'db>>,
}

/// The type an expression is expected to have, and why.
#[derive(Clone, Copy)]
struct Expectation<'a, 'db> {
    ty: &'a Ty<'db>,
    reason: Reason<'db>,
}

/// The first `mut` array or struct in `ty` that an `imm` one holds, with
/// the `imm` one. Functions aren't looked into: a function's type says what
/// it takes, not what it holds.
pub fn mut_inside_imm<'db>(ty: &Ty<'db>) -> Option<(Ty<'db>, Ty<'db>)> {
    fn has(ty: &Ty<'_>, mutability: Mutability) -> bool {
        ty.mutability() == Some(&Ty::Mutability(mutability))
    }
    fn find_mut<'db>(ty: &Ty<'db>) -> Option<Ty<'db>> {
        if has(ty, Mutability::Mut) {
            return Some(ty.clone());
        }
        let mut found = None;
        if !matches!(ty, Ty::Fn(..)) {
            ty.walk_children(|t| found = found.take().or_else(|| find_mut(t)));
        }
        found
    }
    if matches!(ty, Ty::Fn(..)) {
        return None;
    }
    if has(ty, Mutability::Imm) {
        let mut found = None;
        ty.walk_children(|t| found = found.take().or_else(|| find_mut(t)));
        if let Some(inner) = found {
            return Some((ty.clone(), inner));
        }
    }
    let mut found = None;
    ty.walk_children(|t| found = found.take().or_else(|| mut_inside_imm(t)));
    found
}

/// The mutability of an array literal: `Mut` for `mut [a, b]`, and `Imm`
/// for `[a, b]`.
fn literal_mutability<'db>(is_mut: bool) -> Ty<'db> {
    Ty::Mutability(if is_mut { Mutability::Mut } else { Mutability::Imm })
}

/// The reason for the `index`th of several expressions that must have the
/// same type, like the arms of a `match`: the reason they all have, or else
/// the first of them.
fn reason_for_nth<'db>(reason: Reason<'db>, first: ExprId, index: usize) -> Reason<'db> {
    match reason {
        Reason::None if index > 0 => Reason::SameAs(first),
        reason => reason,
    }
}

struct Breakable<'db> {
    label: Option<LabelId>,
    /// Whether it's a loop, which `break` without a label leaves.
    is_loop: bool,
    /// The type of the value `break` gives, if it can give one.
    ty: Option<Ty<'db>>,
    broken: bool,
}

pub(crate) struct InferenceContext<'a, 'db> {
    pub db: &'db dyn Db,
    pub def: DefWithBodyId<'db>,
    pub body: &'db Body,
    pub solver: &'a mut Solver<'db>,
    env: &'a FnEnv<'db>,
    pub result: InferenceResult<'db>,
    return_ty: Ty<'db>,
    /// Why values returned must have `return_ty`.
    return_reason: Reason<'db>,
    breakables: Vec<Breakable<'db>>,
    /// Variables declared by a `let` without a value, like `let x;`, that
    /// haven't been assigned yet. The first `x = ...` initializes them, so
    /// it's allowed even if they aren't `mut`.
    uninitialized: Vec<BindingId>,
    /// Whether the callee being inferred is that of `mut Wrapper(x)`, so
    /// that the tuple struct it makes is `mut`. Taken by the struct's
    /// constructor; if nothing takes it, `mut` was before something else.
    mut_constructor: bool,
}

impl<'a, 'db> InferenceContext<'a, 'db> {
    pub fn new(db: &'db dyn Db, def: DefWithBodyId<'db>, solver: &'a mut Solver<'db>, env: &'a FnEnv<'db>) -> Self {
        let body = boltlang_hir_def::body::body(db, def);
        InferenceContext {
            db,
            def,
            body,
            solver,
            env,
            result: InferenceResult::default(),
            return_ty: Ty::Error,
            return_reason: Reason::None,
            breakables: Vec::new(),
            uninitialized: Vec::new(),
            mut_constructor: false,
        }
    }

    fn origin(&self, expr: ExprId) -> Origin<'db> {
        Origin::new(self.def, ExprOrPat::Expr(expr))
    }

    fn new_var(&mut self) -> Ty<'db> {
        self.solver.new_var(VarKind::General)
    }

    /// A variable for the mutability of an array.
    fn new_mutability(&mut self) -> Ty<'db> {
        self.solver.new_var(VarKind::Mutability)
    }

    /// The expected type, or a new variable when nothing is expected.
    fn expected_or_var(&mut self, expected: Option<Expectation<'_, 'db>>) -> Ty<'db> {
        expected.map(|e| e.ty.clone()).unwrap_or_else(|| self.new_var())
    }

    pub(crate) fn unresolved(&mut self, expr: ExprOrPat) {
        self.result.diagnostics.push(InferenceDiagnostic::UnresolvedName { expr });
    }

    /// The type of `def`'s struct or enum, with new variables for its type
    /// arguments and, for a struct, the mutability `mutability`. Also gives
    /// what to substitute in its field types: the arguments, then for a
    /// struct its mutability (see [`struct_args`]).
    pub(crate) fn fresh_adt(&mut self, def: VariantDef<'db>, mutability: Ty<'db>) -> (Ty<'db>, Box<[Ty<'db>]>) {
        let (adt, generic_def) = def.adt();
        let count = generic_def.generic_params(self.db).type_params.len();
        let args: Vec<Ty<'db>> = (0..count).map(|_| self.new_var()).collect();
        let ty = Ty::adt(adt, args.clone(), mutability);
        let subst = match &ty {
            Ty::Adt(AdtId::Struct(_), args, mutability) => struct_args(args, mutability),
            _ => args,
        };
        (ty, subst.into())
    }

    fn resolver(&self, expr: ExprId) -> Resolver<'db> {
        Resolver::for_expr(self.db, self.def, expr)
    }

    fn lower_ty(&mut self, expr: ExprId, type_ref: TypeRefId) -> Ty<'db> {
        let resolver = self.resolver(expr);
        let mut ctx = TyLoweringContext::new(self.db, &resolver, &self.body.store);
        // A body can use the named mutabilities of its function.
        if let DefWithBody::Function(f) = self.def.def(self.db) {
            ctx = ctx.with_mutabilities(GenericDefId::Function(f));
        }
        let ty = ctx.lower_ty(type_ref);
        self.result.diagnostics.extend(ctx.diagnostics.into_iter().map(InferenceDiagnostic::Lower));
        // An array without `mut` or `imm` in a body can be either, which
        // the value decides.
        let ty = ty.fill_unwritten(&mut || self.solver.new_var(VarKind::Mutability));
        let origin = self.origin(expr);
        self.solver.flatten_assoc(&ty, origin)
    }

    /// Infer a function body with the given parameter and return types.
    pub fn infer_fn_body(&mut self, params: &[Ty<'db>], ret: &Ty<'db>) {
        // The associated types in the signature, like `C.Item`, are decided
        // by the solver, which knows the `where` clauses and impls.
        let origin = self.origin(self.body.body_expr);
        let ret = &self.solver.flatten_assoc(ret, origin);
        let mut params = params.iter();
        for &pat in self.body.params.iter() {
            let ty = params.next().cloned().unwrap_or(Ty::Error);
            let ty = self.solver.flatten_assoc(&ty, origin);
            self.check_pat(pat, &ty);
        }
        self.return_ty = ret.clone();
        self.return_reason = match self.def.def(self.db) {
            DefWithBody::Function(f) if function_signature(self.db, f).ret_type.is_some() => Reason::ReturnType,
            _ => Reason::None,
        };
        let body_expr = self.body.body_expr;
        self.check_expr_because(body_expr, ret, self.return_reason);
    }

    /// Infer a body that's an expression, like that of a const.
    pub fn infer_expr_body(&mut self, expected: Option<&Ty<'db>>) -> Ty<'db> {
        let body_expr = self.body.body_expr;
        let reason = match self.def.def(self.db) {
            DefWithBody::Const(_) => Reason::ConstType,
            _ => Reason::None,
        };
        match expected {
            Some(ty) => {
                self.check_expr_because(body_expr, ty, reason);
                ty.clone()
            }
            None => self.infer_expr(body_expr),
        }
    }

    pub fn check_expr(&mut self, expr: ExprId, expected: &Ty<'db>) {
        self.check_expr_because(expr, expected, Reason::None);
    }

    fn check_expr_because(&mut self, expr: ExprId, expected: &Ty<'db>, reason: Reason<'db>) {
        let actual = self.infer_expr_expect(expr, Some(Expectation { ty: expected, reason }));
        self.solver.equal(expected, &actual, Origin { reason, ..self.origin(expr) });
    }

    pub fn infer_expr(&mut self, expr: ExprId) -> Ty<'db> {
        self.infer_expr_expect(expr, None)
    }

    /// Infer the type of an expression. `expected` is a hint, for branches
    /// and lambdas; the caller checks the result against it.
    fn infer_expr_expect(&mut self, expr: ExprId, expected: Option<Expectation<'_, 'db>>) -> Ty<'db> {
        let ty = self.infer_expr_inner(expr, expected);
        self.result.expr_types.insert(expr, ty.clone());
        ty
    }

    fn infer_expr_inner(&mut self, expr: ExprId, expected: Option<Expectation<'_, 'db>>) -> Ty<'db> {
        let body = self.body;
        let reason = expected.map_or(Reason::None, |e| e.reason);
        match &body[expr] {
            Expr::Missing => Ty::Error,
            Expr::Path(path) => self.infer_path(expr, path),
            Expr::Literal(lit) => self.literal_ty(lit),
            Expr::Block { statements, tail, label, .. } => {
                if let Some(label) = label {
                    let ty = self.expected_or_var(expected);
                    self.breakables.push(Breakable { label: Some(*label), is_loop: false, ty: Some(ty.clone()), broken: false });
                    let actual = self.infer_block(statements, *tail, Some(Expectation { ty: &ty, reason }));
                    self.breakables.pop();
                    self.solver.equal(&ty, &actual, self.origin(expr));
                    ty
                } else {
                    self.infer_block(statements, *tail, expected)
                }
            }
            Expr::Call { callee, args, is_mut } => self.infer_call(expr, *callee, args, *is_mut),
            Expr::Field { expr: base, name } => self.infer_field(expr, *base, name),
            Expr::Await { expr: inner } | Expr::Try { expr: inner } => {
                self.infer_expr(*inner);
                self.result.diagnostics.push(InferenceDiagnostic::Unsupported { expr });
                Ty::Error
            }
            Expr::Cast { expr: inner, type_ref } => {
                self.infer_expr(*inner);
                self.lower_ty(expr, *type_ref)
            }
            Expr::Index { base, index } => {
                // Through the impl of `Index` for the type, arrays too.
                let base_ty = self.infer_expr(*base);
                self.check_expr(*index, &Ty::Builtin(BuiltinType::Int(BuiltinInt::I32)));
                self.lang_assoc(LangClass::Index, &base_ty, expr).unwrap_or(Ty::Error)
            }
            Expr::Prefix { op, expr: inner } => {
                let ty = self.infer_expr(*inner);
                if *op == PrefixOp::Not && matches!(self.solver.unifier.shallow_resolve(&ty), Ty::Var(_)) {
                    // `!x` of an unknown type is most likely a boolean.
                    self.solver.equal(&Ty::bool(), &ty, self.origin(*inner));
                }
                ty
            }
            Expr::Binary { op, lhs, rhs } => self.infer_binary(expr, op, *lhs, *rhs),
            Expr::Range { lhs, rhs, .. } => {
                for e in lhs.iter().chain(rhs.iter()) {
                    self.infer_expr(*e);
                }
                self.result.diagnostics.push(InferenceDiagnostic::Unsupported { expr });
                Ty::Error
            }
            Expr::Tuple { exprs } => {
                let hints: Option<Vec<Ty<'db>>> = match expected.map(|e| self.solver.unifier.shallow_resolve(e.ty)) {
                    Some(Ty::Tuple(tys)) if tys.len() == exprs.len() => Some(tys.into_vec()),
                    _ => None,
                };
                let tys = exprs
                    .iter()
                    .enumerate()
                    .map(|(i, e)| self.infer_expr_expect(*e, hints.as_ref().map(|h| Expectation { ty: &h[i], reason })))
                    .collect();
                Ty::Tuple(tys)
            }
            Expr::Array { exprs, is_mut } => {
                let elem = match expected.map(|e| self.solver.unifier.shallow_resolve(e.ty)) {
                    Some(Ty::Array(_, elem)) => *elem,
                    _ => self.new_var(),
                };
                for (i, e) in exprs.iter().enumerate() {
                    self.check_expr_because(*e, &elem, reason_for_nth(reason, exprs[0], i));
                }
                Ty::array(literal_mutability(*is_mut), elem)
            }
            Expr::ArrayRepeat { value, count, is_mut } => {
                let elem = self.infer_expr(*value);
                self.check_expr(*count, &Ty::Builtin(BuiltinType::Uint(BuiltinUint::Usize)));
                Ty::array(literal_mutability(*is_mut), elem)
            }
            Expr::Record { path, fields, spread, is_mut } => {
                self.infer_record(expr, path.as_ref(), fields, *spread, *is_mut)
            }
            Expr::If { condition, then_branch, else_branch } => {
                self.check_expr(*condition, &Ty::bool());
                match else_branch {
                    Some(else_branch) => {
                        let ty = self.expected_or_var(expected);
                        self.check_expr_because(*then_branch, &ty, reason);
                        self.check_expr_because(*else_branch, &ty, reason_for_nth(reason, *then_branch, 1));
                        ty
                    }
                    None => {
                        self.check_expr(*then_branch, &Ty::unit());
                        Ty::unit()
                    }
                }
            }
            Expr::Let { pat, expr: value } => {
                let ty = self.infer_expr(*value);
                self.check_pat(*pat, &ty);
                Ty::bool()
            }
            Expr::Match { expr: scrutinee, arms } => {
                let scrutinee_ty = self.infer_expr(*scrutinee);
                let ty = self.expected_or_var(expected);
                for (i, arm) in arms.iter().enumerate() {
                    self.check_pat(arm.pat, &scrutinee_ty);
                    if let Some(guard) = arm.guard {
                        self.check_expr(guard, &Ty::bool());
                    }
                    self.check_expr_because(arm.expr, &ty, reason_for_nth(reason, arms[0].expr, i));
                }
                if arms.is_empty() { Ty::Never } else { ty }
            }
            Expr::While { condition, body: loop_body, label } => {
                self.breakables.push(Breakable { label: *label, is_loop: true, ty: None, broken: false });
                self.check_expr(*condition, &Ty::bool());
                self.check_expr(*loop_body, &Ty::unit());
                self.breakables.pop();
                Ty::unit()
            }
            Expr::Loop { body: loop_body, label } => {
                let ty = self.expected_or_var(expected);
                self.breakables.push(Breakable { label: *label, is_loop: true, ty: Some(ty.clone()), broken: false });
                self.check_expr(*loop_body, &Ty::unit());
                let breakable = self.breakables.pop().unwrap();
                // A loop that's never left doesn't give a value.
                if breakable.broken { ty } else { Ty::Never }
            }
            Expr::For { pat, iterable, body: loop_body, label } => {
                // Through the impl of `Iterable` for the type, arrays too,
                // whose iterator gives the items with `Iterator`.
                let iterable_ty = self.infer_expr(*iterable);
                let elem = self
                    .lang_assoc(LangClass::Iterable, &iterable_ty, *iterable)
                    .and_then(|iter| self.lang_assoc(LangClass::Iterator, &iter, expr))
                    .unwrap_or(Ty::Error);
                self.check_pat(*pat, &elem);
                self.breakables.push(Breakable { label: *label, is_loop: true, ty: None, broken: false });
                self.check_expr(*loop_body, &Ty::unit());
                self.breakables.pop();
                Ty::unit()
            }
            Expr::Break { expr: value, label } => {
                let target = match label {
                    Some(name) => {
                        let scopes = expr_scopes(self.db, self.def);
                        let id = scopes.scope_for(expr).and_then(|s| scopes.resolve_label(s, Some(name)));
                        self.breakables.iter().rposition(|b| b.label.is_some() && b.label == id)
                    }
                    None => self.breakables.iter().rposition(|b| b.is_loop),
                };
                match target {
                    Some(i) => {
                        self.breakables[i].broken = true;
                        let ty = self.breakables[i].ty.clone().unwrap_or_else(Ty::unit);
                        match value {
                            Some(value) => self.check_expr(*value, &ty),
                            None => self.solver.equal(&ty, &Ty::unit(), self.origin(expr)),
                        }
                    }
                    None => {
                        if let Some(value) = value {
                            self.infer_expr(*value);
                        }
                        self.result.diagnostics.push(InferenceDiagnostic::BreakOutside { expr });
                    }
                }
                Ty::Never
            }
            Expr::Continue { .. } => Ty::Never,
            Expr::Return { expr: value } => {
                let ret = self.return_ty.clone();
                match value {
                    Some(value) => self.check_expr_because(*value, &ret, self.return_reason),
                    None => self.solver.equal(&ret, &Ty::unit(), Origin { reason: self.return_reason, ..self.origin(expr) }),
                }
                Ty::Never
            }
            Expr::Yield { expr: value } => {
                if let Some(value) = value {
                    self.infer_expr(*value);
                }
                self.result.diagnostics.push(InferenceDiagnostic::Unsupported { expr });
                Ty::unit()
            }
            Expr::Lambda { params, param_types, ret_type, body: lambda_body, .. } => {
                let (hint_params, hint_ret) = match expected.map(|e| self.solver.unifier.shallow_resolve(e.ty)) {
                    Some(Ty::Fn(ps, r)) if ps.len() == params.len() => (Some(ps), Some(*r)),
                    _ => (None, None),
                };
                let mut param_tys = Vec::new();
                for (i, (&pat, type_ref)) in params.iter().zip(param_types.iter()).enumerate() {
                    let ty = match (type_ref, &hint_params) {
                        (Some(type_ref), _) => self.lower_ty(expr, *type_ref),
                        (None, Some(hints)) => hints[i].clone(),
                        (None, None) => self.new_var(),
                    };
                    self.check_pat(pat, &ty);
                    param_tys.push(ty);
                }
                let ret = match (ret_type, hint_ret) {
                    (Some(type_ref), _) => self.lower_ty(expr, *type_ref),
                    (None, Some(hint)) => hint,
                    (None, None) => self.new_var(),
                };
                let ret_reason = ret_type.map_or(Reason::None, Reason::Annotation);
                let outer_return = std::mem::replace(&mut self.return_ty, ret.clone());
                let outer_reason = std::mem::replace(&mut self.return_reason, ret_reason);
                let outer_breakables = std::mem::take(&mut self.breakables);
                self.check_expr_because(*lambda_body, &ret, ret_reason);
                self.breakables = outer_breakables;
                self.return_reason = outer_reason;
                self.return_ty = outer_return;
                Ty::Fn(param_tys.into(), Box::new(ret))
            }
            Expr::MacroCall { .. } => Ty::Error,
        }
    }

    fn infer_block(
        &mut self,
        statements: &[Statement],
        tail: Option<ExprId>,
        expected: Option<Expectation<'_, 'db>>,
    ) -> Ty<'db> {
        let mut diverges = false;
        for statement in statements {
            match statement {
                Statement::Let { pat, type_ref, initializer, else_branch } => {
                    let ty = match type_ref {
                        Some(type_ref) => {
                            let at = initializer.unwrap_or(self.body.body_expr);
                            self.lower_ty(at, *type_ref)
                        }
                        None => self.new_var(),
                    };
                    if let Some(init) = initializer {
                        self.check_expr_because(*init, &ty, type_ref.map_or(Reason::None, Reason::Annotation));
                    } else if let Pat::Bind { id, subpat: None } = &self.body[*pat] {
                        self.uninitialized.push(*id);
                    }
                    if let Some(else_branch) = else_branch {
                        self.check_expr(*else_branch, &Ty::Never);
                    }
                    self.check_pat(*pat, &ty);
                }
                Statement::Expr { expr, has_semi } => {
                    let ty = self.infer_expr(*expr);
                    if matches!(self.solver.unifier.shallow_resolve(&ty), Ty::Never) {
                        diverges = true;
                    } else if !has_semi {
                        // A block-like expression used as a statement.
                        self.solver.equal(&Ty::unit(), &ty, self.origin(*expr));
                    }
                }
                Statement::Item => {}
            }
        }
        match (tail, expected) {
            // Check the last expression itself, so that errors point at it.
            (Some(tail), Some(expected)) => {
                self.check_expr_because(tail, expected.ty, expected.reason);
                expected.ty.clone()
            }
            (Some(tail), None) => self.infer_expr(tail),
            (None, _) if diverges => Ty::Never,
            (None, _) => Ty::unit(),
        }
    }

    fn literal_ty(&mut self, lit: &Literal) -> Ty<'db> {
        let suffixed = |suffix: &Option<Name>| suffix.as_ref().and_then(|s| BuiltinType::by_name(s.as_str())).map(Ty::Builtin);
        match lit {
            Literal::Int(_, suffix) => suffixed(suffix).unwrap_or_else(|| self.solver.new_var(VarKind::Int)),
            Literal::Float(_, suffix) => suffixed(suffix).unwrap_or_else(|| self.solver.new_var(VarKind::Float)),
            Literal::Char(_) => Ty::Builtin(BuiltinType::Char),
            Literal::Byte(_) => Ty::Builtin(BuiltinType::Uint(BuiltinUint::U8)),
            Literal::String(_) | Literal::CString(_) => Ty::Builtin(BuiltinType::String),
            Literal::ByteString(_) => Ty::array(literal_mutability(false), Ty::Builtin(BuiltinType::Uint(BuiltinUint::U8))),
        }
    }

    /// The type of a function, instantiated for this use.
    fn function_ty(&mut self, expr: ExprId, f: FunctionId<'db>) -> Ty<'db> {
        if let Some(ty) = self.env.mono.get(&f) {
            return ty.clone();
        }
        let origin = self.origin(expr);
        if let Some(scheme) = self.env.done.get(&f) {
            return scheme.instantiate(self.solver, origin);
        }
        match function_scheme(self.db, f) {
            Ok(scheme) => scheme.instantiate(self.solver, origin),
            Err(InferenceCycle) => {
                self.result.diagnostics.push(InferenceDiagnostic::CyclicInference);
                Ty::Error
            }
        }
    }

    fn infer_path(&mut self, expr: ExprId, path: &Path) -> Ty<'db> {
        let resolver = self.resolver(expr);
        match resolver.resolve_path_in_value_ns(self.db, path) {
            Some(ValueResolution::Value(value)) => self.value_ty(expr, value),
            Some(ValueResolution::Type(resolution)) => self.type_as_value(expr, resolution),
            None => {
                self.unresolved(ExprOrPat::Expr(expr));
                Ty::Error
            }
        }
    }

    fn value_ty(&mut self, expr: ExprId, value: ValueNs<'db>) -> Ty<'db> {
        match value {
            ValueNs::Local(binding) => self.result.binding_types.get(binding).cloned().unwrap_or(Ty::Error),
            ValueNs::Function(f) => self.function_ty(expr, f),
            ValueNs::Const(c) => self.const_value_ty(expr, c),
            // Const generics aren't supported yet.
            ValueNs::ConstParam { .. } => Ty::Error,
            ValueNs::Variant(v) => self.constructor_ty(VariantDef::Variant(v), false),
            ValueNs::Builtin(v) => Ty::Builtin(v.ty()),
        }
    }

    /// The type of a constant used as a value. A class constant, like the
    /// `ZERO` of `class Zero[T] { const ZERO: T; }`, is generic in the
    /// class's parameters and needs an instance, like a class method.
    fn const_value_ty(&mut self, expr: ExprId, c: ConstId<'db>) -> Ty<'db> {
        let ty = const_ty(self.db, c).ty.clone();
        let ItemContainerId::Class(class) = c.container(self.db) else { return ty };
        let def = GenericDefId::Class(class);
        let scheme = Scheme {
            params: generic_param_ids(self.db, [def]),
            bound: 0,
            predicates: Box::new([ClassConstraint { class, args: identity_args(self.db, def) }]),
            fields: Box::new([]),
            assoc: Box::new([]),
            ty,
        };
        scheme.instantiate(self.solver, self.origin(expr))
    }

    /// The type of a struct or variant used as a value: a function for a
    /// tuple struct, the type itself for a unit one. A struct is `imm`, or
    /// `mut` when it's made by `mut Wrapper(x)`.
    fn constructor_ty(&mut self, def: VariantDef<'db>, is_mut: bool) -> Ty<'db> {
        let (_, generic_def) = def.adt();
        let (adt_ty, args) = self.fresh_adt(def, literal_mutability(is_mut));
        let fields = field_types(self.db, def);
        match fields.kind {
            FieldsKind::Unit => adt_ty,
            FieldsKind::Tuple => {
                let params = fields.fields.iter().map(|(_, t)| subst_params(t, generic_def, &args)).collect();
                Ty::Fn(params, Box::new(adt_ty))
            }
            // A record struct can't be used as a value.
            FieldsKind::Record => Ty::Error,
        }
    }

    /// A path that names a type used as a value: a constructor.
    fn type_as_value(&mut self, expr: ExprId, resolution: TypeResolution<'db>) -> Ty<'db> {
        match (resolution.ty, resolution.unresolved) {
            (TypeNs::Struct(s), None) => {
                let is_mut = std::mem::take(&mut self.mut_constructor);
                self.constructor_ty(VariantDef::Struct(s), is_mut)
            }
            (TypeNs::Variant(v), None) => self.constructor_ty(VariantDef::Variant(v), false),
            _ => {
                self.unresolved(ExprOrPat::Expr(expr));
                Ty::Error
            }
        }
    }

    /// Whether `expr` is a path that doesn't name a value, like `Color` or
    /// a module, so that `expr.name` is a longer path.
    fn path_extended(&self, expr: ExprId, name: &Name) -> Option<Path> {
        let Expr::Path(path) = &self.body[expr] else { return None };
        let resolver = self.resolver(expr);
        if let Some(ValueResolution::Value(_)) = resolver.resolve_path_in_value_ns(self.db, path) {
            return None;
        }
        let mut segments = path.segments.to_vec();
        segments.push(PathSegment { name: name.clone(), generic_args: None });
        Some(Path { segments: segments.into() })
    }

    fn infer_field(&mut self, expr: ExprId, base: ExprId, name: &Name) -> Ty<'db> {
        if let Some(path) = self.path_extended(base, name) {
            return self.infer_path(expr, &path);
        }
        let base_ty = self.infer_expr(base);
        // The solver finds the field once the type is known; until then, a
        // variable stands for the field's type.
        let field = self.new_var();
        let constraint = FieldConstraint { ty: base_ty, name: name.clone(), field: field.clone(), assign: false };
        self.solver.add(Constraint::Field { constraint, origin: self.origin(expr) });
        field
    }

    fn infer_call(&mut self, expr: ExprId, callee: ExprId, args: &[ExprId], is_mut: bool) -> Ty<'db> {
        // `x.f(y)`: a field holding a function, a longer path, or `f(x, y)`.
        if let Expr::Field { expr: receiver, name } = &self.body[callee] {
            let (receiver, name) = (*receiver, name.clone());
            if self.path_extended(receiver, &name).is_none() {
                let receiver_ty = self.infer_expr(receiver);
                let is_field = match self.solver.unifier.shallow_resolve(&receiver_ty) {
                    Ty::Adt(AdtId::Struct(s), ..) => field_types(self.db, VariantDef::Struct(s)).field(&name).is_some(),
                    _ => false,
                };
                if !is_field {
                    if is_mut {
                        self.result.diagnostics.push(InferenceDiagnostic::NoMutability { expr });
                    }
                    return self.infer_method_call(expr, callee, receiver, &receiver_ty, &name, args);
                }
            }
        }
        // A tuple struct's constructor takes `mut_constructor`.
        self.mut_constructor = is_mut;
        let callee_ty = self.infer_expr(callee);
        if std::mem::take(&mut self.mut_constructor) {
            self.result.diagnostics.push(InferenceDiagnostic::NoMutability { expr });
        }
        let function = match &self.body[callee] {
            Expr::Path(path) => match self.resolver(callee).resolve_path_in_value_ns(self.db, path) {
                Some(ValueResolution::Value(ValueNs::Function(f))) => Some(f),
                _ => None,
            },
            _ => None,
        };
        self.apply(expr, callee, &callee_ty, function, args, None)
    }

    /// `x.f(y)` as `f(x, y)`, with the `f` in scope. Functions for a type are
    /// found like any other: by importing them.
    fn infer_method_call(
        &mut self,
        expr: ExprId,
        callee: ExprId,
        receiver: ExprId,
        receiver_ty: &Ty<'db>,
        name: &Name,
        args: &[ExprId],
    ) -> Ty<'db> {
        let path = Path::from_name(name.clone());
        let mut function = None;
        let callee_ty = match self.resolver(expr).resolve_path_in_value_ns(self.db, &path) {
            Some(ValueResolution::Value(ValueNs::Function(f))) => {
                function = Some(f);
                self.result.method_calls.insert(expr, f);
                self.function_ty(callee, f)
            }
            // A local holding a function works too.
            Some(ValueResolution::Value(value)) => self.value_ty(callee, value),
            _ => {
                if !matches!(self.solver.unifier.shallow_resolve(receiver_ty), Ty::Error) {
                    self.unresolved(ExprOrPat::Expr(callee));
                }
                Ty::Error
            }
        };
        self.result.expr_types.insert(callee, callee_ty.clone());
        self.apply(expr, callee, &callee_ty, function, args, Some((receiver, receiver_ty)))
    }

    /// Call a value of type `callee_ty` with `args`, after `receiver` if
    /// there's one. `function` is the function it is, if it's known.
    fn apply(
        &mut self,
        expr: ExprId,
        callee: ExprId,
        callee_ty: &Ty<'db>,
        function: Option<FunctionId<'db>>,
        args: &[ExprId],
        receiver: Option<(ExprId, &Ty<'db>)>,
    ) -> Ty<'db> {
        let count = args.len() + receiver.is_some() as usize;
        let (params, ret) = match self.solver.unifier.shallow_resolve(callee_ty) {
            Ty::Fn(params, ret) => (params, *ret),
            // A number literal's type isn't known yet, but it's no function.
            Ty::Var(v) if self.solver.unifier.var_kind(v) == Some(VarKind::General) => {
                let params: Box<[Ty<'db>]> = (0..count).map(|_| self.new_var()).collect();
                let ret = self.new_var();
                let fn_ty = Ty::Fn(params.clone(), Box::new(ret.clone()));
                self.solver.equal(&fn_ty, callee_ty, self.origin(callee));
                (params, ret)
            }
            Ty::Error | Ty::Never => {
                for &arg in args {
                    self.infer_expr(arg);
                }
                return Ty::Error;
            }
            other => {
                self.result.diagnostics.push(InferenceDiagnostic::NotAFunction { expr: callee, ty: other });
                for &arg in args {
                    self.infer_expr(arg);
                }
                return Ty::Error;
            }
        };
        if params.len() != count {
            self.result.diagnostics.push(InferenceDiagnostic::WrongArgCount {
                expr: ExprOrPat::Expr(expr),
                expected: params.len(),
                found: count,
            });
        }
        let reason = |index: usize| match function {
            Some(function) => Reason::Argument { function, index: index as u32 },
            None => Reason::None,
        };
        let mut params = params.iter();
        if let Some((receiver, receiver_ty)) = receiver
            && let Some(param) = params.next()
        {
            self.solver.equal(param, receiver_ty, Origin { reason: reason(0), ..self.origin(receiver) });
        }
        let first = receiver.is_some() as usize;
        for (i, &arg) in args.iter().enumerate() {
            match params.next() {
                Some(param) => self.check_expr_because(arg, param, reason(first + i)),
                None => {
                    self.infer_expr(arg);
                }
            }
        }
        // The arguments may decide associated types in the result, like the
        // `imm [i32]` of `to_imm(mut [1])`, which what follows should know.
        self.solver.solve_pending_assoc();
        ret
    }

    fn infer_binary(&mut self, expr: ExprId, op: &BinaryOp, lhs: ExprId, rhs: ExprId) -> Ty<'db> {
        match op {
            BinaryOp::Arith(ArithOp::Shl | ArithOp::Shr) => {
                let ty = self.infer_expr(lhs);
                let shift = self.solver.new_var(VarKind::Int);
                self.check_expr(rhs, &shift);
                ty
            }
            BinaryOp::Arith(_) => {
                let ty = self.infer_expr(lhs);
                self.check_expr_because(rhs, &ty, Reason::SameAs(lhs));
                ty
            }
            BinaryOp::Cmp(_) => {
                let ty = self.infer_expr(lhs);
                self.check_expr_because(rhs, &ty, Reason::SameAs(lhs));
                Ty::bool()
            }
            BinaryOp::Logic(_) => {
                self.check_expr(lhs, &Ty::bool());
                self.check_expr(rhs, &Ty::bool());
                Ty::bool()
            }
            BinaryOp::Assign(op) => {
                let ty = self.infer_expr(lhs);
                self.check_expr_because(rhs, &ty, Reason::SameAs(lhs));
                self.check_place(lhs, op.is_none());
                Ty::unit()
            }
            // `a <$> b` calls the function `(<$>)`.
            BinaryOp::Custom(name) => {
                let resolver = self.resolver(expr);
                let mut function = None;
                let callee_ty = match resolver.resolve_path_in_value_ns(self.db, &Path::from_name(name.clone())) {
                    Some(ValueResolution::Value(value)) => {
                        if let ValueNs::Function(f) = value {
                            function = Some(f);
                        }
                        self.value_ty(expr, value)
                    }
                    _ => {
                        self.unresolved(ExprOrPat::Expr(expr));
                        Ty::Error
                    }
                };
                let lhs_ty = self.infer_expr(lhs);
                self.apply(expr, expr, &callee_ty, function, &[rhs], Some((lhs, &lhs_ty)))
            }
        }
    }

    /// Report an assignment to `place` if it can't be assigned to. `plain`
    /// is whether it's `=` rather than a compound assignment like `+=`.
    ///
    /// Arrays are shared, so assigning to an element, or to a field of an
    /// element, changes the array: the array must be `mut`, and the
    /// variable it's in doesn't matter. Otherwise the place must be a
    /// variable that's `mut`, or a field of one: structs are values, so
    /// `p.x = 1` changes the variable `p`.
    fn check_place(&mut self, place: ExprId, plain: bool) {
        let body = self.body;
        let mut root = place;
        loop {
            match &body[root] {
                Expr::Field { expr: base, .. } => {
                    let base = *base;
                    // A path like `Color.Red` has no type of its own.
                    let Some(base_ty) = self.result.expr_types.get(base) else {
                        root = base;
                        continue;
                    };
                    match self.solver.unifier.resolve(base_ty) {
                        // A struct is shared, like an array.
                        ty @ Ty::Adt(AdtId::Struct(_), ..) => {
                            let mutability = ty.mutability().unwrap().clone();
                            return self.solver.require_mut(&ty, &mutability, self.origin(base));
                        }
                        // Decided with the field constraint, once the type is
                        // known.
                        Ty::Var(_) => return self.solver.assign_field(self.origin(root)),
                        Ty::Error | Ty::Never => return,
                        // A tuple is a value: it's changed in its variable.
                        _ => root = base,
                    }
                }
                Expr::Index { base, .. } => return self.check_mut_index(root, *base),
                _ => break,
            }
        }
        let path = match &body[root] {
            Expr::Path(path) => path,
            // Already reported, or unknown until macros are expanded.
            Expr::Missing | Expr::MacroCall { .. } => return,
            _ => {
                self.result.diagnostics.push(InferenceDiagnostic::NotAPlace { expr: place });
                return;
            }
        };
        let binding = match self.resolver(root).resolve_path_in_value_ns(self.db, path) {
            Some(ValueResolution::Value(ValueNs::Local(binding))) => binding,
            // An unresolved name is already reported.
            None => return,
            // A function, constant, variant or type.
            Some(_) => {
                self.result.diagnostics.push(InferenceDiagnostic::NotAPlace { expr: place });
                return;
            }
        };
        if !body[binding].mutable {
            // The first `x = ...` after `let x;` initializes `x`.
            if plain && root == place && let Some(i) = self.uninitialized.iter().position(|&b| b == binding) {
                self.uninitialized.swap_remove(i);
                return;
            }
            self.result.diagnostics.push(InferenceDiagnostic::AssignToImmutable { expr: place, binding });
        }
    }

    /// `expr`, an element of which is assigned to at `place`, needs an impl
    /// of `IndexMut`. An array or struct that isn't `mut` is reported as such
    /// instead, and one whose mutability isn't known yet becomes `mut`.
    fn check_mut_index(&mut self, place: ExprId, expr: ExprId) {
        // A type that an associated type decides, like that of
        // `let ys = to_imm(xs)`, may be known once what's pending is solved.
        if let Ty::Var(_) = self.solver.unifier.shallow_resolve(self.result.expr_ty(expr)) {
            self.solver.solve();
        }
        let ty = self.solver.unifier.resolve(self.result.expr_ty(expr));
        if matches!(ty, Ty::Error | Ty::Never) {
            return;
        }
        // An array or struct that isn't `mut` can't be changed, which says
        // more than that there's no impl for it; and one whose mutability
        // isn't known yet becomes `mut`.
        if let Some(mutability) = ty.mutability() {
            let changeable = matches!(
                self.solver.unifier.shallow_resolve(mutability),
                Ty::Mutability(Mutability::Mut) | Ty::Var(_) | Ty::Error | Ty::Never
            );
            self.solver.require_mut(&ty, mutability, self.origin(expr));
            if !changeable {
                return;
            }
        }
        let file = self.def.def(self.db).file(self.db);
        // Without `Index`, reading the element already reported it.
        let Some(class) = LangClass::IndexMut.class(self.db, file) else { return };
        let constraint = ClassConstraint { class, args: Box::new([ty]) };
        self.solver.add(Constraint::Class { constraint, origin: self.origin(place) });
    }

    /// The associated type of `lang`, a class of the prelude that syntax
    /// uses, for `arg`, which must have an instance of it at `at`. Without a
    /// prelude that defines the class, that's reported, and it's `None`.
    fn lang_assoc(&mut self, lang: LangClass, arg: &Ty<'db>, at: ExprId) -> Option<Ty<'db>> {
        let file = self.def.def(self.db).file(self.db);
        let Some((class, alias)) = lang.class(self.db, file).and_then(|c| Some((c, lang.assoc(self.db, c)?))) else {
            self.result.diagnostics.push(InferenceDiagnostic::NoLangClass { expr: at, class: lang.name() });
            return None;
        };
        let origin = self.origin(at);
        let constraint = ClassConstraint { class, args: Box::new([arg.clone()]) };
        self.solver.add(Constraint::Class { constraint, origin });
        Some(self.solver.flatten_assoc(&Ty::Assoc(alias, Box::new([arg.clone()])), origin))
    }

    fn infer_record(
        &mut self,
        expr: ExprId,
        path: Option<&Path>,
        fields: &[RecordField],
        spread: Option<ExprId>,
        is_mut: bool,
    ) -> Ty<'db> {
        let resolver = self.resolver(expr);
        let def = match path.and_then(|p| resolver.resolve_path_in_type_ns(self.db, p)) {
            Some(TypeResolution { ty: TypeNs::Struct(s), unresolved: None }) => Some(VariantDef::Struct(s)),
            Some(TypeResolution { ty: TypeNs::Variant(v), unresolved: None }) => Some(VariantDef::Variant(v)),
            _ => None,
        };
        let Some(def) = def else {
            self.unresolved(ExprOrPat::Expr(expr));
            for field in fields {
                self.infer_expr(field.expr);
            }
            if let Some(spread) = spread {
                self.infer_expr(spread);
            }
            return Ty::Error;
        };
        let (_, generic_def) = def.adt();
        // A variant can't be `mut`: an enum's values can't be changed.
        if is_mut && matches!(def, VariantDef::Variant(_)) {
            self.result.diagnostics.push(InferenceDiagnostic::NoMutability { expr });
        }
        let (ty, args) = self.fresh_adt(def, literal_mutability(is_mut));
        let field_tys = field_types(self.db, def);
        let mut seen = Vec::new();
        for field in fields {
            match field_tys.field(&field.name) {
                Some((index, field_ty)) => {
                    seen.push(index);
                    let field_ty = subst_params(field_ty, generic_def, &args);
                    let reason = Reason::Field { variant: def, index: index as u32 };
                    self.check_expr_because(field.expr, &field_ty, reason);
                }
                None => {
                    self.infer_expr(field.expr);
                    self.result.diagnostics.push(InferenceDiagnostic::NoSuchField {
                        expr: ExprOrPat::Expr(field.expr),
                        name: field.name.clone(),
                    });
                }
            }
        }
        match spread {
            // The fields are copied from `base`, so it can have any
            // mutability.
            Some(spread) => {
                let base = match &ty {
                    Ty::Adt(id @ AdtId::Struct(_), args, _) => Ty::adt(*id, args.clone(), self.new_mutability()),
                    ty => ty.clone(),
                };
                self.check_expr(spread, &base)
            }
            None => {
                let missing: Vec<Name> = field_tys
                    .fields
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| !seen.contains(i))
                    .map(|(_, (name, _))| name.clone())
                    .collect();
                if !missing.is_empty() {
                    self.result.diagnostics.push(InferenceDiagnostic::MissingFields { expr, names: missing });
                }
            }
        }
        ty
    }

    /// Resolve all types once solving is done.
    pub fn finish(mut self) -> InferenceResult<'db> {
        let unifier = &self.solver.unifier;
        self.result.map_types(&mut |t| unifier.resolve(t));
        self.result.check_deep_immutability();
        self.result
    }

    pub fn take_result(&mut self) -> InferenceResult<'db> {
        std::mem::take(&mut self.result)
    }
}

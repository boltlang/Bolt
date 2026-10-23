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
    builtin_type::{BuiltinType, BuiltinUint},
    hir::{
        ArithOp, BinaryOp, Binding, BindingId, Expr, ExprId, LabelId, Literal, Pat, PatId, Path, PathSegment,
        PrefixOp, RecordField, Statement, TypeRefId,
    },
    ids::{FunctionId, ItemContainerId},
    signatures::function_signature,
    resolver::{Resolver, TypeNs, TypeResolution, ValueNs, ValueResolution},
    scope::expr_scopes,
};

use crate::{
    lower::{
        FieldsKind, ImplTarget, LowerDiagnostic, TyLoweringContext, VariantDef, const_ty, field_types, identity_args,
        impl_header, subst_params,
    },
    check::function_scheme,
    scheme::Scheme,
    solve::{ExprOrPat, Origin, Reason, Solver, TypeError, visible_impls},
    ty::{AdtId, Ty, VarKind},
};

/// The types found in a body, and what went wrong.
#[derive(Debug, Default, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct InferenceResult<'db> {
    pub expr_types: ArenaMap<Expr, Ty<'db>>,
    pub pat_types: ArenaMap<Pat, Ty<'db>>,
    pub binding_types: ArenaMap<Binding, Ty<'db>>,
    /// The function a call like `x.f(y)` calls, when it's `f(x, y)`.
    pub method_calls: ArenaMap<Expr, FunctionId<'db>>,
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
    /// A field or index of a value whose type isn't known yet.
    TypeNeeded { expr: ExprId },
    /// A label or loop to break out of that doesn't exist.
    BreakOutside { expr: ExprId },
    /// Something the checker doesn't support yet, like `?` or ranges.
    Unsupported { expr: ExprId },
    /// Functions without complete signatures in files that import each
    /// other call each other; one of them needs a signature.
    CyclicInference,
}

impl<'db> InferenceDiagnostic<'db> {
    pub(crate) fn map_types(&mut self, f: &mut impl FnMut(&Ty<'db>) -> Ty<'db>) {
        match self {
            InferenceDiagnostic::Type(TypeError::Mismatch { expected, actual, .. }) => {
                *expected = f(expected);
                *actual = f(actual);
            }
            InferenceDiagnostic::Type(TypeError::InfiniteType { ty, .. }) | InferenceDiagnostic::NotAFunction { ty, .. } => {
                *ty = f(ty)
            }
            InferenceDiagnostic::Type(
                TypeError::NoInstance { constraint, .. } | TypeError::Ambiguous { constraint, .. },
            ) => *constraint = constraint.map(f),
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
        }
    }

    fn origin(&self, expr: ExprId) -> Origin<'db> {
        Origin::new(self.def, ExprOrPat::Expr(expr))
    }

    fn new_var(&mut self) -> Ty<'db> {
        self.solver.new_var(VarKind::General)
    }

    fn resolver(&self, expr: ExprId) -> Resolver<'db> {
        Resolver::for_expr(self.db, self.def, expr)
    }

    fn lower_ty(&mut self, expr: ExprId, type_ref: TypeRefId) -> Ty<'db> {
        let resolver = self.resolver(expr);
        let mut ctx = TyLoweringContext::new(self.db, &resolver, &self.body.store);
        let ty = ctx.lower_ty(type_ref);
        self.result.diagnostics.extend(ctx.diagnostics.into_iter().map(InferenceDiagnostic::Lower));
        ty
    }

    /// Infer a function body with the given parameter and return types.
    pub fn infer_fn_body(&mut self, params: &[Ty<'db>], ret: &Ty<'db>) {
        let mut params = params.iter();
        if let Some(id) = self.body.self_param {
            let ty = params.next().cloned().unwrap_or(Ty::Error);
            self.result.binding_types.insert(id, ty);
        }
        for &pat in self.body.params.iter() {
            let ty = params.next().cloned().unwrap_or(Ty::Error);
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
                    let ty = expected.map(|e| e.ty.clone()).unwrap_or_else(|| self.new_var());
                    self.breakables.push(Breakable { label: Some(*label), is_loop: false, ty: Some(ty.clone()), broken: false });
                    let actual = self.infer_block(statements, *tail, Some(Expectation { ty: &ty, reason }));
                    self.breakables.pop();
                    self.solver.equal(&ty, &actual, self.origin(expr));
                    ty
                } else {
                    self.infer_block(statements, *tail, expected)
                }
            }
            Expr::Call { callee, args } => self.infer_call(expr, *callee, args),
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
                let base_ty = self.infer_expr(*base);
                let index_ty = self.solver.new_var(VarKind::Int);
                self.check_expr(*index, &index_ty);
                let elem = self.new_var();
                self.solver.equal(&Ty::Array(Box::new(elem.clone())), &base_ty, self.origin(*base));
                elem
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
            Expr::Array { exprs } => {
                let elem = match expected.map(|e| self.solver.unifier.shallow_resolve(e.ty)) {
                    Some(Ty::Array(elem)) => *elem,
                    _ => self.new_var(),
                };
                for (i, e) in exprs.iter().enumerate() {
                    self.check_expr_because(*e, &elem, reason_for_nth(reason, exprs[0], i));
                }
                Ty::Array(Box::new(elem))
            }
            Expr::ArrayRepeat { value, count } => {
                let elem = self.infer_expr(*value);
                self.check_expr(*count, &Ty::Builtin(BuiltinType::Uint(BuiltinUint::Usize)));
                Ty::Array(Box::new(elem))
            }
            Expr::Record { path, fields, spread } => self.infer_record(expr, path.as_ref(), fields, *spread),
            Expr::If { condition, then_branch, else_branch } => {
                self.check_expr(*condition, &Ty::bool());
                match else_branch {
                    Some(else_branch) => {
                        let ty = expected.map(|e| e.ty.clone()).unwrap_or_else(|| self.new_var());
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
                let ty = expected.map(|e| e.ty.clone()).unwrap_or_else(|| self.new_var());
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
                let ty = expected.map(|e| e.ty.clone()).unwrap_or_else(|| self.new_var());
                self.breakables.push(Breakable { label: *label, is_loop: true, ty: Some(ty.clone()), broken: false });
                self.check_expr(*loop_body, &Ty::unit());
                let breakable = self.breakables.pop().unwrap();
                // A loop that's never left doesn't give a value.
                if breakable.broken { ty } else { Ty::Never }
            }
            Expr::For { pat, iterable, body: loop_body, label } => {
                let iterable_ty = self.infer_expr(*iterable);
                // Without an iterator class yet, only arrays can be looped over.
                let elem = self.new_var();
                self.solver.equal(&Ty::Array(Box::new(elem.clone())), &iterable_ty, self.origin(*iterable));
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
            Literal::Bool(_) => Ty::bool(),
            Literal::Int(_, suffix) => suffixed(suffix).unwrap_or_else(|| self.solver.new_var(VarKind::Int)),
            Literal::Float(_, suffix) => suffixed(suffix).unwrap_or_else(|| self.solver.new_var(VarKind::Float)),
            Literal::Char(_) => Ty::Builtin(BuiltinType::Char),
            Literal::Byte(_) => Ty::Builtin(BuiltinType::Uint(BuiltinUint::U8)),
            Literal::String(_) | Literal::CString(_) => Ty::Builtin(BuiltinType::Str),
            Literal::ByteString(_) => Ty::Array(Box::new(Ty::Builtin(BuiltinType::Uint(BuiltinUint::U8)))),
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
            Err(()) => {
                self.result.diagnostics.push(InferenceDiagnostic::CyclicInference);
                Ty::Error
            }
        }
    }

    fn infer_path(&mut self, expr: ExprId, path: &Path) -> Ty<'db> {
        let resolver = self.resolver(expr);
        match resolver.resolve_path_in_value_ns(self.db, path) {
            Some(ValueResolution::Value(value)) => self.value_ty(expr, value),
            Some(ValueResolution::Type(resolution)) => self.type_as_value(expr, path, resolution),
            None => {
                self.result.diagnostics.push(InferenceDiagnostic::UnresolvedName { expr: ExprOrPat::Expr(expr) });
                Ty::Error
            }
        }
    }

    fn value_ty(&mut self, expr: ExprId, value: ValueNs<'db>) -> Ty<'db> {
        match value {
            ValueNs::Local(binding) => self.result.binding_types.get(binding).cloned().unwrap_or(Ty::Error),
            ValueNs::Function(f) => self.function_ty(expr, f),
            ValueNs::Const(c) => const_ty(self.db, c).ty.clone(),
            // Const generics aren't supported yet.
            ValueNs::ConstParam { .. } => Ty::Error,
            ValueNs::Variant(v) => self.constructor_ty(VariantDef::Variant(v)),
        }
    }

    /// The type of a struct or variant used as a value: a function for a
    /// tuple struct, the type itself for a unit one.
    fn constructor_ty(&mut self, def: VariantDef<'db>) -> Ty<'db> {
        let (adt, generic_def) = def.adt();
        let args: Box<[Ty<'db>]> = identity_args(self.db, generic_def).iter().map(|_| self.new_var()).collect();
        let adt_ty = Ty::Adt(adt, args.clone());
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

    /// A path that names a type: a constructor, or an inherent function of
    /// the type like `List.new`.
    fn type_as_value(&mut self, expr: ExprId, path: &Path, resolution: TypeResolution<'db>) -> Ty<'db> {
        match (resolution.ty, resolution.unresolved) {
            (TypeNs::Struct(s), None) => self.constructor_ty(VariantDef::Struct(s)),
            (TypeNs::Variant(v), None) => self.constructor_ty(VariantDef::Variant(v)),
            (TypeNs::Struct(s), Some(i)) if i + 1 == path.segments.len() => {
                self.inherent_fn_ty(expr, AdtId::Struct(s), &path.segments[i].name)
            }
            (TypeNs::Enum(e), Some(i)) if i + 1 == path.segments.len() => {
                self.inherent_fn_ty(expr, AdtId::Enum(e), &path.segments[i].name)
            }
            _ => {
                self.result.diagnostics.push(InferenceDiagnostic::UnresolvedName { expr: ExprOrPat::Expr(expr) });
                Ty::Error
            }
        }
    }

    fn inherent_fn_ty(&mut self, expr: ExprId, adt: AdtId<'db>, name: &Name) -> Ty<'db> {
        match self.inherent_fn(adt, name) {
            Some(f) => self.function_ty(expr, f),
            None => {
                self.result.diagnostics.push(InferenceDiagnostic::UnresolvedName { expr: ExprOrPat::Expr(expr) });
                Ty::Error
            }
        }
    }

    /// A function named `name` in an impl like `impl List[T] { }`.
    fn inherent_fn(&self, adt: AdtId<'db>, name: &Name) -> Option<FunctionId<'db>> {
        let file = crate::check::def_file(self.db, self.def);
        visible_impls(self.db, file).iter().find_map(|&imp| {
            let ImplTarget::Inherent(Ty::Adt(target, _)) = &impl_header(self.db, imp).target else { return None };
            if *target != adt {
                return None;
            }
            imp.item(self.db).items.iter().find_map(|item| match *item {
                boltlang_hir_def::item_tree::AssocItem::Function(idx) => {
                    let f = FunctionId::new(self.db, ItemContainerId::Impl(imp), idx);
                    let fname = f.item(self.db).name.as_ref()?;
                    let matches = match fname {
                        boltlang_hir_def::item_tree::FunctionName::Name(n) => n == name,
                        boltlang_hir_def::item_tree::FunctionName::Operator(_) => false,
                    };
                    matches.then_some(f)
                }
                _ => None,
            })
        })
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
        match self.field_ty(expr, &base_ty, name) {
            Some(ty) => ty,
            None => Ty::Error,
        }
    }

    /// The type of the field `name` of a value of type `ty`, reporting a
    /// diagnostic if there's none.
    fn field_ty(&mut self, expr: ExprId, ty: &Ty<'db>, name: &Name) -> Option<Ty<'db>> {
        match self.solver.unifier.shallow_resolve(ty) {
            Ty::Adt(AdtId::Struct(s), args) => {
                let fields = field_types(self.db, VariantDef::Struct(s));
                if let Some((_, field_ty)) = fields.field(name) {
                    return Some(subst_params(field_ty, boltlang_hir_def::resolver::GenericDefId::Struct(s), &args));
                }
            }
            Ty::Tuple(tys) => {
                if let Some(ty) = name.as_str().parse::<usize>().ok().and_then(|i| tys.get(i)) {
                    return Some(ty.clone());
                }
            }
            Ty::Var(_) => {
                // Like OCaml: the struct in scope with a field of this name.
                return match self.struct_with_field(name) {
                    Some(s) => {
                        let def = boltlang_hir_def::resolver::GenericDefId::Struct(s);
                        let args: Box<[Ty<'db>]> = identity_args(self.db, def).iter().map(|_| self.new_var()).collect();
                        self.solver.equal(&Ty::Adt(AdtId::Struct(s), args.clone()), ty, self.origin(expr));
                        let fields = field_types(self.db, VariantDef::Struct(s));
                        Some(subst_params(fields.field(name).unwrap().1, def, &args))
                    }
                    None => {
                        self.result.diagnostics.push(InferenceDiagnostic::TypeNeeded { expr });
                        None
                    }
                };
            }
            Ty::Error => return None,
            _ => {}
        }
        self.result.diagnostics.push(InferenceDiagnostic::NoSuchField { expr: ExprOrPat::Expr(expr), name: name.clone() });
        None
    }

    /// The one struct in the file's scope with a field named `name`.
    fn struct_with_field(&self, name: &Name) -> Option<boltlang_hir_def::ids::StructId<'db>> {
        let file = crate::check::def_file(self.db, self.def);
        let scope = &boltlang_hir_def::nameres::file_scope(self.db, file).scope;
        let mut found = None;
        for (_, entry) in scope.entries(boltlang_hir_def::nameres::Namespace::Types) {
            let boltlang_hir_def::ids::ModuleDefId::Struct(s) = entry.def else { continue };
            let fields = field_types(self.db, VariantDef::Struct(s));
            if fields.kind == FieldsKind::Record && fields.field(name).is_some() {
                if found.is_some() {
                    return None;
                }
                found = Some(s);
            }
        }
        found
    }

    fn infer_call(&mut self, expr: ExprId, callee: ExprId, args: &[ExprId]) -> Ty<'db> {
        // `x.f(y)`: a field holding a function, a longer path, or `f(x, y)`.
        if let Expr::Field { expr: receiver, name } = &self.body[callee] {
            let (receiver, name) = (*receiver, name.clone());
            if self.path_extended(receiver, &name).is_none() {
                let receiver_ty = self.infer_expr(receiver);
                let is_field = match self.solver.unifier.shallow_resolve(&receiver_ty) {
                    Ty::Adt(AdtId::Struct(s), _) => field_types(self.db, VariantDef::Struct(s)).field(&name).is_some(),
                    _ => false,
                };
                if !is_field {
                    return self.infer_method_call(expr, callee, receiver, &receiver_ty, &name, args);
                }
            }
        }
        let callee_ty = self.infer_expr(callee);
        let function = match &self.body[callee] {
            Expr::Path(path) => match self.resolver(callee).resolve_path_in_value_ns(self.db, path) {
                Some(ValueResolution::Value(ValueNs::Function(f))) => Some(f),
                _ => None,
            },
            _ => None,
        };
        self.apply(expr, callee, &callee_ty, function, args, None)
    }

    /// `x.f(y)` as `f(x, y)`. `f` is a function in scope, or else a function
    /// of an inherent impl of the type of `x`.
    fn infer_method_call(
        &mut self,
        expr: ExprId,
        callee: ExprId,
        receiver: ExprId,
        receiver_ty: &Ty<'db>,
        name: &Name,
        args: &[ExprId],
    ) -> Ty<'db> {
        let resolver = self.resolver(expr);
        let path = Path::from_name(name.clone());
        let function = match resolver.resolve_path_in_value_ns(self.db, &path) {
            Some(ValueResolution::Value(ValueNs::Function(f))) => Some(f),
            _ => match self.solver.unifier.shallow_resolve(receiver_ty) {
                Ty::Adt(adt, _) => self.inherent_fn(adt, name),
                _ => None,
            },
        };
        let callee_ty = match function {
            Some(f) => {
                self.result.method_calls.insert(expr, f);
                self.function_ty(callee, f)
            }
            None => {
                // A local holding a function works too.
                match resolver.resolve_path_in_value_ns(self.db, &path) {
                    Some(ValueResolution::Value(value)) => self.value_ty(callee, value),
                    _ => {
                        if !matches!(self.solver.unifier.shallow_resolve(receiver_ty), Ty::Error) {
                            self.result
                                .diagnostics
                                .push(InferenceDiagnostic::UnresolvedName { expr: ExprOrPat::Expr(callee) });
                        }
                        Ty::Error
                    }
                }
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
            Ty::Var(_) => {
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
            BinaryOp::Assign(_) => {
                let ty = self.infer_expr(lhs);
                self.check_expr_because(rhs, &ty, Reason::SameAs(lhs));
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
                        self.result.diagnostics.push(InferenceDiagnostic::UnresolvedName { expr: ExprOrPat::Expr(expr) });
                        Ty::Error
                    }
                };
                let lhs_ty = self.infer_expr(lhs);
                self.apply(expr, expr, &callee_ty, function, &[rhs], Some((lhs, &lhs_ty)))
            }
        }
    }

    fn infer_record(
        &mut self,
        expr: ExprId,
        path: Option<&Path>,
        fields: &[RecordField],
        spread: Option<ExprId>,
    ) -> Ty<'db> {
        let resolver = self.resolver(expr);
        let def = match path.and_then(|p| resolver.resolve_path_in_type_ns(self.db, p)) {
            Some(TypeResolution { ty: TypeNs::Struct(s), unresolved: None }) => Some(VariantDef::Struct(s)),
            Some(TypeResolution { ty: TypeNs::Variant(v), unresolved: None }) => Some(VariantDef::Variant(v)),
            _ => None,
        };
        let Some(def) = def else {
            self.result.diagnostics.push(InferenceDiagnostic::UnresolvedName { expr: ExprOrPat::Expr(expr) });
            for field in fields {
                self.infer_expr(field.expr);
            }
            if let Some(spread) = spread {
                self.infer_expr(spread);
            }
            return Ty::Error;
        };
        let (adt, generic_def) = def.adt();
        let args: Box<[Ty<'db>]> = identity_args(self.db, generic_def).iter().map(|_| self.new_var()).collect();
        let ty = Ty::Adt(adt, args.clone());
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
            Some(spread) => self.check_expr(spread, &ty),
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
        self.result
    }

    pub fn take_result(&mut self) -> InferenceResult<'db> {
        std::mem::take(&mut self.result)
    }
}

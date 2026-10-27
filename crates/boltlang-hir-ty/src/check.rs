//! The queries that infer bodies.
//!
//! A function whose type comes from its declaration (see
//! [`is_declared`]) is checked on its own, so editing its body doesn't
//! affect its callers. The other functions of a file are inferred together
//! by [`file_inference`]: grouped by which call which, each group inferred
//! after the groups it calls, and then generalized.

use boltlang_base_db::{Db, File};
use boltlang_hir_def::{
    body::{DefWithBody, DefWithBodyId, body},
    hir::{BinaryOp, Expr, Path},
    ids::{FunctionId, ItemContainerId},
    item_tree::{ModItem, item_tree},
    resolver::{Resolver, ValueNs, ValueResolution},
};

use crate::{
    infer::{FnEnv, InferenceContext, InferenceDiagnostic, InferenceResult},
    lower::{ClassConstraint, const_ty, fn_sig},
    scheme::{
        Scheme, assumptions, class_impl_method_sig, declared_scheme, generic_chain, generic_param_ids, is_declared,
    },
    solve::{ExprOrPat, FieldConstraint, Origin, Solver, TypeError, resolve_error, vars_in},
    unify::Unifier,
    ty::{Ty, TyVar, VarKind},
};

/// Why a function's type can't be inferred: files that import each other
/// have functions without complete signatures that call each other, so
/// neither file can be inferred first. One of them needs a signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InferenceCycle;

/// The type of a function, for its callers.
pub fn function_scheme<'db>(db: &'db dyn Db, f: FunctionId<'db>) -> Result<&'db Scheme<'db>, InferenceCycle> {
    if is_declared(db, f) {
        return Ok(declared_scheme(db, f));
    }
    let inference = file_inference(db, f.file(db));
    inference.schemes.iter().find(|(g, _)| *g == f).map(|(_, s)| s).ok_or(InferenceCycle)
}

/// The types in the body of `def`.
#[salsa::tracked(returns(ref))]
pub fn infer_body<'db>(db: &'db dyn Db, def: DefWithBodyId<'db>) -> InferenceResult<'db> {
    let file = def.def(db).file(db);
    match def.def(db) {
        DefWithBody::Function(f) if !is_declared(db, f) => {
            let inference = file_inference(db, file);
            match inference.results.iter().find(|(g, _)| *g == f) {
                Some((_, result)) => result.clone(),
                None => InferenceResult { diagnostics: vec![InferenceDiagnostic::CyclicInference], ..Default::default() },
            }
        }
        def_kind => {
            // A body checked on its own.
            let env = FnEnv::default();
            let mut solver = Solver::new(db, file);
            let mut ctx = InferenceContext::new(db, def, &mut solver, &env);
            match def_kind {
                DefWithBody::Function(f) => {
                    for assumption in assumptions(db, f) {
                        ctx.solver.assume(assumption);
                    }
                    let Ty::Fn(params, ret) = &declared_scheme(db, f).ty else { unreachable!() };
                    // A method of a class impl must have the class's types.
                    if let Some((class_params, class_ret)) = class_impl_method_sig(db, f) {
                        let origin = Origin::new(def, ExprOrPat::Expr(ctx.body.body_expr));
                        for (own, class) in params.iter().zip(class_params.iter()) {
                            ctx.solver.equal(class, own, origin);
                        }
                        ctx.solver.equal(&class_ret, ret, origin);
                    }
                    ctx.infer_fn_body(params, ret);
                }
                DefWithBody::Const(c) => {
                    ctx.infer_expr_body(Some(&const_ty(db, c).ty));
                }
                // The value of a variant, like the `1` of `A = 1`.
                DefWithBody::Variant(_) => {
                    let int = ctx.solver.new_var(VarKind::Int);
                    ctx.infer_expr_body(Some(&int));
                }
                DefWithBody::File(_) => {
                    ctx.infer_expr_body(None);
                }
            }
            ctx.solver.finish();
            let errors = std::mem::take(&mut ctx.solver.errors);
            let mut result = ctx.finish();
            result.diagnostics.extend(errors.into_iter().map(InferenceDiagnostic::Type));
            result
        }
    }
}

/// The functions of a file without complete signatures, inferred together.
#[derive(Debug, Default, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct FileInference<'db> {
    pub schemes: Vec<(FunctionId<'db>, Scheme<'db>)>,
    pub results: Vec<(FunctionId<'db>, InferenceResult<'db>)>,
}

#[salsa::tracked(returns(ref), cycle_result = file_inference_cycle)]
pub fn file_inference<'db>(db: &'db dyn Db, file: File) -> FileInference<'db> {
    let functions = inferred_functions(db, file);
    let calls: Vec<Vec<usize>> = functions
        .iter()
        .map(|&f| called_functions(db, f).iter().filter_map(|g| functions.iter().position(|h| h == g)).collect())
        .collect();
    let mut env = FnEnv::default();
    let mut out = FileInference::default();
    for group in strongly_connected(&calls) {
        let group: Vec<FunctionId<'db>> = group.into_iter().map(|i| functions[i]).collect();
        infer_group(db, file, &group, &mut env, &mut out);
    }
    out
}

fn file_inference_cycle<'db>(_db: &'db dyn Db, _id: salsa::Id, _file: File) -> FileInference<'db> {
    FileInference::default()
}

/// Infer a group of functions that call each other, and generalize them.
fn infer_group<'db>(
    db: &'db dyn Db,
    file: File,
    group: &[FunctionId<'db>],
    env: &mut FnEnv<'db>,
    out: &mut FileInference<'db>,
) {
    let mut solver = Solver::new(db, file);
    let mut signatures = Vec::new();
    for &f in group {
        for assumption in assumptions(db, f) {
            solver.assume(assumption);
        }
        let sig = fn_sig(db, f);
        let params: Box<[Ty<'db>]> =
            sig.params.iter().map(|t| t.clone().unwrap_or_else(|| solver.new_var(VarKind::General))).collect();
        // The body decides the mutabilities of the return type that aren't
        // written.
        let ret = match &sig.ret {
            Some(ret) => ret.fill_unwritten(&mut || solver.new_var(VarKind::Mutability)),
            None => solver.new_var(VarKind::General),
        };
        env.mono.insert(f, Ty::Fn(params.clone(), Box::new(ret.clone())));
        signatures.push((params, ret));
    }
    let mut results = Vec::new();
    for (&f, (params, ret)) in group.iter().zip(&signatures) {
        let def = DefWithBodyId::new(db, DefWithBody::Function(f));
        let mut ctx = InferenceContext::new(db, def, &mut solver, env);
        ctx.infer_fn_body(params, ret);
        results.push((def, ctx.take_result()));
    }

    // Integer variables don't become type parameters; they default first.
    solver.solve_with_defaults();
    // The errors are resolved before what's unknown because of them becomes
    // the error type, or they'd be left out for having it in them.
    let errors = std::mem::take(&mut solver.errors).into_iter().map(|e| resolve_error(&solver.unifier, e)).collect();
    solver.unifier.error_unknowns();
    let pending = solver.take_pending();
    let pending_fields = solver.take_pending_fields();
    let solved = Solved { unifier: &solver.unifier, errors, pending, pending_fields };
    for (&f, (def, result)) in group.iter().zip(results) {
        let ty = solved.unifier.resolve(&env.mono.remove(&f).unwrap());
        let (scheme, mut result) = solved.generalize(db, f, def, &ty, result);
        result.check_deep_immutability();
        env.done.insert(f, scheme.clone());
        out.schemes.push((f, scheme));
        out.results.push((f, result));
    }
}

/// What's left once a group of functions is solved, for generalizing them.
struct Solved<'a, 'db> {
    unifier: &'a Unifier<'db>,
    /// The errors of all the functions, resolved.
    errors: Vec<TypeError<'db>>,
    /// The class and field constraints that couldn't be solved, resolved.
    pending: Vec<(ClassConstraint<'db>, Origin<'db>)>,
    pending_fields: Vec<(FieldConstraint<'db>, Origin<'db>)>,
}

impl<'db> Solved<'_, 'db> {
    /// Make the type `ty` of the function `f` a scheme, generic over the
    /// variables still in it, and put the types of its body, `result`, in
    /// terms of the scheme's variables.
    fn generalize(
        &self,
        db: &'db dyn Db,
        f: FunctionId<'db>,
        def: DefWithBodyId<'db>,
        ty: &Ty<'db>,
        mut result: InferenceResult<'db>,
    ) -> (Scheme<'db>, InferenceResult<'db>) {
        let mut vars = Generalization::default();
        vars.add(ty);
        let (fields, untyped) = self.function_fields(def, &mut vars);
        let (predicates, ambiguous) = self.function_predicates(db, f, def, &vars);
        let scheme = Scheme {
            params: generic_param_ids(db, generic_chain(db, f)),
            bound: vars.vars.len() as u32,
            predicates,
            fields,
            ty: vars.generalize(ty),
        };
        result.map_types(&mut |t| vars.generalize(&self.unifier.resolve(t)));
        let errors = self.errors.iter().filter(|e| e.origin().body == def).cloned();
        let untyped = untyped.into_iter().map(|origin| TypeError::TypeNeeded { origin });
        result.diagnostics.extend(errors.chain(untyped).chain(ambiguous).map(|error| {
            let mut diagnostic = InferenceDiagnostic::Type(error);
            diagnostic.map_types(&mut |t| vars.generalize(t));
            diagnostic
        }));
        (scheme, result)
    }

    /// The field constraints that become part of the type of the function
    /// with the body `def`: those about values of its types, and then those
    /// about the types of their fields, like `b.y: c` after `a.x: b` for
    /// `p.x.y`. The types of the fields join `vars`. Also gives the origins
    /// of the function's other field constraints, whose types are never
    /// found.
    fn function_fields(
        &self,
        def: DefWithBodyId<'db>,
        vars: &mut Generalization,
    ) -> (Box<[FieldConstraint<'db>]>, Vec<Origin<'db>>) {
        let mut rest: Vec<_> = self.pending_fields.iter().filter(|(_, origin)| origin.body == def).collect();
        let mut accepted = Vec::new();
        // Accepting a constraint can make others about the function's types,
        // so this goes on until nothing changes.
        loop {
            let before = accepted.len();
            rest.retain(|(constraint, _)| {
                if !vars.covers([&constraint.ty]) {
                    return true;
                }
                vars.add(&constraint.field);
                accepted.push(constraint);
                false
            });
            if accepted.len() == before {
                break;
            }
        }
        let mut fields = Vec::new();
        for constraint in accepted {
            let field = constraint.map(|t| vars.generalize(t));
            if !fields.contains(&field) {
                fields.push(field);
            }
        }
        (fields.into(), rest.into_iter().map(|(_, origin)| *origin).collect())
    }

    /// The class constraints that become part of the type of `f`: those it
    /// assumes, and those of its body `def` about the types in `vars`. Also
    /// gives the errors for those of its body about other types, which can't
    /// be decided.
    fn function_predicates(
        &self,
        db: &'db dyn Db,
        f: FunctionId<'db>,
        def: DefWithBodyId<'db>,
        vars: &Generalization,
    ) -> (Box<[ClassConstraint<'db>]>, Vec<TypeError<'db>>) {
        let mut predicates = assumptions(db, f);
        let mut ambiguous = Vec::new();
        for (constraint, origin) in self.pending.iter().filter(|(_, origin)| origin.body == def) {
            if vars.covers(constraint.args.iter()) {
                let predicate = constraint.map(|t| vars.generalize(t));
                if !predicates.contains(&predicate) {
                    predicates.push(predicate);
                }
            } else {
                ambiguous.push(TypeError::Ambiguous { origin: *origin, constraint: constraint.clone() });
            }
        }
        (predicates.into(), ambiguous)
    }
}

/// The inference variables a scheme is generic over: the `i`th becomes
/// `Ty::Bound(i)`.
#[derive(Default)]
struct Generalization {
    vars: Vec<TyVar>,
}

impl Generalization {
    /// Make the scheme generic over the variables in `ty` too.
    fn add(&mut self, ty: &Ty<'_>) {
        vars_in(ty, &mut self.vars);
    }

    /// Whether `tys` have variables, and the scheme is generic over all of
    /// them.
    fn covers<'a, 'db: 'a>(&self, tys: impl IntoIterator<Item = &'a Ty<'db>>) -> bool {
        let mut found = Vec::new();
        for ty in tys {
            vars_in(ty, &mut found);
        }
        !found.is_empty() && found.iter().all(|v| self.vars.contains(v))
    }

    /// `ty` with the scheme's variables as [`Ty::Bound`].
    fn generalize<'db>(&self, ty: &Ty<'db>) -> Ty<'db> {
        ty.fold(&mut |t| match t {
            Ty::Var(v) => self.vars.iter().position(|w| w == v).map(|i| Ty::Bound(i as u32)),
            _ => None,
        })
    }
}

/// The functions of a file whose types are inferred from their bodies: those
/// at the top level with incomplete signatures.
#[salsa::tracked(returns(ref))]
pub fn inferred_functions<'db>(db: &'db dyn Db, file: File) -> Vec<FunctionId<'db>> {
    item_tree(db, file)
        .top_level
        .iter()
        .filter_map(|item| match *item {
            ModItem::Function(idx) => Some(FunctionId::new(db, ItemContainerId::File(file), idx)),
            _ => None,
        })
        .filter(|&f| !is_declared(db, f))
        .collect()
}

/// The functions a function's body refers to by name.
fn called_functions<'db>(db: &'db dyn Db, f: FunctionId<'db>) -> Vec<FunctionId<'db>> {
    let def = DefWithBodyId::new(db, DefWithBody::Function(f));
    let body = body(db, def);
    let mut out = Vec::new();
    for (id, expr) in body.exprs.iter() {
        let path = match expr {
            Expr::Path(path) => path.clone(),
            // `x.f()` may call `f`.
            Expr::Field { name, .. } => Path::from_name(name.clone()),
            Expr::Binary { op: BinaryOp::Custom(name), .. } => Path::from_name(name.clone()),
            _ => continue,
        };
        let resolver = Resolver::for_expr(db, def, id);
        if let Some(ValueResolution::Value(ValueNs::Function(g))) = resolver.resolve_path_in_value_ns(db, &path)
            && !out.contains(&g)
        {
            out.push(g);
        }
    }
    out
}

/// Tarjan's algorithm: the strongly connected components of a graph, each
/// after the components it has edges to.
fn strongly_connected(edges: &[Vec<usize>]) -> Vec<Vec<usize>> {
    struct State<'a> {
        edges: &'a [Vec<usize>],
        index: Vec<Option<usize>>,
        low: Vec<usize>,
        on_stack: Vec<bool>,
        stack: Vec<usize>,
        next: usize,
        out: Vec<Vec<usize>>,
    }
    fn visit(s: &mut State<'_>, v: usize) {
        s.index[v] = Some(s.next);
        s.low[v] = s.next;
        s.next += 1;
        s.stack.push(v);
        s.on_stack[v] = true;
        for &w in &s.edges[v] {
            match s.index[w] {
                None => {
                    visit(s, w);
                    s.low[v] = s.low[v].min(s.low[w]);
                }
                Some(i) if s.on_stack[w] => s.low[v] = s.low[v].min(i),
                Some(_) => {}
            }
        }
        if Some(s.low[v]) == s.index[v] {
            let mut component = Vec::new();
            loop {
                let w = s.stack.pop().unwrap();
                s.on_stack[w] = false;
                component.push(w);
                if w == v {
                    break;
                }
            }
            component.reverse();
            s.out.push(component);
        }
    }
    let n = edges.len();
    let mut s = State {
        edges,
        index: vec![None; n],
        low: vec![0; n],
        on_stack: vec![false; n],
        stack: Vec::new(),
        next: 0,
        out: Vec::new(),
    };
    for v in 0..n {
        if s.index[v].is_none() {
            visit(&mut s, v);
        }
    }
    s.out
}

#[cfg(test)]
mod tests;

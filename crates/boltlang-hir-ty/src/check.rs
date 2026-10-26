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
    solve::{ExprOrPat, FieldConstraint, Solver, TypeError, Origin, resolve_error, vars_in},
    ty::{Ty, TyVar, VarKind},
};

pub fn def_file<'db>(db: &'db dyn Db, def: DefWithBodyId<'db>) -> File {
    match def.def(db) {
        DefWithBody::Function(id) => id.file(db),
        DefWithBody::Const(id) => id.file(db),
        DefWithBody::Variant(id) => id.parent.file(db),
        DefWithBody::File(file) => file,
    }
}

/// The type of a function, for its callers. `Err` if it can't be inferred
/// because files that import each other have functions without complete
/// signatures that call each other.
pub fn function_scheme<'db>(db: &'db dyn Db, f: FunctionId<'db>) -> Result<&'db Scheme<'db>, ()> {
    if is_declared(db, f) {
        return Ok(declared_scheme(db, f));
    }
    let inference = file_inference(db, f.file(db));
    inference.schemes.iter().find(|(g, _)| *g == f).map(|(_, s)| s).ok_or(())
}

/// The types in the body of `def`.
#[salsa::tracked(returns(ref))]
pub fn infer_body<'db>(db: &'db dyn Db, def: DefWithBodyId<'db>) -> InferenceResult<'db> {
    let file = def_file(db, def);
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
        let ret = sig.ret.clone().unwrap_or_else(|| solver.new_var(VarKind::General));
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
    let mut errors: Vec<_> =
        std::mem::take(&mut solver.errors).into_iter().map(|e| resolve_error(&solver.unifier, e)).collect();
    solver.unifier.error_unknowns();
    let pending = solver.take_pending();
    let pending_fields = solver.take_pending_fields();

    for (&f, (def, mut result)) in group.iter().zip(results) {
        let ty = solver.unifier.resolve(&env.mono.remove(&f).unwrap());
        let mut vars = Vec::new();
        vars_in(&ty, &mut vars);
        let generalize = |t: &Ty<'db>, vars: &[TyVar]| {
            t.fold(&mut |t| match t {
                Ty::Var(v) => vars.iter().position(|w| w == v).map(|i| Ty::Bound(i as u32)),
                _ => None,
            })
        };
        // The fields of values of the function's type become part of it,
        // and the types of those fields with them. That can make more
        // constraints about the function's type, like `b.y: c` after
        // `a.x: b` for `p.x.y`, so this goes on until nothing changes.
        let mut own_fields: Vec<_> = pending_fields.iter().filter(|(_, origin)| origin.body == def).collect();
        let mut fields: Vec<FieldConstraint<'db>> = Vec::new();
        let mut accepted = Vec::new();
        loop {
            let before = accepted.len();
            own_fields.retain(|(constraint, _)| {
                let mut ty_vars = Vec::new();
                vars_in(&constraint.ty, &mut ty_vars);
                if ty_vars.is_empty() || !ty_vars.iter().all(|v| vars.contains(v)) {
                    return true;
                }
                vars_in(&constraint.field, &mut vars);
                accepted.push(constraint.clone());
                false
            });
            if accepted.len() == before {
                break;
            }
        }
        for constraint in &accepted {
            let field = constraint.map(|t| generalize(t, &vars));
            if !fields.contains(&field) {
                fields.push(field);
            }
        }
        for (_, origin) in own_fields {
            errors.push(TypeError::TypeNeeded { origin: *origin });
        }
        // The constraints on this function's type become part of it.
        let mut predicates: Vec<ClassConstraint<'db>> = assumptions(db, f);
        for (constraint, origin) in &pending {
            if origin.body != def {
                continue;
            }
            let mut constraint_vars = Vec::new();
            constraint.args.iter().for_each(|t| vars_in(t, &mut constraint_vars));
            if constraint_vars.iter().all(|v| vars.contains(v)) && !constraint_vars.is_empty() {
                let pred = constraint.map(|t| generalize(t, &vars));
                if !predicates.contains(&pred) {
                    predicates.push(pred);
                }
            } else {
                errors.push(TypeError::Ambiguous { origin: *origin, constraint: constraint.clone() });
            }
        }
        let scheme = Scheme {
            params: generic_param_ids(db, generic_chain(db, f)),
            bound: vars.len() as u32,
            predicates: predicates.into(),
            fields: fields.into(),
            ty: generalize(&ty, &vars),
        };

        result.map_types(&mut |t| generalize(&solver.unifier.resolve(t), &vars));
        let own_errors = errors.iter().filter(|e| e.origin().body == def).cloned();
        result.diagnostics.extend(own_errors.map(|e| {
            let mut d = InferenceDiagnostic::Type(e);
            d.map_types(&mut |t| generalize(t, &vars));
            d
        }));
        env.done.insert(f, scheme.clone());
        out.schemes.push((f, scheme));
        out.results.push((f, result));
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

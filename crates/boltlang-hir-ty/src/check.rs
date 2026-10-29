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
    hir::{BinaryOp, Expr, Path, PathSegment},
    ids::{FunctionId, ImplId, ItemContainerId, TypeAliasId},
    item_tree::{ModItem, item_tree},
    resolver::{GenericDefId, Resolver, ValueNs, ValueResolution},
};

use crate::{
    holds::split,
    infer::{FnEnv, InferenceContext, InferenceDiagnostic, InferenceResult},
    lower::{
        ClassConstraint, assoc_type_bounds, class_assoc_types, const_ty, fn_sig, generic_predicates, impl_assoc_ty,
        impl_header, subst_params, with_superclasses,
    },
    scheme::{
        Scheme, SchemeSources, assumptions, assumptions_with_sources, at_most_assumptions, class_impl_method_sig, declared_scheme,
        generic_chain, generic_param_ids, is_declared,
    },
    solve::{
        AssocConstraint, AtMostConstraint, Constraint, ConstraintSource, ExprOrPat, FieldConstraint, Origin, Solver, TypeError,
        normalize_assoc, prune_implied_errors, resolve_error, vars_in,
    },
    unify::Unifier,
    ty::{Mutability, Ty, TyVar, VarKind},
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
                    for (assumption, _) in at_most_assumptions(db, f) {
                        ctx.solver.assume_at_most(assumption);
                    }
                    let Ty::Fn(params, ret, _) = &declared_scheme(db, f).ty else { unreachable!() };
                    for assumption in implied_assumptions(db, f, params) {
                        ctx.solver.assume_at_most(assumption);
                    }
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
            result.add_instances(def, &solver);
            result.diagnostics.extend(errors.into_iter().map(InferenceDiagnostic::Type));
            // After the type errors, which can report the same thing.
            result.check_deep_immutability(db);
            result
        }
    }
}

/// The bounds of the associated types of an impl's class that the impl's
/// types for them don't meet, like `Iterator[Pair]` for `type Iter = Pair;`
/// in an impl of `class Iterable[C] { type Iter: Iterator; }` when `Pair`
/// has no impl of `Iterator`. The impl's own bounds and `where` clauses are
/// assumed. Each is given with the associated type of the class.
#[salsa::tracked(returns(ref))]
pub fn unmet_assoc_bounds<'db>(db: &'db dyn Db, imp: ImplId<'db>) -> Vec<(TypeAliasId<'db>, ClassConstraint<'db>)> {
    let Some(target) = impl_header(db, imp).class.clone() else { return Vec::new() };
    let file = imp.file(db);
    // The solver reports at a place in a body; the errors are only counted.
    let def = DefWithBodyId::new(db, DefWithBody::File(file));
    let origin = Origin::new(def, ExprOrPat::Expr(body(db, def).body_expr));
    let class_def = GenericDefId::Class(target.class);
    let mut unmet = Vec::new();
    for alias in class_assoc_types(db, target.class) {
        // A missing one is reported with the impl's other missing items.
        let Some(own) = impl_assoc_ty(db, imp, alias) else { continue };
        for bound in assoc_type_bounds(db, alias).predicates.iter() {
            let constraint = bound.map(|t| {
                let t = subst_params(t, class_def, &target.args);
                let t = t.fold(&mut |t| match t {
                    Ty::Assoc(a, args) if *a == alias && *args == target.args => Some(own.clone()),
                    _ => None,
                });
                normalize_assoc(db, file, &t)
            });
            let mut solver = Solver::new(db, file);
            for predicate in generic_predicates(db, GenericDefId::Impl(imp)).predicates.iter() {
                solver.assume(predicate.clone());
            }
            solver.add(Constraint::Class { constraint: constraint.clone(), origin });
            solver.solve();
            if solver.errors.iter().any(|e| matches!(e, TypeError::NoInstance { .. })) {
                unmet.push((alias, constraint));
            }
        }
    }
    unmet
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

/// What the body of `f` can assume because the values it's given exist
/// (#256): that its parameter types, and its impl's instance, are
/// well-formed, like `T ⊑ 'm` for `xs: 'm [T]`, as Rust's implied bounds
/// do. They aren't part of `f`'s type for its callers: arguments meet them
/// already, being values. What's about a type still being inferred isn't
/// assumed.
fn implied_assumptions<'db>(db: &'db dyn Db, f: FunctionId<'db>, params: &[Ty<'db>]) -> Vec<AtMostConstraint<'db>> {
    let mutable = Ty::Mutability(Mutability::Mut);
    let mut out = Vec::new();
    for param in params {
        split(db, param, &mutable, &mut out);
    }
    if let ItemContainerId::Impl(imp) = f.container(db)
        && let Some(instance) = &impl_header(db, imp).class
    {
        for arg in instance.args.iter() {
            split(db, arg, &mutable, &mut out);
        }
    }
    out.retain(|a| !a.held.ty().any(&mut |t| matches!(t, Ty::Var(_))) && !a.bound.any(&mut |t| matches!(t, Ty::Var(_))));
    out
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
        for (assumption, _) in at_most_assumptions(db, f) {
            solver.assume_at_most(assumption);
        }
        let sig = fn_sig(db, f);
        let params: Box<[Ty<'db>]> =
            sig.params.iter().map(|t| t.clone().unwrap_or_else(|| solver.new_var(VarKind::General))).collect();
        for assumption in implied_assumptions(db, f, &params) {
            solver.assume_at_most(assumption);
        }
        // The body decides the mutabilities of the return type that aren't
        // written.
        let ret = match &sig.ret {
            Some(ret) => ret.fill_unwritten(&mut || solver.new_var(VarKind::Mutability)),
            None => solver.new_var(VarKind::General),
        };
        env.mono.insert(f, Ty::func(params.clone(), ret.clone()));
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
    solver.keep_pending_assoc();
    // What's left about the functions' types becomes part of them; the
    // other unknowns in "at most" constraints can be chosen to meet them.
    let mut keep = Vec::new();
    for f in group {
        vars_in(&solver.unifier.resolve(&env.mono[f]), &mut keep);
    }
    solver.settle_at_most(&keep, true);
    // The errors are resolved before what's unknown because of them becomes
    // the error type, or they'd be left out for having it in them.
    let mut errors: Vec<_> =
        std::mem::take(&mut solver.errors).into_iter().map(|e| resolve_error(&solver.unifier, e)).collect();
    prune_implied_errors(db, &mut errors);
    solver.unifier.error_unknowns();
    let pending = solver.take_pending();
    let pending_fields = solver.take_pending_fields();
    let pending_assoc = solver.take_pending_assoc();
    let pending_at_most = solver.take_pending_at_most();
    let solved = Solved { unifier: &solver.unifier, errors, pending, pending_fields, pending_assoc, pending_at_most };
    for (&f, (def, mut result)) in group.iter().zip(results) {
        result.add_instances(def, &solver);
        let ty = solved.unifier.resolve(&env.mono.remove(&f).unwrap());
        let (scheme, mut result) = solved.generalize(db, f, def, &ty, result);
        result.check_deep_immutability(db);
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
    pending_assoc: Vec<(AssocConstraint<'db>, Origin<'db>)>,
    pending_at_most: Vec<(AtMostConstraint<'db>, Origin<'db>)>,
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
        let (fields, field_sources, untyped) = self.function_fields(def, &mut vars);
        let (assoc, assoc_sources) = self.function_assoc(def, &mut vars);
        let (predicates, predicate_sources, ambiguous) = self.function_predicates(db, f, def, &vars);
        let (bounds, bound_sources) = self.function_bounds(db, f, def, &vars);
        let scheme = Scheme {
            params: generic_param_ids(db, generic_chain(db, f)),
            bound: vars.vars.len() as u32,
            predicates,
            fields,
            assoc,
            bounds,
            ty: vars.generalize(ty),
            sources: SchemeSources {
                predicates: predicate_sources,
                fields: field_sources,
                assoc: assoc_sources,
                bounds: bound_sources,
            },
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
    /// `p.x.y`. The types of the fields join `vars`. Also gives where each
    /// comes from, and the origins of the function's other field
    /// constraints, whose types are never found.
    fn function_fields(
        &self,
        def: DefWithBodyId<'db>,
        vars: &mut Generalization,
    ) -> (Box<[FieldConstraint<'db>]>, Sources<'db>, Vec<Origin<'db>>) {
        let mut rest: Vec<_> = self.pending_fields.iter().filter(|(_, origin)| origin.body == def).collect();
        let mut accepted = Vec::new();
        // Accepting a constraint can make others about the function's types,
        // so this goes on until nothing changes.
        loop {
            let before = accepted.len();
            rest.retain(|(constraint, origin)| {
                if !vars.covers([&constraint.ty]) {
                    return true;
                }
                vars.add(&constraint.field);
                accepted.push((constraint, origin));
                false
            });
            if accepted.len() == before {
                break;
            }
        }
        let mut fields = Vec::new();
        let mut sources = Vec::new();
        for (constraint, origin) in accepted {
            let field = constraint.map(|t| vars.generalize(t));
            if !fields.contains(&field) {
                fields.push(field);
                sources.push(Some(source_in_body(origin)));
            }
        }
        (fields.into(), sources.into(), rest.into_iter().map(|(_, origin)| *origin).collect())
    }

    /// The associated type constraints that become part of the type of the
    /// function with the body `def`: those about its types, like
    /// `a.Item = Bool`. The variables of the types they must be join `vars`.
    /// Those about other types are already reported through their class.
    fn function_assoc(
        &self,
        def: DefWithBodyId<'db>,
        vars: &mut Generalization,
    ) -> (Box<[AssocConstraint<'db>]>, Sources<'db>) {
        let mut out = Vec::new();
        let mut sources = Vec::new();
        for (constraint, origin) in self.pending_assoc.iter().filter(|(_, origin)| origin.body == def) {
            if !vars.covers(constraint.args.iter()) {
                continue;
            }
            vars.add(&constraint.ty);
            let generalized = AssocConstraint {
                alias: constraint.alias,
                args: constraint.args.iter().map(|t| vars.generalize(t)).collect(),
                ty: vars.generalize(&constraint.ty),
            };
            if !out.contains(&generalized) {
                out.push(generalized);
                sources.push(Some(source_in_body(origin)));
            }
        }
        (out.into(), sources.into())
    }

    /// The "at most" constraints that become part of the type of the
    /// function with the body `def`: those about its type's variables and
    /// its parameters, which its callers must meet.
    fn function_bounds(
        &self,
        db: &'db dyn Db,
        f: FunctionId<'db>,
        def: DefWithBodyId<'db>,
        vars: &Generalization,
    ) -> (Box<[AtMostConstraint<'db>]>, Sources<'db>) {
        // Those it assumes first, as for class constraints.
        let (mut out, mut sources): (Vec<_>, Vec<_>) = at_most_assumptions(db, f).into_iter().unzip();
        for (constraint, origin) in self.pending_at_most.iter().filter(|(_, origin)| origin.body == def) {
            let mut found = Vec::new();
            vars_in(constraint.held.ty(), &mut found);
            vars_in(&constraint.bound, &mut found);
            if !found.iter().all(|v| vars.vars.contains(v)) {
                continue;
            }
            let generalized = constraint.map(|t| vars.generalize(t));
            if !out.contains(&generalized) {
                out.push(generalized);
                sources.push(Some(source_in_body(origin)));
            }
        }
        (out.into(), sources.into())
    }

    /// The class constraints that become part of the type of `f`: those it
    /// assumes, and those of its body `def` about the types in `vars`, with
    /// where each comes from. Also gives the errors for those of its body
    /// about other types, which can't be decided.
    fn function_predicates(
        &self,
        db: &'db dyn Db,
        f: FunctionId<'db>,
        def: DefWithBodyId<'db>,
        vars: &Generalization,
    ) -> (Box<[ClassConstraint<'db>]>, Sources<'db>, Vec<TypeError<'db>>) {
        let mut predicates = assumptions_with_sources(db, f);
        let declared = predicates.len();
        let mut ambiguous = Vec::new();
        for (constraint, origin) in self.pending.iter().filter(|(_, origin)| origin.body == def) {
            if vars.covers(constraint.args.iter()) {
                let predicate = constraint.map(|t| vars.generalize(t));
                if !predicates.iter().any(|(p, _)| *p == predicate) {
                    predicates.push((predicate, Some(source_in_body(origin))));
                }
            } else if !constraint.args.iter().any(|t| t.any(&mut |t| *t == Ty::Error)) {
                // Not about a type that's unknown because of an error, which
                // is reported already.
                ambiguous.push(TypeError::Ambiguous { origin: *origin, constraint: constraint.clone() });
            }
        }
        // Leave out those of the body that another one implies, like
        // `Index[a]` next to `IndexMut[a]`.
        let implied: Vec<ClassConstraint<'db>> = predicates
            .iter()
            .flat_map(|(p, _)| with_superclasses(db, vec![p.clone()]).into_iter().filter(move |s| s != p))
            .collect();
        let mut index = 0;
        predicates.retain(|(p, _)| {
            index += 1;
            index <= declared || !implied.contains(p)
        });
        let (predicates, sources): (Vec<_>, Vec<_>) = predicates.into_iter().unzip();
        (predicates.into(), sources.into(), ambiguous)
    }
}

/// Where each of some constraints of a function's type comes from.
type Sources<'db> = Box<[Option<ConstraintSource<'db>>]>;

/// The source of a constraint of a function's type that its body has at
/// `origin`: the use that needs it.
fn source_in_body<'db>(origin: &Origin<'db>) -> ConstraintSource<'db> {
    ConstraintSource::Body { body: origin.body, node: origin.node }
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
        let paths = match expr {
            Expr::Path(path) => vec![path.clone()],
            // `x.f()` may call `f`, and `Vec.new` is a path.
            Expr::Field { expr: base, name } => {
                let mut paths = vec![Path::from_name(name.clone())];
                if let Expr::Path(base) = &body[*base] {
                    let mut segments = base.segments.to_vec();
                    segments.push(PathSegment { name: name.clone(), generic_args: None });
                    paths.push(Path { segments: segments.into() });
                }
                paths
            }
            Expr::Binary { op: BinaryOp::Custom(name), .. } => vec![Path::from_name(name.clone())],
            _ => continue,
        };
        let resolver = Resolver::for_expr(db, def, id);
        for path in paths {
            if let Some(ValueResolution::Value(ValueNs::Function(g))) = resolver.resolve_path_in_value_ns(db, &path)
                && !out.contains(&g)
            {
                out.push(g);
            }
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

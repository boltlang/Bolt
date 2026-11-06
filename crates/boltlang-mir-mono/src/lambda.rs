//! Lambda lifting: what each lambda of a body captures, so that a backend
//! can make it a function of its own, with its captures as its first
//! parameters, as Conduit has closures.
//!
//! A lambda is part of the body it's in: its types are the body's in each
//! instance, and the uses in it are the body's uses (see
//! [`crate::collect`]). So a lifted lambda is an instance and the lambda's
//! expression, and it has the instance's mutability parameters.
//!
//! A lambda captures a local of the body around it, not a copy of its
//! value: a local that's assigned is the same place inside and outside,
//! like a ref in Conduit.

use std::collections::HashSet;

use boltlang_hir_def::{
    body::{Body, DefWithBodyId, body},
    hir::{BindingId, Expr, ExprId, Pat, PatId},
    resolver::{Resolver, ValueNs, ValueResolution},
};

use boltlang_base_db::Db;

/// A lambda of a body.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::SalsaValue)]
pub struct Lambda {
    /// The lambda's expression.
    pub expr: ExprId,
    /// The locals of the code around the lambda that it uses, in the order
    /// it first uses them. A lambda in a lambda that uses a local from
    /// outside both makes the outer one capture it too.
    pub captures: Box<[BindingId]>,
}

/// The lambdas of the body `def`, in the order they're written.
#[salsa::tracked(returns(ref))]
pub fn body_lambdas<'db>(db: &'db dyn Db, def: DefWithBodyId<'db>) -> Vec<Lambda> {
    let body = body(db, def);
    let mut lambdas = Vec::new();
    walk(body, body.body_expr, &mut |e| {
        if let Expr::Lambda { .. } = &body[e] {
            lambdas.push(Lambda { expr: e, captures: captures(db, def, body, e).into() });
        }
    });
    lambdas
}

/// The locals the lambda `lambda` uses that it doesn't bind itself.
fn captures<'db>(
    db: &'db dyn Db,
    def: DefWithBodyId<'db>,
    body: &Body,
    lambda: ExprId,
) -> Vec<BindingId> {
    let mut bound = HashSet::new();
    walk(body, lambda, &mut |e| {
        for pat in body[e].pats() {
            bindings(body, pat, &mut bound);
        }
    });
    let mut out = Vec::new();
    walk(body, lambda, &mut |e| {
        if let Expr::Path(path) = &body[e]
            && let Some(ValueResolution::Value(ValueNs::Local(b))) =
                Resolver::for_expr(db, def, e).resolve_path_in_value_ns(db, path)
            && !bound.contains(&b)
            && !out.contains(&b)
        {
            out.push(b);
        }
    });
    out
}

/// Call `f` on `e` and every expression in it, outer ones first, in the
/// order they're written. A list, not recursion, so that deeply nested code
/// doesn't use up the stack.
fn walk(body: &Body, e: ExprId, f: &mut dyn FnMut(ExprId)) {
    let mut stack = vec![e];
    while let Some(e) = stack.pop() {
        f(e);
        let mut children = body[e].children();
        children.reverse();
        stack.extend(children);
    }
}

/// The bindings of the pattern `pat` and the patterns in it.
fn bindings(body: &Body, pat: PatId, out: &mut HashSet<BindingId>) {
    let mut stack = vec![pat];
    while let Some(p) = stack.pop() {
        if let Pat::Bind { id, .. } = &body[p] {
            out.insert(*id);
        }
        stack.extend(body[p].children());
    }
}

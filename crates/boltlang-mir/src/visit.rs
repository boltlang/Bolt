//! Walking a body.
//!
//! A [`Visitor`] reads a body, and a [`VisitorMut`] can change it. Both
//! visit each node through `visit_expr`, whose default visits its children
//! in the order they're evaluated, through [`walk_expr`] or
//! [`walk_expr_mut`]. A pass overrides `visit_expr` for the nodes it cares
//! about, and calls the walk for the others.
//!
//! A [`VisitorMut`] rewrites in place: it replaces a node with
//! [`Body::replace`], or adds new ones with [`Body::alloc`]. The walk reads
//! a node's children before it visits them, so a visitor can change the
//! node it's at, and the children it then visits are the old ones.

use crate::body::{Body, ExprId, ExprKind, Place, Stmt};

impl ExprKind<'_> {
    /// The node's children, in the order they're evaluated.
    pub fn children(&self) -> Vec<ExprId> {
        let mut out = Vec::new();
        let opt = |out: &mut Vec<ExprId>, e: &Option<ExprId>| out.extend(*e);
        match self {
            ExprKind::Literal(_)
            | ExprKind::Bool(_)
            | ExprKind::Local(_)
            | ExprKind::Item(_)
            | ExprKind::Closure { .. }
            | ExprKind::Continue { .. }
            | ExprKind::Unsupported(_) => {}
            ExprKind::Call { callee, args } => {
                out.push(*callee);
                out.extend(args.iter());
            }
            ExprKind::Construct { fields: es, .. } | ExprKind::Array { elems: es } => {
                out.extend(es.iter());
            }
            ExprKind::ArrayRepeat { value, count } => out.extend([*value, *count]),
            ExprKind::Field { base: e, .. }
            | ExprKind::Cast { expr: e }
            | ExprKind::Imm { expr: e }
            | ExprKind::Let { scrutinee: e, .. }
            | ExprKind::Loop { body: e, .. } => out.push(*e),
            ExprKind::Block { stmts, tail, .. } => {
                for s in stmts.iter() {
                    match s {
                        Stmt::Let { init, else_, .. } => {
                            opt(&mut out, init);
                            opt(&mut out, else_);
                        }
                        Stmt::Expr(e) => out.push(*e),
                    }
                }
                opt(&mut out, tail);
            }
            ExprKind::If { cond, then, else_ } => {
                out.extend([*cond, *then]);
                opt(&mut out, else_);
            }
            ExprKind::Logic { lhs, rhs, .. } => out.extend([*lhs, *rhs]),
            ExprKind::Match { scrutinee, arms, .. } => {
                out.push(*scrutinee);
                for arm in arms.iter() {
                    opt(&mut out, &arm.guard);
                    out.push(arm.body);
                }
            }
            ExprKind::While { cond, body, .. } => out.extend([*cond, *body]),
            ExprKind::For { iterable, body, .. } => out.extend([*iterable, *body]),
            ExprKind::Break { value, .. } | ExprKind::Return { value } => opt(&mut out, value),
            ExprKind::Assign { place, value, .. } => {
                match place {
                    Place::Local(_) => {}
                    Place::Field { base, .. } => out.push(*base),
                    Place::Index { base, index, .. } => out.extend([*base, *index]),
                }
                out.push(*value);
            }
        }
        out
    }
}

/// Reads a body: see the module's documentation.
pub trait Visitor<'db> {
    fn visit_expr(&mut self, body: &Body<'db>, e: ExprId) {
        walk_expr(self, body, e);
    }
}

/// Visit the children of `e`.
pub fn walk_expr<'db, V: Visitor<'db> + ?Sized>(v: &mut V, body: &Body<'db>, e: ExprId) {
    for child in body[e].kind.children() {
        v.visit_expr(body, child);
    }
}

/// Changes a body in place: see the module's documentation.
pub trait VisitorMut<'db> {
    fn visit_expr(&mut self, body: &mut Body<'db>, e: ExprId) {
        walk_expr_mut(self, body, e);
    }
}

/// Visit the children of `e`.
pub fn walk_expr_mut<'db, V: VisitorMut<'db> + ?Sized>(v: &mut V, body: &mut Body<'db>, e: ExprId) {
    for child in body[e].kind.children() {
        v.visit_expr(body, child);
    }
}

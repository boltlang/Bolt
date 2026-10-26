//! Which local variables and labels are visible where in a body, like
//! rust-analyzer's `ExprScopes`.
//!
//! Each scope has a parent and the bindings it adds. A `let` statement
//! starts a new scope, so a later `let x` shadows an earlier one, and the
//! initializer of `let x = x` still sees the old `x`.

use boltlang_base_db::Db;

use crate::{
    Name,
    arena::{Arena, ArenaMap, Idx},
    body::{Body, DefWithBodyId, body},
    hir::{BinaryOp, BindingId, Expr, ExprId, LabelId, LogicOp, Pat, PatId, Statement},
};

pub type ScopeId = Idx<ScopeData>;

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct ScopeData {
    pub parent: Option<ScopeId>,
    /// The bindings this scope adds, in order.
    pub entries: Vec<ScopeEntry>,
    /// The label of the loop or block this scope belongs to.
    pub label: Option<(LabelId, Name)>,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct ScopeEntry {
    pub name: Name,
    pub binding: BindingId,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::SalsaValue)]
pub struct ExprScopes {
    scopes: Arena<ScopeData>,
    scope_by_expr: ArenaMap<Expr, ScopeId>,
}

/// The scopes of the body of `def`.
#[salsa::tracked(returns(ref))]
pub fn expr_scopes<'db>(db: &'db dyn Db, def: DefWithBodyId<'db>) -> ExprScopes {
    ExprScopes::new(body(db, def))
}

impl ExprScopes {
    pub fn new(body: &Body) -> ExprScopes {
        let mut scopes = ExprScopes { scopes: Arena::default(), scope_by_expr: ArenaMap::default() };
        let root = scopes.new_scope(None);
        for &param in body.params.iter() {
            scopes.add_pat_bindings(body, root, param);
        }
        // Defaults and const blocks in parameter patterns see the parameters.
        for &param in body.params.iter() {
            scopes.walk_pat_exprs(body, param, root);
        }
        scopes.walk_expr(body, body.body_expr, root);
        scopes
    }

    /// The scope the body starts in, with the parameters.
    pub fn root(&self) -> ScopeId {
        Idx::new(0)
    }

    pub fn data(&self, scope: ScopeId) -> &ScopeData {
        &self.scopes[scope]
    }

    /// The scope an expression is evaluated in.
    pub fn scope_for(&self, expr: ExprId) -> Option<ScopeId> {
        self.scope_by_expr.get(expr).copied()
    }

    /// `scope` and its ancestors, innermost first.
    pub fn scope_chain(&self, scope: Option<ScopeId>) -> impl Iterator<Item = ScopeId> + '_ {
        std::iter::successors(scope, move |&s| self.scopes[s].parent)
    }

    /// The binding a name refers to in `scope`, if it's a local variable.
    pub fn resolve_name(&self, scope: ScopeId, name: &Name) -> Option<BindingId> {
        self.scope_chain(Some(scope))
            .find_map(|s| self.scopes[s].entries.iter().rev().find(|e| &e.name == name).map(|e| e.binding))
    }

    /// The label `'name` refers to in `scope`, or the innermost label if
    /// `name` is `None`.
    pub fn resolve_label(&self, scope: ScopeId, name: Option<&Name>) -> Option<LabelId> {
        self.scope_chain(Some(scope)).find_map(|s| match &self.scopes[s].label {
            Some((id, label)) if name.is_none_or(|n| n == label) => Some(*id),
            _ => None,
        })
    }

    fn new_scope(&mut self, parent: Option<ScopeId>) -> ScopeId {
        self.scopes.alloc(ScopeData { parent, entries: Vec::new(), label: None })
    }

    fn new_labeled_scope(&mut self, body: &Body, parent: ScopeId, label: Option<LabelId>) -> ScopeId {
        let scope = self.new_scope(Some(parent));
        self.scopes[scope].label = label.map(|id| (id, body[id].name.clone()));
        scope
    }

    fn add_binding(&mut self, body: &Body, scope: ScopeId, binding: BindingId) {
        let name = body[binding].name.clone();
        self.scopes[scope].entries.push(ScopeEntry { name, binding });
    }

    /// Add the bindings of a pattern. In `A(x) | B(x)` each alternative has
    /// its own `x`; the first one is added, and the type checker checks the
    /// others against it.
    fn add_pat_bindings(&mut self, body: &Body, scope: ScopeId, pat: PatId) {
        match &body[pat] {
            Pat::Bind { id, .. } => self.add_binding(body, scope, *id),
            Pat::Or(alternatives) => {
                if let Some(&first) = alternatives.first() {
                    self.add_pat_bindings(body, scope, first);
                }
                return;
            }
            // The ends of a range are literals or paths, not bindings.
            Pat::Range { .. } => return,
            _ => {}
        }
        for child in body[pat].children() {
            self.add_pat_bindings(body, scope, child);
        }
    }

    /// Walk the expressions inside a pattern, which are only const blocks.
    fn walk_pat_exprs(&mut self, body: &Body, pat: PatId, scope: ScopeId) {
        let mut stack = vec![pat];
        while let Some(pat) = stack.pop() {
            match &body[pat] {
                Pat::ConstBlock(expr) => {
                    self.walk_expr(body, *expr, scope);
                }
                pat => stack.extend(pat.children()),
            }
        }
    }

    /// Walk an expression evaluated in `scope`, and return the scope after
    /// it, which differs only for a `let` in a condition: its bindings are
    /// visible in the rest of the condition and in the branch it guards.
    fn walk_expr(&mut self, body: &Body, expr: ExprId, scope: ScopeId) -> ScopeId {
        self.scope_by_expr.insert(expr, scope);
        match &body[expr] {
            Expr::Let { pat, expr } => {
                self.walk_expr(body, *expr, scope);
                self.walk_pat_exprs(body, *pat, scope);
                let inner = self.new_scope(Some(scope));
                self.add_pat_bindings(body, inner, *pat);
                return inner;
            }
            // `a && let x = b && x > 0` chains bindings left to right.
            Expr::Binary { op: BinaryOp::Logic(LogicOp::And), lhs, rhs } => {
                let scope = self.walk_expr(body, *lhs, scope);
                return self.walk_expr(body, *rhs, scope);
            }
            Expr::Block { statements, tail, label, .. } => {
                let mut scope = self.new_labeled_scope(body, scope, *label);
                for statement in statements.iter() {
                    match statement {
                        Statement::Let { pat, type_ref: _, initializer, else_branch } => {
                            if let Some(init) = initializer {
                                self.walk_expr(body, *init, scope);
                            }
                            if let Some(else_branch) = else_branch {
                                self.walk_expr(body, *else_branch, scope);
                            }
                            self.walk_pat_exprs(body, *pat, scope);
                            scope = self.new_scope(Some(scope));
                            self.add_pat_bindings(body, scope, *pat);
                        }
                        Statement::Expr { expr, .. } => {
                            self.walk_expr(body, *expr, scope);
                        }
                        Statement::Item => {}
                    }
                }
                if let Some(tail) = tail {
                    self.walk_expr(body, *tail, scope);
                }
            }
            Expr::If { condition, then_branch, else_branch } => {
                let inner = self.walk_expr(body, *condition, scope);
                self.walk_expr(body, *then_branch, inner);
                if let Some(else_branch) = else_branch {
                    self.walk_expr(body, *else_branch, scope);
                }
            }
            Expr::While { condition, body: loop_body, label } => {
                let labeled = self.new_labeled_scope(body, scope, *label);
                let inner = self.walk_expr(body, *condition, labeled);
                self.walk_expr(body, *loop_body, inner);
            }
            Expr::Loop { body: loop_body, label } => {
                let labeled = self.new_labeled_scope(body, scope, *label);
                self.walk_expr(body, *loop_body, labeled);
            }
            Expr::For { pat, iterable, body: loop_body, label } => {
                self.walk_expr(body, *iterable, scope);
                self.walk_pat_exprs(body, *pat, scope);
                let inner = self.new_labeled_scope(body, scope, *label);
                self.add_pat_bindings(body, inner, *pat);
                self.walk_expr(body, *loop_body, inner);
            }
            Expr::Match { expr, arms } => {
                self.walk_expr(body, *expr, scope);
                for arm in arms.iter() {
                    self.walk_pat_exprs(body, arm.pat, scope);
                    let mut inner = self.new_scope(Some(scope));
                    self.add_pat_bindings(body, inner, arm.pat);
                    if let Some(guard) = arm.guard {
                        inner = self.walk_expr(body, guard, inner);
                    }
                    self.walk_expr(body, arm.expr, inner);
                }
            }
            Expr::Lambda { params, body: lambda_body, .. } => {
                for &param in params.iter() {
                    self.walk_pat_exprs(body, param, scope);
                }
                let inner = self.new_scope(Some(scope));
                for &param in params.iter() {
                    self.add_pat_bindings(body, inner, param);
                }
                self.walk_expr(body, *lambda_body, inner);
            }
            other => {
                // Everything else evaluates its parts in the same scope.
                for child in children(other) {
                    self.walk_expr(body, child, scope);
                }
            }
        }
        scope
    }
}

/// The subexpressions of an expression that doesn't introduce bindings.
fn children(expr: &Expr) -> Vec<ExprId> {
    let mut out = Vec::new();
    match expr {
        Expr::Missing | Expr::Path(_) | Expr::Literal(_) | Expr::Continue { .. } | Expr::MacroCall { .. } => {}
        Expr::Call { callee, args } => {
            out.push(*callee);
            out.extend(args.iter().copied());
        }
        Expr::Field { expr, .. }
        | Expr::Await { expr }
        | Expr::Try { expr }
        | Expr::Cast { expr, .. }
        | Expr::Prefix { expr, .. } => out.push(*expr),
        Expr::Index { base, index } => out.extend([*base, *index]),
        Expr::Binary { lhs, rhs, .. } => out.extend([*lhs, *rhs]),
        Expr::Range { lhs, rhs, .. } => out.extend(lhs.iter().chain(rhs.iter()).copied()),
        Expr::Tuple { exprs } | Expr::Array { exprs } => out.extend(exprs.iter().copied()),
        Expr::ArrayRepeat { value, count } => out.extend([*value, *count]),
        Expr::Record { fields, spread, .. } => {
            out.extend(fields.iter().map(|f| f.expr));
            out.extend(*spread);
        }
        Expr::Break { expr, .. } | Expr::Return { expr } | Expr::Yield { expr } => out.extend(*expr),
        // These are handled by walk_expr.
        Expr::Let { .. }
        | Expr::Block { .. }
        | Expr::If { .. }
        | Expr::While { .. }
        | Expr::Loop { .. }
        | Expr::For { .. }
        | Expr::Match { .. }
        | Expr::Lambda { .. } => unreachable!(),
    }
    out
}

#[cfg(test)]
mod tests;

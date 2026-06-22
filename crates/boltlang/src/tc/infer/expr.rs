use crate::{
    BlockExpr, CallExpr, Diagnostic, Expr, FunExpr, LitExpr, NamedExpr, Node, SourceElement, util::IterExt
};

use super::{
    Constraint,
    Constraints,
    InferContext,
    Provenance,
    SymbolKind,
    Type,
    TypeEnvId,
    UNIT_TYPE,
};

impl InferContext {

    pub fn check_expr(&mut self, expr: &Expr, ty: &Type, env: TypeEnvId) -> (Constraints, Vec<Diagnostic>) {
        // Attempt to handle some special cases
        match (expr, ty) {
            // Case where a literal expression is matched with a literal type
            (Expr::Lit(lit), ty) => if let Some(value) = lit.value() {
                if self.infer_literal(value) == *ty {
                    return (vec![], vec![]);
                }
            }
            // Case where a lambda expression is being compared with the type
            (Expr::Fun(fun), ty) => {
                let mut ds = Vec::new();
                let mut cs = Constraints::new();
                let new_env = self.fork_env(env);
                let mut ty = ty.clone();
                for pattern in fun.params() {
                    let (arg_ty, ret_ty) = match ty {
                        Type::Fun(arg_ty, ret_ty) => (arg_ty.as_ref().clone(), ret_ty.as_ref().clone()),
                        ty => {
                            let arg_ty: Type = self.fresh_type_var().into();
                            let ret_ty: Type = self.fresh_type_var().into();
                            cs.push(Constraint::TypesEqual {
                                provenance: Provenance::UnexpectedFun(fun.syntax().text_range().into()),
                                left: ty.clone(),
                                right: Type::fun(arg_ty.clone(), ret_ty.clone()),
                            });
                            (arg_ty, ret_ty)
                        }
                    };
                    let (param_cs, param_ds) = self.check_pattern(&pattern, &arg_ty, new_env.id(), env);
                    cs.extend(param_cs);
                    ds.extend(param_ds);
                    ty = ret_ty;
                }
                let (body_cs, body_ds) = self.check_expr(expr, &ty, new_env.id());
                cs.extend(body_cs);
                ds.extend(body_ds);
                self.drop_env(new_env);
                return (cs, ds)
            },
            // No special case, so the logic below will run
            _ => {},
        }
        // Fallback logic that just performs further downward inference
        let (actual_ty, mut cs, ds) = self.infer_expr(expr, env);
        cs.push(Constraint::TypesEqual {
            provenance: Provenance::ExpectedUnify(expr.syntax().text_range().into()),
            left: actual_ty,
            right: ty.clone(),
        });
        (cs, ds)
    }

    pub fn infer_named_expr(&mut self, expr: &NamedExpr, env: TypeEnvId) -> (Type, Constraints,  Vec<Diagnostic>) {
        match expr.name() {
            Some(name) => self.lookup(
                name.text(),
                name.text_range().into(),
                SymbolKind::Var,
                env
            ),
            None => (self.fresh_type_var().into(), vec![], vec![]),
        }
    }

    pub fn infer_lit_expr(&mut self, expr: &LitExpr, _env: TypeEnvId) -> (Type, Constraints, Vec<Diagnostic>) {
        match expr.value() {
            Some(lit) => (self.infer_literal(lit), vec![], vec![]),
            None => (self.fresh_type_var().into(), vec![], vec![]),
        }
    }

    pub fn infer_block_expr(&mut self, expr: &BlockExpr, env: TypeEnvId) -> (Type, Constraints, Vec<Diagnostic>) {
        let elements = match expr.block() {
            Some(block) => block.elements().collect(),
            None => vec![],
        };
        if elements.is_empty() {
            return (UNIT_TYPE.clone(), vec![], vec![]);
        }
        let mut cs = Constraints::new();
        let mut ds = Vec::new();
        for element in elements.iter().skip_last(1) {
            let (el_cs, el_ds) = self.infer_element(element, false, env);
            cs.extend(el_cs);
            ds.extend(el_ds);
        }
        let last = elements.last().unwrap();
        let ty = if let SourceElement::Expr(expr) = last {
            let (ty, ty_out, ty_ds) = self.infer_expr(&expr, env);
            cs.extend(ty_out);
            ds.extend(ty_ds);
            ty
        } else {
            UNIT_TYPE.clone()
        };
        (ty, cs, ds)
    }

    pub fn infer_fun_expr(&mut self, expr: &FunExpr, env: TypeEnvId) -> (Type, Constraints, Vec<Diagnostic>) {
        let mut cs = Constraints::new();
        let mut ds = Vec::new();
        let new_env = self.fork_env(env);
        let (mut ty, ty_cs, ty_ds) = match expr.body() {
            Some(expr) => self.infer_expr(&expr, env),
            None => (self.fresh_type_var().into(), vec![], vec![]),
        };
        cs.extend(ty_cs);
        ds.extend(ty_ds);
        for pattern in expr.params().collect::<Vec<_>>().into_iter().rev() {
            let (param_ty, param_cs, param_ds) = self.infer_pattern(&pattern, new_env.id(), env);
            cs.extend(param_cs);
            ds.extend(param_ds);
            ty = Type::fun(param_ty, ty);
        }
        self.drop_env(new_env);
        (ty, cs, ds)
    }

    pub fn infer_call_expr(&mut self, expr: &CallExpr, env: TypeEnvId) -> (Type, Constraints, Vec<Diagnostic>) {
        let (mut fun_ty, mut cs, mut ds) = match expr.operator() {
            Some(e) => self.infer_expr(&e, env),
            None => (self.fresh_type_var().into(), vec![], vec![]),
        };
        for arg in expr.args() {
            let (arg_ty, ret_ty_2) = match &fun_ty {
                Type::Fun(left, right) => (left.as_ref().clone(), right.as_ref().clone()),
                ty => {
                    let arg_ty: Type = self.fresh_type_var().into();
                    let ret_ty: Type = self.fresh_type_var().into();
                    cs.push(Constraint::TypesEqual {
                        provenance: Provenance::AppExpectedFun(expr.syntax().text_range().into()),
                        left: Type::fun(arg_ty.clone(), ret_ty.clone()),
                        right: ty.clone(),
                    });
                    (arg_ty, ret_ty)
                }
            };
            let (check_cs, check_ds) = self.check_expr(&arg, &arg_ty, env);
            cs.extend(check_cs);
            ds.extend(check_ds);
            fun_ty = ret_ty_2;
        }
        (fun_ty, cs, ds)
    }

    pub fn infer_expr(&mut self, expr: &Expr, env: TypeEnvId) -> (Type, Constraints, Vec<Diagnostic>) {
        match expr {
            Expr::Block(expr) => self.infer_block_expr(expr, env),
            Expr::Named(named) => self.infer_named_expr(named, env),
            Expr::Lit(lit) => self.infer_lit_expr(lit, env),
            Expr::Fun(fun) => self.infer_fun_expr(fun, env),
            Expr::Call(call) => self.infer_call_expr(call, env),
        }
    }

}

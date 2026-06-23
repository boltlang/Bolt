use crate::{
    BlockExpr, CallExpr, Expr, FunExpr, LitExpr, NamedExpr, Node, SourceElement, util::IterExt
};

use super::{
    Constraint,
    GenOut,
    InferContext,
    Provenance,
    SymbolKind,
    Type,
    TypeEnvId,
    UNIT_TYPE,
};

impl InferContext {

    pub fn check_expr(&mut self, expr: &Expr, ty: &Type, env: TypeEnvId) -> GenOut {
        // Attempt to handle some special cases
        match (expr, ty) {
            // Case where a literal expression is matched with a literal type
            (Expr::Lit(lit), ty) => if let Some(value) = lit.value() {
                if self.infer_literal(value) == *ty {
                    return GenOut::new();
                }
            }
            // Case where a lambda expression is being compared with the type
            (Expr::Fun(fun), ty) => {
                let mut out = GenOut::new();
                let new_env = self.fork_env(env);
                let mut ty = ty.clone();
                for pattern in fun.params() {
                    let (arg_ty, ret_ty) = match ty {
                        Type::Fun(arg_ty, ret_ty) => (arg_ty.as_ref().clone(), ret_ty.as_ref().clone()),
                        ty => {
                            let arg_ty: Type = self.fresh_type_var().into();
                            let ret_ty: Type = self.fresh_type_var().into();
                            out.add_constraint(Constraint::TypesEqual {
                                provenance: Provenance::UnexpectedFun(fun.syntax().text_range().into()),
                                left: ty.clone(),
                                right: Type::fun(arg_ty.clone(), ret_ty.clone()),
                            });
                            (arg_ty, ret_ty)
                        }
                    };
                    let param_out = self.check_pattern(&pattern, &arg_ty, new_env.id(), env);
                    out.extend(param_out);
                    ty = ret_ty;
                }
                let body_out = self.check_expr(expr, &ty, new_env.id());
                out.extend(body_out);
                self.drop_env(new_env);
                return out;
            },
            // No special case, so the logic below will run
            _ => {},
        }
        // Fallback logic that just performs further downward inference
        let (mut out, actual_ty) = self.infer_expr(expr, env);
        out.add_constraint(Constraint::TypesEqual {
            provenance: Provenance::ExpectedUnify(expr.syntax().text_range().into()),
            left: actual_ty,
            right: ty.clone(),
        });
        out
    }

    pub fn infer_named_expr(&mut self, expr: &NamedExpr, env: TypeEnvId) -> (GenOut, Type) {
        match expr.name() {
            Some(name) => self.lookup(
                name.text(),
                name.text_range().into(),
                SymbolKind::Var,
                env
            ),
            None => (GenOut::new(), self.fresh_type_var().into()),
        }
    }

    pub fn infer_lit_expr(&mut self, expr: &LitExpr, _env: TypeEnvId) -> (GenOut, Type) {
        match expr.value() {
            Some(lit) => (GenOut::new(), self.infer_literal(lit)),
            None => (GenOut::new(), self.fresh_type_var().into()),
        }
    }

    pub fn infer_block_expr(&mut self, expr: &BlockExpr, env: TypeEnvId) -> (GenOut, Type) {
        let elements = match expr.block() {
            Some(block) => block.elements().collect(),
            None => vec![],
        };
        if elements.is_empty() {
            return (GenOut::new(), UNIT_TYPE.clone());
        }
        let mut out = GenOut::new();
        for element in elements.iter().skip_last(1) {
            let el_out = self.infer_element(element, false, env);
            out.extend(el_out);
        }
        let last = elements.last().unwrap();
        let ty = if let SourceElement::Expr(expr) = last {
            let (ty_out, ty) = self.infer_expr(&expr, env);
            out.extend(ty_out);
            ty
        } else {
            UNIT_TYPE.clone()
        };
        (out, ty)
    }

    pub fn infer_fun_expr(&mut self, expr: &FunExpr, env: TypeEnvId) -> (GenOut, Type) {
        let mut out = GenOut::new();
        let new_env = self.fork_env(env);
        let (ty_out, mut ty) = match expr.body() {
            Some(expr) => self.infer_expr(&expr, env),
            None => (GenOut::new(), self.fresh_type_var().into()),
        };
        out.extend(ty_out);
        for pattern in expr.params().collect::<Vec<_>>().into_iter().rev() {
            let (param_out, param_ty) = self.infer_pattern(&pattern, new_env.id(), env);
            out.extend(param_out);
            ty = Type::fun(param_ty, ty);
        }
        self.drop_env(new_env);
        (out, ty)
    }

    pub fn infer_call_expr(&mut self, expr: &CallExpr, env: TypeEnvId) -> (GenOut, Type) {
        let (mut out, mut fun_ty) = match expr.operator() {
            Some(e) => self.infer_expr(&e, env),
            None => (GenOut::new(), self.fresh_type_var().into()),
        };
        for arg in expr.args() {
            let (arg_ty, ret_ty_2) = match &fun_ty {
                Type::Fun(left, right) => (left.as_ref().clone(), right.as_ref().clone()),
                ty => {
                    let arg_ty: Type = self.fresh_type_var().into();
                    let ret_ty: Type = self.fresh_type_var().into();
                    out.add_constraint(Constraint::TypesEqual {
                        provenance: Provenance::AppExpectedFun(expr.syntax().text_range().into()),
                        left: Type::fun(arg_ty.clone(), ret_ty.clone()),
                        right: ty.clone(),
                    });
                    (arg_ty, ret_ty)
                }
            };
            let check_out = self.check_expr(&arg, &arg_ty, env);
            out.extend(check_out);
            fun_ty = ret_ty_2;
        }
        (out, fun_ty)
    }

    pub fn infer_expr(&mut self, expr: &Expr, env: TypeEnvId) -> (GenOut, Type) {
        match expr {
            Expr::Block(expr) => self.infer_block_expr(expr, env),
            Expr::Named(named) => self.infer_named_expr(named, env),
            Expr::Lit(lit) => self.infer_lit_expr(lit, env),
            Expr::Fun(fun) => self.infer_fun_expr(fun, env),
            Expr::Call(call) => self.infer_call_expr(call, env),
        }
    }

}

use crate::{
    Diagnostic,
    Expr,
    FuncDecl,
    Node,
    Pattern,
    TypeExpr,
    VarDecl, tc::infer::GenOut,
};
use super::{
    Constraint,
    Constraints,
    InferContext,
    Provenance,
    SymbolKind,
    Type,
    TypeEnvId,
};

impl InferContext {

    fn infer_var_decl_like(
        &mut self,
        pattern: &Option<Pattern>,
        te: &Option<TypeExpr>,
        expr: &Option<Expr>,
        to_insert: TypeEnvId,
        generalize: bool,
        env: TypeEnvId
    ) -> (GenOut, Type) {
        let mut out = GenOut::new();
        let mut ty = None;
        if let Some(te) = te {
            let (te_ty_out, te_ty) = self.infer_type_expr(&te, env);
            out.extend(te_ty_out);
            ty = Some(te_ty);
        }
        if let Some(pattern) = pattern {
            match &ty {
                Some(ty) => {
                    let patt_out = self.check_pattern(&pattern, ty, to_insert, env);
                    out.extend(patt_out);
                }
                None => {
                    let (patt_out, patt_ty) = self.infer_pattern(&pattern, to_insert, env);
                    out.extend(patt_out);
                    ty = Some(patt_ty);
                }
            }
        }
        if let Some(expr) = expr {
            match &ty {
                Some(ty) => {
                    let expr_out = self.check_expr(&expr, &ty, env);
                    out.extend(expr_out);
                }
                None => {
                    let (expr_out, _expr_ty) = self.infer_expr(&expr, env);
                    out.extend(expr_out);
                }
            }
        }
        (out, ty.unwrap_or_else(|| self.fresh_type_var().into()))
    }

    pub fn infer_var_decl(&mut self, decl: &VarDecl, is_toplevel: bool, env: TypeEnvId) -> GenOut {
        self.infer_var_decl_like(
            &decl.pattern(),
            &decl.type_expr(),
            &decl.expr(),
            env,
            is_toplevel,
            env
        ).0
    }

    pub fn infer_func_decl(&mut self, node: &FuncDecl, env: TypeEnvId) -> GenOut {
        let mut out = GenOut::new();
        let new_env = self.fork_env(env);
        let ret_ty = match node.body() {
            None => self.fresh_type_var().into(),
            Some(expr) => {
                let (body_out, body_ty) = self.infer_expr(&expr, env);
                out.extend(body_out);
                body_ty
            }
        };
        let actual_ty = Type::signature(
            node.params()
                .map(|param| {
                    let (param_out, param_ty) = self.infer_var_decl_like(
                        &param.pattern(),
                        &param.type_expr(),
                        &param.default(),
                        new_env.id(),
                        false,
                        env
                    );
                    out.extend(param_out);
                    param_ty
                })
                .collect::<Vec<_>>(),
            ret_ty
        );
        if let Some(te) = node.type_signature() {
            let (sig_out, sig_ty) = self.infer_type_expr(&te, env);
            out.extend(sig_out);
            out.add_constraint(Constraint::TypesEqual {
                provenance: Provenance::TypeSignature(te.syntax().text_range().into()),
                left: actual_ty.clone(),
                right: sig_ty,
            });
        }
        if let Some(name) = node.name() {
            self.env_add(env, name.text(), SymbolKind::Var, self.generalize(actual_ty, env));
        }
        self.drop_env(new_env);
        out
    }

}

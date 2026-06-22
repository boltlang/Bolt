use crate::{
    Diagnostic,
    Expr,
    FuncDecl,
    Node,
    Pattern,
    TypeExpr,
    VarDecl,
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
    ) -> (Type, Constraints, Vec<Diagnostic>) {
        let mut cs = Constraints::new();
        let mut ds = Vec::new();
        let mut ty = None;
        if let Some(te) = te {
            let (te_ty, te_ty_cs, te_ty_ds) = self.infer_type_expr(&te, env);
            cs.extend(te_ty_cs);
            ds.extend(te_ty_ds);
            ty = Some(te_ty);
        }
        if let Some(pattern) = pattern {
            match &ty {
                Some(ty) => {
                    let (patt_cs, patt_ds) = self.check_pattern(&pattern, ty, to_insert, env);
                    cs.extend(patt_cs);
                    ds.extend(patt_ds);
                }
                None => {
                    let (patt_ty, patt_cs, patt_ds) = self.infer_pattern(&pattern, to_insert, env);
                    cs.extend(patt_cs);
                    ds.extend(patt_ds);
                    ty = Some(patt_ty);
                }
            }
        }
        if let Some(expr) = expr {
            match &ty {
                Some(ty) => {
                    let (expr_cs, expr_ds) = self.check_expr(&expr, &ty, env);
                    cs.extend(expr_cs);
                    ds.extend(expr_ds);
                }
                None => {
                    let (_expr_ty, expr_cs, expr_ds) = self.infer_expr(&expr, env);
                    cs.extend(expr_cs);
                    ds.extend(expr_ds);
                }
            }
        }
        (ty.unwrap_or_else(|| self.fresh_type_var().into()), cs, ds)
    }

    pub fn infer_var_decl(&mut self, decl: &VarDecl, is_toplevel: bool, env: TypeEnvId) -> (Constraints, Vec<Diagnostic>) {
        let (_ty, cs, ds) = self.infer_var_decl_like(
            &decl.pattern(),
            &decl.type_expr(),
            &decl.expr(),
            env,
            is_toplevel,
            env
        );
        (cs, ds)
    }

    pub fn infer_func_decl(&mut self, node: &FuncDecl, env: TypeEnvId) -> (Constraints, Vec<Diagnostic>) {
        let mut cs = Constraints::new();
        let mut ds = Vec::new();
        let new_env = self.fork_env(env);
        let ret_ty = match node.body() {
            None => self.fresh_type_var().into(),
            Some(expr) => {
                let (body_ty, body_cs, body_ds) = self.infer_expr(&expr, env);
                cs.extend(body_cs);
                ds.extend(body_ds);
                body_ty
            }
        };
        let actual_ty = Type::signature(
            node.params()
                .map(|param| {
                    let (param_ty, param_cs, param_ds) = self.infer_var_decl_like(
                        &param.pattern(),
                        &param.type_expr(),
                        &param.default(),
                        new_env.id(),
                        false,
                        env
                    );
                    cs.extend(param_cs);
                    ds.extend(param_ds);
                    param_ty
                })
                .collect::<Vec<_>>(),
            ret_ty
        );
        if let Some(te) = node.type_signature() {
            let (sig_ty, sig_cs, sig_ds) = self.infer_type_expr(&te, env);
            cs.extend(sig_cs);
            ds.extend(sig_ds);
            cs.push(Constraint::TypesEqual {
                provenance: Provenance::TypeSignature(te.syntax().text_range().into()),
                left: actual_ty.clone(),
                right: sig_ty,
            });
        }
        if let Some(name) = node.name() {
            self.env_add(env, name.text(), SymbolKind::Var, self.generalize(actual_ty, env));
        }
        self.drop_env(new_env);
        (cs, ds)
    }

}


use crate::{
    ArrowTypeExpr, Diagnostic, NamedTypeExpr, Node, TypeExpr
};

use super::{Type, TypeEnvId, Constraint, Provenance, InferContext, Constraints};

impl InferContext {

    pub fn check_type_expression(&mut self, te: &TypeExpr, ty: &Type, env: TypeEnvId) -> (Constraints, Vec<Diagnostic>) {
        let (te_ty, mut cs, ds) = self.infer_type_expr(&te, env);
        cs.push(Constraint::TypesEqual {
            provenance: Provenance::TypeSignature(te.syntax().text_range().into()),
            left: te_ty,
            right: ty.clone(),
        });
        (cs, ds)
    }

    pub fn infer_arrow_type_expr(&mut self, expr: &ArrowTypeExpr, env: TypeEnvId) -> (Type, Constraints, Vec<Diagnostic>) {
        let mut cs = Constraints::new();
        let mut ds = Vec::new();
        let ret_ty = match expr.return_ty() {
            None => self.fresh_type_var().into(),
            Some(te) => {
                let (ret_ty, ret_cs, ret_ds) = self.infer_type_expr(&te, env);
                cs.extend(ret_cs);
                ds.extend(ret_ds);
                ret_ty
            }
        };
        let ty = Type::signature(
            expr.params()
                .map(|te| {
                    let (param_ty, param_cs, param_ds) = self.infer_type_expr(&te, env);
                    cs.extend(param_cs);
                    ds.extend(param_ds);
                    param_ty
                })
                // FIXME params should return a DoubleEndedIterator
                .collect::<Vec<_>>(),
            ret_ty
        );
        (ty, cs, ds)
    }

    pub fn infer_named_type_expr(&mut self, expr: &NamedTypeExpr, env: TypeEnvId) -> (Type, Constraints, Vec<Diagnostic>) {
        match expr.name() {
            None => (self.fresh_type_var().into(), vec![], vec![]),
            Some(name) => self.lookup(
                name.text(),
                name.text_range().into(),
                super::SymbolKind::Type,
                env
            ),
        }
    }

    pub fn infer_type_expr(&mut self, te: &TypeExpr, env: TypeEnvId) -> (Type, Constraints, Vec<Diagnostic>) {
        match te {
            TypeExpr::Arrow(arrow) => self.infer_arrow_type_expr(arrow, env),
            TypeExpr::Named(named) => self.infer_named_type_expr(named, env),
        }
    }

}

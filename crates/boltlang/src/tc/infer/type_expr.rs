
use crate::{
    ArrowTypeExpr,
    NamedTypeExpr,
    Node,
    TypeExpr,
};

use super::{
    Constraint,
    GenOut,
    InferContext,
    Provenance,
    Type,
    TypeEnvId,
};

impl InferContext {

    pub fn check_type_expression(&mut self, te: &TypeExpr, ty: &Type, env: TypeEnvId) -> GenOut {
        let (mut out, te_ty) = self.infer_type_expr(&te, env);
        out.add_constraint(Constraint::TypesEqual {
            provenance: Provenance::TypeSignature(te.syntax().text_range().into()),
            left: te_ty,
            right: ty.clone(),
        });
        out
    }

    pub fn infer_arrow_type_expr(&mut self, expr: &ArrowTypeExpr, env: TypeEnvId) -> (GenOut, Type) {
        let mut out = GenOut::new();
        let ret_ty = match expr.return_ty() {
            None => self.fresh_type_var().into(),
            Some(te) => {
                let (ret_out, ret_ty) = self.infer_type_expr(&te, env);
                out.extend(ret_out);
                ret_ty
            }
        };
        let ty = Type::signature(
            expr.params()
                .map(|te| {
                    let (param_out, param_ty) = self.infer_type_expr(&te, env);
                    out.extend(param_out);
                    param_ty
                })
                // FIXME params should return a DoubleEndedIterator
                .collect::<Vec<_>>(),
            ret_ty
        );
        (out, ty)
    }

    pub fn infer_named_type_expr(&mut self, expr: &NamedTypeExpr, env: TypeEnvId) -> (GenOut, Type) {
        match expr.name() {
            None => (GenOut::new(), self.fresh_type_var().into()),
            Some(name) => self.lookup(
                name.text(),
                name.text_range().into(),
                super::SymbolKind::Type,
                env
            ),
        }
    }

    pub fn infer_type_expr(&mut self, te: &TypeExpr, env: TypeEnvId) -> (GenOut, Type) {
        match te {
            TypeExpr::Arrow(arrow) => self.infer_arrow_type_expr(arrow, env),
            TypeExpr::Named(named) => self.infer_named_type_expr(named, env),
        }
    }

}

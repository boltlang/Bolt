
use crate::{NamedPattern, Pattern, TypedPattern, tc::infer::core::GenOut};
use super::{Scheme, SymbolKind, InferContext, Type, TypeEnvId};

impl InferContext {

    pub fn check_pattern(&mut self, pattern: &Pattern, ty: &Type, to_insert: TypeEnvId, env: TypeEnvId) -> GenOut {
        match pattern {
            Pattern::Named(named) => {
                if let Some(name) = named.name() {
                    self.env_add(to_insert, name.text(), SymbolKind::Var, Scheme::mono(ty.clone()));
                }
                GenOut::new()
            },
            Pattern::Typed(typed) => {
                let mut out= GenOut::new();
                if let Some(p) = typed.pattern() {
                    let out_2 = self.check_pattern(&p, ty, to_insert, env);
                    out.extend(out_2);
                }
                if let Some(te) = typed.type_expression() {
                    let out_2 = self.check_type_expression(&te, ty, env);
                    out.extend(out_2);
                }
                out
            }
        }
    }

    pub fn infer_named_pattern(&mut self, pattern: &NamedPattern, to_insert: TypeEnvId, _env: TypeEnvId) -> (GenOut, Type) {
        let ty: Type = self.fresh_type_var().into();
        if let Some(name) = pattern.name() {
            self.env_add(to_insert, name.text().to_string(), SymbolKind::Var, Scheme::mono(ty.clone()));
        }
        (GenOut::new(), ty)
    }

    pub fn infer_typed_pattern(&mut self, pattern: &TypedPattern, to_insert: TypeEnvId, env: TypeEnvId) -> (GenOut, Type) {
        match (pattern.pattern(), pattern.type_expression()) {
            (Some(p), Some(te)) => {
                let (mut out, ty) = self.infer_type_expr(&te, env);
                out.extend(self.check_pattern(&p, &ty, to_insert, env));
                (out, ty)
            },
            (Some(p), None) => self.infer_pattern(&p, to_insert, env),
            (None, Some(te)) => self.infer_type_expr(&te, env),
            (None, None) => (GenOut::new(), self.fresh_type_var().into()),
        }
    }

    pub fn infer_pattern(&mut self, pattern: &Pattern, to_insert: TypeEnvId, env: TypeEnvId) -> (GenOut, Type) {
        match pattern {
            Pattern::Named(named) => self.infer_named_pattern(named, to_insert, env),
            Pattern::Typed(typed) => self.infer_typed_pattern(typed, to_insert, env),
        }
    }

}

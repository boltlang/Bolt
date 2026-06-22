
use crate::{Diagnostic, NamedPattern, Pattern, TypedPattern};
use super::{Scheme, SymbolKind, InferContext, Type, TypeEnvId, Constraints};

impl InferContext {

    pub fn check_pattern(&mut self, pattern: &Pattern, ty: &Type, to_insert: TypeEnvId, env: TypeEnvId) -> (Constraints, Vec<Diagnostic>) {
        match pattern {
            Pattern::Named(named) => {
                if let Some(name) = named.name() {
                    self.env_add(to_insert, name.text(), SymbolKind::Var, Scheme::mono(ty.clone()));
                }
                (vec![], vec![])
            },
            Pattern::Typed(typed) => {
                let mut cs = Vec::new();
                let mut ds = Vec::new();
                if let Some(p) = typed.pattern() {
                    let (cs_2, ds_2) = self.check_pattern(&p, ty, to_insert, env);
                    cs.extend(cs_2);
                    ds.extend(ds_2);
                }
                if let Some(te) = typed.type_expression() {
                    let (cs_2, ds_2) = self.check_type_expression(&te, ty, env);
                    cs.extend(cs_2);
                    ds.extend(ds_2);
                }
                (cs, ds)
            }
        }
    }

    pub fn infer_named_pattern(&mut self, pattern: &NamedPattern, to_insert: TypeEnvId, _env: TypeEnvId) -> (Type, Constraints, Vec<Diagnostic>) {
        let ty: Type = self.fresh_type_var().into();
        if let Some(name) = pattern.name() {
            self.env_add(to_insert, name.text().to_string(), SymbolKind::Var, Scheme::mono(ty.clone()));
        }
        (ty, vec![], vec![])
    }

    pub fn infer_typed_pattern(&mut self, pattern: &TypedPattern, to_insert: TypeEnvId, env: TypeEnvId) -> (Type, Constraints, Vec<Diagnostic>) {
        match (pattern.pattern(), pattern.type_expression()) {
            (Some(p), Some(te)) => {
                let (ty, mut cs, mut ds) = self.infer_type_expr(&te, env);
                let (cs_2, ds_2) = self.check_pattern(&p, &ty, to_insert, env);
                cs.extend(cs_2);
                ds.extend(ds_2);
                (ty, cs, ds)
            },
            (Some(p), None) => self.infer_pattern(&p, to_insert, env),
            (None, Some(te)) => self.infer_type_expr(&te, env),
            (None, None) => (self.fresh_type_var().into(), vec![], vec![]),
        }
    }

    pub fn infer_pattern(&mut self, pattern: &Pattern, to_insert: TypeEnvId, env: TypeEnvId) -> (Type, Constraints, Vec<Diagnostic>) {
        match pattern {
            Pattern::Named(named) => self.infer_named_pattern(named, to_insert, env),
            Pattern::Typed(typed) => self.infer_typed_pattern(typed, to_insert, env),
        }
    }

}

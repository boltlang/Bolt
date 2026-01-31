use ena::unify::InPlaceUnificationTable;

use crate::{diagnostic::TypeMismatchDiagnostic, tc::ConId, Diagnostic, Type};

use super::TVar;

pub struct Unifier {
    table: InPlaceUnificationTable<TVar>,
}

pub enum UnifyError {
    ConArgsLengthMismatch(ConId, Vec<Type>, Vec<Type>),
    TypeMismatch(Type, Type),
    OccursCheck(Type, TVar),
}

impl Unifier {

    pub fn new() -> Self {
        Unifier {
            table: InPlaceUnificationTable::new(),
        }
    }

    pub fn fresh_type_var(&mut self) -> TVar {
        self.table.new_key(None)
    }

    pub fn unify_type_type(&mut self, a: &Type, b: &Type) -> Vec<UnifyError> {
        let mut out = Vec::new();
        self.unify_type_type_impl(a, b, &mut out);
        out
    }

    fn unify_type_type_impl(&mut self, a: &Type, b: &Type, out: &mut Vec<UnifyError>) {
        match (a, b) {
            (Type::Con(a_id, a_args), Type::Con(b_id, b_args)) if a_id == b_id => {
                if a_args.len() != b_args.len() {
                   out.push(UnifyError::ConArgsLengthMismatch(a_id.clone(), a_args.clone(), b_args.clone()));
                }
                for (a_arg, b_arg) in a_args.iter().zip(b_args) {
                    self.unify_type_type_impl(a_arg, b_arg, out);
                }
            }
            (Type::Fun(a_arg, a_ret), Type::Fun(b_arg, b_ret)) => {
                self.unify_type_type_impl(a_arg, b_arg, out);
                self.unify_type_type_impl(a_ret, b_ret, out);
            }
            (Type::UniVar(left), Type::UniVar(right)) => {
                if let Err((l, r)) = self.table.unify_var_var(left.clone(), right.clone()) {
                    out.push(UnifyError::TypeMismatch(l, r));
                }
            }
            (Type::UniVar(var), ty) | (ty, Type::UniVar(var)) => {
                if ty.has_uni_var(var) {
                    out.push(UnifyError::OccursCheck(ty.clone(), var.clone()));
                    return;
                }
                if let Err((l, r)) = self.table.unify_var_value(var.clone(), Some(ty.clone())) {
                    out.push(UnifyError::TypeMismatch(l, r));
                }
            }
            _ => out.push(UnifyError::TypeMismatch(a.clone(), b.clone())),
        }
    }

    pub fn normalize_type(&mut self, ty: Type) -> Type {
        match ty {
            Type::UniVar(var) => 
                self.table.probe_value(var).unwrap_or(ty),
            Type::Fun(left, right) =>
                Type::fun(self.normalize_type(*left), self.normalize_type(*right)),
            Type::Con(id, args) =>
                Type::Con(id, args.into_iter().map(|t| self.normalize_type(t)).collect()),
        }
    }

    pub fn normalize_diagnostic(&mut self, diagnostic: Diagnostic) -> Diagnostic {
        match diagnostic {
            Diagnostic::TypeMismatch(TypeMismatchDiagnostic {
                checked,
                inferred,
                provenance,
            }) => Diagnostic::TypeMismatch(TypeMismatchDiagnostic {
                checked: self.normalize_type(checked),
                inferred: self.normalize_type(inferred),
                provenance,
            }),
            diag => diag,
        }
    }

}

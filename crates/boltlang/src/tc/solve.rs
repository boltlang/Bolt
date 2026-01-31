use crate::{Diagnostic, diagnostic::{AppExpectedFunDiagnostic, ConArgsLengthMismatchDiagnostic, Diagnostics, ExpectedUnifyDiagnostic, InfiniteTypeDiagnostic, UnexpectedFunDiagnostic}, tc::{infer::{Constraint, Provenance}, unify::{Unifier, UnifyError}}};

pub struct Solver {
    pub unifier: Unifier,
}

impl Solver {

    pub fn new() -> Self {
        Self {
            unifier: Unifier::new(),
        }
    }

    pub fn add(&mut self, constraint: &Constraint, diagnostics: &mut dyn Diagnostics) {
        match constraint {
            Constraint::TypesEqual { provenance, left, right } => {
                for diag in self.unifier.unify_type_type(&left, &right) {
                    diagnostics.add(match diag {
                        UnifyError::OccursCheck(ty, var) => 
                            InfiniteTypeDiagnostic {
                                // provenance.node(), // TODO
                                ty,
                                var
                            }.into(),
                        UnifyError::ConArgsLengthMismatch(id, a_args, b_args) =>
                            ConArgsLengthMismatchDiagnostic::new(id, a_args, b_args).into(),
                        UnifyError::TypeMismatch(a, b) => match provenance {
                            Provenance::UnexpectedFun(node) =>
                                UnexpectedFunDiagnostic {
                                    // node, // TODO
                                    expected_ty: a,
                                    fun_ty: b,
                                }.into(),
                            Provenance::AppExpectedFun(node) =>
                                AppExpectedFunDiagnostic {
                                    // node, // TODO
                                    inferred_ty: a,
                                    expected_fun_ty: b,
                                }.into(),
                            Provenance::ExpectedUnify(node) =>
                                ExpectedUnifyDiagnostic {
                                    // node, // TODO,
                                    inferred: a,
                                    checked: b,
                                }.into(),
                        }
                    });
                }
            }
        }
    }

    pub fn solve(&mut self) {
        // noop
    }

}

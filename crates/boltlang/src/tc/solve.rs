use crate::{
    diagnostic::{
        AppExpectedFunDiagnostic,
        ConArgsLengthMismatchDiagnostic,
        ExpectedUnifyDiagnostic,
        InfiniteTypeDiagnostic,
        UnexpectedFunDiagnostic,
        UnmatchedTypeSignatureDiagnostic
    },
    tc::{infer::{Constraint, Provenance}, unify::{Unifier, UnifyError}},
    Diagnostic
};

pub struct Solver {
    diagnostics: Vec<Diagnostic>,
    pub unifier: Unifier,
}

impl Solver {

    pub fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
            unifier: Unifier::new(),
        }
    }

    pub fn take_diagnostics(&mut self) -> Vec<Diagnostic> {
        std::mem::take(&mut self.diagnostics)
    }

    pub fn add(&mut self, constraint: &Constraint) {
        match constraint {
            Constraint::TypesEqual { provenance, left, right } => {
                for diag in self.unifier.unify_type_type(&left, &right) {
                    self.diagnostics.push(match diag {
                        UnifyError::OccursCheck(ty, var) => 
                            InfiniteTypeDiagnostic {
                                source: provenance.source().clone(),
                                ty,
                                var
                            }.into(),
                        UnifyError::ConArgsLengthMismatch(id, a_args, b_args) =>
                            ConArgsLengthMismatchDiagnostic {
                                source: provenance.source().clone(),
                                id,
                                a_args,
                                b_args
                            }.into(),
                        UnifyError::TypeMismatch(a, b) => match provenance {
                            Provenance::TypeSignature(source) =>
                                UnmatchedTypeSignatureDiagnostic {
                                    source: source.clone(),
                                    sig_ty: a,
                                    actual_ty: b,
                                }.into(),
                            Provenance::UnexpectedFun(source) =>
                                UnexpectedFunDiagnostic {
                                    source: source.clone(),
                                    expected_ty: a,
                                    fun_ty: b,
                                }.into(),
                            Provenance::AppExpectedFun(source) =>
                                AppExpectedFunDiagnostic {
                                    source: source.clone(),
                                    inferred_ty: a,
                                    expected_fun_ty: b,
                                }.into(),
                            Provenance::ExpectedUnify(source) =>
                                ExpectedUnifyDiagnostic {
                                    source: source.clone(),
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

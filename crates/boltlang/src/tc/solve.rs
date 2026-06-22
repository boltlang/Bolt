use crate::{
    diagnostic::{
        ConArgsLengthMismatchDiagnostic,
        InfiniteTypeDiagnostic, TypeMismatchDiagnostic,
        Span,
    },
    tc::{
        types::Type,
        unify::{Unifier, UnifyError}
    },
    Diagnostic
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Provenance {
    TypeSignature(Span),
    AppExpectedFun(Span),
    UnexpectedFun(Span),
    ExpectedUnify(Span),
}

impl Provenance {

    pub fn span(&self) -> &Span {
        match self {
            Provenance::TypeSignature(span) => span,
            Provenance::AppExpectedFun(span) => span,
            Provenance::UnexpectedFun(span) => span,
            Provenance::ExpectedUnify(span) => span,
        }
    }

}

pub enum Constraint {
    TypesEqual {
        provenance: Provenance,
        left: Type,
        right: Type,
    },
}

pub struct Solver {
    pub unifier: Unifier,
}

impl Solver {

    pub fn new() -> Self {
        Self {
            unifier: Unifier::new(),
        }
    }

    pub fn add(&mut self, constraint: &Constraint) -> Vec<Diagnostic> {
        let mut out = Vec::new();
        match constraint {
            Constraint::TypesEqual { provenance, left, right } => {
                for diag in self.unifier.unify_type_type(&left, &right) {
                    out.push(match diag {
                        UnifyError::OccursCheck(ty, var) => 
                            InfiniteTypeDiagnostic {
                                span: provenance.span().clone(),
                                ty,
                                var
                            }.into(),
                        UnifyError::ConArgsLengthMismatch(id, a_args, b_args) =>
                            ConArgsLengthMismatchDiagnostic {
                                span: provenance.span().clone(),
                                id,
                                a_args,
                                b_args
                            }.into(),
                        UnifyError::TypeMismatch(_a, _b) => TypeMismatchDiagnostic {
                                // TODO mark a and b inside left and right
                                inferred: left.clone(),
                                checked: right.clone(),
                                provenance: provenance.clone(),
                            }.into(),
                    });
                }
            }
        }
        out
    }

    pub fn solve(&mut self) {
        // noop
    }

}

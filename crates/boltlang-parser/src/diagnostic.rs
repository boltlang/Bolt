
use boltlang_common::Span;
use serde::{Deserialize, Serialize};

pub struct Diagnostics {
    buffer: Vec<Diagnostic>,
}

impl Diagnostics {

    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
        }
    }

    pub fn push(&mut self, diag: Diagnostic) {
        self.buffer.push(diag);
    }

    pub fn take_diagnostics(&mut self) -> Vec<Diagnostic> {
        std::mem::take(&mut self.buffer)
    }

}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum Diagnostic {
    ExpectedToken(ExpectedTokenDiagnostic),
    UnexpectedChar(UnexpectedCharDiagnostic),
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct UnexpectedCharDiagnostic {
    pub message: String,
    pub span: Span,
}

impl From<UnexpectedCharDiagnostic> for Diagnostic {
    fn from(value: UnexpectedCharDiagnostic) -> Self {
        Diagnostic::UnexpectedChar(value)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExpectedTokenDiagnostic {
    pub message: String,
    pub span: Span,
}

impl From<ExpectedTokenDiagnostic> for Diagnostic {
    fn from(value: ExpectedTokenDiagnostic) -> Self {
        Diagnostic::ExpectedToken(value)
    }
}

impl ExpectedTokenDiagnostic {
    pub fn new<S: Into<String>>(message: S, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }
}

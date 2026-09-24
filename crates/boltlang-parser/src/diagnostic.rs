
use std::fmt::Display;

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

impl Diagnostic {

    pub fn span(&self) -> &Span {
        match self {
            Self::ExpectedToken(diag) => &diag.span,
            Self::UnexpectedChar(diag) => &diag.span,
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::ExpectedToken(diag) => diag.message.clone(),
            Self::UnexpectedChar(diag) => diag.message.clone(),
        }
    }

}

impl Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExpectedToken(diag) => std::fmt::Display::fmt(diag, f),
            Self::UnexpectedChar(diag) => std::fmt::Display::fmt(diag, f),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct UnexpectedCharDiagnostic {
    pub message: String,
    pub span: Span,
}

impl Display for UnexpectedCharDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
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

impl Display for ExpectedTokenDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
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

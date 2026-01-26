
/// Internal representation of an error generated during parsing.
// pub type Diagnostic = String;

use std::fmt::{Debug, Display};

pub type Span = std::ops::Range<usize>;

pub const CODE_SYNTAX_ERROR: u16 = 1;

/// Public-facing parse error.
#[salsa::accumulator]
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Diagnostic {
    inner: InnerDiagnostic,
}

impl Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.inner, f)
    }
}

impl Diagnostic {

    pub fn syntax_error(message: String, offset: usize) -> Self {
        Self { inner: InnerDiagnostic::Syntax(SyntaxDiagnostic { message, offset }) }
    }

    pub fn code(&self) -> u16 {
        self.inner.code()
    }

    pub fn offset(&self) -> Option<usize> {
        self.inner.offset()
    }

}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum InnerDiagnostic {
    Syntax(SyntaxDiagnostic),
}

impl Display for InnerDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Syntax(diag) => std::fmt::Display::fmt(diag, f),
        }
    }
}

impl InnerDiagnostic {
    fn code(&self) -> u16 {
        match self {
            Self::Syntax(diag) => diag.code(),
        }
    }
    fn offset(&self) -> Option<usize> {
        match self {
            Self::Syntax(diag) => Some(diag.offset),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SyntaxDiagnostic {
    message: String,
    offset: usize,
}

impl Display for SyntaxDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl SyntaxDiagnostic {
    fn code(&self) -> u16 {
        CODE_SYNTAX_ERROR
    }
}


use std::fmt::{Debug, Display};

use crate::File;

pub type Span = std::ops::Range<usize>;

pub const CODE_SYNTAX_ERROR: u16 = 1;
pub const CODE_BINDING_NOT_FOUND: u16 = 2;

#[salsa::accumulator]
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Diagnostic {
    pub data: DiagnosticData,
}

impl Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.data, f)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Severity {
    Info,
    Warn,
    Error,
    Fatal,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Source {
    file: File,
    span: Span,
}

impl Source {

    pub fn new(file: File, span: Span) -> Self {
        Self { file, span }
    }

    pub fn file(&self) -> File {
        self.file

    }

    pub fn span(&self) -> &Span {
        &self.span
    }

}

impl Diagnostic {

    pub fn syntax_error(message: String, file: File, offset: usize) -> Self {
        Self { data: DiagnosticData::Syntax(SyntaxDiagnostic { message, offset, file }) }
    }

    pub fn binding_not_found(name: String, source: Source) -> Self {
        Self { data: DiagnosticData::BindingNotFound(BindingNotFoundDiagnostic { name, source }) }
    }

    pub fn code(&self) -> u16 {
        self.data.code()
    }

    pub fn message(&self) -> String {
        self.data.message()
    }

    pub fn severity(&self) -> Severity {
        self.data.severity()
    }

    pub fn source(&self) -> Option<Source> {
        self.data.source()
    }

}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum DiagnosticData {
    Syntax(SyntaxDiagnostic),
    BindingNotFound(BindingNotFoundDiagnostic),
}

impl Display for DiagnosticData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Syntax(diag) => std::fmt::Display::fmt(diag, f),
            Self::BindingNotFound(diag) => std::fmt::Display::fmt(diag, f),
        }
    }
}

impl DiagnosticData {

    pub fn message(&self) -> String {
        format!("{}", self)
    }

    pub fn code(&self) -> u16 {
        match self {
            Self::Syntax(diag) => diag.code(),
            Self::BindingNotFound(diag) => diag.code(),
        }
    }

    pub fn severity(&self) -> Severity {
        Severity::Error
    }

    pub fn source(&self) -> Option<Source> {
        match self {
            Self::Syntax(diag) => Some(Source::new(diag.file, diag.offset..diag.offset)),
            Self::BindingNotFound(diag) => Some(diag.source.clone()),
        }
    }

}

/// Public-facing parse error.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SyntaxDiagnostic {
    message: String,
    offset: usize,
    file: File,
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

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BindingNotFoundDiagnostic {
    name: String,
    source: Source,
}

impl Display for BindingNotFoundDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "name '{}' was not found", self.name)
    }
}

impl BindingNotFoundDiagnostic {
    fn code(&self) -> u16 {
        CODE_BINDING_NOT_FOUND
    }
}

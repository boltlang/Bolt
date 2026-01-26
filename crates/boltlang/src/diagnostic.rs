
/// Internal representation of an error generated during parsing.
// pub type Diagnostic = String;

use std::fmt::{format, Debug, Display};

use crate::File;

pub type Span = std::ops::Range<usize>;

pub const CODE_SYNTAX_ERROR: u16 = 1;

/// Public-facing parse error.
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

#[derive(Clone, Debug)]
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
}

impl Display for DiagnosticData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Syntax(diag) => std::fmt::Display::fmt(diag, f),
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
        }
    }

    pub fn severity(&self) -> Severity {
        match self {
            Self::Syntax(_) => Severity::Error,
        }
    }

    pub fn source(&self) -> Option<Source> {
        match self {
            Self::Syntax(diag) => Some(Source::new(diag.file, diag.offset..diag.offset)),
        }
    }

}

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

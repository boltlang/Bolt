
use std::fmt::{Debug, Display};

use crate::{File, Type, tc::{ConId, SymbolKind, TVar}};

pub type Span = std::ops::Range<usize>;

pub const CODE_SYNTAX_ERROR: u16 = 1;
pub const CODE_BINDING_NOT_FOUND: u16 = 2;
pub const CODE_UNEXPECTED_FUN: u16 = 3;
pub const CODE_APP_EXPECTED_FUN: u16 = 4;
pub const CODE_EXPECTED_UNIFY: u16 = 5;
pub const CODE_INFINITE_TYPE: u16 = 6;
pub const CODE_CON_ARGS_LENGTH_MISMATCH: u16 = 7;

#[salsa::accumulator]
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DbDiagnostic {
    pub data: Diagnostic,
}

impl DbDiagnostic {

    pub fn new(data: Diagnostic) -> DbDiagnostic {
        DbDiagnostic { data }
    }
}

impl Display for DbDiagnostic {
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

impl DbDiagnostic {

    pub fn code(&self) -> u16 {
        self.data.code()
    }

    pub fn message(&self) -> String {
        format!("{}", self.data)
    }

    pub fn severity(&self) -> Severity {
        self.data.severity()
    }

    pub fn source(&self) -> Option<Source> {
        self.data.source()
    }

}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Diagnostic {
    BindingNotFound(BindingNotFoundDiagnostic),
    SyntaxDiagnostic(SyntaxDiagnostic),
    ExpectedUnify(ExpectedUnifyDiagnostic),
    AppExpectedFun(AppExpectedFunDiagnostic),
    UnexpectedFun(UnexpectedFunDiagnostic),
    InfiniteType(InfiniteTypeDiagnostic),
    ConArgsLengthMismatch(ConArgsLengthMismatchDiagnostic),
}

impl Diagnostic {

    fn code(&self) -> u16 {
        match self {
            Self::SyntaxDiagnostic(diag) => diag.code(),
            Self::BindingNotFound(diag) => diag.code(),
            Self::ExpectedUnify(diag) => diag.code(),
            Self::AppExpectedFun(diag) => diag.code(),
            Self::UnexpectedFun(diag) => diag.code(),
            Self::InfiniteType(diag) => diag.code(),
            Self::ConArgsLengthMismatch(diag) => diag.code(),
        }
    }

    fn severity(&self) -> Severity {
        match self {
            Self::SyntaxDiagnostic(diag) => diag.severity(),
            Self::BindingNotFound(diag) => diag.severity(),
            Self::ExpectedUnify(diag) => diag.severity(),
            Self::AppExpectedFun(diag) => diag.severity(),
            Self::UnexpectedFun(diag) => diag.severity(),
            Self::InfiniteType(diag) => diag.severity(),
            Self::ConArgsLengthMismatch(diag) => diag.severity(),
        }
    }

    fn source(&self) -> Option<Source> {
        match self {
            Self::SyntaxDiagnostic(diag) => diag.source(),
            Self::BindingNotFound(diag) => diag.source(),
            Self::ExpectedUnify(diag) => diag.source(),
            Self::AppExpectedFun(diag) => diag.source(),
            Self::UnexpectedFun(diag) => diag.source(),
            Self::InfiniteType(diag) => diag.source(),
            Self::ConArgsLengthMismatch(diag) => diag.source(),
        }
    }

}

impl Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SyntaxDiagnostic(diag) => std::fmt::Display::fmt(diag, f),
            Self::BindingNotFound(diag) => std::fmt::Display::fmt(diag, f),
            Self::ExpectedUnify(diag) => std::fmt::Display::fmt(diag, f),
            Self::AppExpectedFun(diag) => std::fmt::Display::fmt(diag, f),
            Self::UnexpectedFun(diag) => std::fmt::Display::fmt(diag, f),
            Self::InfiniteType(diag) => std::fmt::Display::fmt(diag, f),
            Self::ConArgsLengthMismatch(diag) => std::fmt::Display::fmt(diag, f),
        }
    }
}

/// Public-facing parse error.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SyntaxDiagnostic {
    pub message: String,
    pub offset: usize,
    pub file: File,
}

impl SyntaxDiagnostic {

    pub fn new(message: String, offset: usize, file: File) -> Self {
        Self {
            message,
            offset,
            file,
        }
    }

    fn code(&self) -> u16 {
        CODE_SYNTAX_ERROR
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn source(&self) -> Option<Source> {
        Some(Source::new(self.file, self.offset..self.offset))
    }

}

impl Display for SyntaxDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl From<SyntaxDiagnostic> for Diagnostic {
    fn from(value: SyntaxDiagnostic) -> Self {
        Diagnostic::SyntaxDiagnostic(value)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BindingNotFoundDiagnostic {
    pub name: String,
    pub kind: SymbolKind,
    pub source: Source,
}

impl BindingNotFoundDiagnostic {

    pub fn new(name: String, kind: SymbolKind, source: Source) -> Self {
        BindingNotFoundDiagnostic {
            name,
            kind,
            source
        }
    }

    fn code(&self) -> u16 {
        CODE_BINDING_NOT_FOUND
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn source(&self) -> Option<Source> {
        Some(self.source.clone())
    }

}

impl Display for BindingNotFoundDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "name '{}' was not found", self.name)
    }
}

impl From<BindingNotFoundDiagnostic> for Diagnostic {
    fn from(value: BindingNotFoundDiagnostic) -> Self {
        Diagnostic::BindingNotFound(value)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ExpectedUnifyDiagnostic {
    pub checked: Type,
    pub inferred: Type,
}

impl ExpectedUnifyDiagnostic {

    fn code(&self) -> u16 {
        CODE_EXPECTED_UNIFY
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn source(&self) -> Option<Source> {
        None // FIXME
    }

}

impl Display for ExpectedUnifyDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "expected {} but got {}", self.inferred, self.checked)
    }
}

impl From<ExpectedUnifyDiagnostic> for Diagnostic {
    fn from(value: ExpectedUnifyDiagnostic) -> Self {
        Diagnostic::ExpectedUnify(value)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct AppExpectedFunDiagnostic {
    pub inferred_ty: Type,
    pub expected_fun_ty: Type,
}

impl AppExpectedFunDiagnostic {

    fn code(&self) -> u16 {
        CODE_APP_EXPECTED_FUN
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn source(&self) -> Option<Source> {
        None // FIXME
    }
}

impl Display for AppExpectedFunDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "expected {} to be a function type applicable to {}", self.expected_fun_ty, self.inferred_ty)
    }
}

impl From<AppExpectedFunDiagnostic> for Diagnostic {
    fn from(value: AppExpectedFunDiagnostic) -> Self {
        Diagnostic::AppExpectedFun(value)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct UnexpectedFunDiagnostic {
    pub expected_ty: Type,
    pub fun_ty: Type,
}

impl Display for UnexpectedFunDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "expected data type {} but got a function {}", self.expected_ty, self.fun_ty)
    }
}

impl From<UnexpectedFunDiagnostic> for Diagnostic {
    fn from(value: UnexpectedFunDiagnostic) -> Self {
        Diagnostic::UnexpectedFun(value)
    }
}

impl UnexpectedFunDiagnostic {

    fn code(&self) -> u16 {
        CODE_UNEXPECTED_FUN
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn source(&self) -> Option<Source> {
        None // FIXME
    }

}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct InfiniteTypeDiagnostic {
    pub ty: Type,
    pub var: TVar,
}

impl InfiniteTypeDiagnostic {

    fn code(&self) -> u16 {
        CODE_INFINITE_TYPE
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn source(&self) -> Option<Source> {
        None // FIXME
    }

}

impl From<InfiniteTypeDiagnostic> for Diagnostic {
    fn from(value: InfiniteTypeDiagnostic) -> Self {
        Diagnostic::InfiniteType(value)
    }
}

impl Display for InfiniteTypeDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unifying {} and {} would lead to an infinite type", self.var, self.ty)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConArgsLengthMismatchDiagnostic {
    id: ConId,
    a_args: Vec<Type>,
    b_args: Vec<Type>,
}

impl ConArgsLengthMismatchDiagnostic {

    pub fn new(id: ConId, a_args: Vec<Type>, b_args: Vec<Type>) -> Self {
        Self {
            id,
            a_args,
            b_args,
        }
    }

    fn code(&self) -> u16 {
        CODE_CON_ARGS_LENGTH_MISMATCH
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn source(&self) -> Option<Source> {
        None // FIXME
    }

}

impl Display for ConArgsLengthMismatchDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let a = Type::Con(self.id.clone(), self.a_args.clone());
        let b = Type::Con(self.id.clone(), self.b_args.clone());
        let i = self.b_args.len() as isize - self.a_args.len() as isize;
        if i == 1 {
            write!(f, "{} has one less type argument than {}", a, b)
        } else if i == -1 {
            write!(f, "{} has one more type argument than {}", a, b)
        } else if i < 0 {
            write!(f, "{} has {} more type argument than {}", a, i, b)
        } else {
            write!(f, "{} has {} less type argument than {}", a, i, b)
        }
    }
}

impl From<ConArgsLengthMismatchDiagnostic> for Diagnostic {
    fn from(value: ConArgsLengthMismatchDiagnostic) -> Self {
        Diagnostic::ConArgsLengthMismatch(value)
    }
}

pub trait Diagnostics {
    fn add(&mut self, diag: Diagnostic);
}

pub struct DiagnosticStore {
    storage: Vec<Diagnostic>,
}

impl DiagnosticStore {

    pub fn new() -> Self {
        Self { storage: Vec::new() }
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.storage
    }

    pub fn take_diagnostics(&mut self) -> Vec<Diagnostic> {
        std::mem::take(&mut self.storage)
    }

}

impl Diagnostics for DiagnosticStore {
    fn add(&mut self, diag: Diagnostic) {
        self.storage.push(diag);
    }
}


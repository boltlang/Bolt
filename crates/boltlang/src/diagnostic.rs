
use std::fmt::{Debug, Display};

use crate::{tc::{ConId, Provenance, SymbolKind, TVar}, File, Type};

pub type Span = std::ops::Range<usize>;

pub const CODE_SYNTAX_ERROR: u16 = 1;
pub const CODE_BINDING_NOT_FOUND: u16 = 2;
pub const CODE_EXPECTED_UNIFY: u16 = 5;
pub const CODE_INFINITE_TYPE: u16 = 6;
pub const CODE_CON_ARGS_LENGTH_MISMATCH: u16 = 7;

#[salsa::accumulator]
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DbDiagnostic {
    pub data: DiagnosticWithFile,
}

impl DbDiagnostic {

    pub fn new(data: DiagnosticWithFile) -> DbDiagnostic {
        DbDiagnostic { data }
    }
}

impl Display for DbDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.data.0, f)
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

impl Ord for Source {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.file.cmp(&other.file).then(self.span.start.cmp(&other.span.start).then(self.span.end.cmp(&other.span.end)))
    }
}

impl PartialOrd for Source {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Source {

    pub fn new(file: File, span: Span) -> Self {
        Self { file, span }
    }

    pub fn file(&self) -> File {
        self.file.clone()
    }

    pub fn span(&self) -> &Span {
        &self.span
    }

}

impl DbDiagnostic {

    pub fn code(&self) -> u16 {
        self.data.0.code()
    }

    pub fn message(&self) -> String {
        format!("{}", self.data.0)
    }

    pub fn severity(&self) -> Severity {
        self.data.0.severity()
    }

    pub fn source(&self) -> Source {
        Source::new(self.data.1, self.data.0.span().clone())
        
    }

}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Diagnostic {
    BindingNotFound(BindingNotFoundDiagnostic),
    SyntaxDiagnostic(SyntaxDiagnostic),
    TypeMismatch(TypeMismatchDiagnostic),
    InfiniteType(InfiniteTypeDiagnostic),
    ConArgsLengthMismatch(ConArgsLengthMismatchDiagnostic),
}

pub type DiagnosticWithFile = (Diagnostic, File);

impl Diagnostic {

    pub fn code(&self) -> u16 {
        match self {
            Self::SyntaxDiagnostic(diag) => diag.code(),
            Self::BindingNotFound(diag) => diag.code(),
            Self::TypeMismatch(diag) => diag.code(),
            Self::InfiniteType(diag) => diag.code(),
            Self::ConArgsLengthMismatch(diag) => diag.code(),
        }
    }

    pub fn severity(&self) -> Severity {
        match self {
            Self::SyntaxDiagnostic(diag) => diag.severity(),
            Self::BindingNotFound(diag) => diag.severity(),
            Self::TypeMismatch(diag) => diag.severity(),
            Self::InfiniteType(diag) => diag.severity(),
            Self::ConArgsLengthMismatch(diag) => diag.severity(),
        }
    }

    pub fn span(&self) -> &Span {
        match self {
            Self::SyntaxDiagnostic(diag) => diag.span(),
            Self::BindingNotFound(diag) => diag.span(),
            Self::TypeMismatch(diag) => diag.span(),
            Self::InfiniteType(diag) => diag.span(),
            Self::ConArgsLengthMismatch(diag) => diag.span(),
        }
    }

    pub fn with_file(self, file: File) -> DiagnosticWithFile {
        (self, file)
    }

}

impl Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SyntaxDiagnostic(diag) => std::fmt::Display::fmt(diag, f),
            Self::BindingNotFound(diag) => std::fmt::Display::fmt(diag, f),
            Self::TypeMismatch(diag) => std::fmt::Display::fmt(diag, f),
            Self::InfiniteType(diag) => std::fmt::Display::fmt(diag, f),
            Self::ConArgsLengthMismatch(diag) => std::fmt::Display::fmt(diag, f),
        }
    }
}

/// Public-facing parse error.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SyntaxDiagnostic {
    pub message: String,
    pub span: Span,
}

impl SyntaxDiagnostic {

    pub fn new(message: String, span: Span) -> Self {
        Self {
            message,
            span,
        }
    }

    fn code(&self) -> u16 {
        CODE_SYNTAX_ERROR
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn span(&self) -> &Span  {
        &self.span
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
    pub span: Span,
    pub name: String,
    pub kind: SymbolKind,
}

impl BindingNotFoundDiagnostic {

    pub fn new(name: String, kind: SymbolKind, span: Span) -> Self {
        BindingNotFoundDiagnostic {
            span,
            name,
            kind,
        }
    }

    fn code(&self) -> u16 {
        CODE_BINDING_NOT_FOUND
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn span(&self) -> &Span {
        &self.span
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
pub struct TypeMismatchDiagnostic {
    pub checked: Type,
    pub inferred: Type,
    pub provenance: Provenance,
}

impl TypeMismatchDiagnostic {

    fn code(&self) -> u16 {
        CODE_EXPECTED_UNIFY
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn span(&self) -> &Span {
        self.provenance.span()
    }

}

impl Display for TypeMismatchDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.provenance {
            Provenance::ExpectedUnify(..) => write!(f, "expected {} but got {}", self.checked, self.inferred),
            Provenance::AppExpectedFun(..) => write!(f, "expected {} to be a function type applicable to {}", self.checked, self.inferred),
            Provenance::TypeSignature(..) => write!(f, "type signature expected {} but {} was inferred", self.checked, self.inferred),
            Provenance::UnexpectedFun(..) => write!(f, "expected data type {} but got a function {}", self.checked, self.inferred),
        }
    }
}

impl From<TypeMismatchDiagnostic> for Diagnostic {
    fn from(value: TypeMismatchDiagnostic) -> Self {
        Diagnostic::TypeMismatch(value)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct InfiniteTypeDiagnostic {
    pub span: Span,
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

    fn span(&self) -> &Span {
        &self.span
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
    pub span: Span,
    pub id: ConId,
    pub a_args: Vec<Type>,
    pub b_args: Vec<Type>,
}

impl ConArgsLengthMismatchDiagnostic {

    pub fn new(span: Span, id: ConId, a_args: Vec<Type>, b_args: Vec<Type>) -> Self {
        Self {
            span,
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

    fn span(&self) -> &Span {
        &self.span
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

// pub trait Diagnostics {
//     fn add(&mut self, diag: Diagnostic);
// }

// pub struct DiagnosticStore {
//     storage: Vec<Diagnostic>,
// }

// impl DiagnosticStore {

//     pub fn new() -> Self {
//         Self { storage: Vec::new() }
//     }

//     pub fn diagnostics(&self) -> &[Diagnostic] {
//         &self.storage
//     }

//     pub fn take_diagnostics(&mut self) -> Vec<Diagnostic> {
//         std::mem::take(&mut self.storage)
//     }

// }

// impl IntoIterator for DiagnosticStore {
//     type Item = Diagnostic;
//     type IntoIter = std::vec::IntoIter<Diagnostic>;
//     fn into_iter(self) -> Self::IntoIter {
//         self.storage.into_iter()
//     }
// }

// impl Diagnostics for DiagnosticStore {
//     fn add(&mut self, diag: Diagnostic) {
//         self.storage.push(diag);
//     }
// }


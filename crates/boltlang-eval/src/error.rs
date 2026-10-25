use std::{fmt, ops::Range};

use boltlang_base_db::{Db, File};
use boltlang_hir_def::{
    Name,
    body::{DefWithBody, DefWithBodyId, body_with_source_map},
    hir::ExprId,
};

/// Why an evaluation failed, and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvalError<'db> {
    pub kind: ErrorKind,
    /// The innermost expression that was being evaluated, in its body.
    pub location: Option<(DefWithBodyId<'db>, ExprId)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorKind {
    /// Code that couldn't be parsed.
    Missing,
    UnresolvedName(String),
    /// A value of the wrong kind, like adding a bool. The type checker
    /// reports these before evaluation.
    TypeMismatch(String),
    NoSuchField(Name),
    NotCallable,
    NotAssignable,
    WrongArgCount { expected: usize, found: usize },
    Uninitialized(Name),
    /// A function or const that is only declared, like in a class.
    NoBody(String),
    Overflow,
    DivisionByZero,
    IndexOutOfBounds { index: i128, len: usize },
    /// No arm of a `match`, or the pattern of a `let`, matched.
    NoMatch,
    /// `break` or `continue` outside of a loop.
    BreakOutsideLoop,
    /// A const that needs its own value.
    ConstCycle(String),
    /// Calls nested deeper than [`Limits::max_depth`](crate::Limits).
    StackOverflow,
    /// More steps than [`Limits::fuel`](crate::Limits).
    OutOfFuel,
    Unsupported(&'static str),
}

impl<'db> EvalError<'db> {
    /// The file and byte range of the expression where the error happened.
    pub fn source_range(&self, db: &'db dyn Db) -> Option<(File, Range<u32>)> {
        let (def, expr) = self.location?;
        let file = match def.def(db) {
            DefWithBody::Function(id) => id.file(db),
            DefWithBody::Const(id) => id.file(db),
            DefWithBody::Variant(id) => id.parent.file(db),
            DefWithBody::File(file) => file,
        };
        let ptr = body_with_source_map(db, def).1.expr_syntax(expr)?;
        Some((file, ptr.start..ptr.end))
    }
}

impl<'db> From<ErrorKind> for EvalError<'db> {
    fn from(kind: ErrorKind) -> Self {
        EvalError { kind, location: None }
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorKind::Missing => write!(f, "missing expression"),
            ErrorKind::UnresolvedName(name) => write!(f, "cannot find `{name}`"),
            ErrorKind::TypeMismatch(msg) => write!(f, "type mismatch: {msg}"),
            ErrorKind::NoSuchField(name) => write!(f, "no field `{name}`"),
            ErrorKind::NotCallable => write!(f, "value is not callable"),
            ErrorKind::NotAssignable => write!(f, "cannot assign to this expression"),
            ErrorKind::WrongArgCount { expected, found } => {
                write!(f, "expected {expected} arguments, found {found}")
            }
            ErrorKind::Uninitialized(name) => write!(f, "`{name}` is used before it is initialized"),
            ErrorKind::NoBody(name) => write!(f, "`{name}` has no body"),
            ErrorKind::Overflow => write!(f, "integer overflow"),
            ErrorKind::DivisionByZero => write!(f, "division by zero"),
            ErrorKind::IndexOutOfBounds { index, len } => {
                write!(f, "index {index} is out of bounds for length {len}")
            }
            ErrorKind::NoMatch => write!(f, "no pattern matched the value"),
            ErrorKind::BreakOutsideLoop => write!(f, "`break` or `continue` outside of a loop"),
            ErrorKind::ConstCycle(name) => write!(f, "the value of `{name}` depends on itself"),
            ErrorKind::StackOverflow => write!(f, "too many nested calls"),
            ErrorKind::OutOfFuel => write!(f, "evaluation took too many steps"),
            ErrorKind::Unsupported(what) => write!(f, "{what} is not supported by the evaluator yet"),
        }
    }
}

impl fmt::Display for EvalError<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.kind, f)
    }
}

impl std::error::Error for EvalError<'_> {}

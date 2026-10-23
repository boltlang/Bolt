//! Diagnostics as users see them: with a code, a severity and a message.
//!
//! [`boltlang_hir`] finds the problems and where they are; this crate
//! decides how to talk about them. The result is plain data, so that the
//! CLI and the language server can each show it their own way.
//!
//! Every code is in [`DiagnosticCode`], so that they stay unique and can be
//! documented. Codes are never reused for something else:
//!
//! - `E0001`: syntax errors
//! - `E01xx`: names and imports
//! - `E02xx`: types
//! - `W0001`: things the checker doesn't support yet

use std::fmt;

use boltlang_base_db::{Db, File, source_text};
use boltlang_hir::{Because, BecauseKind, DiagnosticKind};
use boltlang_hir_ty::lower::ClassConstraint;
use boltlang_syntax::rowan::TextRange;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Warning,
    Error,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Severity::Warning => "warning",
            Severity::Error => "error",
        })
    }
}

/// A stable code for a kind of diagnostic, like `E0201`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DiagnosticCode(pub &'static str);

impl DiagnosticCode {
    pub const SYNTAX: Self = Self("E0001");

    pub const DUPLICATE_DEFINITION: Self = Self("E0101");
    pub const UNRESOLVED_IMPORT_PATH: Self = Self("E0102");
    pub const UNRESOLVED_IMPORT_NAME: Self = Self("E0103");
    pub const PRIVATE_IMPORT: Self = Self("E0104");
    pub const UNRESOLVED_NAME: Self = Self("E0105");
    pub const UNRESOLVED_TYPE: Self = Self("E0106");
    pub const NOT_A_TYPE: Self = Self("E0107");
    pub const NOT_A_CLASS: Self = Self("E0108");

    pub const TYPE_MISMATCH: Self = Self("E0201");
    pub const INFINITE_TYPE: Self = Self("E0202");
    pub const NO_INSTANCE: Self = Self("E0203");
    pub const AMBIGUOUS_CONSTRAINT: Self = Self("E0204");
    pub const NOT_A_FUNCTION: Self = Self("E0205");
    pub const WRONG_ARG_COUNT: Self = Self("E0206");
    pub const WRONG_TYPE_ARG_COUNT: Self = Self("E0207");
    pub const NO_SUCH_FIELD: Self = Self("E0208");
    pub const MISSING_FIELDS: Self = Self("E0209");
    pub const TYPE_NEEDED: Self = Self("E0210");
    pub const RECURSIVE_ALIAS: Self = Self("E0211");
    pub const CYCLIC_INFERENCE: Self = Self("E0212");
    pub const BREAK_OUTSIDE: Self = Self("E0213");

    pub const UNSUPPORTED: Self = Self("W0001");

    pub fn as_str(self) -> &'static str {
        self.0
    }

    /// Codes starting with `W` are warnings, the others errors.
    pub fn severity(self) -> Severity {
        if self.0.starts_with('W') { Severity::Warning } else { Severity::Error }
    }
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub severity: Severity,
    pub file: File,
    pub range: TextRange,
    pub message: String,
    /// Other code that helps explain the problem, like the annotation that
    /// made a type expected.
    pub labels: Vec<Label>,
}

/// A place in the source with a note about it. It can be in another file
/// than its diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label {
    pub file: File,
    pub range: TextRange,
    pub message: String,
}

/// The diagnostics of `file`, sorted by where they are.
pub fn diagnostics(db: &dyn Db, file: File) -> Vec<Diagnostic> {
    let text = source_text(db, file);
    boltlang_hir::file_diagnostics(db, file)
        .into_iter()
        .map(|d| {
            let labels = match &d.kind {
                DiagnosticKind::TypeMismatch { because: Some(because), .. } => vec![because_label(db, because)],
                _ => Vec::new(),
            };
            let (code, message) = describe(db, &text[usize::from(d.range.start())..usize::from(d.range.end())], d.kind);
            Diagnostic { code, severity: code.severity(), file: d.file, range: d.range, message, labels }
        })
        .collect()
}

/// The code and message of a diagnostic about `source`, the text it's at.
fn describe(db: &dyn Db, source: &str, kind: DiagnosticKind<'_>) -> (DiagnosticCode, String) {
    use DiagnosticCode as C;
    use DiagnosticKind as K;
    match kind {
        K::Syntax(message) => (C::SYNTAX, message),

        K::DuplicateDefinition { name } => (C::DUPLICATE_DEFINITION, format!("`{name}` is defined more than once")),
        K::UnresolvedImportPath { path } => {
            let message = if path.starts_with("./") || path.starts_with("../") {
                format!("cannot find the file `{path}`")
            } else {
                format!("cannot import `{path}`: only paths starting with `./` or `../` are supported")
            };
            (C::UNRESOLVED_IMPORT_PATH, message)
        }
        K::UnresolvedImportName { name } => {
            (C::UNRESOLVED_IMPORT_NAME, format!("the imported file has nothing named `{name}`"))
        }
        K::PrivateImport { name } => (C::PRIVATE_IMPORT, format!("`{name}` is private")),
        K::UnresolvedName => (C::UNRESOLVED_NAME, format!("cannot find `{source}`")),
        K::UnresolvedType => (C::UNRESOLVED_TYPE, format!("cannot find type `{source}`")),
        K::NotAType => (C::NOT_A_TYPE, format!("`{source}` is not a type")),
        K::NotAClass => (C::NOT_A_CLASS, format!("`{source}` is not a class")),

        K::TypeMismatch { expected, actual, .. } => (
            C::TYPE_MISMATCH,
            format!("expected `{}`, found `{}`", expected.display(db), actual.display(db)),
        ),
        K::InfiniteType { ty } => {
            (C::INFINITE_TYPE, format!("this would need the infinite type `{}`", ty.display(db)))
        }
        K::NoInstance { constraint } => {
            (C::NO_INSTANCE, format!("there is no instance `{}`", constraint_display(db, &constraint)))
        }
        K::AmbiguousConstraint { constraint } => (
            C::AMBIGUOUS_CONSTRAINT,
            format!(
                "cannot tell which instance of `{}` to use; add a type annotation",
                constraint_display(db, &constraint)
            ),
        ),
        K::NotAFunction { ty } => (C::NOT_A_FUNCTION, format!("expected a function, found `{}`", ty.display(db))),
        K::WrongArgCount { expected, found } => (
            C::WRONG_ARG_COUNT,
            format!("expected {}, found {found}", count(expected, "argument", "arguments")),
        ),
        K::WrongTypeArgCount { expected, found } => (
            C::WRONG_TYPE_ARG_COUNT,
            format!("expected {}, found {found}", count(expected, "type argument", "type arguments")),
        ),
        K::NoSuchField { name } => (C::NO_SUCH_FIELD, format!("there is no field `{name}`")),
        K::MissingFields { names } => {
            let list = names.iter().map(|n| format!("`{n}`")).collect::<Vec<_>>().join(", ");
            let noun = if names.len() == 1 { "field" } else { "fields" };
            (C::MISSING_FIELDS, format!("missing {noun} {list}"))
        }
        K::TypeNeeded => (C::TYPE_NEEDED, "the type of this must be known here; add a type annotation".into()),
        K::RecursiveAlias => (C::RECURSIVE_ALIAS, format!("type alias `{source}` refers to itself")),
        K::CyclicInference => (
            C::CYCLIC_INFERENCE,
            "cannot infer the type of this function, because it calls functions in other files that call it back; \
             give it a complete type signature"
                .into(),
        ),
        K::BreakOutside => (C::BREAK_OUTSIDE, "there is no loop or label to break out of".into()),

        K::Unsupported => (C::UNSUPPORTED, "the type checker doesn't support this yet".into()),
    }
}

fn because_label(db: &dyn Db, because: &Because<'_>) -> Label {
    let message = match &because.kind {
        BecauseKind::Annotation => "expected because of this annotation".to_string(),
        BecauseKind::ReturnType => "expected because of the return type".to_string(),
        BecauseKind::ConstType => "expected because of the type of the constant".to_string(),
        BecauseKind::Parameter { function: Some(name) } => format!("expected because of this parameter of `{name}`"),
        BecauseKind::Parameter { function: None } => "expected because of this parameter".to_string(),
        BecauseKind::Field { name } => format!("expected because of the type of field `{name}`"),
        BecauseKind::SameAs { ty } => format!("this has type `{}`", ty.display(db)),
    };
    Label { file: because.file, range: because.range, message }
}

fn constraint_display(db: &dyn Db, constraint: &ClassConstraint<'_>) -> String {
    let name = constraint.class.item(db).name.as_ref().map_or("?", |n| n.as_str());
    let args: Vec<_> = constraint.args.iter().map(|t| t.display(db).to_string()).collect();
    format!("{name}[{}]", args.join(", "))
}

fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

#[cfg(test)]
mod tests;

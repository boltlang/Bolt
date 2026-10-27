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

use boltlang_base_db::{Db, File, parse_file, source_text};
use boltlang_hir::{Because, BecauseKind, ClassItemKind, DiagnosticKind, ImportPathError};
use boltlang_hir_ty::{NumberKind, Ty, TyNames, builtin::Builtin, lower::ClassConstraint};
use boltlang_syntax::rowan::{TextRange, TextSize};

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
    pub const UNRESOLVED_IMPORT_ENUM: Self = Self("E0109");
    pub const NOT_AN_ENUM: Self = Self("E0110");
    pub const UNRESOLVED_PRELUDE: Self = Self("E0111");

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
    pub const NOT_IN_CLASS: Self = Self("E0214");
    pub const MISSING_ITEMS: Self = Self("E0215");
    pub const KIND_MISMATCH: Self = Self("E0216");
    pub const UNKNOWN_BUILTIN: Self = Self("E0217");
    pub const BUILTIN_SIGNATURE: Self = Self("E0218");
    pub const ASSIGN_TO_IMMUTABLE: Self = Self("E0219");
    pub const NOT_A_PLACE: Self = Self("E0220");
    pub const CANNOT_CHANGE: Self = Self("E0221");
    pub const NO_MUTABILITY: Self = Self("E0222");
    pub const MUT_INSIDE_IMM: Self = Self("E0223");
    pub const MUT_FIELD: Self = Self("E0224");
    pub const UNKNOWN_MUTABILITY: Self = Self("E0225");
    pub const NO_ASSOC_TYPE: Self = Self("E0226");
    pub const AMBIGUOUS_ASSOC_TYPE: Self = Self("E0227");
    pub const NO_LANG_CLASS: Self = Self("E0228");

    /// An error while running a program, like a division by zero.
    pub const RUNTIME_ERROR: Self = Self("E0301");

    /// Code that the code generator can't compile.
    pub const CODEGEN_ERROR: Self = Self("E0401");

    pub const UNSUPPORTED: Self = Self("W0001");
    pub const NO_VALUES: Self = Self("W0002");

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
            let names = TyNames::new(db, &types_in(&d.kind));
            let labels = match &d.kind {
                DiagnosticKind::TypeMismatch { because: Some(because), .. }
                | DiagnosticKind::NoInstance { because: Some(because), .. }
                | DiagnosticKind::AmbiguousConstraint { because: Some(because), .. }
                | DiagnosticKind::NoSuchField { because: Some(because), .. } => {
                    vec![because_label(db, &names, because)]
                }
                _ => Vec::new(),
            };
            let (code, message) = describe(db, &names, &text[usize::from(d.range.start())..usize::from(d.range.end())], d.kind);
            Diagnostic { code, severity: code.severity(), file: d.file, range: d.range, message, labels }
        })
        .collect()
}

/// The syntax errors of `file`, without checking anything else.
pub fn syntax_diagnostics(db: &dyn Db, file: File) -> Vec<Diagnostic> {
    parse_file(db, file)
        .errors(db)
        .iter()
        .map(|error| {
            let span = error.span();
            Diagnostic {
                code: DiagnosticCode::SYNTAX,
                severity: Severity::Error,
                file,
                range: TextRange::new(TextSize::from(span.start as u32), TextSize::from(span.end as u32)),
                message: error.message(),
                labels: Vec::new(),
            }
        })
        .collect()
}

/// The types in a diagnostic, in the order they're shown: in the message,
/// then in the labels.
fn types_in<'a, 'db>(kind: &'a DiagnosticKind<'db>) -> Vec<&'a Ty<'db>> {
    match kind {
        DiagnosticKind::TypeMismatch { expected, actual, because } => {
            let mut tys = vec![expected, actual];
            if let Some(Because { kind: BecauseKind::SameAs { ty }, .. }) = because {
                tys.push(ty);
            }
            tys
        }
        DiagnosticKind::InfiniteType { ty }
        | DiagnosticKind::NotAFunction { ty }
        | DiagnosticKind::CannotChange { ty }
        | DiagnosticKind::NoSuchField { ty: Some(ty), .. }
        | DiagnosticKind::NoValues { field_ty: ty, .. }
        | DiagnosticKind::BuiltinSignature { expected: ty, .. } => vec![ty],
        DiagnosticKind::MutInsideImm { ty, inner } => vec![ty, inner],
        DiagnosticKind::NoInstance { constraint, .. } | DiagnosticKind::AmbiguousConstraint { constraint, .. } => {
            constraint.args.iter().collect()
        }
        _ => Vec::new(),
    }
}

/// "an" before a word that starts with a vowel, like `imm Point`, and "a"
/// before others, like `mut Point`.
fn article(word: &str) -> &'static str {
    if word.starts_with(['a', 'e', 'i', 'o', 'u']) { "an" } else { "a" }
}

/// The code and message of a diagnostic about `source`, the text it's at.
/// `names` names the unknown types in it.
fn describe(db: &dyn Db, names: &TyNames, source: &str, kind: DiagnosticKind<'_>) -> (DiagnosticCode, String) {
    use DiagnosticCode as C;
    use DiagnosticKind as K;
    match kind {
        K::Syntax(message) => (C::SYNTAX, message),

        K::DuplicateDefinition { name } => (C::DUPLICATE_DEFINITION, format!("`{name}` is defined more than once")),
        K::UnresolvedImportPath { path, error } => {
            let message = match error {
                ImportPathError::FileNotFound => format!("cannot find the file `{path}`"),
                ImportPathError::PackageNotFound { package } => {
                    format!("cannot find the package `{package}` in `.bolt/packages`")
                }
                ImportPathError::NotInPackage { package, path } => {
                    format!("the package `{package}` has no file `{path}`")
                }
                ImportPathError::Invalid => format!(
                    "cannot import `{path}`: import a file with a path like `./file`, \
                     or a package with its name, like `std/io`"
                ),
            };
            (C::UNRESOLVED_IMPORT_PATH, message)
        }
        K::UnresolvedImportName { name } => {
            (C::UNRESOLVED_IMPORT_NAME, format!("the imported file has nothing named `{name}`"))
        }
        K::PrivateImport { name, package_only: false } => (C::PRIVATE_IMPORT, format!("`{name}` is private")),
        K::PrivateImport { name, package_only: true } => {
            (C::PRIVATE_IMPORT, format!("`{name}` is only visible inside its package"))
        }
        K::UnresolvedImportEnum { path } => (C::UNRESOLVED_IMPORT_ENUM, format!("cannot find the enum `{path}`")),
        K::NotAnEnum { path } => {
            (C::NOT_AN_ENUM, format!("`{path}` is not an enum, so it has no variants to import"))
        }
        K::UnresolvedPrelude { package, error } => {
            let message = match error {
                ImportPathError::PackageNotFound { .. } => {
                    format!("cannot find the package `{package}` of the prelude in `.bolt/packages`")
                }
                ImportPathError::NotInPackage { path, .. } => {
                    format!("the package `{package}` has no prelude: it needs a file `{path}`")
                }
                ImportPathError::FileNotFound | ImportPathError::Invalid => {
                    format!("cannot use `{package}` as the prelude: it's not the name of a package")
                }
            };
            (C::UNRESOLVED_PRELUDE, message)
        }
        K::UnresolvedName => (C::UNRESOLVED_NAME, format!("cannot find `{source}`")),
        K::UnresolvedType => (C::UNRESOLVED_TYPE, format!("cannot find type `{source}`")),
        K::NotAType => (C::NOT_A_TYPE, format!("`{source}` is not a type")),
        K::NotAClass => (C::NOT_A_CLASS, format!("`{source}` is not a class")),

        K::TypeMismatch { expected, actual, .. } => (
            C::TYPE_MISMATCH,
            format!("expected {}, found {}", ty_phrase(db, names, &expected), ty_phrase(db, names, &actual)),
        ),
        K::InfiniteType { ty } => {
            (C::INFINITE_TYPE, format!("this would need the infinite type `{}`", ty.display_with(db, names)))
        }
        K::NoInstance { constraint, .. } => {
            (C::NO_INSTANCE, format!("there is no instance `{}`", constraint_display(db, names, &constraint)))
        }
        K::AmbiguousConstraint { constraint, .. } => (
            C::AMBIGUOUS_CONSTRAINT,
            format!(
                "cannot tell which instance of `{}` to use; add a type annotation",
                constraint_display(db, names, &constraint)
            ),
        ),
        K::NotAFunction { ty } => (C::NOT_A_FUNCTION, format!("expected a function, found `{}`", ty.display_with(db, names))),
        K::WrongArgCount { expected, found } => (
            C::WRONG_ARG_COUNT,
            format!("expected {}, found {found}", count(expected, "argument", "arguments")),
        ),
        K::WrongTypeArgCount { expected, found } => (
            C::WRONG_TYPE_ARG_COUNT,
            format!("expected {}, found {found}", count(expected, "type argument", "type arguments")),
        ),
        K::NoValues { name, variant: None, field, field_ty } => (
            C::NO_VALUES,
            format!(
                "`{name}` has no values, since its field `{field}` needs a value of `{}`, which has none",
                field_ty.display_with(db, names)
            ),
        ),
        K::NoValues { name, variant: Some(variant), field_ty, .. } => (
            C::NO_VALUES,
            format!(
                "`{name}` has no values, since none of its variants can be built: `{variant}` needs a value of `{}`, \
                 which has none",
                field_ty.display_with(db, names)
            ),
        ),
        K::UnknownBuiltin { name } => {
            let names: Vec<_> = Builtin::ALL.iter().map(|b| format!("`{}`", b.name())).collect();
            (C::UNKNOWN_BUILTIN, format!("there is no builtin `{name}`; the builtins are {}", names.join(", ")))
        }
        K::BuiltinSignature { name, expected } => (
            C::BUILTIN_SIGNATURE,
            format!("the builtin `{name}` must have the type `{}`", expected.display_with(db, names)),
        ),
        K::KindMismatch { expected, found } => (
            C::KIND_MISMATCH,
            format!("expected a type of kind `{expected}`, found `{source}` of kind `{found}`"),
        ),
        K::NoSuchField { ty: Some(ty), name, .. } => {
            (C::NO_SUCH_FIELD, format!("`{}` has no field `{name}`", ty.display_with(db, names)))
        }
        K::NoSuchField { ty: None, name, .. } => (C::NO_SUCH_FIELD, format!("there is no field `{name}`")),
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
        K::AssignToImmutable { name } => {
            (C::ASSIGN_TO_IMMUTABLE, format!("cannot assign to `{name}`, which isn't `mut`"))
        }
        K::CannotChange { ty } => {
            let message = match ty {
                Ty::Array(..) => format!(
                    "cannot change an element of `{source}`, which is `{}`; only a `mut` array can be changed",
                    ty.display_with(db, names)
                ),
                ty => {
                    let ty = ty.display_with(db, names).to_string();
                    format!("a field of {} `{ty}` is changed here, but only a `mut` struct can be changed", article(&ty))
                }
            };
            (C::CANNOT_CHANGE, message)
        }
        K::NoMutability => (C::NO_MUTABILITY, "only an array or a struct can be `mut` or `imm`".into()),
        K::MutInsideImm { ty, inner } => {
            let (ty, inner) = (ty.display_with(db, names).to_string(), inner.display_with(db, names).to_string());
            let message = format!(
                "{} `{ty}` can't hold {} `{inner}`: what an `imm` value holds can't be changed either",
                article(&ty),
                article(&inner)
            );
            (C::MUT_INSIDE_IMM, message)
        }
        K::NoAssocType => (
            C::NO_ASSOC_TYPE,
            format!(
                "cannot find the associated type `{source}`: none of the classes that its type parameter has an \
                 instance of declares it; add a bound or a `where` clause"
            ),
        ),
        K::AmbiguousAssocType => (
            C::AMBIGUOUS_ASSOC_TYPE,
            format!("`{source}` is ambiguous: several classes of its type parameter declare it"),
        ),
        K::NoLangClass { class } => (
            C::NO_LANG_CLASS,
            format!("this needs the class `{class}`, but there's no prelude that defines it"),
        ),
        K::UnknownMutability => (
            C::UNKNOWN_MUTABILITY,
            format!(
                "the mutability of `{source}` isn't one of this function's: a name like `'m` can only be used in the \
                 signature and the body of the function or impl that it's written in"
            ),
        ),
        K::MutField => (
            C::MUT_FIELD,
            "a field can't be `mut`: it has the mutability of its struct, so make the struct `mut` instead".into(),
        ),
        K::NotAPlace => {
            (C::NOT_A_PLACE, "cannot assign to this; only a variable, a field or an element can be assigned to".into())
        }
        K::NotInClass { kind, name, class } => {
            let class = class.item(db).name.as_ref().map_or("?", |n| n.as_str());
            (C::NOT_IN_CLASS, format!("`{name}` is not a {} of class `{class}`", item_noun(kind, 1)))
        }
        K::MissingItems { items, class } => {
            let class = class.item(db).name.as_ref().map_or("?", |n| n.as_str());
            (C::MISSING_ITEMS, format!("missing {} of class `{class}`", item_list(&items)))
        }

        K::Unsupported => (C::UNSUPPORTED, "the type checker doesn't support this yet".into()),
    }
}

/// A type in a sentence: `` `i32` ``, or "an integer" for a number literal
/// whose type nothing decided. Inside other types, that's `{integer}`.
fn ty_phrase(db: &dyn Db, names: &TyNames, ty: &Ty<'_>) -> String {
    match ty {
        Ty::Number(NumberKind::Int) => "an integer".to_string(),
        Ty::Number(NumberKind::Float) => "a float".to_string(),
        ty => format!("`{}`", ty.display_with(db, names)),
    }
}

fn because_label(db: &dyn Db, names: &TyNames, because: &Because<'_>) -> Label {
    let message = match &because.kind {
        BecauseKind::Annotation => "expected because of this annotation".to_string(),
        BecauseKind::ReturnType => "expected because of the return type".to_string(),
        BecauseKind::ConstType => "expected because of the type of the constant".to_string(),
        BecauseKind::Parameter { function: Some(name) } => format!("expected because of this parameter of `{name}`"),
        BecauseKind::Parameter { function: None } => "expected because of this parameter".to_string(),
        BecauseKind::Field { name } => format!("expected because of the type of field `{name}`"),
        BecauseKind::SameAs { ty } => format!("this has type `{}`", ty.display_with(db, names)),
        BecauseKind::ClassItem { class } => {
            let class = class.item(db).name.as_ref().map_or("?", |n| n.as_str());
            format!("expected because of the declaration in class `{class}`")
        }
        BecauseKind::UseIn { function: Some(name) } => format!("needed by this use in `{name}`"),
        BecauseKind::UseIn { function: None } => "needed by this use".to_string(),
        BecauseKind::Bound { item: Some(name) } => format!("needed by this bound of `{name}`"),
        BecauseKind::Bound { item: None } => "needed by this bound".to_string(),
    };
    Label { file: because.file, range: because.range, message }
}

fn item_noun(kind: ClassItemKind, count: usize) -> &'static str {
    match (kind, count) {
        (ClassItemKind::Method, 1) => "method",
        (ClassItemKind::Method, _) => "methods",
        (ClassItemKind::Const, 1) => "constant",
        (ClassItemKind::Const, _) => "constants",
        (ClassItemKind::Type, 1) => "type",
        (ClassItemKind::Type, _) => "types",
    }
}

/// Items grouped by kind, like "methods `a`, `b` and type `T`".
fn item_list(items: &[(ClassItemKind, impl fmt::Display)]) -> String {
    let groups: Vec<String> = [ClassItemKind::Method, ClassItemKind::Const, ClassItemKind::Type]
        .into_iter()
        .filter_map(|kind| {
            let names: Vec<String> = items.iter().filter(|(k, _)| *k == kind).map(|(_, n)| format!("`{n}`")).collect();
            (!names.is_empty()).then(|| format!("{} {}", item_noun(kind, names.len()), names.join(", ")))
        })
        .collect();
    match groups.split_last() {
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
        None => String::new(),
    }
}

fn constraint_display(db: &dyn Db, names: &TyNames, constraint: &ClassConstraint<'_>) -> String {
    let name = constraint.class.item(db).name.as_ref().map_or("?", |n| n.as_str());
    let args: Vec<_> = constraint.args.iter().map(|t| t.display_with(db, names).to_string()).collect();
    format!("{name}[{}]", args.join(", "))
}

fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

#[cfg(test)]
mod tests;

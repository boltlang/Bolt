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
use boltlang_hir_def::{
    item_tree::{ModItem, item_tree},
    manifest::manifest_problems,
    nameres::{package_of, prelude_file, resolve_import_path},
};
/// The files a file uses, through its prelude and its imports.
pub use boltlang_hir_def::nameres::files_used_by;
/// The manifest of a file's package, and the name of manifests.
pub use boltlang_hir_def::manifest::{MANIFEST, package_manifest};
use boltlang_hir::{Because, BecauseKind, ClassItemKind, DiagnosticKind, ImportPathError};
use boltlang_hir_ty::{AdtId, NumberKind, Ty, TyNames, builtin::Builtin, lower::ClassConstraint};
use boltlang_syntax::{
    SyntaxKind, SyntaxNode,
    rowan::{TextRange, TextSize},
};

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
    pub const NOT_VISIBLE: Self = Self("E0104");
    pub const UNRESOLVED_NAME: Self = Self("E0105");
    pub const UNRESOLVED_TYPE: Self = Self("E0106");
    pub const NOT_A_TYPE: Self = Self("E0107");
    pub const NOT_A_CLASS: Self = Self("E0108");
    pub const UNRESOLVED_IMPORT_ENUM: Self = Self("E0109");
    pub const NOT_AN_ENUM: Self = Self("E0110");
    pub const UNRESOLVED_PRELUDE: Self = Self("E0111");
    pub const BROKEN_DEPENDENCY: Self = Self("E0112");
    pub const BAD_MANIFEST: Self = Self("E0113");
    pub const UNRESOLVED_OWNER: Self = Self("E0114");
    pub const OWNER_NOT_TOP_LEVEL: Self = Self("E0115");

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
    pub const UNMET_ASSOC_BOUND: Self = Self("E0229");
    pub const NOT_AT_MOST: Self = Self("E0230");
    pub const MUTABILITY_ALONE: Self = Self("E0231");
    pub const WRONG_MUTABILITY_ARG_COUNT: Self = Self("E0232");
    pub const MUTABILITY_PARAM_NOT_ALLOWED: Self = Self("E0233");
    pub const UNUSED_IMPL_PARAM: Self = Self("E0234");
    pub const OVERLAPPING_IMPL: Self = Self("E0235");
    pub const NO_OVERLOAD: Self = Self("E0236");
    pub const AMBIGUOUS_OVERLOAD: Self = Self("E0237");
    pub const OVERLAPPING_FUNCTION: Self = Self("E0238");
    pub const NOT_AN_ASSOC_TYPE_OF: Self = Self("E0239");
    pub const EQUALITY_NOT_ALLOWED: Self = Self("E0240");

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

/// One error for each file that `file` uses, through its prelude or its
/// imports, that has errors of its own (#249): what's in it can't be relied
/// on, so neither can `file`, and running it could fail in ways the checker
/// should rule out. It's at the import that brings the file in, or at the
/// start of `file` for one the prelude does, with a label at the file's
/// first error. The CLI reports these with the diagnostics of `file`; the
/// language server shows the errors in the files themselves instead.
pub fn dependency_diagnostics(db: &dyn Db, file: File) -> Vec<Diagnostic> {
    let prelude = prelude_file(db, file).as_ref().ok().copied().flatten();
    let in_prelude = |f: File| prelude.is_some_and(|p| p == f || files_used_by(db, p).contains(&f));
    let tree = item_tree(db, file);
    let root = SyntaxNode::new_root(parse_file(db, file).tree(db).clone());
    // The files `file` imports itself, with where the import is.
    let imports: Vec<(File, TextRange)> = tree
        .top_level
        .iter()
        .filter_map(|item| match *item {
            ModItem::Import(idx) => Some(&tree.imports[idx.index()]),
            _ => None,
        })
        .filter_map(|import| {
            let imported = resolve_import_path(db, file, import.source.as_ref()?).ok()?;
            let node = import.ast_id.to_node(db, file, &root)?;
            let source = node.children_with_tokens().find(|t| t.kind() == SyntaxKind::STRING);
            Some((imported, source.map_or(node.text_range(), |t| t.text_range())))
        })
        .collect();
    let mut out = Vec::new();
    for &used in files_used_by(db, file) {
        let errors: Vec<Diagnostic> = diagnostics(db, used).into_iter().filter(|d| d.severity == Severity::Error).collect();
        let Some(first) = errors.first() else { continue };
        let name = short_path(db, file, used);
        let count = count(errors.len(), "error", "errors");
        let (range, through) = if in_prelude(used) {
            (TextRange::default(), "through the prelude")
        } else {
            let import = imports.iter().find(|(f, _)| *f == used || files_used_by(db, *f).contains(&used));
            (import.map_or(TextRange::default(), |(_, range)| *range), "through this import")
        };
        out.push(Diagnostic {
            code: DiagnosticCode::BROKEN_DEPENDENCY,
            severity: Severity::Error,
            file,
            range,
            message: format!("`{name}`, which this file uses {through}, has {count}; fix them first"),
            labels: vec![Label { file: used, range: first.range, message: format!("the first: {}", first.message) }],
        });
    }
    out
}

/// The path of `used` as it reads from `file`: from its package, like
/// `std/__prelude__/vec.bolt`, from the directory of `file`, or in full.
pub fn short_path(db: &dyn Db, file: File, used: File) -> String {
    let Some(path) = used.path(db).as_system_path_buf() else { return "?".into() };
    let path = path.as_std_path();
    let base = match package_of(db, used) {
        Some(package) => package.as_std_path().parent().map(|p| p.to_path_buf()),
        None => file.path(db).as_system_path_buf().and_then(|f| f.as_std_path().parent().map(|p| p.to_path_buf())),
    };
    let short = base.as_deref().and_then(|base| path.strip_prefix(base).ok()).unwrap_or(path);
    short.to_string_lossy().into_owned()
}

/// The diagnostics of `file`, sorted by where they are.
pub fn diagnostics(db: &dyn Db, file: File) -> Vec<Diagnostic> {
    let text = source_text(db, file);
    boltlang_hir::file_diagnostics(db, file)
        .into_iter()
        .map(|d| {
            let mut names = TyNames::new(db, &types_in(&d.kind));
            // A bound like `T ⊑ imm` is about mutabilities, so they're all
            // shown, with names like `'m`.
            if let DiagnosticKind::NotAtMost { held, is_mutability, bound, .. } = &d.kind {
                let held = if *is_mutability { as_mutability(held.clone()) } else { held.clone() };
                let tys = [held, as_mutability(bound.clone())];
                names = TyNames::explicit(db, &tys.iter().collect::<Vec<_>>());
            }
            // Different structs or enums with the same name would read as the
            // same type (#239): the message says so, and labels show where
            // each is defined.
            let same_named = same_named(db, &types_in(&d.kind));
            // Types that differ only in mutabilities that aren't shown, like
            // an `imm fn()` and a generic `fn()`, are shown with them.
            if let DiagnosticKind::TypeMismatch { expected, actual, .. } = &d.kind
                && same_named.is_empty()
                && expected.display_with(db, &names).to_string() == actual.display_with(db, &names).to_string()
            {
                names = TyNames::explicit(db, &types_in(&d.kind));
            }
            let mut labels = match &d.kind {
                // Where in `bolt.toml` the error is.
                DiagnosticKind::BadManifest { manifest, message, range } => vec![Label {
                    file: *manifest,
                    range: range.unwrap_or_default(),
                    message: message.clone(),
                }],
                DiagnosticKind::OverlappingImpl { other_file, other_range, .. } => vec![Label {
                    file: *other_file,
                    range: *other_range,
                    message: "the impl it overlaps".to_string(),
                }],
                DiagnosticKind::OverlappingFunction { other_file, other_range, .. } => vec![Label {
                    file: *other_file,
                    range: *other_range,
                    message: "the function it overlaps".to_string(),
                }],
                DiagnosticKind::TypeMismatch { because: Some(because), .. }
                | DiagnosticKind::NoInstance { because: Some(because), .. }
                | DiagnosticKind::AmbiguousConstraint { because: Some(because), .. }
                | DiagnosticKind::NoSuchField { because: Some(because), .. }
                | DiagnosticKind::NotAtMost { because: Some(because), .. } => {
                    vec![because_label(db, &names, because)]
                }
                _ => Vec::new(),
            };
            let (code, mut message) =
                describe(db, &names, &text[usize::from(d.range.start())..usize::from(d.range.end())], d.kind);
            for (name, ids) in &same_named {
                let all = if ids.len() == 2 { "both" } else { "all" };
                message.push_str(&format!(" (these are different types, {all} called `{name}`)"));
                for (i, &id) in ids.iter().enumerate() {
                    let which = if i == 0 { "one" } else { "another" };
                    labels.extend(definition_label(db, id, format!("{which} `{name}` is defined here")));
                }
            }
            Diagnostic { code, severity: code.severity(), file: d.file, range: d.range, message, labels }
        })
        .collect()
}

/// The diagnostics of a `bolt.toml`, in it: that it can't be read, and
/// dependencies that aren't packages. The files of its package report the
/// first of these too, for the CLI, which only looks at them.
pub fn manifest_diagnostics(db: &dyn Db, file: File) -> Vec<Diagnostic> {
    manifest_problems(db, file)
        .into_iter()
        .map(|problem| Diagnostic {
            code: DiagnosticCode::BAD_MANIFEST,
            severity: Severity::Error,
            file,
            range: problem.range.map_or_else(TextRange::default, |(start, end)| TextRange::new(start.into(), end.into())),
            message: problem.message,
            labels: Vec::new(),
        })
        .collect()
}

/// The structs and enums in `tys` that have the same name as another one
/// in them, grouped by the name, in the order they appear.
fn same_named<'db>(db: &'db dyn Db, tys: &[&Ty<'db>]) -> Vec<(String, Vec<AdtId<'db>>)> {
    let mut adts: Vec<AdtId<'db>> = Vec::new();
    for ty in tys {
        ty.any(&mut |t| {
            if let Ty::Adt(id, ..) = t
                && !adts.contains(id)
            {
                adts.push(*id);
            }
            false
        });
    }
    let mut groups: Vec<(String, Vec<AdtId<'db>>)> = Vec::new();
    for id in adts {
        let name = adt_name(db, id);
        match groups.iter_mut().find(|(n, _)| *n == name) {
            Some((_, ids)) => ids.push(id),
            None => groups.push((name, vec![id])),
        }
    }
    groups.retain(|(_, ids)| ids.len() > 1);
    groups
}

fn adt_name(db: &dyn Db, id: AdtId<'_>) -> String {
    let name = match id {
        AdtId::Struct(s) => s.item(db).name.clone(),
        AdtId::Enum(e) => e.item(db).name.clone(),
    };
    name.map_or_else(|| "?".to_string(), |n| n.to_string())
}

/// A label at the name of the struct or enum `id`, where it's defined.
fn definition_label(db: &dyn Db, id: AdtId<'_>, message: String) -> Option<Label> {
    let (file, ast_id) = match id {
        AdtId::Struct(s) => (s.file(db), s.item(db).ast_id),
        AdtId::Enum(e) => (e.file(db), e.item(db).ast_id),
    };
    let root = SyntaxNode::new_root(parse_file(db, file).tree(db).clone());
    let node = ast_id.to_node(db, file, &root)?;
    let range = node.children().find(|n| n.kind() == SyntaxKind::NAME).unwrap_or(node).text_range();
    Some(Label { file, range, message })
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

/// A function type with the mutability `m`, so that naming the unknowns of
/// types names `m` as a mutability.
fn as_mutability<'db>(m: Ty<'db>) -> Ty<'db> {
    Ty::Fn(Box::new([]), Box::new(Ty::unit()), Box::new(m))
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
        DiagnosticKind::NotAtMost { held, .. } => vec![held],
        DiagnosticKind::NoInstance { constraint, .. }
        | DiagnosticKind::AmbiguousConstraint { constraint, .. }
        | DiagnosticKind::UnmetAssocBound { constraint, .. } => {
            constraint.args.iter().collect()
        }
        DiagnosticKind::UnusedImplParam { instance, suggestion, .. } => {
            instance.args.iter().chain(suggestion).collect()
        }
        DiagnosticKind::OverlappingImpl { instance, other, .. } => instance.args.iter().chain(other.args.iter()).collect(),
        DiagnosticKind::NoOverload { ty, .. } => vec![ty],
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
        K::BadManifest { message, .. } => {
            (C::BAD_MANIFEST, format!("the `bolt.toml` of this package has an error: {message}"))
        }

        K::DuplicateDefinition { name } => (C::DUPLICATE_DEFINITION, format!("`{name}` is defined more than once")),
        K::UnresolvedImportPath { path, error } => {
            let message = match error {
                ImportPathError::FileNotFound => format!("cannot find the file `{path}`"),
                ImportPathError::NoManifest { package } => format!(
                    "cannot find the package `{package}`: this file is in no package, so it can only use `std`; \
                     add a `bolt.toml` with `{package}` as a dependency"
                ),
                ImportPathError::NotADependency { package } => {
                    format!("`{package}` isn't a dependency of this package: add it to `[dependencies]` in `bolt.toml`")
                }
                ImportPathError::BadManifest => {
                    format!("cannot import `{path}`: the `bolt.toml` of this package has an error")
                }
                ImportPathError::DependencyNotFound { package, path } => {
                    format!("the dependency `{package}` is at `{path}`, which has no `bolt.toml`")
                }
                ImportPathError::NoStd => "cannot find `std`, which comes with the compiler".to_string(),
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
        K::NotVisible { name, package_only: false } => (C::NOT_VISIBLE, format!("`{name}` is private")),
        K::NotVisible { name, package_only: true } => {
            (C::NOT_VISIBLE, format!("`{name}` is only visible inside its package"))
        }
        K::UnresolvedImportEnum { path } => (C::UNRESOLVED_IMPORT_ENUM, format!("cannot find the enum `{path}`")),
        K::NotAnEnum { path } => {
            (C::NOT_AN_ENUM, format!("`{path}` is not an enum, so it has no variants to import"))
        }
        K::UnresolvedOwner { name } => (
            C::UNRESOLVED_OWNER,
            format!("cannot find a struct or enum `{name}` in this file; only the file that defines a type can add functions to it"),
        ),
        K::OwnerNotTopLevel => (
            C::OWNER_NOT_TOP_LEVEL,
            "a function can only be put in the namespace of a type at the top level of a file".to_string(),
        ),
        K::UnresolvedPrelude { package, error } => {
            let message = match error {
                ImportPathError::NoManifest { .. } | ImportPathError::NotADependency { .. } => {
                    format!("cannot find the package `{package}` of the prelude: it isn't a dependency")
                }
                ImportPathError::BadManifest => {
                    format!("cannot find the prelude of `{package}`: the `bolt.toml` of this package has an error")
                }
                ImportPathError::DependencyNotFound { path, .. } => {
                    format!("the package `{package}` of the prelude is at `{path}`, which has no `bolt.toml`")
                }
                ImportPathError::NoStd => "there's no `std`, which the prelude comes from".to_string(),
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
        // With all its types known, only impls that overlap can make it
        // ambiguous (#276), and an annotation doesn't help.
        K::AmbiguousConstraint { constraint, .. }
            if !constraint.args.iter().any(|t| t.any(&mut |t| matches!(t, Ty::Var(_) | Ty::Number(_)))) =>
        {
            (
                C::AMBIGUOUS_CONSTRAINT,
                format!(
                    "more than one impl fits `{}`, so it's not known which to use; their instances overlap",
                    constraint_display(db, names, &constraint)
                ),
            )
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
        K::NoMutability => (C::NO_MUTABILITY, "only an array, a struct or a function can be `mut` or `imm`".into()),
        // A `mut` mutability argument, like that of `imm VecIter[i32, mut]`.
        K::MutInsideImm { ty, inner: Ty::Mutability(_) } => {
            let ty = ty.display_with(db, names).to_string();
            let message = format!(
                "{} `{ty}` can't have a `mut` mutability argument: what an `imm` value holds can't be changed either",
                article(&ty)
            );
            (C::MUT_INSIDE_IMM, message)
        }
        K::MutInsideImm { ty, inner } => {
            // A struct that isn't `imm` itself holds it in a field that is,
            // like the `data` of a `mut VecIter[mut [i32], imm]`.
            let within = if ty.mutability() == Some(&Ty::Mutability(boltlang_hir_ty::ty::Mutability::Imm)) {
                ""
            } else {
                " in a field that's `imm`"
            };
            let (ty, inner) = (ty.display_with(db, names).to_string(), inner.display_with(db, names).to_string());
            let message = format!(
                "{} `{ty}` can't hold {} `{inner}`{within}: what an `imm` value holds can't be changed either",
                article(&ty),
                article(&inner)
            );
            (C::MUT_INSIDE_IMM, message)
        }
        K::NotAtMost { held, is_mutability, bound, .. } => {
            let bound = names.mutability_name(db, &bound);
            let message = if is_mutability {
                format!("expected a mutability that's at most `{bound}`, found `{}`", names.mutability_name(db, &held))
            } else {
                format!("expected a type that's at most `{bound}`, found `{}`", held.display_with(db, names))
            };
            (C::NOT_AT_MOST, message)
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
        K::UnmetAssocBound { name, class, constraint } => {
            let class = class.item(db).name.as_ref().map_or("?", |n| n.as_str());
            let instance = constraint_display(db, names, &constraint);
            (
                C::UNMET_ASSOC_BOUND,
                format!("there is no instance `{instance}`, which class `{class}` asks of its type `{name}`"),
            )
        }
        K::NoLangClass { class } => (
            C::NO_LANG_CLASS,
            format!("this needs the class `{class}`, but there's no prelude that defines it"),
        ),
        K::UnknownMutability => (
            C::UNKNOWN_MUTABILITY,
            format!(
                "the mutability of `{source}` isn't one of this function's or impl's, or one that this struct declares: \
                 a name like `'m` can only be used in the function or impl whose signature names it, an impl's \
                 associated types, and the fields of a struct that declares it, like `struct VecIter[T, 'v]`"
            ),
        ),
        K::UnusedImplParam { name, instance, suggestion } => {
            let instance = constraint_display(db, names, &instance);
            let fix = match suggestion {
                Some(ty) => format!("write it in the instance, like `{}`, or remove it", ty.display_with(db, names)),
                None if name.starts_with('\'') => "write it before an array or struct in the instance, or remove it".into(),
                None => "use it in the instance, or remove it".into(),
            };
            (
                C::UNUSED_IMPL_PARAM,
                format!("`{name}` isn't in the instance `{instance}`, so choosing the impl can't decide it; {fix}"),
            )
        }
        K::OverlappingImpl { instance, other, .. } => {
            let instance = constraint_display(db, names, &instance);
            let other = constraint_display(db, names, &other);
            (
                C::OVERLAPPING_IMPL,
                format!(
                    "the impl for `{instance}` overlaps the one for `{other}`: some types fit both, so a constraint \
                     can't choose one; remove one of them, or make their instances differ"
                ),
            )
        }
        K::NotAnAssocTypeOf { class, name } => (
            C::NOT_AN_ASSOC_TYPE_OF,
            format!("the class `{class}` has no associated type `{name}`"),
        ),
        K::EqualityNotAllowed => (
            C::EQUALITY_NOT_ALLOWED,
            "what an associated type is, like `Item = T`, can only be written in a bound or `where` clause of a \
             function or an impl"
                .to_string(),
        ),
        K::NoOverload { name, ty, candidates } => (
            C::NO_OVERLOAD,
            format!(
                "no function `{name}` fits this use, of type `{}`; the functions called `{name}` are {}",
                ty.display_with(db, names),
                list_types(&candidates)
            ),
        ),
        K::OverlappingFunction { name, ty, other, .. } => (
            C::OVERLAPPING_FUNCTION,
            format!(
                "this `{name}`, of type `{ty}`, overlaps the `{name}` of type `{other}`: a call could fit both, so it \
                 couldn't choose one; give them different numbers or types of parameters"
            ),
        ),
        K::AmbiguousOverload { name, candidates } => (
            C::AMBIGUOUS_OVERLOAD,
            format!(
                "more than one function `{name}` fits this use: {}; give the arguments types that leave one",
                list_types(&candidates)
            ),
        ),
        K::WrongMutabilityArgCount { expected, found } => (
            C::WRONG_MUTABILITY_ARG_COUNT,
            format!("expected at most {}, found {found}", count(expected, "mutability argument", "mutability arguments")),
        ),
        K::MutabilityParamNotAllowed => (
            C::MUTABILITY_PARAM_NOT_ALLOWED,
            format!("only a struct, a function or an impl can have a mutability parameter like `{source}`"),
        ),
        K::MutabilityAlone => (
            C::MUTABILITY_ALONE,
            format!("`{source}` is a mutability, not a type: on its own, it can only be a bound, like `T: {source}`"),
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

/// Types as text in a sentence: `` `a` ``, `` `a` and `b` `` or
/// `` `a`, `b` and `c` ``.
fn list_types(tys: &[String]) -> String {
    let quoted: Vec<String> = tys.iter().map(|t| format!("`{t}`")).collect();
    match quoted.split_last() {
        None => String::new(),
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
    }
}

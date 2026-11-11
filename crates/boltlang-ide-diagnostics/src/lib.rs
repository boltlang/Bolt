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
use boltlang_hir::{
    BadBuiltin, BadLink, BadTest, Because, BecauseKind, ClassItemKind, DiagnosticKind,
    ImportPathError,
};
/// The manifest of a file's package, and the name of manifests.
pub use boltlang_hir_def::manifest::{MANIFEST, package_manifest};
/// The files a file uses, through its prelude and its imports.
pub use boltlang_hir_def::nameres::files_used_by;
use boltlang_hir_def::{
    ids::ClassId,
    item_tree::{ModItem, item_tree},
    manifest::manifest_problems,
    nameres::{package_of, prelude_file, resolve_import_path},
};
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
    pub const NOT_BOUND_IN_ALL_ALTERNATIVES: Self = Self("E0116");
    pub const MUTABILITY_DIFFERS_IN_ALTERNATIVES: Self = Self("E0117");
    pub const PRIVATE_FIELD: Self = Self("E0118");

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
    pub const UNMET_ASSOC_EQUALITY: Self = Self("E0241");
    pub const BAD_TEST: Self = Self("E0242");
    pub const NO_LANG_TYPE: Self = Self("E0243");
    pub const INVALID_CAST: Self = Self("E0244");
    pub const EFFECT_TWICE: Self = Self("E0245");
    pub const EFFECTS_NOT_ALLOWED: Self = Self("E0246");
    pub const NOT_AN_OPERATION: Self = Self("E0247");
    pub const AMBIGUOUS_OPERATION: Self = Self("E0248");
    pub const MISSING_OPERATIONS: Self = Self("E0249");
    pub const MISSING_BODY: Self = Self("E0250");
    pub const BAD_BUILTIN: Self = Self("E0251");
    pub const BAD_LINK: Self = Self("E0252");
    pub const BAD_CFG: Self = Self("E0253");
    pub const BAD_TRY: Self = Self("E0254");
    pub const MISSING_TYPE: Self = Self("E0255");
    pub const DUPLICATE_FIELD: Self = Self("E0256");
    pub const NOT_SPREADABLE: Self = Self("E0257");
    pub const NOT_A_PACK: Self = Self("E0258");
    pub const DUPLICATE_ARGUMENT: Self = Self("E0259");
    pub const DEFAULT_MISSING: Self = Self("E0260");
    pub const DEFAULT_IN_IMPL: Self = Self("E0261");
    pub const UNKNOWN_ARGUMENT: Self = Self("E0262");
    pub const TOO_MANY_PATTERNS: Self = Self("E0263");

    // E0301 was for errors while running a program, and E0401 for code
    // that the code generator couldn't compile. Nothing gives them since
    // the evaluator and the old Wasm backend were removed. Don't use them
    // for anything else.

    pub const UNSUPPORTED: Self = Self("W0001");
    pub const NO_VALUES: Self = Self("W0002");
    pub const NON_EXHAUSTIVE_MATCH: Self = Self("W0003");
    pub const UNREACHABLE_ARM: Self = Self("W0004");
    pub const SWAPPED_NAMES: Self = Self("W0005");

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
        let errors: Vec<Diagnostic> =
            diagnostics(db, used).into_iter().filter(|d| d.severity == Severity::Error).collect();
        let Some(first) = errors.first() else { continue };
        let name = short_path(db, file, used);
        let count = count(errors.len(), "error", "errors");
        let (range, through) = if in_prelude(used) {
            (TextRange::default(), "through the prelude")
        } else {
            let import =
                imports.iter().find(|(f, _)| *f == used || files_used_by(db, *f).contains(&used));
            (import.map_or(TextRange::default(), |(_, range)| *range), "through this import")
        };
        out.push(Diagnostic {
            code: DiagnosticCode::BROKEN_DEPENDENCY,
            severity: Severity::Error,
            file,
            range,
            message: format!(
                "`{name}`, which this file uses {through}, has {count}; fix them first"
            ),
            labels: vec![Label {
                file: used,
                range: first.range,
                message: format!("the first: {}", first.message),
            }],
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
        None => file
            .path(db)
            .as_system_path_buf()
            .and_then(|f| f.as_std_path().parent().map(|p| p.to_path_buf())),
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
                && expected.display_with(db, &names).to_string()
                    == actual.display_with(db, &names).to_string()
            {
                names = TyNames::explicit(db, &types_in(&d.kind));
            }
            let mut labels = labels(db, &names, &d.kind);
            let (code, mut message) = describe(
                db,
                &names,
                &text[usize::from(d.range.start())..usize::from(d.range.end())],
                d.kind,
            );
            for (name, ids) in &same_named {
                let all = if ids.len() == 2 { "both" } else { "all" };
                message.push_str(&format!(" (these are different types, {all} called `{name}`)"));
                for (i, &id) in ids.iter().enumerate() {
                    let which = if i == 0 { "one" } else { "another" };
                    labels.extend(definition_label(
                        db,
                        id,
                        format!("{which} `{name}` is defined here"),
                    ));
                }
            }
            Diagnostic {
                code,
                severity: code.severity(),
                file: d.file,
                range: d.range,
                message,
                labels,
            }
        })
        .collect()
}

/// The labels of a diagnostic that its kind gives, like where a type was
/// expected.
fn labels(db: &dyn Db, names: &TyNames, kind: &DiagnosticKind<'_>) -> Vec<Label> {
    match kind {
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
        DiagnosticKind::OverlappingFunction { other_file, other_range, .. } => {
            vec![Label {
                file: *other_file,
                range: *other_range,
                message: "the function it overlaps".to_string(),
            }]
        }
        DiagnosticKind::TypeMismatch { because: Some(because), .. }
        | DiagnosticKind::NoInstance { because: Some(because), .. }
        | DiagnosticKind::AmbiguousConstraint { because: Some(because), .. }
        | DiagnosticKind::NoSuchField { because: Some(because), .. }
        | DiagnosticKind::NotAtMost { because: Some(because), .. } => {
            vec![because_label(db, names, because)]
        }
        _ => Vec::new(),
    }
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
            range: problem.range.map_or_else(TextRange::default, |(start, end)| {
                TextRange::new(start.into(), end.into())
            }),
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
                range: TextRange::new(
                    TextSize::from(span.start as u32),
                    TextSize::from(span.end as u32),
                ),
                message: error.message(),
                labels: Vec::new(),
            }
        })
        .collect()
}

/// A function type with the mutability `m`, so that naming the unknowns of
/// types names `m` as a mutability.
fn as_mutability<'db>(m: Ty<'db>) -> Ty<'db> {
    Ty::Fn(
        Box::new(boltlang_hir_ty::ty::Params::positional([])),
        Box::new(Ty::unit()),
        Box::new(m),
        Box::new(Ty::nothing()),
    )
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
        | DiagnosticKind::NotSpreadable { ty }
        | DiagnosticKind::NoValues { field_ty: ty, .. }
        | DiagnosticKind::BuiltinSignature { expected: ty, .. } => vec![ty],
        DiagnosticKind::MutInsideImm { ty, inner } => vec![ty, inner],
        DiagnosticKind::Effects { found, allowed } => vec![found, allowed],
        DiagnosticKind::InvalidCast { from, to } => vec![from, to],
        DiagnosticKind::BadTry { reason } => match reason {
            boltlang_hir_ty::infer::BadTry::NotOptionOrResult(ty)
            | boltlang_hir_ty::infer::BadTry::WrongReturn { ret: ty, .. } => vec![ty],
            boltlang_hir_ty::infer::BadTry::Unknown => vec![],
        },
        DiagnosticKind::NotAtMost { held, .. } => vec![held],
        DiagnosticKind::NoInstance { constraint, .. }
        | DiagnosticKind::AmbiguousConstraint { constraint, .. }
        | DiagnosticKind::UnmetAssocBound { constraint, .. } => constraint.args.iter().collect(),
        DiagnosticKind::UnusedImplParam { instance, suggestion, .. } => {
            instance.args.iter().chain(suggestion).collect()
        }
        DiagnosticKind::OverlappingImpl { instance, other, .. } => {
            instance.args.iter().chain(other.args.iter()).collect()
        }
        DiagnosticKind::NoOverload { ty, .. } => vec![ty],
        DiagnosticKind::UnmetAssocEquality { expected, actual, .. } => vec![expected, actual],
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
fn describe(
    db: &dyn Db,
    names: &TyNames,
    source: &str,
    kind: DiagnosticKind<'_>,
) -> (DiagnosticCode, String) {
    use DiagnosticCode as C;
    use DiagnosticKind as K;
    match kind {
        K::Syntax(message) => (C::SYNTAX, message),
        K::BadCfg { error } => (C::BAD_CFG, cfg_error(&error)),
        K::BadManifest { message, .. } => {
            (C::BAD_MANIFEST, format!("the `bolt.toml` of this package has an error: {message}"))
        }

        K::DuplicateDefinition { name } => (C::DUPLICATE_DEFINITION, format!("`{name}` is defined more than once")),
        K::UnresolvedImportPath { path, error } => {
            (C::UNRESOLVED_IMPORT_PATH, import_path_message(&path, error))
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
        K::NotAnOperation => (
            C::NOT_AN_OPERATION,
            "this isn't an operation of an effect, so an effect arm can't handle it".to_string(),
        ),
        K::AmbiguousOperation => (
            C::AMBIGUOUS_OPERATION,
            "several effects have an operation of this name; write the effect too, like `State.get()`".to_string(),
        ),
        K::MissingOperations { effect, missing } => {
            let names: Vec<String> = missing.iter().map(|n| n.to_string()).collect();
            let effect = effect.as_ref().map_or("this effect".into(), |e| format!("`{e}`"));
            let arms = if names.len() == 1 { "an arm" } else { "arms" };
            (
                C::MISSING_OPERATIONS,
                format!(
                    "this match handles {effect}, so it needs {arms} for {} too",
                    list_types(&names)
                ),
            )
        }
        K::OwnerNotTopLevel => (
            C::OWNER_NOT_TOP_LEVEL,
            "a function can only be put in the namespace of a type at the top level of a file".to_string(),
        ),
        K::PrivateField { strukt, name, package_only } => {
            let field = match strukt {
                Some(s) => format!("`{s}.{name}`"),
                None => format!("`{name}`"),
            };
            let message = if package_only {
                format!("the field {field} is only visible inside its package; make it `pub` to use it here")
            } else {
                format!("the field {field} is private to the file of its struct; make it `pub` to use it here")
            };
            (C::PRIVATE_FIELD, message)
        }
        K::NotBoundInAllAlternatives { name } => (
            C::NOT_BOUND_IN_ALL_ALTERNATIVES,
            format!("`{name}` is bound in some alternatives of this pattern, but not in all of them"),
        ),
        K::MutabilityDiffersInAlternatives { name } => (
            C::MUTABILITY_DIFFERS_IN_ALTERNATIVES,
            format!("`{name}` is `mut` in some alternatives of this pattern, but not in all of them"),
        ),
        K::UnresolvedPrelude { package, error } => {
            (C::UNRESOLVED_PRELUDE, prelude_message(&package, error))
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
            let names: Vec<_> = Builtin::BASE.iter().map(|b| format!("`{}`", b.name())).collect();
            let message = format!(
                "there is no builtin `{name}`; the builtins are {}, and the operations of the primitive types, like `i32_add`",
                names.join(", ")
            );
            (C::UNKNOWN_BUILTIN, message)
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
        K::NotAPack => (
            C::NOT_A_PACK,
            format!(
                "`{source}` isn't a pack of this kind: `*` takes a pack like `*Ts`, and `**` one like `**Ks`"
            ),
        ),
        K::DuplicateArgument { name } => {
            (C::DUPLICATE_ARGUMENT, format!("the argument `{name}` is given more than once"))
        }
        K::SwappedNames { expected, name, given, at } => (
            C::SWAPPED_NAMES,
            format!(
                "this function's parameter {} is `{name}`, but `{}` has `{name}` as parameter {}, so an argument passed as `{name}` goes to another parameter",
                given + 1,
                expected.display_with(db, names),
                at + 1
            ),
        ),
        K::TooManyPatterns { expected, found } => (
            C::TOO_MANY_PATTERNS,
            format!("expected at most {}, found {found}", count(expected, "pattern", "patterns")),
        ),
        K::UnknownArgument { name, unnamed: false } => {
            (C::UNKNOWN_ARGUMENT, format!("no parameter is named `{name}`"))
        }
        K::UnknownArgument { name, unnamed: true } => (
            C::UNKNOWN_ARGUMENT,
            format!(
                "no parameter is named `{name}`; the positional parameters that are left have no \
                 names, so they're passed by position"
            ),
        ),
        K::DefaultMissing => (
            C::DEFAULT_MISSING,
            "this parameter needs a default, since a positional parameter before it has one".into(),
        ),
        K::DefaultInImpl => (
            C::DEFAULT_IN_IMPL,
            "a method of an impl takes the defaults of the class's declaration, so it can't have its own"
                .into(),
        ),
        K::DuplicateField { name } => (C::DUPLICATE_FIELD, format!("the field `{name}` is given more than once")),
        K::NotSpreadable { ty } => (
            C::NOT_SPREADABLE,
            format!(
                "only a tuple can be spread, with `*` for one with positional fields and `**` for one \
                 with named fields, not `{}`",
                ty.display_with(db, names)
            ),
        ),
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
            let class = class_name(db, class);
            let instance = constraint_display(db, names, &constraint);
            (
                C::UNMET_ASSOC_BOUND,
                format!("there is no instance `{instance}`, which class `{class}` asks of its type `{name}`"),
            )
        }
        K::MissingBody => (
            C::MISSING_BODY,
            "this function has no body; only a function the compiler provides, with `#[builtin]`, or the \
             host, with `#[link(..)]`, can do without one"
                .to_string(),
        ),
        K::MissingType => (
            C::MISSING_TYPE,
            "this type alias has no type; only a type the host provides, with `#[link(wasm)]`, can do \
             without one"
                .to_string(),
        ),
        K::BadLink { reason } => (C::BAD_LINK, bad_link_message(db, names, reason)),
        K::BadBuiltin { reason } => (
            C::BAD_BUILTIN,
            match reason {
                BadBuiltin::NotTopLevel => {
                    "a builtin must be a function at the top level of a file, not in a class, an impl or the \
                     namespace of a type"
                }
                BadBuiltin::HasBody => "a builtin can't have a body: the compiler provides it",
            }
            .to_string(),
        ),
        K::BadTest { reason } => (C::BAD_TEST, bad_test_message(reason)),
        K::UnmetAssocEquality { name, item, class, expected, actual } => {
            let class = class_name(db, class);
            (
                C::UNMET_ASSOC_EQUALITY,
                format!(
                    "class `{class}` needs the `{item}` of its type `{name}` to be `{}`, but this impl's is `{}`",
                    expected.display_with(db, names),
                    actual.display_with(db, names)
                ),
            )
        }
        K::NoLangClass { class } => (
            C::NO_LANG_CLASS,
            format!("this needs the class `{class}`, but there's no prelude that defines it"),
        ),
        K::BadTry { reason } => {
            use boltlang_hir_ty::infer::BadTry;
            let message = match reason {
                BadTry::NotOptionOrResult(ty) => format!(
                    "`?` takes an `Option` or a `Result`, not `{}`",
                    ty.display_with(db, names)
                ),
                BadTry::WrongReturn { result: true, ret } => format!(
                    "`?` on a `Result` returns its `Err`, so it can only be in a function that gives a \
                     `Result`, not `{}`",
                    ret.display_with(db, names)
                ),
                BadTry::WrongReturn { result: false, ret } => format!(
                    "`?` on an `Option` returns its `None`, so it can only be in a function that gives an \
                     `Option`, not `{}`",
                    ret.display_with(db, names)
                ),
                BadTry::Unknown => "cannot tell whether this is an `Option` or a `Result`; add a type \
                                    annotation"
                    .to_string(),
            };
            (C::BAD_TRY, message)
        }
        K::InvalidCast { from, to } => (
            C::INVALID_CAST,
            format!(
                "cannot convert `{}` to `{}` with `as`, which only converts between the primitive types, like numbers",
                from.display_with(db, names),
                to.display_with(db, names)
            ),
        ),
        K::NoLangType { ty } => (
            C::NO_LANG_TYPE,
            format!("this needs the type `{ty}`, but there's no prelude that defines it"),
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
            let class = class_name(db, class);
            (C::NOT_IN_CLASS, format!("`{name}` is not a {} of class `{class}`", item_noun(kind, 1)))
        }
        K::MissingItems { items, class } => {
            let class = class_name(db, class);
            (C::MISSING_ITEMS, format!("missing {} of class `{class}`", item_list(&items)))
        }

        K::Unsupported => (C::UNSUPPORTED, "the type checker doesn't support this yet".into()),
        K::Effects { found, allowed } => {
            let found = names.row_name(db, &found);
            let message = match &allowed {
                Ty::Row(effects, rest) if effects.is_empty() && rest.is_empty() => {
                    format!("this does `{found}`, but the code it's in has no effects")
                }
                allowed => format!(
                    "this does `{found}`, but the code it's in only does `{}`",
                    names.row_name(db, allowed)
                ),
            };
            (C::EFFECTS_NOT_ALLOWED, message)
        }
        K::EffectTwice { effect } => (
            C::EFFECT_TWICE,
            format!("`{effect}` is in this row twice, with different arguments"),
        ),
        K::NonExhaustiveMatch { missing: Some(missing) } => (
            C::NON_EXHAUSTIVE_MATCH,
            format!("this match doesn't cover every value: `{missing}` matches none of its arms"),
        ),
        K::NonExhaustiveMatch { missing: None } => {
            (C::NON_EXHAUSTIVE_MATCH, "this match doesn't cover every value".into())
        }
        K::NonExhaustiveOperation { name, missing: Some(missing) } => (
            C::NON_EXHAUSTIVE_MATCH,
            format!("the arms for `{name}` don't cover every value: `{missing}` matches none of them"),
        ),
        K::NonExhaustiveOperation { name, missing: None } => (
            C::NON_EXHAUSTIVE_MATCH,
            format!("the arms for `{name}` don't cover every value"),
        ),
        K::UnreachableArm => (
            C::UNREACHABLE_ARM,
            "this arm is never taken: the arms before it match every value it does".into(),
        ),
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
        BecauseKind::Parameter { function: Some(name) } => {
            format!("expected because of this parameter of `{name}`")
        }
        BecauseKind::Parameter { function: None } => {
            "expected because of this parameter".to_string()
        }
        BecauseKind::Field { name } => format!("expected because of the type of field `{name}`"),
        BecauseKind::SameAs { ty } => format!("this has type `{}`", ty.display_with(db, names)),
        BecauseKind::ClassItem { class } => {
            let class = class_name(db, *class);
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
            let names: Vec<String> =
                items.iter().filter(|(k, _)| *k == kind).map(|(_, n)| format!("`{n}`")).collect();
            (!names.is_empty())
                .then(|| format!("{} {}", item_noun(kind, names.len()), names.join(", ")))
        })
        .collect();
    join_list(&groups, "and")
}

/// The name of `class`, or `?` for a class without one.
fn class_name<'db>(db: &'db dyn Db, class: ClassId<'db>) -> &'db str {
    class.item(db).name.as_ref().map_or("?", |n| n.as_str())
}

fn constraint_display(db: &dyn Db, names: &TyNames, constraint: &ClassConstraint<'_>) -> String {
    let name = class_name(db, constraint.class);
    let args: Vec<_> =
        constraint.args.iter().map(|t| t.display_with(db, names).to_string()).collect();
    format!("{name}[{}]", args.join(", "))
}

fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// Types as text in a sentence: `` `a` ``, `` `a` and `b` `` or
/// `` `a`, `b` and `c` ``.
fn list_types(tys: &[String]) -> String {
    join_list(&quoted(tys), "and")
}

/// `a`, `b` or `c`.
fn either(items: &[String]) -> String {
    join_list(&quoted(items), "or")
}

fn quoted(items: &[String]) -> Vec<String> {
    items.iter().map(|t| format!("`{t}`")).collect()
}

/// The items with commas between them, and `conjunction` before the last,
/// like "a, b and c".
fn join_list(items: &[String], conjunction: &str) -> String {
    match items.split_last() {
        None => String::new(),
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} {conjunction} {last}", rest.join(", ")),
    }
}

/// The message for an import of `path` that can't be found.
fn import_path_message(path: &str, error: ImportPathError) -> String {
    match error {
        ImportPathError::FileNotFound => format!("cannot find the file `{path}`"),
        ImportPathError::NoManifest { package } => format!(
            "cannot find the package `{package}`: this file is in no package, so it can only use `std`; \
             add a `bolt.toml` with `{package}` as a dependency"
        ),
        ImportPathError::NotADependency { package } => {
            format!(
                "`{package}` isn't a dependency of this package: add it to `[dependencies]` in `bolt.toml`"
            )
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
        // Only the prelude is looked for in two files.
        ImportPathError::TwoPreludes { package } => {
            format!("the package `{package}` has two preludes")
        }
    }
}

/// The message for the prelude of `package` that can't be found.
fn prelude_message(package: &str, error: ImportPathError) -> String {
    match error {
        ImportPathError::NoManifest { .. } | ImportPathError::NotADependency { .. } => {
            format!("cannot find the package `{package}` of the prelude: it isn't a dependency")
        }
        ImportPathError::BadManifest => {
            format!(
                "cannot find the prelude of `{package}`: the `bolt.toml` of this package has an error"
            )
        }
        ImportPathError::DependencyNotFound { path, .. } => {
            format!(
                "the package `{package}` of the prelude is at `{path}`, which has no `bolt.toml`"
            )
        }
        ImportPathError::NoStd => "there's no `std`, which the prelude comes from".to_string(),
        ImportPathError::NotInPackage { .. } => format!(
            "the package `{package}` has no prelude: it needs a file `__prelude__.bolt`, or \
             `__prelude__/lib.bolt` for a prelude of several files"
        ),
        ImportPathError::FileNotFound | ImportPathError::Invalid => {
            format!("cannot use `{package}` as the prelude: it's not the name of a package")
        }
        ImportPathError::TwoPreludes { .. } => format!(
            "the package `{package}` has both a `__prelude__.bolt` and a `__prelude__/lib.bolt`, so \
             it isn't clear which is its prelude; remove one of them"
        ),
    }
}

/// What's wrong with a `#[link]`.
fn bad_link_message(db: &dyn Db, names: &TyNames, reason: BadLink<'_>) -> String {
    match reason {
        BadLink::NotFunctionOrType => {
            "`#[link]` can only be on a function or a type, which the host provides".to_string()
        }
        BadLink::TypeHasValue => {
            "a type linked to the host can't have `= ..`: the host provides it".to_string()
        }
        BadLink::TypeParams => {
            "a type linked to the host can't have type parameters yet".to_string()
        }
        BadLink::TypeNotWasm => {
            "a type of the host is declared with just `#[link(wasm)]`: it has no \
                                 module, name or ABI, and only Wasm has types of the host"
                .to_string()
        }
        BadLink::NotTopLevel => {
            "a function or type linked to the host must be at the top level of a file, not \
                                 in a class, an impl or the namespace of a type"
                .to_string()
        }
        BadLink::HasBody => {
            "a function linked to the host can't have a body: the host provides it".to_string()
        }
        BadLink::Builtin => "a function can't be both a builtin and linked to the host".to_string(),
        BadLink::Written(e) => link_error(&e),
        BadLink::FlatParam(ty) => format!(
            "a function linked with `abi = flat` can't take a `{}`: only scalars, arrays of them, and \
             arrays of `[u8]`",
            ty.display_with(db, names)
        ),
        BadLink::FlatResult(ty) => format!(
            "a function linked with `abi = flat` can't give a `{}`: only scalars, or a tuple of them",
            ty.display_with(db, names)
        ),
        BadLink::Addr => {
            "`#[addr]` is only for an array that a function linked with `abi = flat` takes"
                .to_string()
        }
        BadLink::EmuGated(key) => format!(
            "a function linked to `emu` can't be in a `#[cfg]` on `{key}`, only on `host`: the evaluator \
             is the same host for every machine it imitates"
        ),
        BadLink::EmuSize(ty) => format!(
            "a function linked to `emu` can't take or give a `{}`, whose width is the machine's: the \
             evaluator is the same host for every machine it imitates",
            ty.display_with(db, names)
        ),
    }
}

/// What's wrong with a `#[test]`.
fn bad_test_message(reason: BadTest) -> String {
    match reason {
        BadTest::NotTopLevel => {
            "a test must be a function at the top level of a file, not in a class, an impl or the namespace \
             of a type"
        }
        BadTest::Params => "a test can't have parameters: nothing gives it any",
        BadTest::Generic => "a test can't have type parameters: nothing decides them",
        BadTest::Result => "a test must give `()`: nothing looks at what it gives",
    }
    .to_string()
}

/// What's wrong with a `#[cfg(..)]`.
fn cfg_error(e: &boltlang_hir_def::cfg::CfgError) -> String {
    use boltlang_hir_def::cfg::{CfgError as E, KEYS};
    let keys: Vec<String> = KEYS.iter().map(|k| k.to_string()).collect();
    match e {
        E::Malformed => "`#[cfg]` must say which targets the item is for; write it like \
                         `#[cfg(host = \"emu\")]`, with `all(..)`, `any(..)` and `not(..)` to combine \
                         conditions"
            .to_string(),
        E::UnknownKey(key) => {
            format!("there is no `#[cfg]` key `{key}`; the keys are {}", list_types(&keys))
        }
        E::Reserved(key) => {
            format!("`{key}` can't be used in `#[cfg]` yet: it's kept for when there's a native target")
        }
        E::UnknownValue { key, value, expected } => {
            format!("`{value}` isn't a `{key}` that `#[cfg]` knows; it's {}", either(expected))
        }
        E::NotArity => "`not(..)` takes exactly one condition".to_string(),
        E::NotAnItem => {
            "`#[cfg]` can only be on an item outside a body, like a function, a struct or an import"
                .to_string()
        }
    }
}

/// What's wrong with how a `#[link(..)]` is written.
fn link_error(e: &boltlang_hir_def::item_tree::LinkError) -> String {
    use boltlang_hir_def::item_tree::LinkError as E;
    const FORM: &str = "write it like `#[link(wasm, module = \"env\", name = \"log\")]`";
    match e {
        E::NoTarget => format!("`#[link]` must say what the function is linked to first; {FORM}"),
        E::UnknownTarget(t) => {
            format!(
                "a function can't be linked to `{t}`: it's linked to `wasm`, a Wasm import, or `emu`, \
                 the evaluator; {FORM}"
            )
        }
        E::NoNativeTarget => {
            "a function can't be linked to `c` yet: that comes with the native target".to_string()
        }
        E::NoModule => format!("`#[link(wasm, ..)]` needs the Wasm module of the import; {FORM}"),
        E::ModuleForEmu => {
            "`#[link(emu, ..)]` has no module: the evaluator provides it by its `name` alone"
                .to_string()
        }
        E::UnknownKey(k) => format!("`#[link]` has no `{k}`; it takes `module`, `name` and `abi`"),
        E::UnknownAbi(a) => format!("there's no ABI `{a}`; the only one is `flat`"),
        E::Repeated(k) => format!("`{k}` is given twice"),
        E::Malformed => {
            format!(
                "after `wasm`, `#[link]` takes `key = \"string\"` pairs, and `abi = flat`; {FORM}"
            )
        }
    }
}

#[cfg(test)]
mod tests;

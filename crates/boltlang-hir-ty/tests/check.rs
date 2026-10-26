//! Type checker tests driven by the `.txt` files in `tests/checker` at the
//! root of the repository, in the format of the tree-sitter corpus. Each
//! case is a separate test, named after its file and title, e.g.
//! `inference::mutual_recursion`.
//!
//! ```text
//! ================================================================================
//! Mutual recursion
//! ================================================================================
//! fn even(n) = if n == 0 { true } else { odd(n - 1) };
//! fn odd(n) = if n == 0 { false } else { even(n - 1) };
//! --------------------------------------------------------------------------------
//! even: fn(i32) -> bool
//! odd: fn(i32) -> bool
//! --------------------------------------------------------------------------------
//! error at `x`: expected bool, found i32
//! ```
//!
//! A case is a title between two lines of `=`, the Bolt code to check, and
//! after a line of `-` what the checker found: the types of the functions,
//! then of the top-level statements. The diagnostics come after another line
//! of `-`, which is left out when there are none. Separators are at least
//! three characters long and start at the beginning of a line. A new case
//! can leave out its output: running with `UPDATE_EXPECT=1` fills it in.
//!
//! A case can have several files, each starting with a line like
//! `//- /lib.bolt`, so that they can import each other. Without such lines,
//! the code is `/main.bolt`. With several files, each line of the output
//! starts with the path of its file, like `/lib.bolt: id: fn(a) -> a`.
//! As in the compiler, files get the prelude of `std` when there's one, so a
//! case can have one in `/.bolt/packages/std/__prelude__/lib.bolt`.
//!
//! Run a single case with `cargo test --test check mutual_recursion`. To
//! write what the checker finds into the files instead of comparing it, run
//! with `UPDATE_EXPECT=1`, and review the changes with `git diff`.
//!
//! Lines starting with `:` after the title are attributes, as in the
//! tree-sitter corpus:
//!
//! ```text
//! ================================================================================
//! Polymorphic recursion
//! :todo
//! ================================================================================
//! ```
//!
//! - `:skip`: never run the case, not even with `--ignored`, and report it
//!   as ignored. For a case that crashes or hangs.
//! - `:todo`: the output is what the checker should find, but doesn't yet.
//!   The case is reported as ignored; run the cases with
//!   `cargo test --test check -- --ignored` to see how far along they are.
//! - `:error`: the case must find at least one error, so that an update
//!   can't make it a case without errors by accident.
//!
//! `UPDATE_EXPECT=1` leaves the output of `:skip` and `:todo` cases alone,
//! since it's written by hand, and that of an `:error` case without errors.

use std::{
    ops::Range,
    panic::{AssertUnwindSafe, catch_unwind},
    path::{Path, PathBuf},
};

use boltlang_base_db::{TestDb, source_text};
use boltlang_hir_def::{
    body::{DefWithBody, DefWithBodyId, body_with_source_map},
    expr_store::ExpressionStoreSourceMap,
    hir::{Expr, Statement, TypeRefId},
    ids::{FunctionId, ImplId, ItemContainerId},
    item_tree::{AssocItem, FunctionName, ModItem, item_tree},
    resolver::GenericDefId,
    signatures::{function_signature_with_source_map, impl_signature_with_source_map},
};
use boltlang_hir_ty::{
    InferenceDiagnostic, Ty, TyNames, function_scheme, infer_body,
    lower::{LowerDiagnostic, fn_sig, generic_predicates, impl_header},
    solve::{ExprOrPat, TypeError},
};
use libtest_mimic::{Arguments, Failed, Trial};

/// One test case of a data file.
struct Case {
    title: String,
    attributes: Attributes,
    input: String,
    /// The types, then the diagnostics.
    expected: (String, String),
    /// The lines of the title and the input, which an update keeps.
    head: Range<usize>,
    /// All the lines of the case, which an update keeps for `:skip` and
    /// `:todo` cases.
    lines: Range<usize>,
}

/// The attributes of a case, like `:todo`.
#[derive(Default)]
struct Attributes {
    skip: bool,
    todo: bool,
    error: bool,
}

impl Attributes {
    /// Whether the output is written by hand, so that updates keep it.
    fn manual(&self) -> bool {
        self.skip || self.todo
    }
}

/// Whether `line` is made of at least three `c`s.
fn is_separator(line: &str, c: char) -> bool {
    let line = line.trim_end();
    line.len() >= 3 && line.chars().all(|x| x == c)
}

/// The cases in the text of a data file.
fn parse_cases(text: &str) -> Result<Vec<Case>, String> {
    let lines: Vec<&str> = text.lines().collect();
    let mut cases = Vec::new();
    let mut i = 0;
    // Skip anything before the first case, like blank lines.
    while i < lines.len() && !is_separator(lines[i], '=') {
        i += 1;
    }
    while i < lines.len() {
        // The title, between two lines of `=`.
        let head_start = i;
        let start = i + 1;
        i = start;
        while i < lines.len() && !is_separator(lines[i], '=') {
            i += 1;
        }
        if i == lines.len() {
            return Err(format!("line {start}: the title isn't followed by a line of `=`"));
        }
        let (attribute_lines, title_lines): (Vec<&str>, Vec<&str>) =
            lines[start..i].iter().partition(|line| line.trim_start().starts_with(':'));
        let title = title_lines.join(" ").trim().to_string();
        let mut attributes = Attributes::default();
        for line in attribute_lines {
            match line.trim() {
                ":skip" => attributes.skip = true,
                ":todo" => attributes.todo = true,
                ":error" => attributes.error = true,
                other => {
                    return Err(format!(
                        "`{title}`: unknown attribute `{other}`; the attributes are `:skip`, `:todo` and `:error`"
                    ));
                }
            }
        }
        i += 1;
        // The sections, up to the next case.
        let mut sections = vec![Vec::new()];
        let mut head_end = None;
        while i < lines.len() && !is_separator(lines[i], '=') {
            if is_separator(lines[i], '-') {
                head_end.get_or_insert(i);
                sections.push(Vec::new());
            } else {
                sections.last_mut().unwrap().push(lines[i]);
            }
            i += 1;
        }
        let section = |n: usize| sections.get(n).map(|s: &Vec<&str>| s.join("\n").trim().to_string()).unwrap_or_default();
        // Without a line of `-`, the case has no output yet: it fails, and
        // `UPDATE_EXPECT=1` writes it.
        if sections.len() > 3 {
            return Err(format!("`{title}`: expected the input, the types and optionally the diagnostics"));
        }
        let head = head_start..head_end.unwrap_or(i);
        let input = section(0) + "\n";
        let expected = (section(1), section(2));
        cases.push(Case { title, attributes, input, expected, head, lines: head_start..i });
    }
    Ok(cases)
}

/// The test name for a title: `Mutual recursion` is `mutual_recursion`.
fn slug(title: &str) -> String {
    let slug: String = title.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect();
    slug.split('_').filter(|s| !s.is_empty()).collect::<Vec<_>>().join("_")
}

/// The files of a case: its input split at the lines like `//- /lib.bolt`
/// that start a file, or `/main.bolt` without them.
fn split_files(input: &str) -> Result<Vec<(String, String)>, String> {
    if !input.lines().any(|line| line.starts_with("//-")) {
        return Ok(vec![("/main.bolt".to_string(), input.to_string())]);
    }
    let mut files: Vec<(String, String)> = Vec::new();
    for line in input.lines() {
        if let Some(rest) = line.strip_prefix("//-") {
            let path = rest.trim();
            if !path.starts_with('/') || path.contains(char::is_whitespace) {
                return Err(format!("expected an absolute path after `//-`, like `//- /lib.bolt`: `{line}`"));
            }
            if files.iter().any(|(p, _)| p == path) {
                return Err(format!("two files are called `{path}`"));
            }
            files.push((path.to_string(), String::new()));
        } else if let Some((_, text)) = files.last_mut() {
            text.push_str(line);
            text.push('\n');
        } else if !line.trim().is_empty() {
            return Err(format!("code before the first `//-` line: `{line}`"));
        }
    }
    Ok(files)
}

/// The types of the functions in the files of a case and of their
/// top-level statements, and the diagnostics, each with the code it's
/// about. With several files, each line starts with the path of its file.
fn check(input: &str) -> Result<(String, String), String> {
    let mut db = TestDb::new();
    // As in the compiler, the prelude is the one of `std`, if there is one.
    db.set_prelude(Some("std"), false);
    // The types of a line, with names like `a` for the unknown ones, as in
    // the diagnostics of the compiler.
    let show = |tys: &[&Ty]| -> Vec<String> {
        let names = TyNames::new(&db, tys);
        tys.iter().map(|ty| ty.display_with(&db, &names).to_string()).collect()
    };
    let files: Vec<_> = split_files(input)?.into_iter().map(|(path, text)| (db.file(&path, &text), path)).collect();
    let mut types = Vec::new();
    let mut diagnostics = Vec::new();
    for &(file, ref path) in &files {
        let prefix = if files.len() > 1 { format!("{path}: ") } else { String::new() };
        let tree = item_tree(&db, file);
        let text = source_text(&db, file);
        let type_snippet = |map: &ExpressionStoreSourceMap, id| {
            map.type_syntax(id).map_or("?".to_string(), |p| text[p.start as usize..p.end as usize].to_string())
        };
        let mut defs = Vec::new();
        // The functions of the file and of its impls, those of an impl named
        // after its instance, like `Show[i32].show`.
        let mut functions = Vec::new();
        for item in &tree.top_level {
            match *item {
                ModItem::Function(idx) => functions.push((String::new(), FunctionId::new(&db, ItemContainerId::File(file), idx))),
                ModItem::Impl(idx) => {
                    let id = ImplId::new(&db, file, idx);
                    let header = impl_header(&db, id);
                    let instance = match &header.class {
                        Some(c) => format!(
                            "{}[{}].",
                            c.class.item(&db).name.as_ref().unwrap(),
                            show(&c.args.iter().collect::<Vec<_>>()).join(", ")
                        ),
                        None => "impl.".to_string(),
                    };
                    let (_, map) = impl_signature_with_source_map(&db, id);
                    for lower in &header.diagnostics {
                        diagnostics.push(format!("{prefix}{}", lower_line(lower, &|id| type_snippet(map, id))));
                    }
                    for item in &id.item(&db).items {
                        if let AssocItem::Function(f) = *item {
                            functions.push((instance.clone(), FunctionId::new(&db, ItemContainerId::Impl(id), f)));
                        }
                    }
                }
                _ => {}
            }
        }
        for (instance, f) in functions {
            let name = match f.item(&db).name.as_ref().unwrap() {
                FunctionName::Name(n) | FunctionName::Operator(n) => format!("{instance}{n}"),
            };
            let ty = match function_scheme(&db, f) {
                Ok(scheme) => scheme.display(&db),
                Err(()) => "<cycle>".into(),
            };
            types.push(format!("{prefix}{name}: {ty}"));
            // The problems with the types in its signature.
            let (_, map) = function_signature_with_source_map(&db, f);
            let sig = fn_sig(&db, f);
            let predicates = generic_predicates(&db, GenericDefId::Function(f));
            for lower in sig.diagnostics.iter().chain(&predicates.diagnostics) {
                diagnostics.push(format!("{prefix}{}", lower_line(lower, &|id| type_snippet(map, id))));
            }
            if f.item(&db).has_body {
                defs.push(DefWithBodyId::new(&db, DefWithBody::Function(f)));
            }
        }
        let top = DefWithBodyId::new(&db, DefWithBody::File(file));
        defs.push(top);
        let (top_body, top_map) = body_with_source_map(&db, top);
        let snippet = |map: &ExpressionStoreSourceMap, node: ExprOrPat| {
            let ptr = match node {
                ExprOrPat::Expr(e) => map.expr_syntax(e),
                ExprOrPat::Pat(p) => map.pat_syntax(p),
            };
            ptr.map_or("?".to_string(), |p| text[p.start as usize..p.end as usize].to_string())
        };
        let Expr::Block { statements, .. } = &top_body[top_body.body_expr] else { panic!() };
        let top_result = infer_body(&db, top);
        for statement in statements.iter() {
            match statement {
                Statement::Expr { expr, .. } => types.push(format!(
                    "{prefix}{}: {}",
                    snippet(top_map, ExprOrPat::Expr(*expr)),
                    show(&[top_result.expr_ty(*expr)])[0]
                )),
                Statement::Let { pat, .. } => types.push(format!(
                    "{prefix}let {}: {}",
                    snippet(top_map, ExprOrPat::Pat(*pat)),
                    show(&[top_result.pat_ty(*pat)])[0]
                )),
                Statement::Item => {}
            }
        }
        for def in defs {
            let (_, map) = body_with_source_map(&db, def);
            for diagnostic in &infer_body(&db, def).diagnostics {
                let at = |node| snippet(map, node);
                let at_type = |id| type_snippet(map, id);
                let line = match diagnostic {
                    InferenceDiagnostic::Type(TypeError::Mismatch { origin, expected, actual }) => {
                        let tys = show(&[expected, actual]);
                        format!("error at `{}`: expected {}, found {}", at(origin.node), tys[0], tys[1])
                    }
                    InferenceDiagnostic::Type(TypeError::NoInstance { origin, constraint }) => format!(
                        "error at `{}`: no instance {}[{}]",
                        at(origin.node),
                        constraint.class.item(&db).name.as_ref().unwrap(),
                        show(&constraint.args.iter().collect::<Vec<_>>()).join(", ")
                    ),
                    InferenceDiagnostic::Type(TypeError::Ambiguous { origin, .. }) => {
                        format!("error at `{}`: ambiguous", at(origin.node))
                    }
                    InferenceDiagnostic::UnresolvedName { expr } => format!("error at `{}`: unresolved", at(*expr)),
                    InferenceDiagnostic::WrongArgCount { expr, expected, found } => {
                        format!("error at `{}`: expected {expected} arguments, found {found}", at(*expr))
                    }
                    InferenceDiagnostic::NoSuchField { expr, name } => {
                        format!("error at `{}`: no field {name}", at(*expr))
                    }
                    InferenceDiagnostic::Type(TypeError::TypeNeeded { origin }) => {
                        format!("error at `{}`: type needed", at(origin.node))
                    }
                    InferenceDiagnostic::Type(TypeError::NoSuchField { origin, ty, name }) => {
                        format!("error at `{}`: {} has no field {name}", at(origin.node), show(&[ty])[0])
                    }
                    InferenceDiagnostic::MissingFields { expr, names } => format!(
                        "error at `{}`: missing {}",
                        at(ExprOrPat::Expr(*expr)),
                        names.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(", ")
                    ),
                    InferenceDiagnostic::Type(TypeError::InfiniteType { origin, ty }) => {
                        format!("error at `{}`: infinite type {}", at(origin.node), show(&[ty])[0])
                    }
                    InferenceDiagnostic::NotAFunction { expr, ty } => {
                        format!("error at `{}`: {} is not a function", at(ExprOrPat::Expr(*expr)), show(&[ty])[0])
                    }
                    InferenceDiagnostic::BreakOutside { expr } => {
                        format!("error at `{}`: nothing to break out of", at(ExprOrPat::Expr(*expr)))
                    }
                    InferenceDiagnostic::AssignToImmutable { expr, .. } => {
                        format!("error at `{}`: not mut", at(ExprOrPat::Expr(*expr)))
                    }
                    InferenceDiagnostic::Unsupported { expr } => {
                        format!("warning at `{}`: unsupported", at(ExprOrPat::Expr(*expr)))
                    }
                    InferenceDiagnostic::Lower(lower) => lower_line(lower, &at_type),
                    InferenceDiagnostic::CyclicInference => "error: cyclic inference between files".to_string(),
                };
                diagnostics.push(format!("{prefix}{line}"));
            }
        }
    }
    Ok((types.join("\n"), diagnostics.join("\n")))
}

/// A problem with a type annotation, with `at` giving its code.
fn lower_line(lower: &LowerDiagnostic, at: &dyn Fn(TypeRefId) -> String) -> String {
    match lower {
        LowerDiagnostic::UnresolvedType(id) => format!("error at `{}`: unresolved type", at(*id)),
        LowerDiagnostic::NotAType(id) => format!("error at `{}`: not a type", at(*id)),
        LowerDiagnostic::NotAClass(id) => format!("error at `{}`: not a class", at(*id)),
        LowerDiagnostic::WrongArgCount { type_ref, expected, found } => {
            format!("error at `{}`: expected {expected} type arguments, found {found}", at(*type_ref))
        }
        LowerDiagnostic::KindMismatch { type_ref, expected, found } => {
            format!("error at `{}`: expected kind {expected}, found {found}", at(*type_ref))
        }
        LowerDiagnostic::RecursiveAlias(id) => format!("error at `{}`: recursive alias", at(*id)),
        LowerDiagnostic::Unsupported(id) => format!("warning at `{}`: unsupported", at(*id)),
    }
}

/// What the checker finds for a case, or why it couldn't check it: its
/// files are wrong, or it panicked.
fn check_caught(case: &Case) -> Result<(String, String), String> {
    catch_unwind(AssertUnwindSafe(|| check(&case.input))).unwrap_or_else(|payload| {
        let message = payload
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_default();
        Err(format!("the checker panicked: {message}"))
    })
}

fn run(case: &Case) -> Result<(), Failed> {
    let actual = check_caught(case)?;
    if actual != case.expected {
        return Err(format!("expected:\n{}\nfound:\n{}", show(&case.expected), show(&actual)).into());
    }
    if case.attributes.error && !has_errors(&actual) {
        return Err("the case is marked `:error`, but the checker found no errors".into());
    }
    Ok(())
}

/// Whether the diagnostics of an output have an error, like
/// `error at ...` or `/lib.bolt: error at ...`.
fn has_errors((_, diagnostics): &(String, String)) -> bool {
    diagnostics.lines().any(|line| {
        let line = line.split_once(": ").filter(|(path, _)| path.starts_with('/')).map_or(line, |(_, rest)| rest);
        line.starts_with("error")
    })
}

/// The output sections of a case, without the line of `-` before them.
fn show((types, diagnostics): &(String, String)) -> String {
    if diagnostics.is_empty() { format!("{types}\n") } else { format!("{types}\n{SECTION}\n{diagnostics}\n") }
}

/// Write what the checker finds into the output sections of the cases in
/// `text`. A case that can't be checked keeps its sections, and fails when
/// it runs.
fn update(text: &str, cases: &[Case]) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = String::new();
    let first = cases.first().map_or(lines.len(), |c| c.head.start);
    for line in &lines[..first] {
        out.push_str(line);
        out.push('\n');
    }
    for (i, case) in cases.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        if case.attributes.manual() {
            let mut kept = &lines[case.lines.clone()];
            while let [rest @ .., last] = kept
                && last.trim().is_empty()
            {
                kept = rest;
            }
            for line in kept {
                out.push_str(line);
                out.push('\n');
            }
            continue;
        }
        for line in &lines[case.head.clone()] {
            out.push_str(line);
            out.push('\n');
        }
        let found = match check_caught(case) {
            Ok(found) if !case.attributes.error || has_errors(&found) => found,
            _ => case.expected.clone(),
        };
        out.push_str(SECTION);
        out.push('\n');
        out.push_str(&show(&found));
    }
    out
}

const SECTION: &str = "--------------------------------------------------------------------------------";

/// The data files, in alphabetical order.
fn data_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("can't read {}: {e}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "txt"))
        .collect();
    files.sort();
    files
}

fn main() {
    let args = Arguments::from_args();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/checker");
    let updating = std::env::var_os("UPDATE_EXPECT").is_some_and(|v| !v.is_empty() && v != "0");
    let mut trials = Vec::new();
    for path in data_files(&root) {
        let stem = path.file_stem().unwrap().to_str().unwrap().to_string();
        let mut text = std::fs::read_to_string(&path).unwrap();
        if updating && let Ok(cases) = parse_cases(&text) {
            let updated = update(&text, &cases);
            if updated != text {
                std::fs::write(&path, &updated).unwrap();
                text = updated;
            }
        }
        let cases = match parse_cases(&text) {
            Ok(cases) => cases,
            Err(error) => {
                let message = format!("{}: {error}", path.display());
                trials.push(Trial::test(stem, move || Err(message.into())));
                continue;
            }
        };
        let mut seen = Vec::new();
        for case in cases {
            let name = format!("{stem}::{}", slug(&case.title));
            if seen.contains(&name) {
                let message = format!("{}: two cases are called `{}`", path.display(), case.title);
                trials.push(Trial::test(format!("{name}_again"), move || Err(message.into())));
                continue;
            }
            seen.push(name.clone());
            let manual = case.attributes.manual();
            let trial = if case.attributes.skip {
                // Never run, even with `--ignored`, since it may hang.
                Trial::test(name, || Ok(())).with_kind("skip")
            } else if case.attributes.todo {
                Trial::test(name, move || run(&case)).with_kind("todo")
            } else {
                Trial::test(name, move || run(&case))
            };
            trials.push(trial.with_ignored_flag(manual));
        }
    }
    libtest_mimic::run(&args, trials).exit();
}

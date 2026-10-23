//! Type checker tests driven by the `.txt` files in `tests/data`, in the
//! format of the tree-sitter corpus. Each case is a separate test, named
//! after its file and title, e.g. `inference::mutual_recursion`.
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
//! three characters long and start at the beginning of a line.
//!
//! Run a single case with `cargo test --test check mutual_recursion`.

use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    path::{Path, PathBuf},
};

use boltlang_base_db::{TestDb, source_text};
use boltlang_hir_def::{
    body::{DefWithBody, DefWithBodyId, body_with_source_map},
    expr_store::ExpressionStoreSourceMap,
    hir::{Expr, Statement},
    ids::{FunctionId, ItemContainerId},
    item_tree::{FunctionName, ModItem, item_tree},
};
use boltlang_hir_ty::{
    InferenceDiagnostic, function_scheme, infer_body,
    solve::{ExprOrPat, TypeError},
};
use libtest_mimic::{Arguments, Failed, Trial};

/// One test case of a data file.
struct Case {
    title: String,
    input: String,
    /// The types, then the diagnostics.
    expected: (String, String),
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
        let start = i + 1;
        i = start;
        while i < lines.len() && !is_separator(lines[i], '=') {
            i += 1;
        }
        if i == lines.len() {
            return Err(format!("line {start}: the title isn't followed by a line of `=`"));
        }
        let title = lines[start..i].join(" ").trim().to_string();
        i += 1;
        // The sections, up to the next case.
        let mut sections = vec![Vec::new()];
        while i < lines.len() && !is_separator(lines[i], '=') {
            if is_separator(lines[i], '-') {
                sections.push(Vec::new());
            } else {
                sections.last_mut().unwrap().push(lines[i]);
            }
            i += 1;
        }
        let section = |n: usize| sections.get(n).map(|s: &Vec<&str>| s.join("\n").trim().to_string()).unwrap_or_default();
        if sections.len() < 2 || sections.len() > 3 {
            return Err(format!("`{title}`: expected the input, the types and optionally the diagnostics"));
        }
        cases.push(Case { input: section(0) + "\n", expected: (section(1), section(2)), title });
    }
    Ok(cases)
}

/// The test name for a title: `Mutual recursion` is `mutual_recursion`.
fn slug(title: &str) -> String {
    let slug: String = title.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect();
    slug.split('_').filter(|s| !s.is_empty()).collect::<Vec<_>>().join("_")
}

/// The types of the functions in `source` and of its top-level statements,
/// and the diagnostics, each with the code it's about.
fn check(source: &str) -> (String, String) {
    let db = TestDb::new();
    let file = db.file("/main.bolt", source);
    let tree = item_tree(&db, file);
    let mut types = Vec::new();
    let mut diagnostics = Vec::new();
    let mut defs = Vec::new();
    for item in &tree.top_level {
        let ModItem::Function(idx) = *item else { continue };
        let f = FunctionId::new(&db, ItemContainerId::File(file), idx);
        let name = match f.item(&db).name.as_ref().unwrap() {
            FunctionName::Name(n) | FunctionName::Operator(n) => n.to_string(),
        };
        let ty = match function_scheme(&db, f) {
            Ok(scheme) => scheme.display(&db),
            Err(()) => "<cycle>".into(),
        };
        types.push(format!("{name}: {ty}"));
        if f.item(&db).has_body {
            defs.push(DefWithBodyId::new(&db, DefWithBody::Function(f)));
        }
    }
    let top = DefWithBodyId::new(&db, DefWithBody::File(file));
    defs.push(top);
    let (top_body, top_map) = body_with_source_map(&db, top);
    let text = source_text(&db, file);
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
                "{}: {}",
                snippet(top_map, ExprOrPat::Expr(*expr)),
                top_result.expr_ty(*expr).display(&db)
            )),
            Statement::Let { pat, .. } => types.push(format!(
                "let {}: {}",
                snippet(top_map, ExprOrPat::Pat(*pat)),
                top_result.pat_ty(*pat).display(&db)
            )),
            Statement::Item => {}
        }
    }
    for def in defs {
        let (_, map) = body_with_source_map(&db, def);
        for diagnostic in &infer_body(&db, def).diagnostics {
            let at = |node| snippet(map, node);
            diagnostics.push(match diagnostic {
                InferenceDiagnostic::Type(TypeError::Mismatch { origin, expected, actual }) => format!(
                    "error at `{}`: expected {}, found {}",
                    at(origin.node),
                    expected.display(&db),
                    actual.display(&db)
                ),
                InferenceDiagnostic::Type(TypeError::NoInstance { origin, constraint }) => format!(
                    "error at `{}`: no instance {}[{}]",
                    at(origin.node),
                    constraint.class.item(&db).name.as_ref().unwrap(),
                    constraint.args.iter().map(|t| t.display(&db).to_string()).collect::<Vec<_>>().join(", ")
                ),
                InferenceDiagnostic::Type(TypeError::Ambiguous { origin, .. }) => {
                    format!("error at `{}`: ambiguous", at(origin.node))
                }
                InferenceDiagnostic::UnresolvedName { expr } => format!("error at `{}`: unresolved", at(*expr)),
                InferenceDiagnostic::WrongArgCount { expr, expected, found } => {
                    format!("error at `{}`: expected {expected} arguments, found {found}", at(*expr))
                }
                InferenceDiagnostic::NoSuchField { expr, name } => format!("error at `{}`: no field {name}", at(*expr)),
                InferenceDiagnostic::Type(TypeError::TypeNeeded { origin }) => {
                    format!("error at `{}`: type needed", at(origin.node))
                }
                InferenceDiagnostic::Type(TypeError::NoSuchField { origin, ty, name }) => {
                    format!("error at `{}`: {} has no field {name}", at(origin.node), ty.display(&db))
                }
                InferenceDiagnostic::MissingFields { expr, names } => format!(
                    "error at `{}`: missing {}",
                    at(ExprOrPat::Expr(*expr)),
                    names.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(", ")
                ),
                other => format!("error: {other:?}"),
            });
        }
    }
    (types.join("\n"), diagnostics.join("\n"))
}

fn run(case: &Case) -> Result<(), Failed> {
    let actual = match catch_unwind(AssertUnwindSafe(|| check(&case.input))) {
        Ok(actual) => actual,
        Err(payload) => {
            let message = payload
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_default();
            return Err(format!("the checker panicked: {message}").into());
        }
    };
    if actual == case.expected {
        return Ok(());
    }
    let show = |(types, diagnostics): &(String, String)| {
        if diagnostics.is_empty() { format!("{types}\n") } else { format!("{types}\n{SECTION}\n{diagnostics}\n") }
    };
    Err(format!("expected:\n{}\nfound:\n{}", show(&case.expected), show(&actual)).into())
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
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let mut trials = Vec::new();
    for path in data_files(&root) {
        let stem = path.file_stem().unwrap().to_str().unwrap().to_string();
        let text = std::fs::read_to_string(&path).unwrap();
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
            trials.push(Trial::test(name, move || run(&case)));
        }
    }
    libtest_mimic::run(&args, trials).exit();
}

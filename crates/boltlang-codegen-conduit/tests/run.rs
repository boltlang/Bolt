//! Tests that run the programs in `tests/run` at the root of the
//! repository with Conduit's evaluator, as `bolt run` does. The files have
//! the format of `boltlang-corpus`:
//!
//! ```text
//! ================================================================================
//! Factorial
//! :test-prelude
//! ================================================================================
//! fn fac(n: i32) -> i32 = if n == 0 { 1 } else { n * fac(n - 1) };
//! print(show(fac(5)));
//! --------------------------------------------------------------------------------
//! 120
//! ```
//!
//! The output is what the program writes to stdout and stderr. When it
//! fails, the last line is `error: ` and the message, like
//! `error: the program stopped with an error`. When it can't be lowered
//! yet, the output is the lowering's errors, each starting with `error `.
//!
//! The programs get no prelude, unless the case has the attribute
//! `:prelude`, for that of `packages/std`, or `:test-prelude`, for the
//! small one of `tests/std`, which doesn't change when `std` does. They
//! must have no type errors, nor must the prelude. A case can have several
//! files, each starting with a line like `//- /lib.bolt`; the program is
//! `/main.bolt`.
//!
//! Lines starting with `:` after the title are attributes:
//!
//! - `:prelude` and `:test-prelude`: the prelude, as above.
//! - `:todo`: the output is what the program should give, but doesn't yet.
//!   The case is ignored, unless run with `--ignored`.
//! - `:skip`: never run, for programs that crash or hang.
//! - `:fuel N`: stop after about `N` steps, instead of ten million.
//!
//! Run the cases of one file with `cargo test --test run functions::`. To
//! write the output into the files instead of comparing it, run with
//! `UPDATE_EXPECT=1`, and review the changes with `git diff`.

use std::{path::Path, sync::Arc};

use boltlang_base_db::{File, TestDb};
use boltlang_codegen_conduit::{lower, run::run_captured};
use boltlang_corpus::{
    Case, catch, data_files, data_name, parse_cases, slug, split_files, update, updating,
};
use boltlang_ide_diagnostics::{Severity, dependency_diagnostics, diagnostics};
use libtest_mimic::{Arguments, Failed, Trial};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Prelude {
    #[default]
    None,
    Std,
    Test,
}

struct Attributes {
    todo: bool,
    skip: bool,
    fuel: u64,
    prelude: Prelude,
}

impl Attributes {
    fn parse(case: &Case) -> Result<Self, String> {
        let mut attributes =
            Attributes { todo: false, skip: false, fuel: 10_000_000, prelude: Prelude::None };
        for attribute in &case.attributes {
            let mut words = attribute.split_whitespace();
            match (words.next(), words.next()) {
                (Some("todo"), None) => attributes.todo = true,
                (Some("skip"), None) => attributes.skip = true,
                (Some("prelude"), None) => attributes.prelude = Prelude::Std,
                (Some("test-prelude"), None) => attributes.prelude = Prelude::Test,
                (Some("fuel"), Some(n)) => {
                    attributes.fuel = n.replace('_', "").parse().map_err(|_| {
                        format!("`{}`: expected a number after `:fuel`", case.title)
                    })?;
                }
                _ => {
                    return Err(format!(
                        "`{}`: unknown attribute `:{attribute}`; the attributes are `:todo`, \
                         `:skip`, `:fuel N`, `:prelude` and `:test-prelude`",
                        case.title
                    ));
                }
            }
        }
        Ok(attributes)
    }
}

/// A database with the files of a case and `std`, with the prelude the
/// case asks for, and the program's file.
fn database(case: &Case, prelude: Prelude) -> Result<(TestDb, File), String> {
    let mut db = TestDb::new();
    if prelude == Prelude::Test {
        db.add_test_std();
    } else {
        db.add_std();
    }
    db.set_prelude((prelude != Prelude::None).then_some("std"), prelude != Prelude::None);
    let files: Vec<_> = split_files(&case.input)?
        .into_iter()
        .map(|(path, text)| (path.clone(), db.file(&path, &text)))
        .collect();
    let main = files.iter().find(|(path, _)| path == "/main.bolt").map(|(_, file)| *file);
    let main = main.ok_or("the program needs a file `/main.bolt`")?;
    let errors = files.iter().flat_map(|(path, file)| {
        diagnostics(&db, *file)
            .into_iter()
            .filter(|d| d.severity == Severity::Error)
            .map(move |d| format!("{path}: {} at {:?}", d.message, d.range))
    });
    // Errors in the prelude, which a case doesn't check itself, with the
    // first of each file (#249).
    let case_files: Vec<File> = files.iter().map(|(_, file)| *file).collect();
    let broken = dependency_diagnostics(&db, main).into_iter().filter_map(|d| {
        let label = d.labels.first()?;
        (!case_files.contains(&label.file)).then(|| format!("{}: {}", d.message, label.message))
    });
    let errors: Vec<String> = errors.chain(broken).collect();
    if !errors.is_empty() {
        return Err(format!("the program has errors:\n{}", errors.join("\n")));
    }
    Ok((db, main))
}

/// What running a case prints, as in the corpus.
fn output(case: &Case, attributes: &Attributes) -> Result<String, String> {
    let (db, file) = database(case, attributes.prelude)?;
    catch("the lowering or the run", || {
        let program = match lower(&db, file) {
            Ok(program) => program,
            Err(errors) => {
                let lines: Vec<String> = errors.iter().map(|e| format!("error {e}")).collect();
                return Ok(lines.join("\n"));
            }
        };
        let (mut out, result) = run_captured(&program, program.entry, Some(attributes.fuel));
        if let Err(error) = result {
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&format!("error: {error}"));
        }
        Ok(out.trim().to_string())
    })
}

fn test(case: &Case, attributes: &Attributes) -> Result<(), Failed> {
    if case.sections.is_empty() {
        return Err("the case has no output yet: run with `UPDATE_EXPECT=1` to write it".into());
    }
    let found = output(case, attributes)?;
    let expected = case.section(0);
    if found != expected {
        return Err(format!("expected:\n{expected}\nfound:\n{found}").into());
    }
    Ok(())
}

/// What an update writes for a case, or `None` to keep it, like a case
/// that's `:todo` or `:skip`, or that can't run.
fn updated(case: &Case) -> Option<Vec<String>> {
    let attributes = Attributes::parse(case).ok()?;
    if attributes.todo || attributes.skip {
        return None;
    }
    Some(vec![output(case, &attributes).ok()?])
}

fn main() {
    let args = Arguments::from_args();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/run");
    let mut trials = Vec::new();
    for path in data_files(&root) {
        let stem = data_name(&root, &path);
        let mut text = std::fs::read_to_string(&path).unwrap();
        if updating()
            && let Ok(cases) = parse_cases(&text, 1)
        {
            let new = update(&text, &cases, updated);
            if new != text {
                std::fs::write(&path, &new).unwrap();
                text = new;
            }
        }
        let fail = |name: String, message: String| Trial::test(name, move || Err(message.into()));
        let cases = match parse_cases(&text, 1) {
            Ok(cases) => cases,
            Err(error) => {
                trials.push(fail(stem, format!("{}: {error}", path.display())));
                continue;
            }
        };
        let mut seen = Vec::new();
        for case in cases {
            let name = format!("{stem}::{}", slug(&case.title));
            if seen.contains(&name) {
                trials.push(fail(
                    format!("{name}_again"),
                    format!("{}: two cases are called `{}`", path.display(), case.title),
                ));
                continue;
            }
            seen.push(name.clone());
            let attributes = match Attributes::parse(&case) {
                Ok(attributes) => attributes,
                Err(error) => {
                    trials.push(fail(name, format!("{}: {error}", path.display())));
                    continue;
                }
            };
            let trial = if attributes.skip {
                // Never run, even with `--ignored`, since it may hang.
                Trial::test(name, || Ok(())).with_kind("skip").with_ignored_flag(true)
            } else {
                let todo = attributes.todo;
                let case = Arc::new((case, attributes));
                let trial = Trial::test(name, move || test(&case.0, &case.1));
                if todo { trial.with_kind("todo").with_ignored_flag(true) } else { trial }
            };
            trials.push(trial);
        }
    }
    libtest_mimic::run(&args, trials).exit();
}

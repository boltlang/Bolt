//! Tests that run the programs in `tests/run` at the root of the
//! repository twice: with the evaluator, like `bolt run`, and compiled to
//! WebAssembly, like `bolt run --wasm`. Both must give the output of the
//! case. The files have the format of `boltlang-corpus`:
//!
//! ```text
//! ================================================================================
//! Factorial
//! ================================================================================
//! fn fac(n: i32) -> i32 = if n == 0 { 1 } else { n * fac(n - 1) };
//! print("hello");
//! fac(5);
//! --------------------------------------------------------------------------------
//! hello
//! 120
//! ```
//!
//! The output is what `bolt run` prints: what the program printed, then the
//! value of its last statement when that isn't `()`. When the program
//! fails, the last line is `error: ` and the message, like
//! `error: panicked: negative`; for compiled code, that's also the
//! message of a code generation error, like a feature that isn't supported
//! yet.
//!
//! The programs get the prelude of `packages/std`, and must have no type
//! errors, nor must the prelude. A case can have several files, each starting with a line like
//! `//- /lib.bolt`; the program is `/main.bolt`.
//!
//! Each case is two tests, like `functions::factorial::eval` and
//! `functions::factorial::wasm`. Run the cases of one file with
//! `cargo test --test run functions::`. To write the evaluator's output
//! into the files instead of comparing it, run with `UPDATE_EXPECT=1`, and
//! review the changes with `git diff`.
//!
//! Lines starting with `:` after the title are attributes:
//!
//! - `:todo wasm`: the compiled code doesn't give this output yet, for
//!   example because it uses a feature that isn't compiled. Its test is
//!   ignored; run `cargo test --test run -- --ignored` to see how far along
//!   the code generator is. `:todo eval` is the same for the evaluator, and
//!   `:todo` is both.
//! - `:skip wasm`, `:skip eval` and `:skip`: never run, for programs that
//!   crash or hang.
//! - `:fuel N`: stop after about `N` steps, instead of ten million. The
//!   evaluator counts expressions and compiled code counts instructions,
//!   so only use it for programs that run out of fuel either way.
//!
//! An update writes the evaluator's output, or the compiled code's when the
//! evaluator is `:todo` or `:skip`, and leaves a case alone when both are.

use std::path::Path;

use boltlang_base_db::{File, TestDb};
use boltlang_codegen::{link, run};
use boltlang_corpus::{Case, catch, data_files, data_name, parse_cases, slug, split_files, update, updating};
use boltlang_eval::{Evaluator, Limits, Recorder};
use boltlang_ide_diagnostics::{Severity, dependency_diagnostics, diagnostics};
use libtest_mimic::{Arguments, Failed, Trial};

/// The two ways of running a program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Backend {
    Eval,
    Wasm,
}

impl Backend {
    const ALL: [Backend; 2] = [Backend::Eval, Backend::Wasm];

    fn name(self) -> &'static str {
        match self {
            Backend::Eval => "eval",
            Backend::Wasm => "wasm",
        }
    }
}

/// What a case's attributes say about running it with one backend.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Status {
    #[default]
    Run,
    Todo,
    Skip,
}

struct Attributes {
    eval: Status,
    wasm: Status,
    fuel: u64,
}

impl Attributes {
    fn parse(case: &Case) -> Result<Self, String> {
        let mut attributes = Attributes { eval: Status::Run, wasm: Status::Run, fuel: 10_000_000 };
        for attribute in &case.attributes {
            let mut words = attribute.split_whitespace();
            let status = match words.next() {
                Some("todo") => Status::Todo,
                Some("skip") => Status::Skip,
                Some("fuel") => {
                    attributes.fuel = words
                        .next()
                        .and_then(|n| n.replace('_', "").parse().ok())
                        .ok_or_else(|| format!("`{}`: expected a number after `:fuel`", case.title))?;
                    continue;
                }
                _ => {
                    return Err(format!(
                        "`{}`: unknown attribute `:{attribute}`; the attributes are `:todo`, `:skip` and `:fuel`, \
                         the first two optionally followed by `eval` or `wasm`",
                        case.title
                    ));
                }
            };
            let backends = match words.next() {
                None => &Backend::ALL[..],
                Some("eval") => &[Backend::Eval],
                Some("wasm") => &[Backend::Wasm],
                Some(other) => return Err(format!("`{}`: unknown backend `{other}`; use `eval` or `wasm`", case.title)),
            };
            for backend in backends {
                *attributes.status_mut(*backend) = status;
            }
        }
        Ok(attributes)
    }

    fn status(&self, backend: Backend) -> Status {
        match backend {
            Backend::Eval => self.eval,
            Backend::Wasm => self.wasm,
        }
    }

    fn status_mut(&mut self, backend: Backend) -> &mut Status {
        match backend {
            Backend::Eval => &mut self.eval,
            Backend::Wasm => &mut self.wasm,
        }
    }
}

/// A database with the files of a case and the prelude of `std`, and the
/// program's file.
fn database(case: &Case) -> Result<(TestDb, File), String> {
    let mut db = TestDb::new();
    db.add_std();
    db.set_prelude(Some("std"), true);
    let files: Vec<_> =
        split_files(&case.input)?.into_iter().map(|(path, text)| (path.clone(), db.file(&path, &text))).collect();
    let main = files.iter().find(|(path, _)| path == "/main.bolt").map(|(_, file)| *file);
    let main = main.ok_or("the program needs a file `/main.bolt`")?;
    let errors: Vec<_> = files
        .iter()
        .flat_map(|&(ref path, file)| {
            diagnostics(&db, file)
                .into_iter()
                .filter(|d| d.severity == Severity::Error)
                .map(move |d| format!("{path}: {} at {:?}", d.message, d.range))
        })
        .collect();
    // Errors in the prelude, which a case doesn't check itself, with the
    // first of each file (#249).
    let case_files: Vec<File> = files.iter().map(|&(_, file)| file).collect();
    let broken = dependency_diagnostics(&db, main).into_iter().filter_map(|d| {
        let label = d.labels.first()?;
        (!case_files.contains(&label.file)).then(|| format!("{}: {}", d.message, label.message))
    });
    let errors: Vec<String> = errors.into_iter().chain(broken).collect();
    if !errors.is_empty() {
        return Err(format!("the program has errors:\n{}", errors.join("\n")));
    }
    Ok((db, main))
}

/// What running a case with `backend` prints, as in the corpus.
fn output(case: &Case, attributes: &Attributes, backend: Backend) -> Result<String, String> {
    let (db, file) = database(case)?;
    let printed = Recorder::default();
    let result = match backend {
        Backend::Eval => {
            let limits = Limits { fuel: Some(attributes.fuel), ..Limits::default() };
            let mut evaluator = Evaluator::with_limits(&db, limits).with_mutability_checks().with_host(printed.clone());
            catch("the evaluator", || Ok(evaluator.eval_file(file).map_err(|e| vec![e.kind.to_string()])))?
        }
        Backend::Wasm => catch("the code generator", || {
            Ok(match link(&db, file) {
                Ok(bytes) => run(&db, file, &bytes, printed.clone(), Some(attributes.fuel)).map_err(|e| vec![e.to_string()]),
                Err(errors) => Err(errors.iter().map(|e| e.to_string()).collect()),
            })
        })?,
    };
    let mut out = printed.output();
    match result {
        Ok(value) if !value.is_unit() => out.push_str(&format!("{}\n", value.display(&db))),
        Ok(_) => {}
        Err(errors) => {
            for error in errors {
                out.push_str(&format!("error: {error}\n"));
            }
        }
    }
    Ok(out.trim().to_string())
}

fn test(case: &Case, attributes: &Attributes, backend: Backend) -> Result<(), Failed> {
    if case.sections.is_empty() {
        return Err("the case has no output yet: run with `UPDATE_EXPECT=1` to write it".into());
    }
    let found = output(case, attributes, backend)?;
    let expected = case.section(0);
    if found != expected {
        return Err(format!("expected:\n{expected}\nfound:\n{found}").into());
    }
    Ok(())
}

/// What an update writes for a case: the output of the first backend that
/// runs it, or `None` to keep the case. A case that can't run keeps its
/// output, and fails when it's tested.
fn updated(case: &Case) -> Option<Vec<String>> {
    let attributes = Attributes::parse(case).ok()?;
    let backend = Backend::ALL.into_iter().find(|b| attributes.status(*b) == Status::Run)?;
    Some(vec![output(case, &attributes, backend).ok()?])
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
                trials.push(fail(format!("{name}_again"), format!("{}: two cases are called `{}`", path.display(), case.title)));
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
            let case = std::sync::Arc::new((case, attributes));
            for backend in Backend::ALL {
                let name = format!("{name}::{}", backend.name());
                let trial = match case.1.status(backend) {
                    // Never run, even with `--ignored`, since it may hang.
                    Status::Skip => Trial::test(name, || Ok(())).with_kind("skip").with_ignored_flag(true),
                    status => {
                        let case = case.clone();
                        let trial = Trial::test(name, move || test(&case.0, &case.1, backend));
                        if status == Status::Todo { trial.with_kind("todo").with_ignored_flag(true) } else { trial }
                    }
                };
                trials.push(trial);
            }
        }
    }
    libtest_mimic::run(&args, trials).exit();
}

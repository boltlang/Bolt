//! Tests of the diagnostics as users see them, driven by the `.txt` files in
//! `tests/diagnostics` at the root of the repository, in the format of the
//! tree-sitter corpus. Each case is a separate test, named after its file
//! and title, e.g. `mutability::closure_mutability`.
//!
//! A case is a title between two lines of `=`, then the code, with a
//! comment under each line that has a diagnostic:
//!
//! ```text
//! ================================================================================
//! Mismatches point at annotations
//! ================================================================================
//! let x: i32 = True;
//!    //^^^ label: expected because of this annotation
//!             //^^^^ error: expected `i32`, found `Bool`
//! ```
//!
//! The carets mark columns of the closest line above that isn't such a
//! comment. A comment says `error:`, `warning:` or `label:`, for the labels
//! of diagnostics, then the message. A diagnostic at an empty range, like a
//! missing token, is marked by one caret. The case passes when its comments
//! are exactly the diagnostics and labels of its files, so a case without
//! them has none. There's no output after a line of `-`, and no
//! `UPDATE_EXPECT=1`: the comments are written by hand.
//!
//! A case can have several files, each starting with a line like
//! `//- /lib.bolt`, so that they can import each other, or be the
//! `bolt.toml` of a package. Without such lines, the code is `/main.bolt`.
//! The labels of a diagnostic are marked in the file they're in. Files get
//! no prelude, unless the case has the attribute `:prelude`, for the
//! prelude of the repository's `std`, or `:test-prelude`, for the small one
//! of `tests/std`, which has the operators but doesn't change when `std`
//! does. A case can give its own `std` instead, with files in
//! `/toolchain/std`, like `/toolchain/std/__prelude__/lib.bolt`.
//!
//! Lines starting with `:` after the title are attributes:
//!
//! - `:prelude` and `:test-prelude`: the prelude of the files, as above.
//! - `:skip`: never run the case, and report it as ignored. For a case that
//!   crashes or hangs.
//! - `:todo`: the comments are what the compiler should report, but doesn't
//!   yet. The case is ignored, unless run with `--ignored`.
//!
//! Run a single case with `cargo test --test diagnostics closure_mutability`.

use std::path::Path;

use boltlang_base_db::{Settings, SystemPathBuf, TestDb, source_text};
use boltlang_corpus::{Case, catch, data_files, data_name, parse_cases, slug, split_files};
use boltlang_ide_diagnostics::diagnostics;
use boltlang_syntax::rowan::{TextRange, TextSize};
use libtest_mimic::{Arguments, Failed, Trial};

/// The attributes of a case, like `:todo`.
#[derive(Default)]
struct Attributes {
    skip: bool,
    todo: bool,
    prelude: Prelude,
}

/// The prelude of a case.
#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum Prelude {
    #[default]
    None,
    /// That of the repository's `std`, or of the case's own, with
    /// `:prelude`.
    Std,
    /// The small one of `tests/std`, with `:test-prelude`.
    Test,
}

impl Attributes {
    fn parse(case: &Case) -> Result<Self, String> {
        let mut attributes = Attributes::default();
        for attribute in &case.attributes {
            match attribute.as_str() {
                "skip" => attributes.skip = true,
                "todo" => attributes.todo = true,
                "prelude" => attributes.prelude = Prelude::Std,
                "test-prelude" => attributes.prelude = Prelude::Test,
                other => {
                    return Err(format!(
                        "`{}`: unknown attribute `:{other}`; the attributes are `:skip`, `:todo`, \
                         `:prelude` and `:test-prelude`",
                        case.title
                    ));
                }
            }
        }
        Ok(attributes)
    }
}

/// The diagnostics of the files of a case, and those its comments expect,
/// one per line, sorted.
fn check(input: &str, prelude: Prelude) -> Result<(String, String), String> {
    let files = split_files(input)?;
    let mut db = TestDb::new();
    let own_std = files.iter().any(|(path, _)| path.starts_with(&format!("{}/", TestDb::STD_DIR)));
    if own_std {
        Settings::change_std_dir(&mut db, Some(SystemPathBuf::from(TestDb::STD_DIR)));
    }
    match prelude {
        Prelude::None => {}
        Prelude::Std if own_std => db.set_prelude(Some("std"), true),
        Prelude::Std => {
            db.add_std();
            db.set_prelude(Some("std"), true);
        }
        Prelude::Test => {
            db.add_test_std();
            db.set_prelude(Some("std"), true);
        }
    }
    let files: Vec<_> =
        files.iter().map(|(path, text)| (path.as_str(), db.file(path, text))).collect();
    // Only Bolt files have diagnostics: a `bolt.toml` is read by the others.
    let bolt_files: Vec<_> = files.iter().filter(|(path, _)| path.ends_with(".bolt")).collect();
    let all: Vec<_> = bolt_files.iter().flat_map(|&&(_, file)| diagnostics(&db, file)).collect();
    let mut expected = Vec::new();
    let mut actual = Vec::new();
    for &&(path, file) in &bolt_files {
        let text = source_text(&db, file);
        expected.extend(
            annotations(text).into_iter().map(|(range, message)| show(path, text, range, &message)),
        );
        for d in &all {
            if d.file == file {
                actual.push(show(path, text, d.range, &format!("{}: {}", d.severity, d.message)));
            }
            for label in d.labels.iter().filter(|l| l.file == file) {
                actual.push(show(path, text, label.range, &format!("label: {}", label.message)));
            }
        }
    }
    expected.sort();
    actual.sort();
    Ok((expected.join("\n"), actual.join("\n")))
}

/// The comments with carets in `text`, with the range each marks.
fn annotations(text: &str) -> Vec<(TextRange, String)> {
    let mut out = Vec::new();
    let mut target = None;
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let is_annotation = line
            .trim_start()
            .strip_prefix("//")
            .is_some_and(|rest| rest.trim_start().starts_with('^'));
        match target {
            Some(start) if is_annotation => {
                let column = line.find('^').unwrap();
                let len = line[column..].chars().take_while(|&c| c == '^').count();
                let range = TextRange::at(
                    TextSize::from((start + column) as u32),
                    TextSize::from(len as u32),
                );
                out.push((range, line[column + len..].trim().to_string()));
            }
            _ => target = Some(offset),
        }
        offset += line.len();
    }
    out
}

/// A diagnostic as `path:line:column-column message`, to compare them.
fn show(path: &str, text: &str, range: TextRange, message: &str) -> String {
    let range = if range.is_empty() { TextRange::at(range.start(), 1.into()) } else { range };
    let start = usize::from(range.start());
    let line_start = text[..start].rfind('\n').map_or(0, |i| i + 1);
    let line = text[..start].matches('\n').count() + 1;
    let column = start - line_start;
    format!("{path}:{line}:{column}-{} {message}", column + usize::from(range.len()))
}

fn run(case: &Case, attributes: &Attributes) -> Result<(), Failed> {
    let (expected, actual) = catch("the compiler", || check(&case.input, attributes.prelude))?;
    if expected != actual {
        return Err(format!("expected:\n{expected}\nfound:\n{actual}").into());
    }
    Ok(())
}

fn main() {
    let args = Arguments::from_args();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/diagnostics");
    let mut trials = Vec::new();
    for path in data_files(&root) {
        let stem = data_name(&root, &path);
        let text = std::fs::read_to_string(&path).unwrap();
        let cases = match parse_cases(&text, 0) {
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
            let attributes = match Attributes::parse(&case) {
                Ok(attributes) => attributes,
                Err(error) => {
                    let message = format!("{}: {error}", path.display());
                    trials.push(Trial::test(name, move || Err(message.into())));
                    continue;
                }
            };
            let trial = if attributes.skip {
                // Never run, even with `--ignored`, since it may hang.
                Trial::test(name, || Ok(())).with_kind("skip").with_ignored_flag(true)
            } else if attributes.todo {
                Trial::test(name, move || run(&case, &attributes))
                    .with_kind("todo")
                    .with_ignored_flag(true)
            } else {
                Trial::test(name, move || run(&case, &attributes))
            };
            trials.push(trial);
        }
    }
    libtest_mimic::run(&args, trials).exit();
}

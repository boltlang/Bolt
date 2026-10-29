//! The unit tests of `std`: the functions with `#[test]` in the `.bolt`
//! files of `packages/std`, each a test, like `vec::pushing_and_popping`,
//! run by the evaluator as `bolt test` runs them.
//!
//! Run the tests of one file with `cargo test --test std vec::`. A test
//! fails when its file has errors, or when it stops with an error, like a
//! panic.

use std::{
    path::{Path, PathBuf},
    process::ExitCode,
};

use boltlang_base_db::{Db, File, SystemPath, TestDb};
use boltlang_eval::{Evaluator, Limits};
use boltlang_hir_def::nameres::test_functions;
use boltlang_ide_diagnostics::{Severity, diagnostics};
use libtest_mimic::{Arguments, Failed, Trial};

/// The steps a test may take before it's stopped, as in the run tests.
const FUEL: u64 = 10_000_000;

fn main() -> ExitCode {
    let args = Arguments::from_args();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/std");
    let mut paths = Vec::new();
    bolt_files(&root, &mut paths);
    let mut db = TestDb::new();
    db.add_std();
    let mut trials = Vec::new();
    for path in paths {
        let relative = path.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/");
        let file = std_file(&db, &relative);
        for f in test_functions(&db, file) {
            let name = f.item(&db).display_name().unwrap().to_string();
            let (relative, test) = (relative.clone(), name.clone());
            trials.push(Trial::test(format!("{}::{name}", module(&relative)), move || run(&relative, &test)));
        }
    }
    libtest_mimic::run(&args, trials).exit_code()
}

/// Run the test `name` of the file `relative` of `std`, with a database of
/// its own, since tests run on threads of their own.
fn run(relative: &str, name: &str) -> Result<(), Failed> {
    let mut db = TestDb::new();
    db.add_std();
    let file = std_file(&db, relative);
    let errors: Vec<String> = diagnostics(&db, file)
        .into_iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| format!("{relative}: {} at {:?}", d.message, d.range))
        .collect();
    if !errors.is_empty() {
        return Err(format!("the file has errors:\n{}", errors.join("\n")).into());
    }
    let f = test_functions(&db, file)
        .iter()
        .copied()
        .find(|f| f.item(&db).display_name().is_some_and(|n| n.as_str() == name))
        .ok_or_else(|| format!("there's no test `{name}` any more"))?;
    let limits = Limits { fuel: Some(FUEL), ..Limits::default() };
    let mut evaluator = Evaluator::with_limits(&db, limits);
    evaluator.call_function(f, Vec::new()).map(|_| ()).map_err(|error| error.to_string().into())
}

/// The file `relative` of the `std` of `db`.
fn std_file(db: &TestDb, relative: &str) -> File {
    let path = format!("{}/{relative}", TestDb::STD_DIR);
    db.files().resolve(db, SystemPath::new(&path))
}

/// The name of a file of `std` in test names: its path without `.bolt`,
/// with `::` between directories, and without `__prelude__`, whose files
/// are most of `std`: `__prelude__/vec.bolt` is `vec`.
fn module(relative: &str) -> String {
    let relative = relative.strip_prefix("__prelude__/").unwrap_or(relative);
    relative.strip_suffix(".bolt").unwrap_or(relative).replace('/', "::")
}

/// The `.bolt` files in `dir` and the directories in it, in order.
fn bolt_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("can't read {}: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for entry in entries {
        if entry.is_dir() {
            bolt_files(&entry, out);
        } else if entry.extension().is_some_and(|e| e == "bolt") {
            out.push(entry);
        }
    }
}

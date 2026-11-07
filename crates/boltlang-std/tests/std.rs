//! The unit tests of `std`: the functions with `#[test]` in the `.bolt`
//! files of `packages/std`, each run twice, as `bolt test` and `bolt test
//! --wasm` run them: `eval::vec::pushing_and_popping` with Conduit's
//! evaluator, and `wasm::vec::pushing_and_popping` compiled to Wasm and run
//! with Node. Without a `node`, the `wasm::` tests are ignored.
//!
//! Run the tests of one file with `cargo test --test std vec::`, or only
//! those of the evaluator with `eval::`. A test fails when its file has
//! errors, can't be compiled, or stops with an error, like a panic.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::ExitCode,
    sync::{Arc, Mutex, OnceLock},
};

use boltlang_base_db::{Db, File, SystemPath, TestDb};
use boltlang_codegen_conduit::{Lowered, lower, node::NodeHost, run::run, wasm};
use boltlang_hir_def::nameres::test_functions;
use boltlang_ide_diagnostics::{Severity, dependency_diagnostics, diagnostics};
use libtest_mimic::{Arguments, Failed, Trial};

/// The steps a test may take with the evaluator before it's stopped.
const FUEL: u64 = 100_000_000;

/// The module of each file, compiled once for all its tests, by its path.
type Modules = Mutex<HashMap<String, Arc<OnceLock<Result<PathBuf, String>>>>>;

fn main() -> ExitCode {
    let args = Arguments::from_args();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/std");
    let mut paths = Vec::new();
    bolt_files(&root, &mut paths);
    let db = std_db();
    let node = if boltlang_codegen_conduit::node::available() {
        Some(Arc::new(NodeHost::new().expect("a directory for the Wasm modules")))
    } else {
        None
    };
    let modules: Arc<Modules> = Arc::default();
    let mut trials = Vec::new();
    for path in paths {
        let relative = path.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/");
        let file = std_file(&db, &relative);
        for f in test_functions(&db, file) {
            let name = f.item(&db).display_name().unwrap().to_string();
            let test = format!("{}::{name}", module(&relative));
            let (r, n) = (relative.clone(), name.clone());
            trials.push(Trial::test(format!("eval::{test}"), move || run_eval(&r, &n)));
            let (r, n, node, modules) = (relative.clone(), name, node.clone(), modules.clone());
            let ignored = node.is_none();
            let trial = Trial::test(format!("wasm::{test}"), move || match &node {
                Some(node) => run_wasm(node, &modules, &r, &n),
                None => Ok(()),
            });
            trials.push(trial.with_ignored_flag(ignored));
        }
    }
    libtest_mimic::run(&args, trials).exit_code()
}

/// A database with `std`. Each test has one of its own, since tests run on
/// threads of their own.
fn std_db() -> TestDb {
    let mut db = TestDb::new();
    db.add_std();
    db
}

/// The file `relative` of `std`, checked and lowered, with `db` it's in.
fn lowered(db: &TestDb, relative: &str) -> Result<Lowered, Failed> {
    let file = std_file(db, relative);
    let mut found = diagnostics(db, file);
    found.extend(dependency_diagnostics(db, file));
    let errors: Vec<String> = found
        .into_iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| d.message.to_string())
        .collect();
    if !errors.is_empty() {
        return Err(format!("{relative} has errors:\n{}", errors.join("\n")).into());
    }
    lower(db, file).map_err(|errors| {
        let errors: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
        format!("{relative} can't be compiled:\n{}", errors.join("\n")).into()
    })
}

/// Run the test `name` of the file `relative` with the evaluator.
fn run_eval(relative: &str, name: &str) -> Result<(), Failed> {
    let db = std_db();
    let program = lowered(&db, relative)?;
    let &(_, f) = program
        .tests
        .iter()
        .find(|(n, _)| n == name)
        .ok_or_else(|| format!("there's no test `{name}` any more"))?;
    run(&program, f, Some(FUEL)).map_err(|e| e.to_string().into())
}

/// Run the test `name` of the file `relative` in Wasm, in the module of
/// its file, which exports all its tests.
fn run_wasm(node: &NodeHost, modules: &Modules, relative: &str, name: &str) -> Result<(), Failed> {
    let cell = modules.lock().unwrap().entry(relative.to_string()).or_default().clone();
    let module = cell.get_or_init(|| compile(node, relative)).clone()?;
    match node.run_captured(&module, name).map_err(|e| e.to_string())? {
        (Ok(()), _) => Ok(()),
        (Err(why), output) => Err(format!("{why}\n{output}").into()),
    }
}

/// The Wasm module of the file `relative`, written to the host's directory.
fn compile(node: &NodeHost, relative: &str) -> Result<PathBuf, String> {
    let db = std_db();
    let program =
        lowered(&db, relative).map_err(|e| e.message().unwrap_or_default().to_string())?;
    let exports: Vec<_> = program.tests.iter().map(|(n, f)| (*f, n.as_str())).collect();
    let bytes = wasm::build_exporting(&program, true, &exports)
        .map_err(|e| format!("{relative} can't be compiled to Wasm: {e}"))?;
    let path = node.dir.join(format!("{}.wasm", module(relative).replace("::", "-")));
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(path)
}

/// The file `relative` of the `std` of `db`.
fn std_file(db: &TestDb, relative: &str) -> File {
    let path = format!("{}/{relative}", TestDb::STD_DIR);
    db.files().resolve(db, SystemPath::new(&path))
}

/// The name of a file of `std` in test names: its path without `.bolt` and
/// without `test_`, with `::` between directories: `test_vec.bolt` is
/// `vec`.
fn module(relative: &str) -> String {
    let relative = relative.strip_suffix(".bolt").unwrap_or(relative);
    let (dir, name) = relative.rsplit_once('/').unwrap_or(("", relative));
    let name = name.strip_prefix("test_").unwrap_or(name);
    if dir.is_empty() { name.to_string() } else { format!("{}::{name}", dir.replace('/', "::")) }
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

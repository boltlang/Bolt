//! The unit tests of `std`: the functions with `#[test]` in the `.bolt`
//! files of `packages/std`, each run three times:
//! `eval::vec::pushing_and_popping` with Conduit's evaluator, as `bolt test`
//! runs it, `wasm::vec::pushing_and_popping` compiled to Wasm and run with
//! `runtime/node.mjs`, as `bolt test --wasm` runs it, and
//! `browser::vec::pushing_and_popping` with `runtime/browser.mjs`, in Node.
//! Without a `node`, the `wasm::` and `browser::` tests are ignored.
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

use boltlang_base_db::{Db, File, Settings, SystemPath, Target, TestDb};
use boltlang_codegen_conduit::{
    Lowered, lower,
    node::{NodeHost, Runtime},
    run::{run, run_captured, should_panic_outcome},
    wasm,
};
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
    // Each mode has the tests of its target, which `#[cfg]` can pick.
    let emu = std_db(Target::default());
    let wasm = std_db(Target::WASM);
    // Each test's name, and what its `#[should_panic]` expects, if it has
    // one.
    let names = |db: &TestDb, relative: &str| -> Vec<(String, Option<Option<String>>)> {
        let file = std_file(db, relative);
        test_functions(db, file)
            .iter()
            .map(|f| {
                let item = f.item(db);
                let expected = item.should_panic.as_ref().map(|s| s.expected.clone());
                (item.display_name().unwrap().to_string(), expected)
            })
            .collect()
    };
    let node = if boltlang_codegen_conduit::node::available() {
        Some(Arc::new(NodeHost::new().expect("a directory for the Wasm modules")))
    } else {
        None
    };
    let modules: Arc<Modules> = Arc::default();
    let mut trials = Vec::new();
    for path in paths {
        let relative = path.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/");
        for (name, expected) in names(&emu, &relative) {
            let test = format!("eval::{}::{name}", module(&relative));
            let r = relative.clone();
            trials.push(Trial::test(test, move || run_eval(&r, &name, expected.as_ref())));
        }
        for (name, expected) in names(&wasm, &relative) {
            for (prefix, runtime) in [("wasm", Runtime::Node), ("browser", Runtime::Browser)] {
                let test = format!("{prefix}::{}::{name}", module(&relative));
                let (r, n, e) = (relative.clone(), name.clone(), expected.clone());
                let (node, modules) = (node.clone(), modules.clone());
                let ignored = node.is_none();
                let trial = Trial::test(test, move || match &node {
                    Some(node) => run_wasm(node, runtime, &modules, &r, &n, e.as_ref()),
                    None => Ok(()),
                });
                trials.push(trial.with_ignored_flag(ignored));
            }
        }
    }
    libtest_mimic::run(&args, trials).exit_code()
}

/// A database with `std`, compiling for `target`. Each test has one of its
/// own, since tests run on threads of their own.
fn std_db(target: Target) -> TestDb {
    let mut db = TestDb::new();
    db.add_std();
    Settings::change_target(&mut db, target);
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

/// Run the test `name` of the file `relative` with the evaluator. With
/// `should_panic`, it must stop with an error, which has the text it
/// expects, if any.
fn run_eval(
    relative: &str,
    name: &str,
    should_panic: Option<&Option<String>>,
) -> Result<(), Failed> {
    let db = std_db(Target::default());
    let program = lowered(&db, relative)?;
    let &(_, f) = program
        .tests
        .iter()
        .find(|(n, _)| n == name)
        .ok_or_else(|| format!("there's no test `{name}` any more"))?;
    match should_panic {
        Some(expected) => {
            let (output, result) = run_captured(&program, f, Some(FUEL));
            let result = result.map_err(|e| e.to_string());
            should_panic_outcome(result, &output, expected.as_deref()).map_err(Failed::from)
        }
        None => run(&program, f, Some(FUEL)).map_err(|e| e.to_string().into()),
    }
}

/// Run the test `name` of the file `relative` in Wasm, with `runtime`, in
/// the module of its file, which exports all its tests.
fn run_wasm(
    node: &NodeHost,
    runtime: Runtime,
    modules: &Modules,
    relative: &str,
    name: &str,
    should_panic: Option<&Option<String>>,
) -> Result<(), Failed> {
    let cell = modules.lock().unwrap().entry(relative.to_string()).or_default().clone();
    let module = cell.get_or_init(|| compile(node, relative)).clone()?;
    match (node.run_captured(runtime, &module, name).map_err(|e| e.to_string())?, should_panic) {
        ((result, output), Some(expected)) => {
            should_panic_outcome(result, &output, expected.as_deref()).map_err(Failed::from)
        }
        ((Ok(()), _), None) => Ok(()),
        ((Err(why), output), None) => Err(format!("{why}\n{output}").into()),
    }
}

/// The Wasm module of the file `relative`, written to the host's directory.
fn compile(node: &NodeHost, relative: &str) -> Result<PathBuf, String> {
    let db = std_db(Target::WASM);
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

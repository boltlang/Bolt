//! Snapshot tests driven by the `.bolt` files in `tests/data`. Each file is a
//! separate test, e.g. `ok::let_mut`.
//!
//! - `ok/`: must parse without diagnostics or `ERROR` nodes.
//! - `err/`: must produce at least one diagnostic; the snapshot pins down
//!   how the parser recovers.
//! - `todo/ok/`, `todo/err/`: cases the parser can't handle correctly yet.
//!   They show up as ignored; run them with `-- --ignored`. They aren't
//!   snapshotted. Move a file into `ok/` or `err/` once it works.
//! - `corpus/ok/`, `corpus/err/`: the tree-sitter-bolt test corpus, imported
//!   with `scripts/import_corpus.py`. They're checked like `ok/` and `err/`,
//!   except that the files listed in `corpus/FAILING` show up as ignored.
//!   Remove a file from that list once it parses.
//!
//! Add a test by adding a file, then run `cargo insta test --review`.
//! Run a single file with `cargo test --test data let_mut`.

// Lets us capture what a test prints and only show it when the test fails,
// like the default test harness does. This is an internal std feature, so it
// may break on a future nightly; if it does, drop `run_captured` and set
// `INSTA_OUTPUT=minimal` instead.
#![feature(internal_output_capture)]

mod common;

use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use common::{dump, has_error_node, parse_file_with_timeout};
use libtest_mimic::{Arguments, Failed, Trial};

#[derive(Clone, Copy)]
enum Expect {
    Ok,
    Err,
}

struct Group {
    dir: &'static str,
    expect: Expect,
    /// Snapshot name prefix, or `None` to skip snapshotting.
    snapshot: Option<&'static str>,
    ignored: bool,
    /// Whether files listed in `corpus/FAILING` are ignored.
    known_failures: bool,
}

const GROUPS: &[Group] = &[
    Group { dir: "ok", expect: Expect::Ok, snapshot: Some("ok"), ignored: false, known_failures: false },
    Group { dir: "err", expect: Expect::Err, snapshot: Some("err"), ignored: false, known_failures: false },
    Group { dir: "todo/ok", expect: Expect::Ok, snapshot: None, ignored: true, known_failures: false },
    Group { dir: "todo/err", expect: Expect::Err, snapshot: None, ignored: true, known_failures: false },
    Group { dir: "corpus/ok", expect: Expect::Ok, snapshot: Some("corpus_ok"), ignored: false, known_failures: true },
    Group { dir: "corpus/err", expect: Expect::Err, snapshot: Some("corpus_err"), ignored: false, known_failures: true },
];

/// Paths relative to `tests/data/corpus`, e.g. `ok/expressions-identifiers.bolt`.
fn known_failures(root: &Path) -> Vec<String> {
    let list = std::fs::read_to_string(root.join("corpus/FAILING")).unwrap_or_default();
    list.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect()
}

fn bolt_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<_> = entries
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "bolt"))
        .collect();
    files.sort();
    files
}

fn check(path: &Path, expect: Expect, snapshot: Option<&str>) -> Result<(), Failed> {
    let (node, diags) = parse_file_with_timeout(path)?;
    match expect {
        Expect::Ok if !diags.is_empty() => {
            return Err(format!("unexpected diagnostics\n{}", dump(&node, &diags)).into());
        }
        Expect::Ok if has_error_node(&node) => {
            return Err(format!("tree contains ERROR nodes\n{}", dump(&node, &diags)).into());
        }
        Expect::Err if diags.is_empty() => {
            return Err(format!("parsed without diagnostics\n{}", dump(&node, &diags)).into());
        }
        _ => {}
    }
    if let Some(name) = snapshot {
        // Same naming as the earlier `insta::glob!` setup: `data__ok@let.bolt.snap`.
        let file_name = path.file_name().unwrap().to_str().unwrap();
        insta::with_settings!({ input_file => path, snapshot_suffix => file_name }, {
            insta::assert_snapshot!(name, dump(&node, &diags));
        });
    }
    Ok(())
}

/// Run `f` with its output (including insta's diffs, panic messages and
/// anything printed by threads it spawns) captured. The output is dropped if
/// the test passes and appended to the failure message if it fails.
fn run_captured(f: impl FnOnce() -> Result<(), Failed>) -> Result<(), Failed> {
    let buf = Arc::new(Mutex::new(Vec::new()));
    let prev = std::io::set_output_capture(Some(buf.clone()));
    let result = catch_unwind(AssertUnwindSafe(f));
    std::io::set_output_capture(prev);

    let message = match result {
        Ok(Ok(())) => return Ok(()),
        Ok(Err(failed)) => failed.message().unwrap_or_default().to_string(),
        Err(_) => "test panicked".to_string(),
    };
    let output = String::from_utf8_lossy(&buf.lock().unwrap()).into_owned();
    if output.trim().is_empty() {
        Err(message.into())
    } else {
        Err(format!("{message}\n\n{output}").into())
    }
}

fn main() {
    let args = Arguments::from_args();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let failing = known_failures(&root);
    let mut trials = Vec::new();
    for group in GROUPS {
        for path in bolt_files(&root.join(group.dir)) {
            let relative = path.strip_prefix(root.join("corpus")).ok().and_then(Path::to_str);
            let ignored = group.ignored
                || (group.known_failures && relative.is_some_and(|r| failing.iter().any(|f| f == r)));
            let name = format!(
                "{}::{}",
                group.dir.replace('/', "::"),
                path.file_stem().unwrap().to_str().unwrap(),
            );
            let (expect, snapshot) = (group.expect, group.snapshot);
            trials.push(
                Trial::test(name, move || run_captured(|| check(&path, expect, snapshot)))
                    .with_ignored_flag(ignored),
            );
        }
    }
    libtest_mimic::run(&args, trials).exit();
}

//! Snapshot tests driven by the `.bolt` files in `tests/data`. Each file is a
//! separate test, e.g. `ok::let_mut`.
//!
//! - `ok/`: must parse without diagnostics or `ERROR` nodes.
//! - `err/`: must produce at least one diagnostic; the snapshot pins down
//!   how the parser recovers.
//! - `todo/ok/`, `todo/err/`: cases the parser can't handle correctly yet.
//!   They show up as ignored; run them with `-- --ignored`. They aren't
//!   snapshotted. Move a file into `ok/` or `err/` once it works.
//!
//! Add a test by adding a file, then run `cargo insta test --review`.
//! Run a single file with `cargo test --test data let_mut`.

mod common;

use std::path::{Path, PathBuf};

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
}

const GROUPS: &[Group] = &[
    Group { dir: "ok", expect: Expect::Ok, snapshot: Some("ok"), ignored: false },
    Group { dir: "err", expect: Expect::Err, snapshot: Some("err"), ignored: false },
    Group { dir: "todo/ok", expect: Expect::Ok, snapshot: None, ignored: true },
    Group { dir: "todo/err", expect: Expect::Err, snapshot: None, ignored: true },
];

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

fn main() {
    let args = Arguments::from_args();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let mut trials = Vec::new();
    for group in GROUPS {
        for path in bolt_files(&root.join(group.dir)) {
            let name = format!(
                "{}::{}",
                group.dir.replace('/', "::"),
                path.file_stem().unwrap().to_str().unwrap(),
            );
            let (expect, snapshot) = (group.expect, group.snapshot);
            trials.push(
                Trial::test(name, move || check(&path, expect, snapshot))
                    .with_ignored_flag(group.ignored),
            );
        }
    }
    libtest_mimic::run(&args, trials).exit();
}

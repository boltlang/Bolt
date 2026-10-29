//! `bolt test`, run on files in a temporary directory, without a prelude,
//! so that it doesn't depend on `std`.

use std::{fs, path::PathBuf, process::Command};

/// A new directory with `files`, as `(path, text)`.
fn dir_with(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("bolt-test-command-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    for (path, text) in files {
        let path = dir.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    dir
}

/// Run `bolt test --no-prelude` with `args` in `dir`: whether it succeeded,
/// and what it printed to stdout.
fn bolt_test(dir: &PathBuf, args: &[&str]) -> (bool, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_bolt"))
        .arg("test")
        .arg("--no-prelude")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    (output.status.success(), String::from_utf8(output.stdout).unwrap())
}

const PANIC: &str = "extern \"bolt\" { fn panic(message: String) -> !; }\n";

#[test]
fn runs_the_tests_of_a_directory() {
    let dir = dir_with(
        "runs",
        &[
            ("a.bolt", &format!("{PANIC}#[test]\nfn passes() {{}}\n#[test]\nfn fails() {{ panic(\"no\"); }}\n")),
            ("sub/b.bolt", "#[test]\nfn also_passes() { let x = 1; }\n"),
            (".hidden/c.bolt", &format!("{PANIC}#[test]\nfn hidden() {{ panic(\"not run\"); }}\n")),
        ],
    );
    let (ok, out) = bolt_test(&dir, &[]);
    assert!(!ok, "{out}");
    assert_eq!(
        out,
        "test a.bolt::passes ... ok\ntest a.bolt::fails ... FAILED\ntest sub/b.bolt::also_passes ... ok\n\
         test result: FAILED. 2 passed; 1 failed\n"
    );
    let (ok, out) = bolt_test(&dir, &["--filter", "pass"]);
    assert!(ok, "{out}");
    assert!(out.ends_with("test result: ok. 2 passed; 0 failed\n"), "{out}");
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn doesnt_run_the_tests_of_a_file_with_errors() {
    let dir = dir_with("errors", &[("a.bolt", "#[test]\nfn broken() { let x: i32 = True; }\n")]);
    let (ok, out) = bolt_test(&dir, &[]);
    assert!(!ok, "{out}");
    assert_eq!(out, "a.bolt: not run, since it has errors\ntest result: FAILED. 0 passed; 0 failed; 1 not run\n");
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn stops_a_test_that_runs_out_of_fuel() {
    let dir = dir_with("fuel", &[("a.bolt", "#[test]\nfn looping() { loop {} }\n")]);
    let (ok, out) = bolt_test(&dir, &["--fuel", "1000"]);
    assert!(!ok, "{out}");
    assert_eq!(out, "test a.bolt::looping ... FAILED\ntest result: FAILED. 0 passed; 1 failed\n");
    fs::remove_dir_all(dir).unwrap();
}

/// The tests of `std`, the functions with `#[test]` in `packages/std`, pass.
#[test]
fn the_tests_of_std_pass() {
    let std = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/std");
    let output = Command::new(env!("CARGO_BIN_EXE_bolt")).arg("test").arg(&std).output().unwrap();
    let (stdout, stderr) = (String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    assert!(output.status.success(), "{stdout}{stderr}");
    assert!(stdout.contains("::pushing_and_popping ... ok"), "{stdout}");
}

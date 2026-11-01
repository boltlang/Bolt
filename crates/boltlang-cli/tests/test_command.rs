//! `bolt test`, run on files in a temporary directory, with the prelude of
//! the repository's `std`, which has `panic` and the `String` it takes.

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

/// Run `bolt test` with `args` in `dir`: whether it succeeded, and what it
/// printed to stdout.
fn bolt_test(dir: &PathBuf, args: &[&str]) -> (bool, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_bolt"))
        .arg("test")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    (output.status.success(), String::from_utf8(output.stdout).unwrap())
}


#[test]
fn runs_the_tests_of_a_directory() {
    let dir = dir_with(
        "runs",
        &[
            ("a.bolt", "#[test]\nfn passes() {}\n#[test]\nfn fails() { panic(\"no\"); }\n"),
            ("sub/b.bolt", "#[test]\nfn also_passes() { let x = 1; }\n"),
            (".hidden/c.bolt", "#[test]\nfn hidden() { panic(\"not run\"); }\n"),
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

#[test]
fn runs_the_tests_compiled_to_wasm() {
    let dir = dir_with(
        "wasm",
        &[(
            "a.bolt",
            "fn double(x: i32) -> i32 = x * 2;\n\
             #[test]\nfn doubles() { assert(double(21) == 42); }\n\
             #[test]\nfn fails() { assert(double(1) == 3); }\n\
             #[test]\nfn loops() { loop {} }\n",
        )],
    );
    let (ok, out) = bolt_test(&dir, &["--wasm", "--fuel", "100000"]);
    assert!(!ok, "{out}");
    assert_eq!(
        out,
        "test a.bolt::doubles ... ok\ntest a.bolt::fails ... FAILED\ntest a.bolt::loops ... FAILED\n\
         test result: FAILED. 1 passed; 2 failed\n"
    );
    let (ok, out) = bolt_test(&dir, &["--wasm", "--filter", "doubles"]);
    assert!(ok, "{out}");
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_test_that_cant_be_compiled_fails() {
    let dir = dir_with("wasm-unsupported", &[("a.bolt", "#[test]\nfn calls_a_closure() { let f = |x: i32| x; assert(f(1) == 1); }\n")]);
    let output =
        Command::new(env!("CARGO_BIN_EXE_bolt")).args(["test", "--wasm"]).current_dir(&dir).output().unwrap();
    assert!(!output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "test a.bolt::calls_a_closure ... FAILED\ntest result: FAILED. 0 passed; 1 failed\n");
    assert!(String::from_utf8(output.stderr).unwrap().contains("not supported by the code generator"));
    fs::remove_dir_all(dir).unwrap();
}

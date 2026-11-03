//! `bolt dump-ast` and `bolt check`, run on files in a temporary directory,
//! without a prelude, so that they don't depend on `std`.

use std::{fs, path::PathBuf, process::Command};

/// A new directory with `files`, as `(path, text)`.
fn dir_with(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("bolt-commands-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    for (path, text) in files {
        let path = dir.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    dir
}

/// What running `bolt` gave: whether it succeeded, and what it printed to
/// stdout and to stderr.
struct Output {
    ok: bool,
    out: String,
    err: String,
}

/// Run `bolt <command> --no-prelude` with `args` in `dir`.
fn bolt(dir: &PathBuf, command: &str, args: &[&str]) -> Output {
    let mut all = vec!["--no-prelude"];
    all.extend(args);
    bolt_std(dir, command, &all)
}

/// Run `bolt <command>` with `args` in `dir`, with the prelude of `std`.
fn bolt_std(dir: &PathBuf, command: &str, args: &[&str]) -> Output {
    let output = Command::new(env!("CARGO_BIN_EXE_bolt"))
        .arg(command)
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    Output {
        ok: output.status.success(),
        out: String::from_utf8(output.stdout).unwrap(),
        err: String::from_utf8(output.stderr).unwrap(),
    }
}

#[test]
fn dump_ast_prints_the_tree() {
    let dir = dir_with("dump", &[("a.bolt", "let x = 1;\n")]);
    let o = bolt(&dir, "dump-ast", &["a.bolt"]);
    assert!(o.ok, "{}", o.err);
    assert!(o.out.starts_with("- SOURCE_FILE\n"), "{}", o.out);
    assert!(o.out.contains("\"x\" IDENT"), "{}", o.out);
    assert!(o.out.contains("\"1\" INT_NUMBER"), "{}", o.out);
}

#[test]
fn dump_ast_prints_json() {
    let dir = dir_with("dump-json", &[("a.bolt", "let x = 1;\n")]);
    let o = bolt(&dir, "dump-ast", &["a.bolt", "--format", "json"]);
    assert!(o.ok, "{}", o.err);
    let tree: serde_json::Value = serde_json::from_str(&o.out).unwrap();
    assert!(tree.is_object() || tree.is_array(), "{}", o.out);
    assert!(o.out.contains("SOURCE_FILE"), "{}", o.out);
}

#[test]
fn dump_ast_reports_syntax_errors_and_still_prints() {
    let dir = dir_with("dump-error", &[("a.bolt", "let x = ;\n")]);
    let o = bolt(&dir, "dump-ast", &["a.bolt"]);
    assert!(!o.ok);
    assert!(o.err.contains("Error"), "{}", o.err);
    assert!(o.out.starts_with("- SOURCE_FILE\n"), "{}", o.out);
}

#[test]
fn a_missing_file_is_an_error() {
    let dir = dir_with("missing", &[]);
    for command in ["dump-ast", "check"] {
        let o = bolt(&dir, command, &["nope.bolt"]);
        assert!(!o.ok, "{command}");
        assert!(o.err.contains("error: cannot read `nope.bolt`"), "{command}: {}", o.err);
    }
}

#[test]
fn check_succeeds_without_errors() {
    let dir = dir_with(
        "check-ok",
        &[("a.bolt", "fn id(x) = x;\nid(1);\n"), ("b.bolt", "let y = True;\n")],
    );
    let o = bolt(&dir, "check", &["a.bolt", "b.bolt"]);
    assert!(o.ok, "{}", o.err);
    assert_eq!(o.err, "");
}

#[test]
fn check_reports_type_errors_with_the_file() {
    let dir = dir_with("check-error", &[("a.bolt", "let x: i32 = True;\n")]);
    let o = bolt(&dir, "check", &["a.bolt"]);
    assert!(!o.ok);
    assert!(o.err.contains("a.bolt"), "{}", o.err);
    assert!(o.err.contains("expected `i32`"), "{}", o.err);
}

#[test]
fn check_reports_a_used_file_with_errors() {
    let dir = dir_with(
        "check-dependency",
        &[
            ("lib.bolt", "pub fn f() -> i32 = True;\n"),
            ("main.bolt", "import \"./lib\" (f);\nf();\n"),
        ],
    );
    let o = bolt(&dir, "check", &["main.bolt"]);
    assert!(!o.ok);
    assert!(o.err.contains("lib.bolt"), "{}", o.err);
    // Checking both reports the error of `lib.bolt` once, as its own.
    let both = bolt(&dir, "check", &["main.bolt", "lib.bolt"]);
    assert!(!both.ok);
    assert_eq!(both.err.matches("Error:").count(), 1, "{}", both.err);
}

#[test]
fn run_test_and_build_are_not_available() {
    let dir = dir_with("stubs", &[("a.bolt", "1;\n")]);
    for command in ["run", "test", "build"] {
        let o = bolt(&dir, command, &["a.bolt", "--wasm"]);
        assert!(!o.ok, "{command}");
        assert!(o.err.contains("not available until the Conduit backend lands"), "{}", o.err);
    }
}

/// `bolt --version`, with `BOLT_PRELUDE_PATH` set to `std`, or unset.
fn version(std: Option<&str>) -> String {
    let mut command = Command::new(env!("CARGO_BIN_EXE_bolt"));
    command.arg("--version");
    match std {
        Some(path) => command.env("BOLT_PRELUDE_PATH", path),
        None => command.env_remove("BOLT_PRELUDE_PATH"),
    };
    let output = command.output().unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn version_says_where_std_is() {
    let dir = dir_with("version", &[]);
    let real = dir.canonicalize().unwrap();
    assert_eq!(
        version(Some(dir.to_str().unwrap())),
        format!(
            "bolt {}\nstd: {} (from BOLT_PRELUDE_PATH)\n",
            env!("CARGO_PKG_VERSION"),
            real.display()
        )
    );
    assert!(version(Some("/no/such/dir")).ends_with(
        "std: not found: BOLT_PRELUDE_PATH is `/no/such/dir`, which isn't a directory\n"
    ),);
    // A debug build uses the `std` of its repository.
    if cfg!(debug_assertions) {
        assert!(
            version(None).ends_with("(from the repository of this debug build)\n"),
            "{}",
            version(None)
        );
    }
}

#[test]
fn a_missing_std_is_a_warning() {
    // Without `std` there are no operators: `+` is a function of the prelude.
    let dir = dir_with("no-std", &[("a.bolt", "True;\n")]);
    let output = Command::new(env!("CARGO_BIN_EXE_bolt"))
        .args(["check", "--no-prelude", "a.bolt"])
        .env("BOLT_PRELUDE_PATH", "/no/such/dir")
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "");
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "warning: BOLT_PRELUDE_PATH is `/no/such/dir`, which isn't a directory\n"
    );
}

//! `bolt dump-ast`, `bolt check`, `bolt run` and `bolt build`, run on files
//! in a temporary directory, without a prelude, so that they don't depend on
//! `std`.

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
    let output = Command::new(env!("CARGO_BIN_EXE_bolt"))
        .arg(command)
        .arg("--no-prelude")
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

const PRINT: &str = "extern \"bolt\" { fn print(text: String); }\n";

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
    for command in ["dump-ast", "check", "run", "build"] {
        let o = bolt(&dir, command, &["nope.bolt"]);
        assert!(!o.ok, "{command}");
        assert!(o.err.contains("error: cannot read `nope.bolt`"), "{command}: {}", o.err);
    }
}

#[test]
fn check_succeeds_without_errors() {
    let dir = dir_with("check-ok", &[("a.bolt", "fn id(x) = x;\nid(1);\n"), ("b.bolt", "let y = True;\n")]);
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
        &[("lib.bolt", "pub fn f() -> i32 = True;\n"), ("main.bolt", "import \"./lib\" (f);\nf();\n")],
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
fn run_prints_what_the_program_prints_and_its_value() {
    let dir = dir_with("run", &[("a.bolt", &format!("{PRINT}print(\"hi\");\n1 + 2;\n"))]);
    for wasm in [false, true] {
        let args: &[&str] = if wasm { &["a.bolt", "--wasm"] } else { &["a.bolt"] };
        let o = bolt(&dir, "run", args);
        assert!(o.ok, "wasm: {wasm}: {}", o.err);
        assert_eq!(o.out, "hi\n3\n", "wasm: {wasm}");
    }
}

#[test]
fn run_doesnt_print_a_unit_value() {
    let dir = dir_with("run-unit", &[("a.bolt", "fn f() {}\nf();\n")]);
    let o = bolt(&dir, "run", &["a.bolt"]);
    assert!(o.ok, "{}", o.err);
    assert_eq!(o.out, "");
}

#[test]
fn run_doesnt_run_a_program_with_errors() {
    let dir = dir_with("run-error", &[("a.bolt", &format!("{PRINT}print(\"hi\");\nlet x: i32 = True;\n"))]);
    let o = bolt(&dir, "run", &["a.bolt"]);
    assert!(!o.ok);
    assert_eq!(o.out, "");
    // With `--no-check`, it runs anyway.
    let o = bolt(&dir, "run", &["a.bolt", "--no-check"]);
    assert_eq!(o.out, "hi\n", "{}", o.err);
}

#[test]
fn run_reports_an_error_while_running() {
    let dir = dir_with("run-panic", &[("a.bolt", "fn div(a: i32, b: i32) -> i32 = a / b;\ndiv(1, 0);\n")]);
    let o = bolt(&dir, "run", &["a.bolt"]);
    assert!(!o.ok);
    assert!(o.err.contains("division by zero"), "{}", o.err);
}

#[test]
fn run_stops_when_it_runs_out_of_fuel() {
    let dir = dir_with("run-fuel", &[("a.bolt", "fn f(n: i32) -> i32 = f(n);\nf(0);\n")]);
    for wasm in [false, true] {
        let mut args = vec!["a.bolt", "--fuel", "1000"];
        if wasm {
            args.push("--wasm");
        }
        let o = bolt(&dir, "run", &args);
        assert!(!o.ok, "wasm: {wasm}");
        assert!(!o.err.is_empty(), "wasm: {wasm}");
    }
}

#[test]
fn run_reports_what_the_code_generator_doesnt_support() {
    let dir = dir_with("run-unsupported", &[("a.bolt", "(1, 2);\n")]);
    let o = bolt(&dir, "run", &["a.bolt", "--wasm"]);
    assert!(!o.ok);
    assert!(o.err.contains("not supported"), "{}", o.err);
}

#[test]
fn build_writes_a_module() {
    let dir = dir_with("build", &[("a.bolt", "1 + 2;\n")]);
    let o = bolt(&dir, "build", &["a.bolt"]);
    assert!(o.ok, "{}", o.err);
    let bytes = fs::read(dir.join("a.wasm")).unwrap();
    assert_eq!(&bytes[..4], b"\0asm");
    let o = bolt(&dir, "build", &["a.bolt", "-o", "out.wasm"]);
    assert!(o.ok, "{}", o.err);
    assert_eq!(fs::read(dir.join("out.wasm")).unwrap(), bytes);
}

#[test]
fn build_doesnt_write_a_module_with_errors() {
    let dir = dir_with(
        "build-error",
        &[("a.bolt", "let x: i32 = True;\n"), ("b.bolt", "(1, 2);\n")],
    );
    let o = bolt(&dir, "build", &["a.bolt"]);
    assert!(!o.ok);
    assert!(!dir.join("a.wasm").exists());
    let o = bolt(&dir, "build", &["b.bolt"]);
    assert!(!o.ok);
    assert!(o.err.contains("not supported"), "{}", o.err);
    assert!(!dir.join("b.wasm").exists());
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
        format!("bolt {}\nstd: {} (from BOLT_PRELUDE_PATH)\n", env!("CARGO_PKG_VERSION"), real.display())
    );
    assert!(
        version(Some("/no/such/dir")).ends_with("std: not found: BOLT_PRELUDE_PATH is `/no/such/dir`, which isn't a directory\n"),
    );
    // A debug build uses the `std` of its repository.
    if cfg!(debug_assertions) {
        assert!(version(None).ends_with("(from the repository of this debug build)\n"), "{}", version(None));
    }
}

#[test]
fn a_missing_std_is_a_warning() {
    let dir = dir_with("no-std", &[("a.bolt", "1 + 2;\n")]);
    let output = Command::new(env!("CARGO_BIN_EXE_bolt"))
        .args(["run", "--no-prelude", "a.bolt"])
        .env("BOLT_PRELUDE_PATH", "/no/such/dir")
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "3\n");
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "warning: BOLT_PRELUDE_PATH is `/no/such/dir`, which isn't a directory\n"
    );
}

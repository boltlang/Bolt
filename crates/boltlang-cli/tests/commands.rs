//! The commands of `bolt`, run on files in a temporary directory: without
//! a prelude where they can, so that they don't depend on `std`.

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
fn build_writes_a_wasm_module() {
    let dir = dir_with("build", &[("a.bolt", "1;\n")]);
    let o = bolt(&dir, "build", &["a.bolt"]);
    assert!(o.ok, "{}", o.err);
    let bytes = fs::read(dir.join("a.wasm")).unwrap();
    assert!(bytes.starts_with(b"\0asm"), "not a Wasm module");
    let o = bolt(&dir, "build", &["a.bolt", "-o", "b.wasm", "--no-opt"]);
    assert!(o.ok, "{}", o.err);
    assert!(fs::read(dir.join("b.wasm")).unwrap().starts_with(b"\0asm"));
}

#[test]
fn build_writes_nothing_with_errors() {
    let dir = dir_with("build-errors", &[("a.bolt", "let x: i32 = True;\n")]);
    let o = bolt(&dir, "build", &["a.bolt"]);
    assert!(!o.ok);
    assert!(!dir.join("a.wasm").exists());
}

/// Bolt's runtime for Node, which runs a module of `bolt build`.
fn node_runtime() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../runtime/node.mjs")
}

#[test]
fn build_makes_a_module_that_runs_in_node() {
    if Command::new("node").arg("--version").output().is_err() {
        eprintln!("skipped: no `node` to run the module with");
        return;
    }
    let program = "effect Yield { fn yield(value: i32); }\n\
        fn produce() does Yield { yield(1); yield(2); }\n\
        fn sum(body: fn() does Yield) -> i32 =\n\
          match body() { () => 0, effect yield(x), k => x + resume(k, ()) };\n\
        print(\"hello\");\n\
        if sum(produce) == 3 { print(\"3\") } else { print(\"?\") };\n";
    let dir = dir_with("build-node", &[("a.bolt", program)]);
    let o = bolt_std(&dir, "build", &["a.bolt"]);
    assert!(o.ok, "{}", o.err);
    let run = Command::new("node")
        .arg(node_runtime())
        .args(["a.wasm", "main"])
        .current_dir(&dir)
        .output()
        .unwrap();
    let err = String::from_utf8(run.stderr).unwrap();
    assert!(run.status.success(), "{err}");
    assert_eq!(String::from_utf8(run.stdout).unwrap(), "hello\n3\n");
}

#[test]
fn run_prints_through_conduit() {
    let dir = dir_with("run", &[("a.bolt", "print(\"hello\");\nprint(\"world\");\n")]);
    let o = bolt_std(&dir, "run", &["a.bolt"]);
    assert!(o.ok, "{}", o.err);
    assert_eq!(o.out, "hello\nworld\n");
}

#[test]
fn run_with_wasm_prints_through_node() {
    if Command::new("node").arg("--version").output().is_err() {
        eprintln!("skipped: no `node` to run the module with");
        return;
    }
    let program = "print(\"hello\");\neprint(\"to stderr\");\nprint(show(-42));\n";
    let dir = dir_with("run-wasm", &[("a.bolt", program)]);
    let o = bolt_std(&dir, "run", &["--wasm", "a.bolt"]);
    assert!(o.ok, "{}", o.err);
    assert_eq!(o.out, "hello\n-42\n");
    assert!(o.err.contains("to stderr\n"), "{}", o.err);
    assert!(!dir.join("a.wasm").exists());
}

#[test]
fn run_with_wasm_stops_with_an_error() {
    if Command::new("node").arg("--version").output().is_err() {
        eprintln!("skipped: no `node` to run the module with");
        return;
    }
    let dir = dir_with("run-wasm-error", &[("a.bolt", "print(\"before\");\npanic(\"oops\");\n")]);
    let o = bolt_std(&dir, "run", &["--wasm", "a.bolt"]);
    assert!(!o.ok);
    assert_eq!(o.out, "before\n");
    assert!(o.err.contains("oops\n"), "{}", o.err);
    assert!(o.err.contains("stopped with an error"), "{}", o.err);
}

/// A program that says what it was compiled for.
const TARGETS: &str = r#"#[cfg(host = "emu")]
fn host() -> String = "emu";
#[cfg(host = "wasm")]
fn host() -> String = "wasm";
#[cfg(pointer_width = 32)]
fn width() -> String = "32";
#[cfg(pointer_width = 64)]
fn width() -> String = "64";
print(host());
print(width());
"#;

#[test]
fn run_compiles_for_the_target_it_imitates() {
    let dir = dir_with("run-target", &[("a.bolt", TARGETS)]);
    let o = bolt_std(&dir, "run", &["--target", "x86", "a.bolt"]);
    assert!(o.ok, "{}", o.err);
    assert_eq!(o.out, "emu\n32\n");
    let o = bolt_std(&dir, "run", &["--target", "x86_64", "a.bolt"]);
    assert!(o.ok, "{}", o.err);
    assert_eq!(o.out, "emu\n64\n");
    let o = bolt_std(&dir, "run", &["--wasm", "--target", "x86_64", "a.bolt"]);
    assert!(!o.ok);
    assert!(o.err.contains("compiled for `wasm32`, not `x86_64`"), "{}", o.err);
}

#[test]
fn run_with_wasm_compiles_for_wasm32() {
    if Command::new("node").arg("--version").output().is_err() {
        eprintln!("skipped: no `node` to run the module with");
        return;
    }
    let dir = dir_with("run-target-wasm", &[("a.bolt", TARGETS)]);
    let o = bolt_std(&dir, "run", &["--wasm", "a.bolt"]);
    assert!(o.ok, "{}", o.err);
    assert_eq!(o.out, "wasm\n32\n");
}

#[test]
fn run_stops_with_an_error() {
    let dir = dir_with("run-error", &[("a.bolt", "print(\"before\");\npanic(\"oops\");\n")]);
    let o = bolt_std(&dir, "run", &["a.bolt"]);
    assert!(!o.ok);
    assert_eq!(o.out, "before\n");
    assert!(o.err.contains("oops\n"), "{}", o.err);
    assert!(o.err.contains("stopped with an error"), "{}", o.err);
}

#[test]
fn run_stops_out_of_fuel() {
    let dir = dir_with("run-fuel", &[("a.bolt", "let mut i = 0;\nwhile True { i += 1; }\n")]);
    let o = bolt_std(&dir, "run", &["a.bolt", "--fuel", "1000"]);
    assert!(!o.ok);
    assert!(o.err.contains("out of fuel"), "{}", o.err);
}

#[test]
fn test_runs_the_tests_of_files() {
    let source = "fn double(x: i32) -> i32 = x * 2;\n\
        #[test]\nfn doubles() { assert(double(2) == 4); }\n\
        #[test]\nfn fails() { assert(double(2) == 5); }\n";
    let dir = dir_with("test", &[("a.bolt", source)]);
    let o = bolt_std(&dir, "test", &[]);
    assert!(!o.ok);
    assert!(o.out.contains("a.bolt::doubles ... ok"), "{}", o.out);
    assert!(o.out.contains("a.bolt::fails ... "), "{}", o.out);
    assert!(o.out.contains("test result: FAILED. 1 passed; 1 failed"), "{}", o.out);
    let filtered = bolt_std(&dir, "test", &["--filter", "doubles"]);
    assert!(filtered.ok, "{}", filtered.out);
    assert!(filtered.out.contains("test result: ok. 1 passed; 0 failed"), "{}", filtered.out);
}

#[test]
fn test_runs_the_tests_compiled_to_wasm() {
    if Command::new("node").arg("--version").output().is_err() {
        eprintln!("skipped: no `node` to run the tests with");
        return;
    }
    let source = "#[link(wasm, module = \"env\")]\nfn missing();\n\
        fn double(x: i32) -> i32 = x * 2;\n\
        #[test]\nfn doubles() { print(\"in doubles\"); assert(double(2) == 4); }\n\
        #[test]\nfn fails() { assert(double(2) == 5); }\n\
        #[test]\nfn calls_the_host() { missing(); }\n";
    let dir = dir_with("test-wasm", &[("a.bolt", source)]);
    let o = bolt_std(&dir, "test", &["--wasm"]);
    assert!(!o.ok);
    assert!(o.out.contains("a.bolt::doubles ... in doubles\nok"), "{}", o.out);
    assert!(o.out.contains("a.bolt::fails ... "), "{}", o.out);
    assert!(o.err.contains("no host for env.missing"), "{}", o.err);
    assert!(o.out.contains("test result: FAILED. 1 passed; 2 failed"), "{}", o.out);
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

pub(crate) mod parser_tests;

use std::path::Path;

use xshell::{Shell, cmd};

use crate::{CodegenType, project_root};

/// Checks that the `file` has the specified `contents`. If that is not the
/// case, updates the file and then fails the test.
#[allow(clippy::print_stderr)]
fn ensure_file_contents(cg: CodegenType, file: &Path, contents: &str, check: bool) -> bool {
    let contents = normalize_newlines(contents);
    if let Ok(old_contents) = std::fs::read_to_string(file)
        && normalize_newlines(&old_contents) == contents
    {
        // File is already up to date.
        return false;
    }

    let display_path = file.strip_prefix(project_root()).unwrap_or(file);
    if check {
        panic!(
            "{} was not up-to-date{}",
            file.display(),
            if std::env::var("CI").is_ok() {
                format!(
                    "\n    NOTE: run `cargo xtask codegen {cg}` locally and commit the updated files\n"
                )
            } else {
                "".to_owned()
            }
        );
    } else {
        eprintln!(
            "\n\x1b[31;1merror\x1b[0m: {} was not up-to-date, updating\n",
            display_path.display()
        );

        if let Some(parent) = file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(file, contents).unwrap();
        true
    }
}

fn reformat(text: String) -> String {
    let sh = Shell::new().unwrap();
    let rustfmt_toml = project_root().join("rustfmt.toml");
    let toolchain = &std::env::var("RUSTFMT_TOOLCHAIN").unwrap_or("stable".to_owned());
    let version = cmd!(sh, "rustup run {toolchain} rustfmt --version").read().unwrap_or_default();

    // First try explicitly requesting the stable channel via rustup in case nightly is being used by default,
    // then plain rustfmt in case rustup isn't being used to manage the compiler (e.g. when using Nix).
    let mut stdout = if !version.contains(toolchain) {
        let version = cmd!(sh, "rustfmt --version").read().unwrap_or_default();
        if !version.contains(toolchain) {
            panic!(
                "Failed to run rustfmt from toolchain '{toolchain}'. \
                 Please run `rustup component add rustfmt --toolchain {toolchain}` to install it.",
            );
        } else {
            cmd!(sh, "rustfmt --config-path {rustfmt_toml} --config fn_single_line=true")
                .stdin(text)
                .read()
                .unwrap()
        }
    } else {
        cmd!(
            sh,
            "rustup run {toolchain} rustfmt --config-path {rustfmt_toml} --config fn_single_line=true"
        )
        .stdin(text)
        .read()
        .unwrap()
    };
    if !stdout.ends_with('\n') {
        stdout.push('\n');
    }
    stdout
}


fn normalize_newlines(s: &str) -> String {
    s.replace("\r\n", "\n")
}

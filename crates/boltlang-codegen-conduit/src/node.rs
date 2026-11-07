//! Running a Wasm module of [`crate::wasm`] with Node, for `bolt run
//! --wasm`, `bolt test --wasm` and the tests of `std`.
//!
//! The host, `node_host.mjs`, is run as `node host.mjs <module> <export>`
//! and calls the export: `main`, or a test. It gives the module `bolt`'s
//! `write_file` and `flush`, on stdout and stderr, Node's WASI preview 1
//! as `wasi_snapshot_preview1`, with no arguments, environment or
//! preopened directories, and every other import a function that stops
//! the program, as the evaluator does for a host function it doesn't
//! have. It exits with 1 when the export stops with an error, or exits
//! with a code other than 0, and with 2 when the module can't be run.

use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU32, Ordering},
};

/// The script of the host.
pub const HOST: &str = include_str!("node_host.mjs");

/// Whether there's a `node` to run modules with.
pub fn available() -> bool {
    Command::new("node").arg("--version").output().is_ok_and(|o| o.status.success())
}

/// A directory with the host, for modules to be run with it. It's removed
/// when this is dropped.
pub struct NodeHost {
    pub dir: PathBuf,
}

impl NodeHost {
    /// A new directory with the host, in the system's temporary directory.
    pub fn new() -> std::io::Result<Self> {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("bolt-node-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join("host.mjs"), HOST)?;
        Ok(NodeHost { dir })
    }

    /// Call the export `name` of the module at `module`, with its output on
    /// ours: `Err` with why it stopped, if it stopped with an error.
    pub fn run(&self, module: &Path, name: &str) -> std::io::Result<Result<(), String>> {
        // Node's WASI warns that it's experimental.
        let status = Command::new("node")
            .arg("--no-warnings")
            .arg(self.dir.join("host.mjs"))
            .arg(module)
            .arg(name)
            .status()?;
        Ok(match status.code() {
            Some(0) => Ok(()),
            Some(2) => Err("the module couldn't be run".into()),
            _ => Err("the program stopped with an error".into()),
        })
    }
}

impl NodeHost {
    /// Like [`NodeHost::run`], but with what the program wrote to stdout and
    /// stderr given back, after the result, instead of shown.
    pub fn run_captured(
        &self,
        module: &Path,
        name: &str,
    ) -> std::io::Result<(Result<(), String>, String)> {
        let output = Command::new("node")
            .arg("--no-warnings")
            .arg(self.dir.join("host.mjs"))
            .arg(module)
            .arg(name)
            .output()?;
        let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
        text.push_str(&String::from_utf8_lossy(&output.stderr));
        let result = match output.status.code() {
            Some(0) => Ok(()),
            Some(2) => Err("the module couldn't be run".into()),
            _ => Err("the program stopped with an error".into()),
        };
        Ok((result, text))
    }
}

impl Drop for NodeHost {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

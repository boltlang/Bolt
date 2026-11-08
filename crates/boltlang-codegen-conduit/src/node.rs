//! Running a Wasm module of [`crate::wasm`] with Node, for `bolt run
//! --wasm`, `bolt test --wasm` and the tests of `std`.
//!
//! The runtime, `runtime/node.mjs` in the repository, is run as `node
//! runtime.mjs <module> <export>` and calls the export: `main`, or a
//! test. It gives the module Node's WASI preview 1 as
//! `wasi_snapshot_preview1`, with no arguments, environment or preopened
//! directories, and every other import a function that stops the
//! program, as the evaluator does for a host function it doesn't have. It exits with 1 when the export stops
//! with an error, or exits with a code other than 0, and with 2 when the
//! module can't be run.

use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU32, Ordering},
};

/// The script of the runtime.
pub const RUNTIME: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime/node.mjs"));

/// The script of the runtime for browsers, `runtime/browser.mjs`, which
/// Node can run too, with [`BROWSER_DRIVER`].
pub const BROWSER: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime/browser.mjs"));

/// A script that runs a module with the runtime for browsers in Node, as
/// `node browser-driver.mjs <module> <export>`, with the exit codes of
/// `runtime/node.mjs`.
pub const BROWSER_DRIVER: &str = r#"import fs from "node:fs";
import { instantiate } from "./browser.mjs";
const [, , path, name] = process.argv;
let call;
try {
  ({ call } = await instantiate(fs.readFileSync(path)));
} catch (e) {
  console.error(`can't run the module: ${e.message}`);
  process.exit(2);
}
process.exit(call(name));
"#;

/// Which runtime to run a module with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Runtime {
    /// `runtime/node.mjs`, with Node's WASI.
    Node,
    /// `runtime/browser.mjs`, with a WASI of its own and the console.
    Browser,
}

impl Runtime {
    fn script(self) -> &'static str {
        match self {
            Runtime::Node => "runtime.mjs",
            Runtime::Browser => "browser-driver.mjs",
        }
    }
}

/// Whether there's a `node` to run modules with.
pub fn available() -> bool {
    Command::new("node").arg("--version").output().is_ok_and(|o| o.status.success())
}

/// A directory with the runtimes, for modules to be run with them. It's
/// removed when this is dropped.
pub struct NodeHost {
    pub dir: PathBuf,
}

/// The flags Node runs a module with: no warnings, since Node's WASI warns
/// that it's experimental, and Wasm's exception handling, which Conduit's
/// handlers use and Node 24 has only behind a flag (#389).
const NODE_FLAGS: [&str; 2] = ["--no-warnings", "--experimental-wasm-exnref"];

impl NodeHost {
    /// A new directory with the runtimes, in the system's temporary
    /// directory.
    pub fn new() -> std::io::Result<Self> {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("bolt-node-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join("runtime.mjs"), RUNTIME)?;
        std::fs::write(dir.join("browser.mjs"), BROWSER)?;
        std::fs::write(dir.join("browser-driver.mjs"), BROWSER_DRIVER)?;
        Ok(NodeHost { dir })
    }

    /// Call the export `name` of the module at `module`, with its output on
    /// ours: `Err` with why it stopped, if it stopped with an error.
    pub fn run(&self, module: &Path, name: &str) -> std::io::Result<Result<(), String>> {
        // Node's WASI warns that it's experimental.
        let status = Command::new("node")
            .args(NODE_FLAGS)
            .arg(self.dir.join("runtime.mjs"))
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
    /// Like [`NodeHost::run`], with `runtime`, but with what the program
    /// wrote to stdout and stderr given back, after the result, instead of
    /// shown.
    pub fn run_captured(
        &self,
        runtime: Runtime,
        module: &Path,
        name: &str,
    ) -> std::io::Result<(Result<(), String>, String)> {
        let output = Command::new("node")
            .args(NODE_FLAGS)
            .arg(self.dir.join(runtime.script()))
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

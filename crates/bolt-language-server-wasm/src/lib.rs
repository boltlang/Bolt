//! The Bolt language server in the browser, built for
//! `wasm32-unknown-unknown` with wasm-bindgen.
//!
//! There's no stdio or file system there. The page passes each message from
//! the client to [`LanguageServer::handle`] and sends back what it returns,
//! usually from a web worker:
//!
//! ```js
//! import init, { LanguageServer } from "./bolt_language_server_wasm.js";
//!
//! await init();
//! const server = new LanguageServer();
//! // Files that documents import but that aren't open, like packages.
//! server.writeFile("/project/.bolt/packages/std/lib.bolt", "pub fn id(x) = x;\n");
//! self.onmessage = (event) => {
//!   for (const message of server.handle(JSON.stringify(event.data))) {
//!     self.postMessage(JSON.parse(message));
//!   }
//! };
//! ```
//!
//! With `monaco-languageclient`, the worker is the other end of a
//! `BrowserMessageReader` and `BrowserMessageWriter`. Documents need `file:`
//! URIs, like `file:///project/main.bolt`; their paths are where imports are
//! looked up.
//!
//! Build it, and make the JavaScript to load it, with:
//!
//! ```sh
//! cargo build --release --target wasm32-unknown-unknown -p bolt-language-server-wasm
//! wasm-bindgen --target web --out-dir pkg target/wasm32-unknown-unknown/release/bolt_language_server_wasm.wasm
//! ```
//!
//! `wasm-bindgen-cli` must have the version of `wasm-bindgen` in
//! `Cargo.lock`. `scripts/smoke-test.sh` runs a session in Node.
//!
//! A panic can't be caught in wasm, so it ends the module: the page has to
//! start a new one. The panic is shown in the console.

mod connection;

pub use connection::Connection;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct LanguageServer {
    connection: Connection,
}

impl Default for LanguageServer {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl LanguageServer {
    #[wasm_bindgen(constructor)]
    pub fn new() -> LanguageServer {
        LanguageServer { connection: Connection::new() }
    }

    /// Handle a JSON-RPC message from the client. Returns the messages to
    /// send to the client, as JSON.
    pub fn handle(&mut self, message: &str) -> Vec<String> {
        self.connection.handle(message)
    }

    /// Give the server a file that isn't open in the editor, at an absolute
    /// path. Returns the messages to send to the client.
    #[wasm_bindgen(js_name = writeFile)]
    pub fn write_file(&mut self, path: &str, text: &str) -> Result<Vec<String>, JsError> {
        self.connection.write_file(path, text).map_err(|e| JsError::new(&e))
    }

    /// Forget a file given with [`LanguageServer::write_file`]. Returns the
    /// messages to send to the client.
    #[wasm_bindgen(js_name = removeFile)]
    pub fn remove_file(&mut self, path: &str) -> Result<Vec<String>, JsError> {
        self.connection.remove_file(path).map_err(|e| JsError::new(&e))
    }
}

#[cfg(target_family = "wasm")]
unsafe extern "C" {
    /// Runs the constructors of the module, which Salsa needs: it registers
    /// its queries with the `inventory` crate.
    fn __wasm_call_ctors();
}

/// Runs once, when the module is loaded.
#[cfg(target_family = "wasm")]
#[wasm_bindgen(start)]
fn start() {
    // The module is called many times, so the linker may not run the
    // constructors by itself. See the documentation of `inventory`.
    unsafe { __wasm_call_ctors() };
    console_error_panic_hook::set_once();
}

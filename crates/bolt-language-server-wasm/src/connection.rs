//! JSON-RPC for the browser: one message from the client in, the messages
//! to send back out. Nothing here needs wasm, so it's tested natively.

use std::sync::Arc;

use bolt_language_server::{
    NotAFile, Server,
    ls_types::{
        DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams, DocumentDiagnosticParams,
        InitializeParams, MessageType,
    },
};
use boltlang_base_db::{InMemorySystem, SystemPath, WritableSystem};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

// The error codes of JSON-RPC and the protocol.
const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;
const SERVER_NOT_INITIALIZED: i64 = -32002;

pub struct Connection {
    server: Server,
    /// The files that aren't open, which the page gives the server, like
    /// those of `.bolt/packages`.
    files: InMemorySystem,
    initialized: bool,
    shut_down: bool,
    /// The id of the next request to the client.
    next_id: i64,
}

impl Default for Connection {
    fn default() -> Self {
        Self::new()
    }
}

impl Connection {
    pub fn new() -> Self {
        let files = InMemorySystem::new();
        Connection {
            server: Server::new(Arc::new(files.clone())),
            files,
            initialized: false,
            shut_down: false,
            next_id: 1,
        }
    }

    /// Handle a message from the client, and return the messages to send
    /// back: the response to a request, and requests and notifications of
    /// the server, like logging a warning.
    pub fn handle(&mut self, message: &str) -> Vec<String> {
        let message: Value = match serde_json::from_str(message) {
            Ok(message) => message,
            Err(error) => return vec![error_response(Value::Null, PARSE_ERROR, error.to_string()).to_string()],
        };
        let method = message.get("method").and_then(Value::as_str).map(str::to_string);
        let id = message.get("id").cloned();
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        let out = match (method, id) {
            (Some(method), Some(id)) => self.request(&method, id, params),
            (Some(method), None) => self.notification(&method, params),
            // The client's response to a request of the server's, like a
            // refresh, which needs nothing.
            (None, Some(_)) => Vec::new(),
            (None, None) => vec![error_response(Value::Null, INVALID_REQUEST, "not a request or a notification")],
        };
        out.into_iter().map(|message| message.to_string()).collect()
    }

    /// Use `text` for the file at `path`, which isn't open in the editor, like
    /// a file of a package that a document imports.
    pub fn write_file(&mut self, path: &str, text: &str) -> Result<Vec<String>, String> {
        let system_path = absolute(path)?;
        self.files.write_file_bytes(system_path, text.as_bytes()).map_err(|e| format!("can't write {path}: {e}"))?;
        Ok(self.file_changed(system_path))
    }

    /// Forget the file at `path`.
    pub fn remove_file(&mut self, path: &str) -> Result<Vec<String>, String> {
        let system_path = absolute(path)?;
        self.files.remove_file(system_path).map_err(|e| format!("can't remove {path}: {e}"))?;
        Ok(self.file_changed(system_path))
    }

    fn file_changed(&mut self, path: &SystemPath) -> Vec<String> {
        self.server.file_changed(path);
        self.refresh().into_iter().map(|message| message.to_string()).collect()
    }

    fn request(&mut self, method: &str, id: Value, params: Value) -> Vec<Value> {
        if self.shut_down {
            return vec![error_response(id, INVALID_REQUEST, "the server was shut down")];
        }
        if method == "initialize" {
            return match parse::<InitializeParams>(params) {
                Ok(params) => {
                    let result = self.server.initialize(&params);
                    self.initialized = true;
                    vec![response(id, json!(result))]
                }
                Err(error) => vec![error_response(id, INVALID_PARAMS, error)],
            };
        }
        if !self.initialized {
            return vec![error_response(id, SERVER_NOT_INITIALIZED, "the server isn't initialized yet")];
        }
        match method {
            "textDocument/diagnostic" => match parse::<DocumentDiagnosticParams>(params) {
                Ok(params) => {
                    let (report, error) = self.server.diagnostic(&params);
                    let mut out = vec![response(id, json!(report))];
                    out.extend(error.map(warn_not_a_file));
                    out
                }
                Err(error) => vec![error_response(id, INVALID_PARAMS, error)],
            },
            "shutdown" => {
                self.shut_down = true;
                vec![response(id, Value::Null)]
            }
            _ => vec![error_response(id, METHOD_NOT_FOUND, format!("unsupported method {method}"))],
        }
    }

    fn notification(&mut self, method: &str, params: Value) -> Vec<Value> {
        // Like other notifications before `initialize`, `initialized`,
        // `exit` and `$/cancelRequest` need nothing: requests are answered
        // right away, and the page ends the worker.
        if !self.initialized || self.shut_down {
            return Vec::new();
        }
        let result = match method {
            "textDocument/didOpen" => parse::<DidOpenTextDocumentParams>(params).map(|p| self.server.did_open(p)),
            "textDocument/didChange" => parse::<DidChangeTextDocumentParams>(params).map(|p| self.server.did_change(p)),
            "textDocument/didClose" => parse::<DidCloseTextDocumentParams>(params).map(|p| self.server.did_close(p)),
            _ => return Vec::new(),
        };
        match result {
            Ok(Ok(())) => self.refresh().into_iter().collect(),
            Ok(Err(error)) => vec![warn_not_a_file(error)],
            // A notification has no response to put the error in.
            Err(error) => vec![log(MessageType::ERROR, format!("invalid {method}: {error}"))],
        }
    }

    /// After a change, a request for new diagnostics of every document, if
    /// the client supports it.
    fn refresh(&mut self) -> Option<Value> {
        if !self.initialized || !self.server.should_refresh() {
            return None;
        }
        let id = self.next_id;
        self.next_id += 1;
        Some(json!({ "jsonrpc": "2.0", "id": id, "method": "workspace/diagnostic/refresh" }))
    }
}

fn absolute(path: &str) -> Result<&SystemPath, String> {
    let system_path = SystemPath::new(path);
    if system_path.is_absolute() {
        Ok(system_path)
    } else {
        Err(format!("expected an absolute path, like /lib.bolt: {path}"))
    }
}

fn parse<T: DeserializeOwned>(params: Value) -> Result<T, String> {
    serde_json::from_value(params).map_err(|error| error.to_string())
}

fn response(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn error_response(id: Value, code: i64, message: impl Into<String>) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message.into() } })
}

fn log(kind: MessageType, message: String) -> Value {
    json!({ "jsonrpc": "2.0", "method": "window/logMessage", "params": { "type": kind, "message": message } })
}

fn warn_not_a_file(error: NotAFile) -> Value {
    log(MessageType::WARNING, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAIN: &str = "file:///project/main.bolt";

    /// The messages a call returned, parsed.
    fn parsed(messages: Vec<String>) -> Vec<Value> {
        messages.iter().map(|m| serde_json::from_str(m).unwrap()).collect()
    }

    fn send(connection: &mut Connection, message: Value) -> Vec<Value> {
        parsed(connection.handle(&message.to_string()))
    }

    /// An initialized connection, whose client can be asked to refresh.
    fn initialized() -> Connection {
        let mut connection = Connection::new();
        let capabilities = json!({ "workspace": { "diagnostics": { "refreshSupport": true } } });
        let out = send(
            &mut connection,
            json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "capabilities": capabilities } }),
        );
        assert_eq!(out[0]["result"]["serverInfo"]["name"], "bolt-language-server");
        send(&mut connection, json!({ "jsonrpc": "2.0", "method": "initialized", "params": {} }));
        connection
    }

    fn open(connection: &mut Connection, uri: &str, text: &str) -> Vec<Value> {
        let document = json!({ "uri": uri, "languageId": "bolt", "version": 1, "text": text });
        send(connection, json!({ "jsonrpc": "2.0", "method": "textDocument/didOpen", "params": { "textDocument": document } }))
    }

    fn messages(connection: &mut Connection, uri: &str) -> Vec<String> {
        let out = send(
            connection,
            json!({ "jsonrpc": "2.0", "id": 7, "method": "textDocument/diagnostic", "params": { "textDocument": { "uri": uri } } }),
        );
        assert_eq!(out[0]["id"], 7);
        out[0]["result"]["items"].as_array().unwrap().iter().map(|d| d["message"].as_str().unwrap().to_string()).collect()
    }

    #[test]
    fn requests_wait_for_initialize() {
        let mut connection = Connection::new();
        let out = send(&mut connection, json!({ "jsonrpc": "2.0", "id": 1, "method": "shutdown" }));
        assert_eq!(out[0]["error"]["code"], SERVER_NOT_INITIALIZED);
        // Notifications before it are ignored.
        assert!(open(&mut connection, MAIN, "let x: i32 = True;\n").is_empty());
    }

    #[test]
    fn diagnostics_of_documents() {
        let mut connection = initialized();
        // A change asks the client to pull the diagnostics again.
        let out = open(&mut connection, MAIN, "let x: i32 = True;\n");
        assert_eq!(out, [json!({ "jsonrpc": "2.0", "id": 1, "method": "workspace/diagnostic/refresh" })]);
        assert_eq!(messages(&mut connection, MAIN), ["expected `i32`, found `Bool`"]);
        let change = json!({
            "textDocument": { "uri": MAIN, "version": 2 },
            "contentChanges": [{ "text": "let x: i32 = 1;\n" }],
        });
        let out = send(&mut connection, json!({ "jsonrpc": "2.0", "method": "textDocument/didChange", "params": change }));
        assert_eq!(out[0]["id"], 2);
        assert!(messages(&mut connection, MAIN).is_empty());
        // The client's response to a refresh needs nothing.
        assert!(send(&mut connection, json!({ "jsonrpc": "2.0", "id": 2, "result": null })).is_empty());
    }

    #[test]
    fn documents_import_the_files_of_the_page() {
        let mut connection = initialized();
        connection.write_file("/project/lib.bolt", "pub fn negate(x: Bool) -> Bool = !x;\n").unwrap();
        open(&mut connection, MAIN, "import \"./lib\" (negate);\nnegate(1);\n");
        assert_eq!(messages(&mut connection, MAIN), ["expected `Bool`, found an integer"]);
        // Writing a file asks for new diagnostics, which see it.
        let out = parsed(connection.write_file("/project/lib.bolt", "pub fn negate(x: i32) -> i32 = -x;\n").unwrap());
        assert_eq!(out[0]["method"], "workspace/diagnostic/refresh");
        assert!(messages(&mut connection, MAIN).is_empty());
        connection.remove_file("/project/lib.bolt").unwrap();
        assert_eq!(messages(&mut connection, MAIN), ["cannot find the file `./lib`", "cannot find `negate`"]);
        assert!(connection.write_file("lib.bolt", "").is_err());
        assert!(connection.remove_file("/project/lib.bolt").is_err());
    }

    #[test]
    fn documents_that_are_not_files() {
        let mut connection = initialized();
        let out = open(&mut connection, "untitled:Untitled-1", "let x: i32 = True;\n");
        assert_eq!(out[0]["method"], "window/logMessage");
        assert_eq!(out[0]["params"]["message"], "only files are supported, not untitled:Untitled-1");
        assert!(messages(&mut connection, "untitled:Untitled-1").is_empty());
    }

    #[test]
    fn errors() {
        let mut connection = initialized();
        assert_eq!(parsed(connection.handle("{"))[0]["error"]["code"], PARSE_ERROR);
        let out = send(&mut connection, json!({ "jsonrpc": "2.0", "id": 3, "method": "textDocument/hover", "params": {} }));
        assert_eq!(out[0]["error"]["code"], METHOD_NOT_FOUND);
        let out = send(&mut connection, json!({ "jsonrpc": "2.0", "id": 4, "method": "textDocument/diagnostic", "params": 1 }));
        assert_eq!(out[0]["error"]["code"], INVALID_PARAMS);
        let out = send(&mut connection, json!({ "jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {} }));
        assert_eq!(out[0]["params"]["type"], 1);
        let out = send(&mut connection, json!({ "jsonrpc": "2.0", "id": 5, "method": "shutdown" }));
        assert_eq!(out, [json!({ "jsonrpc": "2.0", "id": 5, "result": null })]);
        let out = send(&mut connection, json!({ "jsonrpc": "2.0", "id": 6, "method": "shutdown" }));
        assert_eq!(out[0]["error"]["code"], INVALID_REQUEST);
    }
}

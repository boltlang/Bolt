mod db;
mod position;
mod system;
mod workspace;

use std::{
    any::Any,
    panic::{self, AssertUnwindSafe},
    path::PathBuf,
    sync::{
        Mutex, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
};

use boltlang_base_db::{SystemPath, SystemPathBuf};
use tower_lsp_server::{
    Client, LanguageServer, LspService, Server, jsonrpc,
    ls_types::{
        DiagnosticOptions, DiagnosticServerCapabilities, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
        DidOpenTextDocumentParams, DocumentDiagnosticParams, DocumentDiagnosticReport,
        DocumentDiagnosticReportResult, FullDocumentDiagnosticReport, InitializeParams, InitializeResult,
        InitializedParams, MessageType, RelatedFullDocumentDiagnosticReport, ServerCapabilities, ServerInfo,
        TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions, Uri,
    },
};

use crate::{position::Encoding, workspace::Workspace};

const LANGUAGE_SERVER_NAME: &str = "bolt-language-server";
const LANGUAGE_SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

struct Backend {
    client: Client,
    workspace: Mutex<Workspace>,
    /// Whether the client can be asked to pull the diagnostics of all its
    /// documents again, which an edit of one can change.
    refresh_support: AtomicBool,
}

impl Backend {
    /// Run `f` on the workspace. A panic in it, like a bug in the type
    /// checker, fails only this request: it's logged and returned as an error.
    async fn with_workspace<T>(&self, f: impl FnOnce(&mut Workspace) -> T) -> Result<T, String> {
        let result = guarded(&self.workspace, f);
        if let Err(message) = &result {
            self.client.log_message(MessageType::ERROR, format!("internal error: {message}")).await;
        }
        result
    }

    /// The path of a document, or `None` after logging that it's not a file.
    async fn path_of(&self, uri: &Uri) -> Option<SystemPathBuf> {
        match uri.to_file_path() {
            Some(path) => Some(SystemPathBuf::from(path.into_owned())),
            None => {
                self.client.log_message(MessageType::WARNING, format!("only files are supported, not {}", uri.as_str())).await;
                None
            }
        }
    }

    /// Ask the client for new diagnostics of every document, after an edit.
    async fn refresh_diagnostics(&self) {
        if !self.refresh_support.load(Ordering::Relaxed) {
            return;
        }
        if let Err(error) = self.client.workspace_diagnostic_refresh().await {
            self.client.log_message(MessageType::ERROR, format!("refreshing diagnostics failed: {error}")).await;
        }
    }
}

impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> jsonrpc::Result<InitializeResult> {
        let capabilities = &params.capabilities;
        let encodings = capabilities.general.as_ref().and_then(|g| g.position_encodings.as_deref());
        let encoding = Encoding::choose(encodings);
        let _ = self.with_workspace(|workspace| workspace.encoding = encoding).await;
        let refresh = capabilities
            .workspace
            .as_ref()
            .and_then(|w| w.diagnostics.as_ref())
            .and_then(|d| d.refresh_support)
            .unwrap_or(false);
        self.refresh_support.store(refresh, Ordering::Relaxed);
        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: LANGUAGE_SERVER_NAME.to_string(),
                version: Some(LANGUAGE_SERVER_VERSION.to_string()),
            }),
            capabilities: ServerCapabilities {
                position_encoding: Some(encoding.kind()),
                text_document_sync: Some(TextDocumentSyncCapability::Options(TextDocumentSyncOptions {
                    open_close: Some(true),
                    change: Some(TextDocumentSyncKind::FULL),
                    ..Default::default()
                })),
                diagnostic_provider: Some(DiagnosticServerCapabilities::Options(DiagnosticOptions {
                    identifier: Some("bolt".to_string()),
                    // Imports and labels make diagnostics depend on other files.
                    inter_file_dependencies: true,
                    workspace_diagnostics: false,
                    ..Default::default()
                })),
                ..ServerCapabilities::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client.log_message(MessageType::INFO, "Bolt language server initialized").await;
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let Some(path) = self.path_of(&params.text_document.uri).await else { return };
        let _ = self.with_workspace(|workspace| workspace.set_text(&path, params.text_document.text)).await;
        self.refresh_diagnostics().await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let Some(path) = self.path_of(&params.text_document.uri).await else { return };
        // With full sync, the last change is the whole document.
        let Some(change) = params.content_changes.into_iter().last() else { return };
        let _ = self.with_workspace(|workspace| workspace.set_text(&path, change.text)).await;
        self.refresh_diagnostics().await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let Some(path) = self.path_of(&params.text_document.uri).await else { return };
        let _ = self.with_workspace(|workspace| workspace.close(&path)).await;
        self.refresh_diagnostics().await;
    }

    async fn diagnostic(&self, params: DocumentDiagnosticParams) -> jsonrpc::Result<DocumentDiagnosticReportResult> {
        let items = match self.path_of(&params.text_document.uri).await {
            Some(path) => self.with_workspace(|workspace| workspace.diagnostics(&path)).await.map_err(|message| {
                let mut error = jsonrpc::Error::internal_error();
                error.message = message.into();
                error
            })?,
            None => Vec::new(),
        };
        Ok(DocumentDiagnosticReportResult::Report(DocumentDiagnosticReport::Full(
            RelatedFullDocumentDiagnosticReport {
                related_documents: None,
                full_document_diagnostic_report: FullDocumentDiagnosticReport { result_id: None, items },
            },
        )))
    }

    async fn shutdown(&self) -> jsonrpc::Result<()> {
        Ok(())
    }
}

/// Run `f` on what `mutex` holds, catching a panic in it. The lock is
/// released normally then, so it isn't poisoned and later requests still
/// work; a lock poisoned some other way is used anyway.
fn guarded<W, T>(mutex: &Mutex<W>, f: impl FnOnce(&mut W) -> T) -> Result<T, String> {
    let mut value = mutex.lock().unwrap_or_else(PoisonError::into_inner);
    panic::catch_unwind(AssertUnwindSafe(|| f(&mut value))).map_err(|payload| panic_message(&*payload))
}

fn panic_message(payload: &(dyn Any + Send)) -> String {
    match (payload.downcast_ref::<&str>(), payload.downcast_ref::<String>()) {
        (Some(message), _) => message.to_string(),
        (_, Some(message)) => message.clone(),
        _ => "the server panicked".to_string(),
    }
}

#[tokio::main]
async fn main() {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
    let workspace = Mutex::new(Workspace::new(SystemPath::new(&cwd)));
    let (service, socket) =
        LspService::new(|client| Backend { client, workspace, refresh_support: AtomicBool::new(false) });
    Server::new(tokio::io::stdin(), tokio::io::stdout(), socket).serve(service).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_panic_fails_only_its_own_call() {
        let mutex = Mutex::new(0);
        let result: Result<(), _> = guarded(&mutex, |_| panic!("oops"));
        assert_eq!(result, Err("oops".to_string()));
        assert!(!mutex.is_poisoned());
        assert_eq!(guarded(&mutex, |n| { *n += 1; *n }), Ok(1));
    }
}

//! The protocol without a transport: what the server answers to the
//! requests and notifications it supports. The binary runs it over stdio
//! with tower-lsp, and `bolt-language-server-wasm` in the browser.

use std::{fmt, sync::Arc};

use boltlang_base_db::{System, SystemPath, SystemPathBuf};
use ls_types::{
    DiagnosticOptions, DiagnosticServerCapabilities, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DocumentDiagnosticParams, DocumentDiagnosticReport, DocumentDiagnosticReportResult,
    FullDocumentDiagnosticReport, InitializeParams, InitializeResult, RelatedFullDocumentDiagnosticReport,
    ServerCapabilities, ServerInfo, TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions, Uri,
};

use crate::{position::Encoding, workspace::Workspace};

pub const NAME: &str = "bolt-language-server";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// A document the server can't work with, because it's not a file, like an
/// `untitled:` one. The frontend logs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotAFile(pub Uri);

impl fmt::Display for NotAFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "only files are supported, not {}", self.0.as_str())
    }
}

pub struct Server {
    workspace: Workspace,
    /// Whether the client can be asked to pull the diagnostics of all its
    /// documents again, which an edit of one can change.
    refresh_support: bool,
}

impl Server {
    /// A server whose files that aren't open come from `disk`.
    pub fn new(disk: Arc<dyn System>) -> Self {
        Server { workspace: Workspace::new(disk), refresh_support: false }
    }

    pub fn initialize(&mut self, params: &InitializeParams) -> InitializeResult {
        let capabilities = &params.capabilities;
        let encodings = capabilities.general.as_ref().and_then(|g| g.position_encodings.as_deref());
        let encoding = Encoding::choose(encodings);
        self.workspace.encoding = encoding;
        self.refresh_support = capabilities
            .workspace
            .as_ref()
            .and_then(|w| w.diagnostics.as_ref())
            .and_then(|d| d.refresh_support)
            .unwrap_or(false);
        InitializeResult {
            server_info: Some(ServerInfo { name: NAME.to_string(), version: Some(VERSION.to_string()) }),
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
        }
    }

    /// Whether to ask the client for new diagnostics of all its documents
    /// after a change, with `workspace/diagnostic/refresh`.
    pub fn should_refresh(&self) -> bool {
        self.refresh_support
    }

    pub fn did_open(&mut self, params: DidOpenTextDocumentParams) -> Result<(), NotAFile> {
        let path = path_of(&params.text_document.uri)?;
        self.workspace.set_text(&path, params.text_document.text);
        Ok(())
    }

    pub fn did_change(&mut self, params: DidChangeTextDocumentParams) -> Result<(), NotAFile> {
        let path = path_of(&params.text_document.uri)?;
        // With full sync, the last change is the whole document.
        if let Some(change) = params.content_changes.into_iter().last() {
            self.workspace.set_text(&path, change.text);
        }
        Ok(())
    }

    pub fn did_close(&mut self, params: DidCloseTextDocumentParams) -> Result<(), NotAFile> {
        let path = path_of(&params.text_document.uri)?;
        self.workspace.close(&path);
        Ok(())
    }

    /// The file at `path` changed, without being open.
    pub fn file_changed(&mut self, path: &SystemPath) {
        self.workspace.file_changed(path);
    }

    /// The diagnostics of a document. A document that isn't a file has none.
    pub fn diagnostic(&self, params: &DocumentDiagnosticParams) -> (DocumentDiagnosticReportResult, Option<NotAFile>) {
        let (items, error) = match path_of(&params.text_document.uri) {
            Ok(path) => (self.workspace.diagnostics(&path), None),
            Err(error) => (Vec::new(), Some(error)),
        };
        let report = DocumentDiagnosticReportResult::Report(DocumentDiagnosticReport::Full(
            RelatedFullDocumentDiagnosticReport {
                related_documents: None,
                full_document_diagnostic_report: FullDocumentDiagnosticReport { result_id: None, items },
            },
        ));
        (report, error)
    }
}

/// The path of the file a document is, from its `file:` URI.
pub fn path_of(uri: &Uri) -> Result<SystemPathBuf, NotAFile> {
    match uri.to_file_path() {
        Some(path) => Ok(SystemPathBuf::from(path.into_owned())),
        None => Err(NotAFile(uri.clone())),
    }
}

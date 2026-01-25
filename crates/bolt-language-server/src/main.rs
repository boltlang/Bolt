use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::Mutex;

use boltlang::{RootDatabase, File, Diagnostic, index_lines, line_column_of_offset, parse_file};
use boltlang::salsa::Database;
use tower_lsp_server::{LspService, jsonrpc, ls_types};
use tower_lsp_server::ls_types::{DiagnosticSeverity, DocumentDiagnosticParams, DocumentDiagnosticReport, DocumentDiagnosticReportResult, FullDocumentDiagnosticReport, InitializeResult, InitializedParams, MessageType, RelatedFullDocumentDiagnosticReport, ServerCapabilities, ServerInfo, Uri, WorkspaceDiagnosticReport, WorkspaceDiagnosticReportResult, WorkspaceDocumentDiagnosticReport, WorkspaceFullDocumentDiagnosticReport};
use tower_lsp_server::{Client, LanguageServer, ls_types::InitializeParams};
use tower_lsp_server::jsonrpc::Result;

const LANGUAGE_SERVER_NAME: &str = "bolt-language-server";
const LANGUAGE_SERVER_VERSION: &str = "0.0.1";

pub struct Backend {
    client: Client,
    db: Mutex<RootDatabase>,
    root_dir: Option<PathBuf>,
}

fn local_file(uri: &Uri) -> &str {
    debug_assert!(uri.scheme().as_str() == "file");
    uri.path().as_str()
}

const E_IO_ERROR: i64 = 1;

impl LanguageServer for Backend {

    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                version: Some(LANGUAGE_SERVER_VERSION.to_string()),
                name: LANGUAGE_SERVER_NAME.to_string(),
            }),
            capabilities: ServerCapabilities {
                // diagnostic_provider: Some(ls_types::DiagnosticServerCapabilities::Options(
                //     ls_types::DiagnosticOptions {
                //         identifier: None,
                //         inter_file_dependencies: false, // TODO
                //         workspace_diagnostics: true,
                //         work_done_progress_options: WorkDoneProgressOptions {
                //             work_done_progress: Some(false),
                //         },
                //     }
                // )),
                ..ServerCapabilities::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client.log_message(MessageType::INFO, "Bolt language server initialized").await;
    }

    async fn diagnostic(&self, params: DocumentDiagnosticParams) -> Result<DocumentDiagnosticReportResult> {
        let path  = local_file(&params.text_document.uri);
        let items = self.diagnostics_for_file(path).await?;
        self.client.log_message(MessageType::ERROR, format!("{:?}", params.text_document)).await;
        Ok(DocumentDiagnosticReportResult::Report(DocumentDiagnosticReport::Full(
            RelatedFullDocumentDiagnosticReport {
                related_documents: None,
                full_document_diagnostic_report: FullDocumentDiagnosticReport {
                    result_id: None,
                    items
                }
            }
        )))
    }

    async fn workspace_diagnostic(&self, params: ls_types::WorkspaceDiagnosticParams) -> Result<ls_types::WorkspaceDiagnosticReportResult> {
        // let roots = {
        //     let db = self.db.lock().unwrap();
        //     root_paths(db, self.root_dir.unwrap())
        // };
        // let items = futures::future::try_join_all(
        //     roots
        //         .iter()
        //         .map(async |path| WorkspaceDocumentDiagnosticReport::Full(WorkspaceFullDocumentDiagnosticReport {
        //             uri: Uri::from_file_path(path).ok_or(server_error(format!("could not find {} on the file system", path)))?,
        //             version: None, // FIXME
        //             full_document_diagnostic_report: FullDocumentDiagnosticReport {
        //                 result_id: None,
        //                 items: self.diagnostics_for_file(path).await?,
        //             }
        //         }))
        // ).await?;
        // TODO process errors
        Ok(WorkspaceDiagnosticReportResult::Report(WorkspaceDiagnosticReport {
            items: vec![],
        }))
    }

    async  fn shutdown(&self) -> Result<()> {
        Ok(())
    }

}

fn server_error(code: i64, message: String) -> jsonrpc::Error {
    jsonrpc::Error {
        code: jsonrpc::ErrorCode::ServerError(code),
        data: None,
        message: Cow::Owned(message),
    }
}

impl Backend {

    async fn diagnostics_for_file(&self, path: &str) -> Result<Vec<ls_types::Diagnostic>> {
        let text = match tokio::fs::read_to_string(path).await {
            Ok(text) => text,
            Err(err) => {
                return Err(server_error(
                    E_IO_ERROR,
                    format!("failed to read file {}: {}", path, err)
                ));
            }
        };
        Ok(self.db.lock().unwrap().attach(|db| {

            let source = db.input(PathBuf::from(path)).unwrap();
            parse_file(db, source);

            macro_rules! into_u32 {
                ($expr:expr) => {
                    match $expr.try_into() {
                        Err(_) => return None,
                        Ok(value) => value,
                    }
                };
            }

            let index = index_lines(db, source);

            parse_file::accumulated::<Diagnostic>(db, source)
                .into_iter()
                .filter_map(|e| {
                    let start = line_column_of_offset(db, index, e.offset()?);
                    Some(ls_types::Diagnostic {
                        code: Some(ls_types::NumberOrString::Number(e.code().into())),
                        code_description: None, // TODO
                        data: None,
                        message: format!("{}", e),
                        range: ls_types::Range {
                            start: ls_types::Position {
                                line: into_u32!(start.line),
                                character: into_u32!(start.column),
                            },
                            end: ls_types::Position {
                                line: into_u32!(start.line),
                                character: into_u32!(start.column)
                            }
                        },
                        related_information: None, // TODO
                        severity: Some(DiagnosticSeverity::ERROR),
                        source: Some(LANGUAGE_SERVER_NAME.to_string()),
                        tags: None,
                    })
                })
                .collect()

        }))

    }

}

#[tokio::main]
async fn main() {
    let db = Mutex::new(RootDatabase::new(None));
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(|client| Backend {
        client,
        db,
        root_dir: None
    });
    tower_lsp_server::Server::new(stdin, stdout, socket).serve(service).await;
}

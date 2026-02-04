use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::Mutex;

use boltlang::{Db, DbDiagnostic, LineColumn, WritableSystem, check_file, index_lines, parse_file};
use boltlang::salsa::Database;
use tower_lsp_server::{LspService, jsonrpc, ls_types};
use tower_lsp_server::ls_types::{DiagnosticSeverity, DocumentDiagnosticParams, DocumentDiagnosticReport, DocumentDiagnosticReportResult, FullDocumentDiagnosticReport, InitializeResult, InitializedParams, MessageType, RelatedFullDocumentDiagnosticReport, ServerCapabilities, ServerInfo, Uri};
use tower_lsp_server::{Client, LanguageServer, ls_types::InitializeParams};

use crate::db::LspDatabase;

mod db;

const LANGUAGE_SERVER_NAME: &str = "bolt-language-server";
const LANGUAGE_SERVER_VERSION: &str = "0.0.1";

pub struct Backend {
    client: Client,
    db: Mutex<LspDatabase>,
    root_dir: Option<PathBuf>,
}

const E_IO_ERROR: i64 = 1;
const E_FILE_NOT_FOUND: i64 = 2;
const E_COMPILER_ERROR: i64 = 3;

macro_rules! logged {
    ($self:ident, $expr:expr) => {
        {
            let result = $expr;
            if let Err(error) = result {
                $self.client.log_message(MessageType::ERROR, format!("{}", error)).await;
                return;
            }
        }
    };
}

impl LanguageServer for Backend {

    async fn initialize(&self, _: InitializeParams) -> jsonrpc::Result<InitializeResult> {
        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                version: Some(LANGUAGE_SERVER_VERSION.to_string()),
                name: LANGUAGE_SERVER_NAME.to_string(),
            }),
            capabilities: ServerCapabilities {
                text_document_sync: Some(ls_types::TextDocumentSyncCapability::Options(
                    ls_types::TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(ls_types::TextDocumentSyncKind::FULL), // TODO INCREMENTAL
                        will_save: Some(false),
                        will_save_wait_until: Some(false),
                        save: Some(ls_types::TextDocumentSyncSaveOptions::Supported(true)),
                    }
                )),
                diagnostic_provider: Some(ls_types::DiagnosticServerCapabilities::Options(
                    ls_types::DiagnosticOptions {
                        identifier: None,
                        inter_file_dependencies: false, // TODO
                        workspace_diagnostics: true,
                        work_done_progress_options: ls_types::WorkDoneProgressOptions {
                            work_done_progress: Some(false),
                        },
                    }
                )),
                ..ServerCapabilities::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client.log_message(MessageType::INFO, "Bolt language server initialized").await;
    }
    
    async fn did_open(&self, params: ls_types::DidOpenTextDocumentParams) {
        logged!(self, self.db.lock().unwrap().system.write_file_bytes_virtual(
            params.text_document.uri.as_str(),
            params.text_document.text.as_bytes()
        ));
    }

    async fn did_change(&self, params: ls_types::DidChangeTextDocumentParams) {
        let result: boltlang::Result<()> = {
            let db = self.db.lock().unwrap();
            let path = params.text_document.uri.as_str();
            let file = db.files().resolve_virtual(&*db, path);
            db.attach(|db| {
                let real_path = file.path(db).clone();
                let mut contents = file.read_to_string(db)?;
                let index = index_lines(db, file).lines(db);
                for change in params.content_changes {
                    match change.range {
                        Some(range) => {
                            let start = index.offset_from_line_column(&LineColumn::new(
                                    range.start.line.try_into().unwrap(),
                                    range.start.character.try_into().unwrap()
                                ));
                            let end = index.offset_from_line_column(
                                    &LineColumn::new(range.end.line.try_into().unwrap(),
                                    range.end.character.try_into().unwrap()
                                ));
                            contents.replace_range(start..end, &change.text);
                        },
                        None => contents = change.text,
                    }
                }
                db.system.write_file_bytes_virtual(path, contents.as_bytes())?;
                Ok(())
            })
        };
        if let Err(error) = result {
            self.client.log_message(MessageType::ERROR, error).await;
        }
        // self.client.publish_diagnostics(
        //     params.text_document.uri.clone(),
        //     logged!(self.diagnostics_for_file(&params.text_document.uri).await),
        //     None,
        // ).await;
    }

    async fn did_save(&self, params: ls_types::DidSaveTextDocumentParams) {


        macro_rules! logged {
            ($expr:expr) => {
                {
                    let result = $expr;
                    match result {
                        Ok(value) => value,
                        Err(error) => {
                            self.client.log_message(MessageType::ERROR, format!("{}", error)).await;
                            return;
                        }
                    }
                }
            };
        }

        if let Some(text) = params.text {
            logged!(self.db.lock().unwrap().system.write_file_bytes_virtual(params.text_document.uri.as_str(), text.as_bytes()));
        }

    }

    async fn diagnostic(&self, params: DocumentDiagnosticParams) -> jsonrpc::Result<DocumentDiagnosticReportResult> {
        let items = self.diagnostics_for_file(&params.text_document.uri).await?;
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

    // async fn workspace_diagnostic(&self, params: ls_types::WorkspaceDiagnosticParams) -> Result<ls_types::WorkspaceDiagnosticReportResult> {
    //     // let roots = {
    //     //     let db = self.db.lock().unwrap();
    //     //     root_paths(db, self.root_dir.unwrap())
    //     // };
    //     // let items = futures::future::try_join_all(
    //     //     roots
    //     //         .iter()
    //     //         .map(async |path| WorkspaceDocumentDiagnosticReport::Full(WorkspaceFullDocumentDiagnosticReport {
    //     //             uri: Uri::from_file_path(path).ok_or(server_error(format!("could not find {} on the file system", path)))?,
    //     //             version: None, // FIXME
    //     //             full_document_diagnostic_report: FullDocumentDiagnosticReport {
    //     //                 result_id: None,
    //     //                 items: self.diagnostics_for_file(path).await?,
    //     //             }
    //     //         }))
    //     // ).await?;
    //     // TODO process errors
    //     self.client.log_message(MessageType::ERROR, "TEEEEEEST").await;
    //     let pos = ls_types::Position { line: 0, character: 0 };
    //     Ok(WorkspaceDiagnosticReportResult::Report(WorkspaceDiagnosticReport {
    //         items: vec![
    //             WorkspaceDocumentDiagnosticReport::Full(WorkspaceFullDocumentDiagnosticReport {
    //                 uri: Uri::from_str("file:///home/samvv/Projects/bolt/test.bolt").unwrap(),
    //                 version: None,
    //                 full_document_diagnostic_report: FullDocumentDiagnosticReport {
    //                     result_id: None,
    //                     items: vec![
    //                         ls_types::Diagnostic {
    //                             code: None,
    //                             code_description: None,
    //                             severity: Some(DiagnosticSeverity::ERROR),
    //                             message: "This is a test".to_string(),
    //                             data: None,
    //                             source: None,
    //                             related_information: None,
    //                             tags: None,
    //                             range: ls_types::Range { start: pos, end: pos },
    //                         },
    //                     ]
    //                 },
    //             })
    //         ],
    //     }))
    // }

    async  fn shutdown(&self) -> jsonrpc::Result<()> {
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

fn from_boltlang_error(error: boltlang::Error) -> jsonrpc::Error {
    server_error(E_COMPILER_ERROR, format!("{}", error))
}

impl Backend {

    async fn diagnostics_for_file(&self, uri: &Uri) -> jsonrpc::Result<Vec<ls_types::Diagnostic>> {

        self.db.lock().unwrap().attach(|db| {

            let file = db.files().resolve_virtual(db, uri.as_str());
            let _result = check_file(db, file);
            let index = index_lines(db, file).lines(db);

            Ok(parse_file::accumulated::<DbDiagnostic>(db, file)
                .into_iter()
                .filter_map(|e| {
                    let source = e.source()?;
                    let start = index.line_column_of_offset(source.span().start);
                    let end = index.line_column_of_offset(source.span().end);
                    Some(ls_types::Diagnostic {
                        code: Some(ls_types::NumberOrString::Number(e.code().into())),
                        code_description: None, // TODO
                        data: None,
                        message: format!("{}", e),
                        range: ls_types::Range {
                            start: ls_types::Position {
                                line: start.line.try_into().ok()?,
                                character: start.column.try_into().ok()?,
                            },
                            end: ls_types::Position {
                                line: end.line.try_into().ok()?,
                                character: end.column.try_into().ok()?,
                            }
                        },
                        related_information: None, // TODO
                        severity: Some(DiagnosticSeverity::ERROR),
                        source: Some(LANGUAGE_SERVER_NAME.to_string()),
                        tags: None,
                    })
                })
                .collect())

        })

    }

}

#[tokio::main]
async fn main() {
    let db = Mutex::new(LspDatabase::default());
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(|client| Backend {
        client,
        db,
        root_dir: None
    });
    tower_lsp_server::Server::new(stdin, stdout, socket).serve(service).await;
}

//! The language server over stdio, with tower-lsp. In the browser, where
//! there's no stdio, `bolt-language-server-wasm` runs the same [`Server`].

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::{
        any::Any,
        panic::{self, AssertUnwindSafe},
        path::PathBuf,
        sync::{Arc, Mutex, PoisonError},
    };

    use bolt_language_server::{NotAFile, Server};
    use boltlang_base_db::{OsSystem, SystemPath, SystemPathBuf};
    use tower_lsp_server::{
        Client, LanguageServer, LspService, jsonrpc,
        ls_types::{
            DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
            DocumentDiagnosticParams, DocumentDiagnosticReportResult, InitializeParams, InitializeResult,
            InitializedParams, MessageType, WorkspaceDiagnosticParams, WorkspaceDiagnosticReportResult,
        },
    };

    struct Backend {
        client: Client,
        server: Mutex<Server>,
    }

    impl Backend {
        /// Run `f` on the server. A panic in it, like a bug in the type
        /// checker, fails only this request: it's logged and returned as an
        /// error.
        async fn with_server<T>(&self, f: impl FnOnce(&mut Server) -> T) -> Result<T, String> {
            let result = guarded(&self.server, f);
            if let Err(message) = &result {
                self.client.log_message(MessageType::ERROR, format!("internal error: {message}")).await;
            }
            result
        }

        async fn warn_not_a_file(&self, error: NotAFile) {
            self.client.log_message(MessageType::WARNING, error.to_string()).await;
        }

        /// After a change: log what went wrong, warn about files that the
        /// open documents use that have newly got errors, and ask the client
        /// for new diagnostics of every document.
        async fn changed(&self, result: Result<Result<(), NotAFile>, String>) {
            if let Ok(Err(error)) = result {
                self.warn_not_a_file(error).await;
            }
            for notice in self.with_server(|s| s.notices()).await.unwrap_or_default() {
                self.client.show_message(MessageType::WARNING, notice).await;
            }
            let refresh = guarded(&self.server, |s| s.should_refresh()).unwrap_or(false);
            if !refresh {
                return;
            }
            if let Err(error) = self.client.workspace_diagnostic_refresh().await {
                self.client.log_message(MessageType::ERROR, format!("refreshing diagnostics failed: {error}")).await;
            }
        }
    }

    impl LanguageServer for Backend {
        async fn initialize(&self, params: InitializeParams) -> jsonrpc::Result<InitializeResult> {
            self.with_server(|s| s.initialize(&params)).await.map_err(internal_error)
        }

        async fn initialized(&self, _: InitializedParams) {
            self.client.log_message(MessageType::INFO, "Bolt language server initialized").await;
        }

        async fn did_open(&self, params: DidOpenTextDocumentParams) {
            let result = self.with_server(|s| s.did_open(params)).await;
            self.changed(result).await;
        }

        async fn did_change(&self, params: DidChangeTextDocumentParams) {
            let result = self.with_server(|s| s.did_change(params)).await;
            self.changed(result).await;
        }

        async fn did_close(&self, params: DidCloseTextDocumentParams) {
            let result = self.with_server(|s| s.did_close(params)).await;
            self.changed(result).await;
        }

        async fn diagnostic(&self, params: DocumentDiagnosticParams) -> jsonrpc::Result<DocumentDiagnosticReportResult> {
            let (report, error) = self.with_server(|s| s.diagnostic(&params)).await.map_err(internal_error)?;
            if let Some(error) = error {
                self.warn_not_a_file(error).await;
            }
            Ok(report)
        }

        async fn workspace_diagnostic(
            &self,
            _: WorkspaceDiagnosticParams,
        ) -> jsonrpc::Result<WorkspaceDiagnosticReportResult> {
            self.with_server(|s| s.workspace_diagnostic()).await.map_err(internal_error)
        }

        async fn shutdown(&self) -> jsonrpc::Result<()> {
            Ok(())
        }
    }

    fn internal_error(message: String) -> jsonrpc::Error {
        let mut error = jsonrpc::Error::internal_error();
        error.message = message.into();
        error
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
    pub async fn main() {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        let mut server = Server::new(Arc::new(OsSystem::new(SystemPath::new(&cwd))));
        // The `std` that comes with the compiler. Without one, `std` is looked
        // up in `.bolt/packages`; stderr is where the editor logs it.
        match boltlang_std::locate() {
            Ok((dir, _)) => server.set_std_dir(Some(SystemPathBuf::from(dir))),
            Err(error) => eprintln!("warning: {error}"),
        }
        let server = Mutex::new(server);
        let (service, socket) = LspService::new(|client| Backend { client, server });
        tower_lsp_server::Server::new(tokio::io::stdin(), tokio::io::stdout(), socket).serve(service).await;
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
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    native::main();
}

// There's no stdio in the browser; see `bolt-language-server-wasm`.
#[cfg(target_arch = "wasm32")]
fn main() {}

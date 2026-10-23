//! What the server knows about the files it works on, without the protocol
//! around it: the documents open in the editor, and their diagnostics.

use boltlang_base_db::{Db, File, FilePath, SystemPath, source_text};
use boltlang_ide_diagnostics::{Severity, diagnostics};
use tower_lsp_server::ls_types::{self, DiagnosticRelatedInformation, DiagnosticSeverity, Location, NumberOrString, Uri};

use crate::{
    db::LspDatabase,
    position::{Encoding, LineIndex},
};

pub struct Workspace {
    db: LspDatabase,
    pub encoding: Encoding,
}

impl Workspace {
    pub fn new(cwd: &SystemPath) -> Self {
        Workspace { db: LspDatabase::new(cwd), encoding: Encoding::Utf16 }
    }

    /// Use `text` for the document at `path`, when it's opened or edited.
    pub fn set_text(&mut self, path: &SystemPath, text: String) {
        self.db.system.set_text(path, text);
        self.sync(path);
    }

    /// Go back to what's on disk when the document at `path` is closed.
    pub fn close(&mut self, path: &SystemPath) {
        self.db.system.remove_text(path);
        self.sync(path);
    }

    /// Let Salsa know that the file at `path` changed.
    fn sync(&mut self, path: &SystemPath) {
        let file = self.db.files().resolve(&self.db, path);
        file.sync(&mut self.db);
    }

    /// The diagnostics of the file at `path`, in the protocol's terms.
    pub fn diagnostics(&self, path: &SystemPath) -> Vec<ls_types::Diagnostic> {
        let db = &self.db;
        let file = db.files().resolve(db, path);
        if !file.exists(db) {
            return Vec::new();
        }
        let text = source_text(db, file);
        let index = LineIndex::new(text);
        diagnostics(db, file)
            .into_iter()
            .map(|d| {
                let related: Vec<_> = d
                    .labels
                    .iter()
                    .filter_map(|label| {
                        let uri = uri_of(db, label.file)?;
                        let text = source_text(db, label.file);
                        let range = LineIndex::new(text).range(label.range, self.encoding);
                        Some(DiagnosticRelatedInformation { location: Location { uri, range }, message: label.message.clone() })
                    })
                    .collect();
                ls_types::Diagnostic {
                    range: index.range(d.range, self.encoding),
                    severity: Some(match d.severity {
                        Severity::Error => DiagnosticSeverity::ERROR,
                        Severity::Warning => DiagnosticSeverity::WARNING,
                    }),
                    code: Some(NumberOrString::String(d.code.to_string())),
                    code_description: None,
                    source: Some("bolt".to_string()),
                    message: d.message,
                    related_information: (!related.is_empty()).then_some(related),
                    tags: None,
                    data: None,
                }
            })
            .collect()
    }
}

fn uri_of(db: &dyn Db, file: File) -> Option<Uri> {
    match file.path(db) {
        FilePath::System(path) => Uri::from_file_path(path.as_std_path()),
        FilePath::SystemVirtual(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Paths that don't exist on disk, so that only the open documents count.
    const MAIN: &str = "/nonexistent/bolt-language-server-test/main.bolt";
    const LIB: &str = "/nonexistent/bolt-language-server-test/lib.bolt";

    fn messages(workspace: &Workspace, path: &str) -> Vec<String> {
        workspace.diagnostics(SystemPath::new(path)).into_iter().map(|d| d.message).collect()
    }

    #[test]
    fn diagnostics_follow_edits() {
        let mut workspace = Workspace::new(SystemPath::new("/"));
        workspace.set_text(SystemPath::new(MAIN), "let x: i32 = true;\n".into());
        assert_eq!(messages(&workspace, MAIN), ["expected `i32`, found `bool`"]);
        workspace.set_text(SystemPath::new(MAIN), "let x: i32 = 1;\n".into());
        assert!(messages(&workspace, MAIN).is_empty());
        // A closed document that isn't on disk has no diagnostics.
        workspace.close(SystemPath::new(MAIN));
        assert!(messages(&workspace, MAIN).is_empty());
    }

    #[test]
    fn open_documents_can_import_each_other() {
        let mut workspace = Workspace::new(SystemPath::new("/"));
        workspace.set_text(SystemPath::new(MAIN), "import \"./lib\" (negate);\nnegate(1);\n".into());
        assert_eq!(messages(&workspace, MAIN), ["cannot find the file `./lib`", "cannot find `negate`"]);
        workspace.set_text(SystemPath::new(LIB), "pub fn negate(x) = !x;\n".into());
        let found = workspace.diagnostics(SystemPath::new(MAIN));
        assert_eq!(found.iter().map(|d| d.message.as_str()).collect::<Vec<_>>(), ["expected `bool`, found an integer"]);
        // The label points at the parameter in the other file.
        let related = found[0].related_information.as_ref().unwrap();
        assert!(related[0].location.uri.as_str().ends_with("lib.bolt"));
        assert_eq!(related[0].message, "expected because of this parameter of `negate`");
        assert_eq!(related[0].location.range.start, ls_types::Position { line: 0, character: 14 });
    }
}

//! What the server knows about the files it works on, without the protocol
//! around it: the documents open in the editor, and their diagnostics.

use std::sync::Arc;

use boltlang_base_db::{Db, File, FilePath, Settings, System, SystemPath, source_text};
use boltlang_ide_diagnostics::{Severity, diagnostics, files_used_by, short_path};
use ls_types::{DiagnosticRelatedInformation, DiagnosticSeverity, Location, NumberOrString, Uri};

use crate::{
    db::LspDatabase,
    position::{Encoding, LineIndex},
};

pub struct Workspace {
    db: LspDatabase,
    pub encoding: Encoding,
    /// The files whose diagnostics the last workspace report gave, so that
    /// the next one can clear those that aren't used anymore.
    reported: Vec<File>,
    /// The files that open documents use with errors that a notice was
    /// already given for, so that it's given once, until they're fixed.
    noticed: Vec<File>,
}

impl Workspace {
    /// A workspace whose files that aren't open come from `disk`.
    pub fn new(disk: Arc<dyn System>) -> Self {
        Workspace { db: LspDatabase::new(disk), encoding: Encoding::Utf16, reported: Vec::new(), noticed: Vec::new() }
    }

    /// Use the prelude of `prelude`, or none. A prelude asked for by name
    /// that can't be found is an error.
    pub fn set_prelude(&mut self, prelude: Option<String>) {
        let required = prelude.is_some();
        Settings::change_prelude(&mut self.db, prelude, required);
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

    /// Let Salsa know that the file at `path` changed, like a file that
    /// isn't open and was written to the disk.
    pub fn file_changed(&mut self, path: &SystemPath) {
        self.sync(path);
    }

    fn sync(&mut self, path: &SystemPath) {
        let file = self.db.files().resolve(&self.db, path);
        file.sync(&mut self.db);
    }

    /// The files that the open documents use, through their prelude or
    /// their imports, that aren't open themselves (#249). Their errors are
    /// the open documents' business too, but the editor doesn't ask for
    /// them document by document.
    fn used_files(&self) -> Vec<File> {
        let db = &self.db;
        let open: Vec<File> = self.db.system.open_paths().iter().map(|p| db.files().resolve(db, p)).collect();
        let mut used = Vec::new();
        for &document in &open {
            for &file in files_used_by(db, document) {
                if !open.contains(&file) && !used.contains(&file) {
                    used.push(file);
                }
            }
        }
        used
    }

    /// The diagnostics of the files that the open documents use, for a
    /// workspace report, each with its URI. A file reported last time that
    /// isn't used anymore is given without any, so that the editor forgets
    /// them.
    pub fn workspace_diagnostics(&mut self) -> Vec<(Uri, Vec<ls_types::Diagnostic>)> {
        let used = self.used_files();
        let mut out = Vec::new();
        for &file in &used {
            out.extend(uri_of(&self.db, file).map(|uri| (uri, self.file_diagnostics(file))));
        }
        for &file in self.reported.iter().filter(|f| !used.contains(f)) {
            out.extend(uri_of(&self.db, file).map(|uri| (uri, Vec::new())));
        }
        self.reported = used;
        out
    }

    /// A message for each file that the open documents use that has newly
    /// got errors, for an editor that doesn't ask for the workspace's
    /// diagnostics: "`std/__prelude__/vec.bolt`, which the open files use,
    /// has 2 errors, the first on line 199: ...". It's given once until the
    /// file's errors are fixed.
    pub fn notices(&mut self) -> Vec<String> {
        let db = &self.db;
        let open = self.db.system.open_paths();
        let Some(from) = open.first().map(|p| db.files().resolve(db, p)) else {
            self.noticed.clear();
            return Vec::new();
        };
        let mut broken = Vec::new();
        let mut out = Vec::new();
        for file in self.used_files() {
            let errors: Vec<_> = diagnostics(db, file).into_iter().filter(|d| d.severity == Severity::Error).collect();
            let Some(first) = errors.first() else { continue };
            broken.push(file);
            if self.noticed.contains(&file) {
                continue;
            }
            let text = source_text(db, file);
            let line = text[..usize::from(first.range.start())].matches('\n').count() + 1;
            let count = if errors.len() == 1 { "1 error".to_string() } else { format!("{} errors", errors.len()) };
            out.push(format!(
                "`{}`, which the open files use, has {count}, the first on line {line}: {}",
                short_path(db, from, file),
                first.message
            ));
        }
        self.noticed = broken;
        out
    }

    /// The diagnostics of the file at `path`, in the protocol's terms.
    pub fn diagnostics(&self, path: &SystemPath) -> Vec<ls_types::Diagnostic> {
        let db = &self.db;
        let file = db.files().resolve(db, path);
        self.file_diagnostics(file)
    }

    fn file_diagnostics(&self, file: File) -> Vec<ls_types::Diagnostic> {
        let db = &self.db;
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
        FilePath::System(path) => file_uri(path),
        FilePath::SystemVirtual(_) => None,
    }
}

/// The `file:` URI of an absolute path. `Uri::from_file_path` asks the file
/// system for a path that `std` doesn't think is absolute, and in the
/// browser, `std` thinks no path is and there's no file system to ask.
fn file_uri(path: &SystemPath) -> Option<Uri> {
    if !path.is_absolute() {
        return None;
    }
    if cfg!(windows) {
        return Uri::from_file_path(path.as_std_path());
    }
    let mut uri = String::from("file://");
    for byte in path.as_std_path().to_str()?.bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~".contains(&byte) {
            uri.push(byte as char);
        } else {
            uri.push_str(&format!("%{byte:02X}"));
        }
    }
    uri.parse().ok()
}

#[cfg(test)]
mod tests {
    use boltlang_base_db::{InMemorySystem, WritableSystem};

    use super::*;

    const MAIN: &str = "/project/main.bolt";
    const LIB: &str = "/project/lib.bolt";

    fn workspace() -> (Workspace, InMemorySystem) {
        let disk = InMemorySystem::new();
        (Workspace::new(Arc::new(disk.clone())), disk)
    }

    fn messages(workspace: &Workspace, path: &str) -> Vec<String> {
        workspace.diagnostics(SystemPath::new(path)).into_iter().map(|d| d.message).collect()
    }

    #[test]
    fn diagnostics_follow_the_prelude() {
        let (mut workspace, disk) = workspace();
        disk.write_file_bytes(SystemPath::new("/project/.bolt/packages/std/__prelude__/lib.bolt"), b"pub fn id(x: i32) -> i32 = x;\n")
            .unwrap();
        workspace.set_text(SystemPath::new(MAIN), "id(1);\n".into());
        assert!(messages(&workspace, MAIN).is_empty());
        workspace.set_prelude(None);
        assert_eq!(messages(&workspace, MAIN), ["cannot find `id`"]);
        workspace.set_prelude(Some("mine".into()));
        assert_eq!(messages(&workspace, MAIN), [
            "cannot find the package `mine` of the prelude in `.bolt/packages`",
            "cannot find `id`"
        ]);
    }

    #[test]
    fn diagnostics_follow_edits() {
        let (mut workspace, _) = workspace();
        workspace.set_text(SystemPath::new(MAIN), "let x: i32 = True;\n".into());
        assert_eq!(messages(&workspace, MAIN), ["expected `i32`, found `Bool`"]);
        workspace.set_text(SystemPath::new(MAIN), "let x: i32 = 1;\n".into());
        assert!(messages(&workspace, MAIN).is_empty());
        // A closed document that isn't on disk has no diagnostics.
        workspace.close(SystemPath::new(MAIN));
        assert!(messages(&workspace, MAIN).is_empty());
    }

    #[test]
    fn open_documents_can_import_each_other() {
        let (mut workspace, _) = workspace();
        workspace.set_text(SystemPath::new(MAIN), "import \"./lib\" (negate);\nnegate(1);\n".into());
        assert_eq!(messages(&workspace, MAIN), ["cannot find the file `./lib`", "cannot find `negate`"]);
        workspace.set_text(SystemPath::new(LIB), "pub fn negate(x) = !x;\n".into());
        let found = workspace.diagnostics(SystemPath::new(MAIN));
        assert_eq!(found.iter().map(|d| d.message.as_str()).collect::<Vec<_>>(), ["expected `Bool`, found an integer"]);
        // The label points at the parameter in the other file.
        let related = found[0].related_information.as_ref().unwrap();
        assert!(related[0].location.uri.as_str().ends_with("lib.bolt"));
        assert_eq!(related[0].message, "expected because of this parameter of `negate`");
        assert_eq!(related[0].location.range.start, ls_types::Position { line: 0, character: 14 });
    }

    /// The messages of a workspace report, by the file name of each URI.
    fn workspace_messages(workspace: &mut Workspace) -> Vec<(String, Vec<String>)> {
        let name = |uri: &Uri| uri.as_str().rsplit('/').next().unwrap().to_string();
        let report = workspace.workspace_diagnostics();
        report.iter().map(|(uri, items)| (name(uri), items.iter().map(|d| d.message.clone()).collect())).collect()
    }

    #[test]
    fn errors_of_the_prelude_are_reported_where_they_are() {
        let (mut workspace, disk) = workspace();
        let prelude = SystemPath::new("/project/.bolt/packages/std/__prelude__/lib.bolt");
        disk.write_file_bytes(prelude, b"pub fn id(x: i32) -> i32 = True;
").unwrap();
        workspace.set_text(SystemPath::new(MAIN), "id(1);
".into());
        // Not in the document that uses it...
        assert!(messages(&workspace, MAIN).is_empty());
        // ...but in the prelude's file, and once in a notice.
        assert_eq!(workspace_messages(&mut workspace), [("lib.bolt".to_string(), vec!["expected `i32`, found `Bool`".to_string()])]);
        assert_eq!(workspace.notices(), [
            "`std/__prelude__/lib.bolt`, which the open files use, has 1 error, the first on line 1: expected `i32`, found `Bool`"
        ]);
        assert!(workspace.notices().is_empty());
        // Fixed, the report clears it, and breaking it again gives a new notice.
        disk.write_file_bytes(prelude, b"pub fn id(x: i32) -> i32 = x;
").unwrap();
        workspace.file_changed(prelude);
        assert_eq!(workspace_messages(&mut workspace), [("lib.bolt".to_string(), vec![])]);
        assert!(workspace.notices().is_empty());
        disk.write_file_bytes(prelude, b"pub fn id(x: i32) -> i32 = True;
").unwrap();
        workspace.file_changed(prelude);
        assert_eq!(workspace.notices().len(), 1);
        // Once no open document uses it, it's cleared one last time, then left out.
        workspace.close(SystemPath::new(MAIN));
        assert_eq!(workspace_messages(&mut workspace), [("lib.bolt".to_string(), vec![])]);
        assert!(workspace_messages(&mut workspace).is_empty());
        assert!(workspace.notices().is_empty());
    }

    #[test]
    fn an_open_file_isn_t_in_the_workspace_report() {
        let (mut workspace, _) = workspace();
        workspace.set_text(SystemPath::new(LIB), "pub fn negate(x: Bool) -> Bool = 1;
".into());
        workspace.set_text(SystemPath::new(MAIN), "import \"./lib\" (negate);\nnegate(True);\n".into());
        // Its errors are in its own document's diagnostics.
        assert!(workspace_messages(&mut workspace).is_empty());
        assert!(workspace.notices().is_empty());
        assert_eq!(messages(&workspace, LIB), ["expected `Bool`, found an integer"]);
    }

    #[test]
    fn file_uris() {
        let uri = |path: &str| file_uri(SystemPath::new(path)).map(|u| u.as_str().to_string());
        assert_eq!(uri("/project/main.bolt").as_deref(), Some("file:///project/main.bolt"));
        assert_eq!(uri("/a b/ü.bolt").as_deref(), Some("file:///a%20b/%C3%BC.bolt"));
        assert_eq!(uri("relative.bolt"), None);
        // It's the path again.
        let back = crate::path_of(&file_uri(SystemPath::new("/a b/ü.bolt")).unwrap()).unwrap();
        assert_eq!(back.as_path(), SystemPath::new("/a b/ü.bolt"));
    }

    #[test]
    fn files_that_aren_t_open_come_from_the_disk() {
        let (mut workspace, disk) = workspace();
        disk.write_file_bytes(SystemPath::new(LIB), b"pub fn negate(x: Bool) -> Bool = !x;\n").unwrap();
        workspace.set_text(SystemPath::new(MAIN), "import \"./lib\" (negate);\nnegate(1);\n".into());
        assert_eq!(messages(&workspace, MAIN), ["expected `Bool`, found an integer"]);
        // A change on the disk is seen once it's reported.
        disk.write_file_bytes(SystemPath::new(LIB), b"pub fn negate(x: i32) -> i32 = -x;\n").unwrap();
        workspace.file_changed(SystemPath::new(LIB));
        assert!(messages(&workspace, MAIN).is_empty());
    }
}

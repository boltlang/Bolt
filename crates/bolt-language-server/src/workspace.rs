//! What the server knows about the files it works on, without the protocol
//! around it: the documents open in the editor, and their diagnostics.

use std::{collections::HashMap, sync::Arc};

use boltlang_base_db::{Db, File, FilePath, Settings, System, SystemPath, SystemPathBuf, source_text};
use boltlang_ide_diagnostics::{
    DiagnosticCode, MANIFEST, Severity, diagnostics, files_used_by, manifest_diagnostics, package_manifest, short_path,
};
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
    /// The paths that documents were opened with, by their real path, for
    /// those opened through a symbolic link. A file is one [`File`] at its
    /// real path (#238), but the editor knows the document by the path it
    /// opened.
    opened: HashMap<SystemPathBuf, SystemPathBuf>,
}

impl Workspace {
    /// A workspace whose files that aren't open come from `disk`.
    pub fn new(disk: Arc<dyn System>) -> Self {
        Workspace {
            db: LspDatabase::new(disk),
            encoding: Encoding::Utf16,
            reported: Vec::new(),
            noticed: Vec::new(),
            opened: HashMap::new(),
        }
    }

    /// Use the prelude of `prelude`, or none. A prelude asked for by name
    /// that can't be found is an error.
    pub fn set_prelude(&mut self, prelude: Option<String>) {
        let required = prelude.is_some();
        Settings::change_prelude(&mut self.db, prelude, required);
    }

    /// Use the `std` in `dir`, the one that comes with the compiler, or
    /// look it up in `.bolt/packages` without one.
    pub fn set_std_dir(&mut self, dir: Option<SystemPathBuf>) {
        Settings::change_std_dir(&mut self.db, dir);
    }

    /// Use `text` for the document at `path`, when it's opened or edited.
    pub fn set_text(&mut self, path: &SystemPath, text: String) {
        let real = self.real_path(path);
        if real.as_path() != path {
            self.opened.insert(real.clone(), path.to_system_path_buf());
        }
        self.db.system.set_text(&real, text);
        self.sync(&real);
    }

    /// Go back to what's on disk when the document at `path` is closed.
    pub fn close(&mut self, path: &SystemPath) {
        let real = self.real_path(path);
        self.opened.remove(&real);
        self.db.system.remove_text(&real);
        self.sync(&real);
    }

    /// The path of the file at `path`, with symbolic links resolved: where
    /// the text of a document is kept. A document that isn't on disk yet
    /// keeps its path.
    fn real_path(&self, path: &SystemPath) -> SystemPathBuf {
        self.db.system.canonicalize_path(path).unwrap_or_else(|_| path.to_system_path_buf())
    }

    /// The URI of `file`: the one its document was opened with, if it's
    /// open through a symbolic link.
    fn uri_of(&self, file: File) -> Option<Uri> {
        match file.path(&self.db) {
            FilePath::System(path) => file_uri(self.opened.get(path).map_or(path, |opened| opened)),
            FilePath::SystemVirtual(_) => None,
        }
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
    /// their imports, and the `bolt.toml` of each of their packages, that
    /// aren't open themselves (#249). Their errors are the open documents'
    /// business too, but the editor doesn't ask for them document by
    /// document.
    fn used_files(&self) -> Vec<File> {
        let db = &self.db;
        let open: Vec<File> = self.db.system.open_paths().iter().map(|p| db.files().resolve(db, p)).collect();
        let mut used = Vec::new();
        let add = |file: File, used: &mut Vec<File>| {
            if !open.contains(&file) && !used.contains(&file) {
                used.push(file);
            }
        };
        for &document in open.iter().filter(|&&f| !is_manifest(db, f)) {
            for &file in files_used_by(db, document) {
                add(file, &mut used);
            }
        }
        let bolt_files: Vec<File> = open.iter().copied().filter(|&f| !is_manifest(db, f)).chain(used.iter().copied()).collect();
        for file in bolt_files {
            if let Some(manifest) = package_manifest(db, file) {
                add(manifest, &mut used);
            }
        }
        used
    }

    /// The diagnostics of `file`: of a `bolt.toml`, its own, and of a Bolt
    /// file, without that its package's `bolt.toml` has an error (E0113),
    /// which the editor shows in the `bolt.toml`.
    fn ide_diagnostics(&self, file: File) -> Vec<boltlang_ide_diagnostics::Diagnostic> {
        let db = &self.db;
        if is_manifest(db, file) {
            return manifest_diagnostics(db, file);
        }
        // Another kind of file, like a `Cargo.toml` an editor sends too.
        if !file.path(db).as_system_path_buf().is_some_and(|p| p.as_std_path().extension().is_some_and(|e| e == "bolt")) {
            return Vec::new();
        }
        diagnostics(db, file).into_iter().filter(|d| d.code != DiagnosticCode::BAD_MANIFEST).collect()
    }

    /// The diagnostics of the files that the open documents use, for a
    /// workspace report, each with its URI. A file reported last time that
    /// isn't used anymore is given without any, so that the editor forgets
    /// them.
    pub fn workspace_diagnostics(&mut self) -> Vec<(Uri, Vec<ls_types::Diagnostic>)> {
        let used = self.used_files();
        let mut out = Vec::new();
        for &file in &used {
            out.extend(self.uri_of(file).map(|uri| (uri, self.file_diagnostics(file))));
        }
        for &file in self.reported.iter().filter(|f| !used.contains(f)) {
            out.extend(self.uri_of(file).map(|uri| (uri, Vec::new())));
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
            let errors: Vec<_> = self.ide_diagnostics(file).into_iter().filter(|d| d.severity == Severity::Error).collect();
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
        self.ide_diagnostics(file)
            .into_iter()
            .map(|d| {
                let related: Vec<_> = d
                    .labels
                    .iter()
                    .filter_map(|label| {
                        let uri = self.uri_of(label.file)?;
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

/// Whether `file` is a `bolt.toml`.
fn is_manifest(db: &dyn Db, file: File) -> bool {
    file.path(db).as_system_path_buf().and_then(|p| p.as_std_path().file_name()).is_some_and(|n| n == MANIFEST)
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
        disk.write_file_bytes(SystemPath::new("/toolchain/std/__prelude__/lib.bolt"), b"pub fn id(x: i32) -> i32 = x;\n").unwrap();
        workspace.set_std_dir(Some(SystemPathBuf::from("/toolchain/std")));
        workspace.set_text(SystemPath::new(MAIN), "id(1);\n".into());
        assert!(messages(&workspace, MAIN).is_empty());
        workspace.set_prelude(None);
        assert_eq!(messages(&workspace, MAIN), ["cannot find `id`"]);
        workspace.set_prelude(Some("mine".into()));
        assert_eq!(messages(&workspace, MAIN), [
            "cannot find the package `mine` of the prelude: it isn't a dependency",
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

    #[test]
    fn a_document_opened_through_a_link_is_the_file_imports_use() {
        let (mut workspace, disk) = workspace();
        disk.write_file_bytes(SystemPath::new("/project/real/lib.bolt"), b"pub fn f() -> i32 = 1;\n").unwrap();
        disk.create_symlink(SystemPath::new("/project/link.bolt"), SystemPath::new("real/lib.bolt")).unwrap();
        let link = "/project/link.bolt";
        // The editor opens the link, and adds `negate` without saving (#238).
        workspace.set_text(SystemPath::new(link), "pub fn f() -> i32 = 1;\npub fn negate(x) = !x;\n".into());
        workspace.set_text(SystemPath::new(MAIN), "import \"./link\" (f, negate);\nnegate(1);\n".into());
        let found = workspace.diagnostics(SystemPath::new(MAIN));
        assert_eq!(found.iter().map(|d| d.message.as_str()).collect::<Vec<_>>(), ["expected `Bool`, found an integer"]);
        // The label is in the document, by the path the editor knows it by.
        let related = found[0].related_information.as_ref().unwrap();
        assert!(related[0].location.uri.as_str().ends_with("/project/link.bolt"), "{:?}", related[0].location.uri);
        // Closing it goes back to what's on disk.
        workspace.close(SystemPath::new(link));
        assert_eq!(messages(&workspace, MAIN), ["the imported file has nothing named `negate`", "cannot find `negate`"]);
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
        workspace.set_std_dir(Some(SystemPathBuf::from("/toolchain/std")));
        let prelude = SystemPath::new("/toolchain/std/__prelude__/lib.bolt");
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

    const MANIFEST_PATH: &str = "/project/bolt.toml";

    #[test]
    fn a_manifest_s_errors_are_in_it() {
        let (mut workspace, _) = workspace();
        workspace.set_text(SystemPath::new(MANIFEST_PATH), "[package]\nname = 1\n".into());
        workspace.set_text(SystemPath::new(MAIN), "let x: i32 = 1;\n".into());
        // In the `bolt.toml`, where the error is...
        let found = workspace.diagnostics(SystemPath::new(MANIFEST_PATH));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].code, Some(NumberOrString::String("E0113".into())));
        assert_eq!(found[0].range.start, ls_types::Position { line: 1, character: 7 });
        // ...and not in each file of its package.
        assert!(messages(&workspace, MAIN).is_empty());
        // Editing it clears it, and a dependency that isn't a package is
        // reported at its path.
        workspace.set_text(
            SystemPath::new(MANIFEST_PATH),
            "[package]\nname = \"p\"\n[dependencies]\nshapes = { path = \"../shapes\" }\n".into(),
        );
        let found = workspace.diagnostics(SystemPath::new(MANIFEST_PATH));
        assert_eq!(found.iter().map(|d| d.message.as_str()).collect::<Vec<_>>(), ["there's no package at `../shapes`: it has no `bolt.toml`"]);
        assert_eq!(found[0].range.start, ls_types::Position { line: 3, character: 18 });
    }

    #[test]
    fn other_files_have_no_diagnostics() {
        let (mut workspace, _) = workspace();
        workspace.set_text(SystemPath::new("/project/Cargo.toml"), "[package]\nname = \"x\"\n".into());
        assert!(messages(&workspace, "/project/Cargo.toml").is_empty());
    }

    #[test]
    fn a_manifest_that_isn_t_open_is_in_the_workspace_report() {
        let (mut workspace, disk) = workspace();
        disk.write_file_bytes(SystemPath::new(MANIFEST_PATH), b"[package]\nname = \"a b\"\n").unwrap();
        workspace.set_text(SystemPath::new(MAIN), "let x: i32 = 1;\n".into());
        assert!(messages(&workspace, MAIN).is_empty());
        assert_eq!(workspace_messages(&mut workspace), [(
            "bolt.toml".to_string(),
            vec!["`a b` can't be the name of a package: use letters, digits, `_` and `-`".to_string()]
        )]);
        assert_eq!(workspace.notices(), [
            "`project/bolt.toml`, which the open files use, has 1 error, the first on line 2: `a b` can't be the name of a package: use letters, digits, `_` and `-`"
        ]);
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

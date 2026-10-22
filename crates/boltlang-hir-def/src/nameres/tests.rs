use super::*;
use crate::test_db::TestDb;

/// The names in a namespace of a scope, sorted, with `(imported)` after the
/// ones brought in by an import.
fn names(scope: &ItemScope, ns: Namespace) -> Vec<String> {
    scope
        .entries(ns)
        .map(|(name, entry)| {
            if entry.import.is_some() { format!("{name} (imported)") } else { name.to_string() }
        })
        .collect()
}

fn diagnostics(scope: &FileScope) -> Vec<String> {
    scope
        .diagnostics
        .iter()
        .map(|d| match d {
            NameResDiagnostic::Duplicate { name, .. } => format!("duplicate {name}"),
            NameResDiagnostic::UnresolvedImportPath { path, .. } => format!("unresolved path {path}"),
            NameResDiagnostic::UnresolvedImportName { name, .. } => format!("unresolved name {name}"),
            NameResDiagnostic::PrivateImport { name, .. } => format!("private {name}"),
        })
        .collect()
}

#[test]
fn local_items_are_in_their_namespaces() {
    let db = TestDb::new();
    let file = db.file(
        "/main.bolt",
        r#"
fn f() {}
fn (+)(a, b) = a;
struct S;
enum E { A }
class C[T] { fn show(x: T); }
const K: i32 = 1;
type T = S;
impl S {}
extern "C" { fn malloc(n: usize); }
macro_rules! m { () => {} }
fn f() {}
"#,
    );
    let scope = local_scope(&db, file);
    assert_eq!(names(&scope.scope, Namespace::Values), ["+", "K", "f", "malloc", "show"]);
    assert_eq!(names(&scope.scope, Namespace::Types), ["C", "E", "S", "T"]);
    assert_eq!(names(&scope.scope, Namespace::Macros), ["m"]);
    assert_eq!(scope.scope.impls().len(), 1);
    assert_eq!(diagnostics(scope), ["duplicate f"]);
}

#[test]
fn imports_bring_in_names() {
    let db = TestDb::new();
    db.file(
        "/lib/shapes.bolt",
        "pub struct Circle;\npub fn area() {}\nfn secret() {}\npub(package) const PI: f64 = 3.14;\npub enum Shape {}\n",
    );
    db.file("/lib/io.bolt", "pub fn print() {}\n");
    let main = db.file(
        "/main.bolt",
        r#"
import "./lib/shapes" (Circle, area as compute_area);
import "./lib/shapes" *;
import "./lib/io" as io;
import "./lib/io";
fn area() {}
"#,
    );
    let scope = file_scope(&db, main);
    assert_eq!(names(&scope.scope, Namespace::Types), ["Circle (imported)", "Shape (imported)", "io (imported)"]);
    // The local `area` wins over the one from the glob import.
    assert_eq!(names(&scope.scope, Namespace::Values), ["PI (imported)", "area", "compute_area (imported)"]);
    assert!(diagnostics(scope).is_empty(), "{:?}", diagnostics(scope));
    assert!(matches!(
        scope.scope.get(Namespace::Types, &Name::new("io")).unwrap().def,
        ModuleDefId::File(_)
    ));
}

#[test]
fn import_errors() {
    let db = TestDb::new();
    db.file("/lib.bolt", "pub fn a() {}\nfn hidden() {}\n");
    let main = db.file(
        "/main.bolt",
        r#"
import "./missing" (x);
import "std/io" (print);
import "./lib" (nope, hidden, a);
fn a() {}
"#,
    );
    let scope = file_scope(&db, main);
    assert_eq!(
        diagnostics(scope),
        ["unresolved path ./missing", "unresolved path std/io", "unresolved name nope", "private hidden", "duplicate a"]
    );
}

#[test]
fn files_can_import_each_other() {
    let db = TestDb::new();
    let a = db.file("/a.bolt", "import \"./b\" (b);\npub fn a() {}\n");
    let b = db.file("/b.bolt", "import \"./a\" (a);\npub fn b() {}\n");
    assert_eq!(names(&file_scope(&db, a).scope, Namespace::Values), ["a", "b (imported)"]);
    assert_eq!(names(&file_scope(&db, b).scope, Namespace::Values), ["a (imported)", "b"]);
}

#[test]
fn import_paths_are_relative_to_the_importing_file() {
    let db = TestDb::new();
    let target = db.file("/src/util.bolt", "");
    let importer = db.file("/src/app/main.bolt", "");
    assert_eq!(resolve_import_path(&db, importer, "../util"), Some(target));
    assert_eq!(resolve_import_path(&db, importer, "../util.bolt"), Some(target));
    assert_eq!(resolve_import_path(&db, importer, "./util"), None);
}

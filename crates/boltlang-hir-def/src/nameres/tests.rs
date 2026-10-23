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
            NameResDiagnostic::UnresolvedImportPath { path, error, .. } => format!("unresolved path {path}: {error:?}"),
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
        [
            "unresolved path ./missing: FileNotFound",
            "unresolved path std/io: PackageNotFound { package: \"std\" }",
            "unresolved name nope",
            "private hidden",
            "duplicate a"
        ]
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
    assert_eq!(resolve_import_path(&db, importer, "../util"), Ok(target));
    assert_eq!(resolve_import_path(&db, importer, "../util.bolt"), Ok(target));
    assert_eq!(resolve_import_path(&db, importer, "./util"), Err(ImportPathError::FileNotFound));
}

#[test]
fn packages_are_in_the_nearest_bolt_packages() {
    let db = TestDb::new();
    let io = db.file("/proj/.bolt/packages/std/io.bolt", "");
    let lib = db.file("/proj/.bolt/packages/std/lib.bolt", "");
    let nested = db.file("/proj/.bolt/packages/std/fs/path.bolt", "");
    let inner_io = db.file("/proj/sub/.bolt/packages/std/io.bolt", "");
    let main = db.file("/proj/src/main.bolt", "");
    let inner = db.file("/proj/sub/deep/main.bolt", "");
    let resolve = |importer, source| resolve_import_path(&db, importer, source);
    assert_eq!(resolve(main, "std/io"), Ok(io));
    assert_eq!(resolve(main, "std/io.bolt"), Ok(io));
    assert_eq!(resolve(main, "std"), Ok(lib));
    assert_eq!(resolve(main, "std/fs/path"), Ok(nested));
    // The nearest package wins, even when it doesn't have the file.
    assert_eq!(resolve(inner, "std/io"), Ok(inner_io));
    assert_eq!(
        resolve(inner, "std"),
        Err(ImportPathError::NotInPackage { package: "std".into(), path: "lib.bolt".into() })
    );
    assert_eq!(resolve(main, "json"), Err(ImportPathError::PackageNotFound { package: "json".into() }));
}

#[test]
fn import_paths_must_be_relative_or_in_a_package() {
    let db = TestDb::new();
    db.file("/proj/.bolt/packages/std/io.bolt", "");
    let main = db.file("/proj/main.bolt", "");
    for source in ["/proj/main", "1std/io", "std/../io", "std/./io", "std//io", "std/", ""] {
        assert_eq!(resolve_import_path(&db, main, source), Err(ImportPathError::Invalid), "{source}");
    }
}

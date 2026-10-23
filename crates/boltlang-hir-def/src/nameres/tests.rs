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
            NameResDiagnostic::PrivateImport { name, package_only: false, .. } => format!("private {name}"),
            NameResDiagnostic::PrivateImport { name, package_only: true, .. } => format!("package-only {name}"),
            NameResDiagnostic::UnresolvedImportEnum { path, .. } => format!("unresolved enum {path}"),
            NameResDiagnostic::NotAnEnum { path, .. } => format!("not an enum {path}"),
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
fn a + b = a;
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
fn imports_of_variants() {
    let db = TestDb::new();
    db.file("/shapes.bolt", "pub enum Shape { Circle { r: f64 }, Square(f64) }\nenum Hidden { A }\n");
    db.file("/option.bolt", "pub enum Option[T] { Some(T), None }\n");
    let main = db.file(
        "/main.bolt",
        r#"
import Option.*;
import "./option" (Option);
import "./shapes" as s;
import s.Shape.*;
import Color.*;
enum Color { Red, Square }
fn Red() {}
"#,
    );
    let scope = file_scope(&db, main);
    // The enum can come from a later import. Like a glob import, a variant
    // doesn't replace a name that's already in scope: the function `Red`
    // stays, and `Square` is the one of `Shape`, imported first.
    assert_eq!(names(&scope.scope, Namespace::Values), [
        "Circle (imported)",
        "None (imported)",
        "Red",
        "Some (imported)",
        "Square (imported)",
    ]);
    assert!(names(&scope.scope, Namespace::Types).contains(&"Circle (imported)".to_string()));
    let square = scope.scope.get(Namespace::Values, &Name::new("Square")).unwrap().def;
    let ModuleDefId::Variant(square) = square else { panic!("{square:?}") };
    assert_eq!(square.parent.item(&db).name, Some(Name::new("Shape")));
    assert!(diagnostics(scope).is_empty(), "{:?}", diagnostics(scope));
}

#[test]
fn import_of_variants_errors() {
    let db = TestDb::new();
    db.file("/shapes.bolt", "enum Hidden { A }\npub struct Point;\n");
    let main = db.file(
        "/main.bolt",
        r#"
import "./shapes" as s;
import Nope.*;
import s.Hidden.*;
import s.Point.*;
import Color.Red.*;
import s.Nope.*;
enum Color { Red }
"#,
    );
    assert_eq!(
        diagnostics(file_scope(&db, main)),
        ["unresolved enum Nope", "private Hidden", "not an enum s.Point", "unresolved enum Color.Red", "unresolved enum s.Nope"]
    );
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

#[test]
fn package_items_are_only_visible_in_their_package() {
    let db = TestDb::new();
    db.file("/proj/.bolt/packages/geo/lib.bolt", "pub(package) fn helper() {}\npub fn area() {}\nfn secret() {}\n");
    let inside = db.file("/proj/.bolt/packages/geo/shapes.bolt", "import \"./lib\" (helper, area);\n");
    let glob_inside = db.file("/proj/.bolt/packages/geo/all.bolt", "import \"geo\" *;\n");
    let outside = db.file("/proj/main.bolt", "import \"geo\" (helper, area, secret);\n");
    let glob_outside = db.file("/proj/src/all.bolt", "import \"geo\" *;\n");
    assert!(diagnostics(file_scope(&db, inside)).is_empty());
    assert_eq!(names(&file_scope(&db, glob_inside).scope, Namespace::Values), ["area (imported)", "helper (imported)"]);
    assert_eq!(diagnostics(file_scope(&db, outside)), ["package-only helper", "private secret"]);
    assert_eq!(names(&file_scope(&db, glob_outside).scope, Namespace::Values), ["area (imported)"]);
}

#[test]
fn files_are_in_the_innermost_package() {
    let db = TestDb::new();
    let geo = db.file("/proj/.bolt/packages/geo/lib.bolt", "");
    let math = db.file("/proj/.bolt/packages/geo/.bolt/packages/math/lib.bolt", "");
    let deep = db.file("/proj/.bolt/packages/geo/src/deep.bolt", "");
    let a = db.file("/proj/a/x.bolt", "");
    let b = db.file("/proj/b/y.bolt", "");
    let dir = |p: &str| Some(SystemPathBuf::from(p));
    assert_eq!(package_of(&db, geo), dir("/proj/.bolt/packages/geo"));
    assert_eq!(package_of(&db, deep), dir("/proj/.bolt/packages/geo"));
    assert_eq!(package_of(&db, math), dir("/proj/.bolt/packages/geo/.bolt/packages/math"));
    // The files of the project are one package, wherever they are.
    assert_eq!(package_of(&db, a), None);
    assert!(is_visible_from(&db, RawVisibility::Package, a, b));
    assert!(!is_visible_from(&db, RawVisibility::Package, geo, a));
    assert!(!is_visible_from(&db, RawVisibility::Package, math, geo));
    assert!(!is_visible_from(&db, RawVisibility::Private, a, b));
    assert!(is_visible_from(&db, RawVisibility::Private, a, a));
}

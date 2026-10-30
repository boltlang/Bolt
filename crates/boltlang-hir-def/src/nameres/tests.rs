use super::*;
use crate::test_db::TestDb;

/// The names in a namespace of a scope, sorted, with `(imported)` after the
/// ones brought in by an import.
fn names(scope: &ItemScope, ns: Namespace) -> Vec<String> {
    scope
        .entries(ns)
        .map(|(name, entry)| {
            if entry.prelude {
                format!("{name} (prelude)")
            } else if entry.import.is_some() {
                format!("{name} (imported)")
            } else {
                name.to_string()
            }
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
            NameResDiagnostic::UnresolvedPrelude { package, error } => format!("unresolved prelude {package}: {error:?}"),
            NameResDiagnostic::UnresolvedOwner { name, .. } => format!("unresolved owner {name}"),
            NameResDiagnostic::OwnerNotTopLevel { .. } => "owner not top level".to_string(),
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
const f: i32 = 1;
"#,
    );
    let scope = local_scope(&db, file);
    // The two `f`s are overloads (#275), but a const can't share a name.
    assert_eq!(names(&scope.scope, Namespace::Values), ["+", "K", "f", "f", "malloc", "show"]);
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
    // The local `area` and the one from the glob import are overloads.
    assert_eq!(names(&scope.scope, Namespace::Values), ["PI (imported)", "area", "area (imported)", "compute_area (imported)"]);
    assert!(diagnostics(scope).is_empty(), "{:?}", diagnostics(scope));
    assert!(matches!(
        scope.scope.get(Namespace::Types, &Name::new("io")).unwrap().def,
        ModuleDefId::File(_)
    ));
}

/// A glob import doesn't clash with a name imported by name, before or
/// after it: the one imported by name wins.
#[test]
fn glob_imports_give_way_to_names_imported_by_name() {
    let db = TestDb::new();
    let a = db.file("/a.bolt", "pub struct Point;\npub struct Line;\n");
    let b = db.file("/b.bolt", "pub struct Point;\n");
    for text in ["import \"./a\" *;\nimport \"./b\" (Point);\n", "import \"./b\" (Point);\nimport \"./a\" *;\n"] {
        let main = db.file("/main.bolt", text);
        let scope = file_scope(&db, main);
        assert!(diagnostics(scope).is_empty(), "{text}: {:?}", diagnostics(scope));
        let point = scope.scope.get(Namespace::Types, &Name::new("Point")).unwrap().def;
        assert!(matches!(point, ModuleDefId::Struct(s) if s.file(&db) == b), "{text}");
        let line = scope.scope.get(Namespace::Types, &Name::new("Line")).unwrap().def;
        assert!(matches!(line, ModuleDefId::Struct(s) if s.file(&db) == a), "{text}");
    }
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
fn pub_imports_of_variants_are_exported() {
    let db = TestDb::new();
    db.file(
        "/option.bolt",
        "pub enum Option[T] { None, Some(T) }\npub import Option.*;\npub enum Hidden { A }\nimport Hidden.*;\n\
         enum Private { B }\npub import Private.*;\n",
    );
    db.file("/shapes.bolt", "pub enum Shape { Circle }\n");
    db.file("/reexport.bolt", "import \"./shapes\" as s;\npub import s.Shape.*;\n");
    let main = db.file(
        "/main.bolt",
        r#"
import "./option" (Option, None, Some);
import "./option" *;
import "./reexport" (Circle);
"#,
    );
    let scope = file_scope(&db, main);
    // `A` isn't exported by a private import, nor `B` by a private enum.
    assert_eq!(names(&scope.scope, Namespace::Values), ["Circle (imported)", "None (imported)", "Some (imported)"]);
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
            "unresolved path std/io: NoStd",
            "unresolved name nope",
            "private hidden",
        ]
    );
    // The imported `a` and the local one are overloads of `a` (#275).
    assert_eq!(scope.scope.get_all(Namespace::Values, &Name::new("a")).count(), 2);
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
fn packages_are_the_dependencies_in_bolt_toml() {
    let db = TestDb::new();
    db.file(
        "/proj/bolt.toml",
        "[package]\nname = \"demo\"\n[dependencies]\nshapes = { path = \"../shapes\" }\nlost = { path = \"../nowhere\" }\n",
    );
    db.file("/shapes/bolt.toml", "[package]\nname = \"shapes\"\n[dependencies]\ngeo = { path = \"../geo\" }\n");
    db.file("/geo/bolt.toml", "[package]\nname = \"geo\"\n");
    let geo = db.file("/geo/lib.bolt", "");
    let lib = db.file("/shapes/lib.bolt", "");
    let circle = db.file("/shapes/circle.bolt", "");
    let nested = db.file("/shapes/fs/path.bolt", "");
    let main = db.file("/proj/src/main.bolt", "");
    let loose = db.file("/elsewhere/loose.bolt", "");
    let resolve = |importer, source| resolve_import_path(&db, importer, source);
    assert_eq!(resolve(main, "shapes/circle"), Ok(circle));
    assert_eq!(resolve(main, "shapes/circle.bolt"), Ok(circle));
    assert_eq!(resolve(main, "shapes"), Ok(lib));
    assert_eq!(resolve(main, "shapes/fs/path"), Ok(nested));
    assert_eq!(
        resolve(main, "shapes/square"),
        Err(ImportPathError::NotInPackage { package: "shapes".into(), path: "square.bolt".into() })
    );
    // A dependency's own dependencies are its own.
    assert_eq!(resolve(circle, "geo"), Ok(geo));
    assert_eq!(resolve(main, "geo"), Err(ImportPathError::NotADependency { package: "geo".into() }));
    assert_eq!(
        resolve(main, "lost"),
        Err(ImportPathError::DependencyNotFound { package: "lost".into(), path: "../nowhere".into() })
    );
    // A file in no package can only use `std`, which this database lacks.
    assert_eq!(resolve(loose, "shapes"), Err(ImportPathError::NoManifest { package: "shapes".into() }));
    assert_eq!(resolve(loose, "std"), Err(ImportPathError::NoStd));
}

#[test]
fn a_broken_manifest_has_no_dependencies() {
    let db = TestDb::new();
    db.file("/proj/bolt.toml", "[package]\nname = 1\n");
    let main = db.file("/proj/main.bolt", "");
    assert_eq!(resolve_import_path(&db, main, "shapes"), Err(ImportPathError::BadManifest));
}

#[test]
fn std_comes_with_the_compiler() {
    let mut db = TestDb::new();
    db.set_prelude(Some("std"), false);
    let io = db.file("/toolchain/std/io.bolt", "pub fn print() {}\n");
    let prelude = db.file("/toolchain/std/__prelude__/lib.bolt", "");
    let main = db.file("/proj/main.bolt", "");
    Settings::change_std_dir(&mut db, Some(SystemPathBuf::from("/toolchain/std")));
    let resolve = |importer, source| resolve_import_path(&db, importer, source);
    assert_eq!(resolve(main, "std/io"), Ok(io));
    assert_eq!(
        resolve(main, "std/missing"),
        Err(ImportPathError::NotInPackage { package: "std".into(), path: "missing.bolt".into() })
    );
    // Its files are the package `std`, which doesn't get its own prelude.
    assert_eq!(package_of(&db, io), Some(SystemPathBuf::from("/toolchain/std")));
    assert_eq!(package_of(&db, main), None);
    assert_eq!(prelude_file(&db, main), &Ok(Some(prelude)));
    assert_eq!(prelude_file(&db, io), &Ok(None));
}

#[test]
fn import_paths_must_be_relative_or_in_a_package() {
    let db = TestDb::new();
    let main = db.file("/proj/main.bolt", "");
    for source in ["/proj/main", "1std/io", "std/../io", "std/./io", "std//io", "std/", ""] {
        assert_eq!(resolve_import_path(&db, main, source), Err(ImportPathError::Invalid), "{source}");
    }
}

#[test]
fn package_items_are_only_visible_in_their_package() {
    let db = TestDb::new();
    db.file("/geo/bolt.toml", "[package]\nname = \"geo\"\n");
    db.file("/geo/lib.bolt", "pub(package) fn helper() {}\npub fn area() {}\nfn secret() {}\n");
    let inside = db.file("/geo/shapes.bolt", "import \"./lib\" (helper, area);\n");
    let glob_inside = db.file("/geo/all.bolt", "import \"./lib\" *;\n");
    db.file("/proj/bolt.toml", "[package]\nname = \"proj\"\n[dependencies]\ngeo = { path = \"../geo\" }\n");
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
    db.file("/geo/bolt.toml", "[package]\nname = \"geo\"\n");
    db.file("/geo/math/bolt.toml", "[package]\nname = \"math\"\n");
    let geo = db.file("/geo/lib.bolt", "");
    let math = db.file("/geo/math/lib.bolt", "");
    let deep = db.file("/geo/src/deep.bolt", "");
    let a = db.file("/loose/a/x.bolt", "");
    let b = db.file("/loose/b/y.bolt", "");
    let dir = |p: &str| Some(SystemPathBuf::from(p));
    assert_eq!(package_of(&db, geo), dir("/geo"));
    assert_eq!(package_of(&db, deep), dir("/geo"));
    assert_eq!(package_of(&db, math), dir("/geo/math"));
    // Files in no package are in none, together.
    assert_eq!(package_of(&db, a), None);
    assert!(is_visible_from(&db, RawVisibility::Package, a, b));
    assert!(!is_visible_from(&db, RawVisibility::Package, geo, a));
    assert!(!is_visible_from(&db, RawVisibility::Package, math, geo));
    assert!(!is_visible_from(&db, RawVisibility::Private, a, b));
    assert!(is_visible_from(&db, RawVisibility::Private, a, a));
}

#[test]
fn pub_imports_re_export() {
    let db = TestDb::new();
    db.file("/a.bolt", "pub fn f() {}\npub fn g() {}\npub struct S;\n");
    db.file("/list.bolt", "pub import \"./a\" (f, S as T);\n");
    db.file("/glob.bolt", "pub import \"./a\" *;\n");
    db.file("/private.bolt", "import \"./a\" (f);\n");
    db.file("/alias.bolt", "pub import \"./a\" as m;\n");
    let main = db.file(
        "/main.bolt",
        r#"
import "./list" (f, T);
import "./glob" (g);
import "./alias" (m);
"#,
    );
    let scope = file_scope(&db, main);
    assert_eq!(names(&scope.scope, Namespace::Values), ["f (imported)", "g (imported)"]);
    assert_eq!(names(&scope.scope, Namespace::Types), ["T (imported)", "m (imported)"]);
    assert!(diagnostics(scope).is_empty(), "{:?}", diagnostics(scope));
    // A plain import keeps the name to the file.
    let other = db.file("/other.bolt", "import \"./private\" (f);\nimport \"./private\" *;\n");
    assert_eq!(diagnostics(file_scope(&db, other)), ["private f"]);
    assert!(names(&file_scope(&db, other).scope, Namespace::Values).is_empty());
}

#[test]
fn re_exports_go_through_several_files() {
    let db = TestDb::new();
    db.file("/option.bolt", "pub enum Option[T] { None, Some(T) }\n");
    db.file("/types.bolt", "pub import \"./option\" (Option);\npub import Option.*;\n");
    db.file("/lib.bolt", "pub import \"./types\" *;\n");
    let main = db.file("/main.bolt", "import \"./lib\" *;\n");
    let scope = file_scope(&db, main);
    assert_eq!(names(&scope.scope, Namespace::Values), ["None (imported)", "Some (imported)"]);
    assert!(names(&scope.scope, Namespace::Types).contains(&"Option (imported)".to_string()));
}

#[test]
fn files_can_re_export_from_each_other() {
    let db = TestDb::new();
    let a = db.file("/a.bolt", "pub import \"./b\" *;\npub fn a() {}\n");
    let b = db.file("/b.bolt", "pub import \"./a\" *;\npub fn b() {}\n");
    let main = db.file("/main.bolt", "import \"./a\" (a, b);\n");
    assert_eq!(names(&file_scope(&db, a).scope, Namespace::Values), ["a", "b (imported)"]);
    assert_eq!(names(&file_scope(&db, b).scope, Namespace::Values), ["a (imported)", "b"]);
    assert_eq!(names(&file_scope(&db, main).scope, Namespace::Values), ["a (imported)", "b (imported)"]);
    assert!(diagnostics(file_scope(&db, main)).is_empty());
}

#[test]
fn re_exports_are_as_visible_as_the_import_and_the_item() {
    let db = TestDb::new();
    db.file("/geo/bolt.toml", "[package]\nname = \"geo\"\n");
    db.file("/geo/shapes.bolt", "pub fn area() {}\npub(package) fn helper() {}\n");
    db.file("/geo/lib.bolt", "pub import \"./shapes\" (area, helper);\npub(package) import \"./shapes\" (area as area2);\n");
    let inside = db.file("/geo/user.bolt", "import \"./lib\" (area, helper, area2);\n");
    db.file("/proj/bolt.toml", "[package]\nname = \"proj\"\n[dependencies]\ngeo = { path = \"../geo\" }\n");
    let outside = db.file("/proj/main.bolt", "import \"geo\" (area, helper, area2);\n");
    assert!(diagnostics(file_scope(&db, inside)).is_empty(), "{:?}", diagnostics(file_scope(&db, inside)));
    assert_eq!(diagnostics(file_scope(&db, outside)), ["package-only helper", "package-only area2"]);
}

#[test]
fn the_prelude_comes_last() {
    let mut db = TestDb::new();
    db.set_prelude(Some("std"), false);
    db.file("/toolchain/std/__prelude__/option.bolt", "pub enum Option[T] { None, Some(T) }\n");
    db.file(
        "/toolchain/std/__prelude__/lib.bolt",
        "pub import \"./option\" (Option);\npub import Option.*;\npub fn id() {}\nfn hidden() {}\n",
    );
    Settings::change_std_dir(&mut db, Some(SystemPathBuf::from("/toolchain/std")));
    let main = db.file(
        "/main.bolt",
        "enum Maybe { Some, Nothing }\nimport Maybe.*;\nimport Option.*;\nfn id() {}\n",
    );
    let scope = file_scope(&db, main);
    // `Some` is the one of `Maybe`, and `None` the one of `Option`, whose
    // enum is found in the prelude. The prelude's `id` is one more
    // overload of `id`.
    assert_eq!(names(&scope.scope, Namespace::Values), [
        "None (imported)",
        "Nothing (imported)",
        "Some (imported)",
        "id",
        "id (prelude)"
    ]);
    assert_eq!(names(&scope.scope, Namespace::Types), ["Maybe", "None (imported)", "Nothing (imported)", "Option (prelude)", "Some (imported)"]);
    let some = scope.scope.get(Namespace::Values, &Name::new("Some")).unwrap().def;
    let ModuleDefId::Variant(some) = some else { panic!("{some:?}") };
    assert_eq!(some.parent.item(&db).name, Some(Name::new("Maybe")));
    assert!(diagnostics(scope).is_empty(), "{:?}", diagnostics(scope));
}


#[test]
fn a_dependency_reached_through_a_link_is_one_package() {
    let db = TestDb::new();
    db.file("/shapes/bolt.toml", "[package]\nname = \"shapes\"\n");
    let real = db.file("/shapes/lib.bolt", "");
    db.file("/proj/bolt.toml", "[package]\nname = \"proj\"\n[dependencies]\nshapes = { path = \"vendor/shapes\" }\n");
    db.symlink("/proj/vendor/shapes", "../../shapes");
    let main = db.file("/proj/main.bolt", "");
    // Imports reach it by its real path, and a file opened through the link
    // is in the same package.
    assert_eq!(resolve_import_path(&db, main, "shapes"), Ok(real));
    let through_link = db.files().resolve(&db, SystemPath::new("/proj/vendor/shapes/lib.bolt"));
    assert_eq!(through_link, real);
    assert_eq!(package_of(&db, through_link), Some(SystemPathBuf::from("/shapes")));
    assert_eq!(package_of(&db, real), Some(SystemPathBuf::from("/shapes")));
}

#[test]
fn a_file_reached_through_a_link_is_one_file() {
    let db = TestDb::new();
    let real = db.file("/proj/real/lib.bolt", "import \"./helper\" (h);\n");
    let helper = db.file("/proj/real/helper.bolt", "pub fn h() -> i32 = 1;\n");
    db.symlink("/proj/link.bolt", "real/lib.bolt");
    // By the link, it's the same file, at its real path, so its imports are
    // next to it, not next to the link (#238).
    let through_link = db.files().resolve(&db, SystemPath::new("/proj/link.bolt"));
    assert_eq!(through_link, real);
    assert_eq!(through_link.path(&db).as_system_path_buf(), Some(&SystemPathBuf::from("/proj/real/lib.bolt")));
    assert_eq!(resolve_import_path(&db, through_link, "./helper"), Ok(helper));
}

#[test]
fn a_package_chooses_its_prelude() {
    let mut db = TestDb::new();
    db.set_prelude(Some("std"), false);
    let std_prelude = db.file("/toolchain/std/__prelude__/lib.bolt", "");
    Settings::change_std_dir(&mut db, Some(SystemPathBuf::from("/toolchain/std")));
    db.file("/geo/bolt.toml", "[package]\nname = \"geo\"\n");
    let geo_prelude = db.file("/geo/__prelude__/lib.bolt", "");
    db.file("/plain/bolt.toml", "[package]\nname = \"plain\"\n");
    db.file("/bare/bolt.toml", "[package]\nname = \"bare\"\nprelude = false\n");
    db.file("/own/bolt.toml", "[package]\nname = \"own\"\nprelude = \"geo\"\n[dependencies]\ngeo = { path = \"../geo\" }\n");
    let prelude = |path: &str| prelude_file(&db, db.file(path, "")).clone();
    assert_eq!(prelude("/plain/main.bolt"), Ok(Some(std_prelude)));
    assert_eq!(prelude("/loose/main.bolt"), Ok(Some(std_prelude)));
    assert_eq!(prelude("/bare/main.bolt"), Ok(None));
    assert_eq!(prelude("/own/main.bolt"), Ok(Some(geo_prelude)));
    // The files of `std` have none, but the classes of `std`'s.
    let io = db.file("/toolchain/std/io.bolt", "");
    assert_eq!(prelude_file(&db, io), &Ok(None));
    assert_eq!(prelude_lib(&db, io), Some(std_prelude));
}

#[test]
fn the_prelude_package_has_no_prelude() {
    let mut db = TestDb::new();
    db.set_prelude(Some("std"), false);
    let lib = db.file("/toolchain/std/__prelude__/lib.bolt", "pub fn id() {}\n");
    let io = db.file("/toolchain/std/io.bolt", "pub fn print() {}\n");
    Settings::change_std_dir(&mut db, Some(SystemPathBuf::from("/toolchain/std")));
    assert_eq!(prelude_file(&db, lib), &Ok(None));
    assert_eq!(prelude_file(&db, io), &Ok(None));
    assert!(names(&file_scope(&db, io).scope, Namespace::Values) == ["print"]);
}

#[test]
fn a_missing_prelude_is_an_error_only_when_asked_for() {
    let mut db = TestDb::new();
    let main = db.file("/main.bolt", "fn f() {}\n");
    db.set_prelude(Some("std"), false);
    assert!(diagnostics(file_scope(&db, main)).is_empty());
    db.set_prelude(Some("std"), true);
    assert_eq!(diagnostics(file_scope(&db, main)), ["unresolved prelude std: NoStd"]);
    db.set_prelude(None, true);
    assert!(diagnostics(file_scope(&db, main)).is_empty());
    // A prelude that a `bolt.toml` names is asked for.
    db.set_prelude(Some("std"), false);
    db.file("/empty/bolt.toml", "[package]\nname = \"empty\"\n");
    db.file("/proj/bolt.toml", "[package]\nname = \"proj\"\nprelude = \"empty\"\n[dependencies]\nempty = { path = \"../empty\" }\n");
    let own = db.file("/proj/main.bolt", "fn f() {}\n");
    assert_eq!(diagnostics(file_scope(&db, own)), [
        "unresolved prelude empty: NotInPackage { package: \"empty\", path: \"__prelude__/lib.bolt\" }"
    ]);
}

#[test]
fn functions_of_a_type_are_in_its_namespace() {
    let db = TestDb::new();
    let file = db.file(
        "/main.bolt",
        r#"
struct Vec[T] { len: i32 }
enum Shape { Circle }
fn Vec.new() {}
fn Vec.len() {}
fn Shape.unit() {}
fn new() {}
"#,
    );
    let scope = local_scope(&db, file);
    // `Vec.new` doesn't take the name `new` in the file.
    assert_eq!(names(&scope.scope, Namespace::Values), ["new"]);
    let members = |owner: &str| -> Vec<String> {
        scope.members.get(&Name::new(owner)).map_or(Vec::new(), |m| m.keys().map(|n| n.to_string()).collect())
    };
    assert_eq!(members("Vec"), ["len", "new"]);
    assert_eq!(members("Shape"), ["unit"]);
    assert!(diagnostics(scope).is_empty(), "{:?}", diagnostics(scope));
}

#[test]
fn functions_of_a_type_need_a_type_of_the_file() {
    let db = TestDb::new();
    db.file("/lib.bolt", "pub struct Point;\n");
    let file = db.file(
        "/main.bolt",
        r#"
import "./lib" (Point);
enum Shape { Circle }
fn f() {}
class C[T] { fn Shape.g(x: T); }
fn Point.origin() {}
fn Nope.a() {}
fn f.b() {}
fn Shape.Circle() {}
fn Shape.c() {}
fn Shape.c() {}
"#,
    );
    // `Point` is imported, not defined here.
    assert_eq!(
        diagnostics(local_scope(&db, file)),
        [
            "unresolved owner Point",
            "unresolved owner Nope",
            "unresolved owner f",
            "duplicate Circle",
            "duplicate c",
            "owner not top level",
        ]
    );
}

#[test]
fn tests_are_listed_and_not_in_scope() {
    let db = TestDb::new();
    let file = db.file(
        "/main.bolt",
        r#"
fn push(x: i32) {}
#[test]
fn push() {}
#[test]
fn second() {}
#[derive(Debug)]
fn other() {}
"#,
    );
    let scope = local_scope(&db, file);
    // The test called `push` isn't one more `push`.
    assert_eq!(names(&scope.scope, Namespace::Values), ["other", "push"]);
    let names: Vec<String> =
        test_functions(&db, file).iter().map(|f| f.item(&db).name.as_ref().unwrap().name().to_string()).collect();
    assert_eq!(names, ["push", "second"]);
    assert!(diagnostics(scope).is_empty(), "{:?}", diagnostics(scope));
}

#[test]
fn a_prelude_of_one_file_is_its_package_s_prelude_bolt() {
    let mut db = TestDb::new();
    db.set_prelude(Some("std"), true);
    db.file("/toolchain/std/__prelude__.bolt", "pub fn id() {}\n");
    Settings::change_std_dir(&mut db, Some(SystemPathBuf::from("/toolchain/std")));
    let main = db.file("/main.bolt", "fn f() {}\n");
    let scope = file_scope(&db, main);
    assert_eq!(names(&scope.scope, Namespace::Values), ["f", "id (prelude)"]);
    assert!(diagnostics(scope).is_empty(), "{:?}", diagnostics(scope));
}

#[test]
fn a_package_with_two_preludes_has_none() {
    let mut db = TestDb::new();
    db.set_prelude(Some("std"), true);
    db.file("/toolchain/std/__prelude__.bolt", "pub fn id() {}\n");
    db.file("/toolchain/std/__prelude__/lib.bolt", "pub fn other() {}\n");
    Settings::change_std_dir(&mut db, Some(SystemPathBuf::from("/toolchain/std")));
    let main = db.file("/main.bolt", "fn f() {}\n");
    let scope = file_scope(&db, main);
    assert_eq!(names(&scope.scope, Namespace::Values), ["f"]);
    assert_eq!(diagnostics(scope), ["unresolved prelude std: TwoPreludes { package: \"std\" }"]);
}

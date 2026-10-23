use boltlang_base_db::TestDb;

use super::*;

// Most tests of the checker are in `tests/data`, run by `tests/check.rs`.
// These ones need several files, or edit a file between two checks.

#[test]
fn imported_functions_without_signatures() {
    let db = TestDb::new();
    db.file("/lib.bolt", "pub fn id(x) = x;\n");
    let main = db.file("/main.bolt", "import \"./lib\" (id);\nfn f(y: i32) -> bool = id(y);\n");
    let ModItem::Function(idx) = item_tree(&db, main).top_level[1] else { panic!() };
    let f = DefWithBodyId::new(&db, DefWithBody::Function(FunctionId::new(&db, ItemContainerId::File(main), idx)));
    let diagnostics = &infer_body(&db, f).diagnostics;
    assert!(
        matches!(&diagnostics[..], [InferenceDiagnostic::Type(TypeError::Mismatch { expected, actual, .. })]
            if *expected == Ty::bool() && actual.display(&db).to_string() == "i32"),
        "{diagnostics:?}"
    );
}

#[test]
fn files_that_infer_each_other_report_a_cycle() {
    let db = TestDb::new();
    let a = db.file("/a.bolt", "import \"./b\" (g);\npub fn f(x) = g(x);\n");
    db.file("/b.bolt", "import \"./a\" (f);\npub fn g(x) = f(x);\n");
    let ModItem::Function(idx) = item_tree(&db, a).top_level[1] else { panic!() };
    let f = DefWithBodyId::new(&db, DefWithBody::Function(FunctionId::new(&db, ItemContainerId::File(a), idx)));
    assert!(infer_body(&db, f).diagnostics.contains(&InferenceDiagnostic::CyclicInference));
}

/// The type of the top-level statements of a file, to check what reruns.
#[salsa::tracked]
fn top_level_ok(db: &dyn Db, file: File) -> bool {
    infer_body(db, DefWithBodyId::new(db, DefWithBody::File(file))).diagnostics.is_empty()
}

#[test]
fn editing_a_declared_function_does_not_rerun_its_callers() {
    let mut db = TestDb::new();
    let file = db.file("/main.bolt", "fn f(x: i32) -> i32 = x;\nfn g(y) = y;\nf(g(1));\n");
    assert!(*top_level_ok(&db, file));
    let executed = db.log_executed(|db| {
        db.edit(file, "fn f(x: i32) -> i32 = x + 1;\nfn g(y) = y;\nf(g(1));\n");
        assert!(*top_level_ok(db, file));
    });
    assert!(!executed.iter().any(|q| q.contains("file_inference") || q.contains("top_level_ok")), "{executed:?}");

    // Editing a function without a signature reinfers the file's group.
    let executed = db.log_executed(|db| {
        db.edit(file, "fn f(x: i32) -> i32 = x + 1;\nfn g(y) = { y };\nf(g(1));\n");
        assert!(*top_level_ok(db, file));
    });
    assert!(executed.iter().any(|q| q.contains("file_inference")), "{executed:?}");
}

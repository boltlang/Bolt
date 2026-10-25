use boltlang_base_db::TestDb;

use super::*;

// Most tests of the checker are in `tests/checker` at the root of the
// repository, run by `tests/check.rs`.
// This one edits a file between two checks.

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

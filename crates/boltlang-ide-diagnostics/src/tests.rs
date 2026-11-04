//! The diagnostics as users see them are tested by the cases in
//! `tests/diagnostics`, run by `tests/diagnostics.rs`. Here are the tests
//! that need more than the diagnostics of a case's files.

use boltlang_base_db::{File, Settings, SystemPathBuf, TestDb, source_text};
use boltlang_syntax::rowan::TextRange;

/// A database whose `std`, the one that comes with the compiler, is in
/// [`TestDb::STD_DIR`], for files there that a test gives.
fn with_std_dir() -> TestDb {
    let mut db = TestDb::new();
    Settings::change_std_dir(&mut db, Some(SystemPathBuf::from(TestDb::STD_DIR)));
    db
}

/// The messages of the errors of the files `main` uses, with the place of
/// each and of its label, as `line:column`.
fn broken(db: &TestDb, main: File) -> Vec<String> {
    let at = |file: File, range: TextRange| {
        let text = source_text(db, file);
        let before = &text[..usize::from(range.start())];
        let line = before.matches('\n').count() + 1;
        let column = before.len() - before.rfind('\n').map_or(0, |i| i + 1) + 1;
        format!("{line}:{column}")
    };
    crate::dependency_diagnostics(db, main)
        .into_iter()
        .map(|d| {
            let label = &d.labels[0];
            format!(
                "{} {} | {} {}",
                at(d.file, d.range),
                d.message,
                at(label.file, label.range),
                label.message
            )
        })
        .collect()
}

#[test]
fn errors_in_files_a_file_uses() {
    let mut db = with_std_dir();
    db.set_prelude(Some("std"), true);
    db.file(
        "/toolchain/std/__prelude__/lib.bolt",
        "import \"./more\";\npub fn id(x: i32) -> i32 = x;\n",
    );
    let more =
        db.file("/toolchain/std/__prelude__/more.bolt", "let a: i32 = True;\nlet b: Bool = 1;\n");
    db.file("/proj/deep.bolt", "pub fn f() -> i32 = True;\n");
    db.file("/proj/lib.bolt", "import \"./deep\" (f);\npub fn g() -> i32 = f();\n");
    let main = db.file("/proj/main.bolt", "let x = 1;\nimport \"./lib\" (g);\ng();\n");
    assert_eq!(
        broken(&db, main),
        [
            "1:1 `std/__prelude__/more.bolt`, which this file uses through the prelude, has 2 errors; fix them first \
         | 1:14 the first: expected `i32`, found `Bool`",
            "2:8 `deep.bolt`, which this file uses through this import, has 1 error; fix them first \
         | 1:21 the first: expected `i32`, found `Bool`",
        ]
    );
    // A file's own errors aren't among them.
    assert!(broken(&db, more).is_empty());
}

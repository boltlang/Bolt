//! The diagnostics as users see them are tested by the cases in
//! `tests/diagnostics`, run by `tests/diagnostics.rs`. Here are the tests
//! that need more than the diagnostics of a case's files.

use boltlang_base_db::{Db, File, Settings, SystemPath, SystemPathBuf, TestDb, source_text};
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

/// The errors of the `.bolt` files in `dir` on disk and the directories in
/// it, which are at `at` in `db`, as `path:line:column message`.
fn errors_in_dir(db: &TestDb, dir: &std::path::Path, at: &str, out: &mut Vec<String>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
    entries.sort();
    for path in entries {
        let name = format!("{at}/{}", path.file_name().unwrap().to_str().unwrap());
        if path.is_dir() {
            errors_in_dir(db, &path, &name, out);
        } else if path.extension().is_some_and(|ext| ext == "bolt") {
            let file = db.files().resolve(db, SystemPath::new(&name));
            let text = source_text(db, file);
            for d in crate::diagnostics(db, file) {
                if d.severity == crate::Severity::Error {
                    let before = &text[..usize::from(d.range.start())];
                    let line = before.matches('\n').count() + 1;
                    let column = before.len() - before.rfind('\n').map_or(0, |i| i + 1) + 1;
                    out.push(format!("{name}:{line}:{column} {}", d.message));
                }
            }
        }
    }
}

/// The prelude of `:test-prelude`, `tests/std`, has no errors. The cases
/// that use it only see the errors of their own files, so without this,
/// an error in it would only show up in the cases of `tests/run`.
#[test]
fn the_test_prelude_has_no_errors() {
    let mut db = TestDb::new();
    db.add_test_std();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/std");
    let mut errors = Vec::new();
    errors_in_dir(&db, &dir, TestDb::STD_DIR, &mut errors);
    assert!(errors.is_empty(), "tests/std has errors:\n{}", errors.join("\n"));
}

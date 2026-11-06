//! What the lowering makes of a program is tested by the cases in
//! `tests/lower`, run by `tests/lower.rs`. Here are the tests that are
//! about more than one case.

use boltlang_base_db::TestDb;

use crate::program;

/// Every test file of `std` is a program whose uses can all be compiled.
#[test]
fn the_tests_of_std_collect_without_errors() {
    let mut db = TestDb::new();
    db.add_std();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/std");
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_str().unwrap().to_string())
        .filter(|n| n.starts_with("test_") && n.ends_with(".bolt"))
        .collect();
    names.sort();
    assert!(!names.is_empty());
    let mut problems = Vec::new();
    for name in names {
        let path = format!("{}/{name}", TestDb::STD_DIR);
        let file =
            boltlang_base_db::Db::files(&db).resolve(&db, boltlang_base_db::SystemPath::new(&path));
        let p = program(&db, file);
        assert!(p.instances.len() > 1, "{name} has no instances");
        for (i, site, e) in &p.errors {
            problems.push(format!("{name}: in {} at {site:?}: {e:?}", i.display(&db)));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

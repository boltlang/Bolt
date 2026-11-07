//! What the lowering makes of a program is tested by the cases in
//! `tests/lower`, run by `tests/lower.rs`. Here are the tests that are
//! about more than one case.

use boltlang_base_db::TestDb;

use crate::program;

/// Every test file of `std` is a program whose uses can all be compiled,
/// for the evaluator and for Wasm.
#[test]
fn the_tests_of_std_collect_without_errors() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/std");
    // The test files, by their path in `std`, like `sys/test_emu.bolt`.
    fn test_files(dir: &std::path::Path, prefix: &str, out: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name().to_str().unwrap().to_string();
            if entry.path().is_dir() {
                test_files(&entry.path(), &format!("{prefix}{name}/"), out);
            } else if name.starts_with("test_") && name.ends_with(".bolt") {
                out.push(format!("{prefix}{name}"));
            }
        }
    }
    let mut names = Vec::new();
    test_files(&dir, "", &mut names);
    names.sort();
    assert!(!names.is_empty());
    let mut problems = Vec::new();
    for target in [boltlang_base_db::Target::default(), boltlang_base_db::Target::WASM] {
        let mut db = TestDb::new();
        db.add_std();
        boltlang_base_db::Settings::change_target(&mut db, target);
        for name in &names {
            let path = format!("{}/{name}", TestDb::STD_DIR);
            let file = boltlang_base_db::Db::files(&db)
                .resolve(&db, boltlang_base_db::SystemPath::new(&path));
            // A file can have tests only for some targets, like `test_wasi.bolt`.
            if boltlang_hir_def::nameres::test_functions(&db, file).is_empty() {
                continue;
            }
            let p = program(&db, file);
            assert!(p.instances.len() > 1, "{name} has no instances for {target}");
            for (i, site, e) in &p.errors {
                problems
                    .push(format!("{name} ({target}): in {} at {site:?}: {e:?}", i.display(&db)));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// `imm T` in a generic function is what `imm` makes of the type it's
/// instantiated at.
#[test]
fn imm_of_a_type_parameter_in_an_instance() {
    let mut db = TestDb::new();
    db.set_prelude(Some("std"), false);
    db.add_std();
    let file = db.file("/main.bolt", "fn h(x) = imm x;\nlet a = h(mut [mut [1]]);\n");
    let p = program(&db, file);
    assert!(p.errors.is_empty(), "{:?}", p.errors);
    let h = p
        .instances
        .iter()
        .find(|i| i.instance.function(&db).is_some())
        .expect("an instance of h")
        .instance;
    let cx = crate::Cx::new(&db, h, &[]).unwrap();
    let ty = cx.instance_ty().unwrap();
    assert_eq!(ty.display(&db).to_string(), "fn(mut [mut [i32]]) -> imm [imm [i32]]");
}

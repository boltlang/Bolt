//! Tests of the MIR that bodies build to, driven by the `.txt` files in
//! `tests/mir` at the root of the repository, in the format of the
//! tree-sitter corpus. Each case is a separate test, named after its file
//! and title, e.g. `basics::a_function_that_adds`.
//!
//! A case is a title between two lines of `=`, the Bolt code, and after a
//! line of `-` the MIR of each body of the case's files, as
//! `Body::display` gives it: first a file's top-level statements, if it
//! has any, then its functions, each followed by its lambdas.
//!
//! A case can have several files, each starting with a line like
//! `//- /lib.bolt`, so that they can import each other. Without such lines,
//! the code is `/main.bolt`. With several files, each body starts with the
//! path of its file. Files get no prelude, unless the case has the
//! attribute `:prelude`, for that of the repository's `std`, or
//! `:test-prelude`, for the small one of `tests/std`.
//!
//! Lines starting with `:` after the title are attributes:
//!
//! - `:prelude` and `:test-prelude`: the prelude of the files, as above.
//! - `:types`: show the type of each node, as `Body::display_with_types`.
//! - `:skip`: never run the case, and report it as ignored.
//! - `:todo`: the output is what the builder should give, but doesn't yet.
//!   The case is ignored, unless run with `--ignored`.
//!
//! Run a single case with `cargo test --test mir adds`. To write what the
//! builder gives into the files instead of comparing it, run with
//! `UPDATE_EXPECT=1`, and review the changes with `git diff`. An update
//! leaves the output of `:skip` and `:todo` cases alone.

use std::path::Path;

use boltlang_base_db::{File, Settings, SystemPathBuf, TestDb};
use boltlang_corpus::{Case, case_trial, catch, split_files, trials};
use boltlang_hir_def::{
    body::{DefWithBody, DefWithBodyId, body},
    hir::{Expr, Statement},
    ids::{FunctionId, ItemContainerId},
    item_tree::{ModItem, item_tree},
};
use boltlang_mir::Body;
use boltlang_mir_build::mir_bodies;
use libtest_mimic::{Arguments, Failed};

#[derive(Default)]
struct Attributes {
    skip: bool,
    todo: bool,
    types: bool,
    prelude: Prelude,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum Prelude {
    #[default]
    None,
    Std,
    Test,
}

impl Attributes {
    fn parse(case: &Case) -> Result<Self, String> {
        let mut attributes = Attributes::default();
        for attribute in &case.attributes {
            match attribute.as_str() {
                "skip" => attributes.skip = true,
                "todo" => attributes.todo = true,
                "types" => attributes.types = true,
                "prelude" => attributes.prelude = Prelude::Std,
                "test-prelude" => attributes.prelude = Prelude::Test,
                other => {
                    return Err(format!(
                        "`{}`: unknown attribute `:{other}`; the attributes are `:skip`, `:todo`, \
                         `:types`, `:prelude` and `:test-prelude`",
                        case.title
                    ));
                }
            }
        }
        Ok(attributes)
    }
}

/// The MIR of a case's files.
fn build(case: &Case) -> Result<String, String> {
    let attributes = Attributes::parse(case)?;
    catch("building MIR", || {
        let files = split_files(&case.input)?;
        let mut db = TestDb::new();
        db.set_prelude((attributes.prelude != Prelude::None).then_some("std"), false);
        if files.iter().any(|(path, _)| path.starts_with(&format!("{}/", TestDb::STD_DIR))) {
            Settings::change_std_dir(&mut db, Some(SystemPathBuf::from(TestDb::STD_DIR)));
        } else if attributes.prelude == Prelude::Test {
            db.add_test_std();
        } else {
            db.add_std();
        }
        let files: Vec<(String, File)> =
            files.into_iter().map(|(path, text)| (path.clone(), db.file(&path, &text))).collect();
        let several = files.len() > 1;
        let show = |b: &Body| {
            if attributes.types { b.display_with_types(&db) } else { b.display(&db) }
        };
        let mut out = String::new();
        for (path, file) in &files {
            if path.starts_with(&format!("{}/", TestDb::STD_DIR)) {
                continue;
            }
            for def in bodies(&db, *file) {
                let bodies = mir_bodies(&db, def);
                for b in bodies.all() {
                    if several {
                        out.push_str(&format!("{path}: "));
                    }
                    out.push_str(&show(b));
                }
            }
        }
        Ok(out.trim().to_string())
    })
}

/// The bodies of a file: its top-level statements, if it has any, then its
/// functions.
fn bodies(db: &TestDb, file: File) -> Vec<DefWithBodyId<'_>> {
    let mut defs = Vec::new();
    let top = DefWithBodyId::new(db, DefWithBody::File(file));
    let hir = body(db, top);
    let has_code = match &hir[hir.body_expr] {
        Expr::Block { statements, tail, .. } => {
            tail.is_some() || statements.iter().any(|s| !matches!(s, Statement::Item))
        }
        _ => true,
    };
    if has_code {
        defs.push(top);
    }
    for item in &item_tree(db, file).top_level {
        if let ModItem::Function(idx) = *item {
            let f = FunctionId::new(db, ItemContainerId::File(file), idx);
            if f.item(db).has_body {
                defs.push(DefWithBodyId::new(db, DefWithBody::Function(f)));
            }
        }
    }
    defs
}

fn run(case: &Case) -> Result<(), Failed> {
    let actual = build(case)?;
    let expected = case.section(0);
    if actual != expected {
        return Err(format!("expected:\n{expected}\nfound:\n{actual}").into());
    }
    Ok(())
}

fn updated(case: &Case) -> Option<Vec<String>> {
    let attributes = Attributes::parse(case).ok()?;
    if attributes.skip || attributes.todo {
        return None;
    }
    let found = build(case).unwrap_or_else(|_| case.section(0).to_string());
    Some(vec![found])
}

fn main() {
    let args = Arguments::from_args();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/mir");
    let trials = trials(&root, 1, updated, |name, case| {
        let attributes = Attributes::parse(&case)?;
        Ok(case_trial(name, attributes.skip, attributes.todo, move || run(&case)))
    });
    libtest_mimic::run(&args, trials).exit();
}

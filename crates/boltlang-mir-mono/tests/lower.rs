//! Tests of what `boltlang-mir-mono` makes of a program, driven by the `.txt`
//! files in `tests/lower` at the root of the repository, in the format of
//! the tree-sitter corpus. Each case is a separate test, named after its
//! file and title, e.g. `decisions::patterns::ranges_and_arms_never_taken`.
//!
//! A case is a title between two lines of `=`, the Bolt code, and after a
//! line of `-` what the lowering makes of it. What that is depends on the
//! directory of the file:
//!
//! - `instances`: the instances of the program, each with what it calls,
//!   indented under it, then the functions that are erased and the uses
//!   that can't be compiled:
//!
//!   ```text
//!   <file>
//!     -> id[i32]
//!   id[i32]
//!   ```
//!
//! - `decisions`: the decision graph of each `match`, after a line with
//!   the value it matches on, then whether it can fail, with a value that
//!   does, and the arms that are never reached:
//!
//!   ```text
//!   match `x`
//!   n0 = switch $0 { 0 => n1, _ => n2 }
//!   n1 = arm 0
//!   n2 = arm 1
//!   ```
//!
//! - `lambdas`: each lambda, with the locals around it that it captures:
//!
//!   ```text
//!   `|x| x + n` captures n
//!   ```
//!
//! A case can have several files, each starting with a line like
//! `//- /lib.bolt`, so that they can import each other. Without such lines,
//! the code is `/main.bolt`, which is the program's entry with several
//! files too. With several files, the line of each instance or `match`
//! starts with the path of its file. Files get no prelude, unless the case
//! has the attribute `:prelude`, for that of the repository's `std`, or
//! `:test-prelude`, for the small one of `tests/std`. A case can give its
//! own `std` in `/toolchain/std`.
//!
//! Lines starting with `:` after the title are attributes:
//!
//! - `:prelude` and `:test-prelude`: the prelude of the files, as above.
//! - `:skip`: never run the case, and report it as ignored. For a case that
//!   crashes or hangs.
//! - `:todo`: the output is what the lowering should give, but doesn't yet.
//!   The case is ignored, unless run with `--ignored`.
//! - `:error`: the output must have an error, so that an update can't make
//!   it a case without errors by accident.
//!
//! Run a single case with `cargo test --test lower ranges_and_arms`.
//! To write what the lowering gives into the files instead of comparing
//! it, run with `UPDATE_EXPECT=1`, and review the changes with `git diff`.
//! An update leaves the output of `:skip` and `:todo` cases alone, and that
//! of an `:error` case without errors.

use std::path::Path;

use boltlang_base_db::{File, Settings, SystemPathBuf, TestDb, source_text};
use boltlang_corpus::{
    Case, catch, data_files, data_name, parse_cases, slug, split_files, update, updating,
};
use boltlang_hir_def::{
    body::{DefWithBody, DefWithBodyId, body_with_source_map},
    expr_store::ExpressionStoreSourceMap,
    hir::{Expr, ExprId, PatId},
    ids::{FunctionId, ItemContainerId},
    item_tree::{ModItem, item_tree},
};
use boltlang_mir_build::lambda::body_lambdas;
use boltlang_mir_mono::{ClassItem, Role, Use, UseError, program};
use boltlang_pattern::decision::{DecisionError, body_decisions};
use libtest_mimic::{Arguments, Failed, Trial};

/// What a case shows, from the directory of its file.
#[derive(Clone, Copy)]
enum Kind {
    Instances,
    Decisions,
    Lambdas,
}

/// The attributes of a case, like `:todo`.
#[derive(Default)]
struct Attributes {
    skip: bool,
    todo: bool,
    error: bool,
    prelude: Prelude,
}

/// The prelude of a case.
#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum Prelude {
    #[default]
    None,
    /// That of the repository's `std`, or of the case's own, with
    /// `:prelude`.
    Std,
    /// The small one of `tests/std`, with `:test-prelude`.
    Test,
}

impl Attributes {
    fn parse(case: &Case) -> Result<Self, String> {
        let mut attributes = Attributes::default();
        for attribute in &case.attributes {
            match attribute.as_str() {
                "skip" => attributes.skip = true,
                "todo" => attributes.todo = true,
                "error" => attributes.error = true,
                "prelude" => attributes.prelude = Prelude::Std,
                "test-prelude" => attributes.prelude = Prelude::Test,
                other => {
                    return Err(format!(
                        "`{}`: unknown attribute `:{other}`; the attributes are `:skip`, `:todo`, \
                         `:error`, `:prelude` and `:test-prelude`",
                        case.title
                    ));
                }
            }
        }
        Ok(attributes)
    }

    /// Whether the output is written by hand, so that updates keep it.
    fn manual(&self) -> bool {
        self.skip || self.todo
    }
}

/// The files of a case, in a database with the prelude it asks for.
struct Program {
    db: TestDb,
    files: Vec<(String, File)>,
}

impl Program {
    fn new(input: &str, prelude: Prelude) -> Result<Self, String> {
        let files = split_files(input)?;
        let mut db = TestDb::new();
        db.set_prelude((prelude != Prelude::None).then_some("std"), false);
        if files.iter().any(|(path, _)| path.starts_with(&format!("{}/", TestDb::STD_DIR))) {
            Settings::change_std_dir(&mut db, Some(SystemPathBuf::from(TestDb::STD_DIR)));
        } else if prelude == Prelude::Test {
            db.add_test_std();
        } else {
            db.add_std();
        }
        let files =
            files.into_iter().map(|(path, text)| (path.clone(), db.file(&path, &text))).collect();
        Ok(Program { db, files })
    }

    /// What starts the line of something in `file`: its path, with several
    /// files.
    fn prefix(&self, file: File) -> Option<String> {
        let (path, _) = self.files.iter().find(|(_, f)| *f == file)?;
        Some(if self.files.len() > 1 { format!("{path}: ") } else { String::new() })
    }

    /// The source text of an expression or a pattern of `def`.
    fn snippet(&self, def: DefWithBodyId<'_>, node: Node) -> String {
        let text = source_text(&self.db, def.def(&self.db).file(&self.db));
        let (_, map): &(_, ExpressionStoreSourceMap) = body_with_source_map(&self.db, def);
        let ptr = match node {
            Node::Expr(e) => map.expr_syntax(e),
            Node::Pat(p) => map.pat_syntax(p),
        };
        ptr.map_or("?".to_string(), |p| text[p.start as usize..p.end as usize].to_string())
    }

    /// The instances of the program, from `/main.bolt`, that are of the
    /// case's files, with what each one calls, then what was erased and
    /// what went wrong.
    fn instances(&self) -> Result<String, String> {
        let db = &self.db;
        let (_, main) =
            self.files.iter().find(|(path, _)| path == "/main.bolt").ok_or_else(|| {
                "a case of `instances` needs a file `/main.bolt`, the program's entry".to_string()
            })?;
        let p = program(db, *main);
        let mut out = String::new();
        for i in &p.instances {
            // Those of `std` would drown out the program's.
            let Some(prefix) = i.instance.file(db).and_then(|f| self.prefix(f)) else { continue };
            out.push_str(&format!("{prefix}{}\n", i.instance.display(db)));
            for (_, u) in &i.uses {
                match u {
                    Use::Instance { instance, mutabilities, rows, dicts } => {
                        out.push_str(&format!("  -> {}", instance.display(db)));
                        if !mutabilities.is_empty() {
                            let ms: Vec<String> =
                                mutabilities.iter().map(|m| m.to_string()).collect();
                            out.push_str(&format!(" with {}", ms.join(", ")));
                        }
                        if rows.iter().any(|r| !r.is_nothing()) {
                            let rs: Vec<String> = rows.iter().map(|r| r.display(db)).collect();
                            out.push_str(&format!(" rows {}", rs.join(", ")));
                        }
                        for d in dicts.iter() {
                            out.push_str(&format!(" dict {}", d.display(db)));
                        }
                        out.push('\n');
                    }
                    Use::Method { dict, item, .. } => {
                        let name = match item {
                            ClassItem::Function(f) => {
                                f.item(db).display_name().map(|n| n.to_string())
                            }
                            ClassItem::Const(c) => c.item(db).name.as_ref().map(|n| n.to_string()),
                        };
                        let name = name.unwrap_or_default();
                        out.push_str(&format!("  -> {name} of {}\n", dict.display(db)));
                    }
                    Use::Builtin(_) | Use::Extern(_) | Use::Op { .. } => {}
                }
            }
        }
        for f in &p.erased {
            let name = f.item(db).display_name().map(|n| n.to_string()).unwrap_or_default();
            out.push_str(&format!("erased {name}\n"));
        }
        for (i, site, e) in &p.errors {
            let body = i.body(db);
            let snippet = |site: &boltlang_mir_mono::Site<'_>| match body
                .and_then(|b| site.origin(db, b).zip(Some(b)))
            {
                Some((e, b)) => self.snippet(b, Node::Expr(e)),
                None => "?".to_string(),
            };
            let at = match site {
                Some(s) => match s.role {
                    Role::Item => format!(" at `{}`", snippet(s)),
                    Role::Set => format!(" at the `set_index` of `{}`", snippet(s)),
                    Role::Get => format!(" at the `index` of `{}`", snippet(s)),
                    Role::Op => format!(" at the operator of `{}`", snippet(s)),
                    Role::Iter => format!(" at the `iter` of `{}`", snippet(s)),
                    Role::Next => format!(" at the `next` of `{}`", snippet(s)),
                    Role::Const(_) => format!(" at a constant in `{}`", snippet(s)),
                    Role::Imm => format!(" at the copy of `{}`", snippet(s)),
                },
                None => String::new(),
            };
            let error = match e {
                UseError::UnknownType => "a type isn't known".to_string(),
                UseError::Unsupported(what) => format!("{what} isn't supported yet"),
                UseError::Cycle => "its type depends on itself across files".to_string(),
                UseError::NoImpl(c) => format!("nothing gives `{c}`"),
            };
            out.push_str(&format!("error in {}{at}: {error}\n", i.display(db)));
        }
        Ok(out)
    }

    /// The bodies of a file: its top-level statements, then its functions.
    fn bodies(&self, file: File) -> Vec<DefWithBodyId<'_>> {
        let db = &self.db;
        let mut defs = vec![DefWithBodyId::new(db, DefWithBody::File(file))];
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

    /// The lambdas of the case's files, of their top-level statements and
    /// functions, each with what it captures.
    fn lambdas(&self) -> String {
        let db = &self.db;
        let mut out = String::new();
        for &(_, file) in &self.files {
            let prefix = self.prefix(file).unwrap_or_default();
            for def in self.bodies(file) {
                let (body, _) = body_with_source_map(db, def);
                for lambda in body_lambdas(db, def) {
                    let names: Vec<&str> =
                        lambda.captures.iter().map(|&b| body.bindings[b].name.as_str()).collect();
                    let captures =
                        if names.is_empty() { "nothing".to_string() } else { names.join(", ") };
                    out.push_str(&format!(
                        "{prefix}`{}` captures {captures}\n",
                        self.snippet(def, Node::Expr(lambda.expr))
                    ));
                }
            }
        }
        out
    }

    /// The decisions of the `match`es of the case's files, of their
    /// top-level statements and functions.
    fn decisions(&self) -> String {
        let db = &self.db;
        let mut out = String::new();
        for &(_, file) in &self.files {
            let prefix = self.prefix(file).unwrap_or_default();
            for def in self.bodies(file) {
                let (body, _) = body_with_source_map(db, def);
                for (expr, found) in &body_decisions(db, def).matches {
                    let Expr::Match { expr: value, .. } = &body[*expr] else { continue };
                    out.push_str(&format!(
                        "{prefix}match `{}`\n",
                        self.snippet(def, Node::Expr(*value))
                    ));
                    let d = match found {
                        Ok(d) => d,
                        Err(DecisionError::Unsupported(p)) => {
                            let p = self.snippet(def, Node::Pat(*p));
                            out.push_str(&format!(
                                "error: the pattern `{p}` isn't supported yet\n"
                            ));
                            continue;
                        }
                        Err(DecisionError::Unknown(p)) => {
                            let p = self.snippet(def, Node::Pat(*p));
                            out.push_str(&format!("error: the pattern `{p}` isn't known\n"));
                            continue;
                        }
                    };
                    out.push_str(&d.display(db));
                    if let Some(missing) = d.missing(db) {
                        out.push_str(&format!("missing {missing}\n"));
                    }
                    let unreached: Vec<String> = d
                        .reached_arms()
                        .iter()
                        .enumerate()
                        .filter(|(_, r)| !**r)
                        .map(|(i, _)| i.to_string())
                        .collect();
                    if !unreached.is_empty() {
                        out.push_str(&format!("unreached {}\n", unreached.join(", ")));
                    }
                }
            }
        }
        out
    }
}

#[derive(Clone, Copy)]
enum Node {
    Expr(ExprId),
    Pat(PatId),
}

/// What the lowering gives for a case, or why it couldn't: its files are
/// wrong, or it panicked.
fn lower(case: &Case, kind: Kind) -> Result<String, String> {
    let prelude = Attributes::parse(case)?.prelude;
    catch("the lowering", || {
        let program = Program::new(&case.input, prelude)?;
        let out = match kind {
            Kind::Instances => program.instances()?,
            Kind::Decisions => program.decisions(),
            Kind::Lambdas => program.lambdas(),
        };
        Ok(out.trim().to_string())
    })
}

/// Whether an output has an error, on a line of its own.
fn has_errors(output: &str) -> bool {
    output.lines().any(|line| line.starts_with("error"))
}

fn run(case: &Case, kind: Kind, attributes: &Attributes) -> Result<(), Failed> {
    let actual = lower(case, kind)?;
    let expected = case.section(0);
    if actual != expected {
        return Err(format!("expected:\n{expected}\nfound:\n{actual}").into());
    }
    if attributes.error && !has_errors(&actual) {
        return Err("the case is marked `:error`, but the lowering found no errors".into());
    }
    Ok(())
}

/// What an update writes for a case: what the lowering gives, or `None` to
/// keep it. A case that can't be lowered keeps its output, and fails when
/// it runs.
fn updated(case: &Case, kind: Kind) -> Option<Vec<String>> {
    let attributes = Attributes::parse(case).ok()?;
    if attributes.manual() {
        return None;
    }
    let found = match lower(case, kind) {
        Ok(found) if !attributes.error || has_errors(&found) => found,
        _ => case.section(0).to_string(),
    };
    Some(vec![found])
}

fn main() {
    let args = Arguments::from_args();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/lower");
    let mut trials = Vec::new();
    for path in data_files(&root) {
        let stem = data_name(&root, &path);
        let kind = match stem.split("::").next() {
            Some("instances") => Kind::Instances,
            Some("decisions") => Kind::Decisions,
            Some("lambdas") => Kind::Lambdas,
            _ => {
                let message = format!(
                    "{}: a file of `tests/lower` must be in `instances`, `decisions` or `lambdas`",
                    path.display()
                );
                trials.push(Trial::test(stem, move || Err(message.into())));
                continue;
            }
        };
        let mut text = std::fs::read_to_string(&path).unwrap();
        if updating()
            && let Ok(cases) = parse_cases(&text, 1)
        {
            let new = update(&text, &cases, |case| updated(case, kind));
            if new != text {
                std::fs::write(&path, &new).unwrap();
                text = new;
            }
        }
        let cases = match parse_cases(&text, 1) {
            Ok(cases) => cases,
            Err(error) => {
                let message = format!("{}: {error}", path.display());
                trials.push(Trial::test(stem, move || Err(message.into())));
                continue;
            }
        };
        let mut seen = Vec::new();
        for case in cases {
            let name = format!("{stem}::{}", slug(&case.title));
            if seen.contains(&name) {
                let message = format!("{}: two cases are called `{}`", path.display(), case.title);
                trials.push(Trial::test(format!("{name}_again"), move || Err(message.into())));
                continue;
            }
            seen.push(name.clone());
            let attributes = match Attributes::parse(&case) {
                Ok(attributes) => attributes,
                Err(error) => {
                    let message = format!("{}: {error}", path.display());
                    trials.push(Trial::test(name, move || Err(message.into())));
                    continue;
                }
            };
            let trial = if attributes.skip {
                // Never run, even with `--ignored`, since it may hang.
                Trial::test(name, || Ok(())).with_kind("skip").with_ignored_flag(true)
            } else if attributes.todo {
                Trial::test(name, move || run(&case, kind, &attributes))
                    .with_kind("todo")
                    .with_ignored_flag(true)
            } else {
                Trial::test(name, move || run(&case, kind, &attributes))
            };
            trials.push(trial);
        }
    }
    libtest_mimic::run(&args, trials).exit();
}

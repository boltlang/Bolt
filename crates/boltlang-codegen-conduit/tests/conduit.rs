//! Tests of lowering Bolt to Conduit, driven by the `.txt` files in
//! `tests/conduit` at the root of the repository, in the format of the
//! tree-sitter corpus. Each case is a separate test, named after its file
//! and title, e.g. `control_flow::a_while_loop`.
//!
//! A case is a title between two lines of `=`, the Bolt code, then two
//! sections of output, each after a line of `-`:
//!
//! 1. The Conduit IR the code lowers to, with functions named after what
//!    they're instances of, like `id_i32` for `id[i32]`. Or why it
//!    couldn't be lowered, as lines starting with `error`.
//! 2. What running it does, in Conduit's evaluator: the entry file's
//!    top-level statements, then each test. Every call of a host function
//!    is a line, like `env.log(3)`, and a test that fails, or a run that
//!    stops with an error, says so. The host functions give nothing back.
//!
//! Host functions come from `extern` blocks, in the module the block
//! names, like `extern "env" { fn log(x: i32); }`. The IR must pass
//! Conduit's verifier, or the case fails.
//!
//! A case can have several files, each starting with a line like
//! `//- /lib.bolt`, with `/main.bolt` as the entry. Files get no prelude,
//! unless the case has the attribute `:prelude`, for that of the
//! repository's `std`, or `:test-prelude`, for the small one of
//! `tests/std`.
//!
//! Lines starting with `:` after the title are attributes:
//!
//! - `:prelude` and `:test-prelude`: the prelude of the files, as above.
//! - `:skip`: never run the case, and report it as ignored.
//! - `:todo`: the output is what the lowering should give, but doesn't yet.
//!   The case is ignored, unless run with `--ignored`.
//! - `:error`: the lowering must fail, so that an update can't make it a
//!   case without errors by accident.
//!
//! Run a single case with `cargo test --test conduit a_while_loop`. To
//! write what the lowering gives into the files instead of comparing it,
//! run with `UPDATE_EXPECT=1`, and review the changes with `git diff`.

use std::{cell::RefCell, path::Path, rc::Rc};

use boltlang_base_db::{Settings, SystemPathBuf, TestDb};
use boltlang_codegen_conduit::lower;
use boltlang_corpus::{
    Case, catch, data_files, data_name, parse_cases, slug, split_files, update, updating,
};
use conduit_eval::{Host, Machine, Value};
use conduit_ir::{Extern, ExternId, Module};
use libtest_mimic::{Arguments, Failed, Trial};

#[derive(Default)]
struct Attributes {
    skip: bool,
    todo: bool,
    error: bool,
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

    fn manual(&self) -> bool {
        self.skip || self.todo
    }
}

/// A host that writes down every call. The module `bolt`'s `write_file`
/// and `flush` are a line for each flush instead, like `stdout: hi`.
#[derive(Default, Clone)]
struct Log {
    lines: Rc<RefCell<Vec<String>>>,
    /// What each file has waiting, by its number.
    files: Rc<RefCell<std::collections::BTreeMap<i32, Vec<u8>>>>,
}

impl Host for Log {
    fn call(&mut self, _: ExternId, decl: &Extern, args: &[Value]) -> Result<Vec<Value>, String> {
        if &*decl.module == "bolt" {
            match (&*decl.name, args) {
                ("write_file", [Value::I32(file), Value::I32(byte)]) => {
                    self.files.borrow_mut().entry(*file).or_default().push(*byte as u8);
                    return Ok(Vec::new());
                }
                ("flush", [Value::I32(file)]) => {
                    let bytes = self.files.borrow_mut().remove(file).unwrap_or_default();
                    let name = match file {
                        1 => "stdout".to_string(),
                        2 => "stderr".to_string(),
                        n => format!("file {n}"),
                    };
                    let text = String::from_utf8_lossy(&bytes);
                    for line in text.strip_suffix('\n').unwrap_or(&text).split('\n') {
                        let line = format!("{name}: {line}");
                        self.lines.borrow_mut().push(line.trim_end().to_string());
                    }
                    return Ok(Vec::new());
                }
                _ => {}
            }
        }
        let args: Vec<String> = args.iter().map(show_value).collect();
        self.lines.borrow_mut().push(format!("{}.{}({})", decl.module, decl.name, args.join(", ")));
        if !decl.sig.results.is_empty() {
            return Err(format!("`{}` gives a value, which the tests' host can't", decl.name));
        }
        Ok(Vec::new())
    }
}

fn show_value(v: &Value) -> String {
    match v {
        Value::I8(n) => n.to_string(),
        Value::I16(n) => n.to_string(),
        Value::I32(n) => n.to_string(),
        Value::I64(n) => n.to_string(),
        Value::F32(x) => x.to_string(),
        Value::F64(x) => x.to_string(),
        Value::Obj(id) => format!("#{}", id.0),
        Value::Unset => "unset".to_string(),
    }
}

/// The IR, or the errors, and what running it does.
fn check(input: &str, prelude: Prelude) -> Result<(String, String), String> {
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
    let files: Vec<_> =
        files.iter().map(|(path, text)| (path.clone(), db.file(path, text))).collect();
    let (_, main) = files
        .iter()
        .find(|(path, _)| path == "/main.bolt")
        .ok_or("a case needs a file `/main.bolt`, the program's entry")?;
    let lowered = match lower(&db, *main) {
        Ok(lowered) => lowered,
        Err(errors) => {
            let lines: Vec<String> = errors.iter().map(|e| format!("error {e}")).collect();
            return Ok((lines.join("\n"), String::new()));
        }
    };
    let ir = conduit_ir::text::print(&lowered.module);
    let mut run = Vec::new();
    run.extend(execute(&lowered.module, lowered.entry, None));
    for (name, f) in &lowered.tests {
        run.extend(execute(&lowered.module, *f, Some(name)));
    }
    Ok((ir.trim().to_string(), run.join("\n")))
}

/// What running `f` does, and how it ends if it fails.
fn execute(module: &Module, f: conduit_ir::FuncId, test: Option<&str>) -> Vec<String> {
    let log = Log::default();
    let mut machine = Machine::new(module, log.clone());
    let result = machine.call(f, Vec::new());
    let mut lines = Vec::new();
    if let Some(name) = test {
        lines.push(format!("test {name}"));
    }
    lines.extend(log.lines.take());
    if let Err(e) = result {
        lines.push(format!("error: {e}"));
    }
    lines
}

fn lower_caught(case: &Case) -> Result<(String, String), String> {
    let prelude = Attributes::parse(case)?.prelude;
    catch("the lowering", || check(&case.input, prelude))
}

fn has_errors((ir, _): &(String, String)) -> bool {
    ir.lines().any(|line| line.starts_with("error"))
}

fn run(case: &Case, attributes: &Attributes) -> Result<(), Failed> {
    let actual = lower_caught(case)?;
    let expected = (case.section(0).to_string(), case.section(1).to_string());
    if actual != expected {
        return Err(format!(
            "expected:\n{}\nfound:\n{}",
            boltlang_corpus::show(&[expected.0, expected.1]),
            boltlang_corpus::show(&[actual.0, actual.1])
        )
        .into());
    }
    if attributes.error && !has_errors(&actual) {
        return Err("the case is marked `:error`, but it was lowered without errors".into());
    }
    Ok(())
}

fn updated(case: &Case) -> Option<Vec<String>> {
    let attributes = Attributes::parse(case).ok()?;
    if attributes.manual() {
        return None;
    }
    let found = match lower_caught(case) {
        Ok(found) if !attributes.error || has_errors(&found) => found,
        _ => (case.section(0).to_string(), case.section(1).to_string()),
    };
    Some(vec![found.0, found.1])
}

fn main() {
    let args = Arguments::from_args();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/conduit");
    let mut trials = Vec::new();
    for path in data_files(&root) {
        let stem = data_name(&root, &path);
        let mut text = std::fs::read_to_string(&path).unwrap();
        if updating()
            && let Ok(cases) = parse_cases(&text, 2)
        {
            let new = update(&text, &cases, updated);
            if new != text {
                std::fs::write(&path, &new).unwrap();
                text = new;
            }
        }
        let cases = match parse_cases(&text, 2) {
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
                Trial::test(name, || Ok(())).with_kind("skip").with_ignored_flag(true)
            } else if attributes.todo {
                Trial::test(name, move || run(&case, &attributes))
                    .with_kind("todo")
                    .with_ignored_flag(true)
            } else {
                Trial::test(name, move || run(&case, &attributes))
            };
            trials.push(trial);
        }
    }
    libtest_mimic::run(&args, trials).exit();
}

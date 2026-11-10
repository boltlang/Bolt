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
use boltlang_corpus::{Case, case_trial, catch, split_files, trials};
use conduit_eval::{Host, HostError, Machine, Value};
use conduit_ir::{Extern, Module};
use libtest_mimic::{Arguments, Failed};

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

/// A host that writes down every call, by its `emu` name. `write` to
/// stdout or stderr, as std's and the test prelude's `print` do, is a line
/// for each line it writes instead, like `stdout: hi`, once its newline
/// is written.
#[derive(Default, Clone)]
struct Log {
    lines: Rc<RefCell<Vec<String>>>,
    /// The text of a line on stdout or stderr that has no newline yet.
    pending: Rc<RefCell<std::collections::BTreeMap<i32, String>>>,
}

impl Log {
    /// Write down what's still pending, as at the end of a run.
    fn finish(&self) {
        for (fd, text) in std::mem::take(&mut *self.pending.borrow_mut()) {
            self.line(fd, &text);
        }
    }

    fn line(&self, fd: i32, text: &str) {
        let file = if fd == 1 { "stdout" } else { "stderr" };
        self.lines.borrow_mut().push(format!("{file}: {text}").trim_end().to_string());
    }
}

impl Host for Log {
    fn call(
        &mut self,
        name: &str,
        decl: &Extern,
        args: &[Value],
        arrays: &mut conduit_eval::Arrays<'_, '_>,
    ) -> Result<Vec<Value>, HostError> {
        if name == "write"
            && let [Value::I32(fd @ (1 | 2)), buf] = args
        {
            let bytes: Vec<u8> = arrays
                .get(buf)
                .unwrap_or_default()
                .iter()
                .filter_map(|b| if let Value::I8(b) = b { Some(*b as u8) } else { None })
                .collect();
            let mut pending = self.pending.borrow_mut().remove(fd).unwrap_or_default();
            pending.push_str(&String::from_utf8_lossy(&bytes));
            while let Some(end) = pending.find('\n') {
                self.line(*fd, &pending[..end]);
                pending.drain(..=end);
            }
            if !pending.is_empty() {
                self.pending.borrow_mut().insert(*fd, pending);
            }
            // Success, and how many bytes it wrote.
            let values = [0, bytes.len() as i64];
            let results = decl.sig.results.iter().zip(values).map(|(t, n)| match t {
                conduit_ir::Ty::I64 => Value::I64(n),
                conduit_ir::Ty::I16 => Value::I16(n as i16),
                conduit_ir::Ty::I8 => Value::I8(n as i8),
                _ => Value::I32(n as i32),
            });
            return Ok(results.collect());
        }
        // A flat extern's arrays are shown with their elements, like
        // `[1, 2]`, and its results are 1, 2 and so on, so that a test sees
        // where each goes.
        let flat = decl.abi == conduit_ir::Abi::Flat;
        let show = |v: &Value| if flat { show_array(arrays, v) } else { show_value(v) };
        let args: Vec<String> = args.iter().map(show).collect();
        self.lines.borrow_mut().push(format!("{name}({})", args.join(", ")));
        if flat {
            let results = decl.sig.results.iter().enumerate().map(|(i, t)| match t {
                conduit_ir::Ty::I64 => Value::I64(i as i64 + 1),
                conduit_ir::Ty::I16 => Value::I16(i as i16 + 1),
                conduit_ir::Ty::I8 => Value::I8(i as i8 + 1),
                _ => Value::I32(i as i32 + 1),
            });
            return Ok(results.collect());
        }
        if !decl.sig.results.is_empty() {
            return Err(format!("`{name}` gives a value, which the tests' host can't").into());
        }
        Ok(Vec::new())
    }
}

/// A value, with the elements of an array in it, as `[1, [2, 3]]`.
fn show_array(arrays: &conduit_eval::Arrays<'_, '_>, v: &Value) -> String {
    match arrays.get(v) {
        Some(elems) => {
            let elems: Vec<String> = elems.iter().map(|e| show_array(arrays, e)).collect();
            format!("[{}]", elems.join(", "))
        }
        None => show_value(v),
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
        Value::Host(n) => format!("host {n}"),
        Value::Null => "null".to_string(),
        Value::Function(n) => format!("host function {n}"),
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
    log.finish();
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
    let trials = trials(&root, 2, updated, |name, case| {
        let attributes = Attributes::parse(&case)?;
        let (skip, todo) = (attributes.skip, attributes.todo);
        Ok(case_trial(name, skip, todo, move || run(&case, &attributes)))
    });
    libtest_mimic::run(&args, trials).exit();
}

//! Tests of what the code generator emits. That the compiled code gives
//! the same output as the evaluator is tested by the programs in
//! `tests/run`: see `tests/run.rs`.

use boltlang_base_db::{File, TestDb};
use boltlang_eval::{ErrorKind, Host};
use wasmi::{Caller, Linker, Module, Store, Val};

use crate::{
    RunError, link, reachable_instances, run,
    run::{State, engine, linker},
};

/// A host that keeps what was printed, and the values passed to the
/// imports `env.log` and `env.log64`.
#[derive(Default)]
struct TestHost {
    logged: Vec<i64>,
    printed: String,
}

impl Host for TestHost {
    fn print(&mut self, text: &str) -> Result<(), ErrorKind> {
        self.printed.push_str(text);
        self.printed.push('\n');
        Ok(())
    }
}

fn compile(db: &TestDb, file: File) -> Vec<u8> {
    link(db, file).unwrap_or_else(|errors| {
        let messages: Vec<_> = errors.iter().map(|e| e.to_string()).collect();
        panic!("compiling failed: {}", messages.join("; "))
    })
}

/// Instantiate a module, with the builtins and the imports `env.log` and
/// `env.log64`.
fn instantiate(bytes: &[u8]) -> (Store<State<TestHost>>, wasmi::Instance) {
    let engine = engine(None);
    let module = Module::new(&engine, bytes).unwrap();
    let mut store = Store::new(&engine, State::new(TestHost::default()));
    let mut linker: Linker<State<TestHost>> = linker(&engine);
    linker
        .func_wrap("env", "log", |mut caller: Caller<'_, State<TestHost>>, x: i32| {
            caller.data_mut().host.logged.push(x as i64)
        })
        .unwrap();
    linker
        .func_wrap("env", "log64", |mut caller: Caller<'_, State<TestHost>>, x: i64| {
            caller.data_mut().host.logged.push(x)
        })
        .unwrap();
    let instance = linker.instantiate_and_start(&mut store, &module).unwrap();
    (store, instance)
}

/// Call an exported function, and give its result.
fn call(store: &mut Store<State<TestHost>>, instance: &wasmi::Instance, name: &str, args: &[Val]) -> Option<Val> {
    let func = instance.get_func(&*store, name).unwrap_or_else(|| panic!("no export `{name}`"));
    let mut results = vec![Val::I32(0); func.ty(&*store).results().len()];
    func.call(&mut *store, args, &mut results).unwrap();
    results.pop()
}

/// Run `main`, and give its result and what was logged.
fn run_main(code: &str) -> (Option<Val>, Vec<i64>) {
    let db = TestDb::new();
    let file = db.file("/main.bolt", code);
    let (mut store, instance) = instantiate(&compile(&db, file));
    let result = call(&mut store, &instance, "main", &[]);
    (result, std::mem::take(&mut store.data_mut().host.logged))
}

#[test]
fn generic_functions_are_monomorphized() {
    let code = "fn id(x) = x;\nid(1) as f64 + id(1.5);";
    let db = TestDb::new();
    let file = db.file("/main.bolt", code);
    // `main`, `id` for `i32` and `id` for `f64`.
    assert_eq!(reachable_instances(&db, file).len(), 3);
}

#[test]
fn narrow_integers_wrap_around() {
    // The evaluator has no fixed sizes, so this is Wasm's own behavior.
    assert_eq!(run_main("let x: u8 = 255;\nx + 1;").0.map(|v| v.i32()), Some(Some(0)));
    assert_eq!(run_main("let x: i8 = 127;\nx + 1;").0.map(|v| v.i32()), Some(Some(-128)));
    assert_eq!(run_main("let x: u16 = 0;\nx - 1;").0.map(|v| v.i32()), Some(Some(65535)));
}

#[test]
fn extern_functions_are_imports() {
    let (result, logged) = run_main("extern { fn log(x: i32); fn log64(x: i64); }\nfn f(n: i32) { log(n); log(n * 2); }\nf(3);\nlog64(7);");
    assert!(result.is_none());
    assert_eq!(logged, [3, 6, 7]);
}

#[test]
fn functions_are_exported() {
    let db = TestDb::new();
    let file = db.file("/main.bolt", "fn fac(n: i64) -> i64 = if n == 0 { 1 } else { n * fac(n - 1) };\nfac(1);");
    let (mut store, instance) = instantiate(&compile(&db, file));
    let result = call(&mut store, &instance, "fac", &[Val::I64(20)]);
    assert_eq!(result.and_then(|v| v.i64()), Some(2432902008176640000));
}

const COLOR: &str = "enum Color { Red, Green, Blue }\n";

#[test]
fn enums_with_fields_and_ordering_are_errors() {
    let errors = |code: &str| {
        let db = TestDb::new();
        let file = db.file("/main.bolt", code);
        link(&db, file).unwrap_err().iter().map(|e| e.to_string()).collect::<Vec<_>>()
    };
    assert_eq!(
        errors("enum Shape { Dot, Circle(f64) }\nShape.Dot;"),
        ["a value of type `Shape` is not supported by the code generator yet"]
    );
    assert_eq!(
        errors(&format!("{COLOR}Color.Red < Color.Blue;")),
        ["ordering enums is not supported by the code generator yet"]
    );
}

#[test]
fn unsupported_code_is_an_error() {
    let db = TestDb::new();
    let file = db.file("/main.bolt", "struct P { x: i32 }\nfn f() -> i32 = 1;\nlet p = P { x: f() };\n");
    let errors = link(&db, file).unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].to_string(), "a struct is not supported by the code generator yet");
    let (_, range) = errors[0].source_range(&db).unwrap();
    assert_eq!(&"struct P { x: i32 }\nfn f() -> i32 = 1;\nlet p = P { x: f() };\n"[range.start as usize..range.end as usize], "P { x: f() }");
}

#[test]
fn editing_a_function_recompiles_only_that_function() {
    let mut db = TestDb::new();
    let file = db.file("/main.bolt", "fn f(n: i32) -> i32 = n + 1;\nfn g(n: i32) -> i32 = n * 2;\nf(g(1));");
    compile(&db, file);
    let executed = db.log_executed(|db| {
        db.edit(file, "fn f(n: i32) -> i32 = n + 2;\nfn g(n: i32) -> i32 = n * 2;\nf(g(1));");
        let (mut store, instance) = instantiate(&compile(db, file));
        assert_eq!(call(&mut store, &instance, "main", &[]).and_then(|v| v.i32()), Some(4));
    });
    let compiled: Vec<_> = executed.iter().filter(|q| q.starts_with("compile_instance")).collect();
    assert_eq!(compiled.len(), 1, "{executed:#?}");
}

/// A prelude that declares the builtins, as `std`'s does.
const BUILTINS: &str = "extern \"bolt\" {\n  pub fn print(text: String);\n  pub fn panic(message: String) -> !;\n}\n";

fn db_with_builtins(code: &str) -> (TestDb, File) {
    let mut db = TestDb::new();
    db.file("/.bolt/packages/std/__prelude__/lib.bolt", BUILTINS);
    db.set_prelude(Some("std"), true);
    let file = db.file("/main.bolt", code);
    (db, file)
}

#[test]
fn strings_are_stored_once() {
    let (db, file) = db_with_builtins("print(\"twice\");\nprint(\"twice\");\nprint(\"other\");");
    let bytes = compile(&db, file);
    let count = |needle: &[u8]| bytes.windows(needle.len()).filter(|w| *w == needle).count();
    assert_eq!(count(b"twice"), 1);
    assert_eq!(count(b"other"), 1);
}

#[test]
fn panic_traps_with_its_message() {
    let (db, file) =
        db_with_builtins("fn check(n: i32) -> i32 = if n < 0 { panic(\"negative\") } else { n };\ncheck(1) + check(-1);");
    let (mut store, instance) = instantiate(&compile(&db, file));
    let func = instance.get_func(&store, "main").unwrap();
    let mut results = [Val::I32(0)];
    let error = func.call(&mut store, &[], &mut results).unwrap_err();
    assert!(error.to_string().contains("panicked: negative"), "{error}");
}

/// Run `code` as `bolt run --wasm` does, with `fuel` when it's given.
fn run_wasm(code: &str, fuel: Option<u64>) -> Result<String, RunError> {
    let (db, file) = db_with_builtins(code);
    run(&db, file, &compile(&db, file), TestHost::default(), fuel).map(|value| value.display(&db).to_string())
}

#[test]
fn extern_functions_cant_be_run() {
    assert_eq!(
        run_wasm("extern { fn log(x: i32); }\nlog(1);", None),
        Err(RunError::MissingImport { module: "env".into(), name: "log".into() })
    );
}

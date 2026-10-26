//! Most tests are differential: the evaluator and the compiled code must
//! agree on what a program gives.

use boltlang_base_db::{File, TestDb};
use boltlang_eval::{ErrorKind, Evaluator, Host, Recorder};
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

/// The evaluator and the compiled code give the same result.
#[track_caller]
fn same(code: &str) {
    let db = TestDb::new();
    let file = db.file("/main.bolt", code);
    let expected = Evaluator::new(&db).eval_file(file).unwrap();
    let found = run(&db, file, &compile(&db, file), TestHost::default(), None).unwrap();
    assert_eq!(found.display(&db).to_string(), expected.display(&db).to_string(), "{code}");
}

#[test]
fn arithmetic_and_comparisons() {
    same("1 + 2 * 3 - 4 / 2;");
    same("17 % 5;");
    same("7 - 10;");
    same("1 < 2 && 2 <= 2 || False;");
    same("!(3 == 4);");
    same("1 << 4 | 1;");
    same("-(5 * 5);");
    same("1.5 * 2.0 - 0.25;");
}

#[test]
fn recursion() {
    same("fn fac(n: i32) -> i32 = match n { 0 | 1 => 1, k => k * fac(k - 1) };\nfac(10);");
    same("fn fib(n: i32) -> i32 = if n < 2 { n } else { fib(n - 1) + fib(n - 2) };\nfib(15);");
    // Inferred, mutually recursive functions.
    same("fn even(n) = if n == 0 { True } else { odd(n - 1) };\nfn odd(n) = if n == 0 { False } else { even(n - 1) };\neven(10);");
}

#[test]
fn loops() {
    same("let mut i = 0;\nlet mut sum = 0;\nwhile i < 10 { i += 1; if i % 2 == 0 { continue; } sum += i; }\nsum;");
    same("let mut n = 1;\nlet x = loop { if n * n > 50 { break n * n; } n += 1; };\nx;");
    same(
        "let mut steps = 0;\nlet mut i = 0;\n'outer: while i < 3 { i += 1; let mut j = 0; while j < 3 { j += 1; steps += 1; if i * j == 4 { break 'outer; } } }\nsteps;",
    );
    same("let v = 'b: { if 12 > 10 { break 'b 1; } 2 };\nv;");
}

#[test]
fn early_return() {
    same("fn first_big(a: i32, b: i32) -> i32 { if a > 10 { return a; } if b > 10 { return b; } 0 }\nfirst_big(3, 12);");
}

#[test]
fn patterns() {
    let classify = "fn classify(n: i32) -> i32 = match n {
          0 => 0,
          1 | 2 | 3 => 1,
          4..=9 => 2,
          x if x < 0 => -1,
          _ => 3,
        };\n";
    for n in ["0", "2", "7", "-5", "100"] {
        same(&format!("{classify}classify({n});"));
    }
    same("let x = 4;\nif let 1..=5 = x { 1 } else { 0 };");
    same("fn half(n: i32) -> i32 { let 0..=9 = n else { return -1; }; n / 2 }\nhalf(8);");
    same("fn half(n: i32) -> i32 { let 0..=9 = n else { return -1; }; n / 2 }\nhalf(80);");
    same("fn pick(b: Bool) -> i32 = match b { True => 1, False => 2 };\npick(1 > 2);");
}

#[test]
fn generic_functions_are_monomorphized() {
    let code = "fn id(x) = x;\nid(1) as f64 + id(1.5);";
    same(code);
    let db = TestDb::new();
    let file = db.file("/main.bolt", code);
    // `main`, `id` for `i32` and `id` for `f64`.
    assert_eq!(reachable_instances(&db, file).len(), 3);
}

#[test]
fn casts_and_64_bit_integers() {
    same("7 as f64 / 2.0;");
    same("3.9 as i32;");
    same("fn big(n: i64) -> i64 = n * 1000000000;\nbig(10);");
    same("let x: i64 = 1 << 40;\n(x >> 38) as i32;");
}

#[test]
fn consts_are_computed_at_compile_time() {
    same("const N: i32 = 6 * 7;\nN + 1;");
}

#[test]
fn method_calls_and_operators() {
    same("fn double(n: i32) -> i32 = n * 2;\n5.double().double();");
    same("fn a <+> b = a * 10 + b;\n1 <+> 2 <+> 3;");
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
fn fieldless_enums() {
    // A value is the index of its variant.
    same(&format!("{COLOR}Color.Blue;"));
    same(&format!("{COLOR}let c = Color.Green;\nc;"));
    // In parameters, results and matches.
    same(&format!(
        "{COLOR}fn next(c: Color) -> Color = match c {{ Color.Red => Color.Green, Color.Green => Color.Blue, Color.Blue => Color.Red }};\nnext(next(Color.Red));"
    ));
    // Variants imported into scope, in expressions and patterns.
    same(&format!(
        "{COLOR}import Color.*;\nfn code(c) = match c {{ Red => 1, Green => 2, Blue => 3 }};\ncode(Blue) * 10 + code(Red);"
    ));
    same(&format!("{COLOR}import Color.*;\nfn warm(c: Color) -> Bool = match c {{ Red | Green => True, _ => False }};\nwarm(Green) && !warm(Blue);"));
    // Equality.
    same(&format!("{COLOR}Color.Red == Color.Red && Color.Red != Color.Blue;"));
    // Variables that change.
    same(&format!("{COLOR}let mut c = Color.Red;\nif c == Color.Red {{ c = Color.Blue; }}\nc;"));
    // Generic enums without fields, and consts.
    same("enum Tag[T] { A, B }\nfn tag(x: i32) -> Tag[i32] = if x > 0 { Tag.B } else { Tag.A };\ntag(5);");
    same(&format!("{COLOR}const FAVORITE: Color = Color.Green;\nfn is_favorite(c: Color) -> Bool = match c {{ FAVORITE => True, _ => False }};\n(FAVORITE == Color.Green) && is_favorite(Color.Green);"));
}

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

/// The evaluator and the compiled code print the same.
#[track_caller]
fn prints_the_same(code: &str, expected: &str) {
    let (db, file) = db_with_builtins(code);
    let recorder = Recorder::default();
    Evaluator::new(&db).with_host(recorder.clone()).eval_file(file).unwrap();
    assert_eq!(recorder.output(), expected, "the evaluator");
    let (mut store, instance) = instantiate(&compile(&db, file));
    call(&mut store, &instance, "main", &[]);
    assert_eq!(store.data().host.printed, expected, "the compiled code");
}

#[test]
fn print() {
    prints_the_same("print(\"Hello, world!\");", "Hello, world!\n");
    prints_the_same(
        "fn greet(name: String) { print(\"hello\"); print(name); }\ngreet(\"Bolt\");\ngreet(\"Wasm\");",
        "hello\nBolt\nhello\nWasm\n",
    );
    // Strings are values like others, in variables, consts and results.
    prints_the_same(
        "const GREETING: String = \"hi\";\nfn pick(b: Bool) -> String = if b { GREETING } else { \"bye\" };\nlet s = pick(False);\nprint(pick(True));\nprint(s);",
        "hi\nbye\n",
    );
    prints_the_same("print(\"héllo, wörld ✓\");", "héllo, wörld ✓\n");
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
fn results_are_shown_like_the_evaluators() {
    assert_eq!(run_wasm("1 < 2;", None).unwrap(), "True");
    assert_eq!(run_wasm("'x';", None).unwrap(), "'x'");
    assert_eq!(run_wasm("let s = \"hi\";\ns;", None).unwrap(), "\"hi\"");
    assert_eq!(run_wasm(&format!("{COLOR}Color.Green;"), None).unwrap(), "Green");
    // Unsigned integers aren't negative.
    assert_eq!(run_wasm("let x: u32 = 4000000000;\nx;", None).unwrap(), "4000000000");
    assert_eq!(run_wasm("let x: u8 = 255;\nx + 1;", None).unwrap(), "0");
    assert_eq!(run_wasm("let x = 1;", None).unwrap(), "()");
}

#[test]
fn running_reports_errors_like_the_evaluator() {
    let panic = "fn check(n: i32) -> i32 = if n < 0 { panic(\"negative\") } else { n };\ncheck(1) + check(-1);";
    assert_eq!(run_wasm(panic, None), Err(RunError::Eval(ErrorKind::Panic("negative".into()))));
    assert_eq!(run_wasm("fn div(a: i32, b: i32) -> i32 = a / b;\ndiv(1, 0);", None), Err(RunError::Eval(ErrorKind::DivisionByZero)));
    assert_eq!(run_wasm("loop {}", Some(10_000)), Err(RunError::Eval(ErrorKind::OutOfFuel)));
    assert_eq!(run_wasm("let mut i = 0;\nwhile i < 10 { i += 1; }\ni;", Some(10_000)).unwrap(), "10");
}

#[test]
fn extern_functions_cant_be_run() {
    assert_eq!(
        run_wasm("extern { fn log(x: i32); }\nlog(1);", None),
        Err(RunError::MissingImport { module: "env".into(), name: "log".into() })
    );
}

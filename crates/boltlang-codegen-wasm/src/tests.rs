//! Most tests are differential: the evaluator and the compiled code must
//! agree on what a program gives.

use boltlang_base_db::{File, TestDb};
use boltlang_eval::{Evaluator, Value};
use wasmi::{Caller, Engine, Linker, Module, Store, Val};

use crate::{link, reachable_instances};

/// What the host saw: the values passed to the imports `log` and `log64`.
#[derive(Default)]
struct Host {
    logged: Vec<i64>,
}

fn compile(db: &TestDb, file: File) -> Vec<u8> {
    link(db, file).unwrap_or_else(|errors| {
        let messages: Vec<_> = errors.iter().map(|e| e.to_string()).collect();
        panic!("compiling failed: {}", messages.join("; "))
    })
}

/// Instantiate a module, with the imports `env.log` and `env.log64`.
fn instantiate(bytes: &[u8]) -> (Store<Host>, wasmi::Instance) {
    let engine = Engine::default();
    let module = Module::new(&engine, bytes).unwrap();
    let mut store = Store::new(&engine, Host::default());
    let mut linker = Linker::<Host>::new(&engine);
    linker
        .func_wrap("env", "log", |mut caller: Caller<'_, Host>, x: i32| caller.data_mut().logged.push(x as i64))
        .unwrap();
    linker
        .func_wrap("env", "log64", |mut caller: Caller<'_, Host>, x: i64| caller.data_mut().logged.push(x))
        .unwrap();
    let instance = linker.instantiate_and_start(&mut store, &module).unwrap();
    (store, instance)
}

/// Call an exported function, and give its result.
fn call(store: &mut Store<Host>, instance: &wasmi::Instance, name: &str, args: &[Val]) -> Option<Val> {
    let func = instance.get_func(&*store, name).unwrap_or_else(|| panic!("no export `{name}`"));
    let mut results = vec![Val::I32(0); func.ty(&*store).results().len()];
    func.call(&mut *store, args, &mut results).unwrap();
    results.pop()
}

/// Run `main`, and give its result and what was logged.
fn run(code: &str) -> (Option<Val>, Vec<i64>) {
    let db = TestDb::new();
    let file = db.file("/main.bolt", code);
    let (mut store, instance) = instantiate(&compile(&db, file));
    let result = call(&mut store, &instance, "main", &[]);
    (result, std::mem::take(&mut store.data_mut().logged))
}

/// A result as a number, to compare the evaluator's with Wasm's.
#[derive(Debug, PartialEq)]
enum Num {
    Int(i128),
    Float(f64),
    Unit,
}

fn from_wasm(val: Option<Val>) -> Num {
    match val {
        None => Num::Unit,
        Some(Val::I32(n)) => Num::Int(n as i128),
        Some(Val::I64(n)) => Num::Int(n as i128),
        Some(Val::F32(x)) => Num::Float(f32::from(x) as f64),
        Some(Val::F64(x)) => Num::Float(f64::from(x)),
        Some(other) => panic!("unexpected result {other:?}"),
    }
}

/// The evaluator and the compiled code give the same result.
#[track_caller]
fn same(code: &str) {
    let db = TestDb::new();
    let file = db.file("/main.bolt", code);
    let expected = match Evaluator::new(&db).eval_file(file).unwrap() {
        Value::Int(n) => Num::Int(n),
        Value::Bool(b) => Num::Int(b as i128),
        Value::Char(c) => Num::Int(c as i128),
        Value::Float(x) => Num::Float(x),
        Value::Tuple(items) if items.is_empty() => Num::Unit,
        other => panic!("the evaluator gave {}", other.display(&db)),
    };
    assert_eq!(from_wasm(run(code).0), expected, "{code}");
}

#[test]
fn arithmetic_and_comparisons() {
    same("1 + 2 * 3 - 4 / 2;");
    same("17 % 5;");
    same("7 - 10;");
    same("1 < 2 && 2 <= 2 || false;");
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
    same("fn even(n) = if n == 0 { true } else { odd(n - 1) };\nfn odd(n) = if n == 0 { false } else { even(n - 1) };\neven(10);");
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
    assert_eq!(run("let x: u8 = 255;\nx + 1;").0.map(|v| v.i32()), Some(Some(0)));
    assert_eq!(run("let x: i8 = 127;\nx + 1;").0.map(|v| v.i32()), Some(Some(-128)));
    assert_eq!(run("let x: u16 = 0;\nx - 1;").0.map(|v| v.i32()), Some(Some(65535)));
}

#[test]
fn extern_functions_are_imports() {
    let (result, logged) = run("extern { fn log(x: i32); fn log64(x: i64); }\nfn f(n: i32) { log(n); log(n * 2); }\nf(3);\nlog64(7);");
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

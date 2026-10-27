use boltlang_base_db::TestDb;

use crate::{ErrorKind, Evaluator, Limits, Value};

/// Run the file and show its value, or the error.
fn run(code: &str) -> String {
    run_with(code, Limits::default())
}

/// A database with the repository's `std` as the prelude, and `code` as
/// `/main.bolt`.
fn db_with_std(code: &str) -> (TestDb, boltlang_base_db::File) {
    let mut db = TestDb::new();
    db.add_std();
    db.set_prelude(Some("std"), true);
    let file = db.file("/main.bolt", code);
    (db, file)
}

fn run_with(code: &str, limits: Limits) -> String {
    let (db, file) = db_with_std(code);
    let mut evaluator = Evaluator::with_limits(&db, limits);
    match evaluator.eval_file(file) {
        Ok(value) => value.display(&db).to_string(),
        Err(e) => format!("error: {e}"),
    }
}

fn error_kind(code: &str, limits: Limits) -> ErrorKind {
    let (db, file) = db_with_std(code);
    Evaluator::with_limits(&db, limits).eval_file(file).unwrap_err().kind
}

#[test]
fn arithmetic() {
    assert_eq!(run("1 + 2 * 3"), "7");
    assert_eq!(run("(1 + 2) * 3"), "9");
    assert_eq!(run("7 % 3 - -1"), "2");
    assert_eq!(run("1.5 * 2.0"), "3.0");
    assert_eq!(run("1 << 4 | 1"), "17");
    assert_eq!(run("\"ab\" + \"cd\""), "\"abcd\"");
    assert_eq!(run("3 as f64"), "3.0");
}

#[test]
fn comparisons_and_logic() {
    assert_eq!(run("1 < 2 && 2 <= 2"), "True");
    assert_eq!(run("1 == 2 || !(3 != 3)"), "True");
    assert_eq!(run("(1, 2) < (1, 3)"), "True");
    assert_eq!(run("[1, 2] == [1, 2]"), "True");
}

#[test]
fn true_and_false_are_names() {
    assert_eq!(run("(True, !True, False || True)"), "(True, False, True)");
    assert_eq!(run("match 1 > 2 { True => \"yes\", False => \"no\" }"), "\"no\"");
    assert_eq!(run("enum Answer { True, Maybe }\nimport Answer.*;\nmatch True { Maybe => 1, True => 2 }"), "2");
}

#[test]
fn recursion() {
    let code = "
        fn fac(x: i32) -> i32 {
          if x == 0 {
            return 1
          } else {
            return x * fac(x - 1);
          }
        }
        fac(10)
    ";
    assert_eq!(run(code), "3628800");
}

#[test]
fn mutual_recursion() {
    let code = "
        fn even(n) = if n == 0 { True } else { odd(n - 1) };
        fn odd(n) = if n == 0 { False } else { even(n - 1) };
        (even(10), odd(7), even(3))
    ";
    assert_eq!(run(code), "(True, True, False)");
}

#[test]
fn variables_and_loops() {
    let code = "
        let mut sum = 0;
        let mut i = 0;
        while i < 10 {
          i += 1;
          if i % 2 == 0 { continue; }
          sum += i;
        }
        sum
    ";
    assert_eq!(run(code), "25");

    let code = "
        let mut total = 0;
        for i in 1..=4 { total += i; }
        total
    ";
    assert_eq!(run(code), "10");

    let code = "
        let mut n = 0;
        let x = loop { n += 1; if n == 5 { break n * 2; } };
        x
    ";
    assert_eq!(run(code), "10");
}

#[test]
fn closures_capture_variables() {
    let code = "
        let mut count = 0;
        let incr = |by| { count += by; count };
        incr(2);
        incr(3)
    ";
    assert_eq!(run(code), "5");

    let code = "
        fn adder(n) = |x| x + n;
        let add2 = adder(2);
        add2(40)
    ";
    assert_eq!(run(code), "42");
}

#[test]
fn structs() {
    let code = "
        struct Point { x: i32, y: i32 }
        let mut p = Point { x: 1, y: 2 };
        p.x = 10;
        let q = Point { y: 5, ..p };
        (p.x + p.y, q)
    ";
    assert_eq!(run(code), "(12, Point { x: 10, y: 5 })");
}

#[test]
fn enums_and_match() {
    let code = "
        enum Shape { Circle(i32), Rect { w: i32, h: i32 }, Empty }
        fn area(s) = match s {
          Shape.Circle(r) => 3 * r * r,
          Shape.Rect { w, h } => w * h,
          Shape.Empty => 0,
        };
        [area(Shape.Circle(2)), area(Shape.Rect { w: 2, h: 3 }), area(Shape.Empty)]
    ";
    assert_eq!(run(code), "[12, 6, 0]");
}

#[test]
fn patterns() {
    let code = "
        fn classify(n) = match n {
          0 => \"zero\",
          1 | 2 | 3 => \"small\",
          4..=9 => \"medium\",
          x if x < 0 => \"negative\",
          _ => \"large\",
        };
        [classify(0), classify(2), classify(7), classify(-3), classify(100)]
    ";
    assert_eq!(run(code), r#"["zero", "small", "medium", "negative", "large"]"#);

    let code = "
        let (a, .., z) = (1, 2, 3, 4);
        let [first, ..] = [5, 6];
        a + z + first
    ";
    assert_eq!(run(code), "10");
}

#[test]
fn method_calls_are_calls_to_functions_in_scope() {
    let code = "
        struct Counter { n: i32 }
        fn get(c) = c.n;
        fn plus(c, k) = c.n + k;
        let c = Counter { n: 4 };
        c.get() + c.plus(1)
    ";
    assert_eq!(run(code), "9");
}

#[test]
fn consts() {
    let code = "
        const N: i32 = 6;
        const M: i32 = N * 7;
        M
    ";
    assert_eq!(run(code), "42");
    assert!(matches!(error_kind("const A: i32 = A + 1; A", Limits::default()), ErrorKind::ConstCycle(_)));
}

#[test]
fn calling_from_outside() {
    let db = TestDb::new();
    let file = db.file("/main.bolt", "fn square(x) = x * x;");
    let mut evaluator = Evaluator::new(&db);
    let square = evaluator.function(file, "square").unwrap();
    let result = evaluator.call_function(square, vec![Value::Int(12)]).unwrap();
    assert!(result.equals(&Value::Int(144)));
}

#[test]
fn errors() {
    let limits = Limits::default();
    assert_eq!(error_kind("1 / 0", limits), ErrorKind::DivisionByZero);
    assert_eq!(error_kind("[1, 2][5]", limits), ErrorKind::IndexOutOfBounds { index: 5, len: 2 });
    assert_eq!(error_kind("let x = 1; y", limits), ErrorKind::UnresolvedName("y".into()));
    assert_eq!(error_kind("match 3 { 1 => 0 }", limits), ErrorKind::NoMatch);
}

#[test]
fn limits_stop_runaway_code() {
    let fuel = Limits { fuel: Some(1000), ..Limits::default() };
    assert_eq!(error_kind("loop {}", fuel), ErrorKind::OutOfFuel);

    let shallow = Limits { max_depth: 50, ..Limits::default() };
    assert_eq!(error_kind("fn f(n) = f(n + 1); f(0)", shallow), ErrorKind::StackOverflow);
    assert_eq!(run_with("fn f(n) = if n == 0 { 0 } else { f(n - 1) }; f(40)", shallow), "0");
}

#[test]
fn deep_recursion_does_not_overflow_the_native_stack() {
    let code = "fn count(n) = if n == 0 { 0 } else { 1 + count(n - 1) }; count(5000)";
    assert_eq!(run(code), "5000");
}

#[test]
fn a_file_has_the_value_of_its_last_expression_statement() {
    assert_eq!(run("fn f(x) = x + 1; f(1);"), "2");
    assert_eq!(run("1; let x = 2;"), "()");
}

/// `std` keeps `array_alloc` to itself, so tests of it declare it.
const ARRAY_ALLOC: &str = "extern \"bolt\" { fn array_alloc[T](len: i32) -> mut [T]; }\n";

#[test]
fn print_goes_to_the_host() {
    let (db, file) = db_with_std("fn greet(name: String) { print(\"hello\"); print(name); }\ngreet(\"Bolt\");\nprint(\"bye\");");
    let recorder = crate::Recorder::default();
    let mut evaluator = Evaluator::new(&db).with_host(recorder.clone());
    evaluator.eval_file(file).unwrap();
    assert_eq!(recorder.output(), "hello\nBolt\nbye\n");
}

#[test]
fn effects_are_refused_without_a_host_and_in_consts() {
    let (db, file) = db_with_std("print(\"hi\");");
    assert_eq!(Evaluator::new(&db).eval_file(file).unwrap_err().kind, ErrorKind::Effect("print"));

    // Even with a host, a const can't print.
    let (db, file) = db_with_std("fn noisy() -> i32 { print(\"evaluating\"); 1 }\nconst N: i32 = noisy();\nN;");
    let recorder = crate::Recorder::default();
    let error = Evaluator::new(&db).with_host(recorder.clone()).eval_file(file).unwrap_err();
    assert_eq!(error.kind, ErrorKind::Effect("print"));
    assert_eq!(recorder.output(), "");
}

#[test]
fn array_len_counts_the_elements() {
    let (db, file) = db_with_std("let xs = mut [1, 2];\nlet n = array_len(xs);\nxs[0] = 3;\n(n, array_len([True]), array_len([]));");
    let value = Evaluator::new(&db).eval_file(file).unwrap();
    assert_eq!(value.display(&db).to_string(), "(2, 1, 0)");
}

#[test]
fn array_alloc_makes_elements_to_set() {
    let (db, file) =
        db_with_std(&format!("{ARRAY_ALLOC}let xs: mut [i32] = array_alloc(3);\nxs[0] = 1;\nxs[2] = 3;\n(array_len(xs), xs);"));
    let value = Evaluator::new(&db).eval_file(file).unwrap();
    assert_eq!(value.display(&db).to_string(), "(3, [1, _, 3])");
}

#[test]
fn reading_an_unset_element_is_an_error() {
    let prefix = format!("{ARRAY_ALLOC}let xs: mut [i32] = array_alloc(3);\nxs[0] = 1;\n");
    for read in ["xs[1];", "xs[1] += 1;", "for x in xs { x; }", "match xs { [a, b, c] => a, _ => 0 };"] {
        let (db, file) = db_with_std(&format!("{prefix}{read}"));
        let error = Evaluator::new(&db).eval_file(file).unwrap_err();
        assert_eq!(error.kind, ErrorKind::UnsetElement { index: 1 }, "{read}");
    }
    let (db, file) = db_with_std(&format!("{ARRAY_ALLOC}struct P {{ x: i32 }}\nlet ps: mut [mut P] = array_alloc(1);\nps[0].x = 1;"));
    let error = Evaluator::new(&db).eval_file(file).unwrap_err();
    assert_eq!(error.kind, ErrorKind::UnsetElement { index: 0 });
}

#[test]
fn array_copy_copies_ranges() {
    // Between arrays, and within one, to the right and to the left.
    let code = "let a = mut [1, 2, 3, 4, 5];\n\
                let b = mut [0, 0, 0];\n\
                array_copy(b, 1, a, 3, 2);\n\
                let c = mut [1, 2, 3, 4, 5];\n\
                array_copy(c, 1, c, 0, 3);\n\
                let d = mut [1, 2, 3, 4, 5];\n\
                array_copy(d, 0, d, 2, 3);\n\
                array_copy(d, 5, d, 0, 0);\n\
                (b, c, d);";
    assert_eq!(run(code), "([0, 4, 5], [1, 1, 2, 3, 5], [3, 4, 5, 4, 5])");
}

#[test]
fn array_copy_copies_unset_elements_as_they_are() {
    let code = format!("{ARRAY_ALLOC}let a: mut [i32] = array_alloc(3);\na[0] = 1;\nlet b = mut [7, 8, 9];\narray_copy(b, 0, a, 0, 3);\nb;");
    assert_eq!(run(&code), "[1, _, _]");
}

#[test]
fn array_copy_checks_its_ranges() {
    let limits = Limits::default();
    let range = |start, end, len| ErrorKind::RangeOutOfBounds { start, end, len };
    assert_eq!(error_kind("array_copy(mut [0, 0], 0, [1, 2, 3], 2, 2);", limits), range(2, 4, 3));
    assert_eq!(error_kind("array_copy(mut [0, 0], 1, [1, 2, 3], 0, 2);", limits), range(1, 3, 2));
    assert_eq!(error_kind("array_copy(mut [0, 0], 0, [1, 2], -1, 1);", limits), range(-1, 0, 2));
    assert_eq!(error_kind("array_copy(mut [0, 0], 0, [1, 2], 1, -1);", limits), range(1, 0, 2));
}

#[test]
fn an_error_in_std_is_reported_at_the_program() {
    // `xs[5]` fails in `std`'s `impl Index[[T]]`.
    let (db, file) = db_with_std("let xs = [1];\nxs[5];");
    let error = Evaluator::new(&db).eval_file(file).unwrap_err();
    assert_eq!(error.kind, ErrorKind::IndexOutOfBounds { index: 5, len: 1 });
    assert_eq!(error.source_range(&db), Some((file, 14..19)));
}

#[test]
fn panic_stops_with_its_message() {
    let (db, file) = db_with_std("fn check(n: i32) -> i32 = if n < 0 { panic(\"negative\") } else { n };\ncheck(1) + check(-1);");
    let error = Evaluator::new(&db).eval_file(file).unwrap_err();
    assert_eq!(error.kind, ErrorKind::Panic("negative".into()));
    assert_eq!(error.to_string(), "panicked: negative");
}

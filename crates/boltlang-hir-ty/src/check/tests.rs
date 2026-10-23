use boltlang_base_db::{TestDb, source_text};
use boltlang_hir_def::{
    body::body_with_source_map,
    hir::{Statement},
    item_tree::FunctionName,
};

use super::*;
use crate::{infer::InferenceDiagnostic, solve::ExprOrPat};

/// The types of the functions in `source` and of its top-level statements,
/// then the diagnostics, each with the code it's about.
fn check(source: &str) -> String {
    let db = TestDb::new();
    let file = db.file("/main.bolt", source);
    let tree = item_tree(&db, file);
    let mut lines = Vec::new();
    let mut defs = Vec::new();
    for item in &tree.top_level {
        let ModItem::Function(idx) = *item else { continue };
        let f = FunctionId::new(&db, ItemContainerId::File(file), idx);
        let name = match f.item(&db).name.as_ref().unwrap() {
            FunctionName::Name(n) | FunctionName::Operator(n) => n.to_string(),
        };
        let ty = match function_scheme(&db, f) {
            Ok(scheme) => scheme.display(&db),
            Err(()) => "<cycle>".into(),
        };
        lines.push(format!("{name}: {ty}"));
        if f.item(&db).has_body {
            defs.push(DefWithBodyId::new(&db, DefWithBody::Function(f)));
        }
    }
    let top = DefWithBodyId::new(&db, DefWithBody::File(file));
    defs.push(top);
    let (top_body, top_map) = body_with_source_map(&db, top);
    let text = source_text(&db, file);
    let snippet = |map: &boltlang_hir_def::expr_store::ExpressionStoreSourceMap, node: ExprOrPat| {
        let ptr = match node {
            ExprOrPat::Expr(e) => map.expr_syntax(e),
            ExprOrPat::Pat(p) => map.pat_syntax(p),
        };
        ptr.map_or("?".to_string(), |p| text[p.start as usize..p.end as usize].to_string())
    };
    let Expr::Block { statements, .. } = &top_body[top_body.body_expr] else { panic!() };
    let top_result = infer_body(&db, top);
    for statement in statements.iter() {
        match statement {
            Statement::Expr { expr, .. } => lines.push(format!(
                "{}: {}",
                snippet(top_map, ExprOrPat::Expr(*expr)),
                top_result.expr_ty(*expr).display(&db)
            )),
            Statement::Let { pat, .. } => lines.push(format!(
                "let {}: {}",
                snippet(top_map, ExprOrPat::Pat(*pat)),
                top_result.pat_ty(*pat).display(&db)
            )),
            Statement::Item => {}
        }
    }
    for def in defs {
        let (_, map) = body_with_source_map(&db, def);
        for diagnostic in &infer_body(&db, def).diagnostics {
            let at = |node| snippet(map, node);
            lines.push(match diagnostic {
                InferenceDiagnostic::Type(TypeError::Mismatch { origin, expected, actual }) => format!(
                    "error at `{}`: expected {}, found {}",
                    at(origin.node),
                    expected.display(&db),
                    actual.display(&db)
                ),
                InferenceDiagnostic::Type(TypeError::NoInstance { origin, constraint }) => format!(
                    "error at `{}`: no instance {}[{}]",
                    at(origin.node),
                    constraint.class.item(&db).name.as_ref().unwrap(),
                    constraint.args.iter().map(|t| t.display(&db).to_string()).collect::<Vec<_>>().join(", ")
                ),
                InferenceDiagnostic::Type(TypeError::Ambiguous { origin, .. }) => {
                    format!("error at `{}`: ambiguous", at(origin.node))
                }
                InferenceDiagnostic::UnresolvedName { expr } => format!("error at `{}`: unresolved", at(*expr)),
                InferenceDiagnostic::WrongArgCount { expr, expected, found } => {
                    format!("error at `{}`: expected {expected} arguments, found {found}", at(*expr))
                }
                InferenceDiagnostic::NoSuchField { expr, name } => format!("error at `{}`: no field {name}", at(*expr)),
                InferenceDiagnostic::Type(TypeError::TypeNeeded { origin }) => {
                    format!("error at `{}`: type needed", at(origin.node))
                }
                InferenceDiagnostic::Type(TypeError::NoSuchField { origin, ty, name }) => {
                    format!("error at `{}`: {} has no field {name}", at(origin.node), ty.display(&db))
                }
                InferenceDiagnostic::MissingFields { expr, names } => format!(
                    "error at `{}`: missing {}",
                    at(ExprOrPat::Expr(*expr)),
                    names.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(", ")
                ),
                other => format!("error: {other:?}"),
            });
        }
    }
    lines.join("\n")
}

#[test]
fn generalizes_functions_without_signatures() {
    // The example from test.bolt, with `+` on a string reported.
    assert_eq!(
        check("fn id(x) { return x }\nid(1) + 1;\nid(\"foo\") + 1;\nlet b = id(true);\n"),
        "id: forall a. fn(a) -> a\n\
         id(1) + 1: i32\n\
         id(\"foo\") + 1: str\n\
         let b: bool\n\
         error at `1`: expected str, found i32"
    );
}

#[test]
fn functions_used_before_their_definition() {
    assert_eq!(
        check("fn pair(x) = (first(x), first(true));\nfn first(y) = y;\n"),
        "pair: forall a. fn(a) -> (a, bool)\nfirst: forall a. fn(a) -> a"
    );
}

#[test]
fn mutual_recursion() {
    assert_eq!(
        check("fn even(n) = if n == 0 { true } else { odd(n - 1) };\nfn odd(n) = if n == 0 { false } else { even(n - 1) };\n"),
        "even: fn(i32) -> bool\nodd: fn(i32) -> bool"
    );
}

#[test]
fn declared_signatures_are_checked() {
    assert_eq!(
        check("fn f(x: i32) -> bool { x }\nfn g[T](x: T) -> T = x;\ng(1u8);\nf(true);\n"),
        "f: fn(i32) -> bool\n\
         g: forall T. fn(T) -> T\n\
         g(1u8): u8\n\
         f(true): bool\n\
         error at `x`: expected bool, found i32\n\
         error at `true`: expected i32, found bool"
    );
}

#[test]
fn class_constraints_are_inferred_and_solved() {
    assert_eq!(
        check(
            "class Show[T] { fn show(x: T) -> str; }\n\
             impl Show[i32] { fn show(x) = \"int\"; }\n\
             fn twice(x) = (show(x), show(x));\n\
             twice(1);\n\
             twice(true);\n"
        ),
        "twice: forall a. Show[a] => fn(a) -> (str, str)\n\
         twice(1): (str, str)\n\
         twice(true): (str, str)\n\
         error at `twice`: no instance Show[bool]"
    );
}

#[test]
fn where_clauses_are_assumed() {
    assert_eq!(
        check(
            "class Eq[T] { fn eq(a: T, b: T) -> bool; }\n\
             class Ord[T] where Eq[T] { fn lt(a: T, b: T) -> bool; }\n\
             fn le[T](a: T, b: T) -> bool where Ord[T] = lt(a, b) || eq(a, b);\n\
             fn bad[T](a: T) -> bool = eq(a, a);\n"
        ),
        "le: forall T. Ord[T] => fn(T, T) -> bool\n\
         bad: forall T. fn(T) -> bool\n\
         error at `eq`: no instance Eq[T]"
    );
}

#[test]
fn data_types_and_patterns() {
    assert_eq!(
        check(
            "enum Option[T] { None, Some(T) }\n\
             struct Point { x: i32, y: i32 }\n\
             fn get(o, d) = match o { Option.Some(v) => v, Option.None => d };\n\
             fn norm(p) = p.x * p.x;\n\
             let p = Point { x: 1, y: 2 };\n\
             let Point { x, .. } = p;\n\
             get(Option.Some(p), Point { x: 0 });\n\
             p.z;\n"
        ),
        "get: forall a. fn(Option[a], a) -> a\n\
         norm: forall a b. a.x: b => fn(a) -> b\n\
         let p: Point\n\
         let Point { x, .. }: Point\n\
         get(Option.Some(p), Point { x: 0 }): Point\n\
         p.z: {error}\n\
         error at `Point { x: 0 }`: missing y\n\
         error at `p.z`: Point has no field z"
    );
}

#[test]
fn fields_of_unknown_types_become_part_of_the_function_type() {
    assert_eq!(
        check(
            "struct Point { x: i32, y: i32 }\n\
             struct Size { x: u8 }\n\
             struct Line { start: Point }\n\
             fn get_x(p) = p.x;\n\
             fn start_x(l) = l.start.x;\n\
             get_x(Point { x: 1, y: 2 });\n\
             get_x(Size { x: 3u8 });\n\
             start_x(Line { start: Point { x: 1, y: 2 } });\n\
             get_x((1, 2));\n\
             let t = (true, 1);\n\
             t.0;\n"
        ),
        "get_x: forall a b. a.x: b => fn(a) -> b\n\
         start_x: forall a b c. a.start: c => c.x: b => fn(a) -> b\n\
         get_x(Point { x: 1, y: 2 }): i32\n\
         get_x(Size { x: 3u8 }): u8\n\
         start_x(Line { start: Point { x: 1, y: 2 } }): i32\n\
         get_x((1, 2)): {error}\n\
         let t: (bool, i32)\n\
         t.0: bool\n\
         error at `get_x`: (i32, i32) has no field x"
    );
}

#[test]
fn fields_of_types_never_found_are_reported() {
    assert_eq!(
        check("fn f() { let g = |p| p.x; 1 }\n"),
        "f: fn() -> i32\n\
         error at `p.x`: type needed"
    );
}

#[test]
fn method_call_syntax() {
    assert_eq!(
        check(
            "struct Counter { n: i32 }\n\
             fn get(c: Counter) -> i32 = c.n;\n\
             fn inc(x: i32) -> i32 = x + 1;\n\
             let c = Counter { n: 1 };\n\
             c.get().inc();\n\
             c.n.inc();\n\
             get(c);\n"
        ),
        "get: fn(Counter) -> i32\n\
         inc: fn(i32) -> i32\n\
         let c: Counter\n\
         c.get().inc(): i32\n\
         c.n.inc(): i32\n\
         get(c): i32"
    );
}

#[test]
fn lambdas() {
    assert_eq!(
        check(
            "fn apply(f, x) = f(x);\n\
             apply(fn (x) = x + 1, 2);\n\
             apply(|b| !b, true);\n\
             let k = |x: i32| x;\n\
             k(true);\n"
        ),
        "apply: forall a b. fn(fn(a) -> b, a) -> b\n\
         apply(fn (x) = x + 1, 2): i32\n\
         apply(|b| !b, true): bool\n\
         let k: fn(i32) -> i32\n\
         k(true): i32\n\
         error at `true`: expected i32, found bool"
    );
}

#[test]
fn imported_functions_without_signatures() {
    let db = TestDb::new();
    db.file("/lib.bolt", "pub fn id(x) = x;\n");
    let main = db.file("/main.bolt", "import \"./lib\" (id);\nfn f(y: i32) -> bool = id(y);\n");
    let ModItem::Function(idx) = item_tree(&db, main).top_level[1] else { panic!() };
    let f = DefWithBodyId::new(&db, DefWithBody::Function(FunctionId::new(&db, ItemContainerId::File(main), idx)));
    let diagnostics = &infer_body(&db, f).diagnostics;
    assert!(
        matches!(&diagnostics[..], [InferenceDiagnostic::Type(TypeError::Mismatch { expected, actual, .. })]
            if *expected == Ty::bool() && actual.display(&db).to_string() == "i32"),
        "{diagnostics:?}"
    );
}

#[test]
fn files_that_infer_each_other_report_a_cycle() {
    let db = TestDb::new();
    let a = db.file("/a.bolt", "import \"./b\" (g);\npub fn f(x) = g(x);\n");
    db.file("/b.bolt", "import \"./a\" (f);\npub fn g(x) = f(x);\n");
    let ModItem::Function(idx) = item_tree(&db, a).top_level[1] else { panic!() };
    let f = DefWithBodyId::new(&db, DefWithBody::Function(FunctionId::new(&db, ItemContainerId::File(a), idx)));
    assert!(infer_body(&db, f).diagnostics.contains(&InferenceDiagnostic::CyclicInference));
}

/// The type of the top-level statements of a file, to check what reruns.
#[salsa::tracked]
fn top_level_ok(db: &dyn Db, file: File) -> bool {
    infer_body(db, DefWithBodyId::new(db, DefWithBody::File(file))).diagnostics.is_empty()
}

#[test]
fn editing_a_declared_function_does_not_rerun_its_callers() {
    let mut db = TestDb::new();
    let file = db.file("/main.bolt", "fn f(x: i32) -> i32 = x;\nfn g(y) = y;\nf(g(1));\n");
    assert!(*top_level_ok(&db, file));
    let executed = db.log_executed(|db| {
        db.edit(file, "fn f(x: i32) -> i32 = x + 1;\nfn g(y) = y;\nf(g(1));\n");
        assert!(*top_level_ok(db, file));
    });
    assert!(!executed.iter().any(|q| q.contains("file_inference") || q.contains("top_level_ok")), "{executed:?}");

    // Editing a function without a signature reinfers the file's group.
    let executed = db.log_executed(|db| {
        db.edit(file, "fn f(x: i32) -> i32 = x + 1;\nfn g(y) = { y };\nf(g(1));\n");
        assert!(*top_level_ok(db, file));
    });
    assert!(executed.iter().any(|q| q.contains("file_inference")), "{executed:?}");
}

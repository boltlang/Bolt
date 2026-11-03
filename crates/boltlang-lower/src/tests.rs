use boltlang_base_db::TestDb;

use crate::{Dict, Use, program};

/// The instances of a program with `std`'s prelude, and what each one
/// calls, one per line, then what was erased and what went wrong.
fn instances(src: &str) -> String {
    let mut db = TestDb::new();
    db.set_prelude(Some("std"), false);
    db.add_std();
    let file = db.file("/main.bolt", src);
    let p = program(&db, file);
    let mut out = String::new();
    for i in &p.instances {
        // std's own instances would drown out the program's.
        let path = i.instance.body(&db).def(&db).file(&db);
        if path != file {
            continue;
        }
        out.push_str(&format!("{}\n", i.instance.display(&db)));
        for (_, u) in &i.uses {
            match u {
                Use::Instance { instance, mutabilities, dicts } => {
                    out.push_str(&format!("  -> {}", instance.display(&db)));
                    if !mutabilities.is_empty() {
                        let ms: Vec<String> = mutabilities.iter().map(|m| m.to_string()).collect();
                        out.push_str(&format!(" with {}", ms.join(", ")));
                    }
                    for d in dicts.iter() {
                        out.push_str(&format!(" dict {}", show_dict(&db, d)));
                    }
                    out.push('\n');
                }
                Use::Method { dict, item, .. } => {
                    let name = match item {
                        crate::ClassItem::Function(f) => {
                            f.item(&db).display_name().map(|n| n.to_string()).unwrap_or_default()
                        }
                        crate::ClassItem::Const(c) => {
                            c.item(&db).name.as_ref().map(|n| n.to_string()).unwrap_or_default()
                        }
                    };
                    out.push_str(&format!("  -> {name} of {}\n", show_dict(&db, dict)));
                }
                Use::Builtin(_) | Use::Extern(_) => {}
            }
        }
    }
    for f in &p.erased {
        let name = f.item(&db).display_name().map(|n| n.to_string()).unwrap_or_default();
        out.push_str(&format!("erased {name}\n"));
    }
    for (i, site, e) in &p.errors {
        out.push_str(&format!("error in {} at {site:?}: {e:?}\n", i.display(&db)));
    }
    out
}

fn show_dict(db: &TestDb, d: &Dict<'_>) -> String {
    match d {
        Dict::Param(i) => format!("#{i}"),
        Dict::Super { of, index } => format!("{}.super{index}", show_dict(db, of)),
        Dict::Impl { imp, args, subs } => {
            let header = boltlang_hir_ty::lower::impl_header(db, *imp);
            let mut s = match &header.class {
                Some(c) => {
                    let args: Vec<String> =
                        c.args.iter().map(|t| t.display(db).to_string()).collect();
                    format!(
                        "impl {}[{}]",
                        crate::instance::class_name(db, c.class),
                        args.join(", ")
                    )
                }
                None => "impl".to_string(),
            };
            if !args.is_empty() {
                let args: Vec<String> = args.iter().map(|t| t.display(db).to_string()).collect();
                s.push_str(&format!(" at [{}]", args.join(", ")));
            }
            if !subs.is_empty() {
                let subs: Vec<String> = subs.iter().map(|d| show_dict(db, d)).collect();
                s.push_str(&format!(" with ({})", subs.join(", ")));
            }
            s
        }
    }
}

fn check(src: &str, expected: &str) {
    let found = instances(src);
    assert_eq!(found, expected, "\n{found}");
}

#[test]
fn a_generic_function_has_an_instance_for_each_type() {
    check(
        "fn id[T](x: T) -> T = x;\nid(1);\nid(True);\n",
        "<file>\n  -> id[i32]\n  -> id[Bool]\nid[i32]\nid[Bool]\n",
    );
}

#[test]
fn a_class_method_calls_the_impl_the_types_choose() {
    check(
        "class Sh[T] { fn sh(x: T) -> i32; }\nimpl Sh[i32] { fn sh(x) = 1; }\n\
         fn f[T](x: T) -> i32 where Sh[T] = sh(x);\nf(1);\n",
        "<file>\n  -> f[i32]\nf[i32]\n  -> Sh[i32].sh\nSh[i32].sh\n",
    );
}

#[test]
fn instances_are_generic_in_mutabilities() {
    check(
        "fn g(xs: 'm [i32]) -> i32 = xs[0];\ng(mut [1]);\ng([1]);\n",
        "<file>\n  -> g with mut\n  -> g with imm\ng\n  -> Index[[T]].index[i32] with '0\n",
    );
}

#[test]
fn syntax_calls_the_methods_of_the_prelude() {
    check(
        "let xs = [1, 2];\nlet mut s = 0;\nfor x in xs { s += x; }\nxs[0];\n",
        "<file>\n  -> +[i32]\n  -> Iterable['v [T]].iter[i32] with imm\n\
         \x20 -> Iterator[mut ArrayIter[T]].next[i32] with imm\n\
         \x20 -> Index[[T]].index[i32] with imm\n",
    );
}

#[test]
fn methods_of_types_and_constants_of_impls() {
    check(
        "let v = Vec.new();\nv.push(1);\nlet e: List[i32] = empty;\n",
        "<file>\n  -> Vec.new[i32]\n  -> push[i32]\n  -> Monoid[List[T]].empty[i32]\n",
    );
}

#[test]
fn polymorphic_recursion_is_erased() {
    check(
        "enum Nested[A] { Flat(A), Nest(Nested[(A, A)]) }\n\
         fn depth[A](n: Nested[A]) -> i32 = match n {\n\
         \x20 Nested.Flat(_) => 1,\n  Nested.Nest(m) => 1 + depth(m),\n};\n\
         depth(Nested.Flat(1));\n",
        "<file>\n  -> depth[any0]\ndepth[any0]\n  -> depth[any0]\n  -> +[i32]\nerased depth\n",
    );
}

/// The classes of a function that's erased come from dictionaries, which a
/// caller builds from impls.
const SH: &str = "class Sh[T] { fn sh(x: T) -> i32; }\nimpl Sh[i32] { fn sh(x) = 1; }\n\
    impl Sh[Bool] { fn sh(x) = 2; }\n\
    impl[A, B] Sh[(A, B)] where Sh[A], Sh[B] { fn sh(p) = sh(p.0) + sh(p.1); }\n";

#[test]
fn erased_code_calls_class_methods_through_dictionaries() {
    let src = format!(
        "{SH}enum Nested[A] {{ Flat(A), Nest(Nested[(A, A)]) }}\n\
         fn shn[A](n: Nested[A]) -> i32 where Sh[A] = match n {{\n\
         \x20 Nested.Flat(x) => sh(x),\n  Nested.Nest(m) => shn(m),\n}};\n\
         shn(Nested.Flat(1));\n"
    );
    check(
        &src,
        "<file>\n  -> shn[any0] dict impl Sh[i32]\nshn[any0]\n  -> sh of #0\n\
         \x20 -> shn[any0] dict impl Sh[(A, B)] at [any0, any0] with (#0, #0)\n\
         Sh[i32].sh\nSh[(A, B)].sh[any0, any0]\n  -> sh of #0\n  -> sh of #0\n  -> +[i32]\n\
         erased shn\n",
    );
}

#[test]
fn erased_types_keep_their_own_dictionaries() {
    let src = format!(
        "{SH}fn f[A, B](x: A, y: B, n: i32) -> i32 where Sh[A], Sh[B] =\n\
         \x20 if n == 0 {{ sh((x, y)) }} else {{ f((x, y), y, n - 1) }};\n\
         f(1, True, 3);\n#[test]\nfn t() {{ f(True, 1, 0); }}\n"
    );
    check(
        &src,
        "<file>\n  -> f[any0, any1] dict impl Sh[i32] dict impl Sh[Bool]\n\
         t\n  -> f[any0, any1] dict impl Sh[Bool] dict impl Sh[i32]\n\
         f[any0, any1]\n  -> ==[i32]\n  -> sh of impl Sh[(A, B)] at [any0, any1] with (#0, #1)\n\
         \x20 -> f[any0, any1] dict impl Sh[(A, B)] at [any0, any1] with (#0, #1) dict #1\n\
         \x20 -> -[i32]\nSh[Bool].sh\nSh[i32].sh\nSh[(A, B)].sh[any0, any1]\n\
         \x20 -> sh of #0\n  -> sh of #1\n  -> +[i32]\nerased f\n",
    );
}

/// Every test file of `std` is a program whose uses can all be compiled.
#[test]
fn the_tests_of_std_collect_without_errors() {
    let mut db = TestDb::new();
    db.add_std();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/std");
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_str().unwrap().to_string())
        .filter(|n| n.starts_with("test_") && n.ends_with(".bolt"))
        .collect();
    names.sort();
    assert!(!names.is_empty());
    let mut problems = Vec::new();
    for name in names {
        let path = format!("{}/{name}", TestDb::STD_DIR);
        let file =
            boltlang_base_db::Db::files(&db).resolve(&db, boltlang_base_db::SystemPath::new(&path));
        let p = program(&db, file);
        assert!(p.instances.len() > 1, "{name} has no instances");
        for (i, site, e) in &p.errors {
            problems.push(format!("{name}: in {} at {site:?}: {e:?}", i.display(&db)));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

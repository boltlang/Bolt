use boltlang_base_db::{TestDb, source_text};
use boltlang_syntax::rowan::{TextRange, TextSize};

use crate::diagnostics;

/// Check the diagnostics of a file against its annotations. A comment like
///
/// ```text
/// let x: i32 = true;
///    //^^^ label: expected because of this annotation
///             //^^^^ error: expected `i32`, found `bool`
/// ```
///
/// marks the columns of the carets in the line above it, which is the
/// closest line that isn't an annotation. `label:` marks the labels of
/// diagnostics. A diagnostic at an empty range, like a missing token, is
/// marked by one caret.
#[track_caller]
fn check(fixture: &str) {
    check_files(&[("/main.bolt", fixture)]);
}

/// Like [`check`], for several files that can import each other. Labels
/// are marked in the file they're in, which can be another file than that
/// of their diagnostic.
#[track_caller]
fn check_files(files: &[(&str, &str)]) {
    let db = TestDb::new();
    let files: Vec<_> = files.iter().map(|(path, text)| (*path, db.file(path, text))).collect();
    let all: Vec<_> = files.iter().flat_map(|&(_, file)| diagnostics(&db, file)).collect();
    for &(path, file) in &files {
        let text = source_text(&db, file);
        let mut expected: Vec<String> =
            annotations(&text).into_iter().map(|(range, message)| show(path, &text, range, &message)).collect();
        let mut actual = Vec::new();
        for d in &all {
            if d.file == file {
                actual.push(show(path, &text, d.range, &format!("{}: {}", d.severity, d.message)));
            }
            for label in d.labels.iter().filter(|l| l.file == file) {
                actual.push(show(path, &text, label.range, &format!("label: {}", label.message)));
            }
        }
        expected.sort();
        actual.sort();
        assert_eq!(expected.join("\n"), actual.join("\n"));
    }
}

fn annotations(text: &str) -> Vec<(TextRange, String)> {
    let mut out = Vec::new();
    let mut target = None;
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let is_annotation = line.trim_start().strip_prefix("//").is_some_and(|rest| rest.trim_start().starts_with('^'));
        match target {
            Some(start) if is_annotation => {
                let column = line.find('^').unwrap();
                let len = line[column..].chars().take_while(|&c| c == '^').count();
                let range = TextRange::at(TextSize::from((start + column) as u32), TextSize::from(len as u32));
                out.push((range, line[column + len..].trim().to_string()));
            }
            _ => target = Some(offset),
        }
        offset += line.len();
    }
    out
}

/// A diagnostic as `path:line:column-column message`, to compare them.
fn show(path: &str, text: &str, range: TextRange, message: &str) -> String {
    let range = if range.is_empty() { TextRange::at(range.start(), 1.into()) } else { range };
    let start = usize::from(range.start());
    let line_start = text[..start].rfind('\n').map_or(0, |i| i + 1);
    let line = text[..start].matches('\n').count() + 1;
    let column = start - line_start;
    format!("{path}:{line}:{column}-{} {message}", column + usize::from(range.len()))
}

#[test]
fn no_diagnostics() {
    check(
        r#"
fn id(x) = x;
struct Point { x: i32, y: i32 }
enum Color { Red = 1, Green, Blue }
let p = Point { x: id(1), y: 2 };
"#,
    );
}

#[test]
fn type_mismatch() {
    check(
        r#"
fn id(x) { return x }
  id(1) + 1;
  id("foo") + 1;
//^^^^^^^^^ label: this has type `str`
            //^ error: expected `str`, found `i32`
"#,
    );
}

#[test]
fn mismatch_in_declared_function() {
    check(
        r#"
fn f(x: i32) -> bool { x }
              //^^^^ label: expected because of the return type
                     //^ error: expected `bool`, found `i32`
"#,
    );
}

#[test]
fn unresolved_name_does_not_cascade() {
    check(
        r#"
  foo + 1;
//^^^ error: cannot find `foo`
  true + foo;
       //^^^ error: cannot find `foo`
"#,
    );
}

#[test]
fn syntax_error() {
    check(
        r#"
fn f( = 1;
    //^ error: expected ')'
"#,
    );
}

#[test]
fn types_in_signatures() {
    check(
        r#"
struct Pair[A, B] { a: A, b: B }
struct Wrap { inner: Missing }
                   //^^^^^^^ error: cannot find type `Missing`
fn f(p: Pair[i32]) -> i32 = 1;
      //^^^^^^^^^ error: expected 2 type arguments, found 1
const C: Nope = 1;
       //^^^^ error: cannot find type `Nope`
type Loop = Loop;
   //^^^^ error: type alias `Loop` refers to itself
fn g(x: Loop) -> i32 = 1;
      //^^^^ error: type alias `Loop` refers to itself
type A = B;
   //^ error: type alias `A` refers to itself
type B = A;
   //^ error: type alias `B` refers to itself
"#,
    );
}

#[test]
fn types_in_bodies() {
    check(
        r#"
fn f() {
  let x: Nope = 1;
       //^^^^ error: cannot find type `Nope`
  x
}
"#,
    );
}

#[test]
fn fields() {
    check(
        r#"
struct Point { x: i32, y: i32 }
let p = Point { x: 1 };
      //^^^^^^^^^^^^^^ error: missing field `y`
  p.z;
//^^^ error: there is no field `z`
"#,
    );
}

#[test]
fn class_instances() {
    check(
        r#"
class Show[T] { fn show(x: T) -> str; }
impl Show[i32] { fn show(x) = "int"; }
  show(true);
//^^^^ error: there is no instance `Show[bool]`
"#,
    );
}

#[test]
fn duplicate_definitions() {
    check(
        r#"
fn f() {}
fn f() {}
 //^ error: `f` is defined more than once
"#,
    );
}

#[test]
fn import_errors() {
    check_files(&[
        ("/lib.bolt", "pub fn a() {}\nfn hidden() {}\n"),
        (
            "/main.bolt",
            r#"
import "./missing" (x);
     //^^^^^^^^^^^ error: cannot find the file `./missing`
import "std/io" (print);
     //^^^^^^^^ error: cannot import `std/io`: only paths starting with `./` or `../` are supported
import "./lib" (nope, hidden, a);
              //^^^^ error: the imported file has nothing named `nope`
                    //^^^^^^ error: `hidden` is private
"#,
        ),
    ]);
}

#[test]
fn cyclic_inference() {
    let a = r#"
import "./b" (g);
pub fn f(x) = g(x);
     //^ error: cannot infer the type of this function, because it calls functions in other files that call it back; give it a complete type signature
"#;
    let b = r#"
import "./a" (f);
pub fn g(x) = f(x);
     //^ error: cannot infer the type of this function, because it calls functions in other files that call it back; give it a complete type signature
"#;
    // Both are reported, whichever file is checked first.
    check_files(&[("/a.bolt", a), ("/b.bolt", b)]);
    check_files(&[("/b.bolt", b), ("/a.bolt", a)]);
}

#[test]
fn unsupported_is_a_warning() {
    check(
        r#"
fn f(x: i32) -> i32 = x?;
                    //^^ warning: the type checker doesn't support this yet
"#,
    );
}

#[test]
fn bodies_of_items_in_impls_and_enums() {
    check(
        r#"
class Get[T] { fn get(x: T) -> i32; }
struct Counter { n: i32 }
impl Get[Counter] {
  fn get(c) = c;
            //^ error: expected `i32`, found `Counter`
}
enum E { A = true }
           //^^^^ error: expected `i32`, found `bool`
"#,
    );
}

#[test]
fn mismatches_point_at_annotations() {
    check(
        r#"
const C: bool = 1;
       //^^^^ label: expected because of the type of the constant
              //^ error: expected `bool`, found `i32`
fn f() {
  let x: i32 = true;
       //^^^ label: expected because of this annotation
             //^^^^ error: expected `i32`, found `bool`
  x
}
fn g() -> i32 { return true; }
        //^^^ label: expected because of the return type
                     //^^^^ error: expected `i32`, found `bool`
"#,
    );
}

#[test]
fn mismatches_point_at_parameters() {
    check(
        r#"
fn takes(n: i32, b: bool) -> i32 = n;
                  //^^^^ label: expected because of this parameter of `takes`
  takes(1, 2);
         //^ error: expected `bool`, found `i32`
"#,
    );
}

#[test]
fn parameters_can_be_in_other_files() {
    check_files(&[
        (
            "/lib.bolt",
            r#"
pub fn negate(x) = !x;
            //^ label: expected because of this parameter of `negate`
"#,
        ),
        (
            "/main.bolt",
            r#"
import "./lib" (negate);
  negate(1);
       //^ error: expected `bool`, found `i32`
"#,
        ),
    ]);
}

#[test]
fn mismatches_point_at_fields() {
    check(
        r#"
struct Point { x: i32, y: bool }
                        //^^^^ label: expected because of the type of field `y`
let p = Point { x: 1, y: 2 };
                       //^ error: expected `bool`, found `i32`
"#,
    );
}

#[test]
fn branches_point_at_each_other() {
    check(
        r#"
let a = if true { 1 } else { "no" };
              //^^^^^ label: this has type `i32`
                           //^^^^ error: expected `i32`, found `str`
fn f(b) = match b { true => 1, false => "no" };
                          //^ label: this has type `i32`
                                      //^^^^ error: expected `i32`, found `str`
"#,
    );
}

#[test]
fn impls_are_only_for_classes() {
    check(
        r#"
struct Counter { n: i32 }
impl Counter {
   //^^^^^^^ error: `Counter` is not a class
  fn get(c) = c.n;
}
"#,
    );
}

#[test]
fn methods_are_functions_in_scope() {
    check(
        r#"
struct Counter { n: i32 }
fn get(c: Counter) -> i32 = c.n;
let c = Counter { n: 1 };
let n: i32 = c.get();
  c.size();
  //^^^^ error: cannot find `size`
"#,
    );
}

#[test]
fn one_type_per_file() {
    check_files(&[
        ("/list.bolt", "pub struct List { n: i32 }\npub fn new() -> List = List { n: 0 };\npub fn len(l: List) -> i32 = l.n;\n"),
        (
            "/main.bolt",
            r#"
import "./list" as List;
let l = List.new();
let n: i32 = List.len(l);
let b: bool = List.len(l);
     //^^^^ label: expected because of this annotation
            //^^^^^^^^^^^ error: expected `bool`, found `i32`
"#,
        ),
    ]);
}

#[test]
fn methods_must_be_declared_by_the_class() {
    check(
        r#"
class Show[T] { fn show(x: T) -> str; }
impl Show[i32] {
  fn show(x) = "int";
  fn shwo(x) = x + 1;
   //^^^^ error: `shwo` is not a method of class `Show`
}
struct Dog;
impl Dog { fn bark(d) = d; }
   //^^^ error: `Dog` is not a class
"#,
    );
}

#[test]
fn impls_must_define_the_methods_of_their_class() {
    check(
        r#"
class Eq[T] {
  fn eq(a: T, b: T) -> bool;
  fn ne(a: T, b: T) -> bool = !eq(a, b);
}
class Ord[T] { fn lt(a: T, b: T) -> bool; fn gt(a: T, b: T) -> bool; }
impl Eq[i32] { fn eq(a, b) = true; }
impl Eq[bool] {}
   //^^^^^^^^ error: missing method `eq` of class `Eq`
impl Ord[i32];
   //^^^^^^^^ error: missing methods `lt`, `gt` of class `Ord`
"#,
    );
}

#[test]
fn impls_must_define_the_consts_and_types_of_their_class() {
    check(
        r#"
class Sized[T] {
  const SIZE: usize;
            //^^^^^ label: expected because of the declaration in class `Sized`
  const ALIGN: usize = 8;
  type Repr;
  type Out = T;
  fn size(x: T) -> usize;
}
impl Sized[i32] {
  const SIZE: usize = 4;
  type Repr = i32;
  fn size(x) = 4;
}
impl Sized[bool] {}
   //^^^^^^^^^^^ error: missing method `size`, constant `SIZE` and type `Repr` of class `Sized`
impl Sized[u8] {
   //^^^^^^^^^ error: missing type `Repr` of class `Sized`
  const SIZE: bool = true;
            //^^^^ error: expected `usize`, found `bool`
  const SIZ: usize = 1;
      //^^^ error: `SIZ` is not a constant of class `Sized`
  type Rep = u8;
     //^^^ error: `Rep` is not a type of class `Sized`
  fn size(x) = 1;
}
class Default[T] { const VALUE: T; }
                              //^ label: expected because of the declaration in class `Default`
impl Default[i32] { const VALUE: i32 = 0; }
impl Default[bool] { const VALUE: i32 = 0; }
                                //^^^ error: expected `bool`, found `i32`
"#,
    );
}

#[test]
fn unknown_types_are_named_per_message() {
    check(
        r#"
let f = |x| x;
  f + 1;
//^ label: this has type `fn(a) -> a`
    //^ error: expected `fn(a) -> a`, found `i32`
class Show[T] { fn show(x: T) -> str; }
let s = show([]);
      //^^^^ error: cannot tell which instance of `Show[[a]]` to use; add a type annotation
"#,
    );
}

#[test]
fn unknown_types_skip_the_names_of_type_parameters() {
    check(
        r#"
fn g[a](x: a) = x + (|y| y);
              //^ label: this has type `a`
                   //^^^^^ error: expected `a`, found `fn(b) -> b`
"#,
    );
}

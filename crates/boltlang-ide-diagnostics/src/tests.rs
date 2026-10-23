use boltlang_base_db::{TestDb, source_text};
use boltlang_syntax::rowan::{TextRange, TextSize};

use crate::diagnostics;

/// Check the diagnostics of a file against its annotations. A comment like
///
/// ```text
/// let x: i32 = true;
///             //^^^^ error: expected `i32`, found `bool`
/// ```
///
/// marks the columns of the carets in the line above it, which is the
/// closest line that isn't an annotation. A diagnostic at an empty range,
/// like a missing token, is marked by one caret.
#[track_caller]
fn check(fixture: &str) {
    check_files(&[("/main.bolt", fixture)]);
}

/// Like [`check`], for several files that can import each other.
#[track_caller]
fn check_files(files: &[(&str, &str)]) {
    let db = TestDb::new();
    let files: Vec<_> = files.iter().map(|(path, text)| (*path, db.file(path, text))).collect();
    for (path, file) in files {
        let text = source_text(&db, file);
        let mut expected: Vec<String> =
            annotations(&text).into_iter().map(|(range, message)| show(path, &text, range, &message)).collect();
        let mut actual: Vec<String> = diagnostics(&db, file)
            .into_iter()
            .map(|d| {
                let range = if d.range.is_empty() { TextRange::at(d.range.start(), 1.into()) } else { d.range };
                show(path, &text, range, &format!("{}: {}", d.severity, d.message))
            })
            .collect();
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
            //^ error: expected `str`, found `i32`
"#,
    );
}

#[test]
fn mismatch_in_declared_function() {
    check(
        r#"
fn f(x: i32) -> bool { x }
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
struct Counter { n: i32 }
impl Counter {
  fn get(c: Counter) -> bool = c.n;
                             //^^^ error: expected `bool`, found `i32`
}
enum E { A = true }
           //^^^^ error: expected `i32`, found `bool`
"#,
    );
}

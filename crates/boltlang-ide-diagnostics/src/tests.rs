use boltlang_base_db::{TestDb, source_text};
use boltlang_syntax::rowan::{TextRange, TextSize};

use crate::diagnostics;

/// Check the diagnostics of a file against its annotations. A comment like
///
/// ```text
/// let x: i32 = True;
///    //^^^ label: expected because of this annotation
///             //^^^^ error: expected `i32`, found `Bool`
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
    check_db(TestDb::new(), files);
}

/// Like [`check`], with the repository's `std` as the prelude, for code
/// that indexes arrays or loops over them.
#[track_caller]
fn check_with_std(fixture: &str) {
    let mut db = TestDb::new();
    db.add_std();
    db.set_prelude(Some("std"), true);
    check_db(db, &[("/main.bolt", fixture)]);
}

#[track_caller]
fn check_db(db: TestDb, files: &[(&str, &str)]) {
    let files: Vec<_> = files.iter().map(|(path, text)| (*path, db.file(path, text))).collect();
    let all: Vec<_> = files.iter().flat_map(|&(_, file)| diagnostics(&db, file)).collect();
    for &(path, file) in &files {
        let text = source_text(&db, file);
        let mut expected: Vec<String> =
            annotations(text).into_iter().map(|(range, message)| show(path, text, range, &message)).collect();
        let mut actual = Vec::new();
        for d in &all {
            if d.file == file {
                actual.push(show(path, text, d.range, &format!("{}: {}", d.severity, d.message)));
            }
            for label in d.labels.iter().filter(|l| l.file == file) {
                actual.push(show(path, text, label.range, &format!("label: {}", label.message)));
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
//^^^^^^^^^ label: this has type `String`
            //^ error: expected `String`, found an integer
"#,
    );
}

#[test]
fn mismatch_in_declared_function() {
    check(
        r#"
fn f(x: i32) -> Bool { x }
              //^^^^ label: expected because of the return type
                     //^ error: expected `Bool`, found `i32`
"#,
    );
}

#[test]
fn unresolved_name_does_not_cascade() {
    check(
        r#"
  foo + 1;
//^^^ error: cannot find `foo`
  True + foo;
       //^^^ error: cannot find `foo`
"#,
    );
}

#[test]
fn unresolved_type_is_reported_without_its_arguments() {
    check(
        r#"
  let x: List[T, i32] = 1;
       //^^^^ error: cannot find type `List`
            //^ error: cannot find type `T`
"#,
    );
}

#[test]
fn custom_operator_calls_its_definition() {
    check(
        r#"
  fn[T] (a: T) <> (b: T) -> T = a;
  let x: i32 = 1 <> 2;
  let y: Bool = 1 <> 2;
       //^^^^ label: expected because of this annotation
              //^^^^^^ error: expected `Bool`, found an integer
"#,
    );
}

#[test]
fn range_before_a_syntax_error_leaves_out_whitespace() {
    check(
        r#"
  fn f() = g x;
         //^ error: cannot find `g`
           //^ error: expected ';'
           //^ error: cannot find `x`
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
  //^ error: `imm Point` has no field `z`
"#,
    );
}

#[test]
fn class_instances() {
    check(
        r#"
class Show[T] { fn show(x: T) -> String; }
impl Show[i32] { fn show(x) = "int"; }
  show(True);
//^^^^ error: there is no instance `Show[Bool]`
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
     //^^^^^^^^ error: cannot find the package `std` in `.bolt/packages`
import "./lib" (nope, hidden, a);
              //^^^^ error: the imported file has nothing named `nope`
                    //^^^^^^ error: `hidden` is private
"#,
        ),
    ]);
}

#[test]
fn imports_of_variants() {
    check_files(&[
        ("/shapes.bolt", "pub enum Shape { Circle(f64), Square(f64) }\npub struct Point;\n"),
        (
            "/main.bolt",
            r#"
import "./shapes" as s;
import s.Shape.*;
import s.Point.*;
     //^^^^^^^ error: `s.Point` is not an enum, so it has no variants to import
import Nope.*;
     //^^^^ error: cannot find the enum `Nope`
let a = Circle(1.0);
let b: s.Shape = Square(2.0);
let c: i32 = Circle(1.0);
           //^^^^^^^^^^^ error: expected `i32`, found `Shape`
     //^^^ label: expected because of this annotation
"#,
        ),
    ]);
}

#[test]
fn imported_variant_in_a_pattern_matches_the_variant() {
    // Without the import, `None` would bind a new name and match anything.
    check(
        r#"
  import Opt.*;
  enum Opt { Some(i32), None }
  fn f(n: i32) -> i32 = match n {
    None => 0,
  //^^^^ error: expected `i32`, found `Opt`
    other => other,
  };
  fn g(o: Opt) -> Opt = match o {
    None => None,
    Some(x) => Some(x + 1),
  };
"#,
    );
}

#[test]
fn unknown_fields_are_reported_at_their_name() {
    check(
        r#"
  struct Point { x: i32, y: i32 }
  let z = 3;
  Point { x: 1, y: 2, w: 3 };
                    //^ error: there is no field `w`
  Point { x: 1, y: 2, z };
                    //^ error: there is no field `z`
  fn f(p: Point) = match p { Point { x, v, .. } => x };
                                      //^ error: there is no field `v`
"#,
    );
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
enum E { A = True }
           //^^^^ error: expected an integer, found `Bool`
"#,
    );
}

#[test]
fn mismatches_point_at_annotations() {
    check(
        r#"
const C: Bool = 1;
       //^^^^ label: expected because of the type of the constant
              //^ error: expected `Bool`, found an integer
fn f() {
  let x: i32 = True;
       //^^^ label: expected because of this annotation
             //^^^^ error: expected `i32`, found `Bool`
  x
}
fn g() -> i32 { return True; }
        //^^^ label: expected because of the return type
                     //^^^^ error: expected `i32`, found `Bool`
"#,
    );
}

#[test]
fn mismatches_point_at_parameters() {
    check(
        r#"
fn takes(n: i32, b: Bool) -> i32 = n;
                  //^^^^ label: expected because of this parameter of `takes`
  takes(1, 2);
         //^ error: expected `Bool`, found an integer
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
       //^ error: expected `Bool`, found an integer
"#,
        ),
    ]);
}

#[test]
fn mismatches_point_at_fields() {
    check(
        r#"
struct Point { x: i32, y: Bool }
                        //^^^^ label: expected because of the type of field `y`
let p = Point { x: 1, y: 2 };
                       //^ error: expected `Bool`, found an integer
"#,
    );
}

#[test]
fn branches_point_at_each_other() {
    check(
        r#"
let a = if True { 1 } else { "no" };
              //^^^^^ label: this has type `i32`
                           //^^^^ error: expected an integer, found `String`
fn f(b) = match b { True => 1, False => "no" };
                          //^ label: this has type `i32`
                                      //^^^^ error: expected an integer, found `String`
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
let b: Bool = List.len(l);
     //^^^^ label: expected because of this annotation
            //^^^^^^^^^^^ error: expected `Bool`, found `i32`
"#,
        ),
    ]);
}

#[test]
fn methods_must_be_declared_by_the_class() {
    check(
        r#"
class Show[T] { fn show(x: T) -> String; }
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
  fn eq(a: T, b: T) -> Bool;
  fn ne(a: T, b: T) -> Bool = !eq(a, b);
}
class Ord[T] { fn lt(a: T, b: T) -> Bool; fn gt(a: T, b: T) -> Bool; }
impl Eq[i32] { fn eq(a, b) = True; }
impl Eq[Bool] {}
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
impl Sized[Bool] {}
   //^^^^^^^^^^^ error: missing method `size`, constant `SIZE` and type `Repr` of class `Sized`
impl Sized[u8] {
   //^^^^^^^^^ error: missing type `Repr` of class `Sized`
  const SIZE: Bool = True;
            //^^^^ error: expected `usize`, found `Bool`
  const SIZ: usize = 1;
      //^^^ error: `SIZ` is not a constant of class `Sized`
  type Rep = u8;
     //^^^ error: `Rep` is not a type of class `Sized`
  fn size(x) = 1;
}
class Default[T] { const VALUE: T; }
                              //^ label: expected because of the declaration in class `Default`
impl Default[i32] { const VALUE: i32 = 0; }
impl Default[Bool] { const VALUE: i32 = 0; }
                                //^^^ error: expected `Bool`, found `i32`
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
    //^ error: expected `fn(a) -> a`, found an integer
class Show[T] { fn show(x: T) -> String; }
let s = show([]);
      //^^^^ error: cannot tell which instance of `Show[imm [a]]` to use; add a type annotation
"#,
    );
}

#[test]
fn changing_arrays_that_are_not_mut() {
    check_with_std(
        r#"
let xs = [1, 2];
  xs[0] = 3;
//^^ error: cannot change an element of `xs`, which is `imm [i32]`; only a `mut` array can be changed
fn first(ys: [i32]) { ys[0] = 1; }
                    //^^ error: cannot change an element of `ys`, which is `[i32]`; only a `mut` array can be changed
let zs = mut [1, 2];
zs[0] = 3;
struct Inventory { items: [i32] }
fn clear(inv: Inventory) { inv.items[0] = 0; }
                         //^^^^^^^^^ error: cannot change an element of `inv.items`, which is `[i32]`; only a `mut` array can be changed
"#,
    );
}

#[test]
fn changing_structs_that_are_not_mut() {
    check(
        r#"
struct Point { x: i32, y: i32 }
let p = Point { x: 1, y: 2 };
  p.x = 3;
//^ error: a field of an `imm Point` is changed here, but only a `mut` struct can be changed
let q = mut Point { x: 1, y: 2 };
q.x = 3;
fn move_x(r) { r.x = 1; }
  move_x(p);
//^^^^^^ error: a field of an `imm Point` is changed here, but only a `mut` struct can be changed
struct Bag { items: mut [i32] }
                  //^^^^^^^^^ error: a field can't be `mut`: it has the mutability of its struct, so make the struct `mut` instead
let a = [mut [1]];
      //^^^^^^^^^ error: an `imm [mut [i32]]` can't hold a `mut [i32]`: what an `imm` value holds can't be changed either
fn f(x: mut i32) {}
      //^^^^^^^ error: only an array or a struct can be `mut` or `imm`
"#,
    );
}

#[test]
fn unknown_mutabilities() {
    check(
        r#"
fn same(xs: 'm [i32]) -> 'm [i32] = xs;
fn f() { let ys: 'q [i32] = [1]; }
               //^^^^^^^^ error: the mutability of `'q [i32]` isn't one of this function's: a name like `'m` can only be used in the signature and the body of the function or impl that it's written in
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

#[test]
fn package_imports() {
    check_files(&[
        (
            "/proj/.bolt/packages/std/io.bolt",
            r#"
pub fn print(s: String) -> () = ();
              //^^^^^^ label: expected because of this parameter of `print`
"#,
        ),
        (
            "/proj/main.bolt",
            r#"
import "std/io" (print);
import "std" (x);
     //^^^^^ error: the package `std` has no file `lib.bolt`
import "std/../secret" (y);
     //^^^^^^^^^^^^^^^ error: cannot import `std/../secret`: import a file with a path like `./file`, or a package with its name, like `std/io`
print("hello");
print(1);
    //^ error: expected `String`, found an integer
"#,
        ),
    ]);
}

#[test]
fn package_items_through_an_alias() {
    check_files(&[
        ("/proj/.bolt/packages/geo/lib.bolt", "pub(package) fn helper() -> i32 = 1;\npub fn area() -> i32 = helper();\n"),
        (
            "/proj/main.bolt",
            r#"
import "geo" (helper);
            //^^^^^^ error: `helper` is only visible inside its package
import "geo" as G;
let a: i32 = G.area();
let h: i32 = G.helper();
             //^^^^^^ error: cannot find `helper`
"#,
        ),
    ]);
}

#[test]
fn fields_of_values_of_unknown_types() {
    check(
        r#"
struct Point { x: i32 }
struct Size { x: u8 }
fn get_x(p) = p.x;
let a: i32 = get_x(Point { x: 1 });
let b: u8 = get_x(Size { x: 2u8 });
let c = get_x(True);
      //^^^^^ error: `Bool` has no field `x`
"#,
    );
}

#[test]
fn kinds() {
    check(
        r#"
enum Either[E, A] { Left(E), Right(A) }
class Functor[F] { fn fmap[A, B](f: fn (A) -> B, x: F[A]) -> F[B]; }
impl Functor[i32] {}
           //^^^ error: expected a type of kind `Type -> Type`, found `i32` of kind `Type`
   //^^^^^^^^^^^^ error: missing method `fmap` of class `Functor`
impl Functor[Either] {}
           //^^^^^^ error: expected a type of kind `Type -> Type`, found `Either` of kind `Type -> Type -> Type`
   //^^^^^^^^^^^^^^^ error: missing method `fmap` of class `Functor`
// `x: F` comes first, so it makes `F` a type.
fn f[F](x: F) -> i32 where Functor[F] = 1;
                                 //^ error: expected a type of kind `Type -> Type`, found `F` of kind `Type`
"#,
    );
}

#[test]
fn types_without_values() {
    check(
        r#"
enum Node[T] { Ref(Node[T]) }
   //^^^^ warning: `Node` has no values, since none of its variants can be built: `Ref` needs a value of `Node[T]`, which has none
struct S { s: S }
     //^ warning: `S` has no values, since its field `s` needs a value of `S`, which has none
struct A { b: B }
     //^ warning: `A` has no values, since its field `b` needs a value of `B`, which has none
struct B { a: A }
     //^ warning: `B` has no values, since its field `a` needs a value of `A`, which has none
enum Never {}
struct Wrap { n: Never }
     //^^^^ warning: `Wrap` has no values, since its field `n` needs a value of `Never`, which has none
enum List[T] { Cons(T, List[T]), Nil }
struct Tree { children: [Tree] }
"#,
    );
}

#[test]
fn types_without_values_in_other_files() {
    check_files(&[
        (
            "/a.bolt",
            r#"import "./b" (B);
pub struct A { b: B }
         //^ warning: `A` has no values, since its field `b` needs a value of `B`, which has none
"#,
        ),
        (
            "/b.bolt",
            r#"
import "./a" (A);
pub struct B { a: A }
         //^ warning: `B` has no values, since its field `a` needs a value of `A`, which has none
"#,
        ),
    ]);
}

#[test]
fn builtin_declarations() {
    check(
        r#"
extern "bolt" {
  fn print(text: String);
  fn prnt(text: String);
   //^^^^ error: there is no builtin `prnt`; the builtins are `print`, `panic`, `array_len`, `array_alloc`, `array_get`, `array_set`
  fn panic(message: String) -> i32;
   //^^^^^ error: the builtin `panic` must have the type `fn(String) -> !`
}
"#,
    );
}

#[test]
fn generic_builtin_declarations() {
    // The type parameters can have any name.
    check(r#"extern "bolt" { fn array_len[Elem](items: [Elem]) -> i32; }"#);
    check(
        r#"
extern "bolt" {
  fn array_len[T](xs: mut [T]) -> i32;
   //^^^^^^^^^ error: the builtin `array_len` must have the type `fn([a]) -> i32`
}
"#,
    );
    check(
        r#"
extern "bolt" {
  fn array_len[T](xs: [T]) -> T;
   //^^^^^^^^^ error: the builtin `array_len` must have the type `fn([a]) -> i32`
}
"#,
    );
    check(r#"extern "bolt" { fn array_alloc[T](len: i32) -> mut [T]; }"#);
    // Without `mut`, the result would be `imm`, and the elements could never
    // be set.
    check(
        r#"
extern "bolt" {
  fn array_alloc[T](len: i32) -> [T];
   //^^^^^^^^^^^ error: the builtin `array_alloc` must have the type `fn(i32) -> mut [a]`
}
"#,
    );
}

#[test]
fn associated_types() {
    check(
        r#"
class Index[C] { type Item; }
class Other[C] { type Item; }
fn none[C](c: C) -> C.Item = none(c);
                  //^^^^^^ error: cannot find the associated type `C.Item`: none of the classes that its type parameter has an instance of declares it; add a bound or a `where` clause
fn both[C](c: C) -> C.Item where Index[C], Other[C] = both(c);
                  //^^^^^^ error: `C.Item` is ambiguous: several classes of its type parameter declare it
fn one[C](c: C) -> C.Item where Index[C] = one(c);
"#,
    );
}

#[test]
fn assign_to_immutable() {
    check(
        r#"
fn f() {
  let x = 1;
  x = 2;
//^ error: cannot assign to `x`, which isn't `mut`
  let mut y = 1;
  y = 2;
}
"#,
    );
}

#[test]
fn assign_to_non_place() {
    check(
        r#"
fn g() -> i32 = 1;
fn f() {
  g() = 3;
//^^^ error: cannot assign to this; only a variable, a field or an element can be assigned to
}
"#,
    );
}

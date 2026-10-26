use boltlang_base_db::{File, parse_file};
use boltlang_syntax::SyntaxNode;

use super::*;
use crate::test_db::TestDb;

#[test]
fn lowers_all_kinds_of_items() {
    let db = TestDb::new();
    let file = db.file(
        "/main.bolt",
        r#"
import "./a";
pub import "./b" (x, y as z);
import "./c" as c;
import "./d" *;
pub fn f(x: i32) -> i32 = x;
fn a + b = add(a, b);
async const fn g();
pub(package) struct Point { x: i32 }
enum Color { Red, Green = 2 }
class Show[T] { fn show(x: T) -> String; type Out; }
impl Show[Point] { fn show(x: Point) -> String { "" } const N: i32 = 1; m!(); }
const MAX: usize = 10;
type Alias = Point;
extern "C" { fn malloc(size: usize) -> Ptr; }
macro_rules! m { () => {} }
let top_level_statements_are_not_items = 1;
"#,
    );
    let tree = item_tree(&db, file);
    insta_like(
        &format!("{tree:?}"),
        &[
            "Import(Idx::<Import>(0))",
            "source: Some(\"./b\")",
            "(\"x\", None)",
            "(\"y\", Some(\"z\"))",
            "Alias(\"c\")",
            "kind: Glob",
            "Operator(\"+\")",
            "is_async: true",
            "has_body: false",
            "visibility: Package",
            "\"Red\"",
            "\"Green\"",
            "abi: Some(\"C\")",
        ],
    );
    assert_eq!(tree.top_level.len(), 15);
    assert_eq!(tree.functions.len(), 6);
    assert_eq!(tree.classes[0].items.len(), 2);
    assert_eq!(tree.impls[0].items.len(), 3);
    assert_eq!(tree.macro_calls.len(), 1);
}

/// Assert that `text` contains each of `parts`.
fn insta_like(text: &str, parts: &[&str]) {
    for part in parts {
        assert!(text.contains(part), "{part:?} not found in:\n{text}");
    }
}

#[test]
fn ast_ids_point_back_to_their_items() {
    let db = TestDb::new();
    let file = db.file(
        "/main.bolt",
        "fn a() { fn not_an_item() {} }\nclass C[T] { fn b(x: T); }\nfn c() {}\nimpl D[i32] { fn d() {} }\n",
    );
    let tree = item_tree(&db, file);
    let root = root(&db, file);
    let names: Vec<_> = tree
        .functions
        .iter()
        .map(|f| {
            let node = f.ast_id.to_node(&db, file, &root).unwrap();
            let decl = ast::FuncDecl::cast(node).unwrap();
            decl.name().unwrap().text()
        })
        .collect();
    assert_eq!(names, ["a", "b", "c", "d"]);
}

fn root(db: &TestDb, file: File) -> SyntaxNode {
    SyntaxNode::new_root(parse_file(db, file).tree(db).clone())
}

/// A query that depends on the item tree, to check when it reruns.
#[salsa::tracked]
fn function_count(db: &dyn Db, file: File) -> usize {
    item_tree(db, file).functions.len()
}

#[test]
fn editing_a_body_does_not_change_the_item_tree() {
    let mut db = TestDb::new();
    let file = db.file("/main.bolt", "fn f() { 1 }\nfn g() {}\n");
    assert_eq!(*function_count(&db, file), 2);

    let executed = db.log_executed(|db| {
        db.edit(file, "fn f() { 1 + 2 + 3 }\nfn g() {}\n");
        assert_eq!(*function_count(db, file), 2);
    });
    assert!(executed.iter().any(|q| q.contains("item_tree")), "{executed:?}");
    assert!(!executed.iter().any(|q| q.contains("function_count")), "{executed:?}");

    let executed = db.log_executed(|db| {
        db.edit(file, "fn f() { 1 + 2 + 3 }\nfn g() {}\nfn h() {}\n");
        assert_eq!(*function_count(db, file), 3);
    });
    assert!(executed.iter().any(|q| q.contains("function_count")), "{executed:?}");
}

#[test]
fn every_ast_id_finds_its_node() {
    let db = TestDb::new();
    let text = r#"
import "./a" (x);
fn f() { fn not_an_item() {} }
struct Point { x: i32 }
enum Color { Red }
class Show[T] { fn show(x: T) -> String; type Out; const N: T; }
impl Show[Point] { fn show(x) = ""; type Out = i32; const N: Point = Point { x: 1 }; m!(); }
extern "C" { fn malloc(size: usize) -> Ptr; }
macro_rules! m { () => {} }
const MAX: usize = 10;
type Alias = Point;
"#;
    let file = db.file("/main.bolt", text);
    let tree = item_tree(&db, file);
    let root = root(&db, file);
    let ast_ids: Vec<AstId> = std::iter::empty()
        .chain(tree.imports.iter().map(|i| i.ast_id))
        .chain(tree.functions.iter().map(|i| i.ast_id))
        .chain(tree.structs.iter().map(|i| i.ast_id))
        .chain(tree.enums.iter().map(|i| i.ast_id))
        .chain(tree.classes.iter().map(|i| i.ast_id))
        .chain(tree.impls.iter().map(|i| i.ast_id))
        .chain(tree.consts.iter().map(|i| i.ast_id))
        .chain(tree.type_aliases.iter().map(|i| i.ast_id))
        .chain(tree.extern_blocks.iter().map(|i| i.ast_id))
        .chain(tree.macro_rules.iter().map(|i| i.ast_id))
        .chain(tree.macro_calls.iter().map(|i| i.ast_id))
        .collect();
    // Everything but the function in a body is an item.
    assert_eq!(ast_ids.len(), 18);
    let mut ranges = Vec::new();
    for ast_id in ast_ids {
        let node = ast_id.to_node(&db, file, &root).unwrap_or_else(|| panic!("no node for {ast_id:?}"));
        assert_eq!(node.kind(), ast_id.kind);
        ranges.push(node.text_range());
    }
    // Each finds a node of its own.
    let count = ranges.len();
    ranges.sort_by_key(|r| (r.start(), r.end()));
    ranges.dedup();
    assert_eq!(ranges.len(), count);
}

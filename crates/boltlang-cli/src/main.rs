
use std::path::PathBuf;

use boltlang::rowan::NodeOrToken;
use boltlang::salsa::Database;

use boltlang::{
    RootDatabase,
    File,
    SyntaxElement,
    Diagnostic,
    SyntaxKind,
    SyntaxNode,
    parse_file
};

fn print(indent: usize, element: SyntaxElement) {
    let kind: SyntaxKind = element.kind();
    print!("{:indent$}", "", indent = indent);
    match element {
        NodeOrToken::Node(node) => {
            println!("- {:?}", kind);
            for child in node.children_with_tokens() {
                print(indent + 2, child);
            }
        }
        NodeOrToken::Token(token) => println!("- {:?} {:?}", token.text(), kind),
    }
}

fn main() {
    let mut args = std::env::args();
    let fname = args.nth(1).expect("must provide a filename");
    let text = std::fs::read_to_string(&fname).expect(&format!("could not read {}", &fname));
    RootDatabase::new(None).attach(|db| {
        let file = File::new(db, PathBuf::from(fname), text);
        let parsed = parse_file(db, file);
        let errors = parse_file::accumulated::<Diagnostic>(db, file);
        for error in errors {
            eprintln!("{error:#?}");
        }
        print(0, SyntaxNode::new_root(parsed.node(db).clone()).into());
    });
}

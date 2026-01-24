
use boltlang::rowan::NodeOrToken;
use boltlang::salsa::Database;

use boltlang::{
    BoltDatabaseImpl,
    SourceProgram,
    SyntaxElement,
    SyntaxError,
    SyntaxKind,
    SyntaxNode,
    parse
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
    BoltDatabaseImpl::default().attach(|db| {
        let source_program = SourceProgram::new(db, text);
        let prog = parse(db, source_program);
        let errors = parse::accumulated::<SyntaxError>(db, source_program);
        for error in errors {
            eprintln!("{error:#?}");
        }
        print(0, SyntaxNode::new_root(prog.node(db).clone()).into());
    });
}

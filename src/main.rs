
use rowan::NodeOrToken;

use crate::syntax::{SyntaxElement, SyntaxKind, SyntaxNode};
use crate::parser::{event::intersperse_trivia, grammar::parse_source_file, parser::Parser};

mod util;
mod syntax;
mod scanner;
mod parser;

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
    let lexed = scanner::tokenize(&text);
    let inp = lexed.to_input();
    let mut p = Parser::new(&inp);
    parse_source_file(&mut p);
    let interspersed = intersperse_trivia(
        p.finish().into_iter(),
        &lexed
    );
    let (node, errors) = parser::event::process(
        interspersed.into_iter(),
        &lexed,
        &text
    );
    for error in errors {
        eprintln!("Error: {}", error);
    }
    let syn = SyntaxNode::new_root(node);
    print(0, syn.into());
}

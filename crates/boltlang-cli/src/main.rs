
use std::path::PathBuf;

use ariadne::Report;
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
use fluent_uri::Uri;

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

fn severity_to_report_kind(sev: boltlang::Severity) -> ariadne::ReportKind<'static> {
    match sev {
        boltlang::Severity::Info => ariadne::ReportKind::Advice,
        boltlang::Severity::Warn => ariadne::ReportKind::Warning,
        boltlang::Severity::Error => ariadne::ReportKind::Error,
        boltlang::Severity::Fatal => ariadne::ReportKind::Error,
    }
}

fn main() {
    let mut args = std::env::args();
    let fname = args.nth(1).expect("must provide a filename");
    let text = std::fs::read_to_string(&fname).expect(&format!("could not read {}", &fname));
    RootDatabase::new(None).attach(|db| {
        let file = File::new(db, format!("file://{}", std::fs::canonicalize(fname).unwrap().to_string_lossy()), text);
        let parsed = parse_file(db, file);
        let diagnostics = parse_file::accumulated::<Diagnostic>(db, file);
        let root_node = SyntaxNode::new_root(parsed.node(db).clone());
        for diagnostic in diagnostics {
            match diagnostic.source() {
                None => eprintln!("Error: {}", diagnostic),
                Some(source) => {
                    let parsed = Uri::parse(source.file().uri(db)).unwrap();
                    let fname = parsed.path();
                    let contents = source.file().contents(db);
                    Report::build(severity_to_report_kind(diagnostic.severity()), (fname.as_str(), source.span().clone()))
                        .with_code(diagnostic.code())
                        .with_message(diagnostic.message())
                        .with_label(
                            ariadne::Label::new((fname.as_str(), source.span().clone()))
                                .with_message(diagnostic.message())
                        )
                        .finish()
                        .print((fname.as_str(), ariadne::Source::from(contents)))
                        .unwrap();
                }
            }
        }
        print(0, root_node.into());
    });
}

use std::io::Write;

use boltlang_parser::Diagnostic;
use boltlang_syntax::{NodeOrToken, SyntaxElement, SyntaxKind, SyntaxNode, rowan::GreenNode};

fn write_element<W: Write>(writer: &mut W, element: &SyntaxElement, indent: usize) -> std::io::Result<()> {
    match element {
        NodeOrToken::Node(node) => {
            writeln!(writer, "{:indent$}- {name:?}", "", indent = indent, name = element.kind())?;
            for child in node.children_with_tokens() {
                write_element(writer, &child, indent + 2)?;
            }
            Ok(())
        }
        NodeOrToken::Token(token) => {
            writeln!(writer, "{:indent$}- {name:?} {text:?}", "", indent = indent, name = element.kind(), text = token.text())
        }
    }
}

/// Render a syntax tree followed by its diagnostics as text for snapshotting.
pub fn dump(node: &GreenNode, diags: &[Diagnostic]) -> String {
    let node = SyntaxNode::new_root(node.clone()).into();
    let mut out = Vec::new();
    write_element(&mut out, &node, 0).unwrap();
    for d in diags {
        write!(out, "---\n{}", yaml_serde::to_string(d).unwrap()).unwrap();
    }
    String::from_utf8(out).unwrap()
}

/// Whether the tree contains an `ERROR` node anywhere.
pub fn has_error_node(node: &GreenNode) -> bool {
    SyntaxNode::new_root(node.clone())
        .descendants()
        .any(|n| n.kind() == SyntaxKind::ERROR)
}

/// Parse a whole file on its own thread, so a hang or panic in the parser
/// becomes an error for this file instead of stalling the whole test run.
pub fn parse_file_with_timeout(path: &std::path::Path) -> Result<(GreenNode, Vec<Diagnostic>), String> {
    use std::sync::mpsc::RecvTimeoutError;
    let text = std::fs::read_to_string(path).unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(boltlang_parser::parse_file(&text));
    });
    match rx.recv_timeout(std::time::Duration::from_secs(5)) {
        Ok(result) => Ok(result),
        Err(RecvTimeoutError::Timeout) => Err(format!("{}: did not finish parsing within 5s", path.display())),
        Err(RecvTimeoutError::Disconnected) => Err(format!("{}: parser panicked", path.display())),
    }
}

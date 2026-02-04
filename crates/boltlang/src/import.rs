
use std::collections::HashSet;

use rowan::SyntaxNode;

use crate::{Db, File, Node, SourceFile, parse_file};

/// Returns all import paths in a given file.
#[salsa::tracked]
pub fn import_paths(db: &dyn Db, file: File) -> Vec<File> {
    let root_green_node = parse_file(db, file);
    let syntax = SyntaxNode::new_root(root_green_node.node(db).clone());
    let mut out = Vec::new();
    if let Some(sf) = SourceFile::cast(syntax) {
        for element in sf.elements() {
            // if let ImportDecl(decl) = element {
            //     todo!()
            // }
        }
    }
    out
}

/// Calulates all files that might be imported by importing the given file, recursively.
#[salsa::tracked]
pub fn transitive_imports(db: &dyn Db, file: File) -> Vec<File> {

    let mut visited = HashSet::new();

    // The file itself should be output too.
    visited.insert(file);

    fn visit(db: &dyn Db, file: File, visited: &mut HashSet<File>) {
        if visited.contains(&file) {
            return;
        }
        visited.insert(file.clone());
        for file_2 in import_paths(db, file) {
            visit(db, file_2, visited);
        }
    }

    visit(db, file, &mut visited);

    visited.into_iter().collect()
}


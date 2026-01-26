
use fluent_uri::Uri;

use crate::{error::{Error, Result}, parser::parse_file, BorrowedUri, OwnedUri, RootDatabase};

/// Returns all import paths in a given file.
pub fn import_paths(db: &RootDatabase, uri: &BorrowedUri) -> Result<Vec<OwnedUri>> {
    let file = db.load(uri)?.ok_or_else(|| Error::FileNotFound(uri.to_owned()))?;
    let root_node = parse_file(db, file);

    Ok(vec![]) // FIXME
}

/// Calulates all files that might be imported by importing the given file, recursively.
pub fn transitive_imports(db: &RootDatabase, uri: &BorrowedUri) -> Result<Vec<OwnedUri>> {
    // TODO detect cycles to avoid infinite recursion
    let out = vec![ uri.to_owned() ];
    // TODO
    Ok(out)
}


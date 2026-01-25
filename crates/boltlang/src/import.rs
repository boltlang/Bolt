use std::path::{Path, PathBuf};

use crate::{error::Result, parser::parse_file, RootDatabase};

/// Returns all import paths in a given file.
pub fn import_paths(db: &RootDatabase, path: &Path) -> Result<Vec<PathBuf>> {
    let file = db.input(path.to_path_buf())?;
    let root_node = parse_file(db, file);

    Ok(vec![]) // FIXME
}

/// Calulates all files that might be imported by importing the given file, recursively.
pub fn transitive_import_paths(db: &RootDatabase, path: &Path) -> Result<Vec<PathBuf>> {
    // TODO detect cycles to avoid infinite recursion
    let out = vec![ path.to_path_buf() ];
    // TODO
    Ok(out)
}


use crate::{Db, File};

/// The text of `file`, or nothing if it can't be read.
#[salsa::tracked]
pub fn source_text(db: &dyn Db, file: File) -> String {
    file.read_to_string(db).unwrap_or_default()
}

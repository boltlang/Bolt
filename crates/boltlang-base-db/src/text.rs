use crate::{Db, File, FilePath};

#[salsa::tracked]
pub fn source_text(db: &dyn Db, file: File) -> String {
    let path = file.path(db);
    let mut read_error = None;
    match path {
        FilePath::System(system) => {
            // Add a dependency on the revision to ensure the operation gets re-executed when the file changes.
            let _ = file.revision(db);

            db
                .system()
                .read_to_string(system)
                .unwrap_or_else(|err| {
                    read_error = Some(err);
                    String::new()
                })
        }
            
        FilePath::SystemVirtual(system_virtual) => {
            // Add a dependency on the revision to ensure the operation gets re-executed when the file changes.
            let _ = file.revision(db);

            db
                .system()
                .read_virtual_path_to_string(system_virtual)
                .unwrap_or_else(|err| {
                    read_error = Some(err);
                    String::new()
                })
        }
    }
}

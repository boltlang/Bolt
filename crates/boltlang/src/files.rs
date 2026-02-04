use std::{sync::Arc};

use salsa::{Durability, Setter};

use crate::{
    Db, FxDashMap, system::{
        SystemPath, SystemPathBuf, SystemVirtualPath, SystemVirtualPathBuf
    }, vfs::FileRevision
};

use self::private::FileStatus;

#[derive(Default, Clone)]
pub struct Files {
    inner: Arc<FilesInner>,
}

#[derive(Default)]
struct FilesInner {
    system_by_path: FxDashMap<SystemPathBuf, File>,
    virtual_system_by_path: FxDashMap<SystemPathBuf, File>,
}

impl Files {

    pub fn new() -> Self {
        Self {
            inner: Arc::new(FilesInner {
                system_by_path: FxDashMap::default(),
                virtual_system_by_path: FxDashMap::default(),
            })
        }
    }

    pub fn try_resolve(&self, db: &dyn Db, path: &SystemPath) -> Option<File> {
        let absolute: crate::PathBuf = SystemPath::absolute(path, db.system().current_directory().unwrap());
        self.inner.system_by_path.get(&absolute).map(|x| x.clone())
    }

    /// Looks up a file by its `path`.
    ///
    /// For a non-existing file, creates a new salsa [`File`] ingredient and stores it for future lookups.
    ///
    /// The operation always succeeds even if the path doesn't exist on disk, isn't accessible or if the path points to a directory.
    /// In these cases, a file with status [`FileStatus::NotFound`] is returned.
    /// 
    /// # Panics
    /// 
    /// Panics if the current working directory could not be retreived, which should never happen in practical scenarios.
    pub fn resolve(&self, db: &dyn Db, path: &SystemPath) -> File {
        let absolute = SystemPath::absolute(path, db.system().current_directory().unwrap());
        *self
            .inner
            .system_by_path
            .entry(absolute.clone())
            .or_insert_with(|| {
                let metadata = db.system().path_metadata(path);
                let durability = Durability::default();
                let builder = File::builder(FilePath::System(absolute))
                    .path_durability(Durability::HIGH);
                let builder  = match metadata {
                    Ok(metadata) if metadata.file_type().is_file() => builder
                        .revision(metadata.revision()),
                    Ok(metadata) if metadata.file_type().is_directory() => builder
                        .status(FileStatus::IsADirectory),
                    _ => builder
                        .status(FileStatus::NotFound)
                        .status_durability(Durability::MEDIUM.max(durability))
                };
                builder.new(db)
            })
    }

    /// Looks up a file by its virtual `path`.
    ///
    /// For a non-existing file, creates a new salsa [`File`] ingredient and stores it for future lookups.
    ///
    /// The operation always succeeds even if the path doesn't exist on disk, isn't accessible or if the path points to a directory.
    /// In these cases, a file with status [`FileStatus::NotFound`] is returned.
    /// 
    /// # Panics
    /// 
    /// Panics if the current working directory could not be retreived, which should never happen in practical scenarios.
    pub fn resolve_virtual(&self, db: &dyn Db, path: &SystemVirtualPath) -> File {
        let absolute = SystemPath::absolute(path, db.system().current_directory().unwrap());
        *self
            .inner
            .virtual_system_by_path
            .entry(absolute.clone())
            .or_insert_with(|| {
                let metadata = db.system().virtual_path_metadata(path);
                let durability = Durability::default();
                let builder = File::builder(FilePath::System(absolute))
                    .path_durability(Durability::HIGH);
                let builder  = match metadata {
                    Ok(metadata) if metadata.file_type().is_file() => builder
                        .revision(metadata.revision()),
                    Ok(metadata) if metadata.file_type().is_directory() => builder
                        .status(FileStatus::IsADirectory),
                    _ => builder
                        .status(FileStatus::NotFound)
                        .status_durability(Durability::MEDIUM.max(durability))
                };
                builder.new(db)
            })
    }


}

/// A file that's either stored on the host system's file system or in the vendored file system.
///
/// # Ordering
///
/// Ordering is based on the file's salsa-assigned id and not on its values.
/// The id may change between runs.
#[salsa::input]
#[derive(Debug, PartialOrd, Ord)]
pub struct File {

    /// The path of the file (immutable).
    #[returns(ref)]
    pub path: FilePath,

    /// The unix permissions of the file. Only supported on unix systems. Always `None` on Windows
    /// or when the file has been deleted.
    #[default]
    pub permissions: Option<u32>,
    
    /// The file revision. A file has changed if the revisions don't compare equal.
    #[default]
    pub revision: FileRevision,

    /// The status of the file.
    ///
    /// Salsa doesn't support deleting inputs. The only way to signal dependent queries that
    /// the file has been deleted is to change the status to `Deleted`.
    #[default]
    pub status: FileStatus,

}

impl File {

    pub fn read_to_string(&self, db: &dyn Db) -> std::io::Result<String> {
        let path = self.path(db);

        match path {
            FilePath::System(system) => {
                // Add a dependency on the revision to ensure the operation gets re-executed when the file changes.
                let _ = self.revision(db);

                db.system().read_to_string(system)
            }
            // FilePath::Vendored(vendored) => db.vendored().read_to_string(vendored),
            FilePath::SystemVirtual(system_virtual) => {
                // Add a dependency on the revision to ensure the operation gets re-executed when the file changes.
                let _ = self.revision(db);

                db.system().read_virtual_path_to_string(system_virtual)
            }
        }
    }

    /// Syncs the [`File`]'s state with the state of the file on the system.
    pub fn sync(self, db: &mut dyn Db) {
        let path = self.path(db).clone();

        match path {
            FilePath::System(system) => {
                // Files::touch_root(db, &system);
                Self::sync_system_path(db, &system, Some(self));
            }
            // FilePath::Vendored(_) => {
            //     // Readonly, can never be out of date.
            // }
            FilePath::SystemVirtual(_) => {
                tracing::debug!("Updating the revision of `{:?}`", self.path(db));
                let current_revision = self.revision(db);
                self.set_revision(db)
                    .to(FileRevision::new(current_revision.as_u128() + 1));
            }
        }
    }

    /// Private method providing the implementation for [`Self::sync_path`] and [`Self::sync`] for
    /// system paths.
    /// 
    /// Takes in the resolved file handle if any. Otherwise resolves the file using the system.
    fn sync_system_path(db: &mut dyn Db, path: &SystemPath, file: Option<File>) {
        let Some(file) = file.or_else(|| db.files().try_resolve(db, path)) else {
            return;
        };

        let (status, revision, permission) = match db.system().path_metadata(path) {
            Ok(metadata) if metadata.file_type().is_file() => (
                FileStatus::Exists,
                metadata.revision(),
                metadata.permissions(),
            ),
            Ok(metadata) if metadata.file_type().is_directory() => {
                (FileStatus::IsADirectory, FileRevision::zero(), None)
            }
            _ => (FileStatus::NotFound, FileRevision::zero(), None),
        };

        if file.status(db) != status {
            tracing::debug!("Updating the status of `{:?}`", file.path(db));
            file.set_status(db).to(status);
        }

        if file.revision(db) != revision {
            tracing::debug!("Updating the revision of `{:?}`", file.path(db));
            file.set_revision(db).to(revision);
        }

        if file.permissions(db) != permission {
            tracing::debug!("Updating the permissions of `{:?}`", file.path(db));
            file.set_permissions(db).to(permission);
        }

    }

}

// The types in here need to be public because they're salsa ingredients but we
// don't want them to be publicly accessible. That's why we put them into a private module.
mod private {
    #[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
    pub enum FileStatus {
        /// The file exists.
        #[default]
        Exists,

        /// The path isn't a file and instead points to a directory.
        IsADirectory,

        /// The path doesn't exist, isn't accessible, or no longer exists.
        NotFound,
    }
}

/// Path to a file.
///
/// The path abstracts that files in Ruff can come from different sources:
///
/// * a file stored on the [host system](crate::system::System).
/// * a virtual file stored on the [host system](crate::system::System).
/// * a vendored file stored in the [vendored file system](crate::vendored::VendoredFileSystem).
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum FilePath {
    /// Path to a file on the [host system](crate::system::System).
    System(SystemPathBuf),
    /// Path to a virtual file on the [host system](crate::system::System).
    SystemVirtual(SystemVirtualPathBuf),
    // /// Path to a file vendored as part of Ruff. Stored in the [vendored file system](crate::vendored::VendoredFileSystem).
    // Vendored(VendoredPathBuf),
}


impl FilePath {

    pub fn as_system_path_buf(&self) -> Option<&SystemPathBuf> {
        match self {
            FilePath::System(pb) => Some(pb),
            _ => None,
        }
    }

    pub fn to_system_virtual_path_buf(&self) -> Option<&SystemVirtualPathBuf> {
        match self {
            FilePath::SystemVirtual(pb) => Some(pb),
            _ => None,
        }
    }

}

impl From<SystemPathBuf> for FilePath {
    fn from(value: SystemPathBuf) -> Self {
        FilePath::System(value)
    }
}

impl From<SystemVirtualPathBuf> for FilePath {
    fn from(value: SystemVirtualPathBuf) -> Self {
        FilePath::SystemVirtual(value)
    }
}
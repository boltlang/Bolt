use std::{os::unix::fs::PermissionsExt, sync::Arc};

use filetime::FileTime;

use crate::vfs::FileType;
use crate::{system::{common::Result, System, SystemPath, SystemPathBuf, SystemVirtualPath}, vfs::Metadata};

fn file_not_found() -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::NotFound, "file not found")
}


#[derive(Clone)]
pub struct OsSystem {
    inner: Arc<OsSystemInner>,
}

struct OsSystemInner {
    cwd: SystemPathBuf,
}

fn other_error<E: std::fmt::Display>(error: E) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Other, format!("{}", error))
}

impl OsSystem {

    pub fn new(cwd: impl AsRef<SystemPath>) -> Self {
        let cwd = cwd.as_ref();
        assert!(cwd.is_absolute());
        Self {
            inner: Arc::new(OsSystemInner {
                cwd: cwd.to_system_path_buf(),
            })
        }
    }

}

impl System for OsSystem {

    fn read_to_string(&self, path: &SystemPath) -> Result<String> {
        std::fs::read_to_string(path.as_std_path())
    }

    fn read_virtual_path_to_string(&self, _path: &SystemVirtualPath) -> Result<String> {
        Err(file_not_found())
    }

    fn current_directory(&self) -> Result<SystemPathBuf> {
        Ok(self.inner.cwd.clone())
    }

    fn canonicalize_path(&self, path: &SystemPath) -> Result<SystemPathBuf> {
        std::fs::canonicalize(path.as_std_path()).map(SystemPathBuf::from_std_path_buf)
    }

    fn path_metadata(&self, path: &SystemPath) -> Result<Metadata> {

        let metadata = std::fs::metadata(path.as_std_path())?;

        let file_type = if metadata.file_type().is_dir() {
            FileType::Directory
        } else if metadata.file_type().is_file() {
            FileType::File
        } else if metadata.file_type().is_symlink() {
            FileType::Symlink
        } else {
            return Err(other_error("path {} points to an unsupported file type"));
        };

        Ok(Metadata::new(
            FileTime::from_last_modification_time(&metadata).into(),
            Some(metadata.permissions().mode()),
            file_type,
        ))
    }

    fn virtual_path_metadata(&self, _path: &SystemVirtualPath) -> Result<Metadata> {
        Err(file_not_found())
    }

}

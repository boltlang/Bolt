//! The file system of the operating system. There's none in the browser,
//! so this isn't built for `wasm32`.

use std::sync::Arc;

use filetime::FileTime;

use crate::vfs::FileType;
use crate::{
    system::{
        System, SystemPath, SystemPathBuf,
        common::{Result, other_error},
    },
    vfs::Metadata,
};

#[derive(Clone)]
pub struct OsSystem {
    inner: Arc<OsSystemInner>,
}

struct OsSystemInner {
    cwd: SystemPathBuf,
}

impl OsSystem {
    pub fn new(cwd: impl AsRef<SystemPath>) -> Self {
        let cwd = cwd.as_ref();
        assert!(cwd.is_absolute());
        Self { inner: Arc::new(OsSystemInner { cwd: cwd.to_system_path_buf() }) }
    }
}

impl System for OsSystem {
    fn read_to_string(&self, path: &SystemPath) -> Result<String> {
        std::fs::read_to_string(path.as_std_path())
    }

    fn current_directory(&self) -> Result<SystemPathBuf> {
        Ok(self.inner.cwd.clone())
    }

    fn canonicalize_path(&self, path: &SystemPath) -> Result<SystemPathBuf> {
        std::fs::canonicalize(path.as_std_path()).map(SystemPathBuf::from_std_path_buf)
    }

    fn read_directory(&self, path: &SystemPath) -> Result<Vec<SystemPathBuf>> {
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(path.as_std_path())? {
            entries.push(SystemPathBuf::from_std_path_buf(entry?.path()));
        }
        entries.sort();
        Ok(entries)
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
            return Err(other_error(format!(
                "path {} points to an unsupported file type",
                path.display()
            )));
        };

        #[cfg(unix)]
        let permissions = {
            use std::os::unix::fs::PermissionsExt;
            Some(metadata.permissions().mode())
        };
        #[cfg(not(unix))]
        let permissions = None;
        Ok(Metadata::new(
            FileTime::from_last_modification_time(&metadata).into(),
            permissions,
            file_type,
        ))
    }
}

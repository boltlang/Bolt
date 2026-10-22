
use std::collections::hash_map::Entry as HashMapEntry;
use std::sync::{Arc, RwLock};

use filetime::FileTime;
use rustc_hash::FxHashMap;

use crate::system::SystemVirtualPath;

use super::{FileType, Path, PathBuf, Metadata};

#[derive(Clone)]
pub struct MemoryFs {
    inner: Arc<MemoryFsInner>,
}

pub struct MemoryFsInner {
    by_path: RwLock<FxHashMap<PathBuf, Entry>>,
    virtual_by_path: RwLock<FxHashMap<PathBuf, Entry>>,
    cwd: PathBuf,
}

impl MemoryFs {

    pub fn new() -> Self {
        Self::with_cwd("/")
    }

    pub fn with_cwd(cwd: impl Into<PathBuf>) -> Self {
        let cwd = cwd.into();
        Self {
            inner: Arc::new(MemoryFsInner {
                by_path: RwLock::new(FxHashMap::default()),
                virtual_by_path: RwLock::new(FxHashMap::default()),
                cwd,
            })
        }
    }

}

fn file_not_found() -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::NotFound, "file not found")
}

fn is_a_dir() -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::IsADirectory, "file is a directory")
}

fn other_error<E: std::fmt::Display>(error: E) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Other, format!("{}", error))
}

impl MemoryFs {

    /// Permission used by all files and directories
    const PERMISSION: u32 = 0o755;

    pub fn read_to_string(&self, path: &super::Path) -> std::io::Result<String> {
        let path = self.canonicalize(path)?;
        let lock = self.inner.by_path.read().map_err(other_error)?;
        let entry = lock.get(&path).ok_or_else(file_not_found)?;
        match entry {
            Entry::Directory(_) => return Err(is_a_dir()),
            Entry::File(entry) => String::from_utf8(entry.content.to_vec()).map_err(other_error),
        }
    }

    pub fn canonicalize(&self, path: impl AsRef<Path>) -> std::io::Result<PathBuf> {
        let path = path.as_ref();

        // Emulate returning ENOENT
        let lock = self.inner.by_path.read().map_err(other_error)?;
        let _ = lock.get(path).ok_or_else(file_not_found)?;

        Ok(self.normalize_path(path))
    }

    /// # Panics
    /// 
    /// Panics if the current working directory variable could not be locked. In
    /// classic scenarios, this should never happen.
    fn normalize_path(&self, path: impl AsRef<Path>) -> PathBuf {
        let normalized = Path::absolute(path, &self.inner.cwd);
        normalized.to_system_path_buf()
    }

    pub fn current_directory(&self) -> std::io::Result<PathBuf> {
        Ok(self.inner.cwd.clone())
    }

    pub fn metadata(&self, path: &Path) -> std::io::Result<Metadata> {
        let path = self.canonicalize(path)?;
        let lock = self.inner.by_path.read().map_err(other_error)?;
        let entry = lock.get(&path).ok_or_else(file_not_found)?;
        Ok(match entry {
            Entry::Directory(entry) => Metadata::new(
                entry.last_modified.into(),
                Some(Self::PERMISSION),
                FileType::Directory
            ),
            Entry::File(entry) => Metadata::new(
                entry.last_modified.into(),
                Some(Self::PERMISSION),
                FileType::File,
            ),
        })
    }

    pub fn write_file_bytes(&self, path: &Path, content: impl AsRef<[u8]>) -> std::io::Result<()> {
        let content = content.as_ref();
        let path = self.canonicalize(path)?;
        let mut lock = self.inner.by_path.write().map_err(other_error)?;
        match lock.entry(path) {
            HashMapEntry::Occupied(mut entry) => match entry.get_mut() {
                Entry::File(file) => { file.content = content.into(); },
                Entry::Directory(_) => return Err(is_a_dir()),
            }
            HashMapEntry::Vacant(entry) => {
                entry.insert(Entry::File(File {
                    last_modified: FileTime::now(),
                    content: content.into(),
                }));
            }
        }
        Ok(())
    }

    pub fn write_file_bytes_virtual(&self, path: &SystemVirtualPath, content: impl AsRef<[u8]>) -> std::io::Result<()> {
        let content = content.as_ref();
        let path = self.canonicalize(path)?;
        let mut lock = self.inner.virtual_by_path.write().map_err(other_error)?;
        match lock.entry(path) {
            HashMapEntry::Occupied(mut entry) => match entry.get_mut() {
                Entry::File(file) => { file.content = content.into(); },
                Entry::Directory(_) => return Err(is_a_dir()),
            }
            HashMapEntry::Vacant(entry) => {
                entry.insert(Entry::File(File {
                    last_modified: FileTime::now(),
                    content: content.into(),
                }));
            }
        }
        Ok(())
    }

}

enum Entry {
    File(File),
    Directory(Directory),
}

struct File {
    last_modified: FileTime,
    content: Box<[u8]>,
}

struct Directory {
    last_modified: FileTime,
}

use std::collections::hash_map::Entry as HashMapEntry;
use std::sync::{Arc, RwLock};

use filetime::FileTime;
use rustc_hash::FxHashMap;

use crate::system::{SystemVirtualPath, file_time_now};

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

impl Default for MemoryFs {
    fn default() -> Self {
        Self::new()
    }
}

/// The time of a write to a file last written at `previous`: now, or just
/// after `previous` if the clock didn't move, as it may not in the browser
/// within a millisecond. The time is the revision of the file, so it must
/// change for the change to be seen.
fn later_than(previous: FileTime) -> FileTime {
    let now = file_time_now();
    if now > previous {
        return now;
    }
    match previous.nanoseconds() {
        999_999_999 => FileTime::from_unix_time(previous.unix_seconds() + 1, 0),
        nanos => FileTime::from_unix_time(previous.unix_seconds(), nanos + 1),
    }
}

fn file_not_found() -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::NotFound, "file not found")
}

fn is_a_dir() -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::IsADirectory, "file is a directory")
}

fn other_error<E: std::fmt::Display>(error: E) -> std::io::Error {
    std::io::Error::other(format!("{}", error))
}

impl MemoryFs {

    /// Permission used by all files and directories
    const PERMISSION: u32 = 0o755;

    pub fn read_to_string(&self, path: &super::Path) -> std::io::Result<String> {
        let path = self.canonicalize(path)?;
        let lock = self.inner.by_path.read().map_err(other_error)?;
        let entry = lock.get(&path).ok_or_else(file_not_found)?;
        match entry {
            Entry::Directory(_) => Err(is_a_dir()),
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
        // Not `canonicalize`, which fails if the file doesn't exist yet.
        let path = self.normalize_path(path);
        let mut lock = self.inner.by_path.write().map_err(other_error)?;
        // Like on disk, the directories of a file exist.
        for dir in path.as_std_path().ancestors().skip(1) {
            lock.entry(PathBuf::from_std_path_buf(dir.to_path_buf()))
                .or_insert_with(|| Entry::Directory(Directory { last_modified: file_time_now() }));
        }
        match lock.entry(path) {
            HashMapEntry::Occupied(mut entry) => match entry.get_mut() {
                Entry::File(file) => {
                    file.content = content.into();
                    file.last_modified = later_than(file.last_modified);
                },
                Entry::Directory(_) => return Err(is_a_dir()),
            }
            HashMapEntry::Vacant(entry) => {
                entry.insert(Entry::File(File {
                    last_modified: file_time_now(),
                    content: content.into(),
                }));
            }
        }
        Ok(())
    }

    /// Remove the file at `path`. Its directories stay.
    pub fn remove_file(&self, path: &Path) -> std::io::Result<()> {
        let path = self.normalize_path(path);
        let mut lock = self.inner.by_path.write().map_err(other_error)?;
        match lock.get(&path) {
            Some(Entry::File(_)) => {
                lock.remove(&path);
                Ok(())
            }
            Some(Entry::Directory(_)) => Err(is_a_dir()),
            None => Err(file_not_found()),
        }
    }

    pub fn write_file_bytes_virtual(&self, path: &SystemVirtualPath, content: impl AsRef<[u8]>) -> std::io::Result<()> {
        let content = content.as_ref();
        // Not `canonicalize`, which fails if the file doesn't exist yet.
        let path = self.normalize_path(path);
        let mut lock = self.inner.virtual_by_path.write().map_err(other_error)?;
        match lock.entry(path) {
            HashMapEntry::Occupied(mut entry) => match entry.get_mut() {
                Entry::File(file) => {
                    file.content = content.into();
                    file.last_modified = later_than(file.last_modified);
                },
                Entry::Directory(_) => return Err(is_a_dir()),
            }
            HashMapEntry::Vacant(entry) => {
                entry.insert(Entry::File(File {
                    last_modified: file_time_now(),
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
#[cfg(test)]
mod tests {
    use super::MemoryFs;
    use crate::SystemPath;

    #[test]
    fn writes_new_files_and_overwrites_them() {
        let fs = MemoryFs::new();
        let path = SystemPath::new("/a.bolt");
        fs.write_file_bytes(path, b"one").unwrap();
        assert_eq!(fs.read_to_string(path).unwrap(), "one");
        let before = fs.metadata(path).unwrap().revision();
        // Right after: the revision changes even if the clock didn't.
        fs.write_file_bytes(path, b"two").unwrap();
        assert_eq!(fs.read_to_string(path).unwrap(), "two");
        assert_ne!(fs.metadata(path).unwrap().revision(), before);
    }

    #[test]
    fn the_directories_of_files_exist() {
        let fs = MemoryFs::new();
        fs.write_file_bytes(SystemPath::new("/a/b/c.bolt"), b"").unwrap();
        for dir in ["/a/b", "/a", "/"] {
            assert!(fs.metadata(SystemPath::new(dir)).unwrap().file_type().is_directory(), "{dir}");
        }
        assert!(fs.read_to_string(SystemPath::new("/a")).is_err());
    }

    #[test]
    fn removes_files() {
        let fs = MemoryFs::new();
        let path = SystemPath::new("/dir/a.bolt");
        fs.write_file_bytes(path, b"one").unwrap();
        fs.remove_file(path).unwrap();
        assert!(fs.read_to_string(path).is_err());
        assert!(fs.remove_file(path).is_err());
        // Its directory stays, and isn't a file to remove.
        assert!(fs.metadata(SystemPath::new("/dir")).is_ok());
        assert!(fs.remove_file(SystemPath::new("/dir")).is_err());
    }
}

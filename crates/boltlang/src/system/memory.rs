
use crate::{
    System,
    SystemPath,
    WritableSystem,
    system::{common::Result, SystemPathBuf, SystemVirtualPath},
    vfs::{MemoryFs, Metadata}
};

fn file_not_found() -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::NotFound, "file not found")
}

#[derive(Clone)]
pub struct InMemorySystem {
    fs: MemoryFs,
}

impl InMemorySystem {

    pub fn new() -> Self {
        Self::with_cwd("/")
    }

    pub fn with_cwd(cwd: impl Into<SystemPathBuf>) -> Self {
        Self {
            fs: MemoryFs::with_cwd(cwd),
        }
    }

}


impl System for InMemorySystem {

    fn canonicalize_path(&self, path: &SystemPath) -> Result<SystemPathBuf> {
        self.fs.canonicalize(path)
    }

    fn current_directory(&self) -> Result<SystemPathBuf> {
        self.fs.current_directory()
    }

    fn path_metadata(&self, path: &SystemPath) -> Result<Metadata> {
        self.fs.metadata(path)
    }

    fn virtual_path_metadata(&self, _path: &SystemVirtualPath) -> Result<Metadata> {
        Err(file_not_found())
    }

    fn read_to_string(&self, path: &SystemPath) -> Result<String> {
        self.fs.read_to_string(path)
    }

    fn read_virtual_path_to_string(&self, _path: &SystemVirtualPath) -> Result<String> {
        Err(file_not_found())
    }

}

impl WritableSystem for InMemorySystem {

    fn write_file_bytes(&self, path: &SystemPath, content: &[u8]) -> Result<()> {
        self.fs.write_file_bytes(path, content)
    }

    fn write_file_bytes_virtual(&self, path: &SystemVirtualPath, content: &[u8]) -> Result<()> {
        self.fs.write_file_bytes_virtual(path, content)
    }

}
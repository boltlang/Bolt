use std::sync::Arc;

use crate::vfs::Metadata;
use crate::system::{InMemorySystem, SystemPathBuf, common::{Result, System, WritableSystem}, path::{SystemPath, SystemVirtualPath}};

#[allow(unused)]
#[derive(Clone)]
pub struct TestSystem {
    inner: Arc<dyn WritableSystem>,
}

impl TestSystem {

    pub fn new(inner: impl WritableSystem + 'static) -> Self {
        Self { inner: Arc::new(inner) }
    }

}

impl Default for TestSystem {
    fn default() -> Self {
        Self::new(InMemorySystem::new())
    }
}

impl System for TestSystem {

    fn read_to_string(&self, path: &SystemPath) -> Result<String> {
        self.inner.read_to_string(path)
    }

    fn read_virtual_path_to_string(&self, path: &SystemVirtualPath) -> Result<String> {
        self.inner.read_virtual_path_to_string(path)
    }

    fn canonicalize_path(&self, path: &SystemPath) -> Result<SystemPathBuf> {
        self.inner.canonicalize_path(path)
    }

    fn current_directory(&self) -> Result<SystemPathBuf> {
        self.inner.current_directory()
    }

    fn path_metadata(&self, path: &SystemPath) -> Result<Metadata> {
        self.inner.path_metadata(path)
    }

    fn virtual_path_metadata(&self, path: &SystemVirtualPath) -> Result<Metadata> {
        self.inner.virtual_path_metadata(path)
    }

}

impl WritableSystem for TestSystem {

    fn write_file_bytes(&self, path: &SystemPath, contents: &[u8]) -> Result<()> {
        self.inner.write_file_bytes(path, contents)
    }

    fn write_file_bytes_virtual(&self, path: &SystemVirtualPath, contents: &[u8]) -> Result<()> {
        self.inner.write_file_bytes_virtual(path, contents)
    }

}

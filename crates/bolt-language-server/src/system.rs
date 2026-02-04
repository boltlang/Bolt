use boltlang::{System, SystemPath};


#[derive(Clone)]
pub struct LspSystem {
    fs: MemoryFs,
}

impl System for LspSystem {

    fn path_metadata(&self, path: &SystemPath) -> Result<Metadata> {
        self.fs.metadata(path)
    }

    fn canonicalize_path(&self, path: &SystemPath) -> Result<SystemPathBuf> {
        self.fs.canonicalize_path(path)
    }

    fn current_directory(&self) -> Result<SystemPathBuf> {
        self.fs.current_directory()
    }

    fn read_to_string(&self, path: &SystemPath) -> Result<String> {
        self.fs.read_to_string(path)
    }

}

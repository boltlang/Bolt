//! The files the server sees: those of another system, like the disk, with
//! the text of the documents open in the editor in their place, since that
//! text isn't saved yet.

use std::{
    collections::HashMap,
    io::Result,
    sync::{
        Arc, RwLock,
        atomic::{AtomicU64, Ordering},
    },
};

use boltlang_base_db::{FileRevision, FileType, Metadata, System, SystemPath, SystemPathBuf, SystemVirtualPath};

/// The text of an open document, and a revision that changes with each
/// edit, so that Salsa notices it.
struct Document {
    text: String,
    revision: u64,
}

#[derive(Clone)]
pub struct LspSystem {
    /// Where the files that aren't open come from: the disk, or in the
    /// browser, the files the page gives the server.
    disk: Arc<dyn System>,
    open: Arc<RwLock<HashMap<SystemPathBuf, Document>>>,
    revisions: Arc<AtomicU64>,
}

impl LspSystem {
    pub fn new(disk: Arc<dyn System>) -> Self {
        LspSystem { disk, open: Arc::default(), revisions: Arc::new(AtomicU64::new(1)) }
    }

    /// Use `text` for the file at `path`, instead of what's on disk.
    pub fn set_text(&self, path: &SystemPath, text: String) {
        let revision = self.revisions.fetch_add(1, Ordering::Relaxed);
        self.open.write().unwrap().insert(path.to_system_path_buf(), Document { text, revision });
    }

    /// The paths of the open documents, in order.
    pub fn open_paths(&self) -> Vec<SystemPathBuf> {
        let mut paths: Vec<SystemPathBuf> = self.open.read().unwrap().keys().cloned().collect();
        paths.sort();
        paths
    }

    /// Go back to what's on disk for the file at `path`.
    pub fn remove_text(&self, path: &SystemPath) {
        self.open.write().unwrap().remove(path);
    }
}

impl System for LspSystem {
    fn read_to_string(&self, path: &SystemPath) -> Result<String> {
        match self.open.read().unwrap().get(path) {
            Some(document) => Ok(document.text.clone()),
            None => self.disk.read_to_string(path),
        }
    }

    fn read_virtual_path_to_string(&self, path: &SystemVirtualPath) -> Result<String> {
        self.disk.read_virtual_path_to_string(path)
    }

    fn path_metadata(&self, path: &SystemPath) -> Result<Metadata> {
        match self.open.read().unwrap().get(path) {
            Some(document) => {
                Ok(Metadata::new(FileRevision::new(document.revision.into()), None, FileType::File))
            }
            None => self.disk.path_metadata(path),
        }
    }

    fn virtual_path_metadata(&self, path: &SystemVirtualPath) -> Result<Metadata> {
        self.disk.virtual_path_metadata(path)
    }

    fn current_directory(&self) -> Result<SystemPathBuf> {
        self.disk.current_directory()
    }

    fn canonicalize_path(&self, path: &SystemPath) -> Result<SystemPathBuf> {
        self.disk.canonicalize_path(path)
    }

    fn read_directory(&self, path: &SystemPath) -> Result<Vec<SystemPathBuf>> {
        self.disk.read_directory(path)
    }
}

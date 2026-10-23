use std::sync::Arc;

use boltlang_base_db::{Db, Files, System, salsa};

use crate::system::LspSystem;

#[salsa::db]
#[derive(Clone)]
pub struct LspDatabase {
    storage: salsa::Storage<Self>,
    files: Files,
    pub system: LspSystem,
}

impl LspDatabase {
    pub fn new(disk: Arc<dyn System>) -> Self {
        Self { storage: salsa::Storage::new(None), files: Files::new(), system: LspSystem::new(disk) }
    }
}

impl salsa::Database for LspDatabase {}

#[salsa::db]
impl Db for LspDatabase {
    fn files(&self) -> &Files {
        &self.files
    }
    fn system(&self) -> &dyn System {
        &self.system
    }
}

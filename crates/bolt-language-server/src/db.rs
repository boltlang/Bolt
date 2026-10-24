use std::sync::Arc;

use boltlang_base_db::{DEFAULT_PRELUDE, Db, Files, Settings, System, salsa};

use crate::system::LspSystem;

#[salsa::db]
#[derive(Clone)]
pub struct LspDatabase {
    storage: salsa::Storage<Self>,
    files: Files,
    pub system: LspSystem,
}

impl LspDatabase {
    /// A database with the default prelude, until the client says
    /// otherwise.
    pub fn new(disk: Arc<dyn System>) -> Self {
        let db = Self { storage: salsa::Storage::new(None), files: Files::new(), system: LspSystem::new(disk) };
        Settings::init(&db, Some(DEFAULT_PRELUDE.to_string()), false);
        db
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

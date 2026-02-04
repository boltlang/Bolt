use boltlang::{Db, Files, InMemorySystem, System, salsa};

#[salsa::db]
#[derive(Clone)]
pub struct LspDatabase {
    storage: salsa::Storage<Self>,
    pub files: Files,
    pub system: InMemorySystem,
}

impl Default for LspDatabase {
    fn default() -> Self {
        Self {
            storage: salsa::Storage::new(None),
            files: Files::new(),
            system: InMemorySystem::new(),
        }
    }
}

impl salsa::Database for LspDatabase {}

#[salsa::db]
impl Db for LspDatabase {
    fn files(&self) ->  &Files {
        &self.files
    }
    fn system(&self) ->  &dyn System {
        &self.system
    }
}
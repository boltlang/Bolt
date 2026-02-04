
use boltlang::{Db, Files, OsSystem, System, SystemPath, salsa};

#[salsa::db]
#[derive(Clone)]
pub struct CliDatabase {
    storage: salsa::Storage<Self>,
    files: Files,
    system: OsSystem,
}

impl CliDatabase {
    pub fn new(cwd: &SystemPath) -> Self {
        Self {
            storage: salsa::Storage::new(None),
            files: Files::new(),
            system: OsSystem::new(cwd),
        }
    }
}

impl salsa::Database for CliDatabase {}

#[salsa::db]
impl Db for CliDatabase {
    fn files(&self) ->  &Files {
        &self.files
    }
    fn system(&self) ->  &dyn System {
        &self.system
    }
}



use boltlang_base_db::{Db, Files, OsSystem, Settings, System, SystemPath, salsa};

#[salsa::db]
#[derive(Clone)]
pub struct CliDatabase {
    storage: salsa::Storage<Self>,
    files: Files,
    system: OsSystem,
}

impl CliDatabase {
    /// A database for the files in `cwd`, with the prelude of `prelude`;
    /// see [`Settings`].
    pub fn new(cwd: &SystemPath, prelude: Option<String>, prelude_required: bool) -> Self {
        let db = Self { storage: salsa::Storage::new(None), files: Files::new(), system: OsSystem::new(cwd) };
        Settings::init(&db, prelude, prelude_required);
        db
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


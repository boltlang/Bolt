
use boltlang_base_db::{Db, Files, OsSystem, Settings, System, SystemPath, SystemPathBuf, salsa};

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
    /// `std` is the one in `std_dir`, which comes with the compiler.
    pub fn new(cwd: &SystemPath, prelude: Option<String>, prelude_required: bool, std_dir: Option<SystemPathBuf>) -> Self {
        let mut db = Self { storage: salsa::Storage::new(None), files: Files::new(), system: OsSystem::new(cwd) };
        Settings::init(&db, prelude, prelude_required);
        Settings::change_std_dir(&mut db, std_dir);
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


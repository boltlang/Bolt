//! A database for tests, with files in memory, shared by the crates that
//! build on this one.

use std::sync::{Arc, Mutex};

use crate::{Db, File, Files, InMemorySystem, Settings, System, SystemPath, WritableSystem};

#[salsa::db]
#[derive(Clone)]
pub struct TestDb {
    storage: salsa::Storage<Self>,
    files: Files,
    system: InMemorySystem,
    /// The queries that ran, while logging is on.
    executed: Arc<Mutex<Option<Vec<String>>>>,
}

impl TestDb {

    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let executed: Arc<Mutex<Option<Vec<String>>>> = Default::default();
        let log = executed.clone();
        let storage = salsa::Storage::new(Some(Box::new(move |event: salsa::Event| {
            if let salsa::EventKind::WillExecute { database_key } = event.kind {
                if let Some(log) = &mut *log.lock().unwrap() {
                    log.push(format!("{database_key:?}"));
                }
            }
        })));
        let db = Self { storage, files: Files::default(), system: InMemorySystem::new(), executed };
        Settings::init(&db, None, false);
        db
    }

    /// Change the prelude, which is off by default. See [`Settings`].
    pub fn set_prelude(&mut self, prelude: Option<&str>, required: bool) {
        Settings::change_prelude(self, prelude.map(str::to_string), required);
    }

    /// Create a file with the given contents.
    pub fn file(&self, path: &str, text: &str) -> File {
        let path = SystemPath::new(path);
        self.system.write_file_bytes(path, text.as_bytes()).unwrap();
        self.files.resolve(self, path)
    }

    /// Change the contents of a file.
    pub fn edit(&mut self, file: File, text: &str) {
        let path = file.path(self).as_system_path_buf().unwrap().clone();
        self.system.write_file_bytes(&path, text.as_bytes()).unwrap();
        file.sync(self);
    }

    /// Run `f` and return the queries that ran during it.
    pub fn log_executed(&mut self, f: impl FnOnce(&mut Self)) -> Vec<String> {
        *self.executed.lock().unwrap() = Some(Vec::new());
        f(self);
        self.executed.lock().unwrap().take().unwrap()
    }

    #[cfg(test)]
    #[allow(unused)]
    pub fn take_logs(&mut self) -> Vec<String> {
        let mut logs = self.executed.lock().unwrap();
        if let Some(logs) = &mut *logs {
            std::mem::take(logs)
        } else {
            vec![]
        }
    }

}

impl salsa::Database for TestDb {}

#[salsa::db]
impl Db for TestDb {
    fn files(&self) -> &Files {
        &self.files
    }
    fn system(&self) -> &dyn System {
        &self.system
    }
}

#[cfg(test)]
mod test {

    use crate::{Db, FilePath, SystemPath, SystemPathBuf, TestDb};

    #[test]
    fn test_get_file() {
        let db = TestDb::new();
        let file = db.files().resolve(&db, SystemPath::new("foo/bar.txt"));
        assert_eq!(file.path(&db), &FilePath::System(SystemPathBuf::from("/foo/bar.txt")));
    }

}

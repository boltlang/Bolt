
use crate::files::Files;
use crate::system::System;

#[salsa::db]
pub trait Db : salsa::Database {
    fn system(&self) -> &dyn System;
    fn files(&self) -> &Files;
}

// impl RootDatabase {

//     /// Create a new Salsa database for Bolt
//     pub fn new() -> Self {

//         #[cfg(test)]
//         let logs = <Arc<Mutex<Option<Vec<String>>>>>::default();

//         Self {
//             storage: salsa::Storage::new(
//                 #[cfg(not(test))]
//                 None,
//                 #[cfg(test)]
//                 Some(Box::new({
//                     let logs = logs.clone();
//                     move |event| {
//                         eprintln!("Event: {event:?}");
//                         // Log interesting events, if logging is enabled
//                         if let Some(logs) = &mut *logs.lock().unwrap() {
//                             // only log interesting events
//                             if let salsa::EventKind::WillExecute { .. } = event.kind {
//                                 logs.push(format!("Event: {event:?}"));
//                             }
//                         }
//                     }
//                 }))
//             ),
//             files: Files::default(),
//             system: OsSystem::default(),
//             #[cfg(test)]
//             logs,
//         }
//     }

//     #[cfg(test)]
//     #[allow(unused)]
//     pub fn take_logs(&self) -> Vec<String> {
//         let mut logs = self.logs.lock().unwrap();
//         if let Some(logs) = &mut *logs {
//             std::mem::take(logs)
//         } else {
//             vec![]
//         }
//     }

//     // pub fn load_transitive(&self, uri: &BorrowedUri) -> Result<()> {
//     //     // FIXME This should go to the language server or CLI
//     //     for uri in transitive_imports(self, uri)? {
//     //         let _ = self.load(uri.as_str());
//     //     }
//     //     Ok(())
//     // }

//     // pub fn load(&self, uri: &BorrowedUri) -> Result<Option<File>> {
//     //     Ok(match self.files.entry(uri.to_owned()) {
//     //         Entry::Occupied(entry) => {
//     //             Some(*entry.get())
//     //         }
//     //         Entry::Vacant(_entry) => {
//     //             None
//     //         }
//     //     })
//     // }

//     // pub fn load(&mut self, uri: BorrowedUri) -> Result<()> {
//     //     let contents = self.fs.read_to_string(uri);
//     //     let uri = canonical_uri(self, &uri)?;

//     //     // We can't use DashMap::entry due to borrow issues
//     //     if self.files.contains_key(&uri) {
//     //         let file = self.files.get(&uri).unwrap().clone();
//     //         file.set_contents(self).to(contents);
//     //     } else {
//     //         let file = File::new(self, uri.to_owned(), contents);
//     //         self.files.insert(uri.clone(), file);
//     //     }
//     //     Ok(())
//     // }

//     // pub fn input(&mut self, uri: OwnedUri, contents: String) -> Result<File> {

//     //     // Ensure that the URI is unique
//     //     let uri = canonical_uri(self, uri.as_str())?;

//     //     let file = match self.files.entry(uri.clone()) {
//     //         Entry::Occupied(entry) => {
//     //             *entry.get()
//     //         }
//     //         Entry::Vacant(entry) => {
//     //             *entry.insert(File::new(self, uri, contents.clone()))
//     //         }
//     //     };

//     //     // Set the contents of the file, overriding any previous contents
//     //     file.set_contents(self).to(contents);

//     //     Ok(file)
//     // }

// }

/// A function or variable name.
///
/// Names are shared between functions and variables, so we only need one of these structs for
/// both.
#[salsa::interned]
pub struct Name<'db> {
    #[returns(ref)]
    pub text: String,
}

// /// Returns the complete path from the workspace root to the file pointed to by the given path.
// ///
// /// This way, a database that stores this path on disk can be moved without any issues.
// pub fn canonical_uri(db: &dyn salsa::Database, uri: &BorrowedUri) -> Result<OwnedUri> {
//     // TODO make this relative to the root workspace
//     Ok(Uri::parse(uri)?.normalize().to_string())
// }

#[cfg(test)]
mod test {

    use std::sync::{Arc, Mutex};

    use crate::{Db, FilePath, Files, InMemorySystem, System, SystemPath, system::SystemPathBuf};

    #[salsa::db]
    #[derive(Clone)]
    pub struct TestDatabase {
        storage: salsa::Storage<Self>,
        files: Files,
        system: InMemorySystem,
        logs: Arc<Mutex<Option<Vec<String>>>>,
    }

    impl TestDatabase {
        pub fn new() -> Self {
            let logs = <Arc<Mutex<Option<Vec<String>>>>>::default();
            Self {
                storage: salsa::Storage::new(
                    Some(Box::new({
                        let logs = logs.clone();
                        move |event| {
                            eprintln!("Event: {event:?}");
                            // Log interesting events, if logging is enabled
                            if let Some(logs) = &mut *logs.lock().unwrap() {
                                // only log interesting events
                                if let salsa::EventKind::WillExecute { .. } = event.kind {
                                    logs.push(format!("Event: {event:?}"));
                                }
                            }
                        }
                    }))
                ),
                files: Files::default(),
                system: InMemorySystem::new(),
                #[cfg(test)]
                logs,
            }
        }
    }

    impl salsa::Database for TestDatabase {}

    #[salsa::db]
    impl Db for TestDatabase {
        fn files(&self) ->  &Files {
            &self.files
        }
        fn system(&self) ->  &dyn System {
            &self.system
        }
    }

    #[test]
    fn test_get_file() {
        let db = TestDatabase::new();
        let file = db.files().resolve(&db, SystemPath::new("foo/bar.txt"));
        assert_eq!(file.path(&db), &FilePath::System(SystemPathBuf::from("/foo/bar.txt")));
    }

}

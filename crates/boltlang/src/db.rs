use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crossbeam_channel::Sender;
use dashmap::{DashMap, Entry};
use fluent_uri::Uri;
use notify_debouncer_mini::DebounceEventResult;
use notify_debouncer_mini::new_debouncer;
use notify_debouncer_mini::notify::RecursiveMode;
use notify_debouncer_mini::{Debouncer, notify::RecommendedWatcher};
use rowan::GreenNode;
use salsa::Setter;

use crate::error::Result;
use crate::import::transitive_imports;
use crate::{BorrowedUri, OwnedUri};

#[salsa::input]
#[derive(Debug)]
pub struct File {
    pub uri: OwnedUri,
    #[returns(ref)]
    pub contents: String,
}

#[salsa::db]
#[derive(Clone)]
pub struct RootDatabase {
    storage: salsa::Storage<Self>,
    files: DashMap<OwnedUri, File>,
    // The logs are only used for testing and demonstrating reuse:
    #[cfg(test)]
    logs: Arc<Mutex<Option<Vec<String>>>>,
}

impl RootDatabase {

    /// Create a new Salsa database for Bolt
    ///
    /// The optional [tx] argument holds the stream that will receive events. It enables or
    /// disables the file watcher.
    pub fn new(tx: Option<Sender<DebounceEventResult>>) -> Self {

        #[cfg(test)]
        let logs = <Arc<Mutex<Option<Vec<String>>>>>::default();

        Self {
            storage: salsa::Storage::new(
                #[cfg(not(test))]
                None,
                #[cfg(test)]
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
            files: DashMap::new(),
            #[cfg(test)]
            logs,
        }
    }

    #[cfg(test)]
    #[allow(unused)]
    pub fn take_logs(&self) -> Vec<String> {
        let mut logs = self.logs.lock().unwrap();
        if let Some(logs) = &mut *logs {
            std::mem::take(logs)
        } else {
            vec![]
        }
    }

    pub fn load_transitive(&self, uri: &BorrowedUri) -> Result<()> {
        // FIXME This should go to the language server or CLI
        for uri in transitive_imports(self, uri)? {
            let _ = self.load(uri.as_str());
        }
        Ok(())
    }

    pub fn load(&self, uri: &BorrowedUri) -> Result<Option<File>> {
        Ok(match self.files.entry(uri.to_owned()) {
            Entry::Occupied(entry) => {
                Some(*entry.get())
            }
            Entry::Vacant(_entry) => {
                None
            }
        })
    }

//     pub fn load(&mut self, uri: BorrowedUri) -> Result<()> {
//         let contents = self.fs.read_to_string(uri);
//         let uri = canonical_uri(self, &uri)?;

//         // We can't use DashMap::entry due to borrow issues
//         if self.files.contains_key(&uri) {
//             let file = self.files.get(&uri).unwrap().clone();
//             file.set_contents(self).to(contents);
//         } else {
//             let file = File::new(self, uri.to_owned(), contents);
//             self.files.insert(uri.clone(), file);
//         }
//         Ok(())
//     }

    pub fn input(&mut self, uri: OwnedUri, contents: String) -> Result<File> {

        // Ensure that the URI is unique
        let uri = canonical_uri(self, uri.as_str())?;

        let file = match self.files.entry(uri.clone()) {
            Entry::Occupied(entry) => {
                *entry.get()
            }
            Entry::Vacant(entry) => {
                *entry.insert(File::new(self, uri, contents.clone()))
            }
        };

        // Set the contents of the file, overriding any previous contents
        file.set_contents(self).to(contents);

        Ok(file)
    }

}

#[salsa::db]
impl salsa::Database for RootDatabase {}

#[salsa::tracked(debug)]
pub struct DbNode<'db> {
    #[tracked]
    #[returns(ref)]
    pub node: GreenNode,
}

/// A function or variable name.
///
/// Names are shared between functions and variables, so we only need one of these structs for
/// both.
#[salsa::interned(debug)]
pub struct Name<'db> {
    #[returns(ref)]
    pub text: String,
}

/// Returns the complete path from the workspace root to the file pointed to by the given path.
///
/// This way, a database that stores this path on disk can be moved without any issues.
pub fn canonical_uri(db: &dyn salsa::Database, uri: &BorrowedUri) -> Result<OwnedUri> {
    // TODO make this relative to the root workspace
    Ok(Uri::parse(uri)?.normalize().to_string())
}


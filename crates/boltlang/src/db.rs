
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use notify_debouncer_mini::new_debouncer;
use crossbeam_channel::Sender;
use dashmap::{DashMap, Entry};
use notify_debouncer_mini::DebounceEventResult;
use notify_debouncer_mini::notify::RecursiveMode;
use notify_debouncer_mini::{Debouncer, notify::RecommendedWatcher};
use rowan::GreenNode;
use salsa::{Setter};

use crate::error::Result;
use crate::import::transitive_import_paths;

#[salsa::input]
pub struct File {
    pub path: PathBuf,
    #[returns(ref)]
    pub contents: String,
}

#[salsa::db]
#[derive(Clone)]
pub struct RootDatabase {
    storage: salsa::Storage<Self>,
    files: DashMap<PathBuf, File>,
    watcher: Option<Arc<Mutex<Debouncer<RecommendedWatcher>>>>,
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
            watcher: tx.map(|tx| Arc::new(Mutex::new(new_debouncer(Duration::from_secs(1), tx).unwrap()))),
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

    pub fn load_transitive(&mut self, path: &Path) -> Result<()> {

        for path in transitive_import_paths(self, path)? {

            let path = canonical_path(self, &path)?;
            let contents = std::fs::read_to_string(&path)?;

            // We can't use DashMap::entry due to borrow issues
            if self.files.contains_key(&path) {
                let file = self.files.get(&path).unwrap().clone();
                file.set_contents(self).to(contents);
            } else {
                let file = File::new(self, path.clone(), contents);
                self.files.insert(path.clone(), file);
            }

        }

        Ok(())
    }

    pub fn input(&self, path: PathBuf) -> std::io::Result<File> {

        // Ensure that the path is unique
        let path = path.canonicalize()?;

        Ok(match self.files.entry(path.clone()) {
            Entry::Occupied(entry) => *entry.get(),
            Entry::Vacant(entry) => {
                if let Some(watcher) = &self.watcher {
                    watcher
                        .lock()
                        .unwrap()
                        .watcher()
                        .watch(&path, RecursiveMode::NonRecursive)
                        .unwrap();
                }
                let contents = std::fs::read_to_string(&path)?;
                *entry.insert(File::new(self, path, contents))
            }
        })
    }

}

#[salsa::db]
impl salsa::Database for RootDatabase {}

#[salsa::tracked(debug)]
pub struct ParsedFile<'db> {
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
pub fn canonical_path(db: &dyn salsa::Database, path: &Path) -> std::io::Result<PathBuf> {
    // TODO make this relative to the root workspace
    path.canonicalize()
}


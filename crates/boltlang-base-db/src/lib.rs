//! The base of the Bolt compiler's Salsa database: files, the systems they
//! come from, their text and their syntax trees. Like rust-analyzer's
//! `base-db`, it sits below the HIR crates so that they can query files.

mod db;
mod error;
mod files;
mod parse;
mod system;
mod text;
mod test_db;
mod vfs;

use std::hash::BuildHasherDefault;

use rustc_hash::FxHasher;

/// Re-export of the Salsa library that Bolt uses
pub use salsa;

/// Re-export of the Rowan library that Bolt uses
pub use boltlang_syntax::rowan;

pub type OwnedUri = String;

pub type BorrowedUri = str;

pub type FxDashMap<K, V> = dashmap::DashMap<K, V, BuildHasherDefault<FxHasher>>;
pub type FxDashSet<K> = dashmap::DashSet<K, BuildHasherDefault<FxHasher>>;

pub use {
    db::Db,
    error::{Error, Result},
    files::{File, FilePath, Files},
    parse::{Parse, parse_file},
    system::{System, SystemPath, SystemPathBuf, SystemVirtualPath, SystemVirtualPathBuf, WritableSystem, OsSystem, InMemorySystem},
    text::{LineIndex, index_lines, source_text},
    vfs::{FileRevision, FileType, MemoryFs, Metadata, Path, PathBuf},
};

pub use crate::system::TestSystem;
pub use crate::test_db::TestDb;

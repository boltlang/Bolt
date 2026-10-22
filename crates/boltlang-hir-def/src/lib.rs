//! Definitions in Bolt programs, modelled after rust-analyzer's `hir-def`
//! but much simpler, since every file is a module and imports are explicit.
//!
//! - [`item_tree`] summarises the items of a file without their bodies, so
//!   that editing a body doesn't invalidate anything that depends on it.
//! - [`ids`] has interned IDs for items, pointing into item trees.
//! - [`nameres`] resolves the names at the top level of each file,
//!   including imports.

pub mod ids;
pub mod item_tree;
mod name;
pub mod nameres;

pub use name::Name;

#[cfg(test)]
mod test_db;

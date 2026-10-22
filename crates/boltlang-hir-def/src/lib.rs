//! Definitions in Bolt programs, modelled after rust-analyzer's `hir-def`
//! but much simpler, since every file is a module and imports are explicit.
//!
//! - [`item_tree`] summarises the items of a file without their bodies, so
//!   that editing a body doesn't invalidate anything that depends on it.
//! - [`ids`] has interned IDs for items, pointing into item trees.
//! - [`nameres`] resolves the names at the top level of each file,
//!   including imports.
//! - [`expr_store`] holds lowered expressions, patterns and types, in the
//!   forms defined in [`hir`].
//! - [`body`] lowers the bodies of functions, consts, enum discriminants and
//!   the top-level statements of files.
//! - [`signatures`] lowers everything about items except their bodies.

pub mod arena;
pub mod body;
pub mod expr_store;
pub mod hir;
pub mod ids;
pub mod item_tree;
mod name;
pub mod nameres;
pub mod signatures;

pub use name::Name;

#[cfg(test)]
mod test_db;

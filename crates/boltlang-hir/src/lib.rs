//! The semantic model of a Bolt program, on top of hir-def and hir-ty, for
//! the tools that show it to users.
//!
//! - [`diagnostics`] collects the problems found in a file, from parsing
//!   to type checking, and finds where in the source they are.

pub mod diagnostics;

pub use diagnostics::{Diagnostic, DiagnosticKind, file_diagnostics};

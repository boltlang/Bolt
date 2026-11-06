//! Compiling patterns to decision graphs, which both the diagnostics of
//! `match` and the lowering of a body use.
//!
//! - [`decision`]: the tests that a `match` or a `let` makes, as a graph.

pub mod decision;

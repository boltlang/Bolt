//! Building MIR from HIR and what the type checker found.
//!
//! - [`build`]: the MIR of a body and of its lambdas, [`mir_bodies`].
//! - [`lambda`]: what each lambda captures.

pub mod build;
pub mod lambda;

pub use build::{MirBodies, mir_bodies};

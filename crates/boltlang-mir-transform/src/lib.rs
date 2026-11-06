//! Passes over MIR: rewrites of a body that run after it's built, like
//! desugarings, and the shims the compiler makes, like the constructors of
//! tuple structs and variants used as functions, and the deep copy of
//! `imm e`.
//!
//! - [`shim`]: the bodies of [`boltlang_mir::Shim`]s.
//!
//! A pass changes a body in place, through `boltlang_mir::VisitorMut`.
//! There are no passes yet.

pub mod shim;

pub use shim::{CtorSig, check_imm_deep, ctor_sig, shim_body};

#[cfg(test)]
mod tests;

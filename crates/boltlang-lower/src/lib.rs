//! Lowering after type checking, shared by the backends.
//!
//! So far: monomorphization, which instances of its functions a program
//! needs and what each use in them calls, and compiling patterns to
//! decisions.
//!
//! - [`instance`]: an [`Instance`] is a body at concrete types. It's
//!   specialized on types, not on mutabilities: those are its parameters.
//! - [`ty`]: the closed types of an instance, [`MonoTy`], with
//!   [`MonoTy::Erased`] for erased types.
//! - [`uses`]: what a use of a function or constant is in an instance, and
//!   the dictionaries that erased code calls class methods through.
//! - [`collect`]: every instance a program needs, from its entry file and
//!   its tests, erasing functions that recurse polymorphically.
//! - [`decision`]: the tests that a `match` or a `let` makes, as a graph.

pub mod collect;
pub mod decision;
pub mod instance;
pub mod ty;
pub mod uses;

pub use collect::{InstanceUses, Program, Site, program};
pub use instance::{Instance, MonoConstraint};
pub use ty::{MonoMut, MonoTy};
pub use uses::{ClassItem, Cx, Dict, Use, UseError};

#[cfg(test)]
mod tests;

//! Lowering after type checking, shared by the backends.
//!
//! So far: monomorphization, which instances of its functions a program
//! needs and what each use in them calls, and the captures of lambdas, for
//! lifting them. Patterns are compiled to decisions in `boltlang-pattern`.
//!
//! - [`instance`]: an [`Instance`] is a body at concrete types. It's
//!   specialized on types, not on mutabilities: those are its parameters.
//! - [`ty`]: the closed types of an instance, [`MonoTy`], with
//!   [`MonoTy::Erased`] for erased types.
//! - [`uses`]: what a use of a function or constant is in an instance, and
//!   the dictionaries that erased code calls class methods through.
//! - [`collect`]: every instance a program needs, from its entry file and
//!   its tests, erasing functions that recurse polymorphically.
//! - [`lambda`]: what each lambda captures, to lift it to a function.

pub mod collect;
pub mod instance;
pub mod lambda;
pub mod ty;
pub mod uses;

pub use collect::{InstanceUses, Program, Site, program};
pub use instance::{Instance, MonoConstraint};
pub use ty::{MonoMut, MonoTy, TyError, field_types};
pub use uses::{ClassItem, Cx, Dict, Use, UseError};

#[cfg(test)]
mod tests;

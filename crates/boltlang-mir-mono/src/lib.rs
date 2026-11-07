//! Monomorphization, shared by the backends: which instances of its
//! functions a program needs, and what each use in them calls.
//!
//! Patterns are compiled to decisions in `boltlang-pattern`, and the
//! captures of lambdas are found in `boltlang-mir-build`.
//!
//! - [`instance`]: an [`Instance`] is a body at concrete types. It's
//!   specialized on types, not on mutabilities: those are its parameters.
//! - [`ty`]: the closed types of an instance, [`MonoTy`], with
//!   [`MonoTy::Erased`] for erased types.
//! - [`uses`]: what a use of a function or constant is in an instance, and
//!   the dictionaries that erased code calls class methods through.
//! - [`collect`]: every instance a program needs, from its entry file and
//!   its tests, erasing functions that recurse polymorphically.

pub mod collect;
pub mod instance;
pub mod ty;
pub mod uses;

pub use collect::{InstanceUses, Program, Role, Site, program};
pub use instance::{Instance, InstanceDef, MonoConstraint};
pub use ty::{MonoEffect, MonoMut, MonoRow, MonoTy, TyError, closed_ty, field_types};
pub use uses::{ClassItem, Cx, Dict, Use, UseError};

#[cfg(test)]
mod tests;

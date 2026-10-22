//! Type checking, on top of hir-def.
//!
//! Like the old checker, inference generates constraints, which the
//! [`solve::Solver`] solves with the help of a [`unify::Unifier`] that only
//! knows about types.
//!
//! - [`ty`] defines the types.
//! - [`unify`] makes types equal by binding inference variables.
//! - [`lower`] turns type references and signatures into types.

pub mod lower;
pub mod ty;
pub mod unify;

pub use ty::{AdtId, GenericParamId, Ty, TyVar, VarKind};

//! Type checking, on top of hir-def.
//!
//! Like the old checker, inference generates constraints, which the
//! [`solve::Solver`] solves with the help of a [`unify::Unifier`] that only
//! knows about types.
//!
//! - [`ty`] defines the types.
//! - [`builtin`] lists the functions the compiler provides, like `print`.
//! - [`unify`] makes types equal by binding inference variables.
//! - [`kind`] infers the kinds of type parameters, like `Type -> Type`.
//! - [`inhabited`] finds the structs and enums that have no values.
//! - [`lower`] turns type references and signatures into types.
//! - [`solve`] solves equality and class constraints.
//! - [`scheme`] has the types of functions, which can be generic.
//! - [`infer`] infers the types in a body, and [`check`] has the queries
//!   that run it.

pub mod builtin;
pub mod check;
pub mod holds;
pub mod infer;
pub mod inhabited;
pub mod kind;
pub mod lang;
pub mod lower;
pub mod scheme;
pub mod solve;
pub mod ty;
pub mod unify;

pub use check::{InferenceCycle, function_scheme, infer_body};
pub use infer::{InferenceDiagnostic, InferenceResult};
pub use scheme::Scheme;
pub use ty::{AdtId, GenericParamId, NumberKind, Ty, TyNames, TyVar, VarKind};

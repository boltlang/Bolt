//! A tree-walking evaluator for Bolt, working directly on the bodies of
//! hir-def.
//!
//! It's meant for running small programs and, later, for evaluating code at
//! compile time, like the bodies of macros and consts. It doesn't need the
//! type checker: paths are resolved with the [`Resolver`] of hir-def, and
//! `x.f(y)` is `f(x, y)` with the `f` in scope, as in the type checker.
//!
//! It's deliberately simple:
//!
//! - All integers are `i128`, whatever their type, so `255u8 + 1` is `256`.
//!   Only overflowing `i128` is an error.
//! - Class methods aren't dispatched, since that needs types. Calling a
//!   method that only has a signature in a class is an error.
//! - Macros, `async`, `await` and `yield` aren't supported.
//!
//! Every evaluation can be bounded with [`Limits`], so that code from an
//! editor can't loop forever or overflow the stack.
//!
//! [`Resolver`]: boltlang_hir_def::resolver::Resolver

mod error;
mod eval;
mod value;

pub use error::{ErrorKind, EvalError};
pub use eval::{Evaluator, Limits};
pub use value::{Closure, Ctor, Value};

#[cfg(test)]
mod tests;

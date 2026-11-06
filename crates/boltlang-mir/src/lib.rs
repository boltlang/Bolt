//! Bolt's MIR: a body after type checking, with everything the checker
//! decided written into it, for the passes and the backends.
//!
//! MIR is a tree, like HIR, not a control-flow graph: that's Conduit's job.
//! It differs from HIR in that each node has its type, each name what it
//! refers to, and each call the function it calls, so nothing has to be
//! looked up in side tables. It's generic: the types can contain the body's
//! type parameters, and monomorphization substitutes them later.
//!
//! The nodes live in an arena, and a pass rewrites them in place: it
//! replaces a node, or adds new ones. So a node's id stays the same, and so
//! does the HIR expression it came from.
//!
//! - [`body`]: the bodies and their nodes.
//! - [`visit`]: walking a body, to read it ([`Visitor`]) or to rewrite it
//!   ([`VisitorMut`]).
//! - [`display`]: a body as text, for tests and debugging.

pub mod body;
pub mod display;
pub mod visit;

pub use body::{
    Arm, Body, BodyOwner, Capture, ClassItem, Expr, ExprId, ExprKind, ItemRef, LabelId, Local,
    LocalDecl, Pattern, Place, Shim, Stmt,
};
pub use visit::{Visitor, VisitorMut, walk_expr, walk_expr_mut};

#[cfg(test)]
mod tests;

//! Passes over MIR: rewrites of a body that run after it's built, like
//! desugarings, with the shims they make, like the deep copy of `imm e`.
//!
//! A pass changes a body in place, through `boltlang_mir::VisitorMut`.
//! There are no passes yet.

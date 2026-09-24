
mod types;
mod infer;
mod solve;
mod unify;

use std::collections::HashMap;

use crate::rowan::GreenNode;

pub use infer::{SymbolKind, Scheme, InferContext, Constraints, Provenance};
pub use types::{Type, TVar, ConId, TVSub};

#[derive(Clone, Eq, PartialEq)]
pub struct CheckResult {
    pub mapping: HashMap<GreenNode, Scheme>,
}


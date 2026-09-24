
use boltlang_common::Span;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub message: String,
    pub span: Span,
}

impl Diagnostic {
    pub fn new<S: Into<String>>(message: S, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }
}

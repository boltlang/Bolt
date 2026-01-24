
/// Internal representation of an error generated during parsing.
pub type Error = String;

/// Public-facing parse error.
#[salsa::accumulator]
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SyntaxError {
    pub message: String,
    pub offset: usize,
}

pub const CODE_SYNTAX_ERROR: u16 = 1;

impl SyntaxError {

    pub fn new(message: impl Into<String>, offset: usize) -> Self {
        Self {
            message: message.into(),
            offset,
        }
    }

    pub fn code(&self) -> u16 {
        CODE_SYNTAX_ERROR
    }

}


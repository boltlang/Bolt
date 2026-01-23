
/// Internal representation of an error generated during parsing.
pub type Error = String;

/// Public-facing parse error.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SyntaxError {
    pub message: String,
    pub offset: usize,
}

impl SyntaxError {

    pub fn new(message: impl Into<String>, offset: usize) -> Self {
        Self {
            message: message.into(),
            offset,
        }
    }

}


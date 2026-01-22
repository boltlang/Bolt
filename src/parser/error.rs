
/// Internal representation of an error generated during parsing.
pub type Error = String;

/// Public-facing parse error.
pub struct SyntaxError {
    message: String,
    offset: usize,
}

impl SyntaxError {

    pub fn new(message: impl Into<String>, offset: usize) -> Self {
        Self {
            message: message.into(),
            offset,
        }
    }

}


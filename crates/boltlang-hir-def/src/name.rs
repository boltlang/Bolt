use std::fmt;

/// The name of something, like a function or a variable.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, salsa::SalsaValue)]
pub struct Name(String);

impl Name {
    pub fn new(text: impl Into<String>) -> Self {
        Name(text.into())
    }

    /// The name an identifier stands for: `r#match` is the name `match`.
    pub fn from_ident(text: &str) -> Self {
        Name::new(text.strip_prefix("r#").unwrap_or(text))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

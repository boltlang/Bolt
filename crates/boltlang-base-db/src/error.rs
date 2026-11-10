use crate::OwnedUri;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("the file {0} was not found in the database")]
    FileNotFound(OwnedUri),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Custom(String),
}

impl From<fluent_uri::ParseError> for Error {
    fn from(value: fluent_uri::ParseError) -> Self {
        Error::Custom(format!("failed to parse as URI: {}", value))
    }
}

pub type Result<T> = std::result::Result<T, Error>;

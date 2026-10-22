
use crate::OwnedUri;

// #[derive(Debug)]
// pub struct Error {
//     // This field is boxed in order to avoid copying it on the stack
//     inner: Box<InnerError>,
// }

// impl Display for Error {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         self.inner.fmt(f)
//     }
// }

// impl From<InnerError> for Error {
//     fn from(value: InnerError) -> Self {
//         Error { inner: Box::new(value) }
//     }
// }

// impl std::error::Error for Error {}

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

use std::fmt::Display;

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
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

use thiserror::Error;

// Error type for the sparse I/O library.
#[derive(Debug, Error)]
pub enum Error {
    /// An I/O error occurred.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// An out of bounds read occurred.
    #[error("Out-of-bounds read")]
    OOB,

    /// An error occurred outside of the handled cases.
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;

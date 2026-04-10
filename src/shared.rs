use std::fmt;
use std::io::{Error, ErrorKind};
use std::sync::Arc;

use bytes::Bytes;
use futures::future::{BoxFuture, Shared};

#[derive(Clone, Debug)]
pub(crate) struct SharedIoError {
    kind: ErrorKind,
    raw_os_error: Option<i32>,
    message: Arc<str>,
}

impl SharedIoError {
    pub(crate) fn into_io_error(self) -> Error {
        if let Some(code) = self.raw_os_error {
            Error::from_raw_os_error(code)
        } else {
            Error::new(self.kind, self.message.to_string())
        }
    }
}

impl From<Error> for SharedIoError {
    fn from(error: Error) -> Self {
        let kind = error.kind();
        let raw_os_error = error.raw_os_error();
        let message: Arc<str> = error.to_string().into();

        Self {
            kind,
            raw_os_error,
            message,
        }
    }
}

impl fmt::Display for SharedIoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for SharedIoError {}

pub(crate) type SharedChunk = Shared<BoxFuture<'static, Result<Bytes, SharedIoError>>>;

pub(crate) const DEFAULT_CHUNK_SIZE: usize = 128 * 1024; // 128 KiB

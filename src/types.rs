//! Shared public types used by SparseIO traits and objects.

use std::io;
use std::pin::Pin;

use bytes::Bytes;

/// Stream type for chunked byte output from a viewer.
pub type ByteStream = Pin<Box<dyn Iterator<Item = io::Result<Bytes>> + Send>>;

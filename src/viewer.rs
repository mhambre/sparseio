//! Object viewer returned by [`SparseIO::open`](crate::SparseIO::open).

use std::io;

use bytes::Bytes;

use crate::types::ByteStream;

/// Read interface for a single upstream object mediated by SparseIO.
#[allow(dead_code, clippy::len_without_is_empty)]
pub struct Viewer {
    /// Stub
    _stub: (),
}

impl Viewer {
    /// Alias for offset-based reads.
    pub fn read_at(&self, _offset: usize, _length: usize) -> io::Result<Bytes> {
        todo!("read bytes at offset from upstream object")
    }

    /// Return the total length of the upstream object.
    pub fn len(&self) -> io::Result<usize> {
        todo!("return upstream object length")
    }

    /// Convert this viewer into a chunked byte stream.
    pub fn bytestream(&self) -> ByteStream {
        todo!("convert viewer into a chunked byte stream")
    }
}

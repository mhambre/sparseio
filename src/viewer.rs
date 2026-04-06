use std::sync::Arc;

use crate::{Result, SparseIO, Writer, reader::Reader};

/// Viewer type for the sparse I/O library. This struct provides an interface to read
/// from a sparse object opened via [`crate::SparseIO`].
pub struct Viewer<R: Reader, W: Writer> {
    cursor: usize,
    io: Arc<SparseIO<R, W>>,
}

impl<R: Reader, W: Writer> Viewer<R, W> {
    pub(crate) fn new(io: Arc<SparseIO<R, W>>) -> Self {
        Self { cursor: 0, io }
    }

    /// Move the file read cursor to the specified offset. Failure occurs
    /// if the offset is out of bounds (greater than the length of the sparse object).
    pub async fn seek(&mut self, offset: usize) -> Result<()> {
        if offset > self.io.len {
            return Err(crate::Error::OOB);
        }
        self.cursor = offset;
        Ok(())
    }

    /// Read data from the sparse object into the provided buffer, starting at the current cursor position.
    /// The cursor is advanced by the number of bytes read. Failure occurs if the read goes out of bounds.
    pub async fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let len = self.io.len();
        if self.cursor >= len || self.cursor.saturating_add(buf.len()) >= len {
            return Err(crate::Error::OOB);
        }

        unimplemented!("Read logic not implemented yet");
    }

    // Get a bytestream of the sparse object starting at the current cursor position, and of the specified
    // length.
    // pub async fn stream(&mut self, len: usize) -> Result::<impl futures::Stream<Item = Result<bytes::Bytes>>> {
    //     unimplemented!("Stream logic not implemented yet");
    // }
}

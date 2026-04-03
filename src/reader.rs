use crate::{Result, inner::SourceReader};

/// Reader type for the sparse I/O library. This struct provides an interface to read
/// from a sparse object opened via [`crate::SparseIO`].
pub struct SparseReader<I: SourceReader> {
    cursor: usize,
    inner: I,
}

impl<I: SourceReader> SparseReader<I> {
    pub(crate) fn new(inner: I) -> Self {
        Self { cursor: 0, inner }
    }

    /// Move the file read cursor to the specified offset. Failure occurs
    /// if the offset is out of bounds (greater than the length of the sparse object).
    pub async fn seek(&mut self, offset: usize) -> Result<()> {
        if offset > self.inner.len().await {
            return Err(crate::Error::OOB);
        }
        self.cursor = offset;
        Ok(())
    }

    /// Read data from the sparse object into the provided buffer, starting at the current cursor position.
    /// The cursor is advanced by the number of bytes read. Failure occurs if the read goes out of bounds.
    pub async fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let len = self.inner.len().await;
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

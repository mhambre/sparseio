use std::io::{Error, ErrorKind, Result};
use std::sync::Arc;

use bytes::Bytes;
use futures::stream::{self, BoxStream};

use crate::{SparseIO, Writer, reader::Reader};

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
    pub fn seek(&mut self, offset: usize) -> Result<()> {
        if offset > self.io.len() {
            return Err(Error::new(ErrorKind::InvalidInput, "seek offset out of bounds"));
        }
        self.cursor = offset;
        Ok(())
    }

    /// Read data from the sparse object into `buf`, starting at the current
    /// cursor position.
    ///
    /// The cursor is advanced by the number of bytes copied from the sparse
    /// object. Any portion of `buf` beyond EOF remains zero-filled.
    pub async fn read(&mut self, buf: &mut [u8]) -> Result<usize>
    where
        R: Send + Sync + 'static,
        W: Send + Sync + 'static,
    {
        let copied = Self::fill_from_cursor(&self.io, self.cursor, buf).await?;
        self.cursor += copied;
        Ok(copied)
    }

    /// Return a byte stream that starts at the current cursor and continues
    /// until EOF or dropped.
    ///
    /// The start offset does not need to be chunk-aligned. Bytes are emitted along
    /// chunk boundaries but the user you must be cognisant of the fact that the first and
    /// last chunk may be partial.
    pub async fn to_bytestream(&mut self) -> BoxStream<'static, Result<Bytes>>
    where
        R: Send + Sync + 'static,
        W: Send + Sync + 'static,
    {
        let start = self.cursor;
        let end = self.io.len();
        self.cursor = end;
        let io = self.io.clone();

        Box::pin(stream::unfold(StreamState { io, cursor: start, end }, |mut state| async move {
            if state.cursor >= state.end {
                return None;
            }

            let chunk_size = state.io.chunk_size;
            let in_chunk = state.cursor % chunk_size;
            let chunk_base = state.cursor - in_chunk;
            match state.io.read_chunk(chunk_base).await {
                Ok(chunk) => {
                    if in_chunk >= chunk.len() {
                        return None;
                    }

                    let available = chunk.len() - in_chunk; // Front partial case
                    let remaining = state.end - state.cursor;
                    let emit_len = available.min(remaining); // Back partial case
                    let out = Bytes::copy_from_slice(&chunk[in_chunk..in_chunk + emit_len]);
                    state.cursor += emit_len;
                    Some((Ok(out), state))
                },
                Err(err) => Some((Err(err), state)),
            }
        }))
    }

    /// Internal helper to fill a provided buffer.
    async fn fill_from_cursor(io: &Arc<SparseIO<R, W>>, cursor: usize, buf: &mut [u8]) -> Result<usize>
    where
        R: Send + Sync + 'static,
        W: Send + Sync + 'static,
    {
        // Edges
        buf.fill(0);
        if buf.is_empty() {
            return Ok(0);
        }
        let len = io.len();
        if cursor >= len {
            return Ok(0);
        }

        let max_copy = (len - cursor).min(buf.len());
        let mut copied = 0usize;
        let chunk_size = io.chunk_size;

        while copied < max_copy {
            let absolute_offset = cursor + copied;
            let chunk_base = absolute_offset - (absolute_offset % chunk_size);
            let chunk = io.read_chunk(chunk_base).await?;
            if chunk.is_empty() {
                break;
            }

            let in_chunk = absolute_offset - chunk_base;
            if in_chunk >= chunk.len() {
                break;
            }

            let available = chunk.len() - in_chunk;
            let to_copy = (max_copy - copied).min(available);
            buf[copied..copied + to_copy].copy_from_slice(&chunk[in_chunk..in_chunk + to_copy]);
            copied += to_copy;
        }

        Ok(copied)
    }
}

struct StreamState<R: Reader, W: Writer> {
    io: Arc<SparseIO<R, W>>,
    cursor: usize,
    end: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    use std::collections::HashMap;

    #[derive(Clone)]
    struct TestReader {
        data: Arc<Vec<u8>>,
    }

    impl TestReader {
        fn new(data: Vec<u8>) -> Self {
            Self { data: Arc::new(data) }
        }
    }

    impl Reader for TestReader {
        async fn read_at(&self, offset: usize, buffer: &mut [u8]) -> std::io::Result<usize> {
            let start = offset.min(self.data.len());
            let end = (start + buffer.len()).min(self.data.len());
            let src = &self.data[start..end];
            buffer[..src.len()].copy_from_slice(src);
            Ok(src.len())
        }

        async fn len(&self) -> std::io::Result<usize> {
            Ok(self.data.len())
        }
    }

    #[derive(Default)]
    struct TestWriter {
        extents: HashMap<usize, Bytes>,
    }

    impl Writer for TestWriter {
        async fn create_extent(&mut self, offset: usize, data: Bytes) -> std::io::Result<()> {
            self.extents.insert(offset, data);
            Ok(())
        }

        async fn read_extent(&self, offset: usize) -> std::io::Result<Bytes> {
            Ok(self.extents.get(&offset).cloned().unwrap_or_else(Bytes::new))
        }

        async fn delete_extent(&mut self, offset: usize) -> std::io::Result<()> {
            self.extents.remove(&offset);
            Ok(())
        }
    }

    #[tokio::test]
    async fn read_reads_across_chunk_boundaries_from_unaligned_cursor() {
        let data: Vec<u8> = (0..128).map(|v| v as u8).collect();
        let io = Arc::new(
            SparseIO::builder()
                .chunk_size(16)
                .reader(TestReader::new(data.clone()))
                .writer(TestWriter::default())
                .build()
                .await
                .expect("builder should succeed"),
        );

        let mut viewer = io.viewer();
        viewer.seek(3).expect("seek should succeed");

        let mut buf = vec![0u8; 40];
        let read = viewer.read(&mut buf).await.expect("read should succeed");

        assert_eq!(read, 40);
        assert_eq!(buf, data[3..43].to_vec());
    }

    #[tokio::test]
    async fn read_zero_fills_past_end_of_object() {
        let data: Vec<u8> = (0..32).map(|v| v as u8).collect();
        let io = Arc::new(
            SparseIO::builder()
                .chunk_size(8)
                .reader(TestReader::new(data.clone()))
                .writer(TestWriter::default())
                .build()
                .await
                .expect("builder should succeed"),
        );

        let mut viewer = io.viewer();
        viewer.seek(28).expect("seek should succeed");

        let mut buf = vec![0xFFu8; 10];
        let read = viewer.read(&mut buf).await.expect("read should succeed");

        assert_eq!(read, 4);
        assert_eq!(&buf[..4], &data[28..32]);
        assert!(buf[4..].iter().all(|b| *b == 0));
    }

    #[tokio::test]
    async fn to_bytestream_streams_from_cursor_to_eof_and_advances_cursor() {
        let data: Vec<u8> = (0..64).map(|v| v as u8).collect();
        let io = Arc::new(
            SparseIO::builder()
                .chunk_size(16)
                .reader(TestReader::new(data.clone()))
                .writer(TestWriter::default())
                .build()
                .await
                .expect("builder should succeed"),
        );

        let mut viewer = io.viewer();
        viewer.seek(5).expect("seek should succeed");

        let chunks = viewer.to_bytestream().await.collect::<Vec<_>>().await;
        let mut joined = Vec::new();
        for chunk in chunks {
            let chunk = chunk.expect("stream chunk should be ok");
            joined.extend_from_slice(&chunk);
        }
        assert_eq!(joined, data[5..].to_vec());

        let mut next = [0u8; 1];
        let read = viewer.read(&mut next).await.expect("follow-up read should succeed");
        assert_eq!(read, 0);
    }

    #[tokio::test]
    async fn to_bytestream_can_be_dropped_early() {
        let data: Vec<u8> = (0..64).map(|v| v as u8).collect();
        let io = Arc::new(
            SparseIO::builder()
                .chunk_size(16)
                .reader(TestReader::new(data.clone()))
                .writer(TestWriter::default())
                .build()
                .await
                .expect("builder should succeed"),
        );

        let mut viewer = io.viewer();
        viewer.seek(7).expect("seek should succeed");

        let first = viewer
            .to_bytestream()
            .await
            .next()
            .await
            .expect("stream should produce a chunk")
            .expect("chunk should be ok");
        assert_eq!(first, Bytes::from(data[7..16].to_vec()));
    }
}

use std::io;

use async_trait::async_trait;
use bytes::Bytes;

use crate::Reader;
use crate::utils::rand::Latency;

/// In-memory reader that serves windows from a fixed byte buffer.
pub(crate) struct MockReader {
    data: Bytes,
    latency: Latency,
}

impl MockReader {
    /// Create a reader using the default upstream latency profile.
    pub(crate) fn new(data: impl Into<Bytes>) -> Self {
        Self {
            data: data.into(),
            latency: Latency::reader(),
        }
    }

    /// Create a reader with an explicit latency profile.
    pub(crate) fn with_latency(data: impl Into<Bytes>, latency: Latency) -> Self {
        Self {
            data: data.into(),
            latency,
        }
    }
}

#[async_trait]
impl Reader for MockReader {
    /// Return the size of the fixed source buffer.
    async fn len(&self) -> io::Result<usize> {
        self.latency.pause().await;

        Ok(self.data.len())
    }

    /// Return a zero-copy slice for a valid byte window.
    async fn read_at(&self, offset: usize, length: usize) -> io::Result<Bytes> {
        self.latency.pause().await;

        let end = offset
            .checked_add(length)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "reader range overflowed usize"))?;

        if end > self.data.len() {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "reader range extends past end of source"));
        }

        Ok(self.data.slice(offset..end))
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::time::Duration;

    use bytes::Bytes;

    use super::MockReader;
    use crate::Reader;
    use crate::utils::rand::Latency;

    #[tokio::test]
    async fn len_returns_source_length() {
        let reader = MockReader::new(Bytes::from_static(b"hello"));

        assert_eq!(reader.len().await.expect("len should be readable"), 5);
    }

    #[tokio::test]
    async fn read_at_returns_window_into_source_bytes() {
        let data = Bytes::from_static(b"hello world");
        let reader = MockReader::new(data.clone());

        let window = reader.read_at(6, 5).await.expect("range should be readable");

        assert_eq!(window, Bytes::from_static(b"world"));
        assert_eq!(window.as_ptr(), data.slice(6..11).as_ptr());
    }

    #[tokio::test]
    async fn read_at_allows_empty_window_at_end() {
        let reader = MockReader::new(Bytes::from_static(b"hello"));

        assert_eq!(reader.read_at(5, 0).await.expect("empty end range should be readable"), Bytes::new());
    }

    #[tokio::test]
    async fn read_at_rejects_out_of_bounds_window() {
        let reader = MockReader::new(Bytes::from_static(b"hello"));
        let error = reader.read_at(4, 2).await.expect_err("range should fail");

        assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[tokio::test]
    async fn can_construct_with_latency() {
        let latency = Latency::new(Duration::ZERO, Duration::ZERO, 1);
        let reader = MockReader::with_latency(Bytes::from_static(b"hello"), latency);

        assert_eq!(reader.len().await.expect("len should be readable"), 5);
    }
}

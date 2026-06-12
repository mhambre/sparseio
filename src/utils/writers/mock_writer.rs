use std::collections::HashMap;
use std::io;

use async_trait::async_trait;
use bytes::Bytes;
use parking_lot::RwLock;

use crate::Writer;
use crate::utils::rand::Latency;

#[derive(Default)]
/// In-memory writer backed by a lock-protected byte map.
pub(crate) struct MockWriter {
    values: RwLock<HashMap<String, Bytes>>,
    latency: Latency,
}

impl MockWriter {
    /// Create an empty writer using the default cache latency profile.
    pub(crate) fn new() -> Self {
        Self {
            values: RwLock::new(HashMap::new()),
            latency: Latency::intra_dc(),
        }
    }

    /// Create an empty writer with an explicit latency profile.
    pub(crate) fn with_latency(latency: Latency) -> Self {
        Self {
            values: RwLock::new(HashMap::new()),
            latency,
        }
    }
}

#[async_trait]
impl Writer for MockWriter {
    /// Store bytes under the given key, replacing any existing value.
    async fn write(&self, key: &str, value: Bytes) -> io::Result<()> {
        self.latency.pause().await;
        self.values.write().insert(key.to_owned(), value);

        Ok(())
    }

    /// Read the bytes currently stored for a key.
    async fn read(&self, key: &str) -> io::Result<Option<Bytes>> {
        self.latency.pause().await;

        Ok(self.values.read().get(key).cloned())
    }

    /// Remove a key from the mock cache.
    async fn delete(&self, key: &str) -> io::Result<()> {
        self.latency.pause().await;
        self.values.write().remove(key);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bytes::Bytes;

    use super::MockWriter;
    use crate::Writer;
    use crate::utils::rand::Latency;

    #[tokio::test]
    async fn write_read_and_delete_value() {
        let writer = MockWriter::new();
        let value = Bytes::from_static(b"cached");

        writer.write("key", value).await.expect("write should succeed");
        assert_eq!(writer.read("key").await.expect("read should succeed"), Some(Bytes::from_static(b"cached")));

        writer.delete("key").await.expect("delete should succeed");
        assert_eq!(writer.read("key").await.expect("read should succeed"), None);
    }

    #[tokio::test]
    async fn delete_missing_key_succeeds() {
        let writer = MockWriter::new();

        writer.delete("missing").await.expect("delete should succeed");
    }

    #[tokio::test]
    async fn can_construct_with_latency() {
        let writer = MockWriter::with_latency(Latency::new(Duration::ZERO, Duration::ZERO, 1));
        let value = Bytes::from_static(b"cached");

        writer.write("key", value.clone()).await.expect("write should succeed");
        assert_eq!(writer.read("key").await.expect("read should succeed"), Some(value));
    }
}

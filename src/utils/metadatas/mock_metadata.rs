use std::collections::HashMap;
use std::io;

use async_trait::async_trait;
use bytes::Bytes;
use parking_lot::RwLock;

use crate::Metadata;
use crate::utils::rand::Latency;

#[derive(Default)]
/// In-memory metadata store backed by a lock-protected byte map.
pub(crate) struct MockMetadata {
    values: RwLock<HashMap<String, Bytes>>,
    latency: Latency,
}

impl MockMetadata {
    /// Create an empty metadata store using the default cache latency profile.
    pub(crate) fn new() -> Self {
        Self {
            values: RwLock::new(HashMap::new()),
            latency: Latency::intra_dc(),
        }
    }

    /// Create an empty metadata store with an explicit latency profile.
    pub(crate) fn with_latency(latency: Latency) -> Self {
        Self {
            values: RwLock::new(HashMap::new()),
            latency,
        }
    }
}

#[async_trait]
impl Metadata for MockMetadata {
    /// Read the metadata value currently stored for a key.
    async fn get(&self, key: &str) -> io::Result<Option<Bytes>> {
        self.latency.pause().await;

        Ok(self.values.read().get(key).cloned())
    }

    /// Store a metadata value, replacing any existing value.
    async fn set(&self, key: &str, value: Bytes) -> io::Result<()> {
        self.latency.pause().await;
        self.values.write().insert(key.to_owned(), value);

        Ok(())
    }

    /// Remove a key from the mock metadata store.
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

    use super::MockMetadata;
    use crate::Metadata;
    use crate::utils::rand::Latency;

    #[tokio::test]
    async fn set_get_and_delete_value() {
        let metadata = MockMetadata::new();

        metadata
            .set("key", Bytes::from_static(b"metadata"))
            .await
            .expect("set should succeed");
        assert_eq!(metadata.get("key").await.expect("get should succeed"), Some(Bytes::from_static(b"metadata")));

        metadata.delete("key").await.expect("delete should succeed");
        assert_eq!(metadata.get("key").await.expect("get should succeed"), None);
    }

    #[tokio::test]
    async fn delete_missing_key_succeeds() {
        let metadata = MockMetadata::new();

        metadata.delete("missing").await.expect("delete should succeed");
    }

    #[tokio::test]
    async fn can_construct_with_latency() {
        let metadata = MockMetadata::with_latency(Latency::new(Duration::ZERO, Duration::ZERO, 1));

        metadata
            .set("key", Bytes::from_static(b"metadata"))
            .await
            .expect("set should succeed");
        assert_eq!(metadata.get("key").await.expect("get should succeed"), Some(Bytes::from_static(b"metadata")));
    }
}

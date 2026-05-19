use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use bytes::Bytes;

/// Reader double that fails once for selected offsets.
#[derive(Clone)]
pub struct Reader {
    data: Arc<Vec<u8>>,
    fail_offsets: Arc<HashSet<usize>>,
    failures: Arc<AtomicUsize>,
}

impl Reader {
    /// Creates a flaky reader that fails once on any listed offset.
    pub fn fail_once_at(data: Bytes, fail_offsets: impl IntoIterator<Item = usize>) -> Self {
        Self {
            data: Arc::new(data.to_vec()),
            fail_offsets: Arc::new(fail_offsets.into_iter().collect()),
            failures: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl crate::Reader for Reader {
    async fn read_at(&self, offset: usize, buffer: &mut [u8]) -> std::io::Result<usize> {
        if self.fail_offsets.contains(&offset) && self.failures.fetch_add(1, Ordering::SeqCst) == 0 {
            return Err(std::io::Error::other(format!("transient reader failure at {offset}")));
        }

        if offset >= self.data.len() || buffer.is_empty() {
            return Ok(0);
        }

        let end = (offset + buffer.len()).min(self.data.len());
        let src = &self.data[offset..end];
        buffer[..src.len()].copy_from_slice(src);
        Ok(src.len())
    }

    async fn len(&self) -> std::io::Result<usize> {
        Ok(self.data.len())
    }
}

/// Writer double that fails once for selected cache keys.
#[derive(Default, Clone)]
pub struct Writer {
    entries: Arc<tokio::sync::Mutex<std::collections::BTreeMap<String, Bytes>>>,
    fail_keys: Arc<HashSet<String>>,
    failures: Arc<AtomicUsize>,
}

impl Writer {
    pub fn fail_once_at(fail_keys: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            entries: Arc::new(tokio::sync::Mutex::new(std::collections::BTreeMap::new())),
            fail_keys: Arc::new(fail_keys.into_iter().map(Into::into).collect()),
            failures: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl crate::Writer for Writer {
    async fn set_cache(&mut self, key: &str, value: &[u8]) -> std::io::Result<()> {
        if self.fail_keys.contains(key) && self.failures.fetch_add(1, Ordering::SeqCst) == 0 {
            return Err(std::io::Error::other("transient writer failure"));
        }

        self.entries.lock().await.insert(key.to_owned(), Bytes::copy_from_slice(value));
        Ok(())
    }

    async fn get_cache(&self, key: &str) -> std::io::Result<Option<Bytes>> {
        Ok(self.entries.lock().await.get(key).cloned())
    }

    async fn delete_cache(&mut self, key: &str) -> std::io::Result<()> {
        self.entries.lock().await.remove(key);
        Ok(())
    }
}

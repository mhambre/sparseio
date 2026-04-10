use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use bytes::Bytes;

/// Reader double that fails once for selected offsets.
///
/// This is useful when a test needs to prove SparseIO clears in-flight state
/// after a transient upstream error and allows a later retry to succeed.
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

/// Writer double that fails once for selected offsets.
///
/// This is useful when a test needs to prove SparseIO clears in-flight state
/// after a transient materialization failure and retries cleanly.
#[derive(Default, Clone)]
pub struct Writer {
    extents: Arc<tokio::sync::Mutex<std::collections::BTreeMap<usize, Bytes>>>,
    fail_offsets: Arc<HashSet<usize>>,
    failures: Arc<AtomicUsize>,
}

impl Writer {
    /// Creates a flaky writer that fails once on any listed offset.
    pub fn fail_once_at(fail_offsets: impl IntoIterator<Item = usize>) -> Self {
        Self {
            extents: Arc::new(tokio::sync::Mutex::new(std::collections::BTreeMap::new())),
            fail_offsets: Arc::new(fail_offsets.into_iter().collect()),
            failures: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl crate::Writer for Writer {
    async fn create_extent(&mut self, offset: usize, data: Bytes) -> std::io::Result<()> {
        if self.fail_offsets.contains(&offset) && self.failures.fetch_add(1, Ordering::SeqCst) == 0 {
            return Err(std::io::Error::other(format!("transient writer failure at {offset}")));
        }

        self.extents.lock().await.insert(offset, data);
        Ok(())
    }

    async fn read_extent(&self, offset: usize) -> std::io::Result<Bytes> {
        Ok(self.extents.lock().await.get(&offset).cloned().unwrap_or_else(Bytes::new))
    }

    async fn delete_extent(&mut self, offset: usize) -> std::io::Result<()> {
        self.extents.lock().await.remove(&offset);
        Ok(())
    }
}

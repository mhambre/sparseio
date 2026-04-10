use std::collections::BTreeMap;
use std::sync::Arc;

use bytes::Bytes;

/// In-memory oracle reader that returns exact fixture bytes.
///
/// This is the canonical known-good reader for harnesses and integration
/// tests.
#[derive(Clone)]
pub struct Reader {
    data: Arc<Vec<u8>>,
}

impl Reader {
    /// Creates a new oracle reader from deterministic fixture bytes.
    pub fn new(data: Bytes) -> Self {
        Self {
            data: Arc::new(data.to_vec()),
        }
    }
}

impl crate::Reader for Reader {
    async fn read_at(&self, offset: usize, buffer: &mut [u8]) -> std::io::Result<usize> {
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

/// In-memory oracle writer that stores exact extents by offset.
///
/// This is the canonical known-good writer for harnesses and integration
/// tests.
#[derive(Default, Clone)]
pub struct Writer {
    extents: Arc<tokio::sync::Mutex<BTreeMap<usize, Bytes>>>,
}

impl crate::Writer for Writer {
    async fn create_extent(&mut self, offset: usize, data: Bytes) -> std::io::Result<()> {
        self.extents.lock().await.insert(offset, data);
        Ok(())
    }

    async fn read_extent(&self, offset: usize) -> std::io::Result<Bytes> {
        Ok(self
            .extents
            .lock()
            .await
            .get(&offset)
            .cloned()
            .unwrap_or_else(Bytes::new))
    }

    async fn delete_extent(&mut self, offset: usize) -> std::io::Result<()> {
        self.extents.lock().await.remove(&offset);
        Ok(())
    }
}

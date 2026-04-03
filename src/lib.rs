//! SparseIO

mod coverage;
mod error;
pub mod inner;
mod reader;
mod shared;
pub mod sources;
pub mod storage;

use std::collections::HashMap;
use std::sync::Arc;

use bytes::Bytes;
pub use error::{Error, Result};
use futures::FutureExt;
use futures::future::BoxFuture;
pub use inner::SourceReader;
pub use reader::SparseReader;
pub use storage::{Extent, ExtentStore};

use tokio::sync::Mutex;

use crate::coverage::Coverage;
use crate::shared::{DEFAULT_CHUNK_SIZE, SharedChunk};

/// User-facing API for sparse I/O operations. Provides an interface to read from a sparse object, which is backed by
/// an [`crate::storage::ExtentStore`] for storing filled in extents, and a `SourceReader` for the actual I/O operations.
#[derive(Clone)]
pub struct SparseIO<I: SourceReader, S: ExtentStore> {
    len: usize,
    chunk_size: usize,
    inner: I,

    /// Internal data structures
    coverage: Coverage,
    storage: S,
    flights: Arc<Mutex<HashMap<usize, SharedChunk>>>,
}

impl<I: SourceReader + Send + Sync + 'static, S: ExtentStore + Send + Sync + 'static> SparseIO<I, S> {
    pub fn builder() -> SparseIOBuilder<I, S> {
        SparseIOBuilder::new()
    }

    /// Seeks to the given offset in the sparse object. Returns an error if the offset is out of bounds.
    pub async fn seek(&self, offset: usize) -> std::io::Result<()> {
        if offset > self.len {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "Offset out of bounds"));
        }

        Ok(())
    }

    /// Helper method to normalize an offset to the nearest chunk boundary.
    async fn normalize_offset(&self, offset: usize) -> usize {
        offset - (offset % self.chunk_size)
    }

    /// Fetches a chunk of data from the source defined by [`crate::SourceReader`] at the given offset.
    async fn fetch_chunk_from_source(self: Arc<Self>, offset: usize) -> Result<Bytes> {
        let mut buffer = vec![0u8; self.chunk_size];
        let bytes_read = self.inner.read_at(offset, &mut buffer).await?;
        buffer.truncate(bytes_read);
        Ok(Bytes::from(buffer))
    }

    /// Gets an existing flight or creates a new one for the given offset. Offset must be aligned to the chunk size,
    /// and should be validated as such by the caller before calling this method. Offset must be aligned to the chunk
    /// size by this point.
    async fn get_or_create_flight(self: Arc<Self>, offset: usize) -> SharedChunk {
        let mut flights = self.flights.lock().await;
        if let Some(flight) = flights.get(&offset) {
            flight.clone()
        } else {
            let self_c = self.clone();
            let fut: BoxFuture<'static, std::result::Result<Bytes, String>> =
                async move { self_c.fetch_chunk_from_source(offset).await.map_err(|e: Error| e.to_string()) }.boxed();

            let shared = fut.shared();
            flights.insert(offset, shared.clone());
            shared
        }
    }

    /// Returns a new [`crate::SparseReader`] for this sparse object. The reader provides an interface to
    /// read from the sparse object,
    pub fn reader(self: Arc<Self>) -> SparseReader<I>
    where
        I: Clone,
    {
        SparseReader::new(self.inner.clone())
    }
}

pub struct SparseIOBuilder<I, S> {
    len: Option<usize>,
    chunk_size: usize,
    inner: Option<I>,
    storage: Option<S>,
}

impl<I, S> Default for SparseIOBuilder<I, S> {
    fn default() -> Self {
        Self {
            len: None,
            chunk_size: DEFAULT_CHUNK_SIZE,
            inner: None,
            storage: None,
        }
    }
}

impl<I, S> SparseIOBuilder<I, S> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the final length of the sparse object. This is a required field.
    pub fn len(mut self, len: usize) -> Self {
        self.len = Some(len);
        self
    }

    /// Sets the chunk size for the sparse object. This is an optional field, and
    /// defaults to 128KB.
    pub fn chunk_size(mut self, chunk_size: usize) -> Self {
        self.chunk_size = chunk_size;
        self
    }

    /// Sets the inner source for the sparse object (Sparse reader). This is a required field.
    pub fn inner(mut self, inner: I) -> Self {
        self.inner = Some(inner);
        self
    }

    /// Sets the extent store for the sparse object (Sparse writer). This is a required field.
    pub fn storage(mut self, storage: S) -> Self {
        self.storage = Some(storage);
        self
    }
}

impl<I: SourceReader, S: ExtentStore> SparseIOBuilder<I, S> {
    pub fn build(self) -> Result<SparseIO<I, S>> {
        // Required fields
        let len = self
            .len
            .ok_or_else(|| Error::Other("SparseIOBuilder is missing required field: len".to_string()))?;
        let inner = self
            .inner
            .ok_or_else(|| Error::Other("SparseIOBuilder is missing required field: inner".to_string()))?;
        let storage = self
            .storage
            .ok_or_else(|| Error::Other("SparseIOBuilder is missing required field: storage".to_string()))?;

        // Optional fields
        if self.chunk_size == 0 {
            return Err(Error::Other("SparseIOBuilder field chunk_size must be greater than zero".to_string()));
        }

        Ok(SparseIO {
            len,
            chunk_size: self.chunk_size,
            inner,
            coverage: Coverage::new(),
            storage,
            flights: Arc::new(Mutex::new(HashMap::new())),
        })
    }
}

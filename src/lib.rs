//! SparseIO - A Rust library for efficient sparse I/O operations. This library provides an interface to define how to efficiently
//! read from and write to sparse objects, while tracking cache coverage and in-flight operations to optimize performance and resource usage
//! in order to deduplicate work done across multiple I/O operations.

mod coverage;
mod error;
pub mod reader;
mod shared;
pub mod sources;
mod viewer;
pub mod writer;

use std::collections::HashMap;
use std::sync::Arc;

use bytes::Bytes;
pub use error::{Error, Result};
use futures::FutureExt;
use futures::future::BoxFuture;
pub use reader::Reader;
pub use viewer::Viewer;
pub use writer::{Extent, Writer};

use tokio::sync::Mutex;

use crate::coverage::Coverage;
use crate::shared::{DEFAULT_CHUNK_SIZE, SharedChunk};

/// User-facing API for sparse I/O operations. Provides an interface to read from a sparse object, which is backed by
/// an [`crate::writer::Writer`] for storing filled in extents, and a [`crate::reader::Reader`] for the actual I/O operations.
#[derive(Clone)]
pub struct SparseIO<R: Reader, W: Writer> {
    len: usize,
    chunk_size: usize,

    /// Internal data structures
    coverage: Arc<Mutex<Coverage>>,
    writer: Arc<Mutex<W>>,
    flights: Arc<Mutex<HashMap<usize, SharedChunk>>>,
    reader: R,
}

impl<R: Reader + Send + Sync + 'static, W: Writer + Send + Sync + 'static> SparseIO<R, W> {
    /// Fetches a chunk of data from the source defined by [`crate::Reader`] at the given offset.
    async fn fetch_chunk_from_source(&self, offset: usize) -> Result<Bytes> {
        let mut buffer = vec![0u8; self.chunk_size];
        let bytes_read = self.reader.read_at(offset, &mut buffer).await?;
        buffer.truncate(bytes_read);
        Ok(Bytes::from(buffer))
    }

    /// Gets an existing flight or creates a new one for the given offset. Offset must be aligned to the chunk size,
    /// and should be validated as such by the caller before calling this method. Offset must be aligned to the chunk
    /// size by this point.
    async fn get_or_create_flight(self: &Arc<Self>, offset: usize) -> SharedChunk {
        let mut flights = self.flights.lock().await;

        if let Some(flight) = flights.get(&offset) {
            flight.clone()
        } else {
            let self_c = self.clone();
            let fut: BoxFuture<'static, std::result::Result<Bytes, String>> = self_c.get_new_chunk(offset).boxed();
            let shared = fut.shared();
            flights.insert(offset, shared.clone());
            shared
        }
    }

    /// Fully internal function for managing the flow from Read -> Flight -> Store -> Update Coverage
    async fn get_new_chunk(self: Arc<Self>, offset: usize) -> std::result::Result<Bytes, String> {
        let self_c = self.clone();
        let chunk_data = self_c.fetch_chunk_from_source(offset).await.map_err(|e| e.to_string())?;

        // Write to store and update coverage
        self.writer
            .lock()
            .await
            .create_extent(offset, chunk_data.clone())
            .await
            .map_err(|e| e.to_string())?;
        self.coverage.lock().await.insert(offset, chunk_data.len()).await;

        Ok(chunk_data)
    }

    /// Internal function to read a chunk of data at the given offset. This function will first check the coverage
    /// to see if the chunk is already filled in, and if so, will read from the store. If not, it will fetch the chunk
    /// from the source, write it to the store, update coverage, and then return the data.
    ///
    /// Leveraged by the reader to construct a stream, extract a chunk of data for a read, etc.
    async fn read_chunk(self: &Arc<Self>, offset: usize) -> Result<Bytes> {
        // Check coverage to see if chunk is filled in
        let offset_norm = self.normalize_offset(offset);
        if let Some((start, end)) = self.coverage.lock().await.get(offset).await {
            if offset >= start && offset < end {
                // Chunk is filled in, read from store
                if let Some(_) = self.writer.lock().await.read_extent(offset).await? {
                    unimplemented!("Read logic from store not implemented yet, should return bytes not extent");
                } else {
                    // This should never happen, as coverage indicates this chunk is filled in, but store does not have it
                    return Err(Error::Other(format!(
                        "Inconsistent state: coverage indicates chunk at offset {} is filled in, but store does not have it",
                        offset
                    )));
                }
            } else {
                return Err(Error::OOB);
            }
        } else {
            // Chunk is not filled in, fetch from source, write to store, update coverage
            let chunk_data = self
                .get_or_create_flight(offset_norm)
                .await
                .await
                .map_err(|e| Error::Other(e))?;

            Ok(chunk_data)
        }
    }
}

impl<R: Reader, W: Writer> SparseIO<R, W> {
    /// Returns a new [`crate::SparseIOBuilder`] to construct a [`crate::SparseIO`] instance.
    pub fn builder() -> SparseIOBuilder<R, W> {
        SparseIOBuilder::new()
    }

    /// Gets the total length of the underlying sparse object.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Helper function to normalize the offset provided to align with the chunk size.
    fn normalize_offset(&self, offset: usize) -> usize {
        offset - (offset % self.chunk_size)
    }

    /// Returns a new [`crate::Viewer`] for this sparse object. The viewer provides an interface to
    /// read from the sparse object,
    pub fn viewer(self: &Arc<Self>) -> Viewer<R, W> {
        Viewer::new(self.clone())
    }
}

pub struct SparseIOBuilder<R, W> {
    len: Option<usize>,
    chunk_size: usize,
    reader: Option<R>,
    writer: Option<W>,
}

impl<R, W> Default for SparseIOBuilder<R, W> {
    fn default() -> Self {
        Self {
            len: None,
            chunk_size: DEFAULT_CHUNK_SIZE,
            reader: None,
            writer: None,
        }
    }
}

impl<R, W> SparseIOBuilder<R, W> {
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

    /// Sets the source reader for the sparse object. This is a required field.
    pub fn reader(mut self, reader: R) -> Self {
        self.reader = Some(reader);
        self
    }

    /// Sets the extent store for the sparse object. This is a required field.
    pub fn writer(mut self, writer: W) -> Self {
        self.writer = Some(writer);
        self
    }
}

impl<R: Reader, W: Writer> SparseIOBuilder<R, W> {
    pub fn build(self) -> Result<SparseIO<R, W>> {
        // Required fields
        let len = self
            .len
            .ok_or_else(|| Error::Other("SparseIOBuilder is missing required field: len".to_string()))?;
        let reader = self
            .reader
            .ok_or_else(|| Error::Other("SparseIOBuilder is missing required field: reader".to_string()))?;
        let writer = self
            .writer
            .ok_or_else(|| Error::Other("SparseIOBuilder is missing required field: writer".to_string()))?;

        // Optional fields
        if self.chunk_size == 0 {
            return Err(Error::Other("SparseIOBuilder field chunk_size must be greater than zero".to_string()));
        }

        Ok(SparseIO {
            len,
            chunk_size: self.chunk_size,
            reader,
            coverage: Arc::new(Mutex::new(Coverage::new())),
            writer: Arc::new(Mutex::new(writer)),
            flights: Arc::new(Mutex::new(HashMap::new())),
        })
    }
}

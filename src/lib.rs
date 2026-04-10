//! SparseIO - A Rust library for efficient sparse I/O operations. This library provides an interface to define how to
//! efficiently read from and write to sparse objects, while tracking cache coverage and in-flight operations to
//! optimize performance and resource usage in order to deduplicate work done across multiple I/O operations.

mod coverage;
#[cfg(feature = "debug")]
pub mod debug;
mod reader;
mod shared;
pub mod sources;
#[cfg(all(any(test, feature = "utils"), not(docsrs)))]
pub mod utils;
mod viewer;
mod writer;

use std::collections::HashMap;
use std::io::{Error, ErrorKind, Result};
use std::sync::Arc;

use bytes::Bytes;
use futures::FutureExt;
pub use reader::Reader;
use tokio::sync::Mutex;
pub use viewer::Viewer;
pub use writer::Writer;

use crate::coverage::Coverage;
use crate::shared::{DEFAULT_CHUNK_SIZE, SharedChunk};

/// User-facing API for sparse I/O operations. Provides an interface to read from a sparse object, which is backed by
/// an [`crate::Writer`] for storing filled in extents, and a [`crate::Reader`] for the actual I/O operations.
pub struct SparseIO<R: Reader, W: Writer> {
    // User controllable args
    chunk_size: usize,

    // Internal states
    len: usize,

    /// Internal data structures
    coverage: Mutex<Coverage>,
    flights: Arc<Mutex<HashMap<usize, SharedChunk>>>,
    writer: Mutex<W>,
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

    /// Fully internal function for managing the flow from Read -> Store -> Update Coverage.
    async fn fetch_and_store_chunk(self: Arc<Self>, offset: usize) -> Result<Bytes> {
        let chunk_data = self.fetch_chunk_from_source(offset).await?;

        // Write to store and update coverage
        self.writer.lock().await.create_extent(offset, chunk_data.clone()).await?;
        self.coverage.lock().await.insert(offset, chunk_data.len());

        Ok(chunk_data)
    }

    /// Returns an existing in-flight chunk future for `offset`, or creates one.
    async fn get_or_create_flight(self: &Arc<Self>, offset: usize) -> SharedChunk {
        let mut flights = self.flights.lock().await;
        if let Some(flight) = flights.get(&offset) {
            return flight.clone();
        }

        let io = self.clone();
        let shared = io
            .fetch_and_store_chunk(offset)
            .map(|res| res.map_err(|err| err.to_string()))
            .boxed()
            .shared();
        flights.insert(offset, shared.clone());
        shared
    }

    /// Internal function to read a chunk of data at the given offset. This function will first check the coverage
    /// to see if the chunk is already filled in, and if so, will read from the store. If not, it will fetch the chunk
    /// from the source, write it to the store, update coverage, and then return the data.
    ///
    /// Leveraged by the reader to construct a stream, extract a chunk of data for a read, etc.
    pub async fn read_chunk(self: &Arc<Self>, offset: usize) -> Result<Bytes> {
        // Check coverage to see if chunk is filled in
        let offset_norm = self.normalize_offset(offset);
        if offset_norm >= self.len() {
            return Err(Error::new(ErrorKind::UnexpectedEof, "range not satisfied"));
        }

        if let Some((_, end)) = self.coverage.lock().await.get(offset_norm) {
            if offset_norm < end {
                // Chunk is filled in, read from store
                return self.writer.lock().await.read_extent(offset_norm).await;
            }
        }

        // Chunk is not filled in, fetch it once and share in-flight work across concurrent callers.
        let flight = self.get_or_create_flight(offset_norm).await;
        let chunk_data = flight.await.map_err(Error::other);
        self.flights.lock().await.remove(&offset_norm);
        Ok(chunk_data?)
    }
}

impl<R: Reader, W: Writer> SparseIO<R, W> {
    /// Returns a new [`crate::Builder`] to construct a [`crate::SparseIO`] instance.
    pub fn builder() -> Builder<R, W> {
        Builder::new()
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

/// Builder pattern for constructing a [`crate::SparseIO`] instance. This allows callers to specify required
/// and optional fields, and provides validation before constructing the final instance.
pub struct Builder<R, W> {
    chunk_size: usize,
    reader: Option<R>,
    writer: Option<W>,
}

impl<R, W> Default for Builder<R, W> {
    fn default() -> Self {
        Self {
            chunk_size: DEFAULT_CHUNK_SIZE,
            reader: None,
            writer: None,
        }
    }
}

impl<R, W> Builder<R, W> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the chunk size for the sparse object. This is an optional field, and
    /// defaults to 128KB.
    pub fn chunk_size(mut self, chunk_size: usize) -> Self {
        self.chunk_size = chunk_size;
        self
    }

    /// Sets the source reader for the sparse object. This is a required field.
    /// The reader defines how the sparse I/O library fetches data from the source,
    /// and can either be constructed manually from the [`crate::Reader`] trait,
    /// or from an implementation defined in the [`crate::sources`] module.
    pub fn reader(mut self, reader: R) -> Self {
        self.reader = Some(reader);
        self
    }

    /// Sets the extent store for the sparse object. This is a required field.
    /// The writer defines how the sparse I/O library stores data to the destination for
    /// caching. It can either be constructed manually from the [`crate::Writer`] trait,
    /// or from an implementation defined in the [`crate::sources`] module.
    pub fn writer(mut self, writer: W) -> Self {
        self.writer = Some(writer);
        self
    }
}

impl<R: Reader, W: Writer> Builder<R, W> {
    pub async fn build(self) -> Result<SparseIO<R, W>> {
        // Required fields
        let reader = self
            .reader
            .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "builder missing required field: reader"))?;
        let writer = self
            .writer
            .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "builder missing required field: writer"))?;

        // Optional fields
        if self.chunk_size == 0 {
            return Err(Error::new(ErrorKind::InvalidInput, "builder field chunk_size must be greater than zero"));
        }

        let len = reader.len().await?;

        Ok(SparseIO {
            chunk_size: self.chunk_size,
            len,
            reader,
            coverage: Mutex::new(Coverage::new()),
            flights: Arc::new(Mutex::new(HashMap::new())),
            writer: Mutex::new(writer),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::counting::Reader as CountingReader;
    use crate::utils::oracle;

    /// This test pins the cached-length behavior so repeated calls do not
    /// re-query the source after the initial build.
    #[tokio::test]
    async fn len_is_cached_by_default() {
        let data = vec![1u8; 128];
        let reader = CountingReader::new(oracle::Reader::new(Bytes::from(data)));
        let writer = oracle::Writer::default();

        let io = Builder::new()
            .reader(reader.clone())
            .writer(writer)
            .build()
            .await
            .expect("builder should succeed");

        assert_eq!(io.len(), 128);
        assert_eq!(io.len(), 128);
        assert_eq!(reader.len_read_count(), 1, "len should be computed once when cache is enabled");
    }

    /// This test ensures the builder performs the initial length probe
    /// exactly once and stores the result for later use.
    #[tokio::test]
    async fn len_is_prefetched_during_build() {
        let data = vec![1u8; 128];
        let reader = CountingReader::new(oracle::Reader::new(Bytes::from(data)));
        let writer = oracle::Writer::default();

        let io = Builder::new()
            .reader(reader.clone())
            .writer(writer)
            .build()
            .await
            .expect("builder should succeed");

        assert_eq!(reader.len_read_count(), 1, "build should fetch length exactly once");
        assert_eq!(io.len(), 128);
        assert_eq!(io.len(), 128);
        assert_eq!(reader.len_read_count(), 1, "len() should not trigger extra source reads");
    }
}
